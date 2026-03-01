//! GPU Error Types
//!
//! Error handling for GPU operations including device initialization,
//! buffer management, shader execution, and kernel dispatch.

use thiserror::Error;

/// Result type for GPU operations
pub type GpuResult<T> = Result<T, GpuError>;

/// Errors that can occur during GPU operations
#[derive(Debug, Error)]
pub enum GpuError {
    /// No suitable GPU adapter found
    #[error("No suitable GPU adapter found. Ensure a GPU with compute capabilities is available.")]
    NoAdapter,

    /// No compatible GPU device found
    #[error("No compatible GPU device found: {0}")]
    NoDevice(String),

    /// GPU device creation failed
    #[error("Failed to create GPU device: {0}")]
    DeviceCreation(String),

    /// Device request failed
    #[error("Failed to request GPU device: {0}")]
    DeviceRequestFailed(String),

    /// Shader compilation failed
    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),

    /// Buffer allocation failed
    #[error("Buffer allocation failed: {0}")]
    BufferAllocation(String),

    /// Buffer allocation failed with details
    #[error("Buffer allocation failed: requested {requested_bytes} bytes, reason: {reason}")]
    BufferAllocationFailed {
        /// Number of bytes requested
        requested_bytes: u64,
        /// Reason for failure
        reason: String,
    },

    /// Buffer size exceeds maximum allowed
    #[error("Buffer size {size} exceeds maximum allowed {max}")]
    BufferTooLarge {
        /// Requested size
        size: u64,
        /// Maximum allowed size
        max: u64,
    },

    /// Buffer size mismatch
    #[error("Buffer size mismatch: expected {expected}, got {actual}")]
    BufferSizeMismatch { expected: usize, actual: usize },

    /// Buffer read-back failed
    #[error("Buffer read-back failed: {0}")]
    BufferReadFailed(String),

    /// Buffer mapping failed
    #[error("Buffer mapping failed: {0}")]
    BufferMapFailed(String),

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Invalid binding configuration
    #[error("Invalid binding configuration: expected {expected} bindings, got {actual}")]
    InvalidBindingCount {
        /// Expected number of bindings
        expected: usize,
        /// Actual number of bindings
        actual: usize,
    },

    /// Invalid workgroup configuration
    #[error("Invalid workgroup configuration: [{x}, {y}, {z}] exceeds device limits")]
    InvalidWorkgroupSize {
        /// X dimension
        x: u32,
        /// Y dimension
        y: u32,
        /// Z dimension
        z: u32,
    },

    /// Compute pipeline creation failed
    #[error("Failed to create compute pipeline: {0}")]
    PipelineCreation(String),

    /// Command encoding failed
    #[error("Command encoding failed: {0}")]
    CommandEncoding(String),

    /// GPU execution failed
    #[error("GPU execution failed: {0}")]
    ExecutionFailed(String),

    /// Buffer read failed
    #[error("Failed to read buffer: {0}")]
    BufferRead(String),

    /// Buffer write failed
    #[error("Failed to write buffer: {0}")]
    BufferWrite(String),

    /// Timeout waiting for GPU operation
    #[error("GPU operation timed out after {0}ms")]
    Timeout(u64),

    /// Graph has no edges
    #[error("Graph has no edges to compute")]
    EmptyGraph,

    /// Invalid configuration
    #[error("Invalid GPU configuration: {0}")]
    InvalidConfig(String),

    /// Feature not supported
    #[error("GPU feature not supported: {0}")]
    UnsupportedFeature(String),

    /// Adapter request failed
    #[error("Failed to request GPU adapter: {0}")]
    AdapterRequest(String),

    /// Out of GPU memory
    #[error("Out of GPU memory: requested {requested_bytes} bytes")]
    OutOfMemory {
        /// Number of bytes requested
        requested_bytes: u64,
    },

    /// Device lost
    #[error("GPU device lost: {0}")]
    DeviceLost(String),

    /// Internal error
    #[error("Internal GPU error: {0}")]
    Internal(String),
}

impl GpuError {
    /// Check if this error indicates GPU is unavailable and fallback should be used
    pub fn should_fallback(&self) -> bool {
        matches!(
            self,
            GpuError::NoAdapter
                | GpuError::NoDevice(_)
                | GpuError::DeviceCreation(_)
                | GpuError::DeviceRequestFailed(_)
                | GpuError::AdapterRequest(_)
                | GpuError::UnsupportedFeature(_)
        )
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            GpuError::Timeout(_)
                | GpuError::BufferRead(_)
                | GpuError::BufferReadFailed(_)
                | GpuError::ExecutionFailed(_)
        )
    }
}

impl From<wgpu::RequestDeviceError> for GpuError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        Self::DeviceRequestFailed(e.to_string())
    }
}

impl From<wgpu::BufferAsyncError> for GpuError {
    fn from(e: wgpu::BufferAsyncError) -> Self {
        Self::BufferMapFailed(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_fallback() {
        assert!(GpuError::NoAdapter.should_fallback());
        assert!(GpuError::NoDevice("test".into()).should_fallback());
        assert!(GpuError::DeviceCreation("test".into()).should_fallback());
        assert!(!GpuError::Timeout(100).should_fallback());
        assert!(!GpuError::EmptyGraph.should_fallback());
    }

    #[test]
    fn test_is_recoverable() {
        assert!(GpuError::Timeout(100).is_recoverable());
        assert!(GpuError::BufferRead("test".into()).is_recoverable());
        assert!(GpuError::BufferReadFailed("test".into()).is_recoverable());
        assert!(!GpuError::NoDevice("test".into()).is_recoverable());
        assert!(!GpuError::NoAdapter.is_recoverable());
    }

    #[test]
    fn test_error_display() {
        let err = GpuError::BufferAllocationFailed {
            requested_bytes: 1024,
            reason: "out of memory".to_string(),
        };
        assert!(err.to_string().contains("1024"));
        assert!(err.to_string().contains("out of memory"));
    }

    #[test]
    fn test_workgroup_error() {
        let err = GpuError::InvalidWorkgroupSize {
            x: 1000,
            y: 1,
            z: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("1000"));
    }
}
