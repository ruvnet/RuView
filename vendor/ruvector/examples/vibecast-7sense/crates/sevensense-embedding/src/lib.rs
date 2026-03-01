//! # sevensense-embedding
//!
//! Embedding bounded context for 7sense bioacoustics platform.
//!
//! This crate provides Perch 2.0 ONNX integration for generating 1536-dimensional
//! embeddings from preprocessed audio segments. It handles model loading, inference,
//! normalization, and quantization for efficient storage and retrieval.
//!
//! ## Architecture
//!
//! The crate follows Domain-Driven Design (DDD) principles:
//!
//! - **Domain Layer**: Core entities (`Embedding`, `EmbeddingModel`) and repository traits
//! - **Application Layer**: Services for embedding generation and batch processing
//! - **Infrastructure Layer**: ONNX Runtime integration and model management
//!
//! ## Usage
//!
//! ```rust,ignore
//! use sevensense_embedding::{
//!     EmbeddingService, ModelManager, ModelConfig,
//!     domain::Embedding,
//! };
//!
//! // Initialize model manager
//! let config = ModelConfig::default();
//! let model_manager = ModelManager::new(config)?;
//!
//! // Create embedding service
//! let service = EmbeddingService::new(model_manager, 8);
//!
//! // Generate embedding from spectrogram
//! let embedding = service.embed_segment(&spectrogram).await?;
//! ```
//!
//! ## Features
//!
//! - **Perch 2.0 Integration**: Full support for EfficientNet-B3 bioacoustic embeddings
//! - **Batch Processing**: Efficient batch inference with configurable batch sizes
//! - **Model Hot-Swap**: Update models without service restart
//! - **Quantization**: F16 and INT8 quantization for reduced storage
//! - **Validation**: Comprehensive embedding validation (NaN detection, dimension checks)

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod normalization;
pub mod quantization;

// Re-export main types for convenience
pub use domain::entities::{
    Embedding, EmbeddingId, EmbeddingModel, EmbeddingMetadata,
    StorageTier, ModelVersion, InputSpecification,
};
pub use domain::repository::EmbeddingRepository;
pub use application::services::EmbeddingService;
pub use infrastructure::model_manager::{ModelManager, ModelConfig};
pub use infrastructure::onnx_inference::OnnxInference;

/// Embedding dimension for Perch 2.0 model
pub const EMBEDDING_DIM: usize = 1536;

/// Target sample rate for Perch 2.0 (32kHz)
pub const TARGET_SAMPLE_RATE: u32 = 32000;

/// Target window duration in seconds (5s)
pub const TARGET_WINDOW_SECONDS: f32 = 5.0;

/// Target window samples (160,000 = 5s at 32kHz)
pub const TARGET_WINDOW_SAMPLES: usize = 160_000;

/// Mel spectrogram bins for Perch 2.0
pub const MEL_BINS: usize = 128;

/// Mel spectrogram frames for Perch 2.0
pub const MEL_FRAMES: usize = 500;

/// Crate version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Common result type for embedding operations
pub type Result<T> = std::result::Result<T, EmbeddingError>;

/// Unified error type for embedding operations
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    /// Model loading or initialization error
    #[error("Model error: {0}")]
    Model(#[from] infrastructure::model_manager::ModelError),

    /// ONNX inference error
    #[error("Inference error: {0}")]
    Inference(#[from] infrastructure::onnx_inference::InferenceError),

    /// Embedding validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// Invalid input dimensions
    #[error("Invalid dimensions: expected {expected}, got {actual}")]
    InvalidDimensions {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Repository error
    #[error("Repository error: {0}")]
    Repository(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Checksum verification failed
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum
        expected: String,
        /// Actual checksum
        actual: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(EMBEDDING_DIM, 1536);
        assert_eq!(TARGET_SAMPLE_RATE, 32000);
        assert_eq!(TARGET_WINDOW_SAMPLES, 160_000);
        assert_eq!(MEL_BINS, 128);
        assert_eq!(MEL_FRAMES, 500);
    }
}
