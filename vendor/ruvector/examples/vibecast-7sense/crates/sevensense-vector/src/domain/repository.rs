//! Repository traits for the Vector Space bounded context.
//!
//! These traits define the persistence abstractions that allow the domain
//! to remain independent of specific storage implementations.

use async_trait::async_trait;

use super::entities::{EmbeddingId, SimilarityEdge, EdgeType, StoredVector, VectorMetadata};
use super::error::VectorError;

/// Result type for repository operations.
pub type RepoResult<T> = Result<T, VectorError>;

/// Repository trait for vector index operations.
///
/// This trait abstracts the HNSW index storage, allowing for different
/// implementations (in-memory, file-backed, distributed).
#[async_trait]
pub trait VectorIndexRepository: Send + Sync {
    /// Insert a single vector into the index.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this vector
    /// * `vector` - The embedding vector data
    ///
    /// # Errors
    /// Returns error if the vector dimensions don't match the index configuration.
    async fn insert(&self, id: &EmbeddingId, vector: &[f32]) -> RepoResult<()>;

    /// Search for the k nearest neighbors to a query vector.
    ///
    /// # Arguments
    /// * `query` - The query vector
    /// * `k` - Number of neighbors to return
    ///
    /// # Returns
    /// A vector of (id, distance) tuples, sorted by ascending distance.
    async fn search(&self, query: &[f32], k: usize) -> RepoResult<Vec<(EmbeddingId, f32)>>;

    /// Insert multiple vectors in a batch.
    ///
    /// This is more efficient than multiple single inserts due to
    /// amortized locking and potential parallelization.
    ///
    /// # Arguments
    /// * `items` - Slice of (id, vector) pairs to insert
    async fn batch_insert(&self, items: &[(EmbeddingId, Vec<f32>)]) -> RepoResult<()>;

    /// Remove a vector from the index.
    ///
    /// # Arguments
    /// * `id` - The ID of the vector to remove
    ///
    /// # Note
    /// Not all HNSW implementations support efficient removal.
    /// Some may mark as deleted without reclaiming space.
    async fn remove(&self, id: &EmbeddingId) -> RepoResult<()>;

    /// Check if a vector exists in the index.
    async fn contains(&self, id: &EmbeddingId) -> RepoResult<bool>;

    /// Get the current number of vectors in the index.
    async fn len(&self) -> RepoResult<usize>;

    /// Check if the index is empty.
    async fn is_empty(&self) -> RepoResult<bool> {
        Ok(self.len().await? == 0)
    }

    /// Clear all vectors from the index.
    async fn clear(&self) -> RepoResult<()>;

    /// Get the dimensionality of vectors in this index.
    fn dimensions(&self) -> usize;
}

/// Extended repository trait with additional query capabilities.
#[async_trait]
pub trait VectorIndexRepositoryExt: VectorIndexRepository {
    /// Search with a filter predicate.
    ///
    /// # Arguments
    /// * `query` - The query vector
    /// * `k` - Number of neighbors to return
    /// * `filter` - Predicate that must return true for results to be included
    async fn search_with_filter<F>(
        &self,
        query: &[f32],
        k: usize,
        filter: F,
    ) -> RepoResult<Vec<(EmbeddingId, f32)>>
    where
        F: Fn(&EmbeddingId) -> bool + Send + Sync;

    /// Search within a distance threshold.
    ///
    /// Returns all vectors within the given distance, up to a maximum count.
    async fn search_within_radius(
        &self,
        query: &[f32],
        radius: f32,
        max_results: usize,
    ) -> RepoResult<Vec<(EmbeddingId, f32)>>;

    /// Get multiple vectors by their IDs.
    async fn get_vectors(&self, ids: &[EmbeddingId]) -> RepoResult<Vec<Option<StoredVector>>>;

    /// Get a single vector by ID.
    async fn get_vector(&self, id: &EmbeddingId) -> RepoResult<Option<StoredVector>>;

    /// Update the metadata for a vector.
    async fn update_metadata(&self, id: &EmbeddingId, metadata: VectorMetadata) -> RepoResult<()>;

    /// List all vector IDs in the index.
    async fn list_ids(&self, offset: usize, limit: usize) -> RepoResult<Vec<EmbeddingId>>;
}

/// Repository trait for graph edge operations.
///
/// This manages the similarity graph between embeddings, supporting
/// graph-based queries and traversals.
#[async_trait]
pub trait GraphEdgeRepository: Send + Sync {
    /// Add an edge between two embeddings.
    async fn add_edge(&self, edge: SimilarityEdge) -> RepoResult<()>;

    /// Add multiple edges in a batch.
    async fn add_edges(&self, edges: &[SimilarityEdge]) -> RepoResult<()>;

    /// Remove an edge between two embeddings.
    async fn remove_edge(&self, from: &EmbeddingId, to: &EmbeddingId) -> RepoResult<()>;

    /// Get all edges from a given embedding.
    async fn get_edges_from(&self, id: &EmbeddingId) -> RepoResult<Vec<SimilarityEdge>>;

    /// Get all edges to a given embedding.
    async fn get_edges_to(&self, id: &EmbeddingId) -> RepoResult<Vec<SimilarityEdge>>;

    /// Get edges of a specific type from an embedding.
    async fn get_edges_by_type(
        &self,
        id: &EmbeddingId,
        edge_type: EdgeType,
    ) -> RepoResult<Vec<SimilarityEdge>>;

    /// Find edges with similarity above a threshold.
    async fn get_strong_edges(
        &self,
        id: &EmbeddingId,
        min_similarity: f32,
    ) -> RepoResult<Vec<SimilarityEdge>>;

    /// Get the number of edges in the graph.
    async fn edge_count(&self) -> RepoResult<usize>;

    /// Clear all edges.
    async fn clear(&self) -> RepoResult<()>;

    /// Remove all edges connected to an embedding.
    async fn remove_edges_for(&self, id: &EmbeddingId) -> RepoResult<()>;
}

/// Trait for graph traversal operations.
#[async_trait]
pub trait GraphTraversal: GraphEdgeRepository {
    /// Find the shortest path between two embeddings.
    async fn shortest_path(
        &self,
        from: &EmbeddingId,
        to: &EmbeddingId,
        max_depth: usize,
    ) -> RepoResult<Option<Vec<EmbeddingId>>>;

    /// Find all embeddings within n hops of a given embedding.
    async fn neighbors_within_hops(
        &self,
        id: &EmbeddingId,
        hops: usize,
    ) -> RepoResult<Vec<(EmbeddingId, usize)>>;

    /// Find connected components in the graph.
    async fn connected_components(&self) -> RepoResult<Vec<Vec<EmbeddingId>>>;

    /// Calculate PageRank-style centrality for embeddings.
    async fn centrality_scores(&self) -> RepoResult<Vec<(EmbeddingId, f32)>>;
}

/// Trait for persistence operations.
#[async_trait]
pub trait IndexPersistence: Send + Sync {
    /// Save the index to a file.
    async fn save(&self, path: &std::path::Path) -> RepoResult<()>;

    /// Load the index from a file.
    async fn load(path: &std::path::Path) -> RepoResult<Self>
    where
        Self: Sized;

    /// Export the index to a portable format.
    async fn export(&self, path: &std::path::Path, format: ExportFormat) -> RepoResult<()>;

    /// Import vectors from a portable format.
    async fn import(&self, path: &std::path::Path, format: ExportFormat) -> RepoResult<usize>;
}

/// Export formats for index data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Binary format using bincode (fast, compact).
    Bincode,
    /// JSON format (readable, portable).
    Json,
    /// NumPy-compatible format for vector data.
    Numpy,
    /// CSV format for interoperability.
    Csv,
}

/// Statistics about an index.
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Total number of vectors.
    pub vector_count: usize,

    /// Number of bytes used.
    pub memory_bytes: usize,

    /// Average search latency in microseconds.
    pub avg_search_latency_us: f64,

    /// Index build time in milliseconds.
    pub build_time_ms: u64,

    /// Dimensionality.
    pub dimensions: usize,

    /// Number of levels in the HNSW graph.
    pub levels: usize,

    /// Average connections per node.
    pub avg_connections: f64,
}

/// Trait for index statistics and monitoring.
#[async_trait]
pub trait IndexMonitoring: Send + Sync {
    /// Get current index statistics.
    async fn stats(&self) -> RepoResult<IndexStats>;

    /// Get memory usage breakdown.
    async fn memory_usage(&self) -> RepoResult<MemoryUsage>;

    /// Run a self-check to verify index integrity.
    async fn verify(&self) -> RepoResult<VerificationResult>;
}

/// Memory usage breakdown.
#[derive(Debug, Clone)]
pub struct MemoryUsage {
    /// Memory used by vector data.
    pub vectors_bytes: usize,

    /// Memory used by the HNSW graph structure.
    pub graph_bytes: usize,

    /// Memory used by ID mappings.
    pub id_map_bytes: usize,

    /// Memory used by metadata.
    pub metadata_bytes: usize,

    /// Total memory.
    pub total_bytes: usize,
}

/// Result of index verification.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the index is valid.
    pub is_valid: bool,

    /// List of issues found.
    pub issues: Vec<String>,

    /// Number of orphaned nodes.
    pub orphaned_nodes: usize,

    /// Number of broken links.
    pub broken_links: usize,
}

impl VerificationResult {
    /// Create a successful verification result.
    pub fn ok() -> Self {
        Self {
            is_valid: true,
            issues: Vec::new(),
            orphaned_nodes: 0,
            broken_links: 0,
        }
    }

    /// Create a failed verification result.
    pub fn failed(issues: Vec<String>) -> Self {
        Self {
            is_valid: false,
            issues,
            orphaned_nodes: 0,
            broken_links: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_result() {
        let ok = VerificationResult::ok();
        assert!(ok.is_valid);
        assert!(ok.issues.is_empty());

        let failed = VerificationResult::failed(vec!["test issue".into()]);
        assert!(!failed.is_valid);
        assert_eq!(failed.issues.len(), 1);
    }
}
