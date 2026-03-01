//! Stage III Encoder: Dimensional Data via GNN Graph Topology
//!
//! CRV Stage III captures spatial sketches and geometric relationships.
//! These naturally form a graph where sketch elements are nodes and spatial
//! relationships are edges. The GNN layer learns to propagate spatial
//! context through the graph, producing an embedding that captures the
//! full dimensional structure of the target.
//!
//! # Architecture
//!
//! Sketch elements → node features, spatial relationships → edge weights.
//! A GNN forward pass aggregates neighborhood information to produce
//! a graph-level embedding.

use crate::error::{CrvError, CrvResult};
use crate::types::{CrvConfig, GeometricKind, SpatialRelationType, StageIIIData};
use ruvector_gnn::layer::RuvectorLayer;
use ruvector_gnn::search::cosine_similarity;

/// Stage III encoder using GNN graph topology.
#[derive(Debug)]
pub struct StageIIIEncoder {
    /// Embedding dimensionality.
    dim: usize,
    /// GNN layer for spatial message passing.
    gnn_layer: RuvectorLayer,
}

impl StageIIIEncoder {
    /// Create a new Stage III encoder.
    pub fn new(config: &CrvConfig) -> Self {
        let dim = config.dimensions;
        // Single GNN layer: input_dim -> hidden_dim, 1 head
        // heads=1 always divides any dim, and dropout=0.0 is always valid
        let gnn_layer =
            RuvectorLayer::new(dim, dim, 1, 0.0).expect("dim is always divisible by 1 head");

        Self { dim, gnn_layer }
    }

    /// Encode a sketch element into a node feature vector.
    fn encode_element(
        &self,
        label: &str,
        kind: GeometricKind,
        position: (f32, f32),
        scale: Option<f32>,
    ) -> Vec<f32> {
        let mut features = vec![0.0f32; self.dim];

        // Geometric kind encoding (one-hot style in first 8 dims)
        let kind_idx = match kind {
            GeometricKind::Point => 0,
            GeometricKind::Line => 1,
            GeometricKind::Curve => 2,
            GeometricKind::Rectangle => 3,
            GeometricKind::Circle => 4,
            GeometricKind::Triangle => 5,
            GeometricKind::Polygon => 6,
            GeometricKind::Freeform => 7,
        };
        if kind_idx < self.dim {
            features[kind_idx] = 1.0;
        }

        // Position encoding (normalized)
        if 8 < self.dim {
            features[8] = position.0;
        }
        if 9 < self.dim {
            features[9] = position.1;
        }

        // Scale encoding
        if let Some(s) = scale {
            if 10 < self.dim {
                features[10] = s;
            }
        }

        // Label hash encoding (spread across remaining dims)
        for (i, byte) in label.bytes().enumerate() {
            let idx = 11 + (i % (self.dim.saturating_sub(11)));
            if idx < self.dim {
                features[idx] += (byte as f32 / 255.0) * 0.5;
            }
        }

        // L2 normalize
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }

    /// Compute edge weight from spatial relationship type.
    fn relationship_weight(relation: SpatialRelationType) -> f32 {
        match relation {
            SpatialRelationType::Adjacent => 0.8,
            SpatialRelationType::Contains => 0.9,
            SpatialRelationType::Above => 0.6,
            SpatialRelationType::Below => 0.6,
            SpatialRelationType::Inside => 0.95,
            SpatialRelationType::Surrounding => 0.85,
            SpatialRelationType::Connected => 0.7,
            SpatialRelationType::Separated => 0.3,
        }
    }

    /// Encode Stage III data into a graph-level embedding.
    ///
    /// Builds a graph from sketch elements and relationships,
    /// runs GNN message passing, then aggregates node embeddings
    /// into a single graph-level vector.
    pub fn encode(&self, data: &StageIIIData) -> CrvResult<Vec<f32>> {
        if data.sketch_elements.is_empty() {
            return Err(CrvError::EmptyInput("No sketch elements".to_string()));
        }

        // Build label → index mapping
        let label_to_idx: std::collections::HashMap<&str, usize> = data
            .sketch_elements
            .iter()
            .enumerate()
            .map(|(i, elem)| (elem.label.as_str(), i))
            .collect();

        // Encode each element as a node feature vector
        let node_features: Vec<Vec<f32>> = data
            .sketch_elements
            .iter()
            .map(|elem| self.encode_element(&elem.label, elem.kind, elem.position, elem.scale))
            .collect();

        // For each node, collect neighbor embeddings and edge weights
        // based on the spatial relationships
        let mut aggregated = vec![vec![0.0f32; self.dim]; node_features.len()];

        for (node_idx, node_feat) in node_features.iter().enumerate() {
            let label = &data.sketch_elements[node_idx].label;

            // Find all relationships involving this node
            let mut neighbor_feats = Vec::new();
            let mut edge_weights = Vec::new();

            for rel in &data.relationships {
                if rel.from == *label {
                    if let Some(&neighbor_idx) = label_to_idx.get(rel.to.as_str()) {
                        neighbor_feats.push(node_features[neighbor_idx].clone());
                        edge_weights.push(Self::relationship_weight(rel.relation) * rel.strength);
                    }
                } else if rel.to == *label {
                    if let Some(&neighbor_idx) = label_to_idx.get(rel.from.as_str()) {
                        neighbor_feats.push(node_features[neighbor_idx].clone());
                        edge_weights.push(Self::relationship_weight(rel.relation) * rel.strength);
                    }
                }
            }

            // GNN forward pass for this node
            aggregated[node_idx] =
                self.gnn_layer
                    .forward(node_feat, &neighbor_feats, &edge_weights);
        }

        // Aggregate into graph-level embedding via mean pooling
        let mut graph_embedding = vec![0.0f32; self.dim];
        for node_emb in &aggregated {
            for (i, &v) in node_emb.iter().enumerate() {
                if i < self.dim {
                    graph_embedding[i] += v;
                }
            }
        }

        let n = aggregated.len() as f32;
        for v in &mut graph_embedding {
            *v /= n;
        }

        Ok(graph_embedding)
    }

    /// Compute similarity between two Stage III embeddings.
    pub fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        cosine_similarity(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SketchElement, SpatialRelationship};

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 32,
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_encoder_creation() {
        let config = test_config();
        let encoder = StageIIIEncoder::new(&config);
        assert_eq!(encoder.dim, 32);
    }

    #[test]
    fn test_element_encoding() {
        let config = test_config();
        let encoder = StageIIIEncoder::new(&config);

        let features =
            encoder.encode_element("building", GeometricKind::Rectangle, (0.5, 0.3), Some(2.0));
        assert_eq!(features.len(), 32);
    }

    #[test]
    fn test_full_encode() {
        let config = test_config();
        let encoder = StageIIIEncoder::new(&config);

        let data = StageIIIData {
            sketch_elements: vec![
                SketchElement {
                    label: "tower".to_string(),
                    kind: GeometricKind::Rectangle,
                    position: (0.5, 0.8),
                    scale: Some(3.0),
                },
                SketchElement {
                    label: "base".to_string(),
                    kind: GeometricKind::Rectangle,
                    position: (0.5, 0.2),
                    scale: Some(5.0),
                },
                SketchElement {
                    label: "path".to_string(),
                    kind: GeometricKind::Line,
                    position: (0.3, 0.1),
                    scale: None,
                },
            ],
            relationships: vec![
                SpatialRelationship {
                    from: "tower".to_string(),
                    to: "base".to_string(),
                    relation: SpatialRelationType::Above,
                    strength: 0.9,
                },
                SpatialRelationship {
                    from: "path".to_string(),
                    to: "base".to_string(),
                    relation: SpatialRelationType::Adjacent,
                    strength: 0.7,
                },
            ],
        };

        let embedding = encoder.encode(&data).unwrap();
        assert_eq!(embedding.len(), 32);
    }

    #[test]
    fn test_empty_elements() {
        let config = test_config();
        let encoder = StageIIIEncoder::new(&config);

        let data = StageIIIData {
            sketch_elements: vec![],
            relationships: vec![],
        };

        assert!(encoder.encode(&data).is_err());
    }
}
