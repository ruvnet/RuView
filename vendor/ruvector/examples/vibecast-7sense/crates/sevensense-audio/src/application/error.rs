//! Error types for the audio application layer.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during audio processing.
#[derive(Debug, Error)]
pub enum AudioError {
    /// Failed to read audio file.
    #[error("Failed to read audio file '{path}': {message}")]
    FileRead {
        path: PathBuf,
        message: String,
    },

    /// Unsupported audio format.
    #[error("Unsupported audio format: {format}")]
    UnsupportedFormat {
        format: String,
    },

    /// Resampling error.
    #[error("Resampling failed: {0}")]
    Resampling(String),

    /// Segmentation error.
    #[error("Segmentation failed: {0}")]
    Segmentation(String),

    /// Spectrogram computation error.
    #[error("Spectrogram computation failed: {0}")]
    Spectrogram(String),

    /// Invalid audio data.
    #[error("Invalid audio data: {0}")]
    InvalidData(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Repository error.
    #[error("Repository error: {0}")]
    Repository(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),
}

impl AudioError {
    /// Creates a FileRead error.
    pub fn file_read(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::FileRead {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Creates an UnsupportedFormat error.
    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            format: format.into(),
        }
    }

    /// Creates a Resampling error.
    pub fn resampling(message: impl Into<String>) -> Self {
        Self::Resampling(message.into())
    }

    /// Creates a Segmentation error.
    pub fn segmentation(message: impl Into<String>) -> Self {
        Self::Segmentation(message.into())
    }

    /// Creates a Spectrogram error.
    pub fn spectrogram(message: impl Into<String>) -> Self {
        Self::Spectrogram(message.into())
    }

    /// Creates an InvalidData error.
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::InvalidData(message.into())
    }

    /// Creates a Repository error.
    pub fn repository(message: impl Into<String>) -> Self {
        Self::Repository(message.into())
    }
}

/// Result type for audio operations.
pub type AudioResult<T> = Result<T, AudioError>;
