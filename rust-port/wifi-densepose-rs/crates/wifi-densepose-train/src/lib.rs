//! # WiFi-DensePose Training Infrastructure
//!
//! This crate provides the complete training pipeline for the WiFi-DensePose pose
//! estimation model. It includes configuration management, dataset loading with
//! subcarrier interpolation, loss functions, evaluation metrics, and the training
//! loop orchestrator.
//!
//! ## Architecture
//!
//! ```text
//! TrainingConfig ──► Trainer ──► Model
//!       │               │
//!       │           DataLoader
//!       │               │
//!       │         CsiDataset (MmFiDataset | SyntheticCsiDataset)
//!       │               │
//!       │         subcarrier::interpolate_subcarriers
//!       │
//!       └──► losses / metrics
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use wifi_densepose_train::config::TrainingConfig;
//! use wifi_densepose_train::dataset::{SyntheticCsiDataset, SyntheticConfig, CsiDataset};
//!
//! // Build config
//! let config = TrainingConfig::default();
//! config.validate().expect("config is valid");
//!
//! // Create a synthetic dataset (deterministic, fixed-seed)
//! let syn_cfg = SyntheticConfig::default();
//! let dataset = SyntheticCsiDataset::new(200, syn_cfg);
//!
//! // Load one sample
//! let sample = dataset.get(0).unwrap();
//! println!("amplitude shape: {:?}", sample.amplitude.shape());
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod dataset;
pub mod error;
pub mod losses;
pub mod metrics;
pub mod model;
pub mod proof;
pub mod subcarrier;
pub mod trainer;

// Convenient re-exports at the crate root.
pub use config::{ConfigError, TrainingConfig};
pub use dataset::{CsiDataset, CsiSample, DataLoader, DatasetError, MmFiDataset, SyntheticCsiDataset, SyntheticConfig};
pub use error::{TrainError, TrainResult};
pub use subcarrier::{compute_interp_weights, interpolate_subcarriers, select_subcarriers_by_variance};

/// Crate version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
