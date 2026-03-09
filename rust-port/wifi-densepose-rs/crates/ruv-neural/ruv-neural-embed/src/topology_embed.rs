//! Topology-based graph embedding.
//!
//! Extracts a feature vector of hand-crafted topological metrics from a brain graph,
//! including mincut estimate, modularity, efficiency, degree statistics, and more.

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::Result;
use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::traits::EmbeddingGenerator;

use crate::default_metadata;

/// Topology-based embedder: converts a brain graph into a vector of topological features.
pub struct TopologyEmbedder {
    /// Include global minimum cut estimate.
    pub include_mincut: bool,
    /// Include modularity estimate.
    pub include_modularity: bool,
    /// Include global and local efficiency.
    pub include_efficiency: bool,
    /// Include degree distribution statistics.
    pub include_degree_stats: bool,
}

impl TopologyEmbedder {
    /// Create a new topology embedder with all features enabled.
    pub fn new() -> Self {
        Self {
            include_mincut: true,
            include_modularity: true,
            include_efficiency: true,
            include_degree_stats: true,
        }
    }

    /// Estimate global minimum cut via the minimum node degree.
    fn estimate_mincut(graph: &BrainGraph) -> f64 {
        if graph.num_nodes < 2 {
            return 0.0;
        }
        (0..graph.num_nodes)
            .map(|i| graph.node_degree(i))
            .fold(f64::INFINITY, f64::min)
    }

    /// Estimate modularity using a simple greedy two-partition.
    fn estimate_modularity(graph: &BrainGraph) -> f64 {
        let n = graph.num_nodes;
        if n < 2 {
            return 0.0;
        }
        let total_weight = graph.total_weight();
        if total_weight < 1e-12 {
            return 0.0;
        }

        let adj = graph.adjacency_matrix();
        let degrees: Vec<f64> = (0..n).map(|i| graph.node_degree(i)).collect();

        let mut sorted_degrees: Vec<(usize, f64)> =
            degrees.iter().copied().enumerate().collect();
        sorted_degrees.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let mid = n / 2;

        let mut partition = vec![0i32; n];
        for (rank, &(node, _)) in sorted_degrees.iter().enumerate() {
            partition[node] = if rank < mid { 1 } else { -1 };
        }

        let two_m = 2.0 * total_weight;
        let mut q = 0.0;
        for i in 0..n {
            for j in 0..n {
                if partition[i] == partition[j] {
                    q += adj[i][j] - degrees[i] * degrees[j] / two_m;
                }
            }
        }
        q / two_m
    }

    /// Compute global efficiency: average of 1/shortest_path for all node pairs.
    fn global_efficiency(graph: &BrainGraph) -> f64 {
        let n = graph.num_nodes;
        if n < 2 {
            return 0.0;
        }

        let adj = graph.adjacency_matrix();
        let mut sum_inv_dist = 0.0;

        for source in 0..n {
            let mut dist = vec![usize::MAX; n];
            dist[source] = 0;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(source);

            while let Some(u) = queue.pop_front() {
                for v in 0..n {
                    if dist[v] == usize::MAX && adj[u][v] > 1e-12 {
                        dist[v] = dist[u] + 1;
                        queue.push_back(v);
                    }
                }
            }

            for v in 0..n {
                if v != source && dist[v] != usize::MAX {
                    sum_inv_dist += 1.0 / dist[v] as f64;
                }
            }
        }

        sum_inv_dist / (n * (n - 1)) as f64
    }

    /// Compute mean local efficiency.
    fn local_efficiency(graph: &BrainGraph) -> f64 {
        let n = graph.num_nodes;
        if n == 0 {
            return 0.0;
        }

        let adj = graph.adjacency_matrix();
        let mut total = 0.0;

        for node in 0..n {
            let neighbors: Vec<usize> = (0..n)
                .filter(|&j| j != node && adj[node][j] > 1e-12)
                .collect();
            let k = neighbors.len();
            if k < 2 {
                continue;
            }

            let mut sub_sum = 0.0;
            for &i in &neighbors {
                for &j in &neighbors {
                    if i != j && adj[i][j] > 1e-12 {
                        sub_sum += 1.0;
                    }
                }
            }
            total += sub_sum / (k * (k - 1)) as f64;
        }

        total / n as f64
    }

    /// Compute graph entropy from edge weight distribution.
    fn graph_entropy(graph: &BrainGraph) -> f64 {
        if graph.edges.is_empty() {
            return 0.0;
        }
        let total: f64 = graph.edges.iter().map(|e| e.weight.abs()).sum();
        if total < 1e-12 {
            return 0.0;
        }

        let mut entropy = 0.0;
        for edge in &graph.edges {
            let p = edge.weight.abs() / total;
            if p > 1e-12 {
                entropy -= p * p.ln();
            }
        }
        entropy
    }

    /// Estimate the Fiedler value (algebraic connectivity).
    fn estimate_fiedler(graph: &BrainGraph) -> f64 {
        let n = graph.num_nodes;
        if n < 2 {
            return 0.0;
        }

        let adj = graph.adjacency_matrix();
        let degrees: Vec<f64> = (0..n).map(|i| adj[i].iter().sum::<f64>()).collect();

        let mut laplacian = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    laplacian[i][j] = degrees[i];
                } else {
                    laplacian[i][j] = -adj[i][j];
                }
            }
        }

        let max_eig: f64 = (0..n)
            .map(|i| {
                let diag = laplacian[i][i];
                let off: f64 = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| laplacian[i][j].abs())
                    .sum();
                diag + off
            })
            .fold(0.0_f64, f64::max);

        let e0: Vec<f64> = vec![1.0 / (n as f64).sqrt(); n];

        let mut v: Vec<f64> = (0..n).map(|i| ((i + 1) as f64).sin()).collect();
        let dot0: f64 = v.iter().zip(e0.iter()).map(|(a, b)| a * b).sum();
        for i in 0..n {
            v[i] -= dot0 * e0[i];
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-12 {
            return 0.0;
        }
        for x in &mut v {
            *x /= norm;
        }

        let mut eigenvalue = 0.0;
        for _ in 0..200 {
            let mut w = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    if i == j {
                        w[i] += (max_eig - laplacian[i][j]) * v[j];
                    } else {
                        w[i] += -laplacian[i][j] * v[j];
                    }
                }
            }

            let dot: f64 = w.iter().zip(e0.iter()).map(|(a, b)| a * b).sum();
            for i in 0..n {
                w[i] -= dot * e0[i];
            }

            let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm < 1e-12 {
                break;
            }
            eigenvalue = norm;
            for x in &mut w {
                *x /= norm;
            }
            v = w;
        }

        (max_eig - eigenvalue).max(0.0)
    }

    /// Compute average clustering coefficient.
    fn clustering_coefficient(graph: &BrainGraph) -> f64 {
        let n = graph.num_nodes;
        if n == 0 {
            return 0.0;
        }

        let adj = graph.adjacency_matrix();
        let mut total = 0.0;

        for node in 0..n {
            let neighbors: Vec<usize> = (0..n)
                .filter(|&j| j != node && adj[node][j] > 1e-12)
                .collect();
            let k = neighbors.len();
            if k < 2 {
                continue;
            }

            let mut triangles = 0usize;
            for i in 0..k {
                for j in (i + 1)..k {
                    if adj[neighbors[i]][neighbors[j]] > 1e-12 {
                        triangles += 1;
                    }
                }
            }
            total += 2.0 * triangles as f64 / (k * (k - 1)) as f64;
        }

        total / n as f64
    }

    /// Count connected components via BFS.
    fn num_components(graph: &BrainGraph) -> usize {
        let n = graph.num_nodes;
        if n == 0 {
            return 0;
        }

        let adj = graph.adjacency_matrix();
        let mut visited = vec![false; n];
        let mut count = 0;

        for start in 0..n {
            if visited[start] {
                continue;
            }
            count += 1;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            visited[start] = true;
            while let Some(u) = queue.pop_front() {
                for v in 0..n {
                    if !visited[v] && adj[u][v] > 1e-12 {
                        visited[v] = true;
                        queue.push_back(v);
                    }
                }
            }
        }

        count
    }

    /// Generate the topology embedding.
    pub fn embed_graph(&self, graph: &BrainGraph) -> Result<NeuralEmbedding> {
        let mut values = Vec::new();

        if self.include_mincut {
            values.push(Self::estimate_mincut(graph));
        }

        if self.include_modularity {
            values.push(Self::estimate_modularity(graph));
        }

        if self.include_efficiency {
            values.push(Self::global_efficiency(graph));
            values.push(Self::local_efficiency(graph));
        }

        values.push(Self::graph_entropy(graph));
        values.push(Self::estimate_fiedler(graph));

        if self.include_degree_stats {
            let n = graph.num_nodes;
            let degrees: Vec<f64> = (0..n).map(|i| graph.node_degree(i)).collect();

            let mean_deg = if n > 0 {
                degrees.iter().sum::<f64>() / n as f64
            } else {
                0.0
            };
            let std_deg = if n > 0 {
                let var =
                    degrees.iter().map(|d| (d - mean_deg).powi(2)).sum::<f64>() / n as f64;
                var.sqrt()
            } else {
                0.0
            };
            let max_deg = degrees.iter().cloned().fold(0.0_f64, f64::max);
            let min_deg = degrees.iter().cloned().fold(f64::INFINITY, f64::min);
            let min_deg = if min_deg.is_infinite() { 0.0 } else { min_deg };

            values.push(mean_deg);
            values.push(std_deg);
            values.push(max_deg);
            values.push(min_deg);
        }

        values.push(graph.density());
        values.push(Self::clustering_coefficient(graph));
        values.push(Self::num_components(graph) as f64);

        let meta = default_metadata("topology", graph.atlas);
        NeuralEmbedding::new(values, graph.timestamp, meta)
    }

    /// Number of features produced with current settings.
    pub fn feature_count(&self) -> usize {
        let mut count = 0;
        if self.include_mincut {
            count += 1;
        }
        if self.include_modularity {
            count += 1;
        }
        if self.include_efficiency {
            count += 2;
        }
        count += 2; // entropy + fiedler
        if self.include_degree_stats {
            count += 4;
        }
        count += 3; // density, clustering, components
        count
    }
}

impl Default for TopologyEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingGenerator for TopologyEmbedder {
    fn embedding_dim(&self) -> usize {
        self.feature_count()
    }

    fn embed(&self, graph: &BrainGraph) -> Result<NeuralEmbedding> {
        self.embed_graph(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_triangle() -> BrainGraph {
        BrainGraph {
            num_nodes: 3,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 0,
                    target: 2,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        }
    }

    #[test]
    fn test_topology_embed_triangle() {
        let graph = make_triangle();
        let embedder = TopologyEmbedder::new();
        let emb = embedder.embed(&graph).unwrap();

        assert_eq!(emb.dimension, embedder.feature_count());
        assert_eq!(emb.metadata.embedding_method, "topology");

        let dim = emb.dimension;
        // Last three values: density, clustering, components
        assert!((emb.vector[dim - 3] - 1.0).abs() < 1e-10, "density should be 1.0");
        assert!((emb.vector[dim - 2] - 1.0).abs() < 1e-10, "clustering should be 1.0");
        assert!((emb.vector[dim - 1] - 1.0).abs() < 1e-10, "should be 1 component");
    }

    #[test]
    fn test_topology_captures_known_features() {
        let graph = make_triangle();
        let embedder = TopologyEmbedder::new();
        let emb = embedder.embed(&graph).unwrap();

        // Global efficiency of K3: all pairs distance 1, so efficiency = 1.0
        // index: mincut(0), modularity(1), global_eff(2), local_eff(3)
        assert!(
            (emb.vector[2] - 1.0).abs() < 1e-10,
            "global efficiency of K3 should be 1.0, got {}",
            emb.vector[2]
        );
    }

    #[test]
    fn test_empty_graph() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };
        let embedder = TopologyEmbedder::new();
        let emb = embedder.embed(&graph).unwrap();
        let dim = emb.dimension;
        assert!((emb.vector[dim - 3]).abs() < 1e-10);
        assert!((emb.vector[dim - 2]).abs() < 1e-10);
        assert!((emb.vector[dim - 1] - 4.0).abs() < 1e-10);
    }
}
