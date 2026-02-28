//! Compress Layer - Binary Tensor Storage
//!
//! This module handles the compression and storage of representation tensors.
//! Unlike standard RAG which stores text, REFRAG stores pre-computed embeddings
//! that can be directly injected into LLM context.

use crate::types::RefragEntry;
use ndarray::{Array1, Array2};
use std::io::{Read, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CompressError {
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Invalid tensor data: {0}")]
    InvalidTensor(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Quantization error: {0}")]
    QuantizationError(String),
}

pub type Result<T> = std::result::Result<T, CompressError>;

/// Tensor compression strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStrategy {
    /// No compression - store raw f32 values
    None,
    /// Float16 quantization (2x compression)
    Float16,
    /// Int8 scalar quantization (4x compression)
    Int8,
    /// Binary quantization (32x compression)
    Binary,
}

/// Tensor compressor for REFRAG entries
pub struct TensorCompressor {
    /// Expected tensor dimensions
    dimensions: usize,
    /// Compression strategy
    strategy: CompressionStrategy,
}

impl TensorCompressor {
    /// Create a new tensor compressor
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            strategy: CompressionStrategy::None,
        }
    }

    /// Set compression strategy
    pub fn with_strategy(mut self, strategy: CompressionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Compress a float vector to binary representation
    pub fn compress(&self, vector: &[f32]) -> Result<Vec<u8>> {
        if vector.len() != self.dimensions {
            return Err(CompressError::DimensionMismatch {
                expected: self.dimensions,
                actual: vector.len(),
            });
        }

        match self.strategy {
            CompressionStrategy::None => self.compress_none(vector),
            CompressionStrategy::Float16 => self.compress_float16(vector),
            CompressionStrategy::Int8 => self.compress_int8(vector),
            CompressionStrategy::Binary => self.compress_binary(vector),
        }
    }

    /// Decompress binary representation back to float vector
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<f32>> {
        match self.strategy {
            CompressionStrategy::None => self.decompress_none(data),
            CompressionStrategy::Float16 => self.decompress_float16(data),
            CompressionStrategy::Int8 => self.decompress_int8(data),
            CompressionStrategy::Binary => self.decompress_binary(data),
        }
    }

    /// Get compression ratio for current strategy
    pub fn compression_ratio(&self) -> f32 {
        match self.strategy {
            CompressionStrategy::None => 1.0,
            CompressionStrategy::Float16 => 2.0,
            CompressionStrategy::Int8 => 4.0,
            CompressionStrategy::Binary => 32.0,
        }
    }

    // --- Compression implementations ---

    fn compress_none(&self, vector: &[f32]) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(vector.len() * 4);
        for &v in vector {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        Ok(bytes)
    }

    fn decompress_none(&self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() != self.dimensions * 4 {
            return Err(CompressError::InvalidTensor(format!(
                "Expected {} bytes, got {}",
                self.dimensions * 4,
                data.len()
            )));
        }

        let mut vector = Vec::with_capacity(self.dimensions);
        for chunk in data.chunks_exact(4) {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            vector.push(f32::from_le_bytes(bytes));
        }
        Ok(vector)
    }

    fn compress_float16(&self, vector: &[f32]) -> Result<Vec<u8>> {
        // Simple float16 approximation using truncation
        let mut bytes = Vec::with_capacity(vector.len() * 2);
        for &v in vector {
            let bits = v.to_bits();
            // Truncate mantissa from 23 bits to 10 bits
            let sign = (bits >> 31) & 1;
            let exp = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
            let mantissa = (bits >> 13) & 0x3FF;

            let f16 = if exp <= 0 {
                0u16 // Underflow to zero
            } else if exp >= 31 {
                ((sign as u16) << 15) | 0x7C00 // Overflow to infinity
            } else {
                ((sign as u16) << 15) | ((exp as u16) << 10) | (mantissa as u16)
            };

            bytes.extend_from_slice(&f16.to_le_bytes());
        }
        Ok(bytes)
    }

    fn decompress_float16(&self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() != self.dimensions * 2 {
            return Err(CompressError::InvalidTensor(format!(
                "Expected {} bytes for float16, got {}",
                self.dimensions * 2,
                data.len()
            )));
        }

        let mut vector = Vec::with_capacity(self.dimensions);
        for chunk in data.chunks_exact(2) {
            let f16 = u16::from_le_bytes([chunk[0], chunk[1]]);
            let sign = ((f16 >> 15) & 1) as u32;
            let exp = ((f16 >> 10) & 0x1F) as i32;
            let mantissa = (f16 & 0x3FF) as u32;

            let f32_bits = if exp == 0 {
                0u32 // Zero
            } else if exp == 31 {
                (sign << 31) | 0x7F800000 // Infinity
            } else {
                let new_exp = (exp - 15 + 127) as u32;
                (sign << 31) | (new_exp << 23) | (mantissa << 13)
            };

            vector.push(f32::from_bits(f32_bits));
        }
        Ok(vector)
    }

    fn compress_int8(&self, vector: &[f32]) -> Result<Vec<u8>> {
        // Find min/max for scaling
        let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
        let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let scale = if (max - min).abs() < f32::EPSILON {
            1.0
        } else {
            255.0 / (max - min)
        };

        // Header: min (4 bytes) + scale (4 bytes)
        let mut bytes = Vec::with_capacity(8 + vector.len());
        bytes.extend_from_slice(&min.to_le_bytes());
        bytes.extend_from_slice(&scale.to_le_bytes());

        // Quantized values
        for &v in vector {
            let quantized = ((v - min) * scale).round() as u8;
            bytes.push(quantized);
        }

        Ok(bytes)
    }

    fn decompress_int8(&self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() != 8 + self.dimensions {
            return Err(CompressError::InvalidTensor(format!(
                "Expected {} bytes for int8, got {}",
                8 + self.dimensions,
                data.len()
            )));
        }

        let min = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let scale = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        let mut vector = Vec::with_capacity(self.dimensions);
        for &q in &data[8..] {
            let v = min + (q as f32) / scale;
            vector.push(v);
        }

        Ok(vector)
    }

    fn compress_binary(&self, vector: &[f32]) -> Result<Vec<u8>> {
        let num_bytes = (self.dimensions + 7) / 8;
        let mut bits = vec![0u8; num_bytes];

        for (i, &v) in vector.iter().enumerate() {
            if v > 0.0 {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits[byte_idx] |= 1 << bit_idx;
            }
        }

        Ok(bits)
    }

    fn decompress_binary(&self, data: &[u8]) -> Result<Vec<f32>> {
        let expected_bytes = (self.dimensions + 7) / 8;
        if data.len() != expected_bytes {
            return Err(CompressError::InvalidTensor(format!(
                "Expected {} bytes for binary, got {}",
                expected_bytes,
                data.len()
            )));
        }

        let mut vector = Vec::with_capacity(self.dimensions);
        for i in 0..self.dimensions {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let bit = (data[byte_idx] >> bit_idx) & 1;
            vector.push(if bit == 1 { 1.0 } else { -1.0 });
        }

        Ok(vector)
    }
}

/// Batch compressor for multiple entries
pub struct BatchCompressor {
    compressor: TensorCompressor,
}

impl BatchCompressor {
    pub fn new(dimensions: usize, strategy: CompressionStrategy) -> Self {
        Self {
            compressor: TensorCompressor::new(dimensions).with_strategy(strategy),
        }
    }

    /// Compress multiple vectors in parallel
    pub fn compress_batch(&self, vectors: &[Vec<f32>]) -> Result<Vec<Vec<u8>>> {
        vectors
            .iter()
            .map(|v| self.compressor.compress(v))
            .collect()
    }

    /// Create RefragEntry from vector and text
    pub fn create_entry(
        &self,
        id: impl Into<String>,
        search_vector: Vec<f32>,
        representation_vector: Vec<f32>,
        text: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Result<RefragEntry> {
        let tensor = self.compressor.compress(&representation_vector)?;

        Ok(RefragEntry::new(id, search_vector, text).with_tensor(tensor, model_id))
    }
}

/// Tensor utilities
pub mod utils {
    use super::*;

    /// Convert ndarray to bytes
    pub fn array_to_bytes(arr: &Array1<f32>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(arr.len() * 4);
        for &v in arr.iter() {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes
    }

    /// Convert bytes to ndarray
    pub fn bytes_to_array(data: &[u8]) -> Array1<f32> {
        let mut values = Vec::with_capacity(data.len() / 4);
        for chunk in data.chunks_exact(4) {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            values.push(f32::from_le_bytes(bytes));
        }
        Array1::from_vec(values)
    }

    /// Normalize a vector to unit length
    pub fn normalize(vector: &mut [f32]) {
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for v in vector.iter_mut() {
                *v /= norm;
            }
        }
    }

    /// Compute cosine similarity between two vectors
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > f32::EPSILON && norm_b > f32::EPSILON {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compression() {
        let compressor = TensorCompressor::new(4);
        let vector = vec![1.0, 2.0, 3.0, 4.0];

        let compressed = compressor.compress(&vector).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(vector, decompressed);
    }

    #[test]
    fn test_binary_compression() {
        let compressor = TensorCompressor::new(8).with_strategy(CompressionStrategy::Binary);
        let vector = vec![1.0, -1.0, 0.5, -0.5, 1.0, 1.0, -1.0, -1.0];

        let compressed = compressor.compress(&vector).unwrap();
        assert_eq!(compressed.len(), 1); // 8 bits = 1 byte

        let decompressed = compressor.decompress(&compressed).unwrap();
        // Binary only preserves sign
        assert_eq!(
            decompressed,
            vec![1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0]
        );
    }

    #[test]
    fn test_dimension_mismatch() {
        let compressor = TensorCompressor::new(4);
        let vector = vec![1.0, 2.0, 3.0]; // Wrong size

        let result = compressor.compress(&vector);
        assert!(matches!(
            result,
            Err(CompressError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_batch_compression() {
        let batch = BatchCompressor::new(4, CompressionStrategy::None);
        let vectors = vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]];

        let compressed = batch.compress_batch(&vectors).unwrap();
        assert_eq!(compressed.len(), 2);
    }
}
