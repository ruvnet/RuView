//! Raft RPC messages
//!
//! Defines the RPC message types for Raft consensus:
//! - AppendEntries (log replication and heartbeat)
//! - RequestVote (leader election)
//! - InstallSnapshot (snapshot transfer)

use crate::{log::LogEntry, log::Snapshot, LogIndex, NodeId, Term};
use serde::{Deserialize, Serialize};

/// AppendEntries RPC request
///
/// Invoked by leader to replicate log entries; also used as heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    /// Leader's term
    pub term: Term,

    /// Leader's ID (so followers can redirect clients)
    pub leader_id: NodeId,

    /// Index of log entry immediately preceding new ones
    pub prev_log_index: LogIndex,

    /// Term of prevLogIndex entry
    pub prev_log_term: Term,

    /// Log entries to store (empty for heartbeat)
    pub entries: Vec<LogEntry>,

    /// Leader's commitIndex
    pub leader_commit: LogIndex,
}

impl AppendEntriesRequest {
    /// Create a new AppendEntries request
    pub fn new(
        term: Term,
        leader_id: NodeId,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: Vec<LogEntry>,
        leader_commit: LogIndex,
    ) -> Self {
        Self {
            term,
            leader_id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
        }
    }

    /// Create a heartbeat (AppendEntries with no entries)
    pub fn heartbeat(term: Term, leader_id: NodeId, leader_commit: LogIndex) -> Self {
        Self {
            term,
            leader_id,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: Vec::new(),
            leader_commit,
        }
    }

    /// Check if this is a heartbeat message
    pub fn is_heartbeat(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

/// AppendEntries RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    /// Current term, for leader to update itself
    pub term: Term,

    /// True if follower contained entry matching prevLogIndex and prevLogTerm
    pub success: bool,

    /// The follower's last log index (for optimization)
    pub match_index: Option<LogIndex>,

    /// Conflict information for faster log backtracking
    pub conflict_index: Option<LogIndex>,
    pub conflict_term: Option<Term>,
}

impl AppendEntriesResponse {
    /// Create a successful response
    pub fn success(term: Term, match_index: LogIndex) -> Self {
        Self {
            term,
            success: true,
            match_index: Some(match_index),
            conflict_index: None,
            conflict_term: None,
        }
    }

    /// Create a failure response
    pub fn failure(
        term: Term,
        conflict_index: Option<LogIndex>,
        conflict_term: Option<Term>,
    ) -> Self {
        Self {
            term,
            success: false,
            match_index: None,
            conflict_index,
            conflict_term,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

/// RequestVote RPC request
///
/// Invoked by candidates to gather votes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteRequest {
    /// Candidate's term
    pub term: Term,

    /// Candidate requesting vote
    pub candidate_id: NodeId,

    /// Index of candidate's last log entry
    pub last_log_index: LogIndex,

    /// Term of candidate's last log entry
    pub last_log_term: Term,
}

impl RequestVoteRequest {
    /// Create a new RequestVote request
    pub fn new(
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) -> Self {
        Self {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

/// RequestVote RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteResponse {
    /// Current term, for candidate to update itself
    pub term: Term,

    /// True means candidate received vote
    pub vote_granted: bool,
}

impl RequestVoteResponse {
    /// Create a vote granted response
    pub fn granted(term: Term) -> Self {
        Self {
            term,
            vote_granted: true,
        }
    }

    /// Create a vote denied response
    pub fn denied(term: Term) -> Self {
        Self {
            term,
            vote_granted: false,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

/// InstallSnapshot RPC request
///
/// Invoked by leader to send chunks of a snapshot to a follower
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotRequest {
    /// Leader's term
    pub term: Term,

    /// Leader's ID (so follower can redirect clients)
    pub leader_id: NodeId,

    /// The snapshot replaces all entries up through and including this index
    pub last_included_index: LogIndex,

    /// Term of lastIncludedIndex
    pub last_included_term: Term,

    /// Byte offset where chunk is positioned in the snapshot file
    pub offset: u64,

    /// Raw bytes of the snapshot chunk, starting at offset
    pub data: Vec<u8>,

    /// True if this is the last chunk
    pub done: bool,
}

impl InstallSnapshotRequest {
    /// Create a new InstallSnapshot request
    pub fn new(
        term: Term,
        leader_id: NodeId,
        snapshot: Snapshot,
        offset: u64,
        chunk_size: usize,
    ) -> Self {
        let data_len = snapshot.data.len();
        let chunk_end = std::cmp::min(offset as usize + chunk_size, data_len);
        let chunk = snapshot.data[offset as usize..chunk_end].to_vec();
        let done = chunk_end >= data_len;

        Self {
            term,
            leader_id,
            last_included_index: snapshot.last_included_index,
            last_included_term: snapshot.last_included_term,
            offset,
            data: chunk,
            done,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

/// InstallSnapshot RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    /// Current term, for leader to update itself
    pub term: Term,

    /// True if snapshot was successfully installed
    pub success: bool,

    /// The byte offset for the next chunk (for resume)
    pub next_offset: Option<u64>,
}

impl InstallSnapshotResponse {
    /// Create a successful response
    pub fn success(term: Term, next_offset: Option<u64>) -> Self {
        Self {
            term,
            success: true,
            next_offset,
        }
    }

    /// Create a failure response
    pub fn failure(term: Term) -> Self {
        Self {
            term,
            success: false,
            next_offset: None,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

/// RPC message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    AppendEntriesRequest(AppendEntriesRequest),
    AppendEntriesResponse(AppendEntriesResponse),
    RequestVoteRequest(RequestVoteRequest),
    RequestVoteResponse(RequestVoteResponse),
    InstallSnapshotRequest(InstallSnapshotRequest),
    InstallSnapshotResponse(InstallSnapshotResponse),
}

impl RaftMessage {
    /// Get the term from the message
    pub fn term(&self) -> Term {
        match self {
            RaftMessage::AppendEntriesRequest(req) => req.term,
            RaftMessage::AppendEntriesResponse(resp) => resp.term,
            RaftMessage::RequestVoteRequest(req) => req.term,
            RaftMessage::RequestVoteResponse(resp) => resp.term,
            RaftMessage::InstallSnapshotRequest(req) => req.term,
            RaftMessage::InstallSnapshotResponse(resp) => resp.term,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_entries_heartbeat() {
        let req = AppendEntriesRequest::heartbeat(1, "leader".to_string(), 10);
        assert!(req.is_heartbeat());
        assert_eq!(req.entries.len(), 0);
    }

    #[test]
    fn test_append_entries_serialization() {
        let req = AppendEntriesRequest::new(1, "leader".to_string(), 10, 1, vec![], 10);

        let bytes = req.to_bytes().unwrap();
        let decoded = AppendEntriesRequest::from_bytes(&bytes).unwrap();

        assert_eq!(req.term, decoded.term);
        assert_eq!(req.leader_id, decoded.leader_id);
    }

    #[test]
    fn test_request_vote_serialization() {
        let req = RequestVoteRequest::new(2, "candidate".to_string(), 15, 2);

        let bytes = req.to_bytes().unwrap();
        let decoded = RequestVoteRequest::from_bytes(&bytes).unwrap();

        assert_eq!(req.term, decoded.term);
        assert_eq!(req.candidate_id, decoded.candidate_id);
    }

    #[test]
    fn test_response_types() {
        let success = AppendEntriesResponse::success(1, 10);
        assert!(success.success);
        assert_eq!(success.match_index, Some(10));

        let failure = AppendEntriesResponse::failure(1, Some(5), Some(1));
        assert!(!failure.success);
        assert_eq!(failure.conflict_index, Some(5));
    }

    #[test]
    fn test_vote_responses() {
        let granted = RequestVoteResponse::granted(1);
        assert!(granted.vote_granted);

        let denied = RequestVoteResponse::denied(1);
        assert!(!denied.vote_granted);
    }
}
