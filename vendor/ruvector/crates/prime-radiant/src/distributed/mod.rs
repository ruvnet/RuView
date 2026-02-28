//! Distributed Coherence Module
//!
//! Provides Raft-based multi-node coherence coordination using `ruvector-raft`.
//!
//! # Features
//!
//! - Raft consensus for coherence state replication
//! - Replicated state machine for energy values
//! - Checkpoint and snapshot support
//! - Incoherent region tracking across cluster
//! - Leader-based write coordination
//!
//! # Architecture
//!
//! The distributed coherence system uses Raft consensus to ensure that all
//! nodes in the cluster have a consistent view of the coherence state:
//!
//! ```text
//! +-------------+     +-------------+     +-------------+
//! |   Node 1    |<--->|   Node 2    |<--->|   Node 3    |
//! |  (Leader)   |     |  (Follower) |     |  (Follower) |
//! +-------------+     +-------------+     +-------------+
//!       |                   |                   |
//!       v                   v                   v
//! +-------------+     +-------------+     +-------------+
//! | State Mach  |     | State Mach  |     | State Mach  |
//! +-------------+     +-------------+     +-------------+
//! ```
//!
//! - **Leader**: Accepts write operations (energy updates, state changes)
//! - **Followers**: Replicate state from leader, serve read operations
//! - **State Machine**: Applies committed commands to local state
//!
//! # Example
//!
//! ```ignore
//! use prime_radiant::distributed::{DistributedCoherence, DistributedCoherenceConfig};
//!
//! let config = DistributedCoherenceConfig::single_node("node1");
//! let mut coherence = DistributedCoherence::new(config);
//!
//! // Update energy (leader only)
//! coherence.update_energy(1, 2, 0.5)?;
//!
//! // Get total energy
//! let total = coherence.total_energy();
//! ```

mod adapter;
mod config;
mod state;

pub use adapter::{ClusterStatus, CoherenceCommand, CommandResult, RaftAdapter};
pub use config::{DistributedCoherenceConfig, NodeRole};
pub use state::{
    ApplyResult, Checkpoint, CoherenceStateMachine, EdgeEnergy, IncoherentRegion, NodeState,
    StateSnapshot, StateSummary,
};

/// Result type for distributed operations
pub type Result<T> = std::result::Result<T, DistributedError>;

/// Errors in distributed coherence operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum DistributedError {
    /// Not the leader
    #[error("Not the leader, current leader: {leader:?}")]
    NotLeader { leader: Option<String> },

    /// No leader available
    #[error("No leader available in the cluster")]
    NoLeader,

    /// Command failed
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Replication failed
    #[error("Replication failed: {0}")]
    ReplicationFailed(String),

    /// Timeout
    #[error("Operation timed out")]
    Timeout,

    /// Node not found
    #[error("Node not found: {0}")]
    NodeNotFound(u64),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Main distributed coherence engine
///
/// Combines Raft consensus with coherence state machine to provide
/// replicated coherence tracking across a cluster of nodes.
#[derive(Debug)]
pub struct DistributedCoherence {
    /// Configuration
    config: DistributedCoherenceConfig,
    /// Raft adapter
    raft: RaftAdapter,
    /// State machine
    state_machine: CoherenceStateMachine,
    /// Update counter for checkpoint scheduling
    update_counter: usize,
}

impl DistributedCoherence {
    /// Create a new distributed coherence engine
    pub fn new(config: DistributedCoherenceConfig) -> Self {
        let raft = RaftAdapter::new(config.clone());
        let state_machine = CoherenceStateMachine::new(config.dimension);

        Self {
            config,
            raft,
            state_machine,
            update_counter: 0,
        }
    }

    /// Create with default configuration (single node)
    pub fn single_node(node_id: &str) -> Self {
        Self::new(DistributedCoherenceConfig::single_node(node_id))
    }

    /// Update energy for an edge
    ///
    /// This operation goes through Raft consensus and is replicated to all nodes.
    pub fn update_energy(
        &mut self,
        source: u64,
        target: u64,
        energy: f32,
    ) -> Result<CommandResult> {
        let result = self.raft.update_energy((source, target), energy)?;

        // Apply to local state machine
        self.apply_pending_commands();

        // Check if we need a checkpoint
        self.maybe_checkpoint()?;

        Ok(result)
    }

    /// Set node state vector
    pub fn set_node_state(&mut self, node_id: u64, state: Vec<f32>) -> Result<CommandResult> {
        let result = self.raft.set_node_state(node_id, state)?;
        self.apply_pending_commands();
        self.maybe_checkpoint()?;
        Ok(result)
    }

    /// Mark a region as incoherent
    pub fn mark_incoherent(&mut self, region_id: u64, nodes: Vec<u64>) -> Result<CommandResult> {
        let result = self.raft.mark_incoherent(region_id, nodes)?;
        self.apply_pending_commands();
        Ok(result)
    }

    /// Clear incoherence flag for a region
    pub fn clear_incoherent(&mut self, region_id: u64) -> Result<CommandResult> {
        let result = self.raft.clear_incoherent(region_id)?;
        self.apply_pending_commands();
        Ok(result)
    }

    /// Apply pending commands from Raft to state machine
    fn apply_pending_commands(&mut self) {
        let commands = self.raft.take_pending_commands();
        let mut index = self.state_machine.summary().applied_index;

        for cmd in commands {
            index += 1;
            self.state_machine.apply(&cmd, index);
            self.update_counter += 1;
        }
    }

    /// Create checkpoint if needed
    fn maybe_checkpoint(&mut self) -> Result<()> {
        if !self.config.enable_checkpoints {
            return Ok(());
        }

        if self.update_counter >= self.config.checkpoint_interval {
            self.checkpoint()?;
            self.update_counter = 0;
        }

        Ok(())
    }

    /// Force a checkpoint
    pub fn checkpoint(&mut self) -> Result<CommandResult> {
        let total_energy = self.state_machine.total_energy();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let result = self.raft.checkpoint(total_energy, timestamp)?;
        self.apply_pending_commands();
        Ok(result)
    }

    /// Get total energy
    pub fn total_energy(&self) -> f32 {
        self.state_machine.total_energy()
    }

    /// Get energy for a specific edge
    pub fn get_edge_energy(&self, source: u64, target: u64) -> Option<f32> {
        self.state_machine.get_edge_energy((source, target))
    }

    /// Get node state
    pub fn get_node_state(&self, node_id: u64) -> Option<&NodeState> {
        self.state_machine.get_node_state(node_id)
    }

    /// Check if a node is in an incoherent region
    pub fn is_node_incoherent(&self, node_id: u64) -> bool {
        self.state_machine.is_node_incoherent(node_id)
    }

    /// Get number of active incoherent regions
    pub fn num_incoherent_regions(&self) -> usize {
        self.state_machine.num_incoherent_regions()
    }

    /// Get state machine summary
    pub fn summary(&self) -> StateSummary {
        self.state_machine.summary()
    }

    /// Get cluster status
    pub fn cluster_status(&self) -> ClusterStatus {
        self.raft.cluster_status()
    }

    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        self.raft.is_leader()
    }

    /// Get current role
    pub fn role(&self) -> NodeRole {
        self.raft.role()
    }

    /// Get configuration
    pub fn config(&self) -> &DistributedCoherenceConfig {
        &self.config
    }

    /// Get latest checkpoint
    pub fn latest_checkpoint(&self) -> Option<&Checkpoint> {
        self.state_machine.latest_checkpoint()
    }

    /// Create snapshot of current state
    pub fn snapshot(&self) -> StateSnapshot {
        self.state_machine.snapshot()
    }

    /// Restore from snapshot
    pub fn restore(&mut self, snapshot: StateSnapshot) {
        self.state_machine.restore(snapshot);
    }

    /// Compute coherence status
    pub fn coherence_status(&self) -> CoherenceStatus {
        let summary = self.state_machine.summary();
        let cluster = self.raft.cluster_status();

        let is_coherent = summary.total_energy < self.config.coherence_threshold
            && summary.num_incoherent_regions == 0;

        CoherenceStatus {
            is_coherent,
            total_energy: summary.total_energy,
            threshold: self.config.coherence_threshold,
            num_incoherent_regions: summary.num_incoherent_regions,
            cluster_healthy: cluster.is_healthy(),
            is_leader: cluster.can_write(),
        }
    }
}

/// Overall coherence status
#[derive(Debug, Clone)]
pub struct CoherenceStatus {
    /// Whether the system is coherent
    pub is_coherent: bool,
    /// Total energy
    pub total_energy: f32,
    /// Coherence threshold
    pub threshold: f32,
    /// Number of incoherent regions
    pub num_incoherent_regions: usize,
    /// Whether cluster is healthy
    pub cluster_healthy: bool,
    /// Whether this node can write
    pub is_leader: bool,
}

impl CoherenceStatus {
    /// Get coherence ratio (lower is better)
    pub fn coherence_ratio(&self) -> f32 {
        if self.threshold > 0.0 {
            self.total_energy / self.threshold
        } else {
            if self.total_energy > 0.0 {
                f32::INFINITY
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_coherence_creation() {
        let coherence = DistributedCoherence::single_node("node1");
        assert!(coherence.is_leader());
        assert_eq!(coherence.total_energy(), 0.0);
    }

    #[test]
    fn test_update_energy() {
        let mut coherence = DistributedCoherence::single_node("node1");

        let result = coherence.update_energy(1, 2, 0.5).unwrap();
        assert!(result.success);

        assert!((coherence.total_energy() - 0.5).abs() < 1e-6);
        assert!((coherence.get_edge_energy(1, 2).unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_set_node_state() {
        let mut coherence = DistributedCoherence::single_node("node1");

        let state = vec![1.0, 2.0, 3.0, 4.0];
        coherence.set_node_state(1, state.clone()).unwrap();

        let retrieved = coherence.get_node_state(1).unwrap();
        assert_eq!(retrieved.state.len(), 4);
    }

    #[test]
    fn test_incoherent_regions() {
        let mut coherence = DistributedCoherence::single_node("node1");

        coherence.mark_incoherent(1, vec![10, 20]).unwrap();
        assert_eq!(coherence.num_incoherent_regions(), 1);
        assert!(coherence.is_node_incoherent(10));

        coherence.clear_incoherent(1).unwrap();
        assert_eq!(coherence.num_incoherent_regions(), 0);
    }

    #[test]
    fn test_coherence_status() {
        let mut coherence = DistributedCoherence::single_node("node1");

        // Initially coherent
        let status = coherence.coherence_status();
        assert!(status.is_coherent);

        // Add high energy
        for i in 0..100 {
            coherence.update_energy(i, i + 1, 0.001).unwrap();
        }

        let status = coherence.coherence_status();
        // May or may not be coherent depending on threshold
        assert!(status.cluster_healthy);
        assert!(status.is_leader);
    }

    #[test]
    fn test_checkpoint() {
        let config = DistributedCoherenceConfig {
            enable_checkpoints: true,
            checkpoint_interval: 1,
            ..DistributedCoherenceConfig::single_node("node1")
        };
        let mut coherence = DistributedCoherence::new(config);

        coherence.update_energy(1, 2, 0.5).unwrap();
        coherence.checkpoint().unwrap();

        let cp = coherence.latest_checkpoint().unwrap();
        assert!((cp.total_energy - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_snapshot_restore() {
        let mut coherence1 = DistributedCoherence::single_node("node1");
        coherence1.update_energy(1, 2, 0.5).unwrap();
        coherence1.set_node_state(1, vec![1.0; 64]).unwrap();

        let snapshot = coherence1.snapshot();

        let mut coherence2 = DistributedCoherence::single_node("node2");
        coherence2.restore(snapshot);

        assert!((coherence2.get_edge_energy(1, 2).unwrap() - 0.5).abs() < 1e-6);
        assert!(coherence2.get_node_state(1).is_some());
    }

    #[test]
    fn test_cluster_status() {
        let coherence = DistributedCoherence::single_node("node1");
        let status = coherence.cluster_status();

        assert!(status.is_healthy());
        assert!(status.can_write());
        assert_eq!(status.cluster_size, 1);
    }
}
