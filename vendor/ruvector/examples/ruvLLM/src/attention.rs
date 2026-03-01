//! Multi-head graph attention engine with edge features
//!
//! Implements graph attention mechanism that considers both node embeddings
//! and edge features for context ranking in RAG.

use crate::config::EmbeddingConfig;
use crate::error::Result;
use crate::memory::SubGraph;
use crate::types::{EdgeType, MemoryNode};

use ndarray::{Array1, Array2};
use rand::Rng;
use rayon::prelude::*;
use std::collections::HashMap;

/// Graph context after attention
#[derive(Debug, Clone)]
pub struct GraphContext {
    /// Output embedding (combined from attention)
    pub embedding: Vec<f32>,
    /// Nodes ranked by attention
    pub ranked_nodes: Vec<MemoryNode>,
    /// Attention weights for ranked nodes
    pub attention_weights: Vec<f32>,
    /// Per-head attention weights (for analysis)
    pub head_weights: Vec<Vec<f32>>,
    /// Summary statistics
    pub summary: GraphSummary,
}

/// Summary of graph attention
#[derive(Debug, Clone, Default)]
pub struct GraphSummary {
    /// Number of nodes attended
    pub num_nodes: usize,
    /// Number of edges
    pub num_edges: usize,
    /// Attention entropy (higher = more diffuse attention)
    pub attention_entropy: f32,
    /// Mean attention weight
    pub mean_attention: f32,
    /// Attention concentration (Gini coefficient)
    pub gini_coefficient: f32,
    /// Edge influence score
    pub edge_influence: f32,
}

/// Multi-head graph attention engine
pub struct GraphAttentionEngine {
    /// Embedding dimension
    dim: usize,
    /// Number of attention heads
    num_heads: usize,
    /// Head dimension
    head_dim: usize,
    /// Query projection matrices (per head)
    wq: Vec<Array2<f32>>,
    /// Key projection matrices (per head)
    wk: Vec<Array2<f32>>,
    /// Value projection matrices (per head)
    wv: Vec<Array2<f32>>,
    /// Output projection
    wo: Array2<f32>,
    /// Edge type embeddings
    edge_embeddings: HashMap<EdgeType, Array1<f32>>,
    /// Edge feature dimension
    edge_dim: usize,
    /// Layer normalization gamma
    ln_gamma: Array1<f32>,
    /// Layer normalization beta
    ln_beta: Array1<f32>,
    /// Temperature for attention scaling
    temperature: f32,
}

impl GraphAttentionEngine {
    /// Create a new graph attention engine
    pub fn new(config: &EmbeddingConfig) -> Result<Self> {
        let dim = config.dimension;
        let num_heads = 8;
        let head_dim = dim / num_heads;
        let edge_dim = 32;

        let mut rng = rand::thread_rng();
        let scale = (2.0 / (dim + head_dim) as f32).sqrt();

        // Initialize projection matrices for each head
        let mut wq = Vec::with_capacity(num_heads);
        let mut wk = Vec::with_capacity(num_heads);
        let mut wv = Vec::with_capacity(num_heads);

        for _ in 0..num_heads {
            wq.push(random_matrix(&mut rng, dim, head_dim, scale));
            wk.push(random_matrix(&mut rng, dim, head_dim, scale));
            wv.push(random_matrix(&mut rng, dim, head_dim, scale));
        }

        // Output projection
        let wo = random_matrix(&mut rng, dim, dim, scale);

        // Edge type embeddings
        let mut edge_embeddings = HashMap::new();
        for edge_type in [
            EdgeType::Cites,
            EdgeType::Follows,
            EdgeType::SameTopic,
            EdgeType::AgentStep,
            EdgeType::Derived,
            EdgeType::Contains,
        ] {
            edge_embeddings.insert(edge_type, random_vector(&mut rng, edge_dim));
        }

        // Layer norm parameters
        let ln_gamma = Array1::ones(dim);
        let ln_beta = Array1::zeros(dim);

        Ok(Self {
            dim,
            num_heads,
            head_dim,
            wq,
            wk,
            wv,
            wo,
            edge_embeddings,
            edge_dim,
            ln_gamma,
            ln_beta,
            temperature: 1.0,
        })
    }

    /// Set attention temperature
    pub fn set_temperature(&mut self, temp: f32) {
        self.temperature = temp.max(0.01);
    }

    /// Attend over subgraph with multi-head attention
    pub fn attend(&self, query: &[f32], subgraph: &SubGraph) -> Result<GraphContext> {
        if subgraph.nodes.is_empty() {
            return Ok(GraphContext {
                embedding: query.to_vec(),
                ranked_nodes: vec![],
                attention_weights: vec![],
                head_weights: vec![],
                summary: GraphSummary::default(),
            });
        }

        let n = subgraph.nodes.len();
        let query_arr = Array1::from_vec(query.to_vec());

        // Build edge feature matrix
        let edge_features = self.build_edge_features(subgraph);

        // Compute multi-head attention in parallel
        let head_results: Vec<(Vec<f32>, Array1<f32>)> = (0..self.num_heads)
            .into_par_iter()
            .map(|head| {
                // Project query
                let q = self.wq[head].t().dot(&query_arr);

                // Project all node keys and values
                let mut keys = Array2::zeros((n, self.head_dim));
                let mut values = Array2::zeros((n, self.head_dim));

                for (i, node) in subgraph.nodes.iter().enumerate() {
                    let node_vec = Array1::from_vec(node.vector.clone());
                    let k = self.wk[head].t().dot(&node_vec);
                    let v = self.wv[head].t().dot(&node_vec);
                    keys.row_mut(i).assign(&k);
                    values.row_mut(i).assign(&v);
                }

                // Compute attention scores: Q @ K^T / sqrt(d)
                let mut scores: Vec<f32> = Vec::with_capacity(n);
                let scale_factor = (self.head_dim as f32).sqrt() * self.temperature;
                for i in 0..n {
                    let k = keys.row(i);
                    scores.push(q.dot(&k) / scale_factor);
                }

                // Add edge-based bias
                for i in 0..n {
                    if let Some(edge_feat) = edge_features.get(&subgraph.nodes[i].id) {
                        let bias = edge_feat.iter().sum::<f32>() / edge_feat.len() as f32 * 0.1;
                        scores[i] += bias;
                    }
                }

                // Softmax
                let weights = softmax(&scores);

                // Weighted sum of values
                let mut output = Array1::zeros(self.head_dim);
                for (i, &w) in weights.iter().enumerate() {
                    if w > 1e-6 {
                        // Skip near-zero weights
                        output = output + &values.row(i).to_owned() * w;
                    }
                }

                (weights, output)
            })
            .collect();

        let (all_head_weights, head_outputs): (Vec<Vec<f32>>, Vec<Array1<f32>>) =
            head_results.into_iter().unzip();

        // Concatenate heads
        let mut concat = Array1::zeros(self.dim);
        for (h, output) in head_outputs.iter().enumerate() {
            for (i, &v) in output.iter().enumerate() {
                concat[h * self.head_dim + i] = v;
            }
        }

        // Output projection
        let projected = self.wo.t().dot(&concat);

        // Add residual and layer norm
        let residual = &query_arr + &projected;
        let output = layer_norm(&residual, &self.ln_gamma, &self.ln_beta);

        // Average attention weights across heads
        let avg_weights = average_weights(&all_head_weights);

        // Rank nodes by attention
        let mut indexed: Vec<(usize, f32)> = avg_weights
            .iter()
            .enumerate()
            .map(|(i, &w)| (i, w))
            .collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let ranked_nodes: Vec<MemoryNode> = indexed
            .iter()
            .map(|(i, _)| subgraph.nodes[*i].clone())
            .collect();
        let ranked_weights: Vec<f32> = indexed.iter().map(|(_, w)| *w).collect();

        // Compute summary statistics
        let summary = GraphSummary {
            num_nodes: n,
            num_edges: subgraph.edges.len(),
            attention_entropy: entropy(&avg_weights),
            mean_attention: avg_weights.iter().sum::<f32>() / n as f32,
            gini_coefficient: gini_coefficient(&avg_weights),
            edge_influence: self.compute_edge_influence(subgraph, &avg_weights),
        };

        Ok(GraphContext {
            embedding: output.to_vec(),
            ranked_nodes,
            attention_weights: ranked_weights,
            head_weights: all_head_weights,
            summary,
        })
    }

    /// Attend with cross-attention (query attends to memory, memory attends to query)
    pub fn cross_attend(
        &self,
        query: &[f32],
        subgraph: &SubGraph,
    ) -> Result<(GraphContext, Vec<f32>)> {
        // Forward attention: query -> memory
        let forward_ctx = self.attend(query, subgraph)?;

        // Backward attention: memory -> query (simplified)
        // Each node's "attention" to the query
        let mut backward_weights = Vec::with_capacity(subgraph.nodes.len());
        let query_arr = Array1::from_vec(query.to_vec());

        for node in &subgraph.nodes {
            let node_arr = Array1::from_vec(node.vector.clone());
            let score = node_arr.dot(&query_arr) / (self.dim as f32).sqrt();
            backward_weights.push(score);
        }
        let backward_weights = softmax(&backward_weights);

        Ok((forward_ctx, backward_weights))
    }

    /// Build edge features for each node
    fn build_edge_features(&self, subgraph: &SubGraph) -> HashMap<String, Vec<f32>> {
        let mut features: HashMap<String, Vec<f32>> = HashMap::new();

        for edge in &subgraph.edges {
            // Get edge type embedding
            let edge_emb = self
                .edge_embeddings
                .get(&edge.edge_type)
                .map(|e| e.to_vec())
                .unwrap_or_else(|| vec![0.0; self.edge_dim]);

            // Add to source node's features
            let src_features = features
                .entry(edge.src.clone())
                .or_insert_with(|| vec![0.0; self.edge_dim]);
            for (i, v) in edge_emb.iter().enumerate() {
                src_features[i] += v * edge.weight;
            }

            // Add to destination node's features (incoming edge)
            let dst_features = features
                .entry(edge.dst.clone())
                .or_insert_with(|| vec![0.0; self.edge_dim]);
            for (i, v) in edge_emb.iter().enumerate() {
                dst_features[i] += v * edge.weight * 0.5; // Incoming edges have less influence
            }
        }

        features
    }

    /// Compute edge influence on attention
    fn compute_edge_influence(&self, subgraph: &SubGraph, weights: &[f32]) -> f32 {
        if subgraph.edges.is_empty() || weights.is_empty() {
            return 0.0;
        }

        let mut influence = 0.0;
        for edge in &subgraph.edges {
            // Find indices of source and destination
            let src_idx = subgraph.nodes.iter().position(|n| n.id == edge.src);
            let dst_idx = subgraph.nodes.iter().position(|n| n.id == edge.dst);

            if let (Some(si), Some(di)) = (src_idx, dst_idx) {
                // Correlation between connected nodes' attention weights
                influence += weights[si] * weights[di] * edge.weight;
            }
        }

        influence / subgraph.edges.len() as f32
    }
}

/// Random matrix initialization
fn random_matrix(rng: &mut impl Rng, rows: usize, cols: usize, scale: f32) -> Array2<f32> {
    Array2::from_shape_fn((rows, cols), |_| rng.gen_range(-scale..scale))
}

/// Random vector initialization
fn random_vector(rng: &mut impl Rng, size: usize) -> Array1<f32> {
    Array1::from_shape_fn(size, |_| rng.gen_range(-0.1..0.1))
}

/// Softmax function
fn softmax(x: &[f32]) -> Vec<f32> {
    let max = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = x.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    if sum > 0.0 {
        exp.iter().map(|v| v / sum).collect()
    } else {
        vec![1.0 / x.len() as f32; x.len()]
    }
}

/// Layer normalization
fn layer_norm(x: &Array1<f32>, gamma: &Array1<f32>, beta: &Array1<f32>) -> Array1<f32> {
    let mean = x.mean().unwrap_or(0.0);
    let var = x.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32;
    let std = (var + 1e-5).sqrt();

    let normalized = x.mapv(|v| (v - mean) / std);
    &normalized * gamma + beta
}

/// Average weights across heads
fn average_weights(head_weights: &[Vec<f32>]) -> Vec<f32> {
    if head_weights.is_empty() {
        return vec![];
    }

    let n = head_weights[0].len();
    let num_heads = head_weights.len();

    (0..n)
        .map(|i| head_weights.iter().map(|w| w[i]).sum::<f32>() / num_heads as f32)
        .collect()
}

/// Entropy of probability distribution
fn entropy(probs: &[f32]) -> f32 {
    -probs
        .iter()
        .filter(|&&p| p > 0.0)
        .map(|&p| p * p.ln())
        .sum::<f32>()
}

/// Gini coefficient (measure of inequality)
fn gini_coefficient(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    let n = values.len() as f32;
    let mut sorted: Vec<f32> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let sum: f32 = sorted.iter().sum();
    if sum == 0.0 {
        return 0.0;
    }

    let mut numerator = 0.0;
    for (i, &v) in sorted.iter().enumerate() {
        numerator += (2.0 * (i + 1) as f32 - n - 1.0) * v;
    }

    numerator / (n * sum)
}

/// Dot product of two vectors
#[allow(dead_code)]
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Weighted sum of node embeddings
#[allow(dead_code)]
fn weighted_sum(nodes: &[MemoryNode], weights: &[f32], dim: usize) -> Vec<f32> {
    let mut result = vec![0.0f32; dim];

    for (node, &weight) in nodes.iter().zip(weights.iter()) {
        for (i, &v) in node.vector.iter().take(dim).enumerate() {
            result[i] += v * weight;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeType;
    use std::collections::HashMap;

    fn create_test_node(id: &str, dim: usize, seed: u64) -> MemoryNode {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() - 0.5).collect();
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        vec.iter_mut().for_each(|x| *x /= norm);

        MemoryNode {
            id: id.into(),
            vector: vec,
            text: format!("Test node {}", id),
            node_type: NodeType::Document,
            source: "test".into(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_attention_empty_subgraph() {
        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query = vec![1.0; config.dimension];
        let subgraph = SubGraph {
            nodes: vec![],
            edges: vec![],
            center_ids: vec![],
        };

        let context = engine.attend(&query, &subgraph).unwrap();
        assert_eq!(context.embedding, query);
        assert!(context.ranked_nodes.is_empty());
    }

    #[test]
    fn test_attention_single_node() {
        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query: Vec<f32> = vec![0.1; config.dimension];
        let node = create_test_node("test", config.dimension, 42);

        let subgraph = SubGraph {
            nodes: vec![node],
            edges: vec![],
            center_ids: vec!["test".into()],
        };

        let context = engine.attend(&query, &subgraph).unwrap();
        assert_eq!(context.ranked_nodes.len(), 1);
        assert_eq!(context.attention_weights.len(), 1);
        // Single node should get all attention
        assert!((context.attention_weights[0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_attention_multiple_nodes() {
        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query: Vec<f32> = vec![0.1; config.dimension];
        let nodes: Vec<MemoryNode> = (0..5)
            .map(|i| create_test_node(&format!("node-{}", i), config.dimension, i as u64))
            .collect();

        let subgraph = SubGraph {
            nodes,
            edges: vec![],
            center_ids: vec!["node-0".into()],
        };

        let context = engine.attend(&query, &subgraph).unwrap();
        assert_eq!(context.ranked_nodes.len(), 5);
        assert_eq!(context.attention_weights.len(), 5);

        // Weights should sum to 1
        let sum: f32 = context.attention_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);

        // Weights should be sorted descending
        for i in 1..context.attention_weights.len() {
            assert!(context.attention_weights[i - 1] >= context.attention_weights[i]);
        }
    }

    #[test]
    fn test_attention_with_edges() {
        use crate::types::MemoryEdge;

        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query: Vec<f32> = vec![0.1; config.dimension];
        let nodes: Vec<MemoryNode> = (0..3)
            .map(|i| create_test_node(&format!("node-{}", i), config.dimension, i as u64))
            .collect();

        let edges = vec![
            MemoryEdge {
                id: "e1".into(),
                src: "node-0".into(),
                dst: "node-1".into(),
                edge_type: EdgeType::Cites,
                weight: 1.0,
                metadata: HashMap::new(),
            },
            MemoryEdge {
                id: "e2".into(),
                src: "node-1".into(),
                dst: "node-2".into(),
                edge_type: EdgeType::Follows,
                weight: 0.5,
                metadata: HashMap::new(),
            },
        ];

        let subgraph = SubGraph {
            nodes,
            edges,
            center_ids: vec!["node-0".into()],
        };

        let context = engine.attend(&query, &subgraph).unwrap();
        assert_eq!(context.summary.num_edges, 2);
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let scores = vec![1.0, 2.0, 3.0, 0.5, -1.0];
        let probs = softmax(&scores);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_softmax_stable() {
        // Large values should not cause overflow
        let scores = vec![1000.0, 1001.0, 1002.0];
        let probs = softmax(&scores);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_entropy() {
        // Uniform distribution has max entropy
        let uniform = vec![0.25, 0.25, 0.25, 0.25];
        let uniform_entropy = entropy(&uniform);

        // Concentrated distribution has low entropy
        let concentrated = vec![0.97, 0.01, 0.01, 0.01];
        let concentrated_entropy = entropy(&concentrated);

        assert!(uniform_entropy > concentrated_entropy);
    }

    #[test]
    fn test_gini_coefficient() {
        // Perfect equality
        let equal = vec![0.25, 0.25, 0.25, 0.25];
        let gini_equal = gini_coefficient(&equal);
        assert!(gini_equal.abs() < 0.01);

        // High inequality
        let unequal = vec![0.97, 0.01, 0.01, 0.01];
        let gini_unequal = gini_coefficient(&unequal);
        assert!(gini_unequal > gini_equal);
    }

    #[test]
    fn test_layer_norm() {
        let x = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0]);
        let gamma = Array1::ones(4);
        let beta = Array1::zeros(4);

        let normalized = layer_norm(&x, &gamma, &beta);

        // Mean should be close to 0
        let mean: f32 = normalized.iter().sum::<f32>() / normalized.len() as f32;
        assert!(mean.abs() < 0.01);

        // Variance should be close to 1
        let var: f32 =
            normalized.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / normalized.len() as f32;
        assert!((var - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_multi_head_weights() {
        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query: Vec<f32> = vec![0.1; config.dimension];
        let nodes: Vec<MemoryNode> = (0..3)
            .map(|i| create_test_node(&format!("node-{}", i), config.dimension, i as u64))
            .collect();

        let subgraph = SubGraph {
            nodes,
            edges: vec![],
            center_ids: vec![],
        };

        let context = engine.attend(&query, &subgraph).unwrap();

        // Should have weights from all heads
        assert_eq!(context.head_weights.len(), 8); // 8 heads

        // Each head's weights should sum to 1
        for head_weights in &context.head_weights {
            let sum: f32 = head_weights.iter().sum();
            assert!((sum - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_cross_attention() {
        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query: Vec<f32> = vec![0.1; config.dimension];
        let nodes: Vec<MemoryNode> = (0..3)
            .map(|i| create_test_node(&format!("node-{}", i), config.dimension, i as u64))
            .collect();

        let subgraph = SubGraph {
            nodes,
            edges: vec![],
            center_ids: vec![],
        };

        let (forward_ctx, backward_weights) = engine.cross_attend(&query, &subgraph).unwrap();

        // Forward context should be valid
        assert_eq!(forward_ctx.ranked_nodes.len(), 3);

        // Backward weights should sum to 1
        let sum: f32 = backward_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
