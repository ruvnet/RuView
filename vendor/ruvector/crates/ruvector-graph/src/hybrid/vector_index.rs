//! Vector indexing for graph elements
//!
//! Integrates RuVector's index (HNSW or Flat) with graph nodes, edges, and hyperedges.

use crate::error::{GraphError, Result};
use crate::types::{EdgeId, NodeId, Properties, PropertyValue};
use dashmap::DashMap;
use parking_lot::RwLock;
use ruvector_core::index::flat::FlatIndex;
#[cfg(feature = "hnsw_rs")]
use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
#[cfg(feature = "hnsw_rs")]
use ruvector_core::types::HnswConfig;
use ruvector_core::types::{DistanceMetric, SearchResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Type of graph element that can be indexed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VectorIndexType {
    /// Node embeddings
    Node,
    /// Edge embeddings
    Edge,
    /// Hyperedge embeddings
    Hyperedge,
}

/// Configuration for embedding storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Dimension of embeddings
    pub dimensions: usize,
    /// Distance metric for similarity
    pub metric: DistanceMetric,
    /// HNSW index configuration (only used when hnsw_rs feature is enabled)
    #[cfg(feature = "hnsw_rs")]
    pub hnsw_config: HnswConfig,
    /// Property name where embeddings are stored
    pub embedding_property: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            dimensions: 384, // Common for small models like MiniLM
            metric: DistanceMetric::Cosine,
            #[cfg(feature = "hnsw_rs")]
            hnsw_config: HnswConfig::default(),
            embedding_property: "embedding".to_string(),
        }
    }
}

// Index type alias based on feature flags
#[cfg(feature = "hnsw_rs")]
type IndexImpl = HnswIndex;
#[cfg(not(feature = "hnsw_rs"))]
type IndexImpl = FlatIndex;

/// Hybrid index combining graph structure with vector search
pub struct HybridIndex {
    /// Node embeddings index
    node_index: Arc<RwLock<Option<IndexImpl>>>,
    /// Edge embeddings index
    edge_index: Arc<RwLock<Option<IndexImpl>>>,
    /// Hyperedge embeddings index
    hyperedge_index: Arc<RwLock<Option<IndexImpl>>>,

    /// Mapping from node IDs to internal vector IDs
    node_id_map: Arc<DashMap<NodeId, String>>,
    /// Mapping from edge IDs to internal vector IDs
    edge_id_map: Arc<DashMap<EdgeId, String>>,
    /// Mapping from hyperedge IDs to internal vector IDs
    hyperedge_id_map: Arc<DashMap<String, String>>,

    /// Configuration
    config: EmbeddingConfig,
}

impl HybridIndex {
    /// Create a new hybrid index
    pub fn new(config: EmbeddingConfig) -> Result<Self> {
        Ok(Self {
            node_index: Arc::new(RwLock::new(None)),
            edge_index: Arc::new(RwLock::new(None)),
            hyperedge_index: Arc::new(RwLock::new(None)),
            node_id_map: Arc::new(DashMap::new()),
            edge_id_map: Arc::new(DashMap::new()),
            hyperedge_id_map: Arc::new(DashMap::new()),
            config,
        })
    }

    /// Initialize index for a specific element type
    #[cfg(feature = "hnsw_rs")]
    pub fn initialize_index(&self, index_type: VectorIndexType) -> Result<()> {
        let index = HnswIndex::new(
            self.config.dimensions,
            self.config.metric,
            self.config.hnsw_config.clone(),
        )
        .map_err(|e| GraphError::IndexError(format!("Failed to create HNSW index: {}", e)))?;

        match index_type {
            VectorIndexType::Node => {
                *self.node_index.write() = Some(index);
            }
            VectorIndexType::Edge => {
                *self.edge_index.write() = Some(index);
            }
            VectorIndexType::Hyperedge => {
                *self.hyperedge_index.write() = Some(index);
            }
        }

        Ok(())
    }

    /// Initialize index for a specific element type (Flat index for WASM)
    #[cfg(not(feature = "hnsw_rs"))]
    pub fn initialize_index(&self, index_type: VectorIndexType) -> Result<()> {
        let index = FlatIndex::new(self.config.dimensions, self.config.metric);

        match index_type {
            VectorIndexType::Node => {
                *self.node_index.write() = Some(index);
            }
            VectorIndexType::Edge => {
                *self.edge_index.write() = Some(index);
            }
            VectorIndexType::Hyperedge => {
                *self.hyperedge_index.write() = Some(index);
            }
        }

        Ok(())
    }

    /// Add node embedding to index
    pub fn add_node_embedding(&self, node_id: NodeId, embedding: Vec<f32>) -> Result<()> {
        if embedding.len() != self.config.dimensions {
            return Err(GraphError::InvalidEmbedding(format!(
                "Expected {} dimensions, got {}",
                self.config.dimensions,
                embedding.len()
            )));
        }

        let mut index_guard = self.node_index.write();
        let index = index_guard
            .as_mut()
            .ok_or_else(|| GraphError::IndexError("Node index not initialized".to_string()))?;

        let vector_id = format!("node_{}", node_id);
        index
            .add(vector_id.clone(), embedding)
            .map_err(|e| GraphError::IndexError(format!("Failed to add node embedding: {}", e)))?;

        self.node_id_map.insert(node_id, vector_id);
        Ok(())
    }

    /// Add edge embedding to index
    pub fn add_edge_embedding(&self, edge_id: EdgeId, embedding: Vec<f32>) -> Result<()> {
        if embedding.len() != self.config.dimensions {
            return Err(GraphError::InvalidEmbedding(format!(
                "Expected {} dimensions, got {}",
                self.config.dimensions,
                embedding.len()
            )));
        }

        let mut index_guard = self.edge_index.write();
        let index = index_guard
            .as_mut()
            .ok_or_else(|| GraphError::IndexError("Edge index not initialized".to_string()))?;

        let vector_id = format!("edge_{}", edge_id);
        index
            .add(vector_id.clone(), embedding)
            .map_err(|e| GraphError::IndexError(format!("Failed to add edge embedding: {}", e)))?;

        self.edge_id_map.insert(edge_id, vector_id);
        Ok(())
    }

    /// Add hyperedge embedding to index
    pub fn add_hyperedge_embedding(&self, hyperedge_id: String, embedding: Vec<f32>) -> Result<()> {
        if embedding.len() != self.config.dimensions {
            return Err(GraphError::InvalidEmbedding(format!(
                "Expected {} dimensions, got {}",
                self.config.dimensions,
                embedding.len()
            )));
        }

        let mut index_guard = self.hyperedge_index.write();
        let index = index_guard
            .as_mut()
            .ok_or_else(|| GraphError::IndexError("Hyperedge index not initialized".to_string()))?;

        let vector_id = format!("hyperedge_{}", hyperedge_id);
        index.add(vector_id.clone(), embedding).map_err(|e| {
            GraphError::IndexError(format!("Failed to add hyperedge embedding: {}", e))
        })?;

        self.hyperedge_id_map.insert(hyperedge_id, vector_id);
        Ok(())
    }

    /// Search for similar nodes
    pub fn search_similar_nodes(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        let index_guard = self.node_index.read();
        let index = index_guard
            .as_ref()
            .ok_or_else(|| GraphError::IndexError("Node index not initialized".to_string()))?;

        let results = index
            .search(query, k)
            .map_err(|e| GraphError::IndexError(format!("Search failed: {}", e)))?;

        Ok(results
            .into_iter()
            .filter_map(|result| {
                // Remove "node_" prefix to get original ID
                let node_id = result.id.strip_prefix("node_")?.to_string();
                Some((node_id, result.score))
            })
            .collect())
    }

    /// Search for similar edges
    pub fn search_similar_edges(&self, query: &[f32], k: usize) -> Result<Vec<(EdgeId, f32)>> {
        let index_guard = self.edge_index.read();
        let index = index_guard
            .as_ref()
            .ok_or_else(|| GraphError::IndexError("Edge index not initialized".to_string()))?;

        let results = index
            .search(query, k)
            .map_err(|e| GraphError::IndexError(format!("Search failed: {}", e)))?;

        Ok(results
            .into_iter()
            .filter_map(|result| {
                let edge_id = result.id.strip_prefix("edge_")?.to_string();
                Some((edge_id, result.score))
            })
            .collect())
    }

    /// Search for similar hyperedges
    pub fn search_similar_hyperedges(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let index_guard = self.hyperedge_index.read();
        let index = index_guard
            .as_ref()
            .ok_or_else(|| GraphError::IndexError("Hyperedge index not initialized".to_string()))?;

        let results = index
            .search(query, k)
            .map_err(|e| GraphError::IndexError(format!("Search failed: {}", e)))?;

        Ok(results
            .into_iter()
            .filter_map(|result| {
                let hyperedge_id = result.id.strip_prefix("hyperedge_")?.to_string();
                Some((hyperedge_id, result.score))
            })
            .collect())
    }

    /// Extract embedding from properties
    pub fn extract_embedding(&self, properties: &Properties) -> Result<Option<Vec<f32>>> {
        let prop_value = match properties.get(&self.config.embedding_property) {
            Some(v) => v,
            None => return Ok(None),
        };

        match prop_value {
            PropertyValue::Array(arr) => {
                let embedding: Result<Vec<f32>> = arr
                    .iter()
                    .map(|v| match v {
                        PropertyValue::Float(f) => Ok(*f as f32),
                        PropertyValue::Integer(i) => Ok(*i as f32),
                        _ => Err(GraphError::InvalidEmbedding(
                            "Embedding array must contain numbers".to_string(),
                        )),
                    })
                    .collect();
                embedding.map(Some)
            }
            _ => Err(GraphError::InvalidEmbedding(
                "Embedding property must be an array".to_string(),
            )),
        }
    }

    /// Get index statistics
    pub fn stats(&self) -> HybridIndexStats {
        let node_count = self.node_id_map.len();
        let edge_count = self.edge_id_map.len();
        let hyperedge_count = self.hyperedge_id_map.len();

        HybridIndexStats {
            node_count,
            edge_count,
            hyperedge_count,
            total_embeddings: node_count + edge_count + hyperedge_count,
        }
    }
}

/// Statistics about the hybrid index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridIndexStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub hyperedge_count: usize,
    pub total_embeddings: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_index_creation() -> Result<()> {
        let config = EmbeddingConfig::default();
        let index = HybridIndex::new(config)?;

        let stats = index.stats();
        assert_eq!(stats.total_embeddings, 0);

        Ok(())
    }

    #[test]
    fn test_node_embedding_indexing() -> Result<()> {
        let config = EmbeddingConfig {
            dimensions: 4,
            ..Default::default()
        };
        let index = HybridIndex::new(config)?;
        index.initialize_index(VectorIndexType::Node)?;

        let embedding = vec![1.0, 2.0, 3.0, 4.0];
        index.add_node_embedding("node1".to_string(), embedding)?;

        let stats = index.stats();
        assert_eq!(stats.node_count, 1);

        Ok(())
    }

    #[test]
    fn test_similarity_search() -> Result<()> {
        let config = EmbeddingConfig {
            dimensions: 4,
            ..Default::default()
        };
        let index = HybridIndex::new(config)?;
        index.initialize_index(VectorIndexType::Node)?;

        // Add some embeddings
        index.add_node_embedding("node1".to_string(), vec![1.0, 0.0, 0.0, 0.0])?;
        index.add_node_embedding("node2".to_string(), vec![0.9, 0.1, 0.0, 0.0])?;
        index.add_node_embedding("node3".to_string(), vec![0.0, 1.0, 0.0, 0.0])?;

        // Search for similar to node1
        let results = index.search_similar_nodes(&[1.0, 0.0, 0.0, 0.0], 2)?;

        assert!(results.len() <= 2);
        if !results.is_empty() {
            assert_eq!(results[0].0, "node1");
        }

        Ok(())
    }
}
