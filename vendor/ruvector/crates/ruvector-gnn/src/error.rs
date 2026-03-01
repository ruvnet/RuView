//! Error types for the GNN module.

use thiserror::Error;

/// Result type alias for GNN operations.
pub type Result<T> = std::result::Result<T, GnnError>;

/// Errors that can occur during GNN operations.
#[derive(Error, Debug)]
pub enum GnnError {
    /// Tensor dimension mismatch
    #[error("Tensor dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension
        expected: String,
        /// Actual dimension
        actual: String,
    },

    /// Invalid tensor shape
    #[error("Invalid tensor shape: {0}")]
    InvalidShape(String),

    /// Layer configuration error
    #[error("Layer configuration error: {0}")]
    LayerConfig(String),

    /// Training error
    #[error("Training error: {0}")]
    Training(String),

    /// Compression error
    #[error("Compression error: {0}")]
    Compression(String),

    /// Search error
    #[error("Search error: {0}")]
    Search(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Memory mapping error
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Memory mapping error: {0}")]
    Mmap(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Core library error
    #[error("Core error: {0}")]
    Core(#[from] ruvector_core::error::RuvectorError),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl GnnError {
    /// Create a dimension mismatch error
    pub fn dimension_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::DimensionMismatch {
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create an invalid shape error
    pub fn invalid_shape(msg: impl Into<String>) -> Self {
        Self::InvalidShape(msg.into())
    }

    /// Create a layer config error
    pub fn layer_config(msg: impl Into<String>) -> Self {
        Self::LayerConfig(msg.into())
    }

    /// Create a training error
    pub fn training(msg: impl Into<String>) -> Self {
        Self::Training(msg.into())
    }

    /// Create a compression error
    pub fn compression(msg: impl Into<String>) -> Self {
        Self::Compression(msg.into())
    }

    /// Create a search error
    pub fn search(msg: impl Into<String>) -> Self {
        Self::Search(msg.into())
    }

    /// Create a memory mapping error
    #[cfg(not(target_arch = "wasm32"))]
    pub fn mmap(msg: impl Into<String>) -> Self {
        Self::Mmap(msg.into())
    }

    /// Create an invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
