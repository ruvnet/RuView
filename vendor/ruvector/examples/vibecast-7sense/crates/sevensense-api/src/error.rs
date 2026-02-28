//! API error types and HTTP response handling.
//!
//! This module provides a unified error type for all API endpoints with
//! proper HTTP status code mapping and JSON error responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

use crate::services::{AnalysisError, AudioError, EmbeddingError, InterpretationError, VectorError};

/// Unified API error type.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Resource not found (404)
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Bad request with validation errors (400)
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Unauthorized access (401)
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Forbidden access (403)
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Conflict with existing resource (409)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Payload too large (413)
    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    /// Unsupported media type (415)
    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),

    /// Rate limit exceeded (429)
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Internal server error (500)
    #[error("Internal error: {0}")]
    Internal(String),

    /// Service unavailable (503)
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Audio processing error
    #[error("Audio processing error: {0}")]
    AudioProcessing(#[from] AudioError),

    /// Embedding error
    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    /// Vector index error
    #[error("Vector index error: {0}")]
    VectorIndex(#[from] VectorError),

    /// Analysis error
    #[error("Analysis error: {0}")]
    Analysis(#[from] AnalysisError),

    /// Interpretation error
    #[error("Interpretation error: {0}")]
    Interpretation(#[from] InterpretationError),

    /// Generic anyhow error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// JSON error response body.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error type identifier
    #[schema(example = "not_found")]
    pub error: String,
    /// Human-readable error message
    #[schema(example = "Recording with ID xyz not found")]
    pub message: String,
    /// Optional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ApiError {
    /// Get the HTTP status code for this error.
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::AudioProcessing(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Embedding(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::VectorIndex(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Analysis(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Interpretation(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error type identifier.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::Conflict(_) => "conflict",
            Self::PayloadTooLarge(_) => "payload_too_large",
            Self::UnsupportedMediaType(_) => "unsupported_media_type",
            Self::RateLimitExceeded => "rate_limit_exceeded",
            Self::Internal(_) => "internal_error",
            Self::ServiceUnavailable(_) => "service_unavailable",
            Self::AudioProcessing(_) => "audio_processing_error",
            Self::Embedding(_) => "embedding_error",
            Self::VectorIndex(_) => "vector_index_error",
            Self::Analysis(_) => "analysis_error",
            Self::Interpretation(_) => "interpretation_error",
            Self::Other(_) => "internal_error",
        }
    }

    /// Create a not found error for a specific resource type.
    #[must_use]
    pub fn not_found<T: std::fmt::Display>(resource: &str, id: T) -> Self {
        Self::NotFound(format!("{resource} with ID {id} not found"))
    }

    /// Create a validation error with details.
    #[must_use]
    pub fn validation<T: Serialize>(message: &str, details: T) -> Self {
        Self::BadRequest(format!(
            "{}: {}",
            message,
            serde_json::to_string(&details).unwrap_or_default()
        ))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_type = self.error_type();
        let message = self.to_string();

        // Log internal errors
        match &self {
            Self::Internal(_)
            | Self::Other(_)
            | Self::Embedding(_)
            | Self::VectorIndex(_)
            | Self::Analysis(_)
            | Self::Interpretation(_) => {
                tracing::error!(error = %self, "Internal API error");
            }
            _ => {
                tracing::debug!(error = %self, "API error response");
            }
        }

        let body = ErrorResponse {
            error: error_type.to_string(),
            message,
            details: None,
            request_id: None,
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for API handlers.
pub type ApiResult<T> = Result<T, ApiError>;

/// Extension trait for adding context to errors.
pub trait ResultExt<T> {
    /// Convert error to `ApiError` with context.
    fn api_context(self, context: &str) -> ApiResult<T>;

    /// Convert to not found error if None.
    fn or_not_found(self, resource: &str, id: &str) -> ApiResult<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ResultExt<T> for Result<T, E> {
    fn api_context(self, context: &str) -> ApiResult<T> {
        self.map_err(|e| ApiError::Internal(format!("{context}: {e}")))
    }

    fn or_not_found(self, _resource: &str, _id: &str) -> ApiResult<T> {
        self.map_err(|e| ApiError::Internal(e.to_string()))
    }
}

impl<T> ResultExt<T> for Option<T> {
    fn api_context(self, context: &str) -> ApiResult<T> {
        self.ok_or_else(|| ApiError::Internal(format!("{context}: value was None")))
    }

    fn or_not_found(self, resource: &str, id: &str) -> ApiResult<T> {
        self.ok_or_else(|| ApiError::not_found(resource, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            ApiError::NotFound("test".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::BadRequest("test".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::RateLimitExceeded.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn test_not_found_helper() {
        let err = ApiError::not_found("Recording", "abc-123");
        assert!(err.to_string().contains("Recording"));
        assert!(err.to_string().contains("abc-123"));
    }

    #[test]
    fn test_error_response_serialization() {
        let response = ErrorResponse {
            error: "not_found".into(),
            message: "Resource not found".into(),
            details: None,
            request_id: Some("req-123".into()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("not_found"));
        assert!(json.contains("req-123"));
    }
}
