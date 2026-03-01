//! Error types for EXO-AI core

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for substrate operations
#[derive(Debug, Error)]
pub enum Error {
    /// Backend error
    #[error("Backend error: {0}")]
    Backend(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid query
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),
}
