//! GNN Layer Implementation for HNSW Topology
//!
//! This module implements graph neural network layers that operate on HNSW graph structure,
//! including attention mechanisms, normalization, and gated recurrent updates.

use crate::error::GnnError;
use ndarray::{Array1, Array2, ArrayView1};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

/// Linear transformation layer (weight matrix multiplication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Linear {
    weights: Array2<f32>,
    bias: Array1<f32>,
}

impl Linear {
    /// Create a new linear layer with Xavier/Glorot initialization
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        let mut rng = rand::thread_rng();

        // Xavier initialization: scale = sqrt(2.0 / (input_dim + output_dim))
        let scale = (2.0 / (input_dim + output_dim) as f32).sqrt();
        let normal = Normal::new(0.0, scale as f64).unwrap();

        let weights =
            Array2::from_shape_fn((output_dim, input_dim), |_| normal.sample(&mut rng) as f32);

        let bias = Array1::zeros(output_dim);

        Self { weights, bias }
    }

    /// Forward pass: y = Wx + b
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let x = ArrayView1::from(input);
        let output = self.weights.dot(&x) + &self.bias;
        output.to_vec()
    }

    /// Get output dimension
    pub fn output_dim(&self) -> usize {
        self.weights.shape()[0]
    }
}

/// Layer normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerNorm {
    gamma: Array1<f32>,
    beta: Array1<f32>,
    eps: f32,
}

impl LayerNorm {
    /// Create a new layer normalization layer
    pub fn new(dim: usize, eps: f32) -> Self {
        Self {
            gamma: Array1::ones(dim),
            beta: Array1::zeros(dim),
            eps,
        }
    }

    /// Forward pass: normalize and scale
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let x = ArrayView1::from(input);

        // Compute mean and variance
        let mean = x.mean().unwrap_or(0.0);
        let variance = x.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32;

        // Normalize
        let normalized = x.mapv(|v| (v - mean) / (variance + self.eps).sqrt());

        // Scale and shift
        let output = &self.gamma * &normalized + &self.beta;
        output.to_vec()
    }
}

/// Multi-head attention mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHeadAttention {
    num_heads: usize,
    head_dim: usize,
    q_linear: Linear,
    k_linear: Linear,
    v_linear: Linear,
    out_linear: Linear,
}

impl MultiHeadAttention {
    /// Create a new multi-head attention layer
    ///
    /// # Errors
    /// Returns `GnnError::LayerConfig` if `embed_dim` is not divisible by `num_heads`.
    pub fn new(embed_dim: usize, num_heads: usize) -> Result<Self, GnnError> {
        if embed_dim % num_heads != 0 {
            return Err(GnnError::layer_config(format!(
                "Embedding dimension ({}) must be divisible by number of heads ({})",
                embed_dim, num_heads
            )));
        }

        let head_dim = embed_dim / num_heads;

        Ok(Self {
            num_heads,
            head_dim,
            q_linear: Linear::new(embed_dim, embed_dim),
            k_linear: Linear::new(embed_dim, embed_dim),
            v_linear: Linear::new(embed_dim, embed_dim),
            out_linear: Linear::new(embed_dim, embed_dim),
        })
    }

    /// Forward pass: compute multi-head attention
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Key vectors from neighbors
    /// * `values` - Value vectors from neighbors
    ///
    /// # Returns
    /// Attention-weighted output vector
    pub fn forward(&self, query: &[f32], keys: &[Vec<f32>], values: &[Vec<f32>]) -> Vec<f32> {
        if keys.is_empty() || values.is_empty() {
            return query.to_vec();
        }

        // Project query, keys, and values
        let q = self.q_linear.forward(query);
        let k: Vec<Vec<f32>> = keys.iter().map(|k| self.k_linear.forward(k)).collect();
        let v: Vec<Vec<f32>> = values.iter().map(|v| self.v_linear.forward(v)).collect();

        // Reshape for multi-head attention
        let q_heads = self.split_heads(&q);
        let k_heads: Vec<Vec<Vec<f32>>> = k.iter().map(|k_vec| self.split_heads(k_vec)).collect();
        let v_heads: Vec<Vec<Vec<f32>>> = v.iter().map(|v_vec| self.split_heads(v_vec)).collect();

        // Compute attention for each head
        let mut head_outputs = Vec::new();
        for h in 0..self.num_heads {
            let q_h = &q_heads[h];
            let k_h: Vec<&Vec<f32>> = k_heads.iter().map(|heads| &heads[h]).collect();
            let v_h: Vec<&Vec<f32>> = v_heads.iter().map(|heads| &heads[h]).collect();

            let head_output = self.scaled_dot_product_attention(q_h, &k_h, &v_h);
            head_outputs.push(head_output);
        }

        // Concatenate heads
        let concat: Vec<f32> = head_outputs.into_iter().flatten().collect();

        // Final linear projection
        self.out_linear.forward(&concat)
    }

    /// Split vector into multiple heads
    fn split_heads(&self, x: &[f32]) -> Vec<Vec<f32>> {
        let mut heads = Vec::new();
        for h in 0..self.num_heads {
            let start = h * self.head_dim;
            let end = start + self.head_dim;
            heads.push(x[start..end].to_vec());
        }
        heads
    }

    /// Scaled dot-product attention
    fn scaled_dot_product_attention(
        &self,
        query: &[f32],
        keys: &[&Vec<f32>],
        values: &[&Vec<f32>],
    ) -> Vec<f32> {
        if keys.is_empty() {
            return query.to_vec();
        }

        let scale = (self.head_dim as f32).sqrt();

        // Compute attention scores
        let scores: Vec<f32> = keys
            .iter()
            .map(|k| {
                let dot: f32 = query.iter().zip(k.iter()).map(|(q, k)| q * k).sum();
                dot / scale
            })
            .collect();

        // Softmax with epsilon guard against division by zero
        let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let exp_scores: Vec<f32> = scores.iter().map(|&s| (s - max_score).exp()).collect();
        let sum_exp: f32 = exp_scores.iter().sum::<f32>().max(1e-10);
        let attention_weights: Vec<f32> = exp_scores.iter().map(|&e| e / sum_exp).collect();

        // Weighted sum of values
        let mut output = vec![0.0; self.head_dim];
        for (weight, value) in attention_weights.iter().zip(values.iter()) {
            for (out, &val) in output.iter_mut().zip(value.iter()) {
                *out += weight * val;
            }
        }

        output
    }
}

/// Gated Recurrent Unit (GRU) cell for state updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GRUCell {
    // Update gate
    w_z: Linear,
    u_z: Linear,

    // Reset gate
    w_r: Linear,
    u_r: Linear,

    // Candidate hidden state
    w_h: Linear,
    u_h: Linear,
}

impl GRUCell {
    /// Create a new GRU cell
    pub fn new(input_dim: usize, hidden_dim: usize) -> Self {
        Self {
            // Update gate
            w_z: Linear::new(input_dim, hidden_dim),
            u_z: Linear::new(hidden_dim, hidden_dim),

            // Reset gate
            w_r: Linear::new(input_dim, hidden_dim),
            u_r: Linear::new(hidden_dim, hidden_dim),

            // Candidate hidden state
            w_h: Linear::new(input_dim, hidden_dim),
            u_h: Linear::new(hidden_dim, hidden_dim),
        }
    }

    /// Forward pass: update hidden state
    ///
    /// # Arguments
    /// * `input` - Current input
    /// * `hidden` - Previous hidden state
    ///
    /// # Returns
    /// Updated hidden state
    pub fn forward(&self, input: &[f32], hidden: &[f32]) -> Vec<f32> {
        // Update gate: z_t = sigmoid(W_z * x_t + U_z * h_{t-1})
        let z =
            self.sigmoid_vec(&self.add_vecs(&self.w_z.forward(input), &self.u_z.forward(hidden)));

        // Reset gate: r_t = sigmoid(W_r * x_t + U_r * h_{t-1})
        let r =
            self.sigmoid_vec(&self.add_vecs(&self.w_r.forward(input), &self.u_r.forward(hidden)));

        // Candidate hidden state: h_tilde = tanh(W_h * x_t + U_h * (r_t ⊙ h_{t-1}))
        let r_hidden = self.mul_vecs(&r, hidden);
        let h_tilde =
            self.tanh_vec(&self.add_vecs(&self.w_h.forward(input), &self.u_h.forward(&r_hidden)));

        // Final hidden state: h_t = (1 - z_t) ⊙ h_{t-1} + z_t ⊙ h_tilde
        let one_minus_z: Vec<f32> = z.iter().map(|&zval| 1.0 - zval).collect();
        let term1 = self.mul_vecs(&one_minus_z, hidden);
        let term2 = self.mul_vecs(&z, &h_tilde);

        self.add_vecs(&term1, &term2)
    }

    /// Sigmoid activation with numerical stability
    fn sigmoid(&self, x: f32) -> f32 {
        if x > 0.0 {
            1.0 / (1.0 + (-x).exp())
        } else {
            let ex = x.exp();
            ex / (1.0 + ex)
        }
    }

    /// Sigmoid for vectors
    fn sigmoid_vec(&self, v: &[f32]) -> Vec<f32> {
        v.iter().map(|&x| self.sigmoid(x)).collect()
    }

    /// Tanh activation
    fn tanh(&self, x: f32) -> f32 {
        x.tanh()
    }

    /// Tanh for vectors
    fn tanh_vec(&self, v: &[f32]) -> Vec<f32> {
        v.iter().map(|&x| self.tanh(x)).collect()
    }

    /// Element-wise addition
    fn add_vecs(&self, a: &[f32], b: &[f32]) -> Vec<f32> {
        a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
    }

    /// Element-wise multiplication
    fn mul_vecs(&self, a: &[f32], b: &[f32]) -> Vec<f32> {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).collect()
    }
}

/// Main GNN layer operating on HNSW topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvectorLayer {
    /// Message weight matrix
    w_msg: Linear,

    /// Aggregation weight matrix
    w_agg: Linear,

    /// GRU update cell
    w_update: GRUCell,

    /// Multi-head attention
    attention: MultiHeadAttention,

    /// Layer normalization
    norm: LayerNorm,

    /// Dropout rate
    dropout: f32,
}

impl RuvectorLayer {
    /// Create a new Ruvector GNN layer
    ///
    /// # Arguments
    /// * `input_dim` - Dimension of input node embeddings
    /// * `hidden_dim` - Dimension of hidden representations
    /// * `heads` - Number of attention heads
    /// * `dropout` - Dropout rate (0.0 to 1.0)
    ///
    /// # Errors
    /// Returns `GnnError::LayerConfig` if `dropout` is outside `[0.0, 1.0]` or
    /// if `hidden_dim` is not divisible by `heads`.
    pub fn new(
        input_dim: usize,
        hidden_dim: usize,
        heads: usize,
        dropout: f32,
    ) -> Result<Self, GnnError> {
        if !(0.0..=1.0).contains(&dropout) {
            return Err(GnnError::layer_config(format!(
                "Dropout must be between 0.0 and 1.0, got {}",
                dropout
            )));
        }

        Ok(Self {
            w_msg: Linear::new(input_dim, hidden_dim),
            w_agg: Linear::new(hidden_dim, hidden_dim),
            w_update: GRUCell::new(hidden_dim, hidden_dim),
            attention: MultiHeadAttention::new(hidden_dim, heads)?,
            norm: LayerNorm::new(hidden_dim, 1e-5),
            dropout,
        })
    }

    /// Forward pass through the GNN layer
    ///
    /// # Arguments
    /// * `node_embedding` - Current node's embedding
    /// * `neighbor_embeddings` - Embeddings of neighbor nodes
    /// * `edge_weights` - Weights of edges to neighbors (e.g., distances)
    ///
    /// # Returns
    /// Updated node embedding
    pub fn forward(
        &self,
        node_embedding: &[f32],
        neighbor_embeddings: &[Vec<f32>],
        edge_weights: &[f32],
    ) -> Vec<f32> {
        if neighbor_embeddings.is_empty() {
            // No neighbors: return normalized projection
            let projected = self.w_msg.forward(node_embedding);
            return self.norm.forward(&projected);
        }

        // Step 1: Message passing - transform node and neighbor embeddings
        let node_msg = self.w_msg.forward(node_embedding);
        let neighbor_msgs: Vec<Vec<f32>> = neighbor_embeddings
            .iter()
            .map(|n| self.w_msg.forward(n))
            .collect();

        // Step 2: Attention-based aggregation
        let attention_output = self
            .attention
            .forward(&node_msg, &neighbor_msgs, &neighbor_msgs);

        // Step 3: Weighted aggregation using edge weights
        let weighted_msgs = self.aggregate_messages(&neighbor_msgs, edge_weights);

        // Step 4: Combine attention and weighted aggregation
        let combined = self.add_vecs(&attention_output, &weighted_msgs);
        let aggregated = self.w_agg.forward(&combined);

        // Step 5: GRU update
        let updated = self.w_update.forward(&aggregated, &node_msg);

        // Step 6: Apply dropout (simplified - always apply scaling)
        let dropped = self.apply_dropout(&updated);

        // Step 7: Layer normalization
        self.norm.forward(&dropped)
    }

    /// Aggregate neighbor messages with edge weights
    fn aggregate_messages(&self, messages: &[Vec<f32>], weights: &[f32]) -> Vec<f32> {
        if messages.is_empty() || weights.is_empty() {
            return vec![0.0; self.w_msg.output_dim()];
        }

        // Normalize weights to sum to 1
        let weight_sum: f32 = weights.iter().sum();
        let normalized_weights: Vec<f32> = if weight_sum > 0.0 {
            weights.iter().map(|&w| w / weight_sum).collect()
        } else {
            vec![1.0 / weights.len() as f32; weights.len()]
        };

        // Weighted sum
        let dim = messages[0].len();
        let mut aggregated = vec![0.0; dim];

        for (msg, &weight) in messages.iter().zip(normalized_weights.iter()) {
            for (agg, &m) in aggregated.iter_mut().zip(msg.iter()) {
                *agg += weight * m;
            }
        }

        aggregated
    }

    /// Apply dropout (simplified version - just scales by (1-dropout))
    fn apply_dropout(&self, input: &[f32]) -> Vec<f32> {
        let scale = 1.0 - self.dropout;
        input.iter().map(|&x| x * scale).collect()
    }

    /// Element-wise vector addition
    fn add_vecs(&self, a: &[f32], b: &[f32]) -> Vec<f32> {
        a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_layer() {
        let linear = Linear::new(4, 2);
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = linear.forward(&input);
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_layer_norm() {
        let norm = LayerNorm::new(4, 1e-5);
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = norm.forward(&input);

        // Check that output has zero mean (approximately)
        let mean: f32 = output.iter().sum::<f32>() / output.len() as f32;
        assert!((mean).abs() < 1e-5);
    }

    #[test]
    fn test_multihead_attention() {
        let attention = MultiHeadAttention::new(8, 2).unwrap();
        let query = vec![0.5; 8];
        let keys = vec![vec![0.3; 8], vec![0.7; 8]];
        let values = vec![vec![0.2; 8], vec![0.8; 8]];

        let output = attention.forward(&query, &keys, &values);
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_multihead_attention_invalid_dims() {
        let result = MultiHeadAttention::new(10, 3);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("divisible"));
    }

    #[test]
    fn test_gru_cell() {
        let gru = GRUCell::new(4, 8);
        let input = vec![1.0; 4];
        let hidden = vec![0.5; 8];

        let new_hidden = gru.forward(&input, &hidden);
        assert_eq!(new_hidden.len(), 8);
    }

    #[test]
    fn test_ruvector_layer() {
        let layer = RuvectorLayer::new(4, 8, 2, 0.1).unwrap();

        let node = vec![1.0, 2.0, 3.0, 4.0];
        let neighbors = vec![vec![0.5, 1.0, 1.5, 2.0], vec![2.0, 3.0, 4.0, 5.0]];
        let weights = vec![0.3, 0.7];

        let output = layer.forward(&node, &neighbors, &weights);
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_ruvector_layer_no_neighbors() {
        let layer = RuvectorLayer::new(4, 8, 2, 0.1).unwrap();

        let node = vec![1.0, 2.0, 3.0, 4.0];
        let neighbors: Vec<Vec<f32>> = vec![];
        let weights: Vec<f32> = vec![];

        let output = layer.forward(&node, &neighbors, &weights);
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_ruvector_layer_invalid_dropout() {
        let result = RuvectorLayer::new(4, 8, 2, 1.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_ruvector_layer_invalid_heads() {
        let result = RuvectorLayer::new(4, 7, 3, 0.1);
        assert!(result.is_err());
    }
}
