//! Fragmenting Algorithm for Dynamic Minimum Cut
//!
//! Handles graph fragmentation after edge deletions.
//! Recursively processes disconnected components.

use crate::connectivity::DynamicConnectivity;
use crate::graph::{DynamicGraph, EdgeId, VertexId};
use crate::instance::WitnessHandle;
use roaring::RoaringBitmap;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// A fragment (connected component) of the graph
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Fragment ID
    pub id: u64,
    /// Vertices in this fragment
    pub vertices: HashSet<VertexId>,
    /// Edges within this fragment
    pub edges: Vec<EdgeId>,
    /// Minimum cut value within this fragment
    pub min_cut: u64,
    /// Witness for the minimum cut
    pub witness: Option<WitnessHandle>,
}

/// Result of fragmenting algorithm
#[derive(Debug, Clone)]
pub enum FragmentResult {
    /// Graph is connected, single fragment
    Connected {
        /// Minimum cut value
        min_cut: u64,
        /// Witness for the minimum cut
        witness: WitnessHandle,
    },
    /// Graph has multiple fragments (disconnected)
    Disconnected {
        /// All fragments in the graph
        fragments: Vec<Fragment>,
        /// Global min cut is 0 when disconnected
        global_min_cut: u64,
    },
}

/// Fragmenting algorithm for handling graph decomposition
pub struct FragmentingAlgorithm {
    /// Reference to the graph
    graph: Arc<DynamicGraph>,
    /// Current fragments
    fragments: Vec<Fragment>,
    /// Vertex to fragment mapping
    vertex_fragment: HashMap<VertexId, u64>,
    /// Next fragment ID
    next_id: u64,
    /// Connectivity checker
    connectivity: DynamicConnectivity,
}

impl FragmentingAlgorithm {
    /// Create a new fragmenting algorithm instance
    pub fn new(graph: Arc<DynamicGraph>) -> Self {
        let mut alg = Self {
            graph,
            fragments: Vec::new(),
            vertex_fragment: HashMap::new(),
            next_id: 0,
            connectivity: DynamicConnectivity::new(),
        };
        alg.rebuild();
        alg
    }

    /// Rebuild fragment structure from scratch
    pub fn rebuild(&mut self) {
        self.fragments.clear();
        self.vertex_fragment.clear();
        self.next_id = 0;
        self.connectivity = DynamicConnectivity::new();

        // Build connectivity structure
        for edge in self.graph.edges() {
            self.connectivity.insert_edge(edge.source, edge.target);
        }

        // Find connected components using BFS
        let components = self.find_connected_components();

        // Create fragments for each component
        for component in components {
            self.create_fragment(component);
        }
    }

    /// Find all connected components
    fn find_connected_components(&self) -> Vec<HashSet<VertexId>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        for vertex in self.graph.vertices() {
            if !visited.contains(&vertex) {
                let component = self.bfs_component(vertex, &mut visited);
                if !component.is_empty() {
                    components.push(component);
                }
            }
        }

        components
    }

    /// BFS to find a single connected component
    fn bfs_component(&self, start: VertexId, visited: &mut HashSet<VertexId>) -> HashSet<VertexId> {
        let mut component = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(start);
        visited.insert(start);
        component.insert(start);

        while let Some(current) = queue.pop_front() {
            for (neighbor, _edge_id) in self.graph.neighbors(current) {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                    component.insert(neighbor);
                }
            }
        }

        component
    }

    /// Create a fragment from a set of vertices
    fn create_fragment(&mut self, vertices: HashSet<VertexId>) {
        let fragment_id = self.next_id;
        self.next_id += 1;

        // Find edges within this fragment
        let edges: Vec<EdgeId> = self
            .graph
            .edges()
            .into_iter()
            .filter(|e| vertices.contains(&e.source) && vertices.contains(&e.target))
            .map(|e| e.id)
            .collect();

        // Compute minimum cut within fragment
        let (min_cut, witness) = self.compute_fragment_min_cut(&vertices);

        let fragment = Fragment {
            id: fragment_id,
            vertices: vertices.clone(),
            edges,
            min_cut,
            witness,
        };

        // Update vertex mapping
        for &v in &vertices {
            self.vertex_fragment.insert(v, fragment_id);
        }

        self.fragments.push(fragment);
    }

    /// Compute minimum cut within a fragment
    fn compute_fragment_min_cut(
        &self,
        vertices: &HashSet<VertexId>,
    ) -> (u64, Option<WitnessHandle>) {
        if vertices.len() <= 1 {
            return (u64::MAX, None);
        }

        // For small fragments, use brute force
        if vertices.len() < 20 {
            return self.brute_force_min_cut(vertices);
        }

        // For larger fragments, use heuristic
        self.heuristic_min_cut(vertices)
    }

    /// Brute force minimum cut for small fragments
    fn brute_force_min_cut(&self, vertices: &HashSet<VertexId>) -> (u64, Option<WitnessHandle>) {
        let vertex_vec: Vec<_> = vertices.iter().copied().collect();
        let n = vertex_vec.len();

        if n >= 20 {
            return (u64::MAX, None);
        }

        let mut min_cut = u64::MAX;
        let mut best_set = HashSet::new();

        // Enumerate all non-empty proper subsets
        let max_mask = 1u64 << n;
        for mask in 1..max_mask - 1 {
            let mut subset: HashSet<VertexId> = HashSet::new();
            for (i, &v) in vertex_vec.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    subset.insert(v);
                }
            }

            // Check if subset is connected within fragment
            if !self.is_connected_within(&subset, vertices) {
                continue;
            }

            // Compute boundary within fragment
            let boundary = self.compute_boundary_within(&subset, vertices);

            if boundary < min_cut {
                min_cut = boundary;
                best_set = subset;
            }
        }

        if min_cut == u64::MAX || best_set.is_empty() {
            return (u64::MAX, None);
        }

        // Create witness
        let membership: RoaringBitmap = best_set.iter().map(|&v| v as u32).collect();
        let seed = *best_set.iter().next().unwrap();
        let witness = WitnessHandle::new(seed, membership, min_cut);

        (min_cut, Some(witness))
    }

    /// Heuristic minimum cut for larger fragments
    fn heuristic_min_cut(&self, vertices: &HashSet<VertexId>) -> (u64, Option<WitnessHandle>) {
        // Use minimum degree as upper bound
        let mut min_degree = u64::MAX;
        let mut min_vertex = None;

        for &v in vertices {
            let degree = self
                .graph
                .neighbors(v)
                .into_iter()
                .filter(|(n, _)| vertices.contains(n))
                .count() as u64;

            if degree < min_degree {
                min_degree = degree;
                min_vertex = Some(v);
            }
        }

        if let Some(v) = min_vertex {
            let mut membership = RoaringBitmap::new();
            membership.insert(v as u32);
            let witness = WitnessHandle::new(v, membership, min_degree);
            return (min_degree, Some(witness));
        }

        (u64::MAX, None)
    }

    /// Check if subset is connected within fragment
    fn is_connected_within(
        &self,
        subset: &HashSet<VertexId>,
        fragment: &HashSet<VertexId>,
    ) -> bool {
        if subset.is_empty() {
            return true;
        }

        let start = *subset.iter().next().unwrap();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            for (neighbor, _edge_id) in self.graph.neighbors(current) {
                if subset.contains(&neighbor)
                    && fragment.contains(&neighbor)
                    && visited.insert(neighbor)
                {
                    queue.push_back(neighbor);
                }
            }
        }

        visited.len() == subset.len()
    }

    /// Compute boundary of subset within fragment
    fn compute_boundary_within(
        &self,
        subset: &HashSet<VertexId>,
        fragment: &HashSet<VertexId>,
    ) -> u64 {
        let mut boundary = 0u64;

        for edge in self.graph.edges().into_iter() {
            // Only count edges within the fragment
            if !fragment.contains(&edge.source) || !fragment.contains(&edge.target) {
                continue;
            }

            let src_in = subset.contains(&edge.source);
            let tgt_in = subset.contains(&edge.target);

            if src_in != tgt_in {
                boundary += 1;
            }
        }

        boundary
    }

    /// Handle edge insertion
    pub fn insert_edge(&mut self, _edge_id: EdgeId, u: VertexId, v: VertexId) {
        self.connectivity.insert_edge(u, v);

        // Check if this merges two fragments
        let u_frag = self.vertex_fragment.get(&u).copied();
        let v_frag = self.vertex_fragment.get(&v).copied();

        match (u_frag, v_frag) {
            (Some(uf), Some(vf)) if uf != vf => {
                // Merge fragments
                self.merge_fragments(uf, vf);
            }
            (None, Some(_)) | (Some(_), None) | (None, None) => {
                // New vertex or edge - rebuild
                self.rebuild();
            }
            _ => {
                // Same fragment - update min cut
                self.update_fragment_containing(u);
            }
        }
    }

    /// Handle edge deletion
    pub fn delete_edge(&mut self, _edge_id: EdgeId, u: VertexId, v: VertexId) {
        self.connectivity.delete_edge(u, v);

        // Check if this splits a fragment
        if !self.connectivity.connected(u, v) {
            // Fragment split - rebuild
            self.rebuild();
        } else {
            // Same fragment - update min cut
            self.update_fragment_containing(u);
        }
    }

    /// Merge two fragments
    fn merge_fragments(&mut self, _frag1_id: u64, _frag2_id: u64) {
        // Simple approach: rebuild
        // Optimization: could merge in place
        self.rebuild();
    }

    /// Update fragment containing vertex
    fn update_fragment_containing(&mut self, v: VertexId) {
        if let Some(&frag_id) = self.vertex_fragment.get(&v) {
            // Find the fragment and get its vertices
            let vertices = self
                .fragments
                .iter()
                .find(|f| f.id == frag_id)
                .map(|f| f.vertices.clone());

            if let Some(vertices) = vertices {
                // Compute min cut outside of borrow
                let (min_cut, witness) = self.compute_fragment_min_cut(&vertices);

                // Now update the fragment
                if let Some(frag) = self.fragments.iter_mut().find(|f| f.id == frag_id) {
                    frag.min_cut = min_cut;
                    frag.witness = witness;
                }
            }
        }
    }

    /// Query the current result
    pub fn query(&self) -> FragmentResult {
        if self.fragments.len() <= 1 {
            if let Some(frag) = self.fragments.first() {
                if let Some(ref witness) = frag.witness {
                    return FragmentResult::Connected {
                        min_cut: frag.min_cut,
                        witness: witness.clone(),
                    };
                }
            }
            // Empty or single vertex graph
            let mut membership = RoaringBitmap::new();
            membership.insert(0);
            return FragmentResult::Connected {
                min_cut: u64::MAX,
                witness: WitnessHandle::new(0, membership, u64::MAX),
            };
        }

        FragmentResult::Disconnected {
            fragments: self.fragments.clone(),
            global_min_cut: 0,
        }
    }

    /// Get number of fragments
    pub fn num_fragments(&self) -> usize {
        self.fragments.len()
    }

    /// Is graph connected?
    pub fn is_connected(&self) -> bool {
        self.fragments.len() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = Arc::new(DynamicGraph::new());
        let alg = FragmentingAlgorithm::new(graph);
        assert_eq!(alg.num_fragments(), 0);
    }

    #[test]
    fn test_connected_graph() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(1, 2, 1.0).unwrap();

        let alg = FragmentingAlgorithm::new(graph);
        assert_eq!(alg.num_fragments(), 1);
        assert!(alg.is_connected());
    }

    #[test]
    fn test_disconnected_graph() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let alg = FragmentingAlgorithm::new(graph);
        assert_eq!(alg.num_fragments(), 2);
        assert!(!alg.is_connected());
    }

    #[test]
    fn test_dynamic_split() {
        let graph = Arc::new(DynamicGraph::new());
        let _e1 = graph.insert_edge(0, 1, 1.0).unwrap();
        let e2 = graph.insert_edge(1, 2, 1.0).unwrap();

        let mut alg = FragmentingAlgorithm::new(Arc::clone(&graph));
        assert!(alg.is_connected());

        // Delete middle edge to split
        graph.delete_edge(1, 2).unwrap();
        alg.delete_edge(e2, 1, 2);

        assert!(!alg.is_connected());
        assert_eq!(alg.num_fragments(), 2);
    }

    #[test]
    fn test_dynamic_merge() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let mut alg = FragmentingAlgorithm::new(Arc::clone(&graph));
        assert!(!alg.is_connected());

        // Add bridge edge
        let bridge = graph.insert_edge(1, 2, 1.0).unwrap();
        alg.insert_edge(bridge, 1, 2);

        assert!(alg.is_connected());
        assert_eq!(alg.num_fragments(), 1);
    }

    #[test]
    fn test_query_connected() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 0, 1.0).unwrap();

        let alg = FragmentingAlgorithm::new(graph);

        match alg.query() {
            FragmentResult::Connected { min_cut, .. } => {
                assert_eq!(min_cut, 2); // Cycle has min cut 2
            }
            _ => panic!("Expected connected result"),
        }
    }

    #[test]
    fn test_query_disconnected() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let alg = FragmentingAlgorithm::new(graph);

        match alg.query() {
            FragmentResult::Disconnected { global_min_cut, .. } => {
                assert_eq!(global_min_cut, 0);
            }
            _ => panic!("Expected disconnected result"),
        }
    }
}
