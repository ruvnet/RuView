//! Error types for hyperbolic HNSW operations

use thiserror::Error;

/// Errors that can occur during hyperbolic operations
#[derive(Error, Debug, Clone)]
pub enum HyperbolicError {
    /// Vector is outside the Poincaré ball
    #[error("Vector norm {norm} exceeds ball radius (1/sqrt(c) - eps) for curvature c={curvature}")]
    OutsideBall { norm: f32, curvature: f32 },

    /// Invalid curvature parameter
    #[error("Invalid curvature: {0}. Must be positive.")]
    InvalidCurvature(f32),

    /// Dimension mismatch between vectors
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// Numerical instability detected
    #[error("Numerical instability: {0}")]
    NumericalInstability(String),

    /// Shard not found
    #[error("Shard not found: {0}")]
    ShardNotFound(String),

    /// Index out of bounds
    #[error("Index {index} out of bounds for size {size}")]
    IndexOutOfBounds { index: usize, size: usize },

    /// Empty collection
    #[error("Cannot perform operation on empty collection")]
    EmptyCollection,

    /// Search failed
    #[error("Search failed: {0}")]
    SearchFailed(String),
}

/// Result type for hyperbolic operations
pub type HyperbolicResult<T> = Result<T, HyperbolicError>;
