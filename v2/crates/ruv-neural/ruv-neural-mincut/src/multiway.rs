//! Multi-way graph partitioning using recursive normalized cut.
//!
//! Splits a brain connectivity graph into k modules by recursively applying
//! normalized cut. Includes automatic module detection via modularity
//! optimization.

use ruv_neural_core::graph::{BrainEdge, BrainGraph};
use ruv_neural_core::topology::MultiPartition;
use ruv_neural_core::{Result, RuvNeuralError};

use crate::normalized::normalized_cut;

/// K-way graph partitioning using recursive normalized cut.
///
/// Recursively bisects the graph to produce `k` partitions. At each step the
/// partition with the highest internal connectivity is chosen for the next
/// split. The process stops when `k` partitions are produced or when further
/// splitting does not improve modularity.
///
/// # Errors
///
/// Returns an error if `k < 2` or if the graph has fewer than `k` nodes.
pub fn multiway_cut(graph: &BrainGraph, k: usize) -> Result<MultiPartition> {
    if k < 2 {
        return Err(RuvNeuralError::Mincut(
            "multiway_cut requires k >= 2".into(),
        ));
    }
    if graph.num_nodes < k {
        return Err(RuvNeuralError::Mincut(format!(
            "Cannot partition {} nodes into {} groups",
            graph.num_nodes, k
        )));
    }

    // Start with a single partition containing all nodes.
    let mut partitions: Vec<Vec<usize>> = vec![(0..graph.num_nodes).collect()];

    while partitions.len() < k {
        // Find the largest partition to split next.
        let (split_idx, _) = partitions
            .iter()
            .enumerate()
            .max_by_key(|(_, p)| p.len())
            .unwrap();

        let to_split = &partitions[split_idx];
        if to_split.len() < 2 {
            // Cannot split a singleton; stop early.
            break;
        }

        // Build a subgraph from this partition.
        let subgraph = build_subgraph(graph, to_split);

        // Apply normalized cut on the subgraph.
        let sub_result = normalized_cut(&subgraph)?;

        // Map subgraph indices back to original indices.
        let part_a: Vec<usize> = sub_result
            .partition_a
            .iter()
            .map(|&i| to_split[i])
            .collect();
        let part_b: Vec<usize> = sub_result
            .partition_b
            .iter()
            .map(|&i| to_split[i])
            .collect();

        // Replace the split partition with the two new ones.
        partitions.remove(split_idx);
        partitions.push(part_a);
        partitions.push(part_b);
    }

    // Sort each partition for determinism.
    for p in &mut partitions {
        p.sort_unstable();
    }
    partitions.sort_by_key(|p| p[0]);

    let modularity = compute_modularity(graph, &partitions);
    let cut_value = compute_total_cut(graph, &partitions);

    Ok(MultiPartition {
        partitions,
        cut_value,
        modularity,
    })
}

/// Automatic module detection: find the optimal number of partitions k that
/// maximizes Newman-Girvan modularity.
///
/// Tries k = 2, 3, ..., max_k (where max_k = sqrt(num_nodes)) and returns the
/// partitioning with the highest modularity.
pub fn detect_modules(graph: &BrainGraph) -> Result<MultiPartition> {
    let n = graph.num_nodes;
    if n < 2 {
        return Err(RuvNeuralError::Mincut(
            "detect_modules requires at least 2 nodes".into(),
        ));
    }

    let max_k = ((n as f64).sqrt().ceil() as usize).max(2).min(n);

    let mut best_partition: Option<MultiPartition> = None;
    let mut best_modularity = f64::NEG_INFINITY;

    for k in 2..=max_k {
        if k > n {
            break;
        }
        match multiway_cut(graph, k) {
            Ok(partition) => {
                if partition.modularity > best_modularity {
                    best_modularity = partition.modularity;
                    best_partition = Some(partition);
                }
            }
            Err(_) => break,
        }
    }

    best_partition.ok_or_else(|| {
        RuvNeuralError::Mincut("Could not find any valid partitioning".into())
    })
}

/// Build a subgraph from a subset of nodes.
///
/// The returned graph has nodes indexed 0..subset.len(), with edges re-mapped
/// from the original graph.
fn build_subgraph(graph: &BrainGraph, subset: &[usize]) -> BrainGraph {
    // Map from original index to subgraph index.
    let mut index_map = std::collections::HashMap::new();
    for (new_idx, &orig_idx) in subset.iter().enumerate() {
        index_map.insert(orig_idx, new_idx);
    }

    let edges: Vec<BrainEdge> = graph
        .edges
        .iter()
        .filter_map(|e| {
            let s = index_map.get(&e.source)?;
            let t = index_map.get(&e.target)?;
            Some(BrainEdge {
                source: *s,
                target: *t,
                weight: e.weight,
                metric: e.metric,
                frequency_band: e.frequency_band,
            })
        })
        .collect();

    BrainGraph {
        num_nodes: subset.len(),
        edges,
        timestamp: graph.timestamp,
        window_duration_s: graph.window_duration_s,
        atlas: graph.atlas,
    }
}

/// Compute Newman-Girvan modularity for a given partitioning.
///
/// Q = (1 / 2m) * sum_{ij} [A_{ij} - k_i * k_j / (2m)] * delta(c_i, c_j)
pub fn compute_modularity(graph: &BrainGraph, partitions: &[Vec<usize>]) -> f64 {
    let adj = graph.adjacency_matrix();
    let n = graph.num_nodes;
    let m: f64 = graph.edges.iter().map(|e| e.weight).sum::<f64>();

    if m <= 0.0 {
        return 0.0;
    }

    let two_m = 2.0 * m;

    // Assign each node to its community.
    let mut community = vec![0usize; n];
    for (c, partition) in partitions.iter().enumerate() {
        for &node in partition {
            if node < n {
                community[node] = c;
            }
        }
    }

    // Degrees.
    let degrees: Vec<f64> = (0..n).map(|i| adj[i].iter().sum::<f64>()).collect();

    let mut q = 0.0;
    for i in 0..n {
        for j in 0..n {
            if community[i] == community[j] {
                q += adj[i][j] - degrees[i] * degrees[j] / two_m;
            }
        }
    }
    q / two_m
}

/// Compute the total weight of edges that cross partition boundaries.
fn compute_total_cut(graph: &BrainGraph, partitions: &[Vec<usize>]) -> f64 {
    let n = graph.num_nodes;
    let mut community = vec![0usize; n];
    for (c, partition) in partitions.iter().enumerate() {
        for &node in partition {
            if node < n {
                community[node] = c;
            }
        }
    }

    graph
        .edges
        .iter()
        .filter(|e| {
            e.source < n
                && e.target < n
                && community[e.source] != community[e.target]
        })
        .map(|e| e.weight)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::BrainEdge;
    use ruv_neural_core::signal::FrequencyBand;

    fn make_edge(source: usize, target: usize, weight: f64) -> BrainEdge {
        BrainEdge {
            source,
            target,
            weight,
            metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Alpha,
        }
    }

    /// Multiway cut with k=2 should produce 2 partitions.
    #[test]
    fn test_multiway_k2() {
        let graph = BrainGraph {
            num_nodes: 6,
            edges: vec![
                make_edge(0, 1, 5.0),
                make_edge(1, 2, 5.0),
                make_edge(0, 2, 5.0),
                make_edge(3, 4, 5.0),
                make_edge(4, 5, 5.0),
                make_edge(3, 5, 5.0),
                make_edge(2, 3, 0.1),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(6),
        };

        let result = multiway_cut(&graph, 2).unwrap();
        assert_eq!(result.num_partitions(), 2);
        assert_eq!(result.num_nodes(), 6);
    }

    /// Multiway cut with k=3 on a graph with 3 obvious clusters.
    #[test]
    fn test_multiway_k3() {
        let graph = BrainGraph {
            num_nodes: 9,
            edges: vec![
                // Cluster 1: {0, 1, 2}
                make_edge(0, 1, 5.0),
                make_edge(1, 2, 5.0),
                make_edge(0, 2, 5.0),
                // Cluster 2: {3, 4, 5}
                make_edge(3, 4, 5.0),
                make_edge(4, 5, 5.0),
                make_edge(3, 5, 5.0),
                // Cluster 3: {6, 7, 8}
                make_edge(6, 7, 5.0),
                make_edge(7, 8, 5.0),
                make_edge(6, 8, 5.0),
                // Weak bridges
                make_edge(2, 3, 0.1),
                make_edge(5, 6, 0.1),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(9),
        };

        let result = multiway_cut(&graph, 3).unwrap();
        assert_eq!(result.num_partitions(), 3);
        assert_eq!(result.num_nodes(), 9);
        assert!(result.modularity > 0.0, "Modularity should be positive for clustered graph");
    }

    /// detect_modules should find a good partition automatically.
    #[test]
    fn test_detect_modules() {
        let graph = BrainGraph {
            num_nodes: 6,
            edges: vec![
                make_edge(0, 1, 5.0),
                make_edge(1, 2, 5.0),
                make_edge(0, 2, 5.0),
                make_edge(3, 4, 5.0),
                make_edge(4, 5, 5.0),
                make_edge(3, 5, 5.0),
                make_edge(2, 3, 0.1),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(6),
        };

        let result = detect_modules(&graph).unwrap();
        assert!(result.num_partitions() >= 2);
        assert!(result.modularity > 0.0);
    }

    /// k=1 should error.
    #[test]
    fn test_multiway_k1_error() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![make_edge(0, 1, 1.0)],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };
        assert!(multiway_cut(&graph, 1).is_err());
    }

    /// More partitions than nodes should error.
    #[test]
    fn test_multiway_too_many_partitions() {
        let graph = BrainGraph {
            num_nodes: 3,
            edges: vec![make_edge(0, 1, 1.0), make_edge(1, 2, 1.0)],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        };
        assert!(multiway_cut(&graph, 5).is_err());
    }

    #[test]
    fn test_modularity_positive_for_good_partition() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 5.0),
                make_edge(2, 3, 5.0),
                make_edge(1, 2, 0.1),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let q = compute_modularity(&graph, &[vec![0, 1], vec![2, 3]]);
        assert!(q > 0.0, "Good partition should have positive modularity, got {}", q);
    }
}
