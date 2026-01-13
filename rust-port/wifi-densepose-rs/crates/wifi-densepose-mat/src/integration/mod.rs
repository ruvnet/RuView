//! Integration layer (Anti-Corruption Layer) for upstream crates.
//!
//! This module provides adapters to translate between:
//! - wifi-densepose-signal types and wifi-Mat domain types
//! - wifi-densepose-nn inference results and detection results
//! - wifi-densepose-hardware interfaces and sensor abstractions

mod signal_adapter;
mod neural_adapter;
mod hardware_adapter;

pub use signal_adapter::SignalAdapter;
pub use neural_adapter::NeuralAdapter;
pub use hardware_adapter::HardwareAdapter;

/// Configuration for integration layer
#[derive(Debug, Clone, Default)]
pub struct IntegrationConfig {
    /// Use GPU acceleration if available
    pub use_gpu: bool,
    /// Batch size for neural inference
    pub batch_size: usize,
    /// Enable signal preprocessing optimizations
    pub optimize_signal: bool,
}

/// Error type for integration layer
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Signal processing error
    #[error("Signal adapter error: {0}")]
    Signal(String),

    /// Neural network error
    #[error("Neural adapter error: {0}")]
    Neural(String),

    /// Hardware error
    #[error("Hardware adapter error: {0}")]
    Hardware(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Data format error
    #[error("Data format error: {0}")]
    DataFormat(String),
}
