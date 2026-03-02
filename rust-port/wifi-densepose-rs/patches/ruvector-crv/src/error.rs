//! Error types for the CRV protocol integration.

use thiserror::Error;

/// CRV-specific errors.
#[derive(Debug, Error)]
pub enum CrvError {
    /// Dimension mismatch between expected and actual vector sizes.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Invalid CRV stage number.
    #[error("Invalid stage: {0} (must be 1-6)")]
    InvalidStage(u8),

    /// Empty input data.
    #[error("Empty input: {0}")]
    EmptyInput(String),

    /// Session not found.
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Encoding failure.
    #[error("Encoding error: {0}")]
    EncodingError(String),

    /// Attention mechanism error.
    #[error("Attention error: {0}")]
    AttentionError(#[from] ruvector_attention::AttentionError),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Result type alias for CRV operations.
pub type CrvResult<T> = Result<T, CrvError>;
