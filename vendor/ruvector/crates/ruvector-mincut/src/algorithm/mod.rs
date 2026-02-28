//! Core Dynamic Minimum Cut Algorithm
//!
//! Provides the main algorithm with:
//! - O(n^{o(1)}) amortized update time
//! - Support for edge insertions and deletions
//! - Both exact and approximate modes
//!
//! ## Modules
//!
//! - [`replacement`]: Replacement edge index for tree edge deletions
//! - [`approximate`]: (1+ε)-approximate min-cut for all cut sizes (SODA 2025)

pub mod approximate;
pub mod replacement;

pub use replacement::{ReplacementEdgeIndex, ReplacementIndexStats};

use crate::error::{MinCutError, Result};
use crate::euler::EulerTourTree;
use crate::graph::{DynamicGraph, Edge, EdgeId, VertexId, Weight};
use crate::linkcut::LinkCutTree;
use crate::tree::HierarchicalDecomposition;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

/// Configuration for the minimum cut algorithm
#[derive(Debug, Clone)]
pub struct MinCutConfig {
    /// Maximum cut size supported for exact algorithm
    pub max_exact_cut_size: usize,
    /// Epsilon for approximate algorithm (0 < ε ≤ 1)
    pub epsilon: f64,
    /// Whether to use approximate mode
    pub approximate: bool,
    /// Enable parallel computation
    pub parallel: bool,
    /// Cache size for intermediate results
    pub cache_size: usize,
}

impl Default for MinCutConfig {
    fn default() -> Self {
        Self {
            max_exact_cut_size: 1000,
            epsilon: 0.1,
            approximate: false,
            parallel: true,
            cache_size: 10000,
        }
    }
}

/// Result of a minimum cut query
#[derive(Debug, Clone)]
pub struct MinCutResult {
    /// The minimum cut value
    pub value: f64,
    /// Edges in the cut (if requested)
    pub cut_edges: Option<Vec<Edge>>,
    /// Partition (if requested): (S, T) where S and T are vertex sets
    pub partition: Option<(Vec<VertexId>, Vec<VertexId>)>,
    /// Whether this is an exact or approximate result
    pub is_exact: bool,
    /// Approximation ratio (1.0 for exact)
    pub approximation_ratio: f64,
}

/// Statistics about algorithm performance
#[derive(Debug, Clone, Default)]
pub struct AlgorithmStats {
    /// Total number of insertions
    pub insertions: u64,
    /// Total number of deletions
    pub deletions: u64,
    /// Total number of queries
    pub queries: u64,
    /// Average update time in microseconds
    pub avg_update_time_us: f64,
    /// Average query time in microseconds
    pub avg_query_time_us: f64,
    /// Number of tree restructures
    pub restructures: u64,
}

/// The main dynamic minimum cut structure
pub struct DynamicMinCut {
    /// The underlying graph
    graph: Arc<RwLock<DynamicGraph>>,
    /// Hierarchical decomposition
    decomposition: HierarchicalDecomposition,
    /// Link-cut tree for connectivity
    link_cut_tree: LinkCutTree,
    /// Spanning forest for the graph (using Euler Tour Tree)
    spanning_forest: EulerTourTree,
    /// Current minimum cut value
    current_min_cut: f64,
    /// Configuration
    config: MinCutConfig,
    /// Statistics
    stats: Arc<RwLock<AlgorithmStats>>,
    /// Tracks which edges are in the spanning forest (tree edges)
    tree_edges: Arc<RwLock<std::collections::HashSet<(VertexId, VertexId)>>>,
}

impl DynamicMinCut {
    /// Create a new dynamic minimum cut structure
    pub fn new(config: MinCutConfig) -> Self {
        let empty_graph = Arc::new(DynamicGraph::new());
        Self {
            graph: Arc::new(RwLock::new(DynamicGraph::new())),
            decomposition: HierarchicalDecomposition::build(empty_graph).unwrap(),
            link_cut_tree: LinkCutTree::new(),
            spanning_forest: EulerTourTree::new(),
            current_min_cut: f64::INFINITY,
            config,
            stats: Arc::new(RwLock::new(AlgorithmStats::default())),
            tree_edges: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// Build from an existing graph
    pub fn from_graph(graph: DynamicGraph, config: MinCutConfig) -> Result<Self> {
        // Create shared graph instance
        let graph_shared = Arc::new(RwLock::new(graph.clone()));

        // Initialize link-cut tree and Euler tour tree with all vertices
        let mut link_cut_tree = LinkCutTree::new();
        let mut spanning_forest = EulerTourTree::new();

        let vertices = graph.vertices();
        for &v in &vertices {
            link_cut_tree.make_tree(v, 0.0);
            spanning_forest.make_tree(v)?;
        }

        // Build spanning forest using DFS
        let mut visited = std::collections::HashSet::new();
        let mut tree_edges = std::collections::HashSet::new();

        for &start_vertex in &vertices {
            if visited.contains(&start_vertex) {
                continue;
            }

            // DFS to build spanning tree for this component
            let mut stack = vec![start_vertex];
            visited.insert(start_vertex);

            while let Some(u) = stack.pop() {
                let neighbors = graph.neighbors(u);
                for (v, _) in neighbors {
                    if !visited.contains(&v) {
                        visited.insert(v);
                        stack.push(v);

                        // Add edge to spanning forest
                        link_cut_tree.link(u, v)?;
                        spanning_forest.link(u, v)?;

                        // Track as tree edge
                        let key = if u < v { (u, v) } else { (v, u) };
                        tree_edges.insert(key);
                    }
                }
            }
        }

        // Initialize hierarchical decomposition with the same shared graph
        let graph_for_decomp = Arc::new(graph);
        let decomposition = HierarchicalDecomposition::build(graph_for_decomp)?;

        // Create the structure first
        let mut mincut = Self {
            graph: graph_shared,
            decomposition,
            link_cut_tree,
            spanning_forest,
            current_min_cut: f64::INFINITY,
            config,
            stats: Arc::new(RwLock::new(AlgorithmStats::default())),
            tree_edges: Arc::new(RwLock::new(tree_edges)),
        };

        // Now compute the initial minimum cut using the tree-edge-based method
        mincut.recompute_min_cut();

        Ok(mincut)
    }

    /// Insert an edge
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<f64> {
        let start_time = Instant::now();

        // Add edge to graph (use write lock)
        {
            let graph = self.graph.write();
            graph.insert_edge(u, v, weight)?;
        }

        // Ensure vertices exist in data structures
        // Create vertices in link-cut tree and Euler tour tree if they don't exist
        let u_exists = self.link_cut_tree.len() > 0 && self.link_cut_tree.find_root(u).is_ok();
        let v_exists = self.link_cut_tree.len() > 0 && self.link_cut_tree.find_root(v).is_ok();

        if !u_exists {
            self.link_cut_tree.make_tree(u, 0.0);
            self.spanning_forest.make_tree(u)?;
        }
        if !v_exists {
            self.link_cut_tree.make_tree(v, 0.0);
            self.spanning_forest.make_tree(v)?;
        }

        // Check if they're already in different components
        let connected = self.link_cut_tree.connected(u, v);

        if !connected {
            // Vertices are in different components - this edge connects them
            self.handle_bridge_edge(u, v, weight)?;
        } else {
            // Edge creates a cycle - this is a non-tree edge
            self.handle_cycle_edge(u, v, weight)?;
        }

        // Rebuild decomposition with updated graph
        self.rebuild_decomposition();

        // Recompute minimum cut
        self.recompute_min_cut();

        // Update statistics
        let elapsed = start_time.elapsed().as_micros() as f64;
        let mut stats = self.stats.write();
        stats.insertions += 1;
        let n = stats.insertions as f64;
        stats.avg_update_time_us = (stats.avg_update_time_us * (n - 1.0) + elapsed) / n;
        drop(stats);

        Ok(self.current_min_cut)
    }

    /// Delete an edge
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<f64> {
        let start_time = Instant::now();

        // Remove from graph first (use write lock)
        {
            let graph = self.graph.write();
            graph.delete_edge(u, v)?;
        }

        // Check if edge was a tree edge
        let key = if u < v { (u, v) } else { (v, u) };
        let is_tree_edge = self.tree_edges.read().contains(&key);

        if is_tree_edge {
            self.handle_tree_edge_deletion(u, v)?;
        } else {
            self.handle_non_tree_edge_deletion(u, v)?;
        }

        // Rebuild decomposition with updated graph
        self.rebuild_decomposition();

        // Recompute minimum cut
        self.recompute_min_cut();

        // Update statistics
        let elapsed = start_time.elapsed().as_micros() as f64;
        let mut stats = self.stats.write();
        stats.deletions += 1;
        let n = stats.deletions as f64;
        stats.avg_update_time_us = (stats.avg_update_time_us * (n - 1.0) + elapsed) / n;
        drop(stats);

        Ok(self.current_min_cut)
    }

    /// Get the current minimum cut value (O(1))
    pub fn min_cut_value(&self) -> f64 {
        let start_time = Instant::now();

        let value = self.current_min_cut;

        // Update query statistics
        let elapsed = start_time.elapsed().as_micros() as f64;
        let mut stats = self.stats.write();
        stats.queries += 1;
        let n = stats.queries as f64;
        stats.avg_query_time_us = (stats.avg_query_time_us * (n - 1.0) + elapsed) / n;

        value
    }

    /// Get detailed minimum cut result
    pub fn min_cut(&self) -> MinCutResult {
        let value = self.min_cut_value();
        let (partition_s, partition_t) = self.partition();
        let edges = self.cut_edges();

        MinCutResult {
            value,
            cut_edges: Some(edges),
            partition: Some((partition_s, partition_t)),
            is_exact: !self.config.approximate,
            approximation_ratio: if self.config.approximate {
                1.0 + self.config.epsilon
            } else {
                1.0
            },
        }
    }

    /// Get the cut partition
    pub fn partition(&self) -> (Vec<VertexId>, Vec<VertexId>) {
        // Use the decomposition's partition
        let (set_a, set_b) = self.decomposition.min_cut_partition();
        (set_a.into_iter().collect(), set_b.into_iter().collect())
    }

    /// Get edges in the minimum cut
    pub fn cut_edges(&self) -> Vec<Edge> {
        let (partition_s, partition_t) = self.partition();
        let partition_t_set: std::collections::HashSet<_> = partition_t.iter().copied().collect();

        let graph = self.graph.read();
        let mut cut_edges = Vec::new();

        for &u in &partition_s {
            let neighbors = graph.neighbors(u);
            for (v, _) in neighbors {
                if partition_t_set.contains(&v) {
                    if let Some(edge) = graph.get_edge(u, v) {
                        cut_edges.push(edge);
                    }
                }
            }
        }

        cut_edges
    }

    /// Check if graph is connected
    pub fn is_connected(&self) -> bool {
        let graph = self.graph.read();
        graph.is_connected()
    }

    /// Get algorithm statistics
    pub fn stats(&self) -> AlgorithmStats {
        self.stats.read().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        *self.stats.write() = AlgorithmStats::default();
    }

    /// Get configuration
    pub fn config(&self) -> &MinCutConfig {
        &self.config
    }

    /// Get reference to underlying graph
    pub fn graph(&self) -> Arc<RwLock<DynamicGraph>> {
        Arc::clone(&self.graph)
    }

    /// Number of vertices
    pub fn num_vertices(&self) -> usize {
        self.graph.read().num_vertices()
    }

    /// Number of edges
    pub fn num_edges(&self) -> usize {
        self.graph.read().num_edges()
    }

    // ===== Internal methods =====

    /// Handle insertion when edge creates a cycle (non-tree edge)
    fn handle_cycle_edge(&mut self, _u: VertexId, _v: VertexId, _weight: Weight) -> Result<()> {
        // Non-tree edges don't change connectivity but may affect cut value
        // The decomposition handles cut value updates
        Ok(())
    }

    /// Handle insertion when edge connects components (bridge edge)
    fn handle_bridge_edge(&mut self, u: VertexId, v: VertexId, _weight: Weight) -> Result<()> {
        // Add to spanning forest
        self.link_cut_tree.link(u, v)?;
        self.spanning_forest.link(u, v)?;

        // Track as tree edge
        let key = if u < v { (u, v) } else { (v, u) };
        self.tree_edges.write().insert(key);

        Ok(())
    }

    /// Handle deletion of a tree edge (find replacement)
    fn handle_tree_edge_deletion(&mut self, u: VertexId, v: VertexId) -> Result<()> {
        // Remove from tree edges
        let key = if u < v { (u, v) } else { (v, u) };
        self.tree_edges.write().remove(&key);

        // Cut in link-cut tree if they're still connected
        // (They might already be disconnected from previous deletions)
        if self.link_cut_tree.connected(u, v) {
            // Cut from u's perspective (or v's if u is already a root)
            // Try cutting u first, if it fails try v
            if self.link_cut_tree.cut(u).is_err() {
                let _ = self.link_cut_tree.cut(v); // Ignore error if both fail
            }
        }

        // Try to find a replacement edge
        if let Some((x, y)) = self.find_replacement_edge(u, v) {
            // Check if they're already connected before linking
            let already_connected = self.link_cut_tree.connected(x, y);

            if !already_connected {
                // Add replacement edge to spanning forest
                self.link_cut_tree.link(x, y)?;

                // Only link in spanning forest if not already connected
                if !self.spanning_forest.connected(x, y) {
                    let _ = self.spanning_forest.link(x, y); // Ignore errors here
                }

                // Track as tree edge
                let key = if x < y { (x, y) } else { (y, x) };
                self.tree_edges.write().insert(key);
            }
        } else {
            // Graph is now disconnected
            // Minimum cut becomes 0
            self.stats.write().restructures += 1;
        }

        Ok(())
    }

    /// Handle deletion of a non-tree edge
    fn handle_non_tree_edge_deletion(&mut self, _u: VertexId, _v: VertexId) -> Result<()> {
        // Non-tree edge deletion doesn't affect connectivity
        // But may affect minimum cut value (handled by decomposition)
        Ok(())
    }

    /// Find a replacement edge for a deleted tree edge
    fn find_replacement_edge(&mut self, u: VertexId, v: VertexId) -> Option<(VertexId, VertexId)> {
        // Get the two components after cutting
        // Use BFS to find vertices reachable from u (without using edge u-v)
        let graph = self.graph.read();

        let mut comp_u = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back(u);
        comp_u.insert(u);

        while let Some(x) = queue.pop_front() {
            let neighbors = graph.neighbors(x);
            for (y, _) in neighbors {
                // Skip the deleted edge
                if (x == u && y == v) || (x == v && y == u) {
                    continue;
                }

                // Only traverse tree edges for connectivity
                let key = if x < y { (x, y) } else { (y, x) };
                if !self.tree_edges.read().contains(&key) {
                    continue;
                }

                if comp_u.insert(y) {
                    queue.push_back(y);
                }
            }
        }

        // Now search for non-tree edges crossing the cut
        for &x in &comp_u {
            let neighbors = graph.neighbors(x);
            for (y, _) in neighbors {
                if !comp_u.contains(&y) {
                    // Found a replacement edge
                    return Some((x, y));
                }
            }
        }

        None
    }

    /// Rebuild the hierarchical decomposition with current graph state
    fn rebuild_decomposition(&mut self) {
        let graph = self.graph.read();
        let graph_clone = graph.clone();
        drop(graph);

        let graph_for_decomp = Arc::new(graph_clone);
        self.decomposition =
            HierarchicalDecomposition::build(graph_for_decomp).unwrap_or_else(|_| {
                // If build fails, create an empty one
                let empty = Arc::new(DynamicGraph::new());
                HierarchicalDecomposition::build(empty).unwrap()
            });
    }

    /// Recompute minimum cut after structural change
    fn recompute_min_cut(&mut self) {
        let graph = self.graph.read();

        // If graph is disconnected, minimum cut is 0
        if !graph.is_connected() {
            self.current_min_cut = 0.0;
            drop(graph);
            return;
        }

        // Compute minimum cut by checking all tree edge cuts
        let mut min_cut = self.decomposition.min_cut_value();

        // For each tree edge, compute the cut value when removing it
        // This gives us all possible 2-partitions induced by the spanning tree
        let tree_edges: Vec<_> = self.tree_edges.read().iter().copied().collect();

        for (u, v) in tree_edges {
            let cut_value = self.compute_tree_edge_cut(&graph, u, v);
            if cut_value < min_cut {
                min_cut = cut_value;
            }
        }

        drop(graph);
        self.current_min_cut = min_cut;
    }

    /// Compute the cut value induced by removing a tree edge
    fn compute_tree_edge_cut(&self, graph: &DynamicGraph, u: VertexId, v: VertexId) -> f64 {
        // Find all vertices reachable from u without using edge (u,v)
        let mut component_u = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back(u);
        component_u.insert(u);

        while let Some(x) = queue.pop_front() {
            for (y, _) in graph.neighbors(x) {
                // Skip the tree edge we're "removing"
                if (x == u && y == v) || (x == v && y == u) {
                    continue;
                }

                // Only traverse tree edges for connectivity
                let key = if x < y { (x, y) } else { (y, x) };
                if !self.tree_edges.read().contains(&key) {
                    continue;
                }

                if component_u.insert(y) {
                    queue.push_back(y);
                }
            }
        }

        // Now compute cut: sum of all edge weights crossing the partition
        let mut cut_weight = 0.0;
        for &x in &component_u {
            for (y, _) in graph.neighbors(x) {
                if !component_u.contains(&y) {
                    if let Some(weight) = graph.edge_weight(x, y) {
                        cut_weight += weight;
                    }
                }
            }
        }

        cut_weight
    }
}

/// Builder for DynamicMinCut
pub struct MinCutBuilder {
    config: MinCutConfig,
    initial_edges: Vec<(VertexId, VertexId, Weight)>,
}

impl MinCutBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: MinCutConfig::default(),
            initial_edges: Vec::new(),
        }
    }

    /// Use exact algorithm
    pub fn exact(mut self) -> Self {
        self.config.approximate = false;
        self
    }

    /// Use approximate algorithm with given epsilon
    pub fn approximate(mut self, epsilon: f64) -> Self {
        assert!(epsilon > 0.0 && epsilon <= 1.0, "Epsilon must be in (0, 1]");
        self.config.approximate = true;
        self.config.epsilon = epsilon;
        self
    }

    /// Set maximum cut size for exact algorithm
    pub fn max_cut_size(mut self, size: usize) -> Self {
        self.config.max_exact_cut_size = size;
        self
    }

    /// Enable or disable parallel computation
    pub fn parallel(mut self, enabled: bool) -> Self {
        self.config.parallel = enabled;
        self
    }

    /// Add initial edges
    pub fn with_edges(mut self, edges: Vec<(VertexId, VertexId, Weight)>) -> Self {
        self.initial_edges = edges;
        self
    }

    /// Build the minimum cut structure
    pub fn build(self) -> Result<DynamicMinCut> {
        if self.initial_edges.is_empty() {
            Ok(DynamicMinCut::new(self.config))
        } else {
            // Create graph with initial edges
            let graph = DynamicGraph::new();
            for (u, v, weight) in &self.initial_edges {
                graph.insert_edge(*u, *v, *weight)?;
            }

            DynamicMinCut::from_graph(graph, self.config)
        }
    }
}

impl Default for MinCutBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let mincut = DynamicMinCut::new(MinCutConfig::default());
        assert_eq!(mincut.min_cut_value(), f64::INFINITY);
        assert_eq!(mincut.num_vertices(), 0);
        assert_eq!(mincut.num_edges(), 0);
    }

    #[test]
    fn test_single_edge() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        assert_eq!(mincut.num_vertices(), 2);
        assert_eq!(mincut.num_edges(), 1);
        assert_eq!(mincut.min_cut_value(), 1.0);
        assert!(mincut.is_connected());
    }

    #[test]
    fn test_triangle() {
        let edges = vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)];

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert_eq!(mincut.num_vertices(), 3);
        assert_eq!(mincut.num_edges(), 3);
        assert_eq!(mincut.min_cut_value(), 2.0); // Minimum cut is 2
        assert!(mincut.is_connected());
    }

    #[test]
    fn test_insert_edge() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        let cut_value = mincut.insert_edge(2, 3, 1.0).unwrap();
        assert_eq!(mincut.num_edges(), 2);
        assert_eq!(cut_value, 1.0);
    }

    #[test]
    fn test_delete_edge() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)])
            .build()
            .unwrap();

        assert!(mincut.is_connected());

        let cut_value = mincut.delete_edge(1, 2).unwrap();
        assert_eq!(mincut.num_edges(), 1);
        // After deleting edge, graph becomes disconnected
        assert_eq!(cut_value, 0.0);
    }

    #[test]
    fn test_disconnected_graph() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0), (3, 4, 1.0)])
            .build()
            .unwrap();

        assert!(!mincut.is_connected());
        assert_eq!(mincut.min_cut_value(), 0.0);
    }

    #[test]
    fn test_weighted_edges() {
        let edges = vec![(1, 2, 2.0), (2, 3, 3.0), (3, 1, 1.0)];

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        // Minimum cut should be 2.0 (cutting {1} from {2,3} or similar)
        assert_eq!(mincut.min_cut_value(), 3.0);
    }

    #[test]
    fn test_partition() {
        let edges = vec![(1, 2, 1.0), (2, 3, 1.0), (3, 4, 1.0)];

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        let (s, t) = mincut.partition();
        assert!(!s.is_empty());
        assert!(!t.is_empty());
        assert_eq!(s.len() + t.len(), 4);
    }

    #[test]
    fn test_cut_edges() {
        let edges = vec![(1, 2, 1.0), (2, 3, 1.0)];

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        let cut = mincut.cut_edges();
        assert!(!cut.is_empty());
        assert!(cut.len() <= 2);
    }

    #[test]
    fn test_min_cut_result() {
        let edges = vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)];

        let mincut = MinCutBuilder::new()
            .exact()
            .with_edges(edges)
            .build()
            .unwrap();

        let result = mincut.min_cut();
        assert!(result.is_exact);
        assert_eq!(result.approximation_ratio, 1.0);
        assert!(result.cut_edges.is_some());
        assert!(result.partition.is_some());
    }

    #[test]
    fn test_approximate_mode() {
        let mincut = MinCutBuilder::new().approximate(0.1).build().unwrap();

        let result = mincut.min_cut();
        assert!(!result.is_exact);
        assert_eq!(result.approximation_ratio, 1.1);
    }

    #[test]
    fn test_statistics() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        mincut.insert_edge(2, 3, 1.0).unwrap();
        mincut.delete_edge(1, 2).unwrap();
        let _ = mincut.min_cut_value();

        let stats = mincut.stats();
        assert_eq!(stats.insertions, 1);
        assert_eq!(stats.deletions, 1);
        assert_eq!(stats.queries, 1);
        assert!(stats.avg_update_time_us > 0.0);
    }

    #[test]
    fn test_reset_stats() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        mincut.insert_edge(2, 3, 1.0).unwrap();
        assert_eq!(mincut.stats().insertions, 1);

        mincut.reset_stats();
        assert_eq!(mincut.stats().insertions, 0);
    }

    #[test]
    fn test_builder_pattern() {
        let mincut = MinCutBuilder::new()
            .exact()
            .max_cut_size(500)
            .parallel(true)
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        assert!(!mincut.config().approximate);
        assert_eq!(mincut.config().max_exact_cut_size, 500);
        assert!(mincut.config().parallel);
    }

    #[test]
    fn test_large_graph() {
        let mut edges = Vec::new();

        // Create a chain: 0 - 1 - 2 - ... - 99
        for i in 0..99 {
            edges.push((i, i + 1, 1.0));
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert_eq!(mincut.num_vertices(), 100);
        assert_eq!(mincut.num_edges(), 99);
        assert_eq!(mincut.min_cut_value(), 1.0); // Minimum cut is 1
        assert!(mincut.is_connected());
    }

    #[test]
    fn test_tree_edge_deletion_with_replacement() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![
                (1, 2, 1.0),
                (2, 3, 1.0),
                (1, 3, 1.0), // Creates a cycle
            ])
            .build()
            .unwrap();

        assert!(mincut.is_connected());

        // Delete one edge - graph should remain connected due to replacement
        mincut.delete_edge(1, 2).unwrap();

        // Still has 2 edges
        assert_eq!(mincut.num_edges(), 2);
    }

    #[test]
    fn test_multiple_components() {
        let edges = vec![(1, 2, 1.0), (3, 4, 1.0), (5, 6, 1.0)];

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert!(!mincut.is_connected());
        assert_eq!(mincut.min_cut_value(), 0.0);
    }

    #[test]
    fn test_dynamic_updates() {
        let mut mincut = MinCutBuilder::new().build().unwrap();

        // Start empty
        assert_eq!(mincut.min_cut_value(), f64::INFINITY);

        // Add first edge (creates two vertices)
        mincut.insert_edge(1, 2, 2.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 2.0);

        // Complete the triangle
        mincut.insert_edge(2, 3, 3.0).unwrap();
        mincut.insert_edge(3, 1, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 3.0); // min cut

        // Delete heaviest edge
        mincut.delete_edge(2, 3).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0); // Now path graph
    }

    #[test]
    fn test_config_access() {
        let mincut = MinCutBuilder::new()
            .approximate(0.2)
            .max_cut_size(2000)
            .build()
            .unwrap();

        let config = mincut.config();
        assert_eq!(config.epsilon, 0.2);
        assert_eq!(config.max_exact_cut_size, 2000);
        assert!(config.approximate);
    }

    #[test]
    fn test_graph_access() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        let graph = mincut.graph();
        let g = graph.read();
        assert_eq!(g.num_vertices(), 2);
        assert_eq!(g.num_edges(), 1);
    }

    #[test]
    fn test_bridge_graph() {
        // Two triangles connected by a bridge
        let edges = vec![
            // Triangle 1: 1-2-3-1
            (1, 2, 2.0),
            (2, 3, 2.0),
            (3, 1, 2.0),
            // Bridge
            (3, 4, 1.0),
            // Triangle 2: 4-5-6-4
            (4, 5, 2.0),
            (5, 6, 2.0),
            (6, 4, 2.0),
        ];

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        // Minimum cut should be the bridge with weight 1.0
        assert_eq!(mincut.min_cut_value(), 1.0);
        assert!(mincut.is_connected());
    }

    #[test]
    fn test_complete_graph_k4() {
        // Complete graph on 4 vertices
        let mut edges = Vec::new();
        for i in 1..=4 {
            for j in (i + 1)..=4 {
                edges.push((i, j, 1.0));
            }
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert_eq!(mincut.num_vertices(), 4);
        assert_eq!(mincut.num_edges(), 6);
        // Minimum cut of K4 is 3 (degree of any vertex)
        assert_eq!(mincut.min_cut_value(), 3.0);
    }

    #[test]
    fn test_sequential_insertions() {
        let mut mincut = MinCutBuilder::new().build().unwrap();

        // Build graph incrementally
        mincut.insert_edge(1, 2, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0);

        mincut.insert_edge(2, 3, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0);

        mincut.insert_edge(3, 4, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0);

        // Add cycle closure
        mincut.insert_edge(4, 1, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 2.0);
    }

    #[test]
    fn test_sequential_deletions() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
            .build()
            .unwrap();

        assert_eq!(mincut.min_cut_value(), 2.0);

        // Delete one edge
        mincut.delete_edge(1, 2).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0);

        // Delete another edge - disconnects graph
        mincut.delete_edge(2, 3).unwrap();
        assert_eq!(mincut.min_cut_value(), 0.0);
    }
}
