//! Conjugate Gradient solver for symmetric positive-definite systems.
//!
//! Solves `Ax = b` where `A` is a symmetric positive-definite (SPD) sparse
//! matrix in CSR format. The algorithm converges in at most `n` iterations
//! for an `n x n` system, but in practice converges in
//! `O(sqrt(kappa) * log(1/eps))` iterations where `kappa = cond(A)`.
//!
//! # Algorithm
//!
//! Implements the Hestenes-Stiefel variant of Conjugate Gradient:
//!
//! ```text
//! r = b - A*x
//! z = M^{-1} * r        (preconditioner; z = r when disabled)
//! p = z
//! rz = r . z
//!
//! for k in 0..max_iterations:
//!     Ap = A * p
//!     alpha = rz / (p . Ap)
//!     x  = x + alpha * p
//!     r  = r - alpha * Ap
//!     if ||r||_2 < tolerance * ||b||_2:
//!         converged; break
//!     z  = M^{-1} * r
//!     rz_new = r . z
//!     beta = rz_new / rz
//!     p  = z + beta * p
//!     rz = rz_new
//! ```
//!
//! # Preconditioning
//!
//! When `use_preconditioner` is `true`, a diagonal (Jacobi) preconditioner is
//! applied: `M = diag(A)`, so that `z_i = r_i / A_{ii}`. This reduces the
//! effective condition number for diagonally-dominant systems without adding
//! significant per-iteration cost.
//!
//! # Numerical precision
//!
//! All dot products and norm computations use `f64` accumulation even though
//! the matrix may store `f32` values, preventing catastrophic cancellation in
//! the inner products that drive the CG recurrence.
//!
//! # Convergence
//!
//! Theoretical bound:
//! `||x_k - x*||_A <= 2 * ((sqrt(kappa) - 1)/(sqrt(kappa) + 1))^k * ||x_0 - x*||_A`
//! where `kappa` is the 2-condition number of `A`.

use std::time::Instant;

use tracing::{debug, trace, warn};

use crate::error::{SolverError, ValidationError};
use crate::traits::SolverEngine;
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, ConvergenceInfo, CsrMatrix,
    SolverResult, SparsityProfile,
};

// ═══════════════════════════════════════════════════════════════════════════
// Helper functions -- f64-accumulated linear algebra primitives
// ═══════════════════════════════════════════════════════════════════════════

/// Compute the dot product of two `f32` slices using `f64` accumulation.
///
/// Uses a 4-wide accumulator to exploit instruction-level parallelism and
/// reduce the dependency chain length, preventing precision loss in the
/// inner products that drive the CG recurrence.
///
/// # Panics
///
/// Debug-asserts that `a.len() == b.len()`.
#[inline]
pub fn dot_product_f64(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "dot_product_f64: length mismatch");

    let n = a.len();
    let chunks = n / 4;
    let remainder = n % 4;

    let mut acc0: f64 = 0.0;
    let mut acc1: f64 = 0.0;
    let mut acc2: f64 = 0.0;
    let mut acc3: f64 = 0.0;

    for i in 0..chunks {
        let j = i * 4;
        acc0 += a[j] as f64 * b[j] as f64;
        acc1 += a[j + 1] as f64 * b[j + 1] as f64;
        acc2 += a[j + 2] as f64 * b[j + 2] as f64;
        acc3 += a[j + 3] as f64 * b[j + 3] as f64;
    }

    let base = chunks * 4;
    for i in 0..remainder {
        acc0 += a[base + i] as f64 * b[base + i] as f64;
    }

    (acc0 + acc1) + (acc2 + acc3)
}

/// Compute the dot product of two `f64` slices with 4-wide accumulation.
///
/// Used internally when the working vectors are already `f64`.
#[inline]
fn dot_f64(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "dot_f64: length mismatch");

    let n = a.len();
    let chunks = n / 4;
    let remainder = n % 4;

    let mut acc0: f64 = 0.0;
    let mut acc1: f64 = 0.0;
    let mut acc2: f64 = 0.0;
    let mut acc3: f64 = 0.0;

    for i in 0..chunks {
        let j = i * 4;
        acc0 += a[j] * b[j];
        acc1 += a[j + 1] * b[j + 1];
        acc2 += a[j + 2] * b[j + 2];
        acc3 += a[j + 3] * b[j + 3];
    }

    let base = chunks * 4;
    for i in 0..remainder {
        acc0 += a[base + i] * b[base + i];
    }

    (acc0 + acc1) + (acc2 + acc3)
}

/// Compute `y[i] += alpha * x[i]` for all `i` (AXPY operation, `f32`).
///
/// # Panics
///
/// Debug-asserts that `x.len() == y.len()`.
#[inline]
pub fn axpy(alpha: f32, x: &[f32], y: &mut [f32]) {
    assert_eq!(x.len(), y.len(), "axpy: length mismatch");

    let n = x.len();
    let chunks = n / 4;
    let base = chunks * 4;

    for i in 0..chunks {
        let j = i * 4;
        y[j] += alpha * x[j];
        y[j + 1] += alpha * x[j + 1];
        y[j + 2] += alpha * x[j + 2];
        y[j + 3] += alpha * x[j + 3];
    }
    for i in base..n {
        y[i] += alpha * x[i];
    }
}

/// Compute `y[i] += alpha * x[i]` for all `i` (AXPY operation, `f64`).
#[inline]
fn axpy_f64(alpha: f64, x: &[f64], y: &mut [f64]) {
    assert_eq!(x.len(), y.len(), "axpy_f64: length mismatch");

    let n = x.len();
    let chunks = n / 4;
    let base = chunks * 4;

    for i in 0..chunks {
        let j = i * 4;
        y[j] += alpha * x[j];
        y[j + 1] += alpha * x[j + 1];
        y[j + 2] += alpha * x[j + 2];
        y[j + 3] += alpha * x[j + 3];
    }
    for i in base..n {
        y[i] += alpha * x[i];
    }
}

/// Compute the L2 norm of an `f32` slice using `f64` accumulation.
///
/// Returns `sqrt(sum(x_i^2))` computed entirely in `f64`.
#[inline]
pub fn norm2(x: &[f32]) -> f64 {
    dot_product_f64(x, x).sqrt()
}

/// Compute the L2 norm of an `f64` slice.
#[inline]
fn norm2_f64(x: &[f64]) -> f64 {
    dot_f64(x, x).sqrt()
}

// ═══════════════════════════════════════════════════════════════════════════
// ConjugateGradientSolver
// ═══════════════════════════════════════════════════════════════════════════

/// Conjugate Gradient solver for symmetric positive-definite sparse systems.
///
/// Stores the solver configuration (tolerance, iteration cap, preconditioning).
/// The solve itself is stateless and may be invoked concurrently on different
/// inputs from multiple threads.
#[derive(Debug, Clone)]
pub struct ConjugateGradientSolver {
    /// Relative residual convergence tolerance.
    ///
    /// The solver declares convergence when `||r||_2 < tolerance * ||b||_2`.
    tolerance: f64,

    /// Maximum number of CG iterations before declaring non-convergence.
    max_iterations: usize,

    /// Whether to apply diagonal (Jacobi) preconditioning.
    ///
    /// When `true`, the preconditioner `M = diag(A)` is used, computing
    /// `z_i = r_i / A_{ii}` each iteration. Beneficial for diagonally-dominant
    /// systems at the cost of O(n) extra work per iteration.
    use_preconditioner: bool,
}

impl ConjugateGradientSolver {
    /// Create a new CG solver.
    ///
    /// # Arguments
    ///
    /// * `tolerance` -- Relative residual threshold for convergence. Must be
    ///   positive and finite.
    /// * `max_iterations` -- Upper bound on CG iterations. Must be >= 1.
    /// * `use_preconditioner` -- Enable diagonal (Jacobi) preconditioning.
    pub fn new(tolerance: f64, max_iterations: usize, use_preconditioner: bool) -> Self {
        Self {
            tolerance,
            max_iterations,
            use_preconditioner,
        }
    }

    /// Return the configured tolerance.
    #[inline]
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Return the configured maximum iterations.
    #[inline]
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Return whether preconditioning is enabled.
    #[inline]
    pub fn use_preconditioner(&self) -> bool {
        self.use_preconditioner
    }

    // -------------------------------------------------------------------
    // Input validation
    // -------------------------------------------------------------------

    /// Validate inputs before entering the CG loop.
    fn validate(&self, matrix: &CsrMatrix<f64>, rhs: &[f64]) -> Result<(), SolverError> {
        if matrix.rows != matrix.cols {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "CG requires a square matrix but got {}x{}",
                    matrix.rows, matrix.cols,
                )),
            ));
        }

        if rhs.len() != matrix.rows {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "rhs length {} does not match matrix rows {}",
                    rhs.len(),
                    matrix.rows,
                )),
            ));
        }

        if matrix.row_ptr.len() != matrix.rows + 1 {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "row_ptr length {} does not equal rows + 1 = {}",
                    matrix.row_ptr.len(),
                    matrix.rows + 1,
                )),
            ));
        }

        if !self.tolerance.is_finite() || self.tolerance <= 0.0 {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: "tolerance".into(),
                    value: self.tolerance.to_string(),
                    expected: "positive finite value".into(),
                },
            ));
        }

        if self.max_iterations == 0 {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: "max_iterations".into(),
                    value: "0".into(),
                    expected: ">= 1".into(),
                },
            ));
        }

        Ok(())
    }

    // -------------------------------------------------------------------
    // Jacobi preconditioner
    // -------------------------------------------------------------------

    /// Build the Jacobi (diagonal) preconditioner from `A`.
    ///
    /// Returns `inv_diag[i] = 1.0 / A_{ii}`. Zero or near-zero diagonal
    /// entries are replaced with `1.0` to prevent division by zero.
    fn build_jacobi_preconditioner(matrix: &CsrMatrix<f64>) -> Vec<f64> {
        let n = matrix.rows;
        let mut inv_diag = vec![1.0f64; n];

        for row in 0..n {
            let start = matrix.row_ptr[row];
            let end = matrix.row_ptr[row + 1];
            for idx in start..end {
                if matrix.col_indices[idx] == row {
                    let diag_val = matrix.values[idx];
                    if diag_val.abs() > f64::EPSILON {
                        inv_diag[row] = 1.0 / diag_val;
                    }
                    break;
                }
            }
        }

        inv_diag
    }

    /// Apply the diagonal preconditioner: `z[i] = inv_diag[i] * r[i]`.
    #[inline]
    fn apply_preconditioner(inv_diag: &[f64], r: &[f64], z: &mut [f64]) {
        assert_eq!(inv_diag.len(), r.len());
        assert_eq!(r.len(), z.len());

        let n = r.len();
        let chunks = n / 4;
        let base = chunks * 4;

        for i in 0..chunks {
            let j = i * 4;
            z[j] = inv_diag[j] * r[j];
            z[j + 1] = inv_diag[j + 1] * r[j + 1];
            z[j + 2] = inv_diag[j + 2] * r[j + 2];
            z[j + 3] = inv_diag[j + 3] * r[j + 3];
        }
        for i in base..n {
            z[i] = inv_diag[i] * r[i];
        }
    }

    // -------------------------------------------------------------------
    // Core CG algorithm
    // -------------------------------------------------------------------

    /// Core CG algorithm implementation.
    ///
    /// Works entirely in `f64` precision internally. The final solution is
    /// down-cast to `f32` in the returned [`SolverResult`] to match the
    /// crate's output contract.
    fn solve_inner(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let start_time = Instant::now();
        let n = matrix.rows;

        // Effective limits: take the tighter of our config and the budget.
        let effective_max_iter = self.max_iterations.min(budget.max_iterations);
        let effective_tol = self.tolerance.min(budget.tolerance);

        // --- Trivial case: zero-dimensional system ---
        if n == 0 {
            return Ok(SolverResult {
                solution: vec![],
                iterations: 0,
                residual_norm: 0.0,
                wall_time: start_time.elapsed(),
                convergence_history: vec![],
                algorithm: Algorithm::CG,
            });
        }

        // --- Allocate working vectors (all f64) ---
        let mut x = vec![0.0f64; n]; // solution (initial guess = zero)
        let mut r = vec![0.0f64; n]; // residual
        let mut z = vec![0.0f64; n]; // preconditioned residual
        let mut p = vec![0.0f64; n]; // search direction
        let mut ap = vec![0.0f64; n]; // A * p scratch buffer

        // --- Build preconditioner (if enabled) ---
        let inv_diag = if self.use_preconditioner {
            Some(Self::build_jacobi_preconditioner(matrix))
        } else {
            None
        };

        // --- r = b - A*x. Since x = 0, r = b ---
        r.copy_from_slice(rhs);

        // --- Convergence threshold: ||r||_2 < tol * ||b||_2 (relative) ---
        let b_norm = norm2_f64(rhs);
        let abs_tolerance = effective_tol * b_norm;

        // Handle zero RHS: the solution is the zero vector.
        if b_norm < f64::EPSILON {
            debug!("CG: zero RHS detected, returning zero solution");
            return Ok(SolverResult {
                solution: vec![0.0f32; n],
                iterations: 0,
                residual_norm: 0.0,
                wall_time: start_time.elapsed(),
                convergence_history: vec![],
                algorithm: Algorithm::CG,
            });
        }

        let initial_residual_norm = norm2_f64(&r);

        // --- z = M^{-1} * r ---
        match &inv_diag {
            Some(diag) => Self::apply_preconditioner(diag, &r, &mut z),
            None => z.copy_from_slice(&r),
        }

        // --- p = z ---
        p.copy_from_slice(&z);

        // --- rz = r . z ---
        let mut rz = dot_f64(&r, &z);

        let mut convergence_history = Vec::with_capacity(effective_max_iter.min(256));
        let mut converged = false;

        debug!(
            "CG: n={}, nnz={}, tol={:.2e}, max_iter={}, precond={}",
            n,
            matrix.nnz(),
            effective_tol,
            effective_max_iter,
            self.use_preconditioner,
        );

        // ===============================================================
        // Main CG loop (Hestenes-Stiefel)
        // ===============================================================
        for k in 0..effective_max_iter {
            // --- Budget: wall-time check ---
            if start_time.elapsed() > budget.max_time {
                warn!("CG: wall-time budget exhausted at iteration {k}");
                return Err(SolverError::BudgetExhausted {
                    reason: format!(
                        "wall-time limit {:?} exceeded at iteration {k}",
                        budget.max_time,
                    ),
                    elapsed: start_time.elapsed(),
                });
            }

            // --- Ap = A * p  (sparse matrix-vector product) ---
            matrix.spmv(&p, &mut ap);

            // --- alpha = rz / (p . Ap) ---
            let p_dot_ap = dot_f64(&p, &ap);

            // Guard: if p.Ap <= 0 the matrix is not SPD or we hit numerical
            // breakdown.
            if p_dot_ap <= 0.0 {
                warn!("CG: non-positive p.Ap = {p_dot_ap:.4e} at iteration {k}");
                return Err(SolverError::NumericalInstability {
                    iteration: k,
                    detail: format!("p.Ap = {p_dot_ap:.6e} <= 0; matrix may not be SPD",),
                });
            }

            let alpha = rz / p_dot_ap;

            // --- x = x + alpha * p ---
            axpy_f64(alpha, &p, &mut x);

            // --- r = r - alpha * Ap ---
            axpy_f64(-alpha, &ap, &mut r);

            // --- Convergence check: ||r||_2 < tol * ||b||_2 ---
            let r_norm = norm2_f64(&r);

            convergence_history.push(ConvergenceInfo {
                iteration: k,
                residual_norm: r_norm,
            });

            trace!(
                "CG iter {k}: ||r|| = {r_norm:.6e}, rel = {:.6e}",
                r_norm / b_norm,
            );

            if r_norm < abs_tolerance {
                converged = true;
                debug!(
                    "CG converged at iteration {k}: ||r|| = {r_norm:.6e}, \
                     rel = {:.6e}",
                    r_norm / b_norm,
                );
                break;
            }

            // --- Divergence detection ---
            // If ||r|| has grown by 10x from the initial residual, the
            // system is likely indefinite or ill-conditioned beyond rescue.
            if r_norm > 10.0 * initial_residual_norm {
                warn!(
                    "CG: divergence at iteration {k}: ||r|| = {r_norm:.6e} \
                     > 10 * ||r_0|| = {:.6e}",
                    10.0 * initial_residual_norm,
                );
                return Err(SolverError::NumericalInstability {
                    iteration: k,
                    detail: format!(
                        "residual diverged: ||r|| = {r_norm:.6e} exceeds \
                         10x initial residual {initial_residual_norm:.6e}",
                    ),
                });
            }

            // --- z = M^{-1} * r ---
            match &inv_diag {
                Some(diag) => Self::apply_preconditioner(diag, &r, &mut z),
                None => z.copy_from_slice(&r),
            }

            // --- rz_new = r . z ---
            let rz_new = dot_f64(&r, &z);

            // Guard: stagnation when rz is near-zero.
            if rz.abs() < f64::EPSILON * f64::EPSILON {
                warn!("CG: rz near zero at iteration {k}, stagnation");
                return Err(SolverError::NumericalInstability {
                    iteration: k,
                    detail: format!("rz = {rz:.6e} is near zero; solver stagnated",),
                });
            }

            // --- beta = rz_new / rz ---
            let beta = rz_new / rz;

            // --- p = z + beta * p ---
            for i in 0..n {
                p[i] = z[i] + beta * p[i];
            }

            // --- rz = rz_new ---
            rz = rz_new;
        }

        let wall_time = start_time.elapsed();
        let final_residual = norm2_f64(&r);

        if !converged {
            debug!(
                "CG: non-convergence after {} iterations, ||r|| = {final_residual:.6e}",
                effective_max_iter,
            );
            return Err(SolverError::NonConvergence {
                iterations: effective_max_iter,
                residual: final_residual,
                tolerance: abs_tolerance,
            });
        }

        // Down-cast the f64 solution to f32 for SolverResult.
        let solution_f32: Vec<f32> = x.iter().map(|&v| v as f32).collect();

        Ok(SolverResult {
            solution: solution_f32,
            iterations: convergence_history.len(),
            residual_norm: final_residual,
            wall_time,
            convergence_history,
            algorithm: Algorithm::CG,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SolverEngine trait implementation
// ═══════════════════════════════════════════════════════════════════════════

impl SolverEngine for ConjugateGradientSolver {
    /// Solve `Ax = b` using the Conjugate Gradient method.
    ///
    /// # Errors
    ///
    /// * [`SolverError::InvalidInput`] -- dimension mismatch or invalid params.
    /// * [`SolverError::NumericalInstability`] -- divergence or non-SPD matrix.
    /// * [`SolverError::NonConvergence`] -- iteration limit exceeded.
    /// * [`SolverError::BudgetExhausted`] -- wall-time limit exceeded.
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        self.validate(matrix, rhs)?;
        self.solve_inner(matrix, rhs, budget)
    }

    /// Estimate CG complexity from the sparsity profile.
    ///
    /// CG converges in `O(sqrt(kappa))` iterations, each costing `O(nnz)` for
    /// the SpMV plus `O(n)` for the vector updates.
    fn estimate_complexity(&self, profile: &SparsityProfile, n: usize) -> ComplexityEstimate {
        // Estimated iterations from condition number, clamped to max_iterations.
        let est_iters = (profile.estimated_condition.sqrt() as usize)
            .max(1)
            .min(self.max_iterations);

        // FLOPs per iteration: 2*nnz (SpMV) + 6*n (dot products, axpy ops).
        let flops_per_iter = 2 * profile.nnz as u64 + 6 * n as u64;
        let estimated_flops = est_iters as u64 * flops_per_iter;

        // Memory: 5 vectors of length n (x, r, z, p, Ap) plus preconditioner.
        let vec_bytes = n * std::mem::size_of::<f64>();
        let precond_bytes = if self.use_preconditioner {
            vec_bytes
        } else {
            0
        };
        let estimated_memory_bytes = 5 * vec_bytes + precond_bytes;

        ComplexityEstimate {
            algorithm: Algorithm::CG,
            estimated_flops,
            estimated_iterations: est_iters,
            estimated_memory_bytes,
            complexity_class: ComplexityClass::SqrtCondition,
        }
    }

    /// Return the algorithm identifier.
    fn algorithm(&self) -> Algorithm {
        Algorithm::CG
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Build a symmetric tridiagonal SPD matrix (f64):
    ///   diag = 4.0, off-diag = -1.0
    fn tridiagonal_spd(n: usize) -> CsrMatrix<f64> {
        let mut entries = Vec::with_capacity(3 * n);
        for i in 0..n {
            if i > 0 {
                entries.push((i, i - 1, -1.0f64));
            }
            entries.push((i, i, 4.0f64));
            if i + 1 < n {
                entries.push((i, i + 1, -1.0f64));
            }
        }
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    /// Build a diagonal matrix from the given values.
    fn diagonal_matrix(diag: &[f64]) -> CsrMatrix<f64> {
        let n = diag.len();
        let entries: Vec<_> = diag.iter().enumerate().map(|(i, &v)| (i, i, v)).collect();
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    /// Build an identity matrix of dimension n.
    fn identity(n: usize) -> CsrMatrix<f64> {
        CsrMatrix::<f64>::identity(n)
    }

    fn default_budget() -> ComputeBudget {
        ComputeBudget {
            max_time: Duration::from_secs(30),
            max_iterations: 10_000,
            tolerance: 1e-10,
        }
    }

    // -----------------------------------------------------------------
    // dot_product_f64
    // -----------------------------------------------------------------

    #[test]
    fn dot_product_f64_basic() {
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![4.0f32, 5.0, 6.0];
        let result = dot_product_f64(&a, &b);
        assert!((result - 32.0).abs() < 1e-10);
    }

    #[test]
    fn dot_product_f64_empty() {
        assert!((dot_product_f64(&[], &[]) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn dot_product_f64_precision() {
        let n = 10_000;
        let a = vec![1.0f32; n];
        let b = vec![1.0f32; n];
        assert!((dot_product_f64(&a, &b) - n as f64).abs() < 1e-10);
    }

    #[test]
    fn dot_product_f64_odd_length() {
        let a = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let b = vec![5.0f32, 4.0, 3.0, 2.0, 1.0];
        // 5 + 8 + 9 + 8 + 5 = 35
        assert!((dot_product_f64(&a, &b) - 35.0).abs() < 1e-10);
    }

    // -----------------------------------------------------------------
    // axpy
    // -----------------------------------------------------------------

    #[test]
    fn axpy_basic() {
        let x = vec![1.0f32, 2.0, 3.0];
        let mut y = vec![10.0f32, 20.0, 30.0];
        axpy(2.0, &x, &mut y);
        assert_eq!(y, vec![12.0, 24.0, 36.0]);
    }

    #[test]
    fn axpy_negative_alpha() {
        let x = vec![1.0f32, 1.0, 1.0];
        let mut y = vec![5.0f32, 5.0, 5.0];
        axpy(-3.0, &x, &mut y);
        assert_eq!(y, vec![2.0, 2.0, 2.0]);
    }

    // -----------------------------------------------------------------
    // norm2
    // -----------------------------------------------------------------

    #[test]
    fn norm2_basic() {
        let x = vec![3.0f32, 4.0];
        assert!((norm2(&x) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn norm2_zero() {
        assert!((norm2(&vec![0.0f32; 5]) - 0.0).abs() < 1e-10);
    }

    // -----------------------------------------------------------------
    // CG solver: convergence on well-conditioned systems
    // -----------------------------------------------------------------

    #[test]
    fn cg_identity_matrix() {
        let n = 5;
        let matrix = identity(n);
        let rhs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-10, 100, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        for i in 0..n {
            assert!(
                (result.solution[i] as f64 - rhs[i]).abs() < 1e-5,
                "x[{i}] = {} != {}",
                result.solution[i],
                rhs[i],
            );
        }
        // Identity should converge in at most 1 iteration.
        assert!(result.iterations <= 1);
    }

    #[test]
    fn cg_diagonal_matrix() {
        let diag = vec![2.0, 3.0, 5.0, 7.0];
        let matrix = diagonal_matrix(&diag);
        let rhs = vec![4.0, 9.0, 25.0, 49.0];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-10, 100, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        let expected = [2.0, 3.0, 5.0, 7.0];
        for i in 0..4 {
            assert!(
                (result.solution[i] as f64 - expected[i]).abs() < 1e-4,
                "x[{i}] = {} != {}",
                result.solution[i],
                expected[i],
            );
        }
    }

    #[test]
    fn cg_tridiagonal_small() {
        let n = 10;
        let matrix = tridiagonal_spd(n);
        let rhs = vec![1.0f64; n];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 200, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        assert!(
            result.residual_norm < 1e-6,
            "residual = {}",
            result.residual_norm,
        );
        assert!(
            result.iterations <= n,
            "took {} iterations for n={}",
            result.iterations,
            n,
        );
    }

    #[test]
    fn cg_tridiagonal_large() {
        let n = 500;
        let matrix = tridiagonal_spd(n);
        let rhs: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0) / n as f64).collect();
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 2000, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        assert!(
            result.residual_norm < 1e-5,
            "residual = {}",
            result.residual_norm,
        );
    }

    // -----------------------------------------------------------------
    // Preconditioning
    // -----------------------------------------------------------------

    #[test]
    fn cg_preconditioned_converges_faster() {
        let n = 100;
        let matrix = tridiagonal_spd(n);
        let rhs = vec![1.0f64; n];
        let budget = default_budget();

        let no_precond = ConjugateGradientSolver::new(1e-8, 500, false);
        let with_precond = ConjugateGradientSolver::new(1e-8, 500, true);

        let result_no = no_precond.solve(&matrix, &rhs, &budget).unwrap();
        let result_yes = with_precond.solve(&matrix, &rhs, &budget).unwrap();

        assert!(result_no.residual_norm < 1e-6);
        assert!(result_yes.residual_norm < 1e-6);

        assert!(
            result_yes.iterations <= result_no.iterations,
            "preconditioned ({}) should use <= iterations than \
             unpreconditioned ({})",
            result_yes.iterations,
            result_no.iterations,
        );
    }

    // -----------------------------------------------------------------
    // Zero RHS / empty system
    // -----------------------------------------------------------------

    #[test]
    fn cg_zero_rhs() {
        let matrix = tridiagonal_spd(5);
        let rhs = vec![0.0f64; 5];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 100, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        assert_eq!(result.iterations, 0);
        for &v in &result.solution {
            assert!((v as f64).abs() < 1e-10);
        }
    }

    #[test]
    fn cg_empty_system() {
        let matrix = CsrMatrix {
            row_ptr: vec![0],
            col_indices: vec![],
            values: Vec::<f64>::new(),
            rows: 0,
            cols: 0,
        };
        let rhs: Vec<f64> = vec![];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 100, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        assert_eq!(result.iterations, 0);
        assert!(result.solution.is_empty());
    }

    // -----------------------------------------------------------------
    // Error cases
    // -----------------------------------------------------------------

    #[test]
    fn cg_dimension_mismatch() {
        let matrix = tridiagonal_spd(3);
        let rhs = vec![1.0f64; 5];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 100, false);
        let err = solver.solve(&matrix, &rhs, &budget).unwrap_err();
        assert!(matches!(err, SolverError::InvalidInput(_)));
    }

    #[test]
    fn cg_non_square_matrix() {
        let matrix = CsrMatrix {
            row_ptr: vec![0, 1, 2],
            col_indices: vec![0, 1],
            values: vec![1.0f64, 1.0],
            rows: 2,
            cols: 3,
        };
        let rhs = vec![1.0f64; 2];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 100, false);
        let err = solver.solve(&matrix, &rhs, &budget).unwrap_err();
        assert!(matches!(err, SolverError::InvalidInput(_)));
    }

    #[test]
    fn cg_non_convergence() {
        let n = 50;
        let matrix = tridiagonal_spd(n);
        let rhs = vec![1.0f64; n];
        let budget = ComputeBudget {
            max_time: Duration::from_secs(30),
            max_iterations: 1,
            tolerance: 1e-15,
        };

        let solver = ConjugateGradientSolver::new(1e-15, 1, false);
        let err = solver.solve(&matrix, &rhs, &budget).unwrap_err();
        assert!(matches!(err, SolverError::NonConvergence { .. }));
    }

    #[test]
    fn cg_budget_iteration_limit() {
        let n = 50;
        let matrix = tridiagonal_spd(n);
        let rhs = vec![1.0f64; n];

        // Solver allows 1000, but budget allows only 2.
        let solver = ConjugateGradientSolver::new(1e-15, 1000, false);
        let budget = ComputeBudget {
            max_time: Duration::from_secs(60),
            max_iterations: 2,
            tolerance: 1e-15,
        };

        let err = solver.solve(&matrix, &rhs, &budget).unwrap_err();
        assert!(matches!(err, SolverError::NonConvergence { .. }));
    }

    // -----------------------------------------------------------------
    // Convergence history
    // -----------------------------------------------------------------

    #[test]
    fn cg_convergence_history_populated() {
        let n = 20;
        let matrix = tridiagonal_spd(n);
        let rhs = vec![1.0f64; n];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-10, 200, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        assert!(!result.convergence_history.is_empty());

        // Final history entry should match the reported residual.
        let last = result.convergence_history.last().unwrap();
        assert!((last.residual_norm - result.residual_norm).abs() < 1e-12);
    }

    #[test]
    fn cg_algorithm_field() {
        let matrix = identity(3);
        let rhs = vec![1.0f64; 3];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-8, 100, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();
        assert_eq!(result.algorithm, Algorithm::CG);
    }

    // -----------------------------------------------------------------
    // Verify Ax = b for computed solution
    // -----------------------------------------------------------------

    #[test]
    fn cg_solution_satisfies_system() {
        let n = 20;
        let matrix = tridiagonal_spd(n);
        let rhs = vec![1.0f64; n];
        let budget = default_budget();

        let solver = ConjugateGradientSolver::new(1e-10, 200, true);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        // Up-cast solution back to f64 and compute Ax.
        let x_f64: Vec<f64> = result.solution.iter().map(|&v| v as f64).collect();
        let mut ax = vec![0.0f64; n];
        matrix.spmv(&x_f64, &mut ax);

        let mut max_err: f64 = 0.0;
        for i in 0..n {
            let err = (ax[i] - rhs[i]).abs();
            if err > max_err {
                max_err = err;
            }
        }

        assert!(
            max_err < 1e-4,
            "max |Ax - b| = {max_err:.6e}, expected < 1e-4",
        );
    }

    // -----------------------------------------------------------------
    // Estimate complexity
    // -----------------------------------------------------------------

    #[test]
    fn estimate_complexity_returns_cg() {
        let solver = ConjugateGradientSolver::new(1e-8, 500, true);
        let profile = SparsityProfile {
            rows: 100,
            cols: 100,
            nnz: 298,
            density: 0.0298,
            is_diag_dominant: true,
            estimated_spectral_radius: 0.5,
            estimated_condition: 100.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 2.98,
            max_nnz_per_row: 3,
        };

        let est = solver.estimate_complexity(&profile, 100);
        assert_eq!(est.algorithm, Algorithm::CG);
        assert_eq!(est.complexity_class, ComplexityClass::SqrtCondition);
        assert!(est.estimated_iterations > 0);
        assert!(est.estimated_flops > 0);
        assert!(est.estimated_memory_bytes > 0);
    }

    // -----------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------

    #[test]
    fn accessors() {
        let solver = ConjugateGradientSolver::new(1e-6, 500, true);
        assert!((solver.tolerance() - 1e-6).abs() < 1e-15);
        assert_eq!(solver.max_iterations(), 500);
        assert!(solver.use_preconditioner());
    }
}
