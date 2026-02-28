//! Semantic search capabilities for graph queries
//!
//! Combines vector similarity with graph traversal for semantic queries.

use crate::error::{GraphError, Result};
use crate::hybrid::vector_index::HybridIndex;
use crate::types::{EdgeId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for semantic search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchConfig {
    /// Maximum path length for traversal
    pub max_path_length: usize,
    /// Minimum similarity threshold
    pub min_similarity: f32,
    /// Top-k results per hop
    pub top_k: usize,
    /// Weight for semantic similarity vs. graph distance
    pub semantic_weight: f32,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            max_path_length: 3,
            min_similarity: 0.7,
            top_k: 10,
            semantic_weight: 0.6,
        }
    }
}

/// Semantic search engine for graph queries
pub struct SemanticSearch {
    /// Vector index for similarity search
    index: HybridIndex,
    /// Configuration
    config: SemanticSearchConfig,
}

impl SemanticSearch {
    /// Create a new semantic search engine
    pub fn new(index: HybridIndex, config: SemanticSearchConfig) -> Self {
        Self { index, config }
    }

    /// Find nodes semantically similar to query embedding
    pub fn find_similar_nodes(&self, query: &[f32], k: usize) -> Result<Vec<SemanticMatch>> {
        let results = self.index.search_similar_nodes(query, k)?;

        // Pre-compute max distance threshold for faster comparison
        // If min_similarity = 0.7, then max_distance = 0.3
        let max_distance = 1.0 - self.config.min_similarity;

        // HNSW returns distance (0 = identical, 1 = orthogonal for cosine)
        // Convert to similarity (1 = identical, 0 = orthogonal)
        // Use filter_map for single-pass optimization and pre-allocate result
        let mut matches = Vec::with_capacity(results.len());
        for (node_id, distance) in results {
            // Filter by distance threshold (faster than converting and comparing)
            if distance <= max_distance {
                matches.push(SemanticMatch {
                    node_id,
                    score: 1.0 - distance,
                    path_length: 0,
                });
            }
        }
        Ok(matches)
    }

    /// Find semantic paths through the graph
    pub fn find_semantic_paths(
        &self,
        start_node: &NodeId,
        query: &[f32],
        max_paths: usize,
    ) -> Result<Vec<SemanticPath>> {
        // This is a placeholder for the actual graph traversal logic
        // In a real implementation, this would:
        // 1. Start from the given node
        // 2. At each hop, find semantically similar neighbors
        // 3. Continue traversal while similarity > threshold
        // 4. Track paths and score them

        let mut paths = Vec::new();

        // Find similar nodes as potential path endpoints
        let similar = self.find_similar_nodes(query, max_paths)?;

        for match_result in similar {
            paths.push(SemanticPath {
                nodes: vec![start_node.clone(), match_result.node_id],
                edges: vec![],
                semantic_score: match_result.score,
                graph_distance: 1,
                combined_score: self.compute_path_score(match_result.score, 1),
            });
        }

        Ok(paths)
    }

    /// Detect clusters using embeddings
    pub fn detect_clusters(
        &self,
        nodes: &[NodeId],
        min_cluster_size: usize,
    ) -> Result<Vec<ClusterResult>> {
        // This is a placeholder for clustering logic
        // Real implementation would use algorithms like:
        // - DBSCAN on embedding space
        // - Community detection on similarity graph
        // - Hierarchical clustering

        let mut clusters = Vec::new();

        // Simple example: group all nodes as one cluster
        if nodes.len() >= min_cluster_size {
            clusters.push(ClusterResult {
                cluster_id: 0,
                nodes: nodes.to_vec(),
                centroid: None,
                coherence_score: 0.85,
            });
        }

        Ok(clusters)
    }

    /// Find semantically related edges
    pub fn find_related_edges(&self, query: &[f32], k: usize) -> Result<Vec<EdgeMatch>> {
        let results = self.index.search_similar_edges(query, k)?;

        // Pre-compute max distance threshold for faster comparison
        let max_distance = 1.0 - self.config.min_similarity;

        // Convert distance to similarity with single-pass optimization
        let mut matches = Vec::with_capacity(results.len());
        for (edge_id, distance) in results {
            if distance <= max_distance {
                matches.push(EdgeMatch {
                    edge_id,
                    score: 1.0 - distance,
                });
            }
        }
        Ok(matches)
    }

    /// Compute combined score for a path
    fn compute_path_score(&self, semantic_score: f32, graph_distance: usize) -> f32 {
        let w = self.config.semantic_weight;
        let distance_penalty = 1.0 / (graph_distance as f32 + 1.0);

        w * semantic_score + (1.0 - w) * distance_penalty
    }

    /// Expand query using similar terms
    pub fn expand_query(&self, query: &[f32], expansion_factor: usize) -> Result<Vec<Vec<f32>>> {
        // Find similar embeddings to expand the query
        let similar = self.index.search_similar_nodes(query, expansion_factor)?;

        // In a real implementation, we would retrieve the actual embeddings
        // For now, return the original query
        Ok(vec![query.to_vec()])
    }
}

/// Result of a semantic match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub node_id: NodeId,
    pub score: f32,
    pub path_length: usize,
}

/// A semantic path through the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticPath {
    /// Nodes in the path
    pub nodes: Vec<NodeId>,
    /// Edges connecting the nodes
    pub edges: Vec<EdgeId>,
    /// Semantic similarity score
    pub semantic_score: f32,
    /// Graph distance (number of hops)
    pub graph_distance: usize,
    /// Combined score (semantic + distance)
    pub combined_score: f32,
}

/// Result of clustering analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterResult {
    pub cluster_id: usize,
    pub nodes: Vec<NodeId>,
    pub centroid: Option<Vec<f32>>,
    pub coherence_score: f32,
}

/// Match result for edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMatch {
    pub edge_id: EdgeId,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hybrid::vector_index::{EmbeddingConfig, VectorIndexType};

    #[test]
    fn test_semantic_search_creation() {
        let config = EmbeddingConfig::default();
        let index = HybridIndex::new(config).unwrap();
        let search_config = SemanticSearchConfig::default();

        let _search = SemanticSearch::new(index, search_config);
    }

    #[test]
    fn test_find_similar_nodes() -> Result<()> {
        let config = EmbeddingConfig {
            dimensions: 4,
            ..Default::default()
        };
        let index = HybridIndex::new(config)?;
        index.initialize_index(VectorIndexType::Node)?;

        // Add test embeddings
        index.add_node_embedding("doc1".to_string(), vec![1.0, 0.0, 0.0, 0.0])?;
        index.add_node_embedding("doc2".to_string(), vec![0.9, 0.1, 0.0, 0.0])?;

        let search = SemanticSearch::new(index, SemanticSearchConfig::default());
        let results = search.find_similar_nodes(&[1.0, 0.0, 0.0, 0.0], 5)?;

        assert!(!results.is_empty());
        Ok(())
    }

    #[test]
    fn test_cluster_detection() -> Result<()> {
        let config = EmbeddingConfig::default();
        let index = HybridIndex::new(config)?;
        let search = SemanticSearch::new(index, SemanticSearchConfig::default());

        let nodes = vec!["n1".to_string(), "n2".to_string(), "n3".to_string()];
        let clusters = search.detect_clusters(&nodes, 2)?;

        assert_eq!(clusters.len(), 1);
        Ok(())
    }

    #[test]
    fn test_similarity_score_range() -> Result<()> {
        // Verify similarity scores are in [0, 1] range after conversion
        let config = EmbeddingConfig {
            dimensions: 4,
            ..Default::default()
        };
        let index = HybridIndex::new(config)?;
        index.initialize_index(VectorIndexType::Node)?;

        // Add embeddings with varying similarity
        index.add_node_embedding("identical".to_string(), vec![1.0, 0.0, 0.0, 0.0])?;
        index.add_node_embedding("similar".to_string(), vec![0.9, 0.1, 0.0, 0.0])?;
        index.add_node_embedding("different".to_string(), vec![0.0, 1.0, 0.0, 0.0])?;

        let search_config = SemanticSearchConfig {
            min_similarity: 0.0, // Accept all results for this test
            ..Default::default()
        };
        let search = SemanticSearch::new(index, search_config);
        let results = search.find_similar_nodes(&[1.0, 0.0, 0.0, 0.0], 10)?;

        // All scores should be in [0, 1]
        for result in &results {
            assert!(
                result.score >= 0.0 && result.score <= 1.0,
                "Score {} out of valid range [0, 1]",
                result.score
            );
        }

        // Identical vector should have highest similarity (close to 1.0)
        if !results.is_empty() {
            let top_result = &results[0];
            assert!(
                top_result.score > 0.9,
                "Identical vector should have score > 0.9"
            );
        }

        Ok(())
    }

    #[test]
    fn test_min_similarity_filtering() -> Result<()> {
        let config = EmbeddingConfig {
            dimensions: 4,
            ..Default::default()
        };
        let index = HybridIndex::new(config)?;
        index.initialize_index(VectorIndexType::Node)?;

        // Add embeddings
        index.add_node_embedding("high_sim".to_string(), vec![1.0, 0.0, 0.0, 0.0])?;
        index.add_node_embedding("low_sim".to_string(), vec![0.0, 0.0, 0.0, 1.0])?;

        // Set high minimum similarity threshold
        let search_config = SemanticSearchConfig {
            min_similarity: 0.9,
            ..Default::default()
        };
        let search = SemanticSearch::new(index, search_config);
        let results = search.find_similar_nodes(&[1.0, 0.0, 0.0, 0.0], 10)?;

        // Low similarity result should be filtered out
        for result in &results {
            assert!(
                result.score >= 0.9,
                "Result with score {} should be filtered out (min: 0.9)",
                result.score
            );
        }

        Ok(())
    }
}
