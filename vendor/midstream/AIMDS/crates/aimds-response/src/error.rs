//! Error types for AIMDS response layer

use thiserror::Error;

/// Result type for response operations
pub type Result<T> = std::result::Result<T, ResponseError>;

/// Errors that can occur in the response system
#[derive(Error, Debug)]
pub enum ResponseError {
    #[error("Meta-learning error: {0}")]
    MetaLearning(String),

    #[error("Mitigation failed: {0}")]
    MitigationFailed(String),

    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("Audit logging error: {0}")]
    AuditError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),

    #[error("Timeout during {operation}: {details}")]
    Timeout {
        operation: String,
        details: String,
    },

    #[error("Strange-loop error: {0}")]
    StrangeLoopError(#[from] midstreamer_strange_loop::StrangeLoopError),

    #[error("AIMDS core error: {0}")]
    CoreError(#[from] aimds_core::AimdsError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

impl ResponseError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ResponseError::Timeout { .. }
                | ResponseError::ResourceUnavailable(_)
        )
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ResponseError::MitigationFailed(_) => ErrorSeverity::Critical,
            ResponseError::RollbackFailed(_) => ErrorSeverity::Critical,
            ResponseError::MetaLearning(_) => ErrorSeverity::Warning,
            ResponseError::Timeout { .. } => ErrorSeverity::Warning,
            _ => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrorSeverity {
    Critical,
    Error,
    Warning,
    Info,
}
