//! Compression and abstraction for memory management

use crate::error::Result;
use crate::memory::MemoryService;
use crate::types::{EdgeType, MemoryEdge, MemoryNode, NodeType};

use std::collections::HashMap;
use uuid::Uuid;

/// Cluster of related nodes
#[derive(Debug, Clone)]
pub struct Cluster {
    /// Node IDs in cluster
    pub node_ids: Vec<String>,
    /// Cluster centroid
    pub centroid: Vec<f32>,
    /// Internal density
    pub density: f32,
}

/// Compression service for creating concept hierarchies
pub struct CompressionService {
    /// Minimum cluster size
    min_cluster_size: usize,
    /// Minimum edge density
    min_edge_density: f32,
    /// Summarization prompt template
    summary_template: String,
}

impl CompressionService {
    /// Create a new compression service
    pub fn new(min_cluster_size: usize, min_edge_density: f32) -> Self {
        Self {
            min_cluster_size,
            min_edge_density,
            summary_template: "Summarize the following related concepts:\n\n{texts}".into(),
        }
    }

    /// Detect clusters in the memory graph
    pub async fn detect_clusters(&self, memory: &MemoryService) -> Result<Vec<Cluster>> {
        // Simple clustering based on vector similarity
        // In production, use proper clustering algorithm (HDBSCAN, etc.)

        let clusters = Vec::new();
        // TODO: Implement clustering
        Ok(clusters)
    }

    /// Summarize a cluster into a concept node
    pub fn summarize_cluster(&self, cluster: &Cluster, nodes: &[MemoryNode]) -> Result<MemoryNode> {
        // Collect texts
        let texts: Vec<&str> = nodes
            .iter()
            .filter(|n| cluster.node_ids.contains(&n.id))
            .map(|n| n.text.as_str())
            .collect();

        // Create summary (mock - in production, use LFM2)
        let summary = format!(
            "Concept summarizing {} related items about: {}",
            texts.len(),
            texts.first().unwrap_or(&"various topics")
        );

        // Create concept node
        let concept = MemoryNode {
            id: Uuid::new_v4().to_string(),
            vector: cluster.centroid.clone(),
            text: summary,
            node_type: NodeType::Concept,
            source: "compression".into(),
            metadata: {
                let mut m = HashMap::new();
                m.insert(
                    "cluster_size".into(),
                    serde_json::json!(cluster.node_ids.len()),
                );
                m.insert("density".into(), serde_json::json!(cluster.density));
                m.insert("source_ids".into(), serde_json::json!(cluster.node_ids));
                m
            },
        };

        Ok(concept)
    }

    /// Create hierarchical edges from concept to members
    pub fn create_hierarchy_edges(
        &self,
        concept_id: &str,
        member_ids: &[String],
    ) -> Vec<MemoryEdge> {
        member_ids
            .iter()
            .map(|member_id| MemoryEdge {
                id: Uuid::new_v4().to_string(),
                src: concept_id.to_string(),
                dst: member_id.clone(),
                edge_type: EdgeType::Contains,
                weight: 1.0,
                metadata: HashMap::new(),
            })
            .collect()
    }

    /// Run full compression job
    pub async fn run_compression(&self, memory: &MemoryService) -> Result<CompressionStats> {
        let mut stats = CompressionStats::default();

        // Detect clusters
        let clusters = self.detect_clusters(memory).await?;
        stats.clusters_found = clusters.len();

        // For each cluster, create concept node
        // (In production, would also archive old nodes)

        Ok(stats)
    }
}

/// Statistics from compression run
#[derive(Debug, Default)]
pub struct CompressionStats {
    /// Number of clusters found
    pub clusters_found: usize,
    /// Number of concepts created
    pub concepts_created: usize,
    /// Number of nodes archived
    pub nodes_archived: usize,
    /// Memory saved in bytes
    pub memory_saved: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_service_creation() {
        let service = CompressionService::new(5, 0.5);
        assert_eq!(service.min_cluster_size, 5);
    }

    #[test]
    fn test_hierarchy_edges() {
        let service = CompressionService::new(5, 0.5);
        let edges = service.create_hierarchy_edges(
            "concept-1",
            &["node-1".into(), "node-2".into(), "node-3".into()],
        );

        assert_eq!(edges.len(), 3);
        assert!(edges.iter().all(|e| e.src == "concept-1"));
        assert!(edges.iter().all(|e| e.edge_type == EdgeType::Contains));
    }
}
