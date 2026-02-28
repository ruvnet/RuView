//! # Domain Errors
//!
//! Strongly-typed error types for domain operations.
//!
//! This module provides error types that follow these principles:
//! - Each error type represents a specific failure mode
//! - Errors carry enough context for debugging and user messaging
//! - Errors implement `std::error::Error` for ecosystem compatibility
//! - Errors are serializable for API responses

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::entities::{ClusterId, EmbeddingId, RecordingId, SegmentId, TaxonId};

/// Top-level error type for all domain operations.
#[derive(Debug, Error)]
pub enum DomainError {
    /// Error related to recording operations.
    #[error("Recording error: {0}")]
    Recording(#[from] RecordingError),

    /// Error related to segment operations.
    #[error("Segment error: {0}")]
    Segment(#[from] SegmentError),

    /// Error related to embedding operations.
    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    /// Error related to cluster operations.
    #[error("Cluster error: {0}")]
    Cluster(#[from] ClusterError),

    /// Error related to analysis operations.
    #[error("Analysis error: {0}")]
    Analysis(#[from] AnalysisError),

    /// Error related to configuration.
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigurationError),

    /// Error related to validation.
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Internal error (unexpected condition).
    #[error("Internal error: {0}")]
    Internal(String),
}

impl DomainError {
    /// Creates a new internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Returns an error code for API responses.
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Recording(e) => e.error_code(),
            Self::Segment(e) => e.error_code(),
            Self::Embedding(e) => e.error_code(),
            Self::Cluster(e) => e.error_code(),
            Self::Analysis(e) => e.error_code(),
            Self::Configuration(e) => e.error_code(),
            Self::Validation(e) => e.error_code(),
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Returns the HTTP status code for this error.
    #[must_use]
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Recording(e) => e.status_code(),
            Self::Segment(e) => e.status_code(),
            Self::Embedding(e) => e.status_code(),
            Self::Cluster(e) => e.status_code(),
            Self::Analysis(e) => e.status_code(),
            Self::Configuration(_) => 500,
            Self::Validation(_) => 400,
            Self::Internal(_) => 500,
        }
    }
}

// =============================================================================
// Recording Errors
// =============================================================================

/// Errors related to recording operations.
#[derive(Debug, Error)]
pub enum RecordingError {
    /// Recording not found.
    #[error("Recording not found: {0}")]
    NotFound(RecordingId),

    /// Recording already exists.
    #[error("Recording already exists: {0}")]
    AlreadyExists(RecordingId),

    /// Invalid audio format.
    #[error("Invalid audio format: {format}. Supported formats: {supported}")]
    InvalidFormat {
        /// The invalid format.
        format: String,
        /// Comma-separated list of supported formats.
        supported: String,
    },

    /// File too large.
    #[error("File too large: {size_bytes} bytes (max: {max_bytes} bytes)")]
    FileTooLarge {
        /// Actual file size.
        size_bytes: u64,
        /// Maximum allowed size.
        max_bytes: u64,
    },

    /// Invalid duration.
    #[error("Invalid duration: {duration_ms}ms (min: {min_ms}ms, max: {max_ms}ms)")]
    InvalidDuration {
        /// Actual duration.
        duration_ms: u64,
        /// Minimum allowed duration.
        min_ms: u64,
        /// Maximum allowed duration.
        max_ms: u64,
    },

    /// Corrupted audio file.
    #[error("Corrupted audio file: {reason}")]
    Corrupted {
        /// Reason for corruption.
        reason: String,
    },

    /// Storage error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Processing error.
    #[error("Processing error: {0}")]
    Processing(String),
}

impl RecordingError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "RECORDING_NOT_FOUND",
            Self::AlreadyExists(_) => "RECORDING_ALREADY_EXISTS",
            Self::InvalidFormat { .. } => "INVALID_AUDIO_FORMAT",
            Self::FileTooLarge { .. } => "FILE_TOO_LARGE",
            Self::InvalidDuration { .. } => "INVALID_DURATION",
            Self::Corrupted { .. } => "CORRUPTED_FILE",
            Self::Storage(_) => "STORAGE_ERROR",
            Self::Processing(_) => "PROCESSING_ERROR",
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::AlreadyExists(_) => 409,
            Self::InvalidFormat { .. } => 415,
            Self::FileTooLarge { .. } => 413,
            Self::InvalidDuration { .. } => 400,
            Self::Corrupted { .. } => 400,
            Self::Storage(_) => 503,
            Self::Processing(_) => 500,
        }
    }
}

// =============================================================================
// Segment Errors
// =============================================================================

/// Errors related to segment operations.
#[derive(Debug, Error)]
pub enum SegmentError {
    /// Segment not found.
    #[error("Segment not found: {0}")]
    NotFound(SegmentId),

    /// Invalid time range.
    #[error("Invalid time range: start={start_ms}ms, end={end_ms}ms")]
    InvalidTimeRange {
        /// Start time.
        start_ms: u64,
        /// End time.
        end_ms: u64,
    },

    /// Time range out of bounds.
    #[error("Time range out of bounds: segment ends at {end_ms}ms but recording is {duration_ms}ms")]
    OutOfBounds {
        /// Segment end time.
        end_ms: u64,
        /// Recording duration.
        duration_ms: u64,
    },

    /// Segment too short.
    #[error("Segment too short: {duration_ms}ms (min: {min_ms}ms)")]
    TooShort {
        /// Actual duration.
        duration_ms: u64,
        /// Minimum required duration.
        min_ms: u64,
    },

    /// Overlapping segments.
    #[error("Segment overlaps with existing segment: {existing_id}")]
    Overlapping {
        /// Existing segment ID.
        existing_id: SegmentId,
    },

    /// Classification failed.
    #[error("Classification failed: {reason}")]
    ClassificationFailed {
        /// Failure reason.
        reason: String,
    },
}

impl SegmentError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "SEGMENT_NOT_FOUND",
            Self::InvalidTimeRange { .. } => "INVALID_TIME_RANGE",
            Self::OutOfBounds { .. } => "TIME_RANGE_OUT_OF_BOUNDS",
            Self::TooShort { .. } => "SEGMENT_TOO_SHORT",
            Self::Overlapping { .. } => "OVERLAPPING_SEGMENT",
            Self::ClassificationFailed { .. } => "CLASSIFICATION_FAILED",
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::InvalidTimeRange { .. } => 400,
            Self::OutOfBounds { .. } => 400,
            Self::TooShort { .. } => 400,
            Self::Overlapping { .. } => 409,
            Self::ClassificationFailed { .. } => 500,
        }
    }
}

// =============================================================================
// Embedding Errors
// =============================================================================

/// Errors related to embedding operations.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// Embedding not found.
    #[error("Embedding not found: {0}")]
    NotFound(EmbeddingId),

    /// Model not available.
    #[error("Embedding model not available: {model_name}")]
    ModelNotAvailable {
        /// Model name.
        model_name: String,
    },

    /// Dimension mismatch.
    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: u32,
        /// Actual dimension.
        actual: u32,
    },

    /// Generation failed.
    #[error("Embedding generation failed: {reason}")]
    GenerationFailed {
        /// Failure reason.
        reason: String,
    },

    /// Indexing failed.
    #[error("Embedding indexing failed: {reason}")]
    IndexingFailed {
        /// Failure reason.
        reason: String,
    },

    /// Search failed.
    #[error("Embedding search failed: {reason}")]
    SearchFailed {
        /// Failure reason.
        reason: String,
    },

    /// Vector database error.
    #[error("Vector database error: {0}")]
    VectorDb(String),
}

impl EmbeddingError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "EMBEDDING_NOT_FOUND",
            Self::ModelNotAvailable { .. } => "MODEL_NOT_AVAILABLE",
            Self::DimensionMismatch { .. } => "DIMENSION_MISMATCH",
            Self::GenerationFailed { .. } => "GENERATION_FAILED",
            Self::IndexingFailed { .. } => "INDEXING_FAILED",
            Self::SearchFailed { .. } => "SEARCH_FAILED",
            Self::VectorDb(_) => "VECTOR_DB_ERROR",
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::ModelNotAvailable { .. } => 503,
            Self::DimensionMismatch { .. } => 400,
            Self::GenerationFailed { .. } => 500,
            Self::IndexingFailed { .. } => 500,
            Self::SearchFailed { .. } => 500,
            Self::VectorDb(_) => 503,
        }
    }
}

// =============================================================================
// Cluster Errors
// =============================================================================

/// Errors related to cluster operations.
#[derive(Debug, Error)]
pub enum ClusterError {
    /// Cluster not found.
    #[error("Cluster not found: {0}")]
    NotFound(ClusterId),

    /// Empty cluster (no members).
    #[error("Cannot perform operation on empty cluster: {0}")]
    Empty(ClusterId),

    /// Invalid merge (clusters too dissimilar).
    #[error("Cannot merge clusters: similarity {similarity} below threshold {threshold}")]
    InvalidMerge {
        /// Actual similarity.
        similarity: f32,
        /// Required threshold.
        threshold: f32,
    },

    /// Cluster already identified.
    #[error("Cluster already identified as: {taxon_id}")]
    AlreadyIdentified {
        /// Existing taxon.
        taxon_id: TaxonId,
    },

    /// Insufficient members for operation.
    #[error("Insufficient cluster members: {count} (min: {min_required})")]
    InsufficientMembers {
        /// Actual count.
        count: u32,
        /// Minimum required.
        min_required: u32,
    },
}

impl ClusterError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "CLUSTER_NOT_FOUND",
            Self::Empty(_) => "CLUSTER_EMPTY",
            Self::InvalidMerge { .. } => "INVALID_MERGE",
            Self::AlreadyIdentified { .. } => "CLUSTER_ALREADY_IDENTIFIED",
            Self::InsufficientMembers { .. } => "INSUFFICIENT_MEMBERS",
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Empty(_) => 400,
            Self::InvalidMerge { .. } => 400,
            Self::AlreadyIdentified { .. } => 409,
            Self::InsufficientMembers { .. } => 400,
        }
    }
}

// =============================================================================
// Analysis Errors
// =============================================================================

/// Errors related to analysis operations.
#[derive(Debug, Error)]
pub enum AnalysisError {
    /// Analysis request not found.
    #[error("Analysis request not found: {request_id}")]
    NotFound {
        /// Request ID.
        request_id: String,
    },

    /// Analysis already in progress.
    #[error("Analysis already in progress for target")]
    AlreadyInProgress,

    /// Invalid region.
    #[error("Invalid region: radius {radius_m}m exceeds maximum {max_radius_m}m")]
    InvalidRegion {
        /// Requested radius.
        radius_m: f64,
        /// Maximum allowed radius.
        max_radius_m: f64,
    },

    /// Invalid time period.
    #[error("Invalid time period: {reason}")]
    InvalidTimePeriod {
        /// Reason.
        reason: String,
    },

    /// Insufficient data.
    #[error("Insufficient data for analysis: {reason}")]
    InsufficientData {
        /// Reason.
        reason: String,
    },

    /// Analysis timeout.
    #[error("Analysis timed out after {timeout_secs} seconds")]
    Timeout {
        /// Timeout duration.
        timeout_secs: u64,
    },

    /// Analysis failed.
    #[error("Analysis failed: {reason}")]
    Failed {
        /// Failure reason.
        reason: String,
    },
}

impl AnalysisError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "ANALYSIS_NOT_FOUND",
            Self::AlreadyInProgress => "ANALYSIS_IN_PROGRESS",
            Self::InvalidRegion { .. } => "INVALID_REGION",
            Self::InvalidTimePeriod { .. } => "INVALID_TIME_PERIOD",
            Self::InsufficientData { .. } => "INSUFFICIENT_DATA",
            Self::Timeout { .. } => "ANALYSIS_TIMEOUT",
            Self::Failed { .. } => "ANALYSIS_FAILED",
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::NotFound { .. } => 404,
            Self::AlreadyInProgress => 409,
            Self::InvalidRegion { .. } => 400,
            Self::InvalidTimePeriod { .. } => 400,
            Self::InsufficientData { .. } => 400,
            Self::Timeout { .. } => 504,
            Self::Failed { .. } => 500,
        }
    }
}

// =============================================================================
// Configuration Errors
// =============================================================================

/// Errors related to configuration.
#[derive(Debug, Error)]
pub enum ConfigurationError {
    /// Missing required configuration.
    #[error("Missing required configuration: {key}")]
    Missing {
        /// Configuration key.
        key: String,
    },

    /// Invalid configuration value.
    #[error("Invalid configuration value for {key}: {reason}")]
    Invalid {
        /// Configuration key.
        key: String,
        /// Reason.
        reason: String,
    },

    /// Configuration file not found.
    #[error("Configuration file not found: {path}")]
    FileNotFound {
        /// File path.
        path: String,
    },

    /// Configuration parse error.
    #[error("Failed to parse configuration: {reason}")]
    ParseError {
        /// Reason.
        reason: String,
    },
}

impl ConfigurationError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "CONFIG_MISSING",
            Self::Invalid { .. } => "CONFIG_INVALID",
            Self::FileNotFound { .. } => "CONFIG_FILE_NOT_FOUND",
            Self::ParseError { .. } => "CONFIG_PARSE_ERROR",
        }
    }
}

// =============================================================================
// Validation Errors
// =============================================================================

/// Errors related to input validation.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Required field missing.
    #[error("Required field missing: {field}")]
    RequiredField {
        /// Field name.
        field: String,
    },

    /// Field value out of range.
    #[error("Field {field} out of range: {value} (min: {min}, max: {max})")]
    OutOfRange {
        /// Field name.
        field: String,
        /// Actual value.
        value: String,
        /// Minimum allowed.
        min: String,
        /// Maximum allowed.
        max: String,
    },

    /// Invalid field format.
    #[error("Invalid format for field {field}: {reason}")]
    InvalidFormat {
        /// Field name.
        field: String,
        /// Reason.
        reason: String,
    },

    /// Multiple validation errors.
    #[error("Multiple validation errors: {}", .errors.join(", "))]
    Multiple {
        /// List of error messages.
        errors: Vec<String>,
    },
}

impl ValidationError {
    /// Returns an error code.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::RequiredField { .. } => "REQUIRED_FIELD_MISSING",
            Self::OutOfRange { .. } => "VALUE_OUT_OF_RANGE",
            Self::InvalidFormat { .. } => "INVALID_FORMAT",
            Self::Multiple { .. } => "VALIDATION_ERRORS",
        }
    }

    /// Creates a validation error for a required field.
    pub fn required(field: impl Into<String>) -> Self {
        Self::RequiredField {
            field: field.into(),
        }
    }

    /// Creates a validation error for an out-of-range value.
    pub fn out_of_range<T: std::fmt::Display>(
        field: impl Into<String>,
        value: T,
        min: T,
        max: T,
    ) -> Self {
        Self::OutOfRange {
            field: field.into(),
            value: value.to_string(),
            min: min.to_string(),
            max: max.to_string(),
        }
    }

    /// Creates a validation error for an invalid format.
    pub fn invalid_format(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidFormat {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

// =============================================================================
// API Error Response
// =============================================================================

/// Serializable error response for API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code for programmatic handling.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Additional error details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ErrorResponse {
    /// Creates a new error response.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            request_id: None,
        }
    }

    /// Adds details to the error response.
    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Adds a request ID to the error response.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl From<&DomainError> for ErrorResponse {
    fn from(error: &DomainError) -> Self {
        Self::new(error.error_code(), error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_error_codes() {
        let err = RecordingError::NotFound(RecordingId::new());
        assert_eq!(err.error_code(), "RECORDING_NOT_FOUND");
        assert_eq!(err.status_code(), 404);
    }

    #[test]
    fn test_domain_error_conversion() {
        let recording_err = RecordingError::NotFound(RecordingId::new());
        let domain_err = DomainError::from(recording_err);
        assert_eq!(domain_err.error_code(), "RECORDING_NOT_FOUND");
        assert_eq!(domain_err.status_code(), 404);
    }

    #[test]
    fn test_validation_error_builders() {
        let err = ValidationError::required("name");
        assert_eq!(err.error_code(), "REQUIRED_FIELD_MISSING");

        let err = ValidationError::out_of_range("age", 150, 0, 120);
        assert_eq!(err.error_code(), "VALUE_OUT_OF_RANGE");
    }

    #[test]
    fn test_error_response_serialization() {
        let response = ErrorResponse::new("TEST_ERROR", "Test error message")
            .with_request_id("req-123");

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("TEST_ERROR"));
        assert!(json.contains("req-123"));
    }
}
