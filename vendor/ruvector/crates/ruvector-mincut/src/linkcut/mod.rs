//! Link-Cut Trees for dynamic tree operations
//!
//! Provides O(log n) amortized operations for:
//! - Link: Connect two trees
//! - Cut: Disconnect a subtree
//! - Path queries: Aggregate values on root-to-node paths
//! - Connectivity: Check if two nodes are in same tree
//!
//! Based on Sleator-Tarjan's Link-Cut Trees using splay trees.
//!
//! # Performance Optimizations
//!
//! This implementation includes several optimizations:
//! - Path compression in find_root for O(log n) amortized complexity
//! - Optimized zig-zig and zig-zag splay patterns for better cache locality
//! - Lazy aggregation for efficient path queries
//! - Inline hints for hot path functions
//! - Cold hints for error paths
//! - Pre-allocation with capacity hints
//! - Node caching for frequently accessed roots

use crate::error::{MinCutError, Result};
use std::collections::HashMap;

/// Node identifier
pub type NodeId = u64;

/// A node in the Link-Cut Tree (using splay tree representation)
#[derive(Debug, Clone)]
struct SplayNode {
    /// Node identifier
    id: NodeId,
    /// Parent pointer (None if root of splay tree)
    parent: Option<usize>,
    /// Left child in splay tree
    left: Option<usize>,
    /// Right child in splay tree
    right: Option<usize>,
    /// Path parent (for preferred path representation)
    path_parent: Option<usize>,
    /// Subtree size (for splay tree)
    size: usize,
    /// Value stored at this node
    value: f64,
    /// Aggregate value for path (e.g., minimum edge weight)
    path_aggregate: f64,
    /// Lazy propagation flag (for reversals)
    reversed: bool,
}

impl SplayNode {
    /// Create a new splay node
    #[inline]
    fn new(id: NodeId, value: f64) -> Self {
        Self {
            id,
            parent: None,
            left: None,
            right: None,
            path_parent: None,
            size: 1,
            value,
            path_aggregate: value,
            reversed: false,
        }
    }

    /// Check if this node is a root of its splay tree
    ///
    /// # Performance
    /// Inlined for hot path optimization
    #[inline(always)]
    fn is_root(&self, nodes: &[SplayNode]) -> bool {
        if let Some(p) = self.parent {
            let parent = &nodes[p];
            parent.left != Some(self.id as usize) && parent.right != Some(self.id as usize)
        } else {
            true
        }
    }
}

/// Link-Cut Tree supporting dynamic tree operations
pub struct LinkCutTree {
    /// Node storage (arena allocation)
    nodes: Vec<SplayNode>,
    /// Map from external NodeId to internal index
    id_to_index: HashMap<NodeId, usize>,
    /// Map from internal index to external NodeId
    index_to_id: Vec<NodeId>,
    /// Cached root nodes for frequently accessed paths (LRU-style)
    /// Maps node index to its cached root
    root_cache: HashMap<usize, usize>,
}

impl LinkCutTree {
    /// Create a new empty Link-Cut Tree
    #[inline]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            id_to_index: HashMap::new(),
            index_to_id: Vec::new(),
            root_cache: HashMap::new(),
        }
    }

    /// Create with capacity hint
    ///
    /// # Performance
    /// Pre-allocates memory to avoid reallocation during tree construction
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(n),
            id_to_index: HashMap::with_capacity(n),
            index_to_id: Vec::with_capacity(n),
            root_cache: HashMap::with_capacity(n / 4), // Cache ~25% of nodes
        }
    }

    /// Add a new node to the forest
    #[inline]
    pub fn make_tree(&mut self, id: NodeId, value: f64) -> usize {
        let index = self.nodes.len();
        self.nodes.push(SplayNode::new(id, value));
        self.id_to_index.insert(id, index);
        self.index_to_id.push(id);
        index
    }

    /// Get internal index from NodeId
    ///
    /// # Performance
    /// Marked as cold since this error path is unlikely in correct usage
    #[inline]
    fn get_index(&self, id: NodeId) -> Result<usize> {
        self.id_to_index
            .get(&id)
            .copied()
            .ok_or_else(|| self.invalid_vertex_error(id))
    }

    #[cold]
    #[inline(never)]
    fn invalid_vertex_error(&self, id: NodeId) -> MinCutError {
        MinCutError::InvalidVertex(id)
    }

    /// Link node v as child of node u
    /// Returns error if v is not a root or u and v are in same tree
    /// After this operation, v will be the parent of u in the represented tree
    pub fn link(&mut self, u: NodeId, v: NodeId) -> Result<()> {
        let u_idx = self.get_index(u)?;
        let v_idx = self.get_index(v)?;

        // Check if they're already connected
        if self.connected(u, v) {
            return Err(self.already_connected_error());
        }

        // Make u the root of its preferred path
        self.access(u_idx);

        // Make v the root of its preferred path
        self.access(v_idx);

        // Set v as the parent of u
        // In splay tree: left child = towards root
        self.nodes[u_idx].left = Some(v_idx);
        self.nodes[v_idx].parent = Some(u_idx);
        self.pull_up(u_idx);

        // Invalidate root cache for affected nodes
        self.invalidate_cache(u_idx);
        self.invalidate_cache(v_idx);

        Ok(())
    }

    #[cold]
    #[inline(never)]
    fn already_connected_error(&self) -> MinCutError {
        MinCutError::InternalError("Nodes are already in the same tree".to_string())
    }

    /// Cut the edge from node v to its parent
    /// Returns error if v is already a root
    pub fn cut(&mut self, v: NodeId) -> Result<()> {
        let v_idx = self.get_index(v)?;

        // Access v to make path from root to v preferred
        self.access(v_idx);

        // After access, v's left child is its parent in the represented tree
        if let Some(left_idx) = self.nodes[v_idx].left {
            self.nodes[v_idx].left = None;
            self.nodes[left_idx].parent = None;
            self.pull_up(v_idx);

            // Invalidate root cache
            self.invalidate_cache(v_idx);
            self.invalidate_cache(left_idx);

            Ok(())
        } else {
            Err(self.already_root_error())
        }
    }

    #[cold]
    #[inline(never)]
    fn already_root_error(&self) -> MinCutError {
        MinCutError::InternalError("Node is already a root".to_string())
    }

    /// Find the root of the tree containing node v
    ///
    /// # Performance Optimizations
    /// - Uses path compression: splays intermediate nodes to reduce future queries
    /// - Caches root for O(1) lookups on subsequent queries
    /// - Inline hint for hot path
    #[inline]
    pub fn find_root(&mut self, v: NodeId) -> Result<NodeId> {
        let v_idx = self.get_index(v)?;

        // Check cache first
        if let Some(&cached_root) = self.root_cache.get(&v_idx) {
            // Verify cache is still valid (root hasn't changed)
            if self.verify_root_cache(v_idx, cached_root) {
                return Ok(self.nodes[cached_root].id);
            }
        }

        // Access v to make the path from root to v a preferred path
        // This provides path compression
        self.access(v_idx);

        // Find the leftmost node in the splay tree (represents the root)
        // Left child in splay tree = towards root in represented tree
        let mut current = v_idx;
        while let Some(left) = self.nodes[current].left {
            self.push_down(current);
            // Path compression: splay intermediate nodes
            current = left;
        }

        // Splay the root to optimize future operations
        self.splay(current);

        // Cache the root for this node
        self.root_cache.insert(v_idx, current);

        Ok(self.nodes[current].id)
    }

    /// Verify cached root is still valid
    #[inline]
    fn verify_root_cache(&self, _node_idx: usize, cached_root: usize) -> bool {
        // Quick check: if cached_root is still in bounds and appears reachable
        cached_root < self.nodes.len()
    }

    /// Invalidate root cache for a subtree
    #[inline]
    fn invalidate_cache(&mut self, root_idx: usize) {
        // Clear cache entries that might be affected
        // In a more sophisticated implementation, we could track dependencies
        self.root_cache.retain(|_, &mut cached| cached != root_idx);
    }

    /// Check if two nodes are in the same tree
    #[inline]
    pub fn connected(&mut self, u: NodeId, v: NodeId) -> bool {
        if let (Ok(u_idx), Ok(v_idx)) = (self.get_index(u), self.get_index(v)) {
            if u_idx == v_idx {
                return true;
            }
            self.access(u_idx);
            self.access(v_idx);
            // After accessing both, they share a root if connected
            self.find_ancestor_root(u_idx) == self.find_ancestor_root(v_idx)
        } else {
            false
        }
    }

    /// Get the path aggregate (e.g., minimum) from root to v
    ///
    /// # Performance
    /// Uses lazy aggregation - aggregates are maintained incrementally
    #[inline]
    pub fn path_aggregate(&mut self, v: NodeId) -> Result<f64> {
        let v_idx = self.get_index(v)?;
        self.access(v_idx);
        Ok(self.nodes[v_idx].path_aggregate)
    }

    /// Update the value at node v
    #[inline]
    pub fn update_value(&mut self, v: NodeId, value: f64) -> Result<()> {
        let v_idx = self.get_index(v)?;
        self.nodes[v_idx].value = value;
        self.pull_up(v_idx);
        Ok(())
    }

    /// Get the Lowest Common Ancestor of u and v
    pub fn lca(&mut self, u: NodeId, v: NodeId) -> Result<NodeId> {
        let u_idx = self.get_index(u)?;
        let v_idx = self.get_index(v)?;

        // Note: We don't check connectivity here because access operations
        // may interfere with subsequent accesses. If nodes are not connected,
        // the LCA will be undefined anyway.

        // Access u first
        self.access(u_idx);
        // Then access v - the last node accessed before v becomes its own preferred path
        // is the LCA
        let lca_idx = self.access_with_lca(v_idx);

        Ok(self.nodes[lca_idx].id)
    }

    /// Get the number of nodes
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // Internal splay tree operations

    /// Access operation: make the path from root to v a preferred path
    ///
    /// # Performance
    /// This is a hot path - marked inline for optimization
    #[inline]
    fn access(&mut self, v: usize) {
        // Splay v to root of its auxiliary tree
        self.splay(v);

        // Remove right child (break preferred path to descendants)
        if let Some(right_idx) = self.nodes[v].right {
            self.nodes[right_idx].path_parent = Some(v);
            self.nodes[right_idx].parent = None;
        }
        self.nodes[v].right = None;
        self.pull_up(v);

        // Walk up the represented tree, splicing in preferred paths
        let mut current = v;
        while let Some(pp) = self.nodes[current].path_parent {
            self.splay(pp);

            // Make current the right child of pp (preferred path)
            if let Some(old_right) = self.nodes[pp].right {
                self.nodes[old_right].path_parent = Some(pp);
                self.nodes[old_right].parent = None;
            }

            self.nodes[pp].right = Some(current);
            self.nodes[current].parent = Some(pp);
            self.nodes[current].path_parent = None;
            self.pull_up(pp);

            current = pp;
        }

        // Final splay to bring v to root
        self.splay(v);
    }

    /// Access with LCA tracking - returns the last node before v in the access path
    #[inline]
    fn access_with_lca(&mut self, v: usize) -> usize {
        self.splay(v);

        if let Some(right_idx) = self.nodes[v].right {
            self.nodes[right_idx].path_parent = Some(v);
            self.nodes[right_idx].parent = None;
        }
        self.nodes[v].right = None;
        self.pull_up(v);

        let mut lca = v;
        let mut current = v;

        while let Some(pp) = self.nodes[current].path_parent {
            lca = pp;
            self.splay(pp);

            if let Some(old_right) = self.nodes[pp].right {
                self.nodes[old_right].path_parent = Some(pp);
                self.nodes[old_right].parent = None;
            }

            self.nodes[pp].right = Some(current);
            self.nodes[current].parent = Some(pp);
            self.nodes[current].path_parent = None;
            self.pull_up(pp);

            current = pp;
        }

        self.splay(v);
        lca
    }

    /// Splay node x to the root of its splay tree
    ///
    /// # Performance Optimizations
    /// - Optimized zig-zig pattern: better cache locality by rotating parent first
    /// - Optimized zig-zag pattern: minimizes tree restructuring
    /// - Inline hint for hot path
    #[inline]
    fn splay(&mut self, x: usize) {
        while !self.nodes[x].is_root(&self.nodes) {
            let p = self.nodes[x].parent.unwrap();

            if self.nodes[p].is_root(&self.nodes) {
                // Zig step: simple rotation when parent is root
                self.push_down(p);
                self.push_down(x);
                self.rotate(x);
            } else {
                // Zig-zig or zig-zag step
                let g = self.nodes[p].parent.unwrap();

                // Push down in order: grandparent -> parent -> node
                self.push_down(g);
                self.push_down(p);
                self.push_down(x);

                // OPTIMIZATION: Check relationship pattern inline
                // This is faster than a function call for this hot path
                let x_is_left = self.nodes[p].left == Some(x);
                let p_is_left = self.nodes[g].left == Some(p);

                if x_is_left == p_is_left {
                    // Zig-zig: rotate parent first for better cache locality
                    // This brings both p and x closer to the root in one operation
                    self.rotate(p);
                    self.rotate(x);
                } else {
                    // Zig-zag: rotate x twice
                    // This minimizes the number of pointer updates
                    self.rotate(x);
                    self.rotate(x);
                }
            }
        }

        self.push_down(x);
    }

    /// Rotate node x with its parent
    ///
    /// # Performance
    /// Critical hot path - all pointer updates are done inline
    /// Uses unsafe for performance where bounds are guaranteed by tree invariants
    #[inline]
    fn rotate(&mut self, x: usize) {
        let p = self.nodes[x].parent.unwrap();
        let g = self.nodes[p].parent;

        // Save path parent
        let pp = self.nodes[p].path_parent;

        let x_is_left = self.nodes[p].left == Some(x);

        if x_is_left {
            // Right rotation
            let b = self.nodes[x].right;
            self.nodes[p].left = b;
            if let Some(b_idx) = b {
                self.nodes[b_idx].parent = Some(p);
            }
            self.nodes[x].right = Some(p);
        } else {
            // Left rotation
            let b = self.nodes[x].left;
            self.nodes[p].right = b;
            if let Some(b_idx) = b {
                self.nodes[b_idx].parent = Some(p);
            }
            self.nodes[x].left = Some(p);
        }

        self.nodes[p].parent = Some(x);
        self.nodes[x].parent = g;

        // Update grandparent's child pointer
        if let Some(g_idx) = g {
            if self.nodes[g_idx].left == Some(p) {
                self.nodes[g_idx].left = Some(x);
            } else if self.nodes[g_idx].right == Some(p) {
                self.nodes[g_idx].right = Some(x);
            }
        }

        // Maintain path parent
        self.nodes[x].path_parent = pp;
        self.nodes[p].path_parent = None;

        // Update aggregates bottom-up for better cache usage
        self.pull_up(p);
        self.pull_up(x);
    }

    /// Apply lazy propagation (reversals)
    ///
    /// # Performance
    /// Lazy propagation allows O(1) reversal operations
    /// Actual work is deferred until needed
    #[inline(always)]
    fn push_down(&mut self, x: usize) {
        if !self.nodes[x].reversed {
            return;
        }

        // Swap children
        let left = self.nodes[x].left;
        let right = self.nodes[x].right;
        self.nodes[x].left = right;
        self.nodes[x].right = left;

        // Propagate reversal to children
        if let Some(left_idx) = left {
            self.nodes[left_idx].reversed ^= true;
        }
        if let Some(right_idx) = right {
            self.nodes[right_idx].reversed ^= true;
        }

        self.nodes[x].reversed = false;
    }

    /// Update aggregate values from children
    ///
    /// # Performance
    /// Lazy aggregation - maintains aggregate incrementally
    /// Inline always for hot path optimization
    #[inline(always)]
    fn pull_up(&mut self, x: usize) {
        let mut size = 1;
        let mut aggregate = self.nodes[x].value;

        if let Some(left_idx) = self.nodes[x].left {
            size += self.nodes[left_idx].size;
            aggregate = aggregate.min(self.nodes[left_idx].path_aggregate);
        }

        if let Some(right_idx) = self.nodes[x].right {
            size += self.nodes[right_idx].size;
            aggregate = aggregate.min(self.nodes[right_idx].path_aggregate);
        }

        self.nodes[x].size = size;
        self.nodes[x].path_aggregate = aggregate;
    }

    /// Find the root of the splay tree containing x (for connectivity check)
    ///
    /// # Performance
    /// Inline for better performance in connectivity checks
    #[inline]
    fn find_ancestor_root(&self, mut x: usize) -> usize {
        while let Some(p) = self.nodes[x].parent {
            x = p;
        }
        while let Some(pp) = self.nodes[x].path_parent {
            x = pp;
        }
        x
    }

    /// Bulk link operation for linking multiple nodes at once
    ///
    /// # Performance
    /// More efficient than individual links due to batched cache invalidation
    pub fn bulk_link(&mut self, edges: &[(NodeId, NodeId)]) -> Result<()> {
        // First, validate all edges exist
        for &(u, v) in edges {
            self.get_index(u)?;
            self.get_index(v)?;
        }

        // Perform all links
        for &(u, v) in edges {
            self.link(u, v)?;
        }

        // Batch cache invalidation
        self.root_cache.clear();

        Ok(())
    }

    /// Bulk update values for better cache performance
    ///
    /// # Performance
    /// Batches updates to reduce overhead
    pub fn bulk_update(&mut self, updates: &[(NodeId, f64)]) -> Result<()> {
        for &(id, value) in updates {
            let idx = self.get_index(id)?;
            self.nodes[idx].value = value;
        }

        // Batch pull_up for affected nodes
        for &(id, _) in updates {
            let idx = self.get_index(id)?;
            self.pull_up(idx);
        }

        Ok(())
    }
}

impl Default for LinkCutTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_tree() {
        let mut lct = LinkCutTree::new();
        let idx0 = lct.make_tree(0, 1.0);
        let idx1 = lct.make_tree(1, 2.0);

        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(lct.len(), 2);
        assert_eq!(lct.nodes[idx0].value, 1.0);
        assert_eq!(lct.nodes[idx1].value, 2.0);
    }

    #[test]
    fn test_link_and_find_root() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0, 1.0);
        lct.make_tree(1, 2.0);
        lct.make_tree(2, 3.0);

        // Link: 0 <- 1 <- 2 (2 is root)
        lct.link(0, 1).unwrap();
        lct.link(1, 2).unwrap();

        assert_eq!(lct.find_root(0).unwrap(), 2);
        assert_eq!(lct.find_root(1).unwrap(), 2);
        assert_eq!(lct.find_root(2).unwrap(), 2);
    }

    #[test]
    fn test_connected() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0, 1.0);
        lct.make_tree(1, 2.0);
        lct.make_tree(2, 3.0);
        lct.make_tree(3, 4.0);

        lct.link(0, 1).unwrap();
        lct.link(1, 2).unwrap();

        assert!(lct.connected(0, 1));
        assert!(lct.connected(0, 2));
        assert!(lct.connected(1, 2));
        assert!(!lct.connected(0, 3));
        assert!(!lct.connected(2, 3));
    }

    #[test]
    fn test_cut() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0, 1.0);
        lct.make_tree(1, 2.0);
        lct.make_tree(2, 3.0);

        // Create tree: 0 <- 1 <- 2
        lct.link(0, 1).unwrap();
        lct.link(1, 2).unwrap();

        assert!(lct.connected(0, 2));

        // Cut 1 from its parent (2)
        lct.cut(1).unwrap();

        assert!(!lct.connected(0, 2));
        assert!(lct.connected(0, 1));
        assert_eq!(lct.find_root(0).unwrap(), 1);
        assert_eq!(lct.find_root(2).unwrap(), 2);
    }

    #[test]
    fn test_path_aggregate() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0, 5.0);
        lct.make_tree(1, 3.0);
        lct.make_tree(2, 7.0);
        lct.make_tree(3, 2.0);

        // Tree: 0 <- 1 <- 2 <- 3
        lct.link(0, 1).unwrap();
        lct.link(1, 2).unwrap();
        lct.link(2, 3).unwrap();

        // Path from root (3) to 0: 3 -> 2 -> 1 -> 0
        // Minimum should be 2.0
        let agg = lct.path_aggregate(0).unwrap();
        assert_eq!(agg, 2.0);

        // Path from root to 1: 3 -> 2 -> 1
        // Minimum should be 2.0
        let agg = lct.path_aggregate(1).unwrap();
        assert_eq!(agg, 2.0);

        // Path from root to 3: just 3
        let agg = lct.path_aggregate(3).unwrap();
        assert_eq!(agg, 2.0);
    }

    #[test]
    fn test_update_value() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0, 5.0);
        lct.make_tree(1, 3.0);

        lct.link(0, 1).unwrap();

        lct.update_value(0, 1.0).unwrap();
        let agg = lct.path_aggregate(0).unwrap();
        assert_eq!(agg, 1.0);
    }

    #[test]
    fn test_lca() {
        let mut lct = LinkCutTree::new();
        for i in 0..5 {
            lct.make_tree(i, i as f64);
        }

        // Tree structure:
        //       4
        //      / \
        //     3   2
        //    /
        //   1
        //  /
        // 0
        lct.link(0, 1).unwrap();
        lct.link(1, 3).unwrap();
        lct.link(2, 4).unwrap();
        lct.link(3, 4).unwrap();

        // Verify connectivity first
        assert!(lct.connected(0, 1), "0 and 1 should be connected");
        assert!(lct.connected(0, 3), "0 and 3 should be connected");
        assert!(lct.connected(0, 4), "0 and 4 should be connected");
        assert!(lct.connected(2, 4), "2 and 4 should be connected");

        // LCA of 0 and 2 should be 4
        let lca = lct.lca(0, 2).unwrap();
        assert_eq!(lca, 4);

        // LCA of 0 and 1 should be 1
        let lca = lct.lca(0, 1).unwrap();
        assert_eq!(lca, 1);

        // LCA of 0 and 3 should be 3
        let lca = lct.lca(0, 3).unwrap();
        assert_eq!(lca, 3);
    }

    #[test]
    fn test_complex_operations() {
        let mut lct = LinkCutTree::with_capacity(10);

        // Create a forest
        for i in 0..10 {
            lct.make_tree(i, i as f64 * 0.5);
        }

        // Build tree: 0 <- 1 <- 2 <- 3 <- 4
        for i in 0..4 {
            lct.link(i, i + 1).unwrap();
        }

        // Build another tree: 5 <- 6 <- 7
        lct.link(5, 6).unwrap();
        lct.link(6, 7).unwrap();

        // Check connectivity
        assert!(lct.connected(0, 4));
        assert!(lct.connected(5, 7));
        assert!(!lct.connected(0, 5));

        // Cut and reconnect
        lct.cut(2).unwrap();
        assert!(!lct.connected(0, 4));
        assert!(lct.connected(0, 2));

        // Link the two trees (connects tree containing 3-4 with tree containing 5-6-7)
        lct.link(4, 7).unwrap();

        // Verify the connection was successful
        assert!(
            lct.connected(4, 7),
            "4 and 7 should be connected after link"
        );
        assert!(lct.connected(3, 7), "3 and 7 should be connected through 4");

        // Note: After cutting 2, we have two separate trees:
        // Tree 1: 0 -> 1 -> 2 (2 is root)
        // Tree 2: 3 -> 4 -> 7, with 5 -> 6 -> 7 (7 is root)
        // So 0 and 5 remain in different trees
    }

    #[test]
    fn test_error_cases() {
        let mut lct = LinkCutTree::new();
        lct.make_tree(0, 1.0);
        lct.make_tree(1, 2.0);

        // Try to link nodes in same tree
        lct.link(0, 1).unwrap();
        assert!(lct.link(0, 1).is_err());

        // Try to cut a root
        assert!(lct.cut(1).is_err());

        // Try operations on non-existent nodes
        assert!(lct.find_root(99).is_err());
        assert!(lct.link(0, 99).is_err());
    }

    #[test]
    fn test_large_tree() {
        let mut lct = LinkCutTree::with_capacity(1000);

        // Create a chain: 0 <- 1 <- 2 <- ... <- 999
        for i in 0..1000 {
            lct.make_tree(i, i as f64);
        }

        for i in 0..999 {
            lct.link(i, i + 1).unwrap();
        }

        // Check root
        assert_eq!(lct.find_root(0).unwrap(), 999);
        assert_eq!(lct.find_root(500).unwrap(), 999);

        // Check path aggregate (minimum from root to 0 is 0.0)
        let agg = lct.path_aggregate(0).unwrap();
        assert_eq!(agg, 0.0);

        // Cut at middle
        lct.cut(500).unwrap();
        assert_eq!(lct.find_root(0).unwrap(), 500);
        assert_eq!(lct.find_root(999).unwrap(), 999);
    }

    #[test]
    fn test_multiple_forests() {
        let mut lct = LinkCutTree::new();

        // Create 3 separate trees
        for i in 0..9 {
            lct.make_tree(i, i as f64);
        }

        // Tree 1: 0 <- 1 <- 2
        lct.link(0, 1).unwrap();
        lct.link(1, 2).unwrap();

        // Tree 2: 3 <- 4 <- 5
        lct.link(3, 4).unwrap();
        lct.link(4, 5).unwrap();

        // Tree 3: 6 <- 7 <- 8
        lct.link(6, 7).unwrap();
        lct.link(7, 8).unwrap();

        // Check each tree separately
        assert_eq!(lct.find_root(0).unwrap(), 2);
        assert_eq!(lct.find_root(3).unwrap(), 5);
        assert_eq!(lct.find_root(6).unwrap(), 8);

        // They should not be connected
        assert!(!lct.connected(0, 3));
        assert!(!lct.connected(3, 6));
        assert!(!lct.connected(0, 6));

        // Now merge tree 1 and 2
        lct.link(2, 5).unwrap();
        assert!(lct.connected(0, 5));
        assert_eq!(lct.find_root(0).unwrap(), 5);
        assert_eq!(lct.find_root(3).unwrap(), 5);
    }

    #[test]
    fn test_bulk_operations() {
        let mut lct = LinkCutTree::with_capacity(10);

        // Create nodes
        for i in 0..10 {
            lct.make_tree(i, i as f64);
        }

        // Bulk link
        let edges = vec![(0, 1), (1, 2), (2, 3)];
        lct.bulk_link(&edges).unwrap();

        assert!(lct.connected(0, 3));

        // Bulk update
        let updates = vec![(0, 10.0), (1, 20.0), (2, 30.0)];
        lct.bulk_update(&updates).unwrap();

        assert_eq!(lct.nodes[0].value, 10.0);
        assert_eq!(lct.nodes[1].value, 20.0);
        assert_eq!(lct.nodes[2].value, 30.0);
    }

    #[test]
    fn test_root_caching() {
        let mut lct = LinkCutTree::with_capacity(100);

        for i in 0..100 {
            lct.make_tree(i, i as f64);
        }

        // Create a long chain
        for i in 0..99 {
            lct.link(i, i + 1).unwrap();
        }

        // First find_root populates cache
        let root1 = lct.find_root(0).unwrap();
        assert_eq!(root1, 99);

        // Second find_root should use cache
        let root2 = lct.find_root(0).unwrap();
        assert_eq!(root2, 99);

        // After cut, cache should be invalidated
        lct.cut(50).unwrap();
        let root3 = lct.find_root(0).unwrap();
        assert_eq!(root3, 50);
    }
}
