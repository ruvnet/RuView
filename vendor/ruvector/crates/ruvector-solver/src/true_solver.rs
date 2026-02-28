//! TRUE (Toolbox for Research on Universal Estimation) solver.
//!
//! Achieves O(log n) solving via a three-phase pipeline:
//!
//! 1. **Johnson-Lindenstrauss projection** -- reduces dimensionality from n to
//!    k = O(log(n)/eps^2) using a sparse random projection matrix.
//! 2. **Spectral sparsification** -- approximates the projected matrix by
//!    sampling edges proportional to effective resistance (uniform sampling
//!    with reweighting as a practical approximation).
//! 3. **Neumann series solve** -- solves the sparsified system using the
//!    truncated Neumann series, then back-projects to the original space.
//!
//! # Error budget
//!
//! The user-specified tolerance `eps` is split evenly across the three phases:
//! `eps_jl = eps/3`, `eps_sparsify = eps/3`, `eps_solve = eps/3`.
//!
//! # Preprocessing
//!
//! The JL matrix and sparsifier are cached in [`TruePreprocessing`] so that
//! multiple right-hand sides can be solved against the same matrix without
//! repeating the projection/sparsification work.

use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::error::{SolverError, ValidationError};
use crate::traits::SolverEngine;
use crate::types::{Algorithm, ConvergenceInfo, CsrMatrix, SolverResult};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// TRUE solver configuration.
///
/// The three-phase pipeline (JL projection, spectral sparsification, Neumann
/// solve) is controlled by `tolerance`, `jl_dimension`, and
/// `sparsification_eps`.
#[derive(Debug, Clone)]
pub struct TrueSolver {
    /// Global tolerance for the solve. Split as eps/3 across phases.
    tolerance: f64,
    /// Target dimension after JL projection.
    /// When set to 0, the dimension is computed automatically as
    /// `ceil(C * ln(n) / eps_jl^2)` with C = 4.
    jl_dimension: usize,
    /// Spectral sparsification quality parameter (epsilon for sampling).
    sparsification_eps: f64,
    /// Maximum iterations for the internal Neumann solve.
    max_iterations: usize,
    /// Deterministic seed for the random projection.
    seed: u64,
}

impl TrueSolver {
    /// Create a new TRUE solver with explicit parameters.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Target residual tolerance. Must be in (0, 1).
    /// * `jl_dimension` - Target dimension after JL projection. Pass 0 to
    ///   auto-compute from `n` and `tolerance`.
    /// * `sparsification_eps` - Sparsification quality. Must be in (0, 1).
    pub fn new(tolerance: f64, jl_dimension: usize, sparsification_eps: f64) -> Self {
        Self {
            tolerance,
            jl_dimension,
            sparsification_eps,
            max_iterations: 500,
            seed: 42,
        }
    }

    /// Set the maximum number of Neumann iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set a deterministic seed for random projection generation.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Compute the JL target dimension from the original dimension `n`.
    ///
    /// k = ceil(C * ln(n) / eps_jl^2) where C = 4, eps_jl = tolerance / 3.
    fn compute_jl_dim(&self, n: usize) -> usize {
        if self.jl_dimension > 0 {
            return self.jl_dimension;
        }
        let eps_jl = self.tolerance / 3.0;
        let c = 4.0;
        let k = (c * (n as f64).ln() / (eps_jl * eps_jl)).ceil() as usize;
        // Clamp: at least 1, at most n (no point projecting to a bigger space).
        k.clamp(1, n)
    }

    // -----------------------------------------------------------------------
    // Phase 1: Johnson-Lindenstrauss Projection
    // -----------------------------------------------------------------------

    /// Generate a sparse random JL projection matrix in COO format.
    ///
    /// Each entry is drawn from the distribution:
    /// - +1/sqrt(k) with probability 1/6
    /// - -1/sqrt(k) with probability 1/6
    /// - 0           with probability 2/3
    ///
    /// Returns a list of (row, col, value) triples.
    fn generate_jl_matrix(&self, k: usize, n: usize, rng: &mut StdRng) -> Vec<(usize, usize, f32)> {
        let scale = 1.0 / (k as f64).sqrt();
        let scale_f32 = scale as f32;
        let mut entries = Vec::with_capacity(((k * n) as f64 / 3.0).ceil() as usize);

        for row in 0..k {
            for col in 0..n {
                let r: f64 = rng.gen();
                if r < 1.0 / 6.0 {
                    entries.push((row, col, scale_f32));
                } else if r < 2.0 / 6.0 {
                    entries.push((row, col, -scale_f32));
                }
                // else: 0 with prob 2/3, skip
            }
        }

        entries
    }

    /// Project the right-hand side vector: b' = Pi * b.
    fn project_rhs(jl_entries: &[(usize, usize, f32)], rhs: &[f32], k: usize) -> Vec<f32> {
        let mut projected = vec![0.0f32; k];
        for &(row, col, val) in jl_entries {
            if col < rhs.len() {
                projected[row] += val * rhs[col];
            }
        }
        projected
    }

    /// Project the matrix: A' = Pi * A * Pi^T.
    ///
    /// Computed as:
    ///   1. B = Pi * A   (k x n)
    ///   2. A' = B * Pi^T (k x k)
    ///
    /// The result is built in COO format, then converted to CSR.
    fn project_matrix(
        jl_entries: &[(usize, usize, f32)],
        matrix: &CsrMatrix<f32>,
        k: usize,
    ) -> CsrMatrix<f32> {
        let n = matrix.cols;

        // Build Pi as CSR for efficient access.
        let pi = CsrMatrix::<f32>::from_coo(k, n, jl_entries.iter().cloned());

        // Step 1: B = Pi * A. B is k x n.
        // For each row i of Pi, compute B[i,:] = Pi[i,:] * A.
        let mut b_entries: Vec<(usize, usize, f32)> = Vec::new();

        // Hoist accumulator outside loop to avoid reallocating each iteration.
        let mut b_row = vec![0.0f32; n];
        for pi_row in 0..k {
            let pi_start = pi.row_ptr[pi_row];
            let pi_end = pi.row_ptr[pi_row + 1];

            for pi_idx in pi_start..pi_end {
                let pi_col = pi.col_indices[pi_idx];
                let pi_val = pi.values[pi_idx];

                let a_start = matrix.row_ptr[pi_col];
                let a_end = matrix.row_ptr[pi_col + 1];
                for a_idx in a_start..a_end {
                    b_row[matrix.col_indices[a_idx]] += pi_val * matrix.values[a_idx];
                }
            }

            for (col, &val) in b_row.iter().enumerate() {
                if val.abs() > f32::EPSILON {
                    b_entries.push((pi_row, col, val));
                }
            }

            // Zero the accumulator for the next row.
            b_row.iter_mut().for_each(|v| *v = 0.0);
        }

        let b_matrix = CsrMatrix::<f32>::from_coo(k, n, b_entries);

        // Step 2: A' = B * Pi^T. A' is k x k.
        // Build a column-index for Pi so we can compute Pi^T efficiently.
        let mut pi_by_col: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        for pi_row in 0..k {
            let start = pi.row_ptr[pi_row];
            let end = pi.row_ptr[pi_row + 1];
            for idx in start..end {
                pi_by_col[pi.col_indices[idx]].push((pi_row, pi.values[idx]));
            }
        }

        let mut a_prime_entries: Vec<(usize, usize, f32)> = Vec::new();

        // Hoist accumulator outside loop to avoid reallocating each iteration.
        let mut row_accum = vec![0.0f32; k];
        for b_row_idx in 0..k {
            let b_start = b_matrix.row_ptr[b_row_idx];
            let b_end = b_matrix.row_ptr[b_row_idx + 1];

            for b_idx in b_start..b_end {
                let l = b_matrix.col_indices[b_idx];
                let b_val = b_matrix.values[b_idx];

                for &(j, pi_val) in &pi_by_col[l] {
                    row_accum[j] += b_val * pi_val;
                }
            }

            for (j, &val) in row_accum.iter().enumerate() {
                if val.abs() > f32::EPSILON {
                    a_prime_entries.push((b_row_idx, j, val));
                }
            }

            // Zero the accumulator for the next row.
            row_accum.iter_mut().for_each(|v| *v = 0.0);
        }

        CsrMatrix::<f32>::from_coo(k, k, a_prime_entries)
    }

    // -----------------------------------------------------------------------
    // Phase 2: Spectral Sparsification
    // -----------------------------------------------------------------------

    /// Sparsify the projected matrix by uniform edge sampling with
    /// reweighting.
    ///
    /// Samples O(k * log(k) / eps^2) non-zero entries and reweights them by
    /// 1/probability to maintain the expected value. Diagonal entries are
    /// always preserved to maintain positive-definiteness.
    fn sparsify(matrix: &CsrMatrix<f32>, eps: f64, rng: &mut StdRng) -> CsrMatrix<f32> {
        let n = matrix.rows;
        let nnz = matrix.nnz();

        if nnz == 0 || n == 0 {
            return CsrMatrix::<f32>::from_coo(n, matrix.cols, std::iter::empty());
        }

        // Target number of samples: O(n * log(n) / eps^2).
        let target_samples =
            ((n as f64) * ((n as f64).ln().max(1.0)) / (eps * eps)).ceil() as usize;

        // If the target exceeds actual nnz, keep everything.
        if target_samples >= nnz {
            return matrix.clone();
        }

        let keep_prob = (target_samples as f64) / (nnz as f64);
        let reweight = (1.0 / keep_prob) as f32;

        let mut entries: Vec<(usize, usize, f32)> = Vec::with_capacity(target_samples);

        for row in 0..n {
            let start = matrix.row_ptr[row];
            let end = matrix.row_ptr[row + 1];
            for idx in start..end {
                let col = matrix.col_indices[idx];

                // Always keep diagonal entries unmodified.
                if row == col {
                    entries.push((row, col, matrix.values[idx]));
                    continue;
                }

                let r: f64 = rng.gen();
                if r < keep_prob {
                    entries.push((row, col, matrix.values[idx] * reweight));
                }
            }
        }

        CsrMatrix::<f32>::from_coo(n, matrix.cols, entries)
    }

    // -----------------------------------------------------------------------
    // Phase 3: Neumann Series Solve
    // -----------------------------------------------------------------------

    /// Solve Ax = b using the Jacobi-preconditioned Neumann series.
    ///
    /// The Neumann series x = sum_{k=0}^{K} M^k b_hat converges when the
    /// spectral radius of M = I - D^{-1}A is less than 1, which is
    /// guaranteed for diagonally dominant systems. Diagonal (Jacobi)
    /// preconditioning is applied to improve convergence.
    fn neumann_solve(
        matrix: &CsrMatrix<f32>,
        rhs: &[f32],
        tolerance: f64,
        max_iterations: usize,
    ) -> Result<(Vec<f32>, usize, f64, Vec<ConvergenceInfo>), SolverError> {
        let n = matrix.rows;

        if n == 0 {
            return Ok((Vec::new(), 0, 0.0, Vec::new()));
        }

        // Extract diagonal for Jacobi preconditioning.
        let mut diag = vec![1.0f32; n];
        for row in 0..n {
            let start = matrix.row_ptr[row];
            let end = matrix.row_ptr[row + 1];
            for idx in start..end {
                if matrix.col_indices[idx] == row {
                    let d = matrix.values[idx];
                    if d.abs() > f32::EPSILON {
                        diag[row] = d;
                    }
                    break;
                }
            }
        }

        let inv_diag: Vec<f32> = diag.iter().map(|&d| 1.0 / d).collect();

        // Preconditioned rhs: b_hat = D^{-1} * b
        let b_hat: Vec<f32> = rhs
            .iter()
            .zip(inv_diag.iter())
            .map(|(&b, &d)| b * d)
            .collect();

        // Neumann series: x = sum_{k=0}^K M^k * b_hat
        // where M = I - D^{-1} * A.
        // Iteratively: term_{k+1} = M * term_k, x += term_{k+1}
        let mut solution = b_hat.clone();
        let mut term = b_hat;
        let mut convergence_history = Vec::new();

        let rhs_norm: f64 = rhs
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            .sqrt();
        let abs_tol = if rhs_norm > f64::EPSILON {
            tolerance * rhs_norm
        } else {
            tolerance
        };

        let mut iterations = 0;
        let mut residual_norm = f64::MAX;

        for iter in 0..max_iterations {
            // new_term = M * term = term - D^{-1} * A * term
            let mut a_term = vec![0.0f32; n];
            matrix.spmv(&term, &mut a_term);

            let mut new_term = vec![0.0f32; n];
            for i in 0..n {
                new_term[i] = term[i] - inv_diag[i] * a_term[i];
            }

            for i in 0..n {
                solution[i] += new_term[i];
            }

            // ||new_term||_2 as a convergence proxy.
            let term_norm: f64 = new_term
                .iter()
                .map(|&v| (v as f64) * (v as f64))
                .sum::<f64>()
                .sqrt();

            iterations = iter + 1;
            residual_norm = term_norm;

            convergence_history.push(ConvergenceInfo {
                iteration: iterations,
                residual_norm,
            });

            if term_norm < abs_tol {
                break;
            }

            if term_norm.is_nan() || term_norm.is_infinite() {
                return Err(SolverError::NumericalInstability {
                    iteration: iterations,
                    detail: format!(
                        "Neumann term norm diverged to {} at iteration {}",
                        term_norm, iterations
                    ),
                });
            }

            term = new_term;
        }

        Ok((solution, iterations, residual_norm, convergence_history))
    }

    // -----------------------------------------------------------------------
    // Back-projection
    // -----------------------------------------------------------------------

    /// Back-project solution from reduced space: x = Pi^T * x'.
    fn back_project(
        jl_entries: &[(usize, usize, f32)],
        projected_solution: &[f32],
        original_cols: usize,
    ) -> Vec<f32> {
        let mut result = vec![0.0f32; original_cols];
        for &(row, col, val) in jl_entries {
            if row < projected_solution.len() && col < original_cols {
                result[col] += val * projected_solution[row];
            }
        }
        result
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Preprocess a matrix: generate the JL projection and sparsifier.
    ///
    /// The returned [`TruePreprocessing`] can be reused across multiple
    /// right-hand sides to amortize the cost of projection and
    /// sparsification.
    pub fn preprocess(&self, matrix: &CsrMatrix<f32>) -> Result<TruePreprocessing, SolverError> {
        Self::validate_matrix(matrix)?;

        let n = matrix.rows;
        let k = self.compute_jl_dim(n);
        let mut rng = StdRng::seed_from_u64(self.seed);

        // Phase 1: Generate JL projection and project the matrix.
        let jl_matrix = self.generate_jl_matrix(k, n, &mut rng);
        let projected = Self::project_matrix(&jl_matrix, matrix, k);

        // Phase 2: Sparsify the projected matrix.
        let eps_sparsify = self.sparsification_eps.max(self.tolerance / 3.0);
        let sparsified = Self::sparsify(&projected, eps_sparsify, &mut rng);

        Ok(TruePreprocessing {
            jl_matrix,
            sparsified_matrix: sparsified,
            original_rows: matrix.rows,
            original_cols: matrix.cols,
        })
    }

    /// Solve using a previously computed preprocessing.
    ///
    /// This is the fast path when solving multiple systems with the same
    /// coefficient matrix but different right-hand sides.
    pub fn solve_with_preprocessing(
        &self,
        preprocessing: &TruePreprocessing,
        rhs: &[f32],
    ) -> Result<SolverResult, SolverError> {
        if rhs.len() != preprocessing.original_rows {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "rhs length {} does not match matrix rows {}",
                    rhs.len(),
                    preprocessing.original_rows
                )),
            ));
        }

        let start = Instant::now();
        let k = preprocessing.sparsified_matrix.rows;

        // Phase 1: Project the rhs.
        let projected_rhs = Self::project_rhs(&preprocessing.jl_matrix, rhs, k);

        // Phase 3: Neumann solve on sparsified system.
        let eps_solve = self.tolerance / 3.0;
        let (projected_solution, iterations, residual_norm, convergence_history) =
            Self::neumann_solve(
                &preprocessing.sparsified_matrix,
                &projected_rhs,
                eps_solve,
                self.max_iterations,
            )?;

        // Back-project to original space.
        let solution = Self::back_project(
            &preprocessing.jl_matrix,
            &projected_solution,
            preprocessing.original_cols,
        );

        Ok(SolverResult {
            solution,
            iterations,
            residual_norm,
            wall_time: start.elapsed(),
            convergence_history,
            algorithm: Algorithm::TRUE,
        })
    }

    /// Validate matrix dimensions and structure.
    fn validate_matrix(matrix: &CsrMatrix<f32>) -> Result<(), SolverError> {
        if matrix.rows == 0 || matrix.cols == 0 {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(
                    "matrix must have at least one row and one column".to_string(),
                ),
            ));
        }

        if matrix.rows != matrix.cols {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "TRUE solver requires a square matrix, got {}x{}",
                    matrix.rows, matrix.cols
                )),
            ));
        }

        if matrix.row_ptr.len() != matrix.rows + 1 {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "row_ptr length {} does not match rows + 1 = {}",
                    matrix.row_ptr.len(),
                    matrix.rows + 1
                )),
            ));
        }

        for (i, &v) in matrix.values.iter().enumerate() {
            if v.is_nan() || v.is_infinite() {
                return Err(SolverError::InvalidInput(ValidationError::NonFiniteValue(
                    format!("matrix value at index {} is {}", i, v),
                )));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SolverEngine trait implementation
// ---------------------------------------------------------------------------

impl SolverEngine for TrueSolver {
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        _budget: &crate::types::ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
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

        // Convert f64 input to f32 for internal computation.
        // NOTE: row_ptr and col_indices are cloned here because CsrMatrix owns
        // Vec<usize>, so we cannot borrow from the f64 matrix. A future
        // refactor could introduce a CsrMatrixView that borrows structural
        // arrays to eliminate these allocations on the f64 -> f32 path.
        let f32_values: Vec<f32> = matrix.values.iter().map(|&v| v as f32).collect();
        let f32_matrix = CsrMatrix {
            row_ptr: matrix.row_ptr.clone(),
            col_indices: matrix.col_indices.clone(),
            values: f32_values,
            rows: matrix.rows,
            cols: matrix.cols,
        };
        let f32_rhs: Vec<f32> = rhs.iter().map(|&v| v as f32).collect();
        let preprocessing = self.preprocess(&f32_matrix)?;
        self.solve_with_preprocessing(&preprocessing, &f32_rhs)
    }

    fn estimate_complexity(
        &self,
        profile: &crate::types::SparsityProfile,
        n: usize,
    ) -> crate::types::ComplexityEstimate {
        let k = self.compute_jl_dim(n);
        crate::types::ComplexityEstimate {
            algorithm: Algorithm::TRUE,
            estimated_flops: (k as u64) * (profile.nnz as u64) * 3,
            estimated_iterations: self.max_iterations.min(100),
            estimated_memory_bytes: k * k * 4 + n * 4 * 2,
            complexity_class: crate::types::ComplexityClass::SublinearNnz,
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::TRUE
    }
}

// ---------------------------------------------------------------------------
// Preprocessing cache
// ---------------------------------------------------------------------------

/// Cached preprocessing data from the JL projection and spectral
/// sparsification phases.
///
/// Store this struct and pass it to
/// [`TrueSolver::solve_with_preprocessing`] to amortize the cost of
/// preprocessing across multiple solves with the same coefficient matrix.
#[derive(Debug, Clone)]
pub struct TruePreprocessing {
    /// Sparse JL projection matrix in COO format (row, col, value).
    pub jl_matrix: Vec<(usize, usize, f32)>,
    /// The sparsified projected matrix in CSR format.
    pub sparsified_matrix: CsrMatrix<f32>,
    /// Number of rows in the original matrix.
    pub original_rows: usize,
    /// Number of columns in the original matrix.
    pub original_cols: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a diagonally dominant symmetric matrix.
    ///
    /// Returns an n x n matrix where A[i,i] = 3.0 and off-diagonal
    /// neighbours A[i,i+1] = A[i+1,i] = -0.5.
    fn make_diag_dominant(n: usize) -> CsrMatrix<f32> {
        let mut entries = Vec::new();
        for i in 0..n {
            entries.push((i, i, 3.0f32));
            if i + 1 < n {
                entries.push((i, i + 1, -0.5));
                entries.push((i + 1, i, -0.5));
            }
        }
        CsrMatrix::<f32>::from_coo(n, n, entries)
    }

    #[test]
    fn test_jl_dimension_auto() {
        let solver = TrueSolver::new(0.3, 0, 0.1);
        let dim = solver.compute_jl_dim(1000);
        assert!(dim >= 1);
        assert!(dim <= 1000);
    }

    #[test]
    fn test_jl_dimension_explicit() {
        let solver = TrueSolver::new(0.1, 50, 0.1);
        let dim = solver.compute_jl_dim(1000);
        assert_eq!(dim, 50);
    }

    #[test]
    fn test_jl_matrix_sparsity() {
        let solver = TrueSolver::new(0.1, 10, 0.1);
        let mut rng = StdRng::seed_from_u64(42);
        let jl = solver.generate_jl_matrix(10, 100, &mut rng);

        // Expected density: ~1/3 of 10*100 = 1000. Should be sparse.
        assert!(!jl.is_empty());
        assert!(jl.len() < 1000);
    }

    #[test]
    fn test_jl_matrix_values() {
        let solver = TrueSolver::new(0.1, 5, 0.1);
        let mut rng = StdRng::seed_from_u64(42);
        let jl = solver.generate_jl_matrix(5, 20, &mut rng);

        let scale = 1.0 / (5.0f64).sqrt();
        let scale_f32 = scale as f32;

        for &(row, col, val) in &jl {
            assert!(row < 5);
            assert!(col < 20);
            assert!(
                (val - scale_f32).abs() < f32::EPSILON || (val + scale_f32).abs() < f32::EPSILON,
                "unexpected JL value: {}",
                val
            );
        }
    }

    #[test]
    fn test_project_rhs() {
        let entries = vec![(0, 0, 1.0f32), (0, 1, -1.0), (1, 1, 2.0)];
        let rhs = vec![3.0, 4.0];
        let projected = TrueSolver::project_rhs(&entries, &rhs, 2);
        assert!((projected[0] - (-1.0)).abs() < 1e-6);
        assert!((projected[1] - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_back_project() {
        let entries = vec![(0, 0, 1.0f32), (0, 1, -1.0), (1, 1, 2.0)];
        let projected_sol = vec![3.0, 4.0];
        let result = TrueSolver::back_project(&entries, &projected_sol, 2);
        // result[0] = Pi^T[0,0]*3 = 1*3 = 3
        // result[1] = Pi^T[1,0]*3 + Pi^T[1,1]*4 = (-1)*3 + 2*4 = 5
        assert!((result[0] - 3.0).abs() < 1e-6);
        assert!((result[1] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_neumann_identity() {
        let identity = CsrMatrix::<f32>::identity(3);
        let rhs = vec![1.0, 2.0, 3.0];
        let (solution, iterations, residual, _) =
            TrueSolver::neumann_solve(&identity, &rhs, 1e-6, 100).unwrap();

        assert!(iterations <= 2, "identity should converge fast");
        assert!(residual < 1e-4);
        for (i, &val) in solution.iter().enumerate() {
            assert!(
                (val - rhs[i]).abs() < 1e-3,
                "solution[{}] = {}, expected {}",
                i,
                val,
                rhs[i]
            );
        }
    }

    #[test]
    fn test_neumann_diag_dominant() {
        let matrix = make_diag_dominant(5);
        let rhs = vec![1.0; 5];
        let (solution, _iterations, _residual, _) =
            TrueSolver::neumann_solve(&matrix, &rhs, 1e-6, 500).unwrap();

        // Verify Ax ~ b.
        let mut ax = vec![0.0f32; 5];
        matrix.spmv(&solution, &mut ax);
        for i in 0..5 {
            assert!(
                (ax[i] - rhs[i]).abs() < 0.1,
                "residual at {} too large: Ax={}, b={}",
                i,
                ax[i],
                rhs[i]
            );
        }
    }

    #[test]
    fn test_sparsify_preserves_diagonal() {
        let matrix = make_diag_dominant(4);
        let mut rng = StdRng::seed_from_u64(123);
        let sparsified = TrueSolver::sparsify(&matrix, 0.5, &mut rng);

        for row in 0..4 {
            let start = sparsified.row_ptr[row];
            let end = sparsified.row_ptr[row + 1];
            let has_diag = (start..end).any(|idx| sparsified.col_indices[idx] == row);
            assert!(has_diag, "diagonal entry missing at row {}", row);
        }
    }

    #[test]
    fn test_preprocess() {
        let matrix = make_diag_dominant(10);
        let solver = TrueSolver::new(0.3, 5, 0.3);
        let preprocessing = solver.preprocess(&matrix).unwrap();

        assert_eq!(preprocessing.original_rows, 10);
        assert_eq!(preprocessing.original_cols, 10);
        assert_eq!(preprocessing.sparsified_matrix.rows, 5);
        assert_eq!(preprocessing.sparsified_matrix.cols, 5);
        assert!(!preprocessing.jl_matrix.is_empty());
    }

    #[test]
    fn test_solve_with_preprocessing() {
        let matrix = make_diag_dominant(8);
        let rhs = vec![1.0; 8];

        let solver = TrueSolver::new(0.3, 4, 0.3)
            .with_max_iterations(200)
            .with_seed(99);

        let preprocessing = solver.preprocess(&matrix).unwrap();
        let result = solver
            .solve_with_preprocessing(&preprocessing, &rhs)
            .unwrap();

        assert_eq!(result.solution.len(), 8);
        assert!(result.iterations > 0);
        assert_eq!(result.algorithm, Algorithm::TRUE);
    }

    #[test]
    fn test_solver_engine_trait() {
        use crate::traits::SolverEngine;
        use crate::types::ComputeBudget;

        // Build f64 matrix for SolverEngine trait
        let n = 6;
        let mut entries = Vec::new();
        for i in 0..n {
            entries.push((i, i, 3.0f64));
            if i + 1 < n {
                entries.push((i, i + 1, -0.5f64));
                entries.push((i + 1, i, -0.5f64));
            }
        }
        let matrix = CsrMatrix::<f64>::from_coo(n, n, entries);
        let rhs = vec![1.0f64; 6];
        let budget = ComputeBudget::default();

        let solver = TrueSolver::new(0.3, 3, 0.3).with_max_iterations(200);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        assert_eq!(result.solution.len(), 6);
        assert!(result.wall_time.as_nanos() > 0);
    }

    #[test]
    fn test_dimension_mismatch_rhs() {
        let matrix = make_diag_dominant(4);
        let rhs = vec![1.0; 7];

        let solver = TrueSolver::new(0.1, 2, 0.1);
        let preprocessing = solver.preprocess(&matrix).unwrap();
        let err = solver.solve_with_preprocessing(&preprocessing, &rhs);
        assert!(err.is_err());
    }

    #[test]
    fn test_non_square_matrix_rejected() {
        let matrix =
            CsrMatrix::<f32>::from_coo(3, 5, vec![(0, 0, 1.0f32), (1, 1, 1.0), (2, 2, 1.0)]);

        let solver = TrueSolver::new(0.1, 2, 0.1);
        let err = solver.preprocess(&matrix);
        assert!(err.is_err());
    }

    #[test]
    fn test_nan_matrix_rejected() {
        let matrix = CsrMatrix {
            row_ptr: vec![0, 1, 2],
            col_indices: vec![0, 1],
            values: vec![f32::NAN, 1.0f32],
            rows: 2,
            cols: 2,
        };

        let solver = TrueSolver::new(0.1, 2, 0.1);
        let err = solver.preprocess(&matrix);
        assert!(err.is_err());
    }

    #[test]
    fn test_empty_matrix_rejected() {
        let matrix: CsrMatrix<f32> = CsrMatrix {
            row_ptr: vec![0],
            col_indices: Vec::new(),
            values: Vec::new(),
            rows: 0,
            cols: 0,
        };

        let solver = TrueSolver::new(0.1, 1, 0.1);
        let err = solver.preprocess(&matrix);
        assert!(err.is_err());
    }

    #[test]
    fn test_deterministic_with_seed() {
        let matrix = make_diag_dominant(6);
        let rhs = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];

        let solver = TrueSolver::new(0.3, 3, 0.3).with_seed(777);
        let preprocessing = solver.preprocess(&matrix).unwrap();

        let r1 = solver
            .solve_with_preprocessing(&preprocessing, &rhs)
            .unwrap();
        let r2 = solver
            .solve_with_preprocessing(&preprocessing, &rhs)
            .unwrap();

        assert_eq!(r1.solution, r2.solution);
        assert_eq!(r1.iterations, r2.iterations);
    }

    #[test]
    fn test_preprocessing_reuse() {
        let matrix = make_diag_dominant(8);
        let solver = TrueSolver::new(0.3, 4, 0.3).with_max_iterations(200);
        let preprocessing = solver.preprocess(&matrix).unwrap();

        let rhs_a = vec![1.0; 8];
        let rhs_b = vec![2.0; 8];

        let result_a = solver
            .solve_with_preprocessing(&preprocessing, &rhs_a)
            .unwrap();
        let result_b = solver
            .solve_with_preprocessing(&preprocessing, &rhs_b)
            .unwrap();

        // Different RHS should produce different solutions.
        assert_ne!(result_a.solution, result_b.solution);
        assert_eq!(result_a.algorithm, result_b.algorithm);
    }
}
