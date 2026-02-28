//! Error types for the SONA tuning integration module.

use thiserror::Error;

/// Result type for SONA tuning operations.
pub type SonaTuningResult<T> = Result<T, SonaTuningError>;

/// Errors that can occur in SONA tuning operations.
#[derive(Debug, Error)]
pub enum SonaTuningError {
    /// Invalid threshold configuration.
    #[error("invalid threshold configuration: {0}")]
    InvalidThresholdConfig(String),

    /// Trajectory tracking error.
    #[error("trajectory tracking error: {0}")]
    TrajectoryError(String),

    /// Learning loop error.
    #[error("learning loop error: {0}")]
    LearningLoopError(String),

    /// Pattern not found in reasoning bank.
    #[error("pattern not found: {0}")]
    PatternNotFound(String),

    /// Consolidation error.
    #[error("knowledge consolidation error: {0}")]
    ConsolidationError(String),

    /// Dimension mismatch between input and configuration.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },

    /// Engine not initialized.
    #[error("SONA engine not initialized")]
    EngineNotInitialized,

    /// Regime tracking error.
    #[error("regime tracking error: {0}")]
    RegimeTrackingError(String),

    /// Synchronization error between learning loops.
    #[error("loop synchronization error: {0}")]
    SyncError(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigurationError(String),

    /// Internal error.
    #[error("internal SONA tuning error: {0}")]
    Internal(String),
}

impl SonaTuningError {
    /// Create a dimension mismatch error.
    #[must_use]
    pub fn dim_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create a trajectory error.
    #[must_use]
    pub fn trajectory(msg: impl Into<String>) -> Self {
        Self::TrajectoryError(msg.into())
    }

    /// Create a learning loop error.
    #[must_use]
    pub fn learning_loop(msg: impl Into<String>) -> Self {
        Self::LearningLoopError(msg.into())
    }
}
