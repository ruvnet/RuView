//! Edge-featured graph attention (GATv2 style)
//!
//! Extends standard graph attention with edge feature integration.

use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use crate::utils::stable_softmax;

/// Configuration for edge-featured attention
#[derive(Clone, Debug)]
pub struct EdgeFeaturedConfig {
    pub node_dim: usize,
    pub edge_dim: usize,
    pub num_heads: usize,
    pub dropout: f32,
    pub concat_heads: bool,
    pub add_self_loops: bool,
    pub negative_slope: f32, // LeakyReLU slope
}

impl Default for EdgeFeaturedConfig {
    fn default() -> Self {
        Self {
            node_dim: 256,
            edge_dim: 64,
            num_heads: 4,
            dropout: 0.0,
            concat_heads: true,
            add_self_loops: true,
            negative_slope: 0.2,
        }
    }
}

impl EdgeFeaturedConfig {
    pub fn builder() -> EdgeFeaturedConfigBuilder {
        EdgeFeaturedConfigBuilder::default()
    }

    pub fn head_dim(&self) -> usize {
        self.node_dim / self.num_heads
    }
}

#[derive(Default)]
pub struct EdgeFeaturedConfigBuilder {
    config: EdgeFeaturedConfig,
}

impl EdgeFeaturedConfigBuilder {
    pub fn node_dim(mut self, d: usize) -> Self {
        self.config.node_dim = d;
        self
    }

    pub fn edge_dim(mut self, d: usize) -> Self {
        self.config.edge_dim = d;
        self
    }

    pub fn num_heads(mut self, n: usize) -> Self {
        self.config.num_heads = n;
        self
    }

    pub fn dropout(mut self, d: f32) -> Self {
        self.config.dropout = d;
        self
    }

    pub fn concat_heads(mut self, c: bool) -> Self {
        self.config.concat_heads = c;
        self
    }

    pub fn negative_slope(mut self, s: f32) -> Self {
        self.config.negative_slope = s;
        self
    }

    pub fn build(self) -> EdgeFeaturedConfig {
        self.config
    }
}

/// Edge-featured graph attention layer
pub struct EdgeFeaturedAttention {
    config: EdgeFeaturedConfig,
    // Weight matrices (would be learnable in training)
    w_node: Vec<f32>, // [num_heads, head_dim, node_dim]
    w_edge: Vec<f32>, // [num_heads, head_dim, edge_dim]
    a_src: Vec<f32>,  // [num_heads, head_dim]
    a_dst: Vec<f32>,  // [num_heads, head_dim]
    a_edge: Vec<f32>, // [num_heads, head_dim]
}

impl EdgeFeaturedAttention {
    pub fn new(config: EdgeFeaturedConfig) -> Self {
        let head_dim = config.head_dim();
        let num_heads = config.num_heads;

        // Xavier initialization
        let node_scale = (2.0 / (config.node_dim + head_dim) as f32).sqrt();
        let edge_scale = (2.0 / (config.edge_dim + head_dim) as f32).sqrt();
        let attn_scale = (1.0 / head_dim as f32).sqrt();

        let mut seed = 42u64;
        let mut rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed as f32) / (u64::MAX as f32) - 0.5
        };

        let w_node: Vec<f32> = (0..num_heads * head_dim * config.node_dim)
            .map(|_| rand() * 2.0 * node_scale)
            .collect();

        let w_edge: Vec<f32> = (0..num_heads * head_dim * config.edge_dim)
            .map(|_| rand() * 2.0 * edge_scale)
            .collect();

        let a_src: Vec<f32> = (0..num_heads * head_dim)
            .map(|_| rand() * 2.0 * attn_scale)
            .collect();

        let a_dst: Vec<f32> = (0..num_heads * head_dim)
            .map(|_| rand() * 2.0 * attn_scale)
            .collect();

        let a_edge: Vec<f32> = (0..num_heads * head_dim)
            .map(|_| rand() * 2.0 * attn_scale)
            .collect();

        Self {
            config,
            w_node,
            w_edge,
            a_src,
            a_dst,
            a_edge,
        }
    }

    /// Transform node features for a specific head
    fn transform_node(&self, node: &[f32], head: usize) -> Vec<f32> {
        let head_dim = self.config.head_dim();
        let node_dim = self.config.node_dim;

        (0..head_dim)
            .map(|i| {
                node.iter()
                    .enumerate()
                    .map(|(j, &nj)| nj * self.w_node[head * head_dim * node_dim + i * node_dim + j])
                    .sum()
            })
            .collect()
    }

    /// Transform edge features for a specific head
    fn transform_edge(&self, edge: &[f32], head: usize) -> Vec<f32> {
        let head_dim = self.config.head_dim();
        let edge_dim = self.config.edge_dim;

        (0..head_dim)
            .map(|i| {
                edge.iter()
                    .enumerate()
                    .map(|(j, &ej)| ej * self.w_edge[head * head_dim * edge_dim + i * edge_dim + j])
                    .sum()
            })
            .collect()
    }

    /// Compute attention coefficient with LeakyReLU
    fn attention_coeff(&self, src: &[f32], dst: &[f32], edge: &[f32], head: usize) -> f32 {
        let head_dim = self.config.head_dim();

        let mut score = 0.0f32;
        for i in 0..head_dim {
            let offset = head * head_dim + i;
            score += src[i] * self.a_src[offset];
            score += dst[i] * self.a_dst[offset];
            score += edge[i] * self.a_edge[offset];
        }

        // LeakyReLU
        if score < 0.0 {
            self.config.negative_slope * score
        } else {
            score
        }
    }
}

impl EdgeFeaturedAttention {
    /// Compute attention with explicit edge features
    pub fn compute_with_edges(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        edges: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.len() != edges.len() {
            return Err(AttentionError::InvalidConfig(
                "Keys and edges must have same length".to_string(),
            ));
        }

        let num_heads = self.config.num_heads;
        let head_dim = self.config.head_dim();
        let n = keys.len();

        // Transform query once per head
        let query_transformed: Vec<Vec<f32>> = (0..num_heads)
            .map(|h| self.transform_node(query, h))
            .collect();

        // Compute per-head outputs
        let mut head_outputs: Vec<Vec<f32>> = Vec::with_capacity(num_heads);

        for h in 0..num_heads {
            // Transform all keys and edges
            let keys_t: Vec<Vec<f32>> = keys.iter().map(|k| self.transform_node(k, h)).collect();
            let edges_t: Vec<Vec<f32>> = edges.iter().map(|e| self.transform_edge(e, h)).collect();

            // Compute attention coefficients
            let coeffs: Vec<f32> = (0..n)
                .map(|i| self.attention_coeff(&query_transformed[h], &keys_t[i], &edges_t[i], h))
                .collect();

            // Softmax
            let weights = stable_softmax(&coeffs);

            // Weighted sum of values
            let mut head_out = vec![0.0f32; head_dim];
            for (i, &w) in weights.iter().enumerate() {
                let value_t = self.transform_node(values[i], h);
                for (j, &vj) in value_t.iter().enumerate() {
                    head_out[j] += w * vj;
                }
            }

            head_outputs.push(head_out);
        }

        // Concatenate or average heads
        if self.config.concat_heads {
            Ok(head_outputs.into_iter().flatten().collect())
        } else {
            let mut output = vec![0.0f32; head_dim];
            for head_out in &head_outputs {
                for (i, &v) in head_out.iter().enumerate() {
                    output[i] += v / num_heads as f32;
                }
            }
            Ok(output)
        }
    }

    /// Get the edge feature dimension
    pub fn edge_dim(&self) -> usize {
        self.config.edge_dim
    }
}

impl Attention for EdgeFeaturedAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty keys".to_string()));
        }
        if query.len() != self.config.node_dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.config.node_dim,
                actual: query.len(),
            });
        }

        // Use zero edge features for basic attention
        let zero_edge = vec![0.0f32; self.config.edge_dim];
        let edges: Vec<&[f32]> = (0..keys.len()).map(|_| zero_edge.as_slice()).collect();

        self.compute_with_edges(query, keys, values, &edges)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        // Apply mask by filtering keys/values
        if let Some(m) = mask {
            let filtered: Vec<(usize, bool)> = m
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, keep)| *keep)
                .collect();
            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(i, _)| keys[*i]).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(i, _)| values[*i]).collect();
            self.compute(query, &filtered_keys, &filtered_values)
        } else {
            self.compute(query, keys, values)
        }
    }

    fn dim(&self) -> usize {
        if self.config.concat_heads {
            self.config.node_dim
        } else {
            self.config.head_dim()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_featured_attention() {
        let config = EdgeFeaturedConfig::builder()
            .node_dim(64)
            .edge_dim(16)
            .num_heads(4)
            .build();

        let attn = EdgeFeaturedAttention::new(config);

        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = (0..10).map(|_| vec![0.3; 64]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|_| vec![1.0; 64]).collect();
        let edges: Vec<Vec<f32>> = (0..10).map(|_| vec![0.2; 16]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();
        let edges_refs: Vec<&[f32]> = edges.iter().map(|e| e.as_slice()).collect();

        let result = attn
            .compute_with_edges(&query, &keys_refs, &values_refs, &edges_refs)
            .unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_without_edges() {
        let config = EdgeFeaturedConfig::builder()
            .node_dim(32)
            .edge_dim(8)
            .num_heads(2)
            .build();

        let attn = EdgeFeaturedAttention::new(config);

        let query = vec![0.5; 32];
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 32]; 5];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 32]; 5];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attn.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_leaky_relu() {
        let config = EdgeFeaturedConfig::builder()
            .node_dim(16)
            .edge_dim(4)
            .num_heads(1)
            .negative_slope(0.2)
            .build();

        let attn = EdgeFeaturedAttention::new(config);

        // Just verify it computes without error
        let query = vec![-1.0; 16];
        let keys: Vec<Vec<f32>> = vec![vec![-0.5; 16]; 3];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 16]; 3];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attn.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 16);
    }
}
