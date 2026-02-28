use thiserror::Error;

/// Errors that can occur during filter operations
#[derive(Error, Debug)]
pub enum FilterError {
    #[error("Index not found for field: {0}")]
    IndexNotFound(String),

    #[error("Invalid index type for field: {0}")]
    InvalidIndexType(String),

    #[error("Type mismatch in filter expression: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Invalid filter expression: {0}")]
    InvalidExpression(String),

    #[error("Field not found in payload: {0}")]
    FieldNotFound(String),

    #[error("Invalid value for operation: {0}")]
    InvalidValue(String),

    #[error("Geo operation error: {0}")]
    GeoError(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, FilterError>;
