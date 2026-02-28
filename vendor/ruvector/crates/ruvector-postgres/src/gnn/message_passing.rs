//! Core message passing framework for Graph Neural Networks
//!
//! This module implements the fundamental message passing paradigm used in GNNs:
//! 1. Message: Compute messages from neighbors
//! 2. Aggregate: Combine messages from all neighbors
//! 3. Update: Update node representations

use rayon::prelude::*;
use std::collections::HashMap;

/// Adjacency list representation of a graph
pub type AdjacencyList = HashMap<usize, Vec<usize>>;

/// Message passing trait for GNN layers
pub trait MessagePassing {
    /// Compute message from source node to target node
    fn message(&self, source_features: &[f32], edge_weight: Option<f32>) -> Vec<f32>;

    /// Aggregate messages from all neighbors
    fn aggregate(&self, messages: Vec<Vec<f32>>) -> Vec<f32>;

    /// Update node features based on aggregated messages
    fn update(&self, node_features: &[f32], aggregated: &[f32]) -> Vec<f32>;
}

/// Build adjacency list from edge index
///
/// # Arguments
/// * `edge_index` - Array of (source, target) edges
/// * `num_nodes` - Total number of nodes in the graph
///
/// # Returns
/// HashMap mapping each node to its list of neighbors
pub fn build_adjacency_list(edge_index: &[(usize, usize)], num_nodes: usize) -> AdjacencyList {
    let mut adj_list: AdjacencyList = HashMap::with_capacity(num_nodes);

    // Initialize all nodes
    for i in 0..num_nodes {
        adj_list.insert(i, Vec::new());
    }

    // Build adjacency list
    for &(src, dst) in edge_index {
        if src < num_nodes && dst < num_nodes {
            adj_list.get_mut(&dst).unwrap().push(src);
        }
    }

    adj_list
}

/// Propagate features through the graph using message passing
///
/// # Arguments
/// * `node_features` - Features for each node [num_nodes x feature_dim]
/// * `edge_index` - Array of (source, target) edges
/// * `layer` - GNN layer implementing MessagePassing trait
///
/// # Returns
/// Updated node features after message passing
pub fn propagate<L: MessagePassing + Sync>(
    node_features: &[Vec<f32>],
    edge_index: &[(usize, usize)],
    layer: &L,
) -> Vec<Vec<f32>> {
    let num_nodes = node_features.len();
    let adj_list = build_adjacency_list(edge_index, num_nodes);

    // Parallel processing of nodes
    (0..num_nodes)
        .into_par_iter()
        .map(|node_id| {
            let neighbors = adj_list.get(&node_id).unwrap();

            if neighbors.is_empty() {
                // Disconnected node - return original features
                return node_features[node_id].clone();
            }

            // Collect messages from neighbors
            let messages: Vec<Vec<f32>> = neighbors
                .iter()
                .filter_map(|&neighbor_id| {
                    if neighbor_id < num_nodes {
                        Some(layer.message(&node_features[neighbor_id], None))
                    } else {
                        None
                    }
                })
                .collect();

            if messages.is_empty() {
                return node_features[node_id].clone();
            }

            // Aggregate messages
            let aggregated = layer.aggregate(messages);

            // Update node features
            layer.update(&node_features[node_id], &aggregated)
        })
        .collect()
}

/// Propagate features with edge weights
pub fn propagate_weighted<L: MessagePassing + Sync>(
    node_features: &[Vec<f32>],
    edge_index: &[(usize, usize)],
    edge_weights: &[f32],
    layer: &L,
) -> Vec<Vec<f32>> {
    let num_nodes = node_features.len();

    // Build weighted adjacency list
    let mut adj_list: HashMap<usize, Vec<(usize, f32)>> = HashMap::with_capacity(num_nodes);
    for i in 0..num_nodes {
        adj_list.insert(i, Vec::new());
    }

    for (idx, &(src, dst)) in edge_index.iter().enumerate() {
        if src < num_nodes && dst < num_nodes {
            let weight = if idx < edge_weights.len() {
                edge_weights[idx]
            } else {
                1.0
            };
            adj_list.get_mut(&dst).unwrap().push((src, weight));
        }
    }

    // Parallel processing of nodes
    (0..num_nodes)
        .into_par_iter()
        .map(|node_id| {
            let neighbors = adj_list.get(&node_id).unwrap();

            if neighbors.is_empty() {
                return node_features[node_id].clone();
            }

            // Collect weighted messages from neighbors
            let messages: Vec<Vec<f32>> = neighbors
                .iter()
                .filter_map(|&(neighbor_id, weight)| {
                    if neighbor_id < num_nodes {
                        Some(layer.message(&node_features[neighbor_id], Some(weight)))
                    } else {
                        None
                    }
                })
                .collect();

            if messages.is_empty() {
                return node_features[node_id].clone();
            }

            // Aggregate and update
            let aggregated = layer.aggregate(messages);
            layer.update(&node_features[node_id], &aggregated)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleLayer;

    impl MessagePassing for SimpleLayer {
        fn message(&self, source_features: &[f32], edge_weight: Option<f32>) -> Vec<f32> {
            let weight = edge_weight.unwrap_or(1.0);
            source_features.iter().map(|&x| x * weight).collect()
        }

        fn aggregate(&self, messages: Vec<Vec<f32>>) -> Vec<f32> {
            if messages.is_empty() {
                return vec![];
            }
            let dim = messages[0].len();
            let mut result = vec![0.0; dim];
            for msg in messages {
                for (i, &val) in msg.iter().enumerate() {
                    result[i] += val;
                }
            }
            result
        }

        fn update(&self, node_features: &[f32], aggregated: &[f32]) -> Vec<f32> {
            node_features
                .iter()
                .zip(aggregated.iter())
                .map(|(&x, &y)| x + y)
                .collect()
        }
    }

    #[test]
    fn test_build_adjacency_list() {
        let edges = vec![(0, 1), (1, 2), (2, 0)];
        let adj_list = build_adjacency_list(&edges, 3);

        assert_eq!(adj_list.get(&0).unwrap(), &vec![2]);
        assert_eq!(adj_list.get(&1).unwrap(), &vec![0]);
        assert_eq!(adj_list.get(&2).unwrap(), &vec![1]);
    }

    #[test]
    fn test_propagate() {
        let node_features = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];

        let edge_index = vec![(0, 1), (1, 2)];

        let layer = SimpleLayer;
        let result = propagate(&node_features, &edge_index, &layer);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].len(), 2);
    }

    #[test]
    fn test_disconnected_nodes() {
        let node_features = vec![vec![1.0], vec![2.0], vec![3.0]];
        let edge_index = vec![(0, 1)]; // Node 2 is disconnected

        let layer = SimpleLayer;
        let result = propagate(&node_features, &edge_index, &layer);

        // Disconnected node should retain original features
        assert_eq!(result[2], vec![3.0]);
    }
}
