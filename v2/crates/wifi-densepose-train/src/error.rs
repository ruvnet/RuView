//! Error types for the WiFi-DensePose training pipeline.
//!
//! This module is the single source of truth for all error types in the
//! training crate. Every module that produces an error imports its error type
//! from here rather than defining it inline, keeping the error hierarchy
//! centralised and consistent.
//!
//! ## Hierarchy
//!
//! ```text
//! TrainError (top-level)
//! ├── ConfigError      (config validation / file loading)
//! ├── DatasetError     (data loading, I/O, format)
//! └── SubcarrierError  (frequency-axis resampling)
//! ```

use thiserror::Error;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// TrainResult
// ---------------------------------------------------------------------------

/// Convenient `Result` alias used by orchestration-level functions.
pub type TrainResult<T> = Result<T, TrainError>;

// ---------------------------------------------------------------------------
// TrainError — top-level aggregator
// ---------------------------------------------------------------------------

/// Top-level error type for the WiFi-DensePose training pipeline.
///
/// Orchestration-level functions (e.g. [`crate::trainer::Trainer`] methods)
/// return `TrainResult<T>`. Lower-level functions in [`crate::config`] and
/// [`crate::dataset`] return their own module-specific error types which are
/// automatically coerced into `TrainError` via [`From`].
#[derive(Debug, Error)]
pub enum TrainError {
    /// A configuration validation or loading error.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// A dataset loading or access error.
    #[error("Dataset error: {0}")]
    Dataset(#[from] DatasetError),

    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The dataset is empty and no training can be performed.
    #[error("Dataset is empty")]
    EmptyDataset,

    /// Index out of bounds when accessing dataset items.
    #[error("Index {index} is out of bounds for dataset of length {len}")]
    IndexOutOfBounds {
        /// The out-of-range index.
        index: usize,
        /// The total number of items in the dataset.
        len: usize,
    },

    /// A shape mismatch was detected between two tensors.
    #[error("Shape mismatch: expected {expected:?}, got {actual:?}")]
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        actual: Vec<usize>,
    },

    /// A training step failed.
    #[error("Training step failed: {0}")]
    TrainingStep(String),

    /// A checkpoint could not be saved or loaded.
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
    /// Construct a [`TrainError::TrainingStep`].
    pub fn training_step<S: Into<String>>(msg: S) -> Self {
        TrainError::TrainingStep(msg.into())
    }

    /// Construct a [`TrainError::Checkpoint`].
    pub fn checkpoint<S: Into<String>>(msg: S, path: impl Into<PathBuf>) -> Self {
        TrainError::Checkpoint { message: msg.into(), path: path.into() }
    }

    /// Construct a [`TrainError::NotImplemented`].
    pub fn not_implemented<S: Into<String>>(msg: S) -> Self {
        TrainError::NotImplemented(msg.into())
    }

    /// Construct a [`TrainError::ShapeMismatch`].
    pub fn shape_mismatch(expected: Vec<usize>, actual: Vec<usize>) -> Self {
        TrainError::ShapeMismatch { expected, actual }
    }
}

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

/// Errors produced when loading or validating a [`TrainingConfig`].
///
/// [`TrainingConfig`]: crate::config::TrainingConfig
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A field has an invalid value.
    #[error("Invalid value for `{field}`: {reason}")]
    InvalidValue {
        /// Name of the field.
        field: &'static str,
        /// Human-readable reason.
        reason: String,
    },

    /// A configuration file could not be read from disk.
    #[error("Cannot read config file `{path}`: {source}")]
    FileRead {
        /// Path that was being read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A configuration file contains malformed JSON.
    #[error("Cannot parse config file `{path}`: {source}")]
    ParseError {
        /// Path that was being parsed.
        path: PathBuf,
        /// Underlying JSON parse error.
        #[source]
        source: serde_json::Error,
    },

    /// A path referenced in the config does not exist.
    #[error("Path `{path}` in config does not exist")]
    PathNotFound {
        /// The missing path.
        path: PathBuf,
    },
}

impl ConfigError {
    /// Construct a [`ConfigError::InvalidValue`].
    pub fn invalid_value<S: Into<String>>(field: &'static str, reason: S) -> Self {
        ConfigError::InvalidValue { field, reason: reason.into() }
    }
}

// ---------------------------------------------------------------------------
// DatasetError
// ---------------------------------------------------------------------------

/// Errors produced while loading or accessing dataset samples.
///
/// Production training code MUST NOT silently suppress these errors.
/// If data is missing, training must fail explicitly so the user is aware.
/// The [`SyntheticCsiDataset`] is the only source of non-file-system data
/// and is restricted to proof/testing use.
///
/// [`SyntheticCsiDataset`]: crate::dataset::SyntheticCsiDataset
#[derive(Debug, Error)]
pub enum DatasetError {
    /// A required data file or directory was not found on disk.
    #[error("Data not found at `{path}`: {message}")]
    DataNotFound {
        /// Path that was expected to contain data.
        path: PathBuf,
        /// Additional context.
        message: String,
    },

    /// A file was found but its format or shape is wrong.
    #[error("Invalid data format in `{path}`: {message}")]
    InvalidFormat {
        /// Path of the malformed file.
        path: PathBuf,
        /// Description of the problem.
        message: String,
    },

    /// A low-level I/O error while reading a data file.
    #[error("I/O error reading `{path}`: {source}")]
    IoError {
        /// Path being read when the error occurred.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The number of subcarriers in the file doesn't match expectations.
    #[error(
        "Subcarrier count mismatch in `{path}`: file has {found}, expected {expected}"
    )]
    SubcarrierMismatch {
        /// Path of the offending file.
        path: PathBuf,
        /// Subcarrier count found in the file.
        found: usize,
        /// Subcarrier count expected.
        expected: usize,
    },

    /// A sample index is out of bounds.
    #[error("Index {idx} out of bounds (dataset has {len} samples)")]
    IndexOutOfBounds {
        /// The requested index.
        idx: usize,
        /// Total length of the dataset.
        len: usize,
    },

    /// A numpy array file could not be parsed.
    #[error("NumPy read error in `{path}`: {message}")]
    NpyReadError {
        /// Path of the `.npy` file.
        path: PathBuf,
        /// Error description.
        message: String,
    },

    /// Metadata for a subject is missing or malformed.
    #[error("Metadata error for subject {subject_id}: {message}")]
    MetadataError {
        /// Subject whose metadata was invalid.
        subject_id: u32,
        /// Description of the problem.
        message: String,
    },

    /// A data format error (e.g. wrong numpy shape) occurred.
    ///
    /// This is a convenience variant for short-form error messages where
    /// the full path context is not available.
    #[error("File format error: {0}")]
    Format(String),

    /// The data directory does not exist.
    #[error("Directory not found: {path}")]
    DirectoryNotFound {
        /// The path that was not found.
        path: String,
    },

    /// No subjects matching the requested IDs were found.
    #[error(
        "No subjects found in `{data_dir}` for IDs: {requested:?}"
    )]
    NoSubjectsFound {
        /// Root data directory.
        data_dir: PathBuf,
        /// IDs that were requested.
        requested: Vec<u32>,
    },

    /// An I/O error that carries no path context.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl DatasetError {
    /// Construct a [`DatasetError::DataNotFound`].
    pub fn not_found<S: Into<String>>(path: impl Into<PathBuf>, msg: S) -> Self {
        DatasetError::DataNotFound { path: path.into(), message: msg.into() }
    }

    /// Construct a [`DatasetError::InvalidFormat`].
    pub fn invalid_format<S: Into<String>>(path: impl Into<PathBuf>, msg: S) -> Self {
        DatasetError::InvalidFormat { path: path.into(), message: msg.into() }
    }

    /// Construct a [`DatasetError::IoError`].
    pub fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        DatasetError::IoError { path: path.into(), source }
    }

    /// Construct a [`DatasetError::SubcarrierMismatch`].
    pub fn subcarrier_mismatch(path: impl Into<PathBuf>, found: usize, expected: usize) -> Self {
        DatasetError::SubcarrierMismatch { path: path.into(), found, expected }
    }

    /// Construct a [`DatasetError::NpyReadError`].
    pub fn npy_read<S: Into<String>>(path: impl Into<PathBuf>, msg: S) -> Self {
        DatasetError::NpyReadError { path: path.into(), message: msg.into() }
    }
}

// ---------------------------------------------------------------------------
// SubcarrierError
// ---------------------------------------------------------------------------

/// Errors produced by the subcarrier resampling / interpolation functions.
#[derive(Debug, Error)]
pub enum SubcarrierError {
    /// The source or destination count is zero.
    #[error("Subcarrier count must be >= 1, got {count}")]
    ZeroCount {
        /// The offending count.
        count: usize,
    },

    /// The array's last dimension does not match the declared source count.
    #[error(
        "Subcarrier shape mismatch: last dim is {actual_sc} but src_n={expected_sc} \
         (full shape: {shape:?})"
    )]
    InputShapeMismatch {
        /// Expected subcarrier count.
        expected_sc: usize,
        /// Actual last-dimension size.
        actual_sc: usize,
        /// Full shape of the input.
        shape: Vec<usize>,
    },

    /// The requested interpolation method is not yet implemented.
    #[error("Interpolation method `{method}` is not implemented")]
    MethodNotImplemented {
        /// Name of the unsupported method.
        method: String,
    },

    /// `src_n == dst_n` — no resampling needed.
    #[error("src_n == dst_n == {count}; call interpolate only when counts differ")]
    NopInterpolation {
        /// The equal count.
        count: usize,
    },

    /// A numerical error during interpolation.
    #[error("Numerical error: {0}")]
    NumericalError(String),
}

impl SubcarrierError {
    /// Construct a [`SubcarrierError::NumericalError`].
    pub fn numerical<S: Into<String>>(msg: S) -> Self {
        SubcarrierError::NumericalError(msg.into())
    }
}
