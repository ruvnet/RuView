//! Error types for the Prime-Radiant coherence engine.
//!
//! This module provides a hierarchical error structure with domain-specific
//! error types for each bounded context.

use crate::types::{EdgeId, NodeId, PolicyBundleId, ScopeId, WitnessId};
use thiserror::Error;

// ============================================================================
// TOP-LEVEL ERROR
// ============================================================================

/// Top-level error type for the coherence engine
#[derive(Debug, Error)]
pub enum CoherenceError {
    /// Error in the knowledge substrate
    #[error("Substrate error: {0}")]
    Substrate(#[from] SubstrateError),

    /// Error in coherence computation
    #[error("Computation error: {0}")]
    Computation(#[from] ComputationError),

    /// Error in governance layer
    #[error("Governance error: {0}")]
    Governance(#[from] GovernanceError),

    /// Error in action execution
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    /// Error in storage layer
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal error (should not happen in normal operation)
    #[error("Internal error: {0}")]
    Internal(String),
}

// ============================================================================
// SUBSTRATE ERRORS
// ============================================================================

/// Errors related to the knowledge substrate (sheaf graph)
#[derive(Debug, Error)]
pub enum SubstrateError {
    /// Node not found in graph
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    /// Edge not found in graph
    #[error("Edge not found: {0}")]
    EdgeNotFound(EdgeId),

    /// Dimension mismatch in state vectors or restriction maps
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Invalid restriction map (not compatible with node dimensions)
    #[error("Invalid restriction map: {0}")]
    InvalidRestrictionMap(String),

    /// Graph is in an inconsistent state
    #[error("Graph inconsistent: {0}")]
    GraphInconsistent(String),

    /// Node already exists
    #[error("Node already exists: {0}")]
    NodeAlreadyExists(NodeId),

    /// Edge already exists
    #[error("Edge already exists: {0}")]
    EdgeAlreadyExists(EdgeId),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// ============================================================================
// COMPUTATION ERRORS
// ============================================================================

/// Errors related to coherence computation
#[derive(Debug, Error)]
pub enum ComputationError {
    /// Residual computation failed
    #[error("Residual computation failed for edge {edge}: {reason}")]
    ResidualFailed {
        /// The edge that failed
        edge: EdgeId,
        /// Reason for failure
        reason: String,
    },

    /// Energy aggregation failed
    #[error("Energy aggregation failed: {0}")]
    AggregationFailed(String),

    /// Spectral analysis failed
    #[error("Spectral analysis failed: {0}")]
    SpectralFailed(String),

    /// Fingerprint mismatch (cache invalidation)
    #[error("Fingerprint mismatch: cached {cached}, current {current}")]
    FingerprintMismatch {
        /// Cached fingerprint
        cached: String,
        /// Current fingerprint
        current: String,
    },

    /// Numerical instability detected
    #[error("Numerical instability: {0}")]
    NumericalInstability(String),
}

// ============================================================================
// GOVERNANCE ERRORS
// ============================================================================

/// Errors related to the governance layer
#[derive(Debug, Error)]
pub enum GovernanceError {
    /// Policy bundle not found
    #[error("Policy bundle not found: {0}")]
    PolicyNotFound(PolicyBundleId),

    /// Policy bundle already activated (cannot modify)
    #[error("Policy bundle already activated: {0}")]
    PolicyAlreadyActivated(PolicyBundleId),

    /// Policy bundle not approved (cannot activate)
    #[error("Policy bundle not approved: {0}")]
    PolicyNotApproved(PolicyBundleId),

    /// Invalid signature
    #[error("Invalid signature from approver")]
    InvalidSignature,

    /// Insufficient approvals
    #[error("Insufficient approvals: required {required}, got {actual}")]
    InsufficientApprovals {
        /// Required number of approvals
        required: usize,
        /// Actual number of approvals
        actual: usize,
    },

    /// Witness chain broken
    #[error("Witness chain broken: expected previous {expected:?}, got {actual:?}")]
    WitnessChainBroken {
        /// Expected previous witness
        expected: Option<WitnessId>,
        /// Actual previous witness
        actual: Option<WitnessId>,
    },

    /// Witness not found
    #[error("Witness not found: {0}")]
    WitnessNotFound(WitnessId),

    /// Witness integrity check failed
    #[error("Witness integrity check failed: {0}")]
    WitnessIntegrityFailed(WitnessId),

    /// Threshold configuration invalid
    #[error("Invalid threshold configuration: {0}")]
    InvalidThreshold(String),

    /// Scope pattern invalid
    #[error("Invalid scope pattern: {0}")]
    InvalidScopePattern(String),
}

// ============================================================================
// EXECUTION ERRORS
// ============================================================================

/// Errors related to action execution
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// Action denied by coherence gate
    #[error("Action denied: {reason} (witness: {witness_id})")]
    Denied {
        /// Witness ID for the denial
        witness_id: WitnessId,
        /// Reason for denial
        reason: String,
    },

    /// Escalation required
    #[error("Escalation required to lane {lane}: {reason}")]
    EscalationRequired {
        /// Required compute lane
        lane: u8,
        /// Reason for escalation
        reason: String,
    },

    /// Action execution failed
    #[error("Action execution failed: {0}")]
    ActionFailed(String),

    /// No policy bundle configured
    #[error("No policy bundle configured for scope: {0}")]
    NoPolicyConfigured(ScopeId),

    /// Policy bundle expired
    #[error("Policy bundle expired: {0}")]
    PolicyExpired(PolicyBundleId),

    /// Timeout waiting for escalation response
    #[error("Escalation timeout after {timeout_ms}ms")]
    EscalationTimeout {
        /// Timeout in milliseconds
        timeout_ms: u64,
    },

    /// Human review required but not available
    #[error("Human review required but not available")]
    HumanReviewUnavailable,
}

// ============================================================================
// STORAGE ERRORS
// ============================================================================

/// Errors related to the storage layer
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database connection failed
    #[error("Database connection failed: {0}")]
    ConnectionFailed(String),

    /// Query execution failed
    #[error("Query failed: {0}")]
    QueryFailed(String),

    /// Transaction failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    /// Record not found
    #[error("Record not found: {entity_type} with id {id}")]
    NotFound {
        /// Type of entity
        entity_type: String,
        /// Entity ID
        id: String,
    },

    /// Duplicate key violation
    #[error("Duplicate key: {0}")]
    DuplicateKey(String),

    /// Serialization failed
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// Deserialization failed
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    /// Event log error
    #[error("Event log error: {0}")]
    EventLogError(String),

    /// Replay failed
    #[error("Replay failed at sequence {sequence}: {reason}")]
    ReplayFailed {
        /// Sequence number where replay failed
        sequence: u64,
        /// Reason for failure
        reason: String,
    },
}

// ============================================================================
// RESULT TYPE ALIAS
// ============================================================================

/// Result type alias for coherence operations
pub type Result<T> = std::result::Result<T, CoherenceError>;

/// Result type alias for substrate operations
pub type SubstrateResult<T> = std::result::Result<T, SubstrateError>;

/// Result type alias for computation operations
pub type ComputationResult<T> = std::result::Result<T, ComputationError>;

/// Result type alias for governance operations
pub type GovernanceResult<T> = std::result::Result<T, GovernanceError>;

/// Result type alias for execution operations
pub type ExecutionResult<T> = std::result::Result<T, ExecutionError>;

/// Result type alias for storage operations
pub type StorageResult<T> = std::result::Result<T, StorageError>;

// ============================================================================
// ERROR CONVERSION UTILITIES
// ============================================================================

impl From<bincode::error::EncodeError> for SubstrateError {
    fn from(e: bincode::error::EncodeError) -> Self {
        Self::Serialization(e.to_string())
    }
}

impl From<bincode::error::DecodeError> for SubstrateError {
    fn from(e: bincode::error::DecodeError) -> Self {
        Self::Serialization(e.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationFailed(e.to_string())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SubstrateError::DimensionMismatch {
            expected: 64,
            actual: 32,
        };
        assert!(err.to_string().contains("64"));
        assert!(err.to_string().contains("32"));
    }

    #[test]
    fn test_error_conversion() {
        let substrate_err = SubstrateError::NodeNotFound(NodeId::new());
        let coherence_err: CoherenceError = substrate_err.into();
        assert!(matches!(coherence_err, CoherenceError::Substrate(_)));
    }
}
