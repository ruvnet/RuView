//! P2P Swarm v2 - Production Grade Rust Implementation
//!
//! Features:
//! - Ed25519 identity keys + X25519 ephemeral keys for ECDH
//! - AES-256-GCM authenticated encryption
//! - Message replay protection (nonces, counters, timestamps)
//! - GUN-based signaling (no external PeerServer)
//! - IPFS CID pointers for large payloads
//! - Ed25519 signatures on all messages
//! - Relay health monitoring
//! - Task execution envelope with resource budgets
//! - WASM compatible
//! - Quantization (4-32x compression)
//! - Hyperdimensional Computing for pattern matching

mod identity;
mod crypto;
mod relay;
mod artifact;
mod envelope;
#[cfg(feature = "native")]
mod swarm;
mod advanced;

pub use identity::{IdentityManager, KeyPair, RegisteredMember};
pub use crypto::{CryptoV2, EncryptedPayload, CanonicalJson};
pub use relay::RelayManager;
pub use artifact::ArtifactStore;
pub use envelope::{SignedEnvelope, TaskEnvelope, TaskReceipt, ArtifactPointer};
#[cfg(feature = "native")]
pub use swarm::{P2PSwarmV2, SwarmStatus};
pub use advanced::{
    // Quantization
    ScalarQuantized, BinaryQuantized, CompressedData,
    // Hyperdimensional Computing
    Hypervector, HdcMemory, HDC_DIMENSION,
    // Adaptive compression
    AdaptiveCompressor, NetworkCondition,
    // Pattern routing
    PatternRouter,
    // HNSW vector index
    HnswIndex,
    // Post-quantum crypto
    HybridKeyPair, HybridPublicKey, HybridSignature,
    // Spiking neural networks
    LIFNeuron, SpikingNetwork,
    // Semantic embeddings
    SemanticEmbedder, SemanticTaskMatcher,
    // Raft consensus
    RaftNode, RaftState, LogEntry,
    RaftVoteRequest, RaftVoteResponse,
    RaftAppendEntries, RaftAppendEntriesResponse,
};
