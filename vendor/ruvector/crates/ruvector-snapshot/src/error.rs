use thiserror::Error;

/// Result type for snapshot operations
pub type Result<T> = std::result::Result<T, SnapshotError>;

/// Errors that can occur during snapshot operations
#[derive(Error, Debug)]
pub enum SnapshotError {
    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),

    #[error("Corrupted snapshot: {0}")]
    CorruptedSnapshot(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid checksum: expected {expected}, got {actual}")]
    InvalidChecksum { expected: String, actual: String },

    #[error("Collection error: {0}")]
    CollectionError(String),
}

impl SnapshotError {
    /// Create a storage error with a custom message
    pub fn storage<S: Into<String>>(msg: S) -> Self {
        SnapshotError::StorageError(msg.into())
    }

    /// Create a corrupted snapshot error with a custom message
    pub fn corrupted<S: Into<String>>(msg: S) -> Self {
        SnapshotError::CorruptedSnapshot(msg.into())
    }

    /// Create a compression error with a custom message
    pub fn compression<S: Into<String>>(msg: S) -> Self {
        SnapshotError::CompressionError(msg.into())
    }
}
