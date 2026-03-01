//! Swarm agent implementation
//!
//! Core agent that handles communication, learning sync, and task execution.

use crate::{
    intelligence::IntelligenceSync,
    memory::VectorMemory,
    protocol::{MessagePayload, MessageType, SwarmMessage},
    transport::{TransportConfig, TransportFactory, TransportHandle},
    Result, SwarmConfig, SwarmError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};

/// Agent roles in the swarm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// Coordinator manages the swarm
    Coordinator,
    /// Worker executes tasks
    Worker,
    /// Scout explores and gathers information
    Scout,
    /// Specialist has domain expertise
    Specialist,
}

impl Default for AgentRole {
    fn default() -> Self {
        AgentRole::Worker
    }
}

/// Peer agent info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub agent_id: String,
    pub role: AgentRole,
    pub capabilities: Vec<String>,
    pub last_seen: u64,
    pub connected: bool,
}

/// Swarm agent
pub struct SwarmAgent {
    config: SwarmConfig,
    transport: Option<TransportHandle>,
    intelligence: Arc<IntelligenceSync>,
    memory: Arc<VectorMemory>,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    message_tx: mpsc::Sender<SwarmMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<SwarmMessage>>>,
    running: Arc<RwLock<bool>>,
}

impl SwarmAgent {
    /// Create new swarm agent
    pub async fn new(config: SwarmConfig) -> Result<Self> {
        let intelligence = Arc::new(IntelligenceSync::new(&config.agent_id));
        let memory = Arc::new(VectorMemory::new(&config.agent_id, 10000));
        let (message_tx, message_rx) = mpsc::channel(1024);

        Ok(Self {
            config,
            transport: None,
            intelligence,
            memory,
            peers: Arc::new(RwLock::new(HashMap::new())),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Get agent ID
    pub fn id(&self) -> &str {
        &self.config.agent_id
    }

    /// Get agent role
    pub fn role(&self) -> AgentRole {
        self.config.agent_role
    }

    /// Connect to swarm
    pub async fn join_swarm(&mut self, coordinator_url: &str) -> Result<()> {
        tracing::info!("Joining swarm at {}", coordinator_url);

        // Create transport
        let transport_config = TransportConfig {
            transport_type: self.config.transport,
            ..Default::default()
        };

        let transport = TransportFactory::create(&transport_config, Some(coordinator_url)).await?;
        self.transport = Some(transport);

        // Send join message
        let join_msg = SwarmMessage::join(
            &self.config.agent_id,
            &format!("{:?}", self.config.agent_role),
            vec!["learning".to_string(), "memory".to_string()],
        );

        self.send_message(join_msg).await?;

        *self.running.write().await = true;

        tracing::info!("Joined swarm successfully");
        Ok(())
    }

    /// Leave swarm gracefully
    pub async fn leave_swarm(&mut self) -> Result<()> {
        tracing::info!("Leaving swarm");

        *self.running.write().await = false;

        let leave_msg = SwarmMessage::leave(&self.config.agent_id);
        self.send_message(leave_msg).await?;

        self.transport = None;

        Ok(())
    }

    /// Send message to swarm
    pub async fn send_message(&self, msg: SwarmMessage) -> Result<()> {
        if let Some(ref transport) = self.transport {
            let bytes = msg.to_bytes().map_err(|e| SwarmError::Serialization(e.to_string()))?;
            transport.send(bytes).await?;
        }
        Ok(())
    }

    /// Broadcast message to all peers
    pub async fn broadcast(&self, msg: SwarmMessage) -> Result<()> {
        self.send_message(msg).await
    }

    /// Sync learning patterns with swarm
    pub async fn sync_patterns(&self) -> Result<()> {
        let state = self.intelligence.get_state();
        let msg = SwarmMessage::sync_patterns(&self.config.agent_id, state);
        self.broadcast(msg).await
    }

    /// Request patterns from specific peer
    pub async fn request_patterns_from(&self, peer_id: &str, since_version: u64) -> Result<()> {
        let msg = SwarmMessage::directed(
            MessageType::RequestPatterns,
            &self.config.agent_id,
            peer_id,
            MessagePayload::Request(crate::protocol::RequestPayload {
                since_version,
                max_entries: 1000,
            }),
        );
        self.send_message(msg).await
    }

    /// Update learning pattern locally
    pub async fn learn(&self, state: &str, action: &str, reward: f64) {
        self.intelligence.update_pattern(state, action, reward);
    }

    /// Get best action for state
    pub async fn get_best_action(&self, state: &str, actions: &[String]) -> Option<(String, f64)> {
        self.intelligence.get_best_action(state, actions)
    }

    /// Store vector in shared memory
    pub async fn store_memory(&self, content: &str, embedding: Vec<f32>) -> Result<String> {
        self.memory.store(content, embedding)
    }

    /// Search vector memory
    pub async fn search_memory(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        self.memory
            .search(query, top_k)
            .into_iter()
            .map(|(entry, score)| (entry.content, score))
            .collect()
    }

    /// Get connected peers
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }

    /// Get swarm statistics
    pub async fn get_stats(&self) -> AgentStats {
        let intelligence_stats = self.intelligence.get_swarm_stats();
        let memory_stats = self.memory.stats();
        let peers = self.peers.read().await;

        AgentStats {
            agent_id: self.config.agent_id.clone(),
            role: self.config.agent_role,
            connected_peers: peers.len(),
            total_patterns: intelligence_stats.total_patterns,
            total_memories: memory_stats.total_entries,
            avg_confidence: intelligence_stats.avg_confidence,
            is_running: *self.running.read().await,
        }
    }

    /// Start background sync loop
    pub async fn start_sync_loop(&self) {
        let intelligence = self.intelligence.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            let mut sync_interval = interval(Duration::from_millis(config.sync_interval_ms));

            while *running.read().await {
                sync_interval.tick().await;

                // Sync patterns periodically
                if config.enable_learning {
                    let state = intelligence.get_state();
                    let msg = SwarmMessage::sync_patterns(&config.agent_id, state);
                    let _ = message_tx.send(msg).await;
                }
            }
        });
    }

    /// Handle incoming message
    pub async fn handle_message(&self, msg: SwarmMessage) -> Result<()> {
        match msg.message_type {
            MessageType::Join => {
                if let MessagePayload::Join(payload) = msg.payload {
                    let sender_id = msg.sender_id.clone();
                    let peer = PeerInfo {
                        agent_id: sender_id.clone(),
                        role: match payload.agent_role.as_str() {
                            "Coordinator" => AgentRole::Coordinator,
                            "Scout" => AgentRole::Scout,
                            "Specialist" => AgentRole::Specialist,
                            _ => AgentRole::Worker,
                        },
                        capabilities: payload.capabilities,
                        last_seen: chrono::Utc::now().timestamp_millis() as u64,
                        connected: true,
                    };
                    self.peers.write().await.insert(sender_id, peer);
                }
            }
            MessageType::Leave => {
                self.peers.write().await.remove(&msg.sender_id);
            }
            MessageType::Ping => {
                let pong = SwarmMessage::pong(&self.config.agent_id);
                self.send_message(pong).await?;
            }
            MessageType::SyncPatterns => {
                if let MessagePayload::Patterns(payload) = msg.payload {
                    self.intelligence
                        .merge_peer_state(&msg.sender_id, &serde_json::to_vec(&payload.state).unwrap())?;
                }
            }
            MessageType::RequestPatterns => {
                if let MessagePayload::Request(payload) = msg.payload {
                    let delta = self.intelligence.get_delta(payload.since_version);
                    let response = SwarmMessage::sync_patterns(&self.config.agent_id, delta);
                    self.send_message(response).await?;
                }
            }
            _ => {}
        }

        // Update peer last_seen
        if let Some(peer) = self.peers.write().await.get_mut(&msg.sender_id) {
            peer.last_seen = chrono::Utc::now().timestamp_millis() as u64;
        }

        Ok(())
    }
}

/// Agent statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent_id: String,
    pub role: AgentRole,
    pub connected_peers: usize,
    pub total_patterns: usize,
    pub total_memories: usize,
    pub avg_confidence: f64,
    pub is_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transport;

    #[tokio::test]
    async fn test_agent_creation() {
        let config = SwarmConfig::default()
            .with_agent_id("test-agent")
            .with_transport(Transport::SharedMemory);

        let agent = SwarmAgent::new(config).await.unwrap();

        assert_eq!(agent.id(), "test-agent");
        assert!(matches!(agent.role(), AgentRole::Worker));
    }

    #[tokio::test]
    async fn test_agent_learning() {
        let config = SwarmConfig::default().with_agent_id("learning-agent");
        let agent = SwarmAgent::new(config).await.unwrap();

        agent.learn("edit_ts", "coder", 0.8).await;
        agent.learn("edit_ts", "reviewer", 0.6).await;

        let actions = vec!["coder".to_string(), "reviewer".to_string()];
        let best = agent.get_best_action("edit_ts", &actions).await;

        assert!(best.is_some());
        assert_eq!(best.unwrap().0, "coder");
    }
}
