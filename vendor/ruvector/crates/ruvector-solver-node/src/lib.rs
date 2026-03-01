//! Node.js NAPI bindings for the RuVector sublinear-time solver.
//!
//! Provides high-performance sparse linear system solving, PageRank
//! computation, and complexity estimation for Node.js applications.
//!
//! All heavy computation runs on worker threads via `tokio::task::spawn_blocking`
//! to avoid blocking the Node.js event loop.

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_solver::types::Algorithm;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Configuration types (NAPI objects)
// ---------------------------------------------------------------------------

/// Configuration for solving a sparse linear system Ax = b.
#[napi(object)]
pub struct SolveConfig {
    /// Non-zero values in CSR format.
    pub values: Vec<f64>,
    /// Column indices for each non-zero entry.
    pub col_indices: Vec<u32>,
    /// Row pointers (length = rows + 1).
    pub row_ptrs: Vec<u32>,
    /// Number of rows in the matrix.
    pub rows: u32,
    /// Number of columns in the matrix.
    pub cols: u32,
    /// Right-hand side vector b.
    pub rhs: Vec<f64>,
    /// Convergence tolerance (default: 1e-6).
    pub tolerance: Option<f64>,
    /// Maximum number of iterations (default: 1000).
    pub max_iterations: Option<u32>,
    /// Algorithm to use: "neumann", "jacobi", "gauss-seidel", "conjugate-gradient".
    /// Defaults to "jacobi".
    pub algorithm: Option<String>,
}

/// Result of solving a sparse linear system.
#[napi(object)]
pub struct SolveResult {
    /// Solution vector x.
    pub solution: Vec<f64>,
    /// Number of iterations performed.
    pub iterations: u32,
    /// Final residual norm ||Ax - b||.
    pub residual: f64,
    /// Whether the solver converged within tolerance.
    pub converged: bool,
    /// Algorithm that was used.
    pub algorithm: String,
    /// Wall-clock time in microseconds.
    pub time_us: u32,
}

/// Configuration for PageRank computation.
#[napi(object)]
pub struct PageRankConfig {
    /// Non-zero values in CSR format (edge weights).
    pub values: Vec<f64>,
    /// Column indices for each non-zero entry.
    pub col_indices: Vec<u32>,
    /// Row pointers (length = rows + 1).
    pub row_ptrs: Vec<u32>,
    /// Number of nodes in the graph.
    pub num_nodes: u32,
    /// Damping factor (default: 0.85).
    pub damping: Option<f64>,
    /// Convergence tolerance (default: 1e-6).
    pub tolerance: Option<f64>,
    /// Maximum number of iterations (default: 100).
    pub max_iterations: Option<u32>,
    /// Personalization vector (uniform if omitted).
    pub personalization: Option<Vec<f64>>,
}

/// Result of PageRank computation.
#[napi(object)]
pub struct PageRankResult {
    /// PageRank scores for each node (sums to 1.0).
    pub scores: Vec<f64>,
    /// Number of iterations performed.
    pub iterations: u32,
    /// Final convergence residual.
    pub residual: f64,
    /// Whether the computation converged.
    pub converged: bool,
    /// Wall-clock time in microseconds.
    pub time_us: u32,
}

/// Configuration for complexity estimation.
#[napi(object)]
pub struct ComplexityConfig {
    /// Number of rows in the matrix.
    pub rows: u32,
    /// Number of non-zero entries.
    pub nnz: u32,
    /// Algorithm to estimate for.
    pub algorithm: Option<String>,
}

/// Result of complexity estimation.
#[napi(object)]
pub struct ComplexityResult {
    /// Estimated time complexity class (e.g. "O(n log n)").
    pub complexity_class: String,
    /// Estimated number of floating-point operations.
    pub estimated_flops: f64,
    /// Recommended algorithm for this problem size.
    pub recommended_algorithm: String,
    /// Estimated wall-clock time in microseconds.
    pub estimated_time_us: f64,
    /// Sparsity ratio (nnz / n^2).
    pub sparsity: f64,
}

/// Convergence history entry.
#[napi(object)]
pub struct ConvergenceEntry {
    /// Iteration index (0-based).
    pub iteration: u32,
    /// Residual norm at this iteration.
    pub residual: f64,
}

/// Result of solving with convergence history.
#[napi(object)]
pub struct SolveWithHistoryResult {
    /// Solution vector x.
    pub solution: Vec<f64>,
    /// Number of iterations performed.
    pub iterations: u32,
    /// Final residual norm.
    pub residual: f64,
    /// Whether the solver converged.
    pub converged: bool,
    /// Algorithm used.
    pub algorithm: String,
    /// Wall-clock time in microseconds.
    pub time_us: u32,
    /// Per-iteration convergence history.
    pub convergence_history: Vec<ConvergenceEntry>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse an algorithm name string into the core Algorithm enum.
fn parse_algorithm(name: &str) -> Result<Algorithm> {
    match name.to_lowercase().as_str() {
        "neumann" | "neumann-series" => Ok(Algorithm::Neumann),
        "jacobi" => Ok(Algorithm::Jacobi),
        "gauss-seidel" | "gaussseidel" | "gs" => Ok(Algorithm::GaussSeidel),
        "forward-push" | "forwardpush" => Ok(Algorithm::ForwardPush),
        "backward-push" | "backwardpush" => Ok(Algorithm::BackwardPush),
        "conjugate-gradient" | "cg" => Ok(Algorithm::CG),
        other => Err(Error::new(
            Status::InvalidArg,
            format!(
                "Unknown algorithm '{}'. Expected one of: neumann, jacobi, \
                 gauss-seidel, conjugate-gradient, forward-push, backward-push",
                other
            ),
        )),
    }
}

/// Validate CSR input dimensions for consistency.
fn validate_csr_input(
    values: &[f64],
    col_indices: &[u32],
    row_ptrs: &[u32],
    rows: usize,
    cols: usize,
) -> Result<()> {
    if row_ptrs.len() != rows + 1 {
        return Err(Error::new(
            Status::InvalidArg,
            format!(
                "row_ptrs length {} does not equal rows + 1 = {}",
                row_ptrs.len(),
                rows + 1
            ),
        ));
    }

    if values.len() != col_indices.len() {
        return Err(Error::new(
            Status::InvalidArg,
            format!(
                "values length {} does not match col_indices length {}",
                values.len(),
                col_indices.len()
            ),
        ));
    }

    let expected_nnz = row_ptrs[rows] as usize;
    if values.len() != expected_nnz {
        return Err(Error::new(
            Status::InvalidArg,
            format!(
                "values length {} does not match row_ptrs[rows] = {}",
                values.len(),
                expected_nnz
            ),
        ));
    }

    // Validate monotonicity of row_ptrs.
    for i in 1..row_ptrs.len() {
        if row_ptrs[i] < row_ptrs[i - 1] {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "row_ptrs is not monotonically non-decreasing at position {}",
                    i
                ),
            ));
        }
    }

    // Validate column indices and value finiteness.
    for (idx, (&col, &val)) in col_indices.iter().zip(values.iter()).enumerate() {
        if col as usize >= cols {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "column index {} at position {} is out of bounds (cols={})",
                    col, idx, cols
                ),
            ));
        }
        if !val.is_finite() {
            return Err(Error::new(
                Status::InvalidArg,
                format!("non-finite value {} at position {}", val, idx),
            ));
        }
    }

    Ok(())
}

/// Sparse matrix-vector multiply y = A*x using CSR arrays.
fn spmv_f64(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    x: &[f64],
    y: &mut [f64],
    rows: usize,
) {
    for i in 0..rows {
        let start = row_ptrs[i];
        let end = row_ptrs[i + 1];
        let mut sum = 0.0f64;
        for idx in start..end {
            sum += values[idx] * x[col_indices[idx]];
        }
        y[i] = sum;
    }
}

/// Compute L2 norm of the residual r = b - A*x.
fn residual_norm(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    x: &[f64],
    b: &[f64],
    rows: usize,
) -> f64 {
    let mut norm_sq = 0.0f64;
    for i in 0..rows {
        let start = row_ptrs[i];
        let end = row_ptrs[i + 1];
        let mut ax_i = 0.0f64;
        for idx in start..end {
            ax_i += values[idx] * x[col_indices[idx]];
        }
        let r = b[i] - ax_i;
        norm_sq += r * r;
    }
    norm_sq.sqrt()
}

/// Extract the diagonal entries of a CSR matrix.
///
/// Returns `None` if any diagonal entry is zero (or missing).
fn extract_diagonal(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    rows: usize,
) -> Option<Vec<f64>> {
    let mut diag = vec![0.0f64; rows];
    for i in 0..rows {
        let start = row_ptrs[i];
        let end = row_ptrs[i + 1];
        let mut found = false;
        for idx in start..end {
            if col_indices[idx] == i {
                diag[i] = values[idx];
                found = true;
                break;
            }
        }
        if !found || diag[i].abs() < 1e-15 {
            return None;
        }
    }
    Some(diag)
}

/// Jacobi iterative solver for Ax = b.
///
/// Requires the diagonal of A to be non-zero. Iterates:
///   x_{k+1}[i] = (b[i] - sum_{j!=i} a_{ij} * x_k[j]) / a_{ii}
fn solve_jacobi(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    rhs: &[f64],
    rows: usize,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, usize, f64, bool, Vec<(usize, f64)>) {
    let mut x = vec![0.0f64; rows];
    let mut x_new = vec![0.0f64; rows];
    let mut history = Vec::new();

    let diag = match extract_diagonal(row_ptrs, col_indices, values, rows) {
        Some(d) => d,
        None => {
            let res = residual_norm(row_ptrs, col_indices, values, &x, rhs, rows);
            history.push((0, res));
            return (x, 0, res, false, history);
        }
    };

    let mut converged = false;
    let mut final_residual = f64::MAX;
    let mut iters = 0;

    for iter in 0..max_iterations {
        for i in 0..rows {
            let start = row_ptrs[i];
            let end = row_ptrs[i + 1];
            let mut sigma = 0.0f64;
            for idx in start..end {
                let j = col_indices[idx];
                if j != i {
                    sigma += values[idx] * x[j];
                }
            }
            x_new[i] = (rhs[i] - sigma) / diag[i];
        }

        std::mem::swap(&mut x, &mut x_new);

        let res = residual_norm(row_ptrs, col_indices, values, &x, rhs, rows);
        history.push((iter, res));
        final_residual = res;
        iters = iter + 1;

        if res < tolerance {
            converged = true;
            break;
        }
    }

    (x, iters, final_residual, converged, history)
}

/// Gauss-Seidel iterative solver for Ax = b.
///
/// Updates x in-place within each iteration using the most recent values.
/// Generally converges faster than Jacobi for the same problem.
fn solve_gauss_seidel(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    rhs: &[f64],
    rows: usize,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, usize, f64, bool, Vec<(usize, f64)>) {
    let mut x = vec![0.0f64; rows];
    let mut history = Vec::new();

    let diag = match extract_diagonal(row_ptrs, col_indices, values, rows) {
        Some(d) => d,
        None => {
            let res = residual_norm(row_ptrs, col_indices, values, &x, rhs, rows);
            history.push((0, res));
            return (x, 0, res, false, history);
        }
    };

    let mut converged = false;
    let mut final_residual = f64::MAX;
    let mut iters = 0;

    for iter in 0..max_iterations {
        for i in 0..rows {
            let start = row_ptrs[i];
            let end = row_ptrs[i + 1];
            let mut sigma = 0.0f64;
            for idx in start..end {
                let j = col_indices[idx];
                if j != i {
                    sigma += values[idx] * x[j];
                }
            }
            x[i] = (rhs[i] - sigma) / diag[i];
        }

        let res = residual_norm(row_ptrs, col_indices, values, &x, rhs, rows);
        history.push((iter, res));
        final_residual = res;
        iters = iter + 1;

        if res < tolerance {
            converged = true;
            break;
        }
    }

    (x, iters, final_residual, converged, history)
}

/// Neumann series solver: x = sum_{k=0}^{K} (I - A)^k * b.
///
/// Converges when the spectral radius of (I - A) is less than 1.
fn solve_neumann(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    rhs: &[f64],
    rows: usize,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, usize, f64, bool, Vec<(usize, f64)>) {
    let mut x = vec![0.0f64; rows];
    let mut term = rhs.to_vec();
    let mut temp = vec![0.0f64; rows];
    let mut history = Vec::new();

    let mut converged = false;
    let mut final_residual = f64::MAX;
    let mut iters = 0;

    for iter in 0..max_iterations {
        // Accumulate current term into x.
        for i in 0..rows {
            x[i] += term[i];
        }

        // Compute next term: term_{k+1} = (I - A) * term_k
        spmv_f64(row_ptrs, col_indices, values, &term, &mut temp, rows);
        for i in 0..rows {
            temp[i] = term[i] - temp[i];
        }
        std::mem::swap(&mut term, &mut temp);

        let term_norm: f64 = term.iter().map(|&t| t * t).sum::<f64>().sqrt();
        let res = residual_norm(row_ptrs, col_indices, values, &x, rhs, rows);
        history.push((iter, res));
        final_residual = res;
        iters = iter + 1;

        if res < tolerance || term_norm < tolerance * 1e-2 {
            converged = true;
            break;
        }

        // Divergence detection.
        if !term_norm.is_finite() {
            break;
        }
    }

    (x, iters, final_residual, converged, history)
}

/// Conjugate gradient solver for symmetric positive-definite Ax = b.
fn solve_cg(
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    rhs: &[f64],
    rows: usize,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, usize, f64, bool, Vec<(usize, f64)>) {
    let mut x = vec![0.0f64; rows];
    let mut history = Vec::new();

    // r = b - A*x (initially r = b since x = 0).
    let mut r = rhs.to_vec();
    let mut p = r.clone();
    let mut ap = vec![0.0f64; rows];

    let mut rs_old: f64 = r.iter().map(|&v| v * v).sum();
    let tol_sq = tolerance * tolerance;

    let mut converged = false;
    let mut final_residual = rs_old.sqrt();
    let mut iters = 0;

    for iter in 0..max_iterations {
        spmv_f64(row_ptrs, col_indices, values, &p, &mut ap, rows);

        let p_ap: f64 = p.iter().zip(ap.iter()).map(|(&a, &b)| a * b).sum();
        if p_ap.abs() < 1e-30 {
            break;
        }
        let alpha = rs_old / p_ap;

        for i in 0..rows {
            x[i] += alpha * p[i];
        }

        for i in 0..rows {
            r[i] -= alpha * ap[i];
        }

        let rs_new: f64 = r.iter().map(|&v| v * v).sum();
        final_residual = rs_new.sqrt();
        history.push((iter, final_residual));
        iters = iter + 1;

        if rs_new < tol_sq {
            converged = true;
            break;
        }

        let beta = rs_new / rs_old;
        for i in 0..rows {
            p[i] = r[i] + beta * p[i];
        }

        rs_old = rs_new;
    }

    (x, iters, final_residual, converged, history)
}

/// Dispatch to the appropriate solver based on algorithm selection.
fn dispatch_solver(
    algo: Algorithm,
    row_ptrs: &[usize],
    col_indices: &[usize],
    values: &[f64],
    rhs: &[f64],
    rows: usize,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, usize, f64, bool, Vec<(usize, f64)>) {
    match algo {
        Algorithm::Jacobi => solve_jacobi(
            row_ptrs,
            col_indices,
            values,
            rhs,
            rows,
            tolerance,
            max_iterations,
        ),
        Algorithm::GaussSeidel => solve_gauss_seidel(
            row_ptrs,
            col_indices,
            values,
            rhs,
            rows,
            tolerance,
            max_iterations,
        ),
        Algorithm::Neumann => solve_neumann(
            row_ptrs,
            col_indices,
            values,
            rhs,
            rows,
            tolerance,
            max_iterations,
        ),
        Algorithm::CG => solve_cg(
            row_ptrs,
            col_indices,
            values,
            rhs,
            rows,
            tolerance,
            max_iterations,
        ),
        // Forward/backward push are graph algorithms, not general linear solvers.
        // Fall back to Jacobi.
        _ => solve_jacobi(
            row_ptrs,
            col_indices,
            values,
            rhs,
            rows,
            tolerance,
            max_iterations,
        ),
    }
}

// ---------------------------------------------------------------------------
// NapiSolver
// ---------------------------------------------------------------------------

/// High-performance sparse linear solver with automatic algorithm selection.
///
/// Provides async methods for solving Ax = b, computing PageRank, and
/// estimating computational complexity. All heavy computation runs on
/// worker threads.
///
/// # Example
/// ```javascript
/// const { NapiSolver } = require('@ruvector/solver');
///
/// const solver = new NapiSolver();
/// const result = await solver.solve({
///   values: [4, -1, -1, 4, -1, -1, 4],
///   colIndices: [0, 1, 0, 1, 2, 1, 2],
///   rowPtrs: [0, 2, 5, 7],
///   rows: 3, cols: 3,
///   rhs: [1, 0, 1],
/// });
/// console.log('Solution:', result.solution);
/// console.log('Converged:', result.converged);
/// ```
#[napi]
pub struct NapiSolver {
    default_tolerance: f64,
    default_max_iterations: usize,
}

#[napi]
impl NapiSolver {
    /// Create a new solver instance with default settings.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            default_tolerance: 1e-6,
            default_max_iterations: 1000,
        }
    }

    /// Solve a sparse linear system Ax = b asynchronously.
    ///
    /// Runs the computation on a worker thread to avoid blocking the
    /// Node.js event loop.
    ///
    /// # Arguments
    /// * `config` - Solver configuration including the CSR matrix, RHS vector,
    ///   tolerance, max iterations, and algorithm selection.
    ///
    /// # Returns
    /// A `SolveResult` containing the solution vector, convergence info,
    /// and timing data.
    ///
    /// # Example
    /// ```javascript
    /// const result = await solver.solve({
    ///   values: [2, -1, -1, 2, -1, -1, 2],
    ///   colIndices: [0, 1, 0, 1, 2, 1, 2],
    ///   rowPtrs: [0, 2, 5, 7],
    ///   rows: 3, cols: 3,
    ///   rhs: [1, 0, 1],
    ///   tolerance: 1e-8,
    ///   algorithm: 'jacobi',
    /// });
    /// ```
    #[napi]
    pub async fn solve(&self, config: SolveConfig) -> Result<SolveResult> {
        let tolerance = config.tolerance.unwrap_or(self.default_tolerance);
        let max_iterations = config
            .max_iterations
            .map(|m| m as usize)
            .unwrap_or(self.default_max_iterations);
        let algo = parse_algorithm(config.algorithm.as_deref().unwrap_or("jacobi"))?;
        let algo_name = algo.to_string();

        let rows = config.rows as usize;
        let cols = config.cols as usize;
        validate_csr_input(
            &config.values,
            &config.col_indices,
            &config.row_ptrs,
            rows,
            cols,
        )?;

        if config.rhs.len() != rows {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "rhs length {} does not match rows = {}",
                    config.rhs.len(),
                    rows
                ),
            ));
        }

        let values = config.values;
        let col_indices: Vec<usize> = config.col_indices.iter().map(|&c| c as usize).collect();
        let row_ptrs: Vec<usize> = config.row_ptrs.iter().map(|&p| p as usize).collect();
        let rhs = config.rhs;

        let result = tokio::task::spawn_blocking(move || {
            let start = Instant::now();

            let (solution, iterations, residual, converged, _history) = dispatch_solver(
                algo,
                &row_ptrs,
                &col_indices,
                &values,
                &rhs,
                rows,
                tolerance,
                max_iterations,
            );

            let elapsed_us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;

            SolveResult {
                solution,
                iterations: iterations as u32,
                residual,
                converged,
                algorithm: algo_name,
                time_us: elapsed_us,
            }
        })
        .await
        .map_err(|e| Error::from_reason(format!("Solver task failed: {}", e)))?;

        Ok(result)
    }

    /// Solve a sparse linear system from JSON input.
    ///
    /// Accepts a JSON string with the same fields as `SolveConfig` and
    /// returns a JSON string with the `SolveResult` fields.
    ///
    /// # Example
    /// ```javascript
    /// const input = JSON.stringify({
    ///   values: [2, -1, -1, 2],
    ///   col_indices: [0, 1, 0, 1],
    ///   row_ptrs: [0, 2, 4],
    ///   rows: 2, cols: 2,
    ///   rhs: [1, 1],
    /// });
    /// const output = await solver.solveJson(input);
    /// const result = JSON.parse(output);
    /// ```
    #[napi]
    pub async fn solve_json(&self, json: String) -> Result<String> {
        let input: SolveJsonInput = serde_json::from_str(&json)
            .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid JSON input: {}", e)))?;

        let config = SolveConfig {
            values: input.values,
            col_indices: input.col_indices,
            row_ptrs: input.row_ptrs,
            rows: input.rows,
            cols: input.cols,
            rhs: input.rhs,
            tolerance: input.tolerance,
            max_iterations: input.max_iterations,
            algorithm: input.algorithm,
        };

        let result = self.solve(config).await?;

        let output = SolveJsonOutput {
            solution: result.solution,
            iterations: result.iterations,
            residual: result.residual,
            converged: result.converged,
            algorithm: result.algorithm,
            time_us: result.time_us,
        };

        serde_json::to_string(&output).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Compute PageRank scores for a directed graph asynchronously.
    ///
    /// Implements the power iteration method:
    ///   r_{k+1} = d * A^T * D^{-1} * r_k + (1 - d) * p
    /// where d is the damping factor, D is the out-degree diagonal, and p
    /// is the personalization vector.
    ///
    /// # Example
    /// ```javascript
    /// // Simple 3-node graph: 0->1, 1->2, 2->0
    /// const result = await solver.pagerank({
    ///   values: [1, 1, 1],
    ///   colIndices: [1, 2, 0],
    ///   rowPtrs: [0, 1, 2, 3],
    ///   numNodes: 3,
    ///   damping: 0.85,
    /// });
    /// console.log('PageRank:', result.scores);
    /// ```
    #[napi]
    pub async fn pagerank(&self, config: PageRankConfig) -> Result<PageRankResult> {
        let damping = config.damping.unwrap_or(0.85);
        let tolerance = config.tolerance.unwrap_or(1e-6);
        let max_iterations = config.max_iterations.map(|m| m as usize).unwrap_or(100);
        let num_nodes = config.num_nodes as usize;

        if damping < 0.0 || damping > 1.0 {
            return Err(Error::new(
                Status::InvalidArg,
                format!("Damping factor must be in [0, 1], got {}", damping),
            ));
        }

        validate_csr_input(
            &config.values,
            &config.col_indices,
            &config.row_ptrs,
            num_nodes,
            num_nodes,
        )?;

        let values: Vec<f64> = config.values;
        let col_indices: Vec<usize> = config.col_indices.iter().map(|&c| c as usize).collect();
        let row_ptrs: Vec<usize> = config.row_ptrs.iter().map(|&p| p as usize).collect();
        let personalization = config.personalization;

        if let Some(ref pv) = personalization {
            if pv.len() != num_nodes {
                return Err(Error::new(
                    Status::InvalidArg,
                    format!(
                        "personalization length {} does not match num_nodes = {}",
                        pv.len(),
                        num_nodes
                    ),
                ));
            }
        }

        let result = tokio::task::spawn_blocking(move || {
            let start = Instant::now();

            let p = personalization.unwrap_or_else(|| vec![1.0 / num_nodes as f64; num_nodes]);

            // Compute out-degrees for row-stochastic normalization.
            let mut out_degree = vec![0.0f64; num_nodes];
            for i in 0..num_nodes {
                for idx in row_ptrs[i]..row_ptrs[i + 1] {
                    out_degree[i] += values[idx];
                }
            }

            let mut rank = vec![1.0 / num_nodes as f64; num_nodes];
            let mut new_rank = vec![0.0f64; num_nodes];

            let mut converged = false;
            let mut final_residual = f64::MAX;
            let mut iters = 0;

            for iter in 0..max_iterations {
                for i in 0..num_nodes {
                    new_rank[i] = (1.0 - damping) * p[i];
                }

                let mut dangling_sum = 0.0f64;
                for i in 0..num_nodes {
                    let s = row_ptrs[i];
                    let e = row_ptrs[i + 1];
                    if s == e || out_degree[i].abs() < 1e-15 {
                        dangling_sum += rank[i];
                    } else {
                        let contribution = rank[i] / out_degree[i];
                        for idx in s..e {
                            new_rank[col_indices[idx]] += damping * values[idx] * contribution;
                        }
                    }
                }

                if dangling_sum > 0.0 {
                    let dangling_contrib = damping * dangling_sum / num_nodes as f64;
                    for i in 0..num_nodes {
                        new_rank[i] += dangling_contrib;
                    }
                }

                let mut diff = 0.0f64;
                for i in 0..num_nodes {
                    diff += (new_rank[i] - rank[i]).abs();
                }

                std::mem::swap(&mut rank, &mut new_rank);
                final_residual = diff;
                iters = iter + 1;

                if diff < tolerance {
                    converged = true;
                    break;
                }
            }

            let elapsed_us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;

            PageRankResult {
                scores: rank,
                iterations: iters as u32,
                residual: final_residual,
                converged,
                time_us: elapsed_us,
            }
        })
        .await
        .map_err(|e| Error::from_reason(format!("PageRank task failed: {}", e)))?;

        Ok(result)
    }

    /// Estimate computational complexity for a given problem size.
    ///
    /// This is a synchronous method since the estimation is O(1).
    ///
    /// # Example
    /// ```javascript
    /// const estimate = solver.estimateComplexity({
    ///   rows: 10000,
    ///   nnz: 50000,
    ///   algorithm: 'jacobi',
    /// });
    /// console.log('Complexity:', estimate.complexityClass);
    /// console.log('Recommended:', estimate.recommendedAlgorithm);
    /// ```
    #[napi]
    pub fn estimate_complexity(&self, config: ComplexityConfig) -> Result<ComplexityResult> {
        let n = config.rows as f64;
        let nnz = config.nnz as f64;
        let sparsity = if n * n > 0.0 { nnz / (n * n) } else { 0.0 };

        let algo_name = config.algorithm.as_deref().unwrap_or("auto");

        let recommended = if n < 100.0 {
            "gauss-seidel"
        } else if sparsity < 0.01 && n > 10000.0 {
            "conjugate-gradient"
        } else if sparsity < 0.05 {
            "neumann"
        } else {
            "jacobi"
        };

        let (complexity_class, estimated_flops) = match algo_name {
            "neumann" | "neumann-series" => {
                let k = 50.0;
                ("O(k * nnz)".to_string(), k * nnz)
            }
            "jacobi" => {
                let k = n.sqrt().max(10.0);
                ("O(sqrt(n) * nnz)".to_string(), k * nnz)
            }
            "gauss-seidel" | "gs" => {
                let k = (n.sqrt() / 2.0).max(5.0);
                ("O(sqrt(n) * nnz)".to_string(), k * nnz)
            }
            "conjugate-gradient" | "cg" => {
                let cond_est = n.sqrt();
                ("O(sqrt(kappa) * nnz)".to_string(), cond_est * nnz)
            }
            _ => {
                let k = n.sqrt().max(10.0);
                ("O(sqrt(n) * nnz)".to_string(), k * nnz)
            }
        };

        let estimated_time_us = estimated_flops / 1000.0;

        Ok(ComplexityResult {
            complexity_class,
            estimated_flops,
            recommended_algorithm: recommended.to_string(),
            estimated_time_us,
            sparsity,
        })
    }

    /// Solve with full convergence history returned.
    ///
    /// Identical to `solve` but also returns per-iteration residual data
    /// for convergence analysis and visualization.
    ///
    /// # Example
    /// ```javascript
    /// const result = await solver.solveWithHistory({
    ///   values: [4, -1, -1, 4],
    ///   colIndices: [0, 1, 0, 1],
    ///   rowPtrs: [0, 2, 4],
    ///   rows: 2, cols: 2,
    ///   rhs: [1, 1],
    /// });
    /// result.convergenceHistory.forEach(entry => {
    ///   console.log(`Iter ${entry.iteration}: residual = ${entry.residual}`);
    /// });
    /// ```
    #[napi]
    pub async fn solve_with_history(&self, config: SolveConfig) -> Result<SolveWithHistoryResult> {
        let tolerance = config.tolerance.unwrap_or(self.default_tolerance);
        let max_iterations = config
            .max_iterations
            .map(|m| m as usize)
            .unwrap_or(self.default_max_iterations);
        let algo = parse_algorithm(config.algorithm.as_deref().unwrap_or("jacobi"))?;
        let algo_name = algo.to_string();

        let rows = config.rows as usize;
        let cols = config.cols as usize;
        validate_csr_input(
            &config.values,
            &config.col_indices,
            &config.row_ptrs,
            rows,
            cols,
        )?;

        if config.rhs.len() != rows {
            return Err(Error::new(
                Status::InvalidArg,
                format!(
                    "rhs length {} does not match rows = {}",
                    config.rhs.len(),
                    rows
                ),
            ));
        }

        let values = config.values;
        let col_indices: Vec<usize> = config.col_indices.iter().map(|&c| c as usize).collect();
        let row_ptrs: Vec<usize> = config.row_ptrs.iter().map(|&p| p as usize).collect();
        let rhs = config.rhs;

        let result = tokio::task::spawn_blocking(move || {
            let start = Instant::now();

            let (solution, iterations, residual, converged, history) = dispatch_solver(
                algo,
                &row_ptrs,
                &col_indices,
                &values,
                &rhs,
                rows,
                tolerance,
                max_iterations,
            );

            let elapsed_us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;

            let convergence_history: Vec<ConvergenceEntry> = history
                .into_iter()
                .map(|(iter, res)| ConvergenceEntry {
                    iteration: iter as u32,
                    residual: res,
                })
                .collect();

            SolveWithHistoryResult {
                solution,
                iterations: iterations as u32,
                residual,
                converged,
                algorithm: algo_name,
                time_us: elapsed_us,
                convergence_history,
            }
        })
        .await
        .map_err(|e| Error::from_reason(format!("Solver task failed: {}", e)))?;

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Serde types for JSON solve interface
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct SolveJsonInput {
    values: Vec<f64>,
    col_indices: Vec<u32>,
    row_ptrs: Vec<u32>,
    rows: u32,
    cols: u32,
    rhs: Vec<f64>,
    tolerance: Option<f64>,
    max_iterations: Option<u32>,
    algorithm: Option<String>,
}

#[derive(serde::Serialize)]
struct SolveJsonOutput {
    solution: Vec<f64>,
    iterations: u32,
    residual: f64,
    converged: bool,
    algorithm: String,
    time_us: u32,
}

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Get the library version.
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get library information.
#[napi]
pub fn info() -> LibraryInfo {
    LibraryInfo {
        name: "ruvector-solver-node".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "Sublinear-time sparse linear solver for Node.js".to_string(),
        algorithms: vec![
            "neumann".to_string(),
            "jacobi".to_string(),
            "gauss-seidel".to_string(),
            "conjugate-gradient".to_string(),
        ],
        features: vec![
            "async-solve".to_string(),
            "json-interface".to_string(),
            "pagerank".to_string(),
            "complexity-estimation".to_string(),
            "convergence-history".to_string(),
        ],
    }
}

/// Library information.
#[napi(object)]
pub struct LibraryInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub algorithms: Vec<String>,
    pub features: Vec<String>,
}

/// List available solver algorithms.
#[napi]
pub fn available_algorithms() -> Vec<String> {
    vec![
        "neumann".to_string(),
        "jacobi".to_string(),
        "gauss-seidel".to_string(),
        "conjugate-gradient".to_string(),
    ]
}
