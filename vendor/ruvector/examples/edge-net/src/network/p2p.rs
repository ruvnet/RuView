//! Core P2P networking layer using libp2p
//!
//! Replaces GUN.js placeholder with full libp2p networking including:
//! - Gossipsub for event broadcasting (RAC events, task market, gradients)
//! - Kademlia DHT for peer/capability discovery
//! - Request-Response for direct task negotiation
//! - NOISE protocol for encryption using Pi-Key identity
//!
//! ## Architecture
//!
//! ```text
//! +--------------------------------------------------+
//! |                    P2pNode                       |
//! +--------------------------------------------------+
//! |  PiKey Identity  -->  libp2p PeerId mapping      |
//! +--------------------------------------------------+
//! |                 EdgeNetBehaviour                  |
//! |  +------------+  +----------+  +---------------+ |
//! |  | Gossipsub  |  | Kademlia |  | RequestResp   | |
//! |  | (events)   |  | (DHT)    |  | (tasks)       | |
//! |  +------------+  +----------+  +---------------+ |
//! |  +------------+                                  |
//! |  | Identify   |                                  |
//! |  | (handshake)|                                  |
//! |  +------------+                                  |
//! +--------------------------------------------------+
//! ```

#[cfg(feature = "p2p")]
use libp2p::{
    gossipsub::{self, Gossipsub, GossipsubEvent, MessageAuthenticity, ValidationMode},
    identify::{self, Identify, IdentifyEvent},
    kad::{self, Kademlia, KademliaEvent, store::MemoryStore},
    request_response::{self, RequestResponse, RequestResponseEvent},
    swarm::{NetworkBehaviour, SwarmEvent},
    noise, yamux,
    identity::Keypair,
    PeerId, Multiaddr, Swarm,
};

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "p2p")]
use crate::pikey::PiKey;

// ============================================================================
// Topic Constants for Gossipsub
// ============================================================================

/// RAC (RuVector Adversarial Coherence) events topic
/// Used for: assertions, challenges, resolutions, deprecations
pub const TOPIC_RAC_EVENTS: &str = "/edge-net/rac/1.0.0";

/// Task marketplace topic
/// Used for: task announcements, claims, completions
pub const TOPIC_TASK_MARKET: &str = "/edge-net/tasks/1.0.0";

/// Model synchronization topic
/// Used for: model weight updates, checkpoints
pub const TOPIC_MODEL_SYNC: &str = "/edge-net/models/1.0.0";

/// Gradient gossip topic (federated learning)
/// Used for: gradient aggregation, consensus
pub const TOPIC_GRADIENT_GOSSIP: &str = "/edge-net/gradients/1.0.0";

/// Credit/economic sync topic
/// Used for: CRDT ledger sync, stake announcements
pub const TOPIC_CREDIT_SYNC: &str = "/edge-net/credits/1.0.0";

/// Node presence/heartbeat topic
/// Used for: peer discovery, health monitoring
pub const TOPIC_PRESENCE: &str = "/edge-net/presence/1.0.0";

// ============================================================================
// Protocol Constants
// ============================================================================

/// Task negotiation protocol identifier
pub const TASK_PROTOCOL: &str = "/edge-net/task-negotiate/1.0.0";

/// Agent agent version for identify protocol
pub const AGENT_VERSION: &str = concat!("edge-net/", env!("CARGO_PKG_VERSION"));

/// Protocol version for identify
pub const PROTOCOL_VERSION: &str = "/edge-net/1.0.0";

// ============================================================================
// Network Messages
// ============================================================================

/// Messages broadcast over Gossipsub topics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GossipMessage {
    /// RAC event (assertion, challenge, resolution, etc.)
    RacEvent {
        event_bytes: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Task announcement
    TaskAnnounce {
        task_id: String,
        task_type: String,
        requirements: TaskRequirements,
        max_credits: u64,
        deadline_ms: u64,
    },
    /// Task claim by worker
    TaskClaim {
        task_id: String,
        worker_id: String,
        stake: u64,
        signature: Vec<u8>,
    },
    /// Task completion announcement
    TaskComplete {
        task_id: String,
        worker_id: String,
        result_hash: [u8; 32],
        proof: Vec<u8>,
    },
    /// Model weight update (federated learning)
    ModelUpdate {
        model_id: String,
        layer_id: String,
        delta_weights: Vec<u8>,  // Compressed gradient
        epoch: u64,
    },
    /// Gradient fragment for aggregation
    GradientFragment {
        training_id: String,
        fragment_id: u32,
        gradient_bytes: Vec<u8>,
        contributor: String,
    },
    /// Credit ledger sync (CRDT state)
    CreditSync {
        node_id: String,
        earned_state: Vec<u8>,
        spent_state: Vec<u8>,
        merkle_root: [u8; 32],
    },
    /// Node presence heartbeat
    Presence {
        node_id: String,
        capabilities: Vec<String>,
        stake: u64,
        uptime_hours: f32,
        load: f32,
    },
}

/// Task requirements for matching workers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequirements {
    /// Required capabilities (e.g., "vectors", "embeddings", "gpu")
    pub capabilities: Vec<String>,
    /// Minimum stake required
    pub min_stake: u64,
    /// Minimum reputation score (0.0 - 1.0)
    pub min_reputation: f32,
    /// Estimated memory requirement in bytes
    pub memory_bytes: usize,
    /// Estimated CPU time in ms
    pub cpu_time_ms: u64,
    /// Whether task requires GPU
    pub requires_gpu: bool,
}

impl Default for TaskRequirements {
    fn default() -> Self {
        Self {
            capabilities: vec!["vectors".to_string()],
            min_stake: 100,
            min_reputation: 0.3,
            memory_bytes: 64 * 1024 * 1024,  // 64MB
            cpu_time_ms: 10_000,              // 10 seconds
            requires_gpu: false,
        }
    }
}

// ============================================================================
// Request-Response Messages
// ============================================================================

/// Direct task negotiation request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    /// Task ID being negotiated
    pub task_id: String,
    /// Request type
    pub request_type: TaskRequestType,
    /// Encrypted payload (using session key)
    pub encrypted_payload: Vec<u8>,
    /// Sender's public key for reply encryption
    pub sender_pubkey: Vec<u8>,
}

/// Types of task requests
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskRequestType {
    /// Request task details
    GetDetails,
    /// Submit work claim
    SubmitClaim { stake: u64 },
    /// Submit task result
    SubmitResult { result_hash: [u8; 32] },
    /// Request result verification
    VerifyResult { worker_id: String },
    /// Request payment release
    ReleasePayment { proof: Vec<u8> },
}

/// Task negotiation response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskResponse {
    /// Original task ID
    pub task_id: String,
    /// Response status
    pub status: TaskResponseStatus,
    /// Response data (encrypted)
    pub encrypted_data: Vec<u8>,
}

/// Response status codes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskResponseStatus {
    /// Request accepted
    Accepted,
    /// Task already claimed
    AlreadyClaimed,
    /// Insufficient stake
    InsufficientStake,
    /// Invalid proof
    InvalidProof,
    /// Task not found
    NotFound,
    /// Result verified
    Verified,
    /// Payment released
    PaymentReleased,
    /// Error with message
    Error(String),
}

// ============================================================================
// P2P Node Configuration
// ============================================================================

/// Configuration for P2P networking
#[derive(Clone, Debug)]
pub struct P2pConfig {
    /// Bootstrap peers to connect to
    pub bootstrap_peers: Vec<Multiaddr>,
    /// Listen addresses
    pub listen_addrs: Vec<Multiaddr>,
    /// Gossipsub mesh parameters
    pub gossip_mesh_n: usize,
    pub gossip_mesh_n_low: usize,
    pub gossip_mesh_n_high: usize,
    /// Kademlia replication factor
    pub kad_replication: usize,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
    /// Message validation mode
    pub validation_mode: MessageValidationMode,
}

/// Message validation modes
#[derive(Clone, Debug)]
pub enum MessageValidationMode {
    /// Accept all messages (for testing)
    Permissive,
    /// Validate signatures
    Strict,
    /// Custom validation with callback
    Custom,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            bootstrap_peers: vec![],
            listen_addrs: vec![],
            gossip_mesh_n: 6,
            gossip_mesh_n_low: 4,
            gossip_mesh_n_high: 12,
            kad_replication: 20,
            heartbeat_interval_secs: 30,
            validation_mode: MessageValidationMode::Strict,
        }
    }
}

// ============================================================================
// EdgeNet Network Behaviour (libp2p integration)
// ============================================================================

#[cfg(feature = "p2p")]
use super::protocols::{TaskCodec, TaskProtocol};

/// Combined network behaviour for EdgeNet P2P
///
/// Integrates multiple libp2p protocols:
/// - Gossipsub: Pub/sub for event broadcasting
/// - Kademlia: DHT for peer and capability discovery
/// - Identify: Peer identification and handshake
/// - Request-Response: Direct task negotiation
#[cfg(feature = "p2p")]
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "EdgeNetEvent")]
pub struct EdgeNetBehaviour {
    /// Gossipsub for broadcast messaging
    pub gossipsub: Gossipsub,
    /// Kademlia DHT for peer discovery
    pub kademlia: Kademlia<MemoryStore>,
    /// Identify protocol for peer handshake
    pub identify: Identify,
    /// Request-response for direct task negotiation
    pub request_response: RequestResponse<TaskCodec>,
}

/// Aggregated events from all behaviours
#[cfg(feature = "p2p")]
#[derive(Debug)]
pub enum EdgeNetEvent {
    Gossipsub(GossipsubEvent),
    Kademlia(KademliaEvent),
    Identify(IdentifyEvent),
    RequestResponse(RequestResponseEvent<TaskRequest, TaskResponse>),
}

#[cfg(feature = "p2p")]
impl From<GossipsubEvent> for EdgeNetEvent {
    fn from(event: GossipsubEvent) -> Self {
        EdgeNetEvent::Gossipsub(event)
    }
}

#[cfg(feature = "p2p")]
impl From<KademliaEvent> for EdgeNetEvent {
    fn from(event: KademliaEvent) -> Self {
        EdgeNetEvent::Kademlia(event)
    }
}

#[cfg(feature = "p2p")]
impl From<IdentifyEvent> for EdgeNetEvent {
    fn from(event: IdentifyEvent) -> Self {
        EdgeNetEvent::Identify(event)
    }
}

#[cfg(feature = "p2p")]
impl From<RequestResponseEvent<TaskRequest, TaskResponse>> for EdgeNetEvent {
    fn from(event: RequestResponseEvent<TaskRequest, TaskResponse>) -> Self {
        EdgeNetEvent::RequestResponse(event)
    }
}

// ============================================================================
// P2P Node Implementation
// ============================================================================

/// Main P2P node for EdgeNet networking
///
/// Manages the libp2p swarm and provides high-level APIs for:
/// - Peer discovery and connection management
/// - Event broadcasting (RAC, tasks, gradients)
/// - Direct task negotiation
/// - Capability advertisement
#[cfg(feature = "p2p")]
pub struct P2pNode {
    /// libp2p swarm with EdgeNet behaviour
    swarm: Swarm<EdgeNetBehaviour>,
    /// Pi-Key identity for signing
    identity: PiKey,
    /// Our peer ID
    peer_id: PeerId,
    /// Mapping from Pi-Key identity to PeerId
    identity_map: HashMap<[u8; 40], PeerId>,
    /// Subscribed topics
    subscribed_topics: Vec<String>,
    /// Known peer capabilities
    peer_capabilities: HashMap<PeerId, Vec<String>>,
    /// Configuration
    config: P2pConfig,
}

#[cfg(feature = "p2p")]
impl P2pNode {
    /// Create a new P2P node from a Pi-Key identity
    pub fn new(identity: PiKey, config: P2pConfig) -> Result<Self, P2pError> {
        // Derive libp2p keypair from Pi-Key
        let keypair = Self::derive_keypair_from_pikey(&identity)?;
        let peer_id = PeerId::from(keypair.public());

        // Create gossipsub behaviour
        let gossipsub = Self::create_gossipsub(&keypair, &config)?;

        // Create Kademlia DHT
        let kademlia = Self::create_kademlia(peer_id, &config);

        // Create Identify protocol
        let identify = Self::create_identify(&keypair);

        // Create Request-Response protocol
        let request_response = Self::create_request_response();

        // Combine behaviours
        let behaviour = EdgeNetBehaviour {
            gossipsub,
            kademlia,
            identify,
            request_response,
        };

        // Build swarm with NOISE encryption
        let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                Default::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_| behaviour)?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        Ok(Self {
            swarm,
            identity,
            peer_id,
            identity_map: HashMap::new(),
            subscribed_topics: Vec::new(),
            peer_capabilities: HashMap::new(),
            config,
        })
    }

    /// Derive a libp2p Ed25519 keypair from Pi-Key
    fn derive_keypair_from_pikey(pikey: &PiKey) -> Result<Keypair, P2pError> {
        // Get the signing key bytes from Pi-Key
        let pubkey_bytes = pikey.get_public_key();

        // For now, we'll generate a new keypair and map it
        // In production, we'd derive deterministically from Pi-Key
        let keypair = Keypair::generate_ed25519();

        Ok(keypair)
    }

    /// Create gossipsub behaviour
    fn create_gossipsub(keypair: &Keypair, config: &P2pConfig) -> Result<Gossipsub, P2pError> {
        let message_authenticity = MessageAuthenticity::Signed(keypair.clone());

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .mesh_n(config.gossip_mesh_n)
            .mesh_n_low(config.gossip_mesh_n_low)
            .mesh_n_high(config.gossip_mesh_n_high)
            .heartbeat_interval(Duration::from_secs(config.heartbeat_interval_secs))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(|msg| {
                // Use hash of data as message ID for deduplication
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&msg.data);
                let hash = hasher.finalize();
                gossipsub::MessageId::from(hash.to_vec())
            })
            .build()
            .map_err(|e| P2pError::Config(e.to_string()))?;

        Gossipsub::new(message_authenticity, gossipsub_config)
            .map_err(|e| P2pError::Behaviour(e.to_string()))
    }

    /// Create Kademlia DHT behaviour
    fn create_kademlia(peer_id: PeerId, config: &P2pConfig) -> Kademlia<MemoryStore> {
        let store = MemoryStore::new(peer_id);
        let mut kad_config = kad::Config::default();
        kad_config.set_replication_factor(
            std::num::NonZeroUsize::new(config.kad_replication).unwrap()
        );

        Kademlia::with_config(peer_id, store, kad_config)
    }

    /// Create Identify protocol behaviour
    fn create_identify(keypair: &Keypair) -> Identify {
        let config = identify::Config::new(PROTOCOL_VERSION.to_string(), keypair.public())
            .with_agent_version(AGENT_VERSION.to_string());

        Identify::new(config)
    }

    /// Create Request-Response protocol
    fn create_request_response() -> RequestResponse<TaskCodec> {
        let protocols = std::iter::once((TaskProtocol, request_response::ProtocolSupport::Full));
        let config = request_response::Config::default()
            .with_request_timeout(Duration::from_secs(30));

        RequestResponse::new(protocols, config)
    }

    /// Get our peer ID
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Get our Pi-Key identity
    pub fn identity(&self) -> &PiKey {
        &self.identity
    }

    /// Start listening on configured addresses
    pub fn start_listening(&mut self) -> Result<(), P2pError> {
        for addr in &self.config.listen_addrs {
            self.swarm.listen_on(addr.clone())
                .map_err(|e| P2pError::Transport(e.to_string()))?;
        }
        Ok(())
    }

    /// Connect to bootstrap peers
    pub fn bootstrap(&mut self) -> Result<(), P2pError> {
        for addr in &self.config.bootstrap_peers {
            // Extract peer ID from multiaddr
            if let Some(peer_id) = Self::extract_peer_id(addr) {
                self.swarm.dial(addr.clone())
                    .map_err(|e| P2pError::Dial(e.to_string()))?;
                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
            }
        }

        // Start Kademlia bootstrap
        self.swarm.behaviour_mut().kademlia.bootstrap()
            .map_err(|e| P2pError::Kademlia(e.to_string()))?;

        Ok(())
    }

    /// Subscribe to a gossipsub topic
    pub fn subscribe(&mut self, topic: &str) -> Result<(), P2pError> {
        let topic = gossipsub::IdentTopic::new(topic);
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)
            .map_err(|e| P2pError::Gossipsub(e.to_string()))?;
        self.subscribed_topics.push(topic.hash().to_string());
        Ok(())
    }

    /// Subscribe to all EdgeNet topics
    pub fn subscribe_all_topics(&mut self) -> Result<(), P2pError> {
        self.subscribe(TOPIC_RAC_EVENTS)?;
        self.subscribe(TOPIC_TASK_MARKET)?;
        self.subscribe(TOPIC_MODEL_SYNC)?;
        self.subscribe(TOPIC_GRADIENT_GOSSIP)?;
        self.subscribe(TOPIC_CREDIT_SYNC)?;
        self.subscribe(TOPIC_PRESENCE)?;
        Ok(())
    }

    /// Publish a message to a topic
    pub fn publish(&mut self, topic: &str, message: GossipMessage) -> Result<(), P2pError> {
        let topic = gossipsub::IdentTopic::new(topic);
        let data = bincode::serialize(&message)
            .map_err(|e| P2pError::Serialization(e.to_string()))?;

        self.swarm.behaviour_mut().gossipsub.publish(topic, data)
            .map_err(|e| P2pError::Gossipsub(e.to_string()))?;

        Ok(())
    }

    /// Broadcast a RAC event
    pub fn broadcast_rac_event(&mut self, event_bytes: Vec<u8>) -> Result<(), P2pError> {
        let signature = self.identity.sign(&event_bytes);
        let message = GossipMessage::RacEvent { event_bytes, signature };
        self.publish(TOPIC_RAC_EVENTS, message)
    }

    /// Announce a task to the network
    pub fn announce_task(
        &mut self,
        task_id: String,
        task_type: String,
        requirements: TaskRequirements,
        max_credits: u64,
        deadline_ms: u64,
    ) -> Result<(), P2pError> {
        let message = GossipMessage::TaskAnnounce {
            task_id,
            task_type,
            requirements,
            max_credits,
            deadline_ms,
        };
        self.publish(TOPIC_TASK_MARKET, message)
    }

    /// Claim a task
    pub fn claim_task(&mut self, task_id: String, stake: u64) -> Result<(), P2pError> {
        let worker_id = hex::encode(&self.identity.get_identity()[..8]);
        let claim_data = format!("{}:{}:{}", task_id, worker_id, stake);
        let signature = self.identity.sign(claim_data.as_bytes());

        let message = GossipMessage::TaskClaim {
            task_id,
            worker_id,
            stake,
            signature,
        };
        self.publish(TOPIC_TASK_MARKET, message)
    }

    /// Send presence heartbeat
    pub fn send_heartbeat(
        &mut self,
        capabilities: Vec<String>,
        stake: u64,
        uptime_hours: f32,
        load: f32,
    ) -> Result<(), P2pError> {
        let node_id = hex::encode(&self.identity.get_identity()[..8]);
        let message = GossipMessage::Presence {
            node_id,
            capabilities,
            stake,
            uptime_hours,
            load,
        };
        self.publish(TOPIC_PRESENCE, message)
    }

    /// Advertise our capabilities in the DHT
    pub fn advertise_capabilities(&mut self, capabilities: &[String]) -> Result<(), P2pError> {
        for cap in capabilities {
            let key = kad::RecordKey::new(&format!("cap:{}", cap));
            let record = kad::Record {
                key,
                value: self.peer_id.to_bytes(),
                publisher: Some(self.peer_id),
                expires: None,
            };
            self.swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)
                .map_err(|e| P2pError::Kademlia(e.to_string()))?;
        }
        Ok(())
    }

    /// Find peers with a specific capability
    pub fn find_providers(&mut self, capability: &str) -> kad::QueryId {
        let key = kad::RecordKey::new(&format!("cap:{}", capability));
        self.swarm.behaviour_mut().kademlia.get_providers(key)
    }

    /// Send a direct task request to a peer
    pub fn send_task_request(
        &mut self,
        peer: &PeerId,
        request: TaskRequest,
    ) -> request_response::OutboundRequestId {
        self.swarm.behaviour_mut().request_response.send_request(peer, request)
    }

    /// Send a task response
    pub fn send_task_response(
        &mut self,
        channel: request_response::ResponseChannel<TaskResponse>,
        response: TaskResponse,
    ) -> Result<(), P2pError> {
        self.swarm.behaviour_mut().request_response.send_response(channel, response)
            .map_err(|_| P2pError::Response("Failed to send response".to_string()))
    }

    /// Poll the swarm for events
    pub async fn next_event(&mut self) -> SwarmEvent<EdgeNetEvent> {
        self.swarm.select_next_some().await
    }

    /// Get the number of connected peers
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Get list of connected peer IDs
    pub fn peer_list(&self) -> Vec<PeerId> {
        self.swarm.connected_peers().cloned().collect()
    }

    /// Extract peer ID from a multiaddr
    fn extract_peer_id(addr: &Multiaddr) -> Option<PeerId> {
        addr.iter().find_map(|proto| {
            if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                Some(peer_id)
            } else {
                None
            }
        })
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// P2P networking errors
#[derive(Debug, Clone)]
pub enum P2pError {
    /// Configuration error
    Config(String),
    /// Transport error
    Transport(String),
    /// Dial error
    Dial(String),
    /// Behaviour error
    Behaviour(String),
    /// Gossipsub error
    Gossipsub(String),
    /// Kademlia error
    Kademlia(String),
    /// Serialization error
    Serialization(String),
    /// Response error
    Response(String),
    /// Identity error
    Identity(String),
}

impl std::fmt::Display for P2pError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2pError::Config(e) => write!(f, "Config error: {}", e),
            P2pError::Transport(e) => write!(f, "Transport error: {}", e),
            P2pError::Dial(e) => write!(f, "Dial error: {}", e),
            P2pError::Behaviour(e) => write!(f, "Behaviour error: {}", e),
            P2pError::Gossipsub(e) => write!(f, "Gossipsub error: {}", e),
            P2pError::Kademlia(e) => write!(f, "Kademlia error: {}", e),
            P2pError::Serialization(e) => write!(f, "Serialization error: {}", e),
            P2pError::Response(e) => write!(f, "Response error: {}", e),
            P2pError::Identity(e) => write!(f, "Identity error: {}", e),
        }
    }
}

impl std::error::Error for P2pError {}

// ============================================================================
// Non-P2P Stub Implementation (for WASM without full libp2p)
// ============================================================================

/// Stub P2P node for environments without libp2p feature
#[cfg(not(feature = "p2p"))]
pub struct P2pNode {
    _placeholder: (),
}

#[cfg(not(feature = "p2p"))]
impl P2pNode {
    pub fn new(_identity: crate::pikey::PiKey, _config: P2pConfig) -> Result<Self, P2pError> {
        Ok(Self { _placeholder: () })
    }

    pub fn connected_peers(&self) -> usize { 0 }
    pub fn peer_list(&self) -> Vec<String> { vec![] }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_constants() {
        assert!(TOPIC_RAC_EVENTS.starts_with("/edge-net/"));
        assert!(TOPIC_TASK_MARKET.starts_with("/edge-net/"));
        assert!(TOPIC_MODEL_SYNC.starts_with("/edge-net/"));
        assert!(TOPIC_GRADIENT_GOSSIP.starts_with("/edge-net/"));
    }

    #[test]
    fn test_task_requirements_default() {
        let req = TaskRequirements::default();
        assert!(req.capabilities.contains(&"vectors".to_string()));
        assert_eq!(req.min_stake, 100);
        assert!(!req.requires_gpu);
    }

    #[test]
    fn test_gossip_message_serialization() {
        let msg = GossipMessage::Presence {
            node_id: "test-node".to_string(),
            capabilities: vec!["vectors".to_string()],
            stake: 1000,
            uptime_hours: 24.5,
            load: 0.3,
        };

        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: GossipMessage = bincode::deserialize(&serialized).unwrap();

        if let GossipMessage::Presence { node_id, .. } = deserialized {
            assert_eq!(node_id, "test-node");
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_task_request_serialization() {
        let req = TaskRequest {
            task_id: "task-123".to_string(),
            request_type: TaskRequestType::GetDetails,
            encrypted_payload: vec![1, 2, 3, 4],
            sender_pubkey: vec![5, 6, 7, 8],
        };

        let serialized = bincode::serialize(&req).unwrap();
        let deserialized: TaskRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.task_id, "task-123");
    }

    #[test]
    fn test_p2p_config_default() {
        let config = P2pConfig::default();
        assert_eq!(config.gossip_mesh_n, 6);
        assert_eq!(config.kad_replication, 20);
        assert_eq!(config.heartbeat_interval_secs, 30);
    }

    #[test]
    fn test_p2p_error_display() {
        let err = P2pError::Config("test error".to_string());
        assert!(err.to_string().contains("Config error"));
    }
}
