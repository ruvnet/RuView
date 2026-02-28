//! Distributed Coherence Configuration
//!
//! Configuration for Raft-based multi-node coherence coordination.

use serde::{Deserialize, Serialize};

/// Configuration for distributed coherence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedCoherenceConfig {
    /// This node's identifier
    pub node_id: String,

    /// All cluster member node IDs
    pub cluster_members: Vec<String>,

    /// Coherence state dimension
    pub dimension: usize,

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

    /// Energy threshold for coherence
    pub coherence_threshold: f32,

    /// Synchronization interval (milliseconds)
    pub sync_interval: u64,

    /// Enable energy checkpointing
    pub enable_checkpoints: bool,

    /// Checkpoint interval (number of updates)
    pub checkpoint_interval: usize,

    /// Replication factor for energy states
    pub replication_factor: usize,
}

impl Default for DistributedCoherenceConfig {
    fn default() -> Self {
        Self {
            node_id: "node0".to_string(),
            cluster_members: vec!["node0".to_string()],
            dimension: 64,
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
            max_entries_per_message: 100,
            snapshot_chunk_size: 64 * 1024,
            coherence_threshold: 0.01,
            sync_interval: 100,
            enable_checkpoints: true,
            checkpoint_interval: 1000,
            replication_factor: 3,
        }
    }
}

impl DistributedCoherenceConfig {
    /// Create configuration for a single node (development)
    pub fn single_node(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            cluster_members: vec![node_id.to_string()],
            replication_factor: 1,
            ..Default::default()
        }
    }

    /// Create configuration for a 3-node cluster
    pub fn three_node_cluster(node_id: &str, members: Vec<String>) -> Self {
        assert!(
            members.len() >= 3,
            "Need at least 3 members for 3-node cluster"
        );
        Self {
            node_id: node_id.to_string(),
            cluster_members: members,
            replication_factor: 3,
            ..Default::default()
        }
    }

    /// Create configuration for a 5-node cluster
    pub fn five_node_cluster(node_id: &str, members: Vec<String>) -> Self {
        assert!(
            members.len() >= 5,
            "Need at least 5 members for 5-node cluster"
        );
        Self {
            node_id: node_id.to_string(),
            cluster_members: members,
            replication_factor: 5,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.node_id.is_empty() {
            return Err("node_id cannot be empty".to_string());
        }

        if self.cluster_members.is_empty() {
            return Err("cluster_members cannot be empty".to_string());
        }

        if !self.cluster_members.contains(&self.node_id) {
            return Err("node_id must be in cluster_members".to_string());
        }

        if self.election_timeout_min >= self.election_timeout_max {
            return Err("election_timeout_min must be less than election_timeout_max".to_string());
        }

        if self.heartbeat_interval >= self.election_timeout_min {
            return Err("heartbeat_interval must be less than election_timeout_min".to_string());
        }

        if self.replication_factor > self.cluster_members.len() {
            return Err("replication_factor cannot exceed cluster size".to_string());
        }

        Ok(())
    }

    /// Get quorum size for the cluster
    pub fn quorum_size(&self) -> usize {
        self.cluster_members.len() / 2 + 1
    }

    /// Check if this is a single-node cluster
    pub fn is_single_node(&self) -> bool {
        self.cluster_members.len() == 1
    }

    /// Get number of tolerable failures
    pub fn max_failures(&self) -> usize {
        self.cluster_members
            .len()
            .saturating_sub(self.quorum_size())
    }
}

/// Node role in the distributed system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Following the leader
    Follower,
    /// Candidate for leadership
    Candidate,
    /// Current leader
    Leader,
}

impl NodeRole {
    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        matches!(self, Self::Leader)
    }

    /// Check if this node can accept writes
    pub fn can_write(&self) -> bool {
        matches!(self, Self::Leader)
    }

    /// Get role name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Follower => "follower",
            Self::Candidate => "candidate",
            Self::Leader => "leader",
        }
    }
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DistributedCoherenceConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_single_node_config() {
        let config = DistributedCoherenceConfig::single_node("node1");
        assert!(config.validate().is_ok());
        assert!(config.is_single_node());
        assert_eq!(config.quorum_size(), 1);
    }

    #[test]
    fn test_three_node_config() {
        let members = vec!["n1".to_string(), "n2".to_string(), "n3".to_string()];
        let config = DistributedCoherenceConfig::three_node_cluster("n1", members);
        assert!(config.validate().is_ok());
        assert_eq!(config.quorum_size(), 2);
        assert_eq!(config.max_failures(), 1);
    }

    #[test]
    fn test_invalid_config() {
        let config = DistributedCoherenceConfig {
            node_id: "node1".to_string(),
            cluster_members: vec!["node2".to_string()], // node1 not in members
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_node_role() {
        assert!(NodeRole::Leader.is_leader());
        assert!(NodeRole::Leader.can_write());
        assert!(!NodeRole::Follower.is_leader());
        assert!(!NodeRole::Follower.can_write());
    }
}
