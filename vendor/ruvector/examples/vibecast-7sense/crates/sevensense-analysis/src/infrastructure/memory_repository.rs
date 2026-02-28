//! In-memory repository implementation for testing and development.
//!
//! Provides thread-safe in-memory storage for all analysis entities.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::domain::entities::{
    Anomaly, Cluster, ClusterId, EmbeddingId, Motif, Prototype, RecordingId, SequenceAnalysis,
};
use crate::domain::repository::{
    AnomalyRepository, ClusterRepository, MotifRepository, PrototypeRepository,
    RepositoryError, Result, SequenceRepository,
};

/// In-memory implementation of the analysis repositories.
///
/// Useful for testing and development. Not suitable for production use
/// as data is lost on restart.
pub struct InMemoryAnalysisRepository {
    clusters: RwLock<HashMap<ClusterId, Cluster>>,
    prototypes: RwLock<HashMap<ClusterId, Vec<Prototype>>>,
    motifs: RwLock<HashMap<String, Motif>>,
    sequences: RwLock<HashMap<RecordingId, SequenceAnalysis>>,
    anomalies: RwLock<HashMap<EmbeddingId, Anomaly>>,
    /// Mapping from embedding ID to cluster ID
    embedding_assignments: RwLock<HashMap<EmbeddingId, ClusterId>>,
}

impl InMemoryAnalysisRepository {
    /// Create a new empty in-memory repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clusters: RwLock::new(HashMap::new()),
            prototypes: RwLock::new(HashMap::new()),
            motifs: RwLock::new(HashMap::new()),
            sequences: RwLock::new(HashMap::new()),
            anomalies: RwLock::new(HashMap::new()),
            embedding_assignments: RwLock::new(HashMap::new()),
        }
    }

    /// Get statistics about stored data.
    #[must_use]
    pub fn stats(&self) -> RepositoryStats {
        let clusters = self.clusters.read().unwrap();
        let prototypes = self.prototypes.read().unwrap();
        let motifs = self.motifs.read().unwrap();
        let sequences = self.sequences.read().unwrap();
        let anomalies = self.anomalies.read().unwrap();

        RepositoryStats {
            cluster_count: clusters.len(),
            prototype_count: prototypes.values().map(|v| v.len()).sum(),
            motif_count: motifs.len(),
            sequence_count: sequences.len(),
            anomaly_count: anomalies.len(),
        }
    }
}

impl Default for InMemoryAnalysisRepository {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about repository contents.
#[derive(Debug, Clone)]
pub struct RepositoryStats {
    /// Number of clusters stored.
    pub cluster_count: usize,
    /// Total number of prototypes.
    pub prototype_count: usize,
    /// Number of motifs stored.
    pub motif_count: usize,
    /// Number of sequence analyses stored.
    pub sequence_count: usize,
    /// Number of anomalies stored.
    pub anomaly_count: usize,
}

#[async_trait]
impl ClusterRepository for InMemoryAnalysisRepository {
    async fn save_cluster(&self, cluster: &Cluster) -> Result<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        clusters.insert(cluster.id, cluster.clone());
        Ok(())
    }

    async fn save_clusters(&self, clusters_to_save: &[Cluster]) -> Result<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        for cluster in clusters_to_save {
            clusters.insert(cluster.id, cluster.clone());
        }
        Ok(())
    }

    async fn find_cluster(&self, id: &ClusterId) -> Result<Option<Cluster>> {
        let clusters = self.clusters.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(clusters.get(id).cloned())
    }

    async fn list_clusters(&self) -> Result<Vec<Cluster>> {
        let clusters = self.clusters.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(clusters.values().cloned().collect())
    }

    async fn list_clusters_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Cluster>> {
        let clusters = self.clusters.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(clusters.values().skip(offset).take(limit).cloned().collect())
    }

    async fn assign_to_cluster(
        &self,
        embedding_id: &EmbeddingId,
        cluster_id: &ClusterId,
    ) -> Result<()> {
        let mut assignments = self.embedding_assignments.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        assignments.insert(*embedding_id, *cluster_id);
        Ok(())
    }

    async fn remove_from_cluster(&self, embedding_id: &EmbeddingId) -> Result<()> {
        let mut assignments = self.embedding_assignments.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        assignments.remove(embedding_id);
        Ok(())
    }

    async fn find_cluster_by_embedding(
        &self,
        embedding_id: &EmbeddingId,
    ) -> Result<Option<Cluster>> {
        // Extract the cluster_id and drop the guard before await
        let cluster_id = {
            let assignments = self.embedding_assignments.read().map_err(|e| {
                RepositoryError::Internal(format!("Lock error: {}", e))
            })?;
            assignments.get(embedding_id).cloned()
        };

        if let Some(cluster_id) = cluster_id {
            self.find_cluster(&cluster_id).await
        } else {
            Ok(None)
        }
    }

    async fn delete_cluster(&self, id: &ClusterId) -> Result<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        clusters.remove(id);
        Ok(())
    }

    async fn delete_all_clusters(&self) -> Result<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        clusters.clear();

        let mut assignments = self.embedding_assignments.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        assignments.clear();

        Ok(())
    }

    async fn cluster_count(&self) -> Result<usize> {
        let clusters = self.clusters.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(clusters.len())
    }

    async fn find_clusters_by_label(&self, label_pattern: &str) -> Result<Vec<Cluster>> {
        let clusters = self.clusters.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(clusters
            .values()
            .filter(|c| {
                c.label
                    .as_ref()
                    .map_or(false, |l| l.contains(label_pattern))
            })
            .cloned()
            .collect())
    }

    async fn update_cluster_label(
        &self,
        id: &ClusterId,
        label: Option<String>,
    ) -> Result<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        if let Some(cluster) = clusters.get_mut(id) {
            cluster.label = label;
            Ok(())
        } else {
            Err(RepositoryError::NotFound(format!("Cluster {}", id)))
        }
    }
}

#[async_trait]
impl PrototypeRepository for InMemoryAnalysisRepository {
    async fn save_prototype(&self, prototype: &Prototype) -> Result<()> {
        let mut prototypes = self.prototypes.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        prototypes
            .entry(prototype.cluster_id)
            .or_default()
            .push(prototype.clone());

        Ok(())
    }

    async fn save_prototypes(&self, prototypes_to_save: &[Prototype]) -> Result<()> {
        let mut prototypes = self.prototypes.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        for prototype in prototypes_to_save {
            prototypes
                .entry(prototype.cluster_id)
                .or_default()
                .push(prototype.clone());
        }

        Ok(())
    }

    async fn find_prototypes_by_cluster(
        &self,
        cluster_id: &ClusterId,
    ) -> Result<Vec<Prototype>> {
        let prototypes = self.prototypes.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(prototypes.get(cluster_id).cloned().unwrap_or_default())
    }

    async fn find_best_prototype(
        &self,
        cluster_id: &ClusterId,
    ) -> Result<Option<Prototype>> {
        let prototypes = self.prototypes.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(prototypes.get(cluster_id).and_then(|protos| {
            protos
                .iter()
                .max_by(|a, b| {
                    a.exemplar_score
                        .partial_cmp(&b.exemplar_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned()
        }))
    }

    async fn delete_prototypes_by_cluster(&self, cluster_id: &ClusterId) -> Result<()> {
        let mut prototypes = self.prototypes.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        prototypes.remove(cluster_id);
        Ok(())
    }

    async fn delete_all_prototypes(&self) -> Result<()> {
        let mut prototypes = self.prototypes.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        prototypes.clear();
        Ok(())
    }
}

#[async_trait]
impl MotifRepository for InMemoryAnalysisRepository {
    async fn save_motif(&self, motif: &Motif) -> Result<()> {
        let mut motifs = self.motifs.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        motifs.insert(motif.id.clone(), motif.clone());
        Ok(())
    }

    async fn save_motifs(&self, motifs_to_save: &[Motif]) -> Result<()> {
        let mut motifs = self.motifs.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        for motif in motifs_to_save {
            motifs.insert(motif.id.clone(), motif.clone());
        }
        Ok(())
    }

    async fn find_motif(&self, id: &str) -> Result<Option<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(motifs.get(id).cloned())
    }

    async fn find_motifs_by_cluster(&self, cluster_id: &ClusterId) -> Result<Vec<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(motifs
            .values()
            .filter(|m| m.contains_cluster(cluster_id))
            .cloned()
            .collect())
    }

    async fn list_motifs(&self) -> Result<Vec<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(motifs.values().cloned().collect())
    }

    async fn find_motifs_by_confidence(&self, min_confidence: f32) -> Result<Vec<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(motifs
            .values()
            .filter(|m| m.confidence >= min_confidence)
            .cloned()
            .collect())
    }

    async fn find_motifs_by_occurrences(&self, min_occurrences: usize) -> Result<Vec<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(motifs
            .values()
            .filter(|m| m.occurrences >= min_occurrences)
            .cloned()
            .collect())
    }

    async fn delete_motif(&self, id: &str) -> Result<()> {
        let mut motifs = self.motifs.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        motifs.remove(id);
        Ok(())
    }

    async fn delete_all_motifs(&self) -> Result<()> {
        let mut motifs = self.motifs.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        motifs.clear();
        Ok(())
    }

    async fn motif_count(&self) -> Result<usize> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(motifs.len())
    }

    async fn find_motifs_by_sequence(&self, sequence: &[ClusterId]) -> Result<Vec<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(motifs
            .values()
            .filter(|m| m.sequence == sequence)
            .cloned()
            .collect())
    }

    async fn find_motifs_containing_subsequence(
        &self,
        subsequence: &[ClusterId],
    ) -> Result<Vec<Motif>> {
        let motifs = self.motifs.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(motifs
            .values()
            .filter(|m| {
                m.sequence
                    .windows(subsequence.len())
                    .any(|w| w == subsequence)
            })
            .cloned()
            .collect())
    }
}

#[async_trait]
impl SequenceRepository for InMemoryAnalysisRepository {
    async fn save_sequence_analysis(&self, analysis: &SequenceAnalysis) -> Result<()> {
        let mut sequences = self.sequences.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        sequences.insert(analysis.recording_id, analysis.clone());
        Ok(())
    }

    async fn find_sequence_by_recording(
        &self,
        recording_id: &RecordingId,
    ) -> Result<Option<SequenceAnalysis>> {
        let sequences = self.sequences.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(sequences.get(recording_id).cloned())
    }

    async fn list_sequence_analyses(&self) -> Result<Vec<SequenceAnalysis>> {
        let sequences = self.sequences.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(sequences.values().cloned().collect())
    }

    async fn delete_sequence_by_recording(&self, recording_id: &RecordingId) -> Result<()> {
        let mut sequences = self.sequences.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        sequences.remove(recording_id);
        Ok(())
    }

    async fn delete_all_sequences(&self) -> Result<()> {
        let mut sequences = self.sequences.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        sequences.clear();
        Ok(())
    }

    async fn find_sequences_by_entropy(&self, min_entropy: f32) -> Result<Vec<SequenceAnalysis>> {
        let sequences = self.sequences.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(sequences
            .values()
            .filter(|s| s.entropy >= min_entropy)
            .cloned()
            .collect())
    }

    async fn find_sequences_by_stereotypy(
        &self,
        min_stereotypy: f32,
    ) -> Result<Vec<SequenceAnalysis>> {
        let sequences = self.sequences.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(sequences
            .values()
            .filter(|s| s.stereotypy_score >= min_stereotypy)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl AnomalyRepository for InMemoryAnalysisRepository {
    async fn save_anomaly(&self, anomaly: &Anomaly) -> Result<()> {
        let mut anomalies = self.anomalies.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        anomalies.insert(anomaly.embedding_id, anomaly.clone());
        Ok(())
    }

    async fn save_anomalies(&self, anomalies_to_save: &[Anomaly]) -> Result<()> {
        let mut anomalies = self.anomalies.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        for anomaly in anomalies_to_save {
            anomalies.insert(anomaly.embedding_id, anomaly.clone());
        }
        Ok(())
    }

    async fn find_anomaly(&self, embedding_id: &EmbeddingId) -> Result<Option<Anomaly>> {
        let anomalies = self.anomalies.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(anomalies.get(embedding_id).cloned())
    }

    async fn list_anomalies(&self) -> Result<Vec<Anomaly>> {
        let anomalies = self.anomalies.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(anomalies.values().cloned().collect())
    }

    async fn find_anomalies_by_score(&self, min_score: f32) -> Result<Vec<Anomaly>> {
        let anomalies = self.anomalies.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(anomalies
            .values()
            .filter(|a| a.anomaly_score >= min_score)
            .cloned()
            .collect())
    }

    async fn find_anomalies_by_cluster(&self, cluster_id: &ClusterId) -> Result<Vec<Anomaly>> {
        let anomalies = self.anomalies.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;

        Ok(anomalies
            .values()
            .filter(|a| a.nearest_cluster == *cluster_id)
            .cloned()
            .collect())
    }

    async fn delete_anomaly(&self, embedding_id: &EmbeddingId) -> Result<()> {
        let mut anomalies = self.anomalies.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        anomalies.remove(embedding_id);
        Ok(())
    }

    async fn delete_all_anomalies(&self) -> Result<()> {
        let mut anomalies = self.anomalies.write().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        anomalies.clear();
        Ok(())
    }

    async fn anomaly_count(&self) -> Result<usize> {
        let anomalies = self.anomalies.read().map_err(|e| {
            RepositoryError::Internal(format!("Lock error: {}", e))
        })?;
        Ok(anomalies.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cluster_crud() {
        let repo = InMemoryAnalysisRepository::new();

        let cluster = Cluster::new(
            EmbeddingId::new(),
            vec![EmbeddingId::new()],
            vec![0.0; 10],
            0.1,
        );

        // Save
        repo.save_cluster(&cluster).await.unwrap();

        // Find
        let found = repo.find_cluster(&cluster.id).await.unwrap();
        assert!(found.is_some());

        // List
        let all = repo.list_clusters().await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        repo.delete_cluster(&cluster.id).await.unwrap();
        let found = repo.find_cluster(&cluster.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_motif_crud() {
        let repo = InMemoryAnalysisRepository::new();

        let motif = Motif::new(
            vec![ClusterId::new(), ClusterId::new()],
            5,
            1500.0,
            0.8,
        );

        repo.save_motif(&motif).await.unwrap();

        let found = repo.find_motif(&motif.id).await.unwrap();
        assert!(found.is_some());

        let count = repo.motif_count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_sequence_crud() {
        let repo = InMemoryAnalysisRepository::new();

        let recording_id = RecordingId::new();
        let analysis = SequenceAnalysis::new(
            recording_id,
            vec![],
            1.5,
            0.5,
        );

        repo.save_sequence_analysis(&analysis).await.unwrap();

        let found = repo.find_sequence_by_recording(&recording_id).await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_anomaly_filtering() {
        let repo = InMemoryAnalysisRepository::new();

        let anomaly1 = Anomaly::new(
            EmbeddingId::new(),
            0.9,
            ClusterId::new(),
            2.0,
        );

        let anomaly2 = Anomaly::new(
            EmbeddingId::new(),
            0.3,
            ClusterId::new(),
            0.5,
        );

        repo.save_anomalies(&[anomaly1, anomaly2]).await.unwrap();

        let high_score = repo.find_anomalies_by_score(0.5).await.unwrap();
        assert_eq!(high_score.len(), 1);
    }
}
