//! Error types for the solver crate.
//!
//! Provides structured error variants for convergence failures, numerical
//! instabilities, budget overruns, and invalid inputs. All errors implement
//! `std::error::Error` via `thiserror`.

use std::time::Duration;

use crate::types::Algorithm;

/// Primary error type for solver operations.
#[derive(Debug, thiserror::Error)]
pub enum SolverError {
    /// The iterative solver did not converge within the allowed iteration budget.
    #[error(
        "solver did not converge after {iterations} iterations (residual={residual:.2e}, tol={tolerance:.2e})"
    )]
    NonConvergence {
        /// Number of iterations completed before the budget was exhausted.
        iterations: usize,
        /// Final residual norm at termination.
        residual: f64,
        /// Target tolerance that was not reached.
        tolerance: f64,
    },

    /// A numerical instability was detected (NaN, Inf, or loss of precision).
    #[error("numerical instability at iteration {iteration}: {detail}")]
    NumericalInstability {
        /// Iteration at which the instability was detected.
        iteration: usize,
        /// Human-readable explanation.
        detail: String,
    },

    /// The compute budget (wall-time, iterations, or memory) was exhausted.
    #[error("compute budget exhausted: {reason}")]
    BudgetExhausted {
        /// Which budget limit was hit.
        reason: String,
        /// Wall-clock time elapsed before the budget was hit.
        elapsed: Duration,
    },

    /// The caller supplied invalid input (dimensions, parameters, etc.).
    #[error("invalid input: {0}")]
    InvalidInput(#[from] ValidationError),

    /// The matrix spectral radius exceeds the threshold required by the algorithm.
    #[error(
        "spectral radius {spectral_radius:.4} exceeds limit {limit:.4} for algorithm {algorithm}"
    )]
    SpectralRadiusExceeded {
        /// Estimated spectral radius of the iteration matrix.
        spectral_radius: f64,
        /// Maximum spectral radius the algorithm tolerates.
        limit: f64,
        /// Algorithm that detected the violation.
        algorithm: Algorithm,
    },

    /// A backend-specific error (e.g. nalgebra or BLAS).
    #[error("backend error: {0}")]
    BackendError(String),
}

/// Validation errors for solver inputs.
///
/// These are raised eagerly before any computation begins so that callers get
/// clear diagnostics rather than mysterious numerical failures.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    /// Matrix dimensions are inconsistent (e.g. row_ptrs length vs rows).
    #[error("dimension mismatch: {0}")]
    DimensionMismatch(String),

    /// A value is NaN or infinite where a finite number is required.
    #[error("non-finite value detected: {0}")]
    NonFiniteValue(String),

    /// A column index is out of bounds for the declared number of columns.
    #[error("column index {index} out of bounds for {cols} columns (row {row})")]
    IndexOutOfBounds {
        /// Offending column index.
        index: u32,
        /// Row containing the offending entry.
        row: usize,
        /// Declared column count.
        cols: usize,
    },

    /// The `row_ptrs` array is not monotonically non-decreasing.
    #[error("row_ptrs is not monotonically non-decreasing at position {position}")]
    NonMonotonicRowPtrs {
        /// Position in `row_ptrs` where the violation was detected.
        position: usize,
    },

    /// A parameter is outside its valid range.
    #[error("parameter out of range: {name} = {value} (expected {expected})")]
    ParameterOutOfRange {
        /// Name of the parameter.
        name: String,
        /// The invalid value (as a string for flexibility).
        value: String,
        /// Human-readable description of the valid range.
        expected: String,
    },

    /// Matrix size exceeds the implementation limit.
    #[error("matrix size {rows}x{cols} exceeds maximum supported {max_dim}x{max_dim}")]
    MatrixTooLarge {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
        /// Maximum supported dimension.
        max_dim: usize,
    },
}
