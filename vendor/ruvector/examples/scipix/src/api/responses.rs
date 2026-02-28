use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use super::jobs::JobStatus;

/// Standard text/OCR response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextResponse {
    /// Unique request identifier
    pub request_id: String,

    /// Recognized text
    pub text: String,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,

    /// LaTeX output (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex: Option<String>,

    /// MathML output (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mathml: Option<String>,

    /// HTML output (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

/// PDF processing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfResponse {
    /// PDF job identifier
    pub pdf_id: String,

    /// Current job status
    pub status: JobStatus,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Processing result (when completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,

    /// Error details (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code
    pub error_code: String,

    /// Human-readable error message
    pub message: String,

    /// HTTP status code
    #[serde(skip)]
    pub status: StatusCode,
}

impl ErrorResponse {
    /// Create a validation error response
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self {
            error_code: "VALIDATION_ERROR".to_string(),
            message: message.into(),
            status: StatusCode::BAD_REQUEST,
        }
    }

    /// Create an unauthorized error response
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            error_code: "UNAUTHORIZED".to_string(),
            message: message.into(),
            status: StatusCode::UNAUTHORIZED,
        }
    }

    /// Create a not found error response
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            error_code: "NOT_FOUND".to_string(),
            message: message.into(),
            status: StatusCode::NOT_FOUND,
        }
    }

    /// Create a rate limit error response
    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            error_code: "RATE_LIMIT_EXCEEDED".to_string(),
            message: message.into(),
            status: StatusCode::TOO_MANY_REQUESTS,
        }
    }

    /// Create an internal error response
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            error_code: "INTERNAL_ERROR".to_string(),
            message: message.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Create a service unavailable error response
    /// Used when the service is not fully configured (e.g., missing models)
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            error_code: "SERVICE_UNAVAILABLE".to_string(),
            message: message.into(),
            status: StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Create a not implemented error response
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self {
            error_code: "NOT_IMPLEMENTED".to_string(),
            message: message.into(),
            status: StatusCode::NOT_IMPLEMENTED,
        }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = self.status;
        (status, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_response_serialization() {
        let response = TextResponse {
            request_id: "test-123".to_string(),
            text: "Hello World".to_string(),
            confidence: 0.95,
            latex: Some("x^2".to_string()),
            mathml: None,
            html: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("request_id"));
        assert!(json.contains("test-123"));
        assert!(!json.contains("mathml"));
    }

    #[test]
    fn test_error_response_creation() {
        let error = ErrorResponse::validation_error("Invalid input");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.error_code, "VALIDATION_ERROR");

        let error = ErrorResponse::unauthorized("Invalid credentials");
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);

        let error = ErrorResponse::rate_limited("Too many requests");
        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
    }
}
