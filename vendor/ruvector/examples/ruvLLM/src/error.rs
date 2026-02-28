//! Error types for RuvLLM

use thiserror::Error;

/// Result type for RuvLLM operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for RuvLLM
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Memory/database error
    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),

    /// Router error
    #[error("Router error: {0}")]
    Router(#[from] RouterError),

    /// Embedding error
    #[error("Embedding error: {0}")]
    Embedding(String),

    /// Inference error
    #[error("Inference error: {0}")]
    Inference(#[from] InferenceError),

    /// Learning service error
    #[error("Learning error: {0}")]
    Learning(String),

    /// Attention computation error
    #[error("Attention error: {0}")]
    Attention(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Timeout
    #[error("Operation timed out")]
    Timeout,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Memory-specific errors
#[derive(Error, Debug)]
pub enum MemoryError {
    /// Node not found
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Edge not found
    #[error("Edge not found: {src} -> {dst}")]
    EdgeNotFound { src: String, dst: String },

    /// Index error
    #[error("Index error: {0}")]
    Index(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Capacity exceeded
    #[error("Memory capacity exceeded")]
    CapacityExceeded,
}

/// Router-specific errors
#[derive(Error, Debug)]
pub enum RouterError {
    /// Invalid feature vector
    #[error("Invalid feature vector: expected {expected} dims, got {actual}")]
    InvalidFeatures { expected: usize, actual: usize },

    /// Model not available
    #[error("Model not available: {0:?}")]
    ModelNotAvailable(crate::types::ModelSize),

    /// Weight loading error
    #[error("Failed to load weights: {0}")]
    WeightLoadError(String),

    /// Training error
    #[error("Training error: {0}")]
    TrainingError(String),
}

/// Inference-specific errors
#[derive(Error, Debug)]
pub enum InferenceError {
    /// Model loading error
    #[error("Failed to load model: {0}")]
    ModelLoadError(String),

    /// Generation error
    #[error("Generation failed: {0}")]
    GenerationError(String),

    /// Generation failed (alias)
    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    /// Initialization error
    #[error("Initialization failed: {0}")]
    InitFailed(String),

    /// Out of memory
    #[error("Out of memory for model {0:?}")]
    OutOfMemory(crate::types::ModelSize),

    /// Invalid prompt
    #[error("Invalid prompt: {0}")]
    InvalidPrompt(String),

    /// Context too long
    #[error("Context exceeds maximum length: {length} > {max}")]
    ContextTooLong { length: usize, max: usize },
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}
