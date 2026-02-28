//! Quantized storage for warm and archive tiers.
//!
//! Provides storage for quantized KV cache entries with support for
//! multiple quantization strategies (KIVI, SQuat, KVQuant).

#[cfg(feature = "no_std_gateway")]
use alloc::{vec, vec::Vec};

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

use super::kivi::{KiviQuantizer, QuantScheme, QuantizedKV};
use super::tier::CacheTier;

/// A single quantized entry in the store
#[derive(Debug, Clone)]
pub struct QuantizedEntry {
    /// Quantized key data
    pub key: QuantizedKV,
    /// Quantized value data
    pub value: QuantizedKV,
    /// Original position in sequence
    pub position: usize,
    /// Which tier this entry belongs to
    pub tier: CacheTier,
}

/// Dequantized KV pair (scratch buffer for attention computation)
#[derive(Debug, Clone)]
pub struct DequantizedKV {
    /// Dequantized keys: [seq_len, head_dim]
    pub keys: Vec<f32>,
    /// Dequantized values: [seq_len, head_dim]
    pub values: Vec<f32>,
    /// Number of tokens
    pub len: usize,
}

impl DequantizedKV {
    /// Create empty dequantized buffer
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
            len: 0,
        }
    }

    /// Create with pre-allocated capacity
    pub fn with_capacity(capacity: usize, head_dim: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity * head_dim),
            values: Vec::with_capacity(capacity * head_dim),
            len: 0,
        }
    }

    /// Clear the buffer for reuse
    pub fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
        self.len = 0;
    }
}

impl Default for DequantizedKV {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for quantized store
#[derive(Debug, Clone, Copy)]
pub struct QuantizedStoreConfig {
    /// Number of layers
    pub num_layers: usize,
    /// Number of attention heads per layer
    pub num_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
    /// Maximum tokens in warm tier
    pub warm_capacity: usize,
    /// Maximum tokens in archive tier (0 = unlimited)
    pub archive_capacity: usize,
    /// Bits for warm tier quantization
    pub warm_bits: u8,
    /// Bits for archive tier quantization
    pub archive_bits: u8,
}

impl QuantizedStoreConfig {
    /// Estimate memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        let warm_bytes_per_token = (self.head_dim * self.warm_bits as usize + 7) / 8;
        let archive_bytes_per_token = (self.head_dim * self.archive_bits as usize + 7) / 8;

        let warm_total =
            self.num_layers * self.num_heads * self.warm_capacity * warm_bytes_per_token * 2; // keys + values

        let archive_total =
            self.num_layers * self.num_heads * self.archive_capacity * archive_bytes_per_token * 2;

        // Add scale overhead (8 bytes per token for min/max)
        let scale_overhead = (self.warm_capacity + self.archive_capacity) * 8 * self.num_layers;

        warm_total + archive_total + scale_overhead
    }
}

/// Quantized storage for warm and archive tiers
///
/// Maintains two separate zones:
/// - Warm zone: 4-bit KIVI quantization
/// - Archive zone: 2-bit KIVI/SQuat quantization
pub struct QuantizedStore {
    /// Configuration
    config: QuantizedStoreConfig,

    /// Warm tier entries: [layers][heads]
    warm_keys: Vec<Vec<Vec<u8>>>,
    warm_values: Vec<Vec<Vec<u8>>>,
    warm_key_scales: Vec<Vec<Vec<(f32, f32)>>>,
    warm_value_scales: Vec<Vec<Vec<(f32, f32)>>>,
    warm_len: Vec<usize>,

    /// Archive tier entries: [layers][heads]
    archive_keys: Vec<Vec<Vec<u8>>>,
    archive_values: Vec<Vec<Vec<u8>>>,
    archive_key_scales: Vec<Vec<Vec<(f32, f32)>>>,
    archive_value_scales: Vec<Vec<Vec<(f32, f32)>>>,
    archive_len: Vec<usize>,

    /// KIVI quantizers
    warm_quantizer: KiviQuantizer,
    archive_quantizer: KiviQuantizer,

    /// Scratch buffers for dequantization (per layer)
    scratch: Vec<DequantizedKV>,
}

impl QuantizedStore {
    /// Create a new quantized store
    pub fn new(config: QuantizedStoreConfig) -> Self {
        let warm_bytes_per_token = (config.head_dim * config.warm_bits as usize + 7) / 8;
        let archive_bytes_per_token = (config.head_dim * config.archive_bits as usize + 7) / 8;

        let mut warm_keys = Vec::with_capacity(config.num_layers);
        let mut warm_values = Vec::with_capacity(config.num_layers);
        let mut warm_key_scales = Vec::with_capacity(config.num_layers);
        let mut warm_value_scales = Vec::with_capacity(config.num_layers);

        let mut archive_keys = Vec::with_capacity(config.num_layers);
        let mut archive_values = Vec::with_capacity(config.num_layers);
        let mut archive_key_scales = Vec::with_capacity(config.num_layers);
        let mut archive_value_scales = Vec::with_capacity(config.num_layers);

        for _ in 0..config.num_layers {
            let mut layer_warm_keys = Vec::with_capacity(config.num_heads);
            let mut layer_warm_values = Vec::with_capacity(config.num_heads);
            let mut layer_warm_key_scales = Vec::with_capacity(config.num_heads);
            let mut layer_warm_value_scales = Vec::with_capacity(config.num_heads);

            let mut layer_archive_keys = Vec::with_capacity(config.num_heads);
            let mut layer_archive_values = Vec::with_capacity(config.num_heads);
            let mut layer_archive_key_scales = Vec::with_capacity(config.num_heads);
            let mut layer_archive_value_scales = Vec::with_capacity(config.num_heads);

            for _ in 0..config.num_heads {
                layer_warm_keys.push(vec![0u8; config.warm_capacity * warm_bytes_per_token]);
                layer_warm_values.push(vec![0u8; config.warm_capacity * warm_bytes_per_token]);
                layer_warm_key_scales.push(vec![(0.0f32, 0.0f32); config.warm_capacity]);
                layer_warm_value_scales.push(vec![(0.0f32, 0.0f32); config.warm_capacity]);

                layer_archive_keys
                    .push(vec![0u8; config.archive_capacity * archive_bytes_per_token]);
                layer_archive_values
                    .push(vec![0u8; config.archive_capacity * archive_bytes_per_token]);
                layer_archive_key_scales.push(vec![(0.0f32, 0.0f32); config.archive_capacity]);
                layer_archive_value_scales.push(vec![(0.0f32, 0.0f32); config.archive_capacity]);
            }

            warm_keys.push(layer_warm_keys);
            warm_values.push(layer_warm_values);
            warm_key_scales.push(layer_warm_key_scales);
            warm_value_scales.push(layer_warm_value_scales);

            archive_keys.push(layer_archive_keys);
            archive_values.push(layer_archive_values);
            archive_key_scales.push(layer_archive_key_scales);
            archive_value_scales.push(layer_archive_value_scales);
        }

        let scratch = (0..config.num_layers)
            .map(|_| {
                DequantizedKV::with_capacity(
                    config.warm_capacity + config.archive_capacity,
                    config.head_dim,
                )
            })
            .collect();

        Self {
            config,
            warm_keys,
            warm_values,
            warm_key_scales,
            warm_value_scales,
            warm_len: vec![0; config.num_layers],
            archive_keys,
            archive_values,
            archive_key_scales,
            archive_value_scales,
            archive_len: vec![0; config.num_layers],
            warm_quantizer: KiviQuantizer::new(config.warm_bits, config.head_dim),
            archive_quantizer: KiviQuantizer::new(config.archive_bits, config.head_dim),
            scratch,
        }
    }

    /// Push a KV pair to the warm tier
    pub fn push_warm(&mut self, layer: usize, head: usize, key: &[f32], value: &[f32]) {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);
        assert_eq!(key.len(), self.config.head_dim);
        assert_eq!(value.len(), self.config.head_dim);

        let pos = self.warm_len[layer];
        if pos >= self.config.warm_capacity {
            // Warm is full, need to graduate to archive first
            return;
        }

        // Quantize key with per-channel scheme
        let (key_q, key_min, key_max) = self.warm_quantizer.quantize(key, QuantScheme::PerChannel);
        // Quantize value with per-token scheme
        let (value_q, value_min, value_max) =
            self.warm_quantizer.quantize(value, QuantScheme::PerToken);

        // Store quantized data
        let bytes_per_token = (self.config.head_dim * self.config.warm_bits as usize + 7) / 8;
        let offset = pos * bytes_per_token;

        self.warm_keys[layer][head][offset..offset + key_q.len()].copy_from_slice(&key_q);
        self.warm_values[layer][head][offset..offset + value_q.len()].copy_from_slice(&value_q);
        self.warm_key_scales[layer][head][pos] = (key_min, key_max);
        self.warm_value_scales[layer][head][pos] = (value_min, value_max);

        self.warm_len[layer] = pos + 1;
    }

    /// Graduate oldest warm entries to archive
    ///
    /// Moves `count` oldest entries from warm to archive tier
    pub fn graduate_to_archive(&mut self, layer: usize, count: usize) {
        if count == 0 || self.warm_len[layer] == 0 {
            return;
        }

        let actual_count = count.min(self.warm_len[layer]);
        let warm_bytes = (self.config.head_dim * self.config.warm_bits as usize + 7) / 8;
        let archive_bytes = (self.config.head_dim * self.config.archive_bits as usize + 7) / 8;

        for head in 0..self.config.num_heads {
            for i in 0..actual_count {
                let archive_pos = self.archive_len[layer] + i;
                if archive_pos >= self.config.archive_capacity {
                    break;
                }

                // Get warm entry
                let warm_offset = i * warm_bytes;
                let warm_key = &self.warm_keys[layer][head][warm_offset..warm_offset + warm_bytes];
                let warm_value =
                    &self.warm_values[layer][head][warm_offset..warm_offset + warm_bytes];
                let (key_min, key_max) = self.warm_key_scales[layer][head][i];
                let (value_min, value_max) = self.warm_value_scales[layer][head][i];

                // Dequantize from warm
                let key_fp32 = self.warm_quantizer.dequantize(warm_key, key_min, key_max);
                let value_fp32 = self
                    .warm_quantizer
                    .dequantize(warm_value, value_min, value_max);

                // Re-quantize for archive (more aggressive)
                let (archive_key, ak_min, ak_max) = self
                    .archive_quantizer
                    .quantize(&key_fp32, QuantScheme::PerChannel);
                let (archive_value, av_min, av_max) = self
                    .archive_quantizer
                    .quantize(&value_fp32, QuantScheme::PerToken);

                // Store in archive
                let archive_offset = archive_pos * archive_bytes;
                self.archive_keys[layer][head][archive_offset..archive_offset + archive_key.len()]
                    .copy_from_slice(&archive_key);
                self.archive_values[layer][head]
                    [archive_offset..archive_offset + archive_value.len()]
                    .copy_from_slice(&archive_value);
                self.archive_key_scales[layer][head][archive_pos] = (ak_min, ak_max);
                self.archive_value_scales[layer][head][archive_pos] = (av_min, av_max);
            }
        }

        // Update archive length
        let graduated = actual_count.min(self.config.archive_capacity - self.archive_len[layer]);
        self.archive_len[layer] += graduated;

        // Shift warm entries
        self.shift_warm(layer, actual_count);
    }

    /// Shift warm entries left after graduation
    fn shift_warm(&mut self, layer: usize, count: usize) {
        if count >= self.warm_len[layer] {
            self.warm_len[layer] = 0;
            return;
        }

        let bytes = (self.config.head_dim * self.config.warm_bits as usize + 7) / 8;
        let remaining = self.warm_len[layer] - count;

        for head in 0..self.config.num_heads {
            // Shift data
            let src_start = count * bytes;
            let data_len = remaining * bytes;
            self.warm_keys[layer][head].copy_within(src_start..src_start + data_len, 0);
            self.warm_values[layer][head].copy_within(src_start..src_start + data_len, 0);

            // Shift scales
            for i in 0..remaining {
                self.warm_key_scales[layer][head][i] = self.warm_key_scales[layer][head][i + count];
                self.warm_value_scales[layer][head][i] =
                    self.warm_value_scales[layer][head][i + count];
            }
        }

        self.warm_len[layer] = remaining;
    }

    /// Dequantize warm keys for a layer/head
    pub fn dequantize_warm_keys(&self, layer: usize, head: usize) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);

        let bytes = (self.config.head_dim * self.config.warm_bits as usize + 7) / 8;
        let mut result = Vec::with_capacity(self.warm_len[layer] * self.config.head_dim);

        for i in 0..self.warm_len[layer] {
            let offset = i * bytes;
            let data = &self.warm_keys[layer][head][offset..offset + bytes];
            let (min_val, max_val) = self.warm_key_scales[layer][head][i];
            let dequant = self.warm_quantizer.dequantize(data, min_val, max_val);
            result.extend_from_slice(&dequant);
        }

        result
    }

    /// Dequantize warm values for a layer/head
    pub fn dequantize_warm_values(&self, layer: usize, head: usize) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);

        let bytes = (self.config.head_dim * self.config.warm_bits as usize + 7) / 8;
        let mut result = Vec::with_capacity(self.warm_len[layer] * self.config.head_dim);

        for i in 0..self.warm_len[layer] {
            let offset = i * bytes;
            let data = &self.warm_values[layer][head][offset..offset + bytes];
            let (min_val, max_val) = self.warm_value_scales[layer][head][i];
            let dequant = self.warm_quantizer.dequantize(data, min_val, max_val);
            result.extend_from_slice(&dequant);
        }

        result
    }

    /// Dequantize archive keys for a layer/head
    pub fn dequantize_archive_keys(&self, layer: usize, head: usize) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);

        let bytes = (self.config.head_dim * self.config.archive_bits as usize + 7) / 8;
        let mut result = Vec::with_capacity(self.archive_len[layer] * self.config.head_dim);

        for i in 0..self.archive_len[layer] {
            let offset = i * bytes;
            let data = &self.archive_keys[layer][head][offset..offset + bytes];
            let (min_val, max_val) = self.archive_key_scales[layer][head][i];
            let dequant = self.archive_quantizer.dequantize(data, min_val, max_val);
            result.extend_from_slice(&dequant);
        }

        result
    }

    /// Dequantize archive values for a layer/head
    pub fn dequantize_archive_values(&self, layer: usize, head: usize) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);

        let bytes = (self.config.head_dim * self.config.archive_bits as usize + 7) / 8;
        let mut result = Vec::with_capacity(self.archive_len[layer] * self.config.head_dim);

        for i in 0..self.archive_len[layer] {
            let offset = i * bytes;
            let data = &self.archive_values[layer][head][offset..offset + bytes];
            let (min_val, max_val) = self.archive_value_scales[layer][head][i];
            let dequant = self.archive_quantizer.dequantize(data, min_val, max_val);
            result.extend_from_slice(&dequant);
        }

        result
    }

    /// Get length of warm tier for a layer
    #[inline]
    pub fn warm_len(&self, layer: usize) -> usize {
        self.warm_len[layer]
    }

    /// Get length of archive tier for a layer
    #[inline]
    pub fn archive_len(&self, layer: usize) -> usize {
        self.archive_len[layer]
    }

    /// Get total quantized entries for a layer
    #[inline]
    pub fn total_len(&self, layer: usize) -> usize {
        self.warm_len[layer] + self.archive_len[layer]
    }

    /// Check if warm tier is full for a layer
    #[inline]
    pub fn warm_is_full(&self, layer: usize) -> bool {
        self.warm_len[layer] >= self.config.warm_capacity
    }

    /// Get configuration
    #[inline]
    pub fn config(&self) -> &QuantizedStoreConfig {
        &self.config
    }

    /// Reset store for a layer
    pub fn reset_layer(&mut self, layer: usize) {
        self.warm_len[layer] = 0;
        self.archive_len[layer] = 0;
        self.scratch[layer].clear();
    }

    /// Reset entire store
    pub fn reset(&mut self) {
        for layer in 0..self.config.num_layers {
            self.reset_layer(layer);
        }
    }

    /// Total memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.config.memory_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantized_store_config() {
        let config = QuantizedStoreConfig {
            num_layers: 12,
            num_heads: 8,
            head_dim: 64,
            warm_capacity: 448,
            archive_capacity: 2048,
            warm_bits: 4,
            archive_bits: 2,
        };

        // Just verify it computes without panic
        let _bytes = config.memory_bytes();
    }

    #[test]
    fn test_quantized_store_push_warm() {
        let config = QuantizedStoreConfig {
            num_layers: 1,
            num_heads: 1,
            head_dim: 8,
            warm_capacity: 4,
            archive_capacity: 8,
            warm_bits: 4,
            archive_bits: 2,
        };

        let mut store = QuantizedStore::new(config);

        let key: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let value: Vec<f32> = (0..8).map(|i| (7 - i) as f32).collect();

        store.push_warm(0, 0, &key, &value);
        assert_eq!(store.warm_len(0), 1);

        store.push_warm(0, 0, &key, &value);
        assert_eq!(store.warm_len(0), 2);
    }

    #[test]
    fn test_dequantized_kv() {
        let mut kv = DequantizedKV::with_capacity(10, 64);
        assert_eq!(kv.len, 0);

        kv.keys.extend_from_slice(&[1.0, 2.0, 3.0]);
        kv.len = 1;

        kv.clear();
        assert_eq!(kv.len, 0);
        assert!(kv.keys.is_empty());
    }
}
