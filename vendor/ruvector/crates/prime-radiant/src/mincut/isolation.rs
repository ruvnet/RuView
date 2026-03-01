//! Isolation Structures
//!
//! Data structures representing isolated regions and results.

use super::{EdgeId, VertexId, Weight};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Result of an isolation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationResult {
    /// Vertices in the isolated region
    pub isolated_vertices: HashSet<VertexId>,
    /// Edges in the cut boundary
    pub cut_edges: Vec<EdgeId>,
    /// Total weight of the cut (boundary)
    pub cut_value: f64,
    /// Number of high-energy edges that triggered isolation
    pub num_high_energy_edges: usize,
    /// Threshold used for high-energy classification
    pub threshold: Weight,
    /// Whether the cut was verified by witness tree
    pub is_verified: bool,
}

impl IsolationResult {
    /// Create a result indicating no isolation needed
    pub fn no_isolation() -> Self {
        Self {
            isolated_vertices: HashSet::new(),
            cut_edges: vec![],
            cut_value: 0.0,
            num_high_energy_edges: 0,
            threshold: 0.0,
            is_verified: true,
        }
    }

    /// Check if any vertices were isolated
    pub fn has_isolation(&self) -> bool {
        !self.isolated_vertices.is_empty()
    }

    /// Get number of isolated vertices
    pub fn num_isolated(&self) -> usize {
        self.isolated_vertices.len()
    }

    /// Get number of cut edges
    pub fn num_cut_edges(&self) -> usize {
        self.cut_edges.len()
    }

    /// Calculate isolation efficiency (cut value per isolated vertex)
    pub fn efficiency(&self) -> f64 {
        if self.isolated_vertices.is_empty() {
            return 0.0;
        }
        self.cut_value / self.isolated_vertices.len() as f64
    }

    /// Check if a vertex is in the isolated set
    pub fn is_isolated(&self, vertex: VertexId) -> bool {
        self.isolated_vertices.contains(&vertex)
    }

    /// Get boundary vertices (endpoints of cut edges in isolated set)
    pub fn boundary_vertices(&self) -> HashSet<VertexId> {
        let mut boundary = HashSet::new();
        for (u, v) in &self.cut_edges {
            if self.isolated_vertices.contains(u) {
                boundary.insert(*u);
            }
            if self.isolated_vertices.contains(v) {
                boundary.insert(*v);
            }
        }
        boundary
    }
}

/// A connected region of high-energy edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationRegion {
    /// Vertices in this region
    pub vertices: HashSet<VertexId>,
    /// Internal edges (both endpoints in region)
    pub internal_edges: Vec<EdgeId>,
    /// Boundary edges (one endpoint outside region)
    pub boundary_edges: Vec<EdgeId>,
    /// Total energy of internal edges
    pub total_energy: Weight,
    /// Total weight of boundary edges
    pub boundary_weight: Weight,
    /// Unique region identifier
    pub region_id: usize,
}

impl IsolationRegion {
    /// Get number of vertices in region
    pub fn num_vertices(&self) -> usize {
        self.vertices.len()
    }

    /// Get number of internal edges
    pub fn num_internal_edges(&self) -> usize {
        self.internal_edges.len()
    }

    /// Get number of boundary edges
    pub fn num_boundary_edges(&self) -> usize {
        self.boundary_edges.len()
    }

    /// Calculate region density (edges per vertex)
    pub fn density(&self) -> f64 {
        if self.vertices.is_empty() {
            return 0.0;
        }
        self.internal_edges.len() as f64 / self.vertices.len() as f64
    }

    /// Calculate boundary ratio (boundary / internal edges)
    pub fn boundary_ratio(&self) -> f64 {
        if self.internal_edges.is_empty() {
            return f64::INFINITY;
        }
        self.boundary_edges.len() as f64 / self.internal_edges.len() as f64
    }

    /// Calculate average energy per edge
    pub fn avg_energy(&self) -> Weight {
        if self.internal_edges.is_empty() {
            return 0.0;
        }
        self.total_energy / self.internal_edges.len() as f64
    }

    /// Check if vertex is in this region
    pub fn contains(&self, vertex: VertexId) -> bool {
        self.vertices.contains(&vertex)
    }

    /// Check if edge is internal to this region
    pub fn is_internal_edge(&self, edge: &EdgeId) -> bool {
        self.internal_edges.contains(edge)
    }

    /// Check if edge is on the boundary
    pub fn is_boundary_edge(&self, edge: &EdgeId) -> bool {
        self.boundary_edges.contains(edge)
    }

    /// Get vertices on the boundary (adjacent to outside)
    pub fn boundary_vertices(&self) -> HashSet<VertexId> {
        let mut boundary = HashSet::new();
        for (u, v) in &self.boundary_edges {
            if self.vertices.contains(u) {
                boundary.insert(*u);
            }
            if self.vertices.contains(v) {
                boundary.insert(*v);
            }
        }
        boundary
    }

    /// Get interior vertices (not on boundary)
    pub fn interior_vertices(&self) -> HashSet<VertexId> {
        let boundary = self.boundary_vertices();
        self.vertices
            .iter()
            .filter(|v| !boundary.contains(v))
            .copied()
            .collect()
    }
}

/// Comparison result between two isolation results
#[derive(Debug, Clone)]
pub struct IsolationComparison {
    /// Vertices isolated in both results
    pub common_isolated: HashSet<VertexId>,
    /// Vertices only isolated in first result
    pub only_first: HashSet<VertexId>,
    /// Vertices only isolated in second result
    pub only_second: HashSet<VertexId>,
    /// Jaccard similarity of isolated sets
    pub jaccard_similarity: f64,
}

impl IsolationComparison {
    /// Compare two isolation results
    pub fn compare(first: &IsolationResult, second: &IsolationResult) -> Self {
        let common: HashSet<_> = first
            .isolated_vertices
            .intersection(&second.isolated_vertices)
            .copied()
            .collect();

        let only_first: HashSet<_> = first
            .isolated_vertices
            .difference(&second.isolated_vertices)
            .copied()
            .collect();

        let only_second: HashSet<_> = second
            .isolated_vertices
            .difference(&first.isolated_vertices)
            .copied()
            .collect();

        let union_size =
            first.isolated_vertices.len() + second.isolated_vertices.len() - common.len();
        let jaccard = if union_size > 0 {
            common.len() as f64 / union_size as f64
        } else {
            1.0 // Both empty = identical
        };

        Self {
            common_isolated: common,
            only_first,
            only_second,
            jaccard_similarity: jaccard,
        }
    }

    /// Check if results are identical
    pub fn is_identical(&self) -> bool {
        self.only_first.is_empty() && self.only_second.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_isolation() {
        let result = IsolationResult::no_isolation();
        assert!(!result.has_isolation());
        assert_eq!(result.num_isolated(), 0);
        assert!(result.is_verified);
    }

    #[test]
    fn test_isolation_result() {
        let mut isolated = HashSet::new();
        isolated.insert(1);
        isolated.insert(2);
        isolated.insert(3);

        let result = IsolationResult {
            isolated_vertices: isolated,
            cut_edges: vec![(3, 4), (3, 5)],
            cut_value: 2.5,
            num_high_energy_edges: 2,
            threshold: 1.0,
            is_verified: true,
        };

        assert!(result.has_isolation());
        assert_eq!(result.num_isolated(), 3);
        assert_eq!(result.num_cut_edges(), 2);
        assert!(result.is_isolated(1));
        assert!(!result.is_isolated(4));
    }

    #[test]
    fn test_boundary_vertices() {
        let mut isolated = HashSet::new();
        isolated.insert(1);
        isolated.insert(2);
        isolated.insert(3);

        let result = IsolationResult {
            isolated_vertices: isolated,
            cut_edges: vec![(3, 4), (2, 5)],
            cut_value: 2.0,
            num_high_energy_edges: 1,
            threshold: 1.0,
            is_verified: true,
        };

        let boundary = result.boundary_vertices();
        assert!(boundary.contains(&3));
        assert!(boundary.contains(&2));
        assert!(!boundary.contains(&1)); // Not on boundary
    }

    #[test]
    fn test_region() {
        let mut vertices = HashSet::new();
        vertices.insert(1);
        vertices.insert(2);
        vertices.insert(3);

        let region = IsolationRegion {
            vertices,
            internal_edges: vec![(1, 2), (2, 3)],
            boundary_edges: vec![(3, 4)],
            total_energy: 5.0,
            boundary_weight: 1.0,
            region_id: 0,
        };

        assert_eq!(region.num_vertices(), 3);
        assert_eq!(region.num_internal_edges(), 2);
        assert_eq!(region.num_boundary_edges(), 1);
        assert!((region.avg_energy() - 2.5).abs() < 0.01);
        assert!(region.contains(1));
        assert!(!region.contains(4));
    }

    #[test]
    fn test_comparison() {
        let mut isolated1 = HashSet::new();
        isolated1.insert(1);
        isolated1.insert(2);
        isolated1.insert(3);

        let result1 = IsolationResult {
            isolated_vertices: isolated1,
            cut_edges: vec![],
            cut_value: 0.0,
            num_high_energy_edges: 0,
            threshold: 1.0,
            is_verified: true,
        };

        let mut isolated2 = HashSet::new();
        isolated2.insert(2);
        isolated2.insert(3);
        isolated2.insert(4);

        let result2 = IsolationResult {
            isolated_vertices: isolated2,
            cut_edges: vec![],
            cut_value: 0.0,
            num_high_energy_edges: 0,
            threshold: 1.0,
            is_verified: true,
        };

        let comparison = IsolationComparison::compare(&result1, &result2);

        assert_eq!(comparison.common_isolated.len(), 2); // {2, 3}
        assert_eq!(comparison.only_first.len(), 1); // {1}
        assert_eq!(comparison.only_second.len(), 1); // {4}
        assert!(!comparison.is_identical());
        assert!(comparison.jaccard_similarity > 0.0 && comparison.jaccard_similarity < 1.0);
    }
}
