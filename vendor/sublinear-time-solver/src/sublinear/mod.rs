//! True sublinear-time algorithms for linear system solving
//!
//! This module implements mathematically rigorous sublinear algorithms
//! that achieve O(log n) complexity under specific conditions.

pub mod dimension_reduction;
pub mod spectral_sparsification;
pub mod sublinear_neumann;
pub mod johnson_lindenstrauss;
pub mod sketching;
pub mod fast_sampling;

use crate::matrix::Matrix;
use crate::types::Precision;
use crate::error::{SolverError, Result};

/// Configuration for sublinear algorithms
#[derive(Debug, Clone)]
pub struct SublinearConfig {
    /// Target dimension after dimension reduction
    pub target_dimension: usize,
    /// Sparsification parameter (0 < eps < 1)
    pub sparsification_eps: Precision,
    /// Johnson-Lindenstrauss distortion parameter
    pub jl_distortion: Precision,
    /// Sampling probability for sketching
    pub sampling_probability: Precision,
    /// Maximum recursion depth
    pub max_recursion_depth: usize,
    /// Base case threshold for recursion
    pub base_case_threshold: usize,
}

impl Default for SublinearConfig {
    fn default() -> Self {
        Self {
            target_dimension: 64,
            sparsification_eps: 0.1,
            jl_distortion: 0.5,
            sampling_probability: 0.01,
            max_recursion_depth: 10,
            base_case_threshold: 100,
        }
    }
}

/// Sublinear complexity bounds for different matrix types
#[derive(Debug, Clone)]
pub enum ComplexityBound {
    /// O(log n) for diagonally dominant matrices
    Logarithmic(usize),
    /// O(sqrt(n)) for well-conditioned matrices
    SquareRoot(usize),
    /// O(n^eps) for general sparse matrices
    Sublinear { n: usize, eps: Precision },
}

/// Trait for algorithms that achieve true sublinear complexity
pub trait SublinearSolver {
    /// Verify that the matrix satisfies conditions for sublinear complexity
    fn verify_sublinear_conditions(&self, matrix: &dyn Matrix) -> Result<ComplexityBound>;

    /// Solve with guaranteed sublinear complexity
    fn solve_sublinear(
        &self,
        matrix: &dyn Matrix,
        b: &[Precision],
        config: &SublinearConfig,
    ) -> Result<Vec<Precision>>;

    /// Get the actual complexity bound achieved
    fn complexity_bound(&self) -> ComplexityBound;
}