//! Graph storage for similarity relationships between embeddings.
//!
//! This module provides storage and querying for the similarity graph,
//! supporting edge types like SIMILAR, SEQUENTIAL, and SAME_CLUSTER.

use std::collections::{HashMap, HashSet, VecDeque};

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::domain::{
    EdgeType, EmbeddingId, GraphEdgeRepository, GraphTraversal, SimilarityEdge, VectorError,
};
use crate::domain::repository::RepoResult;

/// In-memory graph store for similarity edges.
///
/// This implementation uses adjacency lists for efficient edge traversal
/// and supports bidirectional lookups.
#[derive(Debug, Default)]
pub struct InMemoryGraphStore {
    /// Forward edges: from_id -> list of edges
    forward: RwLock<HashMap<EmbeddingId, Vec<SimilarityEdge>>>,

    /// Reverse edges: to_id -> list of edges
    reverse: RwLock<HashMap<EmbeddingId, Vec<SimilarityEdge>>>,

    /// Total edge count
    count: RwLock<usize>,
}

impl InMemoryGraphStore {
    /// Create a new empty graph store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of edges.
    pub fn len(&self) -> usize {
        *self.count.read()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get all unique node IDs in the graph.
    pub fn node_ids(&self) -> Vec<EmbeddingId> {
        let forward = self.forward.read();
        let reverse = self.reverse.read();

        let mut ids: HashSet<EmbeddingId> = forward.keys().copied().collect();
        ids.extend(reverse.keys().copied());

        ids.into_iter().collect()
    }

    /// Get the degree (number of connections) for a node.
    pub fn degree(&self, id: &EmbeddingId) -> usize {
        let forward = self.forward.read();
        let reverse = self.reverse.read();

        let out_degree = forward.get(id).map(|e| e.len()).unwrap_or(0);
        let in_degree = reverse.get(id).map(|e| e.len()).unwrap_or(0);

        out_degree + in_degree
    }

    /// Export the graph for serialization.
    pub fn export(&self) -> GraphExport {
        let forward = self.forward.read();
        let edges: Vec<_> = forward.values().flatten().cloned().collect();

        GraphExport { edges }
    }

    /// Import a graph from serialized data.
    pub fn import(&self, data: GraphExport) -> RepoResult<()> {
        let mut forward = self.forward.write();
        let mut reverse = self.reverse.write();
        let mut count = self.count.write();

        forward.clear();
        reverse.clear();
        *count = 0;

        for edge in data.edges {
            forward
                .entry(edge.from_id)
                .or_default()
                .push(edge.clone());

            reverse
                .entry(edge.to_id)
                .or_default()
                .push(edge);

            *count += 1;
        }

        Ok(())
    }
}

#[async_trait]
impl GraphEdgeRepository for InMemoryGraphStore {
    #[instrument(skip(self, edge))]
    async fn add_edge(&self, edge: SimilarityEdge) -> RepoResult<()> {
        let mut forward = self.forward.write();
        let mut reverse = self.reverse.write();
        let mut count = self.count.write();

        forward
            .entry(edge.from_id)
            .or_default()
            .push(edge.clone());

        reverse
            .entry(edge.to_id)
            .or_default()
            .push(edge);

        *count += 1;

        debug!("Added edge, total count: {}", *count);
        Ok(())
    }

    #[instrument(skip(self, edges), fields(count = edges.len()))]
    async fn add_edges(&self, edges: &[SimilarityEdge]) -> RepoResult<()> {
        let mut forward = self.forward.write();
        let mut reverse = self.reverse.write();
        let mut count = self.count.write();

        for edge in edges {
            forward
                .entry(edge.from_id)
                .or_default()
                .push(edge.clone());

            reverse
                .entry(edge.to_id)
                .or_default()
                .push(edge.clone());

            *count += 1;
        }

        debug!("Added {} edges, total count: {}", edges.len(), *count);
        Ok(())
    }

    async fn remove_edge(&self, from: &EmbeddingId, to: &EmbeddingId) -> RepoResult<()> {
        let mut forward = self.forward.write();
        let mut reverse = self.reverse.write();
        let mut count = self.count.write();

        let mut removed = false;

        if let Some(edges) = forward.get_mut(from) {
            let len_before = edges.len();
            edges.retain(|e| &e.to_id != to);
            if edges.len() < len_before {
                removed = true;
            }
            if edges.is_empty() {
                forward.remove(from);
            }
        }

        if let Some(edges) = reverse.get_mut(to) {
            edges.retain(|e| &e.from_id != from);
            if edges.is_empty() {
                reverse.remove(to);
            }
        }

        if removed {
            *count = count.saturating_sub(1);
        }

        Ok(())
    }

    async fn get_edges_from(&self, id: &EmbeddingId) -> RepoResult<Vec<SimilarityEdge>> {
        let forward = self.forward.read();
        Ok(forward.get(id).cloned().unwrap_or_default())
    }

    async fn get_edges_to(&self, id: &EmbeddingId) -> RepoResult<Vec<SimilarityEdge>> {
        let reverse = self.reverse.read();
        Ok(reverse.get(id).cloned().unwrap_or_default())
    }

    async fn get_edges_by_type(
        &self,
        id: &EmbeddingId,
        edge_type: EdgeType,
    ) -> RepoResult<Vec<SimilarityEdge>> {
        let forward = self.forward.read();
        Ok(forward
            .get(id)
            .map(|edges| {
                edges
                    .iter()
                    .filter(|e| e.edge_type == edge_type)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn get_strong_edges(
        &self,
        id: &EmbeddingId,
        min_similarity: f32,
    ) -> RepoResult<Vec<SimilarityEdge>> {
        let forward = self.forward.read();
        Ok(forward
            .get(id)
            .map(|edges| {
                edges
                    .iter()
                    .filter(|e| e.similarity() >= min_similarity)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn edge_count(&self) -> RepoResult<usize> {
        Ok(*self.count.read())
    }

    async fn clear(&self) -> RepoResult<()> {
        self.forward.write().clear();
        self.reverse.write().clear();
        *self.count.write() = 0;
        Ok(())
    }

    async fn remove_edges_for(&self, id: &EmbeddingId) -> RepoResult<()> {
        let mut forward = self.forward.write();
        let mut reverse = self.reverse.write();
        let mut count = self.count.write();

        // Remove outgoing edges
        if let Some(edges) = forward.remove(id) {
            *count = count.saturating_sub(edges.len());

            // Clean up reverse references
            for edge in edges {
                if let Some(rev_edges) = reverse.get_mut(&edge.to_id) {
                    rev_edges.retain(|e| &e.from_id != id);
                    if rev_edges.is_empty() {
                        reverse.remove(&edge.to_id);
                    }
                }
            }
        }

        // Remove incoming edges
        if let Some(edges) = reverse.remove(id) {
            *count = count.saturating_sub(edges.len());

            // Clean up forward references
            for edge in edges {
                if let Some(fwd_edges) = forward.get_mut(&edge.from_id) {
                    fwd_edges.retain(|e| &e.to_id != id);
                    if fwd_edges.is_empty() {
                        forward.remove(&edge.from_id);
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl GraphTraversal for InMemoryGraphStore {
    async fn shortest_path(
        &self,
        from: &EmbeddingId,
        to: &EmbeddingId,
        max_depth: usize,
    ) -> RepoResult<Option<Vec<EmbeddingId>>> {
        if from == to {
            return Ok(Some(vec![*from]));
        }

        let forward = self.forward.read();

        // BFS
        let mut visited: HashSet<EmbeddingId> = HashSet::new();
        let mut queue: VecDeque<(EmbeddingId, Vec<EmbeddingId>)> = VecDeque::new();

        visited.insert(*from);
        queue.push_back((*from, vec![*from]));

        while let Some((current, path)) = queue.pop_front() {
            if path.len() > max_depth {
                continue;
            }

            if let Some(edges) = forward.get(&current) {
                for edge in edges {
                    if &edge.to_id == to {
                        let mut result = path.clone();
                        result.push(edge.to_id);
                        return Ok(Some(result));
                    }

                    if !visited.contains(&edge.to_id) {
                        visited.insert(edge.to_id);
                        let mut new_path = path.clone();
                        new_path.push(edge.to_id);
                        queue.push_back((edge.to_id, new_path));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn neighbors_within_hops(
        &self,
        id: &EmbeddingId,
        hops: usize,
    ) -> RepoResult<Vec<(EmbeddingId, usize)>> {
        let forward = self.forward.read();

        let mut visited: HashMap<EmbeddingId, usize> = HashMap::new();
        let mut queue: VecDeque<(EmbeddingId, usize)> = VecDeque::new();

        visited.insert(*id, 0);
        queue.push_back((*id, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= hops {
                continue;
            }

            if let Some(edges) = forward.get(&current) {
                for edge in edges {
                    if !visited.contains_key(&edge.to_id) {
                        visited.insert(edge.to_id, depth + 1);
                        queue.push_back((edge.to_id, depth + 1));
                    }
                }
            }
        }

        // Remove the starting node
        visited.remove(id);

        Ok(visited.into_iter().collect())
    }

    async fn connected_components(&self) -> RepoResult<Vec<Vec<EmbeddingId>>> {
        let forward = self.forward.read();
        let reverse = self.reverse.read();

        // Get all nodes
        let mut all_nodes: HashSet<EmbeddingId> = forward.keys().copied().collect();
        all_nodes.extend(reverse.keys().copied());

        let mut visited: HashSet<EmbeddingId> = HashSet::new();
        let mut components: Vec<Vec<EmbeddingId>> = Vec::new();

        for &start in &all_nodes {
            if visited.contains(&start) {
                continue;
            }

            let mut component: Vec<EmbeddingId> = Vec::new();
            let mut stack: Vec<EmbeddingId> = vec![start];

            while let Some(current) = stack.pop() {
                if visited.contains(&current) {
                    continue;
                }

                visited.insert(current);
                component.push(current);

                // Add neighbors (both directions for undirected view)
                if let Some(edges) = forward.get(&current) {
                    for edge in edges {
                        if !visited.contains(&edge.to_id) {
                            stack.push(edge.to_id);
                        }
                    }
                }
                if let Some(edges) = reverse.get(&current) {
                    for edge in edges {
                        if !visited.contains(&edge.from_id) {
                            stack.push(edge.from_id);
                        }
                    }
                }
            }

            if !component.is_empty() {
                components.push(component);
            }
        }

        Ok(components)
    }

    async fn centrality_scores(&self) -> RepoResult<Vec<(EmbeddingId, f32)>> {
        // Simple degree centrality (normalized)
        let forward = self.forward.read();
        let reverse = self.reverse.read();

        let mut degrees: HashMap<EmbeddingId, usize> = HashMap::new();

        for (id, edges) in forward.iter() {
            *degrees.entry(*id).or_default() += edges.len();
        }
        for (id, edges) in reverse.iter() {
            *degrees.entry(*id).or_default() += edges.len();
        }

        let max_degree = degrees.values().copied().max().unwrap_or(1) as f32;

        let scores: Vec<_> = degrees
            .into_iter()
            .map(|(id, degree)| (id, degree as f32 / max_degree))
            .collect();

        Ok(scores)
    }
}

/// Serializable export format for the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExport {
    /// All edges in the graph.
    pub edges: Vec<SimilarityEdge>,
}

impl GraphExport {
    /// Create a new empty export.
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Save to a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), VectorError> {
        let file = std::fs::File::create(path)?;
        let writer = std::io::BufWriter::new(file);
        bincode::serialize_into(writer, self)?;
        Ok(())
    }

    /// Load from a file.
    pub fn load(path: &std::path::Path) -> Result<Self, VectorError> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let export = bincode::deserialize_from(reader)?;
        Ok(export)
    }
}

impl Default for GraphExport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_query_edges() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        let id3 = EmbeddingId::new();

        let edge1 = SimilarityEdge::new(id1, id2, 0.1);
        let edge2 = SimilarityEdge::new(id1, id3, 0.2);

        store.add_edge(edge1).await.unwrap();
        store.add_edge(edge2).await.unwrap();

        assert_eq!(store.edge_count().await.unwrap(), 2);

        let from_edges = store.get_edges_from(&id1).await.unwrap();
        assert_eq!(from_edges.len(), 2);

        let to_edges = store.get_edges_to(&id2).await.unwrap();
        assert_eq!(to_edges.len(), 1);
    }

    #[tokio::test]
    async fn test_remove_edge() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();

        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap();
        assert_eq!(store.edge_count().await.unwrap(), 1);

        store.remove_edge(&id1, &id2).await.unwrap();
        assert_eq!(store.edge_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_edges_by_type() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        let id3 = EmbeddingId::new();

        store
            .add_edge(SimilarityEdge::new(id1, id2, 0.1).with_type(EdgeType::Similar))
            .await
            .unwrap();
        store
            .add_edge(SimilarityEdge::sequential(id1, id3))
            .await
            .unwrap();

        let similar = store.get_edges_by_type(&id1, EdgeType::Similar).await.unwrap();
        assert_eq!(similar.len(), 1);

        let sequential = store.get_edges_by_type(&id1, EdgeType::Sequential).await.unwrap();
        assert_eq!(sequential.len(), 1);
    }

    #[tokio::test]
    async fn test_strong_edges() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        let id3 = EmbeddingId::new();

        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap(); // 0.9 similarity
        store.add_edge(SimilarityEdge::new(id1, id3, 0.5)).await.unwrap(); // 0.5 similarity

        let strong = store.get_strong_edges(&id1, 0.8).await.unwrap();
        assert_eq!(strong.len(), 1);
        assert_eq!(strong[0].to_id, id2);
    }

    #[tokio::test]
    async fn test_shortest_path() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        let id3 = EmbeddingId::new();

        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap();
        store.add_edge(SimilarityEdge::new(id2, id3, 0.1)).await.unwrap();

        let path = store.shortest_path(&id1, &id3, 10).await.unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], id1);
        assert_eq!(path[2], id3);
    }

    #[tokio::test]
    async fn test_neighbors_within_hops() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        let id3 = EmbeddingId::new();
        let id4 = EmbeddingId::new();

        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap();
        store.add_edge(SimilarityEdge::new(id2, id3, 0.1)).await.unwrap();
        store.add_edge(SimilarityEdge::new(id3, id4, 0.1)).await.unwrap();

        let neighbors = store.neighbors_within_hops(&id1, 2).await.unwrap();
        let neighbor_ids: HashSet<_> = neighbors.iter().map(|(id, _)| *id).collect();

        assert!(neighbor_ids.contains(&id2));
        assert!(neighbor_ids.contains(&id3));
        assert!(!neighbor_ids.contains(&id4)); // 3 hops away
    }

    #[tokio::test]
    async fn test_connected_components() {
        let store = InMemoryGraphStore::new();

        // Component 1
        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap();

        // Component 2
        let id3 = EmbeddingId::new();
        let id4 = EmbeddingId::new();
        store.add_edge(SimilarityEdge::new(id3, id4, 0.1)).await.unwrap();

        let components = store.connected_components().await.unwrap();
        assert_eq!(components.len(), 2);
    }

    #[tokio::test]
    async fn test_remove_edges_for() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        let id3 = EmbeddingId::new();

        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap();
        store.add_edge(SimilarityEdge::new(id1, id3, 0.1)).await.unwrap();
        store.add_edge(SimilarityEdge::new(id3, id1, 0.1)).await.unwrap();

        assert_eq!(store.edge_count().await.unwrap(), 3);

        store.remove_edges_for(&id1).await.unwrap();

        assert_eq!(store.edge_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_export_import() {
        let store = InMemoryGraphStore::new();

        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();

        store.add_edge(SimilarityEdge::new(id1, id2, 0.1)).await.unwrap();

        let export = store.export();
        assert_eq!(export.edges.len(), 1);

        let new_store = InMemoryGraphStore::new();
        new_store.import(export).unwrap();

        assert_eq!(new_store.edge_count().await.unwrap(), 1);
    }
}
