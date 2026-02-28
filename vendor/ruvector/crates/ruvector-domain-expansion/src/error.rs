//! Error types for domain expansion.

use thiserror::Error;

/// Errors that can occur during domain expansion operations.
#[derive(Error, Debug)]
pub enum DomainError {
    /// Problem generation failed.
    #[error("problem generation failed: {0}")]
    Generation(String),

    /// Solution evaluation failed.
    #[error("evaluation failed: {0}")]
    Evaluation(String),

    /// Dimension mismatch between domains.
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// Domain not found in the expansion engine.
    #[error("domain not found: {0}")]
    DomainNotFound(String),

    /// Transfer failed between domains.
    #[error("transfer failed from {source} to {target}: {reason}")]
    TransferFailed {
        source: String,
        target: String,
        reason: String,
    },

    /// Kernel has not been trained on any domain yet.
    #[error("kernel not initialized: {0}")]
    KernelNotInitialized(String),

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}
