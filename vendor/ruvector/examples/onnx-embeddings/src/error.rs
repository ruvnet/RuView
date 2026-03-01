//! Error types for ONNX embeddings

use thiserror::Error;

/// Result type alias for embedding operations
pub type Result<T> = std::result::Result<T, EmbeddingError>;

/// Errors that can occur during embedding operations
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// ONNX Runtime error
    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(#[from] ort::Error),

    /// Tokenizer error
    #[error("Tokenizer error: {0}")]
    Tokenizer(#[from] tokenizers::tokenizer::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Model not found
    #[error("Model not found: {path}")]
    ModelNotFound { path: String },

    /// Tokenizer not found
    #[error("Tokenizer not found: {path}")]
    TokenizerNotFound { path: String },

    /// Invalid model format
    #[error("Invalid model format: {reason}")]
    InvalidModel { reason: String },

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Empty input
    #[error("Empty input provided")]
    EmptyInput,

    /// Batch size exceeded
    #[error("Batch size {size} exceeds maximum {max}")]
    BatchSizeExceeded { size: usize, max: usize },

    /// Sequence too long
    #[error("Sequence length {length} exceeds maximum {max}")]
    SequenceTooLong { length: usize, max: usize },

    /// Download failed
    #[error("Failed to download model: {reason}")]
    DownloadFailed { reason: String },

    /// Cache error
    #[error("Cache error: {reason}")]
    CacheError { reason: String },

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// Invalid configuration
    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Execution provider not available
    #[error("Execution provider not available: {provider}")]
    ExecutionProviderNotAvailable { provider: String },

    /// RuVector integration error
    #[error("RuVector error: {0}")]
    RuVector(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Shape error from ndarray
    #[error("Shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),

    /// Generic error
    #[error("{0}")]
    Other(String),

    /// GPU initialization error
    #[error("GPU initialization failed: {reason}")]
    GpuInitFailed { reason: String },

    /// GPU operation error
    #[error("GPU operation failed: {operation} - {reason}")]
    GpuOperationFailed { operation: String, reason: String },

    /// Shader compilation error
    #[error("Shader compilation failed: {shader} - {reason}")]
    ShaderCompilationFailed { shader: String, reason: String },

    /// GPU buffer error
    #[error("GPU buffer error: {reason}")]
    GpuBufferError { reason: String },

    /// GPU not available
    #[error("GPU not available: {reason}")]
    GpuNotAvailable { reason: String },
}

impl EmbeddingError {
    /// Create a model not found error
    pub fn model_not_found(path: impl Into<String>) -> Self {
        Self::ModelNotFound { path: path.into() }
    }

    /// Create a tokenizer not found error
    pub fn tokenizer_not_found(path: impl Into<String>) -> Self {
        Self::TokenizerNotFound { path: path.into() }
    }

    /// Create an invalid model error
    pub fn invalid_model(reason: impl Into<String>) -> Self {
        Self::InvalidModel {
            reason: reason.into(),
        }
    }

    /// Create a dimension mismatch error
    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create a download failed error
    pub fn download_failed(reason: impl Into<String>) -> Self {
        Self::DownloadFailed {
            reason: reason.into(),
        }
    }

    /// Create a cache error
    pub fn cache_error(reason: impl Into<String>) -> Self {
        Self::CacheError {
            reason: reason.into(),
        }
    }

    /// Create an invalid config error
    pub fn invalid_config(reason: impl Into<String>) -> Self {
        Self::InvalidConfig {
            reason: reason.into(),
        }
    }

    /// Create an execution provider error
    pub fn execution_provider_not_available(provider: impl Into<String>) -> Self {
        Self::ExecutionProviderNotAvailable {
            provider: provider.into(),
        }
    }

    /// Create a RuVector error
    pub fn ruvector(msg: impl Into<String>) -> Self {
        Self::RuVector(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Create a GPU initialization error
    pub fn gpu_init_failed(reason: impl Into<String>) -> Self {
        Self::GpuInitFailed { reason: reason.into() }
    }

    /// Create a GPU operation error
    pub fn gpu_operation_failed(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::GpuOperationFailed {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Create a shader compilation error
    pub fn shader_compilation_failed(shader: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ShaderCompilationFailed {
            shader: shader.into(),
            reason: reason.into(),
        }
    }

    /// Create a GPU buffer error
    pub fn gpu_buffer_error(reason: impl Into<String>) -> Self {
        Self::GpuBufferError { reason: reason.into() }
    }

    /// Create a GPU not available error
    pub fn gpu_not_available(reason: impl Into<String>) -> Self {
        Self::GpuNotAvailable { reason: reason.into() }
    }

    /// Check if this error is a GPU error
    pub fn is_gpu_error(&self) -> bool {
        matches!(
            self,
            Self::GpuInitFailed { .. }
                | Self::GpuOperationFailed { .. }
                | Self::ShaderCompilationFailed { .. }
                | Self::GpuBufferError { .. }
                | Self::GpuNotAvailable { .. }
        )
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Http(_) | Self::DownloadFailed { .. } | Self::CacheError { .. }
        )
    }

    /// Check if this error is a configuration error
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidConfig { .. }
                | Self::InvalidModel { .. }
                | Self::DimensionMismatch { .. }
        )
    }
}
