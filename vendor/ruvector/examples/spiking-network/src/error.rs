//! Error types for the spiking neural network library.

use thiserror::Error;

/// Result type alias for spiking network operations.
pub type Result<T> = std::result::Result<T, SpikingError>;

/// Errors that can occur in spiking neural network operations.
#[derive(Error, Debug)]
pub enum SpikingError {
    /// Invalid neuron parameters
    #[error("Invalid neuron parameters: {0}")]
    InvalidParams(String),

    /// Network topology error
    #[error("Network topology error: {0}")]
    TopologyError(String),

    /// Spike encoding error
    #[error("Spike encoding error: {0}")]
    EncodingError(String),

    /// Router error
    #[error("Router error: {0}")]
    RouterError(String),

    /// Learning error
    #[error("Learning error: {0}")]
    LearningError(String),

    /// Resource exhaustion
    #[error("Resource exhaustion: {0}")]
    ResourceExhausted(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// IO error wrapper
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
