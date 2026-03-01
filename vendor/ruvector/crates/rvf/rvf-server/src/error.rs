//! Error types and HTTP error responses for the RVF server.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Application-level error type.
#[derive(Debug)]
pub enum ServerError {
    /// The store returned an error.
    Store(rvf_types::RvfError),
    /// A request body could not be deserialized.
    BadRequest(String),
    /// The store is not initialized yet.
    NotReady,
}

/// JSON body returned on error.
#[derive(Serialize)]
struct ErrorBody {
    error: String,
    code: u16,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message, code) = match &self {
            ServerError::Store(e) => {
                let code = error_code(e);
                let status = status_for_error(e);
                (status, format!("{e:?}"), code)
            }
            ServerError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone(), 400),
            ServerError::NotReady => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Store not ready".into(),
                503,
            ),
        };

        let body = ErrorBody {
            error: message,
            code,
        };

        (status, axum::Json(body)).into_response()
    }
}

fn error_code(e: &rvf_types::RvfError) -> u16 {
    match e {
        rvf_types::RvfError::Code(c) => *c as u16,
        _ => 500,
    }
}

fn status_for_error(e: &rvf_types::RvfError) -> StatusCode {
    match e {
        rvf_types::RvfError::Code(c) => match c {
            rvf_types::ErrorCode::DimensionMismatch => StatusCode::BAD_REQUEST,
            rvf_types::ErrorCode::ReadOnly => StatusCode::FORBIDDEN,
            rvf_types::ErrorCode::LockHeld => StatusCode::CONFLICT,
            rvf_types::ErrorCode::ManifestNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        },
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl From<rvf_types::RvfError> for ServerError {
    fn from(e: rvf_types::RvfError) -> Self {
        ServerError::Store(e)
    }
}
