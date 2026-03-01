//! Event sourcing for solver operations.
//!
//! Every solver emits [`SolverEvent`]s to an event log, enabling full
//! observability of the solve pipeline: what was requested, how many
//! iterations ran, whether convergence was reached, and whether fallback
//! algorithms were invoked.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::types::{Algorithm, ComputeLane};

/// Events emitted during a solver invocation.
///
/// Events are tagged with `#[serde(tag = "type")]` so they serialise as
/// `{ "type": "SolveRequested", ... }` for easy ingestion into event stores.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SolverEvent {
    /// A solve request was received and is about to begin.
    SolveRequested {
        /// Algorithm that will be attempted first.
        algorithm: Algorithm,
        /// Matrix dimension (number of rows).
        matrix_rows: usize,
        /// Number of non-zeros.
        matrix_nnz: usize,
        /// Compute lane.
        lane: ComputeLane,
    },

    /// One iteration of the solver completed.
    IterationCompleted {
        /// Iteration number (0-indexed).
        iteration: usize,
        /// Current residual norm.
        residual: f64,
        /// Wall time elapsed since the solve began.
        elapsed: Duration,
    },

    /// The solver converged successfully.
    SolveConverged {
        /// Algorithm that produced the result.
        algorithm: Algorithm,
        /// Total iterations executed.
        iterations: usize,
        /// Final residual norm.
        residual: f64,
        /// Total wall time.
        wall_time: Duration,
    },

    /// The solver fell back from one algorithm to another (e.g. Neumann
    /// series spectral radius too high, falling back to CG).
    AlgorithmFallback {
        /// Algorithm that failed or was deemed unsuitable.
        from: Algorithm,
        /// Algorithm that will be tried next.
        to: Algorithm,
        /// Human-readable reason for the fallback.
        reason: String,
    },

    /// The compute budget was exhausted before convergence.
    BudgetExhausted {
        /// Algorithm that was running when the budget was hit.
        algorithm: Algorithm,
        /// Which budget limit was hit.
        limit: BudgetLimit,
        /// Wall time elapsed.
        elapsed: Duration,
    },
}

/// Which budget limit was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetLimit {
    /// Wall-clock time limit.
    WallTime,
    /// Iteration count limit.
    Iterations,
    /// Memory allocation limit.
    Memory,
}
