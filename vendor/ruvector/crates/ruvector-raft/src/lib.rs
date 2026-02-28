//! Raft consensus implementation for ruvector distributed metadata
//!
//! This crate provides a production-ready Raft consensus implementation
//! following the Raft paper specification for managing distributed metadata
//! in the ruvector vector database.

pub mod election;
pub mod log;
pub mod node;
pub mod rpc;
pub mod state;

pub use node::{RaftNode, RaftNodeConfig};
pub use rpc::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    RequestVoteRequest, RequestVoteResponse,
};
pub use state::{LeaderState, PersistentState, RaftState, VolatileState};

use thiserror::Error;

/// Result type for Raft operations
pub type RaftResult<T> = Result<T, RaftError>;

/// Errors that can occur during Raft operations
#[derive(Debug, Error)]
pub enum RaftError {
    #[error("Node is not the leader")]
    NotLeader,

    #[error("No leader available")]
    NoLeader,

    #[error("Invalid term: {0}")]
    InvalidTerm(u64),

    #[error("Invalid log index: {0}")]
    InvalidLogIndex(u64),

    #[error("Serialization error: {0}")]
    SerializationEncodeError(#[from] bincode::error::EncodeError),

    #[error("Deserialization error: {0}")]
    SerializationDecodeError(#[from] bincode::error::DecodeError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Election timeout")]
    ElectionTimeout,

    #[error("Log inconsistency detected")]
    LogInconsistency,

    #[error("Snapshot installation failed: {0}")]
    SnapshotFailed(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Node identifier type
pub type NodeId = String;

/// Term number in Raft consensus
pub type Term = u64;

/// Log index in Raft log
pub type LogIndex = u64;
