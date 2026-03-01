//! Error types for collection management

use thiserror::Error;

/// Result type for collection operations
pub type Result<T> = std::result::Result<T, CollectionError>;

/// Errors that can occur during collection management
#[derive(Debug, Error)]
pub enum CollectionError {
    /// Collection was not found
    #[error("Collection not found: {name}")]
    CollectionNotFound {
        /// Name of the missing collection
        name: String,
    },

    /// Collection already exists
    #[error("Collection already exists: {name}")]
    CollectionAlreadyExists {
        /// Name of the existing collection
        name: String,
    },

    /// Alias was not found
    #[error("Alias not found: {alias}")]
    AliasNotFound {
        /// Name of the missing alias
        alias: String,
    },

    /// Alias already exists
    #[error("Alias already exists: {alias}")]
    AliasAlreadyExists {
        /// Name of the existing alias
        alias: String,
    },

    /// Invalid collection configuration
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration {
        /// Error message
        message: String,
    },

    /// Alias points to non-existent collection
    #[error("Alias '{alias}' points to non-existent collection '{collection}'")]
    InvalidAlias {
        /// Alias name
        alias: String,
        /// Target collection name
        collection: String,
    },

    /// Cannot delete collection with active aliases
    #[error("Cannot delete collection '{collection}' because it has active aliases: {aliases:?}")]
    CollectionHasAliases {
        /// Collection name
        collection: String,
        /// List of aliases
        aliases: Vec<String>,
    },

    /// Invalid collection name
    #[error("Invalid collection name: {name} - {reason}")]
    InvalidName {
        /// Collection name
        name: String,
        /// Reason for invalidity
        reason: String,
    },

    /// Core database error
    #[error("Database error: {0}")]
    DatabaseError(#[from] ruvector_core::error::RuvectorError),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl From<serde_json::Error> for CollectionError {
    fn from(err: serde_json::Error) -> Self {
        CollectionError::SerializationError(err.to_string())
    }
}

impl From<bincode::error::EncodeError> for CollectionError {
    fn from(err: bincode::error::EncodeError) -> Self {
        CollectionError::SerializationError(err.to_string())
    }
}

impl From<bincode::error::DecodeError> for CollectionError {
    fn from(err: bincode::error::DecodeError) -> Self {
        CollectionError::SerializationError(err.to_string())
    }
}
