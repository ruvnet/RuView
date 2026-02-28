//! Tensor compression with adaptive level selection
//!
//! This module provides multi-level tensor compression based on access frequency:
//! - Hot data (f > 0.8): Full precision
//! - Warm data (f > 0.4): Half precision
//! - Cool data (f > 0.1): 8-bit product quantization
//! - Cold data (f > 0.01): 4-bit product quantization
//! - Archive (f <= 0.01): Binary quantization

use crate::error::{GnnError, Result};
use serde::{Deserialize, Serialize};

/// Compression level with associated parameters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompressionLevel {
    /// Full precision - no compression
    None,

    /// Half precision with scale factor
    Half { scale: f32 },

    /// Product quantization with 8-bit codes
    PQ8 { subvectors: u8, centroids: u8 },

    /// Product quantization with 4-bit codes and outlier handling
    PQ4 {
        subvectors: u8,
        outlier_threshold: f32,
    },

    /// Binary quantization with threshold
    Binary { threshold: f32 },
}

/// Compressed tensor data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressedTensor {
    /// Uncompressed full precision data
    Full { data: Vec<f32> },

    /// Half precision data
    Half {
        data: Vec<u16>,
        scale: f32,
        dim: usize,
    },

    /// 8-bit product quantization
    PQ8 {
        codes: Vec<u8>,
        codebooks: Vec<Vec<f32>>,
        subvector_dim: usize,
        dim: usize,
    },

    /// 4-bit product quantization with outliers
    PQ4 {
        codes: Vec<u8>, // Packed 4-bit codes
        codebooks: Vec<Vec<f32>>,
        outliers: Vec<(usize, f32)>, // (index, value) pairs
        subvector_dim: usize,
        dim: usize,
    },

    /// Binary quantization
    Binary {
        bits: Vec<u8>,
        threshold: f32,
        dim: usize,
    },
}

/// Tensor compressor with adaptive level selection
#[derive(Debug, Clone)]
pub struct TensorCompress {
    /// Default compression parameters
    default_level: CompressionLevel,
}

impl Default for TensorCompress {
    fn default() -> Self {
        Self::new()
    }
}

impl TensorCompress {
    /// Create a new tensor compressor with default settings
    pub fn new() -> Self {
        Self {
            default_level: CompressionLevel::None,
        }
    }

    /// Compress an embedding based on access frequency
    ///
    /// # Arguments
    /// * `embedding` - The input embedding vector
    /// * `access_freq` - Access frequency in range [0.0, 1.0]
    ///
    /// # Returns
    /// Compressed tensor using adaptive compression level
    pub fn compress(&self, embedding: &[f32], access_freq: f32) -> Result<CompressedTensor> {
        if embedding.is_empty() {
            return Err(GnnError::InvalidInput("Empty embedding vector".to_string()));
        }

        let level = self.select_level(access_freq);
        self.compress_with_level(embedding, &level)
    }

    /// Compress with explicit compression level
    pub fn compress_with_level(
        &self,
        embedding: &[f32],
        level: &CompressionLevel,
    ) -> Result<CompressedTensor> {
        match level {
            CompressionLevel::None => self.compress_none(embedding),
            CompressionLevel::Half { scale } => self.compress_half(embedding, *scale),
            CompressionLevel::PQ8 {
                subvectors,
                centroids,
            } => self.compress_pq8(embedding, *subvectors, *centroids),
            CompressionLevel::PQ4 {
                subvectors,
                outlier_threshold,
            } => self.compress_pq4(embedding, *subvectors, *outlier_threshold),
            CompressionLevel::Binary { threshold } => self.compress_binary(embedding, *threshold),
        }
    }

    /// Decompress a compressed tensor
    pub fn decompress(&self, compressed: &CompressedTensor) -> Result<Vec<f32>> {
        match compressed {
            CompressedTensor::Full { data } => Ok(data.clone()),
            CompressedTensor::Half { data, scale, dim } => self.decompress_half(data, *scale, *dim),
            CompressedTensor::PQ8 {
                codes,
                codebooks,
                subvector_dim,
                dim,
            } => self.decompress_pq8(codes, codebooks, *subvector_dim, *dim),
            CompressedTensor::PQ4 {
                codes,
                codebooks,
                outliers,
                subvector_dim,
                dim,
            } => self.decompress_pq4(codes, codebooks, outliers, *subvector_dim, *dim),
            CompressedTensor::Binary {
                bits,
                threshold,
                dim,
            } => self.decompress_binary(bits, *threshold, *dim),
        }
    }

    /// Select compression level based on access frequency
    ///
    /// Thresholds:
    /// - f > 0.8: None (hot data)
    /// - f > 0.4: Half (warm data)
    /// - f > 0.1: PQ8 (cool data)
    /// - f > 0.01: PQ4 (cold data)
    /// - f <= 0.01: Binary (archive)
    fn select_level(&self, access_freq: f32) -> CompressionLevel {
        if access_freq > 0.8 {
            CompressionLevel::None
        } else if access_freq > 0.4 {
            CompressionLevel::Half { scale: 1.0 }
        } else if access_freq > 0.1 {
            CompressionLevel::PQ8 {
                subvectors: 8,
                centroids: 16,
            }
        } else if access_freq > 0.01 {
            CompressionLevel::PQ4 {
                subvectors: 8,
                outlier_threshold: 3.0,
            }
        } else {
            CompressionLevel::Binary { threshold: 0.0 }
        }
    }

    // === Compression implementations ===

    fn compress_none(&self, embedding: &[f32]) -> Result<CompressedTensor> {
        Ok(CompressedTensor::Full {
            data: embedding.to_vec(),
        })
    }

    fn compress_half(&self, embedding: &[f32], scale: f32) -> Result<CompressedTensor> {
        // Simple half precision: scale and convert to 16-bit
        let data: Vec<u16> = embedding
            .iter()
            .map(|&x| {
                let scaled = x * scale;
                let clamped = scaled.clamp(-65504.0, 65504.0);
                // Convert to half precision representation
                f32_to_f16_bits(clamped)
            })
            .collect();

        Ok(CompressedTensor::Half {
            data,
            scale,
            dim: embedding.len(),
        })
    }

    fn compress_pq8(
        &self,
        embedding: &[f32],
        subvectors: u8,
        centroids: u8,
    ) -> Result<CompressedTensor> {
        let dim = embedding.len();
        let subvectors = subvectors as usize;

        if dim % subvectors != 0 {
            return Err(GnnError::InvalidInput(format!(
                "Dimension {} not divisible by subvectors {}",
                dim, subvectors
            )));
        }

        let subvector_dim = dim / subvectors;
        let mut codes = Vec::with_capacity(subvectors);
        let mut codebooks = Vec::with_capacity(subvectors);

        // For each subvector, create a codebook and quantize
        for i in 0..subvectors {
            let start = i * subvector_dim;
            let end = start + subvector_dim;
            let subvector = &embedding[start..end];

            // Simple k-means clustering (k=centroids)
            let (codebook, code) = self.quantize_subvector(subvector, centroids as usize);
            codes.push(code);
            codebooks.push(codebook);
        }

        Ok(CompressedTensor::PQ8 {
            codes,
            codebooks,
            subvector_dim,
            dim,
        })
    }

    fn compress_pq4(
        &self,
        embedding: &[f32],
        subvectors: u8,
        outlier_threshold: f32,
    ) -> Result<CompressedTensor> {
        let dim = embedding.len();
        let subvectors = subvectors as usize;

        if dim % subvectors != 0 {
            return Err(GnnError::InvalidInput(format!(
                "Dimension {} not divisible by subvectors {}",
                dim, subvectors
            )));
        }

        let subvector_dim = dim / subvectors;
        let mut codes = Vec::with_capacity(subvectors);
        let mut codebooks = Vec::with_capacity(subvectors);
        let mut outliers = Vec::new();

        // Detect outliers based on magnitude
        let mean = embedding.iter().sum::<f32>() / dim as f32;
        let std_dev =
            (embedding.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / dim as f32).sqrt();

        // For each subvector
        for i in 0..subvectors {
            let start = i * subvector_dim;
            let end = start + subvector_dim;
            let subvector = &embedding[start..end];

            // Extract outliers
            let mut cleaned_subvector = subvector.to_vec();
            for (j, &val) in subvector.iter().enumerate() {
                if (val - mean).abs() > outlier_threshold * std_dev {
                    outliers.push((start + j, val));
                    cleaned_subvector[j] = mean; // Replace with mean
                }
            }

            // Quantize to 4-bit (16 centroids)
            let (codebook, code) = self.quantize_subvector(&cleaned_subvector, 16);
            codes.push(code);
            codebooks.push(codebook);
        }

        Ok(CompressedTensor::PQ4 {
            codes,
            codebooks,
            outliers,
            subvector_dim,
            dim,
        })
    }

    fn compress_binary(&self, embedding: &[f32], threshold: f32) -> Result<CompressedTensor> {
        let dim = embedding.len();
        let num_bytes = (dim + 7) / 8;
        let mut bits = vec![0u8; num_bytes];

        for (i, &val) in embedding.iter().enumerate() {
            if val > threshold {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits[byte_idx] |= 1 << bit_idx;
            }
        }

        Ok(CompressedTensor::Binary {
            bits,
            threshold,
            dim,
        })
    }

    // === Decompression implementations ===

    fn decompress_half(&self, data: &[u16], scale: f32, dim: usize) -> Result<Vec<f32>> {
        if data.len() != dim {
            return Err(GnnError::InvalidInput(format!(
                "Dimension mismatch: expected {}, got {}",
                dim,
                data.len()
            )));
        }

        Ok(data
            .iter()
            .map(|&bits| f16_bits_to_f32(bits) / scale)
            .collect())
    }

    fn decompress_pq8(
        &self,
        codes: &[u8],
        codebooks: &[Vec<f32>],
        subvector_dim: usize,
        dim: usize,
    ) -> Result<Vec<f32>> {
        let subvectors = codes.len();
        let expected_dim = subvectors * subvector_dim;

        if expected_dim != dim {
            return Err(GnnError::InvalidInput(format!(
                "Dimension mismatch: expected {}, got {}",
                dim, expected_dim
            )));
        }

        let mut result = Vec::with_capacity(dim);

        for (code, codebook) in codes.iter().zip(codebooks.iter()) {
            let centroid_idx = *code as usize;
            if centroid_idx >= codebook.len() / subvector_dim {
                return Err(GnnError::InvalidInput(format!(
                    "Invalid centroid index: {}",
                    centroid_idx
                )));
            }

            let start = centroid_idx * subvector_dim;
            let end = start + subvector_dim;
            result.extend_from_slice(&codebook[start..end]);
        }

        Ok(result)
    }

    fn decompress_pq4(
        &self,
        codes: &[u8],
        codebooks: &[Vec<f32>],
        outliers: &[(usize, f32)],
        subvector_dim: usize,
        dim: usize,
    ) -> Result<Vec<f32>> {
        // First decompress using PQ8 logic
        let mut result = self.decompress_pq8(codes, codebooks, subvector_dim, dim)?;

        // Restore outliers
        for &(idx, val) in outliers {
            if idx < result.len() {
                result[idx] = val;
            }
        }

        Ok(result)
    }

    fn decompress_binary(&self, bits: &[u8], _threshold: f32, dim: usize) -> Result<Vec<f32>> {
        let expected_bytes = (dim + 7) / 8;
        if bits.len() != expected_bytes {
            return Err(GnnError::InvalidInput(format!(
                "Dimension mismatch: expected {} bytes, got {}",
                expected_bytes,
                bits.len()
            )));
        }

        let mut result = Vec::with_capacity(dim);

        for i in 0..dim {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let is_set = (bits[byte_idx] & (1 << bit_idx)) != 0;
            result.push(if is_set { 1.0 } else { -1.0 });
        }

        Ok(result)
    }

    // === Helper methods ===

    /// Simple quantization using k-means-like approach
    fn quantize_subvector(&self, subvector: &[f32], k: usize) -> (Vec<f32>, u8) {
        let dim = subvector.len();

        // Initialize centroids using simple range-based approach
        let min_val = subvector.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = subvector.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = max_val - min_val;

        if range < 1e-6 {
            // All values are essentially the same
            let codebook = vec![min_val; dim * k];
            return (codebook, 0);
        }

        // Create k centroids evenly spaced across the range
        let centroids: Vec<Vec<f32>> = (0..k)
            .map(|i| {
                let offset = min_val + (i as f32 / k as f32) * range;
                vec![offset; dim]
            })
            .collect();

        // Find nearest centroid for this subvector
        let code = self.nearest_centroid(subvector, &centroids);

        // Flatten codebook
        let codebook: Vec<f32> = centroids.into_iter().flatten().collect();

        (codebook, code as u8)
    }

    fn nearest_centroid(&self, subvector: &[f32], centroids: &[Vec<f32>]) -> usize {
        centroids
            .iter()
            .enumerate()
            .map(|(i, centroid)| {
                let dist: f32 = subvector
                    .iter()
                    .zip(centroid.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();
                (i, dist)
            })
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

// === Half precision conversion helpers ===

/// Convert f32 to f16 bits (simplified implementation)
fn f32_to_f16_bits(value: f32) -> u16 {
    // Simple conversion: scale to 16-bit range
    // This is a simplified version, not IEEE 754 half precision
    let scaled = (value * 1000.0).clamp(-32768.0, 32767.0);
    ((scaled as i32) + 32768) as u16
}

/// Convert f16 bits to f32 (simplified implementation)
fn f16_bits_to_f32(bits: u16) -> f32 {
    // Reverse of f32_to_f16_bits
    let value = bits as i32 - 32768;
    value as f32 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_none() {
        let compressor = TensorCompress::new();
        let embedding = vec![1.0, 2.0, 3.0, 4.0];

        let compressed = compressor.compress(&embedding, 1.0).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(embedding, decompressed);
    }

    #[test]
    fn test_compress_half() {
        let compressor = TensorCompress::new();
        let embedding = vec![1.0, 2.0, 3.0, 4.0];

        let compressed = compressor.compress(&embedding, 0.5).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        // Half precision should be close but not exact
        for (a, b) in embedding.iter().zip(decompressed.iter()) {
            assert!((a - b).abs() < 0.01, "Expected {}, got {}", a, b);
        }
    }

    #[test]
    fn test_compress_binary() {
        let compressor = TensorCompress::new();
        let embedding = vec![1.0, -1.0, 0.5, -0.5];

        let compressed = compressor.compress(&embedding, 0.005).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        // Binary should be +1 or -1
        assert_eq!(decompressed.len(), embedding.len());
        for val in &decompressed {
            assert!(*val == 1.0 || *val == -1.0);
        }
    }

    #[test]
    fn test_select_level() {
        let compressor = TensorCompress::new();

        // Hot data
        assert!(matches!(
            compressor.select_level(0.9),
            CompressionLevel::None
        ));

        // Warm data
        assert!(matches!(
            compressor.select_level(0.5),
            CompressionLevel::Half { .. }
        ));

        // Cool data
        assert!(matches!(
            compressor.select_level(0.2),
            CompressionLevel::PQ8 { .. }
        ));

        // Cold data
        assert!(matches!(
            compressor.select_level(0.05),
            CompressionLevel::PQ4 { .. }
        ));

        // Archive
        assert!(matches!(
            compressor.select_level(0.001),
            CompressionLevel::Binary { .. }
        ));
    }

    #[test]
    fn test_empty_embedding() {
        let compressor = TensorCompress::new();
        let result = compressor.compress(&[], 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_pq8_compression() {
        let compressor = TensorCompress::new();
        let embedding: Vec<f32> = (0..64).map(|i| i as f32 * 0.1).collect();

        let compressed = compressor.compress_pq8(&embedding, 8, 16).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.len(), embedding.len());
    }

    #[test]
    fn test_round_trip_all_levels() {
        let compressor = TensorCompress::new();
        let embedding: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) * 0.01).collect();

        let access_frequencies = vec![0.9, 0.5, 0.2, 0.05, 0.001];

        for freq in access_frequencies {
            let compressed = compressor.compress(&embedding, freq).unwrap();
            let decompressed = compressor.decompress(&compressed).unwrap();
            assert_eq!(decompressed.len(), embedding.len());
        }
    }

    #[test]
    fn test_half_precision_roundtrip() {
        let compressor = TensorCompress::new();
        // Use values within the supported range (-32.768 to 32.767)
        let values = vec![-30.0, -1.0, 0.0, 1.0, 30.0];

        for val in values {
            let embedding = vec![val; 4];
            let compressed = compressor
                .compress_with_level(&embedding, &CompressionLevel::Half { scale: 1.0 })
                .unwrap();
            let decompressed = compressor.decompress(&compressed).unwrap();

            for (a, b) in embedding.iter().zip(decompressed.iter()) {
                let diff = (a - b).abs();
                assert!(
                    diff < 0.1,
                    "Value {} decompressed to {}, diff: {}",
                    a,
                    b,
                    diff
                );
            }
        }
    }

    #[test]
    fn test_binary_threshold() {
        let compressor = TensorCompress::new();
        let embedding = vec![0.5, -0.5, 1.5, -1.5];

        let compressed = compressor
            .compress_with_level(&embedding, &CompressionLevel::Binary { threshold: 0.0 })
            .unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        // Values > 0 should be 1.0, values <= 0 should be -1.0
        assert_eq!(decompressed, vec![1.0, -1.0, 1.0, -1.0]);
    }

    #[test]
    fn test_pq4_with_outliers() {
        let compressor = TensorCompress::new();
        // Create embedding with some outliers
        let mut embedding: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
        embedding[10] = 100.0; // Outlier
        embedding[30] = -100.0; // Outlier

        let compressed = compressor
            .compress_with_level(
                &embedding,
                &CompressionLevel::PQ4 {
                    subvectors: 8,
                    outlier_threshold: 2.0,
                },
            )
            .unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed.len(), embedding.len());
        // Outliers should be preserved
        assert_eq!(decompressed[10], 100.0);
        assert_eq!(decompressed[30], -100.0);
    }

    #[test]
    fn test_dimension_validation() {
        let compressor = TensorCompress::new();
        let embedding = vec![1.0; 10]; // Not divisible by 8

        let result = compressor.compress_pq8(&embedding, 8, 16);
        assert!(result.is_err());
    }
}
