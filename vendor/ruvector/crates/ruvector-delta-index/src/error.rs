//! Error types for delta index operations

use std::fmt;

/// Result type for index operations
pub type Result<T> = std::result::Result<T, IndexError>;

/// Errors that can occur during index operations
#[derive(Debug, Clone)]
pub enum IndexError {
    /// Dimension mismatch
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Duplicate ID
    DuplicateId(String),

    /// ID not found
    NotFound(String),

    /// Delta error
    DeltaError(String),

    /// Index is full
    IndexFull {
        /// Maximum capacity
        max: usize,
    },

    /// Invalid configuration
    InvalidConfig(String),

    /// Graph corruption detected
    GraphCorruption(String),

    /// Serialization error
    SerializationError(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::DuplicateId(id) => write!(f, "Duplicate ID: {}", id),
            Self::NotFound(id) => write!(f, "ID not found: {}", id),
            Self::DeltaError(msg) => write!(f, "Delta error: {}", msg),
            Self::IndexFull { max } => write!(f, "Index full (max {})", max),
            Self::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
            Self::GraphCorruption(msg) => write!(f, "Graph corruption: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for IndexError {}
