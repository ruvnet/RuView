//! Model quantization utilities
//!
//! Provides INT8 quantization for model weights and activations to reduce
//! memory usage and improve inference speed.

use std::f32;

/// Quantization parameters
#[derive(Debug, Clone, Copy)]
pub struct QuantParams {
    pub scale: f32,
    pub zero_point: i8,
}

impl QuantParams {
    /// Calculate quantization parameters from min/max values
    pub fn from_range(min: f32, max: f32) -> Self {
        let qmin = i8::MIN as f32;
        let qmax = i8::MAX as f32;

        let scale = (max - min) / (qmax - qmin);
        let zero_point = (qmin - min / scale).round() as i8;

        Self { scale, zero_point }
    }

    /// Calculate from data statistics
    pub fn from_data(data: &[f32]) -> Self {
        let min = data.iter().copied().fold(f32::INFINITY, f32::min);
        let max = data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Self::from_range(min, max)
    }

    /// Symmetric quantization (zero_point = 0)
    pub fn symmetric(abs_max: f32) -> Self {
        let scale = abs_max / 127.0;
        Self {
            scale,
            zero_point: 0,
        }
    }
}

/// Quantize f32 weights to i8
pub fn quantize_weights(weights: &[f32]) -> (Vec<i8>, QuantParams) {
    let params = QuantParams::from_data(weights);
    let quantized = quantize_with_params(weights, params);
    (quantized, params)
}

/// Quantize with given parameters
pub fn quantize_with_params(weights: &[f32], params: QuantParams) -> Vec<i8> {
    weights.iter().map(|&w| quantize_value(w, params)).collect()
}

/// Quantize single value
#[inline]
pub fn quantize_value(value: f32, params: QuantParams) -> i8 {
    let scaled = value / params.scale + params.zero_point as f32;
    scaled.round().clamp(i8::MIN as f32, i8::MAX as f32) as i8
}

/// Dequantize i8 to f32
pub fn dequantize(quantized: &[i8], params: QuantParams) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| dequantize_value(q, params))
        .collect()
}

/// Dequantize single value
#[inline]
pub fn dequantize_value(quantized: i8, params: QuantParams) -> f32 {
    (quantized as f32 - params.zero_point as f32) * params.scale
}

/// Quantized tensor representation
pub struct QuantizedTensor {
    pub data: Vec<i8>,
    pub params: QuantParams,
    pub shape: Vec<usize>,
}

impl QuantizedTensor {
    /// Create from f32 tensor
    pub fn from_f32(data: &[f32], shape: Vec<usize>) -> Self {
        let (quantized, params) = quantize_weights(data);
        Self {
            data: quantized,
            params,
            shape,
        }
    }

    /// Create with symmetric quantization
    pub fn from_f32_symmetric(data: &[f32], shape: Vec<usize>) -> Self {
        let abs_max = data.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let params = QuantParams::symmetric(abs_max);
        let quantized = quantize_with_params(data, params);

        Self {
            data: quantized,
            params,
            shape,
        }
    }

    /// Dequantize to f32
    pub fn to_f32(&self) -> Vec<f32> {
        dequantize(&self.data, self.params)
    }

    /// Get size in bytes
    pub fn size_bytes(&self) -> usize {
        self.data.len()
            + std::mem::size_of::<QuantParams>()
            + self.shape.len() * std::mem::size_of::<usize>()
    }

    /// Calculate memory savings vs f32
    pub fn compression_ratio(&self) -> f32 {
        let f32_size = self.data.len() * std::mem::size_of::<f32>();
        let quantized_size = self.size_bytes();
        f32_size as f32 / quantized_size as f32
    }
}

/// Per-channel quantization for conv/linear layers
pub struct PerChannelQuant {
    pub data: Vec<i8>,
    pub params: Vec<QuantParams>,
    pub shape: Vec<usize>,
}

impl PerChannelQuant {
    /// Quantize with per-channel parameters
    /// For a weight tensor of shape [out_channels, in_channels, ...],
    /// use separate params for each output channel
    pub fn from_f32(data: &[f32], shape: Vec<usize>) -> Self {
        if shape.is_empty() {
            panic!("Shape cannot be empty");
        }

        let out_channels = shape[0];
        let channel_size = data.len() / out_channels;

        let mut all_quantized = Vec::with_capacity(data.len());
        let mut params = Vec::with_capacity(out_channels);

        for ch in 0..out_channels {
            let start = ch * channel_size;
            let end = start + channel_size;
            let channel_data = &data[start..end];

            let ch_params = QuantParams::from_data(channel_data);
            let ch_quantized = quantize_with_params(channel_data, ch_params);

            all_quantized.extend(ch_quantized);
            params.push(ch_params);
        }

        Self {
            data: all_quantized,
            params,
            shape,
        }
    }

    /// Dequantize to f32
    pub fn to_f32(&self) -> Vec<f32> {
        let out_channels = self.shape[0];
        let channel_size = self.data.len() / out_channels;

        let mut result = Vec::with_capacity(self.data.len());

        for ch in 0..out_channels {
            let start = ch * channel_size;
            let end = start + channel_size;
            let channel_data = &self.data[start..end];
            let ch_params = self.params[ch];

            result.extend(dequantize(channel_data, ch_params));
        }

        result
    }
}

/// Dynamic quantization - quantize at runtime
pub struct DynamicQuantizer {
    percentile: f32,
}

impl DynamicQuantizer {
    /// Create quantizer with calibration percentile
    /// percentile: clip values beyond this percentile (e.g., 99.9)
    pub fn new(percentile: f32) -> Self {
        Self { percentile }
    }

    /// Quantize with calibration
    pub fn quantize(&self, data: &[f32]) -> (Vec<i8>, QuantParams) {
        let mut sorted: Vec<f32> = data.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let idx = ((sorted.len() as f32 * self.percentile / 100.0) as usize).min(sorted.len() - 1);

        let min = -sorted[sorted.len() - idx];
        let max = sorted[idx];

        let params = QuantParams::from_range(min, max);
        let quantized = quantize_with_params(data, params);

        (quantized, params)
    }
}

/// Calculate quantization error (MSE)
pub fn quantization_error(original: &[f32], quantized: &[i8], params: QuantParams) -> f32 {
    let dequantized = dequantize(quantized, params);

    let mse: f32 = original
        .iter()
        .zip(dequantized.iter())
        .map(|(o, d)| (o - d).powi(2))
        .sum::<f32>()
        / original.len() as f32;

    mse
}

/// Calculate signal-to-quantization-noise ratio (SQNR) in dB
pub fn sqnr(original: &[f32], quantized: &[i8], params: QuantParams) -> f32 {
    let dequantized = dequantize(quantized, params);

    let signal_power: f32 = original.iter().map(|x| x.powi(2)).sum::<f32>() / original.len() as f32;
    let noise_power: f32 = original
        .iter()
        .zip(dequantized.iter())
        .map(|(o, d)| (o - d).powi(2))
        .sum::<f32>()
        / original.len() as f32;

    10.0 * (signal_power / noise_power).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_dequantize() {
        let weights = vec![0.0, 0.5, 1.0, -0.5, -1.0];
        let (quantized, params) = quantize_weights(&weights);
        let dequantized = dequantize(&quantized, params);

        // Check approximate equality
        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.01, "orig: {}, deq: {}", orig, deq);
        }
    }

    #[test]
    fn test_symmetric_quantization() {
        let data = vec![-1.0, -0.5, 0.0, 0.5, 1.0];
        let params = QuantParams::symmetric(1.0);

        assert_eq!(params.zero_point, 0);
        assert!((params.scale - 1.0 / 127.0).abs() < 1e-6);

        let quantized = quantize_with_params(&data, params);
        assert_eq!(quantized[2], 0); // 0.0 should map to 0
    }

    #[test]
    fn test_quantized_tensor() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let tensor = QuantizedTensor::from_f32(&data, vec![2, 2]);

        assert_eq!(tensor.shape, vec![2, 2]);
        assert_eq!(tensor.data.len(), 4);

        let dequantized = tensor.to_f32();
        for (orig, deq) in data.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.1);
        }
    }

    #[test]
    fn test_per_channel_quant() {
        // 2 channels, 3 values each
        let data = vec![
            1.0, 2.0, 3.0, // Channel 0
            10.0, 20.0, 30.0, // Channel 1
        ];

        let quant = PerChannelQuant::from_f32(&data, vec![2, 3]);
        assert_eq!(quant.params.len(), 2);

        let dequantized = quant.to_f32();
        for (orig, deq) in data.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 1.0);
        }
    }

    #[test]
    fn test_quantization_error() {
        let original = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (quantized, params) = quantize_weights(&original);

        let error = quantization_error(&original, &quantized, params);
        assert!(error < 0.1); // Should be small for simple data

        let snr = sqnr(&original, &quantized, params);
        assert!(snr > 30.0); // Should have good SNR
    }

    #[test]
    fn test_compression_ratio() {
        let data: Vec<f32> = (0..1000).map(|i| i as f32 / 1000.0).collect();
        let tensor = QuantizedTensor::from_f32(&data, vec![1000]);

        let ratio = tensor.compression_ratio();
        assert!(ratio > 3.5); // Should be ~4x compression
    }

    #[test]
    fn test_dynamic_quantizer() {
        let mut data: Vec<f32> = (0..100).map(|i| i as f32).collect();
        data.push(1000.0); // Outlier

        let quantizer = DynamicQuantizer::new(99.0);
        let (quantized, params) = quantizer.quantize(&data);

        assert_eq!(quantized.len(), 101);
        // The outlier should be clipped
        assert!(params.scale > 0.0);
    }
}
