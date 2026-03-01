//! Hierarchical Lorentz Attention: Hyperbolic geometry for tree-like structures
//!
//! This mechanism embeds DAG nodes in hyperbolic space using the Lorentz (hyperboloid) model,
//! where hierarchical relationships are naturally represented by distance from the origin.

use super::trait_def::{AttentionError, AttentionScores, DagAttentionMechanism};
use crate::dag::QueryDag;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HierarchicalLorentzConfig {
    /// Curvature parameter (-1.0 for standard Poincaré ball)
    pub curvature: f32,
    /// Scale factor for temporal dimension
    pub time_scale: f32,
    /// Embedding dimension
    pub dim: usize,
    /// Temperature for softmax
    pub temperature: f32,
}

impl Default for HierarchicalLorentzConfig {
    fn default() -> Self {
        Self {
            curvature: -1.0,
            time_scale: 1.0,
            dim: 64,
            temperature: 0.1,
        }
    }
}

pub struct HierarchicalLorentzAttention {
    config: HierarchicalLorentzConfig,
}

impl HierarchicalLorentzAttention {
    pub fn new(config: HierarchicalLorentzConfig) -> Self {
        Self { config }
    }

    /// Lorentz inner product: -x0*y0 + x1*y1 + ... + xn*yn
    fn lorentz_inner(&self, x: &[f32], y: &[f32]) -> f32 {
        if x.is_empty() || y.is_empty() {
            return 0.0;
        }
        -x[0] * y[0] + x[1..].iter().zip(&y[1..]).map(|(a, b)| a * b).sum::<f32>()
    }

    /// Lorentz distance in hyperboloid model
    fn lorentz_distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let inner = self.lorentz_inner(x, y);
        // Clamp to avoid numerical issues with acosh
        let clamped = (-inner).max(1.0);
        clamped.acosh() * self.config.curvature.abs()
    }

    /// Project to hyperboloid: [sqrt(1 + ||x||^2), x1, x2, ..., xn]
    fn project_to_hyperboloid(&self, x: &[f32]) -> Vec<f32> {
        let spatial_norm_sq: f32 = x.iter().map(|v| v * v).sum();
        let time_coord = (1.0 + spatial_norm_sq).sqrt();

        let mut result = Vec::with_capacity(x.len() + 1);
        result.push(time_coord * self.config.time_scale);
        result.extend_from_slice(x);
        result
    }

    /// Compute hierarchical depth for each node
    fn compute_depths(&self, dag: &QueryDag) -> Vec<usize> {
        let n = dag.node_count();
        let mut depths = vec![0; n];
        let mut adj_list: HashMap<usize, Vec<usize>> = HashMap::new();

        // Build adjacency list
        for node_id in dag.node_ids() {
            for &child in dag.children(node_id) {
                adj_list.entry(node_id).or_insert_with(Vec::new).push(child);
            }
        }

        // Find root nodes (nodes with no incoming edges)
        let mut has_incoming = vec![false; n];
        for node_id in dag.node_ids() {
            for &child in dag.children(node_id) {
                if child < n {
                    has_incoming[child] = true;
                }
            }
        }

        // BFS to compute depths
        let mut queue: Vec<usize> = (0..n).filter(|&i| !has_incoming[i]).collect();
        let mut visited = vec![false; n];

        while let Some(node) = queue.pop() {
            if visited[node] {
                continue;
            }
            visited[node] = true;

            if let Some(children) = adj_list.get(&node) {
                for &child in children {
                    if child < n {
                        depths[child] = depths[node] + 1;
                        queue.push(child);
                    }
                }
            }
        }

        depths
    }

    /// Embed node in hyperbolic space based on depth and position
    fn embed_node(&self, node_id: usize, depth: usize, total_nodes: usize) -> Vec<f32> {
        let dim = self.config.dim;
        let mut embedding = vec![0.0; dim];

        // Use depth to determine radial distance from origin
        let radial = (depth as f32 * 0.5).tanh();

        // Angular position based on node ID
        let angle = 2.0 * std::f32::consts::PI * (node_id as f32) / (total_nodes as f32).max(1.0);

        // Spherical coordinates in hyperbolic space
        embedding[0] = radial * angle.cos();
        if dim > 1 {
            embedding[1] = radial * angle.sin();
        }

        // Add noise to remaining dimensions for better separation
        for i in 2..dim {
            embedding[i] = 0.1 * ((node_id + i) as f32).sin();
        }

        embedding
    }

    /// Compute attention using hyperbolic distances
    fn compute_attention_from_distances(&self, distances: &[f32]) -> Vec<f32> {
        if distances.is_empty() {
            return vec![];
        }

        // Convert distances to attention scores using softmax
        // Closer nodes (smaller distance) should have higher attention
        let neg_distances: Vec<f32> = distances
            .iter()
            .map(|&d| -d / self.config.temperature)
            .collect();

        // Softmax
        let max_val = neg_distances
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = neg_distances.iter().map(|&x| (x - max_val).exp()).sum();

        if exp_sum == 0.0 {
            // Uniform distribution if all distances are too large
            return vec![1.0 / distances.len() as f32; distances.len()];
        }

        neg_distances
            .iter()
            .map(|&x| (x - max_val).exp() / exp_sum)
            .collect()
    }
}

impl DagAttentionMechanism for HierarchicalLorentzAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.node_count() == 0 {
            return Err(AttentionError::InvalidDag("Empty DAG".to_string()));
        }

        let n = dag.node_count();

        // Step 1: Compute hierarchical depths
        let depths = self.compute_depths(dag);

        // Step 2: Embed each node in Euclidean space
        let euclidean_embeddings: Vec<Vec<f32>> =
            (0..n).map(|i| self.embed_node(i, depths[i], n)).collect();

        // Step 3: Project to hyperboloid
        let hyperbolic_embeddings: Vec<Vec<f32>> = euclidean_embeddings
            .iter()
            .map(|emb| self.project_to_hyperboloid(emb))
            .collect();

        // Step 4: Compute pairwise distances from a reference point (origin-like)
        let origin = self.project_to_hyperboloid(&vec![0.0; self.config.dim]);
        let distances: Vec<f32> = hyperbolic_embeddings
            .iter()
            .map(|emb| self.lorentz_distance(emb, &origin))
            .collect();

        // Step 5: Convert distances to attention scores
        let scores = self.compute_attention_from_distances(&distances);

        // Step 6: Compute edge weights (optional)
        let mut edge_weights = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let dist =
                    self.lorentz_distance(&hyperbolic_embeddings[i], &hyperbolic_embeddings[j]);
                edge_weights[i][j] = (-dist / self.config.temperature).exp();
            }
        }

        let mut result = AttentionScores::new(scores)
            .with_edge_weights(edge_weights)
            .with_metadata("mechanism".to_string(), "hierarchical_lorentz".to_string());

        result.metadata.insert(
            "avg_depth".to_string(),
            format!("{:.2}", depths.iter().sum::<usize>() as f32 / n as f32),
        );

        Ok(result)
    }

    fn name(&self) -> &'static str {
        "hierarchical_lorentz"
    }

    fn complexity(&self) -> &'static str {
        "O(n²·d)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    #[test]
    fn test_lorentz_distance() {
        let config = HierarchicalLorentzConfig::default();
        let attention = HierarchicalLorentzAttention::new(config);

        let x = vec![1.0, 0.5, 0.3];
        let y = vec![1.2, 0.6, 0.4];

        let dist = attention.lorentz_distance(&x, &y);
        assert!(dist >= 0.0, "Distance should be non-negative");
    }

    #[test]
    fn test_project_to_hyperboloid() {
        let config = HierarchicalLorentzConfig::default();
        let attention = HierarchicalLorentzAttention::new(config);

        let x = vec![0.5, 0.3, 0.2];
        let projected = attention.project_to_hyperboloid(&x);

        assert_eq!(projected.len(), 4);
        assert!(projected[0] > 0.0, "Time coordinate should be positive");
    }

    #[test]
    fn test_hierarchical_attention() {
        let config = HierarchicalLorentzConfig::default();
        let attention = HierarchicalLorentzAttention::new(config);

        let mut dag = QueryDag::new();
        let mut node0 = OperatorNode::new(0, OperatorType::Scan);
        node0.estimated_cost = 1.0;
        dag.add_node(node0);

        let mut node1 = OperatorNode::new(
            1,
            OperatorType::Filter {
                predicate: "x > 0".to_string(),
            },
        );
        node1.estimated_cost = 2.0;
        dag.add_node(node1);

        dag.add_edge(0, 1).unwrap();

        let result = attention.forward(&dag).unwrap();
        assert_eq!(result.scores.len(), 2);
        assert!((result.scores.iter().sum::<f32>() - 1.0).abs() < 0.01);
    }
}
