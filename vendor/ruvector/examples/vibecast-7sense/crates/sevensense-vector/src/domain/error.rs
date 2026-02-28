//! Error types for the Vector Space bounded context.

use std::path::PathBuf;
use thiserror::Error;

use super::entities::{ConfigValidationError, EmbeddingId};

/// Main error type for vector operations.
#[derive(Debug, Error)]
pub enum VectorError {
    /// Dimension mismatch between vector and index.
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Expected dimensions from index configuration.
        expected: usize,
        /// Actual dimensions of the provided vector.
        got: usize,
    },

    /// Vector with this ID already exists.
    #[error("Vector with ID {0} already exists")]
    DuplicateId(EmbeddingId),

    /// Vector with this ID was not found.
    #[error("Vector with ID {0} not found")]
    NotFound(EmbeddingId),

    /// Index capacity exceeded.
    #[error("Index capacity exceeded: max {max}, current {current}")]
    CapacityExceeded {
        /// Maximum capacity.
        max: usize,
        /// Current size.
        current: usize,
    },

    /// Invalid vector data (e.g., contains NaN or Inf).
    #[error("Invalid vector data: {0}")]
    InvalidVector(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigValidationError),

    /// Index is empty.
    #[error("Index is empty")]
    EmptyIndex,

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// IO error during persistence.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// File not found.
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Corrupted index file.
    #[error("Corrupted index file: {0}")]
    CorruptedFile(String),

    /// Lock acquisition failed.
    #[error("Failed to acquire lock: {0}")]
    LockError(String),

    /// Operation timeout.
    #[error("Operation timed out after {0}ms")]
    Timeout(u64),

    /// Index not initialized.
    #[error("Index not initialized")]
    NotInitialized,

    /// Concurrent modification detected.
    #[error("Concurrent modification detected")]
    ConcurrentModification,

    /// Graph operation error.
    #[error("Graph error: {0}")]
    GraphError(String),

    /// Search parameters invalid.
    #[error("Invalid search parameters: {0}")]
    InvalidSearchParams(String),

    /// Internal error (should not happen in normal operation).
    #[error("Internal error: {0}")]
    Internal(String),
}

impl VectorError {
    /// Create a dimension mismatch error.
    pub fn dimension_mismatch(expected: usize, got: usize) -> Self {
        Self::DimensionMismatch { expected, got }
    }

    /// Create a capacity exceeded error.
    pub fn capacity_exceeded(max: usize, current: usize) -> Self {
        Self::CapacityExceeded { max, current }
    }

    /// Create an invalid vector error.
    pub fn invalid_vector(msg: impl Into<String>) -> Self {
        Self::InvalidVector(msg.into())
    }

    /// Create a serialization error.
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::SerializationError(msg.into())
    }

    /// Create a corrupted file error.
    pub fn corrupted(msg: impl Into<String>) -> Self {
        Self::CorruptedFile(msg.into())
    }

    /// Create a lock error.
    pub fn lock(msg: impl Into<String>) -> Self {
        Self::LockError(msg.into())
    }

    /// Create a graph error.
    pub fn graph(msg: impl Into<String>) -> Self {
        Self::GraphError(msg.into())
    }

    /// Create an invalid search params error.
    pub fn invalid_search(msg: impl Into<String>) -> Self {
        Self::InvalidSearchParams(msg.into())
    }

    /// Create an internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Check if this is a retriable error.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::LockError(_) | Self::Timeout(_) | Self::ConcurrentModification
        )
    }

    /// Check if this is a not-found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_) | Self::FileNotFound(_))
    }
}

impl From<bincode::Error> for VectorError {
    fn from(e: bincode::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}

impl From<serde_json::Error> for VectorError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = VectorError::dimension_mismatch(1536, 768);
        assert!(err.to_string().contains("1536"));
        assert!(err.to_string().contains("768"));

        let err = VectorError::NotFound(EmbeddingId::new());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_is_retriable() {
        assert!(VectorError::lock("test").is_retriable());
        assert!(VectorError::Timeout(1000).is_retriable());
        assert!(!VectorError::EmptyIndex.is_retriable());
    }

    #[test]
    fn test_is_not_found() {
        assert!(VectorError::NotFound(EmbeddingId::new()).is_not_found());
        assert!(VectorError::FileNotFound(PathBuf::from("/test")).is_not_found());
        assert!(!VectorError::EmptyIndex.is_not_found());
    }
}
