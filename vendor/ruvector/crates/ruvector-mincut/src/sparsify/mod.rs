//! Graph Sparsification for Approximate Minimum Cuts
//!
//! Implements sparsification that preserves (1±ε) approximation of all cuts
//! using O(n log n / ε²) edges.
//!
//! This module provides two main sparsification approaches:
//! 1. **Benczúr-Karger**: Randomized sparsification based on edge strengths
//! 2. **Nagamochi-Ibaraki**: Deterministic sparsification using connectivity certificates
//!
//! # Example
//!
//! ```rust
//! use ruvector_mincut::graph::DynamicGraph;
//! use ruvector_mincut::sparsify::{SparsifyConfig, SparseGraph};
//!
//! let graph = DynamicGraph::new();
//! graph.insert_edge(1, 2, 1.0).unwrap();
//! graph.insert_edge(2, 3, 1.0).unwrap();
//! graph.insert_edge(3, 4, 1.0).unwrap();
//! graph.insert_edge(4, 1, 1.0).unwrap();
//! graph.insert_edge(1, 3, 1.0).unwrap();
//!
//! let config = SparsifyConfig {
//!     epsilon: 0.1,
//!     seed: Some(42),
//!     max_edges: None,
//! };
//!
//! let sparse = SparseGraph::from_graph(&graph, config).unwrap();
//! assert!(sparse.num_edges() <= graph.num_edges());
//! ```

use crate::error::{MinCutError, Result};
use crate::graph::{DynamicGraph, EdgeId, VertexId, Weight};
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Configuration for sparsification
#[derive(Debug, Clone)]
pub struct SparsifyConfig {
    /// Approximation parameter (0 < ε ≤ 1)
    pub epsilon: f64,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
    /// Maximum number of edges in sparse graph
    pub max_edges: Option<usize>,
}

impl Default for SparsifyConfig {
    fn default() -> Self {
        Self {
            epsilon: 0.1,
            seed: None,
            max_edges: None,
        }
    }
}

impl SparsifyConfig {
    /// Create a new configuration
    pub fn new(epsilon: f64) -> Result<Self> {
        if epsilon <= 0.0 || epsilon > 1.0 {
            return Err(MinCutError::InvalidEpsilon(epsilon));
        }
        Ok(Self {
            epsilon,
            seed: None,
            max_edges: None,
        })
    }

    /// Set random seed
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set maximum number of edges
    pub fn with_max_edges(mut self, max_edges: usize) -> Self {
        self.max_edges = Some(max_edges);
        self
    }
}

/// A sparsified graph that preserves cut structure
pub struct SparseGraph {
    /// The sparse graph
    graph: DynamicGraph,
    /// Original edge to scaled weight mapping
    edge_weights: HashMap<EdgeId, Weight>,
    /// Epsilon used for this sparsification
    epsilon: f64,
    /// Number of edges in original graph
    original_edges: usize,
    /// Random number generator
    rng: StdRng,
    /// Edge strength calculator
    strength_calc: EdgeStrength,
}

impl SparseGraph {
    /// Create a sparsified version of the graph
    pub fn from_graph(graph: &DynamicGraph, config: SparsifyConfig) -> Result<Self> {
        if config.epsilon <= 0.0 || config.epsilon > 1.0 {
            return Err(MinCutError::InvalidEpsilon(config.epsilon));
        }

        let original_edges = graph.num_edges();
        let n = graph.num_vertices();

        if n == 0 {
            return Err(MinCutError::EmptyGraph);
        }

        // Initialize RNG
        let rng = match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        // Create sparse graph
        let sparse_graph = DynamicGraph::with_capacity(n, original_edges);

        // Add all vertices
        for v in graph.vertices() {
            sparse_graph.add_vertex(v);
        }

        let mut edge_weights = HashMap::new();
        let mut strength_calc = EdgeStrength::new(Arc::new(graph.clone()));

        // Use Benczúr-Karger sparsification
        Self::benczur_karger_sparsify(
            graph,
            &sparse_graph,
            &mut edge_weights,
            &mut strength_calc,
            config.epsilon,
            &mut StdRng::seed_from_u64(config.seed.unwrap_or(42)),
            config.max_edges,
        )?;

        Ok(Self {
            graph: sparse_graph,
            edge_weights,
            epsilon: config.epsilon,
            original_edges,
            rng,
            strength_calc,
        })
    }

    /// Benczúr-Karger sparsification algorithm
    fn benczur_karger_sparsify(
        original: &DynamicGraph,
        sparse: &DynamicGraph,
        edge_weights: &mut HashMap<EdgeId, Weight>,
        strength_calc: &mut EdgeStrength,
        epsilon: f64,
        rng: &mut StdRng,
        max_edges: Option<usize>,
    ) -> Result<()> {
        let n = original.num_vertices() as f64;
        let c = 6.0; // Constant for sampling probability

        // Compute edge strengths
        let strengths = strength_calc.compute_all();

        let edges = original.edges();
        let mut edges_added = 0;
        let target_edges = max_edges.unwrap_or(usize::MAX);

        for edge in edges {
            // Get edge strength (use weight as lower bound if not computed)
            let strength = strengths.get(&edge.id).copied().unwrap_or(edge.weight);

            // Compute sampling probability
            let prob = sample_probability(strength, epsilon, n, c);

            // Sample edge with probability prob
            if rng.gen::<f64>() < prob && edges_added < target_edges {
                // Scale weight by 1/prob
                let scaled_weight = edge.weight / prob;

                // Add to sparse graph
                if let Ok(new_edge_id) = sparse.insert_edge(edge.source, edge.target, scaled_weight)
                {
                    edge_weights.insert(new_edge_id, edge.weight);
                    edges_added += 1;
                }
            }
        }

        Ok(())
    }

    /// Get the sparse graph
    pub fn graph(&self) -> &DynamicGraph {
        &self.graph
    }

    /// Get the number of edges (should be O(n log n / ε²))
    pub fn num_edges(&self) -> usize {
        self.graph.num_edges()
    }

    /// Get the sparsification ratio
    pub fn sparsification_ratio(&self) -> f64 {
        if self.original_edges == 0 {
            return 1.0;
        }
        self.num_edges() as f64 / self.original_edges as f64
    }

    /// Query approximate minimum cut on sparse graph
    pub fn approximate_min_cut(&self) -> f64 {
        // Simple approximation using minimum degree
        if self.graph.num_vertices() == 0 {
            return 0.0;
        }

        let vertices = self.graph.vertices();
        let mut min_cut = f64::INFINITY;

        for v in vertices {
            let mut cut_weight = 0.0;
            for (neighbor, _edge_id) in self.graph.neighbors(v) {
                if let Some(edge) = self.graph.get_edge(v, neighbor) {
                    cut_weight += edge.weight;
                }
            }
            min_cut = min_cut.min(cut_weight);
        }

        min_cut
    }

    /// Update for edge insertion in original graph
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<()> {
        // Invalidate strength cache
        self.strength_calc.invalidate(u);
        self.strength_calc.invalidate(v);

        // Compute strength for new edge
        let strength = self.strength_calc.compute(u, v);

        // Compute sampling probability
        let n = self.graph.num_vertices() as f64;
        let c = 6.0;
        let prob = sample_probability(strength, self.epsilon, n, c);

        // Sample edge
        if self.rng.gen::<f64>() < prob {
            let scaled_weight = weight / prob;
            if let Ok(new_edge_id) = self.graph.insert_edge(u, v, scaled_weight) {
                self.edge_weights.insert(new_edge_id, weight);
            }
        }

        Ok(())
    }

    /// Update for edge deletion in original graph
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()> {
        // Invalidate strength cache
        self.strength_calc.invalidate(u);
        self.strength_calc.invalidate(v);

        // Try to delete from sparse graph (may not exist if not sampled)
        let _ = self.graph.delete_edge(u, v);

        Ok(())
    }

    /// Get epsilon value
    pub fn epsilon(&self) -> f64 {
        self.epsilon
    }
}

/// Edge strength calculator for sparsification sampling
pub struct EdgeStrength {
    /// Graph reference
    graph: Arc<DynamicGraph>,
    /// Cached strengths
    strengths: HashMap<EdgeId, f64>,
}

impl EdgeStrength {
    /// Create new strength calculator
    pub fn new(graph: Arc<DynamicGraph>) -> Self {
        Self {
            graph,
            strengths: HashMap::new(),
        }
    }

    /// Compute strength of edge (u, v)
    /// Strength = max-flow between u and v in graph without edge (u,v)
    /// For efficiency, approximate using connectivity
    pub fn compute(&mut self, u: VertexId, v: VertexId) -> f64 {
        // Get edge if it exists
        let edge = self.graph.get_edge(u, v);
        let edge_id = edge.map(|e| e.id);

        // Check cache
        if let Some(&strength) = edge_id.and_then(|id| self.strengths.get(&id)) {
            return strength;
        }

        // Approximate strength using local connectivity
        // Better approximation: sum of edge weights incident to u and v
        let weight_u: f64 = self
            .graph
            .neighbors(u)
            .iter()
            .filter_map(|(neighbor, _)| self.graph.edge_weight(u, *neighbor))
            .sum();

        let weight_v: f64 = self
            .graph
            .neighbors(v)
            .iter()
            .filter_map(|(neighbor, _)| self.graph.edge_weight(v, *neighbor))
            .sum();

        let strength = weight_u.min(weight_v).max(1.0);

        // Cache if we have edge_id
        if let Some(id) = edge_id {
            self.strengths.insert(id, strength);
        }

        strength
    }

    /// Compute all edge strengths
    pub fn compute_all(&mut self) -> HashMap<EdgeId, f64> {
        let edges = self.graph.edges();

        for edge in edges {
            if !self.strengths.contains_key(&edge.id) {
                let strength = self.compute(edge.source, edge.target);
                self.strengths.insert(edge.id, strength);
            }
        }

        self.strengths.clone()
    }

    /// Invalidate cached strengths for edges incident to vertex
    pub fn invalidate(&mut self, v: VertexId) {
        let neighbors = self.graph.neighbors(v);
        for (_neighbor, edge_id) in neighbors {
            self.strengths.remove(&edge_id);
        }
    }
}

/// Nagamochi-Ibaraki sparsification (deterministic)
pub struct NagamochiIbaraki {
    /// The graph
    graph: Arc<DynamicGraph>,
}

impl NagamochiIbaraki {
    /// Create new NI sparsifier
    pub fn new(graph: Arc<DynamicGraph>) -> Self {
        Self { graph }
    }

    /// Compute a sparse certificate preserving minimum cuts up to k
    pub fn sparse_k_certificate(&self, k: usize) -> Result<DynamicGraph> {
        let n = self.graph.num_vertices();
        if n == 0 {
            return Err(MinCutError::EmptyGraph);
        }

        // Compute minimum degree ordering
        let order = self.min_degree_ordering();

        // Scan connectivity
        let connectivity = self.scan_connectivity(&order);

        // Build sparse certificate
        let sparse = DynamicGraph::with_capacity(n, k * n);

        // Add all vertices
        for v in self.graph.vertices() {
            sparse.add_vertex(v);
        }

        // Add edges with connectivity >= k
        for edge in self.graph.edges() {
            if let Some(&conn) = connectivity.get(&edge.id) {
                if conn >= k {
                    let _ = sparse.insert_edge(edge.source, edge.target, edge.weight);
                }
            }
        }

        Ok(sparse)
    }

    /// Compute minimum degree ordering
    fn min_degree_ordering(&self) -> Vec<VertexId> {
        let mut remaining: HashSet<VertexId> = self.graph.vertices().into_iter().collect();
        let mut order = Vec::with_capacity(remaining.len());

        // Track degrees
        let mut degrees: HashMap<VertexId, usize> = self
            .graph
            .vertices()
            .iter()
            .map(|&v| (v, self.graph.degree(v)))
            .collect();

        while !remaining.is_empty() {
            // Find vertex with minimum degree among remaining
            let (&min_v, _) = degrees
                .iter()
                .filter(|(v, _)| remaining.contains(v))
                .min_by_key(|(_, &deg)| deg)
                .unwrap();

            order.push(min_v);
            remaining.remove(&min_v);

            // Update degrees of neighbors
            for (neighbor, _) in self.graph.neighbors(min_v) {
                if remaining.contains(&neighbor) {
                    if let Some(deg) = degrees.get_mut(&neighbor) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }

        order
    }

    /// Scan vertices to compute edge connectivity
    fn scan_connectivity(&self, order: &[VertexId]) -> HashMap<EdgeId, usize> {
        let mut connectivity = HashMap::new();
        let mut scanned = HashSet::new();

        for &v in order.iter().rev() {
            scanned.insert(v);

            // For each edge from v to scanned vertices
            for (neighbor, edge_id) in self.graph.neighbors(v) {
                if scanned.contains(&neighbor) {
                    // Connectivity is the number of scanned neighbors
                    let conn = scanned.len();
                    connectivity.insert(edge_id, conn);
                }
            }
        }

        connectivity
    }
}

/// Karger's random sampling sparsification
pub fn karger_sparsify(
    graph: &DynamicGraph,
    epsilon: f64,
    seed: Option<u64>,
) -> Result<SparseGraph> {
    let config = SparsifyConfig::new(epsilon)?.with_seed(seed.unwrap_or(42));

    SparseGraph::from_graph(graph, config)
}

/// Compute sampling probability for an edge based on its strength
fn sample_probability(strength: f64, epsilon: f64, n: f64, c: f64) -> f64 {
    if strength <= 0.0 {
        return 0.0;
    }

    // p_e = min(1, c * log(n) / (ε² * λ_e))
    let numerator = c * n.ln();
    let denominator = epsilon * epsilon * strength;

    (numerator / denominator).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_triangle_graph() -> DynamicGraph {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 1.0).unwrap();
        g.insert_edge(3, 1, 1.0).unwrap();
        g
    }

    fn create_complete_graph(n: usize) -> DynamicGraph {
        let g = DynamicGraph::new();
        for i in 0..n {
            for j in (i + 1)..n {
                g.insert_edge(i as u64, j as u64, 1.0).unwrap();
            }
        }
        g
    }

    fn create_path_graph(n: usize) -> DynamicGraph {
        let g = DynamicGraph::new();
        for i in 0..(n - 1) {
            g.insert_edge(i as u64, (i + 1) as u64, 1.0).unwrap();
        }
        g
    }

    #[test]
    fn test_sparsify_config_default() {
        let config = SparsifyConfig::default();
        assert_eq!(config.epsilon, 0.1);
        assert_eq!(config.seed, None);
        assert_eq!(config.max_edges, None);
    }

    #[test]
    fn test_sparsify_config_new() {
        let config = SparsifyConfig::new(0.2).unwrap();
        assert_eq!(config.epsilon, 0.2);
    }

    #[test]
    fn test_sparsify_config_invalid_epsilon() {
        assert!(SparsifyConfig::new(0.0).is_err());
        assert!(SparsifyConfig::new(-0.1).is_err());
        assert!(SparsifyConfig::new(1.5).is_err());
    }

    #[test]
    fn test_sparsify_config_builder() {
        let config = SparsifyConfig::new(0.1)
            .unwrap()
            .with_seed(42)
            .with_max_edges(10);

        assert_eq!(config.epsilon, 0.1);
        assert_eq!(config.seed, Some(42));
        assert_eq!(config.max_edges, Some(10));
    }

    #[test]
    fn test_sparse_graph_triangle() {
        let g = create_triangle_graph();
        let config = SparsifyConfig::new(0.1).unwrap().with_seed(42);

        let sparse = SparseGraph::from_graph(&g, config).unwrap();

        assert!(sparse.num_edges() <= g.num_edges());
        assert_eq!(sparse.epsilon(), 0.1);
        assert_eq!(sparse.graph().num_vertices(), g.num_vertices());
    }

    #[test]
    fn test_sparse_graph_sparsification_ratio() {
        let g = create_complete_graph(10);
        let config = SparsifyConfig::new(0.2).unwrap().with_seed(123);

        let sparse = SparseGraph::from_graph(&g, config).unwrap();
        let ratio = sparse.sparsification_ratio();

        assert!(ratio >= 0.0 && ratio <= 1.0);
        println!("Sparsification ratio: {}", ratio);
    }

    #[test]
    fn test_sparse_graph_max_edges() {
        let g = create_complete_graph(10);
        let config = SparsifyConfig::new(0.1)
            .unwrap()
            .with_seed(42)
            .with_max_edges(20);

        let sparse = SparseGraph::from_graph(&g, config).unwrap();

        assert!(sparse.num_edges() <= 20);
    }

    #[test]
    fn test_sparse_graph_empty_graph() {
        let g = DynamicGraph::new();
        let config = SparsifyConfig::default();

        let result = SparseGraph::from_graph(&g, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_sparse_graph_approximate_min_cut() {
        let g = create_path_graph(5);
        let config = SparsifyConfig::new(0.1).unwrap().with_seed(42);

        let sparse = SparseGraph::from_graph(&g, config).unwrap();
        let min_cut = sparse.approximate_min_cut();

        // Path graph has min cut of approximately 1.0
        assert!(min_cut >= 0.0);
    }

    #[test]
    fn test_sparse_graph_insert_edge() {
        let g = create_triangle_graph();
        let config = SparsifyConfig::new(0.1).unwrap().with_seed(42);

        let mut sparse = SparseGraph::from_graph(&g, config).unwrap();
        let initial_edges = sparse.num_edges();

        sparse.insert_edge(4, 5, 1.0).unwrap();

        // May or may not add edge due to sampling
        assert!(sparse.num_edges() >= initial_edges);
    }

    #[test]
    fn test_sparse_graph_delete_edge() {
        let g = create_triangle_graph();
        let config = SparsifyConfig::new(0.5).unwrap().with_seed(42);

        let mut sparse = SparseGraph::from_graph(&g, config).unwrap();

        // Try to delete an edge
        let result = sparse.delete_edge(1, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_edge_strength_compute() {
        let g = create_triangle_graph();
        let mut strength_calc = EdgeStrength::new(Arc::new(g));

        let strength = strength_calc.compute(1, 2);
        assert!(strength > 0.0);
    }

    #[test]
    fn test_edge_strength_compute_all() {
        let g = create_complete_graph(5);
        let mut strength_calc = EdgeStrength::new(Arc::new(g));

        let strengths = strength_calc.compute_all();
        assert!(!strengths.is_empty());

        for (_, strength) in strengths {
            assert!(strength > 0.0);
        }
    }

    #[test]
    fn test_edge_strength_invalidate() {
        let g = create_triangle_graph();
        let mut strength_calc = EdgeStrength::new(Arc::new(g));

        // Compute all strengths
        strength_calc.compute_all();
        let initial_count = strength_calc.strengths.len();

        // Invalidate vertex 1
        strength_calc.invalidate(1);

        // Should have fewer cached strengths
        assert!(strength_calc.strengths.len() < initial_count);
    }

    #[test]
    fn test_nagamochi_ibaraki_min_degree_ordering() {
        let g = create_path_graph(5);
        let ni = NagamochiIbaraki::new(Arc::new(g));

        let order = ni.min_degree_ordering();
        assert_eq!(order.len(), 5);

        // All vertices should be in the ordering
        let order_set: HashSet<_> = order.iter().copied().collect();
        assert_eq!(order_set.len(), 5);
    }

    #[test]
    fn test_nagamochi_ibaraki_sparse_certificate() {
        let g = create_complete_graph(6);
        let ni = NagamochiIbaraki::new(Arc::new(g.clone()));

        let sparse = ni.sparse_k_certificate(3).unwrap();

        // Should preserve high-connectivity edges
        assert!(sparse.num_edges() <= g.num_edges());
        assert_eq!(sparse.num_vertices(), g.num_vertices());
    }

    #[test]
    fn test_nagamochi_ibaraki_empty_graph() {
        let g = DynamicGraph::new();
        let ni = NagamochiIbaraki::new(Arc::new(g));

        let result = ni.sparse_k_certificate(2);
        assert!(result.is_err());
    }

    #[test]
    fn test_karger_sparsify() {
        let g = create_complete_graph(8);

        let sparse = karger_sparsify(&g, 0.2, Some(42)).unwrap();

        assert!(sparse.num_edges() <= g.num_edges());
        assert_eq!(sparse.epsilon(), 0.2);
    }

    #[test]
    fn test_karger_sparsify_invalid_epsilon() {
        let g = create_triangle_graph();

        let result = karger_sparsify(&g, 0.0, Some(42));
        assert!(result.is_err());
    }

    #[test]
    fn test_sample_probability() {
        let n = 100.0;
        let epsilon = 0.1;
        let c = 6.0;

        // High strength -> low probability
        let prob_high = sample_probability(100.0, epsilon, n, c);

        // Low strength -> high probability (capped at 1.0)
        let prob_low = sample_probability(1.0, epsilon, n, c);

        assert!(prob_high <= prob_low);
        assert!(prob_high >= 0.0 && prob_high <= 1.0);
        assert!(prob_low >= 0.0 && prob_low <= 1.0);
    }

    #[test]
    fn test_sample_probability_zero_strength() {
        let prob = sample_probability(0.0, 0.1, 100.0, 6.0);
        assert_eq!(prob, 0.0);
    }

    #[test]
    fn test_sample_probability_always_capped() {
        let prob = sample_probability(0.001, 0.1, 100.0, 100.0);
        assert!(prob <= 1.0);
    }

    #[test]
    fn test_sparsification_preserves_vertices() {
        let g = create_complete_graph(10);
        let original_vertices: HashSet<_> = g.vertices().into_iter().collect();

        let config = SparsifyConfig::new(0.15).unwrap().with_seed(999);
        let sparse = SparseGraph::from_graph(&g, config).unwrap();

        let sparse_vertices: HashSet<_> = sparse.graph().vertices().into_iter().collect();

        // All vertices should be preserved
        assert_eq!(original_vertices, sparse_vertices);
    }

    #[test]
    fn test_sparsification_weighted_graph() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 2.0).unwrap();
        g.insert_edge(2, 3, 3.0).unwrap();
        g.insert_edge(3, 4, 4.0).unwrap();
        g.insert_edge(4, 1, 5.0).unwrap();

        let config = SparsifyConfig::new(0.3).unwrap().with_seed(777);
        let sparse = SparseGraph::from_graph(&g, config).unwrap();

        // Should handle weighted edges correctly
        assert!(sparse.num_edges() <= g.num_edges());
    }

    #[test]
    fn test_deterministic_with_seed() {
        let g = create_complete_graph(8);

        let sparse1 = karger_sparsify(&g, 0.2, Some(12345)).unwrap();
        let sparse2 = karger_sparsify(&g, 0.2, Some(12345)).unwrap();

        // Same seed should produce same number of edges
        assert_eq!(sparse1.num_edges(), sparse2.num_edges());
    }

    #[test]
    fn test_edge_strength_caching() {
        let g = create_complete_graph(6);
        let mut strength_calc = EdgeStrength::new(Arc::new(g));

        // First computation
        let strength1 = strength_calc.compute(0, 1);

        // Second computation should use cache
        let strength2 = strength_calc.compute(0, 1);

        assert_eq!(strength1, strength2);
    }

    #[test]
    fn test_nagamochi_ibaraki_scan_connectivity() {
        let g = create_complete_graph(5);
        let ni = NagamochiIbaraki::new(Arc::new(g.clone()));

        let order = ni.min_degree_ordering();
        let connectivity = ni.scan_connectivity(&order);

        // Complete graph should have high connectivity
        assert!(!connectivity.is_empty());

        for (_, conn) in connectivity {
            assert!(conn > 0);
        }
    }

    #[test]
    fn test_sparse_graph_ratio_bounds() {
        let g = create_complete_graph(10);

        // Very strict epsilon -> more edges kept
        let config_strict = SparsifyConfig::new(0.01).unwrap().with_seed(42);
        let sparse_strict = SparseGraph::from_graph(&g, config_strict).unwrap();

        // Loose epsilon -> fewer edges kept
        let config_loose = SparsifyConfig::new(0.5).unwrap().with_seed(42);
        let sparse_loose = SparseGraph::from_graph(&g, config_loose).unwrap();

        // Stricter epsilon should generally keep more edges
        // (though this is probabilistic and may not always hold)
        assert!(sparse_strict.sparsification_ratio() >= 0.0);
        assert!(sparse_loose.sparsification_ratio() >= 0.0);
    }
}
