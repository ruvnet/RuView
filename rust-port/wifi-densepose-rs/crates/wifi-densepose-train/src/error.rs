//! Error types for the WiFi-DensePose training pipeline.
//!
//! This module defines a hierarchy of errors covering every failure mode in
//! the training pipeline: configuration validation, dataset I/O, subcarrier
//! interpolation, and top-level training orchestration.

use thiserror::Error;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Top-level training error
// ---------------------------------------------------------------------------

/// A convenient `Result` alias used throughout the training crate.
pub type TrainResult<T> = Result<T, TrainError>;

/// Top-level error type for the training pipeline.
///
/// Every public function in this crate that can fail returns
/// `TrainResult<T>`, which is `Result<T, TrainError>`.
#[derive(Debug, Error)]
pub enum TrainError {
    /// Configuration is invalid or internally inconsistent.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// A dataset operation failed (I/O, format, missing data).
    #[error("Dataset error: {0}")]
    Dataset(#[from] DatasetError),

    /// Subcarrier interpolation / resampling failed.
    #[error("Subcarrier interpolation error: {0}")]
    Subcarrier(#[from] SubcarrierError),

    /// An underlying I/O error not covered by a more specific variant.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML (de)serialization error.
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    /// TOML serialization error.
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

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
// Configuration errors
// ---------------------------------------------------------------------------

/// Errors produced when validating or loading a [`TrainingConfig`].
///
/// [`TrainingConfig`]: crate::config::TrainingConfig
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required field has a value that violates a constraint.
    #[error("Invalid value for field `{field}`: {reason}")]
    InvalidValue {
        /// Name of the configuration field.
        field: &'static str,
        /// Human-readable reason the value is invalid.
        reason: String,
    },

    /// The configuration file could not be read.
    #[error("Cannot read configuration file `{path}`: {source}")]
    FileRead {
        /// Path that was being read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The configuration file contains invalid TOML.
    #[error("Cannot parse configuration file `{path}`: {source}")]
    ParseError {
        /// Path that was being parsed.
        path: PathBuf,
        /// Underlying TOML parse error.
        #[source]
        source: toml::de::Error,
    },

    /// A path specified in the config does not exist.
    #[error("Path `{path}` specified in config does not exist")]
    PathNotFound {
        /// The missing path.
        path: PathBuf,
    },
}

impl ConfigError {
    /// Construct an [`ConfigError::InvalidValue`] error.
    pub fn invalid_value<S: Into<String>>(field: &'static str, reason: S) -> Self {
        ConfigError::InvalidValue {
            field,
            reason: reason.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Dataset errors
// ---------------------------------------------------------------------------

/// Errors produced while loading or accessing dataset samples.
#[derive(Debug, Error)]
pub enum DatasetError {
    /// The requested data file or directory was not found.
    ///
    /// Production training data is mandatory; this error is never silently
    /// suppressed. Use [`SyntheticDataset`] only for proof/testing.
    ///
    /// [`SyntheticDataset`]: crate::dataset::SyntheticDataset
    #[error("Data not found at `{path}`: {message}")]
    DataNotFound {
        /// Path that was expected to contain data.
        path: PathBuf,
        /// Additional context.
        message: String,
    },

    /// A file was found but its format is incorrect or unexpected.
    ///
    /// This covers malformed numpy arrays, unexpected shapes, bad JSON
    /// metadata, etc.
    #[error("Invalid data format in `{path}`: {message}")]
    InvalidFormat {
        /// Path of the malformed file.
        path: PathBuf,
        /// Description of the format problem.
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

    /// The number of subcarriers in the data file does not match the
    /// configuration expectation (before or after interpolation).
    #[error(
        "Subcarrier count mismatch in `{path}`: \
         file has {found} subcarriers, expected {expected}"
    )]
    SubcarrierMismatch {
        /// Path of the offending file.
        path: PathBuf,
        /// Number of subcarriers found in the file.
        found: usize,
        /// Number of subcarriers expected by the configuration.
        expected: usize,
    },

    /// A sample index was out of bounds.
    #[error("Index {index} is out of bounds for dataset of length {len}")]
    IndexOutOfBounds {
        /// The requested index.
        index: usize,
        /// Total number of samples.
        len: usize,
    },

    /// A numpy array could not be read.
    #[error("NumPy array read error in `{path}`: {message}")]
    NpyReadError {
        /// Path of the `.npy` file.
        path: PathBuf,
        /// Error description.
        message: String,
    },

    /// A metadata file (e.g., `meta.json`) is missing or malformed.
    #[error("Metadata error for subject {subject_id}: {message}")]
    MetadataError {
        /// Subject whose metadata could not be read.
        subject_id: u32,
        /// Description of the problem.
        message: String,
    },

    /// No subjects matching the requested IDs were found in the data directory.
    #[error(
        "No subjects found in `{data_dir}` matching the requested IDs: {requested:?}"
    )]
    NoSubjectsFound {
        /// Root data directory that was scanned.
        data_dir: PathBuf,
        /// Subject IDs that were requested.
        requested: Vec<u32>,
    },

    /// A subcarrier interpolation error occurred during sample loading.
    #[error("Subcarrier interpolation failed while loading sample {sample_idx}: {source}")]
    InterpolationError {
        /// The sample index being loaded.
        sample_idx: usize,
        /// Underlying interpolation error.
        #[source]
        source: SubcarrierError,
    },
}

impl DatasetError {
    /// Construct a [`DatasetError::DataNotFound`] error.
    pub fn not_found<S: Into<String>>(path: impl Into<PathBuf>, msg: S) -> Self {
        DatasetError::DataNotFound {
            path: path.into(),
            message: msg.into(),
        }
    }

    /// Construct a [`DatasetError::InvalidFormat`] error.
    pub fn invalid_format<S: Into<String>>(path: impl Into<PathBuf>, msg: S) -> Self {
        DatasetError::InvalidFormat {
            path: path.into(),
            message: msg.into(),
        }
    }

    /// Construct a [`DatasetError::IoError`] error.
    pub fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        DatasetError::IoError {
            path: path.into(),
            source,
        }
    }

    /// Construct a [`DatasetError::SubcarrierMismatch`] error.
    pub fn subcarrier_mismatch(path: impl Into<PathBuf>, found: usize, expected: usize) -> Self {
        DatasetError::SubcarrierMismatch {
            path: path.into(),
            found,
            expected,
        }
    }

    /// Construct a [`DatasetError::NpyReadError`] error.
    pub fn npy_read<S: Into<String>>(path: impl Into<PathBuf>, msg: S) -> Self {
        DatasetError::NpyReadError {
            path: path.into(),
            message: msg.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Subcarrier interpolation errors
// ---------------------------------------------------------------------------

/// Errors produced by the subcarrier resampling functions.
#[derive(Debug, Error)]
pub enum SubcarrierError {
    /// The source or destination subcarrier count is zero.
    #[error("Subcarrier count must be at least 1, got {count}")]
    ZeroCount {
        /// The offending count.
        count: usize,
    },

    /// The input array has an unexpected shape.
    #[error(
        "Input array shape mismatch: expected last dimension {expected_sc}, \
         got {actual_sc} (full shape: {shape:?})"
    )]
    InputShapeMismatch {
        /// Expected number of subcarriers (last dimension).
        expected_sc: usize,
        /// Actual number of subcarriers found.
        actual_sc: usize,
        /// Full shape of the input array.
        shape: Vec<usize>,
    },

    /// The requested interpolation method is not implemented.
    #[error("Interpolation method `{method}` is not yet implemented")]
    MethodNotImplemented {
        /// Name of the unimplemented method.
        method: String,
    },

    /// Source and destination subcarrier counts are already equal.
    ///
    /// Callers should check [`TrainingConfig::needs_subcarrier_interp`] before
    /// calling the interpolation routine to avoid this error.
    ///
    /// [`TrainingConfig::needs_subcarrier_interp`]:
    ///     crate::config::TrainingConfig::needs_subcarrier_interp
    #[error(
        "Source and destination subcarrier counts are equal ({count}); \
         no interpolation is needed"
    )]
    NopInterpolation {
        /// The equal count.
        count: usize,
    },

    /// A numerical error occurred during interpolation (e.g., division by zero
    /// due to coincident knot positions).
    #[error("Numerical error during interpolation: {0}")]
    NumericalError(String),
}

impl SubcarrierError {
    /// Construct a [`SubcarrierError::NumericalError`].
    pub fn numerical<S: Into<String>>(msg: S) -> Self {
        SubcarrierError::NumericalError(msg.into())
    }
}
