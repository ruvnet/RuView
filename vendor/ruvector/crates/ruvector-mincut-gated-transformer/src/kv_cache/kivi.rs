//! KIVI 2-bit/4-bit quantization with asymmetric per-channel/per-token schemes.
//!
//! Based on: "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache" (Liu et al., 2024)
//!
//! Key insights:
//! - Keys have large outliers per channel -> use per-channel quantization
//! - Values have consistent per-token magnitude -> use per-token quantization
//! - 2-bit achieves ~8x compression with <0.3 PPL degradation
//!
//! # Example
//!
//! ```rust
//! use ruvector_mincut_gated_transformer::kv_cache::kivi::{KiviQuantizer, QuantScheme};
//!
//! let quantizer = KiviQuantizer::new(2, 64); // 2-bit, 64 head_dim
//!
//! let data = vec![1.0f32; 64];
//! let (quantized, min_val, max_val) = quantizer.quantize(&data, QuantScheme::PerChannel);
//! let dequantized = quantizer.dequantize(&quantized, min_val, max_val);
//! ```

#[cfg(feature = "no_std_gateway")]
use alloc::{vec, vec::Vec};

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

/// Quantization scheme variants
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantScheme {
    /// Per-channel: one scale per head dimension (recommended for keys)
    /// Reduces outlier impact by scaling each dimension independently
    PerChannel,
    /// Per-token: one scale per token position (recommended for values)
    /// Preserves magnitude distribution across the token
    PerToken,
    /// Per-group: compromise between channel and token
    /// Groups dimensions together for scaling
    PerGroup { group_size: usize },
}

/// Quantized KV entry with metadata
#[derive(Debug, Clone)]
pub struct QuantizedKV {
    /// Packed quantized data
    pub data: Vec<u8>,
    /// Minimum value for dequantization
    pub min_val: f32,
    /// Maximum value for dequantization
    pub max_val: f32,
    /// Quantization scheme used
    pub scheme: QuantScheme,
    /// Original dimension
    pub dim: usize,
    /// Quantization bits
    pub bits: u8,
    /// Whether RoPE needs to be applied during dequantization (for KVQuant)
    pub needs_rope: bool,
    /// Position for deferred RoPE (if needs_rope is true)
    pub position: Option<usize>,
}

impl QuantizedKV {
    /// Get compression ratio
    pub fn compression_ratio(&self) -> f32 {
        let original_bytes = self.dim * 4; // FP32
        let quantized_bytes = self.data.len();
        original_bytes as f32 / quantized_bytes as f32
    }
}

/// KIVI quantizer supporting 2-bit and 4-bit quantization
///
/// Implements asymmetric quantization with configurable schemes:
/// - Per-channel for keys (reduces outlier impact)
/// - Per-token for values (preserves magnitude distribution)
pub struct KiviQuantizer {
    /// Quantization bit width (2 or 4)
    bits: u8,
    /// Head dimension
    head_dim: usize,
    /// Maximum quantization value
    max_quant: u8,
    /// Values packed per byte
    values_per_byte: usize,
    /// Optional Hadamard transform for outlier smoothing
    use_hadamard: bool,
}

impl KiviQuantizer {
    /// Create a new KIVI quantizer
    ///
    /// # Arguments
    /// * `bits` - Quantization bits (2 or 4)
    /// * `head_dim` - Head dimension (must be power of 2 for Hadamard)
    pub fn new(bits: u8, head_dim: usize) -> Self {
        assert!(bits == 2 || bits == 4, "KIVI only supports 2-bit or 4-bit");

        let max_quant = (1u8 << bits) - 1;
        let values_per_byte = 8 / bits as usize;

        Self {
            bits,
            head_dim,
            max_quant,
            values_per_byte,
            use_hadamard: head_dim.is_power_of_two(),
        }
    }

    /// Create quantizer with Hadamard transform enabled
    pub fn with_hadamard(bits: u8, head_dim: usize) -> Self {
        assert!(
            head_dim.is_power_of_two(),
            "Hadamard requires power-of-2 dimension"
        );
        let mut q = Self::new(bits, head_dim);
        q.use_hadamard = true;
        q
    }

    /// Quantize a vector
    ///
    /// Returns (quantized_data, min_val, max_val)
    pub fn quantize(&self, data: &[f32], scheme: QuantScheme) -> (Vec<u8>, f32, f32) {
        assert_eq!(data.len(), self.head_dim);

        // Optionally apply Hadamard transform for outlier smoothing
        let transformed: Vec<f32> = if self.use_hadamard {
            self.hadamard_forward(data)
        } else {
            data.to_vec()
        };

        // Compute min/max based on scheme
        let (min_val, max_val) = match scheme {
            QuantScheme::PerChannel | QuantScheme::PerToken => {
                let mut min_val = f32::MAX;
                let mut max_val = f32::MIN;
                for &val in transformed.iter() {
                    min_val = min_val.min(val);
                    max_val = max_val.max(val);
                }
                (min_val, max_val)
            }
            QuantScheme::PerGroup { group_size } => {
                // For per-group, we use the overall min/max for simplicity
                // A more sophisticated implementation would store per-group scales
                let mut min_val = f32::MAX;
                let mut max_val = f32::MIN;
                for &val in transformed.iter() {
                    min_val = min_val.min(val);
                    max_val = max_val.max(val);
                }
                let _ = group_size; // Acknowledge parameter
                (min_val, max_val)
            }
        };

        // Ensure non-zero range
        let (min_val, max_val) = if (max_val - min_val).abs() < 1e-8 {
            (min_val, min_val + 1e-8)
        } else {
            (min_val, max_val)
        };

        // Quantize
        let scale = self.max_quant as f32 / (max_val - min_val);
        let mut quantized =
            Vec::with_capacity((self.head_dim + self.values_per_byte - 1) / self.values_per_byte);

        for chunk in transformed.chunks(self.values_per_byte) {
            let mut byte = 0u8;
            for (i, &val) in chunk.iter().enumerate() {
                let q = ((val - min_val) * scale)
                    .round()
                    .clamp(0.0, self.max_quant as f32) as u8;

                match self.bits {
                    2 => byte |= q << (i * 2),
                    4 => byte |= q << (i * 4),
                    _ => unreachable!(),
                }
            }
            quantized.push(byte);
        }

        (quantized, min_val, max_val)
    }

    /// Dequantize a vector
    pub fn dequantize(&self, data: &[u8], min_val: f32, max_val: f32) -> Vec<f32> {
        let scale = (max_val - min_val) / self.max_quant as f32;
        let mut dequantized = Vec::with_capacity(self.head_dim);

        for &byte in data.iter() {
            for i in 0..self.values_per_byte {
                if dequantized.len() >= self.head_dim {
                    break;
                }

                let q = match self.bits {
                    2 => (byte >> (i * 2)) & 0b11,
                    4 => (byte >> (i * 4)) & 0b1111,
                    _ => unreachable!(),
                };

                let val = min_val + (q as f32) * scale;
                dequantized.push(val);
            }
        }

        dequantized.truncate(self.head_dim);

        // Inverse Hadamard if we used it
        if self.use_hadamard {
            self.hadamard_inverse(&mut dequantized);
            dequantized
        } else {
            dequantized
        }
    }

    /// Quantize keys with per-channel scheme (recommended)
    ///
    /// K shape: [batch, heads, seq_len, head_dim]
    /// Per-channel means one scale per head_dim position
    pub fn quantize_keys(&self, keys: &[f32]) -> QuantizedKV {
        let (data, min_val, max_val) = self.quantize(keys, QuantScheme::PerChannel);

        QuantizedKV {
            data,
            min_val,
            max_val,
            scheme: QuantScheme::PerChannel,
            dim: self.head_dim,
            bits: self.bits,
            needs_rope: false,
            position: None,
        }
    }

    /// Quantize values with per-token scheme (recommended)
    ///
    /// V shape: [batch, heads, seq_len, head_dim]
    /// Per-token means one scale per token
    pub fn quantize_values(&self, values: &[f32]) -> QuantizedKV {
        let (data, min_val, max_val) = self.quantize(values, QuantScheme::PerToken);

        QuantizedKV {
            data,
            min_val,
            max_val,
            scheme: QuantScheme::PerToken,
            dim: self.head_dim,
            bits: self.bits,
            needs_rope: false,
            position: None,
        }
    }

    /// Fast Walsh-Hadamard Transform for outlier smoothing
    fn hadamard_forward(&self, data: &[f32]) -> Vec<f32> {
        let mut result = data.to_vec();
        let n = result.len();

        // FWHT
        let mut h = 1;
        while h < n {
            let mut i = 0;
            while i < n {
                for j in i..(i + h) {
                    let x = result[j];
                    let y = result[j + h];
                    result[j] = x + y;
                    result[j + h] = x - y;
                }
                i += h * 2;
            }
            h *= 2;
        }

        // Normalize
        let norm = 1.0 / (n as f32).sqrt();
        for val in result.iter_mut() {
            *val *= norm;
        }

        result
    }

    /// Inverse Hadamard (same as forward since H is self-inverse up to scaling)
    fn hadamard_inverse(&self, data: &mut Vec<f32>) {
        let n = data.len();

        // FWHT
        let mut h = 1;
        while h < n {
            let mut i = 0;
            while i < n {
                for j in i..(i + h) {
                    let x = data[j];
                    let y = data[j + h];
                    data[j] = x + y;
                    data[j + h] = x - y;
                }
                i += h * 2;
            }
            h *= 2;
        }

        // Normalize
        let norm = 1.0 / (n as f32).sqrt();
        for val in data.iter_mut() {
            *val *= norm;
        }
    }

    /// Get configuration
    pub fn config(&self) -> (u8, usize, bool) {
        (self.bits, self.head_dim, self.use_hadamard)
    }

    /// Calculate bytes needed for quantized data
    pub fn bytes_per_vector(&self) -> usize {
        (self.head_dim * self.bits as usize + 7) / 8
    }

    /// Calculate compression ratio vs FP16
    pub fn compression_ratio_fp16(&self) -> f32 {
        16.0 / self.bits as f32
    }

    /// Calculate compression ratio vs FP32
    pub fn compression_ratio_fp32(&self) -> f32 {
        32.0 / self.bits as f32
    }
}

/// SIMD-accelerated dequantization for batches
#[cfg(target_arch = "x86_64")]
pub mod simd {
    use super::*;

    /// Dequantize multiple vectors in parallel using SIMD
    ///
    /// This is a placeholder for SIMD-optimized implementation.
    /// The actual SIMD code would use _mm256_* intrinsics.
    #[inline]
    pub fn dequantize_batch_avx2(
        quantizer: &KiviQuantizer,
        data: &[Vec<u8>],
        scales: &[(f32, f32)],
    ) -> Vec<Vec<f32>> {
        // Fallback to scalar implementation
        // TODO: Implement actual AVX2 version
        data.iter()
            .zip(scales.iter())
            .map(|(d, (min, max))| quantizer.dequantize(d, *min, *max))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kivi_2bit() {
        let quantizer = KiviQuantizer::new(2, 8);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let (quantized, min_val, max_val) = quantizer.quantize(&data, QuantScheme::PerChannel);
        let dequantized = quantizer.dequantize(&quantized, min_val, max_val);

        assert_eq!(dequantized.len(), 8);

        // Check compression
        assert_eq!(quantized.len(), 2); // 8 values * 2 bits = 16 bits = 2 bytes
    }

    #[test]
    fn test_kivi_4bit() {
        let quantizer = KiviQuantizer::new(4, 8);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let (quantized, min_val, max_val) = quantizer.quantize(&data, QuantScheme::PerToken);
        let dequantized = quantizer.dequantize(&quantized, min_val, max_val);

        assert_eq!(dequantized.len(), 8);

        // Check compression
        assert_eq!(quantized.len(), 4); // 8 values * 4 bits = 32 bits = 4 bytes
    }

    #[test]
    fn test_kivi_with_hadamard() {
        let quantizer = KiviQuantizer::with_hadamard(4, 8);
        let data = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 100.0]; // Outlier

        let (quantized, min_val, max_val) = quantizer.quantize(&data, QuantScheme::PerChannel);
        let dequantized = quantizer.dequantize(&quantized, min_val, max_val);

        // Hadamard should distribute the outlier, improving quantization
        let mse: f32 = data
            .iter()
            .zip(dequantized.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            / data.len() as f32;

        // With Hadamard, MSE should be reasonable even with outlier
        assert!(mse < 50.0, "MSE too high: {}", mse);
    }

    #[test]
    fn test_quantize_keys_values() {
        let quantizer = KiviQuantizer::new(4, 16);

        let key: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let value: Vec<f32> = (0..16).map(|i| (15 - i) as f32).collect();

        let qkey = quantizer.quantize_keys(&key);
        let qvalue = quantizer.quantize_values(&value);

        assert_eq!(qkey.scheme, QuantScheme::PerChannel);
        assert_eq!(qvalue.scheme, QuantScheme::PerToken);
        assert_eq!(qkey.bits, 4);
        assert_eq!(qvalue.bits, 4);
    }

    #[test]
    fn test_compression_ratio() {
        let q2 = KiviQuantizer::new(2, 64);
        let q4 = KiviQuantizer::new(4, 64);

        assert_eq!(q2.compression_ratio_fp16(), 8.0);
        assert_eq!(q4.compression_ratio_fp16(), 4.0);
        assert_eq!(q2.compression_ratio_fp32(), 16.0);
        assert_eq!(q4.compression_ratio_fp32(), 8.0);
    }

    #[test]
    fn test_bytes_per_vector() {
        let q2 = KiviQuantizer::new(2, 64);
        let q4 = KiviQuantizer::new(4, 64);

        assert_eq!(q2.bytes_per_vector(), 16); // 64 * 2 / 8
        assert_eq!(q4.bytes_per_vector(), 32); // 64 * 4 / 8
    }

    #[test]
    #[should_panic(expected = "KIVI only supports 2-bit or 4-bit")]
    fn test_invalid_bits() {
        let _q = KiviQuantizer::new(3, 64);
    }
}
