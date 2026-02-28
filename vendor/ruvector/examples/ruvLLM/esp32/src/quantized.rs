//! Quantized tensor operations for memory-efficient inference
//!
//! Supports INT8, INT4, and binary quantization for extreme memory savings.

use heapless::Vec as HVec;
use serde::{Deserialize, Serialize};

/// Maximum tensor size for stack allocation (16KB)
pub const MAX_TENSOR_SIZE: usize = 16 * 1024;

/// Quantization type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationType {
    /// 8-bit signed integer (-128 to 127)
    Int8,
    /// 4-bit signed integer (-8 to 7), packed 2 per byte
    Int4,
    /// Binary weights (-1 or +1), packed 8 per byte
    Binary,
    /// 16-bit fixed point (8.8 format)
    Fixed16,
}

impl QuantizationType {
    /// Bits per weight
    pub const fn bits(&self) -> usize {
        match self {
            Self::Int8 => 8,
            Self::Int4 => 4,
            Self::Binary => 1,
            Self::Fixed16 => 16,
        }
    }

    /// Compression ratio vs FP32
    pub const fn compression_ratio(&self) -> usize {
        32 / self.bits()
    }
}

/// Quantization parameters for dequantization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QuantParams {
    /// Scale factor: real_value = quantized_value * scale + zero_point
    pub scale: f32,
    /// Zero point offset
    pub zero_point: f32,
    /// Min value in original tensor
    pub min_val: f32,
    /// Max value in original tensor
    pub max_val: f32,
}

impl Default for QuantParams {
    fn default() -> Self {
        Self {
            scale: 1.0 / 127.0,
            zero_point: 0.0,
            min_val: -1.0,
            max_val: 1.0,
        }
    }
}

/// Quantized tensor stored in compact format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedTensor<const N: usize> {
    /// Quantized data
    pub data: HVec<u8, N>,
    /// Shape (max 4 dimensions for embedded)
    pub shape: [usize; 4],
    /// Number of dimensions used
    pub ndim: usize,
    /// Quantization type
    pub quant_type: QuantizationType,
    /// Quantization parameters
    pub params: QuantParams,
}

impl<const N: usize> QuantizedTensor<N> {
    /// Create a new quantized tensor from f32 data
    pub fn from_f32(data: &[f32], shape: &[usize], quant_type: QuantizationType) -> crate::Result<Self> {
        if data.is_empty() {
            return Err(crate::Error::QuantizationError("Empty data"));
        }

        // Calculate min/max
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        for &v in data {
            if v < min_val { min_val = v; }
            if v > max_val { max_val = v; }
        }

        let params = match quant_type {
            QuantizationType::Int8 => {
                let scale = (max_val - min_val) / 255.0;
                let zero_point = -min_val / scale - 128.0;
                QuantParams { scale, zero_point, min_val, max_val }
            }
            QuantizationType::Int4 => {
                let scale = (max_val - min_val) / 15.0;
                let zero_point = -min_val / scale - 8.0;
                QuantParams { scale, zero_point, min_val, max_val }
            }
            QuantizationType::Binary => {
                QuantParams {
                    scale: 1.0,
                    zero_point: 0.0,
                    min_val: -1.0,
                    max_val: 1.0,
                }
            }
            QuantizationType::Fixed16 => {
                let scale = (max_val - min_val) / 65535.0;
                QuantParams { scale, zero_point: min_val, min_val, max_val }
            }
        };

        let quantized_data = Self::quantize_data(data, quant_type, &params)?;

        let mut shape_arr = [0usize; 4];
        let ndim = shape.len().min(4);
        for (i, &s) in shape.iter().take(4).enumerate() {
            shape_arr[i] = s;
        }

        Ok(Self {
            data: quantized_data,
            shape: shape_arr,
            ndim,
            quant_type,
            params,
        })
    }

    fn quantize_data(data: &[f32], quant_type: QuantizationType, params: &QuantParams) -> crate::Result<HVec<u8, N>> {
        let mut result = HVec::new();

        match quant_type {
            QuantizationType::Int8 => {
                for &v in data {
                    let q = ((v - params.min_val) / params.scale).round() as i16;
                    let q = q.clamp(-128, 127) as i8;
                    result.push(q as u8).map_err(|_| crate::Error::BufferOverflow)?;
                }
            }
            QuantizationType::Int4 => {
                // Pack 2 values per byte
                for chunk in data.chunks(2) {
                    let v0 = ((chunk[0] - params.min_val) / params.scale).round() as i8;
                    let v1 = if chunk.len() > 1 {
                        ((chunk[1] - params.min_val) / params.scale).round() as i8
                    } else {
                        0
                    };
                    let v0 = (v0.clamp(-8, 7) + 8) as u8;
                    let v1 = (v1.clamp(-8, 7) + 8) as u8;
                    let packed = (v0 & 0x0F) | ((v1 & 0x0F) << 4);
                    result.push(packed).map_err(|_| crate::Error::BufferOverflow)?;
                }
            }
            QuantizationType::Binary => {
                // Pack 8 values per byte
                for chunk in data.chunks(8) {
                    let mut byte = 0u8;
                    for (i, &v) in chunk.iter().enumerate() {
                        if v >= 0.0 {
                            byte |= 1 << i;
                        }
                    }
                    result.push(byte).map_err(|_| crate::Error::BufferOverflow)?;
                }
            }
            QuantizationType::Fixed16 => {
                for &v in data {
                    let q = ((v - params.min_val) / params.scale).round() as u16;
                    result.push((q >> 8) as u8).map_err(|_| crate::Error::BufferOverflow)?;
                    result.push((q & 0xFF) as u8).map_err(|_| crate::Error::BufferOverflow)?;
                }
            }
        }

        Ok(result)
    }

    /// Get total number of elements
    pub fn numel(&self) -> usize {
        self.shape[..self.ndim].iter().product()
    }

    /// Get compressed size in bytes
    pub fn compressed_size(&self) -> usize {
        self.data.len()
    }

    /// Memory savings compared to FP32
    pub fn memory_savings(&self) -> f32 {
        let fp32_size = self.numel() * 4;
        1.0 - (self.compressed_size() as f32 / fp32_size as f32)
    }
}

/// INT8 matrix-vector multiplication (optimized for ESP32)
///
/// Computes: output = weights @ input
/// Where weights is [out_dim, in_dim] and input is [in_dim]
#[inline(never)] // Prevent inlining for better cache behavior
pub fn matmul_int8(
    weights: &[i8],
    _weight_params: &QuantParams,
    input: &[i8],
    _input_params: &QuantParams,
    output: &mut [i32],
    out_dim: usize,
    in_dim: usize,
) {
    debug_assert_eq!(weights.len(), out_dim * in_dim);
    debug_assert_eq!(input.len(), in_dim);
    debug_assert_eq!(output.len(), out_dim);

    for i in 0..out_dim {
        let mut acc: i32 = 0;
        let row_start = i * in_dim;

        // Process 4 elements at a time for better performance
        let chunks = in_dim / 4;
        for j in 0..chunks {
            let idx = j * 4;
            acc += weights[row_start + idx] as i32 * input[idx] as i32;
            acc += weights[row_start + idx + 1] as i32 * input[idx + 1] as i32;
            acc += weights[row_start + idx + 2] as i32 * input[idx + 2] as i32;
            acc += weights[row_start + idx + 3] as i32 * input[idx + 3] as i32;
        }

        // Handle remainder
        for j in (chunks * 4)..in_dim {
            acc += weights[row_start + j] as i32 * input[j] as i32;
        }

        output[i] = acc;
    }
}

/// Dequantize INT32 accumulator to f32
#[inline]
pub fn dequantize_accumulator(
    acc: i32,
    weight_params: &QuantParams,
    input_params: &QuantParams,
) -> f32 {
    acc as f32 * weight_params.scale * input_params.scale
}

/// Binary XNOR-popcount for extreme efficiency
///
/// For binary neural networks: computes hamming similarity
#[inline]
pub fn binary_xnor_popcount(a: &[u8], b: &[u8]) -> i32 {
    debug_assert_eq!(a.len(), b.len());

    let mut count: i32 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        // XNOR: same bits = 1, different = 0
        let xnor = !(x ^ y);
        count += xnor.count_ones() as i32;
    }

    // Convert popcount to -1/+1 dot product equivalent
    // Each byte has 8 bits, so:
    // dot = popcount * 2 - total_bits
    let total_bits = (a.len() * 8) as i32;
    count * 2 - total_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int8_quantization() {
        let data = [-1.0f32, -0.5, 0.0, 0.5, 1.0];
        let tensor: QuantizedTensor<64> = QuantizedTensor::from_f32(
            &data,
            &[5],
            QuantizationType::Int8
        ).unwrap();

        assert_eq!(tensor.numel(), 5);
        assert_eq!(tensor.compressed_size(), 5);
        assert!(tensor.memory_savings() > 0.7); // 75% savings
    }

    #[test]
    fn test_binary_xnor() {
        let a = [0b11110000u8, 0b10101010];
        let b = [0b11110000u8, 0b10101010];

        // Perfect match: all 16 bits same
        let result = binary_xnor_popcount(&a, &b);
        assert_eq!(result, 16); // 16 * 2 - 16 = 16
    }

    #[test]
    fn test_int4_packing() {
        let data = [0.0f32, 0.5, -0.5, 1.0];
        let tensor: QuantizedTensor<64> = QuantizedTensor::from_f32(
            &data,
            &[4],
            QuantizationType::Int4
        ).unwrap();

        // 4 values packed into 2 bytes
        assert_eq!(tensor.compressed_size(), 2);
    }
}
