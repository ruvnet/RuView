//! Dynamic Connectivity for minimum cut wrapper
//!
//! Hybrid implementation using Euler Tour Trees with union-find fallback.
//! Provides O(log n) operations for insertions and queries.
//!
//! # Overview
//!
//! This module provides dynamic connectivity data structures:
//!
//! - [`DynamicConnectivity`]: Euler Tour Tree backend with union-find fallback
//!   - Edge insertions in O(log n) time
//!   - Edge deletions via full rebuild in O(m·α(n)) time
//!   - Connectivity queries in O(log n) time
//!
//! - [`PolylogConnectivity`]: Polylogarithmic worst-case connectivity (arXiv:2510.08297)
//!   - Edge insertions in O(log³ n) expected worst-case
//!   - Edge deletions in O(log³ n) expected worst-case
//!   - Connectivity queries in O(log n) worst-case
//!
//! # Implementation
//!
//! The primary backend uses Euler Tour Trees for O(log n) operations.
//! Falls back to union-find rebuild for deletions until full ETT cut is implemented.
//!
//! The polylog backend uses a hierarchy of O(log n) levels with edge sparsification
//! via low-congestion shortcuts for guaranteed worst-case bounds.

pub mod cache_opt;
pub mod polylog;

use crate::euler::EulerTourTree;
use crate::graph::VertexId;
use std::collections::{HashMap, HashSet};

/// Dynamic connectivity data structure with Euler Tour Tree backend
///
/// Maintains connected components of an undirected graph with support for
/// edge insertions and deletions. Uses Euler Tour Trees for O(log n) operations
/// with union-find fallback for robustness.
///
/// # Examples
///
/// ```ignore
/// let mut dc = DynamicConnectivity::new();
/// dc.add_vertex(0);
/// dc.add_vertex(1);
/// dc.add_vertex(2);
///
/// dc.insert_edge(0, 1);
/// assert!(dc.connected(0, 1));
/// assert!(!dc.connected(0, 2));
///
/// dc.insert_edge(1, 2);
/// assert!(dc.is_connected()); // All vertices connected
///
/// dc.delete_edge(1, 2);
/// assert!(!dc.connected(0, 2));
/// ```
#[derive(Debug, Clone)]
pub struct DynamicConnectivity {
    /// Union-find parent array
    parent: HashMap<VertexId, VertexId>,

    /// Union-find rank array for union by rank
    rank: HashMap<VertexId, usize>,

    /// Current edge set for rebuild on deletions
    /// Edges normalized so smaller vertex is always first
    edges: HashSet<(VertexId, VertexId)>,

    /// Number of vertices
    vertex_count: usize,

    /// Number of connected components
    component_count: usize,

    /// Euler Tour Tree for O(log n) operations
    ett: EulerTourTree,

    /// Whether ETT is in sync with union-find
    ett_synced: bool,
}

impl DynamicConnectivity {
    /// Creates a new empty dynamic connectivity structure
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let dc = DynamicConnectivity::new();
    /// assert_eq!(dc.component_count(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            parent: HashMap::new(),
            rank: HashMap::new(),
            edges: HashSet::new(),
            vertex_count: 0,
            component_count: 0,
            ett: EulerTourTree::new(),
            ett_synced: true,
        }
    }

    /// Adds a vertex to the connectivity structure
    ///
    /// If the vertex already exists, this is a no-op.
    /// Each new vertex starts in its own component.
    ///
    /// # Arguments
    ///
    /// * `v` - The vertex ID to add
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut dc = DynamicConnectivity::new();
    /// dc.add_vertex(0);
    /// assert_eq!(dc.component_count(), 1);
    /// ```
    pub fn add_vertex(&mut self, v: VertexId) {
        if !self.parent.contains_key(&v) {
            self.parent.insert(v, v);
            self.rank.insert(v, 0);
            self.vertex_count += 1;
            self.component_count += 1;

            // Add to Euler Tour Tree (O(log n))
            let _ = self.ett.make_tree(v);
        }
    }

    /// Inserts an edge between two vertices
    ///
    /// Automatically adds vertices if they don't exist.
    /// If vertices are already connected, updates internal state but
    /// doesn't change connectivity.
    ///
    /// # Arguments
    ///
    /// * `u` - First vertex
    /// * `v` - Second vertex
    ///
    /// # Time Complexity
    ///
    /// O(log n) via Euler Tour Tree link operation
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut dc = DynamicConnectivity::new();
    /// dc.insert_edge(0, 1);
    /// assert!(dc.connected(0, 1));
    /// ```
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId) {
        // Add vertices if they don't exist
        self.add_vertex(u);
        self.add_vertex(v);

        // Normalize edge (smaller vertex first)
        let edge = if u < v { (u, v) } else { (v, u) };

        // Add to edge set
        if self.edges.insert(edge) {
            // New edge - perform union
            let root_u = self.find(u);
            let root_v = self.find(v);

            if root_u != root_v {
                self.union(root_u, root_v);

                // Link in Euler Tour Tree (O(log n))
                let _ = self.ett.link(u, v);
            }
        }
    }

    /// Deletes an edge between two vertices
    ///
    /// Triggers a full rebuild of the data structure from the remaining edges.
    /// The ETT is also rebuilt to maintain O(log n) queries.
    ///
    /// # Arguments
    ///
    /// * `u` - First vertex
    /// * `v` - Second vertex
    ///
    /// # Time Complexity
    ///
    /// O(m·α(n)) where m is the number of edges (includes ETT rebuild)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut dc = DynamicConnectivity::new();
    /// dc.insert_edge(0, 1);
    /// dc.delete_edge(0, 1);
    /// assert!(!dc.connected(0, 1));
    /// ```
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) {
        // Normalize edge
        let edge = if u < v { (u, v) } else { (v, u) };

        // Remove from edge set
        if self.edges.remove(&edge) {
            // Mark ETT as out of sync
            self.ett_synced = false;

            // Rebuild the entire structure (including ETT)
            self.rebuild();
        }
    }

    /// Checks if the entire graph is connected (single component)
    ///
    /// # Returns
    ///
    /// `true` if all vertices are in a single connected component,
    /// `false` otherwise
    ///
    /// # Time Complexity
    ///
    /// O(1)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut dc = DynamicConnectivity::new();
    /// dc.add_vertex(0);
    /// dc.add_vertex(1);
    /// assert!(!dc.is_connected());
    ///
    /// dc.insert_edge(0, 1);
    /// assert!(dc.is_connected());
    /// ```
    pub fn is_connected(&self) -> bool {
        self.component_count == 1
    }

    /// Checks if two vertices are in the same connected component
    ///
    /// # Arguments
    ///
    /// * `u` - First vertex
    /// * `v` - Second vertex
    ///
    /// # Returns
    ///
    /// `true` if vertices are connected, `false` otherwise.
    /// Returns `false` if either vertex doesn't exist.
    ///
    /// # Time Complexity
    ///
    /// O(α(n)) amortized via union-find with path compression
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut dc = DynamicConnectivity::new();
    /// dc.insert_edge(0, 1);
    /// dc.insert_edge(1, 2);
    /// assert!(dc.connected(0, 2));
    /// ```
    pub fn connected(&mut self, u: VertexId, v: VertexId) -> bool {
        if !self.parent.contains_key(&u) || !self.parent.contains_key(&v) {
            return false;
        }

        // Use union-find with path compression (effectively O(1) amortized)
        // ETT is maintained for future subtree query optimizations
        self.find(u) == self.find(v)
    }

    /// Fast connectivity check using Euler Tour Tree (O(log n))
    ///
    /// Returns None if ETT is out of sync and result is unreliable.
    /// Use `connected()` for the reliable version.
    #[inline]
    pub fn connected_fast(&self, u: VertexId, v: VertexId) -> Option<bool> {
        if !self.ett_synced {
            return None;
        }
        Some(self.ett.connected(u, v))
    }

    /// Returns the number of connected components
    ///
    /// # Returns
    ///
    /// The current number of connected components
    pub fn component_count(&self) -> usize {
        self.component_count
    }

    /// Returns the number of vertices
    ///
    /// # Returns
    ///
    /// The current number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Finds the root of a vertex's component with path compression
    ///
    /// # Arguments
    ///
    /// * `v` - The vertex to find the root for
    ///
    /// # Returns
    ///
    /// The root vertex of the component containing `v`
    ///
    /// # Panics
    ///
    /// Panics if the vertex doesn't exist in the structure
    fn find(&mut self, v: VertexId) -> VertexId {
        let parent = *self.parent.get(&v).expect("Vertex not found");

        if parent != v {
            // Path compression: make v point directly to root
            let root = self.find(parent);
            self.parent.insert(v, root);
            root
        } else {
            v
        }
    }

    /// Unions two components by rank
    ///
    /// # Arguments
    ///
    /// * `u` - Root of first component
    /// * `v` - Root of second component
    ///
    /// # Notes
    ///
    /// This function assumes `u` and `v` are roots. It should only be
    /// called after `find()` operations.
    fn union(&mut self, u: VertexId, v: VertexId) {
        if u == v {
            return;
        }

        let rank_u = *self.rank.get(&u).unwrap_or(&0);
        let rank_v = *self.rank.get(&v).unwrap_or(&0);

        // Union by rank: attach smaller tree to larger tree
        if rank_u < rank_v {
            self.parent.insert(u, v);
        } else if rank_u > rank_v {
            self.parent.insert(v, u);
        } else {
            // Equal rank: arbitrary choice, increment rank
            self.parent.insert(v, u);
            self.rank.insert(u, rank_u + 1);
        }

        // Decrease component count
        self.component_count -= 1;
    }

    /// Rebuilds the union-find structure from the current edge set
    ///
    /// Called after edge deletions to recompute connected components.
    /// Resets all vertices to singleton components and re-applies all edges.
    /// Also rebuilds the Euler Tour Tree for O(log n) queries.
    ///
    /// # Time Complexity
    ///
    /// O(m·α(n)) where m is the number of edges
    fn rebuild(&mut self) {
        // Collect all vertices
        let vertices: Vec<VertexId> = self.parent.keys().copied().collect();

        // Reset to singleton components
        self.component_count = vertices.len();
        for &v in &vertices {
            self.parent.insert(v, v);
            self.rank.insert(v, 0);
        }

        // Rebuild Euler Tour Tree
        self.ett = EulerTourTree::new();
        for &v in &vertices {
            let _ = self.ett.make_tree(v);
        }

        // Re-apply all edges
        let edges: Vec<(VertexId, VertexId)> = self.edges.iter().copied().collect();
        for (u, v) in edges {
            let root_u = self.find(u);
            let root_v = self.find(v);

            if root_u != root_v {
                self.union(root_u, root_v);
                // Link in ETT
                let _ = self.ett.link(u, v);
            }
        }

        // Mark ETT as synced
        self.ett_synced = true;
    }
}

impl Default for DynamicConnectivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let dc = DynamicConnectivity::new();
        assert_eq!(dc.vertex_count(), 0);
        assert_eq!(dc.component_count(), 0);
    }

    #[test]
    fn test_add_vertex() {
        let mut dc = DynamicConnectivity::new();

        dc.add_vertex(0);
        assert_eq!(dc.vertex_count(), 1);
        assert_eq!(dc.component_count(), 1);

        dc.add_vertex(1);
        assert_eq!(dc.vertex_count(), 2);
        assert_eq!(dc.component_count(), 2);

        // Adding same vertex is no-op
        dc.add_vertex(0);
        assert_eq!(dc.vertex_count(), 2);
        assert_eq!(dc.component_count(), 2);
    }

    #[test]
    fn test_insert_edge_basic() {
        let mut dc = DynamicConnectivity::new();

        dc.insert_edge(0, 1);
        assert_eq!(dc.vertex_count(), 2);
        assert_eq!(dc.component_count(), 1);
        assert!(dc.connected(0, 1));
    }

    #[test]
    fn test_insert_edge_chain() {
        let mut dc = DynamicConnectivity::new();

        dc.insert_edge(0, 1);
        dc.insert_edge(1, 2);
        dc.insert_edge(2, 3);

        assert_eq!(dc.vertex_count(), 4);
        assert_eq!(dc.component_count(), 1);
        assert!(dc.connected(0, 3));
    }

    #[test]
    fn test_is_connected() {
        let mut dc = DynamicConnectivity::new();

        dc.add_vertex(0);
        dc.add_vertex(1);
        assert!(!dc.is_connected());

        dc.insert_edge(0, 1);
        assert!(dc.is_connected());

        dc.add_vertex(2);
        assert!(!dc.is_connected());

        dc.insert_edge(1, 2);
        assert!(dc.is_connected());
    }

    #[test]
    fn test_delete_edge() {
        let mut dc = DynamicConnectivity::new();

        dc.insert_edge(0, 1);
        dc.insert_edge(1, 2);
        assert!(dc.connected(0, 2));

        dc.delete_edge(1, 2);
        assert!(dc.connected(0, 1));
        assert!(!dc.connected(0, 2));
        assert_eq!(dc.component_count(), 2);
    }

    #[test]
    fn test_delete_edge_normalized() {
        let mut dc = DynamicConnectivity::new();

        dc.insert_edge(0, 1);
        assert!(dc.connected(0, 1));

        // Delete with reversed vertices
        dc.delete_edge(1, 0);
        assert!(!dc.connected(0, 1));
    }

    #[test]
    fn test_multiple_components() {
        let mut dc = DynamicConnectivity::new();

        // Component 1: 0-1-2
        dc.insert_edge(0, 1);
        dc.insert_edge(1, 2);

        // Component 2: 3-4
        dc.insert_edge(3, 4);

        // Isolated vertex
        dc.add_vertex(5);

        assert_eq!(dc.vertex_count(), 6);
        assert_eq!(dc.component_count(), 3);

        assert!(dc.connected(0, 2));
        assert!(dc.connected(3, 4));
        assert!(!dc.connected(0, 3));
        assert!(!dc.connected(0, 5));
    }

    #[test]
    fn test_path_compression() {
        let mut dc = DynamicConnectivity::new();

        // Create a long chain
        for i in 0..10 {
            dc.insert_edge(i, i + 1);
        }

        // Path compression should happen on find
        assert!(dc.connected(0, 10));

        // All vertices should now point closer to root
        let root = dc.find(0);
        for i in 0..=10 {
            assert_eq!(dc.find(i), root);
        }
    }

    #[test]
    fn test_union_by_rank() {
        let mut dc = DynamicConnectivity::new();

        // Create two trees of different sizes
        dc.insert_edge(0, 1);
        dc.insert_edge(0, 2);
        dc.insert_edge(0, 3);

        dc.insert_edge(4, 5);

        // Union them
        dc.insert_edge(0, 4);

        assert_eq!(dc.component_count(), 1);
        assert!(dc.connected(1, 5));
    }

    #[test]
    fn test_rebuild_after_multiple_deletions() {
        let mut dc = DynamicConnectivity::new();

        // Create a complete graph K4
        dc.insert_edge(0, 1);
        dc.insert_edge(0, 2);
        dc.insert_edge(0, 3);
        dc.insert_edge(1, 2);
        dc.insert_edge(1, 3);
        dc.insert_edge(2, 3);

        assert!(dc.is_connected());

        // Remove edges to disconnect
        dc.delete_edge(0, 1);
        dc.delete_edge(0, 2);
        dc.delete_edge(0, 3);

        assert!(!dc.is_connected());
        assert_eq!(dc.component_count(), 2);
        assert!(!dc.connected(0, 1));
        assert!(dc.connected(1, 2));
        assert!(dc.connected(1, 3));
    }

    #[test]
    fn test_connected_nonexistent_vertex() {
        let mut dc = DynamicConnectivity::new();

        dc.add_vertex(0);
        assert!(!dc.connected(0, 999));
        assert!(!dc.connected(999, 0));
    }

    #[test]
    fn test_self_loop() {
        let mut dc = DynamicConnectivity::new();

        dc.insert_edge(0, 0);
        assert_eq!(dc.vertex_count(), 1);
        assert_eq!(dc.component_count(), 1);
        assert!(dc.connected(0, 0));
    }

    #[test]
    fn test_duplicate_edges() {
        let mut dc = DynamicConnectivity::new();

        dc.insert_edge(0, 1);
        dc.insert_edge(0, 1); // Duplicate
        dc.insert_edge(1, 0); // Duplicate (reversed)

        assert_eq!(dc.vertex_count(), 2);
        assert_eq!(dc.component_count(), 1);
    }
}
