//! Error types for the WiFi-DensePose training pipeline.
//!
//! This module provides:
//!
//! - [`TrainError`]: top-level error aggregating all training failure modes.
//! - [`TrainResult`]: convenient `Result` alias using `TrainError`.
//!
//! Module-local error types live in their respective modules:
//!
//! - [`crate::config::ConfigError`]: configuration validation errors.
//! - [`crate::dataset::DatasetError`]: dataset loading/access errors.
//!
//! All are re-exported at the crate root for ergonomic use.

use thiserror::Error;
use std::path::PathBuf;

// Import module-local error types so TrainError can wrap them via #[from],
// and re-export them so `lib.rs` can forward them from `error::*`.
pub use crate::config::ConfigError;
pub use crate::dataset::DatasetError;

// ---------------------------------------------------------------------------
// Top-level training error
// ---------------------------------------------------------------------------

/// A convenient `Result` alias used throughout the training crate.
pub type TrainResult<T> = Result<T, TrainError>;

/// Top-level error type for the training pipeline.
///
/// Every orchestration-level function returns `TrainResult<T>`. Lower-level
/// functions in [`crate::config`] and [`crate::dataset`] return their own
/// module-specific error types which are automatically coerced via `#[from]`.
#[derive(Debug, Error)]
pub enum TrainError {
    /// Configuration is invalid or internally inconsistent.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// A dataset operation failed (I/O, format, missing data).
    #[error("Dataset error: {0}")]
    Dataset(#[from] DatasetError),

    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// An underlying I/O error not wrapped by Config or Dataset.
    ///
    /// Note: [`std::io::Error`] cannot be wrapped via `#[from]` here because
    /// both [`ConfigError`] and [`DatasetError`] already implement
    /// `From<std::io::Error>`. Callers should convert via those types instead.
    #[error("I/O error: {0}")]
    Io(String),

    /// An operation was attempted on an empty dataset.
    #[error("Dataset is empty")]
    EmptyDataset,

    /// Index out of bounds when accessing dataset items.
    #[error("Index {index} is out of bounds for dataset of length {len}")]
    IndexOutOfBounds {
        /// The requested index.
        index: usize,
        /// The total number of items.
        len: usize,
    },

    /// A numeric shape/dimension mismatch was detected.
    #[error("Shape mismatch: expected {expected:?}, got {actual:?}")]
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        actual: Vec<usize>,
    },

    /// A training step failed for a reason not covered above.
    #[error("Training step failed: {0}")]
    TrainingStep(String),

    /// Checkpoint could not be saved or loaded.
    #[error("Checkpoint error: {message} (path: {path:?})")]
    Checkpoint {
        /// Human-readable description.
        message: String,
        /// Path that was being accessed.
        path: PathBuf,
    },

    /// Feature not yet implemented.
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl TrainError {
    /// Create a [`TrainError::TrainingStep`] with the given message.
    pub fn training_step<S: Into<String>>(msg: S) -> Self {
        TrainError::TrainingStep(msg.into())
    }

    /// Create a [`TrainError::Checkpoint`] error.
    pub fn checkpoint<S: Into<String>>(msg: S, path: impl Into<PathBuf>) -> Self {
        TrainError::Checkpoint {
            message: msg.into(),
            path: path.into(),
        }
    }

    /// Create a [`TrainError::NotImplemented`] error.
    pub fn not_implemented<S: Into<String>>(msg: S) -> Self {
        TrainError::NotImplemented(msg.into())
    }

    /// Create a [`TrainError::ShapeMismatch`] error.
    pub fn shape_mismatch(expected: Vec<usize>, actual: Vec<usize>) -> Self {
        TrainError::ShapeMismatch { expected, actual }
    }
}

// ---------------------------------------------------------------------------
// SubcarrierError
// ---------------------------------------------------------------------------

/// Errors produced by the subcarrier resampling / interpolation functions.
///
/// These are separate from [`DatasetError`] because subcarrier operations are
/// also usable outside the dataset loading pipeline (e.g. in real-time
/// inference preprocessing).
#[derive(Debug, Error)]
pub enum SubcarrierError {
    /// The source or destination subcarrier count is zero.
    #[error("Subcarrier count must be >= 1, got {count}")]
    ZeroCount {
        /// The offending count.
        count: usize,
    },

    /// The input array's last dimension does not match the declared source count.
    #[error(
        "Subcarrier shape mismatch: last dimension is {actual_sc} \
         but `src_n` was declared as {expected_sc} (full shape: {shape:?})"
    )]
    InputShapeMismatch {
        /// Expected subcarrier count (as declared by the caller).
        expected_sc: usize,
        /// Actual last-dimension size of the input array.
        actual_sc: usize,
        /// Full shape of the input array.
        shape: Vec<usize>,
    },

    /// The requested interpolation method is not yet implemented.
    #[error("Interpolation method `{method}` is not implemented")]
    MethodNotImplemented {
        /// Human-readable name of the unsupported method.
        method: String,
    },

    /// `src_n == dst_n` — no resampling is needed.
    ///
    /// Callers should check [`TrainingConfig::needs_subcarrier_interp`] before
    /// calling the interpolation routine.
    ///
    /// [`TrainingConfig::needs_subcarrier_interp`]:
    ///     crate::config::TrainingConfig::needs_subcarrier_interp
    #[error("src_n == dst_n == {count}; no interpolation needed")]
    NopInterpolation {
        /// The equal count.
        count: usize,
    },

    /// A numerical error during interpolation (e.g. division by zero).
    #[error("Numerical error: {0}")]
    NumericalError(String),
}

impl SubcarrierError {
    /// Construct a [`SubcarrierError::NumericalError`].
    pub fn numerical<S: Into<String>>(msg: S) -> Self {
        SubcarrierError::NumericalError(msg.into())
    }
}
