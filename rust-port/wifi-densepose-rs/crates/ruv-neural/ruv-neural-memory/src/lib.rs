//! rUv Neural Memory — Persistent neural state memory with vector search
//! and longitudinal tracking.
//!
//! This crate provides in-memory and persistent storage for neural embeddings,
//! supporting brute-force and HNSW-based nearest neighbor search, session-based
//! memory management, and longitudinal drift detection.

pub mod hnsw;
pub mod longitudinal;
pub mod persistence;
pub mod session;
pub mod store;

pub use hnsw::HnswIndex;
pub use longitudinal::{LongitudinalTracker, TrendDirection};
pub use persistence::{load_rvf, load_store, save_rvf, save_store};
pub use session::{SessionMemory, SessionMetadata};
pub use store::NeuralMemoryStore;
