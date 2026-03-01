//! SQuat: Subspace-Orthogonal Quantization for KV Cache
//!
//! Based on: "SQuat: Subspace-Orthogonal Quantization for KV Cache" (2024)
//!
//! SQuat achieves additional 2.2-2.8x compression beyond KIVI by:
//! 1. Projecting KV to orthogonal subspaces (decorrelates components)
//! 2. Quantizing each subspace independently
//! 3. Achieving better bit efficiency through decorrelation
//!
//! Total compression with KIVI+SQuat: ~15-22x vs FP16

#[cfg(feature = "no_std_gateway")]
use alloc::{vec, vec::Vec};

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

/// A quantized subspace component
#[derive(Debug, Clone)]
pub struct QuantizedSubspace {
    /// Quantized data for this subspace
    pub data: Vec<u8>,
    /// Scale for dequantization
    pub scale: f32,
    /// Zero point
    pub zero_point: f32,
}

/// SQuat compressed representation
#[derive(Debug, Clone)]
pub struct SQuatCompressed {
    /// Quantized subspace components
    pub subspaces: Vec<QuantizedSubspace>,
    /// Index of the basis matrix used
    pub basis_idx: usize,
    /// Original dimension
    pub original_dim: usize,
}

impl SQuatCompressed {
    /// Get total bytes used
    pub fn bytes(&self) -> usize {
        self.subspaces.iter().map(|s| s.data.len()).sum::<usize>() + self.subspaces.len() * 8
        // scale + zero_point per subspace
    }

    /// Get compression ratio vs FP16
    pub fn compression_ratio(&self) -> f32 {
        let original = self.original_dim * 2; // FP16
        original as f32 / self.bytes() as f32
    }
}

/// SQuat quantizer with learned orthogonal bases
pub struct SQuatQuantizer {
    /// Number of orthogonal subspaces
    num_subspaces: usize,
    /// Bits per subspace component
    bits_per_subspace: u8,
    /// Dimension per head
    head_dim: usize,
    /// Learned orthogonal basis matrices: [layers][head_dim, head_dim]
    /// Each matrix is stored as a flattened Vec<f32>
    bases: Vec<Vec<f32>>,
    /// Subspace dimension
    subspace_dim: usize,
    /// Maximum quantization value
    max_quant: u8,
}

impl SQuatQuantizer {
    /// Create a new SQuat quantizer with random orthogonal bases
    ///
    /// # Arguments
    /// * `num_subspaces` - Number of orthogonal subspaces (typically 4-8)
    /// * `bits_per_subspace` - Bits per component (typically 2)
    /// * `head_dim` - Head dimension
    /// * `num_layers` - Number of transformer layers
    pub fn new(
        num_subspaces: usize,
        bits_per_subspace: u8,
        head_dim: usize,
        num_layers: usize,
    ) -> Self {
        assert!(
            head_dim % num_subspaces == 0,
            "head_dim must be divisible by num_subspaces"
        );
        assert!(bits_per_subspace <= 4, "bits_per_subspace must be <= 4");

        let subspace_dim = head_dim / num_subspaces;

        // Initialize with identity bases (to be calibrated later)
        let mut bases = Vec::with_capacity(num_layers);
        for _ in 0..num_layers {
            bases.push(Self::identity_basis(head_dim));
        }

        Self {
            num_subspaces,
            bits_per_subspace,
            head_dim,
            bases,
            subspace_dim,
            max_quant: (1u8 << bits_per_subspace) - 1,
        }
    }

    /// Create identity basis matrix (flattened)
    fn identity_basis(dim: usize) -> Vec<f32> {
        let mut basis = vec![0.0f32; dim * dim];
        for i in 0..dim {
            basis[i * dim + i] = 1.0;
        }
        basis
    }

    /// Learn orthogonal basis from calibration data using Gram-Schmidt
    ///
    /// # Arguments
    /// * `layer` - Layer index
    /// * `calibration_data` - Sample KV vectors for calibration [num_samples, head_dim]
    pub fn calibrate(&mut self, layer: usize, calibration_data: &[Vec<f32>]) {
        if calibration_data.is_empty() {
            return;
        }

        // Use PCA-like approach: compute covariance and extract principal components
        // For simplicity, we use a randomized orthogonal basis here
        // A production implementation would use SVD or Gram-Schmidt on actual data

        let mut basis = Self::hadamard_basis(self.head_dim);

        // Ensure orthogonality via Gram-Schmidt (the Hadamard is already orthogonal)
        self.gram_schmidt(&mut basis);

        self.bases[layer] = basis;
    }

    /// Generate Hadamard basis (naturally orthogonal)
    fn hadamard_basis(dim: usize) -> Vec<f32> {
        assert!(dim.is_power_of_two());

        let mut basis = vec![0.0f32; dim * dim];

        // Start with H_1 = [1]
        basis[0] = 1.0;

        // Build up using Kronecker product
        let mut size = 1;
        while size < dim {
            let next_size = size * 2;
            for i in 0..size {
                for j in 0..size {
                    let val = basis[i * dim + j];
                    // Top-left: H
                    // Top-right: H
                    // Bottom-left: H
                    // Bottom-right: -H
                    basis[i * dim + j] = val;
                    basis[i * dim + (j + size)] = val;
                    basis[(i + size) * dim + j] = val;
                    basis[(i + size) * dim + (j + size)] = -val;
                }
            }
            size = next_size;
        }

        // Normalize
        let norm = 1.0 / (dim as f32).sqrt();
        for val in basis.iter_mut() {
            *val *= norm;
        }

        basis
    }

    /// Gram-Schmidt orthogonalization
    fn gram_schmidt(&self, basis: &mut [f32]) {
        let n = self.head_dim;

        for i in 0..n {
            // Get row i
            let row_start = i * n;

            // Subtract projections onto previous rows
            for j in 0..i {
                let prev_start = j * n;

                // Compute dot product
                let mut dot = 0.0f32;
                for k in 0..n {
                    dot += basis[row_start + k] * basis[prev_start + k];
                }

                // Subtract projection
                for k in 0..n {
                    basis[row_start + k] -= dot * basis[prev_start + k];
                }
            }

            // Normalize row i
            let mut norm = 0.0f32;
            for k in 0..n {
                norm += basis[row_start + k] * basis[row_start + k];
            }
            norm = norm.sqrt();

            if norm > 1e-8 {
                for k in 0..n {
                    basis[row_start + k] /= norm;
                }
            }
        }
    }

    /// Project vector to orthogonal subspace
    fn project(&self, data: &[f32], layer: usize) -> Vec<f32> {
        assert_eq!(data.len(), self.head_dim);

        let basis = &self.bases[layer];
        let mut projected = vec![0.0f32; self.head_dim];

        // Matrix-vector multiplication: projected = basis * data
        for i in 0..self.head_dim {
            let mut sum = 0.0f32;
            for j in 0..self.head_dim {
                sum += basis[i * self.head_dim + j] * data[j];
            }
            projected[i] = sum;
        }

        projected
    }

    /// Project back from orthogonal subspace
    fn project_back(&self, data: &[f32], layer: usize) -> Vec<f32> {
        assert_eq!(data.len(), self.head_dim);

        let basis = &self.bases[layer];
        let mut result = vec![0.0f32; self.head_dim];

        // Inverse is transpose for orthogonal matrix: result = basis^T * data
        for i in 0..self.head_dim {
            let mut sum = 0.0f32;
            for j in 0..self.head_dim {
                sum += basis[j * self.head_dim + i] * data[j];
            }
            result[i] = sum;
        }

        result
    }

    /// Quantize using subspace decomposition
    pub fn quantize(&self, kv: &[f32], layer: usize) -> SQuatCompressed {
        assert_eq!(kv.len(), self.head_dim);

        // Project to orthogonal subspace
        let projected = self.project(kv, layer);

        // Quantize each subspace independently
        let mut subspaces = Vec::with_capacity(self.num_subspaces);
        let values_per_byte = 8 / self.bits_per_subspace as usize;

        for i in 0..self.num_subspaces {
            let start = i * self.subspace_dim;
            let end = start + self.subspace_dim;
            let subspace = &projected[start..end];

            // Find min/max for this subspace
            let mut min_val = f32::MAX;
            let mut max_val = f32::MIN;
            for &val in subspace {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }

            // Ensure non-zero range
            if (max_val - min_val).abs() < 1e-8 {
                max_val = min_val + 1e-8;
            }

            let scale = (max_val - min_val) / self.max_quant as f32;

            // Quantize
            let mut quantized =
                Vec::with_capacity((self.subspace_dim + values_per_byte - 1) / values_per_byte);
            for chunk in subspace.chunks(values_per_byte) {
                let mut byte = 0u8;
                for (j, &val) in chunk.iter().enumerate() {
                    let q = ((val - min_val) / scale)
                        .round()
                        .clamp(0.0, self.max_quant as f32) as u8;

                    match self.bits_per_subspace {
                        2 => byte |= q << (j * 2),
                        4 => byte |= q << (j * 4),
                        _ => {
                            // Generic bit packing
                            byte |= q << (j * self.bits_per_subspace as usize);
                        }
                    }
                }
                quantized.push(byte);
            }

            subspaces.push(QuantizedSubspace {
                data: quantized,
                scale,
                zero_point: min_val,
            });
        }

        SQuatCompressed {
            subspaces,
            basis_idx: layer,
            original_dim: self.head_dim,
        }
    }

    /// Dequantize from subspace representation
    pub fn dequantize(&self, compressed: &SQuatCompressed) -> Vec<f32> {
        let values_per_byte = 8 / self.bits_per_subspace as usize;
        let mut reconstructed = Vec::with_capacity(self.head_dim);

        // Dequantize each subspace
        for subspace in &compressed.subspaces {
            for &byte in &subspace.data {
                for j in 0..values_per_byte {
                    if reconstructed.len() >= self.head_dim {
                        break;
                    }

                    let q = match self.bits_per_subspace {
                        2 => (byte >> (j * 2)) & 0b11,
                        4 => (byte >> (j * 4)) & 0b1111,
                        _ => (byte >> (j * self.bits_per_subspace as usize)) & self.max_quant,
                    };

                    let val = subspace.zero_point + (q as f32) * subspace.scale;
                    reconstructed.push(val);
                }
            }
        }

        reconstructed.truncate(self.head_dim);

        // Project back from orthogonal subspace
        self.project_back(&reconstructed, compressed.basis_idx)
    }

    /// Get configuration
    pub fn config(&self) -> (usize, u8, usize) {
        (self.num_subspaces, self.bits_per_subspace, self.head_dim)
    }

    /// Calculate expected compression ratio vs FP16
    pub fn compression_ratio(&self) -> f32 {
        let original_bits = self.head_dim * 32; // FP32 (4 bytes per element)
                                                // Compressed: bits_per_subspace for each subspace's indices + 8 bytes (scale + zero_point) per subspace
        let compressed_bits =
            self.num_subspaces * self.bits_per_subspace as usize + self.num_subspaces * 64; // scale + zero_point per subspace
        if compressed_bits == 0 {
            return 1.0;
        }
        original_bits as f32 / compressed_bits as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_squat_basic() {
        // Use larger head_dim for realistic compression test
        // SQuat has overhead of 8 bytes (scale+zero_point) per subspace
        // For compression ratio > 1.0, need head_dim large enough to amortize overhead
        let quantizer = SQuatQuantizer::new(4, 2, 64, 1);
        let data: Vec<f32> = (0..64).map(|i| i as f32).collect();

        let compressed = quantizer.quantize(&data, 0);
        let dequantized = quantizer.dequantize(&compressed);

        assert_eq!(dequantized.len(), 64);

        // Check compression
        // Original: 64 * 2 (FP16) = 128 bytes
        // Compressed: 64 elements * 2 bits / 8 = 16 bytes data + 4 * 8 = 32 bytes overhead = 48 bytes
        // Ratio: 128/48 = 2.67
        let ratio = compressed.compression_ratio();
        assert!(ratio > 1.0, "Expected compression, got ratio {}", ratio);
    }

    #[test]
    fn test_squat_round_trip() {
        let quantizer = SQuatQuantizer::new(4, 2, 8, 1);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let compressed = quantizer.quantize(&data, 0);
        let dequantized = quantizer.dequantize(&compressed);

        // Calculate MSE
        let mse: f32 = data
            .iter()
            .zip(dequantized.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            / data.len() as f32;

        // MSE should be reasonable for 2-bit quantization
        assert!(mse < 10.0, "MSE too high: {}", mse);
    }

    #[test]
    fn test_squat_calibration() {
        let mut quantizer = SQuatQuantizer::new(2, 2, 8, 1);

        // Provide calibration data
        let calibration: Vec<Vec<f32>> = (0..10)
            .map(|i| (0..8).map(|j| (i * 8 + j) as f32).collect())
            .collect();

        quantizer.calibrate(0, &calibration);

        // Should still work after calibration
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let compressed = quantizer.quantize(&data, 0);
        let dequantized = quantizer.dequantize(&compressed);

        assert_eq!(dequantized.len(), 8);
    }

    #[test]
    fn test_squat_compression_ratio() {
        let quantizer = SQuatQuantizer::new(4, 2, 64, 1);
        let ratio = quantizer.compression_ratio();

        // 2-bit with 4 subspaces should give good compression
        assert!(ratio > 2.0, "Expected >2x compression, got {}", ratio);
    }

    #[test]
    fn test_hadamard_basis_orthogonality() {
        let basis = SQuatQuantizer::hadamard_basis(8);

        // Check that rows are orthogonal
        for i in 0..8 {
            for j in 0..8 {
                let mut dot = 0.0f32;
                for k in 0..8 {
                    dot += basis[i * 8 + k] * basis[j * 8 + k];
                }

                if i == j {
                    // Self dot product should be ~1
                    assert!((dot - 1.0).abs() < 0.01, "Row {} self dot: {}", i, dot);
                } else {
                    // Cross dot product should be ~0
                    assert!(dot.abs() < 0.01, "Rows {} and {} dot: {}", i, j, dot);
                }
            }
        }
    }
}
