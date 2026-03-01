//! RVF adapter for agentic-flow swarm coordination.
//!
//! This crate bridges agentic-flow's swarm coordination primitives with the
//! RuVector Format (RVF) segment store, per ADR-029. It provides persistent
//! storage for inter-agent memory sharing, swarm coordination state, and
//! agent learning patterns.
//!
//! # Segment mapping
//!
//! - **VEC_SEG + META_SEG**: Shared memory entries (embeddings + key/value
//!   metadata) for inter-agent memory sharing via the RVF streaming protocol.
//! - **META_SEG**: Swarm coordination state (agent states, topology changes).
//! - **SKETCH_SEG**: Agent learning patterns with effectiveness scores.
//! - **WITNESS_SEG**: Distributed consensus votes with signatures for
//!   tamper-evident audit trails.
//!
//! # Usage
//!
//! ```rust,no_run
//! use rvf_adapter_agentic_flow::{AgenticFlowConfig, RvfSwarmStore};
//!
//! let config = AgenticFlowConfig::new("/tmp/swarm-data", "agent-001");
//! let mut store = RvfSwarmStore::create(config).unwrap();
//!
//! // Share a memory entry with other agents
//! let embedding = vec![0.1f32; 384];
//! store.share_memory("auth-pattern", "JWT with refresh tokens",
//!     "patterns", &embedding).unwrap();
//!
//! // Search shared memories by embedding similarity
//! let results = store.search_shared(&embedding, 5);
//!
//! // Record coordination state
//! store.coordination().record_state("agent-001", "status", "active").unwrap();
//!
//! // Store a learning pattern
//! store.learning().store_pattern("convergent", "Use batched writes",
//!     0.92).unwrap();
//!
//! store.close().unwrap();
//! ```

pub mod config;
pub mod coordination;
pub mod learning;
pub mod swarm_store;

pub use config::{AgenticFlowConfig, ConfigError};
pub use coordination::{ConsensusVote, StateEntry, SwarmCoordination};
pub use learning::{LearningPatternStore, PatternResult};
pub use swarm_store::{
    RvfSwarmStore, SharedMemoryEntry, SharedMemoryResult, SwarmStoreError,
};
