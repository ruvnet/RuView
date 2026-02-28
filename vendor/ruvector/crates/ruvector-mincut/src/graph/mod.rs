//! Graph representation for dynamic minimum cut
//!
//! Provides an adjacency-based graph structure optimized for:
//! - O(1) edge existence queries
//! - O(deg(v)) neighbor iteration
//! - Efficient edge insertion/deletion
//! - Support for weighted edges

use crate::error::{MinCutError, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Unique vertex identifier
pub type VertexId = u64;

/// Unique edge identifier
pub type EdgeId = u64;

/// Edge weight type
pub type Weight = f64;

/// An edge in the graph
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier for this edge
    pub id: EdgeId,
    /// Source vertex of the edge
    pub source: VertexId,
    /// Target vertex of the edge
    pub target: VertexId,
    /// Weight of the edge
    pub weight: Weight,
}

impl Edge {
    /// Create a new edge
    pub fn new(id: EdgeId, source: VertexId, target: VertexId, weight: Weight) -> Self {
        Self {
            id,
            source,
            target,
            weight,
        }
    }

    /// Get the canonical (ordered) endpoints
    pub fn canonical_endpoints(&self) -> (VertexId, VertexId) {
        if self.source <= self.target {
            (self.source, self.target)
        } else {
            (self.target, self.source)
        }
    }

    /// Get the other endpoint of the edge given one endpoint
    pub fn other(&self, v: VertexId) -> Option<VertexId> {
        if self.source == v {
            Some(self.target)
        } else if self.target == v {
            Some(self.source)
        } else {
            None
        }
    }
}

/// Statistics about the graph
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphStats {
    /// Number of vertices in the graph
    pub num_vertices: usize,
    /// Number of edges in the graph
    pub num_edges: usize,
    /// Sum of all edge weights
    pub total_weight: f64,
    /// Minimum vertex degree
    pub min_degree: usize,
    /// Maximum vertex degree
    pub max_degree: usize,
    /// Average vertex degree
    pub avg_degree: f64,
}

/// Dynamic graph structure for minimum cut algorithm
pub struct DynamicGraph {
    /// Adjacency list: vertex -> set of (neighbor, edge_id)
    adjacency: DashMap<VertexId, HashSet<(VertexId, EdgeId)>>,
    /// Edge storage: edge_id -> Edge
    edges: DashMap<EdgeId, Edge>,
    /// Edge lookup: (min(u,v), max(u,v)) -> edge_id
    edge_index: DashMap<(VertexId, VertexId), EdgeId>,
    /// Next edge ID
    next_edge_id: AtomicU64,
    /// Number of vertices
    num_vertices: AtomicUsize,
}

impl DynamicGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            adjacency: DashMap::new(),
            edges: DashMap::new(),
            edge_index: DashMap::new(),
            next_edge_id: AtomicU64::new(0),
            num_vertices: AtomicUsize::new(0),
        }
    }

    /// Create with capacity hint
    pub fn with_capacity(vertices: usize, edges: usize) -> Self {
        Self {
            adjacency: DashMap::with_capacity(vertices),
            edges: DashMap::with_capacity(edges),
            edge_index: DashMap::with_capacity(edges),
            next_edge_id: AtomicU64::new(0),
            num_vertices: AtomicUsize::new(0),
        }
    }

    /// Add a vertex (returns true if new)
    pub fn add_vertex(&self, v: VertexId) -> bool {
        if self.adjacency.contains_key(&v) {
            false
        } else {
            self.adjacency.insert(v, HashSet::new());
            self.num_vertices.fetch_add(1, Ordering::SeqCst);
            true
        }
    }

    /// Check if vertex exists
    pub fn has_vertex(&self, v: VertexId) -> bool {
        self.adjacency.contains_key(&v)
    }

    /// Get canonical edge key (min, max) for consistent lookup
    fn canonical_key(u: VertexId, v: VertexId) -> (VertexId, VertexId) {
        if u <= v {
            (u, v)
        } else {
            (v, u)
        }
    }

    /// Insert an edge (returns edge ID if successful)
    pub fn insert_edge(&self, u: VertexId, v: VertexId, weight: Weight) -> Result<EdgeId> {
        // Self-loops are not allowed
        if u == v {
            return Err(MinCutError::InvalidEdge(u, v));
        }

        // Ensure both vertices exist
        self.add_vertex(u);
        self.add_vertex(v);

        let key = Self::canonical_key(u, v);

        // Check if edge already exists
        if self.edge_index.contains_key(&key) {
            return Err(MinCutError::EdgeExists(u, v));
        }

        // Generate new edge ID
        let edge_id = self.next_edge_id.fetch_add(1, Ordering::SeqCst);

        // Create edge
        let edge = Edge::new(edge_id, u, v, weight);

        // Insert into edge storage
        self.edges.insert(edge_id, edge);

        // Insert into edge index
        self.edge_index.insert(key, edge_id);

        // Update adjacency lists
        self.adjacency.get_mut(&u).unwrap().insert((v, edge_id));
        self.adjacency.get_mut(&v).unwrap().insert((u, edge_id));

        Ok(edge_id)
    }

    /// Delete an edge (returns the removed edge)
    pub fn delete_edge(&self, u: VertexId, v: VertexId) -> Result<Edge> {
        let key = Self::canonical_key(u, v);

        // Get edge ID
        let edge_id = self
            .edge_index
            .remove(&key)
            .ok_or_else(|| MinCutError::EdgeNotFound(u, v))?
            .1;

        // Remove from edge storage
        let (_, edge) = self
            .edges
            .remove(&edge_id)
            .ok_or_else(|| MinCutError::EdgeNotFound(u, v))?;

        // Update adjacency lists
        if let Some(mut neighbors) = self.adjacency.get_mut(&u) {
            neighbors.retain(|(neighbor, eid)| !(*neighbor == v && *eid == edge_id));
        }
        if let Some(mut neighbors) = self.adjacency.get_mut(&v) {
            neighbors.retain(|(neighbor, eid)| !(*neighbor == u && *eid == edge_id));
        }

        Ok(edge)
    }

    /// Check if edge exists
    pub fn has_edge(&self, u: VertexId, v: VertexId) -> bool {
        let key = Self::canonical_key(u, v);
        self.edge_index.contains_key(&key)
    }

    /// Get edge by endpoints
    pub fn get_edge(&self, u: VertexId, v: VertexId) -> Option<Edge> {
        let key = Self::canonical_key(u, v);
        self.edge_index
            .get(&key)
            .and_then(|edge_id| self.edges.get(edge_id.value()).map(|e| *e.value()))
    }

    /// Get all neighbors of a vertex
    pub fn neighbors(&self, v: VertexId) -> Vec<(VertexId, EdgeId)> {
        self.adjacency
            .get(&v)
            .map(|neighbors| neighbors.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get degree of vertex
    pub fn degree(&self, v: VertexId) -> usize {
        self.adjacency
            .get(&v)
            .map(|neighbors| neighbors.len())
            .unwrap_or(0)
    }

    /// Get number of vertices
    pub fn num_vertices(&self) -> usize {
        self.num_vertices.load(Ordering::SeqCst)
    }

    /// Get number of edges
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Get all vertices
    pub fn vertices(&self) -> Vec<VertexId> {
        self.adjacency.iter().map(|entry| *entry.key()).collect()
    }

    /// Get all edges
    pub fn edges(&self) -> Vec<Edge> {
        self.edges.iter().map(|entry| *entry.value()).collect()
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        let num_vertices = self.num_vertices();
        let num_edges = self.num_edges();

        if num_vertices == 0 {
            return GraphStats::default();
        }

        let mut degrees: Vec<usize> = self
            .adjacency
            .iter()
            .map(|entry| entry.value().len())
            .collect();

        degrees.sort_unstable();

        let min_degree = degrees.first().copied().unwrap_or(0);
        let max_degree = degrees.last().copied().unwrap_or(0);
        let total_degree: usize = degrees.iter().sum();
        let avg_degree = total_degree as f64 / num_vertices as f64;

        let total_weight: f64 = self.edges.iter().map(|entry| entry.value().weight).sum();

        GraphStats {
            num_vertices,
            num_edges,
            total_weight,
            min_degree,
            max_degree,
            avg_degree,
        }
    }

    /// Check if graph is connected using BFS
    pub fn is_connected(&self) -> bool {
        let num_vertices = self.num_vertices();

        if num_vertices == 0 {
            return true; // Empty graph is considered connected
        }

        if num_vertices == 1 {
            return true;
        }

        // Get an arbitrary starting vertex
        let start = match self.adjacency.iter().next() {
            Some(entry) => *entry.key(),
            None => return true,
        };

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(v) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(&v) {
                for (neighbor, _) in neighbors.iter() {
                    if visited.insert(*neighbor) {
                        queue.push_back(*neighbor);
                    }
                }
            }
        }

        visited.len() == num_vertices
    }

    /// Get connected components using BFS
    pub fn connected_components(&self) -> Vec<Vec<VertexId>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        for entry in self.adjacency.iter() {
            let start = *entry.key();

            if visited.contains(&start) {
                continue;
            }

            let mut component = Vec::new();
            let mut queue = VecDeque::new();

            queue.push_back(start);
            visited.insert(start);

            while let Some(v) = queue.pop_front() {
                component.push(v);

                if let Some(neighbors) = self.adjacency.get(&v) {
                    for (neighbor, _) in neighbors.iter() {
                        if visited.insert(*neighbor) {
                            queue.push_back(*neighbor);
                        }
                    }
                }
            }

            components.push(component);
        }

        components
    }

    /// Clear all vertices and edges from the graph
    pub fn clear(&self) {
        self.adjacency.clear();
        self.edges.clear();
        self.edge_index.clear();
        self.next_edge_id.store(0, Ordering::SeqCst);
        self.num_vertices.store(0, Ordering::SeqCst);
    }

    /// Remove a vertex and all its incident edges
    pub fn remove_vertex(&self, v: VertexId) -> Result<()> {
        if !self.has_vertex(v) {
            return Err(MinCutError::InvalidVertex(v));
        }

        // Get all incident edges
        let incident_edges: Vec<(VertexId, EdgeId)> = self.neighbors(v);

        // Remove all incident edges
        for (neighbor, _) in incident_edges {
            // Use delete_edge to properly clean up
            let _ = self.delete_edge(v, neighbor);
        }

        // Remove the vertex from adjacency list
        self.adjacency.remove(&v);
        self.num_vertices.fetch_sub(1, Ordering::SeqCst);

        Ok(())
    }

    /// Get the weight of an edge
    pub fn edge_weight(&self, u: VertexId, v: VertexId) -> Option<Weight> {
        self.get_edge(u, v).map(|e| e.weight)
    }

    /// Update the weight of an existing edge
    pub fn update_edge_weight(&self, u: VertexId, v: VertexId, new_weight: Weight) -> Result<()> {
        let key = Self::canonical_key(u, v);

        let edge_id = self
            .edge_index
            .get(&key)
            .ok_or_else(|| MinCutError::EdgeNotFound(u, v))?;

        let edge_id = *edge_id.value();

        if let Some(mut edge_ref) = self.edges.get_mut(&edge_id) {
            edge_ref.weight = new_weight;
            Ok(())
        } else {
            Err(MinCutError::EdgeNotFound(u, v))
        }
    }
}

impl Default for DynamicGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DynamicGraph {
    fn clone(&self) -> Self {
        let new_graph = Self::with_capacity(self.num_vertices(), self.num_edges());

        // Clone vertices
        for entry in self.adjacency.iter() {
            new_graph.add_vertex(*entry.key());
        }

        // Clone edges
        for entry in self.edges.iter() {
            let edge = entry.value();
            let _ = new_graph.insert_edge(edge.source, edge.target, edge.weight);
        }

        new_graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let g = DynamicGraph::new();
        assert_eq!(g.num_vertices(), 0);
        assert_eq!(g.num_edges(), 0);
        assert!(g.is_connected());
    }

    #[test]
    fn test_add_vertex() {
        let g = DynamicGraph::new();
        assert!(g.add_vertex(1));
        assert!(!g.add_vertex(1)); // Adding again returns false
        assert_eq!(g.num_vertices(), 1);
        assert!(g.has_vertex(1));
        assert!(!g.has_vertex(2));
    }

    #[test]
    fn test_insert_edge() {
        let g = DynamicGraph::new();
        let edge_id = g.insert_edge(1, 2, 1.0).unwrap();
        assert_eq!(g.num_edges(), 1);
        assert_eq!(g.num_vertices(), 2);
        assert!(g.has_edge(1, 2));
        assert!(g.has_edge(2, 1)); // Undirected

        // Check edge ID is returned
        assert_eq!(edge_id, 0);
    }

    #[test]
    fn test_insert_duplicate_edge() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        let result = g.insert_edge(1, 2, 2.0);
        assert!(matches!(result, Err(MinCutError::EdgeExists(1, 2))));
    }

    #[test]
    fn test_insert_self_loop() {
        let g = DynamicGraph::new();
        let result = g.insert_edge(1, 1, 1.0);
        assert!(matches!(result, Err(MinCutError::InvalidEdge(1, 1))));
    }

    #[test]
    fn test_delete_edge() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.5).unwrap();
        assert_eq!(g.num_edges(), 1);

        let edge = g.delete_edge(1, 2).unwrap();
        assert_eq!(edge.weight, 1.5);
        assert_eq!(g.num_edges(), 0);
        assert!(!g.has_edge(1, 2));
    }

    #[test]
    fn test_delete_nonexistent_edge() {
        let g = DynamicGraph::new();
        let result = g.delete_edge(1, 2);
        assert!(matches!(result, Err(MinCutError::EdgeNotFound(1, 2))));
    }

    #[test]
    fn test_neighbors() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(1, 3, 1.0).unwrap();
        g.insert_edge(1, 4, 1.0).unwrap();

        let neighbors = g.neighbors(1);
        assert_eq!(neighbors.len(), 3);

        let neighbor_ids: HashSet<VertexId> = neighbors.iter().map(|(v, _)| *v).collect();
        assert!(neighbor_ids.contains(&2));
        assert!(neighbor_ids.contains(&3));
        assert!(neighbor_ids.contains(&4));
    }

    #[test]
    fn test_degree() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(1, 3, 1.0).unwrap();

        assert_eq!(g.degree(1), 2);
        assert_eq!(g.degree(2), 1);
        assert_eq!(g.degree(3), 1);
        assert_eq!(g.degree(4), 0); // Non-existent vertex
    }

    #[test]
    fn test_get_edge() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 2.5).unwrap();

        let edge = g.get_edge(1, 2).unwrap();
        assert_eq!(edge.weight, 2.5);
        assert_eq!(edge.source, 1);
        assert_eq!(edge.target, 2);

        // Symmetric
        let edge = g.get_edge(2, 1).unwrap();
        assert_eq!(edge.weight, 2.5);

        assert!(g.get_edge(1, 3).is_none());
    }

    #[test]
    fn test_stats() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 2.0).unwrap();
        g.insert_edge(3, 1, 3.0).unwrap();

        let stats = g.stats();
        assert_eq!(stats.num_vertices, 3);
        assert_eq!(stats.num_edges, 3);
        assert_eq!(stats.total_weight, 6.0);
        assert_eq!(stats.min_degree, 2);
        assert_eq!(stats.max_degree, 2);
        assert_eq!(stats.avg_degree, 2.0);
    }

    #[test]
    fn test_is_connected_single_component() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 1.0).unwrap();
        g.insert_edge(3, 4, 1.0).unwrap();

        assert!(g.is_connected());
    }

    #[test]
    fn test_is_connected_disconnected() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(3, 4, 1.0).unwrap();

        assert!(!g.is_connected());
    }

    #[test]
    fn test_connected_components() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 1.0).unwrap();
        g.insert_edge(4, 5, 1.0).unwrap();
        g.insert_edge(6, 7, 1.0).unwrap();
        g.insert_edge(7, 8, 1.0).unwrap();

        let components = g.connected_components();
        assert_eq!(components.len(), 3);

        // Each component should have the right size
        let mut sizes: Vec<usize> = components.iter().map(|c| c.len()).collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![2, 3, 3]);
    }

    #[test]
    fn test_clear() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 1.0).unwrap();

        assert_eq!(g.num_vertices(), 3);
        assert_eq!(g.num_edges(), 2);

        g.clear();

        assert_eq!(g.num_vertices(), 0);
        assert_eq!(g.num_edges(), 0);
    }

    #[test]
    fn test_remove_vertex() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 1.0).unwrap();
        g.insert_edge(1, 3, 1.0).unwrap();

        assert_eq!(g.num_vertices(), 3);
        assert_eq!(g.num_edges(), 3);

        g.remove_vertex(2).unwrap();

        assert_eq!(g.num_vertices(), 2);
        assert_eq!(g.num_edges(), 1);
        assert!(!g.has_vertex(2));
        assert!(g.has_edge(1, 3));
        assert!(!g.has_edge(1, 2));
        assert!(!g.has_edge(2, 3));
    }

    #[test]
    fn test_edge_weight() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 3.5).unwrap();

        assert_eq!(g.edge_weight(1, 2), Some(3.5));
        assert_eq!(g.edge_weight(2, 1), Some(3.5));
        assert_eq!(g.edge_weight(1, 3), None);
    }

    #[test]
    fn test_update_edge_weight() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();

        g.update_edge_weight(1, 2, 5.0).unwrap();
        assert_eq!(g.edge_weight(1, 2), Some(5.0));

        let result = g.update_edge_weight(1, 3, 2.0);
        assert!(matches!(result, Err(MinCutError::EdgeNotFound(1, 3))));
    }

    #[test]
    fn test_clone() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 2.0).unwrap();

        let g2 = g.clone();

        assert_eq!(g2.num_vertices(), 3);
        assert_eq!(g2.num_edges(), 2);
        assert!(g2.has_edge(1, 2));
        assert!(g2.has_edge(2, 3));
        assert_eq!(g2.edge_weight(1, 2), Some(1.0));
        assert_eq!(g2.edge_weight(2, 3), Some(2.0));
    }

    #[test]
    fn test_edge_canonical_endpoints() {
        let edge = Edge::new(0, 5, 3, 1.0);
        assert_eq!(edge.canonical_endpoints(), (3, 5));

        let edge = Edge::new(0, 2, 7, 1.0);
        assert_eq!(edge.canonical_endpoints(), (2, 7));
    }

    #[test]
    fn test_edge_other() {
        let edge = Edge::new(0, 1, 2, 1.0);
        assert_eq!(edge.other(1), Some(2));
        assert_eq!(edge.other(2), Some(1));
        assert_eq!(edge.other(3), None);
    }

    #[test]
    fn test_with_capacity() {
        let g = DynamicGraph::with_capacity(100, 200);
        assert_eq!(g.num_vertices(), 0);
        assert_eq!(g.num_edges(), 0);

        // Should work normally
        g.insert_edge(1, 2, 1.0).unwrap();
        assert_eq!(g.num_edges(), 1);
    }

    #[test]
    fn test_vertices_and_edges() {
        let g = DynamicGraph::new();
        g.insert_edge(1, 2, 1.0).unwrap();
        g.insert_edge(2, 3, 2.0).unwrap();

        let vertices = g.vertices();
        assert_eq!(vertices.len(), 3);
        assert!(vertices.contains(&1));
        assert!(vertices.contains(&2));
        assert!(vertices.contains(&3));

        let edges = g.edges();
        assert_eq!(edges.len(), 2);
    }
}
