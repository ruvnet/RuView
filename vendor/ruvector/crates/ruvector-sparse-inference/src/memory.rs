//! Memory management for sparse inference.
//!
//! This module provides weight quantization and neuron caching for efficient
//! memory usage during inference.

use crate::config::CacheConfig;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Quantized weight storage for reduced memory usage.
///
/// Stores neural network weights in a compressed format to reduce
/// memory footprint while maintaining accuracy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedWeights {
    /// Quantized weight data (packed bits)
    data: Vec<u8>,
    /// Scale factors per group
    scales: Vec<f32>,
    /// Zero points per group
    zero_points: Vec<f32>,
    /// Group size for quantization
    group_size: usize,
    /// Original dimensions
    shape: (usize, usize),
    /// Quantization bit width
    bits: u8,
}

impl QuantizedWeights {
    /// Create new quantized weights from f32 data.
    pub fn from_f32(
        data: &[f32],
        rows: usize,
        cols: usize,
        bits: u8,
        group_size: usize,
    ) -> Result<Self> {
        assert!(
            bits == 4 || bits == 8,
            "Only 4-bit and 8-bit quantization supported"
        );

        let num_groups = (data.len() + group_size - 1) / group_size;
        let mut scales = Vec::with_capacity(num_groups);
        let mut zero_points = Vec::with_capacity(num_groups);

        // Calculate per-group scales and zero points
        for group in data.chunks(group_size) {
            let min = group.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max = group.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

            let range = max - min;
            let max_quant = ((1 << bits) - 1) as f32;

            let scale = if range > 0.0 { range / max_quant } else { 1.0 };
            scales.push(scale);
            zero_points.push(min);
        }

        // Quantize the data
        let quantized_data = if bits == 8 {
            data.chunks(group_size)
                .zip(scales.iter().zip(zero_points.iter()))
                .flat_map(|(group, (&scale, &zp))| {
                    group
                        .iter()
                        .map(move |&v| ((v - zp) / scale).round().clamp(0.0, 255.0) as u8)
                })
                .collect()
        } else {
            // 4-bit: pack two values per byte
            let mut packed = Vec::with_capacity((data.len() + 1) / 2);
            let quantized: Vec<u8> = data
                .chunks(group_size)
                .zip(scales.iter().zip(zero_points.iter()))
                .flat_map(|(group, (&scale, &zp))| {
                    group
                        .iter()
                        .map(move |&v| ((v - zp) / scale).round().clamp(0.0, 15.0) as u8)
                })
                .collect();

            for pair in quantized.chunks(2) {
                let byte = pair[0] | (pair.get(1).unwrap_or(&0) << 4);
                packed.push(byte);
            }
            packed
        };

        Ok(Self {
            data: quantized_data,
            scales,
            zero_points,
            group_size,
            shape: (rows, cols),
            bits,
        })
    }

    /// Dequantize to f32.
    pub fn to_f32(&self) -> Vec<f32> {
        let total = self.shape.0 * self.shape.1;
        let mut result = Vec::with_capacity(total);

        if self.bits == 8 {
            for (i, &q) in self.data.iter().take(total).enumerate() {
                let group_idx = i / self.group_size;
                let scale = self.scales[group_idx];
                let zp = self.zero_points[group_idx];
                result.push(q as f32 * scale + zp);
            }
        } else {
            // 4-bit unpacking
            for (i, &byte) in self.data.iter().enumerate() {
                let idx = i * 2;
                if idx < total {
                    let group_idx = idx / self.group_size;
                    let scale = self.scales[group_idx];
                    let zp = self.zero_points[group_idx];
                    result.push((byte & 0x0F) as f32 * scale + zp);
                }
                if idx + 1 < total {
                    let group_idx = (idx + 1) / self.group_size;
                    let scale = self.scales[group_idx];
                    let zp = self.zero_points[group_idx];
                    result.push((byte >> 4) as f32 * scale + zp);
                }
            }
        }

        result
    }

    /// Get shape.
    pub fn shape(&self) -> (usize, usize) {
        self.shape
    }

    /// Get memory size in bytes.
    pub fn memory_size(&self) -> usize {
        self.data.len() + self.scales.len() * 4 + self.zero_points.len() * 4
    }
}

/// Neuron activation cache for hot/cold management.
///
/// Tracks neuron activation frequencies and maintains a cache of
/// frequently accessed ("hot") neuron weights.
#[derive(Debug, Clone)]
pub struct NeuronCache {
    /// Activation counts per neuron
    activation_counts: Vec<u64>,
    /// Hot neuron indices (frequently activated)
    hot_neurons: Vec<usize>,
    /// Cold neuron indices (rarely activated)
    cold_neurons: Vec<usize>,
    /// Threshold for hot classification
    hot_threshold: f64,
    /// Total activations tracked
    total_activations: u64,
    /// Number of neurons
    num_neurons: usize,
}

impl NeuronCache {
    /// Create a new neuron cache from config.
    pub fn new(num_neurons: usize, config: CacheConfig) -> Self {
        Self {
            activation_counts: vec![0; num_neurons],
            hot_neurons: Vec::new(),
            cold_neurons: (0..num_neurons).collect(),
            hot_threshold: config.hot_neuron_fraction as f64,
            total_activations: 0,
            num_neurons,
        }
    }

    /// Create a new neuron cache with explicit threshold.
    pub fn with_threshold(num_neurons: usize, hot_threshold: f64) -> Self {
        Self {
            activation_counts: vec![0; num_neurons],
            hot_neurons: Vec::new(),
            cold_neurons: (0..num_neurons).collect(),
            hot_threshold,
            total_activations: 0,
            num_neurons,
        }
    }

    /// Clear all cache state and reset counters.
    pub fn clear(&mut self) {
        self.activation_counts.fill(0);
        self.hot_neurons.clear();
        self.cold_neurons = (0..self.num_neurons).collect();
        self.total_activations = 0;
    }

    /// Record neuron activations.
    pub fn record_activations(&mut self, active_neurons: &[usize]) {
        for &neuron in active_neurons {
            if neuron < self.activation_counts.len() {
                self.activation_counts[neuron] += 1;
            }
        }
        self.total_activations += 1;

        // Periodically reclassify
        if self.total_activations % 1000 == 0 {
            self.reclassify();
        }
    }

    /// Reclassify neurons as hot or cold.
    pub fn reclassify(&mut self) {
        if self.total_activations == 0 {
            return;
        }

        let threshold = (self.total_activations as f64 * self.hot_threshold) as u64;

        self.hot_neurons.clear();
        self.cold_neurons.clear();

        for (i, &count) in self.activation_counts.iter().enumerate() {
            if count >= threshold {
                self.hot_neurons.push(i);
            } else {
                self.cold_neurons.push(i);
            }
        }
    }

    /// Get hot neurons.
    pub fn hot_neurons(&self) -> &[usize] {
        &self.hot_neurons
    }

    /// Get cold neurons.
    pub fn cold_neurons(&self) -> &[usize] {
        &self.cold_neurons
    }

    /// Get activation frequency for a neuron.
    pub fn activation_frequency(&self, neuron: usize) -> f64 {
        if self.total_activations == 0 || neuron >= self.activation_counts.len() {
            return 0.0;
        }
        self.activation_counts[neuron] as f64 / self.total_activations as f64
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            num_hot: self.hot_neurons.len(),
            num_cold: self.cold_neurons.len(),
            total_activations: self.total_activations,
            hot_ratio: self.hot_neurons.len() as f64 / self.activation_counts.len() as f64,
        }
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of hot neurons.
    pub num_hot: usize,
    /// Number of cold neurons.
    pub num_cold: usize,
    /// Total activations tracked.
    pub total_activations: u64,
    /// Ratio of hot neurons.
    pub hot_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantized_weights_8bit() {
        let data: Vec<f32> = (0..256).map(|i| i as f32 / 256.0).collect();
        let qw = QuantizedWeights::from_f32(&data, 16, 16, 8, 32).unwrap();

        let restored = qw.to_f32();
        assert_eq!(restored.len(), 256);

        // Check reconstruction error
        let max_error: f32 = data
            .iter()
            .zip(restored.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(max_error < 0.01, "Max error: {}", max_error);
    }

    #[test]
    fn test_quantized_weights_4bit() {
        let data: Vec<f32> = (0..256).map(|i| i as f32 / 256.0).collect();
        let qw = QuantizedWeights::from_f32(&data, 16, 16, 4, 32).unwrap();

        let restored = qw.to_f32();
        assert_eq!(restored.len(), 256);

        // 4-bit has more error
        let max_error: f32 = data
            .iter()
            .zip(restored.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(max_error < 0.1, "Max error: {}", max_error);
    }

    #[test]
    fn test_neuron_cache() {
        let mut cache = NeuronCache::with_threshold(100, 0.1);

        // Activate some neurons frequently
        for _ in 0..1000 {
            cache.record_activations(&[0, 1, 2, 3, 4]);
        }

        cache.reclassify();

        assert!(cache.hot_neurons().contains(&0));
        assert!(cache.hot_neurons().contains(&1));
        assert!(!cache.hot_neurons().contains(&50));
    }
}
