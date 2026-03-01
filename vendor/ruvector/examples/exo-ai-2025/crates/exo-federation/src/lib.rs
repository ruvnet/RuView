//! # exo-federation: Distributed Cognitive Mesh
//!
//! This crate implements federated substrate networking with:
//! - Post-quantum cryptographic handshakes
//! - Privacy-preserving onion routing
//! - CRDT-based eventual consistency
//! - Byzantine fault-tolerant consensus
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │      FederatedMesh (Coordinator)        │
//! ├─────────────────────────────────────────┤
//! │ • Local substrate instance              │
//! │ • Consensus coordination                │
//! │ • Federation gateway                    │
//! │ • Cryptographic identity                │
//! └─────────────────────────────────────────┘
//!          │           │           │
//!    ┌─────┘           │           └─────┐
//!    ▼                 ▼                 ▼
//! Handshake         Onion            CRDT
//! Protocol          Router      Reconciliation
//! ```

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod consensus;
pub mod crdt;
pub mod crypto;
pub mod handshake;
pub mod onion;
pub mod transfer_crdt;

pub use consensus::{byzantine_commit, CommitProof};
pub use crdt::{reconcile_crdt, GSet, LWWRegister};
pub use crypto::{EncryptedChannel, PostQuantumKeypair};
pub use handshake::{join_federation, Capability, FederationToken};
pub use onion::{onion_query, OnionHeader};

/// Errors that can occur in federation operations
#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    #[error("Cryptographic operation failed: {0}")]
    CryptoError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Consensus failed: {0}")]
    ConsensusError(String),

    #[error("Invalid federation token")]
    InvalidToken,

    #[error("Insufficient peers for consensus: needed {needed}, got {actual}")]
    InsufficientPeers { needed: usize, actual: usize },

    #[error("CRDT reconciliation failed: {0}")]
    ReconciliationError(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),
}

pub type Result<T> = std::result::Result<T, FederationError>;

/// Unique identifier for a peer in the federation
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PeerId(pub String);

impl PeerId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn generate() -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(rand::random::<[u8; 32]>());
        let hash = hasher.finalize();
        Self(hex::encode(&hash[..16]))
    }
}

/// Network address for a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAddress {
    pub host: String,
    pub port: u16,
    pub public_key: Vec<u8>,
}

impl PeerAddress {
    pub fn new(host: String, port: u16, public_key: Vec<u8>) -> Self {
        Self {
            host,
            port,
            public_key,
        }
    }
}

/// Scope for federated queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FederationScope {
    /// Query only local instance
    Local,
    /// Query direct peers only
    Direct,
    /// Query entire federation (multi-hop)
    Global { max_hops: usize },
}

/// Result from a federated query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedResult {
    pub source: PeerId,
    pub data: Vec<u8>,
    pub score: f32,
    pub timestamp: u64,
}

/// State update for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    pub update_id: String,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// Substrate instance placeholder (will reference exo-core types)
pub struct SubstrateInstance {
    // Placeholder - will integrate with actual substrate
}

/// Federated cognitive mesh coordinator
pub struct FederatedMesh {
    /// Unique identifier for this node
    pub local_id: PeerId,

    /// Local substrate instance
    pub local: Arc<RwLock<SubstrateInstance>>,

    /// Post-quantum cryptographic keypair
    pub pq_keys: PostQuantumKeypair,

    /// Connected peers
    pub peers: Arc<DashMap<PeerId, PeerAddress>>,

    /// Active federation tokens
    pub tokens: Arc<DashMap<PeerId, FederationToken>>,

    /// Encrypted channels to peers
    pub channels: Arc<DashMap<PeerId, EncryptedChannel>>,
}

impl FederatedMesh {
    /// Create a new federated mesh node
    pub fn new(local: SubstrateInstance) -> Result<Self> {
        let local_id = PeerId::generate();
        let pq_keys = PostQuantumKeypair::generate();

        Ok(Self {
            local_id,
            local: Arc::new(RwLock::new(local)),
            pq_keys,
            peers: Arc::new(DashMap::new()),
            tokens: Arc::new(DashMap::new()),
            channels: Arc::new(DashMap::new()),
        })
    }

    /// Join a federation by connecting to a peer
    pub async fn join_federation(&mut self, peer: &PeerAddress) -> Result<FederationToken> {
        let token = join_federation(&self.pq_keys, peer).await?;

        // Store the peer and token
        let peer_id = PeerId::new(token.peer_id.clone());
        self.peers.insert(peer_id.clone(), peer.clone());
        self.tokens.insert(peer_id, token.clone());

        Ok(token)
    }

    /// Execute a federated query across the mesh
    pub async fn federated_query(
        &self,
        query: Vec<u8>,
        scope: FederationScope,
    ) -> Result<Vec<FederatedResult>> {
        match scope {
            FederationScope::Local => {
                // Query only local instance
                Ok(vec![FederatedResult {
                    source: self.local_id.clone(),
                    data: query, // Placeholder
                    score: 1.0,
                    timestamp: current_timestamp(),
                }])
            }
            FederationScope::Direct => {
                // Query direct peers
                let mut results = Vec::new();

                for entry in self.peers.iter() {
                    let peer_id = entry.key().clone();
                    // Placeholder: would actually send query to peer
                    results.push(FederatedResult {
                        source: peer_id,
                        data: query.clone(),
                        score: 0.8,
                        timestamp: current_timestamp(),
                    });
                }

                Ok(results)
            }
            FederationScope::Global { max_hops } => {
                // Use onion routing for privacy
                let _relay_nodes: Vec<_> = self
                    .peers
                    .iter()
                    .take(max_hops)
                    .map(|e| e.key().clone())
                    .collect();

                // Placeholder: would use onion_query
                Ok(vec![])
            }
        }
    }

    /// Commit a state update with Byzantine consensus
    pub async fn byzantine_commit(&self, update: StateUpdate) -> Result<CommitProof> {
        let peer_count = self.peers.len() + 1; // +1 for local
        byzantine_commit(update, peer_count).await
    }

    /// Get the count of peers in the federation
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// Re-export hex for PeerId
use hex;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_federated_mesh_creation() {
        let substrate = SubstrateInstance {};
        let mesh = FederatedMesh::new(substrate).unwrap();
        assert_eq!(mesh.peer_count(), 0);
    }

    #[tokio::test]
    async fn test_local_query() {
        let substrate = SubstrateInstance {};
        let mesh = FederatedMesh::new(substrate).unwrap();

        let results = mesh
            .federated_query(vec![1, 2, 3], FederationScope::Local)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
    }
}
