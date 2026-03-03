//! Sublinear-time solver algorithms for asymmetric diagonally dominant systems.
//!
//! This module implements the core solver algorithms including Neumann series,
//! forward/backward push methods, and hybrid random-walk approaches.

use crate::matrix::Matrix;
use crate::types::{
    Precision, ConvergenceMode, NormType, ErrorBounds, SolverStats,
    DimensionType, MemoryInfo, ProfileData
};
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, string::String, boxed::Box};

pub mod neumann;

// Re-export solver implementations
pub use neumann::NeumannSolver;

/// Configuration options for solver algorithms.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SolverOptions {
    /// Convergence tolerance
    pub tolerance: Precision,
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Convergence detection mode
    pub convergence_mode: ConvergenceMode,
    /// Norm type for error measurement
    pub norm_type: NormType,
    /// Enable detailed statistics collection
    pub collect_stats: bool,
    /// Streaming solution interval (0 = no streaming)
    pub streaming_interval: usize,
    /// Initial guess for the solution (if None, use zero)
    pub initial_guess: Option<Vec<Precision>>,
    /// Enable error bounds computation
    pub compute_error_bounds: bool,
    /// Relative tolerance for error bounds
    pub error_bounds_tolerance: Precision,
    /// Enable performance profiling
    pub enable_profiling: bool,
    /// Random seed for stochastic algorithms
    pub random_seed: Option<u64>,
}

impl Default for SolverOptions {
    fn default() -> Self {
        Self {
            tolerance: 1e-6,
            max_iterations: 1000,
            convergence_mode: ConvergenceMode::ResidualNorm,
            norm_type: NormType::L2,
            collect_stats: false,
            streaming_interval: 0,
            initial_guess: None,
            compute_error_bounds: false,
            error_bounds_tolerance: 1e-8,
            enable_profiling: false,
            random_seed: None,
        }
    }
}

impl SolverOptions {
    /// Create options optimized for high precision.
    pub fn high_precision() -> Self {
        Self {
            tolerance: 1e-12,
            max_iterations: 5000,
            convergence_mode: ConvergenceMode::Combined,
            norm_type: NormType::L2,
            collect_stats: true,
            streaming_interval: 0,
            initial_guess: None,
            compute_error_bounds: true,
            error_bounds_tolerance: 1e-14,
            enable_profiling: false,
            random_seed: None,
        }
    }

    /// Create options optimized for fast solving.
    pub fn fast() -> Self {
        Self {
            tolerance: 1e-3,
            max_iterations: 100,
            convergence_mode: ConvergenceMode::ResidualNorm,
            norm_type: NormType::L2,
            collect_stats: false,
            streaming_interval: 0,
            initial_guess: None,
            compute_error_bounds: false,
            error_bounds_tolerance: 1e-4,
            enable_profiling: false,
            random_seed: None,
        }
    }

    /// Create options optimized for streaming applications.
    pub fn streaming(interval: usize) -> Self {
        Self {
            tolerance: 1e-4,
            max_iterations: 1000,
            convergence_mode: ConvergenceMode::ResidualNorm,
            norm_type: NormType::L2,
            collect_stats: true,
            streaming_interval: interval,
            initial_guess: None,
            compute_error_bounds: false,
            error_bounds_tolerance: 1e-6,
            enable_profiling: true,
            random_seed: None,
        }
    }
}

/// Result of a solver computation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SolverResult {
    /// Final solution vector
    pub solution: Vec<Precision>,
    /// Final residual norm
    pub residual_norm: Precision,
    /// Number of iterations performed
    pub iterations: usize,
    /// Whether the algorithm converged
    pub converged: bool,
    /// Error bounds (if computed)
    pub error_bounds: Option<ErrorBounds>,
    /// Detailed statistics (if collected)
    pub stats: Option<SolverStats>,
    /// Memory usage information
    pub memory_info: Option<MemoryInfo>,
    /// Performance profiling data
    pub profile_data: Option<Vec<ProfileData>>,
}

impl SolverResult {
    /// Create a successful result.
    pub fn success(
        solution: Vec<Precision>,
        residual_norm: Precision,
        iterations: usize,
    ) -> Self {
        Self {
            solution,
            residual_norm,
            iterations,
            converged: true,
            error_bounds: None,
            stats: None,
            memory_info: None,
            profile_data: None,
        }
    }

    /// Create a failure result.
    pub fn failure(
        solution: Vec<Precision>,
        residual_norm: Precision,
        iterations: usize,
    ) -> Self {
        Self {
            solution,
            residual_norm,
            iterations,
            converged: false,
            error_bounds: None,
            stats: None,
            memory_info: None,
            profile_data: None,
        }
    }

    /// Create an error result.
    pub fn error(error: SolverError) -> Self {
        Self {
            solution: Vec::new(),
            residual_norm: Precision::INFINITY,
            iterations: 0,
            converged: false,
            error_bounds: None,
            stats: None,
            memory_info: None,
            profile_data: None,
        }
    }

    /// Check if the solution meets the specified quality criteria.
    pub fn meets_quality_criteria(&self, tolerance: Precision) -> bool {
        self.converged && self.residual_norm <= tolerance
    }
}

/// Partial solution for streaming applications.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartialSolution {
    /// Current iteration number
    pub iteration: usize,
    /// Current solution estimate
    pub solution: Vec<Precision>,
    /// Current residual norm
    pub residual_norm: Precision,
    /// Whether convergence has been achieved
    pub converged: bool,
    /// Estimated remaining iterations
    pub estimated_remaining: Option<usize>,
    /// Timestamp when this solution was computed (not serialized)
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "serde", serde(skip, default = "std::time::Instant::now"))]
    pub timestamp: std::time::Instant,
    #[cfg(not(feature = "std"))]
    pub timestamp: u64,
}

/// Core trait for all solver algorithms.
///
/// This trait defines the interface that all sublinear-time solvers must implement,
/// providing both batch and streaming solution capabilities.
pub trait SolverAlgorithm: Send + Sync {
    /// Solver-specific state type
    type State: SolverState;

    /// Initialize the solver state for a given problem.
    fn initialize(
        &self,
        matrix: &dyn Matrix,
        b: &[Precision],
        options: &SolverOptions,
    ) -> Result<Self::State>;

    /// Perform a single iteration step.
    fn step(&self, state: &mut Self::State) -> Result<StepResult>;

    /// Check if the current state meets convergence criteria.
    fn is_converged(&self, state: &Self::State) -> bool;

    /// Extract the current solution from the state.
    fn extract_solution(&self, state: &Self::State) -> Vec<Precision>;

    /// Update the right-hand side for incremental solving.
    fn update_rhs(&self, state: &mut Self::State, delta_b: &[(usize, Precision)]) -> Result<()>;

    /// Get the algorithm name for identification.
    fn algorithm_name(&self) -> &'static str;

    /// Solve the linear system Ax = b.
    ///
    /// This is the main interface for solving linear systems. It handles
    /// the iteration loop and convergence checking automatically.
    fn solve(
        &self,
        matrix: &dyn Matrix,
        b: &[Precision],
        options: &SolverOptions,
    ) -> Result<SolverResult> {
        let mut state = self.initialize(matrix, b, options)?;
        let mut iterations = 0;

        #[cfg(feature = "std")]
        let start_time = std::time::Instant::now();

        while !self.is_converged(&state) && iterations < options.max_iterations {
            match self.step(&mut state)? {
                StepResult::Continue => {
                    iterations += 1;

                    // Check for numerical issues
                    let residual = state.residual_norm();
                    if !residual.is_finite() {
                        return Err(SolverError::NumericalInstability {
                            reason: "Non-finite residual norm".to_string(),
                            iteration: iterations,
                            residual_norm: residual,
                        });
                    }
                },
                StepResult::Converged => break,
                StepResult::Failed(reason) => {
                    return Err(SolverError::AlgorithmError {
                        algorithm: self.algorithm_name().to_string(),
                        message: reason,
                        context: vec![
                            ("iteration".to_string(), iterations.to_string()),
                            ("residual_norm".to_string(), state.residual_norm().to_string()),
                        ],
                    });
                }
            }
        }

        let converged = self.is_converged(&state);
        let solution = self.extract_solution(&state);
        let residual_norm = state.residual_norm();

        // Check for convergence failure
        if !converged && iterations >= options.max_iterations {
            return Err(SolverError::ConvergenceFailure {
                iterations,
                residual_norm,
                tolerance: options.tolerance,
                algorithm: self.algorithm_name().to_string(),
            });
        }

        let mut result = if converged {
            SolverResult::success(solution, residual_norm, iterations)
        } else {
            SolverResult::failure(solution, residual_norm, iterations)
        };

        // Add optional data if requested
        if options.collect_stats {
            #[cfg(feature = "std")]
            {
                let total_time = start_time.elapsed().as_millis() as f64;
                let mut stats = SolverStats::new();
                stats.total_time_ms = total_time;
                stats.matvec_count = state.matvec_count();
                result.stats = Some(stats);
            }
        }

        if options.compute_error_bounds {
            result.error_bounds = state.error_bounds();
        }

        Ok(result)
    }
}

/// Trait for solver state management.
pub trait SolverState: Send + Sync {
    /// Get the current residual norm.
    fn residual_norm(&self) -> Precision;

    /// Get the number of matrix-vector multiplications performed.
    fn matvec_count(&self) -> usize;

    /// Get error bounds if available.
    fn error_bounds(&self) -> Option<ErrorBounds>;

    /// Get current memory usage.
    fn memory_usage(&self) -> MemoryInfo;

    /// Reset the state for a new solve.
    fn reset(&mut self);
}

/// Result of a single iteration step.
#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    /// Continue iterating
    Continue,
    /// Convergence achieved
    Converged,
    /// Algorithm failed with reason
    Failed(String),
}

/// Utility functions for solver implementations.
pub mod utils {
    use super::*;

    /// Compute the L2 norm of a vector.
    pub fn l2_norm(v: &[Precision]) -> Precision {
        v.iter().map(|x| x * x).sum::<Precision>().sqrt()
    }

    /// Compute the L1 norm of a vector.
    pub fn l1_norm(v: &[Precision]) -> Precision {
        v.iter().map(|x| x.abs()).sum()
    }

    /// Compute the L∞ norm of a vector.
    pub fn linf_norm(v: &[Precision]) -> Precision {
        v.iter().map(|x| x.abs()).fold(0.0, Precision::max)
    }

    /// Compute vector norm according to specified type.
    pub fn compute_norm(v: &[Precision], norm_type: NormType) -> Precision {
        match norm_type {
            NormType::L1 => l1_norm(v),
            NormType::L2 => l2_norm(v),
            NormType::LInfinity => linf_norm(v),
            NormType::Weighted => l2_norm(v), // Default to L2 for weighted
        }
    }

    /// Compute residual vector: r = A*x - b
    pub fn compute_residual(
        matrix: &dyn Matrix,
        x: &[Precision],
        b: &[Precision],
        residual: &mut [Precision],
    ) -> Result<()> {
        matrix.multiply_vector(x, residual)?;
        for (r, &b_val) in residual.iter_mut().zip(b.iter()) {
            *r -= b_val;
        }
        Ok(())
    }

    /// Check convergence based on specified criteria.
    pub fn check_convergence(
        residual_norm: Precision,
        tolerance: Precision,
        mode: ConvergenceMode,
        b_norm: Precision,
        prev_solution: Option<&[Precision]>,
        current_solution: &[Precision],
    ) -> bool {
        match mode {
            ConvergenceMode::ResidualNorm => residual_norm <= tolerance,
            ConvergenceMode::RelativeResidual => {
                if b_norm > 0.0 {
                    (residual_norm / b_norm) <= tolerance
                } else {
                    residual_norm <= tolerance
                }
            },
            ConvergenceMode::SolutionChange => {
                if let Some(prev) = prev_solution {
                    let mut change_norm = 0.0;
                    for (&curr, &prev_val) in current_solution.iter().zip(prev.iter()) {
                        let diff = curr - prev_val;
                        change_norm += diff * diff;
                    }
                    change_norm.sqrt() <= tolerance
                } else {
                    false
                }
            },
            ConvergenceMode::RelativeSolutionChange => {
                if let Some(prev) = prev_solution {
                    let mut change_norm = 0.0;
                    let mut solution_norm = 0.0;
                    for (&curr, &prev_val) in current_solution.iter().zip(prev.iter()) {
                        let diff = curr - prev_val;
                        change_norm += diff * diff;
                        solution_norm += prev_val * prev_val;
                    }
                    if solution_norm > 0.0 {
                        (change_norm.sqrt() / solution_norm.sqrt()) <= tolerance
                    } else {
                        change_norm.sqrt() <= tolerance
                    }
                } else {
                    false
                }
            },
            ConvergenceMode::Combined => {
                // Use the most conservative criterion
                residual_norm <= tolerance &&
                (b_norm == 0.0 || (residual_norm / b_norm) <= tolerance)
            },
        }
    }
}

// Forward declarations for solver implementations that will be added
pub struct ForwardPushSolver;
pub struct BackwardPushSolver;
pub struct HybridSolver;

// Placeholder implementations - will be implemented in separate modules
impl SolverAlgorithm for ForwardPushSolver {
    type State = ();

    fn initialize(&self, _matrix: &dyn Matrix, _b: &[Precision], _options: &SolverOptions) -> Result<Self::State> {
        Err(SolverError::AlgorithmError {
            algorithm: "forward_push".to_string(),
            message: "Not implemented yet".to_string(),
            context: vec![],
        })
    }

    fn step(&self, _state: &mut Self::State) -> Result<StepResult> {
        Err(SolverError::AlgorithmError {
            algorithm: "forward_push".to_string(),
            message: "Not implemented yet".to_string(),
            context: vec![],
        })
    }

    fn is_converged(&self, _state: &Self::State) -> bool {
        false
    }

    fn extract_solution(&self, _state: &Self::State) -> Vec<Precision> {
        Vec::new()
    }

    fn update_rhs(&self, _state: &mut Self::State, _delta_b: &[(usize, Precision)]) -> Result<()> {
        Err(SolverError::AlgorithmError {
            algorithm: "forward_push".to_string(),
            message: "Not implemented yet".to_string(),
            context: vec![],
        })
    }

    fn algorithm_name(&self) -> &'static str {
        "forward_push"
    }
}

impl SolverState for () {
    fn residual_norm(&self) -> Precision {
        0.0
    }

    fn matvec_count(&self) -> usize {
        0
    }

    fn error_bounds(&self) -> Option<ErrorBounds> {
        None
    }

    fn memory_usage(&self) -> MemoryInfo {
        MemoryInfo {
            current_usage_bytes: 0,
            peak_usage_bytes: 0,
            matrix_memory_bytes: 0,
            vector_memory_bytes: 0,
            workspace_memory_bytes: 0,
            allocation_count: 0,
            deallocation_count: 0,
        }
    }

    fn reset(&mut self) {}
}

// Similar placeholder implementations for BackwardPushSolver and HybridSolver
impl SolverAlgorithm for BackwardPushSolver {
    type State = ();
    fn initialize(&self, _matrix: &dyn Matrix, _b: &[Precision], _options: &SolverOptions) -> Result<Self::State> { Ok(()) }
    fn step(&self, _state: &mut Self::State) -> Result<StepResult> { Ok(StepResult::Converged) }
    fn is_converged(&self, _state: &Self::State) -> bool { true }
    fn extract_solution(&self, _state: &Self::State) -> Vec<Precision> { Vec::new() }
    fn update_rhs(&self, _state: &mut Self::State, _delta_b: &[(usize, Precision)]) -> Result<()> { Ok(()) }
    fn algorithm_name(&self) -> &'static str { "backward_push" }
}

impl SolverAlgorithm for HybridSolver {
    type State = ();
    fn initialize(&self, _matrix: &dyn Matrix, _b: &[Precision], _options: &SolverOptions) -> Result<Self::State> { Ok(()) }
    fn step(&self, _state: &mut Self::State) -> Result<StepResult> { Ok(StepResult::Converged) }
    fn is_converged(&self, _state: &Self::State) -> bool { true }
    fn extract_solution(&self, _state: &Self::State) -> Vec<Precision> { Vec::new() }
    fn update_rhs(&self, _state: &mut Self::State, _delta_b: &[(usize, Precision)]) -> Result<()> { Ok(()) }
    fn algorithm_name(&self) -> &'static str { "hybrid" }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::matrix::SparseMatrix;

    #[test]
    fn test_solver_options() {
        let default_opts = SolverOptions::default();
        assert_eq!(default_opts.tolerance, 1e-6);
        assert_eq!(default_opts.max_iterations, 1000);

        let fast_opts = SolverOptions::fast();
        assert_eq!(fast_opts.tolerance, 1e-3);
        assert_eq!(fast_opts.max_iterations, 100);

        let precision_opts = SolverOptions::high_precision();
        assert_eq!(precision_opts.tolerance, 1e-12);
        assert!(precision_opts.compute_error_bounds);
    }

    #[test]
    fn test_solver_result() {
        let result = SolverResult::success(vec![1.0, 2.0], 1e-8, 10);
        assert!(result.converged);
        assert!(result.meets_quality_criteria(1e-6));
        assert!(!result.meets_quality_criteria(1e-10));
    }

    #[test]
    fn test_norm_calculations() {
        use utils::*;

        let v = vec![3.0, 4.0];
        assert_eq!(l1_norm(&v), 7.0);
        assert_eq!(l2_norm(&v), 5.0);
        assert_eq!(linf_norm(&v), 4.0);
    }
}