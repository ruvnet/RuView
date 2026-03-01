//! Index structures for efficient vector search

pub mod flat;
#[cfg(feature = "hnsw")]
pub mod hnsw;

use crate::error::Result;
use crate::types::{SearchResult, VectorId};

/// Trait for vector index implementations
pub trait VectorIndex: Send + Sync {
    /// Add a vector to the index
    fn add(&mut self, id: VectorId, vector: Vec<f32>) -> Result<()>;

    /// Add multiple vectors in batch
    fn add_batch(&mut self, entries: Vec<(VectorId, Vec<f32>)>) -> Result<()> {
        for (id, vector) in entries {
            self.add(id, vector)?;
        }
        Ok(())
    }

    /// Search for k nearest neighbors
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>>;

    /// Remove a vector from the index
    fn remove(&mut self, id: &VectorId) -> Result<bool>;

    /// Get the number of vectors in the index
    fn len(&self) -> usize;

    /// Check if the index is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
