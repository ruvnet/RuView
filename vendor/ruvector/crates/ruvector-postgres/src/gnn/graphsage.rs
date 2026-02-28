//! GraphSAGE layer implementation with neighbor sampling
//!
//! Based on "Inductive Representation Learning on Large Graphs"
//! by Hamilton et al. (2017)

use super::aggregators::mean_aggregate;
use super::message_passing::MessagePassing;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rayon::prelude::*;

/// GraphSAGE aggregation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SAGEAggregator {
    /// Mean aggregator
    Mean,
    /// Max pooling aggregator
    MaxPool,
    /// LSTM aggregator
    LSTM,
}

/// GraphSAGE layer with neighbor sampling
#[derive(Debug, Clone)]
pub struct GraphSAGELayer {
    /// Input feature dimension
    pub in_features: usize,
    /// Output feature dimension
    pub out_features: usize,
    /// Weight matrix for neighbor features
    pub neighbor_weights: Vec<Vec<f32>>,
    /// Weight matrix for self features
    pub self_weights: Vec<Vec<f32>>,
    /// Aggregator type
    pub aggregator: SAGEAggregator,
    /// Number of neighbors to sample
    pub num_samples: usize,
    /// Whether to normalize output
    pub normalize: bool,
}

impl GraphSAGELayer {
    /// Create a new GraphSAGE layer
    pub fn new(in_features: usize, out_features: usize, num_samples: usize) -> Self {
        Self::with_aggregator(in_features, out_features, num_samples, SAGEAggregator::Mean)
    }

    /// Create GraphSAGE layer with specific aggregator
    pub fn with_aggregator(
        in_features: usize,
        out_features: usize,
        num_samples: usize,
        aggregator: SAGEAggregator,
    ) -> Self {
        // Initialize weights
        let scale = (2.0 / (in_features + out_features) as f32).sqrt();

        let neighbor_weights = (0..in_features)
            .map(|i| {
                (0..out_features)
                    .map(|j| {
                        let val = ((i * out_features + j) as f32 * 0.01) % 1.0;
                        (val - 0.5) * scale
                    })
                    .collect()
            })
            .collect();

        let self_weights = (0..in_features)
            .map(|i| {
                (0..out_features)
                    .map(|j| {
                        let val = ((i * out_features + j + 1000) as f32 * 0.01) % 1.0;
                        (val - 0.5) * scale
                    })
                    .collect()
            })
            .collect();

        Self {
            in_features,
            out_features,
            neighbor_weights,
            self_weights,
            aggregator,
            num_samples,
            normalize: true,
        }
    }

    /// Sample k neighbors uniformly at random
    pub fn sample_neighbors(&self, neighbors: &[usize], k: usize) -> Vec<usize> {
        if neighbors.len() <= k {
            return neighbors.to_vec();
        }

        // Use deterministic sampling for reproducibility in tests
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut sampled = neighbors.to_vec();
        sampled.partial_shuffle(&mut rng, k);
        sampled[..k].to_vec()
    }

    /// Apply linear transformation
    fn linear_transform(&self, features: &[f32], weights: &[Vec<f32>]) -> Vec<f32> {
        let mut result = vec![0.0; self.out_features];

        for (i, &feature_val) in features.iter().enumerate() {
            for (j, &weight_val) in weights[i].iter().enumerate() {
                result[j] += feature_val * weight_val;
            }
        }

        result
    }

    /// Forward pass with neighbor sampling
    pub fn forward_with_sampling(
        &self,
        node_features: &[Vec<f32>],
        edge_index: &[(usize, usize)],
        num_samples: Option<usize>,
    ) -> Vec<Vec<f32>> {
        use super::message_passing::build_adjacency_list;

        let num_nodes = node_features.len();
        let k = num_samples.unwrap_or(self.num_samples);
        let adj_list = build_adjacency_list(edge_index, num_nodes);

        (0..num_nodes)
            .into_par_iter()
            .map(|node_id| {
                let neighbors = adj_list.get(&node_id).unwrap();

                // Sample neighbors
                let sampled = self.sample_neighbors(neighbors, k);

                // Collect neighbor features
                let neighbor_features: Vec<Vec<f32>> = sampled
                    .iter()
                    .filter_map(|&neighbor_id| {
                        if neighbor_id < num_nodes {
                            Some(node_features[neighbor_id].clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                // Aggregate neighbor features
                let aggregated = if neighbor_features.is_empty() {
                    vec![0.0; self.in_features]
                } else {
                    match self.aggregator {
                        SAGEAggregator::Mean => mean_aggregate(neighbor_features),
                        SAGEAggregator::MaxPool => {
                            super::aggregators::max_aggregate(neighbor_features)
                        }
                        SAGEAggregator::LSTM => mean_aggregate(neighbor_features), // Simplified
                    }
                };

                // Transform neighbor aggregation
                let neighbor_h = self.linear_transform(&aggregated, &self.neighbor_weights);

                // Transform self features
                let self_h = self.linear_transform(&node_features[node_id], &self.self_weights);

                // Concatenate and apply activation
                let mut combined: Vec<f32> = neighbor_h
                    .iter()
                    .zip(self_h.iter())
                    .map(|(&n, &s)| (n + s).max(0.0))
                    .collect();

                // L2 normalization if enabled
                if self.normalize {
                    let norm: f32 = combined.iter().map(|&x| x * x).sum::<f32>().sqrt();
                    if norm > 0.0 {
                        combined.iter_mut().for_each(|x| *x /= norm);
                    }
                }

                combined
            })
            .collect()
    }

    /// Standard forward pass (uses default num_samples)
    pub fn forward(
        &self,
        node_features: &[Vec<f32>],
        edge_index: &[(usize, usize)],
    ) -> Vec<Vec<f32>> {
        self.forward_with_sampling(node_features, edge_index, None)
    }
}

impl MessagePassing for GraphSAGELayer {
    fn message(&self, source_features: &[f32], _edge_weight: Option<f32>) -> Vec<f32> {
        source_features.to_vec()
    }

    fn aggregate(&self, messages: Vec<Vec<f32>>) -> Vec<f32> {
        match self.aggregator {
            SAGEAggregator::Mean => mean_aggregate(messages),
            SAGEAggregator::MaxPool => super::aggregators::max_aggregate(messages),
            SAGEAggregator::LSTM => mean_aggregate(messages),
        }
    }

    fn update(&self, node_features: &[f32], aggregated: &[f32]) -> Vec<f32> {
        let neighbor_h = self.linear_transform(aggregated, &self.neighbor_weights);
        let self_h = self.linear_transform(node_features, &self.self_weights);

        neighbor_h
            .iter()
            .zip(self_h.iter())
            .map(|(&n, &s)| (n + s).max(0.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphsage_creation() {
        let layer = GraphSAGELayer::new(16, 32, 10);
        assert_eq!(layer.in_features, 16);
        assert_eq!(layer.out_features, 32);
        assert_eq!(layer.num_samples, 10);
    }

    #[test]
    fn test_sample_neighbors() {
        let layer = GraphSAGELayer::new(4, 8, 3);

        let neighbors = vec![0, 1, 2, 3, 4, 5];
        let sampled = layer.sample_neighbors(&neighbors, 3);

        assert_eq!(sampled.len(), 3);

        // Test with fewer neighbors than k
        let few_neighbors = vec![0, 1];
        let sampled_few = layer.sample_neighbors(&few_neighbors, 5);
        assert_eq!(sampled_few.len(), 2);
    }

    #[test]
    fn test_graphsage_forward() {
        let layer = GraphSAGELayer::new(2, 2, 2);

        let node_features = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];

        let edge_index = vec![(0, 1), (1, 2), (2, 0)];

        let result = layer.forward(&node_features, &edge_index);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].len(), 2);
    }

    #[test]
    fn test_different_aggregators() {
        let mean_layer = GraphSAGELayer::with_aggregator(2, 2, 2, SAGEAggregator::Mean);
        let max_layer = GraphSAGELayer::with_aggregator(2, 2, 2, SAGEAggregator::MaxPool);

        let node_features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let edge_index = vec![(0, 1)];

        let mean_result = mean_layer.forward(&node_features, &edge_index);
        let max_result = max_layer.forward(&node_features, &edge_index);

        assert_eq!(mean_result.len(), 2);
        assert_eq!(max_result.len(), 2);
    }

    #[test]
    fn test_normalization() {
        let layer = GraphSAGELayer::new(2, 2, 2);

        let node_features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let edge_index = vec![(0, 1)];

        let result = layer.forward(&node_features, &edge_index);

        // Check L2 normalization
        for features in result {
            let norm: f32 = features.iter().map(|&x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5 || norm == 0.0);
        }
    }
}
