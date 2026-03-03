//! # Temporal Micro-Neural Network with Sublinear Solver Integration
//!
//! This crate implements a novel temporal prediction neural network system that combines
//! traditional micro-nets with sublinear solver gating for improved latency and stability
//! in short-horizon predictions.
//!
//! ## Key Features
//!
//! - **Ultra-Low Latency**: Target <0.9ms P99.9 latency on single CPU core
//! - **Mathematical Certificates**: Sublinear solver gating with error bounds
//! - **Kalman Filter Priors**: Combine physics-based priors with residual learning
//! - **Active Selection**: PageRank-based sample selection for training efficiency
//! - **INT8 Quantization**: SIMD-optimized inference with minimal accuracy loss
//! - **Dual System A/B Testing**: Compare traditional vs temporal solver approaches
//!
//! ## Quick Start
//!
//! ```rust
//! use temporal_neural_net::{
//!     models::{SystemA, SystemB},
//!     data::TimeSeriesData,
//!     training::Trainer,
//!     inference::Predictor,
//!     config::Config,
//! };
//!
//! // Load configuration
//! let config = Config::from_file("configs/B_temporal_solver.yaml")?;
//!
//! // Create data pipeline
//! let data = TimeSeriesData::from_csv("data/trajectory_data.csv")?;
//! let splits = data.temporal_split(0.7, 0.15, 0.15)?;
//!
//! // Train System B (temporal solver)
//! let mut trainer = Trainer::new(config.training);
//! let system_b = SystemB::new(config.model)?;
//! let trained_model = trainer.train(system_b, &splits.train, &splits.val)?;
//!
//! // Run inference with sub-millisecond latency
//! let predictor = Predictor::new(trained_model, config.inference)?;
//! let prediction = predictor.predict(&input_window)?;
//!
//! println!("Prediction: {:?}", prediction);
//! println!("Certificate error: {:.6}", prediction.certificate.error);
//! println!("Latency: {:.3}ms", prediction.latency_ms);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## System Architecture
//!
//! ### System A - Traditional Micro-Net
//! - Residual GRU or TCN architecture
//! - Direct end-to-end prediction
//! - Standard backpropagation training
//! - FP32 training, INT8 inference
//!
//! ### System B - Temporal Solver Net
//! - Same neural architecture as System A
//! - Kalman filter prior integration
//! - Residual learning approach (net predicts residual from prior)
//! - Sublinear solver gate for mathematical verification
//! - PageRank-based active sample selection
//!
//! ## Performance Targets
//!
//! - **Latency Budget (per tick)**:
//!   - Ingest: 0.10ms
//!   - Prior: 0.10ms
//!   - Network: 0.30ms
//!   - Gate: 0.20ms
//!   - Actuation: 0.10ms
//!   - **Total P99.9 ≤ 0.90ms**
//!
//! ## Success Criteria
//!
//! 1. System B reduces P99.9 latency by ≥20% OR
//! 2. System B reduces P99 error by ≥15% with equal latency
//! 3. Gate pass rate ≥90% with avg cert.error ≤0.02

#![warn(missing_docs, clippy::all)]
#![allow(clippy::float_cmp)] // Numerical code often requires exact comparisons

use log::info;

/// Current version of the temporal neural network system
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// System description
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

// Re-export commonly used types and modules
pub use error::{Result, TemporalNeuralError};
pub use config::Config;

// Core modules
pub mod config;
pub mod error;
pub mod models;
pub mod solvers;
pub mod data;
pub mod training;
pub mod inference;
pub mod utils;

// Optional modules
#[cfg(feature = "benchmarks")]
pub mod benchmarks;

// WASM module (when targeting WASM)
#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-exports for convenience
pub mod prelude {
    //! Convenient re-exports for common usage patterns

    pub use crate::{
        config::{Config, ModelConfig, TrainingConfig, InferenceConfig},
        models::{SystemA, SystemB, ModelTrait},
        data::{TimeSeriesData, DataSplits, WindowedSample},
        training::{Trainer, TrainingResult},
        inference::{Predictor, Prediction, Certificate},
        error::{Result, TemporalNeuralError},
    };

    // Re-export sublinear solver types
    pub use ::sublinear::{
        SolverAlgorithm, SolverOptions, SolverResult,
        NeumannSolver, Precision,
    };

    // Re-export common external types
    pub use nalgebra::{DVector, DMatrix};
    pub use rand::Rng;
}

/// Initialize the temporal neural network library
///
/// This function should be called once at the start of your application
/// to set up proper logging and initialize any global state.
pub fn init() -> Result<()> {
    #[cfg(feature = "std")]
    env_logger::try_init().map_err(|e| {
        TemporalNeuralError::InitializationError {
            reason: format!("Failed to initialize logger: {}", e),
        }
    })?;

    info!("Temporal Neural Network v{} initialized", VERSION);
    info!("Features: {}", get_enabled_features().join(", "));

    Ok(())
}

/// Get list of enabled features for this build
pub fn get_enabled_features() -> Vec<&'static str> {
    let mut features = vec!["std"];

    #[cfg(feature = "plots")]
    features.push("plots");

    #[cfg(feature = "huggingface")]
    features.push("huggingface");

    #[cfg(feature = "onnx")]
    features.push("onnx");

    #[cfg(feature = "benchmarks")]
    features.push("benchmarks");

    features
}

/// Build information for debugging and compatibility checks
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildInfo {
    /// Library version
    pub version: &'static str,
    /// Enabled feature flags
    pub features: Vec<&'static str>,
    /// Whether SIMD optimizations are available
    pub simd_support: bool,
    /// Target architecture
    pub target_arch: &'static str,
    /// Build timestamp
    pub build_timestamp: &'static str,
}

/// Get comprehensive build information
pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: VERSION,
        features: get_enabled_features(),
        simd_support: false, // TODO: Check SIMD support properly
        target_arch: std::env::consts::ARCH,
        build_timestamp: option_env!("VERGEN_BUILD_TIMESTAMP").unwrap_or("unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        // Should not panic and should be idempotent
        let _ = init();
        let _ = init();
    }

    #[test]
    fn test_build_info() {
        let info = build_info();
        assert_eq!(info.version, VERSION);
        assert!(!info.features.is_empty());
        assert!(info.features.contains(&"std"));
    }

    #[test]
    fn test_version_info() {
        assert!(!VERSION.is_empty());
        assert!(!DESCRIPTION.is_empty());
    }
}