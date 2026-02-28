//! Repository traits for the embedding bounded context.
//!
//! Defines the interfaces for persisting and retrieving embeddings,
//! following the repository pattern from Domain-Driven Design.

use async_trait::async_trait;

use super::entities::{Embedding, EmbeddingId, EmbeddingModel, SegmentId, StorageTier};
use crate::EmbeddingError;

/// Repository trait for embedding persistence.
///
/// Implementations may use various storage backends:
/// - In-memory (for testing)
/// - Vector databases (Qdrant, Milvus)
/// - Relational databases (PostgreSQL with pgvector)
/// - File-based storage
#[async_trait]
pub trait EmbeddingRepository: Send + Sync {
    /// Save a single embedding to the repository.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedding cannot be persisted.
    async fn save(&self, embedding: &Embedding) -> Result<(), EmbeddingError>;

    /// Find an embedding by its unique identifier.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(embedding))` if found
    /// - `Ok(None)` if not found
    /// - `Err(...)` on storage errors
    async fn find_by_id(&self, id: &EmbeddingId) -> Result<Option<Embedding>, EmbeddingError>;

    /// Find an embedding by its source segment ID.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(embedding))` if found
    /// - `Ok(None)` if not found
    /// - `Err(...)` on storage errors
    async fn find_by_segment(&self, segment_id: &SegmentId) -> Result<Option<Embedding>, EmbeddingError>;

    /// Save multiple embeddings in a batch operation.
    ///
    /// This is more efficient than calling `save` multiple times
    /// as it can use bulk insert operations.
    ///
    /// # Errors
    ///
    /// Returns an error if any embedding cannot be persisted.
    /// Implementations should document their atomicity guarantees.
    async fn batch_save(&self, embeddings: &[Embedding]) -> Result<(), EmbeddingError>;

    /// Delete an embedding by its ID.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if the embedding was deleted
    /// - `Ok(false)` if the embedding was not found
    /// - `Err(...)` on storage errors
    async fn delete(&self, id: &EmbeddingId) -> Result<bool, EmbeddingError>;

    /// Delete embeddings by segment ID.
    ///
    /// Useful when a segment is reprocessed with a new model version.
    ///
    /// # Returns
    ///
    /// Number of embeddings deleted.
    async fn delete_by_segment(&self, segment_id: &SegmentId) -> Result<usize, EmbeddingError>;

    /// Count total embeddings in the repository.
    async fn count(&self) -> Result<u64, EmbeddingError>;

    /// Count embeddings by storage tier.
    async fn count_by_tier(&self, tier: StorageTier) -> Result<u64, EmbeddingError>;

    /// Find embeddings by model version.
    ///
    /// Useful for identifying embeddings that need re-generation
    /// after a model update.
    async fn find_by_model_version(
        &self,
        model_version: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Embedding>, EmbeddingError>;

    /// Update the storage tier for an embedding.
    ///
    /// Used for tiered storage management (hot -> warm -> cold).
    async fn update_tier(
        &self,
        id: &EmbeddingId,
        tier: StorageTier,
    ) -> Result<bool, EmbeddingError>;

    /// Check if an embedding exists.
    async fn exists(&self, id: &EmbeddingId) -> Result<bool, EmbeddingError> {
        Ok(self.find_by_id(id).await?.is_some())
    }

    /// Check if an embedding exists for a segment.
    async fn exists_for_segment(&self, segment_id: &SegmentId) -> Result<bool, EmbeddingError> {
        Ok(self.find_by_segment(segment_id).await?.is_some())
    }
}

/// Repository trait for embedding model management.
///
/// Manages the lifecycle of ONNX models used for embedding generation.
#[async_trait]
pub trait ModelRepository: Send + Sync {
    /// Save or update a model configuration.
    async fn save_model(&self, model: &EmbeddingModel) -> Result<(), EmbeddingError>;

    /// Find a model by name and version.
    async fn find_model(
        &self,
        name: &str,
        version: &str,
    ) -> Result<Option<EmbeddingModel>, EmbeddingError>;

    /// Get the currently active model for a given name.
    async fn get_active_model(&self, name: &str) -> Result<Option<EmbeddingModel>, EmbeddingError>;

    /// List all available models.
    async fn list_models(&self) -> Result<Vec<EmbeddingModel>, EmbeddingError>;

    /// Delete a model configuration.
    async fn delete_model(&self, name: &str, version: &str) -> Result<bool, EmbeddingError>;
}

/// Query parameters for embedding search operations.
#[derive(Debug, Clone, Default)]
pub struct EmbeddingQuery {
    /// Filter by segment IDs
    pub segment_ids: Option<Vec<SegmentId>>,

    /// Filter by model version
    pub model_version: Option<String>,

    /// Filter by storage tier
    pub tier: Option<StorageTier>,

    /// Filter by creation date (after)
    pub created_after: Option<chrono::DateTime<chrono::Utc>>,

    /// Filter by creation date (before)
    pub created_before: Option<chrono::DateTime<chrono::Utc>>,

    /// Maximum results to return
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,
}

impl EmbeddingQuery {
    /// Create a new query builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by segment IDs
    #[must_use]
    pub fn with_segment_ids(mut self, ids: Vec<SegmentId>) -> Self {
        self.segment_ids = Some(ids);
        self
    }

    /// Filter by model version
    #[must_use]
    pub fn with_model_version(mut self, version: impl Into<String>) -> Self {
        self.model_version = Some(version.into());
        self
    }

    /// Filter by storage tier
    #[must_use]
    pub const fn with_tier(mut self, tier: StorageTier) -> Self {
        self.tier = Some(tier);
        self
    }

    /// Set pagination limit
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set pagination offset
    #[must_use]
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Extended repository trait with query support.
#[async_trait]
pub trait QueryableEmbeddingRepository: EmbeddingRepository {
    /// Query embeddings with filters.
    async fn query(&self, query: &EmbeddingQuery) -> Result<Vec<Embedding>, EmbeddingError>;

    /// Find k nearest neighbors to a query vector.
    ///
    /// # Arguments
    ///
    /// * `query_vector` - The query embedding vector
    /// * `k` - Number of neighbors to return
    /// * `ef_search` - HNSW search parameter (larger = more accurate, slower)
    ///
    /// # Returns
    ///
    /// Vector of (embedding, distance) pairs, sorted by distance ascending.
    async fn find_nearest(
        &self,
        query_vector: &[f32],
        k: usize,
        ef_search: Option<usize>,
    ) -> Result<Vec<(Embedding, f32)>, EmbeddingError>;

    /// Find embeddings within a distance threshold.
    async fn find_within_distance(
        &self,
        query_vector: &[f32],
        max_distance: f32,
    ) -> Result<Vec<(Embedding, f32)>, EmbeddingError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// In-memory implementation for testing
    struct InMemoryEmbeddingRepository {
        embeddings: Arc<RwLock<HashMap<EmbeddingId, Embedding>>>,
    }

    impl InMemoryEmbeddingRepository {
        fn new() -> Self {
            Self {
                embeddings: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl EmbeddingRepository for InMemoryEmbeddingRepository {
        async fn save(&self, embedding: &Embedding) -> Result<(), EmbeddingError> {
            self.embeddings.write().await.insert(embedding.id, embedding.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: &EmbeddingId) -> Result<Option<Embedding>, EmbeddingError> {
            Ok(self.embeddings.read().await.get(id).cloned())
        }

        async fn find_by_segment(&self, segment_id: &SegmentId) -> Result<Option<Embedding>, EmbeddingError> {
            Ok(self.embeddings.read().await.values()
                .find(|e| e.segment_id == *segment_id)
                .cloned())
        }

        async fn batch_save(&self, embeddings: &[Embedding]) -> Result<(), EmbeddingError> {
            let mut store = self.embeddings.write().await;
            for embedding in embeddings {
                store.insert(embedding.id, embedding.clone());
            }
            Ok(())
        }

        async fn delete(&self, id: &EmbeddingId) -> Result<bool, EmbeddingError> {
            Ok(self.embeddings.write().await.remove(id).is_some())
        }

        async fn delete_by_segment(&self, segment_id: &SegmentId) -> Result<usize, EmbeddingError> {
            let mut store = self.embeddings.write().await;
            let to_remove: Vec<_> = store.iter()
                .filter(|(_, e)| e.segment_id == *segment_id)
                .map(|(id, _)| *id)
                .collect();
            let count = to_remove.len();
            for id in to_remove {
                store.remove(&id);
            }
            Ok(count)
        }

        async fn count(&self) -> Result<u64, EmbeddingError> {
            Ok(self.embeddings.read().await.len() as u64)
        }

        async fn count_by_tier(&self, tier: StorageTier) -> Result<u64, EmbeddingError> {
            Ok(self.embeddings.read().await.values()
                .filter(|e| e.tier == tier)
                .count() as u64)
        }

        async fn find_by_model_version(
            &self,
            model_version: &str,
            limit: usize,
            offset: usize,
        ) -> Result<Vec<Embedding>, EmbeddingError> {
            Ok(self.embeddings.read().await.values()
                .filter(|e| e.model_version == model_version)
                .skip(offset)
                .take(limit)
                .cloned()
                .collect())
        }

        async fn update_tier(
            &self,
            id: &EmbeddingId,
            tier: StorageTier,
        ) -> Result<bool, EmbeddingError> {
            let mut store = self.embeddings.write().await;
            if let Some(embedding) = store.get_mut(id) {
                embedding.tier = tier;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    #[tokio::test]
    async fn test_in_memory_repository() {
        let repo = InMemoryEmbeddingRepository::new();
        let segment_id = SegmentId::new();
        let vector = vec![0.0; crate::EMBEDDING_DIM];
        let embedding = Embedding::new(segment_id, vector, "test".to_string()).unwrap();

        // Save
        repo.save(&embedding).await.unwrap();

        // Find by ID
        let found = repo.find_by_id(&embedding.id).await.unwrap();
        assert!(found.is_some());

        // Find by segment
        let found = repo.find_by_segment(&segment_id).await.unwrap();
        assert!(found.is_some());

        // Count
        assert_eq!(repo.count().await.unwrap(), 1);

        // Delete
        assert!(repo.delete(&embedding.id).await.unwrap());
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[test]
    fn test_embedding_query_builder() {
        let query = EmbeddingQuery::new()
            .with_model_version("perch-v2-2.0.0-base")
            .with_tier(StorageTier::Hot)
            .with_limit(100)
            .with_offset(0);

        assert_eq!(query.model_version.as_deref(), Some("perch-v2-2.0.0-base"));
        assert_eq!(query.tier, Some(StorageTier::Hot));
        assert_eq!(query.limit, Some(100));
        assert_eq!(query.offset, Some(0));
    }
}
