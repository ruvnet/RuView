//! Error types for Tiny Dancer

use thiserror::Error;

/// Result type for Tiny Dancer operations
pub type Result<T> = std::result::Result<T, TinyDancerError>;

/// Error types for Tiny Dancer operations
#[derive(Error, Debug)]
pub enum TinyDancerError {
    /// Model inference error
    #[error("Model inference failed: {0}")]
    InferenceError(String),

    /// Feature engineering error
    #[error("Feature engineering failed: {0}")]
    FeatureError(String),

    /// Storage error
    #[error("Storage operation failed: {0}")]
    StorageError(String),

    /// Circuit breaker error
    #[error("Circuit breaker triggered: {0}")]
    CircuitBreakerError(String),

    /// Configuration error
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<serde_json::Error> for TinyDancerError {
    fn from(err: serde_json::Error) -> Self {
        TinyDancerError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for TinyDancerError {
    fn from(err: std::io::Error) -> Self {
        TinyDancerError::StorageError(err.to_string())
    }
}
