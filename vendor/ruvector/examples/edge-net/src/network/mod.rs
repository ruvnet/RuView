//! P2P networking layer using GUN.js and WebRTC
//!
//! This module provides:
//! - **NetworkManager**: Basic P2P peer management
//! - **SemanticRouter**: RuVector-based intelligent routing with HNSW indexing

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

pub mod semantic;
pub use semantic::{SemanticRouter, PeerInfo, HnswIndex, PeerId, TopicHash};

/// Network message types
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NetworkMessage {
    /// Announce presence on network
    Announce {
        node_id: String,
        pubkey: Vec<u8>,
        capabilities: Vec<String>,
        stake: u64,
    },
    /// Task submission
    TaskSubmit {
        task_id: String,
        task_type: String,
        encrypted_payload: Vec<u8>,
        max_credits: u64,
        redundancy: u8,
    },
    /// Task claim
    TaskClaim {
        task_id: String,
        worker_id: String,
        stake: u64,
    },
    /// Task result
    TaskResult {
        task_id: String,
        encrypted_result: Vec<u8>,
        proof: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Credit sync (CRDT state)
    CreditSync {
        ledger_state: Vec<u8>,
        merkle_root: [u8; 32],
    },
    /// QDAG transaction
    QDAGTransaction {
        tx_bytes: Vec<u8>,
    },
    /// Heartbeat/ping
    Heartbeat {
        node_id: String,
        timestamp: u64,
        uptime: u64,
    },
}

/// Network peer information
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Peer {
    pub node_id: String,
    pub pubkey: Vec<u8>,
    pub capabilities: Vec<String>,
    pub stake: u64,
    pub reputation: f32,
    pub last_seen: u64,
    pub latency_ms: u32,
}

/// P2P network manager
#[wasm_bindgen]
pub struct WasmNetworkManager {
    node_id: String,
    peers: std::collections::HashMap<String, Peer>,
    relay_urls: Vec<String>,
    connected: bool,
}

#[wasm_bindgen]
impl WasmNetworkManager {
    #[wasm_bindgen(constructor)]
    pub fn new(node_id: &str) -> WasmNetworkManager {
        WasmNetworkManager {
            node_id: node_id.to_string(),
            peers: std::collections::HashMap::new(),
            relay_urls: vec![
                "https://gun-manhattan.herokuapp.com/gun".to_string(),
                "https://gun-us.herokuapp.com/gun".to_string(),
            ],
            connected: false,
        }
    }

    /// Add a relay URL
    #[wasm_bindgen(js_name = addRelay)]
    pub fn add_relay(&mut self, url: &str) {
        self.relay_urls.push(url.to_string());
    }

    /// Check if connected
    #[wasm_bindgen(js_name = isConnected)]
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get peer count
    #[wasm_bindgen(js_name = peerCount)]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Get active peer count (seen in last 60s)
    #[wasm_bindgen(js_name = activePeerCount)]
    pub fn active_peer_count(&self) -> usize {
        let now = js_sys::Date::now() as u64;
        self.peers.values()
            .filter(|p| now - p.last_seen < 60_000)
            .count()
    }

    /// Register a peer
    #[wasm_bindgen(js_name = registerPeer)]
    pub fn register_peer(
        &mut self,
        node_id: &str,
        pubkey: &[u8],
        capabilities: Vec<String>,
        stake: u64,
    ) {
        let peer = Peer {
            node_id: node_id.to_string(),
            pubkey: pubkey.to_vec(),
            capabilities,
            stake,
            reputation: 0.5, // Start neutral
            last_seen: js_sys::Date::now() as u64,
            latency_ms: 0,
        };

        self.peers.insert(node_id.to_string(), peer);
    }

    /// Update peer reputation
    #[wasm_bindgen(js_name = updateReputation)]
    pub fn update_reputation(&mut self, node_id: &str, delta: f32) {
        if let Some(peer) = self.peers.get_mut(node_id) {
            peer.reputation = (peer.reputation + delta).clamp(0.0, 1.0);
        }
    }

    /// Get peers with specific capability
    #[wasm_bindgen(js_name = getPeersWithCapability)]
    pub fn get_peers_with_capability(&self, capability: &str) -> Vec<String> {
        self.peers.values()
            .filter(|p| p.capabilities.contains(&capability.to_string()))
            .filter(|p| p.stake > 0) // Must be staked
            .filter(|p| p.reputation > 0.3) // Must have reasonable reputation
            .map(|p| p.node_id.clone())
            .collect()
    }

    /// Select workers for task execution (reputation-weighted random)
    #[wasm_bindgen(js_name = selectWorkers)]
    pub fn select_workers(&self, capability: &str, count: usize) -> Vec<String> {
        let mut candidates: Vec<_> = self.peers.values()
            .filter(|p| p.capabilities.contains(&capability.to_string()))
            .filter(|p| p.stake > 0)
            .filter(|p| p.reputation > 0.3)
            .collect();

        // Sort by reputation (highest first)
        candidates.sort_by(|a, b| b.reputation.partial_cmp(&a.reputation).unwrap());

        // Take top N
        candidates.into_iter()
            .take(count)
            .map(|p| p.node_id.clone())
            .collect()
    }
}
