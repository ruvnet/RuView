//! Error types for the kernel pack system
//!
//! Provides comprehensive error handling for kernel verification,
//! loading, and execution.

use std::fmt;

/// Errors that can occur during kernel execution
#[derive(Debug, Clone)]
pub enum KernelError {
    /// Execution budget exceeded (epoch deadline reached)
    EpochDeadline,

    /// Out of bounds memory access
    MemoryAccessViolation {
        /// Attempted access offset
        offset: u32,
        /// Attempted access size
        size: u32,
    },

    /// Integer overflow/underflow during computation
    IntegerOverflow,

    /// Unreachable code was executed
    Unreachable,

    /// Stack overflow in WASM execution
    StackOverflow,

    /// Indirect call type mismatch
    IndirectCallTypeMismatch,

    /// Custom trap from kernel with error code
    KernelTrap {
        /// Error code returned by kernel
        code: u32,
        /// Optional error message
        message: Option<String>,
    },

    /// Kernel not found
    KernelNotFound {
        /// Requested kernel ID
        kernel_id: String,
    },

    /// Invalid kernel parameters
    InvalidParameters {
        /// Description of the parameter error
        description: String,
    },

    /// Tensor shape mismatch
    ShapeMismatch {
        /// Expected shape description
        expected: String,
        /// Actual shape description
        actual: String,
    },

    /// Data type mismatch
    DTypeMismatch {
        /// Expected data type
        expected: String,
        /// Actual data type
        actual: String,
    },

    /// Memory allocation failed
    AllocationFailed {
        /// Requested size in bytes
        requested_bytes: usize,
    },

    /// Kernel initialization failed
    InitializationFailed {
        /// Reason for failure
        reason: String,
    },

    /// Runtime error
    RuntimeError {
        /// Error message
        message: String,
    },

    /// Feature not supported
    UnsupportedFeature {
        /// Feature name
        feature: String,
    },
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::EpochDeadline => {
                write!(f, "Kernel execution exceeded time budget (epoch deadline)")
            }
            KernelError::MemoryAccessViolation { offset, size } => {
                write!(
                    f,
                    "Memory access violation: offset={}, size={}",
                    offset, size
                )
            }
            KernelError::IntegerOverflow => write!(f, "Integer overflow during computation"),
            KernelError::Unreachable => write!(f, "Unreachable code executed"),
            KernelError::StackOverflow => write!(f, "Stack overflow"),
            KernelError::IndirectCallTypeMismatch => {
                write!(f, "Indirect call type mismatch")
            }
            KernelError::KernelTrap { code, message } => {
                write!(f, "Kernel trap (code={})", code)?;
                if let Some(msg) = message {
                    write!(f, ": {}", msg)?;
                }
                Ok(())
            }
            KernelError::KernelNotFound { kernel_id } => {
                write!(f, "Kernel not found: {}", kernel_id)
            }
            KernelError::InvalidParameters { description } => {
                write!(f, "Invalid parameters: {}", description)
            }
            KernelError::ShapeMismatch { expected, actual } => {
                write!(f, "Shape mismatch: expected {}, got {}", expected, actual)
            }
            KernelError::DTypeMismatch { expected, actual } => {
                write!(f, "DType mismatch: expected {}, got {}", expected, actual)
            }
            KernelError::AllocationFailed { requested_bytes } => {
                write!(f, "Memory allocation failed: {} bytes", requested_bytes)
            }
            KernelError::InitializationFailed { reason } => {
                write!(f, "Kernel initialization failed: {}", reason)
            }
            KernelError::RuntimeError { message } => {
                write!(f, "Runtime error: {}", message)
            }
            KernelError::UnsupportedFeature { feature } => {
                write!(f, "Unsupported feature: {}", feature)
            }
        }
    }
}

impl std::error::Error for KernelError {}

/// Errors that can occur during kernel verification
#[derive(Debug, Clone)]
pub enum VerifyError {
    /// No trusted signing key matched
    NoTrustedKey,

    /// Signature is invalid
    InvalidSignature {
        /// Description of the signature error
        reason: String,
    },

    /// Hash mismatch
    HashMismatch {
        /// Expected hash
        expected: String,
        /// Actual computed hash
        actual: String,
    },

    /// Manifest parsing failed
    InvalidManifest {
        /// Error message
        message: String,
    },

    /// Version incompatibility
    VersionIncompatible {
        /// Required version range
        required: String,
        /// Actual version
        actual: String,
    },

    /// Runtime too old for kernel pack
    RuntimeTooOld {
        /// Minimum required version
        required: String,
        /// Actual runtime version
        actual: String,
    },

    /// Runtime too new for kernel pack
    RuntimeTooNew {
        /// Maximum supported version
        max_supported: String,
        /// Actual runtime version
        actual: String,
    },

    /// Missing required WASM feature
    MissingFeature {
        /// Kernel that requires the feature
        kernel: String,
        /// Missing feature name
        feature: String,
    },

    /// Kernel not in allowlist
    NotInAllowlist {
        /// Kernel ID
        kernel_id: String,
    },

    /// File I/O error
    IoError {
        /// Error message
        message: String,
    },

    /// Key parsing error
    KeyError {
        /// Error message
        message: String,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::NoTrustedKey => {
                write!(f, "No trusted signing key matched the manifest signature")
            }
            VerifyError::InvalidSignature { reason } => {
                write!(f, "Invalid signature: {}", reason)
            }
            VerifyError::HashMismatch { expected, actual } => {
                write!(f, "Hash mismatch: expected {}, got {}", expected, actual)
            }
            VerifyError::InvalidManifest { message } => {
                write!(f, "Invalid manifest: {}", message)
            }
            VerifyError::VersionIncompatible { required, actual } => {
                write!(
                    f,
                    "Version incompatible: required {}, got {}",
                    required, actual
                )
            }
            VerifyError::RuntimeTooOld { required, actual } => {
                write!(f, "Runtime too old: requires {}, have {}", required, actual)
            }
            VerifyError::RuntimeTooNew {
                max_supported,
                actual,
            } => {
                write!(
                    f,
                    "Runtime too new: max supported {}, have {}",
                    max_supported, actual
                )
            }
            VerifyError::MissingFeature { kernel, feature } => {
                write!(
                    f,
                    "Kernel '{}' requires missing feature: {}",
                    kernel, feature
                )
            }
            VerifyError::NotInAllowlist { kernel_id } => {
                write!(f, "Kernel '{}' not in allowlist", kernel_id)
            }
            VerifyError::IoError { message } => write!(f, "I/O error: {}", message),
            VerifyError::KeyError { message } => write!(f, "Key error: {}", message),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Result type alias for kernel operations
pub type KernelResult<T> = Result<T, KernelError>;

/// Result type alias for verification operations
pub type VerifyResult<T> = Result<T, VerifyError>;

/// Standard kernel error codes (returned by kernel_forward/kernel_backward)
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelErrorCode {
    /// Success
    Ok = 0,
    /// Invalid input tensor
    InvalidInput = 1,
    /// Invalid output tensor
    InvalidOutput = 2,
    /// Invalid kernel parameters
    InvalidParams = 3,
    /// Out of memory
    OutOfMemory = 4,
    /// Operation not implemented
    NotImplemented = 5,
    /// Internal kernel error
    InternalError = 6,
}

impl From<u32> for KernelErrorCode {
    fn from(code: u32) -> Self {
        match code {
            0 => KernelErrorCode::Ok,
            1 => KernelErrorCode::InvalidInput,
            2 => KernelErrorCode::InvalidOutput,
            3 => KernelErrorCode::InvalidParams,
            4 => KernelErrorCode::OutOfMemory,
            5 => KernelErrorCode::NotImplemented,
            _ => KernelErrorCode::InternalError,
        }
    }
}

impl fmt::Display for KernelErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelErrorCode::Ok => write!(f, "OK"),
            KernelErrorCode::InvalidInput => write!(f, "Invalid input tensor"),
            KernelErrorCode::InvalidOutput => write!(f, "Invalid output tensor"),
            KernelErrorCode::InvalidParams => write!(f, "Invalid parameters"),
            KernelErrorCode::OutOfMemory => write!(f, "Out of memory"),
            KernelErrorCode::NotImplemented => write!(f, "Not implemented"),
            KernelErrorCode::InternalError => write!(f, "Internal error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_error_display() {
        let err = KernelError::EpochDeadline;
        assert!(err.to_string().contains("epoch deadline"));

        let err = KernelError::MemoryAccessViolation {
            offset: 100,
            size: 64,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("64"));
    }

    #[test]
    fn test_verify_error_display() {
        let err = VerifyError::HashMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        assert!(err.to_string().contains("abc123"));
        assert!(err.to_string().contains("def456"));
    }

    #[test]
    fn test_error_code_conversion() {
        assert_eq!(KernelErrorCode::from(0), KernelErrorCode::Ok);
        assert_eq!(KernelErrorCode::from(1), KernelErrorCode::InvalidInput);
        assert_eq!(KernelErrorCode::from(100), KernelErrorCode::InternalError);
    }
}
