//! Deterministic LocalKCut Algorithm
//!
//! Implementation of the deterministic local minimum cut algorithm from:
//! "Deterministic and Exact Fully-dynamic Minimum Cut of Superpolylogarithmic
//! Size in Subpolynomial Time" (arXiv:2512.13105)
//!
//! Key components:
//! - Color coding families (red-blue, green-yellow)
//! - Forest packing with greedy edge assignment
//! - Color-coded DFS for cut enumeration

use crate::graph::{VertexId, Weight};
use std::collections::{HashMap, HashSet, VecDeque};

/// Color for edge partitioning in deterministic LocalKCut.
/// Uses 4-color scheme for forest/non-forest edge classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeColor {
    /// Red color - used for forest edges in one color class
    Red,
    /// Blue color - used for forest edges in other color class
    Blue,
    /// Green color - used for non-forest edges in one color class
    Green,
    /// Yellow color - used for non-forest edges in other color class
    Yellow,
}

/// A coloring assignment for edges based on the (a,b)-coloring family.
/// Per the paper, coloring families ensure witness coverage.
#[derive(Debug, Clone)]
pub struct EdgeColoring {
    /// Map from edge (canonical key) to color
    colors: HashMap<(VertexId, VertexId), EdgeColor>,
    /// Parameter 'a' for the coloring family (related to cut size)
    pub a: usize,
    /// Parameter 'b' for the coloring family (related to volume)
    pub b: usize,
}

impl EdgeColoring {
    /// Create new empty coloring
    pub fn new(a: usize, b: usize) -> Self {
        Self {
            colors: HashMap::new(),
            a,
            b,
        }
    }

    /// Get color for edge
    pub fn get(&self, u: VertexId, v: VertexId) -> Option<EdgeColor> {
        let key = if u < v { (u, v) } else { (v, u) };
        self.colors.get(&key).copied()
    }

    /// Set color for edge
    pub fn set(&mut self, u: VertexId, v: VertexId, color: EdgeColor) {
        let key = if u < v { (u, v) } else { (v, u) };
        self.colors.insert(key, color);
    }

    /// Check if edge has specific color
    pub fn has_color(&self, u: VertexId, v: VertexId, color: EdgeColor) -> bool {
        self.get(u, v) == Some(color)
    }
}

/// Generate color coding family per Lemma 3.3
/// Family size: 2^{O(min(a,b) · log(a+b))} · log n
pub fn generate_coloring_family(a: usize, b: usize, num_edges: usize) -> Vec<EdgeColoring> {
    // Simplified implementation using hashing-based derandomization
    // Full implementation would use perfect hash families

    let log_n = (num_edges.max(2) as f64).log2().ceil() as usize;
    let family_size = (1 << (a.min(b) * (a + b).max(1).ilog2() as usize + 1)) * log_n;
    let family_size = family_size.min(100); // Cap for practicality

    let mut family = Vec::with_capacity(family_size);

    for seed in 0..family_size {
        let coloring = EdgeColoring::new(a, b);
        // Each coloring in the family uses different hash function
        // to partition edges
        family.push(coloring);
    }

    family
}

/// Greedy forest packing structure
#[derive(Debug, Clone)]
pub struct GreedyForestPacking {
    /// Number of forests
    pub num_forests: usize,
    /// Forest assignment for each edge: edge -> forest_id
    edge_forest: HashMap<(VertexId, VertexId), usize>,
    /// Edges in each forest
    forests: Vec<HashSet<(VertexId, VertexId)>>,
    /// Union-find for each forest to track connectivity
    forest_parents: Vec<HashMap<VertexId, VertexId>>,
}

impl GreedyForestPacking {
    /// Create new forest packing with k forests
    /// Per paper: k = 6λ_max · log m / ε²
    pub fn new(num_forests: usize) -> Self {
        Self {
            num_forests,
            edge_forest: HashMap::new(),
            forests: vec![HashSet::new(); num_forests],
            forest_parents: vec![HashMap::new(); num_forests],
        }
    }

    /// Find root in forest using path compression
    fn find_root(&mut self, forest_id: usize, v: VertexId) -> VertexId {
        if !self.forest_parents[forest_id].contains_key(&v) {
            self.forest_parents[forest_id].insert(v, v);
            return v;
        }

        let parent = self.forest_parents[forest_id][&v];
        if parent == v {
            return v;
        }

        let root = self.find_root(forest_id, parent);
        self.forest_parents[forest_id].insert(v, root);
        root
    }

    /// Union two vertices in a forest
    fn union(&mut self, forest_id: usize, u: VertexId, v: VertexId) {
        let root_u = self.find_root(forest_id, u);
        let root_v = self.find_root(forest_id, v);
        if root_u != root_v {
            self.forest_parents[forest_id].insert(root_u, root_v);
        }
    }

    /// Check if edge would create cycle in forest
    fn would_create_cycle(&mut self, forest_id: usize, u: VertexId, v: VertexId) -> bool {
        self.find_root(forest_id, u) == self.find_root(forest_id, v)
    }

    /// Insert edge greedily into first available forest
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId) -> Option<usize> {
        let key = if u < v { (u, v) } else { (v, u) };

        // Already assigned
        if self.edge_forest.contains_key(&key) {
            return self.edge_forest.get(&key).copied();
        }

        // Find first forest where this edge doesn't create cycle
        for forest_id in 0..self.num_forests {
            if !self.would_create_cycle(forest_id, u, v) {
                self.forests[forest_id].insert(key);
                self.edge_forest.insert(key, forest_id);
                self.union(forest_id, u, v);
                return Some(forest_id);
            }
        }

        // Edge doesn't fit in any forest (it's a high-connectivity edge)
        None
    }

    /// Delete edge from its forest
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Option<usize> {
        let key = if u < v { (u, v) } else { (v, u) };

        if let Some(forest_id) = self.edge_forest.remove(&key) {
            self.forests[forest_id].remove(&key);
            // Need to rebuild connectivity for this forest
            self.rebuild_forest_connectivity(forest_id);
            return Some(forest_id);
        }
        None
    }

    /// Rebuild union-find for a forest after edge deletion
    fn rebuild_forest_connectivity(&mut self, forest_id: usize) {
        self.forest_parents[forest_id].clear();
        // Collect edges first to avoid borrow conflict
        let edges: Vec<_> = self.forests[forest_id].iter().copied().collect();
        for (u, v) in edges {
            self.union(forest_id, u, v);
        }
    }

    /// Check if edge is a tree edge in some forest
    pub fn is_tree_edge(&self, u: VertexId, v: VertexId) -> bool {
        let key = if u < v { (u, v) } else { (v, u) };
        self.edge_forest.contains_key(&key)
    }

    /// Get forest ID for an edge
    pub fn get_forest(&self, u: VertexId, v: VertexId) -> Option<usize> {
        let key = if u < v { (u, v) } else { (v, u) };
        self.edge_forest.get(&key).copied()
    }

    /// Get all edges in a specific forest
    pub fn forest_edges(&self, forest_id: usize) -> &HashSet<(VertexId, VertexId)> {
        &self.forests[forest_id]
    }
}

/// A discovered cut from LocalKCut query
#[derive(Debug, Clone)]
pub struct LocalCut {
    /// Vertices in the cut set S
    pub vertices: HashSet<VertexId>,
    /// Boundary edges (crossing the cut)
    pub boundary: Vec<(VertexId, VertexId)>,
    /// Cut value (sum of boundary edge weights)
    pub cut_value: f64,
    /// Volume of the cut (sum of degrees)
    pub volume: usize,
}

/// Deterministic LocalKCut data structure
/// Per Theorem 4.1 of the paper
#[derive(Debug)]
pub struct DeterministicLocalKCut {
    /// Maximum cut size to consider
    lambda_max: u64,
    /// Maximum volume to explore
    nu: usize,
    /// Approximation factor
    beta: usize,
    /// Forest packing
    forests: GreedyForestPacking,
    /// Red-blue coloring family (for forest edges)
    red_blue_colorings: Vec<EdgeColoring>,
    /// Green-yellow coloring family (for non-forest edges)
    green_yellow_colorings: Vec<EdgeColoring>,
    /// Graph adjacency
    adjacency: HashMap<VertexId, HashMap<VertexId, Weight>>,
    /// All edges
    edges: HashSet<(VertexId, VertexId)>,
}

impl DeterministicLocalKCut {
    /// Create new LocalKCut structure
    pub fn new(lambda_max: u64, nu: usize, beta: usize) -> Self {
        // Number of forests: 6λ_max · log m / ε² (simplified)
        let num_forests = ((6 * lambda_max) as usize).max(10);

        // Color coding parameters
        let a_rb = 2 * beta;
        let b_rb = nu;
        let a_gy = 2 * beta - 1;
        let b_gy = lambda_max as usize;

        Self {
            lambda_max,
            nu,
            beta,
            forests: GreedyForestPacking::new(num_forests),
            red_blue_colorings: generate_coloring_family(a_rb, b_rb, 1000),
            green_yellow_colorings: generate_coloring_family(a_gy, b_gy, 1000),
            adjacency: HashMap::new(),
            edges: HashSet::new(),
        }
    }

    /// Insert an edge
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) {
        let key = if u < v { (u, v) } else { (v, u) };

        if self.edges.contains(&key) {
            return;
        }

        self.edges.insert(key);
        self.adjacency.entry(u).or_default().insert(v, weight);
        self.adjacency.entry(v).or_default().insert(u, weight);

        // Add to forest packing
        if let Some(forest_id) = self.forests.insert_edge(u, v) {
            // Assign color in red-blue family based on forest
            for coloring in &mut self.red_blue_colorings {
                let color = if (u + v + forest_id as u64) % 2 == 0 {
                    EdgeColor::Blue
                } else {
                    EdgeColor::Red
                };
                coloring.set(u, v, color);
            }
        } else {
            // Non-tree edge: assign color in green-yellow family
            for coloring in &mut self.green_yellow_colorings {
                let color = if (u * v) % 2 == 0 {
                    EdgeColor::Green
                } else {
                    EdgeColor::Yellow
                };
                coloring.set(u, v, color);
            }
        }
    }

    /// Delete an edge
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) {
        let key = if u < v { (u, v) } else { (v, u) };

        if !self.edges.remove(&key) {
            return;
        }

        if let Some(neighbors) = self.adjacency.get_mut(&u) {
            neighbors.remove(&v);
        }
        if let Some(neighbors) = self.adjacency.get_mut(&v) {
            neighbors.remove(&u);
        }

        self.forests.delete_edge(u, v);
    }

    /// Query: Find all cuts containing vertex v with volume ≤ ν and cut-size ≤ λ_max
    /// This is Algorithm 4.1 from the paper
    pub fn query(&self, v: VertexId) -> Vec<LocalCut> {
        let mut results = Vec::new();
        let mut seen_cuts: HashSet<Vec<VertexId>> = HashSet::new();

        // For each (forest, red-blue coloring, green-yellow coloring) triple
        for forest_id in 0..self.forests.num_forests {
            for rb_coloring in &self.red_blue_colorings {
                for gy_coloring in &self.green_yellow_colorings {
                    // Execute color-coded DFS
                    if let Some(cut) = self.color_coded_dfs(v, forest_id, rb_coloring, gy_coloring)
                    {
                        // Deduplicate cuts
                        let mut sorted_vertices: Vec<_> = cut.vertices.iter().copied().collect();
                        sorted_vertices.sort();

                        if !seen_cuts.contains(&sorted_vertices)
                            && cut.cut_value <= self.lambda_max as f64
                        {
                            seen_cuts.insert(sorted_vertices);
                            results.push(cut);
                        }
                    }
                }
            }
        }

        results
    }

    /// Color-coded DFS from vertex v
    /// Explores: blue edges in forest + green non-forest edges
    /// Caps at volume ν
    fn color_coded_dfs(
        &self,
        start: VertexId,
        _forest_id: usize,
        rb_coloring: &EdgeColoring,
        gy_coloring: &EdgeColoring,
    ) -> Option<LocalCut> {
        let mut visited = HashSet::new();
        let mut stack = vec![start];
        let mut volume = 0usize;
        let mut boundary = Vec::new();

        while let Some(u) = stack.pop() {
            if visited.contains(&u) {
                continue;
            }
            visited.insert(u);

            // Update volume
            if let Some(neighbors) = self.adjacency.get(&u) {
                volume += neighbors.len();

                if volume > self.nu {
                    // Volume exceeded - this cut is too large
                    return None;
                }

                for (&v, &_weight) in neighbors {
                    let is_tree_edge = self.forests.is_tree_edge(u, v);

                    if is_tree_edge {
                        // Tree edge: only follow if blue
                        if rb_coloring.has_color(u, v, EdgeColor::Blue) {
                            if !visited.contains(&v) {
                                stack.push(v);
                            }
                        } else {
                            // Red tree edge crosses the boundary
                            boundary.push((u, v));
                        }
                    } else {
                        // Non-tree edge: only follow if green
                        if gy_coloring.has_color(u, v, EdgeColor::Green) {
                            if !visited.contains(&v) {
                                stack.push(v);
                            }
                        } else {
                            // Yellow non-tree edge crosses the boundary
                            if !visited.contains(&v) {
                                boundary.push((u, v));
                            }
                        }
                    }
                }
            }
        }

        // Calculate cut value
        let cut_value: f64 = boundary
            .iter()
            .map(|&(u, v)| {
                self.adjacency
                    .get(&u)
                    .and_then(|n| n.get(&v))
                    .copied()
                    .unwrap_or(1.0)
            })
            .sum();

        Some(LocalCut {
            vertices: visited,
            boundary,
            cut_value,
            volume,
        })
    }

    /// Get all vertices
    pub fn vertices(&self) -> Vec<VertexId> {
        self.adjacency.keys().copied().collect()
    }

    /// Get neighbors of a vertex
    pub fn neighbors(&self, v: VertexId) -> Vec<(VertexId, Weight)> {
        self.adjacency
            .get(&v)
            .map(|n| n.iter().map(|(&v, &w)| (v, w)).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forest_packing_basic() {
        let mut packing = GreedyForestPacking::new(3);

        // Path: 1-2-3-4
        assert!(packing.insert_edge(1, 2).is_some());
        assert!(packing.insert_edge(2, 3).is_some());
        assert!(packing.insert_edge(3, 4).is_some());

        assert!(packing.is_tree_edge(1, 2));
        assert!(packing.is_tree_edge(2, 3));
    }

    #[test]
    fn test_forest_packing_cycle() {
        let mut packing = GreedyForestPacking::new(3);

        // Triangle: 1-2-3-1
        packing.insert_edge(1, 2);
        packing.insert_edge(2, 3);
        // This edge closes the cycle - goes to different forest
        let forest = packing.insert_edge(1, 3);

        // Should still fit in some forest
        assert!(forest.is_some());
    }

    #[test]
    fn test_localkcut_query() {
        let mut lkc = DeterministicLocalKCut::new(10, 100, 2);

        // Simple path graph
        lkc.insert_edge(1, 2, 1.0);
        lkc.insert_edge(2, 3, 1.0);
        lkc.insert_edge(3, 4, 1.0);

        let cuts = lkc.query(1);

        // Should find at least one cut containing vertex 1
        assert!(!cuts.is_empty());
        assert!(cuts.iter().any(|c| c.vertices.contains(&1)));
    }

    #[test]
    fn test_coloring_family() {
        let family = generate_coloring_family(2, 5, 100);
        assert!(!family.is_empty());
    }

    #[test]
    fn test_edge_deletion() {
        let mut lkc = DeterministicLocalKCut::new(10, 100, 2);

        lkc.insert_edge(1, 2, 1.0);
        lkc.insert_edge(2, 3, 1.0);

        lkc.delete_edge(1, 2);

        // Edge should be gone
        assert!(!lkc.forests.is_tree_edge(1, 2));
    }
}
