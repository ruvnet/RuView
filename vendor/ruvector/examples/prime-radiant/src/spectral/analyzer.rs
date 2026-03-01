//! Core Spectral Analyzer
//!
//! Provides the main `SpectralAnalyzer` struct for computing spectral properties
//! of graphs, including eigenvalues, eigenvectors, and derived invariants.

use super::lanczos::{LanczosAlgorithm, PowerIteration};
use super::types::{Graph, SparseMatrix, SpectralGap, Vector, Bottleneck, MinCutPrediction, EPS, NodeId};
use serde::{Deserialize, Serialize};

/// Core spectral analyzer for graph analysis
#[derive(Debug, Clone)]
pub struct SpectralAnalyzer {
    /// The graph being analyzed
    pub graph: Graph,
    /// Graph Laplacian matrix
    pub laplacian: SparseMatrix,
    /// Normalized Laplacian matrix
    pub normalized_laplacian: SparseMatrix,
    /// Computed eigenvalues (sorted ascending)
    pub eigenvalues: Vec<f64>,
    /// Corresponding eigenvectors
    pub eigenvectors: Vec<Vector>,
    /// Configuration
    config: SpectralConfig,
}

/// Configuration for spectral analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralConfig {
    /// Number of eigenvalues to compute
    pub num_eigenvalues: usize,
    /// Use normalized Laplacian
    pub use_normalized: bool,
    /// Maximum iterations for eigenvalue computation
    pub max_iter: usize,
    /// Convergence tolerance
    pub tol: f64,
}

impl Default for SpectralConfig {
    fn default() -> Self {
        Self {
            num_eigenvalues: 10,
            use_normalized: true,
            max_iter: 1000,
            tol: 1e-10,
        }
    }
}

impl SpectralConfig {
    /// Create a builder for configuration
    pub fn builder() -> SpectralConfigBuilder {
        SpectralConfigBuilder::default()
    }
}

/// Builder for SpectralConfig
#[derive(Default)]
pub struct SpectralConfigBuilder {
    config: SpectralConfig,
}

impl SpectralConfigBuilder {
    /// Set number of eigenvalues to compute
    pub fn num_eigenvalues(mut self, n: usize) -> Self {
        self.config.num_eigenvalues = n;
        self
    }

    /// Use normalized Laplacian
    pub fn normalized(mut self, use_norm: bool) -> Self {
        self.config.use_normalized = use_norm;
        self
    }

    /// Set maximum iterations
    pub fn max_iter(mut self, n: usize) -> Self {
        self.config.max_iter = n;
        self
    }

    /// Set convergence tolerance
    pub fn tolerance(mut self, tol: f64) -> Self {
        self.config.tol = tol;
        self
    }

    /// Build the configuration
    pub fn build(self) -> SpectralConfig {
        self.config
    }
}

impl SpectralAnalyzer {
    /// Create a new spectral analyzer for a graph
    pub fn new(graph: Graph) -> Self {
        Self::with_config(graph, SpectralConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(graph: Graph, config: SpectralConfig) -> Self {
        let laplacian = graph.laplacian();
        let normalized_laplacian = graph.normalized_laplacian();

        Self {
            graph,
            laplacian,
            normalized_laplacian,
            eigenvalues: Vec::new(),
            eigenvectors: Vec::new(),
            config,
        }
    }

    /// Compute the Laplacian spectrum
    pub fn compute_laplacian_spectrum(&mut self) -> &[f64] {
        let matrix = if self.config.use_normalized {
            &self.normalized_laplacian
        } else {
            &self.laplacian
        };

        let lanczos = LanczosAlgorithm::new(self.config.num_eigenvalues);
        let (eigenvalues, eigenvectors) = lanczos.compute_smallest(matrix);

        self.eigenvalues = eigenvalues;
        self.eigenvectors = eigenvectors;

        &self.eigenvalues
    }

    /// Get the algebraic connectivity (second smallest eigenvalue)
    /// Also known as the Fiedler value
    pub fn algebraic_connectivity(&self) -> f64 {
        if self.eigenvalues.len() < 2 {
            return 0.0;
        }

        // Skip the first eigenvalue (should be 0 for connected graphs)
        // Find the first non-trivial eigenvalue
        for &ev in &self.eigenvalues {
            if ev > EPS {
                return ev;
            }
        }

        0.0
    }

    /// Get the Fiedler vector (eigenvector for second smallest eigenvalue)
    pub fn fiedler_vector(&self) -> Option<&Vector> {
        if self.eigenvectors.len() < 2 {
            return None;
        }

        // Find index of first non-trivial eigenvalue
        for (i, &ev) in self.eigenvalues.iter().enumerate() {
            if ev > EPS {
                return self.eigenvectors.get(i);
            }
        }

        None
    }

    /// Compute the spectral gap
    pub fn spectral_gap(&self) -> SpectralGap {
        let lambda_1 = self.algebraic_connectivity();
        let lambda_2 = if self.eigenvalues.len() >= 3 {
            // Find third non-trivial eigenvalue
            let mut count = 0;
            for &ev in &self.eigenvalues {
                if ev > EPS {
                    count += 1;
                    if count == 2 {
                        return SpectralGap::new(lambda_1, ev);
                    }
                }
            }
            lambda_1 * 2.0 // Default if not enough eigenvalues
        } else {
            lambda_1 * 2.0
        };

        SpectralGap::new(lambda_1, lambda_2)
    }

    /// Predict minimum cut difficulty using spectral gap
    pub fn predict_min_cut(&self) -> MinCutPrediction {
        let fiedler_value = self.algebraic_connectivity();
        let n = self.graph.n;
        let total_weight = self.graph.total_weight();

        // Cheeger inequality bounds on isoperimetric number
        // h(G) >= lambda_2 / 2 (lower bound)
        // h(G) <= sqrt(2 * lambda_2) (upper bound)

        let lower_bound = fiedler_value / 2.0;
        let upper_bound = (2.0 * fiedler_value).sqrt();

        // Predicted cut based on isoperimetric number and graph volume
        let predicted_cut = if total_weight > EPS {
            // Cut value ~ h(G) * min_volume
            // For balanced cut, min_volume ~ total_weight / 2
            let avg_bound = (lower_bound + upper_bound) / 2.0;
            avg_bound * total_weight / 2.0
        } else {
            0.0
        };

        // Compute confidence based on spectral gap clarity
        let gap = self.spectral_gap();
        let confidence = if gap.ratio > 2.0 {
            0.9 // Clear separation
        } else if gap.ratio > 1.5 {
            0.7
        } else if gap.ratio > 1.2 {
            0.5
        } else {
            0.3 // Gap unclear, low confidence
        };

        // Suggest cut nodes from Fiedler vector
        let cut_nodes = self.find_spectral_cut();

        MinCutPrediction {
            predicted_cut,
            lower_bound: lower_bound * total_weight / 2.0,
            upper_bound: upper_bound * total_weight / 2.0,
            confidence,
            cut_nodes,
        }
    }

    /// Find the optimal cut using the Fiedler vector
    fn find_spectral_cut(&self) -> Vec<NodeId> {
        let fiedler = match self.fiedler_vector() {
            Some(v) => v,
            None => return Vec::new(),
        };

        // Simple threshold at zero
        let positive_nodes: Vec<NodeId> = fiedler
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > 0.0)
            .map(|(i, _)| i)
            .collect();

        let negative_nodes: Vec<NodeId> = fiedler
            .iter()
            .enumerate()
            .filter(|(_, &v)| v <= 0.0)
            .map(|(i, _)| i)
            .collect();

        // Return the smaller set (typically defines the cut boundary)
        if positive_nodes.len() <= negative_nodes.len() {
            positive_nodes
        } else {
            negative_nodes
        }
    }

    /// Detect structural bottlenecks via Fiedler vector analysis
    pub fn detect_bottlenecks(&self) -> Vec<Bottleneck> {
        let fiedler = match self.fiedler_vector() {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };

        let n = self.graph.n;
        let mut bottlenecks = Vec::new();

        // Sort nodes by Fiedler value
        let mut sorted_indices: Vec<usize> = (0..n).collect();
        sorted_indices.sort_by(|&a, &b| {
            fiedler[a].partial_cmp(&fiedler[b]).unwrap()
        });

        // Find bottleneck at median split
        let mid = n / 2;
        let left_set: Vec<NodeId> = sorted_indices[..mid].to_vec();
        let right_set: Vec<NodeId> = sorted_indices[mid..].to_vec();

        // Find crossing edges
        let left_set_hashset: std::collections::HashSet<NodeId> =
            left_set.iter().cloned().collect();

        let mut crossing_edges = Vec::new();
        for &u in &left_set {
            for &(v, _) in &self.graph.adj[u] {
                if !left_set_hashset.contains(&v) {
                    crossing_edges.push((u.min(v), u.max(v)));
                }
            }
        }
        crossing_edges.sort();
        crossing_edges.dedup();

        // Compute bottleneck score (conductance)
        let left_volume: f64 = left_set.iter().map(|&i| self.graph.degree(i)).sum();
        let right_volume: f64 = right_set.iter().map(|&i| self.graph.degree(i)).sum();
        let cut_weight: f64 = crossing_edges
            .iter()
            .map(|&(u, v)| {
                self.graph.adj[u]
                    .iter()
                    .find(|(n, _)| *n == v)
                    .map(|(_, w)| *w)
                    .unwrap_or(0.0)
            })
            .sum();

        let min_volume = left_volume.min(right_volume);
        let score = if min_volume > EPS {
            cut_weight / min_volume
        } else {
            f64::INFINITY
        };

        let volume_ratio = if (left_volume + right_volume) > EPS {
            left_volume.min(right_volume) / (left_volume + right_volume)
        } else {
            0.5
        };

        // Find nodes at the bottleneck (near zero in Fiedler vector)
        let threshold = self.compute_fiedler_threshold(&fiedler);
        let bottleneck_nodes: Vec<NodeId> = fiedler
            .iter()
            .enumerate()
            .filter(|(_, &v)| v.abs() < threshold)
            .map(|(i, _)| i)
            .collect();

        bottlenecks.push(Bottleneck {
            nodes: bottleneck_nodes,
            crossing_edges,
            score,
            volume_ratio,
        });

        // Look for additional bottlenecks at different thresholds
        self.find_additional_bottlenecks(&sorted_indices, &fiedler, &mut bottlenecks);

        bottlenecks
    }

    /// Compute adaptive threshold for Fiedler vector
    fn compute_fiedler_threshold(&self, fiedler: &[f64]) -> f64 {
        let max_val = fiedler.iter().cloned().fold(0.0f64, f64::max);
        let min_val = fiedler.iter().cloned().fold(0.0f64, f64::min);
        let range = max_val - min_val;

        if range > EPS {
            range * 0.1 // 10% of range
        } else {
            0.01
        }
    }

    /// Find additional bottlenecks at quartile splits
    fn find_additional_bottlenecks(
        &self,
        sorted_indices: &[usize],
        fiedler: &[f64],
        bottlenecks: &mut Vec<Bottleneck>,
    ) {
        let n = self.graph.n;

        // Check at quartiles
        for &split_point in &[n / 4, 3 * n / 4] {
            if split_point == 0 || split_point >= n {
                continue;
            }

            let left_set: Vec<NodeId> = sorted_indices[..split_point].to_vec();
            let left_set_hashset: std::collections::HashSet<NodeId> =
                left_set.iter().cloned().collect();

            let mut crossing_edges = Vec::new();
            for &u in &left_set {
                for &(v, _) in &self.graph.adj[u] {
                    if !left_set_hashset.contains(&v) {
                        crossing_edges.push((u.min(v), u.max(v)));
                    }
                }
            }
            crossing_edges.sort();
            crossing_edges.dedup();

            let left_volume: f64 = left_set.iter().map(|&i| self.graph.degree(i)).sum();
            let right_volume: f64 = sorted_indices[split_point..]
                .iter()
                .map(|&i| self.graph.degree(i))
                .sum();

            let cut_weight: f64 = crossing_edges
                .iter()
                .map(|&(u, v)| {
                    self.graph.adj[u]
                        .iter()
                        .find(|(n, _)| *n == v)
                        .map(|(_, w)| *w)
                        .unwrap_or(0.0)
                })
                .sum();

            let min_volume = left_volume.min(right_volume);
            let score = if min_volume > EPS {
                cut_weight / min_volume
            } else {
                continue;
            };

            let volume_ratio = if (left_volume + right_volume) > EPS {
                left_volume.min(right_volume) / (left_volume + right_volume)
            } else {
                0.5
            };

            // Only add if it's a significantly different bottleneck
            if score < 0.9 * bottlenecks[0].score {
                let threshold_val = fiedler[sorted_indices[split_point]].abs() * 0.5;
                let bottleneck_nodes: Vec<NodeId> = fiedler
                    .iter()
                    .enumerate()
                    .filter(|(_, &v)| (v - fiedler[sorted_indices[split_point]]).abs() < threshold_val)
                    .map(|(i, _)| i)
                    .collect();

                bottlenecks.push(Bottleneck {
                    nodes: bottleneck_nodes,
                    crossing_edges,
                    score,
                    volume_ratio,
                });
            }
        }

        // Sort bottlenecks by score (ascending - lower is tighter)
        bottlenecks.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
    }

    /// Get spectral embedding of nodes (coordinates from eigenvectors)
    pub fn spectral_embedding(&self, dimensions: usize) -> Vec<Vector> {
        let n = self.graph.n;
        let dim = dimensions.min(self.eigenvectors.len());

        let mut embedding = vec![vec![0.0; dim]; n];

        // Skip the trivial eigenvector (constant)
        let start_idx = if self.eigenvalues.first().map(|&v| v < EPS).unwrap_or(false) {
            1
        } else {
            0
        };

        for d in 0..dim {
            let ev_idx = start_idx + d;
            if ev_idx < self.eigenvectors.len() {
                for i in 0..n {
                    embedding[i][d] = self.eigenvectors[ev_idx][i];
                }
            }
        }

        embedding
    }

    /// Compute the effective resistance between two nodes
    pub fn effective_resistance(&self, u: NodeId, v: NodeId) -> f64 {
        if u == v || self.eigenvalues.is_empty() {
            return 0.0;
        }

        let mut resistance = 0.0;

        // R_uv = sum_i (1/lambda_i) * (phi_i(u) - phi_i(v))^2
        // Skip the zero eigenvalue
        for (i, (&lambda, eigvec)) in self.eigenvalues.iter()
            .zip(self.eigenvectors.iter())
            .enumerate()
        {
            if lambda > EPS {
                let diff = eigvec[u] - eigvec[v];
                resistance += diff * diff / lambda;
            }
        }

        resistance
    }

    /// Compute total effective resistance (Kirchhoff index)
    pub fn kirchhoff_index(&self) -> f64 {
        let n = self.graph.n;

        if self.eigenvalues.is_empty() {
            return f64::INFINITY;
        }

        // K(G) = n * sum_i (1/lambda_i) for lambda_i > 0
        let sum_reciprocal: f64 = self.eigenvalues
            .iter()
            .filter(|&&lambda| lambda > EPS)
            .map(|&lambda| 1.0 / lambda)
            .sum();

        n as f64 * sum_reciprocal
    }

    /// Estimate the spectral radius (largest eigenvalue)
    pub fn spectral_radius(&self) -> f64 {
        let power = PowerIteration::default();
        let (lambda, _) = power.largest_eigenvalue(&self.laplacian);
        lambda
    }

    /// Check if graph is bipartite using spectral properties
    pub fn is_bipartite(&self) -> bool {
        // A graph is bipartite iff lambda_max = -lambda_min for the adjacency matrix
        let adj = self.graph.adjacency_matrix();
        let power = PowerIteration::default();

        let (lambda_max, _) = power.largest_eigenvalue(&adj);
        let (lambda_min, _) = power.smallest_eigenvalue(&adj, 0.0);

        (lambda_max + lambda_min).abs() < 0.01
    }

    /// Get the number of connected components from eigenvalue spectrum
    pub fn spectral_components(&self) -> usize {
        // Count eigenvalues very close to zero
        self.eigenvalues
            .iter()
            .filter(|&&ev| ev.abs() < 1e-6)
            .count()
            .max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_path_graph(n: usize) -> Graph {
        let edges: Vec<(usize, usize, f64)> = (0..n - 1)
            .map(|i| (i, i + 1, 1.0))
            .collect();
        Graph::from_edges(n, &edges)
    }

    fn create_cycle_graph(n: usize) -> Graph {
        let mut edges: Vec<(usize, usize, f64)> = (0..n - 1)
            .map(|i| (i, i + 1, 1.0))
            .collect();
        edges.push((n - 1, 0, 1.0)); // Close the cycle
        Graph::from_edges(n, &edges)
    }

    fn create_complete_graph(n: usize) -> Graph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                edges.push((i, j, 1.0));
            }
        }
        Graph::from_edges(n, &edges)
    }

    #[test]
    fn test_analyzer_path_graph() {
        let g = create_path_graph(5);
        let mut analyzer = SpectralAnalyzer::new(g);
        analyzer.compute_laplacian_spectrum();

        // Path graph should have small algebraic connectivity
        let lambda_2 = analyzer.algebraic_connectivity();
        assert!(lambda_2 > 0.0);
        assert!(lambda_2 < 1.0);

        // Should have one component
        assert_eq!(analyzer.spectral_components(), 1);
    }

    #[test]
    fn test_analyzer_complete_graph() {
        let g = create_complete_graph(5);
        let mut analyzer = SpectralAnalyzer::new(g);
        analyzer.compute_laplacian_spectrum();

        // Complete graph has high algebraic connectivity
        let lambda_2 = analyzer.algebraic_connectivity();
        assert!(lambda_2 > 0.5);
    }

    #[test]
    fn test_fiedler_vector() {
        let g = create_path_graph(6);
        let mut analyzer = SpectralAnalyzer::new(g);
        analyzer.compute_laplacian_spectrum();

        let fiedler = analyzer.fiedler_vector();
        assert!(fiedler.is_some());

        let v = fiedler.unwrap();
        assert_eq!(v.len(), 6);

        // Fiedler vector should be approximately monotonic for path graph
        // (either increasing or decreasing)
    }

    #[test]
    fn test_bottleneck_detection() {
        // Create a barbell graph (two cliques connected by a single edge)
        let mut g = Graph::new(8);

        // First clique (0, 1, 2, 3)
        for i in 0..4 {
            for j in i + 1..4 {
                g.add_edge(i, j, 1.0);
            }
        }

        // Second clique (4, 5, 6, 7)
        for i in 4..8 {
            for j in i + 1..8 {
                g.add_edge(i, j, 1.0);
            }
        }

        // Bridge
        g.add_edge(3, 4, 1.0);

        let mut analyzer = SpectralAnalyzer::new(g);
        analyzer.compute_laplacian_spectrum();

        let bottlenecks = analyzer.detect_bottlenecks();
        assert!(!bottlenecks.is_empty());

        // The bottleneck should include the bridge edge
        let bridge_found = bottlenecks.iter().any(|b| {
            b.crossing_edges.contains(&(3, 4))
        });
        assert!(bridge_found);
    }

    #[test]
    fn test_min_cut_prediction() {
        let g = create_path_graph(10);
        let mut analyzer = SpectralAnalyzer::new(g);
        analyzer.compute_laplacian_spectrum();

        let prediction = analyzer.predict_min_cut();
        assert!(prediction.predicted_cut > 0.0);
        assert!(prediction.lower_bound <= prediction.upper_bound);
        assert!(prediction.confidence > 0.0);
    }

    #[test]
    fn test_effective_resistance() {
        let g = create_path_graph(5);
        let mut analyzer = SpectralAnalyzer::new(g);
        analyzer.compute_laplacian_spectrum();

        // Effective resistance should increase with distance
        let r_01 = analyzer.effective_resistance(0, 1);
        let r_04 = analyzer.effective_resistance(0, 4);

        assert!(r_01 < r_04);
    }

    #[test]
    fn test_cycle_bipartite() {
        // Even cycle is bipartite
        let even_cycle = create_cycle_graph(6);
        let mut analyzer_even = SpectralAnalyzer::new(even_cycle);
        analyzer_even.compute_laplacian_spectrum();

        // Odd cycle is not bipartite
        let odd_cycle = create_cycle_graph(5);
        let mut analyzer_odd = SpectralAnalyzer::new(odd_cycle);
        analyzer_odd.compute_laplacian_spectrum();

        // Even cycle should be bipartite
        assert!(analyzer_even.is_bipartite());

        // Odd cycle should not be bipartite
        assert!(!analyzer_odd.is_bipartite());
    }
}
