//! Mincut Computation Module
//!
//! Implements the Stoer-Wagner algorithm for computing the global minimum cut
//! on the contracted graph. This is the PRIMARY integrity metric.
//!
//! Complexity: O(VE + V^2 log V) where V is number of nodes and E is edges.
//!
//! IMPORTANT: This computes lambda_cut (minimum cut value), NOT lambda2
//! (algebraic connectivity). These are different concepts:
//! - lambda_cut: Minimum number of edges to disconnect the graph
//! - lambda2: Second smallest eigenvalue of the Laplacian (spectral stress)

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::contracted_graph::{ContractedGraph, NodeType};

/// Configuration for mincut computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MincutConfig {
    /// Whether to compute lambda2 (algebraic connectivity) as well
    pub compute_lambda2: bool,
    /// Maximum iterations for power iteration (lambda2)
    pub max_iterations: usize,
    /// Convergence tolerance for power iteration
    pub tolerance: f64,
}

impl Default for MincutConfig {
    fn default() -> Self {
        Self {
            compute_lambda2: false,
            max_iterations: 100,
            tolerance: 1e-8,
        }
    }
}

/// Result of mincut computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MincutResult {
    /// Minimum cut value (PRIMARY METRIC)
    pub lambda_cut: f32,
    /// Algebraic connectivity (OPTIONAL DRIFT SIGNAL)
    pub lambda2: Option<f32>,
    /// Edges participating in the minimum cut
    pub witness_edges: Vec<WitnessEdge>,
    /// Partition of nodes on one side of the cut
    pub cut_partition: Vec<usize>,
    /// Computation time in milliseconds
    pub computation_time_ms: u64,
}

impl MincutResult {
    /// Check if the graph is well-connected
    pub fn is_well_connected(&self, threshold: f32) -> bool {
        self.lambda_cut >= threshold
    }

    /// Get the number of witness edges
    pub fn witness_count(&self) -> usize {
        self.witness_edges.len()
    }
}

/// An edge that participates in the minimum cut
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEdge {
    /// Source node type
    pub source_type: String,
    /// Source node ID
    pub source_id: i64,
    /// Target node type
    pub target_type: String,
    /// Target node ID
    pub target_id: i64,
    /// Edge type
    pub edge_type: String,
    /// Edge capacity
    pub capacity: f32,
    /// Current flow on the edge
    pub flow: f32,
}

/// Mincut computer using Stoer-Wagner algorithm
pub struct MincutComputer {
    config: MincutConfig,
}

impl MincutComputer {
    /// Create a new mincut computer
    pub fn new() -> Self {
        Self {
            config: MincutConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: MincutConfig) -> Self {
        Self { config }
    }

    /// Compute the minimum cut on a contracted graph
    pub fn compute(&self, graph: &ContractedGraph) -> MincutResult {
        let n = graph.nodes.len();

        // Handle trivial cases
        if n < 2 {
            return MincutResult {
                lambda_cut: 0.0,
                lambda2: None,
                witness_edges: vec![],
                cut_partition: vec![],
                computation_time_ms: 0,
            };
        }

        let start = std::time::Instant::now();

        // Build capacity matrix
        let (capacity, node_index) = graph.build_capacity_matrix();

        // Compute global mincut using Stoer-Wagner
        let (lambda_cut, cut_partition) = self.stoer_wagner_mincut(&capacity);

        // Find witness edges (edges crossing the cut)
        let witness_edges = self.find_witness_edges(graph, &node_index, &cut_partition);

        // Optionally compute lambda2 (algebraic connectivity)
        let lambda2 = if self.config.compute_lambda2 {
            Some(self.compute_algebraic_connectivity(&capacity, n) as f32)
        } else {
            None
        };

        let computation_time_ms = start.elapsed().as_millis() as u64;

        MincutResult {
            lambda_cut: lambda_cut as f32,
            lambda2,
            witness_edges,
            cut_partition,
            computation_time_ms,
        }
    }

    /// Stoer-Wagner algorithm for global minimum cut
    /// Returns (mincut_value, partition of nodes on one side)
    fn stoer_wagner_mincut(&self, capacity: &[Vec<f64>]) -> (f64, Vec<usize>) {
        let n = capacity.len();

        if n == 0 {
            return (0.0, vec![]);
        }

        if n == 1 {
            return (0.0, vec![0]);
        }

        let mut best_cut = f64::MAX;
        let mut best_partition = vec![];

        // Working copies
        let mut vertices: Vec<usize> = (0..n).collect();
        let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
        let mut cap = capacity.to_vec();

        while vertices.len() > 1 {
            // Maximum adjacency search to find s-t cut
            let (s_idx, t_idx, cut_of_phase) = self.minimum_cut_phase(&vertices, &cap);

            if cut_of_phase < best_cut {
                best_cut = cut_of_phase;
                best_partition = merged[vertices[t_idx]].clone();
            }

            // Get actual vertex indices
            let t_vertex = vertices[t_idx];
            let s_vertex = vertices[s_idx];

            // Update capacities - merge t into s
            for &v in &vertices {
                if v != s_vertex && v != t_vertex {
                    cap[s_vertex][v] += cap[t_vertex][v];
                    cap[v][s_vertex] += cap[v][t_vertex];
                }
            }

            // Merge vertex sets
            let t_merged = merged[t_vertex].clone();
            merged[s_vertex].extend(t_merged);

            // Remove t from active vertices
            vertices.remove(t_idx);
        }

        (best_cut, best_partition)
    }

    /// One phase of Stoer-Wagner: find minimum s-t cut using maximum adjacency search
    /// Returns (s_index, t_index, cut_of_phase) where indices are into vertices array
    fn minimum_cut_phase(&self, vertices: &[usize], cap: &[Vec<f64>]) -> (usize, usize, f64) {
        let n = cap.len();
        let num_vertices = vertices.len();

        if num_vertices < 2 {
            return (0, 0, 0.0);
        }

        let mut in_a = vec![false; n];
        let mut cut_weight = vec![0.0f64; n];

        let mut last_idx = 0;
        let mut before_last_idx = 0;

        for _round in 0..num_vertices {
            // Find most tightly connected vertex not yet in A
            let mut max_weight = -1.0;
            let mut max_idx = 0;

            for (idx, &v) in vertices.iter().enumerate() {
                if !in_a[v] && (max_weight < 0.0 || cut_weight[v] > max_weight) {
                    max_weight = cut_weight[v];
                    max_idx = idx;
                }
            }

            let max_v = vertices[max_idx];
            in_a[max_v] = true;
            before_last_idx = last_idx;
            last_idx = max_idx;

            // Update cut weights for remaining vertices
            for (_, &v) in vertices.iter().enumerate() {
                if !in_a[v] {
                    cut_weight[v] += cap[max_v][v];
                }
            }
        }

        // The cut of the phase is the weight of t (the last vertex added)
        let t_vertex = vertices[last_idx];
        (before_last_idx, last_idx, cut_weight[t_vertex])
    }

    /// Find edges crossing the minimum cut (witness edges)
    fn find_witness_edges(
        &self,
        graph: &ContractedGraph,
        node_index: &HashMap<(NodeType, i64), usize>,
        partition: &[usize],
    ) -> Vec<WitnessEdge> {
        let partition_set: HashSet<_> = partition.iter().copied().collect();

        graph
            .edges
            .iter()
            .filter_map(|edge| {
                let i = node_index.get(&edge.source_key())?;
                let j = node_index.get(&edge.target_key())?;

                // Edge crosses cut if exactly one endpoint is in the partition
                let i_in = partition_set.contains(i);
                let j_in = partition_set.contains(j);

                if i_in != j_in {
                    Some(WitnessEdge {
                        source_type: edge.source_type.to_string(),
                        source_id: edge.source_id,
                        target_type: edge.target_type.to_string(),
                        target_id: edge.target_id,
                        edge_type: edge.edge_type.to_string(),
                        capacity: edge.capacity,
                        flow: edge.current_flow,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compute algebraic connectivity (lambda2) as optional drift signal
    /// This is DIFFERENT from mincut - provides spectral stress insight
    fn compute_algebraic_connectivity(&self, capacity: &[Vec<f64>], n: usize) -> f64 {
        if n < 2 {
            return 0.0;
        }

        // Build Laplacian: L = D - A
        let mut laplacian = vec![vec![0.0f64; n]; n];
        for i in 0..n {
            let degree: f64 = capacity[i].iter().sum();
            laplacian[i][i] = degree;
            for j in 0..n {
                laplacian[i][j] -= capacity[i][j];
            }
        }

        // Power iteration to find second smallest eigenvalue
        // Start with random vector orthogonal to constant vector
        let mut v: Vec<f64> = (0..n).map(|i| (i as f64 * 0.7).sin()).collect();

        // Orthogonalize against constant vector (normalize)
        let mean: f64 = v.iter().sum::<f64>() / n as f64;
        for x in &mut v {
            *x -= mean;
        }

        // Normalize
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-10 {
            for x in &mut v {
                *x /= norm;
            }
        }

        // Shifted inverse power iteration for second smallest eigenvalue
        // We want to find λ₂, so we shift to find smallest non-zero eigenvalue
        let shift = 0.001; // Small shift to avoid singular matrix

        for _ in 0..self.config.max_iterations {
            // Compute L*v
            let mut lv = vec![0.0f64; n];
            for i in 0..n {
                for j in 0..n {
                    lv[i] += laplacian[i][j] * v[j];
                }
            }

            // Apply shift: (L + shift*I) * v
            for i in 0..n {
                lv[i] += shift * v[i];
            }

            // Orthogonalize against constant vector
            let mean: f64 = lv.iter().sum::<f64>() / n as f64;
            for x in &mut lv {
                *x -= mean;
            }

            // Normalize
            let norm: f64 = lv.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm < 1e-10 {
                break;
            }

            let mut new_v = lv;
            for x in &mut new_v {
                *x /= norm;
            }

            // Check convergence
            let diff: f64 = v.iter().zip(new_v.iter()).map(|(a, b)| (a - b).abs()).sum();

            v = new_v;

            if diff < self.config.tolerance {
                break;
            }
        }

        // Rayleigh quotient: λ = (v^T L v) / (v^T v)
        let mut vtlv = 0.0f64;
        for i in 0..n {
            for j in 0..n {
                vtlv += v[i] * laplacian[i][j] * v[j];
            }
        }
        let vtv: f64 = v.iter().map(|x| x * x).sum();

        if vtv > 1e-10 {
            (vtlv / vtv).max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for MincutComputer {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute mincut for a given collection
pub fn compute_mincut(graph: &ContractedGraph) -> MincutResult {
    MincutComputer::new().compute(graph)
}

/// Compute mincut with lambda2
pub fn compute_mincut_with_lambda2(graph: &ContractedGraph) -> MincutResult {
    MincutComputer::with_config(MincutConfig {
        compute_lambda2: true,
        ..Default::default()
    })
    .compute(graph)
}

#[cfg(test)]
mod tests {
    use super::super::contracted_graph::{ContractedGraphBuilder, EdgeType};
    use super::*;

    #[test]
    fn test_mincut_empty_graph() {
        let graph = ContractedGraph::new(1);
        let result = compute_mincut(&graph);
        assert_eq!(result.lambda_cut, 0.0);
        assert!(result.witness_edges.is_empty());
    }

    #[test]
    fn test_mincut_single_node() {
        let mut graph = ContractedGraph::new(1);
        graph.add_node(super::super::contracted_graph::ContractedNode::new(
            1,
            NodeType::Partition,
            0,
        ));

        let result = compute_mincut(&graph);
        assert_eq!(result.lambda_cut, 0.0);
    }

    #[test]
    fn test_mincut_two_connected_nodes() {
        use super::super::contracted_graph::{ContractedEdge, ContractedNode};

        let mut graph = ContractedGraph::new(1);
        graph.add_node(ContractedNode::new(1, NodeType::Partition, 0));
        graph.add_node(ContractedNode::new(1, NodeType::Partition, 1));
        graph.add_edge(
            ContractedEdge::new(
                1,
                NodeType::Partition,
                0,
                NodeType::Partition,
                1,
                EdgeType::PartitionLink,
            )
            .with_capacity(5.0),
        );

        let result = compute_mincut(&graph);
        assert!((result.lambda_cut - 5.0).abs() < 0.01);
        assert_eq!(result.witness_edges.len(), 1);
    }

    #[test]
    fn test_mincut_triangle() {
        use super::super::contracted_graph::{ContractedEdge, ContractedNode};

        let mut graph = ContractedGraph::new(1);
        for i in 0..3 {
            graph.add_node(ContractedNode::new(1, NodeType::Partition, i));
        }

        // Create triangle with edges of capacity 1.0
        graph.add_edge(
            ContractedEdge::new(
                1,
                NodeType::Partition,
                0,
                NodeType::Partition,
                1,
                EdgeType::PartitionLink,
            )
            .with_capacity(1.0),
        );
        graph.add_edge(
            ContractedEdge::new(
                1,
                NodeType::Partition,
                1,
                NodeType::Partition,
                2,
                EdgeType::PartitionLink,
            )
            .with_capacity(1.0),
        );
        graph.add_edge(
            ContractedEdge::new(
                1,
                NodeType::Partition,
                0,
                NodeType::Partition,
                2,
                EdgeType::PartitionLink,
            )
            .with_capacity(1.0),
        );

        let result = compute_mincut(&graph);
        // Mincut of a triangle is 2 (cut one node from the other two)
        assert!((result.lambda_cut - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_mincut_with_lambda2() {
        let graph = ContractedGraphBuilder::build_default(1, 5, 0, 1);
        let result = compute_mincut_with_lambda2(&graph);

        assert!(result.lambda2.is_some());
        assert!(result.lambda2.unwrap() >= 0.0);
    }

    #[test]
    fn test_mincut_default_graph() {
        let graph = ContractedGraphBuilder::build_default(1, 5, 10, 2);
        let result = compute_mincut(&graph);

        assert!(result.lambda_cut >= 0.0);
        assert!(result.computation_time_ms < 10000); // Should complete quickly
    }

    #[test]
    fn test_witness_edges() {
        use super::super::contracted_graph::{ContractedEdge, ContractedNode};

        let mut graph = ContractedGraph::new(1);
        graph.add_node(ContractedNode::new(1, NodeType::Partition, 0));
        graph.add_node(ContractedNode::new(1, NodeType::Partition, 1));
        graph.add_edge(
            ContractedEdge::new(
                1,
                NodeType::Partition,
                0,
                NodeType::Partition,
                1,
                EdgeType::PartitionLink,
            )
            .with_capacity(1.0)
            .with_flow(0.5),
        );

        let result = compute_mincut(&graph);
        assert_eq!(result.witness_edges.len(), 1);

        let witness = &result.witness_edges[0];
        assert_eq!(witness.source_type, "partition");
        assert_eq!(witness.edge_type, "partition_link");
        assert_eq!(witness.capacity, 1.0);
        assert_eq!(witness.flow, 0.5);
    }
}
