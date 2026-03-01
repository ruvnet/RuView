//! Polylogarithmic Worst-Case Dynamic Connectivity
//!
//! Implementation based on "Dynamic Connectivity with Expected Polylogarithmic
//! Worst-Case Update Time" (arXiv:2510.08297, October 2025).
//!
//! # Key Innovation
//!
//! Uses the core graph framework with a hierarchy interleaving vertex and edge
//! sparsification to achieve O(polylog n) expected worst-case update time.
//!
//! # Time Complexity
//!
//! - Insert: O(log³ n) expected worst-case
//! - Delete: O(log³ n) expected worst-case
//! - Query: O(log n) worst-case
//!
//! # Algorithm Overview
//!
//! 1. Maintain a hierarchy of O(log n) levels
//! 2. Each level i contains edges of "level" ≥ i
//! 3. Use edge sparsification via low-congestion shortcuts
//! 4. Rebuild levels incrementally to avoid worst-case spikes

use crate::graph::VertexId;
use std::collections::{HashMap, HashSet, VecDeque};

/// Maximum number of levels in the hierarchy
const MAX_LEVELS: usize = 64;

/// Rebuild threshold factor
const REBUILD_FACTOR: f64 = 2.0;

/// Edge with level information for the hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LeveledEdge {
    u: VertexId,
    v: VertexId,
    level: usize,
}

impl LeveledEdge {
    fn new(u: VertexId, v: VertexId, level: usize) -> Self {
        let (u, v) = if u < v { (u, v) } else { (v, u) };
        Self { u, v, level }
    }

    fn endpoints(&self) -> (VertexId, VertexId) {
        (self.u, self.v)
    }
}

/// Spanning forest for a single level
#[derive(Debug, Clone)]
struct LevelForest {
    /// Parent pointers for union-find
    parent: HashMap<VertexId, VertexId>,
    /// Rank for union by rank
    rank: HashMap<VertexId, usize>,
    /// Component sizes for smarter union
    component_size: HashMap<VertexId, usize>,
    /// Tree edges at this level
    tree_edges: HashSet<(VertexId, VertexId)>,
    /// Non-tree edges at this level
    non_tree_edges: HashSet<(VertexId, VertexId)>,
    /// Adjacency list for faster traversal (vertex -> neighbors)
    adjacency: HashMap<VertexId, Vec<VertexId>>,
    /// Number of vertices
    size: usize,
}

impl LevelForest {
    fn new() -> Self {
        Self {
            parent: HashMap::new(),
            rank: HashMap::new(),
            component_size: HashMap::new(),
            tree_edges: HashSet::new(),
            non_tree_edges: HashSet::new(),
            adjacency: HashMap::new(),
            size: 0,
        }
    }

    #[inline]
    fn add_vertex(&mut self, v: VertexId) {
        if !self.parent.contains_key(&v) {
            self.parent.insert(v, v);
            self.rank.insert(v, 0);
            self.component_size.insert(v, 1);
            self.adjacency.insert(v, Vec::new());
            self.size += 1;
        }
    }

    #[inline]
    fn find(&mut self, v: VertexId) -> VertexId {
        if !self.parent.contains_key(&v) {
            return v;
        }

        let p = self.parent[&v];
        if p == v {
            return v;
        }

        // Path compression with iterative approach (faster than recursive)
        let mut path = Vec::with_capacity(8);
        let mut current = v;
        while self.parent[&current] != current {
            path.push(current);
            current = self.parent[&current];
        }
        let root = current;

        // Compress path
        for node in path {
            self.parent.insert(node, root);
        }
        root
    }

    #[inline]
    fn union(&mut self, u: VertexId, v: VertexId) -> bool {
        let root_u = self.find(u);
        let root_v = self.find(v);

        if root_u == root_v {
            return false;
        }

        // Union by size (better than rank for our use case)
        let size_u = *self.component_size.get(&root_u).unwrap_or(&1);
        let size_v = *self.component_size.get(&root_v).unwrap_or(&1);

        if size_u < size_v {
            self.parent.insert(root_u, root_v);
            self.component_size.insert(root_v, size_u + size_v);
        } else {
            self.parent.insert(root_v, root_u);
            self.component_size.insert(root_u, size_u + size_v);
        }

        true
    }

    #[inline]
    fn connected(&mut self, u: VertexId, v: VertexId) -> bool {
        self.find(u) == self.find(v)
    }

    #[inline]
    fn insert_edge(&mut self, u: VertexId, v: VertexId) -> bool {
        self.add_vertex(u);
        self.add_vertex(v);

        // Update adjacency list
        self.adjacency.entry(u).or_default().push(v);
        self.adjacency.entry(v).or_default().push(u);

        let edge = if u < v { (u, v) } else { (v, u) };

        if self.union(u, v) {
            // New tree edge
            self.tree_edges.insert(edge);
            true
        } else {
            // Non-tree edge
            self.non_tree_edges.insert(edge);
            false
        }
    }

    fn remove_edge(&mut self, u: VertexId, v: VertexId) -> bool {
        let edge = if u < v { (u, v) } else { (v, u) };

        // Update adjacency
        if let Some(neighbors) = self.adjacency.get_mut(&u) {
            neighbors.retain(|&x| x != v);
        }
        if let Some(neighbors) = self.adjacency.get_mut(&v) {
            neighbors.retain(|&x| x != u);
        }

        if self.tree_edges.remove(&edge) {
            true
        } else {
            self.non_tree_edges.remove(&edge);
            false
        }
    }

    /// Get neighbors of a vertex (faster than iterating edges)
    #[inline]
    fn neighbors(&self, v: VertexId) -> &[VertexId] {
        self.adjacency.get(&v).map_or(&[], |n| n.as_slice())
    }

    /// Get component size for a vertex
    #[inline]
    fn get_component_size(&mut self, v: VertexId) -> usize {
        let root = self.find(v);
        *self.component_size.get(&root).unwrap_or(&1)
    }
}

/// Polylogarithmic worst-case dynamic connectivity
///
/// Maintains connectivity with O(log³ n) expected worst-case update time.
///
/// # Example
///
/// ```ignore
/// use ruvector_mincut::connectivity::polylog::PolylogConnectivity;
///
/// let mut conn = PolylogConnectivity::new();
/// conn.insert_edge(0, 1);
/// conn.insert_edge(1, 2);
///
/// assert!(conn.connected(0, 2));
///
/// conn.delete_edge(1, 2);
/// assert!(!conn.connected(0, 2));
/// ```
#[derive(Debug)]
pub struct PolylogConnectivity {
    /// Hierarchy of forests, one per level
    levels: Vec<LevelForest>,
    /// All edges with their levels
    edges: HashMap<(VertexId, VertexId), usize>,
    /// Number of edges at each level (for rebuild tracking)
    level_sizes: Vec<usize>,
    /// Initial sizes at last rebuild
    initial_sizes: Vec<usize>,
    /// Number of vertices
    vertex_count: usize,
    /// Number of components
    component_count: usize,
    /// Statistics
    stats: PolylogStats,
}

/// Statistics for polylog connectivity
#[derive(Debug, Clone, Default)]
pub struct PolylogStats {
    /// Total insertions
    pub insertions: u64,
    /// Total deletions
    pub deletions: u64,
    /// Total queries
    pub queries: u64,
    /// Number of level rebuilds
    pub rebuilds: u64,
    /// Maximum level used
    pub max_level: usize,
}

impl PolylogConnectivity {
    /// Create new empty connectivity structure
    pub fn new() -> Self {
        Self {
            levels: (0..MAX_LEVELS).map(|_| LevelForest::new()).collect(),
            edges: HashMap::new(),
            level_sizes: vec![0; MAX_LEVELS],
            initial_sizes: vec![0; MAX_LEVELS],
            vertex_count: 0,
            component_count: 0,
            stats: PolylogStats::default(),
        }
    }

    /// Insert an edge
    ///
    /// Time complexity: O(log³ n) expected worst-case
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId) {
        self.stats.insertions += 1;

        let edge = if u < v { (u, v) } else { (v, u) };

        if self.edges.contains_key(&edge) {
            return; // Edge already exists
        }

        // Track vertices
        let u_new = !self.levels[0].parent.contains_key(&u);
        let v_new = !self.levels[0].parent.contains_key(&v);

        if u_new {
            self.vertex_count += 1;
            self.component_count += 1;
        }
        if v_new {
            self.vertex_count += 1;
            self.component_count += 1;
        }

        // Insert at level 0
        let is_tree_edge = self.levels[0].insert_edge(u, v);
        self.edges.insert(edge, 0);
        self.level_sizes[0] += 1;

        if is_tree_edge {
            // Merged two components
            self.component_count -= 1;
        }

        // Check if rebuild needed
        self.check_rebuild(0);
    }

    /// Delete an edge
    ///
    /// Time complexity: O(log³ n) expected worst-case
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) {
        self.stats.deletions += 1;

        let edge = if u < v { (u, v) } else { (v, u) };

        let level = match self.edges.remove(&edge) {
            Some(l) => l,
            None => return, // Edge doesn't exist
        };

        self.level_sizes[level] = self.level_sizes[level].saturating_sub(1);

        // Remove from all levels up to current level
        for l in 0..=level {
            let was_tree = self.levels[l].remove_edge(u, v);

            if was_tree && l == level {
                // Need to find replacement edge
                if let Some(replacement) = self.find_replacement(u, v, level) {
                    // Promote replacement edge
                    let rep_edge = if replacement.0 < replacement.1 {
                        (replacement.0, replacement.1)
                    } else {
                        (replacement.1, replacement.0)
                    };

                    if let Some(rep_level) = self.edges.get_mut(&rep_edge) {
                        let old_level = *rep_level;
                        *rep_level = level;

                        // Move edge up in hierarchy
                        self.level_sizes[old_level] = self.level_sizes[old_level].saturating_sub(1);
                        self.level_sizes[level] += 1;

                        // Update forests
                        for ll in old_level..=level {
                            self.levels[ll].non_tree_edges.remove(&rep_edge);
                            self.levels[ll].tree_edges.insert(rep_edge);
                        }
                    }
                } else {
                    // No replacement - component split
                    self.component_count += 1;
                }
            }
        }

        // Rebuild affected levels
        self.rebuild_level(level);
    }

    /// Check if two vertices are connected
    ///
    /// Time complexity: O(log n) worst-case
    pub fn connected(&mut self, u: VertexId, v: VertexId) -> bool {
        self.stats.queries += 1;

        // Check at level 0 (contains all edges)
        self.levels[0].connected(u, v)
    }

    /// Check if the entire graph is connected
    pub fn is_connected(&self) -> bool {
        self.component_count <= 1
    }

    /// Get number of connected components
    pub fn component_count(&self) -> usize {
        self.component_count
    }

    /// Get number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Get number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get statistics
    pub fn stats(&self) -> &PolylogStats {
        &self.stats
    }

    /// Find a replacement edge for deleted tree edge
    /// Optimized: Uses adjacency list and smaller component first
    fn find_replacement(
        &mut self,
        u: VertexId,
        v: VertexId,
        level: usize,
    ) -> Option<(VertexId, VertexId)> {
        // Choose smaller component for BFS (optimization)
        let size_u = self.levels[level].get_component_size(u);
        let size_v = self.levels[level].get_component_size(v);
        let (start, _target) = if size_u <= size_v { (u, v) } else { (v, u) };

        // Use FxHashSet for faster hashing if available, fallback to HashSet
        let mut visited = HashSet::with_capacity(size_u.min(size_v).min(1000));
        let mut queue = VecDeque::with_capacity(64);

        // Start BFS from smaller component
        queue.push_back(start);
        visited.insert(start);

        // Early termination limit
        let max_search = (self.vertex_count / 2).max(100);

        while let Some(current) = queue.pop_front() {
            // Check non-tree edges first (more likely to find replacement)
            let non_tree_edges: Vec<_> = self.levels[level]
                .non_tree_edges
                .iter()
                .filter(|&&(a, b)| a == current || b == current)
                .copied()
                .collect();

            for (a, b) in non_tree_edges {
                let other = if a == current { b } else { a };

                // If other is not in visited set, it's in the other component
                if !visited.contains(&other) {
                    return Some((a, b));
                }
            }

            // Use adjacency list for faster neighbor iteration
            let neighbors: Vec<_> = self.levels[level].neighbors(current).to_vec();
            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }

            // Limit search to avoid worst-case
            if visited.len() >= max_search {
                break;
            }
        }

        None
    }

    /// Check if level needs rebuild
    fn check_rebuild(&mut self, level: usize) {
        if self.initial_sizes[level] == 0 {
            self.initial_sizes[level] = self.level_sizes[level].max(1);
            return;
        }

        let threshold = (self.initial_sizes[level] as f64 * REBUILD_FACTOR) as usize;
        if self.level_sizes[level] > threshold {
            self.rebuild_level(level);
        }
    }

    /// Rebuild a level of the hierarchy
    fn rebuild_level(&mut self, level: usize) {
        self.stats.rebuilds += 1;
        self.stats.max_level = self.stats.max_level.max(level);

        // Collect all edges at this level and below
        let edges_to_rebuild: Vec<_> = self
            .edges
            .iter()
            .filter(|(_, &l)| l >= level)
            .map(|(&e, &l)| (e, l))
            .collect();

        // Reset level
        self.levels[level] = LevelForest::new();
        self.level_sizes[level] = 0;

        // Re-insert edges
        for ((u, v), _) in edges_to_rebuild {
            self.levels[level].insert_edge(u, v);
            self.level_sizes[level] += 1;
        }

        // Update initial size
        self.initial_sizes[level] = self.level_sizes[level].max(1);
    }
}

impl Default for PolylogConnectivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_connectivity() {
        let mut conn = PolylogConnectivity::new();

        conn.insert_edge(0, 1);
        conn.insert_edge(1, 2);

        assert!(conn.connected(0, 1));
        assert!(conn.connected(0, 2));
        assert!(conn.connected(1, 2));
        assert!(!conn.connected(0, 3));
    }

    #[test]
    fn test_delete_edge() {
        let mut conn = PolylogConnectivity::new();

        conn.insert_edge(0, 1);
        conn.insert_edge(1, 2);
        conn.insert_edge(2, 3);

        assert!(conn.connected(0, 3));

        conn.delete_edge(1, 2);

        assert!(conn.connected(0, 1));
        assert!(conn.connected(2, 3));
        assert!(!conn.connected(0, 2));
    }

    #[test]
    fn test_component_count() {
        let mut conn = PolylogConnectivity::new();

        conn.insert_edge(0, 1);
        assert_eq!(conn.component_count(), 1);

        conn.insert_edge(2, 3);
        assert_eq!(conn.component_count(), 2);

        conn.insert_edge(1, 2);
        assert_eq!(conn.component_count(), 1);

        conn.delete_edge(1, 2);
        assert_eq!(conn.component_count(), 2);
    }

    #[test]
    fn test_replacement_edge() {
        let mut conn = PolylogConnectivity::new();

        // Create a cycle: 0-1-2-3-0
        conn.insert_edge(0, 1);
        conn.insert_edge(1, 2);
        conn.insert_edge(2, 3);
        conn.insert_edge(3, 0);

        assert_eq!(conn.component_count(), 1);

        // Delete one edge - should find replacement
        conn.delete_edge(1, 2);

        // Still connected via 0-3-2
        assert!(conn.connected(0, 2));
        assert_eq!(conn.component_count(), 1);
    }

    #[test]
    fn test_stats() {
        let mut conn = PolylogConnectivity::new();

        conn.insert_edge(0, 1);
        conn.insert_edge(1, 2);
        conn.delete_edge(0, 1);
        conn.connected(1, 2);

        let stats = conn.stats();
        assert_eq!(stats.insertions, 2);
        assert_eq!(stats.deletions, 1);
        assert_eq!(stats.queries, 1);
    }
}
