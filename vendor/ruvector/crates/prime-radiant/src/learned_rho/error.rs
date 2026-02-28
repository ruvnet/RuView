//! Error types for the learned restriction map module.

use thiserror::Error;

/// Result type for learned restriction map operations.
pub type LearnedRhoResult<T> = Result<T, LearnedRhoError>;

/// Errors that can occur in learned restriction map operations.
#[derive(Debug, Error)]
pub enum LearnedRhoError {
    /// Dimension mismatch.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Training error.
    #[error("training error: {0}")]
    TrainingError(String),

    /// Forward pass error.
    #[error("forward pass error: {0}")]
    ForwardError(String),

    /// Backward pass error.
    #[error("backward pass error: {0}")]
    BackwardError(String),

    /// Consolidation error.
    #[error("consolidation error: {0}")]
    ConsolidationError(String),

    /// Replay buffer error.
    #[error("replay buffer error: {0}")]
    ReplayBufferError(String),

    /// Model not initialized.
    #[error("model not initialized")]
    NotInitialized,

    /// Numerical instability detected.
    #[error("numerical instability: {0}")]
    NumericalInstability(String),

    /// Internal error.
    #[error("internal learned rho error: {0}")]
    Internal(String),
}

impl LearnedRhoError {
    /// Create a dimension mismatch error.
    #[must_use]
    pub fn dim_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create a training error.
    #[must_use]
    pub fn training(msg: impl Into<String>) -> Self {
        Self::TrainingError(msg.into())
    }

    /// Create a numerical instability error.
    #[must_use]
    pub fn numerical(msg: impl Into<String>) -> Self {
        Self::NumericalInstability(msg.into())
    }
}
