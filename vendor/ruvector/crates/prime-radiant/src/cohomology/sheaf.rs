//! Sheaf Data Structure
//!
//! A sheaf on a graph assigns:
//! - A vector space (stalk) to each vertex
//! - Restriction maps between adjacent stalks
//!
//! This is the foundational structure for cohomology computation.

use crate::substrate::NodeId;
use crate::substrate::{RestrictionMap, SheafGraph};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A stalk (fiber) at a vertex - the local data space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stalk {
    /// Dimension of the stalk (vector space dimension)
    pub dimension: usize,
    /// Optional basis vectors (if not standard basis)
    pub basis: Option<Array2<f64>>,
}

impl Stalk {
    /// Create a stalk of given dimension with standard basis
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            basis: None,
        }
    }

    /// Create a stalk with a custom basis
    pub fn with_basis(dimension: usize, basis: Array2<f64>) -> Self {
        assert_eq!(basis.ncols(), dimension, "Basis dimension mismatch");
        Self {
            dimension,
            basis: Some(basis),
        }
    }

    /// Get dimension
    pub fn dim(&self) -> usize {
        self.dimension
    }
}

/// A local section assigns a value in the stalk at each vertex
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSection {
    /// Vertex ID
    pub vertex: NodeId,
    /// Value in the stalk (as a vector)
    pub value: Array1<f64>,
}

impl LocalSection {
    /// Create a new local section
    pub fn new(vertex: NodeId, value: Array1<f64>) -> Self {
        Self { vertex, value }
    }

    /// Create from f32 slice
    pub fn from_slice(vertex: NodeId, data: &[f32]) -> Self {
        let value = Array1::from_iter(data.iter().map(|&x| x as f64));
        Self { vertex, value }
    }

    /// Get dimension
    pub fn dim(&self) -> usize {
        self.value.len()
    }
}

/// A sheaf section is a collection of local sections that are compatible
/// under restriction maps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafSection {
    /// Local sections indexed by vertex
    pub sections: HashMap<NodeId, Array1<f64>>,
    /// Whether this is a global section (fully consistent)
    pub is_global: bool,
}

impl SheafSection {
    /// Create an empty section
    pub fn empty() -> Self {
        Self {
            sections: HashMap::new(),
            is_global: false,
        }
    }

    /// Create a section from local data
    pub fn from_local(sections: HashMap<NodeId, Array1<f64>>) -> Self {
        Self {
            sections,
            is_global: false,
        }
    }

    /// Get the value at a vertex
    pub fn get(&self, vertex: NodeId) -> Option<&Array1<f64>> {
        self.sections.get(&vertex)
    }

    /// Set the value at a vertex
    pub fn set(&mut self, vertex: NodeId, value: Array1<f64>) {
        self.sections.insert(vertex, value);
        self.is_global = false; // Need to recheck
    }

    /// Check if a vertex is in the section's domain
    pub fn contains(&self, vertex: NodeId) -> bool {
        self.sections.contains_key(&vertex)
    }

    /// Number of vertices with assigned values
    pub fn support_size(&self) -> usize {
        self.sections.len()
    }
}

/// Type alias for restriction map function
pub type RestrictionFn = Arc<dyn Fn(&Array1<f64>) -> Array1<f64> + Send + Sync>;

/// A sheaf on a graph
///
/// Assigns stalks to vertices and restriction maps to edges
#[derive(Clone)]
pub struct Sheaf {
    /// Stalks at each vertex
    pub stalks: HashMap<NodeId, Stalk>,
    /// Restriction maps indexed by (source, target) pairs
    /// The map rho_{u->v} restricts from stalk at u to edge space
    restriction_maps: HashMap<(NodeId, NodeId), RestrictionFn>,
    /// Cached dimensions for performance
    stalk_dims: HashMap<NodeId, usize>,
    /// Total dimension (sum of all stalk dimensions)
    total_dim: usize,
}

impl Sheaf {
    /// Create a new empty sheaf
    pub fn new() -> Self {
        Self {
            stalks: HashMap::new(),
            restriction_maps: HashMap::new(),
            stalk_dims: HashMap::new(),
            total_dim: 0,
        }
    }

    /// Build a sheaf from a SheafGraph
    ///
    /// Uses the graph's state vectors as stalks and restriction maps from edges
    pub fn from_graph(graph: &SheafGraph) -> Self {
        let mut sheaf = Self::new();

        // Add stalks from nodes
        for node_id in graph.node_ids() {
            if let Some(node) = graph.get_node(node_id) {
                let dim = node.state.dim();
                sheaf.add_stalk(node_id, Stalk::new(dim));
            }
        }

        // Add restriction maps from edges
        for edge_id in graph.edge_ids() {
            if let Some(edge) = graph.get_edge(edge_id) {
                let source = edge.source;
                let target = edge.target;

                // Create restriction functions from the edge's restriction maps
                let source_rho = edge.rho_source.clone();
                let target_rho = edge.rho_target.clone();

                // Source restriction map
                let source_fn: RestrictionFn = Arc::new(move |v: &Array1<f64>| {
                    let input: Vec<f32> = v.iter().map(|&x| x as f32).collect();
                    let output = source_rho.apply(&input);
                    Array1::from_iter(output.iter().map(|&x| x as f64))
                });

                // Target restriction map
                let target_fn: RestrictionFn = Arc::new(move |v: &Array1<f64>| {
                    let input: Vec<f32> = v.iter().map(|&x| x as f32).collect();
                    let output = target_rho.apply(&input);
                    Array1::from_iter(output.iter().map(|&x| x as f64))
                });

                sheaf.add_restriction(source, target, source_fn.clone());
                sheaf.add_restriction(target, source, target_fn);
            }
        }

        sheaf
    }

    /// Add a stalk at a vertex
    pub fn add_stalk(&mut self, vertex: NodeId, stalk: Stalk) {
        let dim = stalk.dimension;
        self.stalks.insert(vertex, stalk);
        self.stalk_dims.insert(vertex, dim);
        self.total_dim = self.stalk_dims.values().sum();
    }

    /// Add a restriction map
    pub fn add_restriction(&mut self, source: NodeId, target: NodeId, map: RestrictionFn) {
        self.restriction_maps.insert((source, target), map);
    }

    /// Get the stalk at a vertex
    pub fn get_stalk(&self, vertex: NodeId) -> Option<&Stalk> {
        self.stalks.get(&vertex)
    }

    /// Get stalk dimension
    pub fn stalk_dim(&self, vertex: NodeId) -> Option<usize> {
        self.stalk_dims.get(&vertex).copied()
    }

    /// Apply restriction map from source to target
    pub fn restrict(
        &self,
        source: NodeId,
        target: NodeId,
        value: &Array1<f64>,
    ) -> Option<Array1<f64>> {
        self.restriction_maps
            .get(&(source, target))
            .map(|rho| rho(value))
    }

    /// Check if a section is globally consistent
    ///
    /// A section is consistent if for every edge (u,v):
    /// rho_u(s(u)) = rho_v(s(v))
    pub fn is_consistent(&self, section: &SheafSection, tolerance: f64) -> bool {
        for &(source, target) in self.restriction_maps.keys() {
            if let (Some(s_val), Some(t_val)) = (section.get(source), section.get(target)) {
                let s_restricted = self.restrict(source, target, s_val);
                let t_restricted = self.restrict(target, source, t_val);

                if let (Some(s_r), Some(t_r)) = (s_restricted, t_restricted) {
                    let diff = &s_r - &t_r;
                    let norm: f64 = diff.iter().map(|x| x * x).sum::<f64>().sqrt();
                    if norm > tolerance {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Compute residual (inconsistency) at an edge
    pub fn edge_residual(
        &self,
        source: NodeId,
        target: NodeId,
        section: &SheafSection,
    ) -> Option<Array1<f64>> {
        let s_val = section.get(source)?;
        let t_val = section.get(target)?;

        let s_restricted = self.restrict(source, target, s_val)?;
        let t_restricted = self.restrict(target, source, t_val)?;

        Some(&s_restricted - &t_restricted)
    }

    /// Total dimension of the sheaf
    pub fn total_dimension(&self) -> usize {
        self.total_dim
    }

    /// Number of vertices
    pub fn num_vertices(&self) -> usize {
        self.stalks.len()
    }

    /// Iterator over vertices
    pub fn vertices(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.stalks.keys().copied()
    }
}

impl Default for Sheaf {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Sheaf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheaf")
            .field("num_vertices", &self.stalks.len())
            .field("num_restrictions", &self.restriction_maps.len())
            .field("total_dimension", &self.total_dim)
            .finish()
    }
}

/// Builder for constructing sheaves
pub struct SheafBuilder {
    sheaf: Sheaf,
}

impl SheafBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            sheaf: Sheaf::new(),
        }
    }

    /// Add a stalk at a vertex
    pub fn stalk(mut self, vertex: NodeId, dimension: usize) -> Self {
        self.sheaf.add_stalk(vertex, Stalk::new(dimension));
        self
    }

    /// Add an identity restriction between vertices
    pub fn identity_restriction(mut self, source: NodeId, target: NodeId) -> Self {
        let identity: RestrictionFn = Arc::new(|v: &Array1<f64>| v.clone());
        self.sheaf.add_restriction(source, target, identity);
        self
    }

    /// Add a scaling restriction
    pub fn scaling_restriction(mut self, source: NodeId, target: NodeId, scale: f64) -> Self {
        let scale_fn: RestrictionFn = Arc::new(move |v: &Array1<f64>| v * scale);
        self.sheaf.add_restriction(source, target, scale_fn);
        self
    }

    /// Add a projection restriction (select certain dimensions)
    pub fn projection_restriction(
        mut self,
        source: NodeId,
        target: NodeId,
        indices: Vec<usize>,
    ) -> Self {
        let proj_fn: RestrictionFn =
            Arc::new(move |v: &Array1<f64>| Array1::from_iter(indices.iter().map(|&i| v[i])));
        self.sheaf.add_restriction(source, target, proj_fn);
        self
    }

    /// Add a linear restriction with a matrix
    pub fn linear_restriction(
        mut self,
        source: NodeId,
        target: NodeId,
        matrix: Array2<f64>,
    ) -> Self {
        let linear_fn: RestrictionFn = Arc::new(move |v: &Array1<f64>| matrix.dot(v));
        self.sheaf.add_restriction(source, target, linear_fn);
        self
    }

    /// Build the sheaf
    pub fn build(self) -> Sheaf {
        self.sheaf
    }
}

impl Default for SheafBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        Uuid::new_v4()
    }

    #[test]
    fn test_sheaf_creation() {
        let v0 = make_node_id();
        let v1 = make_node_id();

        let sheaf = SheafBuilder::new()
            .stalk(v0, 3)
            .stalk(v1, 3)
            .identity_restriction(v0, v1)
            .identity_restriction(v1, v0)
            .build();

        assert_eq!(sheaf.num_vertices(), 2);
        assert_eq!(sheaf.total_dimension(), 6);
    }

    #[test]
    fn test_consistent_section() {
        let v0 = make_node_id();
        let v1 = make_node_id();

        let sheaf = SheafBuilder::new()
            .stalk(v0, 2)
            .stalk(v1, 2)
            .identity_restriction(v0, v1)
            .identity_restriction(v1, v0)
            .build();

        // Consistent section: same value at both vertices
        let mut section = SheafSection::empty();
        section.set(v0, Array1::from_vec(vec![1.0, 2.0]));
        section.set(v1, Array1::from_vec(vec![1.0, 2.0]));

        assert!(sheaf.is_consistent(&section, 1e-10));
    }

    #[test]
    fn test_inconsistent_section() {
        let v0 = make_node_id();
        let v1 = make_node_id();

        let sheaf = SheafBuilder::new()
            .stalk(v0, 2)
            .stalk(v1, 2)
            .identity_restriction(v0, v1)
            .identity_restriction(v1, v0)
            .build();

        // Inconsistent section: different values
        let mut section = SheafSection::empty();
        section.set(v0, Array1::from_vec(vec![1.0, 2.0]));
        section.set(v1, Array1::from_vec(vec![3.0, 4.0]));

        assert!(!sheaf.is_consistent(&section, 1e-10));
    }

    #[test]
    fn test_edge_residual() {
        let v0 = make_node_id();
        let v1 = make_node_id();

        let sheaf = SheafBuilder::new()
            .stalk(v0, 2)
            .stalk(v1, 2)
            .identity_restriction(v0, v1)
            .identity_restriction(v1, v0)
            .build();

        let mut section = SheafSection::empty();
        section.set(v0, Array1::from_vec(vec![1.0, 2.0]));
        section.set(v1, Array1::from_vec(vec![1.5, 2.5]));

        let residual = sheaf.edge_residual(v0, v1, &section).unwrap();

        // Residual should be [1.0, 2.0] - [1.5, 2.5] = [-0.5, -0.5]
        assert!((residual[0] - (-0.5)).abs() < 1e-10);
        assert!((residual[1] - (-0.5)).abs() < 1e-10);
    }
}
