//! Algorithm router and solver orchestrator.
//!
//! The [`SolverRouter`] inspects a matrix's [`SparsityProfile`] and the
//! caller's [`QueryType`] to select the optimal [`Algorithm`] for each solve
//! request. The [`SolverOrchestrator`] wraps the router together with concrete
//! solver instances and provides high-level `solve` / `solve_with_fallback`
//! entry points.
//!
//! # Routing decision tree
//!
//! | Query | Condition | Algorithm |
//! |-------|-----------|-----------|
//! | `LinearSystem` | diag-dominant + very sparse | Neumann |
//! | `LinearSystem` | low condition number | CG |
//! | `LinearSystem` | else | BMSSP |
//! | `PageRankSingle` | always | ForwardPush |
//! | `PageRankPairwise` | large graph | HybridRandomWalk |
//! | `PageRankPairwise` | small graph | ForwardPush |
//! | `SpectralFilter` | always | Neumann |
//! | `BatchLinearSystem` | large batch | TRUE |
//! | `BatchLinearSystem` | small batch | CG |
//!
//! # Fallback chain
//!
//! When the selected algorithm fails (non-convergence, numerical instability),
//! [`SolverOrchestrator::solve_with_fallback`] tries a deterministic chain:
//!
//! **selected algorithm -> CG -> Dense**

use std::time::Instant;

use tracing::{debug, info, warn};

use crate::error::SolverError;
use crate::traits::SolverEngine;
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, ConvergenceInfo, CsrMatrix,
    QueryType, SolverResult, SparsityProfile,
};

// ---------------------------------------------------------------------------
// RouterConfig
// ---------------------------------------------------------------------------

/// Configuration thresholds that govern the routing decision tree.
///
/// All thresholds have sensible defaults; override them when benchmarks on
/// your workload indicate a different crossover point.
///
/// # Example
///
/// ```rust
/// use ruvector_solver::router::RouterConfig;
///
/// let config = RouterConfig {
///     cg_condition_threshold: 50.0,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Maximum spectral radius for which the Neumann series is attempted.
    ///
    /// If the estimated spectral radius exceeds this value the router will
    /// not select Neumann even for diagonally dominant matrices.
    ///
    /// Default: `0.95`.
    pub neumann_spectral_radius_threshold: f64,

    /// Maximum condition number for which CG is preferred over BMSSP.
    ///
    /// CG converges in O(sqrt(kappa)) iterations; when kappa is too large
    /// a preconditioned method (BMSSP) is cheaper.
    ///
    /// Default: `100.0`.
    pub cg_condition_threshold: f64,

    /// Maximum density (fraction of non-zeros) for the Neumann sublinear
    /// fast-path.
    ///
    /// Neumann is only worthwhile when the matrix is truly sparse.
    ///
    /// Default: `0.05` (5%).
    pub sparsity_sublinear_threshold: f64,

    /// Minimum batch size for which the TRUE solver is preferred over CG
    /// in `BatchLinearSystem` queries.
    ///
    /// Default: `100`.
    pub true_batch_threshold: usize,

    /// Graph size threshold (number of rows) above which
    /// `PageRankPairwise` switches from ForwardPush to HybridRandomWalk.
    ///
    /// Default: `1_000`.
    pub push_graph_size_threshold: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            neumann_spectral_radius_threshold: 0.95,
            cg_condition_threshold: 100.0,
            sparsity_sublinear_threshold: 0.05,
            true_batch_threshold: 100,
            push_graph_size_threshold: 1_000,
        }
    }
}

// ---------------------------------------------------------------------------
// SolverRouter
// ---------------------------------------------------------------------------

/// Stateless algorithm selector.
///
/// Given a [`SparsityProfile`] and a [`QueryType`], the router walks a
/// decision tree (documented in the [module-level docs](self)) to pick the
/// [`Algorithm`] with the best expected cost.
///
/// # Example
///
/// ```rust
/// use ruvector_solver::router::{SolverRouter, RouterConfig};
/// use ruvector_solver::types::{Algorithm, QueryType, SparsityProfile};
///
/// let router = SolverRouter::new(RouterConfig::default());
/// let profile = SparsityProfile {
///     rows: 500,
///     cols: 500,
///     nnz: 1200,
///     density: 0.0048,
///     is_diag_dominant: true,
///     estimated_spectral_radius: 0.4,
///     estimated_condition: 10.0,
///     is_symmetric_structure: true,
///     avg_nnz_per_row: 2.4,
///     max_nnz_per_row: 5,
/// };
///
/// let algo = router.select_algorithm(&profile, &QueryType::LinearSystem);
/// assert_eq!(algo, Algorithm::Neumann);
/// ```
#[derive(Debug, Clone)]
pub struct SolverRouter {
    config: RouterConfig,
}

impl SolverRouter {
    /// Create a new router with the provided configuration.
    pub fn new(config: RouterConfig) -> Self {
        Self { config }
    }

    /// Return a shared reference to the active configuration.
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Select the optimal algorithm for the given matrix profile and query.
    ///
    /// This is a pure function with no side effects -- it does not touch the
    /// matrix data, only the precomputed profile.
    pub fn select_algorithm(&self, profile: &SparsityProfile, query: &QueryType) -> Algorithm {
        match query {
            // ----------------------------------------------------------
            // Linear system: Neumann > CG > BMSSP
            // ----------------------------------------------------------
            QueryType::LinearSystem => self.route_linear_system(profile),

            // ----------------------------------------------------------
            // Single-source PageRank: always ForwardPush
            // ----------------------------------------------------------
            QueryType::PageRankSingle { .. } => {
                debug!("routing to ForwardPush (single-source PageRank)");
                Algorithm::ForwardPush
            }

            // ----------------------------------------------------------
            // Pairwise PageRank: ForwardPush or HybridRandomWalk
            // ----------------------------------------------------------
            QueryType::PageRankPairwise { .. } => {
                if profile.rows > self.config.push_graph_size_threshold {
                    debug!(
                        rows = profile.rows,
                        threshold = self.config.push_graph_size_threshold,
                        "routing to HybridRandomWalk (large graph pairwise PPR)"
                    );
                    Algorithm::HybridRandomWalk
                } else {
                    debug!(
                        rows = profile.rows,
                        "routing to ForwardPush (small graph pairwise PPR)"
                    );
                    Algorithm::ForwardPush
                }
            }

            // ----------------------------------------------------------
            // Spectral filter: always Neumann
            // ----------------------------------------------------------
            QueryType::SpectralFilter { .. } => {
                debug!("routing to Neumann (spectral filter)");
                Algorithm::Neumann
            }

            // ----------------------------------------------------------
            // Batch linear system: TRUE or CG
            // ----------------------------------------------------------
            QueryType::BatchLinearSystem { batch_size } => {
                if *batch_size > self.config.true_batch_threshold {
                    debug!(
                        batch_size,
                        threshold = self.config.true_batch_threshold,
                        "routing to TRUE (large batch)"
                    );
                    Algorithm::TRUE
                } else {
                    debug!(batch_size, "routing to CG (small batch)");
                    Algorithm::CG
                }
            }
        }
    }

    /// Internal routing logic for `LinearSystem` queries.
    fn route_linear_system(&self, profile: &SparsityProfile) -> Algorithm {
        if profile.is_diag_dominant
            && profile.density < self.config.sparsity_sublinear_threshold
            && profile.estimated_spectral_radius < self.config.neumann_spectral_radius_threshold
        {
            debug!(
                density = profile.density,
                spectral_radius = profile.estimated_spectral_radius,
                "routing to Neumann (diag-dominant, sparse, low spectral radius)"
            );
            Algorithm::Neumann
        } else if profile.estimated_condition < self.config.cg_condition_threshold {
            debug!(
                condition = profile.estimated_condition,
                "routing to CG (well-conditioned)"
            );
            Algorithm::CG
        } else {
            debug!(
                condition = profile.estimated_condition,
                "routing to BMSSP (ill-conditioned)"
            );
            Algorithm::BMSSP
        }
    }
}

impl Default for SolverRouter {
    fn default() -> Self {
        Self::new(RouterConfig::default())
    }
}

// ---------------------------------------------------------------------------
// SolverOrchestrator
// ---------------------------------------------------------------------------

/// High-level solver facade that combines routing with execution.
///
/// Owns a [`SolverRouter`] and delegates to the appropriate solver backend.
/// Provides a [`solve_with_fallback`](Self::solve_with_fallback) method that
/// automatically retries with progressively more robust (but slower)
/// algorithms when the first choice fails.
///
/// # Example
///
/// ```rust
/// use ruvector_solver::router::{SolverOrchestrator, RouterConfig};
/// use ruvector_solver::types::{ComputeBudget, CsrMatrix, QueryType};
///
/// let orchestrator = SolverOrchestrator::new(RouterConfig::default());
///
/// let matrix = CsrMatrix::<f64>::from_coo(3, 3, vec![
///     (0, 0, 2.0), (0, 1, -0.5),
///     (1, 0, -0.5), (1, 1, 2.0), (1, 2, -0.5),
///     (2, 1, -0.5), (2, 2, 2.0),
/// ]);
/// let rhs = vec![1.0, 0.0, 1.0];
/// let budget = ComputeBudget::default();
///
/// let result = orchestrator
///     .solve(&matrix, &rhs, QueryType::LinearSystem, &budget)
///     .unwrap();
/// assert!(result.residual_norm < 1e-6);
/// ```
#[derive(Debug, Clone)]
pub struct SolverOrchestrator {
    router: SolverRouter,
}

impl SolverOrchestrator {
    /// Create a new orchestrator with the provided routing configuration.
    pub fn new(config: RouterConfig) -> Self {
        Self {
            router: SolverRouter::new(config),
        }
    }

    /// Return a reference to the inner router.
    pub fn router(&self) -> &SolverRouter {
        &self.router
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Auto-select the best algorithm and solve `Ax = b`.
    ///
    /// Analyses the sparsity profile of `matrix`, routes to the best
    /// algorithm via [`SolverRouter::select_algorithm`], and dispatches.
    ///
    /// # Errors
    ///
    /// Returns [`SolverError`] if the selected solver fails (e.g.
    /// non-convergence, dimension mismatch, numerical instability).
    pub fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        query: QueryType,
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let profile = Self::analyze_sparsity(matrix);
        let algorithm = self.router.select_algorithm(&profile, &query);

        info!(%algorithm, rows = matrix.rows, nnz = matrix.nnz(), "solve: selected algorithm");

        self.dispatch(algorithm, matrix, rhs, budget)
    }

    /// Solve with a deterministic fallback chain.
    ///
    /// Tries the routed algorithm first. On failure, falls back through:
    ///
    /// 1. **Selected algorithm** (from routing)
    /// 2. **CG** (robust iterative)
    /// 3. **Dense** (direct, always works for small systems)
    ///
    /// Each step is only attempted if the previous one returned an error.
    ///
    /// # Errors
    ///
    /// Returns the error from the *last* fallback attempt if all fail.
    pub fn solve_with_fallback(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        query: QueryType,
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let profile = Self::analyze_sparsity(matrix);
        let primary = self.router.select_algorithm(&profile, &query);

        let chain = Self::build_fallback_chain(primary);

        info!(
            ?chain,
            rows = matrix.rows,
            nnz = matrix.nnz(),
            "solve_with_fallback: attempting chain"
        );

        let mut last_err: Option<SolverError> = None;

        for (idx, &algorithm) in chain.iter().enumerate() {
            match self.dispatch(algorithm, matrix, rhs, budget) {
                Ok(result) => {
                    if idx > 0 {
                        info!(
                            %algorithm,
                            "fallback succeeded on attempt {}",
                            idx + 1
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    warn!(
                        %algorithm,
                        error = %e,
                        "algorithm failed, trying next in fallback chain"
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err
            .unwrap_or_else(|| SolverError::BackendError("fallback chain was empty".into())))
    }

    /// Estimate the computational complexity of solving with the routed
    /// algorithm, without actually solving.
    ///
    /// Useful for admission control, cost estimation, or deciding whether
    /// to batch multiple queries.
    pub fn estimate_complexity(
        &self,
        matrix: &CsrMatrix<f64>,
        query: &QueryType,
    ) -> ComplexityEstimate {
        let profile = Self::analyze_sparsity(matrix);
        let algorithm = self.router.select_algorithm(&profile, query);
        let n = profile.rows;

        let (estimated_iterations, complexity_class) = match algorithm {
            Algorithm::Neumann => {
                let k = if profile.estimated_spectral_radius > 0.0
                    && profile.estimated_spectral_radius < 1.0
                {
                    let log_inv_eps = (1.0 / 1e-8_f64).ln();
                    let log_inv_rho = (1.0 / profile.estimated_spectral_radius).ln();
                    (log_inv_eps / log_inv_rho).ceil() as usize
                } else {
                    1000
                };
                (k.min(1000), ComplexityClass::SublinearNnz)
            }
            Algorithm::CG => {
                let iters = (profile.estimated_condition.sqrt()).ceil() as usize;
                (iters.min(1000), ComplexityClass::SqrtCondition)
            }
            Algorithm::ForwardPush | Algorithm::BackwardPush => {
                let iters = ((n as f64).sqrt()).ceil() as usize;
                (iters, ComplexityClass::SublinearNnz)
            }
            Algorithm::HybridRandomWalk => (n.min(1000), ComplexityClass::Linear),
            Algorithm::TRUE => {
                let iters = (profile.estimated_condition.sqrt()).ceil() as usize;
                (iters.min(1000), ComplexityClass::SqrtCondition)
            }
            Algorithm::BMSSP => {
                let iters = (profile.estimated_condition.sqrt().ln()).ceil() as usize;
                (iters.max(1).min(1000), ComplexityClass::Linear)
            }
            Algorithm::Dense => (1, ComplexityClass::Cubic),
            Algorithm::Jacobi | Algorithm::GaussSeidel => (1000, ComplexityClass::Linear),
        };

        let estimated_flops = match algorithm {
            Algorithm::Dense => {
                let dim = n as u64;
                (2 * dim * dim * dim) / 3
            }
            _ => (estimated_iterations as u64) * (2 * profile.nnz as u64 + n as u64),
        };

        let estimated_memory_bytes = match algorithm {
            Algorithm::Dense => n * profile.cols * std::mem::size_of::<f64>(),
            _ => {
                // CSR storage + 3 work vectors.
                let csr = profile.nnz * (std::mem::size_of::<f64>() + std::mem::size_of::<usize>())
                    + (n + 1) * std::mem::size_of::<usize>();
                let work = 3 * n * std::mem::size_of::<f64>();
                csr + work
            }
        };

        ComplexityEstimate {
            algorithm,
            estimated_flops,
            estimated_iterations,
            estimated_memory_bytes,
            complexity_class,
        }
    }

    /// Analyse the sparsity profile of a CSR matrix.
    ///
    /// Performs a single O(nnz) pass over the matrix to compute structural
    /// and numerical properties used by the router. This is intentionally
    /// cheap so it can be called on every solve request.
    pub fn analyze_sparsity(matrix: &CsrMatrix<f64>) -> SparsityProfile {
        let n = matrix.rows;
        let m = matrix.cols;
        let nnz = matrix.nnz();
        let total_entries = (n as f64) * (m as f64);
        let density = if total_entries > 0.0 {
            nnz as f64 / total_entries
        } else {
            0.0
        };

        let mut is_diag_dominant = true;
        let mut max_nnz_per_row: usize = 0;
        let mut sum_off_diag_ratio = 0.0_f64;
        let mut diag_min = f64::INFINITY;
        let mut diag_max = 0.0_f64;
        let mut symmetric_mismatches: usize = 0;

        // Only check symmetry for small-to-medium matrices to keep O(nnz).
        let check_symmetry = nnz <= 100_000;

        for row in 0..n {
            let start = matrix.row_ptr[row];
            let end = matrix.row_ptr[row + 1];
            let row_nnz = end - start;
            max_nnz_per_row = max_nnz_per_row.max(row_nnz);

            let mut diag_val: f64 = 0.0;
            let mut off_diag_sum: f64 = 0.0;

            for idx in start..end {
                let col = matrix.col_indices[idx];
                let val = matrix.values[idx];

                if col == row {
                    diag_val = val.abs();
                } else {
                    off_diag_sum += val.abs();
                }

                // Structural symmetry check: look for (col, row) entry.
                if check_symmetry && col != row && col < n {
                    let col_start = matrix.row_ptr[col];
                    let col_end = matrix.row_ptr[col + 1];
                    let found = matrix.col_indices[col_start..col_end]
                        .binary_search(&row)
                        .is_ok();
                    if !found {
                        symmetric_mismatches += 1;
                    }
                }
            }

            if diag_val <= off_diag_sum {
                is_diag_dominant = false;
            }

            if diag_val > 0.0 {
                let ratio = off_diag_sum / diag_val;
                sum_off_diag_ratio += ratio;
                diag_min = diag_min.min(diag_val);
                diag_max = diag_max.max(diag_val);
            } else if n > 0 {
                is_diag_dominant = false;
                sum_off_diag_ratio += 1.0;
            }
        }

        let avg_nnz_per_row = if n > 0 { nnz as f64 / n as f64 } else { 0.0 };

        // Spectral radius of Jacobi iteration matrix D^{-1}(L+U).
        let estimated_spectral_radius = if n > 0 {
            sum_off_diag_ratio / n as f64
        } else {
            0.0
        };

        // Rough condition number from diagonal range.
        let estimated_condition = if diag_min > 0.0 && diag_min.is_finite() {
            diag_max / diag_min
        } else {
            f64::INFINITY
        };

        let is_symmetric_structure = if check_symmetry {
            symmetric_mismatches == 0
        } else {
            n == m
        };

        SparsityProfile {
            rows: n,
            cols: m,
            nnz,
            density,
            is_diag_dominant,
            estimated_spectral_radius,
            estimated_condition,
            is_symmetric_structure,
            avg_nnz_per_row,
            max_nnz_per_row,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Build a deduplicated fallback chain: `[primary, CG, Dense]`.
    fn build_fallback_chain(primary: Algorithm) -> Vec<Algorithm> {
        let mut chain = Vec::with_capacity(3);
        chain.push(primary);

        if primary != Algorithm::CG {
            chain.push(Algorithm::CG);
        }
        if primary != Algorithm::Dense {
            chain.push(Algorithm::Dense);
        }

        chain
    }

    /// Dispatch a solve request to the concrete solver for `algorithm`.
    ///
    /// Feature-gated solvers return a `BackendError` when the feature is
    /// not compiled in, allowing the fallback chain to proceed.
    fn dispatch(
        &self,
        algorithm: Algorithm,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        match algorithm {
            // ----- Neumann series ------------------------------------------
            Algorithm::Neumann => {
                #[cfg(feature = "neumann")]
                {
                    let solver =
                        crate::neumann::NeumannSolver::new(budget.tolerance, budget.max_iterations);
                    SolverEngine::solve(&solver, matrix, rhs, budget)
                }
                #[cfg(not(feature = "neumann"))]
                {
                    Err(SolverError::BackendError(
                        "neumann feature is not enabled".into(),
                    ))
                }
            }

            // ----- Conjugate Gradient --------------------------------------
            Algorithm::CG => {
                #[cfg(feature = "cg")]
                {
                    let solver = crate::cg::ConjugateGradientSolver::new(
                        budget.tolerance,
                        budget.max_iterations,
                        false,
                    );
                    solver.solve(matrix, rhs, budget)
                }
                #[cfg(not(feature = "cg"))]
                {
                    // Inline CG when the feature crate is not available.
                    self.solve_cg_inline(matrix, rhs, budget)
                }
            }

            // ----- ForwardPush ---------------------------------------------
            Algorithm::ForwardPush => {
                #[cfg(feature = "forward-push")]
                {
                    self.solve_jacobi_fallback(Algorithm::ForwardPush, matrix, rhs, budget)
                }
                #[cfg(not(feature = "forward-push"))]
                {
                    Err(SolverError::BackendError(
                        "forward-push feature is not enabled".into(),
                    ))
                }
            }

            // ----- BackwardPush --------------------------------------------
            Algorithm::BackwardPush => {
                #[cfg(feature = "backward-push")]
                {
                    self.solve_jacobi_fallback(Algorithm::BackwardPush, matrix, rhs, budget)
                }
                #[cfg(not(feature = "backward-push"))]
                {
                    Err(SolverError::BackendError(
                        "backward-push feature is not enabled".into(),
                    ))
                }
            }

            // ----- HybridRandomWalk ----------------------------------------
            Algorithm::HybridRandomWalk => {
                #[cfg(feature = "hybrid-random-walk")]
                {
                    self.solve_jacobi_fallback(Algorithm::HybridRandomWalk, matrix, rhs, budget)
                }
                #[cfg(not(feature = "hybrid-random-walk"))]
                {
                    Err(SolverError::BackendError(
                        "hybrid-random-walk feature is not enabled".into(),
                    ))
                }
            }

            // ----- TRUE batch solver ---------------------------------------
            Algorithm::TRUE => {
                #[cfg(feature = "true-solver")]
                {
                    // TRUE for a single RHS degrades to Neumann.
                    let solver =
                        crate::neumann::NeumannSolver::new(budget.tolerance, budget.max_iterations);
                    let mut result = SolverEngine::solve(&solver, matrix, rhs, budget)?;
                    result.algorithm = Algorithm::TRUE;
                    Ok(result)
                }
                #[cfg(not(feature = "true-solver"))]
                {
                    Err(SolverError::BackendError(
                        "true-solver feature is not enabled".into(),
                    ))
                }
            }

            // ----- BMSSP ---------------------------------------------------
            Algorithm::BMSSP => {
                #[cfg(feature = "bmssp")]
                {
                    self.solve_jacobi_fallback(Algorithm::BMSSP, matrix, rhs, budget)
                }
                #[cfg(not(feature = "bmssp"))]
                {
                    Err(SolverError::BackendError(
                        "bmssp feature is not enabled".into(),
                    ))
                }
            }

            // ----- Dense direct solver -------------------------------------
            Algorithm::Dense => self.solve_dense(matrix, rhs, budget),

            // ----- Legacy iterative solvers --------------------------------
            Algorithm::Jacobi => self.solve_jacobi_fallback(Algorithm::Jacobi, matrix, rhs, budget),
            Algorithm::GaussSeidel => {
                self.solve_jacobi_fallback(Algorithm::GaussSeidel, matrix, rhs, budget)
            }
        }
    }

    /// Inline Conjugate Gradient for symmetric positive-definite systems.
    ///
    /// Standard unpreconditioned CG. Used when the `cg` feature crate is
    /// not compiled in but CG is needed (e.g. as a fallback).
    #[allow(dead_code)]
    fn solve_cg_inline(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let n = matrix.rows;
        validate_square(matrix)?;
        validate_rhs_len(matrix, rhs)?;

        let max_iters = budget.max_iterations;
        let tol = budget.tolerance;
        let start = Instant::now();

        let mut x = vec![0.0_f64; n];
        let mut r: Vec<f64> = rhs.to_vec();
        let mut p = r.clone();
        let mut ap = vec![0.0_f64; n];
        let mut convergence_history = Vec::new();

        let mut r_dot_r = dot(&r, &r);

        for iter in 0..max_iters {
            let residual_norm = r_dot_r.sqrt();

            convergence_history.push(ConvergenceInfo {
                iteration: iter,
                residual_norm,
            });

            if residual_norm.is_nan() || residual_norm.is_infinite() {
                return Err(SolverError::NumericalInstability {
                    iteration: iter,
                    detail: format!("CG residual became {}", residual_norm),
                });
            }

            if residual_norm < tol {
                return Ok(SolverResult {
                    solution: x.iter().map(|&v| v as f32).collect(),
                    iterations: iter,
                    residual_norm,
                    wall_time: start.elapsed(),
                    convergence_history,
                    algorithm: Algorithm::CG,
                });
            }

            // ap = A * p
            matrix.spmv(&p, &mut ap);

            let p_dot_ap = dot(&p, &ap);
            if p_dot_ap.abs() < 1e-30 {
                return Err(SolverError::NumericalInstability {
                    iteration: iter,
                    detail: "CG: p^T A p near zero (matrix may not be SPD)".into(),
                });
            }

            let alpha = r_dot_r / p_dot_ap;

            for i in 0..n {
                x[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }

            let new_r_dot_r = dot(&r, &r);
            let beta = new_r_dot_r / r_dot_r;

            for i in 0..n {
                p[i] = r[i] + beta * p[i];
            }

            r_dot_r = new_r_dot_r;

            if start.elapsed() > budget.max_time {
                return Err(SolverError::BudgetExhausted {
                    reason: "wall-clock time limit exceeded".into(),
                    elapsed: start.elapsed(),
                });
            }
        }

        let final_residual = convergence_history
            .last()
            .map(|c| c.residual_norm)
            .unwrap_or(f64::INFINITY);

        Err(SolverError::NonConvergence {
            iterations: max_iters,
            residual: final_residual,
            tolerance: tol,
        })
    }

    /// Dense direct solver via Gaussian elimination with partial pivoting.
    ///
    /// O(n^3) time and O(n^2) memory. Only used as a last-resort fallback.
    fn solve_dense(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        _budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let n = matrix.rows;
        validate_square(matrix)?;
        validate_rhs_len(matrix, rhs)?;

        const MAX_DENSE_DIM: usize = 4096;
        if n > MAX_DENSE_DIM {
            return Err(SolverError::InvalidInput(
                crate::error::ValidationError::MatrixTooLarge {
                    rows: n,
                    cols: n,
                    max_dim: MAX_DENSE_DIM,
                },
            ));
        }

        let start = Instant::now();

        // Expand CSR to dense augmented matrix [A | b].
        let stride = n + 1;
        let mut aug = vec![0.0_f64; n * stride];
        for row in 0..n {
            let rs = matrix.row_ptr[row];
            let re = matrix.row_ptr[row + 1];
            for idx in rs..re {
                let col = matrix.col_indices[idx];
                aug[row * stride + col] = matrix.values[idx];
            }
            aug[row * stride + n] = rhs[row];
        }

        // Gaussian elimination with partial pivoting.
        for col in 0..n {
            let mut max_val = aug[col * stride + col].abs();
            let mut max_row = col;
            for row in (col + 1)..n {
                let val = aug[row * stride + col].abs();
                if val > max_val {
                    max_val = val;
                    max_row = row;
                }
            }

            if max_val < 1e-12 {
                return Err(SolverError::NumericalInstability {
                    iteration: 0,
                    detail: format!(
                        "dense solver: near-zero pivot ({:.2e}) at column {}",
                        max_val, col
                    ),
                });
            }

            if max_row != col {
                for j in 0..stride {
                    aug.swap(col * stride + j, max_row * stride + j);
                }
            }

            let pivot = aug[col * stride + col];
            for row in (col + 1)..n {
                let factor = aug[row * stride + col] / pivot;
                aug[row * stride + col] = 0.0;
                for j in (col + 1)..stride {
                    let above = aug[col * stride + j];
                    aug[row * stride + j] -= factor * above;
                }
            }
        }

        // Back-substitution.
        let mut solution_f64 = vec![0.0_f64; n];
        for row in (0..n).rev() {
            let mut sum = aug[row * stride + n];
            for col in (row + 1)..n {
                sum -= aug[row * stride + col] * solution_f64[col];
            }
            solution_f64[row] = sum / aug[row * stride + row];
        }

        // Compute residual.
        let mut ax = vec![0.0_f64; n];
        matrix.spmv(&solution_f64, &mut ax);
        let residual_norm: f64 = (0..n)
            .map(|i| {
                let r = rhs[i] - ax[i];
                r * r
            })
            .sum::<f64>()
            .sqrt();

        let solution: Vec<f32> = solution_f64.iter().map(|&v| v as f32).collect();

        Ok(SolverResult {
            solution,
            iterations: 1,
            residual_norm,
            wall_time: start.elapsed(),
            convergence_history: vec![ConvergenceInfo {
                iteration: 0,
                residual_norm,
            }],
            algorithm: Algorithm::Dense,
        })
    }

    /// Generic Jacobi-iteration fallback for algorithms whose specialised
    /// backends are not yet implemented.
    ///
    /// Tags the result with the requested `algorithm` label so callers see
    /// the correct algorithm in the result.
    fn solve_jacobi_fallback(
        &self,
        algorithm: Algorithm,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let n = matrix.rows;
        validate_square(matrix)?;
        validate_rhs_len(matrix, rhs)?;

        let max_iters = budget.max_iterations;
        let tol = budget.tolerance;
        let start = Instant::now();

        // Extract diagonal.
        let mut diag = vec![0.0_f64; n];
        for row in 0..n {
            let rs = matrix.row_ptr[row];
            let re = matrix.row_ptr[row + 1];
            for idx in rs..re {
                if matrix.col_indices[idx] == row {
                    diag[row] = matrix.values[idx];
                    break;
                }
            }
        }

        for (i, &d) in diag.iter().enumerate() {
            if d.abs() < 1e-30 {
                return Err(SolverError::NumericalInstability {
                    iteration: 0,
                    detail: format!("zero or near-zero diagonal at row {} (val={:.2e})", i, d),
                });
            }
        }

        let mut x = vec![0.0_f64; n];
        let mut x_new = vec![0.0_f64; n];
        let mut temp = vec![0.0_f64; n];
        let mut convergence_history = Vec::new();

        for iter in 0..max_iters {
            for row in 0..n {
                let rs = matrix.row_ptr[row];
                let re = matrix.row_ptr[row + 1];
                let mut sum = 0.0_f64;
                for idx in rs..re {
                    let col = matrix.col_indices[idx];
                    if col != row {
                        sum += matrix.values[idx] * x[col];
                    }
                }
                x_new[row] = (rhs[row] - sum) / diag[row];
            }

            matrix.spmv(&x_new, &mut temp);
            let residual_norm: f64 = (0..n)
                .map(|i| {
                    let r = rhs[i] - temp[i];
                    r * r
                })
                .sum::<f64>()
                .sqrt();

            convergence_history.push(ConvergenceInfo {
                iteration: iter,
                residual_norm,
            });

            if residual_norm.is_nan() || residual_norm.is_infinite() {
                return Err(SolverError::NumericalInstability {
                    iteration: iter,
                    detail: format!("residual became {}", residual_norm),
                });
            }

            if residual_norm < tol {
                return Ok(SolverResult {
                    solution: x_new.iter().map(|&v| v as f32).collect(),
                    iterations: iter + 1,
                    residual_norm,
                    wall_time: start.elapsed(),
                    convergence_history,
                    algorithm,
                });
            }

            std::mem::swap(&mut x, &mut x_new);

            if start.elapsed() > budget.max_time {
                return Err(SolverError::BudgetExhausted {
                    reason: "wall-clock time limit exceeded".into(),
                    elapsed: start.elapsed(),
                });
            }
        }

        let final_residual = convergence_history
            .last()
            .map(|c| c.residual_norm)
            .unwrap_or(f64::INFINITY);

        Err(SolverError::NonConvergence {
            iterations: max_iters,
            residual: final_residual,
            tolerance: tol,
        })
    }
}

impl Default for SolverOrchestrator {
    fn default() -> Self {
        Self::new(RouterConfig::default())
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Dot product of two f64 slices.
#[inline]
#[allow(dead_code)]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(
        a.len(),
        b.len(),
        "dot: length mismatch {} vs {}",
        a.len(),
        b.len()
    );
    a.iter().zip(b.iter()).map(|(&ai, &bi)| ai * bi).sum()
}

/// Validate that a matrix is square.
fn validate_square(matrix: &CsrMatrix<f64>) -> Result<(), SolverError> {
    if matrix.rows != matrix.cols {
        return Err(SolverError::InvalidInput(
            crate::error::ValidationError::DimensionMismatch(format!(
                "matrix must be square, got {}x{}",
                matrix.rows, matrix.cols
            )),
        ));
    }
    Ok(())
}

/// Validate that the RHS vector length matches the matrix dimension.
fn validate_rhs_len(matrix: &CsrMatrix<f64>, rhs: &[f64]) -> Result<(), SolverError> {
    if rhs.len() != matrix.rows {
        return Err(SolverError::InvalidInput(
            crate::error::ValidationError::DimensionMismatch(format!(
                "rhs length {} does not match matrix dimension {}",
                rhs.len(),
                matrix.rows
            )),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 3x3 diagonally dominant SPD matrix.
    fn diag_dominant_3x3() -> CsrMatrix<f64> {
        CsrMatrix::<f64>::from_coo(
            3,
            3,
            vec![
                (0, 0, 4.0),
                (0, 1, -1.0),
                (1, 0, -1.0),
                (1, 1, 4.0),
                (1, 2, -1.0),
                (2, 1, -1.0),
                (2, 2, 4.0),
            ],
        )
    }

    fn default_budget() -> ComputeBudget {
        ComputeBudget {
            tolerance: 1e-8,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // Router tests
    // -----------------------------------------------------------------------

    #[test]
    fn routes_diag_dominant_sparse_to_neumann() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 3000,
            density: 0.003,
            is_diag_dominant: true,
            estimated_spectral_radius: 0.5,
            estimated_condition: 10.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 3.0,
            max_nnz_per_row: 5,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::LinearSystem),
            Algorithm::Neumann
        );
    }

    #[test]
    fn routes_well_conditioned_non_diag_dominant_to_cg() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 50_000,
            density: 0.05,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.9,
            estimated_condition: 50.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 50.0,
            max_nnz_per_row: 80,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::LinearSystem),
            Algorithm::CG
        );
    }

    #[test]
    fn routes_ill_conditioned_to_bmssp() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 50_000,
            density: 0.05,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.99,
            estimated_condition: 500.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 50.0,
            max_nnz_per_row: 80,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::LinearSystem),
            Algorithm::BMSSP
        );
    }

    #[test]
    fn routes_single_pagerank_to_forward_push() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 5000,
            cols: 5000,
            nnz: 20_000,
            density: 0.0008,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.85,
            estimated_condition: 100.0,
            is_symmetric_structure: false,
            avg_nnz_per_row: 4.0,
            max_nnz_per_row: 50,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::PageRankSingle { source: 0 }),
            Algorithm::ForwardPush
        );
    }

    #[test]
    fn routes_large_pairwise_to_hybrid_random_walk() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 5000,
            cols: 5000,
            nnz: 20_000,
            density: 0.0008,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.85,
            estimated_condition: 100.0,
            is_symmetric_structure: false,
            avg_nnz_per_row: 4.0,
            max_nnz_per_row: 50,
        };

        assert_eq!(
            router.select_algorithm(
                &profile,
                &QueryType::PageRankPairwise {
                    source: 0,
                    target: 100,
                }
            ),
            Algorithm::HybridRandomWalk
        );
    }

    #[test]
    fn routes_small_pairwise_to_forward_push() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 500,
            cols: 500,
            nnz: 2000,
            density: 0.008,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.85,
            estimated_condition: 100.0,
            is_symmetric_structure: false,
            avg_nnz_per_row: 4.0,
            max_nnz_per_row: 10,
        };

        assert_eq!(
            router.select_algorithm(
                &profile,
                &QueryType::PageRankPairwise {
                    source: 0,
                    target: 10,
                }
            ),
            Algorithm::ForwardPush
        );
    }

    #[test]
    fn routes_spectral_filter_to_neumann() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 100,
            cols: 100,
            nnz: 500,
            density: 0.05,
            is_diag_dominant: true,
            estimated_spectral_radius: 0.3,
            estimated_condition: 5.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 5.0,
            max_nnz_per_row: 8,
        };

        assert_eq!(
            router.select_algorithm(
                &profile,
                &QueryType::SpectralFilter {
                    polynomial_degree: 10,
                }
            ),
            Algorithm::Neumann
        );
    }

    #[test]
    fn routes_large_batch_to_true() {
        let router = SolverRouter::new(RouterConfig::default());
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
            max_nnz_per_row: 10,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::BatchLinearSystem { batch_size: 200 }),
            Algorithm::TRUE
        );
    }

    #[test]
    fn routes_small_batch_to_cg() {
        let router = SolverRouter::new(RouterConfig::default());
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
            max_nnz_per_row: 10,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::BatchLinearSystem { batch_size: 50 }),
            Algorithm::CG
        );
    }

    #[test]
    fn custom_config_overrides_thresholds() {
        let config = RouterConfig {
            cg_condition_threshold: 10.0,
            ..Default::default()
        };
        let router = SolverRouter::new(config);

        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 50_000,
            density: 0.05,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.9,
            estimated_condition: 50.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 50.0,
            max_nnz_per_row: 80,
        };

        assert_eq!(
            router.select_algorithm(&profile, &QueryType::LinearSystem),
            Algorithm::BMSSP
        );
    }

    #[test]
    fn neumann_requires_low_spectral_radius() {
        let router = SolverRouter::new(RouterConfig::default());
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 3000,
            density: 0.003,
            is_diag_dominant: true,
            estimated_spectral_radius: 0.96, // above 0.95 threshold
            estimated_condition: 10.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 3.0,
            max_nnz_per_row: 5,
        };

        // Should fall through to CG, not Neumann.
        assert_eq!(
            router.select_algorithm(&profile, &QueryType::LinearSystem),
            Algorithm::CG
        );
    }

    // -----------------------------------------------------------------------
    // SparsityProfile analysis tests
    // -----------------------------------------------------------------------

    #[test]
    fn analyze_identity_matrix() {
        let matrix = CsrMatrix::<f64>::identity(5);
        let profile = SolverOrchestrator::analyze_sparsity(&matrix);

        assert_eq!(profile.rows, 5);
        assert_eq!(profile.cols, 5);
        assert_eq!(profile.nnz, 5);
        assert!(profile.is_diag_dominant);
        assert!((profile.density - 0.2).abs() < 1e-10);
        assert!(profile.estimated_spectral_radius.abs() < 1e-10);
        assert!((profile.estimated_condition - 1.0).abs() < 1e-10);
        assert!(profile.is_symmetric_structure);
        assert_eq!(profile.max_nnz_per_row, 1);
    }

    #[test]
    fn analyze_diag_dominant() {
        let matrix = diag_dominant_3x3();
        let profile = SolverOrchestrator::analyze_sparsity(&matrix);

        assert!(profile.is_diag_dominant);
        assert!(profile.estimated_spectral_radius < 1.0);
        assert!(profile.is_symmetric_structure);
    }

    #[test]
    fn analyze_empty_matrix() {
        let matrix = CsrMatrix::<f64> {
            row_ptr: vec![0],
            col_indices: vec![],
            values: vec![],
            rows: 0,
            cols: 0,
        };
        let profile = SolverOrchestrator::analyze_sparsity(&matrix);

        assert_eq!(profile.rows, 0);
        assert_eq!(profile.nnz, 0);
        assert_eq!(profile.density, 0.0);
    }

    // -----------------------------------------------------------------------
    // Orchestrator solve tests
    // -----------------------------------------------------------------------

    #[test]
    fn orchestrator_solve_identity() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = CsrMatrix::<f64>::identity(4);
        let rhs = vec![1.0_f64, 2.0, 3.0, 4.0];
        let budget = default_budget();

        let result = orchestrator
            .solve(&matrix, &rhs, QueryType::LinearSystem, &budget)
            .unwrap();

        for (x, b) in result.solution.iter().zip(rhs.iter()) {
            assert!((*x as f64 - b).abs() < 1e-4, "expected {}, got {}", b, x);
        }
    }

    #[test]
    fn orchestrator_solve_diag_dominant() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = diag_dominant_3x3();
        let rhs = vec![1.0_f64, 0.0, 1.0];
        let budget = default_budget();

        let result = orchestrator
            .solve(&matrix, &rhs, QueryType::LinearSystem, &budget)
            .unwrap();

        assert!(result.residual_norm < 1e-6);
    }

    #[test]
    fn orchestrator_solve_with_fallback_succeeds() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = diag_dominant_3x3();
        let rhs = vec![1.0_f64, 0.0, 1.0];
        let budget = default_budget();

        let result = orchestrator
            .solve_with_fallback(&matrix, &rhs, QueryType::LinearSystem, &budget)
            .unwrap();

        assert!(result.residual_norm < 1e-6);
    }

    #[test]
    fn orchestrator_dimension_mismatch() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = CsrMatrix::<f64>::identity(3);
        let rhs = vec![1.0_f64, 2.0]; // wrong length
        let budget = default_budget();

        let result = orchestrator.solve(&matrix, &rhs, QueryType::LinearSystem, &budget);
        assert!(result.is_err());
    }

    #[test]
    fn estimate_complexity_returns_reasonable_values() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = diag_dominant_3x3();

        let estimate = orchestrator.estimate_complexity(&matrix, &QueryType::LinearSystem);

        assert!(estimate.estimated_flops > 0);
        assert!(estimate.estimated_memory_bytes > 0);
        assert!(estimate.estimated_iterations > 0);
    }

    #[test]
    fn fallback_chain_deduplicates() {
        let chain = SolverOrchestrator::build_fallback_chain(Algorithm::CG);
        assert_eq!(chain, vec![Algorithm::CG, Algorithm::Dense]);

        let chain = SolverOrchestrator::build_fallback_chain(Algorithm::Dense);
        assert_eq!(chain, vec![Algorithm::Dense, Algorithm::CG]);

        let chain = SolverOrchestrator::build_fallback_chain(Algorithm::Neumann);
        assert_eq!(
            chain,
            vec![Algorithm::Neumann, Algorithm::CG, Algorithm::Dense]
        );
    }

    #[test]
    fn cg_inline_solves_spd_system() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = diag_dominant_3x3();
        let rhs = vec![1.0_f64, 2.0, 3.0];
        let budget = default_budget();

        let result = orchestrator
            .solve_cg_inline(&matrix, &rhs, &budget)
            .unwrap();

        assert!(result.residual_norm < 1e-6);
        assert_eq!(result.algorithm, Algorithm::CG);
    }

    #[test]
    fn dense_solves_small_system() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = diag_dominant_3x3();
        let rhs = vec![1.0_f64, 2.0, 3.0];
        let budget = default_budget();

        let result = orchestrator.solve_dense(&matrix, &rhs, &budget).unwrap();

        assert!(result.residual_norm < 1e-4);
        assert_eq!(result.algorithm, Algorithm::Dense);
    }

    #[test]
    fn dense_rejects_non_square() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = CsrMatrix::<f64> {
            row_ptr: vec![0, 1, 2],
            col_indices: vec![0, 1],
            values: vec![1.0, 1.0],
            rows: 2,
            cols: 3,
        };
        let rhs = vec![1.0_f64, 1.0];
        let budget = default_budget();

        assert!(orchestrator.solve_dense(&matrix, &rhs, &budget).is_err());
    }

    #[test]
    fn cg_and_dense_agree_on_solution() {
        let orchestrator = SolverOrchestrator::new(RouterConfig::default());
        let matrix = diag_dominant_3x3();
        let rhs = vec![3.0_f64, -1.0, 2.0];
        let budget = default_budget();

        let cg_result = orchestrator
            .solve_cg_inline(&matrix, &rhs, &budget)
            .unwrap();
        let dense_result = orchestrator.solve_dense(&matrix, &rhs, &budget).unwrap();

        for (cg_x, dense_x) in cg_result.solution.iter().zip(dense_result.solution.iter()) {
            assert!(
                (cg_x - dense_x).abs() < 1e-3,
                "CG={} vs Dense={}",
                cg_x,
                dense_x
            );
        }
    }
}
