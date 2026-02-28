//! Error types for consensus operations

use std::fmt;

/// Result type for consensus operations
pub type Result<T> = std::result::Result<T, ConsensusError>;

/// Errors that can occur during consensus
#[derive(Debug, Clone)]
pub enum ConsensusError {
    /// Delta error
    DeltaError(String),
    /// Causal ordering violation
    CausalViolation(String),
    /// Conflict resolution failed
    ConflictResolutionFailed(String),
    /// Invalid operation
    InvalidOperation(String),
    /// Network error
    NetworkError(String),
    /// Timeout
    Timeout(String),
    /// Replica not found
    ReplicaNotFound(String),
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeltaError(msg) => write!(f, "Delta error: {}", msg),
            Self::CausalViolation(msg) => write!(f, "Causal violation: {}", msg),
            Self::ConflictResolutionFailed(msg) => {
                write!(f, "Conflict resolution failed: {}", msg)
            }
            Self::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::Timeout(msg) => write!(f, "Timeout: {}", msg),
            Self::ReplicaNotFound(id) => write!(f, "Replica not found: {}", id),
        }
    }
}

impl std::error::Error for ConsensusError {}
