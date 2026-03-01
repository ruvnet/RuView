//! Solver trait hierarchy.
//!
//! All solver algorithms implement [`SolverEngine`]. Specialised traits
//! ([`SparseLaplacianSolver`], [`SublinearPageRank`]) extend it with
//! domain-specific operations.

use crate::error::SolverError;
use crate::types::{
    Algorithm, ComplexityEstimate, ComputeBudget, CsrMatrix, SolverResult, SparsityProfile,
};

/// Core trait that every solver algorithm must implement.
///
/// A `SolverEngine` accepts a sparse matrix system and a compute budget,
/// returning either a [`SolverResult`] or a structured [`SolverError`].
pub trait SolverEngine: Send + Sync {
    /// Solve the linear system `A x = b` (or the equivalent iterative
    /// problem) subject to the given compute budget.
    ///
    /// # Arguments
    ///
    /// * `matrix` - the sparse coefficient matrix.
    /// * `rhs` - the right-hand side vector `b`.
    /// * `budget` - resource limits for this invocation.
    ///
    /// # Errors
    ///
    /// Returns [`SolverError`] on non-convergence, numerical issues, budget
    /// exhaustion, or invalid input.
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError>;

    /// Estimate the computational cost of solving the given system without
    /// actually performing the solve.
    ///
    /// Implementations should use the [`SparsityProfile`] to make a fast,
    /// heuristic prediction.
    fn estimate_complexity(&self, profile: &SparsityProfile, n: usize) -> ComplexityEstimate;

    /// Return the algorithm identifier for this engine.
    fn algorithm(&self) -> Algorithm;
}

/// Extended trait for solvers that operate on graph Laplacian systems.
///
/// A graph Laplacian `L = D - A` arises naturally in spectral graph theory.
/// Solvers implementing this trait can exploit Laplacian structure (e.g.
/// guaranteed positive semi-definiteness, kernel spanned by the all-ones
/// vector) for faster convergence.
pub trait SparseLaplacianSolver: SolverEngine {
    /// Solve `L x = b` where `L` is a graph Laplacian.
    ///
    /// The solver may add a small regulariser to handle the rank-deficient
    /// case (connected component with zero eigenvalue).
    ///
    /// # Errors
    ///
    /// Returns [`SolverError`] on failure.
    fn solve_laplacian(
        &self,
        laplacian: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError>;

    /// Compute the effective resistance between two nodes.
    ///
    /// Effective resistance `R(s, t) = (e_s - e_t)^T L^+ (e_s - e_t)` is
    /// a fundamental quantity in spectral graph theory.
    fn effective_resistance(
        &self,
        laplacian: &CsrMatrix<f64>,
        source: usize,
        target: usize,
        budget: &ComputeBudget,
    ) -> Result<f64, SolverError>;
}

/// Trait for sublinear-time Personalized PageRank (PPR) algorithms.
///
/// PPR is central to nearest-neighbour search in large graphs. Algorithms
/// implementing this trait run in time proportional to the output size
/// rather than the full graph size.
pub trait SublinearPageRank: Send + Sync {
    /// Compute a sparse approximate PPR vector from a single source node.
    ///
    /// # Arguments
    ///
    /// * `matrix` - column-stochastic transition matrix (or CSR adjacency).
    /// * `source` - index of the source (seed) node.
    /// * `alpha` - teleportation probability (typically 0.15).
    /// * `epsilon` - approximation tolerance; controls output sparsity.
    ///
    /// # Returns
    ///
    /// A vector of `(node_index, ppr_value)` pairs whose values sum to
    /// approximately 1.
    ///
    /// # Errors
    ///
    /// Returns [`SolverError`] on invalid input or budget exhaustion.
    fn ppr(
        &self,
        matrix: &CsrMatrix<f64>,
        source: usize,
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError>;

    /// Compute PPR from a distribution over seed nodes rather than a single
    /// source.
    ///
    /// # Arguments
    ///
    /// * `matrix` - column-stochastic transition matrix.
    /// * `seeds` - `(node_index, weight)` pairs forming the seed distribution.
    /// * `alpha` - teleportation probability.
    /// * `epsilon` - approximation tolerance.
    ///
    /// # Errors
    ///
    /// Returns [`SolverError`] on invalid input or budget exhaustion.
    fn ppr_multi_seed(
        &self,
        matrix: &CsrMatrix<f64>,
        seeds: &[(usize, f64)],
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError>;
}
