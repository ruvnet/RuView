//! Repository traits for the Interpretation bounded context.
//!
//! These traits define the persistence interfaces for evidence packs
//! and related entities.

use async_trait::async_trait;

use crate::{Error, Result};
use super::entities::{EvidencePack, EmbeddingId, ClusterId, ClusterContext};

/// Repository for persisting and retrieving evidence packs.
///
/// Implementations of this trait handle the storage and retrieval
/// of evidence packs, which are the primary artifacts of RAB interpretation.
#[async_trait]
pub trait EvidencePackRepository: Send + Sync {
    /// Save an evidence pack to the repository.
    ///
    /// If an evidence pack with the same ID already exists, it will be updated.
    async fn save(&self, pack: &EvidencePack) -> Result<()>;

    /// Find an evidence pack by its unique identifier.
    async fn find_by_id(&self, id: &str) -> Result<Option<EvidencePack>>;

    /// Find all evidence packs for a given query embedding.
    ///
    /// Returns evidence packs in reverse chronological order (newest first).
    async fn find_by_query(&self, embedding_id: &EmbeddingId) -> Result<Vec<EvidencePack>>;

    /// Delete an evidence pack by ID.
    async fn delete(&self, id: &str) -> Result<bool>;

    /// Find evidence packs created within a time range.
    async fn find_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<EvidencePack>>;

    /// Count total evidence packs in the repository.
    async fn count(&self) -> Result<usize>;
}

/// Repository for cluster information used in interpretation.
///
/// This trait provides read access to cluster data needed for
/// building evidence packs and generating interpretations.
#[async_trait]
pub trait ClusterRepository: Send + Sync {
    /// Get cluster context for an embedding.
    async fn get_cluster_context(&self, embedding_id: &EmbeddingId) -> Result<ClusterContext>;

    /// Get the label for a cluster.
    async fn get_cluster_label(&self, cluster_id: &ClusterId) -> Result<Option<String>>;

    /// Get the exemplar embedding for a cluster.
    async fn get_cluster_exemplar(&self, cluster_id: &ClusterId) -> Result<Option<EmbeddingId>>;

    /// Get all embeddings in a cluster.
    async fn get_cluster_members(&self, cluster_id: &ClusterId) -> Result<Vec<EmbeddingId>>;

    /// Get statistics about a cluster.
    async fn get_cluster_stats(&self, cluster_id: &ClusterId) -> Result<ClusterStats>;
}

/// Statistics about a cluster.
#[derive(Debug, Clone)]
pub struct ClusterStats {
    /// Number of embeddings in the cluster
    pub member_count: usize,
    /// Average distance from cluster center
    pub avg_distance: f32,
    /// Maximum distance from cluster center
    pub max_distance: f32,
    /// Cluster coherence score (0.0 to 1.0)
    pub coherence: f32,
}

impl Default for ClusterStats {
    fn default() -> Self {
        Self {
            member_count: 0,
            avg_distance: 0.0,
            max_distance: 0.0,
            coherence: 0.0,
        }
    }
}

/// In-memory implementation of EvidencePackRepository for testing.
#[derive(Debug, Default)]
pub struct InMemoryEvidencePackRepository {
    packs: std::sync::RwLock<std::collections::HashMap<String, EvidencePack>>,
}

impl InMemoryEvidencePackRepository {
    /// Create a new in-memory repository.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EvidencePackRepository for InMemoryEvidencePackRepository {
    async fn save(&self, pack: &EvidencePack) -> Result<()> {
        let mut packs = self.packs.write().map_err(|e| Error::internal(e.to_string()))?;
        packs.insert(pack.id.clone(), pack.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<EvidencePack>> {
        let packs = self.packs.read().map_err(|e| Error::internal(e.to_string()))?;
        Ok(packs.get(id).cloned())
    }

    async fn find_by_query(&self, embedding_id: &EmbeddingId) -> Result<Vec<EvidencePack>> {
        let packs = self.packs.read().map_err(|e| Error::internal(e.to_string()))?;
        let mut results: Vec<EvidencePack> = packs
            .values()
            .filter(|p| p.query_embedding_id == *embedding_id)
            .cloned()
            .collect();
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(results)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let mut packs = self.packs.write().map_err(|e| Error::internal(e.to_string()))?;
        Ok(packs.remove(id).is_some())
    }

    async fn find_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<EvidencePack>> {
        let packs = self.packs.read().map_err(|e| Error::internal(e.to_string()))?;
        let mut results: Vec<EvidencePack> = packs
            .values()
            .filter(|p| p.created_at >= start && p.created_at <= end)
            .cloned()
            .collect();
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(results)
    }

    async fn count(&self) -> Result<usize> {
        let packs = self.packs.read().map_err(|e| Error::internal(e.to_string()))?;
        Ok(packs.len())
    }
}

/// In-memory implementation of ClusterRepository for testing.
#[derive(Debug, Default)]
pub struct InMemoryClusterRepository {
    clusters: std::sync::RwLock<std::collections::HashMap<ClusterId, ClusterData>>,
    assignments: std::sync::RwLock<std::collections::HashMap<EmbeddingId, ClusterId>>,
}

#[derive(Debug, Clone)]
struct ClusterData {
    label: Option<String>,
    exemplar: Option<EmbeddingId>,
    members: Vec<EmbeddingId>,
    stats: ClusterStats,
}

impl InMemoryClusterRepository {
    /// Create a new in-memory cluster repository.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cluster to the repository.
    pub fn add_cluster(
        &self,
        cluster_id: ClusterId,
        label: Option<String>,
        exemplar: Option<EmbeddingId>,
    ) -> Result<()> {
        let mut clusters = self.clusters.write().map_err(|e| Error::internal(e.to_string()))?;
        clusters.insert(cluster_id, ClusterData {
            label,
            exemplar,
            members: Vec::new(),
            stats: ClusterStats::default(),
        });
        Ok(())
    }

    /// Assign an embedding to a cluster.
    pub fn assign_to_cluster(
        &self,
        embedding_id: EmbeddingId,
        cluster_id: ClusterId,
    ) -> Result<()> {
        let mut assignments = self.assignments.write().map_err(|e| Error::internal(e.to_string()))?;
        assignments.insert(embedding_id.clone(), cluster_id.clone());

        let mut clusters = self.clusters.write().map_err(|e| Error::internal(e.to_string()))?;
        if let Some(cluster) = clusters.get_mut(&cluster_id) {
            cluster.members.push(embedding_id);
        }
        Ok(())
    }
}

#[async_trait]
impl ClusterRepository for InMemoryClusterRepository {
    async fn get_cluster_context(&self, embedding_id: &EmbeddingId) -> Result<ClusterContext> {
        let assignments = self.assignments.read().map_err(|e| Error::internal(e.to_string()))?;
        let cluster_id = assignments.get(embedding_id).cloned();

        if let Some(cid) = &cluster_id {
            let clusters = self.clusters.read().map_err(|e| Error::internal(e.to_string()))?;
            if let Some(cluster) = clusters.get(cid) {
                return Ok(ClusterContext {
                    assigned_cluster: Some(cid.clone()),
                    cluster_label: cluster.label.clone(),
                    confidence: 0.85,
                    exemplar_similarity: 0.90,
                });
            }
        }

        Ok(ClusterContext::empty())
    }

    async fn get_cluster_label(&self, cluster_id: &ClusterId) -> Result<Option<String>> {
        let clusters = self.clusters.read().map_err(|e| Error::internal(e.to_string()))?;
        Ok(clusters.get(cluster_id).and_then(|c| c.label.clone()))
    }

    async fn get_cluster_exemplar(&self, cluster_id: &ClusterId) -> Result<Option<EmbeddingId>> {
        let clusters = self.clusters.read().map_err(|e| Error::internal(e.to_string()))?;
        Ok(clusters.get(cluster_id).and_then(|c| c.exemplar.clone()))
    }

    async fn get_cluster_members(&self, cluster_id: &ClusterId) -> Result<Vec<EmbeddingId>> {
        let clusters = self.clusters.read().map_err(|e| Error::internal(e.to_string()))?;
        Ok(clusters.get(cluster_id).map(|c| c.members.clone()).unwrap_or_default())
    }

    async fn get_cluster_stats(&self, cluster_id: &ClusterId) -> Result<ClusterStats> {
        let clusters = self.clusters.read().map_err(|e| Error::internal(e.to_string()))?;
        Ok(clusters.get(cluster_id).map(|c| c.stats.clone()).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_evidence_pack_repo() {
        use crate::domain::entities::*;

        let repo = InMemoryEvidencePackRepository::new();

        let pack = EvidencePack::new(
            EmbeddingId::new("query-1"),
            Vec::new(),
            ClusterContext::empty(),
            None,
            Interpretation::empty(),
        );

        repo.save(&pack).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        let found = repo.find_by_id(&pack.id).await.unwrap();
        assert!(found.is_some());

        let by_query = repo.find_by_query(&EmbeddingId::new("query-1")).await.unwrap();
        assert_eq!(by_query.len(), 1);

        repo.delete(&pack.id).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_cluster_repo() {
        let repo = InMemoryClusterRepository::new();

        let cluster_id = ClusterId::new("cluster-1");
        repo.add_cluster(
            cluster_id.clone(),
            Some("Song Type A".to_string()),
            Some(EmbeddingId::new("exemplar-1")),
        ).unwrap();

        let embedding_id = EmbeddingId::new("emb-1");
        repo.assign_to_cluster(embedding_id.clone(), cluster_id.clone()).unwrap();

        let context = repo.get_cluster_context(&embedding_id).await.unwrap();
        assert!(context.has_cluster());
        assert_eq!(context.cluster_label, Some("Song Type A".to_string()));

        let label = repo.get_cluster_label(&cluster_id).await.unwrap();
        assert_eq!(label, Some("Song Type A".to_string()));

        let members = repo.get_cluster_members(&cluster_id).await.unwrap();
        assert_eq!(members.len(), 1);
    }
}
