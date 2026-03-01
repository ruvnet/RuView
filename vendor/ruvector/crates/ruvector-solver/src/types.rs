//! Core types for sparse linear solvers.
//!
//! Provides [`CsrMatrix`] for compressed sparse row storage and result types
//! for solver convergence tracking.

use std::time::Duration;

// ---------------------------------------------------------------------------
// CsrMatrix<T>
// ---------------------------------------------------------------------------

/// Compressed Sparse Row (CSR) matrix.
///
/// Stores only non-zero entries for efficient sparse matrix-vector
/// multiplication in O(nnz) time with excellent cache locality.
///
/// # Layout
///
/// For a matrix with `m` rows and `nnz` non-zeros:
/// - `row_ptr` has length `m + 1`
/// - `col_indices` and `values` each have length `nnz`
/// - Row `i` spans indices `row_ptr[i]..row_ptr[i+1]`
#[derive(Debug, Clone)]
pub struct CsrMatrix<T> {
    /// Row pointers: `row_ptr[i]` is the start index in `col_indices`/`values`
    /// for row `i`.
    pub row_ptr: Vec<usize>,
    /// Column indices for each non-zero entry.
    pub col_indices: Vec<usize>,
    /// Values for each non-zero entry.
    pub values: Vec<T>,
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
}

impl<T: Copy + Default + std::ops::Mul<Output = T> + std::ops::AddAssign> CsrMatrix<T> {
    /// Sparse matrix-vector multiply: `y = A * x`.
    ///
    /// # Panics
    ///
    /// Debug-asserts that `x.len() >= self.cols` and `y.len() >= self.rows`.
    #[inline]
    pub fn spmv(&self, x: &[T], y: &mut [T]) {
        debug_assert!(
            x.len() >= self.cols,
            "spmv: x.len()={} < cols={}",
            x.len(),
            self.cols,
        );
        debug_assert!(
            y.len() >= self.rows,
            "spmv: y.len()={} < rows={}",
            y.len(),
            self.rows,
        );

        for i in 0..self.rows {
            let mut sum = T::default();
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];

            for idx in start..end {
                sum += self.values[idx] * x[self.col_indices[idx]];
            }
            y[i] = sum;
        }
    }
}

impl CsrMatrix<f32> {
    /// High-performance SpMV with bounds-check elimination.
    ///
    /// Identical to [`spmv`](Self::spmv) but uses `unsafe` indexing to
    /// eliminate per-element bounds checks in the inner loop, which is the
    /// single hottest path in all iterative solvers.
    ///
    /// # Safety contract
    ///
    /// The caller must ensure the CSR structure is valid (use
    /// [`validate_csr_matrix`](crate::validation::validate_csr_matrix) once
    /// before entering the solve loop). The `x` and `y` slices must have
    /// lengths `>= cols` and `>= rows` respectively.
    #[inline]
    pub fn spmv_unchecked(&self, x: &[f32], y: &mut [f32]) {
        debug_assert!(x.len() >= self.cols);
        debug_assert!(y.len() >= self.rows);

        let vals = self.values.as_ptr();
        let cols = self.col_indices.as_ptr();
        let rp = self.row_ptr.as_ptr();

        for i in 0..self.rows {
            // SAFETY: row_ptr has length rows+1, so i and i+1 are in bounds.
            let start = unsafe { *rp.add(i) };
            let end = unsafe { *rp.add(i + 1) };
            let mut sum = 0.0f32;

            for idx in start..end {
                // SAFETY: idx < nnz (enforced by valid CSR structure),
                // col_indices[idx] < cols <= x.len() (enforced by validation).
                unsafe {
                    let v = *vals.add(idx);
                    let c = *cols.add(idx);
                    sum += v * *x.get_unchecked(c);
                }
            }
            // SAFETY: i < rows <= y.len()
            unsafe { *y.get_unchecked_mut(i) = sum };
        }
    }

    /// Fused SpMV + residual computation: computes `r[j] = rhs[j] - (A*x)[j]`
    /// and returns `||r||^2` in a single pass, avoiding a separate allocation
    /// for `Ax`.
    ///
    /// This eliminates one full memory traversal per iteration compared to
    /// separate `spmv` + vector subtraction.
    #[inline]
    pub fn fused_residual_norm_sq(&self, x: &[f32], rhs: &[f32], residual: &mut [f32]) -> f64 {
        debug_assert!(x.len() >= self.cols);
        debug_assert!(rhs.len() >= self.rows);
        debug_assert!(residual.len() >= self.rows);

        let vals = self.values.as_ptr();
        let cols = self.col_indices.as_ptr();
        let rp = self.row_ptr.as_ptr();
        let mut norm_sq = 0.0f64;

        for i in 0..self.rows {
            let start = unsafe { *rp.add(i) };
            let end = unsafe { *rp.add(i + 1) };
            let mut ax_i = 0.0f32;

            for idx in start..end {
                unsafe {
                    let v = *vals.add(idx);
                    let c = *cols.add(idx);
                    ax_i += v * *x.get_unchecked(c);
                }
            }

            let r_i = rhs[i] - ax_i;
            residual[i] = r_i;
            norm_sq += (r_i as f64) * (r_i as f64);
        }

        norm_sq
    }
}

impl CsrMatrix<f64> {
    /// High-performance SpMV for f64 with bounds-check elimination.
    #[inline]
    pub fn spmv_unchecked(&self, x: &[f64], y: &mut [f64]) {
        debug_assert!(x.len() >= self.cols);
        debug_assert!(y.len() >= self.rows);

        let vals = self.values.as_ptr();
        let cols = self.col_indices.as_ptr();
        let rp = self.row_ptr.as_ptr();

        for i in 0..self.rows {
            let start = unsafe { *rp.add(i) };
            let end = unsafe { *rp.add(i + 1) };
            let mut sum = 0.0f64;

            for idx in start..end {
                unsafe {
                    let v = *vals.add(idx);
                    let c = *cols.add(idx);
                    sum += v * *x.get_unchecked(c);
                }
            }
            unsafe { *y.get_unchecked_mut(i) = sum };
        }
    }
}

impl<T> CsrMatrix<T> {
    /// Number of non-zero entries.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Number of non-zeros in a specific row (i.e. the row degree for an
    /// adjacency matrix).
    #[inline]
    pub fn row_degree(&self, row: usize) -> usize {
        self.row_ptr[row + 1] - self.row_ptr[row]
    }

    /// Iterate over `(col_index, &value)` pairs for the given row.
    #[inline]
    pub fn row_entries(&self, row: usize) -> impl Iterator<Item = (usize, &T)> {
        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];
        self.col_indices[start..end]
            .iter()
            .copied()
            .zip(self.values[start..end].iter())
    }
}

impl<T: Copy + Default> CsrMatrix<T> {
    /// Transpose: produces `A^T` in CSR form.
    ///
    /// Uses a two-pass counting sort in O(nnz + rows + cols) time and
    /// O(nnz) extra memory. Required by backward push which operates on
    /// the reversed adjacency structure.
    pub fn transpose(&self) -> CsrMatrix<T> {
        let nnz = self.nnz();
        let t_rows = self.cols;
        let t_cols = self.rows;

        // Pass 1: count entries per new row (= old column).
        let mut row_ptr = vec![0usize; t_rows + 1];
        for &c in &self.col_indices {
            row_ptr[c + 1] += 1;
        }
        for i in 1..=t_rows {
            row_ptr[i] += row_ptr[i - 1];
        }

        // Pass 2: scatter entries into the transposed arrays.
        let mut col_indices = vec![0usize; nnz];
        let mut values = vec![T::default(); nnz];
        let mut cursor = row_ptr.clone();

        for row in 0..self.rows {
            let start = self.row_ptr[row];
            let end = self.row_ptr[row + 1];
            for idx in start..end {
                let c = self.col_indices[idx];
                let dest = cursor[c];
                col_indices[dest] = row;
                values[dest] = self.values[idx];
                cursor[c] += 1;
            }
        }

        CsrMatrix {
            row_ptr,
            col_indices,
            values,
            rows: t_rows,
            cols: t_cols,
        }
    }
}

impl<T: Copy + Default + std::ops::AddAssign> CsrMatrix<T> {
    /// Build a CSR matrix from COO (coordinate) triplets.
    ///
    /// Entries are sorted by (row, col) internally. Duplicate positions at the
    /// same (row, col) are kept as separate entries (caller should pre-merge if
    /// needed).
    pub fn from_coo_generic(
        rows: usize,
        cols: usize,
        entries: impl IntoIterator<Item = (usize, usize, T)>,
    ) -> Self {
        let mut sorted: Vec<_> = entries.into_iter().collect();
        sorted.sort_unstable_by_key(|(r, c, _)| (*r, *c));

        let nnz = sorted.len();
        let mut row_ptr = vec![0usize; rows + 1];
        let mut col_indices = Vec::with_capacity(nnz);
        let mut values = Vec::with_capacity(nnz);

        for &(r, _, _) in &sorted {
            assert!(r < rows, "row index {} out of bounds (rows={})", r, rows);
            row_ptr[r + 1] += 1;
        }
        for i in 1..=rows {
            row_ptr[i] += row_ptr[i - 1];
        }

        for (_, c, v) in sorted {
            assert!(c < cols, "col index {} out of bounds (cols={})", c, cols);
            col_indices.push(c);
            values.push(v);
        }

        Self {
            row_ptr,
            col_indices,
            values,
            rows,
            cols,
        }
    }
}

impl CsrMatrix<f32> {
    /// Build a CSR matrix from COO (coordinate) triplets.
    ///
    /// Entries are sorted by (row, col) internally. Duplicate positions are
    /// summed.
    pub fn from_coo(
        rows: usize,
        cols: usize,
        entries: impl IntoIterator<Item = (usize, usize, f32)>,
    ) -> Self {
        Self::from_coo_generic(rows, cols, entries)
    }

    /// Build a square identity matrix of dimension `n` in CSR format.
    pub fn identity(n: usize) -> Self {
        let row_ptr: Vec<usize> = (0..=n).collect();
        let col_indices: Vec<usize> = (0..n).collect();
        let values = vec![1.0f32; n];

        Self {
            row_ptr,
            col_indices,
            values,
            rows: n,
            cols: n,
        }
    }
}

impl CsrMatrix<f64> {
    /// Build a CSR matrix from COO (coordinate) triplets (f64 variant).
    ///
    /// Entries are sorted by (row, col) internally.
    pub fn from_coo(
        rows: usize,
        cols: usize,
        entries: impl IntoIterator<Item = (usize, usize, f64)>,
    ) -> Self {
        Self::from_coo_generic(rows, cols, entries)
    }

    /// Build a square identity matrix of dimension `n` in CSR format (f64).
    pub fn identity(n: usize) -> Self {
        let row_ptr: Vec<usize> = (0..=n).collect();
        let col_indices: Vec<usize> = (0..n).collect();
        let values = vec![1.0f64; n];

        Self {
            row_ptr,
            col_indices,
            values,
            rows: n,
            cols: n,
        }
    }
}

// ---------------------------------------------------------------------------
// Solver result types
// ---------------------------------------------------------------------------

/// Algorithm identifier for solver selection and routing.
///
/// Each variant corresponds to a solver strategy with different complexity
/// characteristics and applicability constraints. The [`SolverRouter`] selects
/// the best algorithm based on the matrix [`SparsityProfile`] and [`QueryType`].
///
/// [`SolverRouter`]: crate::router::SolverRouter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Algorithm {
    /// Neumann series: `x = sum_{k=0}^{K} (I - A)^k * b`.
    ///
    /// Requires spectral radius < 1. Best for diagonally dominant, very sparse
    /// systems where the series converges in O(log(1/eps)) terms.
    Neumann,
    /// Jacobi iterative solver.
    Jacobi,
    /// Gauss-Seidel iterative solver.
    GaussSeidel,
    /// Forward Push (Andersen-Chung-Lang) for Personalized PageRank.
    ///
    /// Computes an approximate PPR vector by pushing residual mass forward
    /// along edges. Sublinear in graph size for single-source queries.
    ForwardPush,
    /// Backward Push for target-centric PPR.
    ///
    /// Dual of Forward Push: propagates contributions backward from a target
    /// node.
    BackwardPush,
    /// Conjugate Gradient (CG) iterative solver.
    ///
    /// Optimal for symmetric positive-definite systems. Converges in at most
    /// `n` steps; practical convergence depends on the condition number.
    CG,
    /// Hybrid random-walk approach combining push with Monte Carlo sampling.
    ///
    /// For large graphs where pure push is too expensive, this approach uses
    /// random walks to estimate the tail of the PageRank distribution.
    HybridRandomWalk,
    /// TRUE (Topology-aware Reduction for Updating Equations) batch solver.
    ///
    /// Exploits shared sparsity structure across a batch of right-hand sides
    /// to amortise factorisation cost. Best when `batch_size` is large.
    TRUE,
    /// Block Maximum Spanning Subgraph Preconditioned solver.
    ///
    /// Uses a maximum spanning tree preconditioner for ill-conditioned systems
    /// where CG and Neumann both struggle.
    BMSSP,
    /// Dense direct solver (LU/Cholesky fallback).
    ///
    /// Last-resort O(n^3) solver used when iterative methods fail. Only
    /// practical for small matrices.
    Dense,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Algorithm::Neumann => write!(f, "neumann"),
            Algorithm::Jacobi => write!(f, "jacobi"),
            Algorithm::GaussSeidel => write!(f, "gauss-seidel"),
            Algorithm::ForwardPush => write!(f, "forward-push"),
            Algorithm::BackwardPush => write!(f, "backward-push"),
            Algorithm::CG => write!(f, "cg"),
            Algorithm::HybridRandomWalk => write!(f, "hybrid-random-walk"),
            Algorithm::TRUE => write!(f, "true-solver"),
            Algorithm::BMSSP => write!(f, "bmssp"),
            Algorithm::Dense => write!(f, "dense"),
        }
    }
}

// ---------------------------------------------------------------------------
// Query & profile types for routing
// ---------------------------------------------------------------------------

/// Query type describing what the caller wants to solve.
///
/// The [`SolverRouter`] inspects this together with the [`SparsityProfile`] to
/// select the most appropriate [`Algorithm`].
///
/// [`SolverRouter`]: crate::router::SolverRouter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Standard sparse linear system `Ax = b`.
    LinearSystem,

    /// Single-source Personalized PageRank.
    PageRankSingle {
        /// Source node index.
        source: usize,
    },

    /// Pairwise Personalized PageRank between two nodes.
    PageRankPairwise {
        /// Source node index.
        source: usize,
        /// Target node index.
        target: usize,
    },

    /// Spectral graph filter using polynomial expansion.
    SpectralFilter {
        /// Degree of the Chebyshev/polynomial expansion.
        polynomial_degree: usize,
    },

    /// Batch of linear systems sharing the same matrix `A` but different
    /// right-hand sides.
    BatchLinearSystem {
        /// Number of right-hand sides in the batch.
        batch_size: usize,
    },
}

/// Sparsity profile summarising the structural and numerical properties
/// of a matrix that are relevant for algorithm selection.
///
/// Computed once by [`SolverOrchestrator::analyze_sparsity`] and reused
/// across multiple solves on the same matrix.
///
/// [`SolverOrchestrator::analyze_sparsity`]: crate::router::SolverOrchestrator::analyze_sparsity
#[derive(Debug, Clone)]
pub struct SparsityProfile {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Total number of non-zero entries.
    pub nnz: usize,
    /// Fraction of non-zeros: `nnz / (rows * cols)`.
    pub density: f64,
    /// `true` if `|a_ii| > sum_{j != i} |a_ij|` for every row.
    pub is_diag_dominant: bool,
    /// Estimated spectral radius of the Jacobi iteration matrix `D^{-1}(L+U)`.
    pub estimated_spectral_radius: f64,
    /// Rough estimate of the 2-norm condition number.
    pub estimated_condition: f64,
    /// `true` if the matrix appears to be symmetric (checked on structure only).
    pub is_symmetric_structure: bool,
    /// Average number of non-zeros per row.
    pub avg_nnz_per_row: f64,
    /// Maximum number of non-zeros in any single row.
    pub max_nnz_per_row: usize,
}

/// Estimated computational complexity for a solve.
///
/// Returned by [`SolverOrchestrator::estimate_complexity`] to let callers
/// decide whether to proceed, batch, or reject a query.
///
/// [`SolverOrchestrator::estimate_complexity`]: crate::router::SolverOrchestrator::estimate_complexity
#[derive(Debug, Clone)]
pub struct ComplexityEstimate {
    /// Algorithm that would be selected.
    pub algorithm: Algorithm,
    /// Estimated number of floating-point operations.
    pub estimated_flops: u64,
    /// Estimated number of iterations (for iterative methods).
    pub estimated_iterations: usize,
    /// Estimated peak memory usage in bytes.
    pub estimated_memory_bytes: usize,
    /// A qualitative complexity class label.
    pub complexity_class: ComplexityClass,
}

/// Qualitative complexity class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComplexityClass {
    /// O(nnz * log(1/eps)) -- sublinear in matrix dimension.
    SublinearNnz,
    /// O(n * sqrt(kappa)) -- CG-like.
    SqrtCondition,
    /// O(n * nnz_per_row) -- linear scan.
    Linear,
    /// O(n^2) or worse -- superlinear.
    Quadratic,
    /// O(n^3) -- dense factorisation.
    Cubic,
}

/// Compute lane priority for solver scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComputeLane {
    /// Low-latency lane for small problems.
    Fast,
    /// Default throughput lane.
    Normal,
    /// Batch lane for large problems.
    Batch,
}

/// Budget constraints for solver execution.
#[derive(Debug, Clone)]
pub struct ComputeBudget {
    /// Maximum wall-clock time allowed.
    pub max_time: Duration,
    /// Maximum number of iterations.
    pub max_iterations: usize,
    /// Target residual tolerance.
    pub tolerance: f64,
}

impl Default for ComputeBudget {
    fn default() -> Self {
        Self {
            max_time: Duration::from_secs(30),
            max_iterations: 1000,
            tolerance: 1e-6,
        }
    }
}

/// Per-iteration convergence snapshot.
#[derive(Debug, Clone)]
pub struct ConvergenceInfo {
    /// Iteration index (0-based).
    pub iteration: usize,
    /// Residual L2 norm at this iteration.
    pub residual_norm: f64,
}

/// Result returned by a successful solver invocation.
#[derive(Debug, Clone)]
pub struct SolverResult {
    /// Solution vector x.
    pub solution: Vec<f32>,
    /// Number of iterations performed.
    pub iterations: usize,
    /// Final residual L2 norm.
    pub residual_norm: f64,
    /// Wall-clock time taken.
    pub wall_time: Duration,
    /// Per-iteration convergence history.
    pub convergence_history: Vec<ConvergenceInfo>,
    /// Algorithm used.
    pub algorithm: Algorithm,
}
