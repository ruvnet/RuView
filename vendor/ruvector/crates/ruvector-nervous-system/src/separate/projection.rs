//! Sparse random projection for dimensionality expansion
//!
//! Implements sparse random matrices for efficient high-dimensional projections
//! with controlled sparsity (connection probability).

use crate::{NervousSystemError, Result};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Sparse random projection matrix for dimensionality expansion
///
/// Uses a sparse random matrix to project low-dimensional inputs into
/// high-dimensional space while maintaining computational efficiency.
///
/// # Properties
///
/// - Sparse connectivity (typically 10-20% connections)
/// - Gaussian-distributed weights
/// - Deterministic (seeded) for reproducibility
///
/// # Performance
///
/// - Time complexity: O(input_dim × output_dim × sparsity)
/// - Space complexity: O(input_dim × output_dim)
#[derive(Debug, Clone)]
pub struct SparseProjection {
    /// Projection weights [input_dim × output_dim]
    weights: Vec<Vec<f32>>,

    /// Connection probability (0.0 to 1.0)
    sparsity: f32,

    /// Random seed for reproducibility
    seed: u64,

    /// Input dimension
    input_dim: usize,

    /// Output dimension
    output_dim: usize,
}

impl SparseProjection {
    /// Create a new sparse random projection
    ///
    /// # Arguments
    ///
    /// * `input_dim` - Input vector dimension
    /// * `output_dim` - Output vector dimension (should be >> input_dim)
    /// * `sparsity` - Connection probability (typically 0.1-0.2)
    /// * `seed` - Random seed for reproducibility
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::SparseProjection;
    ///
    /// let projection = SparseProjection::new(128, 10000, 0.15, 42);
    /// ```
    pub fn new(input_dim: usize, output_dim: usize, sparsity: f32, seed: u64) -> Result<Self> {
        if input_dim == 0 {
            return Err(NervousSystemError::InvalidDimension(
                "Input dimension must be > 0".to_string(),
            ));
        }

        if output_dim == 0 {
            return Err(NervousSystemError::InvalidDimension(
                "Output dimension must be > 0".to_string(),
            ));
        }

        if sparsity <= 0.0 || sparsity > 1.0 {
            return Err(NervousSystemError::InvalidSparsity(format!(
                "Sparsity must be in (0, 1], got {}",
                sparsity
            )));
        }

        let mut rng = StdRng::seed_from_u64(seed);
        let mut weights = Vec::with_capacity(input_dim);

        // Initialize sparse random weights
        for _ in 0..input_dim {
            let mut row = Vec::with_capacity(output_dim);
            for _ in 0..output_dim {
                if rng.gen::<f32>() < sparsity {
                    // Gaussian random weight
                    let weight: f32 = rng.gen_range(-1.0..1.0);
                    row.push(weight);
                } else {
                    row.push(0.0);
                }
            }
            weights.push(row);
        }

        Ok(Self {
            weights,
            sparsity,
            seed,
            input_dim,
            output_dim,
        })
    }

    /// Project input vector to high-dimensional space
    ///
    /// # Arguments
    ///
    /// * `input` - Input vector of size input_dim
    ///
    /// # Returns
    ///
    /// Output vector of size output_dim
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::SparseProjection;
    ///
    /// let projection = SparseProjection::new(128, 10000, 0.15, 42).unwrap();
    /// let input = vec![1.0; 128];
    /// let output = projection.project(&input).unwrap();
    /// assert_eq!(output.len(), 10000);
    /// ```
    pub fn project(&self, input: &[f32]) -> Result<Vec<f32>> {
        if input.len() != self.input_dim {
            return Err(NervousSystemError::DimensionMismatch {
                expected: self.input_dim,
                actual: input.len(),
            });
        }

        let mut output = vec![0.0; self.output_dim];

        // Matrix-vector multiplication: output = weights^T × input
        for i in 0..self.input_dim {
            let input_val = input[i];
            if input_val != 0.0 {
                for j in 0..self.output_dim {
                    let weight = self.weights[i][j];
                    if weight != 0.0 {
                        output[j] += input_val * weight;
                    }
                }
            }
        }

        Ok(output)
    }

    /// Get input dimension
    pub fn input_dim(&self) -> usize {
        self.input_dim
    }

    /// Get output dimension
    pub fn output_dim(&self) -> usize {
        self.output_dim
    }

    /// Get sparsity level
    pub fn sparsity(&self) -> f32 {
        self.sparsity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_projection_creation() {
        let proj = SparseProjection::new(128, 1000, 0.15, 42).unwrap();
        assert_eq!(proj.input_dim(), 128);
        assert_eq!(proj.output_dim(), 1000);
        assert_eq!(proj.sparsity(), 0.15);
    }

    #[test]
    fn test_invalid_dimensions() {
        assert!(SparseProjection::new(0, 1000, 0.15, 42).is_err());
        assert!(SparseProjection::new(128, 0, 0.15, 42).is_err());
    }

    #[test]
    fn test_invalid_sparsity() {
        assert!(SparseProjection::new(128, 1000, 0.0, 42).is_err());
        assert!(SparseProjection::new(128, 1000, 1.5, 42).is_err());
    }

    #[test]
    fn test_projection_dimensions() {
        let proj = SparseProjection::new(128, 1000, 0.15, 42).unwrap();
        let input = vec![1.0; 128];
        let output = proj.project(&input).unwrap();
        assert_eq!(output.len(), 1000);
    }

    #[test]
    fn test_projection_dimension_mismatch() {
        let proj = SparseProjection::new(128, 1000, 0.15, 42).unwrap();
        let input = vec![1.0; 64]; // Wrong size
        assert!(proj.project(&input).is_err());
    }

    #[test]
    fn test_projection_deterministic() {
        let proj1 = SparseProjection::new(128, 1000, 0.15, 42).unwrap();
        let proj2 = SparseProjection::new(128, 1000, 0.15, 42).unwrap();

        let input = vec![1.0; 128];
        let output1 = proj1.project(&input).unwrap();
        let output2 = proj2.project(&input).unwrap();

        // Same seed should produce same results
        assert_eq!(output1, output2);
    }

    #[test]
    fn test_projection_sparsity_effect() {
        let proj_sparse = SparseProjection::new(128, 1000, 0.1, 42).unwrap();
        let proj_dense = SparseProjection::new(128, 1000, 0.9, 42).unwrap();

        let input = vec![1.0; 128];
        let output_sparse = proj_sparse.project(&input).unwrap();
        let output_dense = proj_dense.project(&input).unwrap();

        // Dense projection should have larger average magnitude
        // (more connections contributing to each output)
        let avg_sparse: f32 = output_sparse.iter().map(|x| x.abs()).sum::<f32>() / 1000.0;
        let avg_dense: f32 = output_dense.iter().map(|x| x.abs()).sum::<f32>() / 1000.0;

        // 0.9 sparsity means 9x more connections, so roughly sqrt(9) = 3x larger magnitude
        assert!(
            avg_dense > avg_sparse,
            "Dense avg={} should be > sparse avg={}",
            avg_dense,
            avg_sparse
        );
    }

    #[test]
    fn test_zero_input_produces_zero_output() {
        let proj = SparseProjection::new(128, 1000, 0.15, 42).unwrap();
        let input = vec![0.0; 128];
        let output = proj.project(&input).unwrap();

        assert!(output.iter().all(|&x| x == 0.0));
    }
}
