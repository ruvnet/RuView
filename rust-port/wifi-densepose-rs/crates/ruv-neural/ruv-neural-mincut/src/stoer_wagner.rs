//! Stoer-Wagner algorithm for global minimum cut of an undirected weighted graph.
//!
//! Time complexity: O(V^3) using a simple adjacency matrix representation.
//! The algorithm repeatedly performs "minimum cut phases" and merges vertices,
//! tracking the lightest cut found across all phases.

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::MincutResult;
use ruv_neural_core::{Result, RuvNeuralError};

/// Compute the global minimum cut of an undirected weighted graph using the
/// Stoer-Wagner algorithm.
///
/// Returns a [`MincutResult`] containing the cut value, the two partitions,
/// and the edges crossing the cut.
///
/// # Errors
///
/// Returns an error if the graph has fewer than two nodes.
pub fn stoer_wagner_mincut(graph: &BrainGraph) -> Result<MincutResult> {
    let n = graph.num_nodes;
    if n < 2 {
        return Err(RuvNeuralError::Mincut(
            "Stoer-Wagner requires at least 2 nodes".into(),
        ));
    }

    // Build adjacency matrix
    let adj = graph.adjacency_matrix();

    // Working copy of adjacency weights. We will merge rows/cols as the algorithm
    // contracts vertices.
    let mut w: Vec<Vec<f64>> = adj;

    // `merged[i]` holds the list of original node indices that have been merged
    // into supernode i.
    let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();

    // Which supernodes are still active.
    let mut active: Vec<bool> = vec![true; n];

    let mut best_cut_value = f64::INFINITY;
    let mut best_partition: Vec<usize> = Vec::new();

    // We need n-1 phases.
    for _ in 0..(n - 1) {
        let phase_result = minimum_cut_phase(&w, &active, &merged)?;

        if phase_result.cut_of_the_phase < best_cut_value {
            best_cut_value = phase_result.cut_of_the_phase;
            best_partition = phase_result.last_merged_group.clone();
        }

        // Merge the last two vertices of this phase.
        merge_vertices(
            &mut w,
            &mut merged,
            &mut active,
            phase_result.second_last,
            phase_result.last,
        );
    }

    // Build the two partitions.
    let mut partition_a: Vec<usize> = best_partition.clone();
    partition_a.sort_unstable();
    let partition_a_set: std::collections::HashSet<usize> =
        partition_a.iter().copied().collect();
    let mut partition_b: Vec<usize> = (0..n)
        .filter(|i| !partition_a_set.contains(i))
        .collect();
    partition_b.sort_unstable();

    // Find cut edges.
    let cut_edges = find_cut_edges(graph, &partition_a_set);

    Ok(MincutResult {
        cut_value: best_cut_value,
        partition_a,
        partition_b,
        cut_edges,
        timestamp: graph.timestamp,
    })
}

/// Result of a single phase of the Stoer-Wagner algorithm.
struct PhaseResult {
    /// The "cut of the phase" value — weight of edges from the last-added vertex
    /// to the rest of the merged set.
    cut_of_the_phase: f64,
    /// Index of the second-to-last vertex added in the ordering.
    second_last: usize,
    /// Index of the last vertex added in the ordering.
    last: usize,
    /// Original node indices that belong to the last-added supernode.
    last_merged_group: Vec<usize>,
}

/// Execute one phase of the Stoer-Wagner algorithm.
///
/// Greedily grows a set A by adding the most tightly connected vertex at each
/// step. Returns the cut of the phase (the weight connecting the last vertex
/// to the rest) and the indices needed for merging.
fn minimum_cut_phase(
    w: &[Vec<f64>],
    active: &[bool],
    merged: &[Vec<usize>],
) -> Result<PhaseResult> {
    let n = w.len();

    // Find all active nodes.
    let active_nodes: Vec<usize> = (0..n).filter(|&i| active[i]).collect();
    if active_nodes.len() < 2 {
        return Err(RuvNeuralError::Mincut(
            "Not enough active nodes for a phase".into(),
        ));
    }

    // key[v] = total weight of edges from v to the growing set A.
    let mut key: Vec<f64> = vec![0.0; n];
    let mut in_a: Vec<bool> = vec![false; n];

    let mut last = active_nodes[0];
    let mut second_last = active_nodes[0];

    // We add all active nodes one by one.
    for iteration in 0..active_nodes.len() {
        // On first iteration, pick an arbitrary active node as seed.
        if iteration == 0 {
            let seed = active_nodes[0];
            in_a[seed] = true;
            last = seed;
            // Update keys for neighbors of seed.
            for &v in &active_nodes {
                if !in_a[v] {
                    key[v] += w[seed][v];
                }
            }
            continue;
        }

        // Find the active node not in A with the maximum key.
        let mut best_node = usize::MAX;
        let mut best_key = -1.0;
        for &v in &active_nodes {
            if !in_a[v] && key[v] > best_key {
                best_key = key[v];
                best_node = v;
            }
        }

        second_last = last;
        last = best_node;
        in_a[best_node] = true;

        // Update keys.
        for &v in &active_nodes {
            if !in_a[v] {
                key[v] += w[best_node][v];
            }
        }
    }

    Ok(PhaseResult {
        cut_of_the_phase: key[last],
        second_last,
        last,
        last_merged_group: merged[last].clone(),
    })
}

/// Merge vertex `v` into vertex `u`, combining their adjacency weights and
/// original node sets.
fn merge_vertices(
    w: &mut [Vec<f64>],
    merged: &mut [Vec<usize>],
    active: &mut [bool],
    u: usize,
    v: usize,
) {
    let n = w.len();

    // Add v's weights into u.
    for i in 0..n {
        w[u][i] += w[v][i];
        w[i][u] += w[i][v];
    }
    // Zero out self-loop created by merge.
    w[u][u] = 0.0;

    // Move v's original nodes into u's group.
    let v_nodes: Vec<usize> = merged[v].drain(..).collect();
    merged[u].extend(v_nodes);

    // Deactivate v.
    active[v] = false;
}

/// Find all edges crossing the partition boundary.
fn find_cut_edges(
    graph: &BrainGraph,
    partition_a: &std::collections::HashSet<usize>,
) -> Vec<(usize, usize, f64)> {
    graph
        .edges
        .iter()
        .filter(|e| {
            let s_in_a = partition_a.contains(&e.source);
            let t_in_a = partition_a.contains(&e.target);
            s_in_a != t_in_a
        })
        .map(|e| (e.source, e.target, e.weight))
        .collect()
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

    /// Classic 4-node example:
    ///
    /// ```text
    ///   0 --2-- 1
    ///   |       |
    ///   3       3
    ///   |       |
    ///   2 --2-- 3
    /// ```
    ///
    /// Edge weights: 0-1:2, 0-2:3, 1-3:3, 2-3:2
    /// Expected minimum cut = 4 (partition {0,2} vs {1,3} or {0,1} vs {2,3}).
    #[test]
    fn test_stoer_wagner_known_graph() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 2.0),
                make_edge(0, 2, 3.0),
                make_edge(1, 3, 3.0),
                make_edge(2, 3, 2.0),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let result = stoer_wagner_mincut(&graph).unwrap();
        assert!(
            (result.cut_value - 4.0).abs() < 1e-9,
            "Expected mincut 4.0, got {}",
            result.cut_value
        );
        // Verify partition sizes sum to total.
        assert_eq!(
            result.partition_a.len() + result.partition_b.len(),
            4
        );
    }

    /// Complete graph K4 with unit weights: mincut = 3 (remove all edges to one vertex).
    #[test]
    fn test_stoer_wagner_complete_k4() {
        let mut edges = Vec::new();
        for i in 0..4 {
            for j in (i + 1)..4 {
                edges.push(make_edge(i, j, 1.0));
            }
        }
        let graph = BrainGraph {
            num_nodes: 4,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let result = stoer_wagner_mincut(&graph).unwrap();
        assert!(
            (result.cut_value - 3.0).abs() < 1e-9,
            "Expected mincut 3.0 for K4, got {}",
            result.cut_value
        );
    }

    /// Two disconnected components: mincut = 0.
    #[test]
    fn test_stoer_wagner_disconnected() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 5.0),
                make_edge(2, 3, 5.0),
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        let result = stoer_wagner_mincut(&graph).unwrap();
        assert!(
            result.cut_value.abs() < 1e-9,
            "Expected mincut 0.0 for disconnected graph, got {}",
            result.cut_value
        );
    }

    /// Graph with a single node should return an error.
    #[test]
    fn test_stoer_wagner_single_node() {
        let graph = BrainGraph {
            num_nodes: 1,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(1),
        };
        assert!(stoer_wagner_mincut(&graph).is_err());
    }

    /// Complete graph K_n: mincut = n - 1 (unit weights).
    #[test]
    fn test_stoer_wagner_complete_kn() {
        for n in 3..=6 {
            let mut edges = Vec::new();
            for i in 0..n {
                for j in (i + 1)..n {
                    edges.push(make_edge(i, j, 1.0));
                }
            }
            let graph = BrainGraph {
                num_nodes: n,
                edges,
                timestamp: 0.0,
                window_duration_s: 1.0,
                atlas: Atlas::Custom(n),
            };
            let result = stoer_wagner_mincut(&graph).unwrap();
            let expected = (n - 1) as f64;
            assert!(
                (result.cut_value - expected).abs() < 1e-9,
                "K{}: expected mincut {}, got {}",
                n,
                expected,
                result.cut_value
            );
        }
    }
}
