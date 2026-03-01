//! Error types for the ruQu coherence gate system
//!
//! This module defines all error types that can occur during coherence
//! assessment, syndrome processing, and gate decision-making.

use thiserror::Error;

/// Result type alias for ruQu operations
pub type Result<T> = std::result::Result<T, RuQuError>;

/// Main error type for ruQu operations
#[derive(Error, Debug)]
pub enum RuQuError {
    // ═══════════════════════════════════════════════════════════════════════
    // Gate Decision Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Filter evaluation failed
    #[error("Filter evaluation failed: {filter} - {reason}")]
    FilterEvaluationFailed {
        /// Which filter failed
        filter: String,
        /// Reason for failure
        reason: String,
    },

    /// Gate decision timeout exceeded
    #[error("Gate decision timeout: {elapsed_ns}ns exceeded {budget_ns}ns budget")]
    DecisionTimeout {
        /// Time elapsed in nanoseconds
        elapsed_ns: u64,
        /// Budget in nanoseconds
        budget_ns: u64,
    },

    /// Invalid threshold configuration
    #[error("Invalid threshold: {name} = {value} (expected {constraint})")]
    InvalidThreshold {
        /// Threshold name
        name: String,
        /// Actual value
        value: f64,
        /// Expected constraint description
        constraint: String,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // Tile Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Invalid tile identifier
    #[error("Invalid tile ID: {0} (valid range: 0-255)")]
    InvalidTileId(u16),

    /// Tile not found
    #[error("Tile {0} not found in fabric")]
    TileNotFound(u8),

    /// Tile communication failure
    #[error("Tile communication failed: tile {tile_id} - {reason}")]
    TileCommunicationFailed {
        /// Tile that failed
        tile_id: u8,
        /// Reason for failure
        reason: String,
    },

    /// Tile memory exceeded
    #[error("Tile {tile_id} memory exceeded: {used} bytes > {limit} bytes")]
    TileMemoryExceeded {
        /// Tile ID
        tile_id: u8,
        /// Memory used
        used: usize,
        /// Memory limit
        limit: usize,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // Syndrome Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Syndrome buffer overflow
    #[error("Syndrome buffer overflow: capacity {capacity}, attempted write at {position}")]
    SyndromeBufferOverflow {
        /// Buffer capacity
        capacity: usize,
        /// Attempted write position
        position: usize,
    },

    /// Invalid syndrome round
    #[error("Invalid syndrome round: {0}")]
    InvalidSyndromeRound(String),

    /// Syndrome gap detected (missing rounds)
    #[error("Syndrome gap: expected round {expected}, got {actual}")]
    SyndromeGap {
        /// Expected round ID
        expected: u64,
        /// Actual round ID received
        actual: u64,
    },

    /// Detector map mismatch
    #[error("Detector count mismatch: expected {expected}, got {actual}")]
    DetectorCountMismatch {
        /// Expected detector count
        expected: usize,
        /// Actual detector count
        actual: usize,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // Graph Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Graph vertex not found
    #[error("Vertex {0} not found in operational graph")]
    VertexNotFound(u64),

    /// Graph edge not found
    #[error("Edge {0} not found in operational graph")]
    EdgeNotFound(u64),

    /// Invalid graph update
    #[error("Invalid graph update: {0}")]
    InvalidGraphUpdate(String),

    /// Graph version conflict
    #[error("Graph version conflict: expected {expected}, current {current}")]
    GraphVersionConflict {
        /// Expected version
        expected: u64,
        /// Current version
        current: u64,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // Permit/Token Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Permit token expired
    #[error("Permit token expired: expired at {expired_at}, current time {current_time}")]
    PermitExpired {
        /// Expiration timestamp
        expired_at: u64,
        /// Current timestamp
        current_time: u64,
    },

    /// Permit signature invalid
    #[error("Permit signature verification failed")]
    PermitSignatureInvalid,

    /// Permit witness hash mismatch
    #[error("Permit witness hash mismatch")]
    PermitWitnessMismatch,

    /// Action not authorized by permit
    #[error("Action {action_id} not authorized by permit for regions {region_mask:?}")]
    ActionNotAuthorized {
        /// Action ID
        action_id: String,
        /// Region mask from permit
        region_mask: [u64; 4],
    },

    // ═══════════════════════════════════════════════════════════════════════
    // Witness/Receipt Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Witness chain broken
    #[error("Witness chain broken at sequence {sequence}")]
    WitnessChainBroken {
        /// Sequence where chain broke
        sequence: u64,
    },

    /// Receipt not found
    #[error("Receipt not found for sequence {0}")]
    ReceiptNotFound(u64),

    /// Receipt verification failed
    #[error("Receipt verification failed at sequence {sequence}: {reason}")]
    ReceiptVerificationFailed {
        /// Sequence number
        sequence: u64,
        /// Failure reason
        reason: String,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // Fabric Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Fabric not initialized
    #[error("Quantum fabric not initialized")]
    FabricNotInitialized,

    /// Fabric configuration invalid
    #[error("Invalid fabric configuration: {0}")]
    InvalidFabricConfig(String),

    /// Fabric synchronization failed
    #[error("Fabric synchronization failed: {0}")]
    FabricSyncFailed(String),

    // ═══════════════════════════════════════════════════════════════════════
    // Integration Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// MinCut integration error
    #[error("MinCut error: {0}")]
    MinCutError(String),

    /// TileZero integration error
    #[error("TileZero error: {0}")]
    TileZeroError(String),

    // ═══════════════════════════════════════════════════════════════════════
    // General Errors
    // ═══════════════════════════════════════════════════════════════════════
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl RuQuError {
    /// Check if error is recoverable (can retry operation)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RuQuError::DecisionTimeout { .. }
                | RuQuError::TileCommunicationFailed { .. }
                | RuQuError::SyndromeGap { .. }
                | RuQuError::FabricSyncFailed(_)
        )
    }

    /// Check if error indicates data corruption
    pub fn is_corruption(&self) -> bool {
        matches!(
            self,
            RuQuError::WitnessChainBroken { .. }
                | RuQuError::ReceiptVerificationFailed { .. }
                | RuQuError::PermitSignatureInvalid
                | RuQuError::PermitWitnessMismatch
        )
    }

    /// Check if error is a configuration problem
    pub fn is_configuration(&self) -> bool {
        matches!(
            self,
            RuQuError::InvalidThreshold { .. }
                | RuQuError::InvalidFabricConfig(_)
                | RuQuError::DetectorCountMismatch { .. }
        )
    }

    /// Check if error is resource-related
    pub fn is_resource(&self) -> bool {
        matches!(
            self,
            RuQuError::TileMemoryExceeded { .. } | RuQuError::SyndromeBufferOverflow { .. }
        )
    }
}

impl From<serde_json::Error> for RuQuError {
    fn from(err: serde_json::Error) -> Self {
        RuQuError::Serialization(err.to_string())
    }
}

impl From<String> for RuQuError {
    fn from(msg: String) -> Self {
        RuQuError::Internal(msg)
    }
}

impl From<&str> for RuQuError {
    fn from(msg: &str) -> Self {
        RuQuError::Internal(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RuQuError::InvalidTileId(300);
        assert_eq!(err.to_string(), "Invalid tile ID: 300 (valid range: 0-255)");

        let err = RuQuError::DecisionTimeout {
            elapsed_ns: 5000,
            budget_ns: 4000,
        };
        assert!(err.to_string().contains("5000ns"));
        assert!(err.to_string().contains("4000ns"));
    }

    #[test]
    fn test_is_recoverable() {
        assert!(RuQuError::DecisionTimeout {
            elapsed_ns: 5000,
            budget_ns: 4000
        }
        .is_recoverable());

        assert!(RuQuError::TileCommunicationFailed {
            tile_id: 1,
            reason: "timeout".to_string()
        }
        .is_recoverable());

        assert!(!RuQuError::PermitSignatureInvalid.is_recoverable());
    }

    #[test]
    fn test_is_corruption() {
        assert!(RuQuError::WitnessChainBroken { sequence: 42 }.is_corruption());
        assert!(RuQuError::PermitSignatureInvalid.is_corruption());
        assert!(!RuQuError::InvalidTileId(300).is_corruption());
    }

    #[test]
    fn test_is_configuration() {
        assert!(RuQuError::InvalidThreshold {
            name: "tau_deny".to_string(),
            value: -1.0,
            constraint: "> 0".to_string()
        }
        .is_configuration());

        assert!(!RuQuError::Internal("oops".to_string()).is_configuration());
    }

    #[test]
    fn test_from_string() {
        let err: RuQuError = "test error".into();
        assert!(matches!(err, RuQuError::Internal(_)));
        assert_eq!(err.to_string(), "Internal error: test error");
    }
}
