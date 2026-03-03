//! Error types for the detection layer

use thiserror::Error;

/// Result type alias for detection operations
pub type Result<T> = std::result::Result<T, DetectionError>;

/// Error types for detection operations
#[derive(Error, Debug)]
pub enum DetectionError {
    /// Pattern matching error
    #[error("Pattern matching failed: {0}")]
    PatternMatching(String),

    /// Sanitization error
    #[error("Input sanitization failed: {0}")]
    Sanitization(String),

    /// Scheduling error
    #[error("Threat scheduling failed: {0}")]
    Scheduling(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Input too large
    #[error("Input exceeds maximum length of {max} bytes (got {actual})")]
    InputTooLarge { max: usize, actual: usize },

    /// Invalid encoding
    #[error("Invalid UTF-8 encoding: {0}")]
    InvalidEncoding(String),

    /// Temporal comparison error
    #[error("Temporal comparison error: {0}")]
    TemporalCompare(String),

    /// Generic error
    #[error("Detection error: {0}")]
    Generic(String),
}

impl From<anyhow::Error> for DetectionError {
    fn from(err: anyhow::Error) -> Self {
        DetectionError::Generic(err.to_string())
    }
}
