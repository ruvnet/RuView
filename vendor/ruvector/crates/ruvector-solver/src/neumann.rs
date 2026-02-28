//! Jacobi-preconditioned Neumann Series iterative solver.
//!
//! Solves the linear system `Ax = b` by splitting `A = D - R` (where `D` is
//! the diagonal part) and iterating:
//!
//! ```text
//! x_{k+1} = x_k + D^{-1} (b - A x_k)
//! ```
//!
//! This is equivalent to the Neumann series `x = sum_{k=0}^{K} M^k D^{-1} b`
//! where `M = I - D^{-1} A`. Convergence requires `rho(M) < 1`, which is
//! guaranteed for strictly diagonally dominant matrices.
//!
//! # Algorithm
//!
//! The iteration maintains a running solution `x` and residual `r = b - Ax`:
//!
//! ```text
//! x_0 = D^{-1} b
//! for k = 0, 1, 2, ...:
//!     r = b - A * x_k
//!     x_{k+1} = x_k + D^{-1} * r
//!     if ||r|| < tolerance:
//!         break
//! ```
//!
//! # Convergence
//!
//! Before solving, the solver estimates `rho(I - D^{-1}A)` via a 10-step
//! power iteration and rejects the problem with
//! [`SolverError::SpectralRadiusExceeded`] if `rho >= 1.0`. During iteration,
//! if the residual grows by more than 2x between consecutive steps,
//! [`SolverError::NumericalInstability`] is returned.

use std::time::Instant;

use tracing::{debug, info, instrument, warn};

use crate::error::{SolverError, ValidationError};
use crate::traits::SolverEngine;
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, ConvergenceInfo, CsrMatrix,
    SolverResult, SparsityProfile,
};

/// Number of power-iteration steps used to estimate the spectral radius.
const POWER_ITERATION_STEPS: usize = 10;

/// If the residual grows by more than this factor in a single step, the solver
/// declares numerical instability.
const INSTABILITY_GROWTH_FACTOR: f64 = 2.0;

// ---------------------------------------------------------------------------
// NeumannSolver
// ---------------------------------------------------------------------------

/// Neumann Series solver for sparse linear systems.
///
/// Computes `x = sum_{k=0}^{K} (I - A)^k * b` by maintaining a residual
/// vector and accumulating partial sums until convergence.
///
/// # Example
///
/// ```rust
/// use ruvector_solver::types::CsrMatrix;
/// use ruvector_solver::neumann::NeumannSolver;
///
/// // Diagonally dominant 2x2: A = [[2, -0.5], [-0.5, 2]]
/// let a = CsrMatrix::<f32>::from_coo(2, 2, vec![
///     (0, 0, 2.0_f32), (0, 1, -0.5_f32),
///     (1, 0, -0.5_f32), (1, 1, 2.0_f32),
/// ]);
/// let b = vec![1.0_f32, 1.0];
///
/// let solver = NeumannSolver::new(1e-6, 500);
/// let result = solver.solve(&a, &b).unwrap();
/// assert!(result.residual_norm < 1e-4);
/// ```
#[derive(Debug, Clone)]
pub struct NeumannSolver {
    /// Target residual L2 norm for convergence.
    pub tolerance: f64,
    /// Maximum number of iterations before giving up.
    pub max_iterations: usize,
}

impl NeumannSolver {
    /// Create a new `NeumannSolver`.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Stop when `||r|| < tolerance`.
    /// * `max_iterations` - Upper bound on iterations.
    pub fn new(tolerance: f64, max_iterations: usize) -> Self {
        Self {
            tolerance,
            max_iterations,
        }
    }

    /// Estimate the spectral radius of `M = I - D^{-1}A` via 10-step power
    /// iteration.
    ///
    /// Runs [`POWER_ITERATION_STEPS`] iterations of the power method on the
    /// Jacobi iteration matrix `M = I - D^{-1}A`. Returns the Rayleigh-quotient
    /// estimate of the dominant eigenvalue magnitude.
    ///
    /// # Arguments
    ///
    /// * `matrix` - The coefficient matrix `A` (must be square).
    ///
    /// # Returns
    ///
    /// Estimated `|lambda_max(I - D^{-1}A)|`. If this is `>= 1.0`, the
    /// Jacobi-preconditioned Neumann series will diverge.
    #[instrument(skip(matrix), fields(n = matrix.rows))]
    pub fn estimate_spectral_radius(matrix: &CsrMatrix<f32>) -> f64 {
        let n = matrix.rows;
        if n == 0 {
            return 0.0;
        }

        let d_inv = extract_diag_inv_f32(matrix);
        Self::estimate_spectral_radius_with_diag(matrix, &d_inv)
    }

    /// Inner helper: estimate spectral radius using a pre-computed `d_inv`.
    ///
    /// This avoids recomputing the diagonal inverse when the caller already
    /// has it (e.g. `solve()` needs `d_inv` for both the spectral check and
    /// the Jacobi iteration).
    fn estimate_spectral_radius_with_diag(matrix: &CsrMatrix<f32>, d_inv: &[f32]) -> f64 {
        let n = matrix.rows;
        if n == 0 {
            return 0.0;
        }

        // Initialise with a deterministic pseudo-random unit vector.
        let mut v: Vec<f32> = (0..n)
            .map(|i| ((i.wrapping_mul(7).wrapping_add(13)) % 100) as f32 / 100.0)
            .collect();
        let norm = l2_norm_f32(&v);
        if norm > 1e-12 {
            scale_vec_f32(&mut v, 1.0 / norm);
        }

        let mut av = vec![0.0f32; n]; // scratch for A*v
        let mut w = vec![0.0f32; n]; // scratch for M*v = v - D^{-1}*A*v
        let mut eigenvalue_estimate = 0.0f64;

        for _ in 0..POWER_ITERATION_STEPS {
            // w = v - D^{-1} * A * v  (i.e. M * v)
            matrix.spmv(&v, &mut av);
            for j in 0..n {
                w[j] = v[j] - d_inv[j] * av[j];
            }

            // Rayleigh quotient: lambda = v^T w  (v is unit-length).
            let dot: f64 = v
                .iter()
                .zip(w.iter())
                .map(|(&a, &b)| a as f64 * b as f64)
                .sum();
            eigenvalue_estimate = dot;

            // Normalise w -> v for the next step.
            let w_norm = l2_norm_f32(&w);
            if w_norm < 1e-12 {
                break;
            }
            for j in 0..n {
                v[j] = w[j] / w_norm as f32;
            }
        }

        let rho = eigenvalue_estimate.abs();
        debug!(rho, "estimated spectral radius of (I - D^-1 A)");
        rho
    }

    /// Core Jacobi-preconditioned Neumann-series solve operating on `f32`.
    ///
    /// Validates inputs, checks the spectral radius of `I - D^{-1}A` via
    /// power iteration, then runs the iteration returning a [`SolverResult`].
    ///
    /// # Errors
    ///
    /// - [`SolverError::InvalidInput`] if the matrix is non-square or the RHS
    ///   length does not match.
    /// - [`SolverError::SpectralRadiusExceeded`] if `rho(I - D^{-1}A) >= 1`.
    /// - [`SolverError::NumericalInstability`] if the residual grows by more
    ///   than 2x in a single step.
    /// - [`SolverError::NonConvergence`] if the iteration budget is exhausted.
    #[instrument(skip(self, matrix, rhs), fields(n = matrix.rows, nnz = matrix.nnz()))]
    pub fn solve(&self, matrix: &CsrMatrix<f32>, rhs: &[f32]) -> Result<SolverResult, SolverError> {
        let start = Instant::now();
        let n = matrix.rows;

        // ------------------------------------------------------------------
        // Input validation
        // ------------------------------------------------------------------
        if matrix.rows != matrix.cols {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "matrix must be square: got {}x{}",
                    matrix.rows, matrix.cols,
                )),
            ));
        }

        if rhs.len() != n {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "rhs length {} does not match matrix dimension {}",
                    rhs.len(),
                    n,
                )),
            ));
        }

        // Edge case: empty system.
        if n == 0 {
            return Ok(SolverResult {
                solution: Vec::new(),
                iterations: 0,
                residual_norm: 0.0,
                wall_time: start.elapsed(),
                convergence_history: Vec::new(),
                algorithm: Algorithm::Neumann,
            });
        }

        // Extract D^{-1} once — reused for both the spectral radius check
        // and the Jacobi-preconditioned iteration that follows.
        let d_inv = extract_diag_inv_f32(matrix);

        // ------------------------------------------------------------------
        // Spectral radius pre-check (10-step power iteration on I - D^{-1}A)
        // ------------------------------------------------------------------
        let rho = Self::estimate_spectral_radius_with_diag(matrix, &d_inv);
        if rho >= 1.0 {
            warn!(rho, "spectral radius >= 1.0, Neumann series will diverge");
            return Err(SolverError::SpectralRadiusExceeded {
                spectral_radius: rho,
                limit: 1.0,
                algorithm: Algorithm::Neumann,
            });
        }
        info!(rho, "spectral radius check passed");

        // ------------------------------------------------------------------
        // Jacobi-preconditioned iteration (fused kernel)
        //
        //   x_0 = D^{-1} * b
        //   loop:
        //       r = b - A * x_k            (fused with norm computation)
        //       if ||r|| < tolerance: break
        //       x_{k+1} = x_k + D^{-1} * r (fused with residual storage)
        //
        // Key optimization: uses fused_residual_norm_sq to compute
        // r = b - Ax and ||r||^2 in a single pass, avoiding a separate
        // spmv + subtraction + norm computation (3 memory traversals -> 1).
        // ------------------------------------------------------------------
        let mut x: Vec<f32> = (0..n).map(|i| d_inv[i] * rhs[i]).collect();
        let mut r = vec![0.0f32; n]; // residual buffer (reused each iteration)

        let mut convergence_history = Vec::with_capacity(self.max_iterations.min(256));
        let mut prev_residual_norm = f64::MAX;
        let final_residual_norm: f64;
        let mut iterations_done: usize = 0;

        for k in 0..self.max_iterations {
            // Fused: compute r = b - Ax and ||r||^2 in one pass.
            let residual_norm_sq = matrix.fused_residual_norm_sq(&x, rhs, &mut r);
            let residual_norm = residual_norm_sq.sqrt();
            iterations_done = k + 1;

            convergence_history.push(ConvergenceInfo {
                iteration: k,
                residual_norm,
            });

            debug!(iteration = k, residual_norm, "neumann iteration");

            // Convergence check.
            if residual_norm < self.tolerance {
                final_residual_norm = residual_norm;
                info!(iterations = iterations_done, residual_norm, "converged");
                return Ok(SolverResult {
                    solution: x,
                    iterations: iterations_done,
                    residual_norm: final_residual_norm,
                    wall_time: start.elapsed(),
                    convergence_history,
                    algorithm: Algorithm::Neumann,
                });
            }

            // NaN / Inf guard.
            if residual_norm.is_nan() || residual_norm.is_infinite() {
                return Err(SolverError::NumericalInstability {
                    iteration: k,
                    detail: format!("residual became {residual_norm}"),
                });
            }

            // Instability check: residual grew by > 2x.
            if k > 0
                && prev_residual_norm < f64::MAX
                && prev_residual_norm > 0.0
                && residual_norm > INSTABILITY_GROWTH_FACTOR * prev_residual_norm
            {
                warn!(
                    iteration = k,
                    prev = prev_residual_norm,
                    current = residual_norm,
                    "residual diverging",
                );
                return Err(SolverError::NumericalInstability {
                    iteration: k,
                    detail: format!(
                        "residual grew from {prev_residual_norm:.6e} to \
                         {residual_norm:.6e} (>{INSTABILITY_GROWTH_FACTOR:.0}x)",
                    ),
                });
            }

            // Fused update: x[j] += d_inv[j] * r[j]
            // 4-wide unrolled for ILP.
            let chunks = n / 4;
            for c in 0..chunks {
                let j = c * 4;
                x[j] += d_inv[j] * r[j];
                x[j + 1] += d_inv[j + 1] * r[j + 1];
                x[j + 2] += d_inv[j + 2] * r[j + 2];
                x[j + 3] += d_inv[j + 3] * r[j + 3];
            }
            for j in (chunks * 4)..n {
                x[j] += d_inv[j] * r[j];
            }

            prev_residual_norm = residual_norm;
        }

        // Exhausted iteration budget without converging.
        final_residual_norm = prev_residual_norm;
        Err(SolverError::NonConvergence {
            iterations: iterations_done,
            residual: final_residual_norm,
            tolerance: self.tolerance,
        })
    }
}

// ---------------------------------------------------------------------------
// SolverEngine trait implementation (f64 interface)
// ---------------------------------------------------------------------------

impl SolverEngine for NeumannSolver {
    /// Solve via the Neumann series.
    ///
    /// Adapts the `f64` trait interface to the internal `f32` solver by
    /// converting the input matrix and RHS, running the solver, then
    /// returning the `f32` solution.
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let start = Instant::now();

        // Validate that f64 values fit in f32 range.
        for (i, &v) in matrix.values.iter().enumerate() {
            if v.is_finite() && v.abs() > f32::MAX as f64 {
                return Err(SolverError::InvalidInput(ValidationError::NonFiniteValue(
                    format!("matrix value at index {i} ({v:.6e}) overflows f32"),
                )));
            }
        }
        for (i, &v) in rhs.iter().enumerate() {
            if v.is_finite() && v.abs() > f32::MAX as f64 {
                return Err(SolverError::InvalidInput(ValidationError::NonFiniteValue(
                    format!("rhs value at index {i} ({v:.6e}) overflows f32"),
                )));
            }
        }

        // Convert f64 matrix to f32 for the core solver.
        let f32_matrix = CsrMatrix {
            row_ptr: matrix.row_ptr.clone(),
            col_indices: matrix.col_indices.clone(),
            values: matrix.values.iter().map(|&v| v as f32).collect(),
            rows: matrix.rows,
            cols: matrix.cols,
        };
        let f32_rhs: Vec<f32> = rhs.iter().map(|&v| v as f32).collect();

        // Use the tighter of the solver's own tolerance and the caller's budget,
        // but no tighter than f32 precision allows (the Neumann solver operates
        // internally in f32, so residuals below ~f32::EPSILON are unreachable).
        let max_iters = self.max_iterations.min(budget.max_iterations);
        let tol = self
            .tolerance
            .min(budget.tolerance)
            .max(f32::EPSILON as f64 * 4.0);

        let inner_solver = NeumannSolver::new(tol, max_iters);

        let mut result = inner_solver.solve(&f32_matrix, &f32_rhs)?;

        // Check wall-time budget.
        if start.elapsed() > budget.max_time {
            return Err(SolverError::BudgetExhausted {
                reason: "wall-clock time limit exceeded".to_string(),
                elapsed: start.elapsed(),
            });
        }

        // Adjust wall time to include conversion overhead.
        result.wall_time = start.elapsed();
        Ok(result)
    }

    fn estimate_complexity(&self, profile: &SparsityProfile, n: usize) -> ComplexityEstimate {
        // Estimated iterations: ceil( ln(1/tol) / |ln(rho)| )
        let rho = profile.estimated_spectral_radius.max(0.01).min(0.999);
        let est_iters = ((1.0 / self.tolerance).ln() / (1.0 - rho).ln().abs()).ceil() as usize;
        let est_iters = est_iters.min(self.max_iterations).max(1);

        ComplexityEstimate {
            algorithm: Algorithm::Neumann,
            // Each iteration does one SpMV (2 * nnz flops) + O(n) vector ops.
            estimated_flops: (est_iters as u64) * (profile.nnz as u64) * 2,
            estimated_iterations: est_iters,
            // Working memory: x, r, ar (3 vectors of f32).
            estimated_memory_bytes: n * 4 * 3,
            complexity_class: ComplexityClass::SublinearNnz,
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::Neumann
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract `D^{-1}` from a CSR matrix (the reciprocal of each diagonal entry).
///
/// If a diagonal entry is zero or very small, uses `1.0` as a fallback to
/// avoid division by zero.
fn extract_diag_inv_f32(matrix: &CsrMatrix<f32>) -> Vec<f32> {
    let n = matrix.rows;
    let mut d_inv = vec![1.0f32; n];
    for i in 0..n {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        for idx in start..end {
            if matrix.col_indices[idx] == i {
                let diag = matrix.values[idx];
                if diag.abs() > 1e-15 {
                    d_inv[i] = 1.0 / diag;
                } else {
                    warn!(
                        row = i,
                        diag_value = %diag,
                        "zero or near-zero diagonal entry; substituting 1.0 — matrix may be singular"
                    );
                }
                break;
            }
        }
    }
    d_inv
}

/// Compute the L2 (Euclidean) norm of a slice of `f32` values.
///
/// Uses `f64` accumulation to reduce catastrophic cancellation on large
/// vectors.
#[inline]
fn l2_norm_f32(v: &[f32]) -> f32 {
    let sum: f64 = v.iter().map(|&x| (x as f64) * (x as f64)).sum();
    sum.sqrt() as f32
}

/// Scale every element of `v` by `s` in-place.
#[inline]
fn scale_vec_f32(v: &mut [f32], s: f32) {
    for x in v.iter_mut() {
        *x *= s;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CsrMatrix;

    /// Helper: build a diagonally dominant tridiagonal matrix.
    fn tridiag_f32(n: usize, diag_val: f32, off_val: f32) -> CsrMatrix<f32> {
        let mut entries = Vec::new();
        for i in 0..n {
            entries.push((i, i, diag_val));
            if i > 0 {
                entries.push((i, i - 1, off_val));
            }
            if i + 1 < n {
                entries.push((i, i + 1, off_val));
            }
        }
        CsrMatrix::<f32>::from_coo(n, n, entries)
    }

    /// Helper: build a 3x3 system whose eigenvalues are in (0, 2) so that
    /// the Neumann series converges (rho(I - A) < 1).
    fn test_matrix_f64() -> CsrMatrix<f64> {
        CsrMatrix::<f64>::from_coo(
            3,
            3,
            vec![
                (0, 0, 1.0),
                (0, 1, -0.1),
                (1, 0, -0.1),
                (1, 1, 1.0),
                (1, 2, -0.1),
                (2, 1, -0.1),
                (2, 2, 1.0),
            ],
        )
    }

    #[test]
    fn test_new() {
        let solver = NeumannSolver::new(1e-8, 100);
        assert_eq!(solver.tolerance, 1e-8);
        assert_eq!(solver.max_iterations, 100);
    }

    #[test]
    fn test_spectral_radius_identity() {
        let identity = CsrMatrix::<f32>::identity(4);
        let rho = NeumannSolver::estimate_spectral_radius(&identity);
        assert!(rho < 0.1, "expected rho ~ 0 for identity, got {rho}");
    }

    #[test]
    fn test_spectral_radius_pure_diagonal() {
        // For a pure diagonal matrix D, D^{-1}A = I, so M = I - I = 0.
        // The spectral radius should be ~0.
        let a = CsrMatrix::<f32>::from_coo(3, 3, vec![(0, 0, 0.5_f32), (1, 1, 0.5), (2, 2, 0.5)]);
        let rho = NeumannSolver::estimate_spectral_radius(&a);
        assert!(rho < 0.1, "expected rho ~ 0 for diagonal matrix, got {rho}");
    }

    #[test]
    fn test_spectral_radius_empty() {
        let empty = CsrMatrix::<f32> {
            row_ptr: vec![0],
            col_indices: vec![],
            values: vec![],
            rows: 0,
            cols: 0,
        };
        assert_eq!(NeumannSolver::estimate_spectral_radius(&empty), 0.0);
    }

    #[test]
    fn test_spectral_radius_non_diag_dominant() {
        // Matrix where off-diagonal entries dominate:
        // [1  2]
        // [2  1]
        // D^{-1}A = [[1, 2], [2, 1]], so M = I - D^{-1}A = [[0, -2], [-2, 0]].
        // Eigenvalues of M are +2 and -2, so rho(M) = 2 > 1.
        let a = CsrMatrix::<f32>::from_coo(
            2,
            2,
            vec![(0, 0, 1.0_f32), (0, 1, 2.0), (1, 0, 2.0), (1, 1, 1.0)],
        );
        let rho = NeumannSolver::estimate_spectral_radius(&a);
        assert!(
            rho > 1.0,
            "expected rho > 1 for non-diag-dominant matrix, got {rho}"
        );
    }

    #[test]
    fn test_solve_identity() {
        let identity = CsrMatrix::<f32>::identity(3);
        let rhs = vec![1.0_f32, 2.0, 3.0];
        let solver = NeumannSolver::new(1e-6, 100);
        let result = solver.solve(&identity, &rhs).unwrap();
        for (i, (&e, &a)) in rhs.iter().zip(result.solution.iter()).enumerate() {
            assert!((e - a).abs() < 1e-4, "index {i}: expected {e}, got {a}");
        }
        assert!(result.residual_norm < 1e-6);
    }

    #[test]
    fn test_solve_diagonal() {
        let a = CsrMatrix::<f32>::from_coo(3, 3, vec![(0, 0, 0.5_f32), (1, 1, 0.5), (2, 2, 0.5)]);
        let rhs = vec![1.0_f32, 1.0, 1.0];
        let solver = NeumannSolver::new(1e-6, 200);
        let result = solver.solve(&a, &rhs).unwrap();
        for (i, &val) in result.solution.iter().enumerate() {
            assert!(
                (val - 2.0).abs() < 0.01,
                "index {i}: expected ~2.0, got {val}"
            );
        }
    }

    #[test]
    fn test_solve_tridiagonal() {
        // diag=1.0, off=-0.1: Jacobi iteration matrix has rho ~ 0.17.
        // Use 1e-6 tolerance since f32 accumulation limits floor.
        let a = tridiag_f32(5, 1.0, -0.1);
        let rhs = vec![1.0_f32, 0.0, 1.0, 0.0, 1.0];
        let solver = NeumannSolver::new(1e-6, 1000);
        let result = solver.solve(&a, &rhs).unwrap();
        assert!(result.residual_norm < 1e-4);
        assert!(result.iterations > 0);
        assert!(!result.convergence_history.is_empty());
    }

    #[test]
    fn test_solve_empty_system() {
        let a = CsrMatrix::<f32> {
            row_ptr: vec![0],
            col_indices: vec![],
            values: vec![],
            rows: 0,
            cols: 0,
        };
        let result = NeumannSolver::new(1e-6, 10).solve(&a, &[]).unwrap();
        assert_eq!(result.iterations, 0);
        assert!(result.solution.is_empty());
    }

    #[test]
    fn test_solve_dimension_mismatch() {
        let a = CsrMatrix::<f32>::identity(3);
        let rhs = vec![1.0_f32, 2.0];
        let err = NeumannSolver::new(1e-6, 100).solve(&a, &rhs).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("dimension") || msg.contains("mismatch"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_solve_non_square() {
        let a = CsrMatrix::<f32>::from_coo(2, 3, vec![(0, 0, 1.0_f32), (1, 1, 1.0)]);
        let rhs = vec![1.0_f32, 1.0];
        let err = NeumannSolver::new(1e-6, 100).solve(&a, &rhs).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("square") || msg.contains("dimension"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_solve_divergent_matrix() {
        // Non-diag-dominant: off-diagonal entries larger than diagonal.
        let a = CsrMatrix::<f32>::from_coo(
            2,
            2,
            vec![(0, 0, 1.0_f32), (0, 1, 2.0), (1, 0, 2.0), (1, 1, 1.0)],
        );
        let rhs = vec![1.0_f32, 1.0];
        let err = NeumannSolver::new(1e-6, 100).solve(&a, &rhs).unwrap_err();
        assert!(err.to_string().contains("spectral radius"), "got: {}", err);
    }

    #[test]
    fn test_convergence_history_monotone() {
        let a = CsrMatrix::<f32>::identity(4);
        let rhs = vec![1.0_f32; 4];
        let result = NeumannSolver::new(1e-10, 50).solve(&a, &rhs).unwrap();
        assert!(!result.convergence_history.is_empty());
        for window in result.convergence_history.windows(2) {
            assert!(
                window[1].residual_norm <= window[0].residual_norm + 1e-12,
                "residual not decreasing: {} -> {}",
                window[0].residual_norm,
                window[1].residual_norm,
            );
        }
    }

    #[test]
    fn test_algorithm_tag() {
        let a = CsrMatrix::<f32>::identity(2);
        let rhs = vec![1.0_f32; 2];
        let result = NeumannSolver::new(1e-6, 100).solve(&a, &rhs).unwrap();
        assert_eq!(result.algorithm, Algorithm::Neumann);
    }

    #[test]
    fn test_solver_engine_trait_f64() {
        let solver = NeumannSolver::new(1e-6, 200);
        let engine: &dyn SolverEngine = &solver;
        let a = test_matrix_f64();
        let rhs = vec![1.0_f64, 0.0, 1.0];
        let budget = ComputeBudget::default();
        let result = engine.solve(&a, &rhs, &budget).unwrap();
        assert!(result.residual_norm < 1e-4);
        assert_eq!(result.algorithm, Algorithm::Neumann);
    }

    #[test]
    fn test_larger_system_accuracy() {
        let n = 50;
        // diag=1.0, off=-0.1: Jacobi-preconditioned Neumann converges.
        // Use 1e-6 tolerance for f32 precision headroom.
        let a = tridiag_f32(n, 1.0, -0.1);
        let rhs: Vec<f32> = (0..n).map(|i| (i as f32 + 1.0) / n as f32).collect();
        let result = NeumannSolver::new(1e-6, 2000).solve(&a, &rhs).unwrap();
        assert!(
            result.residual_norm < 1e-6,
            "residual too large: {}",
            result.residual_norm
        );
        let mut ax = vec![0.0f32; n];
        a.spmv(&result.solution, &mut ax);
        for i in 0..n {
            assert!(
                (ax[i] - rhs[i]).abs() < 1e-4,
                "A*x[{i}]={} but b[{i}]={}",
                ax[i],
                rhs[i]
            );
        }
    }

    #[test]
    fn test_scalar_system() {
        let a = CsrMatrix::<f32>::from_coo(1, 1, vec![(0, 0, 0.5_f32)]);
        let rhs = vec![4.0_f32];
        let result = NeumannSolver::new(1e-8, 200).solve(&a, &rhs).unwrap();
        assert!(
            (result.solution[0] - 8.0).abs() < 0.01,
            "expected ~8.0, got {}",
            result.solution[0]
        );
    }

    #[test]
    fn test_estimate_complexity() {
        let solver = NeumannSolver::new(1e-6, 1000);
        let profile = SparsityProfile {
            rows: 100,
            cols: 100,
            nnz: 500,
            density: 0.05,
            is_diag_dominant: true,
            estimated_spectral_radius: 0.5,
            estimated_condition: 3.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 5.0,
            max_nnz_per_row: 8,
        };
        let estimate = solver.estimate_complexity(&profile, 100);
        assert_eq!(estimate.algorithm, Algorithm::Neumann);
        assert!(estimate.estimated_flops > 0);
        assert!(estimate.estimated_iterations > 0);
        assert!(estimate.estimated_memory_bytes > 0);
        assert_eq!(estimate.complexity_class, ComplexityClass::SublinearNnz);
    }
}
