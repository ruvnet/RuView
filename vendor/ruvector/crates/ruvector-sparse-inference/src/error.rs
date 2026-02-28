//! Error types for the sparse inference engine.

use thiserror::Error;

/// Result type for sparse inference operations.
pub type Result<T> = std::result::Result<T, SparseInferenceError>;

/// Main error type for sparse inference operations.
#[derive(Debug, Error)]
pub enum SparseInferenceError {
    /// Error in predictor operations.
    #[error("Predictor error: {0}")]
    Predictor(#[from] PredictorError),

    /// Error in model operations.
    #[error("Model error: {0}")]
    Model(#[from] ModelError),

    /// Error in inference operations.
    #[error("Inference error: {0}")]
    Inference(#[from] InferenceError),

    /// Error in cache operations.
    #[error("Cache error: {0}")]
    Cache(String),

    /// Error in quantization operations.
    #[error("Quantization error: {0}")]
    Quantization(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// GGUF error.
    #[error("GGUF error: {0}")]
    Gguf(#[from] GgufError),
}

/// Errors related to predictor operations.
#[derive(Debug, Error)]
pub enum PredictorError {
    /// Invalid predictor configuration.
    #[error("Invalid predictor configuration: {0}")]
    InvalidConfig(String),

    /// Dimension mismatch between input and predictor.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Predictor not calibrated.
    #[error("Predictor not calibrated")]
    NotCalibrated,

    /// Invalid rank for low-rank approximation.
    #[error("Invalid rank: {0}")]
    InvalidRank(usize),

    /// Calibration failed.
    #[error("Calibration failed: {0}")]
    CalibrationFailed(String),
}

/// Errors related to inference operations.
#[derive(Debug, Error)]
pub enum InferenceError {
    /// Input dimension mismatch.
    #[error("Input dimension mismatch: expected {expected}, got {actual}")]
    InputDimensionMismatch { expected: usize, actual: usize },

    /// No active neurons predicted.
    #[error("No active neurons predicted")]
    NoActiveNeurons,

    /// Inference failed.
    #[error("Inference failed: {0}")]
    Failed(String),

    /// Backend error.
    #[error("Backend error: {0}")]
    Backend(String),

    /// Invalid input.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Errors related to model loading.
#[derive(Debug, Error)]
pub enum ModelError {
    /// Invalid model configuration.
    #[error("Invalid model configuration: {0}")]
    InvalidConfig(String),

    /// Dimension mismatch in model weights.
    #[error("Weight dimension mismatch: {0}")]
    WeightDimensionMismatch(String),

    /// Model not loaded.
    #[error("Model not loaded")]
    NotLoaded,

    /// Invalid activation type.
    #[error("Invalid activation type: {0}")]
    InvalidActivation(String),

    /// Failed to load model.
    #[error("Failed to load model: {0}")]
    LoadFailed(String),
}

/// Errors related to GGUF model loading.
#[derive(Debug, Error)]
pub enum GgufError {
    /// Invalid GGUF file format.
    #[error("Invalid GGUF format: {0}")]
    InvalidFormat(String),

    /// IO error during GGUF loading.
    #[error("GGUF IO error: {0}")]
    Io(String),

    /// Unsupported tensor type.
    #[error("Unsupported tensor type: {0}")]
    UnsupportedTensorType(String),

    /// Invalid tensor type code.
    #[error("Invalid tensor type: {0}")]
    InvalidTensorType(u32),

    /// Invalid magic number.
    #[error("Invalid GGUF magic number: {0:#010X}")]
    InvalidMagic(u32),

    /// Unsupported GGUF version.
    #[error("Unsupported GGUF version: {0}")]
    UnsupportedVersion(u32),

    /// Missing metadata key.
    #[error("Missing metadata: {0}")]
    MissingMetadata(String),

    /// Invalid metadata type.
    #[error("Invalid metadata type: {0}")]
    InvalidMetadataType(String),

    /// Invalid value type.
    #[error("Invalid value type: {0}")]
    InvalidValueType(u32),

    /// Tensor not found.
    #[error("Tensor not found: {0}")]
    TensorNotFound(String),
}

impl From<std::io::Error> for GgufError {
    fn from(err: std::io::Error) -> Self {
        GgufError::Io(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for GgufError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        GgufError::InvalidFormat(format!("Invalid UTF-8 string: {}", err))
    }
}

impl From<serde_json::Error> for SparseInferenceError {
    fn from(err: serde_json::Error) -> Self {
        SparseInferenceError::Serialization(err.to_string())
    }
}

impl From<String> for SparseInferenceError {
    fn from(err: String) -> Self {
        SparseInferenceError::Model(ModelError::LoadFailed(err))
    }
}
