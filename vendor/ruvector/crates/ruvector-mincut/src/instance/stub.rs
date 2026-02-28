//! Stub implementation of ProperCutInstance
//!
//! Brute-force reference implementation for testing.
//! Recomputes minimum cut on every query - O(2^n) worst case.
//! Only suitable for small graphs (n < 20).

use super::witness::WitnessHandle;
use super::{InstanceResult, ProperCutInstance};
use crate::graph::{DynamicGraph, EdgeId, VertexId};
use roaring::RoaringBitmap;
use std::collections::{HashMap, HashSet, VecDeque};

/// Stub instance that does brute-force min cut computation
///
/// This implementation:
/// - Stores a local copy of all edges
/// - Enumerates all proper subsets on each query
/// - Checks connectivity via BFS
/// - Computes exact boundary values
///
/// # Performance
///
/// - Query: O(2^n · m) where n = vertices, m = edges
/// - Only practical for n < 20
///
/// # Purpose
///
/// Used as a reference implementation to test the wrapper logic
/// before the real LocalKCut algorithm is ready.
pub struct StubInstance {
    /// Lambda bounds
    lambda_min: u64,
    lambda_max: u64,
    /// Local copy of edges for computation
    edges: Vec<(VertexId, VertexId, EdgeId)>,
    /// Vertex set
    vertices: HashSet<VertexId>,
    /// Adjacency list: vertex -> [(neighbor, edge_id), ...]
    adjacency: HashMap<VertexId, Vec<(VertexId, EdgeId)>>,
}

impl StubInstance {
    /// Create a new stub instance with initial graph state
    ///
    /// This is used for direct testing. The wrapper should use `init()` instead.
    pub fn new(graph: &DynamicGraph, lambda_min: u64, lambda_max: u64) -> Self {
        let mut instance = Self {
            lambda_min,
            lambda_max,
            edges: Vec::new(),
            vertices: HashSet::new(),
            adjacency: HashMap::new(),
        };

        // Copy initial graph state
        for edge in graph.edges() {
            instance.vertices.insert(edge.source);
            instance.vertices.insert(edge.target);
            instance.edges.push((edge.source, edge.target, edge.id));
        }

        instance.rebuild_adjacency();
        instance
    }

    /// Create an empty stub instance for use with the wrapper
    ///
    /// The wrapper will apply all edges via apply_inserts/apply_deletes.
    pub fn new_empty(lambda_min: u64, lambda_max: u64) -> Self {
        Self {
            lambda_min,
            lambda_max,
            edges: Vec::new(),
            vertices: HashSet::new(),
            adjacency: HashMap::new(),
        }
    }

    /// Compute minimum cut via brute-force enumeration
    ///
    /// For each non-empty proper subset S:
    /// 1. Check if S is connected
    /// 2. If connected, compute boundary size
    /// 3. Track minimum
    ///
    /// Returns None if graph is empty or disconnected.
    fn compute_min_cut(&self) -> Option<(u64, WitnessHandle)> {
        if self.vertices.is_empty() {
            return None;
        }

        let n = self.vertices.len();
        if n == 1 {
            // Single vertex: no proper cuts
            return None;
        }

        // Stub instance only works for small graphs to avoid overflow
        // For large graphs, we return a large value to signal AboveRange
        if n >= 20 {
            // Return a large value that will trigger AboveRange
            return None;
        }

        // Check if graph is connected
        if !self.is_connected() {
            // Disconnected graph has min cut 0
            let membership =
                RoaringBitmap::from_iter(self.vertices.iter().take(1).map(|&v| v as u32));
            let seed = *self.vertices.iter().next().unwrap();
            let witness = WitnessHandle::new(seed, membership, 0);
            return Some((0, witness));
        }

        let vertex_vec: Vec<VertexId> = self.vertices.iter().copied().collect();
        let mut min_cut = u64::MAX;
        let mut best_set = HashSet::new();

        // Enumerate all non-empty proper subsets (2^n - 2 subsets)
        // We use bitmasks from 1 to 2^n - 2
        let max_mask = 1u64 << n;

        for mask in 1..max_mask - 1 {
            // Build subset from bitmask
            let mut subset = HashSet::new();
            for (i, &vertex) in vertex_vec.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    subset.insert(vertex);
                }
            }

            // Check if subset is connected
            if !self.is_connected_set(&subset) {
                continue;
            }

            // Compute boundary
            let (boundary_value, _boundary_edges) = self.compute_boundary(&subset);

            if boundary_value < min_cut {
                min_cut = boundary_value;
                best_set = subset.clone();
            }
        }

        if min_cut == u64::MAX {
            // No proper connected cuts found (shouldn't happen for connected graphs)
            return None;
        }

        // Build witness using new API
        // Convert HashSet to RoaringBitmap (u32)
        let membership: RoaringBitmap = best_set.iter().map(|&v| v as u32).collect();

        // Use first vertex in set as seed
        let seed = *best_set.iter().next().unwrap();

        let witness = WitnessHandle::new(seed, membership, min_cut);

        Some((min_cut, witness))
    }

    /// Check if a subset of vertices is connected
    ///
    /// Uses BFS within the subset to check connectivity.
    fn is_connected_set(&self, vertices: &HashSet<VertexId>) -> bool {
        if vertices.is_empty() {
            return true;
        }

        // Start BFS from arbitrary vertex in the set
        let start = *vertices.iter().next().unwrap();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(&current) {
                for &(neighbor, _edge_id) in neighbors {
                    // Only follow edges within the subset
                    if vertices.contains(&neighbor) && visited.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // Connected if we visited all vertices in the subset
        visited.len() == vertices.len()
    }

    /// Compute boundary of a vertex set
    ///
    /// Returns (boundary_value, boundary_edges).
    /// Boundary = edges with exactly one endpoint in the set.
    fn compute_boundary(&self, set: &HashSet<VertexId>) -> (u64, Vec<EdgeId>) {
        let mut boundary_value = 0u64;
        let mut boundary_edges = Vec::new();

        for &(u, v, edge_id) in &self.edges {
            let u_in_set = set.contains(&u);
            let v_in_set = set.contains(&v);

            // Edge is in boundary if exactly one endpoint is in set
            if u_in_set != v_in_set {
                boundary_value += 1;
                boundary_edges.push(edge_id);
            }
        }

        (boundary_value, boundary_edges)
    }

    /// Check if entire graph is connected
    fn is_connected(&self) -> bool {
        self.is_connected_set(&self.vertices)
    }

    /// Rebuild adjacency list from edges
    fn rebuild_adjacency(&mut self) {
        self.adjacency.clear();

        for &(u, v, edge_id) in &self.edges {
            self.adjacency
                .entry(u)
                .or_insert_with(Vec::new)
                .push((v, edge_id));

            self.adjacency
                .entry(v)
                .or_insert_with(Vec::new)
                .push((u, edge_id));
        }
    }

    fn insert(&mut self, edge_id: EdgeId, u: VertexId, v: VertexId) {
        // Add edge to local copy
        self.vertices.insert(u);
        self.vertices.insert(v);
        self.edges.push((u, v, edge_id));
        self.rebuild_adjacency();
    }

    fn delete(&mut self, edge_id: EdgeId, _u: VertexId, _v: VertexId) {
        // Remove edge from local copy
        self.edges.retain(|(_, _, eid)| *eid != edge_id);
        self.rebuild_adjacency();
    }
}

impl ProperCutInstance for StubInstance {
    fn init(_graph: &DynamicGraph, lambda_min: u64, lambda_max: u64) -> Self {
        // For wrapper use: start empty, wrapper will call apply_inserts
        Self::new_empty(lambda_min, lambda_max)
    }

    fn apply_inserts(&mut self, edges: &[(EdgeId, VertexId, VertexId)]) {
        for &(edge_id, u, v) in edges {
            self.insert(edge_id, u, v);
        }
    }

    fn apply_deletes(&mut self, edges: &[(EdgeId, VertexId, VertexId)]) {
        for &(edge_id, u, v) in edges {
            self.delete(edge_id, u, v);
        }
    }

    fn query(&mut self) -> InstanceResult {
        match self.compute_min_cut() {
            Some((value, witness)) if value <= self.lambda_max => {
                InstanceResult::ValueInRange { value, witness }
            }
            _ => InstanceResult::AboveRange,
        }
    }

    fn bounds(&self) -> (u64, u64) {
        (self.lambda_min, self.lambda_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::DynamicGraph;

    #[test]
    fn test_empty_graph() {
        let graph = DynamicGraph::new();
        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        assert!(matches!(result, InstanceResult::AboveRange));
    }

    #[test]
    fn test_single_vertex() {
        let graph = DynamicGraph::new();
        graph.add_vertex(1);

        let mut instance = StubInstance::new(&graph, 0, 10);
        let result = instance.query();
        assert!(matches!(result, InstanceResult::AboveRange));
    }

    #[test]
    fn test_path_graph() {
        // Path: 1 - 2 - 3
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => {
                // Min cut of path is 1
                assert_eq!(value, 1);
            }
            _ => panic!("Expected ValueInRange result"),
        }
    }

    #[test]
    fn test_cycle_graph() {
        // Cycle: 1 - 2 - 3 - 1
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();
        graph.insert_edge(3, 1, 1.0).unwrap();

        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => {
                // Min cut of cycle is 2
                assert_eq!(value, 2);
            }
            _ => panic!("Expected ValueInRange result"),
        }
    }

    #[test]
    fn test_complete_graph_k4() {
        // Complete graph K4
        let graph = DynamicGraph::new();
        for i in 1..=4 {
            for j in i + 1..=4 {
                graph.insert_edge(i, j, 1.0).unwrap();
            }
        }

        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => {
                // Min cut of K4 is 3 (minimum degree)
                assert_eq!(value, 3);
            }
            _ => panic!("Expected ValueInRange result"),
        }
    }

    #[test]
    fn test_disconnected_graph() {
        // Two separate edges: 1-2 and 3-4
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(3, 4, 1.0).unwrap();

        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => {
                // Disconnected graph has min cut 0
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ValueInRange with value 0"),
        }
    }

    #[test]
    fn test_bridge_graph() {
        // Two triangles connected by a bridge
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();
        graph.insert_edge(3, 1, 1.0).unwrap();
        graph.insert_edge(3, 4, 1.0).unwrap(); // Bridge
        graph.insert_edge(4, 5, 1.0).unwrap();
        graph.insert_edge(5, 6, 1.0).unwrap();
        graph.insert_edge(6, 4, 1.0).unwrap();

        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => {
                // Min cut is the bridge (value = 1)
                assert_eq!(value, 1);
            }
            _ => panic!("Expected ValueInRange result"),
        }
    }

    #[test]
    fn test_is_connected_set() {
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();
        graph.insert_edge(3, 4, 1.0).unwrap();

        let instance = StubInstance::new(&graph, 0, 10);

        // Test connected subset
        let mut subset = HashSet::new();
        subset.insert(1);
        subset.insert(2);
        subset.insert(3);
        assert!(instance.is_connected_set(&subset));

        // Test disconnected subset (1 and 4 not directly connected)
        let mut subset = HashSet::new();
        subset.insert(1);
        subset.insert(4);
        assert!(!instance.is_connected_set(&subset));

        // Test single vertex (always connected)
        let mut subset = HashSet::new();
        subset.insert(1);
        assert!(instance.is_connected_set(&subset));
    }

    #[test]
    fn test_compute_boundary() {
        let graph = DynamicGraph::new();
        let e1 = graph.insert_edge(1, 2, 1.0).unwrap();
        let e2 = graph.insert_edge(2, 3, 1.0).unwrap();
        let _e3 = graph.insert_edge(3, 4, 1.0).unwrap();

        let instance = StubInstance::new(&graph, 0, 10);

        // Boundary of {1, 2}
        let mut set = HashSet::new();
        set.insert(1);
        set.insert(2);
        let (value, edges) = instance.compute_boundary(&set);
        assert_eq!(value, 1); // Only edge 2-3 crosses
        assert_eq!(edges.len(), 1);
        assert!(edges.contains(&e2));

        // Boundary of {2}
        let mut set = HashSet::new();
        set.insert(2);
        let (value, edges) = instance.compute_boundary(&set);
        assert_eq!(value, 2); // Edges 1-2 and 2-3 cross
        assert_eq!(edges.len(), 2);
        assert!(edges.contains(&e1));
        assert!(edges.contains(&e2));
    }

    #[test]
    fn test_dynamic_updates() {
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let mut instance = StubInstance::new(&graph, 0, 10);

        // Initial min cut (path: 1-2-3) is 1
        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => assert_eq!(value, 1),
            _ => panic!("Expected ValueInRange"),
        }

        // Insert edge to form cycle
        let e3_id = 100; // Mock edge ID
        instance.apply_inserts(&[(e3_id, 3, 1)]);

        // Now min cut (cycle: 1-2-3-1) is 2
        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => assert_eq!(value, 2),
            _ => panic!("Expected ValueInRange"),
        }

        // Delete one edge to get back to path
        instance.apply_deletes(&[(e3_id, 3, 1)]);

        // Min cut should be 1 again
        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => assert_eq!(value, 1),
            _ => panic!("Expected ValueInRange"),
        }
    }

    #[test]
    fn test_range_bounds() {
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        // Instance with range [2, 5]
        let mut instance = StubInstance::new(&graph, 2, 5);

        // Min cut is 1, which is below range [2,5], but stub only checks <= lambda_max
        // so it returns ValueInRange
        let result = instance.query();
        // Stub doesn't check lambda_min, so behavior depends on implementation

        // Instance with range [0, 1]
        let mut instance = StubInstance::new(&graph, 0, 1);

        // Min cut is 1, which is in range
        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, .. } => assert_eq!(value, 1),
            _ => panic!("Expected ValueInRange"),
        }

        // Instance with range [0, 0]
        let mut instance = StubInstance::new(&graph, 0, 0);

        // Min cut is 1, which is above range
        let result = instance.query();
        assert!(matches!(result, InstanceResult::AboveRange));
    }

    #[test]
    fn test_witness_information() {
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap();

        let mut instance = StubInstance::new(&graph, 0, 10);

        let result = instance.query();
        match result {
            InstanceResult::ValueInRange { value, witness } => {
                assert_eq!(value, 1);
                assert_eq!(witness.boundary_size(), 1);
                assert!(witness.cardinality() > 0);
                assert!(witness.cardinality() < 3); // Proper cut
            }
            _ => panic!("Expected ValueInRange with witness"),
        }
    }
}
