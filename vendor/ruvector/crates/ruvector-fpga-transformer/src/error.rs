//! Error types for FPGA Transformer backend

use thiserror::Error;

/// Result type alias for FPGA Transformer operations
pub type Result<T> = std::result::Result<T, Error>;

/// FPGA Transformer error types
#[derive(Error, Debug)]
pub enum Error {
    /// Model artifact is invalid or corrupted
    #[error("Invalid artifact: {0}")]
    InvalidArtifact(String),

    /// Artifact signature verification failed
    #[error("Signature verification failed: {0}")]
    SignatureError(String),

    /// Test vectors failed validation
    #[error("Test vector validation failed: expected max error {expected}, got {actual}")]
    TestVectorError { expected: i32, actual: i32 },

    /// Model not found or not loaded
    #[error("Model not found: {0:?}")]
    ModelNotFound(crate::types::ModelId),

    /// Shape mismatch between request and model
    #[error("Shape mismatch: expected {expected:?}, got {actual:?}")]
    ShapeMismatch {
        expected: crate::types::FixedShape,
        actual: crate::types::FixedShape,
    },

    /// Input length does not match expected sequence length
    #[error("Input length mismatch: expected {expected}, got {actual}")]
    InputLengthMismatch { expected: usize, actual: usize },

    /// Backend communication error
    #[error("Backend error: {0}")]
    BackendError(String),

    /// Daemon connection failed
    #[error("Daemon connection failed: {0}")]
    DaemonConnectionError(String),

    /// PCIe communication error
    #[error("PCIe error: {0}")]
    PcieError(String),

    /// DMA operation failed
    #[error("DMA error: {0}")]
    DmaError(String),

    /// Gating decision blocked inference
    #[error("Inference blocked by gate: {reason:?}")]
    GateBlocked { reason: crate::types::SkipReason },

    /// Quantization error
    #[error("Quantization error: {0}")]
    QuantizationError(String),

    /// Overflow during fixed-point computation
    #[error("Fixed-point overflow at {location}")]
    FixedPointOverflow { location: &'static str },

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected:08x}, got {actual:08x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    /// Protocol version mismatch
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch { expected: u16, actual: u16 },

    /// Timeout waiting for response
    #[error("Timeout after {ms}ms")]
    Timeout { ms: u64 },

    /// Resource exhausted (memory, slots, etc.)
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Feature not available in this build
    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),
}

impl Error {
    /// Create a new InvalidArtifact error
    pub fn invalid_artifact(msg: impl Into<String>) -> Self {
        Self::InvalidArtifact(msg.into())
    }

    /// Create a new BackendError
    pub fn backend(msg: impl Into<String>) -> Self {
        Self::BackendError(msg.into())
    }

    /// Create a new DaemonConnectionError
    pub fn daemon_connection(msg: impl Into<String>) -> Self {
        Self::DaemonConnectionError(msg.into())
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Timeout { .. }
                | Error::DaemonConnectionError(_)
                | Error::BackendError(_)
                | Error::GateBlocked { .. }
        )
    }

    /// Check if this error indicates a configuration problem
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            Error::InvalidConfig(_)
                | Error::ShapeMismatch { .. }
                | Error::InputLengthMismatch { .. }
                | Error::FeatureNotAvailable(_)
        )
    }
}
