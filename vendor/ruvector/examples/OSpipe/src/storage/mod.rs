//! Vector storage, embedding engine, and trait abstractions.
//!
//! Provides HNSW-backed vector storage for captured frames with
//! cosine similarity search, metadata filtering, delete/update operations,
//! and a pluggable embedding model trait.

pub mod embedding;
pub mod traits;
pub mod vector_store;

pub use embedding::EmbeddingEngine;
pub use traits::{EmbeddingModel, HashEmbeddingModel};
pub use vector_store::{SearchFilter, SearchResult, StoredEmbedding, VectorStore};

#[cfg(not(target_arch = "wasm32"))]
pub use traits::RuvectorEmbeddingModel;
#[cfg(not(target_arch = "wasm32"))]
pub use vector_store::HnswVectorStore;
