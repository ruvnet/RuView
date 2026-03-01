//! Raft state management
//!
//! Implements the state machine for Raft consensus including:
//! - Persistent state (term, vote, log)
//! - Volatile state (commit index, last applied)
//! - Leader-specific state (next index, match index)

use crate::{log::RaftLog, LogIndex, NodeId, Term};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The three states a Raft node can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftState {
    /// Follower state - responds to RPCs from leaders and candidates
    Follower,
    /// Candidate state - attempts to become leader
    Candidate,
    /// Leader state - handles client requests and replicates log
    Leader,
}

impl RaftState {
    /// Returns true if this node is the leader
    pub fn is_leader(&self) -> bool {
        matches!(self, RaftState::Leader)
    }

    /// Returns true if this node is a candidate
    pub fn is_candidate(&self) -> bool {
        matches!(self, RaftState::Candidate)
    }

    /// Returns true if this node is a follower
    pub fn is_follower(&self) -> bool {
        matches!(self, RaftState::Follower)
    }
}

/// Persistent state on all servers
///
/// Updated on stable storage before responding to RPCs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    /// Latest term server has seen (initialized to 0, increases monotonically)
    pub current_term: Term,

    /// Candidate ID that received vote in current term (or None)
    pub voted_for: Option<NodeId>,

    /// Log entries (each entry contains command and term)
    pub log: RaftLog,
}

impl PersistentState {
    /// Create new persistent state with initial values
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: RaftLog::new(),
        }
    }

    /// Increment the current term
    pub fn increment_term(&mut self) {
        self.current_term += 1;
        self.voted_for = None;
    }

    /// Update term if the given term is higher
    pub fn update_term(&mut self, term: Term) -> bool {
        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
            true
        } else {
            false
        }
    }

    /// Vote for a candidate in the current term
    pub fn vote_for(&mut self, candidate_id: NodeId) {
        self.voted_for = Some(candidate_id);
    }

    /// Check if vote can be granted for the given candidate
    pub fn can_vote_for(&self, candidate_id: &NodeId) -> bool {
        match &self.voted_for {
            None => true,
            Some(voted) => voted == candidate_id,
        }
    }

    /// Serialize state to bytes for persistence
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        use bincode::config;
        bincode::encode_to_vec(bincode::serde::Compat(self), config::standard())
    }

    /// Deserialize state from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        use bincode::config;
        let (compat, _): (bincode::serde::Compat<Self>, _) =
            bincode::decode_from_slice(bytes, config::standard())?;
        Ok(compat.0)
    }
}

impl Default for PersistentState {
    fn default() -> Self {
        Self::new()
    }
}

/// Volatile state on all servers
///
/// Can be reconstructed from persistent state
#[derive(Debug, Clone)]
pub struct VolatileState {
    /// Index of highest log entry known to be committed
    /// (initialized to 0, increases monotonically)
    pub commit_index: LogIndex,

    /// Index of highest log entry applied to state machine
    /// (initialized to 0, increases monotonically)
    pub last_applied: LogIndex,
}

impl VolatileState {
    /// Create new volatile state with initial values
    pub fn new() -> Self {
        Self {
            commit_index: 0,
            last_applied: 0,
        }
    }

    /// Update commit index
    pub fn update_commit_index(&mut self, index: LogIndex) {
        if index > self.commit_index {
            self.commit_index = index;
        }
    }

    /// Advance last_applied index
    pub fn apply_entries(&mut self, up_to_index: LogIndex) {
        if up_to_index > self.last_applied {
            self.last_applied = up_to_index;
        }
    }

    /// Get the number of entries that need to be applied
    pub fn pending_entries(&self) -> u64 {
        self.commit_index.saturating_sub(self.last_applied)
    }
}

impl Default for VolatileState {
    fn default() -> Self {
        Self::new()
    }
}

/// Volatile state on leaders
///
/// Reinitialized after election
#[derive(Debug, Clone)]
pub struct LeaderState {
    /// For each server, index of the next log entry to send to that server
    /// (initialized to leader last log index + 1)
    pub next_index: HashMap<NodeId, LogIndex>,

    /// For each server, index of highest log entry known to be replicated
    /// (initialized to 0, increases monotonically)
    pub match_index: HashMap<NodeId, LogIndex>,
}

impl LeaderState {
    /// Create new leader state for the given cluster members
    pub fn new(cluster_members: &[NodeId], last_log_index: LogIndex) -> Self {
        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();

        for member in cluster_members {
            // Initialize next_index to last log index + 1
            next_index.insert(member.clone(), last_log_index + 1);
            // Initialize match_index to 0
            match_index.insert(member.clone(), 0);
        }

        Self {
            next_index,
            match_index,
        }
    }

    /// Update next_index for a follower (decrement on failure)
    pub fn decrement_next_index(&mut self, node_id: &NodeId) {
        if let Some(index) = self.next_index.get_mut(node_id) {
            if *index > 1 {
                *index -= 1;
            }
        }
    }

    /// Update both next_index and match_index for successful replication
    pub fn update_replication(&mut self, node_id: &NodeId, match_index: LogIndex) {
        self.match_index.insert(node_id.clone(), match_index);
        self.next_index.insert(node_id.clone(), match_index + 1);
    }

    /// Get the median match_index for determining commit_index
    pub fn calculate_commit_index(&self) -> LogIndex {
        if self.match_index.is_empty() {
            return 0;
        }

        let mut indices: Vec<LogIndex> = self.match_index.values().copied().collect();
        indices.sort_unstable();

        // Return the median (quorum)
        let mid = indices.len() / 2;
        indices.get(mid).copied().unwrap_or(0)
    }

    /// Get next_index for a specific follower
    pub fn get_next_index(&self, node_id: &NodeId) -> Option<LogIndex> {
        self.next_index.get(node_id).copied()
    }

    /// Get match_index for a specific follower
    pub fn get_match_index(&self, node_id: &NodeId) -> Option<LogIndex> {
        self.match_index.get(node_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_state_checks() {
        assert!(RaftState::Leader.is_leader());
        assert!(RaftState::Candidate.is_candidate());
        assert!(RaftState::Follower.is_follower());
    }

    #[test]
    fn test_persistent_state_term_management() {
        let mut state = PersistentState::new();
        assert_eq!(state.current_term, 0);

        state.increment_term();
        assert_eq!(state.current_term, 1);
        assert!(state.voted_for.is_none());

        state.update_term(5);
        assert_eq!(state.current_term, 5);
    }

    #[test]
    fn test_voting() {
        let mut state = PersistentState::new();
        let candidate = "node1".to_string();

        assert!(state.can_vote_for(&candidate));
        state.vote_for(candidate.clone());
        assert!(state.can_vote_for(&candidate));
        assert!(!state.can_vote_for(&"node2".to_string()));
    }

    #[test]
    fn test_volatile_state() {
        let mut state = VolatileState::new();
        assert_eq!(state.commit_index, 0);
        assert_eq!(state.last_applied, 0);

        state.update_commit_index(10);
        assert_eq!(state.commit_index, 10);
        assert_eq!(state.pending_entries(), 10);

        state.apply_entries(5);
        assert_eq!(state.last_applied, 5);
        assert_eq!(state.pending_entries(), 5);
    }

    #[test]
    fn test_leader_state() {
        let members = vec!["node1".to_string(), "node2".to_string()];
        let mut leader_state = LeaderState::new(&members, 10);

        assert_eq!(leader_state.get_next_index(&members[0]), Some(11));
        assert_eq!(leader_state.get_match_index(&members[0]), Some(0));

        leader_state.update_replication(&members[0], 10);
        assert_eq!(leader_state.get_next_index(&members[0]), Some(11));
        assert_eq!(leader_state.get_match_index(&members[0]), Some(10));
    }

    #[test]
    fn test_commit_index_calculation() {
        let members = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ];
        let mut leader_state = LeaderState::new(&members, 10);

        leader_state.update_replication(&members[0], 5);
        leader_state.update_replication(&members[1], 8);
        leader_state.update_replication(&members[2], 3);

        let commit = leader_state.calculate_commit_index();
        assert_eq!(commit, 5); // Median of [3, 5, 8]
    }
}
