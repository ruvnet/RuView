//! Error types for delta operations

use alloc::string::String;
use core::fmt;

/// Result type for delta operations
pub type Result<T> = core::result::Result<T, DeltaError>;

/// Errors that can occur during delta operations
#[derive(Debug, Clone)]
pub enum DeltaError {
    /// Dimension mismatch between vectors
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Invalid delta encoding
    InvalidEncoding(String),

    /// Compression error
    CompressionError(String),

    /// Decompression error
    DecompressionError(String),

    /// Stream error
    StreamError(String),

    /// Window error
    WindowError(String),

    /// Serialization error
    SerializationError(String),

    /// Index out of bounds
    IndexOutOfBounds {
        /// The index that was accessed
        index: usize,
        /// The length of the collection
        length: usize,
    },

    /// Invalid operation
    InvalidOperation(String),

    /// Buffer overflow
    BufferOverflow {
        /// Required capacity
        required: usize,
        /// Available capacity
        available: usize,
    },

    /// Checksum mismatch
    ChecksumMismatch {
        /// Expected checksum
        expected: u64,
        /// Actual checksum
        actual: u64,
    },

    /// Version incompatibility
    VersionMismatch {
        /// Expected version
        expected: u32,
        /// Actual version
        actual: u32,
    },
}

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::InvalidEncoding(msg) => write!(f, "Invalid encoding: {}", msg),
            Self::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            Self::DecompressionError(msg) => write!(f, "Decompression error: {}", msg),
            Self::StreamError(msg) => write!(f, "Stream error: {}", msg),
            Self::WindowError(msg) => write!(f, "Window error: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Self::IndexOutOfBounds { index, length } => {
                write!(f, "Index out of bounds: {} (length: {})", index, length)
            }
            Self::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            Self::BufferOverflow {
                required,
                available,
            } => {
                write!(
                    f,
                    "Buffer overflow: required {}, available {}",
                    required, available
                )
            }
            Self::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "Checksum mismatch: expected {:016x}, got {:016x}",
                    expected, actual
                )
            }
            Self::VersionMismatch { expected, actual } => {
                write!(f, "Version mismatch: expected {}, got {}", expected, actual)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DeltaError {}
