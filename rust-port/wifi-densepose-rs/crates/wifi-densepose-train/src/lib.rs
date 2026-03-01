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

// Note: #![forbid(unsafe_code)] is intentionally absent because the `tch`
// dependency (PyTorch Rust bindings) internally requires unsafe code via FFI.
// All *this* crate's code is written without unsafe blocks.
#![warn(missing_docs)]

pub mod config;
pub mod dataset;
pub mod domain;
pub mod error;
pub mod eval;
pub mod geometry;
pub mod rapid_adapt;
pub mod subcarrier;
pub mod virtual_aug;

// The following modules use `tch` (PyTorch Rust bindings) for GPU-accelerated
// training and are only compiled when the `tch-backend` feature is enabled.
// Without the feature the crate still provides the dataset / config / subcarrier
// APIs needed for data preprocessing and proof verification.
#[cfg(feature = "tch-backend")]
pub mod losses;
#[cfg(feature = "tch-backend")]
pub mod metrics;
#[cfg(feature = "tch-backend")]
pub mod model;
#[cfg(feature = "tch-backend")]
pub mod proof;
#[cfg(feature = "tch-backend")]
pub mod trainer;

// Convenient re-exports at the crate root.
pub use config::TrainingConfig;
pub use dataset::{CsiDataset, CsiSample, DataLoader, MmFiDataset, SyntheticCsiDataset, SyntheticConfig};
pub use error::{ConfigError, DatasetError, SubcarrierError, TrainError};
// TrainResult<T> is the generic Result alias from error.rs; the concrete
// TrainResult struct from trainer.rs is accessed via trainer::TrainResult.
pub use error::TrainResult as TrainResultAlias;
pub use subcarrier::{compute_interp_weights, interpolate_subcarriers, select_subcarriers_by_variance};

/// Crate version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
