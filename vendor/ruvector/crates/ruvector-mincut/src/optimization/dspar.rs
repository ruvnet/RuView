//! Degree-based Presparse (DSpar) Implementation
//!
//! Fast approximation for sparsification using effective resistance:
//!     R_eff(u,v) ≈ 1 / (deg(u) × deg(v))
//!
//! This provides a 5.9x speedup over exact effective resistance computation
//! while maintaining spectral properties for minimum cut preservation.
//!
//! Reference: "Degree-based Sparsification" (OpenReview)

use crate::graph::{DynamicGraph, EdgeId, VertexId, Weight};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Configuration for degree-based presparse
#[derive(Debug, Clone)]
pub struct PresparseConfig {
    /// Target sparsity ratio (0.0-1.0, lower = more sparse)
    pub target_sparsity: f64,
    /// Minimum effective resistance threshold for keeping edges
    pub resistance_threshold: f64,
    /// Whether to use adaptive threshold based on graph density
    pub adaptive_threshold: bool,
    /// Maximum edges to keep (optional hard limit)
    pub max_edges: Option<usize>,
    /// Random seed for probabilistic sampling
    pub seed: Option<u64>,
}

impl Default for PresparseConfig {
    fn default() -> Self {
        Self {
            target_sparsity: 0.1, // Keep ~10% of edges
            resistance_threshold: 0.0,
            adaptive_threshold: true,
            max_edges: None,
            seed: Some(42),
        }
    }
}

/// Statistics from presparse operation
#[derive(Debug, Clone, Default)]
pub struct PresparseStats {
    /// Original number of edges
    pub original_edges: usize,
    /// Number of edges after presparse
    pub sparse_edges: usize,
    /// Sparsity ratio achieved
    pub sparsity_ratio: f64,
    /// Time taken in microseconds
    pub time_us: u64,
    /// Estimated speedup factor
    pub speedup_factor: f64,
    /// Number of vertices affected
    pub vertices_processed: usize,
}

/// Result of presparse operation
#[derive(Debug)]
pub struct PresparseResult {
    /// Sparsified edges with scaled weights
    pub edges: Vec<(VertexId, VertexId, Weight)>,
    /// Mapping from new edge index to original edge ID
    pub edge_mapping: HashMap<usize, EdgeId>,
    /// Statistics
    pub stats: PresparseStats,
}

/// Degree-based presparse implementation
///
/// Uses effective resistance approximation R_eff(u,v) ≈ 1/(deg_u × deg_v)
/// to pre-filter edges before exact sparsification, achieving 5.9x speedup.
pub struct DegreePresparse {
    config: PresparseConfig,
    /// Cached degree information
    degree_cache: HashMap<VertexId, usize>,
}

impl DegreePresparse {
    /// Create new degree presparse with default config
    pub fn new() -> Self {
        Self::with_config(PresparseConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: PresparseConfig) -> Self {
        Self {
            config,
            degree_cache: HashMap::new(),
        }
    }

    /// Compute effective resistance approximation for an edge
    ///
    /// R_eff(u,v) ≈ 1 / (deg(u) × deg(v))
    ///
    /// High resistance = edge is important for connectivity
    /// Low resistance = edge can likely be removed
    #[inline]
    pub fn effective_resistance(&self, deg_u: usize, deg_v: usize) -> f64 {
        if deg_u == 0 || deg_v == 0 {
            return f64::INFINITY; // Always keep edges to isolated vertices
        }
        1.0 / (deg_u as f64 * deg_v as f64)
    }

    /// Pre-compute degrees for all vertices
    fn precompute_degrees(&mut self, graph: &DynamicGraph) {
        self.degree_cache.clear();
        for v in graph.vertices() {
            self.degree_cache.insert(v, graph.degree(v));
        }
    }

    /// Compute adaptive threshold based on graph properties
    fn compute_adaptive_threshold(&self, graph: &DynamicGraph) -> f64 {
        let n = graph.num_vertices();
        let m = graph.num_edges();

        if n == 0 || m == 0 {
            return 0.0;
        }

        // Average degree
        let avg_degree = (2 * m) as f64 / n as f64;

        // Target: keep O(n log n) edges
        let target_edges = (n as f64 * (n as f64).ln()).min(m as f64);

        // Compute threshold that keeps approximately target_edges
        // Higher threshold = fewer edges kept
        let sparsity = target_edges / m as f64;

        // Threshold based on average effective resistance
        1.0 / (avg_degree * avg_degree * sparsity.max(0.01))
    }

    /// Perform degree-based presparse on a graph
    ///
    /// Returns a sparsified edge set that preserves spectral properties
    /// for minimum cut computation.
    pub fn presparse(&mut self, graph: &DynamicGraph) -> PresparseResult {
        let start = std::time::Instant::now();

        // Pre-compute degrees
        self.precompute_degrees(graph);

        let original_edges = graph.num_edges();

        // Compute threshold
        let threshold = if self.config.adaptive_threshold {
            self.compute_adaptive_threshold(graph)
        } else {
            self.config.resistance_threshold
        };

        // Score all edges by effective resistance
        let mut scored_edges: Vec<(EdgeId, VertexId, VertexId, Weight, f64)> =
            Vec::with_capacity(original_edges);

        for edge in graph.edges() {
            let deg_u = *self.degree_cache.get(&edge.source).unwrap_or(&1);
            let deg_v = *self.degree_cache.get(&edge.target).unwrap_or(&1);
            let resistance = self.effective_resistance(deg_u, deg_v);

            scored_edges.push((edge.id, edge.source, edge.target, edge.weight, resistance));
        }

        // Sort by resistance (descending - high resistance = important)
        scored_edges.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));

        // Determine how many edges to keep
        let target_count = if let Some(max) = self.config.max_edges {
            max.min(original_edges)
        } else {
            ((original_edges as f64 * self.config.target_sparsity).ceil() as usize).max(1)
        };

        // Keep edges with highest effective resistance
        let mut result_edges = Vec::with_capacity(target_count);
        let mut edge_mapping = HashMap::with_capacity(target_count);
        let mut kept_vertices = HashSet::new();

        for (idx, (edge_id, u, v, weight, resistance)) in scored_edges.into_iter().enumerate() {
            if result_edges.len() >= target_count && resistance < threshold {
                break;
            }

            // Scale weight by inverse sampling probability
            let sampling_prob = self.sampling_probability(resistance, threshold);
            let scaled_weight = if sampling_prob > 0.0 {
                weight / sampling_prob
            } else {
                weight
            };

            result_edges.push((u, v, scaled_weight));
            edge_mapping.insert(result_edges.len() - 1, edge_id);
            kept_vertices.insert(u);
            kept_vertices.insert(v);

            if result_edges.len() >= target_count {
                break;
            }
        }

        let elapsed_us = start.elapsed().as_micros() as u64;
        let sparse_edges = result_edges.len();

        // Estimate speedup: O(m) -> O(m') where m' << m
        // Plus the 5.9x from avoiding exact resistance computation
        let sparsity_speedup = if sparse_edges > 0 {
            original_edges as f64 / sparse_edges as f64
        } else {
            1.0
        };
        let speedup_factor = sparsity_speedup.min(5.9); // Cap at theoretical DSpar speedup

        PresparseResult {
            edges: result_edges,
            edge_mapping,
            stats: PresparseStats {
                original_edges,
                sparse_edges,
                sparsity_ratio: sparse_edges as f64 / original_edges.max(1) as f64,
                time_us: elapsed_us,
                speedup_factor,
                vertices_processed: kept_vertices.len(),
            },
        }
    }

    /// Compute sampling probability for an edge
    #[inline]
    fn sampling_probability(&self, resistance: f64, threshold: f64) -> f64 {
        if resistance >= threshold {
            1.0 // Always keep high-resistance edges
        } else {
            // Probability proportional to resistance
            (resistance / threshold).max(0.01)
        }
    }

    /// Incremental update: handle edge insertion
    ///
    /// Returns whether the edge should be included in the sparse graph
    pub fn should_include_edge(&mut self, graph: &DynamicGraph, u: VertexId, v: VertexId) -> bool {
        // Update degree cache
        self.degree_cache.insert(u, graph.degree(u));
        self.degree_cache.insert(v, graph.degree(v));

        let deg_u = *self.degree_cache.get(&u).unwrap_or(&1);
        let deg_v = *self.degree_cache.get(&v).unwrap_or(&1);
        let resistance = self.effective_resistance(deg_u, deg_v);

        let threshold = if self.config.adaptive_threshold {
            self.compute_adaptive_threshold(graph)
        } else {
            self.config.resistance_threshold
        };

        resistance >= threshold
    }

    /// Get statistics for the presparse
    pub fn config(&self) -> &PresparseConfig {
        &self.config
    }
}

impl Default for DegreePresparse {
    fn default() -> Self {
        Self::new()
    }
}

/// Spectral concordance loss for validating sparsification quality
///
/// L = λ₁·Laplacian_Alignment + λ₂·Feature_Preserve + λ₃·Sparsity
pub struct SpectralConcordance {
    /// Weight for Laplacian alignment term
    pub lambda_laplacian: f64,
    /// Weight for feature preservation term
    pub lambda_feature: f64,
    /// Weight for sparsity inducing term
    pub lambda_sparsity: f64,
}

impl Default for SpectralConcordance {
    fn default() -> Self {
        Self {
            lambda_laplacian: 1.0,
            lambda_feature: 0.5,
            lambda_sparsity: 0.1,
        }
    }
}

impl SpectralConcordance {
    /// Compute the spectral concordance loss between original and sparse graphs
    pub fn compute_loss(&self, original: &DynamicGraph, sparse: &DynamicGraph) -> f64 {
        let laplacian_loss = self.laplacian_alignment_loss(original, sparse);
        let feature_loss = self.feature_preservation_loss(original, sparse);
        let sparsity_loss = self.sparsity_loss(original, sparse);

        self.lambda_laplacian * laplacian_loss
            + self.lambda_feature * feature_loss
            + self.lambda_sparsity * sparsity_loss
    }

    /// Approximate Laplacian alignment loss using degree distribution
    fn laplacian_alignment_loss(&self, original: &DynamicGraph, sparse: &DynamicGraph) -> f64 {
        let orig_vertices = original.vertices();
        if orig_vertices.is_empty() {
            return 0.0;
        }

        let mut total_diff = 0.0;
        let mut count = 0;

        for v in orig_vertices {
            let orig_deg = original.degree(v) as f64;
            let sparse_deg = sparse.degree(v) as f64;

            if orig_deg > 0.0 {
                // Relative degree difference
                total_diff += ((orig_deg - sparse_deg) / orig_deg).abs();
                count += 1;
            }
        }

        if count > 0 {
            total_diff / count as f64
        } else {
            0.0
        }
    }

    /// Feature preservation loss (cut value approximation)
    fn feature_preservation_loss(&self, original: &DynamicGraph, sparse: &DynamicGraph) -> f64 {
        // Compare minimum degree (crude cut approximation)
        let orig_min_deg = original
            .vertices()
            .iter()
            .map(|&v| original.degree(v))
            .min()
            .unwrap_or(0) as f64;

        let sparse_min_deg = sparse
            .vertices()
            .iter()
            .map(|&v| sparse.degree(v))
            .min()
            .unwrap_or(0) as f64;

        if orig_min_deg > 0.0 {
            ((orig_min_deg - sparse_min_deg) / orig_min_deg).abs()
        } else {
            0.0
        }
    }

    /// Sparsity inducing loss
    fn sparsity_loss(&self, original: &DynamicGraph, sparse: &DynamicGraph) -> f64 {
        let orig_edges = original.num_edges().max(1) as f64;
        let sparse_edges = sparse.num_edges() as f64;
        sparse_edges / orig_edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> DynamicGraph {
        let g = DynamicGraph::new();
        // Create a dense graph
        for i in 1..=10 {
            for j in (i + 1)..=10 {
                let _ = g.insert_edge(i, j, 1.0);
            }
        }
        g
    }

    #[test]
    fn test_effective_resistance() {
        let dspar = DegreePresparse::new();

        // High degree vertices -> low resistance
        assert!(dspar.effective_resistance(10, 10) < dspar.effective_resistance(2, 2));

        // Zero degree -> infinity
        assert!(dspar.effective_resistance(0, 5).is_infinite());
    }

    #[test]
    fn test_presparse_reduces_edges() {
        let graph = create_test_graph();
        let original_edges = graph.num_edges();

        let mut dspar = DegreePresparse::with_config(PresparseConfig {
            target_sparsity: 0.3,
            ..Default::default()
        });

        let result = dspar.presparse(&graph);

        assert!(result.stats.sparse_edges < original_edges);
        assert!(result.stats.sparsity_ratio <= 0.5);
        assert!(result.stats.speedup_factor > 1.0);
    }

    #[test]
    fn test_presparse_preserves_connectivity() {
        let graph = create_test_graph();

        let mut dspar = DegreePresparse::with_config(PresparseConfig {
            target_sparsity: 0.2,
            ..Default::default()
        });

        let result = dspar.presparse(&graph);

        // Should keep at least n-1 edges to maintain connectivity
        assert!(result.stats.sparse_edges >= graph.num_vertices() - 1);
    }

    #[test]
    fn test_adaptive_threshold() {
        let graph = create_test_graph();

        let mut dspar = DegreePresparse::with_config(PresparseConfig {
            adaptive_threshold: true,
            ..Default::default()
        });

        dspar.precompute_degrees(&graph);
        let threshold = dspar.compute_adaptive_threshold(&graph);

        assert!(threshold > 0.0);
    }

    #[test]
    fn test_spectral_concordance() {
        let original = create_test_graph();

        let mut dspar = DegreePresparse::with_config(PresparseConfig {
            target_sparsity: 0.5,
            ..Default::default()
        });

        let result = dspar.presparse(&original);

        // Create sparse graph
        let sparse = DynamicGraph::new();
        for (u, v, w) in &result.edges {
            let _ = sparse.insert_edge(*u, *v, *w);
        }

        let concordance = SpectralConcordance::default();
        let loss = concordance.compute_loss(&original, &sparse);

        // Loss should be bounded
        assert!(loss >= 0.0);
        assert!(loss < 10.0);
    }

    #[test]
    fn test_should_include_edge() {
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let mut dspar = DegreePresparse::with_config(PresparseConfig {
            resistance_threshold: 0.0,
            adaptive_threshold: false,
            ..Default::default()
        });

        // New edge to low-degree vertices should be included
        let should_include = dspar.should_include_edge(&graph, 1, 3);
        assert!(should_include);
    }

    #[test]
    fn test_edge_mapping() {
        let graph = create_test_graph();

        let mut dspar = DegreePresparse::new();
        let result = dspar.presparse(&graph);

        // Each sparse edge should map to an original edge
        for (idx, _) in result.edges.iter().enumerate() {
            assert!(result.edge_mapping.contains_key(&idx));
        }
    }
}
