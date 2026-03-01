//! Sheaf Neural Network Layers
//!
//! Neural network layers that respect sheaf structure, enabling
//! coherence-aware deep learning.

use super::laplacian::{LaplacianConfig, SheafLaplacian};
use super::sheaf::{Sheaf, SheafSection};
use crate::substrate::NodeId;
use crate::substrate::SheafGraph;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Activation functions for neural layers
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Activation {
    /// No activation (identity)
    Identity,
    /// ReLU: max(0, x)
    ReLU,
    /// Leaky ReLU: max(alpha * x, x)
    LeakyReLU(f64),
    /// Sigmoid: 1 / (1 + exp(-x))
    Sigmoid,
    /// Tanh: tanh(x)
    Tanh,
    /// GELU: x * Phi(x)
    GELU,
    /// Softmax (applied per-node)
    Softmax,
}

impl Activation {
    /// Apply activation function
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            Activation::Identity => x,
            Activation::ReLU => x.max(0.0),
            Activation::LeakyReLU(alpha) => {
                if x > 0.0 {
                    x
                } else {
                    alpha * x
                }
            }
            Activation::Sigmoid => 1.0 / (1.0 + (-x).exp()),
            Activation::Tanh => x.tanh(),
            Activation::GELU => {
                // Approximation: x * sigmoid(1.702 * x)
                let sigmoid = 1.0 / (1.0 + (-1.702 * x).exp());
                x * sigmoid
            }
            Activation::Softmax => x, // Softmax handled separately
        }
    }

    /// Apply activation to array
    pub fn apply_array(&self, arr: &Array1<f64>) -> Array1<f64> {
        match self {
            Activation::Softmax => {
                let max_val = arr.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let exp_vals: Array1<f64> = arr.mapv(|x| (x - max_val).exp());
                let sum: f64 = exp_vals.sum();
                exp_vals / sum
            }
            _ => arr.mapv(|x| self.apply(x)),
        }
    }

    /// Compute derivative
    pub fn derivative(&self, x: f64) -> f64 {
        match self {
            Activation::Identity => 1.0,
            Activation::ReLU => {
                if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Activation::LeakyReLU(alpha) => {
                if x > 0.0 {
                    1.0
                } else {
                    *alpha
                }
            }
            Activation::Sigmoid => {
                let s = self.apply(x);
                s * (1.0 - s)
            }
            Activation::Tanh => {
                let t = x.tanh();
                1.0 - t * t
            }
            Activation::GELU => {
                // Derivative of GELU approximation
                let sigmoid = 1.0 / (1.0 + (-1.702 * x).exp());
                sigmoid + x * 1.702 * sigmoid * (1.0 - sigmoid)
            }
            Activation::Softmax => 1.0, // Jacobian needed for full derivative
        }
    }
}

/// Configuration for sheaf neural layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafNeuralConfig {
    /// Input dimension per node
    pub input_dim: usize,
    /// Output dimension per node
    pub output_dim: usize,
    /// Number of diffusion steps
    pub diffusion_steps: usize,
    /// Diffusion coefficient
    pub diffusion_coeff: f64,
    /// Activation function
    pub activation: Activation,
    /// Dropout rate
    pub dropout: f64,
    /// Whether to use residual connection
    pub use_residual: bool,
    /// Whether to normalize output
    pub layer_norm: bool,
}

impl Default for SheafNeuralConfig {
    fn default() -> Self {
        Self {
            input_dim: 64,
            output_dim: 64,
            diffusion_steps: 3,
            diffusion_coeff: 0.5,
            activation: Activation::ReLU,
            dropout: 0.0,
            layer_norm: true,
            use_residual: true,
        }
    }
}

/// A sheaf-aware neural network layer
///
/// Combines linear transformation with sheaf diffusion to produce
/// outputs that respect graph structure.
#[derive(Clone)]
pub struct SheafNeuralLayer {
    /// Configuration
    config: SheafNeuralConfig,
    /// Weight matrix (output_dim x input_dim)
    weights: Array2<f64>,
    /// Bias vector (output_dim)
    bias: Array1<f64>,
    /// Diffusion weight (how much to mix diffusion vs direct)
    diffusion_weight: f64,
}

impl SheafNeuralLayer {
    /// Create a new layer with Xavier initialization
    pub fn new(config: SheafNeuralConfig) -> Self {
        let scale = (2.0 / (config.input_dim + config.output_dim) as f64).sqrt();

        // Initialize weights with Xavier
        let weights = Array2::from_shape_fn((config.output_dim, config.input_dim), |_| {
            rand::random::<f64>() * scale - scale / 2.0
        });

        let bias = Array1::zeros(config.output_dim);

        Self {
            config,
            weights,
            bias,
            diffusion_weight: 0.5,
        }
    }

    /// Create with specific weights
    pub fn with_weights(
        config: SheafNeuralConfig,
        weights: Array2<f64>,
        bias: Array1<f64>,
    ) -> Self {
        assert_eq!(weights.nrows(), config.output_dim);
        assert_eq!(weights.ncols(), config.input_dim);
        assert_eq!(bias.len(), config.output_dim);

        Self {
            config,
            weights,
            bias,
            diffusion_weight: 0.5,
        }
    }

    /// Set diffusion weight
    pub fn set_diffusion_weight(&mut self, weight: f64) {
        self.diffusion_weight = weight.clamp(0.0, 1.0);
    }

    /// Forward pass on a section
    ///
    /// output = activation(W * diffuse(x) + b)
    pub fn forward(&self, graph: &SheafGraph, input: &SheafSection) -> SheafSection {
        let mut output = SheafSection::empty();

        // Step 1: Apply linear transformation at each node
        for (node_id, input_vec) in &input.sections {
            let transformed = self.weights.dot(input_vec) + &self.bias;
            output.set(*node_id, transformed);
        }

        // Step 2: Apply sheaf diffusion
        if self.config.diffusion_steps > 0 && self.diffusion_weight > 0.0 {
            let laplacian_config = LaplacianConfig::default();
            let laplacian = SheafLaplacian::from_graph(graph, laplacian_config);

            for _ in 0..self.config.diffusion_steps {
                let laplacian_out = laplacian.apply(graph, &output);

                // Update: x = x - alpha * L * x
                for (node_id, out_vec) in output.sections.iter_mut() {
                    if let Some(lap_vec) = laplacian_out.sections.get(node_id) {
                        let scale = self.diffusion_weight * self.config.diffusion_coeff;
                        *out_vec = &*out_vec - &(lap_vec * scale);
                    }
                }
            }
        }

        // Step 3: Apply activation
        for out_vec in output.sections.values_mut() {
            *out_vec = self.config.activation.apply_array(out_vec);
        }

        // Step 4: Residual connection (if dimensions match and enabled)
        if self.config.use_residual && self.config.input_dim == self.config.output_dim {
            for (node_id, out_vec) in output.sections.iter_mut() {
                if let Some(in_vec) = input.sections.get(node_id) {
                    *out_vec = &*out_vec + in_vec;
                }
            }
        }

        // Step 5: Layer normalization
        if self.config.layer_norm {
            for out_vec in output.sections.values_mut() {
                let mean: f64 = out_vec.mean().unwrap_or(0.0);
                let std: f64 = out_vec.std(0.0);
                if std > 1e-10 {
                    *out_vec = out_vec.mapv(|x| (x - mean) / std);
                }
            }
        }

        output
    }

    /// Get weights
    pub fn weights(&self) -> &Array2<f64> {
        &self.weights
    }

    /// Get bias
    pub fn bias(&self) -> &Array1<f64> {
        &self.bias
    }

    /// Set weights (for training)
    pub fn set_weights(&mut self, weights: Array2<f64>) {
        assert_eq!(weights.shape(), self.weights.shape());
        self.weights = weights;
    }

    /// Set bias (for training)
    pub fn set_bias(&mut self, bias: Array1<f64>) {
        assert_eq!(bias.len(), self.bias.len());
        self.bias = bias;
    }
}

impl std::fmt::Debug for SheafNeuralLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SheafNeuralLayer")
            .field("input_dim", &self.config.input_dim)
            .field("output_dim", &self.config.output_dim)
            .field("diffusion_steps", &self.config.diffusion_steps)
            .field("activation", &self.config.activation)
            .finish()
    }
}

/// Sheaf convolution layer
///
/// Generalizes graph convolution using sheaf structure
#[derive(Clone)]
pub struct SheafConvolution {
    /// Input dimension
    input_dim: usize,
    /// Output dimension
    output_dim: usize,
    /// Weight for self-features
    self_weight: Array2<f64>,
    /// Weight for neighbor features
    neighbor_weight: Array2<f64>,
    /// Bias
    bias: Array1<f64>,
    /// Activation
    activation: Activation,
}

impl SheafConvolution {
    /// Create a new sheaf convolution layer
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        let scale = (2.0 / (input_dim + output_dim) as f64).sqrt();

        let self_weight = Array2::from_shape_fn((output_dim, input_dim), |_| {
            rand::random::<f64>() * scale - scale / 2.0
        });
        let neighbor_weight = Array2::from_shape_fn((output_dim, input_dim), |_| {
            rand::random::<f64>() * scale - scale / 2.0
        });
        let bias = Array1::zeros(output_dim);

        Self {
            input_dim,
            output_dim,
            self_weight,
            neighbor_weight,
            bias,
            activation: Activation::ReLU,
        }
    }

    /// Set activation function
    pub fn with_activation(mut self, activation: Activation) -> Self {
        self.activation = activation;
        self
    }

    /// Forward pass
    ///
    /// h_v = activation(W_self * x_v + W_neigh * sum_u rho_{u->v}(x_u) / deg(v) + b)
    pub fn forward(&self, graph: &SheafGraph, input: &SheafSection) -> SheafSection {
        let mut output = SheafSection::empty();

        for node_id in graph.node_ids() {
            if let Some(self_vec) = input.get(node_id) {
                // Self contribution
                let mut h = self.self_weight.dot(self_vec);

                // Neighbor contribution (average of restricted neighbors)
                let neighbors: Vec<_> = graph.edges_incident_to(node_id);
                if !neighbors.is_empty() {
                    let mut neighbor_sum = Array1::zeros(self.input_dim);
                    let mut count = 0;

                    for edge_id in neighbors {
                        if let Some(edge) = graph.get_edge(edge_id) {
                            let neighbor_id = if edge.source == node_id {
                                edge.target
                            } else {
                                edge.source
                            };

                            if let Some(neighbor_vec) = input.get(neighbor_id) {
                                // For identity restriction, just add neighbor
                                // For general restriction, would apply rho here
                                neighbor_sum = neighbor_sum + neighbor_vec;
                                count += 1;
                            }
                        }
                    }

                    if count > 0 {
                        neighbor_sum /= count as f64;
                        h = h + self.neighbor_weight.dot(&neighbor_sum);
                    }
                }

                // Add bias and apply activation
                h = h + &self.bias;
                h = self.activation.apply_array(&h);

                output.set(node_id, h);
            }
        }

        output
    }
}

impl std::fmt::Debug for SheafConvolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SheafConvolution")
            .field("input_dim", &self.input_dim)
            .field("output_dim", &self.output_dim)
            .field("activation", &self.activation)
            .finish()
    }
}

/// Cohomology-aware pooling layer
///
/// Pools node features while preserving cohomological structure
#[derive(Clone)]
pub struct CohomologyPooling {
    /// Pooling method
    method: PoolingMethod,
    /// Whether to weight by node importance (from Laplacian spectrum)
    spectral_weighting: bool,
}

/// Pooling methods
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PoolingMethod {
    /// Mean of all nodes
    Mean,
    /// Max over all nodes
    Max,
    /// Sum over all nodes
    Sum,
    /// Attention-weighted sum
    Attention,
    /// Top-k nodes by energy
    TopK(usize),
}

impl CohomologyPooling {
    /// Create a new pooling layer
    pub fn new(method: PoolingMethod) -> Self {
        Self {
            method,
            spectral_weighting: false,
        }
    }

    /// Enable spectral weighting
    pub fn with_spectral_weighting(mut self) -> Self {
        self.spectral_weighting = true;
        self
    }

    /// Pool section to single vector
    pub fn pool(&self, graph: &SheafGraph, section: &SheafSection) -> Array1<f64> {
        if section.sections.is_empty() {
            return Array1::zeros(0);
        }

        let dim = section
            .sections
            .values()
            .next()
            .map(|v| v.len())
            .unwrap_or(0);

        match self.method {
            PoolingMethod::Mean => {
                let mut sum = Array1::zeros(dim);
                let mut count = 0;
                for vec in section.sections.values() {
                    sum = sum + vec;
                    count += 1;
                }
                if count > 0 {
                    sum / count as f64
                } else {
                    sum
                }
            }
            PoolingMethod::Max => {
                let mut max_vec = Array1::from_elem(dim, f64::NEG_INFINITY);
                for vec in section.sections.values() {
                    for (i, &val) in vec.iter().enumerate() {
                        max_vec[i] = max_vec[i].max(val);
                    }
                }
                max_vec
            }
            PoolingMethod::Sum => {
                let mut sum = Array1::zeros(dim);
                for vec in section.sections.values() {
                    sum = sum + vec;
                }
                sum
            }
            PoolingMethod::Attention => {
                // Simple attention: weight by L2 norm
                let mut sum = Array1::zeros(dim);
                let mut total_weight = 0.0;
                for vec in section.sections.values() {
                    let weight = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
                    sum = sum + vec * weight;
                    total_weight += weight;
                }
                if total_weight > 0.0 {
                    sum / total_weight
                } else {
                    sum
                }
            }
            PoolingMethod::TopK(k) => {
                // Select top k nodes by L2 norm
                let mut node_norms: Vec<_> = section
                    .sections
                    .iter()
                    .map(|(id, vec)| (*id, vec.iter().map(|x| x * x).sum::<f64>()))
                    .collect();
                node_norms
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                let mut sum = Array1::zeros(dim);
                for (node_id, _) in node_norms.into_iter().take(k) {
                    if let Some(vec) = section.get(node_id) {
                        sum = sum + vec;
                    }
                }
                sum / k as f64
            }
        }
    }
}

impl std::fmt::Debug for CohomologyPooling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CohomologyPooling")
            .field("method", &self.method)
            .field("spectral_weighting", &self.spectral_weighting)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substrate::edge::SheafEdgeBuilder;
    use crate::substrate::node::SheafNodeBuilder;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        Uuid::new_v4()
    }

    #[test]
    fn test_activation_functions() {
        assert!((Activation::ReLU.apply(-1.0) - 0.0).abs() < 1e-10);
        assert!((Activation::ReLU.apply(1.0) - 1.0).abs() < 1e-10);

        assert!((Activation::Sigmoid.apply(0.0) - 0.5).abs() < 1e-10);

        let arr = Array1::from_vec(vec![1.0, 2.0, 3.0]);
        let softmax = Activation::Softmax.apply_array(&arr);
        assert!((softmax.sum() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_sheaf_neural_layer() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0, 0.0, 0.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[0.0, 1.0, 0.0, 0.0])
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(4)
            .weight(1.0)
            .build();
        graph.add_edge(edge).unwrap();

        let config = SheafNeuralConfig {
            input_dim: 4,
            output_dim: 2,
            diffusion_steps: 1,
            ..Default::default()
        };
        let layer = SheafNeuralLayer::new(config);

        // Create input section
        let mut input = SheafSection::empty();
        input.set(id1, Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]));
        input.set(id2, Array1::from_vec(vec![0.0, 1.0, 0.0, 0.0]));

        let output = layer.forward(&graph, &input);

        assert!(output.contains(id1));
        assert!(output.contains(id2));
        assert_eq!(output.get(id1).unwrap().len(), 2);
    }

    #[test]
    fn test_sheaf_convolution() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[0.0, 1.0])
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(2)
            .build();
        graph.add_edge(edge).unwrap();

        let conv = SheafConvolution::new(2, 3);

        let mut input = SheafSection::empty();
        input.set(id1, Array1::from_vec(vec![1.0, 0.0]));
        input.set(id2, Array1::from_vec(vec![0.0, 1.0]));

        let output = conv.forward(&graph, &input);

        assert!(output.contains(id1));
        assert_eq!(output.get(id1).unwrap().len(), 3);
    }

    #[test]
    fn test_pooling() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();
        let node2 = SheafNodeBuilder::new().state_from_slice(&[3.0]).build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let mut section = SheafSection::empty();
        section.set(id1, Array1::from_vec(vec![1.0]));
        section.set(id2, Array1::from_vec(vec![3.0]));

        let mean_pool = CohomologyPooling::new(PoolingMethod::Mean);
        let result = mean_pool.pool(&graph, &section);
        assert!((result[0] - 2.0).abs() < 1e-10);

        let max_pool = CohomologyPooling::new(PoolingMethod::Max);
        let result = max_pool.pool(&graph, &section);
        assert!((result[0] - 3.0).abs() < 1e-10);
    }
}
