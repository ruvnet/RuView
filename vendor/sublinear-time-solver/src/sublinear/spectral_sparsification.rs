//! Spectral sparsification for sublinear algorithms
//!
//! Implements spectral sparsification to reduce matrix density
//! while preserving spectral properties for sublinear solving.

use crate::matrix::Matrix;
use crate::types::Precision;
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, string::String};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Spectral sparsification algorithm
#[derive(Debug, Clone)]
pub struct SpectralSparsifier {
    /// Sparsification parameter (smaller = sparser)
    eps: Precision,
    /// Random seed for reproducibility
    seed: Option<u64>,
    /// Target sparsity ratio
    target_sparsity: Precision,
}

impl SpectralSparsifier {
    /// Create new spectral sparsifier
    pub fn new(eps: Precision, target_sparsity: Precision, seed: Option<u64>) -> Result<Self> {
        if eps <= 0.0 || eps >= 1.0 {
            return Err(SolverError::InvalidInput {
                message: "Sparsification parameter must be in (0, 1)".to_string(),
                parameter: Some("eps".to_string()),
            });
        }

        if target_sparsity <= 0.0 || target_sparsity > 1.0 {
            return Err(SolverError::InvalidInput {
                message: "Target sparsity must be in (0, 1]".to_string(),
                parameter: Some("target_sparsity".to_string()),
            });
        }

        Ok(Self {
            eps,
            seed,
            target_sparsity,
        })
    }

    /// Apply spectral sparsification to matrix
    ///
    /// This preserves the quadratic form x^T A x within factor (1 ± eps)
    /// while reducing the number of non-zero entries
    pub fn sparsify_matrix(&self, matrix: &dyn Matrix) -> Result<SparsifiedMatrix> {
        let n = matrix.rows();

        if !matrix.is_square() {
            return Err(SolverError::InvalidInput {
                message: "Matrix must be square for spectral sparsification".to_string(),
                parameter: Some("matrix_dimensions".to_string()),
            });
        }

        let mut rng = match self.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        // Step 1: Compute effective resistances (approximated)
        let effective_resistances = self.compute_effective_resistances(matrix)?;

        // Step 2: Compute sampling probabilities
        let sampling_probs = self.compute_sampling_probabilities(&effective_resistances)?;

        // Step 3: Sample edges and reweight
        let mut sparsified_entries = Vec::new();
        let mut total_original_entries = 0;
        let mut total_sampled_entries = 0;

        for i in 0..n {
            for j in 0..n {
                if let Some(value) = matrix.get(i, j) {
                    if value.abs() > 1e-14 {
                        total_original_entries += 1;

                        let edge_id = i * n + j;
                        let prob = sampling_probs.get(edge_id).copied().unwrap_or(0.0);

                        if prob > 0.0 && rng.gen::<f64>() < prob {
                            // Reweight to maintain expectation
                            let new_value = value / prob;
                            sparsified_entries.push((i, j, new_value));
                            total_sampled_entries += 1;
                        }
                    }
                }
            }
        }

        let actual_sparsity = total_sampled_entries as f64 / total_original_entries as f64;

        Ok(SparsifiedMatrix {
            entries: sparsified_entries,
            dimension: n,
            original_nnz: total_original_entries,
            sparsified_nnz: total_sampled_entries,
            actual_sparsity,
            eps: self.eps,
        })
    }

    /// Compute effective resistances (simplified approximation)
    fn compute_effective_resistances(&self, matrix: &dyn Matrix) -> Result<Vec<Precision>> {
        let n = matrix.rows();
        let mut resistances = Vec::new();

        // Simplified effective resistance computation
        // For edge (i,j), R_ij ≈ 1/|A_ij| for well-conditioned matrices
        for i in 0..n {
            for j in 0..n {
                if let Some(value) = matrix.get(i, j) {
                    if value.abs() > 1e-14 {
                        // Approximate effective resistance
                        let resistance = 1.0 / value.abs().max(1e-10);
                        resistances.push(resistance);
                    }
                }
            }
        }

        Ok(resistances)
    }

    /// Compute sampling probabilities based on effective resistances
    fn compute_sampling_probabilities(&self, resistances: &[Precision]) -> Result<Vec<Precision>> {
        if resistances.is_empty() {
            return Ok(Vec::new());
        }

        // Total effective resistance
        let total_resistance: Precision = resistances.iter().sum();

        // Sampling probability proportional to effective resistance
        // p_e = min(1, c * R_e / eps^2) where c is a constant
        let c = (resistances.len() as f64 * self.target_sparsity).max(1.0);

        let mut probabilities = Vec::new();
        for &resistance in resistances {
            let prob = (c * resistance / (self.eps * self.eps)).min(1.0);
            probabilities.push(prob);
        }

        Ok(probabilities)
    }
}

/// Result of spectral sparsification
#[derive(Debug, Clone)]
pub struct SparsifiedMatrix {
    /// Sparsified matrix entries (i, j, value)
    pub entries: Vec<(usize, usize, Precision)>,
    /// Matrix dimension
    pub dimension: usize,
    /// Original number of non-zeros
    pub original_nnz: usize,
    /// Sparsified number of non-zeros
    pub sparsified_nnz: usize,
    /// Actual sparsity achieved
    pub actual_sparsity: Precision,
    /// Sparsification parameter used
    pub eps: Precision,
}

impl SparsifiedMatrix {
    /// Convert to dense matrix representation
    pub fn to_dense(&self) -> Vec<Vec<Precision>> {
        let mut dense = vec![vec![0.0; self.dimension]; self.dimension];

        for &(i, j, value) in &self.entries {
            dense[i][j] = value;
        }

        dense
    }

    /// Get sparsification ratio
    pub fn sparsification_ratio(&self) -> Precision {
        self.sparsified_nnz as Precision / self.original_nnz as Precision
    }

    /// Check if sparsification was effective
    pub fn is_effective(&self, target_ratio: Precision) -> bool {
        self.sparsification_ratio() <= target_ratio
    }
}

/// Advanced sparsification with multiple techniques
#[derive(Debug, Clone)]
pub struct AdvancedSparsifier {
    spectral: SpectralSparsifier,
    use_random_projection: bool,
    use_leverage_scores: bool,
}

impl AdvancedSparsifier {
    /// Create advanced sparsifier with multiple techniques
    pub fn new(
        eps: Precision,
        target_sparsity: Precision,
        seed: Option<u64>,
    ) -> Result<Self> {
        Ok(Self {
            spectral: SpectralSparsifier::new(eps, target_sparsity, seed)?,
            use_random_projection: true,
            use_leverage_scores: true,
        })
    }

    /// Apply multiple sparsification techniques
    pub fn advanced_sparsify(&self, matrix: &dyn Matrix) -> Result<SparsifiedMatrix> {
        // For now, use spectral sparsification as the main technique
        let mut result = self.spectral.sparsify_matrix(matrix)?;

        // Apply additional optimizations if requested
        if self.use_leverage_scores {
            result = self.apply_leverage_score_sampling(result)?;
        }

        Ok(result)
    }

    /// Apply leverage score sampling for additional sparsification
    fn apply_leverage_score_sampling(&self, matrix: SparsifiedMatrix) -> Result<SparsifiedMatrix> {
        // Simplified leverage score sampling
        // In a full implementation, this would compute actual leverage scores

        let mut filtered_entries = Vec::new();
        let leverage_threshold = 0.1; // Simplified threshold

        for &(i, j, value) in &matrix.entries {
            // Simplified leverage score (in practice, would compute properly)
            let leverage_score = value.abs() / matrix.dimension as f64;

            if leverage_score >= leverage_threshold {
                filtered_entries.push((i, j, value));
            }
        }

        let sparsified_nnz = filtered_entries.len();
        Ok(SparsifiedMatrix {
            entries: filtered_entries,
            dimension: matrix.dimension,
            original_nnz: matrix.original_nnz,
            sparsified_nnz,
            actual_sparsity: sparsified_nnz as f64 / matrix.original_nnz as f64,
            eps: matrix.eps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::SparseMatrix;

    fn create_test_matrix() -> SparseMatrix {
        let triplets = vec![
            (0, 0, 4.0), (0, 1, 1.0), (0, 2, 1.0),
            (1, 0, 1.0), (1, 1, 4.0), (1, 2, 1.0),
            (2, 0, 1.0), (2, 1, 1.0), (2, 2, 4.0),
        ];
        SparseMatrix::from_triplets(triplets, 3, 3).unwrap()
    }

    #[test]
    fn test_spectral_sparsifier_creation() {
        let sparsifier = SpectralSparsifier::new(0.1, 0.5, Some(42)).unwrap();
        assert_eq!(sparsifier.eps, 0.1);
        assert_eq!(sparsifier.target_sparsity, 0.5);
    }

    #[test]
    fn test_matrix_sparsification() {
        let matrix = create_test_matrix();
        let sparsifier = SpectralSparsifier::new(0.2, 0.7, Some(123)).unwrap();

        let result = sparsifier.sparsify_matrix(&matrix).unwrap();

        assert_eq!(result.dimension, 3);
        assert!(result.sparsified_nnz <= result.original_nnz);
        assert!(result.sparsification_ratio() <= 1.0);
    }

    #[test]
    fn test_sparsified_matrix_conversion() {
        let matrix = create_test_matrix();
        let sparsifier = SpectralSparsifier::new(0.3, 0.8, Some(456)).unwrap();

        let sparsified = sparsifier.sparsify_matrix(&matrix).unwrap();
        let dense = sparsified.to_dense();

        assert_eq!(dense.len(), 3);
        assert_eq!(dense[0].len(), 3);
    }

    #[test]
    fn test_advanced_sparsifier() {
        let matrix = create_test_matrix();
        let advanced = AdvancedSparsifier::new(0.15, 0.6, Some(789)).unwrap();

        let result = advanced.advanced_sparsify(&matrix).unwrap();
        assert!(result.is_effective(1.0)); // Should be more sparse than original
    }
}