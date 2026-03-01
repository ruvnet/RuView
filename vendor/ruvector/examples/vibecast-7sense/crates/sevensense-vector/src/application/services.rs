//! Application services for the Vector Space bounded context.
//!
//! These services implement the use cases for vector indexing and search,
//! providing a high-level API that coordinates domain objects and repositories.

use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info, instrument, warn};

use crate::distance::{cosine_similarity, normalize_vector};
use crate::domain::{
    EmbeddingId, HnswConfig, SimilarityEdge, EdgeType,
    VectorError,
};
use crate::infrastructure::hnsw_index::HnswIndex;

/// A search result neighbor with similarity information.
#[derive(Debug, Clone)]
pub struct Neighbor {
    /// The embedding ID of this neighbor.
    pub id: EmbeddingId,

    /// Distance from the query vector.
    pub distance: f32,

    /// Similarity score (1 - distance for cosine).
    pub similarity: f32,

    /// Rank in the result set (0 = closest).
    pub rank: usize,
}

impl Neighbor {
    /// Create a new neighbor from search results.
    pub fn new(id: EmbeddingId, distance: f32, rank: usize) -> Self {
        Self {
            id,
            distance,
            similarity: 1.0 - distance.clamp(0.0, 1.0),
            rank,
        }
    }

    /// Check if this neighbor exceeds a similarity threshold.
    #[inline]
    pub fn is_above_threshold(&self, threshold: f32) -> bool {
        self.similarity >= threshold
    }
}

/// Options for search queries.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Maximum number of results to return.
    pub k: usize,

    /// Minimum similarity threshold (results below this are filtered).
    pub min_similarity: Option<f32>,

    /// Maximum distance threshold.
    pub max_distance: Option<f32>,

    /// ef_search parameter override (higher = more accurate but slower).
    pub ef_search: Option<usize>,

    /// Whether to include the query vector in results if it exists.
    pub include_query: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            k: 10,
            min_similarity: None,
            max_distance: None,
            ef_search: None,
            include_query: false,
        }
    }
}

impl SearchOptions {
    /// Create new search options with specified k.
    pub fn new(k: usize) -> Self {
        Self {
            k,
            ..Default::default()
        }
    }

    /// Set minimum similarity threshold.
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = Some(threshold);
        self
    }

    /// Set maximum distance threshold.
    pub fn with_max_distance(mut self, distance: f32) -> Self {
        self.max_distance = Some(distance);
        self
    }

    /// Set ef_search parameter.
    pub fn with_ef_search(mut self, ef: usize) -> Self {
        self.ef_search = Some(ef);
        self
    }

    /// Include query vector in results.
    pub fn include_query(mut self) -> Self {
        self.include_query = true;
        self
    }
}

/// The main service for vector space operations.
///
/// This service provides a thread-safe interface for:
/// - Adding and removing embeddings
/// - Nearest neighbor search
/// - Building similarity graphs
pub struct VectorSpaceService {
    /// The underlying HNSW index.
    index: Arc<RwLock<HnswIndex>>,

    /// Configuration for this service.
    config: HnswConfig,
}

impl VectorSpaceService {
    /// Create a new vector space service with the given configuration.
    pub fn new(config: HnswConfig) -> Self {
        let index = HnswIndex::new(&config);
        Self {
            index: Arc::new(RwLock::new(index)),
            config,
        }
    }

    /// Create a service from an existing index.
    pub fn from_index(index: HnswIndex, config: HnswConfig) -> Self {
        Self {
            index: Arc::new(RwLock::new(index)),
            config,
        }
    }

    /// Get the index dimensions.
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.config.dimensions
    }

    /// Get the current number of vectors.
    pub fn len(&self) -> usize {
        self.index.read().len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &HnswConfig {
        &self.config
    }

    /// Add a single embedding to the index.
    ///
    /// The vector will be normalized if the configuration specifies normalization.
    #[instrument(skip(self, vector), fields(vector_dim = vector.len()))]
    pub async fn add_embedding(
        &self,
        id: EmbeddingId,
        vector: Vec<f32>,
    ) -> Result<(), VectorError> {
        self.validate_vector(&vector)?;

        let vector = if self.config.normalize {
            normalize_vector(&vector)
        } else {
            vector
        };

        let mut index = self.index.write();
        index.insert(id, &vector)?;

        debug!(id = %id, "Added embedding to index");
        Ok(())
    }

    /// Add multiple embeddings in a batch.
    ///
    /// This is more efficient than multiple single adds due to
    /// amortized locking overhead.
    #[instrument(skip(self, items), fields(batch_size = items.len()))]
    pub async fn add_embeddings_batch(
        &self,
        items: Vec<(EmbeddingId, Vec<f32>)>,
    ) -> Result<usize, VectorError> {
        if items.is_empty() {
            return Ok(0);
        }

        // Validate all vectors first
        for (_, vector) in &items {
            self.validate_vector(vector)?;
        }

        // Normalize if needed
        let items: Vec<_> = if self.config.normalize {
            items
                .into_iter()
                .map(|(id, v)| (id, normalize_vector(&v)))
                .collect()
        } else {
            items
        };

        let mut index = self.index.write();
        let mut added = 0;

        for (id, vector) in &items {
            if let Err(e) = index.insert(*id, vector) {
                warn!(id = %id, error = %e, "Failed to add embedding in batch");
            } else {
                added += 1;
            }
        }

        info!(added, total = items.len(), "Batch insert completed");
        Ok(added)
    }

    /// Find the k nearest neighbors to a query vector.
    #[instrument(skip(self, query), fields(query_dim = query.len(), k))]
    pub async fn find_neighbors(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<Neighbor>, VectorError> {
        self.find_neighbors_with_options(query, SearchOptions::new(k))
            .await
    }

    /// Find neighbors with custom search options.
    #[instrument(skip(self, query, options), fields(query_dim = query.len()))]
    pub async fn find_neighbors_with_options(
        &self,
        query: &[f32],
        options: SearchOptions,
    ) -> Result<Vec<Neighbor>, VectorError> {
        self.validate_vector(query)?;

        let query = if self.config.normalize {
            normalize_vector(query)
        } else {
            query.to_vec()
        };

        let index = self.index.read();

        if index.is_empty() {
            return Ok(Vec::new());
        }

        // Request more results if we're filtering
        let k_fetch = if options.min_similarity.is_some() || options.max_distance.is_some() {
            options.k * 2
        } else {
            options.k
        };

        let results = index.search(&query, k_fetch);

        let mut neighbors: Vec<_> = results
            .into_iter()
            .enumerate()
            .map(|(rank, (id, distance))| Neighbor::new(id, distance, rank))
            .collect();

        // Apply filters
        if let Some(min_sim) = options.min_similarity {
            neighbors.retain(|n| n.similarity >= min_sim);
        }
        if let Some(max_dist) = options.max_distance {
            neighbors.retain(|n| n.distance <= max_dist);
        }

        // Truncate to requested k
        neighbors.truncate(options.k);

        // Re-rank after filtering
        for (rank, neighbor) in neighbors.iter_mut().enumerate() {
            neighbor.rank = rank;
        }

        debug!(found = neighbors.len(), "Neighbor search completed");
        Ok(neighbors)
    }

    /// Find neighbors using a filter predicate.
    ///
    /// The filter function receives an EmbeddingId and returns true if the
    /// embedding should be included in results.
    #[instrument(skip(self, query, filter), fields(query_dim = query.len(), k))]
    pub async fn find_neighbors_with_filter<F>(
        &self,
        query: &[f32],
        k: usize,
        filter: F,
    ) -> Result<Vec<Neighbor>, VectorError>
    where
        F: Fn(&EmbeddingId) -> bool + Send + Sync,
    {
        self.validate_vector(query)?;

        let query = if self.config.normalize {
            normalize_vector(query)
        } else {
            query.to_vec()
        };

        let index = self.index.read();

        if index.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch more results to account for filtering
        let k_fetch = k * 4;
        let results = index.search(&query, k_fetch);

        let mut neighbors: Vec<_> = results
            .into_iter()
            .filter(|(id, _)| filter(id))
            .take(k)
            .enumerate()
            .map(|(rank, (id, distance))| Neighbor::new(id, distance, rank))
            .collect();

        // Re-rank
        for (rank, neighbor) in neighbors.iter_mut().enumerate() {
            neighbor.rank = rank;
        }

        Ok(neighbors)
    }

    /// Remove an embedding from the index.
    #[instrument(skip(self))]
    pub async fn remove_embedding(&self, id: &EmbeddingId) -> Result<(), VectorError> {
        let mut index = self.index.write();
        index.remove(id)?;
        debug!(id = %id, "Removed embedding from index");
        Ok(())
    }

    /// Check if an embedding exists in the index.
    pub fn contains(&self, id: &EmbeddingId) -> bool {
        self.index.read().contains(id)
    }

    /// Get a vector by its ID.
    pub fn get_vector(&self, id: &EmbeddingId) -> Option<Vec<f32>> {
        self.index.read().get_vector(id)
    }

    /// Build similarity edges for an embedding.
    ///
    /// This finds the k nearest neighbors and creates edges to them.
    #[instrument(skip(self, vector))]
    pub async fn build_similarity_edges(
        &self,
        id: EmbeddingId,
        vector: &[f32],
        k: usize,
        min_similarity: f32,
    ) -> Result<Vec<SimilarityEdge>, VectorError> {
        let neighbors = self
            .find_neighbors_with_options(
                vector,
                SearchOptions::new(k).with_min_similarity(min_similarity),
            )
            .await?;

        let edges: Vec<_> = neighbors
            .into_iter()
            .filter(|n| n.id != id) // Exclude self
            .map(|n| {
                SimilarityEdge::new(id, n.id, n.distance)
                    .with_type(EdgeType::Similar)
            })
            .collect();

        Ok(edges)
    }

    /// Compute pairwise similarities for a set of embeddings.
    #[instrument(skip(self, vectors))]
    pub async fn compute_pairwise_similarities(
        &self,
        vectors: &[(EmbeddingId, Vec<f32>)],
    ) -> Result<Vec<(EmbeddingId, EmbeddingId, f32)>, VectorError> {
        if vectors.len() < 2 {
            return Ok(Vec::new());
        }

        // Validate all vectors
        for (_, vector) in vectors {
            self.validate_vector(vector)?;
        }

        // Normalize if needed
        let vectors: Vec<_> = if self.config.normalize {
            vectors
                .iter()
                .map(|(id, v)| (*id, normalize_vector(v)))
                .collect()
        } else {
            vectors.to_vec()
        };

        let mut similarities = Vec::with_capacity(vectors.len() * (vectors.len() - 1) / 2);

        for i in 0..vectors.len() {
            for j in (i + 1)..vectors.len() {
                let sim = cosine_similarity(&vectors[i].1, &vectors[j].1);
                similarities.push((vectors[i].0, vectors[j].0, sim));
            }
        }

        Ok(similarities)
    }

    /// Clear all embeddings from the index.
    pub async fn clear(&self) -> Result<(), VectorError> {
        let mut index = self.index.write();
        index.clear();
        info!("Cleared all embeddings from index");
        Ok(())
    }

    /// Save the index to a file.
    pub async fn save(&self, path: &std::path::Path) -> Result<(), VectorError> {
        let index = self.index.read();
        index.save(path)?;
        info!(path = %path.display(), "Saved index to file");
        Ok(())
    }

    /// Load an index from a file.
    pub async fn load(path: &std::path::Path, config: HnswConfig) -> Result<Self, VectorError> {
        let index = HnswIndex::load(path)?;
        info!(path = %path.display(), "Loaded index from file");
        Ok(Self::from_index(index, config))
    }

    /// Get index statistics.
    pub fn stats(&self) -> IndexStatistics {
        let index = self.index.read();
        IndexStatistics {
            vector_count: index.len(),
            dimensions: self.config.dimensions,
            max_capacity: self.config.max_elements,
            utilization: index.len() as f64 / self.config.max_elements as f64,
        }
    }

    /// Validate a vector.
    fn validate_vector(&self, vector: &[f32]) -> Result<(), VectorError> {
        if vector.len() != self.config.dimensions {
            return Err(VectorError::dimension_mismatch(
                self.config.dimensions,
                vector.len(),
            ));
        }

        // Check for NaN or Inf
        for (i, &v) in vector.iter().enumerate() {
            if v.is_nan() {
                return Err(VectorError::invalid_vector(format!(
                    "NaN value at index {i}"
                )));
            }
            if v.is_infinite() {
                return Err(VectorError::invalid_vector(format!(
                    "Infinite value at index {i}"
                )));
            }
        }

        Ok(())
    }
}

impl Clone for VectorSpaceService {
    fn clone(&self) -> Self {
        Self {
            index: Arc::clone(&self.index),
            config: self.config.clone(),
        }
    }
}

/// Statistics about the vector index.
#[derive(Debug, Clone)]
pub struct IndexStatistics {
    /// Number of vectors in the index.
    pub vector_count: usize,

    /// Dimensionality of vectors.
    pub dimensions: usize,

    /// Maximum capacity.
    pub max_capacity: usize,

    /// Utilization ratio (0.0 - 1.0).
    pub utilization: f64,
}

/// Builder for `VectorSpaceService`.
pub struct VectorSpaceServiceBuilder {
    config: HnswConfig,
}

impl VectorSpaceServiceBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            config: HnswConfig::default(),
        }
    }

    /// Set the dimensions.
    pub fn dimensions(mut self, dim: usize) -> Self {
        self.config.dimensions = dim;
        self
    }

    /// Set the M parameter.
    pub fn m(mut self, m: usize) -> Self {
        self.config.m = m;
        self
    }

    /// Set ef_construction.
    pub fn ef_construction(mut self, ef: usize) -> Self {
        self.config.ef_construction = ef;
        self
    }

    /// Set ef_search.
    pub fn ef_search(mut self, ef: usize) -> Self {
        self.config.ef_search = ef;
        self
    }

    /// Set max elements.
    pub fn max_elements(mut self, max: usize) -> Self {
        self.config.max_elements = max;
        self
    }

    /// Enable or disable normalization.
    pub fn normalize(mut self, normalize: bool) -> Self {
        self.config.normalize = normalize;
        self
    }

    /// Build the service.
    pub fn build(self) -> Result<VectorSpaceService, VectorError> {
        self.config.validate()?;
        Ok(VectorSpaceService::new(self.config))
    }
}

impl Default for VectorSpaceServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> VectorSpaceService {
        let config = HnswConfig::for_dimension(128)
            .with_max_elements(1000)
            .with_normalize(false);
        VectorSpaceService::new(config)
    }

    #[tokio::test]
    async fn test_add_and_search() {
        let service = create_test_service();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();

        let v1: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
        let v2: Vec<f32> = (0..128).map(|i| (i as f32 + 1.0) / 128.0).collect();

        service.add_embedding(id1, v1.clone()).await.unwrap();
        service.add_embedding(id2, v2).await.unwrap();

        assert_eq!(service.len(), 2);

        let neighbors = service.find_neighbors(&v1, 2).await.unwrap();
        assert_eq!(neighbors.len(), 2);
        assert_eq!(neighbors[0].id, id1);
    }

    #[tokio::test]
    async fn test_dimension_mismatch() {
        let service = create_test_service();
        let id = EmbeddingId::new();
        let wrong_dim: Vec<f32> = vec![0.1; 64];

        let result = service.add_embedding(id, wrong_dim).await;
        assert!(matches!(
            result,
            Err(VectorError::DimensionMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let service = create_test_service();

        let items: Vec<_> = (0..10)
            .map(|i| {
                let id = EmbeddingId::new();
                let vector: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32 / 1280.0).collect();
                (id, vector)
            })
            .collect();

        let added = service.add_embeddings_batch(items).await.unwrap();
        assert_eq!(added, 10);
        assert_eq!(service.len(), 10);
    }

    #[tokio::test]
    async fn test_search_with_filter() {
        let service = create_test_service();

        let ids: Vec<_> = (0..5).map(|_| EmbeddingId::new()).collect();

        for (i, id) in ids.iter().enumerate() {
            let vector: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32 / 640.0).collect();
            service.add_embedding(*id, vector).await.unwrap();
        }

        let query: Vec<f32> = (0..128).map(|j| j as f32 / 640.0).collect();

        // Filter to only include odd indices
        let odd_ids: std::collections::HashSet<_> =
            ids.iter().enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, id)| *id).collect();

        let neighbors = service
            .find_neighbors_with_filter(&query, 10, |id| odd_ids.contains(id))
            .await
            .unwrap();

        for n in &neighbors {
            assert!(odd_ids.contains(&n.id));
        }
    }

    #[test]
    fn test_neighbor() {
        let neighbor = Neighbor::new(EmbeddingId::new(), 0.2, 0);
        assert!((neighbor.similarity - 0.8).abs() < 0.001);
        assert!(neighbor.is_above_threshold(0.7));
        assert!(!neighbor.is_above_threshold(0.9));
    }

    #[test]
    fn test_search_options() {
        let opts = SearchOptions::new(10)
            .with_min_similarity(0.8)
            .with_max_distance(0.3);

        assert_eq!(opts.k, 10);
        assert_eq!(opts.min_similarity, Some(0.8));
        assert_eq!(opts.max_distance, Some(0.3));
    }

    #[test]
    fn test_builder() {
        let service = VectorSpaceServiceBuilder::new()
            .dimensions(256)
            .m(16)
            .ef_construction(100)
            .max_elements(5000)
            .build()
            .unwrap();

        assert_eq!(service.dimensions(), 256);
    }
}
