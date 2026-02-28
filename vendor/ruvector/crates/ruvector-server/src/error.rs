//! Error types for the ruvector server

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Result type for server operations
pub type Result<T> = std::result::Result<T, Error>;

/// Server error types
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Collection not found
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),

    /// Collection already exists
    #[error("Collection already exists: {0}")]
    CollectionExists(String),

    /// Point not found
    #[error("Point not found: {0}")]
    PointNotFound(String),

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Core library error
    #[error("Core error: {0}")]
    Core(#[from] ruvector_core::RuvectorError),

    /// Server error
    #[error("Server error: {0}")]
    Server(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::CollectionNotFound(_) | Error::PointNotFound(_) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            Error::CollectionExists(_) => (StatusCode::CONFLICT, self.to_string()),
            Error::InvalidRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Error::Core(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            Error::Server(_) | Error::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            Error::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Serialization(e) => (StatusCode::BAD_REQUEST, e.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
