//! Error types for the tiles integration module.

use thiserror::Error;

/// Result type for tiles operations.
pub type TilesResult<T> = Result<T, TilesError>;

/// Errors that can occur in tile operations.
#[derive(Debug, Error)]
pub enum TilesError {
    /// Tile ID out of valid range (0-255).
    #[error("tile ID {0} out of range (must be 0-255)")]
    TileIdOutOfRange(u16),

    /// Delta buffer is full.
    #[error("delta buffer full for tile {tile_id}, capacity: {capacity}")]
    DeltaBufferFull {
        /// The tile that rejected the delta.
        tile_id: u8,
        /// The buffer capacity.
        capacity: usize,
    },

    /// Tile not initialized.
    #[error("tile {0} not initialized")]
    TileNotInitialized(u8),

    /// Tile in error state.
    #[error("tile {tile_id} in error state: {reason}")]
    TileError {
        /// The tile in error.
        tile_id: u8,
        /// Reason for the error.
        reason: String,
    },

    /// Invalid node ID for shard mapping.
    #[error("invalid node ID {0} for shard mapping")]
    InvalidNodeId(u64),

    /// Witness aggregation failed.
    #[error("witness aggregation failed: {0}")]
    WitnessAggregationFailed(String),

    /// Fabric not started.
    #[error("fabric not started")]
    FabricNotStarted,

    /// Fabric already running.
    #[error("fabric already running")]
    FabricAlreadyRunning,

    /// Coordination error.
    #[error("coordination error: {0}")]
    CoordinationError(String),

    /// Invalid fabric configuration.
    #[error("invalid fabric configuration: {0}")]
    InvalidConfiguration(String),

    /// Tick processing error.
    #[error("tick {tick_number} processing failed: {reason}")]
    TickProcessingFailed {
        /// The tick that failed.
        tick_number: u32,
        /// Reason for the failure.
        reason: String,
    },

    /// Internal error.
    #[error("internal tiles error: {0}")]
    Internal(String),
}

impl TilesError {
    /// Create a new tile error.
    #[must_use]
    pub fn tile_error(tile_id: u8, reason: impl Into<String>) -> Self {
        Self::TileError {
            tile_id,
            reason: reason.into(),
        }
    }

    /// Create a delta buffer full error.
    #[must_use]
    pub fn buffer_full(tile_id: u8, capacity: usize) -> Self {
        Self::DeltaBufferFull { tile_id, capacity }
    }

    /// Create a tick processing failed error.
    #[must_use]
    pub fn tick_failed(tick_number: u32, reason: impl Into<String>) -> Self {
        Self::TickProcessingFailed {
            tick_number,
            reason: reason.into(),
        }
    }
}
