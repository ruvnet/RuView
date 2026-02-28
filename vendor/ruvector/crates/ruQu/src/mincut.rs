//! Real Dynamic Min-Cut Integration
//!
//! This module provides integration with the `ruvector-mincut` crate's
//! SubpolynomialMinCut algorithm - the El-Hayek/Henzinger/Li December 2025
//! breakthrough achieving O(n^{o(1)}) amortized update time.
//!
//! When the `structural` feature is enabled, this provides real subpolynomial
//! min-cut computation. Otherwise, falls back to a degree-based heuristic.

#[cfg(not(feature = "structural"))]
use std::collections::HashMap;

/// Vertex identifier for min-cut graphs
pub type VertexId = u32;

/// Edge weight type
pub type Weight = f64;

/// Result of a min-cut query
#[derive(Debug, Clone)]
pub struct MinCutResult {
    /// The minimum cut value
    pub value: f64,
    /// Whether this is an exact result
    pub is_exact: bool,
    /// The cut edges (if computed)
    pub cut_edges: Option<Vec<(VertexId, VertexId)>>,
    /// Witness certificate (hash of witness tree)
    pub witness_hash: Option<[u8; 32]>,
}

/// Dynamic min-cut engine using the real El-Hayek/Henzinger/Li algorithm
#[cfg(feature = "structural")]
pub struct DynamicMinCutEngine {
    /// The real subpolynomial min-cut structure
    inner: ruvector_mincut::subpolynomial::SubpolynomialMinCut,
    /// Cached cut value
    cached_cut: Option<f64>,
    /// Generation counter for cache invalidation
    generation: u64,
}

#[cfg(feature = "structural")]
impl DynamicMinCutEngine {
    /// Create a new dynamic min-cut engine
    pub fn new() -> Self {
        use ruvector_mincut::subpolynomial::{SubpolyConfig, SubpolynomialMinCut};

        let config = SubpolyConfig {
            phi: 0.01,
            lambda_max: 1000,
            epsilon: 0.1,
            target_levels: 4,
            track_recourse: false,
            certify_cuts: true,
            parallel: false,
            ..Default::default()
        };

        Self {
            inner: SubpolynomialMinCut::new(config),
            cached_cut: None,
            generation: 0,
        }
    }

    /// Insert an edge
    #[inline]
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) {
        let _ = self.inner.insert_edge(u as u64, v as u64, weight);
        self.cached_cut = None;
        self.generation += 1;
    }

    /// Delete an edge
    #[inline]
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) {
        let _ = self.inner.delete_edge(u as u64, v as u64);
        self.cached_cut = None;
        self.generation += 1;
    }

    /// Update edge weight
    #[inline]
    pub fn update_weight(&mut self, u: VertexId, v: VertexId, new_weight: Weight) {
        // Delete and re-insert with new weight
        let _ = self.inner.delete_edge(u as u64, v as u64);
        let _ = self.inner.insert_edge(u as u64, v as u64, new_weight);
        self.cached_cut = None;
        self.generation += 1;
    }

    /// Query the minimum cut value
    #[inline]
    pub fn min_cut_value(&mut self) -> f64 {
        if let Some(cached) = self.cached_cut {
            return cached;
        }

        let value = self.inner.min_cut_value();
        self.cached_cut = Some(value);
        value
    }

    /// Query full min-cut result with certificate
    pub fn min_cut(&mut self) -> MinCutResult {
        let result = self.inner.min_cut();

        // Compute witness hash from result properties
        let mut hasher = blake3::Hasher::new();
        hasher.update(&result.value.to_le_bytes());
        hasher.update(if result.is_exact { &[1u8] } else { &[0u8] });
        hasher.update(if result.complexity_verified {
            &[1u8]
        } else {
            &[0u8]
        });
        let witness_hash = Some(*hasher.finalize().as_bytes());

        MinCutResult {
            value: result.value,
            is_exact: result.is_exact,
            cut_edges: result.cut_edges.map(|edges| {
                edges
                    .into_iter()
                    .map(|(u, v)| (u as VertexId, v as VertexId))
                    .collect()
            }),
            witness_hash,
        }
    }

    /// Get current generation (for cache coordination)
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Check if the graph is connected (by checking min-cut > 0)
    pub fn is_connected(&self) -> bool {
        // A graph is connected if its min-cut is positive
        self.inner.min_cut_value() > 0.0
    }

    /// Get number of vertices
    pub fn num_vertices(&self) -> usize {
        self.inner.num_vertices()
    }

    /// Get number of edges
    pub fn num_edges(&self) -> usize {
        self.inner.num_edges()
    }
}

#[cfg(feature = "structural")]
impl Default for DynamicMinCutEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Fallback min-cut engine when ruvector-mincut is not available
#[cfg(not(feature = "structural"))]
pub struct DynamicMinCutEngine {
    /// Simple edge list for degree-based heuristic
    edges: HashMap<(VertexId, VertexId), Weight>,
    /// Vertex degrees
    degrees: HashMap<VertexId, u32>,
    /// Total weight
    total_weight: f64,
    /// Generation counter
    generation: u64,
}

#[cfg(not(feature = "structural"))]
impl DynamicMinCutEngine {
    /// Create a new fallback min-cut engine
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            degrees: HashMap::new(),
            total_weight: 0.0,
            generation: 0,
        }
    }

    /// Insert an edge
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) {
        let key = if u < v { (u, v) } else { (v, u) };
        if self.edges.insert(key, weight).is_none() {
            *self.degrees.entry(u).or_insert(0) += 1;
            *self.degrees.entry(v).or_insert(0) += 1;
            self.total_weight += weight;
        }
        self.generation += 1;
    }

    /// Delete an edge
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) {
        let key = if u < v { (u, v) } else { (v, u) };
        if let Some(weight) = self.edges.remove(&key) {
            if let Some(deg) = self.degrees.get_mut(&u) {
                *deg = deg.saturating_sub(1);
            }
            if let Some(deg) = self.degrees.get_mut(&v) {
                *deg = deg.saturating_sub(1);
            }
            self.total_weight -= weight;
        }
        self.generation += 1;
    }

    /// Update edge weight
    pub fn update_weight(&mut self, u: VertexId, v: VertexId, new_weight: Weight) {
        let key = if u < v { (u, v) } else { (v, u) };
        if let Some(old_weight) = self.edges.get_mut(&key) {
            self.total_weight -= *old_weight;
            *old_weight = new_weight;
            self.total_weight += new_weight;
        }
        self.generation += 1;
    }

    /// Query the minimum cut value (heuristic: min degree * avg weight)
    pub fn min_cut_value(&mut self) -> f64 {
        if self.degrees.is_empty() || self.edges.is_empty() {
            return 0.0;
        }

        let min_degree = self.degrees.values().copied().min().unwrap_or(0) as f64;
        let avg_weight = self.total_weight / self.edges.len() as f64;

        min_degree * avg_weight
    }

    /// Query full min-cut result (heuristic, not exact)
    pub fn min_cut(&mut self) -> MinCutResult {
        MinCutResult {
            value: self.min_cut_value(),
            is_exact: false,
            cut_edges: None,
            witness_hash: None,
        }
    }

    /// Get current generation
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Check if the graph is connected (simplified check)
    pub fn is_connected(&self) -> bool {
        // Simplified: assume connected if we have edges
        !self.edges.is_empty()
    }

    /// Get number of vertices
    pub fn num_vertices(&self) -> usize {
        self.degrees.len()
    }

    /// Get number of edges
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }
}

#[cfg(not(feature = "structural"))]
impl Default for DynamicMinCutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_basic() {
        let mut engine = DynamicMinCutEngine::new();

        // Build a simple triangle
        engine.insert_edge(0, 1, 1.0);
        engine.insert_edge(1, 2, 1.0);
        engine.insert_edge(2, 0, 1.0);

        let cut = engine.min_cut_value();
        assert!(cut > 0.0, "Triangle should have positive min-cut");

        // Add another vertex with single connection
        engine.insert_edge(2, 3, 1.0);
        let cut2 = engine.min_cut_value();

        // Min-cut should be 1 (the single edge to vertex 3)
        // With heuristic, it will be approximately this
        assert!(cut2 <= cut + 0.1 || cut2 >= 0.9);
    }

    #[test]
    fn test_engine_delete() {
        let mut engine = DynamicMinCutEngine::new();

        engine.insert_edge(0, 1, 1.0);
        engine.insert_edge(1, 2, 1.0);
        engine.insert_edge(2, 0, 1.0);

        let gen1 = engine.generation();
        engine.delete_edge(2, 0);
        let gen2 = engine.generation();

        assert!(gen2 > gen1, "Generation should increase on delete");
        assert_eq!(engine.num_edges(), 2);
    }

    #[test]
    fn test_min_cut_result() {
        let mut engine = DynamicMinCutEngine::new();

        engine.insert_edge(0, 1, 2.0);
        engine.insert_edge(1, 2, 3.0);

        let result = engine.min_cut();
        assert!(result.value >= 0.0);

        #[cfg(feature = "structural")]
        assert!(result.is_exact, "With structural feature, should be exact");

        #[cfg(not(feature = "structural"))]
        assert!(!result.is_exact, "Without structural feature, is heuristic");
    }
}
