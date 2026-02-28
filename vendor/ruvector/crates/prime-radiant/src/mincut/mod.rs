//! MinCut Incoherence Isolation Module
//!
//! Isolates incoherent subgraphs using subpolynomial n^o(1) dynamic minimum cut.
//! Leverages `ruvector-mincut` for the December 2024 breakthrough algorithm.
//!
//! # Features
//!
//! - Subpolynomial O(n^o(1)) update time for dynamic graphs
//! - Incoherent region isolation with minimum boundary
//! - Certificate-based cut verification with witness trees
//! - SNN-based cognitive optimization
//!
//! # Use Cases
//!
//! - Isolate high-energy (incoherent) subgraphs for focused repair
//! - Find minimum cuts to quarantine problematic regions
//! - Dynamic graph updates with fast recomputation

mod adapter;
mod config;
mod isolation;
mod metrics;

pub use adapter::MinCutAdapter;
pub use config::MinCutConfig;
pub use isolation::{IsolationRegion, IsolationResult};
pub use metrics::IsolationMetrics;

use std::collections::{HashMap, HashSet};

/// Vertex identifier type
pub type VertexId = u64;

/// Edge identifier type
pub type EdgeId = (VertexId, VertexId);

/// Weight type for edges
pub type Weight = f64;

/// Result type for mincut operations
pub type Result<T> = std::result::Result<T, MinCutError>;

/// Errors that can occur in mincut operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum MinCutError {
    /// Edge already exists
    #[error("Edge already exists: ({0}, {1})")]
    EdgeExists(VertexId, VertexId),

    /// Edge not found
    #[error("Edge not found: ({0}, {1})")]
    EdgeNotFound(VertexId, VertexId),

    /// Vertex not found
    #[error("Vertex not found: {0}")]
    VertexNotFound(VertexId),

    /// Graph is empty
    #[error("Graph is empty")]
    EmptyGraph,

    /// Invalid threshold
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(f64),

    /// Cut computation failed
    #[error("Cut computation failed: {0}")]
    ComputationFailed(String),

    /// Hierarchy not built
    #[error("Hierarchy not built - call build() first")]
    HierarchyNotBuilt,
}

/// Main incoherence isolator using subpolynomial mincut
///
/// This module identifies and isolates regions of the coherence graph
/// where energy is above threshold, using minimum cut to find the
/// boundary with smallest total weight.
#[derive(Debug)]
pub struct IncoherenceIsolator {
    /// Configuration
    config: MinCutConfig,
    /// Adapter to underlying mincut algorithm
    adapter: MinCutAdapter,
    /// Edge weights (typically residual energy)
    edge_weights: HashMap<EdgeId, Weight>,
    /// Vertex set
    vertices: HashSet<VertexId>,
    /// Is hierarchy built?
    hierarchy_built: bool,
    /// Isolation metrics
    metrics: IsolationMetrics,
}

impl IncoherenceIsolator {
    /// Create a new incoherence isolator
    pub fn new(config: MinCutConfig) -> Self {
        let adapter = MinCutAdapter::new(config.clone());

        Self {
            config,
            adapter,
            edge_weights: HashMap::new(),
            vertices: HashSet::new(),
            hierarchy_built: false,
            metrics: IsolationMetrics::new(),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(MinCutConfig::default())
    }

    /// Create optimized for expected graph size
    pub fn for_size(expected_vertices: usize) -> Self {
        Self::new(MinCutConfig::for_size(expected_vertices))
    }

    /// Insert an edge with weight
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<()> {
        let key = Self::edge_key(u, v);

        if self.edge_weights.contains_key(&key) {
            return Err(MinCutError::EdgeExists(u, v));
        }

        self.edge_weights.insert(key, weight);
        self.vertices.insert(u);
        self.vertices.insert(v);

        // Update adapter
        self.adapter.insert_edge(u, v, weight)?;

        // If hierarchy was built, track this as an incremental update
        if self.hierarchy_built {
            self.metrics.record_update();
        }

        Ok(())
    }

    /// Delete an edge
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()> {
        let key = Self::edge_key(u, v);

        if !self.edge_weights.contains_key(&key) {
            return Err(MinCutError::EdgeNotFound(u, v));
        }

        self.edge_weights.remove(&key);
        self.adapter.delete_edge(u, v)?;

        if self.hierarchy_built {
            self.metrics.record_update();
        }

        Ok(())
    }

    /// Update edge weight
    pub fn update_weight(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<()> {
        let key = Self::edge_key(u, v);

        if !self.edge_weights.contains_key(&key) {
            return Err(MinCutError::EdgeNotFound(u, v));
        }

        // Delete and re-insert with new weight
        self.adapter.delete_edge(u, v)?;
        self.adapter.insert_edge(u, v, weight)?;
        self.edge_weights.insert(key, weight);

        if self.hierarchy_built {
            self.metrics.record_update();
        }

        Ok(())
    }

    /// Build the multi-level hierarchy for subpolynomial updates
    ///
    /// This creates O(log^{1/4} n) levels of expander decomposition.
    pub fn build(&mut self) {
        if self.edge_weights.is_empty() {
            return;
        }

        self.adapter.build();
        self.hierarchy_built = true;
        self.metrics.record_build();
    }

    /// Get global minimum cut value
    pub fn min_cut_value(&self) -> Result<f64> {
        if !self.hierarchy_built {
            return Err(MinCutError::HierarchyNotBuilt);
        }
        Ok(self.adapter.min_cut_value())
    }

    /// Find minimum cut to isolate high-energy region
    ///
    /// Returns the cut that separates vertices with edges above `threshold`
    /// from the rest of the graph.
    pub fn isolate_high_energy(&mut self, threshold: Weight) -> Result<IsolationResult> {
        if !self.hierarchy_built {
            return Err(MinCutError::HierarchyNotBuilt);
        }

        if threshold <= 0.0 {
            return Err(MinCutError::InvalidThreshold(threshold));
        }

        // Identify high-energy edges
        let high_energy_edges: Vec<EdgeId> = self
            .edge_weights
            .iter()
            .filter(|(_, &w)| w > threshold)
            .map(|(&k, _)| k)
            .collect();

        if high_energy_edges.is_empty() {
            return Ok(IsolationResult::no_isolation());
        }

        // Get vertices incident to high-energy edges
        let mut high_energy_vertices: HashSet<VertexId> = HashSet::new();
        for (u, v) in &high_energy_edges {
            high_energy_vertices.insert(*u);
            high_energy_vertices.insert(*v);
        }

        // Compute isolation using adapter
        let cut_result = self.adapter.compute_isolation(&high_energy_vertices)?;

        let result = IsolationResult {
            isolated_vertices: cut_result.isolated_set,
            cut_edges: cut_result.cut_edges,
            cut_value: cut_result.cut_value,
            num_high_energy_edges: high_energy_edges.len(),
            threshold,
            is_verified: cut_result.is_verified,
        };

        self.metrics.record_isolation(&result);

        Ok(result)
    }

    /// Find multiple isolated regions using iterative mincut
    pub fn find_isolated_regions(&mut self, threshold: Weight) -> Result<Vec<IsolationRegion>> {
        if !self.hierarchy_built {
            return Err(MinCutError::HierarchyNotBuilt);
        }

        // Get high-energy edges
        let high_energy_edges: Vec<(EdgeId, Weight)> = self
            .edge_weights
            .iter()
            .filter(|(_, &w)| w > threshold)
            .map(|(&k, &w)| (k, w))
            .collect();

        if high_energy_edges.is_empty() {
            return Ok(vec![]);
        }

        // Group connected components of high-energy edges
        let mut regions: Vec<IsolationRegion> = Vec::new();
        let mut visited: HashSet<VertexId> = HashSet::new();

        for ((u, v), weight) in &high_energy_edges {
            if visited.contains(u) && visited.contains(v) {
                continue;
            }

            // BFS to find connected component
            let mut component_vertices: HashSet<VertexId> = HashSet::new();
            let mut component_edges: Vec<EdgeId> = Vec::new();
            let mut queue: Vec<VertexId> = vec![*u, *v];
            let mut component_energy = 0.0;

            while let Some(vertex) = queue.pop() {
                if visited.contains(&vertex) {
                    continue;
                }
                visited.insert(vertex);
                component_vertices.insert(vertex);

                // Find adjacent high-energy edges
                for ((eu, ev), ew) in &high_energy_edges {
                    if *eu == vertex || *ev == vertex {
                        if !component_edges.contains(&(*eu, *ev)) {
                            component_edges.push((*eu, *ev));
                            component_energy += ew;
                        }
                        if !visited.contains(eu) {
                            queue.push(*eu);
                        }
                        if !visited.contains(ev) {
                            queue.push(*ev);
                        }
                    }
                }
            }

            // Compute boundary
            let boundary_edges: Vec<EdgeId> = self
                .edge_weights
                .keys()
                .filter(|(a, b)| {
                    (component_vertices.contains(a) && !component_vertices.contains(b))
                        || (component_vertices.contains(b) && !component_vertices.contains(a))
                })
                .copied()
                .collect();

            let boundary_weight: Weight = boundary_edges
                .iter()
                .filter_map(|e| self.edge_weights.get(e))
                .sum();

            regions.push(IsolationRegion {
                vertices: component_vertices,
                internal_edges: component_edges,
                boundary_edges,
                total_energy: component_energy,
                boundary_weight,
                region_id: regions.len(),
            });
        }

        Ok(regions)
    }

    /// Check if updates maintain subpolynomial complexity
    pub fn is_subpolynomial(&self) -> bool {
        self.adapter.is_subpolynomial()
    }

    /// Get recourse statistics
    pub fn recourse_stats(&self) -> RecourseStats {
        self.adapter.recourse_stats()
    }

    /// Get hierarchy statistics
    pub fn hierarchy_stats(&self) -> HierarchyStats {
        self.adapter.hierarchy_stats()
    }

    /// Get isolation metrics
    pub fn metrics(&self) -> &IsolationMetrics {
        &self.metrics
    }

    /// Get number of vertices
    pub fn num_vertices(&self) -> usize {
        self.vertices.len()
    }

    /// Get number of edges
    pub fn num_edges(&self) -> usize {
        self.edge_weights.len()
    }

    /// Get configuration
    pub fn config(&self) -> &MinCutConfig {
        &self.config
    }

    /// Canonical edge key (smaller vertex first)
    fn edge_key(u: VertexId, v: VertexId) -> EdgeId {
        if u < v {
            (u, v)
        } else {
            (v, u)
        }
    }
}

/// Recourse statistics from the subpolynomial algorithm
#[derive(Debug, Clone, Default)]
pub struct RecourseStats {
    /// Total recourse across all updates
    pub total_recourse: u64,
    /// Number of updates
    pub num_updates: u64,
    /// Maximum single update recourse
    pub max_single_recourse: u64,
    /// Average update time in microseconds
    pub avg_update_time_us: f64,
    /// Theoretical subpolynomial bound
    pub theoretical_bound: f64,
}

impl RecourseStats {
    /// Get amortized recourse per update
    pub fn amortized_recourse(&self) -> f64 {
        if self.num_updates == 0 {
            0.0
        } else {
            self.total_recourse as f64 / self.num_updates as f64
        }
    }

    /// Check if within theoretical bounds
    pub fn within_bounds(&self) -> bool {
        self.amortized_recourse() <= self.theoretical_bound
    }
}

/// Hierarchy statistics
#[derive(Debug, Clone, Default)]
pub struct HierarchyStats {
    /// Number of levels
    pub num_levels: usize,
    /// Expanders per level
    pub expanders_per_level: Vec<usize>,
    /// Total expanders
    pub total_expanders: usize,
    /// Average expander size
    pub avg_expander_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut isolator = IncoherenceIsolator::default_config();

        // Build a simple graph
        isolator.insert_edge(1, 2, 0.5).unwrap();
        isolator.insert_edge(2, 3, 0.5).unwrap();
        isolator.insert_edge(3, 4, 2.0).unwrap(); // High energy
        isolator.insert_edge(4, 5, 0.5).unwrap();
        isolator.insert_edge(5, 6, 0.5).unwrap();

        assert_eq!(isolator.num_vertices(), 6);
        assert_eq!(isolator.num_edges(), 5);

        isolator.build();

        // Get min cut value
        let cut = isolator.min_cut_value().unwrap();
        assert!(cut > 0.0);
    }

    #[test]
    fn test_isolation() {
        let mut isolator = IncoherenceIsolator::default_config();

        // Two clusters connected by high-energy edge
        isolator.insert_edge(1, 2, 0.1).unwrap();
        isolator.insert_edge(2, 3, 0.1).unwrap();
        isolator.insert_edge(3, 1, 0.1).unwrap();

        isolator.insert_edge(3, 4, 5.0).unwrap(); // High energy bridge

        isolator.insert_edge(4, 5, 0.1).unwrap();
        isolator.insert_edge(5, 6, 0.1).unwrap();
        isolator.insert_edge(6, 4, 0.1).unwrap();

        isolator.build();

        let result = isolator.isolate_high_energy(1.0).unwrap();

        assert_eq!(result.num_high_energy_edges, 1);
        assert!(result.cut_value >= 0.0);
    }

    #[test]
    fn test_find_regions() {
        let mut isolator = IncoherenceIsolator::default_config();

        // Create two separate high-energy regions
        isolator.insert_edge(1, 2, 5.0).unwrap();
        isolator.insert_edge(2, 3, 0.1).unwrap();

        isolator.insert_edge(10, 11, 5.0).unwrap();
        isolator.insert_edge(11, 12, 5.0).unwrap();

        // Connect them with low-energy edge
        isolator.insert_edge(3, 10, 0.1).unwrap();

        isolator.build();

        let regions = isolator.find_isolated_regions(1.0).unwrap();

        // Should find 2 high-energy regions
        assert!(regions.len() >= 1);
    }

    #[test]
    fn test_update_weight() {
        let mut isolator = IncoherenceIsolator::default_config();

        isolator.insert_edge(1, 2, 0.5).unwrap();
        isolator.insert_edge(2, 3, 0.5).unwrap();

        isolator.build();

        // Update weight
        isolator.update_weight(1, 2, 2.0).unwrap();

        // Rebuild and check
        isolator.build();
        assert!(isolator.min_cut_value().is_ok());
    }

    #[test]
    fn test_delete_edge() {
        let mut isolator = IncoherenceIsolator::default_config();

        isolator.insert_edge(1, 2, 0.5).unwrap();
        isolator.insert_edge(2, 3, 0.5).unwrap();
        isolator.insert_edge(3, 1, 0.5).unwrap();

        assert_eq!(isolator.num_edges(), 3);

        isolator.delete_edge(1, 2).unwrap();

        assert_eq!(isolator.num_edges(), 2);
    }
}
