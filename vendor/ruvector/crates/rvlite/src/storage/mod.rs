//! IndexedDB storage backend for WASM persistence
//!
//! Provides async-compatible persistence using IndexedDB for:
//! - Vector database state
//! - Cypher graph state
//! - SPARQL triple store state

pub mod indexeddb;
pub mod state;

#[cfg(feature = "rvf-backend")]
pub mod epoch;

#[cfg(feature = "rvf-backend")]
pub mod writer_lease;

#[cfg(feature = "rvf-backend")]
pub mod id_map;

pub use indexeddb::IndexedDBStorage;
pub use state::{GraphState, RvLiteState, TripleStoreState, VectorState};
