//! Unified error types for OSpipe.

use thiserror::Error;

/// Top-level error type for all OSpipe operations.
#[derive(Error, Debug)]
pub enum OsPipeError {
    /// An error occurred during screen/audio capture processing.
    #[error("Capture error: {0}")]
    Capture(String),

    /// An error occurred in the vector storage layer.
    #[error("Storage error: {0}")]
    Storage(String),

    /// An error occurred during search operations.
    #[error("Search error: {0}")]
    Search(String),

    /// An error occurred in the ingestion pipeline.
    #[error("Pipeline error: {0}")]
    Pipeline(String),

    /// The safety gate denied ingestion of content.
    #[error("Safety gate denied: {reason}")]
    SafetyDenied {
        /// Human-readable reason for denial.
        reason: String,
    },

    /// A configuration-related error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// A JSON serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Convenience alias for `Result<T, OsPipeError>`.
pub type Result<T> = std::result::Result<T, OsPipeError>;
