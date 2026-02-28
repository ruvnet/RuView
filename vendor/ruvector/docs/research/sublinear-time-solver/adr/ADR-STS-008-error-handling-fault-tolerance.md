# ADR-STS-008: Error Handling and Fault Tolerance

## Status

Accepted

## Date

2026-02-20

## Authors

RuVector Architecture Team

## Deciders

Architecture Review Board

---

## Context

Integrating the sublinear-time-solver into RuVector introduces a class of failure modes that do not exist in traditional dense linear algebra: non-convergence when the spectral radius of the Neumann iteration matrix is at or above 1, floating-point accumulation errors compounding across thousands of sparse matrix-vector products, Monte Carlo variance in hybrid random walk methods exceeding acceptable bounds, coarsening pathology in BMSSP multigrid when the graph structure defeats the aggregation heuristic, and Johnson-Lindenstrauss projection distortion compounding through the TRUE pipeline. These failure modes are distinct from standard I/O or resource errors because they are mathematical in nature -- the computation completes without panicking, but the result is numerically meaningless or insufficiently precise.

RuVector subsystems that will consume solver results -- Prime Radiant coherence energy, GNN message-passing aggregation, spectral Chebyshev filtering, graph PageRank, optimal transport, and sparse inference calibration -- each have different tolerance thresholds for approximation error. A coherence energy computation that diverges by 10% may trigger false contradiction detection. A PageRank vector with excess variance may produce incorrect search rankings. The system must detect these conditions, report them with sufficient diagnostic information for debugging, and fall back to methods with stronger guarantees.

Additionally, the solver will operate under compute budgets (wall-clock time, iteration count, memory) imposed by the calling subsystem. Budget exhaustion must not result in undefined behavior or silent corruption. The solver must return the best result achieved so far alongside a clear indication that the budget was exceeded.

The error handling strategy must also account for the quantization layer: RuVector stores vectors at multiple precision levels (f32, u8, int4, binary), and solver output that will be quantized must track solver error and quantization error as separate budgets to ensure the combined error stays within the caller's epsilon.

---

## Decision

### 1. Error Type Hierarchy

A structured error hierarchy that captures the mathematical nature of solver failures, providing enough diagnostic information for the calling subsystem to decide whether to retry, fall back, or propagate the error.

```rust
use std::fmt;

/// Compute budget constraining solver execution.
#[derive(Debug, Clone)]
pub struct ComputeBudget {
    /// Maximum wall-clock time in microseconds.
    pub max_wall_time_us: u64,
    /// Maximum number of solver iterations.
    pub max_iterations: usize,
    /// Maximum memory allocation in bytes.
    pub max_memory_bytes: usize,
    /// Elapsed wall-clock time at point of check.
    pub elapsed_us: u64,
    /// Iterations consumed so far.
    pub iterations_used: usize,
    /// Memory allocated so far in bytes.
    pub memory_used_bytes: usize,
}

impl ComputeBudget {
    pub fn remaining_iterations(&self) -> usize {
        self.max_iterations.saturating_sub(self.iterations_used)
    }

    pub fn remaining_time_us(&self) -> u64 {
        self.max_wall_time_us.saturating_sub(self.elapsed_us)
    }

    pub fn is_exhausted(&self) -> bool {
        self.iterations_used >= self.max_iterations
            || self.elapsed_us >= self.max_wall_time_us
            || self.memory_used_bytes >= self.max_memory_bytes
    }

    pub fn utilization_fraction(&self) -> f64 {
        let iter_frac = self.iterations_used as f64 / self.max_iterations.max(1) as f64;
        let time_frac = self.elapsed_us as f64 / self.max_wall_time_us.max(1) as f64;
        let mem_frac = self.memory_used_bytes as f64 / self.max_memory_bytes.max(1) as f64;
        iter_frac.max(time_frac).max(mem_frac)
    }
}

/// Validation errors for solver input.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Matrix dimensions do not match right-hand side.
    DimensionMismatch { matrix_rows: usize, rhs_len: usize },
    /// Matrix is not square.
    NonSquareMatrix { rows: usize, cols: usize },
    /// Matrix contains NaN or infinity values.
    NonFiniteValues { location: String },
    /// Matrix has zero diagonal entries (cannot use Neumann/Jacobi).
    ZeroDiagonal { indices: Vec<usize> },
    /// Requested epsilon is non-positive or NaN.
    InvalidEpsilon { value: f64 },
    /// Sparsity ratio is outside [0, 1].
    InvalidSparsity { value: f64 },
    /// Empty input (zero-dimension system).
    EmptySystem,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch { matrix_rows, rhs_len } => {
                write!(f, "dimension mismatch: matrix has {} rows but RHS has {} entries",
                    matrix_rows, rhs_len)
            }
            Self::NonSquareMatrix { rows, cols } => {
                write!(f, "non-square matrix: {} x {}", rows, cols)
            }
            Self::NonFiniteValues { location } => {
                write!(f, "non-finite values detected at {}", location)
            }
            Self::ZeroDiagonal { indices } => {
                write!(f, "zero diagonal entries at indices {:?}", &indices[..indices.len().min(10)])
            }
            Self::InvalidEpsilon { value } => {
                write!(f, "invalid epsilon: {}", value)
            }
            Self::InvalidSparsity { value } => {
                write!(f, "invalid sparsity ratio: {}", value)
            }
            Self::EmptySystem => write!(f, "empty system (zero dimensions)"),
        }
    }
}

/// Primary solver error type.
#[derive(Debug)]
pub enum SolverError {
    /// Iterative solver did not reach target residual within budget.
    NonConvergence {
        iterations: usize,
        best_residual: f64,
        target_residual: f64,
        budget: ComputeBudget,
        /// The best solution vector achieved before stopping.
        best_solution: Option<Vec<f64>>,
    },

    /// Numerical instability detected during computation.
    NumericalInstability {
        source: &'static str,
        detail: String,
        /// The iteration at which instability was detected.
        iteration: usize,
        /// The metric that triggered the instability detection.
        metric_value: f64,
        metric_threshold: f64,
    },

    /// Compute budget (time, iterations, or memory) exhausted.
    BudgetExhausted {
        budget: ComputeBudget,
        /// Fraction of convergence achieved (0.0 = no progress, 1.0 = converged).
        progress: f64,
        /// Best solution at point of exhaustion.
        best_solution: Option<Vec<f64>>,
        best_residual: f64,
    },

    /// Input validation failed before solver execution.
    InvalidInput(ValidationError),

    /// Achieved precision is worse than requested.
    PrecisionLoss {
        expected_eps: f64,
        achieved_eps: f64,
        /// Which component lost precision (e.g., "jl_projection", "sparsification").
        component: &'static str,
    },

    /// Matrix sparsity is insufficient for sublinear algorithms.
    SparsityInsufficient {
        actual_density: f64,
        required_max_density: f64,
        nnz: usize,
        n: usize,
    },

    /// Spectral radius check failed (Neumann series will not converge).
    SpectralRadiusExceeded {
        estimated_rho: f64,
        threshold: f64,
    },

    /// Coarsening hierarchy is degenerate (BMSSP pathology).
    CoarseningPathology {
        level: usize,
        coarsening_ratio: f64,
        detail: String,
    },

    /// Random walk variance exceeds statistical bound.
    ExcessiveVariance {
        coefficient_of_variation: f64,
        threshold: f64,
        sample_count: usize,
    },

    /// Wrapped error from an underlying backend.
    BackendError(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonConvergence { iterations, best_residual, target_residual, .. } => {
                write!(f, "non-convergence after {} iterations: residual {:.2e} > target {:.2e}",
                    iterations, best_residual, target_residual)
            }
            Self::NumericalInstability { source, detail, iteration, .. } => {
                write!(f, "numerical instability in {} at iteration {}: {}",
                    source, iteration, detail)
            }
            Self::BudgetExhausted { budget, progress, best_residual, .. } => {
                write!(f, "budget exhausted ({:.0}% utilized, {:.1}% converged, residual {:.2e})",
                    budget.utilization_fraction() * 100.0,
                    progress * 100.0,
                    best_residual)
            }
            Self::InvalidInput(ve) => write!(f, "invalid input: {}", ve),
            Self::PrecisionLoss { expected_eps, achieved_eps, component } => {
                write!(f, "precision loss in {}: expected eps={:.2e}, achieved {:.2e}",
                    component, expected_eps, achieved_eps)
            }
            Self::SparsityInsufficient { actual_density, required_max_density, .. } => {
                write!(f, "insufficient sparsity: density {:.4} > max {:.4}",
                    actual_density, required_max_density)
            }
            Self::SpectralRadiusExceeded { estimated_rho, threshold } => {
                write!(f, "spectral radius {:.4} >= threshold {:.4}", estimated_rho, threshold)
            }
            Self::CoarseningPathology { level, coarsening_ratio, detail } => {
                write!(f, "coarsening pathology at level {} (ratio {:.4}): {}",
                    level, coarsening_ratio, detail)
            }
            Self::ExcessiveVariance { coefficient_of_variation, threshold, sample_count } => {
                write!(f, "excessive variance: CV={:.4} > {:.4} with {} samples",
                    coefficient_of_variation, threshold, sample_count)
            }
            Self::BackendError(e) => write!(f, "backend error: {}", e),
        }
    }
}

impl std::error::Error for SolverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BackendError(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

/// Solver result type alias.
pub type SolverResult<T> = Result<T, SolverError>;
```

### 2. Solver Event System for Convergence Monitoring

An event-based system that allows callers to observe solver progress, cancel long-running computations, and collect diagnostics without polling.

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Events emitted during solver execution.
#[derive(Debug, Clone)]
pub enum SolverEvent {
    /// Emitted after each solver iteration.
    IterationCompleted {
        iteration: usize,
        residual: f64,
        residual_rate: f64,
        elapsed_us: u64,
    },

    /// Emitted when a fallback is triggered.
    FallbackTriggered {
        from_algorithm: &'static str,
        to_algorithm: &'static str,
        reason: String,
    },

    /// Emitted when a numerical guard activates.
    NumericalGuardActivated {
        guard_name: &'static str,
        metric_value: f64,
        threshold: f64,
        action_taken: &'static str,
    },

    /// Emitted when budget utilization crosses a threshold.
    BudgetWarning {
        resource: &'static str,
        utilization_percent: f64,
    },

    /// Emitted when the solver successfully converges.
    Converged {
        iterations: usize,
        final_residual: f64,
        elapsed_us: u64,
    },

    /// Emitted when spectral radius is estimated.
    SpectralRadiusEstimated {
        rho: f64,
        method: &'static str,
    },

    /// Emitted when coarsening level is constructed (BMSSP).
    CoarseningLevelBuilt {
        level: usize,
        vertices: usize,
        edges: usize,
        ratio: f64,
    },
}

/// Cancellation token for cooperative solver interruption.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self { cancelled: Arc::new(AtomicBool::new(false)) }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Observer trait for solver events.
pub trait SolverObserver: Send + Sync {
    fn on_event(&self, event: &SolverEvent);
}

/// Solver execution context carrying budget, cancellation, and observers.
pub struct SolverContext {
    pub budget: ComputeBudget,
    pub cancellation: CancellationToken,
    observers: Vec<Box<dyn SolverObserver>>,
}

impl SolverContext {
    pub fn new(budget: ComputeBudget) -> Self {
        Self {
            budget,
            cancellation: CancellationToken::new(),
            observers: Vec::new(),
        }
    }

    pub fn add_observer(&mut self, observer: Box<dyn SolverObserver>) {
        self.observers.push(observer);
    }

    pub fn emit(&self, event: SolverEvent) {
        for observer in &self.observers {
            observer.on_event(&event);
        }
    }

    /// Check whether the solver should stop (budget exhausted or cancelled).
    pub fn should_stop(&self) -> bool {
        self.cancellation.is_cancelled() || self.budget.is_exhausted()
    }
}
```

### 3. Automatic Fallback Chain

When a sublinear algorithm fails, the solver automatically falls back through a chain of increasingly robust methods. Each transition is logged as a `SolverEvent::FallbackTriggered`.

```
Sublinear Algorithm (TRUE / Neumann / Push / Hybrid / BMSSP)
    |
    | [NonConvergence | NumericalInstability | SpectralRadiusExceeded |
    |  CoarseningPathology | ExcessiveVariance | PrecisionLoss]
    v
Conjugate Gradient (CG) with diagonal preconditioning
    |
    | [NonConvergence after sqrt(kappa) * log(1/eps) iterations |
    |  NumericalInstability (loss of orthogonality)]
    v
Dense Direct Solve (LU / Cholesky via nalgebra)
    |
    | [guaranteed for non-singular systems]
    v
Result or InvalidInput error
```

```rust
/// Fallback chain configuration.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Whether automatic fallback is enabled.
    pub enabled: bool,
    /// Maximum number of fallback levels to attempt.
    pub max_fallback_depth: usize,
    /// Whether to return the best partial result on total failure.
    pub return_partial_on_failure: bool,
    /// CG iteration multiplier relative to sqrt(kappa) estimate.
    pub cg_iteration_multiplier: f64,
    /// Dense solve size limit (do not attempt dense solve above this n).
    pub dense_solve_max_n: usize,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_fallback_depth: 3,
            return_partial_on_failure: true,
            cg_iteration_multiplier: 2.0,
            dense_solve_max_n: 10_000,
        }
    }
}

/// Result of a solve attempt, capturing which algorithm succeeded and any
/// fallbacks that occurred.
#[derive(Debug)]
pub struct SolveOutcome {
    /// The solution vector.
    pub solution: Vec<f64>,
    /// Final residual norm.
    pub residual: f64,
    /// Algorithm that produced the solution.
    pub algorithm: &'static str,
    /// Number of fallbacks triggered before success.
    pub fallback_count: usize,
    /// Chain of (algorithm, error) pairs for each failed attempt.
    pub fallback_chain: Vec<(&'static str, SolverError)>,
    /// Total elapsed time across all attempts in microseconds.
    pub total_elapsed_us: u64,
}

/// Execute the fallback chain for a sparse linear system Ax = b.
pub fn solve_with_fallback(
    matrix: &SparseMatrix,
    rhs: &[f64],
    eps: f64,
    ctx: &mut SolverContext,
    config: &FallbackConfig,
    primary_algorithm: &'static str,
) -> SolverResult<SolveOutcome> {
    let mut fallback_chain: Vec<(&'static str, SolverError)> = Vec::new();
    let start_time = std::time::Instant::now();

    // Phase 1: Attempt primary sublinear algorithm.
    match solve_sublinear(matrix, rhs, eps, ctx, primary_algorithm) {
        Ok(solution) => {
            return Ok(SolveOutcome {
                solution: solution.x,
                residual: solution.residual,
                algorithm: primary_algorithm,
                fallback_count: 0,
                fallback_chain,
                total_elapsed_us: start_time.elapsed().as_micros() as u64,
            });
        }
        Err(e) => {
            ctx.emit(SolverEvent::FallbackTriggered {
                from_algorithm: primary_algorithm,
                to_algorithm: "conjugate_gradient",
                reason: format!("{}", e),
            });
            fallback_chain.push((primary_algorithm, e));
        }
    }

    if !config.enabled || fallback_chain.len() >= config.max_fallback_depth {
        return Err(fallback_chain.pop().map(|(_, e)| e).unwrap());
    }

    // Phase 2: Conjugate Gradient fallback.
    match solve_cg_preconditioned(matrix, rhs, eps, ctx, config.cg_iteration_multiplier) {
        Ok(solution) => {
            return Ok(SolveOutcome {
                solution: solution.x,
                residual: solution.residual,
                algorithm: "conjugate_gradient",
                fallback_count: fallback_chain.len(),
                fallback_chain,
                total_elapsed_us: start_time.elapsed().as_micros() as u64,
            });
        }
        Err(e) => {
            ctx.emit(SolverEvent::FallbackTriggered {
                from_algorithm: "conjugate_gradient",
                to_algorithm: "dense_direct",
                reason: format!("{}", e),
            });
            fallback_chain.push(("conjugate_gradient", e));
        }
    }

    if fallback_chain.len() >= config.max_fallback_depth {
        return Err(fallback_chain.pop().map(|(_, e)| e).unwrap());
    }

    // Phase 3: Dense direct solve (guaranteed for non-singular systems).
    let n = rhs.len();
    if n > config.dense_solve_max_n {
        let err = SolverError::BudgetExhausted {
            budget: ctx.budget.clone(),
            progress: 0.0,
            best_solution: None,
            best_residual: f64::INFINITY,
        };
        fallback_chain.push(("dense_direct", err));
        // Return best partial result from earlier attempts if configured.
        if config.return_partial_on_failure {
            for (alg, ref err) in fallback_chain.iter().rev() {
                if let SolverError::NonConvergence { best_solution: Some(sol), best_residual, .. }
                    | SolverError::BudgetExhausted { best_solution: Some(sol), best_residual, .. } = err
                {
                    return Ok(SolveOutcome {
                        solution: sol.clone(),
                        residual: *best_residual,
                        algorithm: alg,
                        fallback_count: fallback_chain.len(),
                        fallback_chain: Vec::new(),
                        total_elapsed_us: start_time.elapsed().as_micros() as u64,
                    });
                }
            }
        }
        return Err(fallback_chain.pop().map(|(_, e)| e).unwrap());
    }

    match solve_dense_direct(matrix, rhs) {
        Ok(solution) => {
            Ok(SolveOutcome {
                solution: solution.x,
                residual: solution.residual,
                algorithm: "dense_direct",
                fallback_count: fallback_chain.len(),
                fallback_chain,
                total_elapsed_us: start_time.elapsed().as_micros() as u64,
            })
        }
        Err(e) => {
            fallback_chain.push(("dense_direct", e));
            Err(fallback_chain.pop().map(|(_, e)| e).unwrap())
        }
    }
}
```

### 4. Numerical Stability Guards per Algorithm

Each sublinear algorithm has specific numerical pathologies. The following guards detect and mitigate them.

#### 4.1 Neumann Series Guard

```rust
/// Numerical guards for the Neumann series solver.
pub struct NeumannGuard {
    /// Maximum acceptable spectral radius for convergence guarantee.
    pub spectral_radius_threshold: f64,
    /// Regularization delta added to diagonal when rho is borderline.
    pub regularization_delta: f64,
    /// Maximum ratio of consecutive residuals before declaring divergence.
    pub divergence_ratio_threshold: f64,
}

impl Default for NeumannGuard {
    fn default() -> Self {
        Self {
            spectral_radius_threshold: 0.99,
            regularization_delta: 1e-6,
            divergence_ratio_threshold: 1.05,
        }
    }
}

impl NeumannGuard {
    /// Estimate spectral radius of D^{-1}B using power iteration (10 iterations).
    /// Returns Err if rho >= threshold and regularization cannot bring it below.
    pub fn check_spectral_radius(
        &self,
        diagonal: &[f64],
        off_diagonal: &SparseMatrix,
        ctx: &SolverContext,
    ) -> SolverResult<f64> {
        let n = diagonal.len();
        let mut v = vec![1.0 / (n as f64).sqrt(); n];
        let mut rho_estimate = 0.0;

        for power_iter in 0..10 {
            // w = D^{-1} * B * v
            let bv = off_diagonal.matvec(&v);
            let mut w: Vec<f64> = bv.iter()
                .zip(diagonal.iter())
                .map(|(bvi, di)| bvi / di)
                .collect();

            let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm < 1e-15 {
                rho_estimate = 0.0;
                break;
            }
            rho_estimate = norm;
            w.iter_mut().for_each(|x| *x /= norm);
            v = w;

            if ctx.should_stop() { break; }
        }

        ctx.emit(SolverEvent::SpectralRadiusEstimated {
            rho: rho_estimate,
            method: "power_iteration_10",
        });

        if rho_estimate >= self.spectral_radius_threshold {
            Err(SolverError::SpectralRadiusExceeded {
                estimated_rho: rho_estimate,
                threshold: self.spectral_radius_threshold,
            })
        } else {
            Ok(rho_estimate)
        }
    }

    /// Monitor residual ratio between consecutive iterations.
    /// Returns Err if divergence is detected.
    pub fn check_divergence(
        &self,
        prev_residual: f64,
        curr_residual: f64,
        iteration: usize,
    ) -> SolverResult<()> {
        if prev_residual > 0.0 && curr_residual / prev_residual > self.divergence_ratio_threshold {
            Err(SolverError::NumericalInstability {
                source: "neumann_series",
                detail: format!(
                    "residual increased: {:.2e} -> {:.2e} (ratio {:.4})",
                    prev_residual, curr_residual, curr_residual / prev_residual
                ),
                iteration,
                metric_value: curr_residual / prev_residual,
                metric_threshold: self.divergence_ratio_threshold,
            })
        } else {
            Ok(())
        }
    }

    /// Apply diagonal regularization: A' = A + delta * I.
    /// This shifts eigenvalues, reducing spectral radius of the iteration matrix.
    pub fn regularize_diagonal(diagonal: &mut [f64], delta: f64) {
        for d in diagonal.iter_mut() {
            *d += delta;
        }
    }
}
```

#### 4.2 Forward/Backward Push Guard (Kahan Compensated Summation)

```rust
/// Compensated summation accumulator using Kahan's algorithm.
/// Prevents floating-point drift in push-based residual propagation.
#[derive(Debug, Clone)]
pub struct KahanAccumulator {
    sum: f64,
    compensation: f64,
}

impl KahanAccumulator {
    pub fn new() -> Self {
        Self { sum: 0.0, compensation: 0.0 }
    }

    pub fn add(&mut self, value: f64) {
        let y = value - self.compensation;
        let t = self.sum + y;
        self.compensation = (t - self.sum) - y;
        self.sum = t;
    }

    pub fn value(&self) -> f64 {
        self.sum
    }

    pub fn reset(&mut self) {
        self.sum = 0.0;
        self.compensation = 0.0;
    }
}

/// Guard for push-based solvers (Forward Push, Backward Push).
pub struct PushGuard {
    /// Tolerance for mass invariant violation.
    pub mass_invariant_tolerance: f64,
    /// Minimum residual threshold below which entries are zeroed.
    pub residual_floor: f64,
}

impl Default for PushGuard {
    fn default() -> Self {
        Self {
            mass_invariant_tolerance: 1e-10,
            residual_floor: 1e-15,
        }
    }
}

impl PushGuard {
    /// Verify that total mass is conserved: sum(estimate) + sum(residual) = initial_mass.
    /// Push operations redistribute mass; any loss indicates floating-point drift.
    pub fn check_mass_invariant(
        &self,
        estimate: &[f64],
        residual: &[f64],
        initial_mass: f64,
        iteration: usize,
        ctx: &SolverContext,
    ) -> SolverResult<()> {
        let mut est_acc = KahanAccumulator::new();
        for &v in estimate {
            est_acc.add(v);
        }
        let mut res_acc = KahanAccumulator::new();
        for &v in residual {
            res_acc.add(v);
        }

        let total = est_acc.value() + res_acc.value();
        let drift = (total - initial_mass).abs();

        if drift > self.mass_invariant_tolerance * initial_mass.abs().max(1.0) {
            ctx.emit(SolverEvent::NumericalGuardActivated {
                guard_name: "push_mass_invariant",
                metric_value: drift,
                threshold: self.mass_invariant_tolerance * initial_mass.abs().max(1.0),
                action_taken: "error_raised",
            });
            Err(SolverError::NumericalInstability {
                source: "forward_backward_push",
                detail: format!(
                    "mass invariant violated: total={:.15e}, expected={:.15e}, drift={:.2e}",
                    total, initial_mass, drift
                ),
                iteration,
                metric_value: drift,
                metric_threshold: self.mass_invariant_tolerance * initial_mass.abs().max(1.0),
            })
        } else {
            Ok(())
        }
    }

    /// Floor small residuals to zero to prevent denormal accumulation.
    pub fn floor_residuals(&self, residual: &mut [f64]) -> usize {
        let mut zeroed = 0;
        for r in residual.iter_mut() {
            if r.abs() < self.residual_floor {
                *r = 0.0;
                zeroed += 1;
            }
        }
        zeroed
    }
}
```

#### 4.3 Hybrid Random Walk Guard (Variance Control)

```rust
/// Guard for Monte Carlo random walk variance in the Hybrid solver.
pub struct RandomWalkGuard {
    /// Maximum acceptable coefficient of variation (std / mean).
    pub max_cv: f64,
    /// Minimum samples before variance check is meaningful.
    pub min_samples_for_check: usize,
    /// Factor by which to increase sample count on variance failure.
    pub adaptive_sample_multiplier: f64,
    /// Maximum total samples before giving up.
    pub max_total_samples: usize,
}

impl Default for RandomWalkGuard {
    fn default() -> Self {
        Self {
            max_cv: 0.1,
            min_samples_for_check: 32,
            adaptive_sample_multiplier: 2.0,
            max_total_samples: 1_000_000,
        }
    }
}

/// Running statistics for random walk estimates (Welford's online algorithm).
#[derive(Debug, Clone)]
pub struct RunningStats {
    count: usize,
    mean: f64,
    m2: f64,
}

impl RunningStats {
    pub fn new() -> Self {
        Self { count: 0, mean: 0.0, m2: 0.0 }
    }

    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    pub fn count(&self) -> usize { self.count }
    pub fn mean(&self) -> f64 { self.mean }

    pub fn variance(&self) -> f64 {
        if self.count < 2 { return f64::INFINITY; }
        self.m2 / (self.count - 1) as f64
    }

    pub fn std_dev(&self) -> f64 { self.variance().sqrt() }

    pub fn coefficient_of_variation(&self) -> f64 {
        if self.mean.abs() < 1e-15 { return f64::INFINITY; }
        self.std_dev() / self.mean.abs()
    }
}

impl RandomWalkGuard {
    /// Check whether the current sample statistics satisfy variance bounds.
    pub fn check_variance(
        &self,
        stats: &RunningStats,
        ctx: &SolverContext,
    ) -> SolverResult<()> {
        if stats.count() < self.min_samples_for_check {
            return Ok(());
        }

        let cv = stats.coefficient_of_variation();
        if cv > self.max_cv {
            ctx.emit(SolverEvent::NumericalGuardActivated {
                guard_name: "random_walk_variance",
                metric_value: cv,
                threshold: self.max_cv,
                action_taken: "adaptive_resample_or_error",
            });
            Err(SolverError::ExcessiveVariance {
                coefficient_of_variation: cv,
                threshold: self.max_cv,
                sample_count: stats.count(),
            })
        } else {
            Ok(())
        }
    }

    /// Compute the number of additional samples needed to bring CV below threshold.
    /// Uses the relation: CV ~ 1/sqrt(n), so n_needed ~ (cv_current / cv_target)^2 * n_current.
    pub fn additional_samples_needed(&self, stats: &RunningStats) -> usize {
        let cv = stats.coefficient_of_variation();
        if cv <= self.max_cv { return 0; }
        let ratio = cv / self.max_cv;
        let needed = (ratio * ratio * stats.count() as f64).ceil() as usize;
        needed.saturating_sub(stats.count()).min(self.max_total_samples)
    }
}
```

#### 4.4 TRUE Pipeline Guard (Error Budget Allocation)

```rust
/// Error budget allocator for the TRUE pipeline.
/// TRUE combines JL projection, spectral sparsification, and adaptive Neumann.
/// Each component contributes error that must sum to at most the target epsilon.
pub struct TrueErrorBudget {
    pub total_eps: f64,
    pub jl_fraction: f64,
    pub sparsification_fraction: f64,
    pub neumann_fraction: f64,
}

impl TrueErrorBudget {
    /// Default allocation: eps/3 per component.
    pub fn uniform(total_eps: f64) -> Self {
        Self {
            total_eps,
            jl_fraction: 1.0 / 3.0,
            sparsification_fraction: 1.0 / 3.0,
            neumann_fraction: 1.0 / 3.0,
        }
    }

    /// Adaptive allocation based on problem characteristics.
    /// If the graph is already sparse, spend less budget on sparsification.
    /// If the dimension is low, spend less budget on JL.
    pub fn adaptive(total_eps: f64, dimension: usize, density: f64) -> Self {
        let dim_factor = if dimension < 100 { 0.15 } else { 0.35 };
        let sparse_factor = if density < 0.01 { 0.15 } else { 0.35 };
        let neumann_factor = 1.0 - dim_factor - sparse_factor;
        Self {
            total_eps,
            jl_fraction: dim_factor,
            sparsification_fraction: sparse_factor,
            neumann_fraction: neumann_factor,
        }
    }

    pub fn jl_eps(&self) -> f64 { self.total_eps * self.jl_fraction }
    pub fn sparsification_eps(&self) -> f64 { self.total_eps * self.sparsification_fraction }
    pub fn neumann_eps(&self) -> f64 { self.total_eps * self.neumann_fraction }

    /// Track accumulated error from each component and verify total stays within budget.
    pub fn verify_accumulated(
        &self,
        jl_error: f64,
        sparsification_error: f64,
        neumann_error: f64,
    ) -> SolverResult<()> {
        let total_error = jl_error + sparsification_error + neumann_error;
        if total_error > self.total_eps {
            let worst_component = if jl_error >= sparsification_error && jl_error >= neumann_error {
                "jl_projection"
            } else if sparsification_error >= neumann_error {
                "sparsification"
            } else {
                "neumann_series"
            };
            Err(SolverError::PrecisionLoss {
                expected_eps: self.total_eps,
                achieved_eps: total_error,
                component: worst_component,
            })
        } else {
            Ok(())
        }
    }
}
```

#### 4.5 CG Orthogonality Guard

```rust
/// Guard for Conjugate Gradient orthogonality loss.
pub struct CgOrthogonalityGuard {
    /// Reorthogonalization interval (every sqrt(n) steps by default).
    pub reorthogonalize_interval: usize,
    /// Threshold for loss of orthogonality (|p_k^T A p_{k-1}| / (|p_k|*|A p_{k-1}|)).
    pub orthogonality_threshold: f64,
    /// Maximum residual growth ratio before declaring instability.
    pub max_residual_growth: f64,
}

impl CgOrthogonalityGuard {
    pub fn for_dimension(n: usize) -> Self {
        Self {
            reorthogonalize_interval: (n as f64).sqrt().ceil() as usize,
            orthogonality_threshold: 1e-8,
            max_residual_growth: 10.0,
        }
    }

    /// Check whether reorthogonalization is needed at this iteration.
    pub fn should_reorthogonalize(&self, iteration: usize) -> bool {
        iteration > 0 && iteration % self.reorthogonalize_interval == 0
    }

    /// Verify that the search direction maintains sufficient A-conjugacy.
    pub fn check_orthogonality(
        &self,
        p_current: &[f64],
        a_p_prev: &[f64],
        iteration: usize,
        ctx: &SolverContext,
    ) -> SolverResult<()> {
        let dot: f64 = p_current.iter().zip(a_p_prev.iter()).map(|(a, b)| a * b).sum();
        let norm_p: f64 = p_current.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_ap: f64 = a_p_prev.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_p < 1e-15 || norm_ap < 1e-15 {
            return Ok(());
        }

        let orthogonality = dot.abs() / (norm_p * norm_ap);
        if orthogonality > self.orthogonality_threshold {
            ctx.emit(SolverEvent::NumericalGuardActivated {
                guard_name: "cg_orthogonality",
                metric_value: orthogonality,
                threshold: self.orthogonality_threshold,
                action_taken: "reorthogonalization_triggered",
            });
        }
        Ok(())
    }

    /// Monitor residual for unexpected growth indicating numerical breakdown.
    pub fn check_residual_growth(
        &self,
        initial_residual: f64,
        current_residual: f64,
        iteration: usize,
    ) -> SolverResult<()> {
        if initial_residual > 0.0
            && current_residual / initial_residual > self.max_residual_growth
        {
            Err(SolverError::NumericalInstability {
                source: "conjugate_gradient",
                detail: format!(
                    "residual grew from {:.2e} to {:.2e} ({:.1}x)",
                    initial_residual, current_residual,
                    current_residual / initial_residual
                ),
                iteration,
                metric_value: current_residual / initial_residual,
                metric_threshold: self.max_residual_growth,
            })
        } else {
            Ok(())
        }
    }
}
```

#### 4.6 BMSSP Coarsening Guard

```rust
/// Guard for BMSSP (Balanced Multilevel Sparse Solver) coarsening quality.
pub struct BmsspCoarseningGuard {
    /// Minimum acceptable coarsening ratio (coarsened_n / original_n).
    /// Below this, coarsening is too aggressive and loses information.
    pub min_coarsening_ratio: f64,
    /// Maximum acceptable coarsening ratio. Above this, coarsening is too
    /// conservative and the hierarchy is too deep.
    pub max_coarsening_ratio: f64,
    /// Maximum number of hierarchy levels.
    pub max_levels: usize,
    /// V-cycle convergence factor threshold. If the convergence factor
    /// exceeds this, escalate to W-cycle.
    pub v_cycle_convergence_threshold: f64,
}

impl Default for BmsspCoarseningGuard {
    fn default() -> Self {
        Self {
            min_coarsening_ratio: 0.1,
            max_coarsening_ratio: 0.8,
            max_levels: 20,
            v_cycle_convergence_threshold: 0.5,
        }
    }
}

impl BmsspCoarseningGuard {
    /// Validate a coarsening level as it is constructed.
    pub fn check_coarsening_level(
        &self,
        level: usize,
        fine_n: usize,
        coarse_n: usize,
        ctx: &SolverContext,
    ) -> SolverResult<()> {
        if level >= self.max_levels {
            return Err(SolverError::CoarseningPathology {
                level,
                coarsening_ratio: coarse_n as f64 / fine_n as f64,
                detail: format!("exceeded maximum {} levels", self.max_levels),
            });
        }

        let ratio = coarse_n as f64 / fine_n as f64;

        ctx.emit(SolverEvent::CoarseningLevelBuilt {
            level,
            vertices: coarse_n,
            edges: 0,  // filled by caller
            ratio,
        });

        if ratio > self.max_coarsening_ratio {
            return Err(SolverError::CoarseningPathology {
                level,
                coarsening_ratio: ratio,
                detail: format!(
                    "insufficient coarsening: {} -> {} (ratio {:.4} > max {:.4})",
                    fine_n, coarse_n, ratio, self.max_coarsening_ratio
                ),
            });
        }

        if ratio < self.min_coarsening_ratio && coarse_n > 1 {
            ctx.emit(SolverEvent::NumericalGuardActivated {
                guard_name: "bmssp_coarsening_ratio",
                metric_value: ratio,
                threshold: self.min_coarsening_ratio,
                action_taken: "warning_aggressive_coarsening",
            });
        }

        Ok(())
    }

    /// Check V-cycle convergence factor; if poor, recommend W-cycle escalation.
    pub fn check_cycle_convergence(
        &self,
        pre_residual: f64,
        post_residual: f64,
        level: usize,
        ctx: &SolverContext,
    ) -> CycleRecommendation {
        if pre_residual < 1e-15 {
            return CycleRecommendation::Continue;
        }
        let factor = post_residual / pre_residual;
        if factor > self.v_cycle_convergence_threshold {
            ctx.emit(SolverEvent::NumericalGuardActivated {
                guard_name: "bmssp_v_cycle_convergence",
                metric_value: factor,
                threshold: self.v_cycle_convergence_threshold,
                action_taken: "escalate_to_w_cycle",
            });
            CycleRecommendation::EscalateToWCycle { level, factor }
        } else {
            CycleRecommendation::Continue
        }
    }
}

#[derive(Debug)]
pub enum CycleRecommendation {
    Continue,
    EscalateToWCycle { level: usize, factor: f64 },
}
```

### 5. Retry Policy and Circuit Breaker

For repeated solver invocations (e.g., inside an iterative outer loop such as Prime Radiant coherence computation), a circuit breaker pattern prevents wasting compute on a solver configuration that is consistently failing.

```rust
use std::time::Instant;

/// Circuit breaker states for solver invocations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    /// Normal operation. All solve requests pass through.
    Closed,
    /// Too many failures. Solve requests immediately return fallback.
    Open,
    /// Testing whether the problem has resolved. One request passes through.
    HalfOpen,
}

/// Circuit breaker for repeated solver failures.
pub struct SolverCircuitBreaker {
    state: CircuitState,
    failure_count: usize,
    success_count: usize,
    /// Number of consecutive failures before opening the circuit.
    failure_threshold: usize,
    /// Number of consecutive successes in HalfOpen before closing.
    success_threshold: usize,
    /// Time to wait in Open state before transitioning to HalfOpen.
    recovery_timeout_us: u64,
    /// Timestamp when the circuit was opened.
    opened_at: Option<Instant>,
    /// Total calls routed to fallback by the circuit breaker.
    pub fallback_count: usize,
}

impl SolverCircuitBreaker {
    pub fn new(failure_threshold: usize, recovery_timeout_us: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold,
            success_threshold: 2,
            recovery_timeout_us,
            opened_at: None,
            fallback_count: 0,
        }
    }

    /// Check whether a solve request should proceed or be short-circuited.
    pub fn should_attempt(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(opened_at) = self.opened_at {
                    if opened_at.elapsed().as_micros() as u64 >= self.recovery_timeout_us {
                        self.state = CircuitState::HalfOpen;
                        self.success_count = 0;
                        true
                    } else {
                        self.fallback_count += 1;
                        false
                    }
                } else {
                    self.fallback_count += 1;
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful solve.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.state = CircuitState::Closed;
                    self.opened_at = None;
                }
            }
            _ => {
                self.state = CircuitState::Closed;
            }
        }
    }

    /// Record a failed solve.
    pub fn record_failure(&mut self) {
        self.success_count = 0;
        self.failure_count += 1;
        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
            self.opened_at = Some(Instant::now());
        }
    }

    pub fn state(&self) -> CircuitState { self.state }
}
```

### 6. Quantization-Aware Error Propagation

RuVector stores vectors at multiple quantization levels. Solver error and quantization error must be tracked as separate budgets whose sum stays within the caller's tolerance.

```rust
/// Quantization tier used in RuVector.
#[derive(Debug, Clone, Copy)]
pub enum QuantizationTier {
    /// Full f32 precision. No quantization error.
    Full,
    /// Scalar u8 quantization. Error bound: max_val / 255.
    ScalarU8,
    /// 4-bit integer quantization. Error bound: max_val / 15.
    Int4,
    /// Product quantization. Error bound depends on codebook quality.
    ProductQuantization { num_subspaces: usize, bits_per_code: usize },
    /// Binary quantization. Very lossy.
    Binary,
}

impl QuantizationTier {
    /// Worst-case per-element quantization error for a given value range.
    pub fn max_element_error(&self, value_range: f64) -> f64 {
        match self {
            Self::Full => 0.0,
            Self::ScalarU8 => value_range / 255.0,
            Self::Int4 => value_range / 15.0,
            Self::ProductQuantization { bits_per_code, .. } => {
                value_range / ((1usize << bits_per_code) as f64)
            }
            Self::Binary => value_range / 2.0,
        }
    }
}

/// Combined error budget tracking solver error and quantization error.
pub struct CombinedErrorBudget {
    pub total_eps: f64,
    pub solver_eps: f64,
    pub quantization_eps: f64,
    pub solver_error_achieved: f64,
    pub quantization_error_estimated: f64,
}

impl CombinedErrorBudget {
    /// Allocate error budget between solver and quantization.
    /// Solver gets the larger share because quantization error is more predictable.
    pub fn allocate(total_eps: f64, tier: QuantizationTier, value_range: f64, dim: usize) -> Self {
        let quant_error_per_elem = tier.max_element_error(value_range);
        // L2 norm of quantization error for a dim-dimensional vector.
        let quant_error_l2 = quant_error_per_elem * (dim as f64).sqrt();

        // If quantization error alone exceeds budget, solver gets minimal share.
        let quant_budget = quant_error_l2.min(total_eps * 0.5);
        let solver_budget = total_eps - quant_budget;

        Self {
            total_eps,
            solver_eps: solver_budget,
            quantization_eps: quant_budget,
            solver_error_achieved: 0.0,
            quantization_error_estimated: quant_error_l2,
        }
    }

    /// Check whether the combined error is within budget.
    pub fn is_within_budget(&self) -> bool {
        self.solver_error_achieved + self.quantization_error_estimated <= self.total_eps
    }

    /// Return remaining solver error budget after accounting for quantization.
    pub fn remaining_solver_budget(&self) -> f64 {
        (self.total_eps - self.quantization_error_estimated - self.solver_error_achieved).max(0.0)
    }
}
```

### 7. Integration with Prime Radiant Coherence Energy

Solver instability maps to increased contradiction energy in the Prime Radiant coherence engine. When the solver fails or falls back, the coherence subsystem must be notified so it can adjust its energy landscape.

```rust
/// Bridge between solver error events and Prime Radiant coherence energy.
pub struct CoherenceEnergyBridge {
    /// Baseline contradiction energy increase per solver failure.
    pub failure_energy_penalty: f64,
    /// Energy increase per fallback level.
    pub fallback_energy_per_level: f64,
    /// Energy increase proportional to residual gap (achieved - target).
    pub residual_gap_energy_scale: f64,
}

impl Default for CoherenceEnergyBridge {
    fn default() -> Self {
        Self {
            failure_energy_penalty: 0.1,
            fallback_energy_per_level: 0.05,
            residual_gap_energy_scale: 1.0,
        }
    }
}

impl CoherenceEnergyBridge {
    /// Compute contradiction energy contribution from a solver outcome.
    pub fn contradiction_energy(&self, outcome: &SolveOutcome, target_eps: f64) -> f64 {
        let mut energy = 0.0;

        // Penalty for each fallback triggered.
        energy += self.fallback_energy_per_level * outcome.fallback_count as f64;

        // Penalty proportional to residual gap if the solver did not fully converge.
        if outcome.residual > target_eps {
            let gap = (outcome.residual - target_eps) / target_eps.max(1e-15);
            energy += self.residual_gap_energy_scale * gap.min(10.0);
        }

        energy
    }

    /// Compute contradiction energy from a solver failure.
    pub fn failure_energy(&self, error: &SolverError) -> f64 {
        let mut energy = self.failure_energy_penalty;

        match error {
            SolverError::NonConvergence { best_residual, target_residual, .. } => {
                let gap = (best_residual - target_residual) / target_residual.max(1e-15);
                energy += self.residual_gap_energy_scale * gap.min(10.0);
            }
            SolverError::NumericalInstability { .. } => {
                energy += self.failure_energy_penalty * 2.0;
            }
            SolverError::SpectralRadiusExceeded { estimated_rho, threshold } => {
                energy += self.failure_energy_penalty * (estimated_rho / threshold).min(5.0);
            }
            _ => {}
        }

        energy
    }
}

/// Observer that bridges solver events to Prime Radiant.
pub struct PrimeRadiantObserver {
    bridge: CoherenceEnergyBridge,
    accumulated_energy: std::sync::atomic::AtomicU64,
}

impl PrimeRadiantObserver {
    pub fn new(bridge: CoherenceEnergyBridge) -> Self {
        Self {
            bridge,
            accumulated_energy: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn accumulated_energy(&self) -> f64 {
        f64::from_bits(self.accumulated_energy.load(std::sync::atomic::Ordering::Relaxed))
    }
}

impl SolverObserver for PrimeRadiantObserver {
    fn on_event(&self, event: &SolverEvent) {
        match event {
            SolverEvent::FallbackTriggered { .. } => {
                let penalty = self.bridge.fallback_energy_per_level;
                // Atomic add via CAS loop for f64 accumulation.
                loop {
                    let current = self.accumulated_energy.load(std::sync::atomic::Ordering::Relaxed);
                    let current_f64 = f64::from_bits(current);
                    let new_f64 = current_f64 + penalty;
                    match self.accumulated_energy.compare_exchange_weak(
                        current,
                        new_f64.to_bits(),
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
            SolverEvent::NumericalGuardActivated { .. } => {
                // Smaller penalty for guard activations (warning-level).
                let penalty = self.bridge.failure_energy_penalty * 0.25;
                loop {
                    let current = self.accumulated_energy.load(std::sync::atomic::Ordering::Relaxed);
                    let current_f64 = f64::from_bits(current);
                    let new_f64 = current_f64 + penalty;
                    match self.accumulated_energy.compare_exchange_weak(
                        current,
                        new_f64.to_bits(),
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
            _ => {}
        }
    }
}
```

---

## Consequences

### Positive

- Every sublinear algorithm failure is captured with precise diagnostic information (iteration count, residual value, spectral radius, variance metrics), enabling rapid debugging
- The automatic fallback chain guarantees that a solve request always produces a result for non-singular systems, even if the sublinear path fails
- Kahan compensated summation and mass invariant monitoring in push-based methods prevent silent floating-point drift that would otherwise corrupt PageRank and PPR computations
- Compute budget enforcement prevents runaway solves from consuming unbounded resources in production deployments
- The circuit breaker pattern prevents repeated wasted compute when a solver configuration is fundamentally incompatible with the problem structure
- Quantization-aware error budgets prevent the combined solver + quantization error from exceeding caller tolerance, which is critical for RuVector's multi-tier quantization (u8, int4, binary)
- Prime Radiant integration translates solver instability into coherence energy, making mathematical failures visible in the governance layer and enabling automated remediation

### Negative

- The error handling code adds non-trivial runtime overhead: spectral radius estimation (10 power iterations), mass invariant checks (two full-vector scans per push iteration), and Welford variance tracking all consume compute cycles
- The fallback chain may mask fundamental algorithm selection errors -- if a sublinear algorithm consistently falls back to CG, the caller is paying sublinear attempt overhead without benefit
- The circuit breaker introduces state that persists across solve calls, adding complexity to concurrent usage and requiring careful lifecycle management
- Dense direct solve fallback at n > 10,000 is disabled by default, meaning very large ill-conditioned systems have no guaranteed fallback

### Neutral

- The error type hierarchy uses Rust's enum pattern, which is zero-cost at runtime but adds code surface for match exhaustiveness
- SolverEvent observers receive events synchronously on the solver thread; high-frequency iteration events may need buffering for slow observers
- The coherence energy bridge is one-directional (solver to Prime Radiant); Prime Radiant cannot yet instruct the solver to change strategy mid-solve

---

## Options Considered

### Option 1: Panic on Numerical Failure

- **Pros**: Simple implementation. Failures are immediately visible. No partial-result ambiguity.
- **Cons**: Unacceptable for production. A single NaN in a Neumann iteration would crash the entire RuVector process. No fallback, no partial results, no diagnostic information.

### Option 2: Return NaN/Infinity Sentinel Values

- **Pros**: Zero overhead. No error types needed. Caller checks `is_nan()`.
- **Cons**: Silent corruption risk. A NaN that propagates through GNN message-passing, attention scores, or coherence energy is extremely difficult to trace back to its origin. Violates RuVector's data integrity guarantees.

### Option 3: Structured Error Types with Fallback Chain (Selected)

- **Pros**: Full diagnostic information. Automatic fallback with logging. Budget enforcement. Circuit breaker for repeated failures. Compatible with Rust's `Result` idiom and the `?` operator.
- **Cons**: More code. Runtime overhead for guards. Requires callers to handle `SolverResult<T>`.

### Option 4: Exception-Based Error Handling (catch_unwind)

- **Pros**: Can wrap existing code without modifying it. Catches panics from third-party code.
- **Cons**: `catch_unwind` does not catch all panics (e.g., `panic = "abort"`). Performance overhead of unwinding. Does not provide structured diagnostics. Not idiomatic Rust for expected error conditions.

---

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Implementation Status

Comprehensive error hierarchy via thiserror: SolverError with variants for SpectralRadiusExceeded, NonConvergence, NumericalInstability, BudgetExhausted, InvalidInput, ArenaExhausted, and Internal. All variants carry diagnostic context (algorithm, iteration, residual). Results use standard Rust Result<T, SolverError>. Convergence history tracked per-iteration in SolverResult.

---

## Related Decisions

- ADR-STS-001 through ADR-STS-007: Prior sublinear-time-solver integration ADRs
- ADR-001: Deep agentic-flow integration (event-driven patterns)
- ADR-002: Modular DDD Architecture (bounded context for solver errors)
- ADR-003: Security-First Design (input validation at boundaries)
- ADR-009: Hybrid Memory Backend (solver state persistence across retries)

## References

- [Kahan Summation Algorithm](https://en.wikipedia.org/wiki/Kahan_summation_algorithm) -- compensated summation for push-based methods
- [Welford's Online Algorithm](https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance#Welford's_online_algorithm) -- numerically stable running variance
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html) -- fault isolation for repeated failures
- [MADR 3.0](https://adr.github.io/madr/) -- Markdown Any Decision Records format
- RuVector Algorithm Analysis (document 10) -- sublinear algorithm mathematical foundations
- RuVector Architecture Analysis (document 05) -- layered integration strategy
- Spielman and Teng, "Nearly-Linear Time Algorithms for Graph Partitioning, Graph Sparsification, and Solving Linear Systems" -- foundational Laplacian solver theory
- Andersen, Chung, and Lang, "Local Graph Partitioning using PageRank Vectors" -- forward/backward push method theory
