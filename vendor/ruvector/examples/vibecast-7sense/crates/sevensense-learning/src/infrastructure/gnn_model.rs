//! GNN model implementation.
//!
//! This module provides Graph Neural Network implementations including:
//! - GCN (Graph Convolutional Network)
//! - GraphSAGE (Sample and Aggregate)
//! - GAT (Graph Attention Network)

use ndarray::{Array1, Array2};
use rand::Rng;
use rand_distr::{Distribution, Normal, Uniform};
use serde::{Deserialize, Serialize};

use crate::domain::entities::GnnModelType;
use crate::infrastructure::attention::AttentionLayer;

/// Error type for GNN operations
#[derive(Debug, thiserror::Error)]
pub enum GnnError {
    /// Dimension mismatch
    #[error("Dimension mismatch: {0}")]
    DimensionMismatch(String),

    /// Invalid layer configuration
    #[error("Invalid layer configuration: {0}")]
    InvalidConfig(String),

    /// Computation error
    #[error("Computation error: {0}")]
    ComputationError(String),
}

/// Result type for GNN operations
pub type GnnResult<T> = Result<T, GnnError>;

/// Aggregation method for GraphSAGE
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aggregator {
    /// Mean aggregation
    Mean,
    /// Sum aggregation (GCN-style)
    Sum,
    /// Max pooling aggregation
    MaxPool,
    /// LSTM aggregation (sequence-aware)
    Lstm,
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::Mean
    }
}

/// A single layer in a GNN
#[derive(Debug, Clone)]
pub enum GnnLayer {
    /// Graph Convolutional Network layer
    /// H' = σ(D^(-1/2) A D^(-1/2) H W + b)
    Gcn {
        /// Weight matrix (input_dim x output_dim)
        weights: Array2<f32>,
        /// Bias vector (output_dim)
        bias: Array1<f32>,
    },
    /// GraphSAGE layer
    /// h_v' = σ(W * CONCAT(h_v, AGGREGATE({h_u : u ∈ N(v)})))
    GraphSage {
        /// Aggregation method
        aggregator: Aggregator,
        /// Self-weight matrix
        self_weights: Array2<f32>,
        /// Neighbor-weight matrix
        neighbor_weights: Array2<f32>,
        /// Bias vector
        bias: Array1<f32>,
    },
    /// Graph Attention Network layer
    /// Uses attention mechanism to weight neighbor contributions
    Gat {
        /// Weight matrix for linear transformation
        weights: Array2<f32>,
        /// Attention weights (2 * output_dim)
        attention_weights: Array1<f32>,
        /// Number of attention heads
        num_heads: usize,
        /// Bias vector
        bias: Array1<f32>,
        /// Leaky ReLU negative slope
        negative_slope: f32,
    },
}

impl GnnLayer {
    /// Create a new GCN layer
    pub fn gcn(input_dim: usize, output_dim: usize) -> Self {
        let weights = xavier_init(input_dim, output_dim);
        let bias = Array1::zeros(output_dim);
        Self::Gcn { weights, bias }
    }

    /// Create a new GraphSAGE layer
    pub fn graph_sage(input_dim: usize, output_dim: usize, aggregator: Aggregator) -> Self {
        let self_weights = xavier_init(input_dim, output_dim);
        let neighbor_weights = xavier_init(input_dim, output_dim);
        let bias = Array1::zeros(output_dim);
        Self::GraphSage {
            aggregator,
            self_weights,
            neighbor_weights,
            bias,
        }
    }

    /// Create a new GAT layer
    pub fn gat(input_dim: usize, output_dim: usize, num_heads: usize) -> Self {
        let weights = xavier_init(input_dim, output_dim * num_heads);
        // Initialize attention weights with small values
        let mut rng = rand::thread_rng();
        let uniform = Uniform::new(-0.1, 0.1);
        let attention_weights: Array1<f32> = Array1::from_iter(
            (0..2 * output_dim).map(|_| uniform.sample(&mut rng)),
        );
        let bias = Array1::zeros(output_dim * num_heads);
        Self::Gat {
            weights,
            attention_weights,
            num_heads,
            bias,
            negative_slope: 0.2,
        }
    }

    /// Get the input dimension
    #[must_use]
    pub fn input_dim(&self) -> usize {
        match self {
            Self::Gcn { weights, .. } => weights.nrows(),
            Self::GraphSage { self_weights, .. } => self_weights.nrows(),
            Self::Gat { weights, .. } => weights.nrows(),
        }
    }

    /// Get the output dimension
    #[must_use]
    pub fn output_dim(&self) -> usize {
        match self {
            Self::Gcn { weights, .. } => weights.ncols(),
            Self::GraphSage { self_weights, .. } => self_weights.ncols(),
            Self::Gat { weights, num_heads, .. } => weights.ncols() / num_heads,
        }
    }

    /// Forward pass through the layer
    pub fn forward(&self, features: &Array2<f32>, adj_matrix: &Array2<f32>) -> GnnResult<Array2<f32>> {
        match self {
            Self::Gcn { weights, bias } => self.gcn_forward(features, adj_matrix, weights, bias),
            Self::GraphSage {
                aggregator,
                self_weights,
                neighbor_weights,
                bias,
            } => self.sage_forward(features, adj_matrix, *aggregator, self_weights, neighbor_weights, bias),
            Self::Gat {
                weights,
                attention_weights,
                num_heads,
                bias,
                negative_slope,
            } => self.gat_forward(features, adj_matrix, weights, attention_weights, *num_heads, bias, *negative_slope),
        }
    }

    fn gcn_forward(
        &self,
        features: &Array2<f32>,
        adj_matrix: &Array2<f32>,
        weights: &Array2<f32>,
        bias: &Array1<f32>,
    ) -> GnnResult<Array2<f32>> {
        // H' = σ(A_norm * H * W + b)
        // A_norm is already normalized (symmetric normalization)

        // Aggregate neighbor features: AH
        let aggregated = adj_matrix.dot(features);

        // Transform: AH * W
        let transformed = aggregated.dot(weights);

        // Add bias and apply activation
        let mut output = transformed;
        for mut row in output.rows_mut() {
            for (i, val) in row.iter_mut().enumerate() {
                *val = relu(*val + bias[i]);
            }
        }

        Ok(output)
    }

    fn sage_forward(
        &self,
        features: &Array2<f32>,
        adj_matrix: &Array2<f32>,
        aggregator: Aggregator,
        self_weights: &Array2<f32>,
        neighbor_weights: &Array2<f32>,
        bias: &Array1<f32>,
    ) -> GnnResult<Array2<f32>> {
        let n = features.nrows();
        let out_dim = self_weights.ncols();

        // Aggregate neighbor features
        let neighbor_agg = match aggregator {
            Aggregator::Mean => {
                // Mean aggregation
                let mut agg = adj_matrix.dot(features);
                let degrees: Vec<f32> = (0..n).map(|i| adj_matrix.row(i).sum().max(1.0)).collect();
                for (i, mut row) in agg.rows_mut().into_iter().enumerate() {
                    row /= degrees[i];
                }
                agg
            }
            Aggregator::Sum => {
                // Sum aggregation
                adj_matrix.dot(features)
            }
            Aggregator::MaxPool => {
                // Max pooling
                let mut agg = Array2::zeros((n, features.ncols()));
                for i in 0..n {
                    for j in 0..features.ncols() {
                        let mut max_val = f32::NEG_INFINITY;
                        for k in 0..n {
                            if adj_matrix[[i, k]] > 0.0 {
                                max_val = max_val.max(features[[k, j]]);
                            }
                        }
                        agg[[i, j]] = if max_val.is_finite() { max_val } else { 0.0 };
                    }
                }
                agg
            }
            Aggregator::Lstm => {
                // Simplified LSTM: just use mean for now
                // Full implementation would use actual LSTM
                adj_matrix.dot(features)
            }
        };

        // Transform self and neighbor features
        let self_transformed = features.dot(self_weights);
        let neighbor_transformed = neighbor_agg.dot(neighbor_weights);

        // Combine: concat and add bias
        let mut output = Array2::zeros((n, out_dim));
        for i in 0..n {
            for j in 0..out_dim {
                let val = self_transformed[[i, j]] + neighbor_transformed[[i, j]] + bias[j];
                output[[i, j]] = relu(val);
            }
        }

        Ok(output)
    }

    fn gat_forward(
        &self,
        features: &Array2<f32>,
        adj_matrix: &Array2<f32>,
        weights: &Array2<f32>,
        attention_weights: &Array1<f32>,
        num_heads: usize,
        bias: &Array1<f32>,
        negative_slope: f32,
    ) -> GnnResult<Array2<f32>> {
        let n = features.nrows();
        let total_out_dim = weights.ncols();
        let head_dim = total_out_dim / num_heads;

        // Transform features: H * W
        let transformed = features.dot(weights);

        // Multi-head attention
        let mut outputs = Vec::with_capacity(num_heads);

        for head in 0..num_heads {
            let start = head * head_dim;
            let end = start + head_dim;

            // Extract features for this head
            let h = transformed.slice(ndarray::s![.., start..end]).to_owned();

            // Compute attention coefficients
            let mut attention = Array2::zeros((n, n));
            let attention_dim = attention_weights.len() / 2;
            let a_src = attention_weights.slice(ndarray::s![..attention_dim]);
            let a_dst = attention_weights.slice(ndarray::s![attention_dim..]);

            for i in 0..n {
                for j in 0..n {
                    if adj_matrix[[i, j]] > 0.0 {
                        // Compute attention: a^T [Wh_i || Wh_j]
                        let mut e = 0.0;
                        for k in 0..head_dim.min(attention_dim) {
                            e += a_src[k] * h[[i, k]] + a_dst[k] * h[[j, k]];
                        }
                        // Leaky ReLU
                        attention[[i, j]] = leaky_relu(e, negative_slope);
                    } else {
                        attention[[i, j]] = f32::NEG_INFINITY;
                    }
                }
            }

            // Softmax over neighbors
            for mut row in attention.rows_mut() {
                let max_val = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let mut sum = 0.0;
                for val in row.iter_mut() {
                    if val.is_finite() {
                        *val = (*val - max_val).exp();
                        sum += *val;
                    } else {
                        *val = 0.0;
                    }
                }
                if sum > 0.0 {
                    row /= sum;
                }
            }

            // Apply attention
            let head_output = attention.dot(&h);
            outputs.push(head_output);
        }

        // Concatenate head outputs
        let mut output = Array2::zeros((n, total_out_dim));
        for (head, head_out) in outputs.iter().enumerate() {
            let start = head * head_dim;
            for i in 0..n {
                for j in 0..head_dim {
                    output[[i, start + j]] = head_out[[i, j]];
                }
            }
        }

        // Add bias and apply activation
        for mut row in output.rows_mut() {
            for (i, val) in row.iter_mut().enumerate() {
                if i < bias.len() {
                    *val = elu(*val + bias[i], 1.0);
                }
            }
        }

        Ok(output)
    }

    /// Update weights with gradients
    pub fn update_weights(&mut self, gradient: &Array2<f32>, lr: f32, weight_decay: f32) {
        match self {
            Self::Gcn { weights, bias: _ } => {
                // Apply weight decay
                *weights -= &(weights.clone() * weight_decay);
                // Apply gradient update
                if gradient.shape() == weights.shape() {
                    *weights -= &(gradient * lr);
                }
            }
            Self::GraphSage {
                self_weights,
                neighbor_weights,
                bias: _,
                ..
            } => {
                *self_weights -= &(self_weights.clone() * weight_decay);
                *neighbor_weights -= &(neighbor_weights.clone() * weight_decay);
                if gradient.shape() == self_weights.shape() {
                    *self_weights -= &(gradient * lr);
                    *neighbor_weights -= &(gradient * lr);
                }
            }
            Self::Gat { weights, bias: _, .. } => {
                *weights -= &(weights.clone() * weight_decay);
                if gradient.shape() == weights.shape() {
                    *weights -= &(gradient * lr);
                }
            }
        }
    }

    /// Get layer parameters as flattened vector
    #[must_use]
    pub fn get_parameters(&self) -> Vec<f32> {
        match self {
            Self::Gcn { weights, bias } => {
                let mut params: Vec<f32> = weights.iter().cloned().collect();
                params.extend(bias.iter().cloned());
                params
            }
            Self::GraphSage {
                self_weights,
                neighbor_weights,
                bias,
                ..
            } => {
                let mut params: Vec<f32> = self_weights.iter().cloned().collect();
                params.extend(neighbor_weights.iter().cloned());
                params.extend(bias.iter().cloned());
                params
            }
            Self::Gat {
                weights,
                attention_weights,
                bias,
                ..
            } => {
                let mut params: Vec<f32> = weights.iter().cloned().collect();
                params.extend(attention_weights.iter().cloned());
                params.extend(bias.iter().cloned());
                params
            }
        }
    }
}

/// Complete GNN model with multiple layers
#[derive(Debug, Clone)]
pub struct GnnModel {
    /// Model type
    model_type: GnnModelType,
    /// Stacked GNN layers
    layers: Vec<GnnLayer>,
    /// Optional attention layer for final aggregation
    attention: Option<AttentionLayer>,
    /// Dropout probability (applied during training)
    dropout: f32,
    /// Whether the model is in training mode
    training: bool,
}

impl GnnModel {
    /// Create a new GNN model
    #[must_use]
    pub fn new(
        model_type: GnnModelType,
        input_dim: usize,
        output_dim: usize,
        num_layers: usize,
        hidden_dim: usize,
        num_heads: usize,
        dropout: f32,
    ) -> Self {
        let mut layers = Vec::with_capacity(num_layers);

        for i in 0..num_layers {
            let in_dim = if i == 0 { input_dim } else { hidden_dim };
            let out_dim = if i == num_layers - 1 {
                output_dim
            } else {
                hidden_dim
            };

            let layer = match model_type {
                GnnModelType::Gcn => GnnLayer::gcn(in_dim, out_dim),
                GnnModelType::GraphSage => GnnLayer::graph_sage(in_dim, out_dim, Aggregator::Mean),
                GnnModelType::Gat => GnnLayer::gat(in_dim, out_dim, num_heads),
            };

            layers.push(layer);
        }

        // Add attention layer for graph-level readout
        let attention = if num_layers > 0 {
            Some(AttentionLayer::new(output_dim, 64))
        } else {
            None
        };

        Self {
            model_type,
            layers,
            attention,
            dropout,
            training: true,
        }
    }

    /// Set training mode
    pub fn train(&mut self) {
        self.training = true;
    }

    /// Set evaluation mode
    pub fn eval(&mut self) {
        self.training = false;
    }

    /// Get the model type
    #[must_use]
    pub fn model_type(&self) -> GnnModelType {
        self.model_type
    }

    /// Get the number of layers
    #[must_use]
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Get layer dimensions
    #[must_use]
    pub fn layer_dims(&self, layer_idx: usize) -> (usize, usize) {
        if layer_idx < self.layers.len() {
            (
                self.layers[layer_idx].input_dim(),
                self.layers[layer_idx].output_dim(),
            )
        } else {
            (0, 0)
        }
    }

    /// Forward pass through the model
    pub fn forward(
        &self,
        features: &Array2<f32>,
        adj_matrix: &Array2<f32>,
    ) -> GnnResult<Array2<f32>> {
        let mut h = features.clone();

        for (i, layer) in self.layers.iter().enumerate() {
            h = layer.forward(&h, adj_matrix)?;

            // Apply dropout (except on last layer)
            if self.training && i < self.layers.len() - 1 {
                h = self.apply_dropout(&h);
            }
        }

        Ok(h)
    }

    /// Update model weights with gradients
    pub fn update_weights(
        &mut self,
        gradients: &[Array2<f32>],
        lr: f32,
        weight_decay: f32,
    ) {
        for (layer, grad) in self.layers.iter_mut().zip(gradients.iter()) {
            layer.update_weights(grad, lr, weight_decay);
        }
    }

    /// Get all model parameters as flattened vector
    #[must_use]
    pub fn get_parameters(&self) -> Vec<f32> {
        let mut params = Vec::new();
        for layer in &self.layers {
            params.extend(layer.get_parameters());
        }
        params
    }

    /// Get total number of parameters
    #[must_use]
    pub fn num_parameters(&self) -> usize {
        self.get_parameters().len()
    }

    fn apply_dropout(&self, features: &Array2<f32>) -> Array2<f32> {
        if self.dropout <= 0.0 || !self.training {
            return features.clone();
        }

        let mut rng = rand::thread_rng();
        let scale = 1.0 / (1.0 - self.dropout);

        let mut dropped = features.clone();
        for val in dropped.iter_mut() {
            if rng.gen::<f32>() < self.dropout {
                *val = 0.0;
            } else {
                *val *= scale;
            }
        }

        dropped
    }
}

// =========== Activation Functions ===========

/// ReLU activation
fn relu(x: f32) -> f32 {
    x.max(0.0)
}

/// Leaky ReLU activation
fn leaky_relu(x: f32, negative_slope: f32) -> f32 {
    if x >= 0.0 {
        x
    } else {
        negative_slope * x
    }
}

/// ELU activation
fn elu(x: f32, alpha: f32) -> f32 {
    if x >= 0.0 {
        x
    } else {
        alpha * (x.exp() - 1.0)
    }
}

/// GELU activation (Gaussian Error Linear Unit)
#[allow(dead_code)]
fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + (x * 0.7978845608 * (1.0 + 0.044715 * x * x)).tanh())
}

// =========== Initialization ===========

/// Xavier/Glorot uniform initialization
fn xavier_init(fan_in: usize, fan_out: usize) -> Array2<f32> {
    let limit = (6.0 / (fan_in + fan_out) as f32).sqrt();
    let uniform = Uniform::new(-limit, limit);
    let mut rng = rand::thread_rng();

    Array2::from_shape_fn((fan_in, fan_out), |_| uniform.sample(&mut rng))
}

/// Kaiming/He initialization (for ReLU)
#[allow(dead_code)]
fn kaiming_init(fan_in: usize, fan_out: usize) -> Array2<f32> {
    let std = (2.0 / fan_in as f32).sqrt();
    let normal = Normal::new(0.0, std).unwrap();
    let mut rng = rand::thread_rng();

    Array2::from_shape_fn((fan_in, fan_out), |_| normal.sample(&mut rng))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcn_layer() {
        let layer = GnnLayer::gcn(8, 4);
        assert_eq!(layer.input_dim(), 8);
        assert_eq!(layer.output_dim(), 4);

        let features = Array2::from_elem((3, 8), 0.5);
        let adj = Array2::<f32>::eye(3);

        let output = layer.forward(&features, &adj).unwrap();
        assert_eq!(output.shape(), &[3, 4]);
    }

    #[test]
    fn test_graphsage_layer() {
        let layer = GnnLayer::graph_sage(8, 4, Aggregator::Mean);
        assert_eq!(layer.input_dim(), 8);
        assert_eq!(layer.output_dim(), 4);

        let features = Array2::from_elem((3, 8), 0.5);
        let adj = Array2::<f32>::eye(3);

        let output = layer.forward(&features, &adj).unwrap();
        assert_eq!(output.shape(), &[3, 4]);
    }

    #[test]
    fn test_gat_layer() {
        let layer = GnnLayer::gat(8, 4, 2);
        assert_eq!(layer.input_dim(), 8);

        let features = Array2::from_elem((3, 8), 0.5);
        let adj = Array2::<f32>::eye(3);

        let output = layer.forward(&features, &adj).unwrap();
        assert_eq!(output.shape(), &[3, 8]); // 4 * 2 heads
    }

    #[test]
    fn test_gnn_model() {
        let model = GnnModel::new(GnnModelType::Gcn, 16, 8, 2, 32, 4, 0.5);

        assert_eq!(model.model_type(), GnnModelType::Gcn);
        assert_eq!(model.num_layers(), 2);

        let features = Array2::from_elem((5, 16), 0.5);
        let adj = Array2::<f32>::eye(5);

        let output = model.forward(&features, &adj).unwrap();
        assert_eq!(output.shape(), &[5, 8]);
    }

    #[test]
    fn test_activation_functions() {
        assert_eq!(relu(-1.0), 0.0);
        assert_eq!(relu(1.0), 1.0);

        assert!(leaky_relu(-1.0, 0.2) < 0.0);
        assert_eq!(leaky_relu(1.0, 0.2), 1.0);

        assert!(elu(-1.0, 1.0) < 0.0);
        assert_eq!(elu(1.0, 1.0), 1.0);
    }

    #[test]
    fn test_xavier_init() {
        let weights = xavier_init(100, 100);
        assert_eq!(weights.shape(), &[100, 100]);

        // Check values are in reasonable range
        let max = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min = weights.iter().cloned().fold(f32::INFINITY, f32::min);

        assert!(max < 1.0);
        assert!(min > -1.0);
    }

    #[test]
    fn test_model_parameters() {
        let model = GnnModel::new(GnnModelType::Gcn, 8, 4, 2, 16, 1, 0.0);

        let params = model.get_parameters();
        assert!(params.len() > 0);

        // Layer 1: 8*16 + 16 = 144
        // Layer 2: 16*4 + 4 = 68
        // Total: 212
        assert_eq!(model.num_parameters(), 8 * 16 + 16 + 16 * 4 + 4);
    }

    #[test]
    fn test_aggregators() {
        let features = Array2::from_shape_vec((3, 4), vec![
            1.0, 2.0, 3.0, 4.0,
            5.0, 6.0, 7.0, 8.0,
            9.0, 10.0, 11.0, 12.0,
        ]).unwrap();

        let mut adj = Array2::zeros((3, 3));
        adj[[0, 1]] = 1.0;
        adj[[0, 2]] = 1.0;
        adj[[1, 0]] = 1.0;
        adj[[2, 0]] = 1.0;

        // Test mean aggregation
        let layer = GnnLayer::graph_sage(4, 2, Aggregator::Mean);
        let output = layer.forward(&features, &adj).unwrap();
        assert_eq!(output.shape(), &[3, 2]);

        // Test max aggregation
        let layer = GnnLayer::graph_sage(4, 2, Aggregator::MaxPool);
        let output = layer.forward(&features, &adj).unwrap();
        assert_eq!(output.shape(), &[3, 2]);
    }
}
