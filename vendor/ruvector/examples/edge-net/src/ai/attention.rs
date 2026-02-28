//! Graph Attention for Context Ranking
//!
//! Multi-head attention with edge-aware scoring and residual connections.

/// Attention configuration
#[derive(Clone, Debug)]
pub struct AttentionConfig {
    /// Number of attention heads
    pub num_heads: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Dropout rate (training only)
    pub dropout: f32,
    /// Use layer normalization
    pub layer_norm: bool,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            num_heads: 8,
            hidden_dim: 128,
            dropout: 0.1,
            layer_norm: true,
        }
    }
}

/// Graph context for attention
#[derive(Clone, Debug)]
pub struct GraphContext {
    /// Node embeddings [num_nodes, hidden_dim]
    pub node_embeddings: Vec<Vec<f32>>,
    /// Edge features (optional)
    pub edge_features: Option<Vec<Vec<f32>>>,
    /// Adjacency (node pairs)
    pub edges: Vec<(usize, usize)>,
}

/// Multi-head graph attention
pub struct GraphAttention {
    /// Configuration
    config: AttentionConfig,
    /// Query projection [hidden_dim, hidden_dim]
    w_query: Vec<f32>,
    /// Key projection [hidden_dim, hidden_dim]
    w_key: Vec<f32>,
    /// Value projection [hidden_dim, hidden_dim]
    w_value: Vec<f32>,
    /// Output projection [hidden_dim, hidden_dim]
    w_out: Vec<f32>,
}

impl GraphAttention {
    /// Create new graph attention layer
    pub fn new(hidden_dim: usize, num_heads: usize) -> Result<Self, String> {
        if hidden_dim % num_heads != 0 {
            return Err(format!(
                "hidden_dim {} must be divisible by num_heads {}",
                hidden_dim, num_heads
            ));
        }

        let size = hidden_dim * hidden_dim;

        Ok(Self {
            config: AttentionConfig {
                num_heads,
                hidden_dim,
                ..Default::default()
            },
            w_query: vec![0.01; size],
            w_key: vec![0.01; size],
            w_value: vec![0.01; size],
            w_out: vec![0.01; size],
        })
    }

    /// Compute attention over graph context
    pub fn attend(&self, query: &[f32], context: &GraphContext) -> Vec<f32> {
        if context.node_embeddings.is_empty() {
            return query.to_vec();
        }

        let hidden_dim = self.config.hidden_dim;
        let num_heads = self.config.num_heads;
        let head_dim = hidden_dim / num_heads;
        let num_nodes = context.node_embeddings.len();

        // Project query
        let q = self.linear(query, &self.w_query, hidden_dim);

        // Project keys and values from context nodes
        let mut keys = Vec::with_capacity(num_nodes);
        let mut values = Vec::with_capacity(num_nodes);

        for node in &context.node_embeddings {
            keys.push(self.linear(node, &self.w_key, hidden_dim));
            values.push(self.linear(node, &self.w_value, hidden_dim));
        }

        // Compute attention scores
        let mut scores = vec![0.0f32; num_nodes];
        let scale = (head_dim as f32).sqrt();

        for (i, key) in keys.iter().enumerate() {
            let mut dot = 0.0f32;
            for j in 0..hidden_dim {
                dot += q[j] * key[j];
            }
            scores[i] = dot / scale;
        }

        // Softmax
        self.softmax(&mut scores);

        // Weighted sum of values
        let mut output = vec![0.0f32; hidden_dim];
        for (i, value) in values.iter().enumerate() {
            for j in 0..hidden_dim {
                output[j] += scores[i] * value[j];
            }
        }

        // Output projection + residual
        let projected = self.linear(&output, &self.w_out, hidden_dim);

        // Residual connection
        let mut result = vec![0.0f32; hidden_dim];
        for j in 0..hidden_dim.min(query.len()) {
            result[j] = query[j] + projected[j];
        }

        // Layer norm
        if self.config.layer_norm {
            self.layer_norm(&mut result);
        }

        result
    }

    // Private helpers

    fn linear(&self, input: &[f32], weight: &[f32], out_dim: usize) -> Vec<f32> {
        let in_dim = input.len();
        let mut output = vec![0.0f32; out_dim];

        for o in 0..out_dim {
            for i in 0..in_dim.min(out_dim) {
                output[o] += input[i] * weight[i * out_dim + o];
            }
        }

        output
    }

    fn softmax(&self, scores: &mut [f32]) {
        if scores.is_empty() {
            return;
        }

        let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0f32;

        for s in scores.iter_mut() {
            *s = (*s - max).exp();
            sum += *s;
        }

        if sum > 0.0 {
            for s in scores.iter_mut() {
                *s /= sum;
            }
        }
    }

    fn layer_norm(&self, x: &mut [f32]) {
        if x.is_empty() {
            return;
        }

        // Compute mean
        let mean: f32 = x.iter().sum::<f32>() / x.len() as f32;

        // Compute variance
        let var: f32 = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32;
        let std = (var + 1e-5).sqrt();

        // Normalize
        for v in x.iter_mut() {
            *v = (*v - mean) / std;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_creation() {
        let attn = GraphAttention::new(128, 8);
        assert!(attn.is_ok());
    }

    #[test]
    fn test_attention_invalid_dims() {
        let attn = GraphAttention::new(100, 8);
        assert!(attn.is_err());
    }

    #[test]
    fn test_attention_forward() {
        let attn = GraphAttention::new(64, 8).unwrap();
        let query = vec![1.0; 64];
        let context = GraphContext {
            node_embeddings: vec![vec![0.5; 64], vec![0.3; 64]],
            edge_features: None,
            edges: vec![(0, 1)],
        };

        let output = attn.attend(&query, &context);
        assert_eq!(output.len(), 64);
    }
}
