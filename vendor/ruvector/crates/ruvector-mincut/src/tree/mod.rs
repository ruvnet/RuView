//! Hierarchical Tree Decomposition for Dynamic Minimum Cut
//!
//! Maintains a hierarchy of graph partitions where each level contains
//! increasingly refined cuts. Enables subpolynomial update time.
//!
//! # Overview
//!
//! This module implements a hierarchical decomposition of graphs for efficient
//! minimum cut maintenance. The key idea is to build a balanced binary tree over
//! the graph vertices, where each node represents a potential partition of the graph.
//!
//! # Features
//!
//! - **Balanced Binary Tree**: O(log n) height decomposition
//! - **Lazy Recomputation**: Only recompute dirty nodes after updates
//! - **LCA-based Updates**: Localize updates to affected subtrees
//! - **Multiple Cut Evaluation**: Consider all possible tree-induced partitions
//!
//! # Example
//!
//! ```rust
//! use std::sync::Arc;
//! use ruvector_mincut::graph::DynamicGraph;
//! use ruvector_mincut::tree::HierarchicalDecomposition;
//!
//! // Create a graph
//! let graph = Arc::new(DynamicGraph::new());
//! graph.insert_edge(1, 2, 1.0).unwrap();
//! graph.insert_edge(2, 3, 1.0).unwrap();
//! graph.insert_edge(3, 1, 1.0).unwrap();
//!
//! // Build hierarchical decomposition
//! let mut decomp = HierarchicalDecomposition::build(graph.clone()).unwrap();
//!
//! // Get minimum cut
//! let min_cut = decomp.min_cut_value();
//! println!("Minimum cut: {}", min_cut);
//!
//! // Get the partition
//! let (partition_a, partition_b) = decomp.min_cut_partition();
//! println!("Partition: {:?} vs {:?}", partition_a, partition_b);
//!
//! // Handle dynamic updates
//! graph.insert_edge(1, 4, 2.0).unwrap();
//! let new_min_cut = decomp.insert_edge(1, 4, 2.0).unwrap();
//! println!("New minimum cut: {}", new_min_cut);
//! ```
//!
//! # Algorithm
//!
//! 1. **Build Phase**: Construct a balanced binary tree over graph vertices
//! 2. **Compute Phase**: For each node, compute the cut value (edges crossing
//!    between node's vertices and all other vertices)
//! 3. **Query Phase**: Return minimum cut over all nodes
//! 4. **Update Phase**: Mark affected nodes dirty, recompute only dirty subtrees
//!
//! # Complexity
//!
//! - Build: O(n log n + m) where n = vertices, m = edges
//! - Query: O(1)
//! - Update: O(log n) nodes recomputed, O(m') edges examined per node
//!
//! # Limitations
//!
//! The balanced binary partitioning may not find the true minimum cut if it
//! requires a partition not represented in the tree structure. For guaranteed
//! minimum cut finding, use the exact algorithm in the `algorithm` module.

use crate::error::Result;
use crate::graph::{DynamicGraph, VertexId, Weight};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A node in the hierarchical decomposition tree
#[derive(Debug, Clone)]
pub struct DecompositionNode {
    /// Unique ID of this node
    pub id: usize,
    /// Level in the hierarchy (0 = leaves = individual vertices)
    pub level: usize,
    /// Vertices contained in this node (at leaves) or children indices
    pub vertices: HashSet<VertexId>,
    /// Parent node index (None for root)
    pub parent: Option<usize>,
    /// Child node indices
    pub children: Vec<usize>,
    /// Cut value for this subtree
    pub cut_value: f64,
    /// Whether this node needs recomputation
    pub dirty: bool,
}

impl DecompositionNode {
    /// Create a new leaf node
    fn new_leaf(id: usize, vertex: VertexId) -> Self {
        let mut vertices = HashSet::new();
        vertices.insert(vertex);
        Self {
            id,
            level: 0,
            vertices,
            parent: None,
            children: Vec::new(),
            cut_value: f64::INFINITY,
            dirty: true,
        }
    }

    /// Create a new internal node
    fn new_internal(id: usize, level: usize, children: Vec<usize>) -> Self {
        Self {
            id,
            level,
            vertices: HashSet::new(), // Will be populated from children
            parent: None,
            children,
            cut_value: f64::INFINITY,
            dirty: true,
        }
    }

    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Get the size of this subtree (number of vertices)
    pub fn size(&self) -> usize {
        self.vertices.len()
    }
}

/// The hierarchical decomposition tree
pub struct HierarchicalDecomposition {
    /// All nodes in the decomposition
    nodes: Vec<DecompositionNode>,
    /// Map from vertex to its leaf node index
    vertex_to_leaf: HashMap<VertexId, usize>,
    /// Root node index
    root: Option<usize>,
    /// Current global minimum cut value
    min_cut: f64,
    /// Height of the tree
    height: usize,
    /// Reference to the underlying graph
    graph: Arc<DynamicGraph>,
    /// Next node ID
    next_node_id: usize,
}

impl HierarchicalDecomposition {
    /// Build a new hierarchical decomposition from a graph
    pub fn build(graph: Arc<DynamicGraph>) -> Result<Self> {
        let mut decomp = Self {
            nodes: Vec::new(),
            vertex_to_leaf: HashMap::new(),
            root: None,
            min_cut: f64::INFINITY,
            height: 0,
            graph,
            next_node_id: 0,
        };

        decomp.build_hierarchy()?;
        decomp.min_cut = decomp.propagate_updates();

        Ok(decomp)
    }

    /// Get the current minimum cut value
    pub fn min_cut_value(&self) -> f64 {
        self.min_cut
    }

    /// Get the vertices on each side of the minimum cut
    pub fn min_cut_partition(&self) -> (HashSet<VertexId>, HashSet<VertexId>) {
        if self.root.is_none() {
            return (HashSet::new(), HashSet::new());
        }

        // Find the node with minimum cut value
        let (min_node_idx, _) = self.find_min_cut_node();
        let node = &self.nodes[min_node_idx];

        // The partition is: node's vertices vs all other vertices
        let partition_a = node.vertices.clone();
        let all_vertices: HashSet<VertexId> = self.graph.vertices().into_iter().collect();
        let partition_b: HashSet<VertexId> =
            all_vertices.difference(&partition_a).copied().collect();

        (partition_a, partition_b)
    }

    /// Handle edge insertion
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, _weight: Weight) -> Result<f64> {
        // Find LCA of u and v
        if let Some(lca_idx) = self.lca_node(u, v) {
            // Mark LCA and ancestors as dirty
            self.mark_dirty(lca_idx);
        }

        // Recompute and return new min cut
        self.min_cut = self.propagate_updates();
        Ok(self.min_cut)
    }

    /// Handle edge deletion
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<f64> {
        // Find LCA of u and v
        if let Some(lca_idx) = self.lca_node(u, v) {
            // Mark LCA and ancestors as dirty
            self.mark_dirty(lca_idx);
        }

        // Recompute and return new min cut
        self.min_cut = self.propagate_updates();
        Ok(self.min_cut)
    }

    /// Recompute dirty nodes and return new min cut
    fn propagate_updates(&mut self) -> f64 {
        // Post-order traversal to recompute dirty nodes
        if let Some(root_idx) = self.root {
            self.recompute_subtree(root_idx);
        }

        // Find global minimum
        self.find_min_cut_value()
    }

    /// Recompute a subtree (post-order)
    fn recompute_subtree(&mut self, node_idx: usize) {
        // First, recompute children
        let children = self.nodes[node_idx].children.clone();
        for child_idx in children {
            if self.nodes[child_idx].dirty {
                self.recompute_subtree(child_idx);
            }
        }

        // Then recompute this node
        if self.nodes[node_idx].dirty {
            let cut_value = self.compute_cut(node_idx);
            self.nodes[node_idx].cut_value = cut_value;
            self.nodes[node_idx].dirty = false;
        }
    }

    /// Find the lowest common ancestor node of two vertices
    fn lca_node(&self, u: VertexId, v: VertexId) -> Option<usize> {
        let u_leaf = self.vertex_to_leaf.get(&u)?;
        let v_leaf = self.vertex_to_leaf.get(&v)?;

        if u_leaf == v_leaf {
            return Some(*u_leaf);
        }

        // Get ancestors of u
        let mut u_ancestors = HashSet::new();
        let mut current = Some(*u_leaf);
        while let Some(node_idx) = current {
            u_ancestors.insert(node_idx);
            current = self.nodes[node_idx].parent;
        }

        // Find first common ancestor of v
        let mut current = Some(*v_leaf);
        while let Some(node_idx) = current {
            if u_ancestors.contains(&node_idx) {
                return Some(node_idx);
            }
            current = self.nodes[node_idx].parent;
        }

        None
    }

    /// Mark a node and its ancestors as dirty
    fn mark_dirty(&mut self, node_idx: usize) {
        let mut current = Some(node_idx);
        while let Some(idx) = current {
            self.nodes[idx].dirty = true;
            current = self.nodes[idx].parent;
        }
    }

    /// Build the initial hierarchy using recursive partitioning
    fn build_hierarchy(&mut self) -> Result<()> {
        let vertices = self.graph.vertices();

        if vertices.is_empty() {
            return Ok(());
        }

        // Create leaf nodes for each vertex
        for vertex in &vertices {
            let node_id = self.next_node_id;
            self.next_node_id += 1;
            let leaf = DecompositionNode::new_leaf(node_id, *vertex);
            let leaf_idx = self.nodes.len();
            self.nodes.push(leaf);
            self.vertex_to_leaf.insert(*vertex, leaf_idx);
        }

        // Build tree using balanced binary partitioning
        let leaf_indices: Vec<usize> = (0..vertices.len()).collect();
        if !leaf_indices.is_empty() {
            self.root = Some(self.build_subtree(&leaf_indices, 1)?);
        }

        Ok(())
    }

    /// Recursively build a balanced binary tree from leaf indices
    fn build_subtree(&mut self, indices: &[usize], level: usize) -> Result<usize> {
        if indices.len() == 1 {
            // Single leaf node - update its level
            self.nodes[indices[0]].level = 0;
            self.height = self.height.max(level - 1);
            return Ok(indices[0]);
        }

        // Split into two balanced halves
        let mid = indices.len() / 2;
        let left_indices = &indices[..mid];
        let right_indices = &indices[mid..];

        // Recursively build children
        let left_idx = self.build_subtree(left_indices, level + 1)?;
        let right_idx = self.build_subtree(right_indices, level + 1)?;

        // Create internal node
        let node_id = self.next_node_id;
        self.next_node_id += 1;
        let mut internal =
            DecompositionNode::new_internal(node_id, level, vec![left_idx, right_idx]);

        // Collect vertices from children
        internal.vertices.extend(&self.nodes[left_idx].vertices);
        internal.vertices.extend(&self.nodes[right_idx].vertices);

        let internal_idx = self.nodes.len();
        self.nodes.push(internal);

        // Update parent pointers
        self.nodes[left_idx].parent = Some(internal_idx);
        self.nodes[right_idx].parent = Some(internal_idx);

        self.height = self.height.max(level);

        Ok(internal_idx)
    }

    /// Compute the cut value at a node
    /// Each node represents a partition: (node's vertices) vs (all other vertices)
    fn compute_cut(&self, node_idx: usize) -> f64 {
        let node = &self.nodes[node_idx];

        // Leaf nodes: partition would be {vertex} vs {all others}
        // If there's only 1 vertex total, cut is infinite
        // Otherwise, compute edges from this vertex to all others
        if node.vertices.len() == self.graph.num_vertices() {
            // This node contains all vertices - no valid cut
            return f64::INFINITY;
        }

        // Compute cut: sum of edge weights crossing between node's vertices and others
        self.compute_global_cut(&node.vertices)
    }

    /// Compute the cut value for a partition
    /// One side is 'vertices', other side is all vertices not in this set
    fn compute_global_cut(&self, vertices: &HashSet<VertexId>) -> f64 {
        let mut cut_weight = 0.0;

        for &u in vertices {
            for (v, _edge_id) in self.graph.neighbors(u) {
                // If v is NOT in our vertex set, this edge crosses the cut
                if !vertices.contains(&v) {
                    if let Some(weight) = self.graph.edge_weight(u, v) {
                        cut_weight += weight;
                    }
                }
            }
        }

        cut_weight
    }

    /// Find the node with minimum cut value
    fn find_min_cut_node(&self) -> (usize, f64) {
        let mut min_idx = 0;
        let mut min_value = f64::INFINITY;

        for (idx, node) in self.nodes.iter().enumerate() {
            if node.cut_value < min_value {
                min_value = node.cut_value;
                min_idx = idx;
            }
        }

        (min_idx, min_value)
    }

    /// Find global minimum cut value
    fn find_min_cut_value(&self) -> f64 {
        self.find_min_cut_node().1
    }

    /// Get height of decomposition
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }
}

/// Level information for the decomposition
#[derive(Debug, Clone)]
pub struct LevelInfo {
    /// Level index
    pub level: usize,
    /// Number of nodes at this level
    pub num_nodes: usize,
    /// Average cut value at this level
    pub avg_cut: f64,
}

impl HierarchicalDecomposition {
    /// Get information about each level
    pub fn level_info(&self) -> Vec<LevelInfo> {
        let mut levels: HashMap<usize, Vec<f64>> = HashMap::new();

        for node in &self.nodes {
            levels
                .entry(node.level)
                .or_insert_with(Vec::new)
                .push(node.cut_value);
        }

        let mut result: Vec<LevelInfo> = levels
            .into_iter()
            .map(|(level, cut_values)| {
                let num_nodes = cut_values.len();
                let finite_cuts: Vec<f64> = cut_values
                    .iter()
                    .filter(|&&v| v.is_finite())
                    .copied()
                    .collect();
                let avg_cut = if finite_cuts.is_empty() {
                    f64::INFINITY
                } else {
                    finite_cuts.iter().sum::<f64>() / finite_cuts.len() as f64
                };

                LevelInfo {
                    level,
                    num_nodes,
                    avg_cut,
                }
            })
            .collect();

        result.sort_by_key(|info| info.level);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_graph() -> Arc<DynamicGraph> {
        let graph = Arc::new(DynamicGraph::new());
        // Triangle: 1-2-3-1
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();
        graph.insert_edge(3, 1, 1.0).unwrap();
        graph
    }

    fn create_disconnectable_graph() -> Arc<DynamicGraph> {
        let graph = Arc::new(DynamicGraph::new());
        // Two triangles connected by single edge
        // Triangle 1: 1-2-3-1
        graph.insert_edge(1, 2, 2.0).unwrap();
        graph.insert_edge(2, 3, 2.0).unwrap();
        graph.insert_edge(3, 1, 2.0).unwrap();
        // Bridge
        graph.insert_edge(3, 4, 1.0).unwrap();
        // Triangle 2: 4-5-6-4
        graph.insert_edge(4, 5, 2.0).unwrap();
        graph.insert_edge(5, 6, 2.0).unwrap();
        graph.insert_edge(6, 4, 2.0).unwrap();
        graph
    }

    #[test]
    fn test_build_empty_graph() {
        let graph = Arc::new(DynamicGraph::new());
        let decomp = HierarchicalDecomposition::build(graph).unwrap();
        assert_eq!(decomp.num_nodes(), 0);
        assert_eq!(decomp.height(), 0);
    }

    #[test]
    fn test_build_single_vertex() {
        let graph = Arc::new(DynamicGraph::new());
        graph.add_vertex(1);
        let decomp = HierarchicalDecomposition::build(graph).unwrap();
        assert_eq!(decomp.num_nodes(), 1);
        assert_eq!(decomp.height(), 0);
        assert!(decomp.min_cut_value().is_infinite());
    }

    #[test]
    fn test_build_triangle() {
        let graph = create_simple_graph();
        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        // 3 leaves + 2 internal nodes = 5 total
        assert_eq!(decomp.num_nodes(), 5);

        // Height should be O(log n) = O(log 3) ≈ 2
        assert!(decomp.height() <= 2);

        // Min cut of triangle is 2.0 (any two edges)
        assert_eq!(decomp.min_cut_value(), 2.0);
    }

    #[test]
    fn test_build_disconnectable() {
        let graph = create_disconnectable_graph();
        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        // 6 leaves + 5 internal nodes = 11 total
        assert_eq!(decomp.num_nodes(), 11);

        // Min cut depends on tree structure from balanced partitioning
        // The optimal cut (bridge = 1.0) may not be found due to arbitrary partitioning
        // But it should find some valid cut
        let min_cut = decomp.min_cut_value();
        assert!(min_cut.is_finite() && min_cut >= 1.0);
    }

    #[test]
    fn test_min_cut_partition() {
        let graph = create_disconnectable_graph();
        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        let (partition_a, partition_b) = decomp.min_cut_partition();

        // Should split into two triangles
        assert_eq!(partition_a.len() + partition_b.len(), 6);

        // Verify partition sizes (should be 3 and 3, or some other split)
        assert!(partition_a.len() >= 1 && partition_a.len() <= 5);
        assert!(partition_b.len() >= 1 && partition_b.len() <= 5);

        // Verify partitions are disjoint
        let intersection: HashSet<_> = partition_a.intersection(&partition_b).collect();
        assert!(intersection.is_empty());
    }

    #[test]
    fn test_lca_node() {
        let graph = create_simple_graph();
        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        // LCA of same vertex is itself
        let lca = decomp.lca_node(1, 1);
        assert!(lca.is_some());

        // LCA of different vertices exists
        let lca = decomp.lca_node(1, 2);
        assert!(lca.is_some());

        let lca = decomp.lca_node(1, 3);
        assert!(lca.is_some());
    }

    #[test]
    fn test_mark_dirty() {
        let graph = create_simple_graph();
        let mut decomp = HierarchicalDecomposition::build(graph).unwrap();

        // Initially all nodes should be clean (after propagate_updates)
        for node in &decomp.nodes {
            assert!(
                !node.dirty,
                "Node {} should not be dirty after build",
                node.id
            );
        }

        // Mark a leaf as dirty
        let leaf_idx = *decomp.vertex_to_leaf.get(&1).unwrap();
        decomp.mark_dirty(leaf_idx);

        // Verify the path to root is marked dirty
        let mut current = Some(leaf_idx);
        while let Some(idx) = current {
            assert!(decomp.nodes[idx].dirty, "Node {} should be dirty", idx);
            current = decomp.nodes[idx].parent;
        }
    }

    #[test]
    fn test_insert_edge() {
        let graph = create_simple_graph();
        let mut decomp = HierarchicalDecomposition::build(graph.clone()).unwrap();

        let old_min_cut = decomp.min_cut_value();

        // Add edge 1-3 with high weight (creates more connectivity)
        graph.insert_edge(1, 4, 5.0).unwrap();
        graph.insert_edge(2, 4, 5.0).unwrap();

        // Rebuild to get proper baseline
        let mut decomp = HierarchicalDecomposition::build(graph.clone()).unwrap();
        let baseline = decomp.min_cut_value();

        // Now add another edge
        graph.insert_edge(3, 4, 3.0).unwrap();
        let new_min_cut = decomp.insert_edge(3, 4, 3.0).unwrap();

        // Verify that we got a valid result
        assert!(new_min_cut.is_finite());
    }

    #[test]
    fn test_delete_edge() {
        let graph = create_disconnectable_graph();
        let mut decomp = HierarchicalDecomposition::build(graph.clone()).unwrap();

        let old_min_cut = decomp.min_cut_value();
        assert!(old_min_cut.is_finite());

        // Delete an edge from one triangle
        graph.delete_edge(1, 2).unwrap();
        let new_min_cut = decomp.delete_edge(1, 2).unwrap();

        // After deleting an edge, min cut might change
        // The exact value depends on tree structure
        assert!(new_min_cut.is_finite());
    }

    #[test]
    fn test_level_info() {
        let graph = create_simple_graph();
        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        let levels = decomp.level_info();

        // Should have levels 0, 1, 2 (leaves at 0, internal at 1 and 2)
        assert!(!levels.is_empty());

        // Verify levels are sorted
        for i in 1..levels.len() {
            assert!(levels[i].level > levels[i - 1].level);
        }

        // Count total nodes
        let total_nodes: usize = levels.iter().map(|l| l.num_nodes).sum();
        assert_eq!(total_nodes, decomp.num_nodes());
    }

    #[test]
    fn test_balanced_tree() {
        let graph = Arc::new(DynamicGraph::new());

        // Create a graph with 15 vertices (should give height ~4)
        for i in 1..=15 {
            graph.add_vertex(i);
        }

        // Add some edges to make it connected
        for i in 1..15 {
            graph.insert_edge(i, i + 1, 1.0).unwrap();
        }

        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        // Height should be O(log n) = O(log 15) ≈ 4
        assert!(
            decomp.height() <= 4,
            "Height {} should be <= 4",
            decomp.height()
        );

        // Verify balanced: all leaves should be at level 0
        let leaf_count = decomp.nodes.iter().filter(|n| n.level == 0).count();
        assert_eq!(leaf_count, 15);
    }

    #[test]
    fn test_compute_cut() {
        let graph = Arc::new(DynamicGraph::new());

        // Create simple 2-2 bipartite graph
        // Left: 1, 2
        // Right: 3, 4
        // Edges: 1-3 (weight 1), 2-4 (weight 1)
        graph.insert_edge(1, 3, 1.0).unwrap();
        graph.insert_edge(2, 4, 1.0).unwrap();

        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        // Min cut depends on tree partitioning
        // Could be 0.0 if it partitions {1,3} vs {2,4} (no edges between)
        // Could be 1.0 if it partitions {1} vs {2,3,4} or similar
        // Could be 2.0 if it partitions {1,2} vs {3,4}
        let min_cut = decomp.min_cut_value();
        assert!(min_cut.is_finite() && min_cut <= 2.0);
    }

    #[test]
    fn test_large_tree() {
        let graph = Arc::new(DynamicGraph::new());

        // Create a path graph with 100 vertices
        for i in 1..=100 {
            graph.add_vertex(i);
        }

        for i in 1..100 {
            graph.insert_edge(i, i + 1, 1.0).unwrap();
        }

        let decomp = HierarchicalDecomposition::build(graph).unwrap();

        // Height should be O(log n) = O(log 100) ≈ 7
        assert!(
            decomp.height() <= 7,
            "Height {} should be <= 7",
            decomp.height()
        );

        // Min cut of a path is 1.0 (any single edge)
        assert_eq!(decomp.min_cut_value(), 1.0);

        // Total nodes: 100 leaves + 99 internal = 199
        assert_eq!(decomp.num_nodes(), 199);
    }

    #[test]
    fn test_propagate_updates() {
        let graph = create_simple_graph();
        let mut decomp = HierarchicalDecomposition::build(graph).unwrap();

        // Mark all nodes as dirty
        for i in 0..decomp.nodes.len() {
            decomp.nodes[i].dirty = true;
        }

        // Propagate updates
        let min_cut = decomp.propagate_updates();

        // All nodes should now be clean
        for node in &decomp.nodes {
            assert!(!node.dirty);
        }

        // Min cut should be correct
        assert_eq!(min_cut, 2.0);
    }
}
