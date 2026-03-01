//! Error types for Ruvector-Scipix
//!
//! Comprehensive error handling with context, HTTP status mapping, and retry logic.

use std::io;
use thiserror::Error;

/// Result type alias for Scipix operations
pub type Result<T> = std::result::Result<T, ScipixError>;

/// Comprehensive error types for all Scipix operations
#[derive(Debug, Error)]
pub enum ScipixError {
    /// Image loading or processing error
    #[error("Image error: {0}")]
    Image(String),

    /// Machine learning model error
    #[error("Model error: {0}")]
    Model(String),

    /// OCR processing error
    #[error("OCR error: {0}")]
    Ocr(String),

    /// LaTeX generation or parsing error
    #[error("LaTeX error: {0}")]
    LaTeX(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid input error
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Operation timeout
    #[error("Timeout: operation took longer than {0}s")]
    Timeout(u64),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ScipixError {
    /// Check if the error is retryable
    ///
    /// # Returns
    ///
    /// `true` if the operation should be retried, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_scipix::ScipixError;
    ///
    /// let timeout_error = ScipixError::Timeout(30);
    /// assert!(timeout_error.is_retryable());
    ///
    /// let config_error = ScipixError::Config("Invalid parameter".to_string());
    /// assert!(!config_error.is_retryable());
    /// ```
    pub fn is_retryable(&self) -> bool {
        match self {
            // Retryable errors
            ScipixError::Timeout(_) => true,
            ScipixError::RateLimit(_) => true,
            ScipixError::Io(_) => true,
            ScipixError::Internal(_) => true,

            // Non-retryable errors
            ScipixError::Image(_) => false,
            ScipixError::Model(_) => false,
            ScipixError::Ocr(_) => false,
            ScipixError::LaTeX(_) => false,
            ScipixError::Config(_) => false,
            ScipixError::Serialization(_) => false,
            ScipixError::InvalidInput(_) => false,
            ScipixError::NotFound(_) => false,
            ScipixError::Auth(_) => false,
        }
    }

    /// Map error to HTTP status code
    ///
    /// # Returns
    ///
    /// HTTP status code representing the error type
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_scipix::ScipixError;
    ///
    /// let auth_error = ScipixError::Auth("Invalid token".to_string());
    /// assert_eq!(auth_error.status_code(), 401);
    ///
    /// let not_found = ScipixError::NotFound("Model not found".to_string());
    /// assert_eq!(not_found.status_code(), 404);
    /// ```
    pub fn status_code(&self) -> u16 {
        match self {
            ScipixError::Auth(_) => 401,
            ScipixError::NotFound(_) => 404,
            ScipixError::InvalidInput(_) => 400,
            ScipixError::RateLimit(_) => 429,
            ScipixError::Timeout(_) => 408,
            ScipixError::Config(_) => 400,
            ScipixError::Internal(_) => 500,
            _ => 500,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            ScipixError::Image(_) => "image",
            ScipixError::Model(_) => "model",
            ScipixError::Ocr(_) => "ocr",
            ScipixError::LaTeX(_) => "latex",
            ScipixError::Config(_) => "config",
            ScipixError::Io(_) => "io",
            ScipixError::Serialization(_) => "serialization",
            ScipixError::InvalidInput(_) => "invalid_input",
            ScipixError::Timeout(_) => "timeout",
            ScipixError::NotFound(_) => "not_found",
            ScipixError::Auth(_) => "auth",
            ScipixError::RateLimit(_) => "rate_limit",
            ScipixError::Internal(_) => "internal",
        }
    }
}

// Conversion from serde_json::Error
impl From<serde_json::Error> for ScipixError {
    fn from(err: serde_json::Error) -> Self {
        ScipixError::Serialization(err.to_string())
    }
}

// Conversion from toml::de::Error
impl From<toml::de::Error> for ScipixError {
    fn from(err: toml::de::Error) -> Self {
        ScipixError::Config(err.to_string())
    }
}

// Conversion from toml::ser::Error
impl From<toml::ser::Error> for ScipixError {
    fn from(err: toml::ser::Error) -> Self {
        ScipixError::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ScipixError::Image("Failed to load".to_string());
        assert_eq!(err.to_string(), "Image error: Failed to load");
    }

    #[test]
    fn test_is_retryable() {
        assert!(ScipixError::Timeout(30).is_retryable());
        assert!(ScipixError::RateLimit("Exceeded".to_string()).is_retryable());
        assert!(!ScipixError::Config("Invalid".to_string()).is_retryable());
        assert!(!ScipixError::Auth("Unauthorized".to_string()).is_retryable());
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(ScipixError::Auth("".to_string()).status_code(), 401);
        assert_eq!(ScipixError::NotFound("".to_string()).status_code(), 404);
        assert_eq!(ScipixError::InvalidInput("".to_string()).status_code(), 400);
        assert_eq!(ScipixError::RateLimit("".to_string()).status_code(), 429);
        assert_eq!(ScipixError::Timeout(0).status_code(), 408);
        assert_eq!(ScipixError::Internal("".to_string()).status_code(), 500);
    }

    #[test]
    fn test_category() {
        assert_eq!(ScipixError::Image("".to_string()).category(), "image");
        assert_eq!(ScipixError::Model("".to_string()).category(), "model");
        assert_eq!(ScipixError::Ocr("".to_string()).category(), "ocr");
        assert_eq!(ScipixError::LaTeX("".to_string()).category(), "latex");
        assert_eq!(ScipixError::Config("".to_string()).category(), "config");
        assert_eq!(ScipixError::Auth("".to_string()).category(), "auth");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let scipix_err: ScipixError = io_err.into();
        assert!(matches!(scipix_err, ScipixError::Io(_)));
    }

    #[test]
    fn test_from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let scipix_err: ScipixError = json_err.into();
        assert!(matches!(scipix_err, ScipixError::Serialization(_)));
    }
}
