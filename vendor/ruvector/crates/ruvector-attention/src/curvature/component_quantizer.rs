//! Component Quantization for Mixed-Curvature Attention
//!
//! Different precision for each geometric component:
//! - Euclidean: 7-8 bit (needs precision)
//! - Hyperbolic tangent: 5 bit (tolerates noise)
//! - Spherical: 5 bit (only direction matters)

use serde::{Deserialize, Serialize};

/// Quantization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    /// Bits for Euclidean component
    pub euclidean_bits: u8,
    /// Bits for Hyperbolic component
    pub hyperbolic_bits: u8,
    /// Bits for Spherical component
    pub spherical_bits: u8,
}

impl Default for QuantizationConfig {
    fn default() -> Self {
        Self {
            euclidean_bits: 8,
            hyperbolic_bits: 5,
            spherical_bits: 5,
        }
    }
}

/// Quantized vector representation
#[derive(Debug, Clone)]
pub struct QuantizedVector {
    /// Quantized Euclidean component
    pub euclidean: Vec<i8>,
    /// Euclidean scale factor
    pub euclidean_scale: f32,
    /// Quantized Hyperbolic component
    pub hyperbolic: Vec<i8>,
    /// Hyperbolic scale factor
    pub hyperbolic_scale: f32,
    /// Quantized Spherical component
    pub spherical: Vec<i8>,
    /// Spherical scale factor
    pub spherical_scale: f32,
}

/// Component quantizer for efficient storage and compute
#[derive(Debug, Clone)]
pub struct ComponentQuantizer {
    config: QuantizationConfig,
    euclidean_levels: i32,
    hyperbolic_levels: i32,
    spherical_levels: i32,
}

impl ComponentQuantizer {
    /// Create new quantizer
    pub fn new(config: QuantizationConfig) -> Self {
        Self {
            euclidean_levels: (1 << (config.euclidean_bits - 1)) - 1,
            hyperbolic_levels: (1 << (config.hyperbolic_bits - 1)) - 1,
            spherical_levels: (1 << (config.spherical_bits - 1)) - 1,
            config,
        }
    }

    /// Quantize a component vector
    fn quantize_component(&self, values: &[f32], levels: i32) -> (Vec<i8>, f32) {
        if values.is_empty() {
            return (vec![], 1.0);
        }

        // Find absmax for scale
        let absmax = values
            .iter()
            .map(|v| v.abs())
            .fold(0.0f32, f32::max)
            .max(1e-8);

        let scale = absmax / levels as f32;
        let inv_scale = levels as f32 / absmax;

        let quantized: Vec<i8> = values
            .iter()
            .map(|v| (v * inv_scale).round().clamp(-127.0, 127.0) as i8)
            .collect();

        (quantized, scale)
    }

    /// Dequantize a component
    fn dequantize_component(&self, quantized: &[i8], scale: f32) -> Vec<f32> {
        quantized.iter().map(|&q| q as f32 * scale).collect()
    }

    /// Quantize full vector with component ranges
    pub fn quantize(
        &self,
        vector: &[f32],
        e_range: std::ops::Range<usize>,
        h_range: std::ops::Range<usize>,
        s_range: std::ops::Range<usize>,
    ) -> QuantizedVector {
        let (euclidean, euclidean_scale) =
            self.quantize_component(&vector[e_range], self.euclidean_levels);

        let (hyperbolic, hyperbolic_scale) =
            self.quantize_component(&vector[h_range], self.hyperbolic_levels);

        let (spherical, spherical_scale) =
            self.quantize_component(&vector[s_range], self.spherical_levels);

        QuantizedVector {
            euclidean,
            euclidean_scale,
            hyperbolic,
            hyperbolic_scale,
            spherical,
            spherical_scale,
        }
    }

    /// Compute dot product between quantized vectors (integer arithmetic)
    #[inline]
    pub fn quantized_dot_product(
        &self,
        a: &QuantizedVector,
        b: &QuantizedVector,
        weights: &[f32; 3],
    ) -> f32 {
        // Integer dot products
        let dot_e = Self::int_dot(&a.euclidean, &b.euclidean);
        let dot_h = Self::int_dot(&a.hyperbolic, &b.hyperbolic);
        let dot_s = Self::int_dot(&a.spherical, &b.spherical);

        // Scale and weight
        let sim_e = dot_e as f32 * a.euclidean_scale * b.euclidean_scale;
        let sim_h = dot_h as f32 * a.hyperbolic_scale * b.hyperbolic_scale;
        let sim_s = dot_s as f32 * a.spherical_scale * b.spherical_scale;

        weights[0] * sim_e + weights[1] * sim_h + weights[2] * sim_s
    }

    /// Integer dot product (SIMD-friendly)
    #[inline(always)]
    fn int_dot(a: &[i8], b: &[i8]) -> i32 {
        let len = a.len().min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0i32;
        let mut sum1 = 0i32;
        let mut sum2 = 0i32;
        let mut sum3 = 0i32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += a[base] as i32 * b[base] as i32;
            sum1 += a[base + 1] as i32 * b[base + 1] as i32;
            sum2 += a[base + 2] as i32 * b[base + 2] as i32;
            sum3 += a[base + 3] as i32 * b[base + 3] as i32;
        }

        let base = chunks * 4;
        for i in 0..remainder {
            sum0 += a[base + i] as i32 * b[base + i] as i32;
        }

        sum0 + sum1 + sum2 + sum3
    }

    /// Dequantize to full vector
    pub fn dequantize(&self, quant: &QuantizedVector, total_dim: usize) -> Vec<f32> {
        let mut result = vec![0.0f32; total_dim];

        let e_vec = self.dequantize_component(&quant.euclidean, quant.euclidean_scale);
        let h_vec = self.dequantize_component(&quant.hyperbolic, quant.hyperbolic_scale);
        let s_vec = self.dequantize_component(&quant.spherical, quant.spherical_scale);

        let e_end = e_vec.len();
        let h_end = e_end + h_vec.len();

        result[0..e_end].copy_from_slice(&e_vec);
        result[e_end..h_end].copy_from_slice(&h_vec);
        result[h_end..h_end + s_vec.len()].copy_from_slice(&s_vec);

        result
    }

    /// Get memory savings ratio
    pub fn compression_ratio(&self, dim: usize, e_dim: usize, h_dim: usize, s_dim: usize) -> f32 {
        let original_bits = dim as f32 * 32.0;
        let quantized_bits = e_dim as f32 * self.config.euclidean_bits as f32
            + h_dim as f32 * self.config.hyperbolic_bits as f32
            + s_dim as f32 * self.config.spherical_bits as f32
            + 3.0 * 32.0; // 3 scale factors

        original_bits / quantized_bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_dequantize() {
        let quantizer = ComponentQuantizer::new(QuantizationConfig::default());

        let vector = vec![0.5f32; 64];
        let e_range = 0..32;
        let h_range = 32..48;
        let s_range = 48..64;

        let quantized =
            quantizer.quantize(&vector, e_range.clone(), h_range.clone(), s_range.clone());

        assert_eq!(quantized.euclidean.len(), 32);
        assert_eq!(quantized.hyperbolic.len(), 16);
        assert_eq!(quantized.spherical.len(), 16);

        // Dequantize and check approximate equality
        let dequantized = quantizer.dequantize(&quantized, 64);
        for (&orig, &deq) in vector.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.1);
        }
    }

    #[test]
    fn test_quantized_dot_product() {
        let quantizer = ComponentQuantizer::new(QuantizationConfig::default());

        let a = vec![1.0f32; 64];
        let b = vec![1.0f32; 64];
        let e_range = 0..32;
        let h_range = 32..48;
        let s_range = 48..64;

        let qa = quantizer.quantize(&a, e_range.clone(), h_range.clone(), s_range.clone());
        let qb = quantizer.quantize(&b, e_range, h_range, s_range);

        let weights = [0.5, 0.3, 0.2];
        let dot = quantizer.quantized_dot_product(&qa, &qb, &weights);

        // Should be positive for same vectors
        assert!(dot > 0.0);
    }

    #[test]
    fn test_compression_ratio() {
        let quantizer = ComponentQuantizer::new(QuantizationConfig::default());

        let ratio = quantizer.compression_ratio(512, 256, 192, 64);

        // With 8/5/5 bits vs 32 bits, expect ~4-5x compression
        assert!(ratio > 3.0);
        assert!(ratio < 7.0);
    }
}
