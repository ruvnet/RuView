//! Graph Convolutional Network (GCN) layer implementation
//!
//! Based on "Semi-Supervised Classification with Graph Convolutional Networks"
//! by Kipf & Welling (2016)

use super::aggregators::sum_aggregate;
use super::message_passing::MessagePassing;
use rayon::prelude::*;

/// Graph Convolutional Network layer
#[derive(Debug, Clone)]
pub struct GCNLayer {
    /// Input feature dimension
    pub in_features: usize,
    /// Output feature dimension
    pub out_features: usize,
    /// Weight matrix [in_features x out_features]
    pub weights: Vec<Vec<f32>>,
    /// Bias term
    pub bias: Option<Vec<f32>>,
    /// Whether to normalize by degree
    pub normalize: bool,
}

impl GCNLayer {
    /// Create a new GCN layer with random weights
    pub fn new(in_features: usize, out_features: usize) -> Self {
        Self::new_with_normalize(in_features, out_features, true)
    }

    /// Create a new GCN layer with normalization option
    pub fn new_with_normalize(in_features: usize, out_features: usize, normalize: bool) -> Self {
        // Initialize weights with Xavier/Glorot initialization
        let scale = (2.0 / (in_features + out_features) as f32).sqrt();
        let weights = (0..in_features)
            .map(|i| {
                (0..out_features)
                    .map(|j| {
                        // Simple deterministic initialization for testing
                        let val = ((i * out_features + j) as f32 * 0.01) % 1.0;
                        (val - 0.5) * scale
                    })
                    .collect()
            })
            .collect();

        Self {
            in_features,
            out_features,
            weights,
            bias: Some(vec![0.0; out_features]),
            normalize,
        }
    }

    /// Create GCN layer with provided weights
    pub fn with_weights(in_features: usize, out_features: usize, weights: Vec<Vec<f32>>) -> Self {
        assert_eq!(weights.len(), in_features);
        assert_eq!(weights[0].len(), out_features);

        Self {
            in_features,
            out_features,
            weights,
            bias: Some(vec![0.0; out_features]),
            normalize: true,
        }
    }

    /// Apply linear transformation: features @ weights
    pub fn linear_transform(&self, features: &[f32]) -> Vec<f32> {
        assert_eq!(features.len(), self.in_features);

        let mut result = vec![0.0; self.out_features];

        // Matrix multiplication: features @ weights
        for (i, &feature_val) in features.iter().enumerate() {
            for (j, &weight_val) in self.weights[i].iter().enumerate() {
                result[j] += feature_val * weight_val;
            }
        }

        // Add bias if present
        if let Some(ref bias) = self.bias {
            for (i, &b) in bias.iter().enumerate() {
                result[i] += b;
            }
        }

        result
    }

    /// Forward pass with edge index and optional edge weights
    pub fn forward(
        &self,
        node_features: &[Vec<f32>],
        edge_index: &[(usize, usize)],
        edge_weights: Option<&[f32]>,
    ) -> Vec<Vec<f32>> {
        use super::message_passing::{propagate, propagate_weighted};

        // Apply message passing
        let result = if let Some(weights) = edge_weights {
            propagate_weighted(node_features, edge_index, weights, self)
        } else {
            propagate(node_features, edge_index, self)
        };

        // Apply ReLU activation
        result
            .into_par_iter()
            .map(|features| features.iter().map(|&x| x.max(0.0)).collect())
            .collect()
    }

    /// Compute degree normalization factor for a node
    fn compute_norm_factor(&self, degree: usize) -> f32 {
        if self.normalize && degree > 0 {
            1.0 / (degree as f32).sqrt()
        } else {
            1.0
        }
    }
}

impl MessagePassing for GCNLayer {
    fn message(&self, source_features: &[f32], edge_weight: Option<f32>) -> Vec<f32> {
        let weight = edge_weight.unwrap_or(1.0);
        source_features.iter().map(|&x| x * weight).collect()
    }

    fn aggregate(&self, messages: Vec<Vec<f32>>) -> Vec<f32> {
        let degree = messages.len();
        let mut aggregated = sum_aggregate(messages);

        // Apply degree normalization
        if self.normalize && degree > 0 {
            let norm = self.compute_norm_factor(degree);
            aggregated.iter_mut().for_each(|x| *x *= norm);
        }

        aggregated
    }

    fn update(&self, _node_features: &[f32], aggregated: &[f32]) -> Vec<f32> {
        // Apply linear transformation to aggregated features
        self.linear_transform(aggregated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcn_layer_creation() {
        let layer = GCNLayer::new(16, 32);
        assert_eq!(layer.in_features, 16);
        assert_eq!(layer.out_features, 32);
        assert_eq!(layer.weights.len(), 16);
        assert_eq!(layer.weights[0].len(), 32);
    }

    #[test]
    fn test_linear_transform() {
        let weights = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let layer = GCNLayer::with_weights(2, 2, weights);

        let features = vec![1.0, 2.0];
        let result = layer.linear_transform(&features);

        // [1, 2] @ [[1, 2], [3, 4]] = [1*1 + 2*3, 1*2 + 2*4] = [7, 10]
        assert_eq!(result, vec![7.0, 10.0]);
    }

    #[test]
    fn test_gcn_forward() {
        let weights = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let layer = GCNLayer::with_weights(2, 2, weights);

        let node_features = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];

        let edge_index = vec![(0, 1), (1, 2), (2, 0)];

        let result = layer.forward(&node_features, &edge_index, None);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].len(), 2);
    }

    #[test]
    fn test_message_passing() {
        let layer = GCNLayer::new(2, 2);

        let features = vec![1.0, 2.0];
        let message = layer.message(&features, Some(2.0));

        assert_eq!(message, vec![2.0, 4.0]);
    }

    #[test]
    fn test_aggregation() {
        let layer = GCNLayer::new_with_normalize(2, 2, false);

        let messages = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let result = layer.aggregate(messages);

        assert_eq!(result, vec![4.0, 6.0]);
    }

    #[test]
    fn test_normalization() {
        let layer = GCNLayer::new_with_normalize(2, 2, true);

        let messages = vec![vec![4.0, 6.0], vec![0.0, 0.0]];
        let result = layer.aggregate(messages);

        // Degree = 2, norm = 1/sqrt(2) ≈ 0.707
        let expected_norm = 1.0 / (2.0_f32).sqrt();
        assert!((result[0] - 4.0 * expected_norm).abs() < 1e-5);
        assert!((result[1] - 6.0 * expected_norm).abs() < 1e-5);
    }
}
