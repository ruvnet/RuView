//! Node discovery mechanisms for cluster formation
//!
//! Supports static configuration and gossip-based discovery.

use crate::{ClusterError, ClusterNode, NodeStatus, Result};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, info, warn};

/// Service for discovering nodes in the cluster
#[async_trait]
pub trait DiscoveryService: Send + Sync {
    /// Discover nodes in the cluster
    async fn discover_nodes(&self) -> Result<Vec<ClusterNode>>;

    /// Register this node in the discovery service
    async fn register_node(&self, node: ClusterNode) -> Result<()>;

    /// Unregister this node from the discovery service
    async fn unregister_node(&self, node_id: &str) -> Result<()>;

    /// Update node heartbeat
    async fn heartbeat(&self, node_id: &str) -> Result<()>;
}

/// Static discovery using predefined node list
pub struct StaticDiscovery {
    /// Predefined list of nodes
    nodes: Arc<RwLock<Vec<ClusterNode>>>,
}

impl StaticDiscovery {
    /// Create a new static discovery service
    pub fn new(nodes: Vec<ClusterNode>) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(nodes)),
        }
    }

    /// Add a node to the static list
    pub fn add_node(&self, node: ClusterNode) {
        let mut nodes = self.nodes.write();
        nodes.push(node);
    }

    /// Remove a node from the static list
    pub fn remove_node(&self, node_id: &str) {
        let mut nodes = self.nodes.write();
        nodes.retain(|n| n.node_id != node_id);
    }
}

#[async_trait]
impl DiscoveryService for StaticDiscovery {
    async fn discover_nodes(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read();
        Ok(nodes.clone())
    }

    async fn register_node(&self, node: ClusterNode) -> Result<()> {
        self.add_node(node);
        Ok(())
    }

    async fn unregister_node(&self, node_id: &str) -> Result<()> {
        self.remove_node(node_id);
        Ok(())
    }

    async fn heartbeat(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write();
        if let Some(node) = nodes.iter_mut().find(|n| n.node_id == node_id) {
            node.heartbeat();
        }
        Ok(())
    }
}

/// Gossip-based discovery protocol
pub struct GossipDiscovery {
    /// Local node information
    local_node: Arc<RwLock<ClusterNode>>,
    /// Known nodes (node_id -> node)
    nodes: Arc<DashMap<String, ClusterNode>>,
    /// Seed nodes to bootstrap gossip
    seed_nodes: Vec<SocketAddr>,
    /// Gossip interval
    gossip_interval: Duration,
    /// Node timeout
    node_timeout: Duration,
}

impl GossipDiscovery {
    /// Create a new gossip discovery service
    pub fn new(
        local_node: ClusterNode,
        seed_nodes: Vec<SocketAddr>,
        gossip_interval: Duration,
        node_timeout: Duration,
    ) -> Self {
        let nodes = Arc::new(DashMap::new());
        nodes.insert(local_node.node_id.clone(), local_node.clone());

        Self {
            local_node: Arc::new(RwLock::new(local_node)),
            nodes,
            seed_nodes,
            gossip_interval,
            node_timeout,
        }
    }

    /// Start the gossip protocol
    pub async fn start(&self) -> Result<()> {
        info!("Starting gossip discovery protocol");

        // Bootstrap from seed nodes
        self.bootstrap().await?;

        // Start periodic gossip
        let nodes = Arc::clone(&self.nodes);
        let gossip_interval = self.gossip_interval;

        tokio::spawn(async move {
            let mut interval = time::interval(gossip_interval);
            loop {
                interval.tick().await;
                Self::gossip_round(&nodes).await;
            }
        });

        Ok(())
    }

    /// Bootstrap by contacting seed nodes
    async fn bootstrap(&self) -> Result<()> {
        debug!("Bootstrapping from {} seed nodes", self.seed_nodes.len());

        for seed_addr in &self.seed_nodes {
            // In a real implementation, this would contact the seed node
            // For now, we'll simulate it
            debug!("Contacting seed node at {}", seed_addr);
        }

        Ok(())
    }

    /// Perform a gossip round
    async fn gossip_round(nodes: &Arc<DashMap<String, ClusterNode>>) {
        // Select random subset of nodes to gossip with
        let node_list: Vec<_> = nodes.iter().map(|e| e.value().clone()).collect();

        if node_list.len() < 2 {
            return;
        }

        debug!("Gossiping with {} nodes", node_list.len());

        // In a real implementation, we would:
        // 1. Select random peers
        // 2. Exchange node lists
        // 3. Merge received information
        // 4. Detect failures
    }

    /// Merge gossip information from another node
    pub fn merge_gossip(&self, remote_nodes: Vec<ClusterNode>) {
        for node in remote_nodes {
            if let Some(mut existing) = self.nodes.get_mut(&node.node_id) {
                // Update if remote has newer information
                if node.last_seen > existing.last_seen {
                    *existing = node;
                }
            } else {
                // Add new node
                self.nodes.insert(node.node_id.clone(), node);
            }
        }
    }

    /// Remove failed nodes
    pub fn prune_failed_nodes(&self) {
        let now = Utc::now();
        self.nodes.retain(|_, node| {
            let elapsed = now
                .signed_duration_since(node.last_seen)
                .to_std()
                .unwrap_or(Duration::MAX);
            elapsed < self.node_timeout
        });
    }

    /// Get gossip statistics
    pub fn get_stats(&self) -> GossipStats {
        let nodes: Vec<_> = self.nodes.iter().map(|e| e.value().clone()).collect();
        let healthy = nodes
            .iter()
            .filter(|n| n.is_healthy(self.node_timeout))
            .count();

        GossipStats {
            total_nodes: nodes.len(),
            healthy_nodes: healthy,
            seed_nodes: self.seed_nodes.len(),
        }
    }
}

#[async_trait]
impl DiscoveryService for GossipDiscovery {
    async fn discover_nodes(&self) -> Result<Vec<ClusterNode>> {
        Ok(self.nodes.iter().map(|e| e.value().clone()).collect())
    }

    async fn register_node(&self, node: ClusterNode) -> Result<()> {
        self.nodes.insert(node.node_id.clone(), node);
        Ok(())
    }

    async fn unregister_node(&self, node_id: &str) -> Result<()> {
        self.nodes.remove(node_id);
        Ok(())
    }

    async fn heartbeat(&self, node_id: &str) -> Result<()> {
        if let Some(mut node) = self.nodes.get_mut(node_id) {
            node.heartbeat();
        }
        Ok(())
    }
}

/// Gossip protocol statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipStats {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub seed_nodes: usize,
}

/// Multicast-based discovery (for local networks)
pub struct MulticastDiscovery {
    /// Local node
    local_node: ClusterNode,
    /// Discovered nodes
    nodes: Arc<DashMap<String, ClusterNode>>,
    /// Multicast address
    multicast_addr: String,
    /// Multicast port
    multicast_port: u16,
}

impl MulticastDiscovery {
    /// Create a new multicast discovery service
    pub fn new(local_node: ClusterNode, multicast_addr: String, multicast_port: u16) -> Self {
        Self {
            local_node,
            nodes: Arc::new(DashMap::new()),
            multicast_addr,
            multicast_port,
        }
    }

    /// Start multicast discovery
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting multicast discovery on {}:{}",
            self.multicast_addr, self.multicast_port
        );

        // In a real implementation, this would:
        // 1. Join multicast group
        // 2. Send periodic announcements
        // 3. Listen for other nodes
        // 4. Update node list

        Ok(())
    }
}

#[async_trait]
impl DiscoveryService for MulticastDiscovery {
    async fn discover_nodes(&self) -> Result<Vec<ClusterNode>> {
        Ok(self.nodes.iter().map(|e| e.value().clone()).collect())
    }

    async fn register_node(&self, node: ClusterNode) -> Result<()> {
        self.nodes.insert(node.node_id.clone(), node);
        Ok(())
    }

    async fn unregister_node(&self, node_id: &str) -> Result<()> {
        self.nodes.remove(node_id);
        Ok(())
    }

    async fn heartbeat(&self, node_id: &str) -> Result<()> {
        if let Some(mut node) = self.nodes.get_mut(node_id) {
            node.heartbeat();
        }
        Ok(())
    }
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
    async fn test_static_discovery() {
        let node1 = create_test_node("node1", 8000);
        let node2 = create_test_node("node2", 8001);

        let discovery = StaticDiscovery::new(vec![node1, node2]);

        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_static_discovery_register() {
        let discovery = StaticDiscovery::new(vec![]);

        let node = create_test_node("node1", 8000);
        discovery.register_node(node).await.unwrap();

        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_gossip_discovery() {
        let local_node = create_test_node("local", 8000);
        let seed_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9000);

        let discovery = GossipDiscovery::new(
            local_node,
            vec![seed_addr],
            Duration::from_secs(5),
            Duration::from_secs(30),
        );

        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1); // Only local node initially
    }

    #[tokio::test]
    async fn test_gossip_merge() {
        let local_node = create_test_node("local", 8000);
        let discovery = GossipDiscovery::new(
            local_node,
            vec![],
            Duration::from_secs(5),
            Duration::from_secs(30),
        );

        let remote_nodes = vec![
            create_test_node("node1", 8001),
            create_test_node("node2", 8002),
        ];

        discovery.merge_gossip(remote_nodes);

        let stats = discovery.get_stats();
        assert_eq!(stats.total_nodes, 3); // local + 2 remote
    }
}
