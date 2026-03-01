//! HuggingFace Hub integration for RuvLTRA model management
//!
//! This module provides comprehensive HuggingFace Hub integration for publishing,
//! downloading, and managing RuvLTRA models. It supports:
//!
//! - **Model Upload**: Push GGUF files and SONA weights to HF Hub
//! - **Model Download**: Pull models with automatic quantization selection
//! - **Model Registry**: Pre-configured RuvLTRA model collection
//! - **Progress Tracking**: Visual progress bars with resume support
//! - **Integrity Verification**: Checksum validation for downloads
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvllm::hub::{RuvLtraRegistry, ModelDownloader};
//!
//! // Download a model
//! let registry = RuvLtraRegistry::new();
//! let model_info = registry.get("ruvltra-small")?;
//! let downloader = ModelDownloader::new();
//! let path = downloader.download(model_info, None).await?;
//!
//! // Upload a model
//! let uploader = ModelUploader::new("hf_token_here");
//! uploader.upload(
//!     "./my-model.gguf",
//!     "username/my-ruvltra",
//!     Some("My custom RuvLTRA model"),
//! ).await?;
//! ```

pub mod download;
pub mod model_card;
pub mod progress;
pub mod registry;
pub mod upload;

// Re-exports
pub use download::{
    ChecksumVerifier, DownloadConfig, DownloadError, DownloadProgress, ModelDownloader,
};
pub use model_card::{
    DatasetInfo, Framework, License, MetricResult, ModelCard, ModelCardBuilder, TaskType,
};
pub use progress::{
    MultiProgress, ProgressBar, ProgressCallback, ProgressIndicator, ProgressStyle,
};
pub use registry::{
    get_model_info, HardwareRequirements, ModelInfo, ModelSize, QuantizationLevel, RuvLtraRegistry,
};
pub use upload::{ModelMetadata, ModelUploader, UploadConfig, UploadError, UploadProgress};

use std::path::PathBuf;

/// Result type for hub operations
pub type Result<T> = std::result::Result<T, HubError>;

/// Hub operation errors
#[derive(Debug, thiserror::Error)]
pub enum HubError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP error
    #[cfg(feature = "async-runtime")]
    #[error("HTTP error: {0}")]
    Http(String),

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Model not found
    #[error("Model not found: {0}")]
    NotFound(String),

    /// Checksum mismatch
    #[error("Checksum verification failed: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// Invalid model format
    #[error("Invalid model format: {0}")]
    InvalidFormat(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded. Retry after {0} seconds")]
    RateLimit(u64),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Parse error
    #[error("Parse error: {0}")]
    Parse(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Default HuggingFace Hub API endpoint
pub const HF_ENDPOINT: &str = "https://huggingface.co";

/// Default cache directory for downloaded models
pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("huggingface")
        .join("ruvltra")
}

/// Get HuggingFace token from environment
pub fn get_hf_token() -> Option<String> {
    std::env::var("HF_TOKEN")
        .or_else(|_| std::env::var("HUGGING_FACE_HUB_TOKEN"))
        .or_else(|_| std::env::var("HUGGINGFACE_API_KEY"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_cache_dir() {
        let cache_dir = default_cache_dir();
        assert!(cache_dir.to_string_lossy().contains("huggingface"));
        assert!(cache_dir.to_string_lossy().contains("ruvltra"));
    }

    #[test]
    fn test_error_display() {
        let err = HubError::NotFound("model-123".to_string());
        assert_eq!(err.to_string(), "Model not found: model-123");

        let err = HubError::ChecksumMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        assert!(err.to_string().contains("abc123"));
        assert!(err.to_string().contains("def456"));
    }
}
