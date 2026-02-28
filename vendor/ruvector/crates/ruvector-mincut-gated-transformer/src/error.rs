//! Error types for mincut gated transformer.
//!
//! Errors are deterministic and never panic in the production path.

use thiserror::Error;

/// Error types for the mincut gated transformer.
///
/// All errors are deterministic - the same conditions will always produce
/// the same error variant. The inference path never panics.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Configuration is invalid or internally inconsistent
    #[error("Bad configuration: {0}")]
    BadConfig(&'static str),

    /// Weights are malformed, corrupted, or incompatible with config
    #[error("Bad weights: {0}")]
    BadWeights(&'static str),

    /// Input data is invalid or out of expected bounds
    #[error("Bad input: {0}")]
    BadInput(&'static str),

    /// Output buffer is too small for the expected result
    #[error("Output buffer too small: need {needed}, got {provided}")]
    OutputTooSmall {
        /// Required buffer size
        needed: usize,
        /// Provided buffer size
        provided: usize,
    },

    /// Requested mode or feature is not supported
    #[error("Unsupported mode: {0}")]
    UnsupportedMode(&'static str),
}

/// Result type alias for mincut gated transformer operations
pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    /// Check if this error is recoverable (can retry with different input)
    #[inline]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Error::BadInput(_) | Error::OutputTooSmall { .. })
    }

    /// Check if this error is a configuration issue (requires reinitialization)
    #[inline]
    pub fn is_config_error(&self) -> bool {
        matches!(self, Error::BadConfig(_) | Error::BadWeights(_))
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_error_display() {
        let e = Error::BadConfig("invalid head count");
        assert!(e.to_string().contains("invalid head count"));

        let e = Error::OutputTooSmall {
            needed: 100,
            provided: 50,
        };
        assert!(e.to_string().contains("100"));
        assert!(e.to_string().contains("50"));
    }

    #[test]
    fn test_error_recovery_classification() {
        assert!(Error::BadInput("test").is_recoverable());
        assert!(Error::OutputTooSmall {
            needed: 1,
            provided: 0
        }
        .is_recoverable());
        assert!(!Error::BadConfig("test").is_recoverable());
        assert!(!Error::BadWeights("test").is_recoverable());
        assert!(!Error::UnsupportedMode("test").is_recoverable());
    }

    #[test]
    fn test_error_config_classification() {
        assert!(Error::BadConfig("test").is_config_error());
        assert!(Error::BadWeights("test").is_config_error());
        assert!(!Error::BadInput("test").is_config_error());
    }
}
