//! Error types for graph delta operations

use std::fmt;

/// Result type for graph delta operations
pub type Result<T> = std::result::Result<T, GraphDeltaError>;

/// Errors that can occur during graph delta operations
#[derive(Debug, Clone)]
pub enum GraphDeltaError {
    /// Node not found
    NodeNotFound(String),
    /// Edge not found
    EdgeNotFound(String),
    /// Invalid operation
    InvalidOperation(String),
    /// Delta error
    DeltaError(String),
    /// Cycle detected
    CycleDetected,
    /// Constraint violation
    ConstraintViolation(String),
}

impl fmt::Display for GraphDeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            Self::EdgeNotFound(id) => write!(f, "Edge not found: {}", id),
            Self::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            Self::DeltaError(msg) => write!(f, "Delta error: {}", msg),
            Self::CycleDetected => write!(f, "Cycle detected in graph"),
            Self::ConstraintViolation(msg) => write!(f, "Constraint violation: {}", msg),
        }
    }
}

impl std::error::Error for GraphDeltaError {}
