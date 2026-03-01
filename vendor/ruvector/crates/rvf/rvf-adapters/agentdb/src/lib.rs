//! AgentDB adapter for the RuVector Format (RVF).
//!
//! Maps agentdb's vector storage, HNSW index, and memory pattern APIs
//! onto the RVF segment model:
//!
//! - **VEC_SEG**: Raw vector data (episodes, state embeddings)
//! - **INDEX_SEG**: HNSW index layers (A/B/C progressive indexing)
//! - **META_SEG**: Memory pattern metadata (rewards, critiques, tags)
//!
//! Uses the RVText domain profile for text/embedding workloads.

pub mod index_adapter;
pub mod pattern_store;
pub mod vector_store;

pub use index_adapter::RvfIndexAdapter;
pub use pattern_store::{MemoryPattern, RvfPatternStore};
pub use vector_store::RvfVectorStore;
