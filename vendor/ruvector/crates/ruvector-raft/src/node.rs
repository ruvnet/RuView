//! Raft node implementation
//!
//! Coordinates all Raft components:
//! - State machine management
//! - RPC message handling
//! - Log replication
//! - Leader election
//! - Client request processing

use crate::{
    election::{ElectionState, VoteValidator},
    rpc::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, RaftMessage, RequestVoteRequest, RequestVoteResponse,
    },
    state::{LeaderState, PersistentState, RaftState, VolatileState},
    LogIndex, NodeId, RaftError, RaftResult, Term,
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

/// Configuration for a Raft node
#[derive(Debug, Clone)]
pub struct RaftNodeConfig {
    /// This node's ID
    pub node_id: NodeId,

    /// IDs of all cluster members (including self)
    pub cluster_members: Vec<NodeId>,

    /// Minimum election timeout (milliseconds)
    pub election_timeout_min: u64,

    /// Maximum election timeout (milliseconds)
    pub election_timeout_max: u64,

    /// Heartbeat interval (milliseconds)
    pub heartbeat_interval: u64,

    /// Maximum entries per AppendEntries RPC
    pub max_entries_per_message: usize,

    /// Snapshot chunk size (bytes)
    pub snapshot_chunk_size: usize,
}

impl RaftNodeConfig {
    /// Create a new configuration with defaults
    pub fn new(node_id: NodeId, cluster_members: Vec<NodeId>) -> Self {
        Self {
            node_id,
            cluster_members,
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
            max_entries_per_message: 100,
            snapshot_chunk_size: 64 * 1024, // 64KB
        }
    }
}

/// Command to apply to the state machine
#[derive(Debug, Clone)]
pub struct Command {
    pub data: Vec<u8>,
}

/// Result of applying a command
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub index: LogIndex,
    pub term: Term,
}

/// Internal messages for the Raft node
#[derive(Debug)]
enum InternalMessage {
    /// RPC message from another node
    Rpc { from: NodeId, message: RaftMessage },
    /// Client command to replicate
    ClientCommand {
        command: Command,
        response_tx: mpsc::Sender<RaftResult<CommandResult>>,
    },
    /// Election timeout fired
    ElectionTimeout,
    /// Heartbeat timeout fired
    HeartbeatTimeout,
}

/// The Raft consensus node
pub struct RaftNode {
    /// Configuration
    config: RaftNodeConfig,

    /// Persistent state
    persistent: Arc<RwLock<PersistentState>>,

    /// Volatile state
    volatile: Arc<RwLock<VolatileState>>,

    /// Current Raft state (Follower, Candidate, Leader)
    state: Arc<RwLock<RaftState>>,

    /// Leader-specific state (only valid when state is Leader)
    leader_state: Arc<RwLock<Option<LeaderState>>>,

    /// Election state
    election_state: Arc<RwLock<ElectionState>>,

    /// Current leader ID (if known)
    current_leader: Arc<RwLock<Option<NodeId>>>,

    /// Channel for internal messages
    internal_tx: mpsc::UnboundedSender<InternalMessage>,
    internal_rx: Arc<RwLock<mpsc::UnboundedReceiver<InternalMessage>>>,
}

impl RaftNode {
    /// Create a new Raft node
    pub fn new(config: RaftNodeConfig) -> Self {
        let (internal_tx, internal_rx) = mpsc::unbounded_channel();
        let cluster_size = config.cluster_members.len();

        Self {
            persistent: Arc::new(RwLock::new(PersistentState::new())),
            volatile: Arc::new(RwLock::new(VolatileState::new())),
            state: Arc::new(RwLock::new(RaftState::Follower)),
            leader_state: Arc::new(RwLock::new(None)),
            election_state: Arc::new(RwLock::new(ElectionState::new(
                cluster_size,
                config.election_timeout_min,
                config.election_timeout_max,
            ))),
            current_leader: Arc::new(RwLock::new(None)),
            config,
            internal_tx,
            internal_rx: Arc::new(RwLock::new(internal_rx)),
        }
    }

    /// Start the Raft node
    pub async fn start(self: Arc<Self>) {
        info!("Starting Raft node: {}", self.config.node_id);

        // Spawn election timer task
        self.clone().spawn_election_timer();

        // Spawn heartbeat timer task (for leaders)
        self.clone().spawn_heartbeat_timer();

        // Main message processing loop
        self.run().await;
    }

    /// Main message processing loop
    async fn run(self: Arc<Self>) {
        loop {
            let message = {
                let mut rx = self.internal_rx.write();
                rx.recv().await
            };

            match message {
                Some(InternalMessage::Rpc { from, message }) => {
                    self.handle_rpc_message(from, message).await;
                }
                Some(InternalMessage::ClientCommand {
                    command,
                    response_tx,
                }) => {
                    self.handle_client_command(command, response_tx).await;
                }
                Some(InternalMessage::ElectionTimeout) => {
                    self.handle_election_timeout().await;
                }
                Some(InternalMessage::HeartbeatTimeout) => {
                    self.handle_heartbeat_timeout().await;
                }
                None => {
                    warn!("Internal channel closed, stopping node");
                    break;
                }
            }
        }
    }

    /// Handle RPC message from another node
    async fn handle_rpc_message(&self, from: NodeId, message: RaftMessage) {
        // Update term if necessary
        let message_term = message.term();
        let current_term = self.persistent.read().current_term;

        if message_term > current_term {
            self.step_down(message_term).await;
        }

        match message {
            RaftMessage::AppendEntriesRequest(req) => {
                let response = self.handle_append_entries(req).await;
                // TODO: Send response back to sender
                debug!("AppendEntries response to {}: {:?}", from, response);
            }
            RaftMessage::AppendEntriesResponse(resp) => {
                self.handle_append_entries_response(from, resp).await;
            }
            RaftMessage::RequestVoteRequest(req) => {
                let response = self.handle_request_vote(req).await;
                // TODO: Send response back to sender
                debug!("RequestVote response to {}: {:?}", from, response);
            }
            RaftMessage::RequestVoteResponse(resp) => {
                self.handle_request_vote_response(from, resp).await;
            }
            RaftMessage::InstallSnapshotRequest(req) => {
                let response = self.handle_install_snapshot(req).await;
                // TODO: Send response back to sender
                debug!("InstallSnapshot response to {}: {:?}", from, response);
            }
            RaftMessage::InstallSnapshotResponse(resp) => {
                self.handle_install_snapshot_response(from, resp).await;
            }
        }
    }

    /// Handle AppendEntries RPC
    async fn handle_append_entries(&self, req: AppendEntriesRequest) -> AppendEntriesResponse {
        let mut persistent = self.persistent.write();
        let mut volatile = self.volatile.write();

        // Reply false if term < currentTerm
        if req.term < persistent.current_term {
            return AppendEntriesResponse::failure(persistent.current_term, None, None);
        }

        // Reset election timer
        self.election_state.write().reset_timer();
        *self.current_leader.write() = Some(req.leader_id.clone());

        // Reply false if log doesn't contain an entry at prevLogIndex with prevLogTerm
        if !persistent
            .log
            .matches(req.prev_log_index, req.prev_log_term)
        {
            let conflict_index = req.prev_log_index;
            let conflict_term = persistent.log.term_at(conflict_index);
            return AppendEntriesResponse::failure(
                persistent.current_term,
                Some(conflict_index),
                conflict_term,
            );
        }

        // Append new entries
        if !req.entries.is_empty() {
            // Delete conflicting entries and append new ones
            let mut index = req.prev_log_index + 1;
            for entry in &req.entries {
                if let Some(existing_term) = persistent.log.term_at(index) {
                    if existing_term != entry.term {
                        // Conflict found, truncate from here
                        let _ = persistent.log.truncate_from(index);
                    }
                }
                index += 1;
            }

            // Append entries
            if let Err(e) = persistent.log.append_entries(req.entries.clone()) {
                error!("Failed to append entries: {}", e);
                return AppendEntriesResponse::failure(persistent.current_term, None, None);
            }
        }

        // Update commit index
        if req.leader_commit > volatile.commit_index {
            let last_new_entry = if req.entries.is_empty() {
                req.prev_log_index
            } else {
                req.entries.last().unwrap().index
            };
            volatile.update_commit_index(std::cmp::min(req.leader_commit, last_new_entry));
        }

        AppendEntriesResponse::success(persistent.current_term, persistent.log.last_index())
    }

    /// Handle AppendEntries response
    async fn handle_append_entries_response(&self, from: NodeId, resp: AppendEntriesResponse) {
        if !self.state.read().is_leader() {
            return;
        }

        let persistent = self.persistent.write();
        let mut leader_state_guard = self.leader_state.write();

        if let Some(leader_state) = leader_state_guard.as_mut() {
            if resp.success {
                // Update next_index and match_index
                if let Some(match_index) = resp.match_index {
                    leader_state.update_replication(&from, match_index);

                    // Update commit index
                    let new_commit = leader_state.calculate_commit_index();
                    let mut volatile = self.volatile.write();
                    if new_commit > volatile.commit_index {
                        // Verify the entry is from current term
                        if let Some(term) = persistent.log.term_at(new_commit) {
                            if term == persistent.current_term {
                                volatile.update_commit_index(new_commit);
                                info!("Updated commit index to {}", new_commit);
                            }
                        }
                    }
                }
            } else {
                // Decrement next_index and retry
                leader_state.decrement_next_index(&from);
                debug!("Replication failed for {}, decrementing next_index", from);
            }
        }
    }

    /// Handle RequestVote RPC
    async fn handle_request_vote(&self, req: RequestVoteRequest) -> RequestVoteResponse {
        let mut persistent = self.persistent.write();

        // Reply false if term < currentTerm
        if req.term < persistent.current_term {
            return RequestVoteResponse::denied(persistent.current_term);
        }

        let last_log_index = persistent.log.last_index();
        let last_log_term = persistent.log.last_term();

        // Check if we should grant vote
        let should_grant = VoteValidator::should_grant_vote(
            persistent.current_term,
            &persistent.voted_for,
            last_log_index,
            last_log_term,
            &req.candidate_id,
            req.term,
            req.last_log_index,
            req.last_log_term,
        );

        if should_grant {
            persistent.vote_for(req.candidate_id.clone());
            self.election_state.write().reset_timer();
            info!("Granted vote to {} for term {}", req.candidate_id, req.term);
            RequestVoteResponse::granted(persistent.current_term)
        } else {
            debug!("Denied vote to {} for term {}", req.candidate_id, req.term);
            RequestVoteResponse::denied(persistent.current_term)
        }
    }

    /// Handle RequestVote response
    async fn handle_request_vote_response(&self, from: NodeId, resp: RequestVoteResponse) {
        if !self.state.read().is_candidate() {
            return;
        }

        let current_term = self.persistent.read().current_term;
        if resp.term != current_term {
            return;
        }

        if resp.vote_granted {
            let won_election = self.election_state.write().record_vote(from.clone());
            if won_election {
                info!("Won election for term {}", current_term);
                self.become_leader().await;
            }
        }
    }

    /// Handle InstallSnapshot RPC
    async fn handle_install_snapshot(
        &self,
        req: InstallSnapshotRequest,
    ) -> InstallSnapshotResponse {
        let persistent = self.persistent.write();

        if req.term < persistent.current_term {
            return InstallSnapshotResponse::failure(persistent.current_term);
        }

        // TODO: Implement snapshot installation
        // For now, just acknowledge
        InstallSnapshotResponse::success(persistent.current_term, None)
    }

    /// Handle InstallSnapshot response
    async fn handle_install_snapshot_response(
        &self,
        _from: NodeId,
        _resp: InstallSnapshotResponse,
    ) {
        // TODO: Implement snapshot response handling
    }

    /// Handle client command
    async fn handle_client_command(
        &self,
        command: Command,
        response_tx: mpsc::Sender<RaftResult<CommandResult>>,
    ) {
        // Only leader can handle client commands
        if !self.state.read().is_leader() {
            let _ = response_tx.send(Err(RaftError::NotLeader)).await;
            return;
        }

        let mut persistent = self.persistent.write();
        let term = persistent.current_term;
        let index = persistent.log.append(term, command.data);

        let result = CommandResult { index, term };
        let _ = response_tx.send(Ok(result)).await;

        // Trigger immediate replication
        drop(persistent);
        let _ = self.internal_tx.send(InternalMessage::HeartbeatTimeout);
    }

    /// Handle election timeout
    async fn handle_election_timeout(&self) {
        if self.state.read().is_leader() {
            return;
        }

        if !self.election_state.read().should_start_election() {
            return;
        }

        info!("Election timeout, starting election");
        self.start_election().await;
    }

    /// Start a new election
    async fn start_election(&self) {
        // Transition to candidate
        *self.state.write() = RaftState::Candidate;

        // Increment term and vote for self
        let mut persistent = self.persistent.write();
        persistent.increment_term();
        persistent.vote_for(self.config.node_id.clone());
        let term = persistent.current_term;

        // Initialize election state
        self.election_state
            .write()
            .start_election(term, &self.config.node_id);

        let last_log_index = persistent.log.last_index();
        let last_log_term = persistent.log.last_term();

        info!(
            "Starting election for term {} as {}",
            term, self.config.node_id
        );

        // Send RequestVote RPCs to all other nodes
        for member in &self.config.cluster_members {
            if member != &self.config.node_id {
                let _request = RequestVoteRequest::new(
                    term,
                    self.config.node_id.clone(),
                    last_log_index,
                    last_log_term,
                );
                // TODO: Send request to member
                debug!("Would send RequestVote to {}", member);
            }
        }
    }

    /// Become leader after winning election
    async fn become_leader(&self) {
        info!(
            "Becoming leader for term {}",
            self.persistent.read().current_term
        );

        *self.state.write() = RaftState::Leader;
        *self.current_leader.write() = Some(self.config.node_id.clone());

        let last_log_index = self.persistent.read().log.last_index();
        let other_members: Vec<_> = self
            .config
            .cluster_members
            .iter()
            .filter(|m| *m != &self.config.node_id)
            .cloned()
            .collect();

        *self.leader_state.write() = Some(LeaderState::new(&other_members, last_log_index));

        // Send initial heartbeats
        let _ = self.internal_tx.send(InternalMessage::HeartbeatTimeout);
    }

    /// Step down to follower (when discovering higher term)
    async fn step_down(&self, term: Term) {
        info!("Stepping down to follower for term {}", term);

        *self.state.write() = RaftState::Follower;
        *self.leader_state.write() = None;
        *self.current_leader.write() = None;

        let mut persistent = self.persistent.write();
        persistent.update_term(term);
    }

    /// Handle heartbeat timeout (for leaders)
    async fn handle_heartbeat_timeout(&self) {
        if !self.state.read().is_leader() {
            return;
        }

        self.send_heartbeats().await;
    }

    /// Send heartbeats to all followers
    async fn send_heartbeats(&self) {
        let persistent = self.persistent.read();
        let term = persistent.current_term;
        let commit_index = self.volatile.read().commit_index;

        for member in &self.config.cluster_members {
            if member != &self.config.node_id {
                let request = AppendEntriesRequest::heartbeat(
                    term,
                    self.config.node_id.clone(),
                    commit_index,
                );
                // TODO: Send heartbeat to member
                debug!("Would send heartbeat to {}", member);
            }
        }
    }

    /// Spawn election timer task
    fn spawn_election_timer(self: Arc<Self>) {
        let node = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(50));
            loop {
                interval.tick().await;
                if node.election_state.read().should_start_election() {
                    let _ = node.internal_tx.send(InternalMessage::ElectionTimeout);
                }
            }
        });
    }

    /// Spawn heartbeat timer task
    fn spawn_heartbeat_timer(self: Arc<Self>) {
        let node = self.clone();
        tokio::spawn(async move {
            let interval_ms = node.config.heartbeat_interval;
            let mut interval = interval(Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                if node.state.read().is_leader() {
                    let _ = node.internal_tx.send(InternalMessage::HeartbeatTimeout);
                }
            }
        });
    }

    /// Submit a command to the Raft cluster
    pub async fn submit_command(&self, data: Vec<u8>) -> RaftResult<CommandResult> {
        let (tx, mut rx) = mpsc::channel(1);
        let command = Command { data };

        self.internal_tx
            .send(InternalMessage::ClientCommand {
                command,
                response_tx: tx,
            })
            .map_err(|_| RaftError::Internal("Node stopped".to_string()))?;

        rx.recv()
            .await
            .ok_or_else(|| RaftError::Internal("Response channel closed".to_string()))?
    }

    /// Get current state
    pub fn current_state(&self) -> RaftState {
        *self.state.read()
    }

    /// Get current term
    pub fn current_term(&self) -> Term {
        self.persistent.read().current_term
    }

    /// Get current leader
    pub fn current_leader(&self) -> Option<NodeId> {
        self.current_leader.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let config = RaftNodeConfig::new(
            "node1".to_string(),
            vec![
                "node1".to_string(),
                "node2".to_string(),
                "node3".to_string(),
            ],
        );

        let node = RaftNode::new(config);
        assert_eq!(node.current_state(), RaftState::Follower);
        assert_eq!(node.current_term(), 0);
    }
}
