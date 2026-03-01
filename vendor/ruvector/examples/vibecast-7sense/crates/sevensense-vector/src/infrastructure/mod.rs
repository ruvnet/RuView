//! Infrastructure layer for the Vector Space bounded context.
//!
//! Contains:
//! - HNSW index implementation
//! - Graph storage adapters
//! - Persistence implementations

pub mod hnsw_index;
pub mod graph_store;

pub use hnsw_index::HnswIndex;
pub use graph_store::InMemoryGraphStore;
