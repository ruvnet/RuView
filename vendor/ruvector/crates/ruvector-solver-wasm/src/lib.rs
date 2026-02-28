//! WASM bindings for the RuVector sublinear-time solver.
//!
//! Exposes a [`JsSolver`] struct that can be constructed from JavaScript and
//! used to solve sparse linear systems, compute Personalized PageRank, and
//! estimate solve complexity -- all within the browser or any WASM runtime.
//!
//! # Quick Start (JavaScript)
//!
//! ```js
//! import { JsSolver } from "ruvector-solver-wasm";
//!
//! const solver = new JsSolver();
//!
//! // CSR representation of a 3x3 diagonally-dominant matrix.
//! const values   = new Float32Array([4, -1, -1, 4, -1, -1, 4]);
//! const colIdx   = new Uint32Array([0, 1, 0, 1, 2, 1, 2]);
//! const rowPtrs  = new Uint32Array([0, 2, 5, 7]);
//! const rhs      = new Float32Array([1, 0, 1]);
//!
//! const result = solver.solve(values, colIdx, rowPtrs, 3, 3, rhs);
//! console.log(result);
//! ```

mod utils;

use ruvector_solver::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, CsrMatrix, SparsityProfile,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::utils::{console_log, csr_from_js_arrays, set_panic_hook};

// ---------------------------------------------------------------------------
// Module initialisation
// ---------------------------------------------------------------------------

/// Called automatically when the WASM module is loaded.
#[wasm_bindgen(start)]
pub fn init() {
    set_panic_hook();
    console_log("ruvector-solver-wasm module loaded");
}

/// Return the crate version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---------------------------------------------------------------------------
// JsSolver
// ---------------------------------------------------------------------------

/// Top-level solver handle exposed to JavaScript.
///
/// Wraps the algorithm router and iterative solvers, providing a high-level
/// API that accepts CSR arrays directly from JS typed arrays.
#[wasm_bindgen]
pub struct JsSolver {
    /// Default maximum iterations.
    max_iterations: usize,
    /// Default convergence tolerance.
    tolerance: f64,
    /// Default teleportation probability for PageRank.
    alpha: f64,
}

#[wasm_bindgen]
impl JsSolver {
    /// Construct a new solver with default parameters.
    ///
    /// - `max_iterations`: 1000
    /// - `tolerance`: 1e-6
    /// - `alpha` (PageRank teleport): 0.15
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            alpha: 0.15,
        }
    }

    /// Set the maximum number of iterations for iterative solvers.
    #[wasm_bindgen(js_name = "setMaxIterations")]
    pub fn set_max_iterations(&mut self, max_iterations: usize) {
        self.max_iterations = max_iterations;
    }

    /// Set the convergence tolerance.
    #[wasm_bindgen(js_name = "setTolerance")]
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance;
    }

    /// Set the teleportation probability for PageRank.
    #[wasm_bindgen(js_name = "setAlpha")]
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha;
    }

    // -----------------------------------------------------------------------
    // Solve Ax = b
    // -----------------------------------------------------------------------

    /// Solve a sparse linear system `Ax = b`.
    ///
    /// The matrix `A` is provided in CSR format via three flat arrays.
    /// Returns a JSON-serialisable result object on success.
    ///
    /// # Arguments
    ///
    /// * `values`      - Non-zero values (`Float32Array`).
    /// * `col_indices` - Column indices for each non-zero (`Uint32Array`).
    /// * `row_ptrs`    - Row pointers of length `rows + 1` (`Uint32Array`).
    /// * `rows`        - Number of rows.
    /// * `cols`        - Number of columns.
    /// * `rhs`         - Right-hand side vector `b` (`Float32Array`).
    ///
    /// # Errors
    ///
    /// Returns `JsError` on invalid input or non-convergence.
    pub fn solve(
        &self,
        values: &[f32],
        col_indices: &[u32],
        row_ptrs: &[u32],
        rows: usize,
        cols: usize,
        rhs: &[f32],
    ) -> Result<JsValue, JsError> {
        let csr = csr_from_js_arrays(values, col_indices, row_ptrs, rows, cols)
            .map_err(|e| JsError::new(&e))?;

        if rows != cols {
            return Err(JsError::new(
                "solve requires a square matrix (rows must equal cols)",
            ));
        }
        if rhs.len() != rows {
            return Err(JsError::new(&format!(
                "rhs length {} does not match matrix rows {}",
                rhs.len(),
                rows,
            )));
        }

        // Analyse sparsity to choose the algorithm.
        let profile = analyze_sparsity(&csr);
        let algorithm = select_algorithm(&profile);

        // Perform the solve.
        let start = js_sys::Date::now();
        let result = match algorithm {
            Algorithm::Neumann => neumann_solve(&csr, rhs, self.tolerance, self.max_iterations),
            Algorithm::CG => cg_solve(&csr, rhs, self.tolerance, self.max_iterations),
            _ => {
                // Fallback: try Neumann first, then CG.
                let nr = neumann_solve(&csr, rhs, self.tolerance, self.max_iterations);
                if nr.converged {
                    nr
                } else {
                    cg_solve(&csr, rhs, self.tolerance, self.max_iterations)
                }
            }
        };
        let elapsed_us = ((js_sys::Date::now() - start) * 1000.0) as u64;

        let js_result = JsSolverResult {
            solution: result.solution,
            iterations: result.iterations,
            residual: result.residual,
            converged: result.converged,
            algorithm: result.algorithm.to_string(),
            time_us: elapsed_us,
        };

        serde_wasm_bindgen::to_value(&js_result)
            .map_err(|e| JsError::new(&format!("serialisation error: {}", e)))
    }

    // -----------------------------------------------------------------------
    // Personalized PageRank
    // -----------------------------------------------------------------------

    /// Compute Personalized PageRank from a single source node.
    ///
    /// Uses the power-iteration method with teleportation probability `alpha`
    /// (configurable via [`set_alpha`](JsSolver::set_alpha)).
    ///
    /// # Arguments
    ///
    /// * `values`      - Edge weights (`Float32Array`).
    /// * `col_indices` - Column indices (`Uint32Array`).
    /// * `row_ptrs`    - Row pointers (`Uint32Array`).
    /// * `rows`        - Number of nodes.
    /// * `source`      - Source node index.
    /// * `tolerance`   - Convergence tolerance (L1 residual).
    ///
    /// # Errors
    ///
    /// Returns `JsError` on invalid input.
    pub fn pagerank(
        &self,
        values: &[f32],
        col_indices: &[u32],
        row_ptrs: &[u32],
        rows: usize,
        source: usize,
        tolerance: f64,
    ) -> Result<JsValue, JsError> {
        let csr = csr_from_js_arrays(values, col_indices, row_ptrs, rows, rows)
            .map_err(|e| JsError::new(&e))?;

        if source >= rows {
            return Err(JsError::new(&format!(
                "source node {} out of bounds (graph has {} nodes)",
                source, rows,
            )));
        }

        let tol = if tolerance > 0.0 {
            tolerance
        } else {
            self.tolerance
        };

        let start = js_sys::Date::now();
        let result = power_iteration_ppr(&csr, source, self.alpha, tol, self.max_iterations);
        let elapsed_us = ((js_sys::Date::now() - start) * 1000.0) as u64;

        let js_result = JsPageRankResult {
            scores: result.scores,
            iterations: result.iterations,
            residual: result.residual,
            converged: result.converged,
            time_us: elapsed_us,
        };

        serde_wasm_bindgen::to_value(&js_result)
            .map_err(|e| JsError::new(&format!("serialisation error: {}", e)))
    }

    // -----------------------------------------------------------------------
    // Complexity estimation
    // -----------------------------------------------------------------------

    /// Estimate the computational complexity of solving a system with the
    /// given matrix without performing the actual solve.
    ///
    /// Returns a JSON object with the selected algorithm, estimated FLOPS,
    /// estimated iterations, memory usage, and complexity class.
    #[wasm_bindgen(js_name = "estimateComplexity")]
    pub fn estimate_complexity(
        &self,
        values: &[f32],
        col_indices: &[u32],
        row_ptrs: &[u32],
        rows: usize,
        cols: usize,
    ) -> Result<JsValue, JsError> {
        let csr = csr_from_js_arrays(values, col_indices, row_ptrs, rows, cols)
            .map_err(|e| JsError::new(&e))?;

        let profile = analyze_sparsity(&csr);
        let algorithm = select_algorithm(&profile);
        let estimate = build_complexity_estimate(&profile, algorithm);

        let js_est = JsComplexityEstimate {
            algorithm: algorithm.to_string(),
            estimated_flops: estimate.estimated_flops,
            estimated_iterations: estimate.estimated_iterations,
            estimated_memory_bytes: estimate.estimated_memory_bytes,
            complexity_class: format!("{:?}", estimate.complexity_class),
            density: profile.density,
            is_diag_dominant: profile.is_diag_dominant,
            estimated_spectral_radius: profile.estimated_spectral_radius,
        };

        serde_wasm_bindgen::to_value(&js_est)
            .map_err(|e| JsError::new(&format!("serialisation error: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// JS-facing result types (serde-serialisable)
// ---------------------------------------------------------------------------

/// JSON-serialisable solve result returned to JavaScript.
#[derive(Serialize)]
struct JsSolverResult {
    solution: Vec<f32>,
    iterations: usize,
    residual: f64,
    converged: bool,
    algorithm: String,
    time_us: u64,
}

/// JSON-serialisable PageRank result.
#[derive(Serialize)]
struct JsPageRankResult {
    scores: Vec<f32>,
    iterations: usize,
    residual: f64,
    converged: bool,
    time_us: u64,
}

/// JSON-serialisable complexity estimate.
#[derive(Serialize)]
struct JsComplexityEstimate {
    algorithm: String,
    estimated_flops: u64,
    estimated_iterations: usize,
    estimated_memory_bytes: usize,
    complexity_class: String,
    density: f64,
    is_diag_dominant: bool,
    estimated_spectral_radius: f64,
}

// ---------------------------------------------------------------------------
// Internal solver result (before JS conversion)
// ---------------------------------------------------------------------------

struct InternalSolveResult {
    solution: Vec<f32>,
    iterations: usize,
    residual: f64,
    converged: bool,
    algorithm: Algorithm,
}

struct InternalPprResult {
    scores: Vec<f32>,
    iterations: usize,
    residual: f64,
    converged: bool,
}

// ---------------------------------------------------------------------------
// Sparsity analysis
// ---------------------------------------------------------------------------

/// Analyse the sparsity structure of a CSR matrix to inform algorithm
/// selection.
fn analyze_sparsity(csr: &CsrMatrix<f32>) -> SparsityProfile {
    let nnz = csr.values.len();
    let n = csr.rows;
    let total_elements = if n > 0 && csr.cols > 0 {
        n * csr.cols
    } else {
        1
    };
    let density = nnz as f64 / total_elements as f64;

    let mut is_diag_dominant = true;
    let mut max_nnz_per_row: usize = 0;
    let mut est_spectral_sum = 0.0f64;
    let mut symmetric_check = true;

    for row in 0..n {
        let start = csr.row_ptr[row];
        let end = csr.row_ptr[row + 1];
        let row_nnz = end - start;
        if row_nnz > max_nnz_per_row {
            max_nnz_per_row = row_nnz;
        }

        let mut diag_val = 0.0f64;
        let mut off_diag_sum = 0.0f64;

        for idx in start..end {
            let col = csr.col_indices[idx];
            let val = csr.values[idx] as f64;
            if col == row {
                diag_val = val.abs();
            } else {
                off_diag_sum += val.abs();
            }
        }

        if diag_val <= off_diag_sum && diag_val > 0.0 {
            is_diag_dominant = false;
        }
        if diag_val > 0.0 {
            est_spectral_sum += off_diag_sum / diag_val;
        } else if off_diag_sum > 0.0 {
            is_diag_dominant = false;
            est_spectral_sum += 1.0; // pessimistic
        }
    }

    let estimated_spectral_radius = if n > 0 {
        est_spectral_sum / n as f64
    } else {
        0.0
    };

    // Quick structural symmetry check (sample-based for large matrices).
    let check_limit = n.min(64);
    'outer: for row in 0..check_limit {
        let start = csr.row_ptr[row];
        let end = csr.row_ptr[row + 1];
        for idx in start..end {
            let col = csr.col_indices[idx];
            if col >= n || col == row {
                continue;
            }
            // Check if (col, row) entry exists.
            let col_start = csr.row_ptr[col];
            let col_end = csr.row_ptr[col + 1];
            let found = csr.col_indices[col_start..col_end]
                .iter()
                .any(|&c| c == row);
            if !found {
                symmetric_check = false;
                break 'outer;
            }
        }
    }

    let avg_nnz = if n > 0 { nnz as f64 / n as f64 } else { 0.0 };

    // Rough condition estimate from spectral radius.
    let estimated_condition = if estimated_spectral_radius < 1.0 {
        1.0 / (1.0 - estimated_spectral_radius)
    } else {
        estimated_spectral_radius * 100.0 // pessimistic
    };

    SparsityProfile {
        rows: n,
        cols: csr.cols,
        nnz,
        density,
        is_diag_dominant,
        estimated_spectral_radius,
        estimated_condition,
        is_symmetric_structure: symmetric_check,
        avg_nnz_per_row: avg_nnz,
        max_nnz_per_row,
    }
}

// ---------------------------------------------------------------------------
// Algorithm selection
// ---------------------------------------------------------------------------

/// Select the best algorithm given a sparsity profile.
fn select_algorithm(profile: &SparsityProfile) -> Algorithm {
    // Neumann series requires spectral radius < 1.
    if profile.is_diag_dominant && profile.estimated_spectral_radius < 0.95 {
        return Algorithm::Neumann;
    }

    // CG is good for symmetric positive-definite systems.
    if profile.is_symmetric_structure && profile.is_diag_dominant {
        return Algorithm::CG;
    }

    // Default: CG for general sparse systems.
    Algorithm::CG
}

// ---------------------------------------------------------------------------
// Neumann series solver
// ---------------------------------------------------------------------------

/// Neumann series solver for diagonally dominant systems.
///
/// Computes `x = sum_{k=0}^{K} (I - D^{-1} A)^k D^{-1} b` where `D` is the
/// diagonal of `A`. This converges when the spectral radius of `D^{-1}(A - D)`
/// is less than 1.
fn neumann_solve(
    csr: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iterations: usize,
) -> InternalSolveResult {
    let n = csr.rows;

    // Extract diagonal and compute D^{-1} b.
    let mut diag_inv = vec![0.0f32; n];
    for row in 0..n {
        let start = csr.row_ptr[row];
        let end = csr.row_ptr[row + 1];
        for idx in start..end {
            if csr.col_indices[idx] == row {
                let d = csr.values[idx];
                diag_inv[row] = if d.abs() > 1e-30 { 1.0 / d } else { 0.0 };
                break;
            }
        }
    }

    // x = D^{-1} b  (initial approximation: zeroth-order term).
    let mut x: Vec<f32> = rhs
        .iter()
        .zip(diag_inv.iter())
        .map(|(&b, &di)| b * di)
        .collect();

    // Iterate: x_{k+1} = x_k + D^{-1} r_k   where r_k = b - A x_k.
    let mut residual_buf = vec![0.0f32; n];
    let mut converged = false;
    let mut iterations = 0;
    let mut final_residual = f64::MAX;

    for k in 0..max_iterations {
        // Compute r = b - A x.
        spmv(csr, &x, &mut residual_buf);
        for i in 0..n {
            residual_buf[i] = rhs[i] - residual_buf[i];
        }

        // Residual norm.
        let res_norm: f64 = residual_buf
            .iter()
            .map(|&r| (r as f64) * (r as f64))
            .sum::<f64>()
            .sqrt();

        final_residual = res_norm;
        iterations = k + 1;

        if res_norm < tolerance {
            converged = true;
            break;
        }

        // Check for divergence.
        if !res_norm.is_finite() {
            break;
        }

        // Update: x += D^{-1} r.
        for i in 0..n {
            x[i] += diag_inv[i] * residual_buf[i];
        }
    }

    InternalSolveResult {
        solution: x,
        iterations,
        residual: final_residual,
        converged,
        algorithm: Algorithm::Neumann,
    }
}

// ---------------------------------------------------------------------------
// Conjugate Gradient solver
// ---------------------------------------------------------------------------

/// Conjugate Gradient solver for symmetric positive-definite systems.
///
/// Standard CG with residual-based convergence detection.
fn cg_solve(
    csr: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iterations: usize,
) -> InternalSolveResult {
    let n = csr.rows;

    // x_0 = 0, r_0 = b, p_0 = r_0.
    let mut x = vec![0.0f32; n];
    let mut r: Vec<f32> = rhs.to_vec();
    let mut p: Vec<f32> = rhs.to_vec();
    let mut ap = vec![0.0f32; n];

    let mut rr: f64 = r.iter().map(|&v| (v as f64) * (v as f64)).sum();
    let mut converged = false;
    let mut iterations = 0;
    let mut final_residual = rr.sqrt();

    if final_residual < tolerance {
        return InternalSolveResult {
            solution: x,
            iterations: 0,
            residual: final_residual,
            converged: true,
            algorithm: Algorithm::CG,
        };
    }

    for k in 0..max_iterations {
        // ap = A * p.
        spmv(csr, &p, &mut ap);

        // alpha = r^T r / (p^T A p).
        let pap: f64 = p
            .iter()
            .zip(ap.iter())
            .map(|(&pi, &ai)| (pi as f64) * (ai as f64))
            .sum();

        if pap.abs() < 1e-30 {
            // Breakdown: p is in the null space.
            iterations = k + 1;
            break;
        }

        let alpha = rr / pap;
        let alpha_f32 = alpha as f32;

        // x += alpha * p.
        for i in 0..n {
            x[i] += alpha_f32 * p[i];
        }

        // r -= alpha * A p.
        for i in 0..n {
            r[i] -= alpha_f32 * ap[i];
        }

        let rr_new: f64 = r.iter().map(|&v| (v as f64) * (v as f64)).sum();
        final_residual = rr_new.sqrt();
        iterations = k + 1;

        if final_residual < tolerance {
            converged = true;
            break;
        }

        if !rr_new.is_finite() {
            break;
        }

        // beta = r_{k+1}^T r_{k+1} / r_k^T r_k.
        let beta = rr_new / rr;
        let beta_f32 = beta as f32;

        // p = r + beta * p.
        for i in 0..n {
            p[i] = r[i] + beta_f32 * p[i];
        }

        rr = rr_new;
    }

    InternalSolveResult {
        solution: x,
        iterations,
        residual: final_residual,
        converged,
        algorithm: Algorithm::CG,
    }
}

// ---------------------------------------------------------------------------
// Power-iteration PPR
// ---------------------------------------------------------------------------

/// Power iteration for Personalized PageRank.
///
/// Computes `pi = alpha * s + (1 - alpha) * M^T pi` where `s` is the source
/// distribution and `M` is the row-normalised transition matrix.
fn power_iteration_ppr(
    csr: &CsrMatrix<f32>,
    source: usize,
    alpha: f64,
    tolerance: f64,
    max_iterations: usize,
) -> InternalPprResult {
    let n = csr.rows;
    let alpha_f32 = alpha as f32;
    let one_minus_alpha = (1.0 - alpha) as f32;

    // Compute row sums (out-degree) for normalisation.
    let mut row_sums = vec![0.0f32; n];
    for row in 0..n {
        let start = csr.row_ptr[row];
        let end = csr.row_ptr[row + 1];
        let sum: f32 = csr.values[start..end].iter().sum();
        // Dangling nodes get uniform teleport.
        row_sums[row] = if sum > 0.0 { sum } else { 1.0 };
    }

    // pi starts as the source distribution.
    let mut pi = vec![0.0f32; n];
    pi[source] = 1.0;

    let mut new_pi = vec![0.0f32; n];
    let mut converged = false;
    let mut iterations = 0;
    let mut final_residual = f64::MAX;

    for k in 0..max_iterations {
        // new_pi = alpha * e_source + (1-alpha) * M^T * pi
        // where M[i][j] = A[i][j] / row_sum[i].
        new_pi.fill(0.0);

        // Scatter: for each row i, distribute pi[i] to neighbours.
        for row in 0..n {
            if pi[row] == 0.0 {
                continue;
            }
            let start = csr.row_ptr[row];
            let end = csr.row_ptr[row + 1];
            let inv_deg = pi[row] / row_sums[row];

            for idx in start..end {
                let col = csr.col_indices[idx];
                new_pi[col] += one_minus_alpha * csr.values[idx] * inv_deg;
            }
        }

        // Teleportation.
        new_pi[source] += alpha_f32;

        // L1 residual.
        let l1_diff: f64 = pi
            .iter()
            .zip(new_pi.iter())
            .map(|(&a, &b)| ((a - b) as f64).abs())
            .sum();

        std::mem::swap(&mut pi, &mut new_pi);
        final_residual = l1_diff;
        iterations = k + 1;

        if l1_diff < tolerance {
            converged = true;
            break;
        }

        if !l1_diff.is_finite() {
            break;
        }
    }

    InternalPprResult {
        scores: pi,
        iterations,
        residual: final_residual,
        converged,
    }
}

// ---------------------------------------------------------------------------
// Complexity estimation
// ---------------------------------------------------------------------------

/// Build a [`ComplexityEstimate`] based on the sparsity profile and selected
/// algorithm.
fn build_complexity_estimate(
    profile: &SparsityProfile,
    algorithm: Algorithm,
) -> ComplexityEstimate {
    let n = profile.rows;
    let nnz = profile.nnz;

    match algorithm {
        Algorithm::Neumann => {
            // O(nnz * log(1/eps)) iterations; each iteration is O(nnz).
            let est_iters = if profile.estimated_spectral_radius < 1.0 {
                ((1.0 / (1.0 - profile.estimated_spectral_radius)).ln() * 10.0).ceil() as usize
            } else {
                1000
            };
            let est_flops = (nnz as u64) * (est_iters as u64) * 2;

            ComplexityEstimate {
                algorithm,
                estimated_flops: est_flops,
                estimated_iterations: est_iters,
                estimated_memory_bytes: n * 4 * 3, // x, r, diag_inv
                complexity_class: ComplexityClass::SublinearNnz,
            }
        }
        Algorithm::CG => {
            // CG converges in O(sqrt(kappa)) iterations.
            let kappa = profile.estimated_condition.max(1.0);
            let est_iters = (kappa.sqrt() * 2.0).ceil().min(n as f64) as usize;
            let est_flops = (nnz as u64) * (est_iters as u64) * 2;

            ComplexityEstimate {
                algorithm,
                estimated_flops: est_flops,
                estimated_iterations: est_iters,
                estimated_memory_bytes: n * 4 * 4, // x, r, p, Ap
                complexity_class: ComplexityClass::SqrtCondition,
            }
        }
        Algorithm::ForwardPush | Algorithm::BackwardPush => {
            // O(1/epsilon) work, sublinear in graph size.
            let est_iters = 1000;
            ComplexityEstimate {
                algorithm,
                estimated_flops: est_iters as u64 * profile.avg_nnz_per_row.ceil() as u64,
                estimated_iterations: est_iters,
                estimated_memory_bytes: n * 8 * 2, // estimate + residual
                complexity_class: ComplexityClass::SublinearNnz,
            }
        }
        _ => {
            // Conservative fallback.
            ComplexityEstimate {
                algorithm,
                estimated_flops: (nnz as u64) * (n as u64),
                estimated_iterations: n,
                estimated_memory_bytes: n * n * 4,
                complexity_class: ComplexityClass::Quadratic,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Low-level utilities
// ---------------------------------------------------------------------------

/// Sparse matrix-vector product `y = A * x` using the types::CsrMatrix layout.
#[inline]
fn spmv(csr: &CsrMatrix<f32>, x: &[f32], y: &mut [f32]) {
    y.iter_mut().for_each(|v| *v = 0.0);
    for row in 0..csr.rows {
        let start = csr.row_ptr[row];
        let end = csr.row_ptr[row + 1];
        let mut sum = 0.0f32;
        for idx in start..end {
            sum += csr.values[idx] * x[csr.col_indices[idx]];
        }
        y[row] = sum;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a 3x3 diagonally dominant test matrix.
    ///  [[ 4, -1,  0],
    ///   [-1,  4, -1],
    ///   [ 0, -1,  4]]
    fn test_matrix() -> (Vec<f32>, Vec<u32>, Vec<u32>, usize, usize) {
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let col_indices = vec![0, 1, 0, 1, 2, 1, 2];
        let row_ptrs = vec![0, 2, 5, 7];
        (values, col_indices, row_ptrs, 3, 3)
    }

    #[test]
    fn test_analyze_sparsity() {
        let (vals, cols, ptrs, rows, c) = test_matrix();
        let csr = csr_from_js_arrays(&vals, &cols, &ptrs, rows, c).unwrap();
        let profile = analyze_sparsity(&csr);

        assert_eq!(profile.rows, 3);
        assert_eq!(profile.cols, 3);
        assert_eq!(profile.nnz, 7);
        assert!(profile.is_diag_dominant);
        assert!(profile.estimated_spectral_radius < 1.0);
    }

    #[test]
    fn test_select_algorithm_neumann_for_diag_dominant() {
        let (vals, cols, ptrs, rows, c) = test_matrix();
        let csr = csr_from_js_arrays(&vals, &cols, &ptrs, rows, c).unwrap();
        let profile = analyze_sparsity(&csr);
        let algo = select_algorithm(&profile);
        assert_eq!(algo, Algorithm::Neumann);
    }

    #[test]
    fn test_neumann_solve_identity() {
        // Identity matrix: solution should equal rhs.
        let values = vec![1.0f32, 1.0, 1.0];
        let col_indices = vec![0u32, 1, 2];
        let row_ptrs = vec![0u32, 1, 2, 3];
        let csr = csr_from_js_arrays(&values, &col_indices, &row_ptrs, 3, 3).unwrap();
        let rhs = vec![1.0, 2.0, 3.0];

        let result = neumann_solve(&csr, &rhs, 1e-6, 100);
        assert!(result.converged);
        for (i, &v) in result.solution.iter().enumerate() {
            assert!(
                (v - rhs[i]).abs() < 1e-4,
                "solution[{}] = {} != {}",
                i,
                v,
                rhs[i],
            );
        }
    }

    #[test]
    fn test_neumann_solve_tridiagonal() {
        let (vals, cols, ptrs, rows, c) = test_matrix();
        let csr = csr_from_js_arrays(&vals, &cols, &ptrs, rows, c).unwrap();
        let rhs = vec![1.0, 0.0, 1.0];

        let result = neumann_solve(&csr, &rhs, 1e-6, 1000);
        assert!(result.converged, "residual = {}", result.residual);
        assert!(result.iterations < 100);

        // Verify A * x ~ b.
        let mut ax = vec![0.0f32; rows];
        spmv(&csr, &result.solution, &mut ax);
        for i in 0..rows {
            assert!(
                (ax[i] - rhs[i]).abs() < 1e-4,
                "Ax[{}] = {} != {}",
                i,
                ax[i],
                rhs[i],
            );
        }
    }

    #[test]
    fn test_cg_solve_tridiagonal() {
        let (vals, cols, ptrs, rows, c) = test_matrix();
        let csr = csr_from_js_arrays(&vals, &cols, &ptrs, rows, c).unwrap();
        let rhs = vec![1.0, 0.0, 1.0];

        let result = cg_solve(&csr, &rhs, 1e-6, 1000);
        assert!(result.converged, "residual = {}", result.residual);

        let mut ax = vec![0.0f32; rows];
        spmv(&csr, &result.solution, &mut ax);
        for i in 0..rows {
            assert!(
                (ax[i] - rhs[i]).abs() < 1e-4,
                "Ax[{}] = {} != {}",
                i,
                ax[i],
                rhs[i],
            );
        }
    }

    #[test]
    fn test_power_iteration_ppr_convergence() {
        // Simple 3-node chain: 0 -> 1 -> 2 -> 0.
        let values = vec![1.0f32, 1.0, 1.0];
        let col_indices = vec![1u32, 2, 0];
        let row_ptrs = vec![0u32, 1, 2, 3];
        let csr = csr_from_js_arrays(&values, &col_indices, &row_ptrs, 3, 3).unwrap();

        let result = power_iteration_ppr(&csr, 0, 0.15, 1e-6, 1000);
        assert!(result.converged, "residual = {}", result.residual);

        // Source node should have highest PPR score.
        assert!(result.scores[0] > result.scores[1]);
        assert!(result.scores[0] > result.scores[2]);

        // Scores should approximately sum to 1.
        let sum: f32 = result.scores.iter().sum();
        assert!((sum - 1.0).abs() < 0.1, "sum = {}", sum);
    }

    #[test]
    fn test_complexity_estimate() {
        let (vals, cols, ptrs, rows, c) = test_matrix();
        let csr = csr_from_js_arrays(&vals, &cols, &ptrs, rows, c).unwrap();
        let profile = analyze_sparsity(&csr);
        let est = build_complexity_estimate(&profile, Algorithm::Neumann);

        assert_eq!(est.algorithm, Algorithm::Neumann);
        assert!(est.estimated_flops > 0);
        assert!(est.estimated_iterations > 0);
        assert!(est.estimated_memory_bytes > 0);
        assert_eq!(est.complexity_class, ComplexityClass::SublinearNnz);
    }

    #[test]
    fn test_spmv_basic() {
        // [[2, 1], [0, 3]] * [1, 2] = [4, 6]
        let csr = CsrMatrix {
            row_ptr: vec![0, 2, 3],
            col_indices: vec![0, 1, 1],
            values: vec![2.0f32, 1.0, 3.0],
            rows: 2,
            cols: 2,
        };
        let x = [1.0f32, 2.0];
        let mut y = [0.0f32; 2];
        spmv(&csr, &x, &mut y);
        assert_eq!(y, [4.0, 6.0]);
    }

    #[test]
    fn test_version_not_empty() {
        assert!(!version().is_empty());
    }
}
