//! Trait definitions for attention mechanisms.
//!
//! This module defines the core traits that all attention mechanisms implement,
//! including standard attention, graph attention, geometric attention, and
//! trainable attention with backward pass support.

use crate::error::AttentionResult;

/// Mask for sparse attention patterns.
#[derive(Clone, Debug)]
pub struct SparseMask {
    /// Row indices for sparse mask
    pub rows: Vec<usize>,
    /// Column indices for sparse mask
    pub cols: Vec<usize>,
    /// Optional values (if not provided, defaults to 1.0)
    pub values: Option<Vec<f32>>,
}

/// Edge information for graph attention.
#[derive(Clone, Debug)]
pub struct EdgeInfo {
    /// Source node index
    pub src: usize,
    /// Destination node index
    pub dst: usize,
    /// Optional edge features
    pub features: Option<Vec<f32>>,
}

/// Core attention mechanism trait.
///
/// Implements the basic attention computation: Attention(Q, K, V) = softmax(QK^T / sqrt(d_k))V
pub trait Attention: Send + Sync {
    /// Computes attention over the given query, keys, and values.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector of shape [d_model]
    /// * `keys` - Slice of key vectors, each of shape [d_model]
    /// * `values` - Slice of value vectors, each of shape [d_model]
    ///
    /// # Returns
    ///
    /// Output vector of shape [d_model]
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>>;

    /// Computes attention with optional mask.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector of shape [d_model]
    /// * `keys` - Slice of key vectors, each of shape [d_model]
    /// * `values` - Slice of value vectors, each of shape [d_model]
    /// * `mask` - Optional attention mask (true = attend, false = mask out)
    ///
    /// # Returns
    ///
    /// Output vector of shape [d_model]
    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>>;

    /// Returns the model dimension.
    fn dim(&self) -> usize;

    /// Returns the number of attention heads (1 for single-head attention).
    fn num_heads(&self) -> usize {
        1
    }
}

/// Graph attention mechanism trait.
///
/// Extends basic attention to operate over graph structures with explicit edges.
pub trait GraphAttention: Attention {
    /// Computes attention using graph structure.
    ///
    /// # Arguments
    ///
    /// * `node_features` - Features for all nodes, shape [num_nodes, d_model]
    /// * `edges` - Edge information (source, destination, optional features)
    ///
    /// # Returns
    ///
    /// Updated node features of shape [num_nodes, d_model]
    fn compute_with_edges(
        &self,
        node_features: &[Vec<f32>],
        edges: &[EdgeInfo],
    ) -> AttentionResult<Vec<Vec<f32>>>;

    /// Computes attention weights for edges.
    ///
    /// # Arguments
    ///
    /// * `src_feature` - Source node feature
    /// * `dst_feature` - Destination node feature
    /// * `edge_feature` - Optional edge feature
    ///
    /// # Returns
    ///
    /// Attention weight for this edge
    fn compute_edge_attention(
        &self,
        src_feature: &[f32],
        dst_feature: &[f32],
        edge_feature: Option<&[f32]>,
    ) -> AttentionResult<f32>;
}

/// Geometric attention mechanism trait.
///
/// Implements attention in hyperbolic or other geometric spaces with curvature.
pub trait GeometricAttention: Attention {
    /// Computes attention in geometric space with specified curvature.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector in geometric space
    /// * `keys` - Key vectors in geometric space
    /// * `values` - Value vectors
    /// * `curvature` - Curvature parameter (negative for hyperbolic space)
    ///
    /// # Returns
    ///
    /// Output vector in geometric space
    fn compute_geometric(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        curvature: f32,
    ) -> AttentionResult<Vec<f32>>;

    /// Projects vector to geometric space.
    fn project_to_geometric(&self, vector: &[f32], curvature: f32) -> AttentionResult<Vec<f32>>;

    /// Projects vector back from geometric space.
    fn project_from_geometric(&self, vector: &[f32], curvature: f32) -> AttentionResult<Vec<f32>>;
}

/// Sparse attention mechanism trait.
///
/// Implements efficient attention over sparse patterns.
pub trait SparseAttention: Attention {
    /// Computes sparse attention using the provided mask.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector
    /// * `keys` - Key vectors
    /// * `values` - Value vectors
    /// * `mask` - Sparse mask defining attention pattern
    ///
    /// # Returns
    ///
    /// Output vector
    fn compute_sparse(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: &SparseMask,
    ) -> AttentionResult<Vec<f32>>;

    /// Generates a sparse mask for the given sequence length.
    ///
    /// # Arguments
    ///
    /// * `seq_len` - Sequence length
    ///
    /// # Returns
    ///
    /// Sparse mask for attention computation
    fn generate_mask(&self, seq_len: usize) -> AttentionResult<SparseMask>;
}

/// Gradient information for backward pass.
#[derive(Clone, Debug)]
pub struct Gradients {
    /// Gradient w.r.t. query
    pub query_grad: Vec<f32>,
    /// Gradient w.r.t. keys
    pub keys_grad: Vec<Vec<f32>>,
    /// Gradient w.r.t. values
    pub values_grad: Vec<Vec<f32>>,
    /// Gradient w.r.t. attention weights (for analysis)
    pub attention_weights_grad: Option<Vec<f32>>,
}

/// Trainable attention mechanism with backward pass support.
pub trait TrainableAttention: Attention {
    /// Forward pass with gradient tracking.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector
    /// * `keys` - Key vectors
    /// * `values` - Value vectors
    ///
    /// # Returns
    ///
    /// Tuple of (output, attention_weights) for gradient computation
    fn forward(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<(Vec<f32>, Vec<f32>)>;

    /// Backward pass for gradient computation.
    ///
    /// # Arguments
    ///
    /// * `grad_output` - Gradient from downstream layers
    /// * `query` - Query from forward pass
    /// * `keys` - Keys from forward pass
    /// * `values` - Values from forward pass
    /// * `attention_weights` - Attention weights from forward pass
    ///
    /// # Returns
    ///
    /// Gradients w.r.t. inputs
    fn backward(
        &self,
        grad_output: &[f32],
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        attention_weights: &[f32],
    ) -> AttentionResult<Gradients>;

    /// Updates parameters using computed gradients.
    ///
    /// # Arguments
    ///
    /// * `gradients` - Computed gradients
    /// * `learning_rate` - Learning rate for update
    fn update_parameters(
        &mut self,
        gradients: &Gradients,
        learning_rate: f32,
    ) -> AttentionResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_mask_creation() {
        let mask = SparseMask {
            rows: vec![0, 1, 2],
            cols: vec![0, 1, 2],
            values: None,
        };

        assert_eq!(mask.rows.len(), 3);
        assert_eq!(mask.cols.len(), 3);
        assert!(mask.values.is_none());
    }

    #[test]
    fn test_edge_info_creation() {
        let edge = EdgeInfo {
            src: 0,
            dst: 1,
            features: Some(vec![0.5, 0.3]),
        };

        assert_eq!(edge.src, 0);
        assert_eq!(edge.dst, 1);
        assert_eq!(edge.features.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_gradients_creation() {
        let grads = Gradients {
            query_grad: vec![0.1, 0.2],
            keys_grad: vec![vec![0.3, 0.4]],
            values_grad: vec![vec![0.5, 0.6]],
            attention_weights_grad: None,
        };

        assert_eq!(grads.query_grad.len(), 2);
        assert_eq!(grads.keys_grad.len(), 1);
        assert!(grads.attention_weights_grad.is_none());
    }
}
