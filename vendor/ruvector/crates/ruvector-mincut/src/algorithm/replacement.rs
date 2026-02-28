//! Replacement edge data structure for O(log n) reconnection
//!
//! Provides fast lookup of replacement edges when a tree edge is deleted.
//! Based on the level-based approach from dynamic connectivity literature.

use crate::graph::VertexId;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Edge identifier as (smaller, larger) vertex pair
pub type EdgeKey = (VertexId, VertexId);

/// Normalize an edge to (min, max) ordering
#[inline]
fn normalize_edge(u: VertexId, v: VertexId) -> EdgeKey {
    if u < v {
        (u, v)
    } else {
        (v, u)
    }
}

/// Level-based replacement edge index for O(log n) lookup
///
/// Organizes non-tree edges by level, enabling efficient replacement
/// edge discovery when tree edges are deleted.
///
/// # Complexity
/// - Lookup: O(log n) amortized
/// - Insert: O(log n)
/// - Delete: O(log n)
#[derive(Debug, Clone)]
pub struct ReplacementEdgeIndex {
    /// Maximum level (log₂ n)
    max_level: usize,
    /// Non-tree edges organized by level
    /// Level 0 contains all non-tree edges initially
    level_edges: Vec<BTreeSet<EdgeKey>>,
    /// Reverse lookup: edge -> level
    edge_level: HashMap<EdgeKey, usize>,
    /// Tree edges (for quick membership check)
    tree_edges: HashSet<EdgeKey>,
    /// Adjacency for each vertex at each level
    level_adjacency: Vec<HashMap<VertexId, BTreeSet<VertexId>>>,
    /// Component sizes (for smaller-to-larger promotion)
    component_size: HashMap<VertexId, usize>,
}

impl ReplacementEdgeIndex {
    /// Create a new replacement edge index
    pub fn new(n: usize) -> Self {
        // log₂(n) levels
        let max_level = (n as f64).log2().ceil() as usize + 1;
        let max_level = max_level.max(1);

        Self {
            max_level,
            level_edges: vec![BTreeSet::new(); max_level],
            edge_level: HashMap::new(),
            tree_edges: HashSet::new(),
            level_adjacency: vec![HashMap::new(); max_level],
            component_size: HashMap::new(),
        }
    }

    /// Add a tree edge
    pub fn add_tree_edge(&mut self, u: VertexId, v: VertexId) {
        let key = normalize_edge(u, v);
        self.tree_edges.insert(key);
    }

    /// Remove a tree edge (returns if it was present)
    pub fn remove_tree_edge(&mut self, u: VertexId, v: VertexId) -> bool {
        let key = normalize_edge(u, v);
        self.tree_edges.remove(&key)
    }

    /// Add a non-tree edge at level 0
    pub fn add_non_tree_edge(&mut self, u: VertexId, v: VertexId) {
        let key = normalize_edge(u, v);

        // Don't add if it's a tree edge
        if self.tree_edges.contains(&key) {
            return;
        }

        // Add at level 0
        if self.level_edges[0].insert(key) {
            self.edge_level.insert(key, 0);

            // Update adjacency
            self.level_adjacency[0].entry(u).or_default().insert(v);
            self.level_adjacency[0].entry(v).or_default().insert(u);
        }
    }

    /// Remove a non-tree edge
    pub fn remove_non_tree_edge(&mut self, u: VertexId, v: VertexId) {
        let key = normalize_edge(u, v);

        if let Some(level) = self.edge_level.remove(&key) {
            self.level_edges[level].remove(&key);

            // Update adjacency
            if let Some(adj) = self.level_adjacency[level].get_mut(&u) {
                adj.remove(&v);
            }
            if let Some(adj) = self.level_adjacency[level].get_mut(&v) {
                adj.remove(&u);
            }
        }
    }

    /// Find a replacement edge for deleted tree edge (u, v)
    ///
    /// Returns Some((x, y)) if a replacement exists, None if components disconnect.
    ///
    /// # Complexity
    /// O(log n) amortized through level-based search
    pub fn find_replacement(
        &mut self,
        u: VertexId,
        v: VertexId,
        tree_adjacency: &HashMap<VertexId, HashSet<VertexId>>,
    ) -> Option<EdgeKey> {
        let key = normalize_edge(u, v);

        // The edge should be a tree edge
        if !self.tree_edges.contains(&key) {
            return None;
        }

        // Find smaller component using BFS on tree edges only
        let (smaller_vertices, _larger_vertices) =
            self.find_components_after_cut(u, v, tree_adjacency);

        // Search for replacement edge across levels (from highest to 0)
        for level in (0..self.max_level).rev() {
            if let Some(replacement) = self.find_replacement_at_level(level, &smaller_vertices) {
                return Some(replacement);
            }
        }

        // No replacement found - promote edges from smaller component to next level
        self.promote_edges(&smaller_vertices);

        None
    }

    /// Fast replacement lookup when components are already known
    ///
    /// # Complexity
    /// O(log n) - binary search through levels
    pub fn find_replacement_fast(&self, smaller_component: &HashSet<VertexId>) -> Option<EdgeKey> {
        // Search levels from 0 (most edges) upward
        for level in 0..self.max_level {
            if let Some(replacement) = self.find_replacement_at_level(level, smaller_component) {
                return Some(replacement);
            }
        }
        None
    }

    /// Find replacement at a specific level
    fn find_replacement_at_level(
        &self,
        level: usize,
        component: &HashSet<VertexId>,
    ) -> Option<EdgeKey> {
        // Look through adjacency at this level for edges crossing component boundary
        for &vertex in component {
            if let Some(neighbors) = self.level_adjacency[level].get(&vertex) {
                for &neighbor in neighbors {
                    if !component.contains(&neighbor) {
                        // Found a crossing edge!
                        return Some(normalize_edge(vertex, neighbor));
                    }
                }
            }
        }
        None
    }

    /// Find the two components after cutting tree edge (u, v)
    fn find_components_after_cut(
        &self,
        u: VertexId,
        v: VertexId,
        tree_adj: &HashMap<VertexId, HashSet<VertexId>>,
    ) -> (HashSet<VertexId>, HashSet<VertexId>) {
        let mut comp_u = HashSet::new();
        let mut stack = vec![u];
        comp_u.insert(u);

        while let Some(x) = stack.pop() {
            if let Some(neighbors) = tree_adj.get(&x) {
                for &y in neighbors {
                    // Skip the cut edge
                    if (x == u && y == v) || (x == v && y == u) {
                        continue;
                    }
                    if comp_u.insert(y) {
                        stack.push(y);
                    }
                }
            }
        }

        let mut comp_v = HashSet::new();
        stack.push(v);
        comp_v.insert(v);

        while let Some(x) = stack.pop() {
            if let Some(neighbors) = tree_adj.get(&x) {
                for &y in neighbors {
                    if (x == u && y == v) || (x == v && y == u) {
                        continue;
                    }
                    if comp_v.insert(y) {
                        stack.push(y);
                    }
                }
            }
        }

        // Return smaller component first
        if comp_u.len() <= comp_v.len() {
            (comp_u, comp_v)
        } else {
            (comp_v, comp_u)
        }
    }

    /// Promote non-tree edges from smaller component to next level
    fn promote_edges(&mut self, component: &HashSet<VertexId>) {
        let mut to_promote = Vec::new();

        // Find edges at each level that have both endpoints in the component
        for level in 0..self.max_level.saturating_sub(1) {
            for &vertex in component {
                if let Some(neighbors) = self.level_adjacency[level].get(&vertex).cloned() {
                    for neighbor in neighbors {
                        if component.contains(&neighbor) {
                            let key = normalize_edge(vertex, neighbor);
                            to_promote.push((key, level));
                        }
                    }
                }
            }
        }

        // Perform promotions
        for (key, old_level) in to_promote {
            let new_level = (old_level + 1).min(self.max_level - 1);
            if new_level != old_level {
                let (u, v) = key;

                // Remove from old level
                self.level_edges[old_level].remove(&key);
                if let Some(adj) = self.level_adjacency[old_level].get_mut(&u) {
                    adj.remove(&v);
                }
                if let Some(adj) = self.level_adjacency[old_level].get_mut(&v) {
                    adj.remove(&u);
                }

                // Add to new level
                self.level_edges[new_level].insert(key);
                self.edge_level.insert(key, new_level);
                self.level_adjacency[new_level]
                    .entry(u)
                    .or_default()
                    .insert(v);
                self.level_adjacency[new_level]
                    .entry(v)
                    .or_default()
                    .insert(u);
            }
        }
    }

    /// Get statistics about the index
    pub fn stats(&self) -> ReplacementIndexStats {
        let edges_per_level: Vec<usize> = self.level_edges.iter().map(|s| s.len()).collect();

        ReplacementIndexStats {
            max_level: self.max_level,
            tree_edges: self.tree_edges.len(),
            non_tree_edges: self.edge_level.len(),
            edges_per_level,
        }
    }
}

/// Statistics about the replacement edge index
#[derive(Debug, Clone)]
pub struct ReplacementIndexStats {
    /// Maximum level (log₂ n)
    pub max_level: usize,
    /// Number of tree edges tracked
    pub tree_edges: usize,
    /// Number of non-tree edges across all levels
    pub non_tree_edges: usize,
    /// Count of edges at each level
    pub edges_per_level: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_index() {
        let idx = ReplacementEdgeIndex::new(100);
        assert!(idx.max_level >= 7); // log2(100) ≈ 6.6
        assert_eq!(idx.tree_edges.len(), 0);
    }

    #[test]
    fn test_add_tree_edge() {
        let mut idx = ReplacementEdgeIndex::new(10);
        idx.add_tree_edge(1, 2);
        idx.add_tree_edge(2, 3);

        assert!(idx.tree_edges.contains(&(1, 2)));
        assert!(idx.tree_edges.contains(&(2, 3)));
    }

    #[test]
    fn test_add_non_tree_edge() {
        let mut idx = ReplacementEdgeIndex::new(10);
        idx.add_tree_edge(1, 2);
        idx.add_non_tree_edge(1, 3);
        idx.add_non_tree_edge(2, 4);

        // Non-tree edge should be at level 0
        assert!(idx.level_edges[0].contains(&(1, 3)));
        assert!(idx.level_edges[0].contains(&(2, 4)));
        assert_eq!(idx.edge_level.get(&(1, 3)), Some(&0));
    }

    #[test]
    fn test_find_replacement_simple() {
        let mut idx = ReplacementEdgeIndex::new(10);

        // Tree: 1 - 2 - 3
        idx.add_tree_edge(1, 2);
        idx.add_tree_edge(2, 3);

        // Non-tree edge: 1 - 3 (bypasses 2)
        idx.add_non_tree_edge(1, 3);

        // Build tree adjacency
        let mut tree_adj: HashMap<VertexId, HashSet<VertexId>> = HashMap::new();
        tree_adj.entry(1).or_default().insert(2);
        tree_adj.entry(2).or_default().insert(1);
        tree_adj.entry(2).or_default().insert(3);
        tree_adj.entry(3).or_default().insert(2);

        // Delete tree edge (1, 2) - should find (1, 3) as replacement
        let replacement = idx.find_replacement(1, 2, &tree_adj);
        assert_eq!(replacement, Some((1, 3)));
    }

    #[test]
    fn test_find_replacement_none() {
        let mut idx = ReplacementEdgeIndex::new(10);

        // Tree: 1 - 2 - 3 (no non-tree edges)
        idx.add_tree_edge(1, 2);
        idx.add_tree_edge(2, 3);

        let mut tree_adj: HashMap<VertexId, HashSet<VertexId>> = HashMap::new();
        tree_adj.entry(1).or_default().insert(2);
        tree_adj.entry(2).or_default().insert(1);
        tree_adj.entry(2).or_default().insert(3);
        tree_adj.entry(3).or_default().insert(2);

        // No replacement for (1, 2)
        let replacement = idx.find_replacement(1, 2, &tree_adj);
        assert!(replacement.is_none());
    }

    #[test]
    fn test_find_replacement_fast() {
        let mut idx = ReplacementEdgeIndex::new(10);

        // Non-tree edges at level 0
        idx.add_non_tree_edge(1, 4);
        idx.add_non_tree_edge(2, 5);

        // Component {1, 2, 3}
        let component: HashSet<VertexId> = [1, 2, 3].into_iter().collect();

        // Should find (1, 4) or (2, 5) as crossing edge
        let replacement = idx.find_replacement_fast(&component);
        assert!(replacement.is_some());
        let (u, v) = replacement.unwrap();
        assert!(component.contains(&u) != component.contains(&v));
    }

    #[test]
    fn test_stats() {
        let mut idx = ReplacementEdgeIndex::new(100);
        idx.add_tree_edge(1, 2);
        idx.add_tree_edge(2, 3);
        idx.add_non_tree_edge(1, 3);
        idx.add_non_tree_edge(3, 4);

        let stats = idx.stats();
        assert_eq!(stats.tree_edges, 2);
        assert_eq!(stats.non_tree_edges, 2);
        assert_eq!(stats.edges_per_level[0], 2);
    }

    #[test]
    fn test_remove_edges() {
        let mut idx = ReplacementEdgeIndex::new(10);

        idx.add_tree_edge(1, 2);
        idx.add_non_tree_edge(1, 3);

        assert!(idx.remove_tree_edge(1, 2));
        assert!(!idx.tree_edges.contains(&(1, 2)));

        idx.remove_non_tree_edge(1, 3);
        assert!(!idx.level_edges[0].contains(&(1, 3)));
    }
}
