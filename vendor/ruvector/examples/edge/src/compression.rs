//! Tensor compression for efficient network transfer
//!
//! Uses LZ4 compression with optional quantization for vector data.

use crate::{Result, SwarmError};
use serde::{Deserialize, Serialize};

/// Compression level for tensor data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// No compression (fastest)
    None,
    /// Fast LZ4 compression (default)
    Fast,
    /// High compression ratio
    High,
    /// Quantize to 8-bit then compress
    Quantized8,
    /// Quantize to 4-bit then compress
    Quantized4,
}

impl Default for CompressionLevel {
    fn default() -> Self {
        CompressionLevel::Fast
    }
}

/// Tensor codec for compression/decompression
pub struct TensorCodec {
    level: CompressionLevel,
}

impl TensorCodec {
    /// Create new codec with default compression
    pub fn new() -> Self {
        Self {
            level: CompressionLevel::Fast,
        }
    }

    /// Create codec with specific compression level
    pub fn with_level(level: CompressionLevel) -> Self {
        Self { level }
    }

    /// Compress data
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self.level {
            CompressionLevel::None => Ok(data.to_vec()),
            CompressionLevel::Fast | CompressionLevel::High => {
                let compressed = lz4_flex::compress_prepend_size(data);
                Ok(compressed)
            }
            CompressionLevel::Quantized8 | CompressionLevel::Quantized4 => {
                // For quantized, just use LZ4 on the raw data
                // Real implementation would quantize floats first
                let compressed = lz4_flex::compress_prepend_size(data);
                Ok(compressed)
            }
        }
    }

    /// Decompress data
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self.level {
            CompressionLevel::None => Ok(data.to_vec()),
            _ => {
                lz4_flex::decompress_size_prepended(data)
                    .map_err(|e| SwarmError::Compression(e.to_string()))
            }
        }
    }

    /// Compress f32 tensor with quantization
    pub fn compress_tensor(&self, tensor: &[f32]) -> Result<CompressedTensor> {
        match self.level {
            CompressionLevel::Quantized8 => {
                let (quantized, scale, zero_point) = quantize_8bit(tensor);
                let compressed = lz4_flex::compress_prepend_size(&quantized);
                Ok(CompressedTensor {
                    data: compressed,
                    original_len: tensor.len(),
                    quantization: Some(QuantizationParams {
                        bits: 8,
                        scale,
                        zero_point,
                    }),
                })
            }
            CompressionLevel::Quantized4 => {
                let (quantized, scale, zero_point) = quantize_4bit(tensor);
                let compressed = lz4_flex::compress_prepend_size(&quantized);
                Ok(CompressedTensor {
                    data: compressed,
                    original_len: tensor.len(),
                    quantization: Some(QuantizationParams {
                        bits: 4,
                        scale,
                        zero_point,
                    }),
                })
            }
            _ => {
                // No quantization, just compress raw bytes
                let bytes: Vec<u8> = tensor
                    .iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect();
                let compressed = self.compress(&bytes)?;
                Ok(CompressedTensor {
                    data: compressed,
                    original_len: tensor.len(),
                    quantization: None,
                })
            }
        }
    }

    /// Decompress tensor back to f32
    pub fn decompress_tensor(&self, compressed: &CompressedTensor) -> Result<Vec<f32>> {
        let decompressed = lz4_flex::decompress_size_prepended(&compressed.data)
            .map_err(|e| SwarmError::Compression(e.to_string()))?;

        match &compressed.quantization {
            Some(params) if params.bits == 8 => {
                Ok(dequantize_8bit(&decompressed, params.scale, params.zero_point))
            }
            Some(params) if params.bits == 4 => {
                Ok(dequantize_4bit(&decompressed, compressed.original_len, params.scale, params.zero_point))
            }
            _ => {
                // Raw f32 bytes
                let tensor: Vec<f32> = decompressed
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                Ok(tensor)
            }
        }
    }

    /// Get compression ratio estimate for level
    pub fn estimated_ratio(&self) -> f32 {
        match self.level {
            CompressionLevel::None => 1.0,
            CompressionLevel::Fast => 0.5,
            CompressionLevel::High => 0.3,
            CompressionLevel::Quantized8 => 0.15,
            CompressionLevel::Quantized4 => 0.08,
        }
    }
}

impl Default for TensorCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Compressed tensor with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedTensor {
    pub data: Vec<u8>,
    pub original_len: usize,
    pub quantization: Option<QuantizationParams>,
}

/// Quantization parameters for dequantization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationParams {
    pub bits: u8,
    pub scale: f32,
    pub zero_point: f32,
}

/// Quantize f32 to 8-bit
fn quantize_8bit(tensor: &[f32]) -> (Vec<u8>, f32, f32) {
    if tensor.is_empty() {
        return (vec![], 1.0, 0.0);
    }

    let min_val = tensor.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = tensor.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let scale = (max_val - min_val) / 255.0;
    let zero_point = min_val;

    let quantized: Vec<u8> = tensor
        .iter()
        .map(|&v| {
            if scale == 0.0 {
                0u8
            } else {
                ((v - zero_point) / scale).clamp(0.0, 255.0) as u8
            }
        })
        .collect();

    (quantized, scale, zero_point)
}

/// Dequantize 8-bit back to f32
fn dequantize_8bit(quantized: &[u8], scale: f32, zero_point: f32) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| (q as f32) * scale + zero_point)
        .collect()
}

/// Quantize f32 to 4-bit (packed, 2 values per byte)
fn quantize_4bit(tensor: &[f32]) -> (Vec<u8>, f32, f32) {
    if tensor.is_empty() {
        return (vec![], 1.0, 0.0);
    }

    let min_val = tensor.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = tensor.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let scale = (max_val - min_val) / 15.0;
    let zero_point = min_val;

    // Pack two 4-bit values per byte
    let mut packed = Vec::with_capacity((tensor.len() + 1) / 2);

    for chunk in tensor.chunks(2) {
        let v0 = if scale == 0.0 {
            0u8
        } else {
            ((chunk[0] - zero_point) / scale).clamp(0.0, 15.0) as u8
        };

        let v1 = if chunk.len() > 1 && scale != 0.0 {
            ((chunk[1] - zero_point) / scale).clamp(0.0, 15.0) as u8
        } else {
            0u8
        };

        packed.push((v0 << 4) | v1);
    }

    (packed, scale, zero_point)
}

/// Dequantize 4-bit back to f32
fn dequantize_4bit(packed: &[u8], original_len: usize, scale: f32, zero_point: f32) -> Vec<f32> {
    let mut result = Vec::with_capacity(original_len);

    for &byte in packed {
        let v0 = (byte >> 4) as f32 * scale + zero_point;
        let v1 = (byte & 0x0F) as f32 * scale + zero_point;

        result.push(v0);
        if result.len() < original_len {
            result.push(v1);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_compression() {
        let codec = TensorCodec::with_level(CompressionLevel::Fast);
        let data = b"Hello, RuVector Edge! This is test data for compression.";

        let compressed = codec.compress(data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_8bit_quantization() {
        let codec = TensorCodec::with_level(CompressionLevel::Quantized8);
        let tensor: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();

        let compressed = codec.compress_tensor(&tensor).unwrap();
        let decompressed = codec.decompress_tensor(&compressed).unwrap();

        // Check approximate equality (quantization introduces small errors)
        for (orig, dec) in tensor.iter().zip(decompressed.iter()) {
            assert!((orig - dec).abs() < 0.01);
        }
    }

    #[test]
    fn test_4bit_quantization() {
        let codec = TensorCodec::with_level(CompressionLevel::Quantized4);
        let tensor: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();

        let compressed = codec.compress_tensor(&tensor).unwrap();
        let decompressed = codec.decompress_tensor(&compressed).unwrap();

        assert_eq!(decompressed.len(), tensor.len());

        // 4-bit has more error, but should be within bounds
        for (orig, dec) in tensor.iter().zip(decompressed.iter()) {
            assert!((orig - dec).abs() < 0.1);
        }
    }
}
