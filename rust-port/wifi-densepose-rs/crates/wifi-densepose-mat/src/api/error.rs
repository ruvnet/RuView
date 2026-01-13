//! API error types and handling for the MAT REST API.
//!
//! This module provides a unified error type that maps to appropriate HTTP status codes
//! and JSON error responses for the API.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

/// API error type that converts to HTTP responses.
///
/// All errors include:
/// - An HTTP status code
/// - A machine-readable error code
/// - A human-readable message
/// - Optional additional details
#[derive(Debug, Error)]
pub enum ApiError {
    /// Resource not found (404)
    #[error("Resource not found: {resource_type} with id {id}")]
    NotFound {
        resource_type: String,
        id: String,
    },

    /// Invalid request data (400)
    #[error("Bad request: {message}")]
    BadRequest {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Validation error (422)
    #[error("Validation failed: {message}")]
    ValidationError {
        message: String,
        field: Option<String>,
    },

    /// Conflict with existing resource (409)
    #[error("Conflict: {message}")]
    Conflict {
        message: String,
    },

    /// Resource is in invalid state for operation (409)
    #[error("Invalid state: {message}")]
    InvalidState {
        message: String,
        current_state: String,
    },

    /// Internal server error (500)
    #[error("Internal error: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Service unavailable (503)
    #[error("Service unavailable: {message}")]
    ServiceUnavailable {
        message: String,
    },

    /// Domain error from business logic
    #[error("Domain error: {0}")]
    Domain(#[from] crate::MatError),
}

impl ApiError {
    /// Create a not found error for an event.
    pub fn event_not_found(id: Uuid) -> Self {
        Self::NotFound {
            resource_type: "DisasterEvent".to_string(),
            id: id.to_string(),
        }
    }

    /// Create a not found error for a zone.
    pub fn zone_not_found(id: Uuid) -> Self {
        Self::NotFound {
            resource_type: "ScanZone".to_string(),
            id: id.to_string(),
        }
    }

    /// Create a not found error for a survivor.
    pub fn survivor_not_found(id: Uuid) -> Self {
        Self::NotFound {
            resource_type: "Survivor".to_string(),
            id: id.to_string(),
        }
    }

    /// Create a not found error for an alert.
    pub fn alert_not_found(id: Uuid) -> Self {
        Self::NotFound {
            resource_type: "Alert".to_string(),
            id: id.to_string(),
        }
    }

    /// Create a bad request error.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
            source: None,
        }
    }

    /// Create a validation error.
    pub fn validation(message: impl Into<String>, field: Option<String>) -> Self {
        Self::ValidationError {
            message: message.into(),
            field,
        }
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// Get the HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::ValidationError { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::InvalidState { .. } => StatusCode::CONFLICT,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::Domain(_) => StatusCode::BAD_REQUEST,
        }
    }

    /// Get the error code for this error.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "NOT_FOUND",
            Self::BadRequest { .. } => "BAD_REQUEST",
            Self::ValidationError { .. } => "VALIDATION_ERROR",
            Self::Conflict { .. } => "CONFLICT",
            Self::InvalidState { .. } => "INVALID_STATE",
            Self::Internal { .. } => "INTERNAL_ERROR",
            Self::ServiceUnavailable { .. } => "SERVICE_UNAVAILABLE",
            Self::Domain(_) => "DOMAIN_ERROR",
        }
    }
}

/// JSON error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Machine-readable error code
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
    /// Request ID for tracing (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Additional error details.
#[derive(Debug, Serialize)]
pub struct ErrorDetails {
    /// Resource type involved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    /// Resource ID involved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    /// Field that caused the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Current state (for state errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_state: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.error_code().to_string();
        let message = self.to_string();

        let details = match &self {
            ApiError::NotFound { resource_type, id } => Some(ErrorDetails {
                resource_type: Some(resource_type.clone()),
                resource_id: Some(id.clone()),
                field: None,
                current_state: None,
            }),
            ApiError::ValidationError { field, .. } => Some(ErrorDetails {
                resource_type: None,
                resource_id: None,
                field: field.clone(),
                current_state: None,
            }),
            ApiError::InvalidState { current_state, .. } => Some(ErrorDetails {
                resource_type: None,
                resource_id: None,
                field: None,
                current_state: Some(current_state.clone()),
            }),
            _ => None,
        };

        // Log errors
        match &self {
            ApiError::Internal { source, .. } | ApiError::BadRequest { source, .. } => {
                if let Some(src) = source {
                    tracing::error!(error = %self, source = %src, "API error");
                } else {
                    tracing::error!(error = %self, "API error");
                }
            }
            _ => {
                tracing::warn!(error = %self, "API error");
            }
        }

        let body = ErrorResponse {
            code,
            message,
            details,
            request_id: None, // Would be populated from request extension
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for API handlers.
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        let not_found = ApiError::event_not_found(Uuid::new_v4());
        assert_eq!(not_found.status_code(), StatusCode::NOT_FOUND);

        let bad_request = ApiError::bad_request("test");
        assert_eq!(bad_request.status_code(), StatusCode::BAD_REQUEST);

        let internal = ApiError::internal("test");
        assert_eq!(internal.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_codes() {
        let not_found = ApiError::event_not_found(Uuid::new_v4());
        assert_eq!(not_found.error_code(), "NOT_FOUND");

        let validation = ApiError::validation("test", Some("field".to_string()));
        assert_eq!(validation.error_code(), "VALIDATION_ERROR");
    }
}
