//! Error types for the neural network crate.

use thiserror::Error;

/// Result type alias for neural network operations
pub type NnResult<T> = Result<T, NnError>;

/// Neural network errors
#[derive(Error, Debug)]
pub enum NnError {
    /// Configuration validation error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Model loading error
    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    /// Inference error
    #[error("Inference failed: {0}")]
    Inference(String),

    /// Shape mismatch error
    #[error("Shape mismatch: expected {expected:?}, got {actual:?}")]
    ShapeMismatch {
        /// Expected shape
        expected: Vec<usize>,
        /// Actual shape
        actual: Vec<usize>,
    },

    /// Invalid input error
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Backend not available
    #[error("Backend not available: {0}")]
    BackendUnavailable(String),

    /// ONNX Runtime error
    #[cfg(feature = "onnx")]
    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(#[from] ort::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Tensor operation error
    #[error("Tensor operation error: {0}")]
    TensorOp(String),

    /// Unsupported operation
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
}

impl NnError {
    /// Create a configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        NnError::Config(msg.into())
    }

    /// Create a model load error
    pub fn model_load<S: Into<String>>(msg: S) -> Self {
        NnError::ModelLoad(msg.into())
    }

    /// Create an inference error
    pub fn inference<S: Into<String>>(msg: S) -> Self {
        NnError::Inference(msg.into())
    }

    /// Create a shape mismatch error
    pub fn shape_mismatch(expected: Vec<usize>, actual: Vec<usize>) -> Self {
        NnError::ShapeMismatch { expected, actual }
    }

    /// Create an invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        NnError::InvalidInput(msg.into())
    }

    /// Create a tensor operation error
    pub fn tensor_op<S: Into<String>>(msg: S) -> Self {
        NnError::TensorOp(msg.into())
    }
}
