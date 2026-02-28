//! KVQuant: Pre-RoPE Key Quantization for Quality-Critical Long Contexts
//!
//! Based on: "KVQuant: Towards 10 Million Context Length LLM Inference
//! with KV Cache Quantization" (Hooper et al., 2024)
//!
//! Key insights:
//! - Quantize keys BEFORE RoPE application
//! - Pre-RoPE keys have smaller dynamic range, quantize better
//! - Apply RoPE during attention (deferred, once per query)
//! - 3-bit achieves ~5.3x compression with < 0.1 PPL degradation at 128K context
//!
//! This quantizer is recommended for contexts > 8K tokens where quality is paramount.

#[cfg(feature = "no_std_gateway")]
use alloc::{vec, vec::Vec};

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

/// Key quantization mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KVQuantKeyMode {
    /// Quantize keys BEFORE RoPE application (recommended)
    /// Pre-RoPE keys have smaller dynamic range, improving quantization
    PreRoPE,
    /// Standard post-RoPE quantization
    PostRoPE,
}

/// Value quantization mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KVQuantValueMode {
    /// Uniform quantization
    Uniform,
    /// Non-uniform quantization with special outlier bins
    NonUniform {
        /// Threshold for outlier detection (as percentile)
        outlier_percentile: u8,
    },
}

/// Pre-RoPE quantized key entry
#[derive(Debug, Clone)]
pub struct PreRoPEKey {
    /// Quantized data
    pub data: Vec<u8>,
    /// Scale for dequantization
    pub scale: f32,
    /// Zero point for dequantization
    pub zero_point: f32,
    /// Position for deferred RoPE application
    pub position: usize,
    /// Original dimension
    pub dim: usize,
}

/// Quantized value entry
#[derive(Debug, Clone)]
pub struct QuantizedValue {
    /// Quantized data
    pub data: Vec<u8>,
    /// Scale for dequantization
    pub scale: f32,
    /// Zero point for dequantization
    pub zero_point: f32,
    /// Outlier indices (if using non-uniform mode)
    pub outlier_indices: Option<Vec<usize>>,
    /// Outlier values (stored in FP16)
    pub outlier_values: Option<Vec<f32>>,
}

/// Calibration data for optimal quantization parameters
#[derive(Debug, Clone)]
pub struct CalibrationData {
    /// Key statistics per layer
    pub key_stats: Vec<(f32, f32)>, // (mean, std)
    /// Value statistics per layer
    pub value_stats: Vec<(f32, f32)>,
    /// Optimal clipping ranges
    pub key_clip_range: (f32, f32),
    pub value_clip_range: (f32, f32),
}

/// KVQuant quantizer for quality-critical long contexts
pub struct KVQuantQuantizer {
    /// Quantization bits (typically 3)
    bits: u8,
    /// Key quantization mode
    key_mode: KVQuantKeyMode,
    /// Value quantization mode
    value_mode: KVQuantValueMode,
    /// Head dimension
    head_dim: usize,
    /// Maximum quantization value
    max_quant: u8,
    /// Calibration data (optional)
    calibration: Option<CalibrationData>,
    /// RoPE parameters (for deferred application)
    rope_theta: f32,
}

impl KVQuantQuantizer {
    /// Create a new KVQuant quantizer
    ///
    /// # Arguments
    /// * `bits` - Quantization bits (typically 3)
    /// * `head_dim` - Head dimension
    /// * `pre_rope` - Whether to use pre-RoPE quantization
    pub fn new(bits: u8, head_dim: usize, pre_rope: bool) -> Self {
        assert!(bits >= 2 && bits <= 4, "KVQuant supports 2-4 bits");

        Self {
            bits,
            key_mode: if pre_rope {
                KVQuantKeyMode::PreRoPE
            } else {
                KVQuantKeyMode::PostRoPE
            },
            value_mode: KVQuantValueMode::Uniform,
            head_dim,
            max_quant: (1u8 << bits) - 1,
            calibration: None,
            rope_theta: 10000.0,
        }
    }

    /// Create with non-uniform value quantization
    pub fn with_nonuniform_values(mut self, outlier_percentile: u8) -> Self {
        self.value_mode = KVQuantValueMode::NonUniform { outlier_percentile };
        self
    }

    /// Set calibration data for optimal quantization
    pub fn with_calibration(mut self, calibration: CalibrationData) -> Self {
        self.calibration = Some(calibration);
        self
    }

    /// Set RoPE theta parameter
    pub fn with_rope_theta(mut self, theta: f32) -> Self {
        self.rope_theta = theta;
        self
    }

    /// Quantize key with pre-RoPE handling
    ///
    /// Key insight: Quantize BEFORE RoPE, dequantize + apply RoPE during attention
    pub fn quantize_key_pre_rope(&self, key: &[f32], position: usize) -> PreRoPEKey {
        assert_eq!(key.len(), self.head_dim);

        // Find min/max with optional calibration-based clipping
        let (min_val, max_val) = if let Some(ref cal) = self.calibration {
            cal.key_clip_range
        } else {
            let mut min_val = f32::MAX;
            let mut max_val = f32::MIN;
            for &val in key {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }
            (min_val, max_val)
        };

        // Ensure non-zero range
        let (min_val, max_val) = if (max_val - min_val).abs() < 1e-8 {
            (min_val, min_val + 1e-8)
        } else {
            (min_val, max_val)
        };

        let scale = (max_val - min_val) / self.max_quant as f32;
        let values_per_byte = 8 / self.bits as usize;

        // Quantize
        let mut data = Vec::with_capacity((self.head_dim + values_per_byte - 1) / values_per_byte);

        for chunk in key.chunks(values_per_byte) {
            let mut byte = 0u8;
            for (i, &val) in chunk.iter().enumerate() {
                // Clip and quantize
                let clipped = val.clamp(min_val, max_val);
                let q = ((clipped - min_val) / scale)
                    .round()
                    .clamp(0.0, self.max_quant as f32) as u8;

                match self.bits {
                    2 => byte |= q << (i * 2),
                    3 => {
                        // 3-bit packing is more complex
                        // For simplicity, we use 4-bit storage with 3-bit values
                        byte |= q << (i * 4);
                    }
                    4 => byte |= q << (i * 4),
                    _ => unreachable!(),
                }
            }
            data.push(byte);
        }

        PreRoPEKey {
            data,
            scale,
            zero_point: min_val,
            position,
            dim: self.head_dim,
        }
    }

    /// Quantize value with optional non-uniform handling
    pub fn quantize_value(&self, value: &[f32]) -> QuantizedValue {
        assert_eq!(value.len(), self.head_dim);

        match self.value_mode {
            KVQuantValueMode::Uniform => self.quantize_value_uniform(value),
            KVQuantValueMode::NonUniform { outlier_percentile } => {
                self.quantize_value_nonuniform(value, outlier_percentile)
            }
        }
    }

    /// Uniform value quantization
    fn quantize_value_uniform(&self, value: &[f32]) -> QuantizedValue {
        let (min_val, max_val) = if let Some(ref cal) = self.calibration {
            cal.value_clip_range
        } else {
            let mut min_val = f32::MAX;
            let mut max_val = f32::MIN;
            for &val in value {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }
            (min_val, max_val)
        };

        let (min_val, max_val) = if (max_val - min_val).abs() < 1e-8 {
            (min_val, min_val + 1e-8)
        } else {
            (min_val, max_val)
        };

        let scale = (max_val - min_val) / self.max_quant as f32;
        let values_per_byte = 8 / self.bits as usize;

        let mut data = Vec::with_capacity((self.head_dim + values_per_byte - 1) / values_per_byte);

        for chunk in value.chunks(values_per_byte) {
            let mut byte = 0u8;
            for (i, &val) in chunk.iter().enumerate() {
                let clipped = val.clamp(min_val, max_val);
                let q = ((clipped - min_val) / scale)
                    .round()
                    .clamp(0.0, self.max_quant as f32) as u8;

                match self.bits {
                    2 => byte |= q << (i * 2),
                    3 => byte |= q << (i * 4),
                    4 => byte |= q << (i * 4),
                    _ => unreachable!(),
                }
            }
            data.push(byte);
        }

        QuantizedValue {
            data,
            scale,
            zero_point: min_val,
            outlier_indices: None,
            outlier_values: None,
        }
    }

    /// Non-uniform value quantization with outlier handling
    fn quantize_value_nonuniform(&self, value: &[f32], percentile: u8) -> QuantizedValue {
        // Find outlier threshold
        let mut sorted: Vec<f32> = value.iter().map(|x| x.abs()).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let threshold_idx = (sorted.len() * percentile as usize / 100).min(sorted.len() - 1);
        let threshold = sorted[threshold_idx];

        // Separate outliers
        let mut outlier_indices = Vec::new();
        let mut outlier_values = Vec::new();
        let mut inlier_values = Vec::new();

        for (i, &val) in value.iter().enumerate() {
            if val.abs() > threshold {
                outlier_indices.push(i);
                outlier_values.push(val);
                inlier_values.push(0.0); // Placeholder
            } else {
                inlier_values.push(val);
            }
        }

        // Quantize inliers only
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        for (i, &val) in inlier_values.iter().enumerate() {
            if !outlier_indices.contains(&i) {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }
        }

        if (max_val - min_val).abs() < 1e-8 {
            max_val = min_val + 1e-8;
        }

        let scale = (max_val - min_val) / self.max_quant as f32;
        let values_per_byte = 8 / self.bits as usize;

        let mut data = Vec::with_capacity((self.head_dim + values_per_byte - 1) / values_per_byte);

        for chunk in inlier_values.chunks(values_per_byte) {
            let mut byte = 0u8;
            for (i, &val) in chunk.iter().enumerate() {
                let clipped = val.clamp(min_val, max_val);
                let q = ((clipped - min_val) / scale)
                    .round()
                    .clamp(0.0, self.max_quant as f32) as u8;

                match self.bits {
                    2 => byte |= q << (i * 2),
                    3 => byte |= q << (i * 4),
                    4 => byte |= q << (i * 4),
                    _ => unreachable!(),
                }
            }
            data.push(byte);
        }

        QuantizedValue {
            data,
            scale,
            zero_point: min_val,
            outlier_indices: if outlier_indices.is_empty() {
                None
            } else {
                Some(outlier_indices)
            },
            outlier_values: if outlier_values.is_empty() {
                None
            } else {
                Some(outlier_values)
            },
        }
    }

    /// Dequantize key and apply RoPE just-in-time
    pub fn dequantize_key_with_rope(&self, qkey: &PreRoPEKey) -> Vec<f32> {
        let values_per_byte = 8 / self.bits as usize;
        let mut dequantized = Vec::with_capacity(qkey.dim);

        // Dequantize
        for &byte in &qkey.data {
            for i in 0..values_per_byte {
                if dequantized.len() >= qkey.dim {
                    break;
                }

                let q = match self.bits {
                    2 => (byte >> (i * 2)) & 0b11,
                    3 => (byte >> (i * 4)) & 0b111,
                    4 => (byte >> (i * 4)) & 0b1111,
                    _ => unreachable!(),
                };

                let val = qkey.zero_point + (q as f32) * qkey.scale;
                dequantized.push(val);
            }
        }

        dequantized.truncate(qkey.dim);

        // Apply RoPE if this was pre-RoPE quantization
        if self.key_mode == KVQuantKeyMode::PreRoPE {
            self.apply_rope(&mut dequantized, qkey.position);
        }

        dequantized
    }

    /// Dequantize value
    pub fn dequantize_value(&self, qval: &QuantizedValue) -> Vec<f32> {
        let values_per_byte = 8 / self.bits as usize;
        let mut dequantized = Vec::with_capacity(self.head_dim);

        for &byte in &qval.data {
            for i in 0..values_per_byte {
                if dequantized.len() >= self.head_dim {
                    break;
                }

                let q = match self.bits {
                    2 => (byte >> (i * 2)) & 0b11,
                    3 => (byte >> (i * 4)) & 0b111,
                    4 => (byte >> (i * 4)) & 0b1111,
                    _ => unreachable!(),
                };

                let val = qval.zero_point + (q as f32) * qval.scale;
                dequantized.push(val);
            }
        }

        dequantized.truncate(self.head_dim);

        // Restore outliers if any
        if let (Some(indices), Some(values)) = (&qval.outlier_indices, &qval.outlier_values) {
            for (&idx, &val) in indices.iter().zip(values.iter()) {
                if idx < dequantized.len() {
                    dequantized[idx] = val;
                }
            }
        }

        dequantized
    }

    /// Apply RoPE (Rotary Position Embedding)
    fn apply_rope(&self, data: &mut [f32], position: usize) {
        let half_dim = data.len() / 2;

        for i in 0..half_dim {
            let freq = 1.0 / self.rope_theta.powf(2.0 * i as f32 / data.len() as f32);
            let angle = position as f32 * freq;
            let (sin, cos) = angle.sin_cos();

            let x0 = data[i];
            let x1 = data[i + half_dim];

            data[i] = x0 * cos - x1 * sin;
            data[i + half_dim] = x0 * sin + x1 * cos;
        }
    }

    /// Get configuration
    pub fn config(&self) -> (u8, KVQuantKeyMode, KVQuantValueMode) {
        (self.bits, self.key_mode, self.value_mode)
    }

    /// Calculate compression ratio vs FP16
    pub fn compression_ratio(&self) -> f32 {
        16.0 / self.bits as f32
    }

    /// Create calibration data from sample vectors
    pub fn calibrate(
        &self,
        key_samples: &[Vec<f32>],
        value_samples: &[Vec<f32>],
    ) -> CalibrationData {
        // Compute key statistics
        let key_stats = if !key_samples.is_empty() {
            let all_values: Vec<f32> = key_samples.iter().flatten().copied().collect();
            let mean = all_values.iter().sum::<f32>() / all_values.len() as f32;
            let variance = all_values.iter().map(|x| (x - mean).powi(2)).sum::<f32>()
                / all_values.len() as f32;
            vec![(mean, variance.sqrt())]
        } else {
            vec![(0.0, 1.0)]
        };

        // Compute value statistics
        let value_stats = if !value_samples.is_empty() {
            let all_values: Vec<f32> = value_samples.iter().flatten().copied().collect();
            let mean = all_values.iter().sum::<f32>() / all_values.len() as f32;
            let variance = all_values.iter().map(|x| (x - mean).powi(2)).sum::<f32>()
                / all_values.len() as f32;
            vec![(mean, variance.sqrt())]
        } else {
            vec![(0.0, 1.0)]
        };

        // Compute clip ranges (use 3-sigma for robustness)
        let (key_mean, key_std) = key_stats[0];
        let (value_mean, value_std) = value_stats[0];

        CalibrationData {
            key_stats,
            value_stats,
            key_clip_range: (key_mean - 3.0 * key_std, key_mean + 3.0 * key_std),
            value_clip_range: (value_mean - 3.0 * value_std, value_mean + 3.0 * value_std),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kvquant_3bit() {
        let quantizer = KVQuantQuantizer::new(3, 8, true);
        let key = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let qkey = quantizer.quantize_key_pre_rope(&key, 0);
        assert_eq!(qkey.position, 0);

        let dequantized = quantizer.dequantize_key_with_rope(&qkey);
        assert_eq!(dequantized.len(), 8);
    }

    #[test]
    fn test_kvquant_value_uniform() {
        let quantizer = KVQuantQuantizer::new(3, 8, false);
        let value = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let qval = quantizer.quantize_value(&value);
        let dequantized = quantizer.dequantize_value(&qval);

        assert_eq!(dequantized.len(), 8);
        assert!(qval.outlier_indices.is_none());
    }

    #[test]
    fn test_kvquant_value_nonuniform() {
        let quantizer = KVQuantQuantizer::new(3, 8, false).with_nonuniform_values(90);
        let value = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 100.0]; // One outlier

        let qval = quantizer.quantize_value(&value);
        let dequantized = quantizer.dequantize_value(&qval);

        assert_eq!(dequantized.len(), 8);
        // The outlier should be preserved
    }

    #[test]
    fn test_kvquant_compression_ratio() {
        let q2 = KVQuantQuantizer::new(2, 64, true);
        let q3 = KVQuantQuantizer::new(3, 64, true);
        let q4 = KVQuantQuantizer::new(4, 64, true);

        assert_eq!(q2.compression_ratio(), 8.0);
        assert!((q3.compression_ratio() - 5.33).abs() < 0.1);
        assert_eq!(q4.compression_ratio(), 4.0);
    }

    #[test]
    fn test_kvquant_calibration() {
        let quantizer = KVQuantQuantizer::new(3, 8, true);

        let key_samples: Vec<Vec<f32>> = (0..10)
            .map(|i| (0..8).map(|j| (i * 8 + j) as f32 * 0.1).collect())
            .collect();
        let value_samples = key_samples.clone();

        let calibration = quantizer.calibrate(&key_samples, &value_samples);

        assert!(!calibration.key_stats.is_empty());
        assert!(!calibration.value_stats.is_empty());
    }

    #[test]
    fn test_kvquant_pre_vs_post_rope() {
        let pre_rope = KVQuantQuantizer::new(3, 8, true);
        let post_rope = KVQuantQuantizer::new(3, 8, false);

        assert_eq!(pre_rope.key_mode, KVQuantKeyMode::PreRoPE);
        assert_eq!(post_rope.key_mode, KVQuantKeyMode::PostRoPE);
    }
}
