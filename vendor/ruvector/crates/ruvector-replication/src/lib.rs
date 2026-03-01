//! Data replication and synchronization for ruvector
//!
//! This crate provides comprehensive replication capabilities including:
//! - Multi-node replica management
//! - Synchronous, asynchronous, and semi-synchronous replication modes
//! - Conflict resolution with vector clocks and CRDTs
//! - Change data capture and streaming
//! - Automatic failover and split-brain prevention
//!
//! # Examples
//!
//! ```no_run
//! use ruvector_replication::{ReplicaSet, ReplicaRole, SyncMode, SyncManager, ReplicationLog};
//! use std::sync::Arc;
//!
//! fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a replica set
//!     let mut replica_set = ReplicaSet::new("cluster-1");
//!
//!     // Add replicas
//!     replica_set.add_replica("replica-1", "192.168.1.10:9001", ReplicaRole::Primary)?;
//!     replica_set.add_replica("replica-2", "192.168.1.11:9001", ReplicaRole::Secondary)?;
//!
//!     // Create sync manager and configure synchronization
//!     let log = Arc::new(ReplicationLog::new("replica-1"));
//!     let manager = SyncManager::new(Arc::new(replica_set), log);
//!     manager.set_sync_mode(SyncMode::SemiSync { min_replicas: 1 });
//!     Ok(())
//! }
//! ```

pub mod conflict;
pub mod failover;
pub mod replica;
pub mod stream;
pub mod sync;

pub use conflict::{ConflictResolver, LastWriteWins, MergeFunction, VectorClock};
pub use failover::{FailoverManager, FailoverPolicy, HealthStatus};
pub use replica::{Replica, ReplicaRole, ReplicaSet, ReplicaStatus};
pub use stream::{ChangeEvent, ChangeOperation, ReplicationStream};
pub use sync::{LogEntry, ReplicationLog, SyncManager, SyncMode};

use thiserror::Error;

/// Result type for replication operations
pub type Result<T> = std::result::Result<T, ReplicationError>;

/// Errors that can occur during replication operations
#[derive(Error, Debug)]
pub enum ReplicationError {
    #[error("Replica not found: {0}")]
    ReplicaNotFound(String),

    #[error("No primary replica available")]
    NoPrimary,

    #[error("Replication timeout: {0}")]
    Timeout(String),

    #[error("Synchronization failed: {0}")]
    SyncFailed(String),

    #[error("Conflict resolution failed: {0}")]
    ConflictResolution(String),

    #[error("Failover failed: {0}")]
    FailoverFailed(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Quorum not met: needed {needed}, got {available}")]
    QuorumNotMet { needed: usize, available: usize },

    #[error("Split-brain detected")]
    SplitBrain,

    #[error("Invalid replica state: {0}")]
    InvalidState(String),

    #[error("Serialization encode error: {0}")]
    SerializationEncode(#[from] bincode::error::EncodeError),

    #[error("Serialization decode error: {0}")]
    SerializationDecode(#[from] bincode::error::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ReplicationError::QuorumNotMet {
            needed: 2,
            available: 1,
        };
        assert_eq!(err.to_string(), "Quorum not met: needed 2, got 1");
    }
}
