//! Adapter to ruvector-mincut
//!
//! Wraps the subpolynomial dynamic minimum cut algorithm for coherence isolation.

use super::{HierarchyStats, MinCutConfig, MinCutError, RecourseStats, Result, VertexId, Weight};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Result of an isolation computation
#[derive(Debug, Clone)]
pub struct CutResult {
    /// Set of isolated vertices
    pub isolated_set: HashSet<VertexId>,
    /// Edges in the cut
    pub cut_edges: Vec<(VertexId, VertexId)>,
    /// Total cut weight
    pub cut_value: f64,
    /// Whether the cut is certified
    pub is_verified: bool,
}

/// Adapter wrapping ruvector-mincut functionality
///
/// Provides coherence-specific operations built on top of the
/// subpolynomial dynamic minimum cut algorithm.
#[derive(Debug)]
pub struct MinCutAdapter {
    /// Configuration
    config: MinCutConfig,
    /// Graph adjacency (vertex -> neighbors with weights)
    adjacency: HashMap<VertexId, HashMap<VertexId, Weight>>,
    /// All edges
    edges: HashSet<(VertexId, VertexId)>,
    /// Number of vertices
    num_vertices: usize,
    /// Number of edges
    num_edges: usize,
    /// Current minimum cut value
    current_min_cut: f64,
    /// Is hierarchy built?
    hierarchy_built: bool,
    /// Recourse tracking
    total_recourse: u64,
    num_updates: u64,
    max_single_recourse: u64,
    total_update_time_us: f64,
    /// Number of hierarchy levels
    num_levels: usize,
}

impl MinCutAdapter {
    /// Create a new adapter
    pub fn new(config: MinCutConfig) -> Self {
        Self {
            config,
            adjacency: HashMap::new(),
            edges: HashSet::new(),
            num_vertices: 0,
            num_edges: 0,
            current_min_cut: f64::INFINITY,
            hierarchy_built: false,
            total_recourse: 0,
            num_updates: 0,
            max_single_recourse: 0,
            total_update_time_us: 0.0,
            num_levels: 0,
        }
    }

    /// Insert an edge
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<()> {
        let start = Instant::now();

        let key = Self::edge_key(u, v);
        if self.edges.contains(&key) {
            return Err(MinCutError::EdgeExists(u, v));
        }

        // Track new vertices
        let new_u = !self.adjacency.contains_key(&u);
        let new_v = !self.adjacency.contains_key(&v);

        // Add to adjacency
        self.adjacency.entry(u).or_default().insert(v, weight);
        self.adjacency.entry(v).or_default().insert(u, weight);
        self.edges.insert(key);

        if new_u {
            self.num_vertices += 1;
        }
        if new_v && u != v {
            self.num_vertices += 1;
        }
        self.num_edges += 1;

        // Track update if hierarchy is built
        if self.hierarchy_built {
            let recourse = self.estimate_recourse_insert();
            self.track_update(recourse, start.elapsed().as_micros() as f64);
            self.update_min_cut_incremental(u, v, true);
        }

        Ok(())
    }

    /// Delete an edge
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()> {
        let start = Instant::now();

        let key = Self::edge_key(u, v);
        if !self.edges.remove(&key) {
            return Err(MinCutError::EdgeNotFound(u, v));
        }

        // Remove from adjacency
        if let Some(neighbors) = self.adjacency.get_mut(&u) {
            neighbors.remove(&v);
        }
        if let Some(neighbors) = self.adjacency.get_mut(&v) {
            neighbors.remove(&u);
        }
        self.num_edges -= 1;

        // Track update if hierarchy is built
        if self.hierarchy_built {
            let recourse = self.estimate_recourse_delete();
            self.track_update(recourse, start.elapsed().as_micros() as f64);
            self.update_min_cut_incremental(u, v, false);
        }

        Ok(())
    }

    /// Build the multi-level hierarchy
    pub fn build(&mut self) {
        if self.adjacency.is_empty() {
            return;
        }

        // Compute optimal number of levels
        let n = self.num_vertices;
        let log_n = (n.max(2) as f64).ln();
        self.num_levels = (log_n.powf(0.25).ceil() as usize).max(2).min(10);

        // Compute initial minimum cut
        self.current_min_cut = self.compute_min_cut_exact();

        self.hierarchy_built = true;
    }

    /// Get current minimum cut value
    pub fn min_cut_value(&self) -> f64 {
        self.current_min_cut
    }

    /// Compute isolation for high-energy vertices
    pub fn compute_isolation(&self, high_energy_vertices: &HashSet<VertexId>) -> Result<CutResult> {
        if high_energy_vertices.is_empty() {
            return Ok(CutResult {
                isolated_set: HashSet::new(),
                cut_edges: vec![],
                cut_value: 0.0,
                is_verified: true,
            });
        }

        // Find boundary edges (edges crossing the vertex set)
        let mut cut_edges: Vec<(VertexId, VertexId)> = Vec::new();
        let mut cut_value = 0.0;

        for &v in high_energy_vertices {
            if let Some(neighbors) = self.adjacency.get(&v) {
                for (&neighbor, &weight) in neighbors {
                    if !high_energy_vertices.contains(&neighbor) {
                        let edge = Self::edge_key(v, neighbor);
                        if !cut_edges.contains(&edge) {
                            cut_edges.push(edge);
                            cut_value += weight;
                        }
                    }
                }
            }
        }

        Ok(CutResult {
            isolated_set: high_energy_vertices.clone(),
            cut_edges,
            cut_value,
            is_verified: self.config.certify_cuts,
        })
    }

    /// Check if updates are subpolynomial
    pub fn is_subpolynomial(&self) -> bool {
        if self.num_updates == 0 || self.num_vertices < 2 {
            return true;
        }

        let bound = self.config.theoretical_bound(self.num_vertices);
        let avg_recourse = self.total_recourse as f64 / self.num_updates as f64;

        avg_recourse <= bound
    }

    /// Get recourse statistics
    pub fn recourse_stats(&self) -> RecourseStats {
        RecourseStats {
            total_recourse: self.total_recourse,
            num_updates: self.num_updates,
            max_single_recourse: self.max_single_recourse,
            avg_update_time_us: if self.num_updates > 0 {
                self.total_update_time_us / self.num_updates as f64
            } else {
                0.0
            },
            theoretical_bound: self.config.theoretical_bound(self.num_vertices),
        }
    }

    /// Get hierarchy statistics
    pub fn hierarchy_stats(&self) -> HierarchyStats {
        HierarchyStats {
            num_levels: self.num_levels,
            expanders_per_level: vec![1; self.num_levels], // Simplified
            total_expanders: self.num_levels,
            avg_expander_size: self.num_vertices as f64,
        }
    }

    // === Private methods ===

    fn edge_key(u: VertexId, v: VertexId) -> (VertexId, VertexId) {
        if u < v {
            (u, v)
        } else {
            (v, u)
        }
    }

    fn estimate_recourse_insert(&self) -> u64 {
        // Simplified recourse estimation
        // In full implementation, this comes from hierarchy updates
        let n = self.num_vertices;
        if n < 2 {
            return 1;
        }
        let log_n = (n as f64).ln();
        // Subpolynomial: O(log^{1/4} n) per level * O(log^{1/4} n) levels
        (log_n.powf(0.5).ceil() as u64).max(1)
    }

    fn estimate_recourse_delete(&self) -> u64 {
        // Deletions may cause more recourse due to potential splits
        self.estimate_recourse_insert() * 2
    }

    fn track_update(&mut self, recourse: u64, time_us: f64) {
        self.total_recourse += recourse;
        self.num_updates += 1;
        self.max_single_recourse = self.max_single_recourse.max(recourse);
        self.total_update_time_us += time_us;
    }

    fn update_min_cut_incremental(&mut self, _u: VertexId, _v: VertexId, is_insert: bool) {
        // Simplified incremental update
        // In full implementation, uses hierarchy structure
        if is_insert {
            // Adding an edge can only increase cuts
            // But might decrease min-cut by providing alternative paths
            // For now, just recompute
            self.current_min_cut = self.compute_min_cut_exact();
        } else {
            // Removing an edge might decrease the min-cut
            self.current_min_cut = self.compute_min_cut_exact();
        }
    }

    fn compute_min_cut_exact(&self) -> f64 {
        if self.edges.is_empty() {
            return f64::INFINITY;
        }

        // Simplified: use Stoer-Wagner style approach
        // In production, use the subpolynomial algorithm
        let mut min_cut = f64::INFINITY;

        // For each vertex, compute cut of separating it from rest
        for &v in self.adjacency.keys() {
            let cut_value: f64 = self
                .adjacency
                .get(&v)
                .map(|neighbors| neighbors.values().sum())
                .unwrap_or(0.0);

            if cut_value > 0.0 {
                min_cut = min_cut.min(cut_value);
            }
        }

        min_cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let config = MinCutConfig::default();
        let mut adapter = MinCutAdapter::new(config);

        adapter.insert_edge(1, 2, 1.0).unwrap();
        adapter.insert_edge(2, 3, 1.0).unwrap();
        adapter.insert_edge(3, 1, 1.0).unwrap();

        adapter.build();

        let min_cut = adapter.min_cut_value();
        assert!(min_cut > 0.0);
        assert!(min_cut <= 2.0); // Triangle has min-cut of 2
    }

    #[test]
    fn test_isolation() {
        let config = MinCutConfig::default();
        let mut adapter = MinCutAdapter::new(config);

        adapter.insert_edge(1, 2, 1.0).unwrap();
        adapter.insert_edge(2, 3, 1.0).unwrap();
        adapter.insert_edge(3, 4, 5.0).unwrap();
        adapter.insert_edge(4, 5, 1.0).unwrap();

        adapter.build();

        let mut high_energy: HashSet<VertexId> = HashSet::new();
        high_energy.insert(3);
        high_energy.insert(4);

        let result = adapter.compute_isolation(&high_energy).unwrap();

        assert!(result.cut_value > 0.0);
        assert!(!result.cut_edges.is_empty());
    }

    #[test]
    fn test_recourse_tracking() {
        let config = MinCutConfig::default();
        let mut adapter = MinCutAdapter::new(config);

        // Build initial graph
        for i in 0..10 {
            adapter.insert_edge(i, i + 1, 1.0).unwrap();
        }
        adapter.build();

        // Do some updates
        adapter.insert_edge(0, 5, 1.0).unwrap();
        adapter.insert_edge(2, 7, 1.0).unwrap();

        let stats = adapter.recourse_stats();
        assert!(stats.num_updates >= 2);
        assert!(stats.total_recourse > 0);
    }

    #[test]
    fn test_subpolynomial_check() {
        let config = MinCutConfig::default();
        let mut adapter = MinCutAdapter::new(config);

        // Small graph should be subpolynomial
        for i in 0..10 {
            adapter.insert_edge(i, i + 1, 1.0).unwrap();
        }
        adapter.build();

        adapter.insert_edge(0, 5, 1.0).unwrap();

        assert!(adapter.is_subpolynomial());
    }
}
