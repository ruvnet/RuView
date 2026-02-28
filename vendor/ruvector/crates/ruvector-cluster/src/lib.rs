//! Distributed clustering and sharding for ruvector
//!
//! This crate provides distributed coordination capabilities including:
//! - Cluster node management and health monitoring
//! - Consistent hashing for shard distribution
//! - DAG-based consensus protocol
//! - Dynamic node discovery and topology management

pub mod consensus;
pub mod discovery;
pub mod shard;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub use consensus::DagConsensus;
pub use discovery::{DiscoveryService, GossipDiscovery, StaticDiscovery};
pub use shard::{ConsistentHashRing, ShardRouter};

/// Cluster-related errors
#[derive(Debug, Error)]
pub enum ClusterError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Shard not found: {0}")]
    ShardNotFound(u32),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Consensus error: {0}")]
    ConsensusError(String),

    #[error("Discovery error: {0}")]
    DiscoveryError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ClusterError>;

/// Status of a cluster node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is the cluster leader
    Leader,
    /// Node is a follower
    Follower,
    /// Node is campaigning to be leader
    Candidate,
    /// Node is offline or unreachable
    Offline,
}

/// Information about a cluster node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node identifier
    pub node_id: String,
    /// Network address of the node
    pub address: SocketAddr,
    /// Current status of the node
    pub status: NodeStatus,
    /// Last time the node was seen alive
    pub last_seen: DateTime<Utc>,
    /// Metadata about the node
    pub metadata: HashMap<String, String>,
    /// Node capacity (for load balancing)
    pub capacity: f64,
}

impl ClusterNode {
    /// Create a new cluster node
    pub fn new(node_id: String, address: SocketAddr) -> Self {
        Self {
            node_id,
            address,
            status: NodeStatus::Follower,
            last_seen: Utc::now(),
            metadata: HashMap::new(),
            capacity: 1.0,
        }
    }

    /// Check if the node is healthy (seen recently)
    pub fn is_healthy(&self, timeout: Duration) -> bool {
        let now = Utc::now();
        let elapsed = now
            .signed_duration_since(self.last_seen)
            .to_std()
            .unwrap_or(Duration::MAX);
        elapsed < timeout
    }

    /// Update the last seen timestamp
    pub fn heartbeat(&mut self) {
        self.last_seen = Utc::now();
    }
}

/// Information about a data shard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    /// Shard identifier
    pub shard_id: u32,
    /// Primary node responsible for this shard
    pub primary_node: String,
    /// Replica nodes for this shard
    pub replica_nodes: Vec<String>,
    /// Number of vectors in this shard
    pub vector_count: usize,
    /// Shard status
    pub status: ShardStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
}

/// Status of a shard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardStatus {
    /// Shard is active and serving requests
    Active,
    /// Shard is being migrated
    Migrating,
    /// Shard is being replicated
    Replicating,
    /// Shard is offline
    Offline,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Number of replica copies for each shard
    pub replication_factor: usize,
    /// Total number of shards in the cluster
    pub shard_count: u32,
    /// Interval between heartbeat checks
    pub heartbeat_interval: Duration,
    /// Timeout before considering a node offline
    pub node_timeout: Duration,
    /// Enable DAG-based consensus
    pub enable_consensus: bool,
    /// Minimum nodes required for quorum
    pub min_quorum_size: usize,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            shard_count: 64,
            heartbeat_interval: Duration::from_secs(5),
            node_timeout: Duration::from_secs(30),
            enable_consensus: true,
            min_quorum_size: 2,
        }
    }
}

/// Manages a distributed cluster of vector database nodes
pub struct ClusterManager {
    /// Cluster configuration
    config: ClusterConfig,
    /// Map of node_id to ClusterNode
    nodes: Arc<DashMap<String, ClusterNode>>,
    /// Map of shard_id to ShardInfo
    shards: Arc<DashMap<u32, ShardInfo>>,
    /// Consistent hash ring for shard assignment
    hash_ring: Arc<RwLock<ConsistentHashRing>>,
    /// Shard router for query routing
    router: Arc<ShardRouter>,
    /// DAG-based consensus engine
    consensus: Option<Arc<DagConsensus>>,
    /// Discovery service (boxed for type erasure)
    discovery: Box<dyn DiscoveryService>,
    /// Current node ID
    node_id: String,
}

impl ClusterManager {
    /// Create a new cluster manager
    pub fn new(
        config: ClusterConfig,
        node_id: String,
        discovery: Box<dyn DiscoveryService>,
    ) -> Result<Self> {
        let nodes = Arc::new(DashMap::new());
        let shards = Arc::new(DashMap::new());
        let hash_ring = Arc::new(RwLock::new(ConsistentHashRing::new(
            config.replication_factor,
        )));
        let router = Arc::new(ShardRouter::new(config.shard_count));

        let consensus = if config.enable_consensus {
            Some(Arc::new(DagConsensus::new(
                node_id.clone(),
                config.min_quorum_size,
            )))
        } else {
            None
        };

        Ok(Self {
            config,
            nodes,
            shards,
            hash_ring,
            router,
            consensus,
            discovery,
            node_id,
        })
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, node: ClusterNode) -> Result<()> {
        info!("Adding node {} to cluster", node.node_id);

        // Add to hash ring
        {
            let mut ring = self.hash_ring.write();
            ring.add_node(node.node_id.clone());
        }

        // Store node information
        self.nodes.insert(node.node_id.clone(), node.clone());

        // Rebalance shards if needed
        self.rebalance_shards().await?;

        info!("Node {} successfully added", node.node_id);
        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: &str) -> Result<()> {
        info!("Removing node {} from cluster", node_id);

        // Remove from hash ring
        {
            let mut ring = self.hash_ring.write();
            ring.remove_node(node_id);
        }

        // Remove node information
        self.nodes.remove(node_id);

        // Rebalance shards
        self.rebalance_shards().await?;

        info!("Node {} successfully removed", node_id);
        Ok(())
    }

    /// Get node by ID
    pub fn get_node(&self, node_id: &str) -> Option<ClusterNode> {
        self.nodes.get(node_id).map(|n| n.clone())
    }

    /// List all nodes in the cluster
    pub fn list_nodes(&self) -> Vec<ClusterNode> {
        self.nodes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get healthy nodes only
    pub fn healthy_nodes(&self) -> Vec<ClusterNode> {
        self.nodes
            .iter()
            .filter(|entry| entry.value().is_healthy(self.config.node_timeout))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get shard information
    pub fn get_shard(&self, shard_id: u32) -> Option<ShardInfo> {
        self.shards.get(&shard_id).map(|s| s.clone())
    }

    /// List all shards
    pub fn list_shards(&self) -> Vec<ShardInfo> {
        self.shards
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Assign a shard to nodes using consistent hashing
    pub fn assign_shard(&self, shard_id: u32) -> Result<ShardInfo> {
        let ring = self.hash_ring.read();
        let key = format!("shard:{}", shard_id);

        let nodes = ring.get_nodes(&key, self.config.replication_factor);

        if nodes.is_empty() {
            return Err(ClusterError::InvalidConfig(
                "No nodes available for shard assignment".to_string(),
            ));
        }

        let primary_node = nodes[0].clone();
        let replica_nodes = nodes.into_iter().skip(1).collect();

        let shard_info = ShardInfo {
            shard_id,
            primary_node,
            replica_nodes,
            vector_count: 0,
            status: ShardStatus::Active,
            created_at: Utc::now(),
            modified_at: Utc::now(),
        };

        self.shards.insert(shard_id, shard_info.clone());
        Ok(shard_info)
    }

    /// Rebalance shards across nodes
    async fn rebalance_shards(&self) -> Result<()> {
        debug!("Rebalancing shards across cluster");

        for shard_id in 0..self.config.shard_count {
            if let Some(mut shard) = self.shards.get_mut(&shard_id) {
                let ring = self.hash_ring.read();
                let key = format!("shard:{}", shard_id);
                let nodes = ring.get_nodes(&key, self.config.replication_factor);

                if !nodes.is_empty() {
                    shard.primary_node = nodes[0].clone();
                    shard.replica_nodes = nodes.into_iter().skip(1).collect();
                    shard.modified_at = Utc::now();
                }
            } else {
                // Create new shard assignment
                self.assign_shard(shard_id)?;
            }
        }

        debug!("Shard rebalancing complete");
        Ok(())
    }

    /// Run periodic health checks
    pub async fn run_health_checks(&self) -> Result<()> {
        debug!("Running health checks");

        let mut unhealthy_nodes = Vec::new();

        for entry in self.nodes.iter() {
            let node = entry.value();
            if !node.is_healthy(self.config.node_timeout) {
                warn!("Node {} is unhealthy", node.node_id);
                unhealthy_nodes.push(node.node_id.clone());
            }
        }

        // Mark unhealthy nodes as offline
        for node_id in unhealthy_nodes {
            if let Some(mut node) = self.nodes.get_mut(&node_id) {
                node.status = NodeStatus::Offline;
            }
        }

        Ok(())
    }

    /// Start the cluster manager (health checks, discovery, etc.)
    pub async fn start(&self) -> Result<()> {
        info!("Starting cluster manager for node {}", self.node_id);

        // Start discovery service
        let discovered = self.discovery.discover_nodes().await?;
        for node in discovered {
            if node.node_id != self.node_id {
                self.add_node(node).await?;
            }
        }

        // Initialize shards
        for shard_id in 0..self.config.shard_count {
            self.assign_shard(shard_id)?;
        }

        info!("Cluster manager started successfully");
        Ok(())
    }

    /// Get cluster statistics
    pub fn get_stats(&self) -> ClusterStats {
        let nodes = self.list_nodes();
        let shards = self.list_shards();
        let healthy = self.healthy_nodes();

        ClusterStats {
            total_nodes: nodes.len(),
            healthy_nodes: healthy.len(),
            total_shards: shards.len(),
            active_shards: shards
                .iter()
                .filter(|s| s.status == ShardStatus::Active)
                .count(),
            total_vectors: shards.iter().map(|s| s.vector_count).sum(),
        }
    }

    /// Get the shard router
    pub fn router(&self) -> Arc<ShardRouter> {
        Arc::clone(&self.router)
    }

    /// Get the consensus engine
    pub fn consensus(&self) -> Option<Arc<DagConsensus>> {
        self.consensus.as_ref().map(Arc::clone)
    }
}

/// Cluster statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub total_shards: usize,
    pub active_shards: usize,
    pub total_vectors: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_node(id: &str, port: u16) -> ClusterNode {
        ClusterNode::new(
            id.to_string(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
        )
    }

    #[tokio::test]
    async fn test_cluster_node_creation() {
        let node = create_test_node("node1", 8000);
        assert_eq!(node.node_id, "node1");
        assert_eq!(node.status, NodeStatus::Follower);
        assert!(node.is_healthy(Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn test_cluster_manager_creation() {
        let config = ClusterConfig::default();
        let discovery = Box::new(StaticDiscovery::new(vec![]));
        let manager = ClusterManager::new(config, "test-node".to_string(), discovery);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_add_remove_node() {
        let config = ClusterConfig::default();
        let discovery = Box::new(StaticDiscovery::new(vec![]));
        let manager = ClusterManager::new(config, "test-node".to_string(), discovery).unwrap();

        let node = create_test_node("node1", 8000);
        manager.add_node(node).await.unwrap();

        assert_eq!(manager.list_nodes().len(), 1);

        manager.remove_node("node1").await.unwrap();
        assert_eq!(manager.list_nodes().len(), 0);
    }

    #[tokio::test]
    async fn test_shard_assignment() {
        let config = ClusterConfig {
            shard_count: 4,
            replication_factor: 2,
            ..Default::default()
        };
        let discovery = Box::new(StaticDiscovery::new(vec![]));
        let manager = ClusterManager::new(config, "test-node".to_string(), discovery).unwrap();

        // Add some nodes
        for i in 0..3 {
            let node = create_test_node(&format!("node{}", i), 8000 + i);
            manager.add_node(node).await.unwrap();
        }

        // Assign a shard
        let shard = manager.assign_shard(0).unwrap();
        assert_eq!(shard.shard_id, 0);
        assert!(!shard.primary_node.is_empty());
    }
}
