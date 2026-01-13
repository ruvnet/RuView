//! Error types for the WiFi-DensePose system.
//!
//! This module provides comprehensive error handling using [`thiserror`] for
//! automatic `Display` and `Error` trait implementations.
//!
//! # Error Hierarchy
//!
//! - [`CoreError`]: Top-level error type that encompasses all subsystem errors
//! - [`SignalError`]: Errors related to CSI signal processing
//! - [`InferenceError`]: Errors from neural network inference
//! - [`StorageError`]: Errors from data persistence operations
//!
//! # Example
//!
//! ```rust
//! use wifi_densepose_core::error::{CoreError, SignalError};
//!
//! fn process_signal() -> Result<(), CoreError> {
//!     // Signal processing that might fail
//!     Err(SignalError::InvalidSubcarrierCount { expected: 256, actual: 128 }.into())
//! }
//! ```

use thiserror::Error;

/// A specialized `Result` type for core operations.
pub type CoreResult<T> = Result<T, CoreError>;

/// Top-level error type for the WiFi-DensePose system.
///
/// This enum encompasses all possible errors that can occur within the core
/// system, providing a unified error type for the entire crate.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CoreError {
    /// Signal processing error
    #[error("Signal processing error: {0}")]
    Signal(#[from] SignalError),

    /// Neural network inference error
    #[error("Inference error: {0}")]
    Inference(#[from] InferenceError),

    /// Data storage error
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration {
        /// Description of the configuration error
        message: String,
    },

    /// Validation error for input data
    #[error("Validation error: {message}")]
    Validation {
        /// Description of what validation failed
        message: String,
    },

    /// Resource not found
    #[error("Resource not found: {resource_type} with id '{id}'")]
    NotFound {
        /// Type of resource that was not found
        resource_type: &'static str,
        /// Identifier of the missing resource
        id: String,
    },

    /// Operation timed out
    #[error("Operation timed out after {duration_ms}ms: {operation}")]
    Timeout {
        /// The operation that timed out
        operation: String,
        /// Duration in milliseconds before timeout
        duration_ms: u64,
    },

    /// Invalid state for the requested operation
    #[error("Invalid state: expected {expected}, found {actual}")]
    InvalidState {
        /// Expected state
        expected: String,
        /// Actual state
        actual: String,
    },

    /// Internal error (should not happen in normal operation)
    #[error("Internal error: {message}")]
    Internal {
        /// Description of the internal error
        message: String,
    },
}

impl CoreError {
    /// Creates a new configuration error.
    #[must_use]
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Creates a new validation error.
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Creates a new not found error.
    #[must_use]
    pub fn not_found(resource_type: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource_type,
            id: id.into(),
        }
    }

    /// Creates a new timeout error.
    #[must_use]
    pub fn timeout(operation: impl Into<String>, duration_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            duration_ms,
        }
    }

    /// Creates a new invalid state error.
    #[must_use]
    pub fn invalid_state(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::InvalidState {
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Creates a new internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Returns `true` if this error is recoverable.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Signal(e) => e.is_recoverable(),
            Self::Inference(e) => e.is_recoverable(),
            Self::Storage(e) => e.is_recoverable(),
            Self::Timeout { .. } => true,
            Self::NotFound { .. }
            | Self::Configuration { .. }
            | Self::Validation { .. }
            | Self::InvalidState { .. }
            | Self::Internal { .. } => false,
        }
    }
}

/// Errors related to CSI signal processing.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SignalError {
    /// Invalid number of subcarriers in CSI data
    #[error("Invalid subcarrier count: expected {expected}, got {actual}")]
    InvalidSubcarrierCount {
        /// Expected number of subcarriers
        expected: usize,
        /// Actual number of subcarriers received
        actual: usize,
    },

    /// Invalid antenna configuration
    #[error("Invalid antenna configuration: {message}")]
    InvalidAntennaConfig {
        /// Description of the configuration error
        message: String,
    },

    /// Signal amplitude out of valid range
    #[error("Signal amplitude {value} out of range [{min}, {max}]")]
    AmplitudeOutOfRange {
        /// The invalid amplitude value
        value: f64,
        /// Minimum valid amplitude
        min: f64,
        /// Maximum valid amplitude
        max: f64,
    },

    /// Phase unwrapping failed
    #[error("Phase unwrapping failed: {reason}")]
    PhaseUnwrapFailed {
        /// Reason for the failure
        reason: String,
    },

    /// FFT operation failed
    #[error("FFT operation failed: {message}")]
    FftFailed {
        /// Description of the FFT error
        message: String,
    },

    /// Filter design or application error
    #[error("Filter error: {message}")]
    FilterError {
        /// Description of the filter error
        message: String,
    },

    /// Insufficient samples for processing
    #[error("Insufficient samples: need at least {required}, got {available}")]
    InsufficientSamples {
        /// Minimum required samples
        required: usize,
        /// Available samples
        available: usize,
    },

    /// Signal quality too low for reliable processing
    #[error("Signal quality too low: SNR {snr_db:.2} dB below threshold {threshold_db:.2} dB")]
    LowSignalQuality {
        /// Measured SNR in dB
        snr_db: f64,
        /// Required minimum SNR in dB
        threshold_db: f64,
    },

    /// Timestamp synchronization error
    #[error("Timestamp synchronization error: {message}")]
    TimestampSync {
        /// Description of the sync error
        message: String,
    },

    /// Invalid frequency band
    #[error("Invalid frequency band: {band}")]
    InvalidFrequencyBand {
        /// The invalid band identifier
        band: String,
    },
}

impl SignalError {
    /// Returns `true` if this error is recoverable.
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        match self {
            Self::LowSignalQuality { .. }
            | Self::InsufficientSamples { .. }
            | Self::TimestampSync { .. }
            | Self::PhaseUnwrapFailed { .. }
            | Self::FftFailed { .. } => true,
            Self::InvalidSubcarrierCount { .. }
            | Self::InvalidAntennaConfig { .. }
            | Self::AmplitudeOutOfRange { .. }
            | Self::FilterError { .. }
            | Self::InvalidFrequencyBand { .. } => false,
        }
    }
}

/// Errors related to neural network inference.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum InferenceError {
    /// Model file not found or could not be loaded
    #[error("Failed to load model from '{path}': {reason}")]
    ModelLoadFailed {
        /// Path to the model file
        path: String,
        /// Reason for the failure
        reason: String,
    },

    /// Input tensor shape mismatch
    #[error("Input shape mismatch: expected {expected:?}, got {actual:?}")]
    InputShapeMismatch {
        /// Expected tensor shape
        expected: Vec<usize>,
        /// Actual tensor shape
        actual: Vec<usize>,
    },

    /// Output tensor shape mismatch
    #[error("Output shape mismatch: expected {expected:?}, got {actual:?}")]
    OutputShapeMismatch {
        /// Expected tensor shape
        expected: Vec<usize>,
        /// Actual tensor shape
        actual: Vec<usize>,
    },

    /// CUDA/GPU error
    #[error("GPU error: {message}")]
    GpuError {
        /// Description of the GPU error
        message: String,
    },

    /// Model inference failed
    #[error("Inference failed: {message}")]
    InferenceFailed {
        /// Description of the failure
        message: String,
    },

    /// Model not initialized
    #[error("Model not initialized: {name}")]
    ModelNotInitialized {
        /// Name of the uninitialized model
        name: String,
    },

    /// Unsupported model format
    #[error("Unsupported model format: {format}")]
    UnsupportedFormat {
        /// The unsupported format
        format: String,
    },

    /// Quantization error
    #[error("Quantization error: {message}")]
    QuantizationError {
        /// Description of the quantization error
        message: String,
    },

    /// Batch size error
    #[error("Invalid batch size: {size}, maximum is {max_size}")]
    InvalidBatchSize {
        /// The invalid batch size
        size: usize,
        /// Maximum allowed batch size
        max_size: usize,
    },
}

impl InferenceError {
    /// Returns `true` if this error is recoverable.
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        match self {
            Self::GpuError { .. } | Self::InferenceFailed { .. } => true,
            Self::ModelLoadFailed { .. }
            | Self::InputShapeMismatch { .. }
            | Self::OutputShapeMismatch { .. }
            | Self::ModelNotInitialized { .. }
            | Self::UnsupportedFormat { .. }
            | Self::QuantizationError { .. }
            | Self::InvalidBatchSize { .. } => false,
        }
    }
}

/// Errors related to data storage and persistence.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// Database connection failed
    #[error("Database connection failed: {message}")]
    ConnectionFailed {
        /// Description of the connection error
        message: String,
    },

    /// Query execution failed
    #[error("Query failed: {query_type} - {message}")]
    QueryFailed {
        /// Type of query that failed
        query_type: String,
        /// Error message
        message: String,
    },

    /// Record not found
    #[error("Record not found: {table}.{id}")]
    RecordNotFound {
        /// Table name
        table: String,
        /// Record identifier
        id: String,
    },

    /// Duplicate key violation
    #[error("Duplicate key in {table}: {key}")]
    DuplicateKey {
        /// Table name
        table: String,
        /// The duplicate key
        key: String,
    },

    /// Transaction error
    #[error("Transaction error: {message}")]
    TransactionError {
        /// Description of the transaction error
        message: String,
    },

    /// Serialization/deserialization error
    #[error("Serialization error: {message}")]
    SerializationError {
        /// Description of the serialization error
        message: String,
    },

    /// Cache error
    #[error("Cache error: {message}")]
    CacheError {
        /// Description of the cache error
        message: String,
    },

    /// Migration error
    #[error("Migration error: {message}")]
    MigrationError {
        /// Description of the migration error
        message: String,
    },

    /// Storage capacity exceeded
    #[error("Storage capacity exceeded: {current} / {limit} bytes")]
    CapacityExceeded {
        /// Current storage usage
        current: u64,
        /// Storage limit
        limit: u64,
    },
}

impl StorageError {
    /// Returns `true` if this error is recoverable.
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        match self {
            Self::ConnectionFailed { .. }
            | Self::QueryFailed { .. }
            | Self::TransactionError { .. }
            | Self::CacheError { .. } => true,
            Self::RecordNotFound { .. }
            | Self::DuplicateKey { .. }
            | Self::SerializationError { .. }
            | Self::MigrationError { .. }
            | Self::CapacityExceeded { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_error_display() {
        let err = CoreError::configuration("Invalid threshold value");
        assert!(err.to_string().contains("Configuration error"));
        assert!(err.to_string().contains("Invalid threshold"));
    }

    #[test]
    fn test_signal_error_recoverable() {
        let recoverable = SignalError::LowSignalQuality {
            snr_db: 5.0,
            threshold_db: 10.0,
        };
        assert!(recoverable.is_recoverable());

        let non_recoverable = SignalError::InvalidSubcarrierCount {
            expected: 256,
            actual: 128,
        };
        assert!(!non_recoverable.is_recoverable());
    }

    #[test]
    fn test_error_conversion() {
        let signal_err = SignalError::InvalidSubcarrierCount {
            expected: 256,
            actual: 128,
        };
        let core_err: CoreError = signal_err.into();
        assert!(matches!(core_err, CoreError::Signal(_)));
    }

    #[test]
    fn test_not_found_error() {
        let err = CoreError::not_found("CsiFrame", "frame_123");
        assert!(err.to_string().contains("CsiFrame"));
        assert!(err.to_string().contains("frame_123"));
    }

    #[test]
    fn test_timeout_error() {
        let err = CoreError::timeout("inference", 5000);
        assert!(err.to_string().contains("5000ms"));
        assert!(err.to_string().contains("inference"));
    }
}
