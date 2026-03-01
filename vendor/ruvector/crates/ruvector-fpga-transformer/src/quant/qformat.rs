//! Quantization format operations

use crate::types::QuantSpec;

/// Quantize f32 values to i8
pub fn quantize_i8(values: &[f32], spec: &QuantSpec) -> Vec<i8> {
    let scale = spec.scale_q as f32 / 65536.0;
    let zero = spec.zero_q as f32 / 65536.0;

    values
        .iter()
        .map(|&v| {
            let quantized = ((v - zero) / scale).round();
            quantized.clamp(-128.0, 127.0) as i8
        })
        .collect()
}

/// Quantize f32 values to i16
pub fn quantize_i16(values: &[f32]) -> Vec<i16> {
    // Find min/max
    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    // Handle edge case
    if (max - min).abs() < f32::EPSILON {
        return vec![0i16; values.len()];
    }

    let scale = (max - min) / 65535.0;

    values
        .iter()
        .map(|&v| {
            let normalized = (v - min) / scale - 32768.0;
            normalized.round().clamp(-32768.0, 32767.0) as i16
        })
        .collect()
}

/// Dequantize i8 values to f32
pub fn dequantize_i8(values: &[u8], spec: &QuantSpec) -> Vec<f32> {
    let scale = spec.scale_q as f32 / 65536.0;
    let zero = spec.zero_q as f32 / 65536.0;

    values
        .iter()
        .map(|&v| {
            let signed = v as i8;
            signed as f32 * scale + zero
        })
        .collect()
}

/// Dequantize i16 values to f32
pub fn dequantize_i16(values: &[i16], scale: f32, zero: f32) -> Vec<f32> {
    values.iter().map(|&v| v as f32 * scale + zero).collect()
}

/// Symmetric quantization (zero point = 0)
pub fn quantize_symmetric_i8(values: &[f32]) -> (Vec<i8>, f32) {
    let abs_max = values.iter().map(|v| v.abs()).fold(0.0f32, f32::max);

    if abs_max < f32::EPSILON {
        return (vec![0i8; values.len()], 1.0);
    }

    let scale = abs_max / 127.0;

    let quantized = values
        .iter()
        .map(|&v| (v / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();

    (quantized, scale)
}

/// Asymmetric quantization (uses full i8 range)
pub fn quantize_asymmetric_i8(values: &[f32]) -> (Vec<u8>, f32, i32) {
    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    if (max - min).abs() < f32::EPSILON {
        return (vec![0u8; values.len()], 1.0, 0);
    }

    let scale = (max - min) / 255.0;
    let zero_point = (-min / scale).round() as i32;

    let quantized = values
        .iter()
        .map(|&v| {
            let q = (v / scale).round() as i32 + zero_point;
            q.clamp(0, 255) as u8
        })
        .collect();

    (quantized, scale, zero_point)
}

/// Per-channel quantization for weights
pub fn quantize_per_channel_i8(weights: &[f32], out_channels: usize) -> (Vec<i8>, Vec<f32>) {
    let in_features = weights.len() / out_channels;
    let mut quantized = Vec::with_capacity(weights.len());
    let mut scales = Vec::with_capacity(out_channels);

    for c in 0..out_channels {
        let start = c * in_features;
        let end = start + in_features;
        let channel_weights = &weights[start..end];

        let (q, scale) = quantize_symmetric_i8(channel_weights);
        quantized.extend(q);
        scales.push(scale);
    }

    (quantized, scales)
}

/// Blocked quantization for hardware efficiency
pub fn quantize_blocked_i8(values: &[f32], block_size: usize) -> (Vec<i8>, Vec<f32>, Vec<i8>) {
    let num_blocks = (values.len() + block_size - 1) / block_size;
    let mut quantized = Vec::with_capacity(values.len());
    let mut scales = Vec::with_capacity(num_blocks);
    let mut zeros = Vec::with_capacity(num_blocks);

    for block_idx in 0..num_blocks {
        let start = block_idx * block_size;
        let end = (start + block_size).min(values.len());
        let block = &values[start..end];

        let (q, scale) = quantize_symmetric_i8(block);
        quantized.extend(q);
        scales.push(scale);
        zeros.push(0i8);
    }

    (quantized, scales, zeros)
}

/// Matrix quantization for GEMM
#[derive(Debug, Clone)]
pub struct QuantizedMatrix {
    /// Quantized values
    pub data: Vec<i8>,
    /// Rows
    pub rows: usize,
    /// Columns
    pub cols: usize,
    /// Per-row scales (for per-channel quantization)
    pub scales: Vec<f32>,
    /// Per-row zero points
    pub zeros: Vec<i8>,
}

impl QuantizedMatrix {
    /// Quantize a matrix with per-row scaling
    pub fn from_f32(data: &[f32], rows: usize, cols: usize) -> Self {
        assert_eq!(data.len(), rows * cols);

        let (quantized, scales) = quantize_per_channel_i8(data, rows);

        Self {
            data: quantized,
            rows,
            cols,
            scales,
            zeros: vec![0i8; rows],
        }
    }

    /// Get a row
    pub fn row(&self, idx: usize) -> &[i8] {
        let start = idx * self.cols;
        &self.data[start..start + self.cols]
    }

    /// Dequantize to f32
    pub fn to_f32(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.rows * self.cols);

        for r in 0..self.rows {
            let scale = self.scales[r];
            let zero = self.zeros[r] as f32;
            for &v in self.row(r) {
                result.push((v as f32 - zero) * scale);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::QuantSpec;

    #[test]
    fn test_quantize_symmetric() {
        let values = vec![1.0, -1.0, 0.5, -0.5, 0.0];
        let (quantized, scale) = quantize_symmetric_i8(&values);

        // Dequantize and check
        for (i, &q) in quantized.iter().enumerate() {
            let dequant = q as f32 * scale;
            assert!((dequant - values[i]).abs() < 0.1);
        }
    }

    #[test]
    fn test_quantize_asymmetric() {
        let values = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let (quantized, scale, zero) = quantize_asymmetric_i8(&values);

        // Dequantize and check
        for (i, &q) in quantized.iter().enumerate() {
            let dequant = (q as i32 - zero) as f32 * scale;
            assert!((dequant - values[i]).abs() < 0.1);
        }
    }

    #[test]
    fn test_quantized_matrix() {
        let data: Vec<f32> = (0..64).map(|i| i as f32 * 0.1 - 3.2).collect();
        let matrix = QuantizedMatrix::from_f32(&data, 8, 8);

        assert_eq!(matrix.rows, 8);
        assert_eq!(matrix.cols, 8);
        assert_eq!(matrix.scales.len(), 8);

        let dequantized = matrix.to_f32();
        for (orig, deq) in data.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.2);
        }
    }
}
