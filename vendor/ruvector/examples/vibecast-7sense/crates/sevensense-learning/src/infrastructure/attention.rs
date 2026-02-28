//! Attention mechanisms for GNN models.
//!
//! This module provides attention layers and mechanisms including:
//! - Single-head attention
//! - Multi-head attention
//! - Graph-level attention readout

use ndarray::{Array1, Array2, Axis};
use rand::Rng;
use rand_distr::{Distribution, Uniform};

/// Error type for attention operations
#[derive(Debug, thiserror::Error)]
pub enum AttentionError {
    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Invalid configuration
    #[error("Invalid attention configuration: {0}")]
    InvalidConfig(String),

    /// Computation error
    #[error("Attention computation error: {0}")]
    ComputationError(String),
}

/// Result type for attention operations
pub type AttentionResult<T> = Result<T, AttentionError>;

/// Single-head attention layer
#[derive(Debug, Clone)]
pub struct AttentionLayer {
    /// Query weight matrix
    query_weights: Array2<f32>,
    /// Key weight matrix
    key_weights: Array2<f32>,
    /// Value weight matrix
    value_weights: Array2<f32>,
    /// Attention dimension
    attention_dim: usize,
    /// Input dimension
    input_dim: usize,
    /// Output dimension
    output_dim: usize,
    /// Scaling factor
    scale: f32,
}

impl AttentionLayer {
    /// Create a new attention layer
    #[must_use]
    pub fn new(input_dim: usize, attention_dim: usize) -> Self {
        let query_weights = xavier_init(input_dim, attention_dim);
        let key_weights = xavier_init(input_dim, attention_dim);
        let value_weights = xavier_init(input_dim, attention_dim);
        let scale = (attention_dim as f32).sqrt();

        Self {
            query_weights,
            key_weights,
            value_weights,
            attention_dim,
            input_dim,
            output_dim: attention_dim,
            scale,
        }
    }

    /// Get the output dimension
    #[must_use]
    pub fn output_dim(&self) -> usize {
        self.output_dim
    }

    /// Compute attention scores
    pub fn compute_attention(
        &self,
        query: &Array2<f32>,
        key: &Array2<f32>,
        mask: Option<&Array2<f32>>,
    ) -> AttentionResult<Array2<f32>> {
        // Q * K^T / sqrt(d_k)
        let scores = query.dot(&key.t()) / self.scale;

        // Apply mask if provided (set masked positions to -inf)
        let scores = if let Some(mask) = mask {
            let mut masked = scores;
            for i in 0..masked.nrows() {
                for j in 0..masked.ncols() {
                    if mask[[i, j]] == 0.0 {
                        masked[[i, j]] = f32::NEG_INFINITY;
                    }
                }
            }
            masked
        } else {
            scores
        };

        // Softmax
        let attention_weights = softmax_2d(&scores);

        Ok(attention_weights)
    }

    /// Forward pass through the attention layer
    pub fn forward(&self, features: &Array2<f32>) -> AttentionResult<Array2<f32>> {
        if features.ncols() != self.input_dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.input_dim,
                actual: features.ncols(),
            });
        }

        // Compute Q, K, V
        let q = features.dot(&self.query_weights);
        let k = features.dot(&self.key_weights);
        let v = features.dot(&self.value_weights);

        // Compute attention weights
        let attention_weights = self.compute_attention(&q, &k, None)?;

        // Apply attention to values
        let output = attention_weights.dot(&v);

        Ok(output)
    }

    /// Forward pass with explicit Q, K, V
    pub fn forward_qkv(
        &self,
        query: &Array2<f32>,
        key: &Array2<f32>,
        value: &Array2<f32>,
        mask: Option<&Array2<f32>>,
    ) -> AttentionResult<Array2<f32>> {
        // Transform Q, K, V
        let q = query.dot(&self.query_weights);
        let k = key.dot(&self.key_weights);
        let v = value.dot(&self.value_weights);

        // Compute attention weights
        let attention_weights = self.compute_attention(&q, &k, mask)?;

        // Apply attention to values
        let output = attention_weights.dot(&v);

        Ok(output)
    }

    /// Graph-level readout using attention
    pub fn graph_readout(&self, node_features: &Array2<f32>) -> AttentionResult<Array1<f32>> {
        // Compute attention-weighted mean of node features
        let attended = self.forward(node_features)?;

        // Mean over nodes
        let mean = attended.mean_axis(Axis(0)).unwrap();

        Ok(mean)
    }

    /// Update weights with gradient
    pub fn update_weights(&mut self, _lr: f32, weight_decay: f32) {
        self.query_weights -= &(&self.query_weights * weight_decay);
        self.key_weights -= &(&self.key_weights * weight_decay);
        self.value_weights -= &(&self.value_weights * weight_decay);
    }
}

/// Multi-head attention layer
#[derive(Debug, Clone)]
pub struct MultiHeadAttention {
    /// Individual attention heads
    heads: Vec<AttentionLayer>,
    /// Output projection
    output_projection: Array2<f32>,
    /// Number of heads
    num_heads: usize,
    /// Dimension per head
    head_dim: usize,
    /// Total output dimension
    output_dim: usize,
    /// Dropout probability
    dropout: f32,
}

impl MultiHeadAttention {
    /// Create a new multi-head attention layer
    #[must_use]
    pub fn new(input_dim: usize, num_heads: usize, head_dim: usize, dropout: f32) -> Self {
        let mut heads = Vec::with_capacity(num_heads);
        for _ in 0..num_heads {
            heads.push(AttentionLayer::new(input_dim, head_dim));
        }

        let total_dim = num_heads * head_dim;
        let output_projection = xavier_init(total_dim, input_dim);

        Self {
            heads,
            output_projection,
            num_heads,
            head_dim,
            output_dim: input_dim,
            dropout,
        }
    }

    /// Get the number of heads
    #[must_use]
    pub fn num_heads(&self) -> usize {
        self.num_heads
    }

    /// Get the output dimension
    #[must_use]
    pub fn output_dim(&self) -> usize {
        self.output_dim
    }

    /// Forward pass through multi-head attention
    pub fn forward(&self, features: &Array2<f32>) -> AttentionResult<Array2<f32>> {
        let n = features.nrows();

        // Compute attention for each head
        let mut head_outputs = Vec::with_capacity(self.num_heads);
        for head in &self.heads {
            let output = head.forward(features)?;
            head_outputs.push(output);
        }

        // Concatenate head outputs
        let mut concat = Array2::zeros((n, self.num_heads * self.head_dim));
        for (h, output) in head_outputs.iter().enumerate() {
            let start = h * self.head_dim;
            for i in 0..n {
                for j in 0..self.head_dim {
                    concat[[i, start + j]] = output[[i, j]];
                }
            }
        }

        // Apply output projection
        let output = concat.dot(&self.output_projection);

        Ok(output)
    }

    /// Forward pass with explicit Q, K, V
    pub fn forward_qkv(
        &self,
        query: &Array2<f32>,
        key: &Array2<f32>,
        value: &Array2<f32>,
        mask: Option<&Array2<f32>>,
    ) -> AttentionResult<Array2<f32>> {
        let n = query.nrows();

        // Compute attention for each head
        let mut head_outputs = Vec::with_capacity(self.num_heads);
        for head in &self.heads {
            let output = head.forward_qkv(query, key, value, mask)?;
            head_outputs.push(output);
        }

        // Concatenate head outputs
        let mut concat = Array2::zeros((n, self.num_heads * self.head_dim));
        for (h, output) in head_outputs.iter().enumerate() {
            let start = h * self.head_dim;
            for i in 0..n {
                for j in 0..self.head_dim {
                    concat[[i, start + j]] = output[[i, j]];
                }
            }
        }

        // Apply output projection
        let output = concat.dot(&self.output_projection);

        Ok(output)
    }

    /// Graph-level readout using multi-head attention
    pub fn graph_readout(&self, node_features: &Array2<f32>) -> AttentionResult<Array1<f32>> {
        let attended = self.forward(node_features)?;
        let mean = attended.mean_axis(Axis(0)).unwrap();
        Ok(mean)
    }
}

/// Cross-attention between two sequences
#[derive(Debug, Clone)]
pub struct CrossAttention {
    /// Query projection for source
    query_proj: Array2<f32>,
    /// Key projection for target
    key_proj: Array2<f32>,
    /// Value projection for target
    value_proj: Array2<f32>,
    /// Attention dimension
    attention_dim: usize,
    /// Source dimension
    source_dim: usize,
    /// Target dimension
    target_dim: usize,
}

impl CrossAttention {
    /// Create a new cross-attention layer
    #[must_use]
    pub fn new(source_dim: usize, target_dim: usize, attention_dim: usize) -> Self {
        Self {
            query_proj: xavier_init(source_dim, attention_dim),
            key_proj: xavier_init(target_dim, attention_dim),
            value_proj: xavier_init(target_dim, attention_dim),
            attention_dim,
            source_dim,
            target_dim,
        }
    }

    /// Compute cross-attention between source and target
    pub fn forward(
        &self,
        source: &Array2<f32>,
        target: &Array2<f32>,
    ) -> AttentionResult<Array2<f32>> {
        // Project source to query
        let query = source.dot(&self.query_proj);

        // Project target to key and value
        let key = target.dot(&self.key_proj);
        let value = target.dot(&self.value_proj);

        // Compute attention scores
        let scale = (self.attention_dim as f32).sqrt();
        let scores = query.dot(&key.t()) / scale;

        // Softmax
        let attention_weights = softmax_2d(&scores);

        // Apply attention to values
        let output = attention_weights.dot(&value);

        Ok(output)
    }
}

/// Set attention for set-to-set operations
#[derive(Debug, Clone)]
pub struct SetAttention {
    /// Multi-head attention
    mha: MultiHeadAttention,
    /// Layer normalization parameters
    layer_norm_weight: Array1<f32>,
    layer_norm_bias: Array1<f32>,
    /// Feed-forward network
    ffn_w1: Array2<f32>,
    ffn_w2: Array2<f32>,
    /// Dimensions
    input_dim: usize,
    hidden_dim: usize,
}

impl SetAttention {
    /// Create a new set attention layer (Set Transformer style)
    #[must_use]
    pub fn new(input_dim: usize, num_heads: usize, hidden_dim: usize) -> Self {
        let head_dim = input_dim / num_heads;
        let mha = MultiHeadAttention::new(input_dim, num_heads, head_dim, 0.0);

        Self {
            mha,
            layer_norm_weight: Array1::ones(input_dim),
            layer_norm_bias: Array1::zeros(input_dim),
            ffn_w1: xavier_init(input_dim, hidden_dim),
            ffn_w2: xavier_init(hidden_dim, input_dim),
            input_dim,
            hidden_dim,
        }
    }

    /// Forward pass with self-attention and feed-forward
    pub fn forward(&self, features: &Array2<f32>) -> AttentionResult<Array2<f32>> {
        // Self-attention with residual
        let attended = self.mha.forward(features)?;
        let residual1 = features + &attended;
        let normed1 = layer_norm(&residual1, &self.layer_norm_weight, &self.layer_norm_bias);

        // Feed-forward with residual
        let hidden = normed1.dot(&self.ffn_w1).mapv(|x| x.max(0.0)); // ReLU
        let ffn_out = hidden.dot(&self.ffn_w2);
        let residual2 = &normed1 + &ffn_out;
        let output = layer_norm(&residual2, &self.layer_norm_weight, &self.layer_norm_bias);

        Ok(output)
    }

    /// Aggregate set elements using learned attention
    pub fn aggregate(&self, features: &Array2<f32>) -> AttentionResult<Array1<f32>> {
        let transformed = self.forward(features)?;
        Ok(transformed.mean_axis(Axis(0)).unwrap())
    }
}

// =========== Helper Functions ===========

/// Xavier/Glorot initialization
fn xavier_init(fan_in: usize, fan_out: usize) -> Array2<f32> {
    let limit = (6.0 / (fan_in + fan_out) as f32).sqrt();
    let uniform = Uniform::new(-limit, limit);
    let mut rng = rand::thread_rng();

    Array2::from_shape_fn((fan_in, fan_out), |_| uniform.sample(&mut rng))
}

/// Softmax over 2D array (row-wise)
fn softmax_2d(scores: &Array2<f32>) -> Array2<f32> {
    let mut result = scores.clone();

    for mut row in result.rows_mut() {
        // Subtract max for numerical stability
        let max = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let mut sum = 0.0;
        for val in row.iter_mut() {
            if val.is_finite() {
                *val = (*val - max).exp();
                sum += *val;
            } else {
                *val = 0.0;
            }
        }

        if sum > 0.0 {
            row /= sum;
        }
    }

    result
}

/// Layer normalization
fn layer_norm(x: &Array2<f32>, weight: &Array1<f32>, bias: &Array1<f32>) -> Array2<f32> {
    let eps = 1e-5;
    let mut result = x.clone();

    for mut row in result.rows_mut() {
        let mean = row.mean().unwrap_or(0.0);
        let variance = row.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / row.len() as f32;
        let std = (variance + eps).sqrt();

        for (i, val) in row.iter_mut().enumerate() {
            *val = (*val - mean) / std * weight[i] + bias[i];
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_layer() {
        let layer = AttentionLayer::new(8, 16);
        assert_eq!(layer.output_dim(), 16);

        let features = Array2::from_elem((3, 8), 0.5);
        let output = layer.forward(&features).unwrap();

        assert_eq!(output.shape(), &[3, 16]);
    }

    #[test]
    fn test_multi_head_attention() {
        let mha = MultiHeadAttention::new(8, 4, 4, 0.0);
        assert_eq!(mha.num_heads(), 4);
        assert_eq!(mha.output_dim(), 8);

        let features = Array2::from_elem((5, 8), 0.5);
        let output = mha.forward(&features).unwrap();

        assert_eq!(output.shape(), &[5, 8]);
    }

    #[test]
    fn test_cross_attention() {
        let cross = CrossAttention::new(8, 16, 32);

        let source = Array2::from_elem((3, 8), 0.5);
        let target = Array2::from_elem((5, 16), 0.5);

        let output = cross.forward(&source, &target).unwrap();
        assert_eq!(output.shape(), &[3, 32]);
    }

    #[test]
    fn test_set_attention() {
        let set_attn = SetAttention::new(16, 4, 64);

        let features = Array2::from_elem((4, 16), 0.5);
        let output = set_attn.forward(&features).unwrap();

        assert_eq!(output.shape(), &[4, 16]);

        let aggregated = set_attn.aggregate(&features).unwrap();
        assert_eq!(aggregated.len(), 16);
    }

    #[test]
    fn test_softmax() {
        let scores = Array2::from_shape_vec((2, 3), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();

        let probs = softmax_2d(&scores);

        // Each row should sum to 1
        for row in probs.rows() {
            let sum: f32 = row.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5);
        }

        // Higher values should have higher probabilities
        assert!(probs[[0, 2]] > probs[[0, 1]]);
        assert!(probs[[0, 1]] > probs[[0, 0]]);
    }

    #[test]
    fn test_layer_norm() {
        let x = Array2::from_shape_vec((2, 4), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]).unwrap();

        let weight = Array1::ones(4);
        let bias = Array1::zeros(4);

        let normed = layer_norm(&x, &weight, &bias);

        // Each row should have mean ~0 and std ~1
        for row in normed.rows() {
            let mean: f32 = row.iter().sum::<f32>() / row.len() as f32;
            assert!(mean.abs() < 1e-5);
        }
    }

    #[test]
    fn test_graph_readout() {
        let layer = AttentionLayer::new(8, 4);
        let node_features = Array2::from_elem((5, 8), 0.5);

        let readout = layer.graph_readout(&node_features).unwrap();
        assert_eq!(readout.len(), 4);
    }

    #[test]
    fn test_attention_with_mask() {
        let layer = AttentionLayer::new(4, 4);

        let features = Array2::from_elem((3, 4), 0.5);

        // Create a mask that blocks second and third positions
        let mut mask = Array2::ones((3, 3));
        mask[[0, 1]] = 0.0;
        mask[[0, 2]] = 0.0;

        let query = features.dot(&layer.query_weights);
        let key = features.dot(&layer.key_weights);

        let attn_weights = layer.compute_attention(&query, &key, Some(&mask)).unwrap();

        // First row should only attend to itself
        assert!(attn_weights[[0, 0]] > 0.99); // Almost all attention to self
        assert!(attn_weights[[0, 1]] < 0.01);
        assert!(attn_weights[[0, 2]] < 0.01);
    }
}
