//! Error types for AIMDS

use thiserror::Error;

/// AIMDS error types
#[derive(Error, Debug)]
pub enum AimdsError {
    #[error("Detection error: {0}")]
    Detection(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Response error: {0}")]
    Response(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Timeout error: operation timed out after {0}ms")]
    Timeout(u64),

    #[error("External service error: {service}: {message}")]
    ExternalService { service: String, message: String },

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias for AIMDS operations
pub type Result<T> = std::result::Result<T, AimdsError>;

impl AimdsError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AimdsError::Timeout(_) | AimdsError::ExternalService { .. }
        )
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AimdsError::Internal(_) => ErrorSeverity::Critical,
            AimdsError::Configuration(_) => ErrorSeverity::Critical,
            AimdsError::Detection(_) | AimdsError::Analysis(_) => ErrorSeverity::High,
            AimdsError::Timeout(_) | AimdsError::ExternalService { .. } => ErrorSeverity::Medium,
            _ => ErrorSeverity::Low,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        let timeout_err = AimdsError::Timeout(5000);
        assert!(timeout_err.is_retryable());

        let config_err = AimdsError::Configuration("Invalid config".to_string());
        assert!(!config_err.is_retryable());
    }

    #[test]
    fn test_error_severity() {
        let internal_err = AimdsError::Internal("Critical failure".to_string());
        assert_eq!(internal_err.severity(), ErrorSeverity::Critical);

        let timeout_err = AimdsError::Timeout(1000);
        assert_eq!(timeout_err.severity(), ErrorSeverity::Medium);
    }
}
