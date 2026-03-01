//! Cluster analysis service.
//!
//! This module provides the `ClusterEngine` service for discovering
//! and managing clusters of similar segments.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use super::{ClusterData, SegmentEmbedding};

/// Cluster analysis error.
#[derive(Debug, Error)]
pub enum AnalysisError {
    /// Clustering error
    #[error("Clustering failed: {0}")]
    ClusteringError(String),

    /// Invalid parameters
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Cluster engine configuration.
#[derive(Debug, Clone)]
pub struct ClusterEngineConfig {
    /// Minimum cluster size
    pub min_cluster_size: usize,
    /// HDBSCAN min_samples
    pub min_samples: usize,
    /// Distance threshold for merging
    pub merge_threshold: f32,
}

impl Default for ClusterEngineConfig {
    fn default() -> Self {
        Self {
            min_cluster_size: 5,
            min_samples: 3,
            merge_threshold: 0.15,
        }
    }
}

/// Cluster analysis engine.
///
/// Manages cluster discovery, labeling, and updates.
pub struct ClusterEngine {
    config: ClusterEngineConfig,
    // In-memory cluster storage for stub implementation
    clusters: RwLock<HashMap<Uuid, ClusterData>>,
}

impl ClusterEngine {
    /// Create a new cluster engine with the given configuration.
    pub fn new(config: ClusterEngineConfig) -> Result<Self, AnalysisError> {
        Ok(Self {
            config,
            clusters: RwLock::new(HashMap::new()),
        })
    }

    /// Update clusters with new embeddings.
    pub fn update_clusters(&self, _embeddings: &[SegmentEmbedding]) -> Result<(), AnalysisError> {
        // In a real implementation, this would:
        // 1. Run HDBSCAN or similar clustering
        // 2. Merge with existing clusters if similar
        // 3. Update cluster centroids and metadata

        // For the stub, we don't create clusters automatically
        Ok(())
    }

    /// Get all clusters.
    pub fn get_all_clusters(&self) -> Result<Vec<ClusterData>, AnalysisError> {
        let clusters = self
            .clusters
            .read()
            .map_err(|e| AnalysisError::Internal(e.to_string()))?;

        Ok(clusters.values().cloned().collect())
    }

    /// Get a specific cluster by ID.
    pub fn get_cluster(&self, id: &Uuid) -> Result<Option<ClusterData>, AnalysisError> {
        let clusters = self
            .clusters
            .read()
            .map_err(|e| AnalysisError::Internal(e.to_string()))?;

        Ok(clusters.get(id).cloned())
    }

    /// Assign a label to a cluster.
    pub fn assign_label(
        &self,
        cluster_id: &Uuid,
        label: &str,
    ) -> Result<Option<ClusterData>, AnalysisError> {
        let mut clusters = self
            .clusters
            .write()
            .map_err(|e| AnalysisError::Internal(e.to_string()))?;

        if let Some(cluster) = clusters.get_mut(cluster_id) {
            cluster.label = Some(label.to_string());
            Ok(Some(cluster.clone()))
        } else {
            Ok(None)
        }
    }

    /// Create a new cluster manually.
    pub fn create_cluster(
        &self,
        centroid: Vec<f32>,
        exemplar_ids: Vec<Uuid>,
    ) -> Result<ClusterData, AnalysisError> {
        let cluster = ClusterData {
            id: Uuid::new_v4(),
            label: None,
            size: exemplar_ids.len(),
            centroid,
            density: 0.0,
            exemplar_ids,
            species_distribution: vec![],
            created_at: Utc::now(),
        };

        let mut clusters = self
            .clusters
            .write()
            .map_err(|e| AnalysisError::Internal(e.to_string()))?;

        clusters.insert(cluster.id, cluster.clone());

        Ok(cluster)
    }

    /// Delete a cluster.
    pub fn delete_cluster(&self, id: &Uuid) -> Result<bool, AnalysisError> {
        let mut clusters = self
            .clusters
            .write()
            .map_err(|e| AnalysisError::Internal(e.to_string()))?;

        Ok(clusters.remove(id).is_some())
    }

    /// Merge two clusters.
    pub fn merge_clusters(
        &self,
        cluster_a: &Uuid,
        cluster_b: &Uuid,
    ) -> Result<ClusterData, AnalysisError> {
        let mut clusters = self
            .clusters
            .write()
            .map_err(|e| AnalysisError::Internal(e.to_string()))?;

        let a = clusters
            .remove(cluster_a)
            .ok_or_else(|| AnalysisError::NotFound(format!("Cluster {} not found", cluster_a)))?;

        let b = clusters
            .remove(cluster_b)
            .ok_or_else(|| AnalysisError::NotFound(format!("Cluster {} not found", cluster_b)))?;

        // Merge exemplar IDs
        let mut merged_exemplars = a.exemplar_ids;
        merged_exemplars.extend(b.exemplar_ids);

        // Average centroids (simplified)
        let merged_centroid: Vec<f32> = a
            .centroid
            .iter()
            .zip(b.centroid.iter())
            .map(|(x, y)| (x + y) / 2.0)
            .collect();

        let merged = ClusterData {
            id: Uuid::new_v4(),
            label: a.label.or(b.label),
            size: a.size + b.size,
            centroid: merged_centroid,
            density: (a.density + b.density) / 2.0,
            exemplar_ids: merged_exemplars,
            species_distribution: vec![], // Would recompute
            created_at: Utc::now(),
        };

        clusters.insert(merged.id, merged.clone());

        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_engine_creation() {
        let engine = ClusterEngine::new(Default::default());
        assert!(engine.is_ok());
    }

    #[test]
    fn test_create_and_get_cluster() {
        let engine = ClusterEngine::new(Default::default()).unwrap();

        let cluster = engine
            .create_cluster(vec![0.0; 1024], vec![Uuid::new_v4()])
            .unwrap();

        let retrieved = engine.get_cluster(&cluster.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, cluster.id);
    }

    #[test]
    fn test_assign_label() {
        let engine = ClusterEngine::new(Default::default()).unwrap();

        let cluster = engine
            .create_cluster(vec![0.0; 1024], vec![Uuid::new_v4()])
            .unwrap();

        let updated = engine
            .assign_label(&cluster.id, "Test Label")
            .unwrap()
            .unwrap();

        assert_eq!(updated.label, Some("Test Label".to_string()));
    }
}
