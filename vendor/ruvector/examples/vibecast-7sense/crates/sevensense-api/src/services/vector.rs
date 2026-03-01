//! Vector index service.
//!
//! This module provides the `VectorIndex` service for similarity search
//! using vector embeddings.

use std::collections::HashMap;
use std::sync::RwLock;

use thiserror::Error;
use uuid::Uuid;

use super::{SearchResult, SegmentEmbedding, SpeciesInfo};

/// Vector index error.
#[derive(Debug, Error)]
pub enum VectorError {
    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Query error
    #[error("Query error: {0}")]
    QueryError(String),

    /// Index error
    #[error("Index error: {0}")]
    IndexError(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Vector index configuration.
#[derive(Debug, Clone)]
pub struct VectorIndexConfig {
    /// Collection name
    pub collection_name: String,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// HNSW M parameter
    pub hnsw_m: usize,
    /// HNSW ef_construct parameter
    pub hnsw_ef_construct: usize,
}

impl Default for VectorIndexConfig {
    fn default() -> Self {
        Self {
            collection_name: "sevensense_segments".to_string(),
            embedding_dim: 1024,
            hnsw_m: 16,
            hnsw_ef_construct: 100,
        }
    }
}

/// In-memory segment storage for the stub implementation.
struct StoredSegment {
    recording_id: Uuid,
    embedding: Vec<f32>,
    start_time: f64,
    end_time: f64,
    species: Option<SpeciesInfo>,
}

/// Vector index for similarity search.
///
/// Wraps vector database (Qdrant) for efficient nearest neighbor search.
pub struct VectorIndex {
    config: VectorIndexConfig,
    // In-memory storage for stub implementation
    storage: RwLock<HashMap<Uuid, StoredSegment>>,
}

impl VectorIndex {
    /// Create a new vector index with the given configuration.
    pub fn new(config: VectorIndexConfig) -> Result<Self, VectorError> {
        // In a real implementation, this would:
        // 1. Connect to Qdrant
        // 2. Create/verify collection
        // 3. Configure HNSW index

        Ok(Self {
            config,
            storage: RwLock::new(HashMap::new()),
        })
    }

    /// Add a batch of embeddings to the index.
    pub fn add_batch(&self, embeddings: &[SegmentEmbedding]) -> Result<(), VectorError> {
        let mut storage = self
            .storage
            .write()
            .map_err(|e| VectorError::IndexError(e.to_string()))?;

        for emb in embeddings {
            storage.insert(
                emb.id,
                StoredSegment {
                    recording_id: emb.recording_id,
                    embedding: emb.embedding.clone(),
                    start_time: emb.start_time,
                    end_time: emb.end_time,
                    species: emb.species.clone(),
                },
            );
        }

        Ok(())
    }

    /// Get embedding for a segment.
    pub fn get_embedding(&self, segment_id: &Uuid) -> Result<Option<Vec<f32>>, VectorError> {
        let storage = self
            .storage
            .read()
            .map_err(|e| VectorError::QueryError(e.to_string()))?;

        Ok(storage.get(segment_id).map(|s| s.embedding.clone()))
    }

    /// Search for similar segments.
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        min_similarity: f32,
    ) -> Result<Vec<SearchResult>, VectorError> {
        let storage = self
            .storage
            .read()
            .map_err(|e| VectorError::QueryError(e.to_string()))?;

        // Compute distances to all stored embeddings
        let mut results: Vec<(Uuid, f32, &StoredSegment)> = storage
            .iter()
            .map(|(id, seg)| {
                let distance = cosine_distance(query, &seg.embedding);
                (*id, distance, seg)
            })
            .filter(|(_, dist, _)| (1.0 - *dist) >= min_similarity)
            .collect();

        // Sort by distance (ascending)
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        let results: Vec<SearchResult> = results
            .into_iter()
            .take(k)
            .map(|(id, distance, seg)| SearchResult {
                id,
                recording_id: seg.recording_id,
                distance,
                start_time: seg.start_time,
                end_time: seg.end_time,
                species: seg.species.clone(),
            })
            .collect();

        Ok(results)
    }

    /// Delete embeddings for a recording.
    pub fn delete_recording(&self, recording_id: &Uuid) -> Result<usize, VectorError> {
        let mut storage = self
            .storage
            .write()
            .map_err(|e| VectorError::IndexError(e.to_string()))?;

        let to_remove: Vec<Uuid> = storage
            .iter()
            .filter(|(_, seg)| seg.recording_id == *recording_id)
            .map(|(id, _)| *id)
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            storage.remove(&id);
        }

        Ok(count)
    }

    /// Get total number of indexed segments.
    pub fn count(&self) -> Result<usize, VectorError> {
        let storage = self
            .storage
            .read()
            .map_err(|e| VectorError::QueryError(e.to_string()))?;

        Ok(storage.len())
    }
}

/// Compute cosine distance between two vectors.
fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }

    1.0 - (dot / (norm_a * norm_b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_index_creation() {
        let index = VectorIndex::new(Default::default());
        assert!(index.is_ok());
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let dist = cosine_distance(&a, &b);
        assert!((dist - 0.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        let dist = cosine_distance(&a, &c);
        assert!((dist - 1.0).abs() < 0.001);
    }
}
