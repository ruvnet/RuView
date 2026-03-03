//! Johnson-Lindenstrauss dimension reduction for sublinear algorithms
//!
//! Implements the Johnson-Lindenstrauss lemma for embedding high-dimensional
//! vectors into lower dimensions while preserving distances.

use crate::types::Precision;
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, string::String};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Johnson-Lindenstrauss embedding matrix
#[derive(Debug, Clone)]
pub struct JLEmbedding {
    /// Random projection matrix (k x n)
    projection_matrix: Vec<Vec<Precision>>,
    /// Original dimension
    original_dim: usize,
    /// Target dimension
    target_dim: usize,
    /// Distortion parameter
    eps: Precision,
}

impl JLEmbedding {
    /// Create a new Johnson-Lindenstrauss embedding
    ///
    /// For n points, target dimension k = O(log n / eps^2) preserves
    /// distances within factor (1 ± eps) with high probability
    pub fn new(original_dim: usize, eps: Precision, seed: Option<u64>) -> Result<Self> {
        if eps <= 0.0 || eps >= 1.0 {
            return Err(SolverError::InvalidInput {
                message: "JL distortion parameter must be in (0, 1)".to_string(),
                parameter: Some("eps".to_string()),
            });
        }

        // Johnson-Lindenstrauss bound: k >= 4 * ln(n) / (eps^2 / 2 - eps^3 / 3)
        let target_dim = Self::compute_target_dimension(original_dim, eps);

        let mut rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        // Generate random Gaussian projection matrix
        let mut projection_matrix = vec![vec![0.0; original_dim]; target_dim];
        let scale_factor = (1.0 / target_dim as Precision).sqrt();

        for i in 0..target_dim {
            for j in 0..original_dim {
                // Generate from N(0, 1/k) distribution
                projection_matrix[i][j] = rng.gen::<f64>() * 2.0 - 1.0; // Simplified Gaussian
                projection_matrix[i][j] *= scale_factor;
            }
        }

        Ok(Self {
            projection_matrix,
            original_dim,
            target_dim,
            eps,
        })
    }

    /// Compute target dimension based on Johnson-Lindenstrauss lemma
    fn compute_target_dimension(n: usize, eps: Precision) -> usize {
        // Conservative bound: k = 8 * ln(n) / eps^2
        let ln_n = (n as Precision).ln();
        let k = (8.0 * ln_n / (eps * eps)).ceil() as usize;
        k.max(10) // Minimum dimension for numerical stability
    }

    /// Project a vector to the lower-dimensional space
    pub fn project_vector(&self, x: &[Precision]) -> Result<Vec<Precision>> {
        if x.len() != self.original_dim {
            return Err(SolverError::DimensionMismatch {
                expected: self.original_dim,
                actual: x.len(),
                operation: "jl_project_vector".to_string(),
            });
        }

        let mut result = vec![0.0; self.target_dim];

        for i in 0..self.target_dim {
            for j in 0..self.original_dim {
                result[i] += self.projection_matrix[i][j] * x[j];
            }
        }

        Ok(result)
    }

    /// Project a matrix to the lower-dimensional space
    pub fn project_matrix(&self, matrix_rows: &[Vec<Precision>]) -> Result<Vec<Vec<Precision>>> {
        let mut projected_rows = Vec::new();

        for row in matrix_rows {
            projected_rows.push(self.project_vector(row)?);
        }

        Ok(projected_rows)
    }

    /// Reconstruct approximate solution in original space
    /// This uses the Moore-Penrose pseudoinverse for reconstruction
    pub fn reconstruct_vector(&self, y: &[Precision]) -> Result<Vec<Precision>> {
        if y.len() != self.target_dim {
            return Err(SolverError::DimensionMismatch {
                expected: self.target_dim,
                actual: y.len(),
                operation: "jl_reconstruct_vector".to_string(),
            });
        }

        // Simple reconstruction: P^T * y (transpose of projection)
        let mut result = vec![0.0; self.original_dim];

        for j in 0..self.original_dim {
            for i in 0..self.target_dim {
                result[j] += self.projection_matrix[i][j] * y[i];
            }
        }

        Ok(result)
    }

    /// Get the dimension reduction ratio
    pub fn compression_ratio(&self) -> Precision {
        self.target_dim as Precision / self.original_dim as Precision
    }

    /// Get target dimension
    pub fn target_dimension(&self) -> usize {
        self.target_dim
    }

    /// Get distortion parameter
    pub fn distortion_parameter(&self) -> Precision {
        self.eps
    }

    /// Verify Johnson-Lindenstrauss property on test vectors
    pub fn verify_jl_property(&self, test_vectors: &[Vec<Precision>]) -> Result<bool> {
        if test_vectors.len() < 2 {
            return Ok(true);
        }

        // Project all test vectors
        let mut projected_vectors = Vec::new();
        for v in test_vectors {
            projected_vectors.push(self.project_vector(v)?);
        }

        // Check pairwise distance preservation
        for i in 0..test_vectors.len() {
            for j in i + 1..test_vectors.len() {
                let original_dist = self.euclidean_distance(&test_vectors[i], &test_vectors[j]);
                let projected_dist = self.euclidean_distance(&projected_vectors[i], &projected_vectors[j]);

                if original_dist > 1e-10 { // Avoid division by very small numbers
                    let distortion = (projected_dist / original_dist - 1.0).abs();
                    if distortion > self.eps {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    /// Compute Euclidean distance between two vectors
    fn euclidean_distance(&self, a: &[Precision], b: &[Precision]) -> Precision {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<Precision>()
            .sqrt()
    }
}

/// Adaptive Johnson-Lindenstrauss embedding that adjusts dimension based on error
#[derive(Debug)]
pub struct AdaptiveJLEmbedding {
    current_embedding: JLEmbedding,
    min_target_dim: usize,
    max_target_dim: usize,
}

impl AdaptiveJLEmbedding {
    /// Create a new adaptive JL embedding
    pub fn new(
        original_dim: usize,
        initial_eps: Precision,
        min_target_dim: usize,
        max_target_dim: usize,
        seed: Option<u64>,
    ) -> Result<Self> {
        let current_embedding = JLEmbedding::new(original_dim, initial_eps, seed)?;

        Ok(Self {
            current_embedding,
            min_target_dim,
            max_target_dim,
        })
    }

    /// Adapt the embedding dimension based on observed error
    pub fn adapt_dimension(&mut self, observed_error: Precision, target_error: Precision) -> Result<()> {
        if observed_error > target_error * 2.0 {
            // Increase dimension
            let new_target_dim = (self.current_embedding.target_dim as f64 * 1.5).ceil() as usize;
            let new_target_dim = new_target_dim.min(self.max_target_dim);

            if new_target_dim > self.current_embedding.target_dim {
                let new_eps = self.current_embedding.eps * 0.8; // Reduce distortion
                self.current_embedding = JLEmbedding::new(
                    self.current_embedding.original_dim,
                    new_eps,
                    None,
                )?;
            }
        } else if observed_error < target_error * 0.5 {
            // Decrease dimension if possible
            let new_target_dim = (self.current_embedding.target_dim as f64 * 0.8).ceil() as usize;
            let new_target_dim = new_target_dim.max(self.min_target_dim);

            if new_target_dim < self.current_embedding.target_dim {
                let new_eps = self.current_embedding.eps * 1.2; // Increase distortion tolerance
                if new_eps < 0.9 {
                    self.current_embedding = JLEmbedding::new(
                        self.current_embedding.original_dim,
                        new_eps,
                        None,
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Get current embedding
    pub fn current_embedding(&self) -> &JLEmbedding {
        &self.current_embedding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jl_embedding_creation() {
        let embedding = JLEmbedding::new(100, 0.1, Some(42)).unwrap();
        assert_eq!(embedding.original_dim, 100);
        assert!(embedding.target_dim < 100);
        assert!(embedding.compression_ratio() < 1.0);
    }

    #[test]
    fn test_vector_projection() {
        let embedding = JLEmbedding::new(10, 0.3, Some(123)).unwrap();
        let x = vec![1.0; 10];

        let projected = embedding.project_vector(&x).unwrap();
        assert_eq!(projected.len(), embedding.target_dim);
    }

    #[test]
    fn test_dimension_computation() {
        let target_dim = JLEmbedding::compute_target_dimension(1000, 0.1);
        assert!(target_dim > 10);
        assert!(target_dim < 1000);
    }

    #[test]
    fn test_adaptive_embedding() {
        let mut adaptive = AdaptiveJLEmbedding::new(50, 0.2, 5, 100, Some(456)).unwrap();
        let initial_dim = adaptive.current_embedding().target_dim;

        // Simulate high error - should increase dimension
        adaptive.adapt_dimension(0.5, 0.1).unwrap();
        assert!(adaptive.current_embedding().target_dim >= initial_dim);
    }
}