//! Error types for the ruvector-attention crate.
//!
//! This module defines all error types that can occur during attention computation,
//! configuration, and training operations.

use thiserror::Error;

/// Errors that can occur during attention operations.
#[derive(Error, Debug, Clone)]
pub enum AttentionError {
    /// Dimension mismatch between query, key, or value tensors.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension size
        expected: usize,
        /// Actual dimension size
        actual: usize,
    },

    /// Invalid configuration parameter.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Error during attention computation.
    #[error("Computation error: {0}")]
    ComputationError(String),

    /// Memory allocation failure.
    #[error("Memory allocation failed: {0}")]
    MemoryError(String),

    /// Invalid head configuration for multi-head attention.
    #[error("Invalid head count: dimension {dim} not divisible by {num_heads} heads")]
    InvalidHeadCount {
        /// Model dimension
        dim: usize,
        /// Number of attention heads
        num_heads: usize,
    },

    /// Empty input provided.
    #[error("Empty input: {0}")]
    EmptyInput(String),

    /// Invalid edge configuration for graph attention.
    #[error("Invalid edge configuration: {0}")]
    InvalidEdges(String),

    /// Numerical instability detected.
    #[error("Numerical instability: {0}")]
    NumericalInstability(String),

    /// Invalid mask dimensions.
    #[error("Invalid mask dimensions: expected {expected}, got {actual}")]
    InvalidMask {
        /// Expected mask dimensions
        expected: String,
        /// Actual mask dimensions
        actual: String,
    },
}

/// Result type for attention operations.
pub type AttentionResult<T> = Result<T, AttentionError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AttentionError::DimensionMismatch {
            expected: 512,
            actual: 256,
        };
        assert_eq!(err.to_string(), "Dimension mismatch: expected 512, got 256");

        let err = AttentionError::InvalidConfig("dropout must be in [0, 1]".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid configuration: dropout must be in [0, 1]"
        );
    }

    #[test]
    fn test_error_clone() {
        let err = AttentionError::ComputationError("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}
