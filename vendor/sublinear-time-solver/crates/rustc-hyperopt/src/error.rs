//! Error handling for RustC HyperOpt

use thiserror::Error;

/// Result type for RustC HyperOpt operations
pub type Result<T> = std::result::Result<T, OptimizerError>;

/// Errors that can occur during optimization
#[derive(Error, Debug)]
pub enum OptimizerError {
    /// IO error during cache operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Blake3 hashing error
    #[error("Hashing error: {0}")]
    Hashing(String),

    /// Cache operation error
    #[error("Cache error: {0}")]
    Cache(String),

    /// Pattern database error
    #[error("Pattern database error: {0}")]
    PatternDb(String),

    /// Performance tracking error
    #[error("Performance tracking error: {0}")]
    Performance(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}