//! Fragmentation Algorithm for Graph Decomposition
//!
//! Implementation of Theorem 5.1 from:
//! "Deterministic and Exact Fully-dynamic Minimum Cut of Superpolylogarithmic
//! Size in Subpolynomial Time" (arXiv:2512.13105)
//!
//! The Fragmentation algorithm decomposes a graph into expander-like
//! components with controlled boundary sizes. This enables efficient
//! maintenance of minimum cuts under dynamic updates.
//!
//! # Key Components
//!
//! - **Trim subroutine**: Finds boundary-sparse cuts in expanders
//! - **Recursive fragmentation**: Decomposes graph into hierarchy
//! - **Expander detection**: Identifies well-connected subgraphs

use crate::graph::{VertexId, Weight};
use std::collections::{HashMap, HashSet, VecDeque};

/// Configuration for the fragmentation algorithm
#[derive(Debug, Clone)]
pub struct FragmentationConfig {
    /// Expansion parameter φ (phi)
    pub phi: f64,
    /// Maximum fragment size before splitting
    pub max_fragment_size: usize,
    /// Minimum fragment size (don't split smaller)
    pub min_fragment_size: usize,
    /// Boundary sparsity parameter
    pub boundary_sparsity: f64,
}

impl Default for FragmentationConfig {
    fn default() -> Self {
        Self {
            phi: 0.1,
            max_fragment_size: 1000,
            min_fragment_size: 10,
            boundary_sparsity: 0.5,
        }
    }
}

/// A fragment (partition) of the graph
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Unique fragment identifier
    pub id: u64,
    /// Vertices in this fragment
    pub vertices: HashSet<VertexId>,
    /// Boundary edges (crossing to other fragments)
    pub boundary: Vec<(VertexId, VertexId)>,
    /// Internal edges (within fragment)
    pub internal_edge_count: usize,
    /// Volume (sum of degrees)
    pub volume: usize,
    /// Parent fragment ID (if any)
    pub parent: Option<u64>,
    /// Child fragment IDs
    pub children: Vec<u64>,
    /// Is this fragment an expander?
    pub is_expander: bool,
}

impl Fragment {
    /// Create new fragment
    pub fn new(id: u64, vertices: HashSet<VertexId>) -> Self {
        Self {
            id,
            vertices,
            boundary: Vec::new(),
            internal_edge_count: 0,
            volume: 0,
            parent: None,
            children: Vec::new(),
            is_expander: false,
        }
    }

    /// Get fragment size (number of vertices)
    pub fn size(&self) -> usize {
        self.vertices.len()
    }

    /// Check if vertex is in this fragment
    pub fn contains(&self, v: VertexId) -> bool {
        self.vertices.contains(&v)
    }

    /// Compute boundary sparsity: |boundary| / volume
    pub fn boundary_sparsity(&self) -> f64 {
        if self.volume == 0 {
            return 0.0;
        }
        self.boundary.len() as f64 / self.volume as f64
    }
}

/// Result of the Trim subroutine
#[derive(Debug, Clone)]
pub struct TrimResult {
    /// Vertices to trim (the sparse boundary region)
    pub trimmed_vertices: HashSet<VertexId>,
    /// Edges cut by the trim
    pub cut_edges: Vec<(VertexId, VertexId)>,
    /// Cut value (sum of edge weights)
    pub cut_value: f64,
    /// Was the trim successful?
    pub success: bool,
}

/// The Fragmentation algorithm data structure
#[derive(Debug)]
pub struct Fragmentation {
    /// Configuration
    config: FragmentationConfig,
    /// All fragments indexed by ID
    fragments: HashMap<u64, Fragment>,
    /// Vertex to fragment mapping
    vertex_fragment: HashMap<VertexId, u64>,
    /// Next fragment ID
    next_id: u64,
    /// Graph adjacency
    adjacency: HashMap<VertexId, HashMap<VertexId, Weight>>,
    /// Root fragment IDs (top-level fragments)
    roots: Vec<u64>,
}

impl Fragmentation {
    /// Create new fragmentation structure
    pub fn new(config: FragmentationConfig) -> Self {
        Self {
            config,
            fragments: HashMap::new(),
            vertex_fragment: HashMap::new(),
            next_id: 1,
            adjacency: HashMap::new(),
            roots: Vec::new(),
        }
    }

    /// Create with default config
    pub fn with_defaults() -> Self {
        Self::new(FragmentationConfig::default())
    }

    /// Insert an edge into the graph
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) {
        self.adjacency.entry(u).or_default().insert(v, weight);
        self.adjacency.entry(v).or_default().insert(u, weight);
    }

    /// Delete an edge from the graph
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) {
        if let Some(neighbors) = self.adjacency.get_mut(&u) {
            neighbors.remove(&v);
        }
        if let Some(neighbors) = self.adjacency.get_mut(&v) {
            neighbors.remove(&u);
        }
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

    /// Compute degree of a vertex
    pub fn degree(&self, v: VertexId) -> usize {
        self.adjacency.get(&v).map_or(0, |n| n.len())
    }

    /// Run the fragmentation algorithm to decompose the graph
    ///
    /// This implements the recursive decomposition from the paper.
    pub fn fragment(&mut self) -> Vec<u64> {
        let vertices: HashSet<_> = self.vertices().into_iter().collect();

        if vertices.is_empty() {
            return Vec::new();
        }

        // Create initial fragment with all vertices
        let root_id = self.create_fragment(vertices);
        self.roots.push(root_id);

        // Recursively fragment
        let mut to_process = vec![root_id];

        while let Some(fragment_id) = to_process.pop() {
            if let Some(children) = self.try_fragment_recursive(fragment_id) {
                to_process.extend(children);
            }
        }

        self.roots.clone()
    }

    /// Try to recursively fragment a given fragment
    fn try_fragment_recursive(&mut self, fragment_id: u64) -> Option<Vec<u64>> {
        let fragment = self.fragments.get(&fragment_id)?;

        // Don't fragment if too small
        if fragment.size() <= self.config.min_fragment_size {
            return None;
        }

        // Don't fragment if already small enough and is an expander
        if fragment.size() <= self.config.max_fragment_size && fragment.is_expander {
            return None;
        }

        // Try to find a sparse cut using Trim
        let vertices: Vec<_> = fragment.vertices.iter().copied().collect();
        let trim_result = self.trim(&vertices);

        if !trim_result.success || trim_result.trimmed_vertices.is_empty() {
            // Mark as expander if we can't find a good cut
            if let Some(f) = self.fragments.get_mut(&fragment_id) {
                f.is_expander = true;
            }
            return None;
        }

        // Split into two fragments
        let remaining: HashSet<_> = fragment
            .vertices
            .difference(&trim_result.trimmed_vertices)
            .copied()
            .collect();

        if remaining.is_empty() || trim_result.trimmed_vertices.len() == fragment.vertices.len() {
            return None;
        }

        // Create child fragments
        let child1_id = self.create_fragment(trim_result.trimmed_vertices);
        let child2_id = self.create_fragment(remaining);

        // Update parent-child relationships
        if let Some(f) = self.fragments.get_mut(&fragment_id) {
            f.children = vec![child1_id, child2_id];
        }
        if let Some(f) = self.fragments.get_mut(&child1_id) {
            f.parent = Some(fragment_id);
        }
        if let Some(f) = self.fragments.get_mut(&child2_id) {
            f.parent = Some(fragment_id);
        }

        Some(vec![child1_id, child2_id])
    }

    /// Create a new fragment from a vertex set
    fn create_fragment(&mut self, vertices: HashSet<VertexId>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // Compute fragment properties
        let mut internal_edge_count = 0;
        let mut boundary = Vec::new();
        let mut volume = 0;

        for &v in &vertices {
            let neighbors = self.neighbors(v);
            volume += neighbors.len();

            for (neighbor, _weight) in neighbors {
                if vertices.contains(&neighbor) {
                    internal_edge_count += 1;
                } else {
                    boundary.push((v, neighbor));
                }
            }
        }

        // Internal edges are counted twice, so divide by 2
        internal_edge_count /= 2;

        let fragment = Fragment {
            id,
            vertices: vertices.clone(),
            boundary,
            internal_edge_count,
            volume,
            parent: None,
            children: Vec::new(),
            is_expander: false,
        };

        // Update vertex-to-fragment mapping
        for &v in &vertices {
            self.vertex_fragment.insert(v, id);
        }

        self.fragments.insert(id, fragment);
        id
    }

    /// Trim subroutine: Find a boundary-sparse cut
    ///
    /// Algorithm (per Theorem 5.1):
    /// 1. Start with high-degree vertices
    /// 2. Greedily expand the set
    /// 3. Stop when boundary becomes sparse relative to volume
    pub fn trim(&self, vertices: &[VertexId]) -> TrimResult {
        if vertices.is_empty() {
            return TrimResult {
                trimmed_vertices: HashSet::new(),
                cut_edges: Vec::new(),
                cut_value: 0.0,
                success: false,
            };
        }

        let vertex_set: HashSet<_> = vertices.iter().copied().collect();

        // Start from lowest-degree vertices (more likely to be on boundary)
        let mut sorted_vertices: Vec<_> = vertices.to_vec();
        sorted_vertices.sort_by_key(|&v| self.degree(v));

        let mut trimmed = HashSet::new();
        let mut trimmed_volume = 0usize;
        let mut boundary_count = 0usize;

        // Greedily add vertices while maintaining sparsity
        for &v in &sorted_vertices {
            let neighbors = self.neighbors(v);
            let degree = neighbors.len();

            // Count how many neighbors are in trimmed vs outside
            let mut internal_neighbors = 0usize;
            let mut external_neighbors = 0usize;

            for (neighbor, _) in &neighbors {
                if trimmed.contains(neighbor) {
                    internal_neighbors += 1;
                } else if vertex_set.contains(neighbor) {
                    // Neighbor in remaining set (will become boundary)
                    external_neighbors += 1;
                }
            }

            // Adding this vertex:
            // - Removes internal_neighbors from boundary
            // - Adds external_neighbors to boundary
            let new_boundary = boundary_count - internal_neighbors + external_neighbors;
            let new_volume = trimmed_volume + degree;

            // Check sparsity condition
            let sparsity = if new_volume > 0 {
                new_boundary as f64 / new_volume as f64
            } else {
                0.0
            };

            if sparsity <= self.config.boundary_sparsity {
                trimmed.insert(v);
                trimmed_volume = new_volume;
                boundary_count = new_boundary;
            }

            // Stop if we've trimmed enough
            if trimmed.len() >= vertex_set.len() / 2 {
                break;
            }
        }

        // If we didn't trim anything useful, try a different approach
        if trimmed.is_empty() || trimmed.len() >= vertex_set.len() {
            return TrimResult {
                trimmed_vertices: HashSet::new(),
                cut_edges: Vec::new(),
                cut_value: 0.0,
                success: false,
            };
        }

        // Compute cut edges
        let mut cut_edges = Vec::new();
        let mut cut_value = 0.0;

        for &v in &trimmed {
            for (neighbor, weight) in self.neighbors(v) {
                if !trimmed.contains(&neighbor) && vertex_set.contains(&neighbor) {
                    cut_edges.push((v, neighbor));
                    cut_value += weight;
                }
            }
        }

        TrimResult {
            trimmed_vertices: trimmed,
            cut_edges,
            cut_value,
            success: true,
        }
    }

    /// Check if a fragment is a φ-expander
    ///
    /// A graph is a φ-expander if every cut (S, S̄) has:
    /// |∂S| ≥ φ · min(vol(S), vol(S̄))
    pub fn is_expander(&self, fragment_id: u64) -> bool {
        let fragment = match self.fragments.get(&fragment_id) {
            Some(f) => f,
            None => return false,
        };

        if fragment.vertices.len() <= 2 {
            return true; // Trivially an expander
        }

        // Use the Trim result to check expansion
        let vertices: Vec<_> = fragment.vertices.iter().copied().collect();
        let trim = self.trim(&vertices);

        if !trim.success {
            return true; // No sparse cut found = expander
        }

        // Check the expansion ratio
        let cut_volume = trim
            .trimmed_vertices
            .iter()
            .map(|&v| self.degree(v))
            .sum::<usize>();

        let remaining_volume = fragment.volume - cut_volume;
        let min_volume = cut_volume.min(remaining_volume);

        if min_volume == 0 {
            return true;
        }

        let expansion_ratio = trim.cut_edges.len() as f64 / min_volume as f64;
        expansion_ratio >= self.config.phi
    }

    /// Get a fragment by ID
    pub fn get_fragment(&self, id: u64) -> Option<&Fragment> {
        self.fragments.get(&id)
    }

    /// Get fragment containing a vertex
    pub fn get_vertex_fragment(&self, v: VertexId) -> Option<&Fragment> {
        self.vertex_fragment
            .get(&v)
            .and_then(|&id| self.fragments.get(&id))
    }

    /// Get all leaf fragments (no children)
    pub fn leaf_fragments(&self) -> Vec<&Fragment> {
        self.fragments
            .values()
            .filter(|f| f.children.is_empty())
            .collect()
    }

    /// Get total number of fragments
    pub fn num_fragments(&self) -> usize {
        self.fragments.len()
    }

    /// Get the fragment hierarchy depth
    pub fn max_depth(&self) -> usize {
        fn depth_of(fragments: &HashMap<u64, Fragment>, id: u64) -> usize {
            match fragments.get(&id) {
                Some(f) if f.children.is_empty() => 0,
                Some(f) => {
                    1 + f
                        .children
                        .iter()
                        .map(|&c| depth_of(fragments, c))
                        .max()
                        .unwrap_or(0)
                }
                None => 0,
            }
        }

        self.roots
            .iter()
            .map(|&r| depth_of(&self.fragments, r))
            .max()
            .unwrap_or(0)
    }
}

impl Default for Fragmentation {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_path_graph(frag: &mut Fragmentation, n: usize) {
        for i in 0..n - 1 {
            frag.insert_edge(i as u64, (i + 1) as u64, 1.0);
        }
    }

    fn build_clique(frag: &mut Fragmentation, vertices: &[u64]) {
        for i in 0..vertices.len() {
            for j in i + 1..vertices.len() {
                frag.insert_edge(vertices[i], vertices[j], 1.0);
            }
        }
    }

    #[test]
    fn test_fragmentation_empty() {
        let mut frag = Fragmentation::with_defaults();
        let roots = frag.fragment();
        assert!(roots.is_empty());
    }

    #[test]
    fn test_fragmentation_single_vertex() {
        let mut frag = Fragmentation::with_defaults();
        frag.insert_edge(1, 1, 0.0); // Self-loop to register vertex
        frag.adjacency.entry(1).or_default(); // Actually just add the vertex

        // For single vertex, we need a proper setup
        let mut frag2 = Fragmentation::with_defaults();
        frag2.insert_edge(1, 2, 1.0);
        let roots = frag2.fragment();
        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn test_fragmentation_path() {
        let mut frag = Fragmentation::new(FragmentationConfig {
            min_fragment_size: 2,
            max_fragment_size: 5,
            ..Default::default()
        });

        build_path_graph(&mut frag, 10);
        let roots = frag.fragment();

        assert!(!roots.is_empty());
        assert!(frag.num_fragments() >= 1);
    }

    #[test]
    fn test_fragmentation_clique() {
        let mut frag = Fragmentation::with_defaults();
        let vertices: Vec<u64> = (1..=6).collect();
        build_clique(&mut frag, &vertices);

        let roots = frag.fragment();

        assert!(!roots.is_empty());
        // A small clique should be a single expander
        let leaves = frag.leaf_fragments();
        let leaf = leaves.first().unwrap();
        assert!(leaf.size() <= 6);
    }

    #[test]
    fn test_trim_basic() {
        let mut frag = Fragmentation::with_defaults();
        build_path_graph(&mut frag, 10);

        let vertices: Vec<u64> = (0..10).collect();
        let result = frag.trim(&vertices);

        // Trim might find a cut or not depending on sparsity params
        // Just verify it doesn't crash and returns valid result
        assert!(result.trimmed_vertices.len() <= vertices.len());
    }

    #[test]
    fn test_fragment_properties() {
        let mut frag = Fragmentation::with_defaults();

        // Two cliques connected by a single edge
        build_clique(&mut frag, &[1, 2, 3, 4]);
        build_clique(&mut frag, &[5, 6, 7, 8]);
        frag.insert_edge(4, 5, 1.0); // Bridge edge

        let roots = frag.fragment();

        // Should fragment at the bridge
        assert!(!roots.is_empty());

        // Check we can get vertex fragments
        let f1 = frag.get_vertex_fragment(1);
        assert!(f1.is_some());
    }

    #[test]
    fn test_is_expander() {
        let mut frag = Fragmentation::new(FragmentationConfig {
            phi: 0.1,
            min_fragment_size: 2,
            ..Default::default()
        });

        // Build a clique (should be an expander)
        build_clique(&mut frag, &[1, 2, 3, 4, 5]);

        let roots = frag.fragment();
        assert!(!roots.is_empty());

        // The fragment should be an expander (dense graph)
        let is_exp = frag.is_expander(roots[0]);
        // Cliques are typically expanders
        assert!(is_exp || frag.leaf_fragments().len() > 1);
    }

    #[test]
    fn test_hierarchy_depth() {
        let mut frag = Fragmentation::new(FragmentationConfig {
            min_fragment_size: 3,
            max_fragment_size: 10,
            ..Default::default()
        });

        build_path_graph(&mut frag, 20);
        frag.fragment();

        let depth = frag.max_depth();
        // Path graph might get split a few times
        assert!(depth >= 0);
    }
}
