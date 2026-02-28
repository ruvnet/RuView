//! RVF adapter for the claude-flow memory subsystem.
//!
//! This crate bridges claude-flow's key/value/embedding memory model
//! with the RuVector Format (RVF) segment store. Memory entries are
//! persisted as RVF files with the RVText profile, and every mutation
//! is recorded in a WITNESS_SEG audit trail for tamper-evident logging.
//!
//! # Architecture
//!
//! - **`RvfMemoryStore`**: Main API wrapping `RvfStore` for
//!   store/search/retrieve/delete operations on memory entries.
//! - **`WitnessChain`**: Persistent, append-only audit log using
//!   `rvf_crypto::witness` chains (SHAKE-256 linked).
//! - **`ClaudeFlowConfig`**: Configuration for data directory, embedding
//!   dimension, distance metric, and witness toggle.
//!
//! # Usage
//!
//! ```rust,no_run
//! use rvf_adapter_claude_flow::{ClaudeFlowConfig, RvfMemoryStore};
//!
//! let config = ClaudeFlowConfig::new("/tmp/claude-flow-memory", 384);
//! let mut store = RvfMemoryStore::create(config).unwrap();
//!
//! // Store a memory entry with its embedding
//! let embedding = vec![0.1f32; 384];
//! store.store_memory("auth-pattern", "JWT with refresh tokens",
//!     "patterns", &["auth".into()], &embedding).unwrap();
//!
//! // Search by embedding similarity
//! let results = store.search_memory(&embedding, 5, Some("patterns"), None).unwrap();
//!
//! // Retrieve by key
//! let id = store.retrieve_memory("auth-pattern", "patterns");
//!
//! // Delete
//! store.delete_memory("auth-pattern", "patterns").unwrap();
//!
//! store.close().unwrap();
//! ```

pub mod config;
pub mod memory_store;
pub mod witness;

pub use config::ClaudeFlowConfig;
pub use memory_store::{MemoryEntry, MemoryStoreError, RvfMemoryStore};
pub use witness::{WitnessChain, WitnessError};
