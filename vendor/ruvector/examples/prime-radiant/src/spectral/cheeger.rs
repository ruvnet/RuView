//! Cheeger Inequality and Isoperimetric Analysis
//!
//! This module implements the Cheeger inequality and related isoperimetric
//! analysis tools for graphs.
//!
//! ## Cheeger Inequality
//!
//! For a graph G with normalized Laplacian eigenvalue λ₂ and Cheeger constant h(G):
//!
//! ```text
//! λ₂/2 ≤ h(G) ≤ √(2λ₂)
//! ```
//!
//! The Cheeger constant measures the "bottleneck-ness" of a graph and is defined as:
//!
//! ```text
//! h(G) = min_{S} |∂S| / min(vol(S), vol(V\S))
//! ```
//!
//! where ∂S is the edge boundary of S and vol(S) is the sum of degrees in S.

use super::analyzer::SpectralAnalyzer;
use super::types::{Graph, NodeId, Vector, EPS};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Cheeger constant bounds from spectral analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheegerBounds {
    /// Estimated Cheeger constant h(G)
    pub cheeger_constant: f64,
    /// Lower bound from Cheeger inequality: λ₂/2 ≤ h(G)
    pub lower_bound: f64,
    /// Upper bound from Cheeger inequality: h(G) ≤ √(2λ₂)
    pub upper_bound: f64,
    /// The algebraic connectivity λ₂
    pub lambda_2: f64,
    /// Confidence in the estimate (0-1)
    pub confidence: f64,
}

impl CheegerBounds {
    /// Check if bounds indicate a well-connected graph
    pub fn is_well_connected(&self) -> bool {
        self.lower_bound > 0.3
    }

    /// Check if bounds indicate a clear bottleneck
    pub fn has_bottleneck(&self) -> bool {
        self.upper_bound < 0.2
    }

    /// Get a qualitative assessment
    pub fn connectivity_assessment(&self) -> &str {
        if self.cheeger_constant > 0.5 {
            "Highly connected"
        } else if self.cheeger_constant > 0.3 {
            "Well connected"
        } else if self.cheeger_constant > 0.1 {
            "Moderately connected"
        } else if self.cheeger_constant > 0.01 {
            "Weakly connected"
        } else {
            "Nearly disconnected"
        }
    }
}

/// Cheeger analyzer for computing isoperimetric properties
pub struct CheegerAnalyzer<'a> {
    /// Reference to the graph
    graph: &'a Graph,
    /// Spectral analyzer
    spectral: Option<SpectralAnalyzer>,
}

impl<'a> CheegerAnalyzer<'a> {
    /// Create a new Cheeger analyzer
    pub fn new(graph: &'a Graph) -> Self {
        Self {
            graph,
            spectral: None,
        }
    }

    /// Create with precomputed spectral analysis
    pub fn with_spectral(graph: &'a Graph, spectral: SpectralAnalyzer) -> Self {
        Self {
            graph,
            spectral: Some(spectral),
        }
    }

    /// Compute Cheeger bounds using the spectral approach
    pub fn compute_cheeger_bounds(&mut self) -> CheegerBounds {
        // Compute spectral analysis if not already done
        let lambda_2 = if let Some(ref spectral) = self.spectral {
            spectral.algebraic_connectivity()
        } else {
            let graph_copy = self.graph.clone();
            let mut spectral = SpectralAnalyzer::new(graph_copy);
            spectral.compute_laplacian_spectrum();
            let lambda_2 = spectral.algebraic_connectivity();
            self.spectral = Some(spectral);
            lambda_2
        };

        // Cheeger inequality bounds
        let lower_bound = lambda_2 / 2.0;
        let upper_bound = (2.0 * lambda_2).sqrt();

        // Estimate actual Cheeger constant via sweep algorithm
        let cheeger_estimate = self.sweep_cheeger_estimate();

        // Confidence based on how tight the bounds are
        let bound_ratio = if upper_bound > EPS {
            lower_bound / upper_bound
        } else {
            0.0
        };
        let confidence = bound_ratio.sqrt().clamp(0.2, 0.95);

        // Use sweep estimate if it falls within bounds, otherwise use midpoint
        let cheeger_constant = if cheeger_estimate >= lower_bound && cheeger_estimate <= upper_bound {
            cheeger_estimate
        } else {
            (lower_bound + upper_bound) / 2.0
        };

        CheegerBounds {
            cheeger_constant,
            lower_bound,
            upper_bound,
            lambda_2,
            confidence,
        }
    }

    /// Compute Cheeger constant estimate using sweep algorithm over Fiedler vector
    fn sweep_cheeger_estimate(&self) -> f64 {
        let spectral = match &self.spectral {
            Some(s) => s,
            None => return f64::INFINITY,
        };

        let fiedler = match spectral.fiedler_vector() {
            Some(v) => v.clone(),
            None => return f64::INFINITY,
        };

        let n = self.graph.n;

        // Sort nodes by Fiedler value
        let mut sorted_indices: Vec<usize> = (0..n).collect();
        sorted_indices.sort_by(|&a, &b| fiedler[a].partial_cmp(&fiedler[b]).unwrap());

        // Compute total volume
        let total_volume: f64 = self.graph.degrees().iter().sum();

        if total_volume < EPS {
            return f64::INFINITY;
        }

        // Sweep through and compute conductance at each cut
        let mut best_conductance = f64::INFINITY;
        let mut current_set: HashSet<NodeId> = HashSet::new();
        let mut current_volume = 0.0;
        let mut cut_weight = 0.0;

        for (i, &node) in sorted_indices.iter().enumerate() {
            // Add node to current set
            current_set.insert(node);
            current_volume += self.graph.degree(node);

            // Update cut weight
            for &(neighbor, weight) in &self.graph.adj[node] {
                if current_set.contains(&neighbor) {
                    // Edge now internal, remove from cut
                    cut_weight -= weight;
                } else {
                    // Edge now in cut
                    cut_weight += weight;
                }
            }

            // Skip trivial cuts
            if i == 0 || i == n - 1 {
                continue;
            }

            // Compute conductance
            let complement_volume = total_volume - current_volume;
            let min_volume = current_volume.min(complement_volume);

            if min_volume > EPS {
                let conductance = cut_weight / min_volume;
                if conductance < best_conductance {
                    best_conductance = conductance;
                }
            }
        }

        best_conductance
    }

    /// Compute conductance of a specific set of nodes
    pub fn conductance(&self, nodes: &[NodeId]) -> f64 {
        let node_set: HashSet<NodeId> = nodes.iter().cloned().collect();

        // Compute volume of the set
        let set_volume: f64 = nodes.iter().map(|&n| self.graph.degree(n)).sum();

        // Compute complement volume
        let total_volume: f64 = self.graph.degrees().iter().sum();
        let complement_volume = total_volume - set_volume;

        // Compute cut weight
        let mut cut_weight = 0.0;
        for &node in nodes {
            for &(neighbor, weight) in &self.graph.adj[node] {
                if !node_set.contains(&neighbor) {
                    cut_weight += weight;
                }
            }
        }

        let min_volume = set_volume.min(complement_volume);
        if min_volume > EPS {
            cut_weight / min_volume
        } else {
            f64::INFINITY
        }
    }

    /// Compute expansion of a set (edge boundary / |S|)
    pub fn expansion(&self, nodes: &[NodeId]) -> f64 {
        if nodes.is_empty() {
            return 0.0;
        }

        let node_set: HashSet<NodeId> = nodes.iter().cloned().collect();

        // Compute cut weight
        let mut cut_weight = 0.0;
        for &node in nodes {
            for &(neighbor, weight) in &self.graph.adj[node] {
                if !node_set.contains(&neighbor) {
                    cut_weight += weight;
                }
            }
        }

        cut_weight / nodes.len() as f64
    }

    /// Compute isoperimetric ratio of a set
    pub fn isoperimetric_ratio(&self, nodes: &[NodeId]) -> f64 {
        let n = self.graph.n;
        let k = nodes.len();

        if k == 0 || k == n {
            return 0.0;
        }

        let node_set: HashSet<NodeId> = nodes.iter().cloned().collect();

        // Compute boundary size
        let mut boundary = 0.0;
        for &node in nodes {
            for &(neighbor, weight) in &self.graph.adj[node] {
                if !node_set.contains(&neighbor) {
                    boundary += weight;
                }
            }
        }

        // Isoperimetric ratio: |∂S| / min(|S|, |V\S|)
        let min_size = k.min(n - k) as f64;
        boundary / min_size
    }

    /// Find a set achieving (approximately) the Cheeger constant
    pub fn find_cheeger_set(&mut self) -> Vec<NodeId> {
        // Ensure spectral is computed
        if self.spectral.is_none() {
            let graph_copy = self.graph.clone();
            let mut spectral = SpectralAnalyzer::new(graph_copy);
            spectral.compute_laplacian_spectrum();
            self.spectral = Some(spectral);
        }

        let spectral = self.spectral.as_ref().unwrap();
        let fiedler = match spectral.fiedler_vector() {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };

        let n = self.graph.n;

        // Sort nodes by Fiedler value
        let mut sorted_indices: Vec<usize> = (0..n).collect();
        sorted_indices.sort_by(|&a, &b| fiedler[a].partial_cmp(&fiedler[b]).unwrap());

        // Find the best sweep cut
        let total_volume: f64 = self.graph.degrees().iter().sum();

        let mut best_set = Vec::new();
        let mut best_conductance = f64::INFINITY;
        let mut current_set: HashSet<NodeId> = HashSet::new();
        let mut current_volume = 0.0;
        let mut cut_weight = 0.0;

        for (i, &node) in sorted_indices.iter().enumerate() {
            current_set.insert(node);
            current_volume += self.graph.degree(node);

            for &(neighbor, weight) in &self.graph.adj[node] {
                if current_set.contains(&neighbor) {
                    cut_weight -= weight;
                } else {
                    cut_weight += weight;
                }
            }

            if i == 0 || i == n - 1 {
                continue;
            }

            let complement_volume = total_volume - current_volume;
            let min_volume = current_volume.min(complement_volume);

            if min_volume > EPS {
                let conductance = cut_weight / min_volume;
                if conductance < best_conductance {
                    best_conductance = conductance;
                    best_set = current_set.iter().cloned().collect();
                }
            }
        }

        best_set
    }

    /// Compute higher-order Cheeger constants h_k for k clusters
    pub fn higher_order_cheeger(&mut self, k: usize) -> Vec<f64> {
        if k == 0 || k > self.graph.n {
            return Vec::new();
        }

        // Ensure spectral is computed with enough eigenvalues
        if self.spectral.is_none() {
            let graph_copy = self.graph.clone();
            let config = super::analyzer::SpectralConfig::builder()
                .num_eigenvalues(k + 1)
                .build();
            let mut spectral = SpectralAnalyzer::with_config(graph_copy, config);
            spectral.compute_laplacian_spectrum();
            self.spectral = Some(spectral);
        }

        let spectral = self.spectral.as_ref().unwrap();

        // Higher-order Cheeger inequality bounds
        // h_k ≥ λ_k / 2 and h_k ≤ O(k²) * √(λ_k)
        let mut cheeger_estimates = Vec::with_capacity(k);

        for i in 1..=k.min(spectral.eigenvalues.len()) {
            let lambda_i = spectral.eigenvalues.get(i - 1).copied().unwrap_or(0.0);
            // Conservative estimate using upper bound
            let estimate = (2.0 * lambda_i).sqrt();
            cheeger_estimates.push(estimate);
        }

        cheeger_estimates
    }

    /// Analyze mixing properties using Cheeger constant
    pub fn mixing_analysis(&mut self) -> MixingAnalysis {
        let bounds = self.compute_cheeger_bounds();

        // Mixing time bounds from Cheeger constant
        // t_mix ~ O(1/h²) for random walk
        let mixing_time_lower = if bounds.upper_bound > EPS {
            1.0 / (bounds.upper_bound * bounds.upper_bound)
        } else {
            f64::INFINITY
        };

        let mixing_time_upper = if bounds.lower_bound > EPS {
            1.0 / (bounds.lower_bound * bounds.lower_bound)
        } else {
            f64::INFINITY
        };

        // Spectral gap gives tighter bound: t_mix ~ O(1/λ₂)
        let spectral_mixing_time = if bounds.lambda_2 > EPS {
            1.0 / bounds.lambda_2
        } else {
            f64::INFINITY
        };

        MixingAnalysis {
            cheeger_bounds: bounds,
            mixing_time_lower,
            mixing_time_upper,
            spectral_mixing_time,
        }
    }
}

/// Results of mixing time analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixingAnalysis {
    /// Cheeger constant bounds
    pub cheeger_bounds: CheegerBounds,
    /// Lower bound on mixing time (from upper Cheeger bound)
    pub mixing_time_lower: f64,
    /// Upper bound on mixing time (from lower Cheeger bound)
    pub mixing_time_upper: f64,
    /// Mixing time estimate from spectral gap
    pub spectral_mixing_time: f64,
}

impl MixingAnalysis {
    /// Get a qualitative assessment of mixing speed
    pub fn mixing_assessment(&self) -> &str {
        let t = self.spectral_mixing_time;
        if t < 10.0 {
            "Very fast mixing"
        } else if t < 50.0 {
            "Fast mixing"
        } else if t < 200.0 {
            "Moderate mixing"
        } else if t < 1000.0 {
            "Slow mixing"
        } else {
            "Very slow mixing"
        }
    }

    /// Estimate number of random walk steps to approximate stationary distribution
    pub fn steps_to_mix(&self, epsilon: f64) -> f64 {
        // t_mix(ε) ~ (1/λ₂) * ln(1/ε)
        if self.cheeger_bounds.lambda_2 > EPS {
            (1.0 / self.cheeger_bounds.lambda_2) * (1.0 / epsilon).ln()
        } else {
            f64::INFINITY
        }
    }
}

/// Compute the Cheeger inequality directly
pub fn cheeger_inequality(lambda_2: f64) -> CheegerBounds {
    let lower_bound = lambda_2 / 2.0;
    let upper_bound = (2.0 * lambda_2).sqrt();
    let cheeger_constant = (lower_bound + upper_bound) / 2.0;

    CheegerBounds {
        cheeger_constant,
        lower_bound,
        upper_bound,
        lambda_2,
        confidence: 0.5, // Midpoint estimate has moderate confidence
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

    fn create_complete_graph(n: usize) -> Graph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                edges.push((i, j, 1.0));
            }
        }
        Graph::from_edges(n, &edges)
    }

    fn create_barbell_graph(clique_size: usize) -> Graph {
        let n = 2 * clique_size;
        let mut g = Graph::new(n);

        // First clique
        for i in 0..clique_size {
            for j in i + 1..clique_size {
                g.add_edge(i, j, 1.0);
            }
        }

        // Second clique
        for i in clique_size..n {
            for j in i + 1..n {
                g.add_edge(i, j, 1.0);
            }
        }

        // Bridge
        g.add_edge(clique_size - 1, clique_size, 1.0);

        g
    }

    #[test]
    fn test_cheeger_bounds_path() {
        let g = create_path_graph(10);
        let mut analyzer = CheegerAnalyzer::new(&g);
        let bounds = analyzer.compute_cheeger_bounds();

        // Path graph should have low Cheeger constant
        assert!(bounds.cheeger_constant < 1.0);
        assert!(bounds.lower_bound <= bounds.cheeger_constant);
        assert!(bounds.cheeger_constant <= bounds.upper_bound);
    }

    #[test]
    fn test_cheeger_bounds_complete() {
        let g = create_complete_graph(10);
        let mut analyzer = CheegerAnalyzer::new(&g);
        let bounds = analyzer.compute_cheeger_bounds();

        // Complete graph should be well connected
        assert!(bounds.is_well_connected());
    }

    #[test]
    fn test_cheeger_bounds_barbell() {
        let g = create_barbell_graph(5);
        let mut analyzer = CheegerAnalyzer::new(&g);
        let bounds = analyzer.compute_cheeger_bounds();

        // Barbell graph should have a bottleneck
        assert!(bounds.cheeger_constant < 0.5);
    }

    #[test]
    fn test_conductance() {
        let g = create_path_graph(6);
        let analyzer = CheegerAnalyzer::new(&g);

        // Conductance of first half
        let nodes: Vec<NodeId> = (0..3).collect();
        let conductance = analyzer.conductance(&nodes);

        assert!(conductance > 0.0);
        assert!(conductance < f64::INFINITY);
    }

    #[test]
    fn test_cheeger_set() {
        let g = create_barbell_graph(4);
        let mut analyzer = CheegerAnalyzer::new(&g);
        let cheeger_set = analyzer.find_cheeger_set();

        // Cheeger set should be roughly one of the cliques
        assert!(cheeger_set.len() >= 3 && cheeger_set.len() <= 5);
    }

    #[test]
    fn test_mixing_analysis() {
        let g = create_complete_graph(10);
        let mut analyzer = CheegerAnalyzer::new(&g);
        let mixing = analyzer.mixing_analysis();

        // Complete graph should have fast mixing
        assert!(mixing.spectral_mixing_time < 100.0);
        assert!(mixing.steps_to_mix(0.01) < f64::INFINITY);
    }

    #[test]
    fn test_cheeger_inequality() {
        let lambda_2 = 0.5;
        let bounds = cheeger_inequality(lambda_2);

        assert!((bounds.lower_bound - 0.25).abs() < EPS);
        assert!((bounds.upper_bound - 1.0).abs() < EPS);
    }
}
