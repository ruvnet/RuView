//! Repository traits for the Analysis bounded context.
//!
//! These traits define the persistence interfaces for domain entities.
//! Implementations are provided in the infrastructure layer.

use async_trait::async_trait;
use thiserror::Error;

use super::entities::{
    Anomaly, Cluster, ClusterId, EmbeddingId, Motif, Prototype, RecordingId, SequenceAnalysis,
};

/// Errors that can occur during repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Entity not found.
    #[error("Entity not found: {0}")]
    NotFound(String),

    /// Duplicate entity.
    #[error("Duplicate entity: {0}")]
    Duplicate(String),

    /// Database connection error.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Query execution error.
    #[error("Query error: {0}")]
    QueryError(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid data error.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Concurrency conflict.
    #[error("Concurrency conflict: {0}")]
    ConcurrencyError(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for repository operations.
pub type Result<T> = std::result::Result<T, RepositoryError>;

/// Repository for cluster persistence.
#[async_trait]
pub trait ClusterRepository: Send + Sync {
    /// Save a cluster to the repository.
    async fn save_cluster(&self, cluster: &Cluster) -> Result<()>;

    /// Save multiple clusters in a batch.
    async fn save_clusters(&self, clusters: &[Cluster]) -> Result<()>;

    /// Find a cluster by its ID.
    async fn find_cluster(&self, id: &ClusterId) -> Result<Option<Cluster>>;

    /// List all clusters.
    async fn list_clusters(&self) -> Result<Vec<Cluster>>;

    /// List clusters with pagination.
    async fn list_clusters_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Cluster>>;

    /// Assign an embedding to a cluster.
    async fn assign_to_cluster(
        &self,
        embedding_id: &EmbeddingId,
        cluster_id: &ClusterId,
    ) -> Result<()>;

    /// Remove an embedding from its cluster.
    async fn remove_from_cluster(&self, embedding_id: &EmbeddingId) -> Result<()>;

    /// Find the cluster containing a specific embedding.
    async fn find_cluster_by_embedding(
        &self,
        embedding_id: &EmbeddingId,
    ) -> Result<Option<Cluster>>;

    /// Delete a cluster.
    async fn delete_cluster(&self, id: &ClusterId) -> Result<()>;

    /// Delete all clusters.
    async fn delete_all_clusters(&self) -> Result<()>;

    /// Get cluster count.
    async fn cluster_count(&self) -> Result<usize>;

    /// Find clusters by label pattern.
    async fn find_clusters_by_label(&self, label_pattern: &str) -> Result<Vec<Cluster>>;

    /// Update cluster label.
    async fn update_cluster_label(
        &self,
        id: &ClusterId,
        label: Option<String>,
    ) -> Result<()>;
}

/// Repository for prototype persistence.
#[async_trait]
pub trait PrototypeRepository: Send + Sync {
    /// Save a prototype.
    async fn save_prototype(&self, prototype: &Prototype) -> Result<()>;

    /// Save multiple prototypes in a batch.
    async fn save_prototypes(&self, prototypes: &[Prototype]) -> Result<()>;

    /// Find prototypes for a cluster.
    async fn find_prototypes_by_cluster(
        &self,
        cluster_id: &ClusterId,
    ) -> Result<Vec<Prototype>>;

    /// Find the best prototype for a cluster.
    async fn find_best_prototype(
        &self,
        cluster_id: &ClusterId,
    ) -> Result<Option<Prototype>>;

    /// Delete prototypes for a cluster.
    async fn delete_prototypes_by_cluster(&self, cluster_id: &ClusterId) -> Result<()>;

    /// Delete all prototypes.
    async fn delete_all_prototypes(&self) -> Result<()>;
}

/// Repository for motif persistence.
#[async_trait]
pub trait MotifRepository: Send + Sync {
    /// Save a motif.
    async fn save_motif(&self, motif: &Motif) -> Result<()>;

    /// Save multiple motifs in a batch.
    async fn save_motifs(&self, motifs: &[Motif]) -> Result<()>;

    /// Find a motif by its ID.
    async fn find_motif(&self, id: &str) -> Result<Option<Motif>>;

    /// Find motifs containing a specific cluster.
    async fn find_motifs_by_cluster(&self, cluster_id: &ClusterId) -> Result<Vec<Motif>>;

    /// List all motifs.
    async fn list_motifs(&self) -> Result<Vec<Motif>>;

    /// List motifs with minimum confidence.
    async fn find_motifs_by_confidence(&self, min_confidence: f32) -> Result<Vec<Motif>>;

    /// List motifs with minimum occurrences.
    async fn find_motifs_by_occurrences(&self, min_occurrences: usize) -> Result<Vec<Motif>>;

    /// Delete a motif.
    async fn delete_motif(&self, id: &str) -> Result<()>;

    /// Delete all motifs.
    async fn delete_all_motifs(&self) -> Result<()>;

    /// Get motif count.
    async fn motif_count(&self) -> Result<usize>;

    /// Find motifs by sequence pattern (exact match).
    async fn find_motifs_by_sequence(&self, sequence: &[ClusterId]) -> Result<Vec<Motif>>;

    /// Find motifs by sequence pattern (subsequence match).
    async fn find_motifs_containing_subsequence(
        &self,
        subsequence: &[ClusterId],
    ) -> Result<Vec<Motif>>;
}

/// Repository for sequence analysis persistence.
#[async_trait]
pub trait SequenceRepository: Send + Sync {
    /// Save a sequence analysis.
    async fn save_sequence_analysis(&self, analysis: &SequenceAnalysis) -> Result<()>;

    /// Find sequence analysis for a recording.
    async fn find_sequence_by_recording(
        &self,
        recording_id: &RecordingId,
    ) -> Result<Option<SequenceAnalysis>>;

    /// List all sequence analyses.
    async fn list_sequence_analyses(&self) -> Result<Vec<SequenceAnalysis>>;

    /// Delete sequence analysis for a recording.
    async fn delete_sequence_by_recording(&self, recording_id: &RecordingId) -> Result<()>;

    /// Delete all sequence analyses.
    async fn delete_all_sequences(&self) -> Result<()>;

    /// Find sequences with entropy above threshold.
    async fn find_sequences_by_entropy(&self, min_entropy: f32) -> Result<Vec<SequenceAnalysis>>;

    /// Find sequences with stereotypy above threshold.
    async fn find_sequences_by_stereotypy(
        &self,
        min_stereotypy: f32,
    ) -> Result<Vec<SequenceAnalysis>>;
}

/// Repository for anomaly persistence.
#[async_trait]
pub trait AnomalyRepository: Send + Sync {
    /// Save an anomaly.
    async fn save_anomaly(&self, anomaly: &Anomaly) -> Result<()>;

    /// Save multiple anomalies in a batch.
    async fn save_anomalies(&self, anomalies: &[Anomaly]) -> Result<()>;

    /// Find an anomaly by embedding ID.
    async fn find_anomaly(&self, embedding_id: &EmbeddingId) -> Result<Option<Anomaly>>;

    /// List all anomalies.
    async fn list_anomalies(&self) -> Result<Vec<Anomaly>>;

    /// Find anomalies with score above threshold.
    async fn find_anomalies_by_score(&self, min_score: f32) -> Result<Vec<Anomaly>>;

    /// Find anomalies near a specific cluster.
    async fn find_anomalies_by_cluster(&self, cluster_id: &ClusterId) -> Result<Vec<Anomaly>>;

    /// Delete an anomaly.
    async fn delete_anomaly(&self, embedding_id: &EmbeddingId) -> Result<()>;

    /// Delete all anomalies.
    async fn delete_all_anomalies(&self) -> Result<()>;

    /// Get anomaly count.
    async fn anomaly_count(&self) -> Result<usize>;
}

/// Combined repository for all analysis entities.
///
/// This trait combines all individual repositories for convenience
/// when a single interface to all analysis data is needed.
#[async_trait]
pub trait AnalysisRepository:
    ClusterRepository + PrototypeRepository + MotifRepository + SequenceRepository + AnomalyRepository
{
    /// Clear all analysis data.
    async fn clear_all(&self) -> Result<()> {
        self.delete_all_clusters().await?;
        self.delete_all_prototypes().await?;
        self.delete_all_motifs().await?;
        self.delete_all_sequences().await?;
        self.delete_all_anomalies().await?;
        Ok(())
    }
}

/// Unit of work for transactional operations.
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Type of repository returned by this unit of work.
    type Repository: AnalysisRepository;

    /// Begin a new transaction and return a repository.
    async fn begin(&self) -> Result<Self::Repository>;

    /// Commit the current transaction.
    async fn commit(&self) -> Result<()>;

    /// Rollback the current transaction.
    async fn rollback(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_error_display() {
        let err = RepositoryError::NotFound("cluster-123".to_string());
        assert!(format!("{}", err).contains("cluster-123"));

        let err = RepositoryError::QueryError("syntax error".to_string());
        assert!(format!("{}", err).contains("syntax error"));
    }
}
