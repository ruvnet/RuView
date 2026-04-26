//! Normalized cut (Shi-Malik) for balanced graph partitioning.
//!
//! The normalized cut objective is:
//!
//! ```text
//! Ncut(A, B) = cut(A,B) / vol(A) + cut(A,B) / vol(B)
//! ```
//!
//! where vol(S) = sum of degrees of nodes in S.
//!
//! This is solved approximately via the spectral relaxation: find the Fiedler
//! vector of the normalized Laplacian and threshold it.

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::MincutResult;
use ruv_neural_core::{Result, RuvNeuralError};

use crate::spectral_cut::fiedler_decomposition;

/// Compute the normalized minimum cut of a brain graph.
///
/// Uses the spectral method: compute the Fiedler vector of the graph Laplacian,
/// then partition nodes by the sign of each component. The returned cut value
/// is the normalized cut metric: `cut(A,B)/vol(A) + cut(A,B)/vol(B)`.
///
/// # Errors
///
/// Returns an error if the graph has fewer than 2 nodes.
pub fn normalized_cut(graph: &BrainGraph) -> Result<MincutResult> {
    let n = graph.num_nodes;
    if n < 2 {
        return Err(RuvNeuralError::Mincut(
            "Normalized cut requires at least 2 nodes".into(),
        ));
    }

    // Get the Fiedler vector from the unnormalized Laplacian.
    // For normalized cut, ideally we would use the generalized eigenproblem
    // L*x = lambda*D*x. We approximate by using the Fiedler vector of L and
    // then trying multiple threshold sweeps to minimize Ncut.
    let (_fiedler_value, fiedler_vec) = fiedler_decomposition(graph)?;

    // Sweep thresholds along the sorted Fiedler values to find the best Ncut.
    let adj = graph.adjacency_matrix();
    let degrees: Vec<f64> = (0..n)
        .map(|i| adj[i].iter().sum::<f64>())
        .collect();

    // Sort node indices by Fiedler value.
    let mut sorted_indices: Vec<usize> = (0..n).collect();
    sorted_indices.sort_by(|&a, &b| {
        fiedler_vec[a]
            .partial_cmp(&fiedler_vec[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut best_ncut = f64::INFINITY;
    let mut best_split = 1usize; // number of nodes in partition A

    // Track incremental cut and volumes.
    // Start with partition A = empty, B = all. Then move nodes from B to A.
    let total_vol: f64 = degrees.iter().sum();

    let mut vol_a = 0.0;
    let mut in_a = vec![false; n];

    // We also need the cross-cut, which we compute incrementally.
    // cut(A, B) = sum of weights between A and B.
    let mut cut_val = 0.0;

    for split in 0..(n - 1) {
        let node = sorted_indices[split];
        in_a[node] = true;
        vol_a += degrees[node];

        // Update cut: adding `node` to A means:
        // - edges from `node` to other A nodes decrease cut (they were in cut before)
        // - edges from `node` to B nodes increase cut
        for j in 0..n {
            if adj[node][j] > 0.0 {
                if in_a[j] && j != node {
                    // j was already in A, so edge (node, j) was previously a cut edge
                    // (from B to A). Now both are in A, so remove it from cut.
                    cut_val -= adj[node][j];
                } else if !in_a[j] {
                    // j is in B, so adding node to A creates a new cut edge.
                    cut_val += adj[node][j];
                }
            }
        }

        let vol_b = total_vol - vol_a;
        if vol_a > 0.0 && vol_b > 0.0 {
            let ncut = cut_val / vol_a + cut_val / vol_b;
            if ncut < best_ncut {
                best_ncut = ncut;
                best_split = split + 1;
            }
        }
    }

    // Build final partitions.
    let partition_a: Vec<usize> = sorted_indices[..best_split].to_vec();
    let partition_b: Vec<usize> = sorted_indices[best_split..].to_vec();

    let partition_a_set: std::collections::HashSet<usize> =
        partition_a.iter().copied().collect();

    // Compute the actual cut edges and value.
    let mut actual_cut = 0.0;
    let mut cut_edges = Vec::new();
    for edge in &graph.edges {
        let s_in_a = partition_a_set.contains(&edge.source);
        let t_in_a = partition_a_set.contains(&edge.target);
        if s_in_a != t_in_a {
            actual_cut += edge.weight;
            cut_edges.push((edge.source, edge.target, edge.weight));
        }
    }

    // Compute normalized cut value.
    let vol_a: f64 = partition_a.iter().map(|&i| degrees[i]).sum();
    let vol_b: f64 = partition_b.iter().map(|&i| degrees[i]).sum();
    let ncut_value = if vol_a > 0.0 && vol_b > 0.0 {
        actual_cut / vol_a + actual_cut / vol_b
    } else {
        actual_cut
    };

    Ok(MincutResult {
        cut_value: ncut_value,
        partition_a,
        partition_b,
        cut_edges,
        timestamp: graph.timestamp,
    })
}

/// Compute the volume of a node set: sum of weighted degrees.
pub fn volume(graph: &BrainGraph, nodes: &[usize]) -> f64 {
    nodes.iter().map(|&i| graph.node_degree(i)).sum()
}

/// Compute the raw cut weight between two node sets.
pub fn cut_weight(graph: &BrainGraph, set_a: &[usize], set_b: &[usize]) -> f64 {
    let a_set: std::collections::HashSet<usize> = set_a.iter().copied().collect();
    let b_set: std::collections::HashSet<usize> = set_b.iter().copied().collect();

    graph
        .edges
        .iter()
        .filter(|e| {
            (a_set.contains(&e.source) && b_set.contains(&e.target))
                || (b_set.contains(&e.source) && a_set.contains(&e.target))
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

    /// Normalized cut on a barbell graph should separate the two cliques.
    #[test]
    fn test_normalized_cut_barbell() {
        let graph = BrainGraph {
            num_nodes: 6,
            edges: vec![
                // Clique 1: {0, 1, 2}
                make_edge(0, 1, 5.0),
                make_edge(1, 2, 5.0),
                make_edge(0, 2, 5.0),
                // Clique 2: {3, 4, 5}
                make_edge(3, 4, 5.0),
                make_edge(4, 5, 5.0),
                make_edge(3, 5, 5.0),
                // Weak bridge
                make_edge(2, 3, 0.1),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(6),
        };

        let result = normalized_cut(&graph).unwrap();
        // The partition should separate the two cliques.
        assert_eq!(result.partition_a.len() + result.partition_b.len(), 6);
        // Ncut value should be small since the bridge is weak.
        assert!(
            result.cut_value < 1.0,
            "Expected small Ncut for barbell, got {}",
            result.cut_value
        );
    }

    /// Balanced normalized cut produces non-degenerate partitions.
    #[test]
    fn test_normalized_cut_balanced() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 3.0),
                make_edge(2, 3, 3.0),
                make_edge(1, 2, 0.5),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let result = normalized_cut(&graph).unwrap();
        // Both partitions should be non-empty.
        assert!(!result.partition_a.is_empty());
        assert!(!result.partition_b.is_empty());
    }

    #[test]
    fn test_volume_computation() {
        let graph = BrainGraph {
            num_nodes: 3,
            edges: vec![
                make_edge(0, 1, 2.0),
                make_edge(1, 2, 3.0),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        };

        let vol = volume(&graph, &[0, 1]);
        // node 0 degree = 2, node 1 degree = 2 + 3 = 5
        assert!((vol - 7.0).abs() < 1e-9);
    }

    #[test]
    fn test_cut_weight_computation() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 2.0),
                make_edge(1, 2, 3.0),
                make_edge(2, 3, 4.0),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let cw = cut_weight(&graph, &[0, 1], &[2, 3]);
        // Only edge 1-2 (weight 3) crosses the cut.
        assert!((cw - 3.0).abs() < 1e-9);
    }
}
