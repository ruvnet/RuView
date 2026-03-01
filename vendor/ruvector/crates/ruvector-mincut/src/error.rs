//! Error types for the dynamic minimum cut algorithm

use thiserror::Error;

/// Result type for mincut operations
pub type Result<T> = std::result::Result<T, MinCutError>;

/// Errors that can occur in minimum cut operations
#[derive(Error, Debug)]
pub enum MinCutError {
    /// Graph is empty
    #[error("Graph is empty")]
    EmptyGraph,

    /// Invalid vertex ID
    #[error("Invalid vertex ID: {0}")]
    InvalidVertex(u64),

    /// Invalid edge
    #[error("Invalid edge: ({0}, {1})")]
    InvalidEdge(u64, u64),

    /// Edge already exists
    #[error("Edge already exists: ({0}, {1})")]
    EdgeExists(u64, u64),

    /// Edge not found
    #[error("Edge not found: ({0}, {1})")]
    EdgeNotFound(u64, u64),

    /// Graph is disconnected
    #[error("Graph is disconnected")]
    DisconnectedGraph,

    /// Cut size exceeds supported limit
    #[error("Cut size {0} exceeds maximum supported size {1}")]
    CutSizeExceeded(usize, usize),

    /// Invalid epsilon value for approximate algorithm
    #[error("Invalid epsilon value: {0} (must be in (0, 1])")]
    InvalidEpsilon(f64),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Monitoring callback failed
    #[error("Monitoring callback failed: {0}")]
    CallbackError(String),

    /// Internal algorithm error
    #[error("Internal algorithm error: {0}")]
    InternalError(String),

    /// Concurrent modification error
    #[error("Concurrent modification detected")]
    ConcurrentModification,

    /// Capacity exceeded
    #[error("Capacity exceeded: {0}")]
    CapacityExceeded(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

// From implementations for common error types

impl From<std::io::Error> for MinCutError {
    fn from(err: std::io::Error) -> Self {
        MinCutError::SerializationError(err.to_string())
    }
}

impl From<serde_json::Error> for MinCutError {
    fn from(err: serde_json::Error) -> Self {
        MinCutError::SerializationError(err.to_string())
    }
}

impl From<std::fmt::Error> for MinCutError {
    fn from(err: std::fmt::Error) -> Self {
        MinCutError::InternalError(err.to_string())
    }
}

impl From<String> for MinCutError {
    fn from(msg: String) -> Self {
        MinCutError::InternalError(msg)
    }
}

impl From<&str> for MinCutError {
    fn from(msg: &str) -> Self {
        MinCutError::InternalError(msg.to_string())
    }
}

// Additional utility methods for MinCutError
impl MinCutError {
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            MinCutError::InvalidVertex(_)
                | MinCutError::InvalidEdge(_, _)
                | MinCutError::EdgeNotFound(_, _)
                | MinCutError::EdgeExists(_, _)
                | MinCutError::InvalidEpsilon(_)
        )
    }

    /// Check if the error indicates a graph structure problem
    pub fn is_graph_structure_error(&self) -> bool {
        matches!(
            self,
            MinCutError::EmptyGraph
                | MinCutError::DisconnectedGraph
                | MinCutError::InvalidVertex(_)
                | MinCutError::InvalidEdge(_, _)
        )
    }

    /// Check if the error is related to capacity or resource limits
    pub fn is_resource_error(&self) -> bool {
        matches!(
            self,
            MinCutError::CutSizeExceeded(_, _) | MinCutError::CapacityExceeded(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MinCutError::InvalidVertex(42);
        assert_eq!(err.to_string(), "Invalid vertex ID: 42");

        let err = MinCutError::InvalidEdge(1, 2);
        assert_eq!(err.to_string(), "Invalid edge: (1, 2)");

        let err = MinCutError::EmptyGraph;
        assert_eq!(err.to_string(), "Graph is empty");
    }

    #[test]
    fn test_error_from_string() {
        let err: MinCutError = "test error".into();
        assert!(matches!(err, MinCutError::InternalError(_)));
        assert_eq!(err.to_string(), "Internal algorithm error: test error");
    }

    #[test]
    fn test_is_recoverable() {
        assert!(MinCutError::InvalidVertex(1).is_recoverable());
        assert!(MinCutError::EdgeNotFound(1, 2).is_recoverable());
        assert!(!MinCutError::EmptyGraph.is_recoverable());
        assert!(!MinCutError::InternalError("test".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_graph_structure_error() {
        assert!(MinCutError::EmptyGraph.is_graph_structure_error());
        assert!(MinCutError::InvalidVertex(1).is_graph_structure_error());
        assert!(MinCutError::DisconnectedGraph.is_graph_structure_error());
        assert!(!MinCutError::CallbackError("test".to_string()).is_graph_structure_error());
    }

    #[test]
    fn test_is_resource_error() {
        assert!(MinCutError::CutSizeExceeded(100, 50).is_resource_error());
        assert!(MinCutError::CapacityExceeded("test".to_string()).is_resource_error());
        assert!(!MinCutError::EmptyGraph.is_resource_error());
    }

    #[test]
    fn test_serde_json_error_conversion() {
        let json_err = serde_json::from_str::<Vec<u32>>("invalid json");
        assert!(json_err.is_err());
        let mincut_err: MinCutError = json_err.unwrap_err().into();
        assert!(matches!(mincut_err, MinCutError::SerializationError(_)));
    }
}
