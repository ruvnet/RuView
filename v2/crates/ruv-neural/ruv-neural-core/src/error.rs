//! Error types for the ruv-neural pipeline.

use thiserror::Error;

/// Top-level error type for the ruv-neural system.
#[derive(Error, Debug)]
pub enum RuvNeuralError {
    #[error("Sensor error: {0}")]
    Sensor(String),

    #[error("Signal processing error: {0}")]
    Signal(String),

    #[error("Graph construction error: {0}")]
    Graph(String),

    #[error("Mincut computation error: {0}")]
    Mincut(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Decoder error: {0}")]
    Decoder(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("Channel {channel} out of range (max {max})")]
    ChannelOutOfRange { channel: usize, max: usize },

    #[error("Insufficient data: need {needed} samples, have {have}")]
    InsufficientData { needed: usize, have: usize },
}

/// Convenience result type for the ruv-neural system.
pub type Result<T> = std::result::Result<T, RuvNeuralError>;
