//! Error types for the rvlite adapter.
//!
//! Provides a lightweight error enum that wraps `RvfError` and I/O errors,
//! plus a dimension-mismatch variant for early validation.

use core::fmt;

use rvf_types::RvfError;

/// Errors that can occur in rvlite operations.
#[derive(Debug)]
pub enum RvliteError {
    /// An error originating from the RVF runtime or types layer.
    Rvf(RvfError),
    /// An I/O error described by a message string.
    Io(String),
    /// The supplied vector has the wrong number of dimensions.
    DimensionMismatch {
        /// The dimension the collection was created with.
        expected: u16,
        /// The dimension of the vector that was supplied.
        got: usize,
    },
}

impl fmt::Display for RvliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rvf(e) => write!(f, "rvf: {e}"),
            Self::Io(msg) => write!(f, "io: {msg}"),
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl From<RvfError> for RvliteError {
    fn from(e: RvfError) -> Self {
        Self::Rvf(e)
    }
}

impl From<std::io::Error> for RvliteError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Convenience alias used throughout the rvlite crate.
pub type Result<T> = std::result::Result<T, RvliteError>;

#[cfg(test)]
mod tests {
    use super::*;
    use rvf_types::ErrorCode;

    #[test]
    fn display_rvf_variant() {
        let err = RvliteError::Rvf(RvfError::Code(ErrorCode::DimensionMismatch));
        let msg = format!("{err}");
        assert!(msg.contains("rvf:"));
    }

    #[test]
    fn display_io_variant() {
        let err = RvliteError::Io("file not found".into());
        let msg = format!("{err}");
        assert!(msg.contains("io: file not found"));
    }

    #[test]
    fn display_dimension_mismatch() {
        let err = RvliteError::DimensionMismatch {
            expected: 128,
            got: 64,
        };
        let msg = format!("{err}");
        assert!(msg.contains("expected 128"));
        assert!(msg.contains("got 64"));
    }

    #[test]
    fn from_rvf_error() {
        let rvf = RvfError::Code(ErrorCode::FsyncFailed);
        let err: RvliteError = rvf.into();
        matches!(err, RvliteError::Rvf(_));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err: RvliteError = io_err.into();
        match err {
            RvliteError::Io(msg) => assert!(msg.contains("gone")),
            _ => panic!("expected Io variant"),
        }
    }
}
