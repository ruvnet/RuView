//! BMSSP (Bounded Min-Cut Sparse Solver Paradigm) multigrid solver.
//!
//! Implements an algebraic multigrid (AMG) V-cycle solver for Laplacian and
//! symmetric positive-definite sparse systems. The setup phase builds a
//! coarsening hierarchy via aggregation-based coarsening; the solve phase
//! applies recursive V-cycles with Gauss-Seidel smoothing.
//!
//! Asymptotic complexity: O(nnz * log n) per solve for well-conditioned
//! Laplacian systems.
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvector_solver::bmssp::BmsspSolver;
//! use ruvector_solver::traits::SolverEngine;
//! use ruvector_solver::types::{ComputeBudget, CsrMatrix};
//!
//! let matrix = CsrMatrix::<f64>::from_coo(3, 3, vec![
//!     (0, 0, 4.0), (0, 1, -1.0), (0, 2, -1.0),
//!     (1, 0, -1.0), (1, 1, 4.0), (1, 2, -1.0),
//!     (2, 0, -1.0), (2, 1, -1.0), (2, 2, 4.0),
//! ]);
//! let rhs = vec![1.0, 2.0, 3.0];
//! let budget = ComputeBudget::default();
//!
//! let solver = BmsspSolver::new(1e-8, 200);
//! let result = solver.solve(&matrix, &rhs, &budget).unwrap();
//! assert!(result.residual_norm < 1e-6);
//! ```

use std::time::Instant;

use tracing::{debug, trace};

use crate::error::{SolverError, ValidationError};
use crate::traits::SolverEngine;
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, ConvergenceInfo, CsrMatrix,
    SolverResult, SparsityProfile,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default strong-connection threshold for coarsening (theta).
const STRONG_THRESHOLD: f64 = 0.25;

/// Number of Gauss-Seidel sweeps for pre- and post-smoothing.
const SMOOTH_STEPS: usize = 3;

/// Maximum matrix dimension for a direct (dense) solve at the coarsest level.
const COARSEST_DIRECT_LIMIT: usize = 100;

/// Target aggregate size during aggregation-based coarsening.
const TARGET_AGGREGATE_SIZE: usize = 4;

/// Threshold below which a scalar is treated as zero for division safety.
/// Standardized across solver modules (matches neumann.rs and cg.rs).
const NEAR_ZERO_F64: f64 = 1e-15;

// ---------------------------------------------------------------------------
// Public solver struct
// ---------------------------------------------------------------------------

/// Algebraic multigrid V-cycle solver using the BMSSP paradigm.
///
/// Solves `Ax = b` by constructing a coarsening hierarchy from `A` and then
/// applying V-cycles until the residual norm drops below `tolerance` or the
/// iteration budget is exhausted.
#[derive(Debug, Clone)]
pub struct BmsspSolver {
    /// Target residual tolerance (L2 norm).
    tolerance: f64,
    /// Maximum outer V-cycle iterations.
    max_iterations: usize,
    /// Maximum number of coarsening levels in the hierarchy.
    max_levels: usize,
    /// Target coarsening ratio per level (fraction of fine-level rows).
    coarsening_ratio: f64,
}

impl BmsspSolver {
    /// Create a new solver with the given tolerance and iteration budget.
    ///
    /// Uses default values for `max_levels` (20) and `coarsening_ratio` (0.5).
    pub fn new(tolerance: f64, max_iterations: usize) -> Self {
        Self {
            tolerance,
            max_iterations,
            max_levels: 20,
            coarsening_ratio: 0.5,
        }
    }

    /// Create a solver with full control over all parameters.
    pub fn with_params(
        tolerance: f64,
        max_iterations: usize,
        max_levels: usize,
        coarsening_ratio: f64,
    ) -> Self {
        Self {
            tolerance,
            max_iterations,
            max_levels,
            coarsening_ratio,
        }
    }
}

// ---------------------------------------------------------------------------
// Multigrid hierarchy types (module-private)
// ---------------------------------------------------------------------------

/// A single level of the multigrid hierarchy.
struct MultigridLevel {
    /// System matrix at this level.
    matrix: CsrMatrix<f64>,
    /// Prolongation operator P (maps coarse correction -> fine correction).
    prolongation: CsrMatrix<f64>,
    /// Restriction operator R = P^T (maps fine residual -> coarse residual).
    restriction: CsrMatrix<f64>,
}

/// The complete multigrid hierarchy built during the setup phase.
struct MultigridHierarchy {
    /// Coarsening levels. Level 0 is the original (finest) grid; the last
    /// entry holds the coarsest-level matrix with placeholder transfer ops.
    levels: Vec<MultigridLevel>,
    /// Dimension of the coarsest-level matrix.
    coarsest_size: usize,
}

// ---------------------------------------------------------------------------
// Setup phase: build multigrid hierarchy
// ---------------------------------------------------------------------------

/// Build the multigrid coarsening hierarchy from the input matrix.
///
/// At each level we:
/// 1. Aggregate rows into groups of ~[`TARGET_AGGREGATE_SIZE`].
/// 2. Build a piecewise-constant prolongation operator P.
/// 3. Compute R = P^T.
/// 4. Form the Galerkin coarse matrix `A_{l+1} = R * A_l * P`.
/// 5. Stop when the coarse matrix is small enough or we hit `max_levels`.
fn build_hierarchy(
    matrix: &CsrMatrix<f64>,
    max_levels: usize,
    coarsening_ratio: f64,
) -> Result<MultigridHierarchy, SolverError> {
    let mut levels: Vec<MultigridLevel> = Vec::new();
    let mut current = matrix.clone();

    for lvl in 0..max_levels {
        let n = current.rows;
        if n <= COARSEST_DIRECT_LIMIT {
            debug!(level = lvl, size = n, "coarsest level reached");
            break;
        }

        let target_coarse = ((n as f64) * coarsening_ratio).max(1.0) as usize;
        let target_coarse = target_coarse.max(1);

        let aggregates = build_aggregates(&current, target_coarse);
        let num_aggregates = aggregates.iter().copied().max().map_or(0, |m| m + 1);

        if num_aggregates == 0 || num_aggregates >= n {
            debug!(
                level = lvl,
                n, num_aggregates, "coarsening stalled, stopping hierarchy build"
            );
            break;
        }

        let prolongation = build_prolongation(n, num_aggregates, &aggregates);
        let restriction = transpose_csr(&prolongation);

        // Galerkin coarse operator: A_c = R * A * P.
        let ap = sparse_matmul(&current, &prolongation);
        let coarse_matrix = sparse_matmul(&restriction, &ap);

        trace!(
            level = lvl,
            fine = n,
            coarse = coarse_matrix.rows,
            nnz = coarse_matrix.nnz(),
            "built multigrid level"
        );

        levels.push(MultigridLevel {
            matrix: current,
            prolongation,
            restriction,
        });

        current = coarse_matrix;
    }

    let coarsest_size = current.rows;

    // Store the coarsest-level matrix with placeholder transfer operators.
    levels.push(MultigridLevel {
        matrix: current,
        prolongation: empty_csr(),
        restriction: empty_csr(),
    });

    debug!(
        total_levels = levels.len(),
        coarsest_size, "multigrid hierarchy built"
    );

    Ok(MultigridHierarchy {
        levels,
        coarsest_size,
    })
}

/// Construct a zero-sized CSR matrix (placeholder for unused operators).
#[inline]
fn empty_csr() -> CsrMatrix<f64> {
    CsrMatrix {
        row_ptr: vec![0],
        col_indices: Vec::new(),
        values: Vec::new(),
        rows: 0,
        cols: 0,
    }
}

// ---------------------------------------------------------------------------
// Aggregation-based coarsening
// ---------------------------------------------------------------------------

/// Partition rows into aggregates using a greedy seed-based approach.
///
/// Identifies strong connections (`|a_ij| >= theta * max_j |a_ij|`) and
/// groups each seed with up to [`TARGET_AGGREGATE_SIZE`] strongly-connected
/// neighbours.
///
/// Returns a vector of length `n` where `result[i]` is the aggregate index
/// for row `i`.
fn build_aggregates(matrix: &CsrMatrix<f64>, target_coarse: usize) -> Vec<usize> {
    let n = matrix.rows;
    let mut aggregate_id = vec![usize::MAX; n];
    let mut current_agg: usize = 0;

    // Per-row maximum off-diagonal magnitude (for strong-connection test).
    let max_off_diag: Vec<f64> = (0..n)
        .map(|i| {
            let start = matrix.row_ptr[i];
            let end = matrix.row_ptr[i + 1];
            let mut max_val: f64 = 0.0;
            for idx in start..end {
                if matrix.col_indices[idx] != i {
                    max_val = max_val.max(matrix.values[idx].abs());
                }
            }
            max_val
        })
        .collect();

    // Greedy seeding: pick unaggregated nodes and gather strong neighbours.
    for seed in 0..n {
        if aggregate_id[seed] != usize::MAX {
            continue;
        }

        aggregate_id[seed] = current_agg;
        let mut agg_size = 1usize;
        let threshold = STRONG_THRESHOLD * max_off_diag[seed];

        let start = matrix.row_ptr[seed];
        let end = matrix.row_ptr[seed + 1];
        for idx in start..end {
            let j = matrix.col_indices[idx];
            if j == seed || aggregate_id[j] != usize::MAX {
                continue;
            }
            if matrix.values[idx].abs() >= threshold {
                aggregate_id[j] = current_agg;
                agg_size += 1;
                if agg_size >= TARGET_AGGREGATE_SIZE {
                    break;
                }
            }
        }

        current_agg += 1;

        if current_agg >= target_coarse && seed > n / 2 {
            break;
        }
    }

    // Sweep up any remaining unaggregated nodes.
    for i in 0..n {
        if aggregate_id[i] != usize::MAX {
            continue;
        }

        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let mut best_agg = if current_agg > 0 { current_agg - 1 } else { 0 };
        let mut best_strength: f64 = -1.0;

        for idx in start..end {
            let j = matrix.col_indices[idx];
            if j != i && aggregate_id[j] != usize::MAX {
                let strength = matrix.values[idx].abs();
                if strength > best_strength {
                    best_strength = strength;
                    best_agg = aggregate_id[j];
                }
            }
        }

        if best_strength < 0.0 {
            // Isolated node: singleton aggregate.
            aggregate_id[i] = current_agg;
            current_agg += 1;
        } else {
            aggregate_id[i] = best_agg;
        }
    }

    aggregate_id
}

/// Build the prolongation operator P from aggregate assignments.
///
/// P is piecewise-constant interpolation: `P[i, agg(i)] = 1`.
fn build_prolongation(
    fine_rows: usize,
    coarse_cols: usize,
    aggregates: &[usize],
) -> CsrMatrix<f64> {
    let mut row_ptr = Vec::with_capacity(fine_rows + 1);
    let mut col_indices = Vec::with_capacity(fine_rows);
    let mut values = Vec::with_capacity(fine_rows);

    row_ptr.push(0);
    for &agg in aggregates.iter().take(fine_rows) {
        col_indices.push(agg);
        values.push(1.0f64);
        row_ptr.push(col_indices.len());
    }

    CsrMatrix {
        row_ptr,
        col_indices,
        values,
        rows: fine_rows,
        cols: coarse_cols,
    }
}

// ---------------------------------------------------------------------------
// Sparse matrix utilities
// ---------------------------------------------------------------------------

/// Transpose a CSR matrix, returning a new CSR representing `A^T`.
fn transpose_csr(a: &CsrMatrix<f64>) -> CsrMatrix<f64> {
    let (m, n, nnz) = (a.rows, a.cols, a.nnz());

    let mut row_counts = vec![0usize; n];
    for &c in &a.col_indices {
        row_counts[c] += 1;
    }

    let mut row_ptr = vec![0usize; n + 1];
    for i in 0..n {
        row_ptr[i + 1] = row_ptr[i] + row_counts[i];
    }

    let mut col_indices = vec![0usize; nnz];
    let mut values = vec![0.0f64; nnz];
    let mut offset = vec![0usize; n];

    for i in 0..m {
        let start = a.row_ptr[i];
        let end = a.row_ptr[i + 1];
        for idx in start..end {
            let j = a.col_indices[idx];
            let pos = row_ptr[j] + offset[j];
            col_indices[pos] = i;
            values[pos] = a.values[idx];
            offset[j] += 1;
        }
    }

    CsrMatrix {
        row_ptr,
        col_indices,
        values,
        rows: n,
        cols: m,
    }
}

/// Sparse-sparse matrix multiplication: `C = A * B` (both in CSR).
///
/// Uses a dense accumulator per row for collecting partial sums, then
/// compresses into CSR. The accumulator is cleared between rows rather
/// than reallocated, making this efficient for the moderate-sized matrices
/// produced during hierarchy construction.
fn sparse_matmul(a: &CsrMatrix<f64>, b: &CsrMatrix<f64>) -> CsrMatrix<f64> {
    assert_eq!(
        a.cols, b.rows,
        "sparse_matmul: dimension mismatch {}x{} * {}x{}",
        a.rows, a.cols, b.rows, b.cols
    );

    let m = a.rows;
    let n = b.cols;
    let mut row_ptr = Vec::with_capacity(m + 1);
    let mut col_indices = Vec::new();
    let mut values = Vec::new();

    let mut acc = vec![0.0f64; n];
    let mut nz_cols: Vec<usize> = Vec::new();

    row_ptr.push(0);

    for i in 0..m {
        let a_start = a.row_ptr[i];
        let a_end = a.row_ptr[i + 1];
        for a_idx in a_start..a_end {
            let k = a.col_indices[a_idx];
            let a_val = a.values[a_idx];
            let b_start = b.row_ptr[k];
            let b_end = b.row_ptr[k + 1];
            for b_idx in b_start..b_end {
                let j = b.col_indices[b_idx];
                if acc[j] == 0.0 {
                    nz_cols.push(j);
                }
                acc[j] += a_val * b.values[b_idx];
            }
        }

        nz_cols.sort_unstable();
        for &j in &nz_cols {
            let v = acc[j];
            if v.abs() > f64::EPSILON {
                col_indices.push(j);
                values.push(v);
            }
            acc[j] = 0.0;
        }
        nz_cols.clear();
        row_ptr.push(col_indices.len());
    }

    CsrMatrix {
        row_ptr,
        col_indices,
        values,
        rows: m,
        cols: n,
    }
}

// ---------------------------------------------------------------------------
// Smoothers
// ---------------------------------------------------------------------------

/// Forward Gauss-Seidel sweep.
///
/// Updates each component in place:
/// `x_i = (b_i - sum_{j != i} a_ij * x_j) / a_ii`.
///
/// Rows with zero diagonal are skipped to avoid division by zero.
#[inline]
fn gauss_seidel_sweep(matrix: &CsrMatrix<f64>, x: &mut [f64], b: &[f64]) {
    let n = matrix.rows;
    for i in 0..n {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let mut sigma = 0.0f64;
        let mut diag = 0.0f64;
        for idx in start..end {
            let j = matrix.col_indices[idx];
            let v = matrix.values[idx];
            if j == i {
                diag = v;
            } else {
                sigma += v * x[j];
            }
        }
        if diag.abs() > NEAR_ZERO_F64 {
            x[i] = (b[i] - sigma) / diag;
        }
    }
}

// ---------------------------------------------------------------------------
// Coarsest-level direct solve
// ---------------------------------------------------------------------------

/// Solve a small dense system via Gaussian elimination with partial pivoting.
///
/// Builds a dense `n x (n+1)` augmented matrix from the sparse input,
/// factors, and back-substitutes. Used only at the coarsest multigrid level
/// where `n <= COARSEST_DIRECT_LIMIT`.
fn dense_direct_solve(matrix: &CsrMatrix<f64>, b: &[f64]) -> Vec<f64> {
    let n = matrix.rows;
    if n == 0 {
        return Vec::new();
    }

    let stride = n + 1;
    let mut aug = vec![0.0f64; n * stride];

    // Populate augmented matrix [A | b].
    for i in 0..n {
        aug[i * stride + n] = b[i];
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        for idx in start..end {
            let j = matrix.col_indices[idx];
            aug[i * stride + j] = matrix.values[idx];
        }
    }

    // Forward elimination with partial pivoting.
    for col in 0..n {
        let mut max_val = aug[col * stride + col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let v = aug[row * stride + col].abs();
            if v > max_val {
                max_val = v;
                max_row = row;
            }
        }

        if max_row != col {
            let (first, second) = if col < max_row {
                let (left, right) = aug.split_at_mut(max_row * stride);
                (
                    &mut left[col * stride..col * stride + stride],
                    &mut right[..stride],
                )
            } else {
                let (left, right) = aug.split_at_mut(col * stride);
                (
                    &mut right[..stride],
                    &mut left[max_row * stride..max_row * stride + stride],
                )
            };
            first.swap_with_slice(second);
        }

        let pivot = aug[col * stride + col];
        if pivot.abs() < NEAR_ZERO_F64 {
            continue;
        }

        for row in (col + 1)..n {
            let factor = aug[row * stride + col] / pivot;
            aug[row * stride + col] = 0.0;
            for k in (col + 1)..stride {
                aug[row * stride + k] -= factor * aug[col * stride + k];
            }
        }
    }

    // Back-substitution.
    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        let mut sum = aug[i * stride + n];
        for j in (i + 1)..n {
            sum -= aug[i * stride + j] * x[j];
        }
        let diag = aug[i * stride + i];
        if diag.abs() > NEAR_ZERO_F64 {
            x[i] = sum / diag;
        }
    }

    x
}

// ---------------------------------------------------------------------------
// V-cycle
// ---------------------------------------------------------------------------

/// Execute one V-cycle starting at the given level.
///
/// Algorithm:
/// 1. Pre-smooth with [`SMOOTH_STEPS`] Gauss-Seidel sweeps.
/// 2. Compute residual `r = b - A * x` and restrict: `r_c = R * r`.
/// 3. Recurse on the coarse level (or direct-solve at coarsest).
/// 4. Prolongate the coarse correction `e = P * e_c` and correct: `x += e`.
/// 5. Post-smooth with [`SMOOTH_STEPS`] Gauss-Seidel sweeps.
fn v_cycle(hierarchy: &MultigridHierarchy, x: &mut [f64], b: &[f64], level: usize) {
    let num_levels = hierarchy.levels.len();
    let mat = &hierarchy.levels[level].matrix;
    let n = mat.rows;

    // Base case: direct solve at the coarsest level.
    if level == num_levels - 1 || n <= COARSEST_DIRECT_LIMIT {
        let sol = dense_direct_solve(mat, b);
        x[..n].copy_from_slice(&sol);
        return;
    }

    let prol = &hierarchy.levels[level].prolongation;
    let rest = &hierarchy.levels[level].restriction;

    // Pre-smoothing.
    for _ in 0..SMOOTH_STEPS {
        gauss_seidel_sweep(mat, x, b);
    }

    // Compute residual: r = b - A * x.
    let mut ax = vec![0.0f64; n];
    mat.spmv(x, &mut ax);
    let residual: Vec<f64> = (0..n).map(|i| b[i] - ax[i]).collect();

    // Restrict to coarse level: r_c = R * r.
    let coarse_n = rest.rows;
    let mut r_coarse = vec![0.0f64; coarse_n];
    rest.spmv(&residual, &mut r_coarse);

    // Recurse.
    let mut e_coarse = vec![0.0f64; coarse_n];
    v_cycle(hierarchy, &mut e_coarse, &r_coarse, level + 1);

    // Prolongate and correct: x += P * e_c.
    let mut correction = vec![0.0f64; n];
    prol.spmv(&e_coarse, &mut correction);
    for i in 0..n {
        x[i] += correction[i];
    }

    // Post-smoothing.
    for _ in 0..SMOOTH_STEPS {
        gauss_seidel_sweep(mat, x, b);
    }
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

/// Validate the matrix and RHS before solving.
fn validate_inputs(matrix: &CsrMatrix<f64>, rhs: &[f64]) -> Result<(), SolverError> {
    if matrix.rows == 0 || matrix.cols == 0 {
        return Err(SolverError::InvalidInput(
            ValidationError::DimensionMismatch("matrix has zero dimension".into()),
        ));
    }
    if matrix.rows != matrix.cols {
        return Err(SolverError::InvalidInput(
            ValidationError::DimensionMismatch(format!(
                "BMSSP requires square matrix, got {}x{}",
                matrix.rows, matrix.cols
            )),
        ));
    }
    if rhs.len() != matrix.rows {
        return Err(SolverError::InvalidInput(
            ValidationError::DimensionMismatch(format!(
                "RHS length {} does not match matrix dimension {}",
                rhs.len(),
                matrix.rows
            )),
        ));
    }
    if matrix.row_ptr.len() != matrix.rows + 1 {
        return Err(SolverError::InvalidInput(
            ValidationError::DimensionMismatch(format!(
                "row_ptr length {} != rows + 1 = {}",
                matrix.row_ptr.len(),
                matrix.rows + 1
            )),
        ));
    }

    for (idx, &v) in matrix.values.iter().enumerate() {
        if !v.is_finite() {
            return Err(SolverError::InvalidInput(ValidationError::NonFiniteValue(
                format!("matrix value at index {idx}"),
            )));
        }
    }
    for (idx, &v) in rhs.iter().enumerate() {
        if !v.is_finite() {
            return Err(SolverError::InvalidInput(ValidationError::NonFiniteValue(
                format!("RHS value at index {idx}"),
            )));
        }
    }

    Ok(())
}

/// Compute the L2 norm of the residual `r = b - A*x`.
fn residual_l2(matrix: &CsrMatrix<f64>, x: &[f64], b: &[f64]) -> f64 {
    let n = matrix.rows;
    let mut ax = vec![0.0f64; n];
    matrix.spmv(x, &mut ax);
    let mut sum_sq = 0.0f64;
    for i in 0..n {
        let r = b[i] - ax[i];
        sum_sq += r * r;
    }
    sum_sq.sqrt()
}

// ---------------------------------------------------------------------------
// SolverEngine implementation
// ---------------------------------------------------------------------------

impl SolverEngine for BmsspSolver {
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        validate_inputs(matrix, rhs)?;

        let start = Instant::now();
        let n = matrix.rows;

        // Honour both our own config and the caller's budget.
        let tol = self.tolerance.min(budget.tolerance);
        let max_iter = self.max_iterations.min(budget.max_iterations);

        // Trivial case: 1x1 system.
        if n == 1 {
            let diag = if matrix.nnz() > 0 {
                matrix.values[0]
            } else {
                0.0
            };
            if diag.abs() <= f64::EPSILON {
                return Err(SolverError::NumericalInstability {
                    iteration: 0,
                    detail: "1x1 system with zero diagonal".into(),
                });
            }
            return Ok(SolverResult {
                solution: vec![(rhs[0] / diag) as f32],
                iterations: 0,
                residual_norm: 0.0,
                wall_time: start.elapsed(),
                convergence_history: Vec::new(),
                algorithm: Algorithm::BMSSP,
            });
        }

        // Small systems: skip hierarchy, solve directly.
        if n <= COARSEST_DIRECT_LIMIT {
            debug!(n, "small system, using direct solve");
            let sol = dense_direct_solve(matrix, rhs);
            let res = residual_l2(matrix, &sol, rhs);
            return Ok(SolverResult {
                solution: sol.iter().map(|&v| v as f32).collect(),
                iterations: 0,
                residual_norm: res,
                wall_time: start.elapsed(),
                convergence_history: Vec::new(),
                algorithm: Algorithm::BMSSP,
            });
        }

        // Setup phase: build the multigrid hierarchy.
        let hierarchy = build_hierarchy(matrix, self.max_levels, self.coarsening_ratio)?;

        trace!(
            levels = hierarchy.levels.len(),
            coarsest = hierarchy.coarsest_size,
            "AMG hierarchy ready, starting V-cycle iterations"
        );

        // Solve phase: iterate V-cycles.
        let mut x = vec![0.0f64; n];
        let mut ax_buf = vec![0.0f64; n];
        let mut convergence_history = Vec::with_capacity(max_iter);

        let b_norm = {
            let mut s = 0.0f64;
            for &v in rhs {
                s += v * v;
            }
            s.sqrt()
        };

        // Zero RHS: solution is the zero vector.
        if b_norm < tol {
            return Ok(SolverResult {
                solution: vec![0.0f32; n],
                iterations: 0,
                residual_norm: b_norm,
                wall_time: start.elapsed(),
                convergence_history: Vec::new(),
                algorithm: Algorithm::BMSSP,
            });
        }

        for iter in 0..max_iter {
            if start.elapsed() > budget.max_time {
                return Err(SolverError::BudgetExhausted {
                    reason: format!(
                        "wall-time limit {}ms exceeded at iteration {iter}",
                        budget.max_time.as_millis()
                    ),
                    elapsed: start.elapsed(),
                });
            }

            v_cycle(&hierarchy, &mut x, rhs, 0);

            matrix.spmv(&x, &mut ax_buf);
            let res = (0..n)
                .map(|i| {
                    let r = rhs[i] - ax_buf[i];
                    r * r
                })
                .sum::<f64>()
                .sqrt();

            convergence_history.push(ConvergenceInfo {
                iteration: iter,
                residual_norm: res,
            });

            trace!(iteration = iter, residual = res, "V-cycle completed");

            if !res.is_finite() {
                return Err(SolverError::NumericalInstability {
                    iteration: iter,
                    detail: "residual became NaN or Inf during V-cycle".into(),
                });
            }

            if res < tol {
                debug!(iterations = iter + 1, residual = res, "BMSSP converged");
                return Ok(SolverResult {
                    solution: x.iter().map(|&v| v as f32).collect(),
                    iterations: iter + 1,
                    residual_norm: res,
                    wall_time: start.elapsed(),
                    convergence_history,
                    algorithm: Algorithm::BMSSP,
                });
            }
        }

        let final_residual = convergence_history
            .last()
            .map_or(b_norm, |c| c.residual_norm);

        Err(SolverError::NonConvergence {
            iterations: max_iter,
            residual: final_residual,
            tolerance: tol,
        })
    }

    fn estimate_complexity(&self, profile: &SparsityProfile, n: usize) -> ComplexityEstimate {
        // AMG V-cycle: O(nnz * log n) total work. Expected ~log(n) iterations,
        // each costing O(nnz) for smoothing + transfer.
        let log_n = ((n as f64).ln().max(1.0)) as u64;
        let nnz = profile.nnz as u64;
        let estimated_iters = log_n as usize;
        let flops_per_iter = nnz * 6; // ~6 flops/nnz (smooth + restrict + prolong)
        let total_flops = flops_per_iter * log_n;

        // Memory: hierarchy roughly doubles storage.
        let mem = profile.nnz * 16 + n * 8;

        ComplexityEstimate {
            algorithm: Algorithm::BMSSP,
            estimated_flops: total_flops,
            estimated_iterations: estimated_iters,
            estimated_memory_bytes: mem,
            complexity_class: ComplexityClass::SublinearNnz,
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::BMSSP
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 1D Laplacian (tridiagonal: 2 on diagonal, -1 off-diagonal).
    fn laplacian_1d(n: usize) -> CsrMatrix<f64> {
        let mut entries = Vec::new();
        for i in 0..n {
            entries.push((i, i, 2.0f64));
            if i > 0 {
                entries.push((i, i - 1, -1.0));
            }
            if i + 1 < n {
                entries.push((i, i + 1, -1.0));
            }
        }
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    /// Diagonally dominant 3x3 SPD matrix.
    fn diag_dominant_3x3() -> CsrMatrix<f64> {
        CsrMatrix::<f64>::from_coo(
            3,
            3,
            vec![
                (0, 0, 4.0),
                (0, 1, -1.0),
                (0, 2, -1.0),
                (1, 0, -1.0),
                (1, 1, 4.0),
                (1, 2, -1.0),
                (2, 0, -1.0),
                (2, 1, -1.0),
                (2, 2, 4.0),
            ],
        )
    }

    fn budget() -> ComputeBudget {
        ComputeBudget::default()
    }

    #[test]
    fn solve_small_direct() {
        let matrix = diag_dominant_3x3();
        let rhs = vec![1.0, 2.0, 3.0];
        let solver = BmsspSolver::new(1e-8, 100);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();

        assert!(
            result.residual_norm < 1e-5,
            "residual too high: {}",
            result.residual_norm
        );
        assert_eq!(result.algorithm, Algorithm::BMSSP);
        assert_eq!(result.solution.len(), 3);
    }

    #[test]
    fn solve_1d_laplacian_small() {
        let n = 50;
        let matrix = laplacian_1d(n);
        let rhs = vec![1.0f64; n];
        let solver = BmsspSolver::new(1e-6, 200);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();

        assert!(
            result.residual_norm < 1e-5,
            "residual: {}",
            result.residual_norm
        );
        assert_eq!(result.solution.len(), n);
    }

    #[test]
    fn solve_1d_laplacian_medium() {
        let n = 500;
        let matrix = laplacian_1d(n);
        let rhs: Vec<f64> = (0..n).map(|i| (i as f64) / (n as f64)).collect();
        let solver = BmsspSolver::new(1e-4, 500);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();

        assert!(
            result.residual_norm < 1e-3,
            "residual: {}",
            result.residual_norm
        );
        assert!(result.iterations > 0, "should have iterated");
    }

    #[test]
    fn solve_identity() {
        let n = 10;
        let matrix = CsrMatrix::<f64>::identity(n);
        let rhs: Vec<f64> = (1..=n as i32).map(|i| i as f64).collect();
        let solver = BmsspSolver::new(1e-10, 100);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();

        for i in 0..n {
            assert!(
                (result.solution[i] as f64 - rhs[i]).abs() < 1e-3,
                "mismatch at {}: {} vs {}",
                i,
                result.solution[i],
                rhs[i]
            );
        }
    }

    #[test]
    fn solve_zero_rhs() {
        let matrix = laplacian_1d(10);
        let rhs = vec![0.0f64; 10];
        let solver = BmsspSolver::new(1e-8, 100);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();

        for &v in &result.solution {
            assert!(v.abs() < 1e-6, "expected zero solution, got {v}");
        }
    }

    #[test]
    fn reject_dimension_mismatch_rhs() {
        let matrix = laplacian_1d(5);
        let rhs = vec![1.0f64; 3];
        let solver = BmsspSolver::new(1e-8, 100);
        assert!(solver.solve(&matrix, &rhs, &budget()).is_err());
    }

    #[test]
    fn reject_nonsquare_matrix() {
        let matrix = CsrMatrix {
            row_ptr: vec![0, 1, 2],
            col_indices: vec![0, 1],
            values: vec![1.0f64, 1.0],
            rows: 2,
            cols: 3,
        };
        let rhs = vec![1.0, 1.0];
        let solver = BmsspSolver::new(1e-8, 100);
        assert!(solver.solve(&matrix, &rhs, &budget()).is_err());
    }

    #[test]
    fn solve_1x1_system() {
        let matrix = CsrMatrix::<f64>::from_coo(1, 1, vec![(0, 0, 5.0)]);
        let rhs = vec![10.0];
        let solver = BmsspSolver::new(1e-10, 100);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();
        assert!((result.solution[0] as f64 - 2.0).abs() < 1e-5);
    }

    #[test]
    fn with_params_stores_values() {
        let solver = BmsspSolver::with_params(1e-6, 300, 10, 0.3);
        assert!((solver.tolerance - 1e-6).abs() < f64::EPSILON);
        assert_eq!(solver.max_iterations, 300);
        assert_eq!(solver.max_levels, 10);
        assert!((solver.coarsening_ratio - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn convergence_history_populated() {
        let n = 200;
        let matrix = laplacian_1d(n);
        let rhs = vec![1.0f64; n];
        let solver = BmsspSolver::new(1e-6, 500);
        let result = solver.solve(&matrix, &rhs, &budget()).unwrap();

        assert!(!result.convergence_history.is_empty());
        let first = result.convergence_history.first().unwrap().residual_norm;
        let last = result.convergence_history.last().unwrap().residual_norm;
        assert!(
            last < first || first < 1e-6,
            "residual did not decrease: first={first}, last={last}"
        );
    }

    #[test]
    fn transpose_csr_identity() {
        let id = CsrMatrix::<f64>::identity(5);
        let id_t = transpose_csr(&id);
        assert_eq!(id_t.rows, 5);
        assert_eq!(id_t.cols, 5);
        assert_eq!(id_t.nnz(), 5);
        for i in 0..5 {
            assert_eq!(id_t.col_indices[i], i);
            assert!((id_t.values[i] - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn sparse_matmul_identity() {
        let id = CsrMatrix::<f64>::identity(4);
        let a = CsrMatrix::<f64>::from_coo(
            4,
            4,
            vec![
                (0, 0, 2.0),
                (0, 1, 1.0),
                (1, 1, 3.0),
                (2, 2, 4.0),
                (3, 3, 5.0),
            ],
        );
        let result = sparse_matmul(&id, &a);
        assert_eq!(result.rows, 4);
        assert_eq!(result.cols, 4);
        assert_eq!(result.nnz(), a.nnz());
    }

    #[test]
    fn gauss_seidel_diagonal_system() {
        let matrix = CsrMatrix::<f64>::from_coo(2, 2, vec![(0, 0, 4.0), (1, 1, 4.0)]);
        let b = [8.0f64, 12.0];
        let mut x = [0.0f64; 2];
        gauss_seidel_sweep(&matrix, &mut x, &b);
        assert!((x[0] - 2.0).abs() < 1e-10);
        assert!((x[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn dense_direct_solve_3x3() {
        let matrix = diag_dominant_3x3();
        let rhs = [2.0f64, 2.0, 2.0];
        let x = dense_direct_solve(&matrix, &rhs);
        let mut ax = vec![0.0f64; 3];
        matrix.spmv(&x, &mut ax);
        for i in 0..3 {
            assert!(
                (ax[i] - rhs[i]).abs() < 1e-10,
                "dense solve mismatch at {i}: {} vs {}",
                ax[i],
                rhs[i],
            );
        }
    }

    #[test]
    fn algorithm_returns_bmssp() {
        let solver = BmsspSolver::new(1e-6, 100);
        assert_eq!(solver.algorithm(), Algorithm::BMSSP);
    }

    #[test]
    fn estimate_complexity_reasonable() {
        let solver = BmsspSolver::new(1e-6, 100);
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 5000,
            density: 0.005,
            is_diag_dominant: true,
            estimated_spectral_radius: 0.5,
            estimated_condition: 10.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 5.0,
            max_nnz_per_row: 7,
        };
        let est = solver.estimate_complexity(&profile, 1000);
        assert_eq!(est.algorithm, Algorithm::BMSSP);
        assert_eq!(est.complexity_class, ComplexityClass::SublinearNnz);
        assert!(est.estimated_flops > 0);
        assert!(est.estimated_iterations > 0);
        assert!(est.estimated_memory_bytes > 0);
    }
}
