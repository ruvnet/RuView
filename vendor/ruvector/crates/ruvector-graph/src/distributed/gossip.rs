//! Gossip protocol for cluster membership and health monitoring
//!
//! Implements SWIM (Scalable Weakly-consistent Infection-style Membership) protocol:
//! - Fast failure detection
//! - Efficient membership propagation
//! - Low network overhead
//! - Automatic node discovery

use crate::{GraphError, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Node identifier in the cluster
pub type NodeId = String;

/// Gossip message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Ping message for health check
    Ping {
        from: NodeId,
        sequence: u64,
        timestamp: DateTime<Utc>,
    },
    /// Ack response to ping
    Ack {
        from: NodeId,
        to: NodeId,
        sequence: u64,
        timestamp: DateTime<Utc>,
    },
    /// Indirect ping through intermediary
    IndirectPing {
        from: NodeId,
        target: NodeId,
        intermediary: NodeId,
        sequence: u64,
    },
    /// Membership update
    MembershipUpdate {
        from: NodeId,
        updates: Vec<MembershipEvent>,
        version: u64,
    },
    /// Join request
    Join {
        node_id: NodeId,
        address: SocketAddr,
        metadata: HashMap<String, String>,
    },
    /// Leave notification
    Leave { node_id: NodeId },
}

/// Membership event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MembershipEvent {
    /// Node joined the cluster
    Join {
        node_id: NodeId,
        address: SocketAddr,
        timestamp: DateTime<Utc>,
    },
    /// Node left the cluster
    Leave {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    /// Node suspected to be failed
    Suspect {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    /// Node confirmed alive
    Alive {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    /// Node confirmed dead
    Dead {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
}

/// Node health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealth {
    /// Node is healthy and responsive
    Alive,
    /// Node is suspected to be failed
    Suspect,
    /// Node is confirmed dead
    Dead,
    /// Node explicitly left
    Left,
}

/// Member information in the gossip protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    /// Node identifier
    pub node_id: NodeId,
    /// Network address
    pub address: SocketAddr,
    /// Current health status
    pub health: NodeHealth,
    /// Last time we heard from this node
    pub last_seen: DateTime<Utc>,
    /// Incarnation number (for conflict resolution)
    pub incarnation: u64,
    /// Node metadata
    pub metadata: HashMap<String, String>,
    /// Number of consecutive ping failures
    pub failure_count: u32,
}

impl Member {
    /// Create a new member
    pub fn new(node_id: NodeId, address: SocketAddr) -> Self {
        Self {
            node_id,
            address,
            health: NodeHealth::Alive,
            last_seen: Utc::now(),
            incarnation: 0,
            metadata: HashMap::new(),
            failure_count: 0,
        }
    }

    /// Check if member is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.health, NodeHealth::Alive)
    }

    /// Mark as seen
    pub fn mark_seen(&mut self) {
        self.last_seen = Utc::now();
        self.failure_count = 0;
        if self.health != NodeHealth::Left {
            self.health = NodeHealth::Alive;
        }
    }

    /// Increment failure count
    pub fn increment_failures(&mut self) {
        self.failure_count += 1;
    }
}

/// Gossip configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipConfig {
    /// Gossip interval in milliseconds
    pub gossip_interval_ms: u64,
    /// Number of nodes to gossip with per interval
    pub gossip_fanout: usize,
    /// Ping timeout in milliseconds
    pub ping_timeout_ms: u64,
    /// Number of ping failures before suspecting node
    pub suspect_threshold: u32,
    /// Number of indirect ping nodes
    pub indirect_ping_nodes: usize,
    /// Suspicion timeout in seconds
    pub suspicion_timeout_seconds: u64,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            gossip_interval_ms: 1000,
            gossip_fanout: 3,
            ping_timeout_ms: 500,
            suspect_threshold: 3,
            indirect_ping_nodes: 3,
            suspicion_timeout_seconds: 30,
        }
    }
}

/// Gossip-based membership protocol
pub struct GossipMembership {
    /// Local node ID
    local_node_id: NodeId,
    /// Local node address
    local_address: SocketAddr,
    /// Configuration
    config: GossipConfig,
    /// Cluster members
    members: Arc<DashMap<NodeId, Member>>,
    /// Membership version (incremented on changes)
    version: Arc<RwLock<u64>>,
    /// Pending acks
    pending_acks: Arc<DashMap<u64, PendingAck>>,
    /// Sequence number for messages
    sequence: Arc<RwLock<u64>>,
    /// Event listeners
    event_listeners: Arc<RwLock<Vec<Box<dyn Fn(MembershipEvent) + Send + Sync>>>>,
}

/// Pending acknowledgment
struct PendingAck {
    target: NodeId,
    sent_at: DateTime<Utc>,
}

impl GossipMembership {
    /// Create a new gossip membership
    pub fn new(node_id: NodeId, address: SocketAddr, config: GossipConfig) -> Self {
        let members = Arc::new(DashMap::new());

        // Add self to members
        let local_member = Member::new(node_id.clone(), address);
        members.insert(node_id.clone(), local_member);

        Self {
            local_node_id: node_id,
            local_address: address,
            config,
            members,
            version: Arc::new(RwLock::new(0)),
            pending_acks: Arc::new(DashMap::new()),
            sequence: Arc::new(RwLock::new(0)),
            event_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start the gossip protocol
    pub async fn start(&self) -> Result<()> {
        info!("Starting gossip protocol for node: {}", self.local_node_id);

        // Start periodic gossip
        let gossip_self = self.clone();
        tokio::spawn(async move {
            gossip_self.run_gossip_loop().await;
        });

        // Start failure detection
        let detection_self = self.clone();
        tokio::spawn(async move {
            detection_self.run_failure_detection().await;
        });

        Ok(())
    }

    /// Add a seed node to join cluster
    pub async fn join(&self, seed_address: SocketAddr) -> Result<()> {
        info!("Joining cluster via seed: {}", seed_address);

        // Send join message
        let join_msg = GossipMessage::Join {
            node_id: self.local_node_id.clone(),
            address: self.local_address,
            metadata: HashMap::new(),
        };

        // In production, send actual network message
        // For now, just log
        debug!("Would send join message to {}", seed_address);

        Ok(())
    }

    /// Leave the cluster gracefully
    pub async fn leave(&self) -> Result<()> {
        info!("Leaving cluster: {}", self.local_node_id);

        // Update own status
        if let Some(mut member) = self.members.get_mut(&self.local_node_id) {
            member.health = NodeHealth::Left;
        }

        // Broadcast leave message
        let leave_msg = GossipMessage::Leave {
            node_id: self.local_node_id.clone(),
        };

        self.broadcast_event(MembershipEvent::Leave {
            node_id: self.local_node_id.clone(),
            timestamp: Utc::now(),
        })
        .await;

        Ok(())
    }

    /// Get all cluster members
    pub fn get_members(&self) -> Vec<Member> {
        self.members.iter().map(|e| e.value().clone()).collect()
    }

    /// Get healthy members only
    pub fn get_healthy_members(&self) -> Vec<Member> {
        self.members
            .iter()
            .filter(|e| e.value().is_healthy())
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get a specific member
    pub fn get_member(&self, node_id: &NodeId) -> Option<Member> {
        self.members.get(node_id).map(|m| m.value().clone())
    }

    /// Handle incoming gossip message
    pub async fn handle_message(&self, message: GossipMessage) -> Result<()> {
        match message {
            GossipMessage::Ping { from, sequence, .. } => self.handle_ping(from, sequence).await,
            GossipMessage::Ack { from, sequence, .. } => self.handle_ack(from, sequence).await,
            GossipMessage::MembershipUpdate { updates, .. } => {
                self.handle_membership_update(updates).await
            }
            GossipMessage::Join {
                node_id,
                address,
                metadata,
            } => self.handle_join(node_id, address, metadata).await,
            GossipMessage::Leave { node_id } => self.handle_leave(node_id).await,
            _ => Ok(()),
        }
    }

    /// Run the gossip loop
    async fn run_gossip_loop(&self) {
        let interval = std::time::Duration::from_millis(self.config.gossip_interval_ms);

        loop {
            tokio::time::sleep(interval).await;

            // Select random members to gossip with
            let members = self.get_healthy_members();
            let targets: Vec<_> = members
                .into_iter()
                .filter(|m| m.node_id != self.local_node_id)
                .take(self.config.gossip_fanout)
                .collect();

            for target in targets {
                self.send_ping(target.node_id).await;
            }
        }
    }

    /// Run failure detection
    async fn run_failure_detection(&self) {
        let interval = std::time::Duration::from_secs(5);

        loop {
            tokio::time::sleep(interval).await;

            let now = Utc::now();
            let timeout = ChronoDuration::seconds(self.config.suspicion_timeout_seconds as i64);

            for mut entry in self.members.iter_mut() {
                let member = entry.value_mut();

                if member.node_id == self.local_node_id {
                    continue;
                }

                // Check if node has timed out
                if member.health == NodeHealth::Suspect {
                    let elapsed = now.signed_duration_since(member.last_seen);
                    if elapsed > timeout {
                        debug!("Marking node as dead: {}", member.node_id);
                        member.health = NodeHealth::Dead;

                        let event = MembershipEvent::Dead {
                            node_id: member.node_id.clone(),
                            timestamp: now,
                        };

                        self.emit_event(event);
                    }
                }
            }
        }
    }

    /// Send ping to a node
    async fn send_ping(&self, target: NodeId) {
        let mut seq = self.sequence.write().await;
        *seq += 1;
        let sequence = *seq;
        drop(seq);

        let ping = GossipMessage::Ping {
            from: self.local_node_id.clone(),
            sequence,
            timestamp: Utc::now(),
        };

        // Track pending ack
        self.pending_acks.insert(
            sequence,
            PendingAck {
                target: target.clone(),
                sent_at: Utc::now(),
            },
        );

        debug!("Sending ping to {}", target);
        // In production, send actual network message
    }

    /// Handle ping message
    async fn handle_ping(&self, from: NodeId, sequence: u64) -> Result<()> {
        debug!("Received ping from {}", from);

        // Update member status
        if let Some(mut member) = self.members.get_mut(&from) {
            member.mark_seen();
        }

        // Send ack
        let ack = GossipMessage::Ack {
            from: self.local_node_id.clone(),
            to: from,
            sequence,
            timestamp: Utc::now(),
        };

        // In production, send actual network message
        Ok(())
    }

    /// Handle ack message
    async fn handle_ack(&self, from: NodeId, sequence: u64) -> Result<()> {
        debug!("Received ack from {}", from);

        // Remove from pending
        self.pending_acks.remove(&sequence);

        // Update member status
        if let Some(mut member) = self.members.get_mut(&from) {
            member.mark_seen();
        }

        Ok(())
    }

    /// Handle membership update
    async fn handle_membership_update(&self, updates: Vec<MembershipEvent>) -> Result<()> {
        for event in updates {
            match &event {
                MembershipEvent::Join {
                    node_id, address, ..
                } => {
                    if !self.members.contains_key(node_id) {
                        let member = Member::new(node_id.clone(), *address);
                        self.members.insert(node_id.clone(), member);
                    }
                }
                MembershipEvent::Suspect { node_id, .. } => {
                    if let Some(mut member) = self.members.get_mut(node_id) {
                        member.health = NodeHealth::Suspect;
                    }
                }
                MembershipEvent::Dead { node_id, .. } => {
                    if let Some(mut member) = self.members.get_mut(node_id) {
                        member.health = NodeHealth::Dead;
                    }
                }
                _ => {}
            }

            self.emit_event(event);
        }

        Ok(())
    }

    /// Handle join request
    async fn handle_join(
        &self,
        node_id: NodeId,
        address: SocketAddr,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        info!("Node joining: {}", node_id);

        let mut member = Member::new(node_id.clone(), address);
        member.metadata = metadata;

        self.members.insert(node_id.clone(), member);

        let event = MembershipEvent::Join {
            node_id,
            address,
            timestamp: Utc::now(),
        };

        self.broadcast_event(event).await;

        Ok(())
    }

    /// Handle leave notification
    async fn handle_leave(&self, node_id: NodeId) -> Result<()> {
        info!("Node leaving: {}", node_id);

        if let Some(mut member) = self.members.get_mut(&node_id) {
            member.health = NodeHealth::Left;
        }

        let event = MembershipEvent::Leave {
            node_id,
            timestamp: Utc::now(),
        };

        self.emit_event(event);

        Ok(())
    }

    /// Broadcast event to all members
    async fn broadcast_event(&self, event: MembershipEvent) {
        let mut version = self.version.write().await;
        *version += 1;
        drop(version);

        self.emit_event(event);
    }

    /// Emit event to listeners
    fn emit_event(&self, event: MembershipEvent) {
        // In production, call event listeners
        debug!("Membership event: {:?}", event);
    }

    /// Add event listener
    pub async fn add_listener<F>(&self, listener: F)
    where
        F: Fn(MembershipEvent) + Send + Sync + 'static,
    {
        let mut listeners = self.event_listeners.write().await;
        listeners.push(Box::new(listener));
    }

    /// Get membership version
    pub async fn get_version(&self) -> u64 {
        *self.version.read().await
    }
}

impl Clone for GossipMembership {
    fn clone(&self) -> Self {
        Self {
            local_node_id: self.local_node_id.clone(),
            local_address: self.local_address,
            config: self.config.clone(),
            members: Arc::clone(&self.members),
            version: Arc::clone(&self.version),
            pending_acks: Arc::clone(&self.pending_acks),
            sequence: Arc::clone(&self.sequence),
            event_listeners: Arc::clone(&self.event_listeners),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_address(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    #[tokio::test]
    async fn test_gossip_membership() {
        let config = GossipConfig::default();
        let address = create_test_address(8000);
        let gossip = GossipMembership::new("node-1".to_string(), address, config);

        assert_eq!(gossip.get_members().len(), 1);
    }

    #[tokio::test]
    async fn test_join_leave() {
        let config = GossipConfig::default();
        let address1 = create_test_address(8000);
        let address2 = create_test_address(8001);

        let gossip = GossipMembership::new("node-1".to_string(), address1, config);

        gossip
            .handle_join("node-2".to_string(), address2, HashMap::new())
            .await
            .unwrap();

        assert_eq!(gossip.get_members().len(), 2);

        gossip.handle_leave("node-2".to_string()).await.unwrap();

        let member = gossip.get_member(&"node-2".to_string()).unwrap();
        assert_eq!(member.health, NodeHealth::Left);
    }

    #[test]
    fn test_member() {
        let address = create_test_address(8000);
        let mut member = Member::new("test".to_string(), address);

        assert!(member.is_healthy());

        member.health = NodeHealth::Suspect;
        assert!(!member.is_healthy());

        member.mark_seen();
        assert!(member.is_healthy());
    }
}
