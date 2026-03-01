//! Error types for the neural gate integration module.

use thiserror::Error;

/// Result type for neural gate operations.
pub type NeuralGateResult<T> = Result<T, NeuralGateError>;

/// Errors that can occur in neural gate operations.
#[derive(Debug, Error)]
pub enum NeuralGateError {
    /// Gate not initialized.
    #[error("neural gate not initialized")]
    NotInitialized,

    /// Invalid energy value.
    #[error("invalid energy value: {0}")]
    InvalidEnergy(f32),

    /// Hysteresis tracking error.
    #[error("hysteresis tracking error: {0}")]
    HysteresisError(String),

    /// Dendritic processing error.
    #[error("dendritic processing error: {0}")]
    DendriticError(String),

    /// Workspace broadcast error.
    #[error("workspace broadcast error: {0}")]
    WorkspaceError(String),

    /// HDC encoding error.
    #[error("HDC encoding error: {0}")]
    HdcEncodingError(String),

    /// Memory retrieval error.
    #[error("memory retrieval error: {0}")]
    MemoryError(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigurationError(String),

    /// Dimension mismatch.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },

    /// Oscillator synchronization error.
    #[error("oscillator sync error: {0}")]
    OscillatorError(String),

    /// Internal error.
    #[error("internal neural gate error: {0}")]
    Internal(String),
}

impl NeuralGateError {
    /// Create a dimension mismatch error.
    #[must_use]
    pub fn dim_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create a hysteresis error.
    #[must_use]
    pub fn hysteresis(msg: impl Into<String>) -> Self {
        Self::HysteresisError(msg.into())
    }

    /// Create a dendritic error.
    #[must_use]
    pub fn dendritic(msg: impl Into<String>) -> Self {
        Self::DendriticError(msg.into())
    }
}
