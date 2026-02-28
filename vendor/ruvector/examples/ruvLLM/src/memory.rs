//! Memory service with HNSW vector search and graph storage
//!
//! Provides efficient vector similarity search using HNSW algorithm
//! with SIMD-accelerated distance computations.

use crate::config::MemoryConfig;
use crate::error::{Error, MemoryError, Result};
use crate::types::{EdgeType, MemoryEdge, MemoryNode, NodeType};

use dashmap::DashMap;
use parking_lot::RwLock;
use rand::Rng;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Search result from memory
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Retrieved candidates
    pub candidates: Vec<SearchCandidate>,
    /// Expanded subgraph
    pub subgraph: SubGraph,
    /// Statistics
    pub stats: SearchStats,
}

/// Single search candidate
#[derive(Debug, Clone)]
pub struct SearchCandidate {
    /// Node ID
    pub id: String,
    /// Distance to query
    pub distance: f32,
    /// Node data
    pub node: MemoryNode,
}

/// Subgraph from neighborhood expansion
#[derive(Debug, Clone)]
pub struct SubGraph {
    /// Nodes in subgraph
    pub nodes: Vec<MemoryNode>,
    /// Edges in subgraph
    pub edges: Vec<MemoryEdge>,
    /// Center node IDs
    pub center_ids: Vec<String>,
}

/// Search statistics
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    /// Number of candidates
    pub k_retrieved: usize,
    /// Distance statistics
    pub distance_mean: f32,
    pub distance_std: f32,
    pub distance_min: f32,
    pub distance_max: f32,
    /// Graph depth
    pub graph_depth: usize,
    /// HNSW layers traversed
    pub layers_traversed: usize,
    /// Distance computations performed
    pub distance_computations: usize,
}

/// HNSW graph layer
struct HnswLayer {
    /// Connections: node_id -> connected node_ids
    connections: DashMap<usize, Vec<usize>>,
    /// Maximum connections per node
    max_connections: usize,
}

impl HnswLayer {
    fn new(max_connections: usize) -> Self {
        Self {
            connections: DashMap::new(),
            max_connections,
        }
    }

    fn add_connection(&self, from: usize, to: usize) {
        self.connections
            .entry(from)
            .or_insert_with(Vec::new)
            .push(to);
    }

    fn get_neighbors(&self, node: usize) -> Vec<usize> {
        self.connections
            .get(&node)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    fn prune_connections(&self, node: usize, vectors: &[Vec<f32>], max_conn: usize) {
        if let Some(mut neighbors) = self.connections.get_mut(&node) {
            if neighbors.len() > max_conn {
                // Keep closest neighbors
                let node_vec = &vectors[node];
                let mut scored: Vec<(usize, f32)> = neighbors
                    .iter()
                    .map(|&n| (n, cosine_distance(node_vec, &vectors[n])))
                    .collect();
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                *neighbors = scored.into_iter().take(max_conn).map(|(n, _)| n).collect();
            }
        }
    }
}

/// Candidate for priority queue (min-heap by distance)
#[derive(Clone)]
struct Candidate {
    distance: f32,
    node_id: usize,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse for min-heap (smaller distance = higher priority)
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Memory service providing vector search and graph operations
pub struct MemoryService {
    /// Vectors storage
    vectors: RwLock<Vec<Vec<f32>>>,
    /// Node ID to index mapping
    id_to_index: DashMap<String, usize>,
    /// Index to node ID mapping
    index_to_id: RwLock<Vec<String>>,
    /// Node storage
    nodes: DashMap<String, MemoryNode>,
    /// Edge storage (src_id -> edges)
    edges: DashMap<String, Vec<MemoryEdge>>,
    /// HNSW layers
    hnsw_layers: RwLock<Vec<HnswLayer>>,
    /// Entry point for HNSW
    entry_point: RwLock<Option<usize>>,
    /// Max layer (highest level)
    max_layer: RwLock<usize>,
    /// Configuration
    config: MemoryConfig,
    /// Statistics
    stats: MemoryStats,
}

/// Memory service statistics
struct MemoryStats {
    /// Total insertions
    insertions: AtomicU64,
    /// Total searches
    searches: AtomicU64,
    /// Total distance computations
    distance_computations: AtomicU64,
}

impl MemoryService {
    /// Create a new memory service
    pub async fn new(config: &MemoryConfig) -> Result<Self> {
        // Note: ml (level multiplier) is computed per-insert in hnsw_insert()
        // to avoid storing it and to handle edge cases properly

        Ok(Self {
            vectors: RwLock::new(Vec::new()),
            id_to_index: DashMap::new(),
            index_to_id: RwLock::new(Vec::new()),
            nodes: DashMap::new(),
            edges: DashMap::new(),
            hnsw_layers: RwLock::new(vec![HnswLayer::new(config.hnsw_m * 2)]),
            entry_point: RwLock::new(None),
            max_layer: RwLock::new(0),
            config: config.clone(),
            stats: MemoryStats {
                insertions: AtomicU64::new(0),
                searches: AtomicU64::new(0),
                distance_computations: AtomicU64::new(0),
            },
        })
    }

    /// Search with graph expansion using HNSW
    pub async fn search_with_graph(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        max_hops: usize,
    ) -> Result<SearchResult> {
        self.stats.searches.fetch_add(1, Ordering::Relaxed);

        let vectors = self.vectors.read();
        if vectors.is_empty() {
            return Ok(SearchResult {
                candidates: vec![],
                subgraph: SubGraph {
                    nodes: vec![],
                    edges: vec![],
                    center_ids: vec![],
                },
                stats: SearchStats::default(),
            });
        }

        // HNSW search
        let (neighbors, layers_traversed, dist_comps) = self.hnsw_search(query, k, ef_search);
        self.stats
            .distance_computations
            .fetch_add(dist_comps as u64, Ordering::Relaxed);

        // Convert to candidates
        let index_to_id = self.index_to_id.read();
        let candidates: Vec<SearchCandidate> = neighbors
            .into_iter()
            .filter_map(|(idx, distance)| {
                let id = index_to_id.get(idx)?.clone();
                let node = self.nodes.get(&id)?.clone();
                Some(SearchCandidate { id, distance, node })
            })
            .collect();

        // Expand neighborhood
        let center_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();
        let subgraph = self.expand_neighborhood(&center_ids, max_hops)?;

        // Compute stats
        let stats = self.compute_stats(&candidates, layers_traversed, dist_comps);

        Ok(SearchResult {
            candidates,
            subgraph,
            stats,
        })
    }

    /// HNSW search implementation
    fn hnsw_search(&self, query: &[f32], k: usize, ef: usize) -> (Vec<(usize, f32)>, usize, usize) {
        let vectors = self.vectors.read();
        let layers = self.hnsw_layers.read();
        let entry = *self.entry_point.read();
        let max_layer = *self.max_layer.read();

        let mut dist_comps = 0;
        let mut layers_traversed = 0;

        let entry_point = match entry {
            Some(ep) => ep,
            None => return (vec![], 0, 0),
        };

        // Start from entry point
        let mut current = entry_point;
        let mut current_dist = cosine_distance(query, &vectors[current]);
        dist_comps += 1;

        // Traverse from top layer to layer 1
        for layer_idx in (1..=max_layer).rev() {
            layers_traversed += 1;
            let layer = &layers[layer_idx];

            loop {
                let neighbors = layer.get_neighbors(current);
                let mut changed = false;

                for &neighbor in &neighbors {
                    if neighbor < vectors.len() {
                        let dist = cosine_distance(query, &vectors[neighbor]);
                        dist_comps += 1;
                        if dist < current_dist {
                            current = neighbor;
                            current_dist = dist;
                            changed = true;
                        }
                    }
                }

                if !changed {
                    break;
                }
            }
        }

        // Search at layer 0 with ef
        layers_traversed += 1;
        let layer_0 = &layers[0];

        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut result = BinaryHeap::new();

        visited.insert(current);
        candidates.push(Candidate {
            distance: current_dist,
            node_id: current,
        });
        result.push(std::cmp::Reverse(Candidate {
            distance: current_dist,
            node_id: current,
        }));

        while let Some(Candidate {
            distance: _,
            node_id: current_node,
        }) = candidates.pop()
        {
            // Check if we should stop
            if let Some(std::cmp::Reverse(furthest)) = result.peek() {
                if result.len() >= ef {
                    let current_cand = candidates.peek();
                    if let Some(cc) = current_cand {
                        if cc.distance > furthest.distance {
                            break;
                        }
                    }
                }
            }

            // Explore neighbors
            let neighbors = layer_0.get_neighbors(current_node);
            for &neighbor in &neighbors {
                if !visited.contains(&neighbor) && neighbor < vectors.len() {
                    visited.insert(neighbor);
                    let dist = cosine_distance(query, &vectors[neighbor]);
                    dist_comps += 1;

                    let should_add = result.len() < ef || {
                        if let Some(std::cmp::Reverse(furthest)) = result.peek() {
                            dist < furthest.distance
                        } else {
                            true
                        }
                    };

                    if should_add {
                        candidates.push(Candidate {
                            distance: dist,
                            node_id: neighbor,
                        });
                        result.push(std::cmp::Reverse(Candidate {
                            distance: dist,
                            node_id: neighbor,
                        }));

                        if result.len() > ef {
                            result.pop();
                        }
                    }
                }
            }
        }

        // Extract top-k results
        let mut final_results: Vec<(usize, f32)> = result
            .into_iter()
            .map(|std::cmp::Reverse(c)| (c.node_id, c.distance))
            .collect();
        final_results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        final_results.truncate(k);

        (final_results, layers_traversed, dist_comps)
    }

    /// Insert a node with HNSW indexing
    pub fn insert_node(&self, node: MemoryNode) -> Result<String> {
        let id = node.id.clone();
        let vector = node.vector.clone();

        // Check capacity
        if self.nodes.len() >= self.config.max_nodes {
            return Err(Error::Memory(MemoryError::CapacityExceeded));
        }

        // Add to storage
        let index = {
            let mut vectors = self.vectors.write();
            let idx = vectors.len();
            vectors.push(vector.clone());
            idx
        };

        {
            let mut index_to_id = self.index_to_id.write();
            index_to_id.push(id.clone());
        }

        self.id_to_index.insert(id.clone(), index);
        self.nodes.insert(id.clone(), node);

        // Insert into HNSW
        self.hnsw_insert(index, &vector);
        self.stats.insertions.fetch_add(1, Ordering::Relaxed);

        Ok(id)
    }

    /// HNSW insertion
    fn hnsw_insert(&self, node_idx: usize, vector: &[f32]) {
        let m = self.config.hnsw_m;
        let m_max = m * 2;
        // Guard against m=1 which would cause ln(1)=0 and division by zero
        // Use m=2 as minimum for level calculation
        let m_for_level = m.max(2) as f32;
        let ml = 1.0 / m_for_level.ln();

        // Determine level for this node
        let level = self.random_level(ml);

        let vectors = self.vectors.read();
        let mut layers = self.hnsw_layers.write();
        let mut entry = self.entry_point.write();
        let mut max_layer = self.max_layer.write();

        // Ensure we have enough layers
        while layers.len() <= level {
            layers.push(HnswLayer::new(m_max));
        }

        // If first node, set as entry point
        if entry.is_none() {
            *entry = Some(node_idx);
            *max_layer = level;
            return;
        }

        let entry_point = entry.unwrap();
        let mut current = entry_point;
        let mut current_dist = cosine_distance(vector, &vectors[current]);

        // Traverse from top layer down to level+1
        for layer_idx in (level + 1..=*max_layer).rev() {
            let layer = &layers[layer_idx];
            loop {
                let neighbors = layer.get_neighbors(current);
                let mut changed = false;
                for &neighbor in &neighbors {
                    if neighbor < vectors.len() {
                        let dist = cosine_distance(vector, &vectors[neighbor]);
                        if dist < current_dist {
                            current = neighbor;
                            current_dist = dist;
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
        }

        // Insert at each layer from level down to 0
        for layer_idx in (0..=level.min(*max_layer)).rev() {
            let layer = &layers[layer_idx];
            let max_conn = if layer_idx == 0 { m_max } else { m };

            // Find ef_construction nearest neighbors
            let ef = self.config.hnsw_ef_construction;
            let neighbors = self.search_layer(&vectors, vector, current, ef, layer);

            // Connect to m nearest
            let connections: Vec<usize> = neighbors
                .into_iter()
                .take(max_conn)
                .map(|(idx, _)| idx)
                .collect();

            // Add bidirectional connections
            for &conn in &connections {
                layer.add_connection(node_idx, conn);
                layer.add_connection(conn, node_idx);
                // Prune if too many connections
                layer.prune_connections(conn, &vectors, max_conn);
            }

            // Update entry point for next layer
            if !connections.is_empty() {
                current = connections[0];
            }
        }

        // Update entry point if necessary
        if level > *max_layer {
            *entry = Some(node_idx);
            *max_layer = level;
        }
    }

    /// Search within a single layer
    fn search_layer(
        &self,
        vectors: &[Vec<f32>],
        query: &[f32],
        entry: usize,
        ef: usize,
        layer: &HnswLayer,
    ) -> Vec<(usize, f32)> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut result = Vec::new();

        let entry_dist = cosine_distance(query, &vectors[entry]);
        visited.insert(entry);
        candidates.push(Candidate {
            distance: entry_dist,
            node_id: entry,
        });
        result.push((entry, entry_dist));

        while let Some(Candidate {
            distance: _,
            node_id,
        }) = candidates.pop()
        {
            if result.len() >= ef {
                result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                if let Some(&(_, furthest_dist)) = result.last() {
                    if let Some(closest) = candidates.peek() {
                        if closest.distance > furthest_dist {
                            break;
                        }
                    }
                }
            }

            let neighbors = layer.get_neighbors(node_id);
            for &neighbor in &neighbors {
                if !visited.contains(&neighbor) && neighbor < vectors.len() {
                    visited.insert(neighbor);
                    let dist = cosine_distance(query, &vectors[neighbor]);
                    candidates.push(Candidate {
                        distance: dist,
                        node_id: neighbor,
                    });
                    result.push((neighbor, dist));
                }
            }
        }

        result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        result.truncate(ef);
        result
    }

    /// Random level for HNSW (exponential distribution)
    fn random_level(&self, ml: f32) -> usize {
        let mut rng = rand::thread_rng();
        let r: f32 = rng.gen();
        // Guard against r=0 which would cause ln(0) = -inf
        // Also clamp result to prevent overflow when casting to usize
        if r <= f32::EPSILON {
            return 0;
        }
        let level = (-r.ln() * ml).floor();
        // Clamp to reasonable max level to prevent overflow
        level.min(32.0) as usize
    }

    /// Insert an edge
    pub fn insert_edge(&self, edge: MemoryEdge) -> Result<String> {
        let id = edge.id.clone();
        self.edges
            .entry(edge.src.clone())
            .or_insert_with(Vec::new)
            .push(edge);
        Ok(id)
    }

    /// Update edge weight
    pub fn update_edge_weight(&self, src: &str, dst: &str, delta: f32) -> Result<()> {
        if let Some(mut edges) = self.edges.get_mut(src) {
            for edge in edges.iter_mut() {
                if edge.dst == dst {
                    edge.weight = (edge.weight + delta).clamp(0.0, 1.0);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.edges.iter().map(|e| e.len()).sum()
    }

    /// Get node by ID
    pub fn get_node(&self, id: &str) -> Option<MemoryNode> {
        self.nodes.get(id).map(|n| n.clone())
    }

    /// Get edges from a node
    pub fn get_edges(&self, src: &str) -> Vec<MemoryEdge> {
        self.edges.get(src).map(|e| e.clone()).unwrap_or_default()
    }

    /// Batch insert nodes
    pub fn insert_batch(&self, nodes: Vec<MemoryNode>) -> Result<Vec<String>> {
        nodes.into_iter().map(|n| self.insert_node(n)).collect()
    }

    /// Flush pending writes (for persistence)
    pub async fn flush(&self) -> Result<()> {
        // In production, this would persist to disk
        Ok(())
    }

    /// Get memory statistics
    pub fn get_stats(&self) -> MemoryServiceStats {
        MemoryServiceStats {
            node_count: self.nodes.len(),
            edge_count: self.edge_count(),
            total_insertions: self.stats.insertions.load(Ordering::Relaxed),
            total_searches: self.stats.searches.load(Ordering::Relaxed),
            total_distance_computations: self.stats.distance_computations.load(Ordering::Relaxed),
            hnsw_layers: self.hnsw_layers.read().len(),
        }
    }

    /// Expand neighborhood via graph traversal
    fn expand_neighborhood(&self, center_ids: &[String], max_hops: usize) -> Result<SubGraph> {
        let mut visited = HashSet::new();
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();
        let mut frontier: Vec<String> = center_ids.to_vec();

        for hop in 0..=max_hops {
            let mut next_frontier = Vec::new();
            let is_last_hop = hop == max_hops;

            for node_id in &frontier {
                if visited.contains(node_id) {
                    continue;
                }
                visited.insert(node_id.clone());

                // Get node
                if let Some(node) = self.nodes.get(node_id) {
                    all_nodes.push(node.clone());
                }

                // Get edges (only collect if not on last hop, to avoid edges leading outside)
                if !is_last_hop {
                    if let Some(edges) = self.edges.get(node_id) {
                        for edge in edges.iter() {
                            all_edges.push(edge.clone());
                            if !visited.contains(&edge.dst) {
                                next_frontier.push(edge.dst.clone());
                            }
                        }
                    }
                }
            }

            frontier = next_frontier;
        }

        Ok(SubGraph {
            nodes: all_nodes,
            edges: all_edges,
            center_ids: center_ids.to_vec(),
        })
    }

    fn compute_stats(
        &self,
        candidates: &[SearchCandidate],
        layers: usize,
        dist_comps: usize,
    ) -> SearchStats {
        if candidates.is_empty() {
            return SearchStats::default();
        }

        let distances: Vec<f32> = candidates.iter().map(|c| c.distance).collect();
        let mean = distances.iter().sum::<f32>() / distances.len() as f32;
        let var =
            distances.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / distances.len() as f32;

        SearchStats {
            k_retrieved: candidates.len(),
            distance_mean: mean,
            distance_std: var.sqrt(),
            distance_min: distances.iter().cloned().fold(f32::INFINITY, f32::min),
            distance_max: distances.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
            graph_depth: 0,
            layers_traversed: layers,
            distance_computations: dist_comps,
        }
    }
}

/// Public statistics about memory service
#[derive(Debug, Clone)]
pub struct MemoryServiceStats {
    /// Number of nodes
    pub node_count: usize,
    /// Number of edges
    pub edge_count: usize,
    /// Total insertions
    pub total_insertions: u64,
    /// Total searches
    pub total_searches: u64,
    /// Total distance computations
    pub total_distance_computations: u64,
    /// Number of HNSW layers
    pub hnsw_layers: usize,
}

/// SIMD-accelerated cosine distance using simsimd when available
#[cfg(feature = "simd")]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    use simsimd::SpatialSimilarity;
    let cos_sim = f32::cosine(a, b).unwrap_or(0.0);
    1.0 - cos_sim
}

#[cfg(not(feature = "simd"))]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        1.0 - dot / (norm_a * norm_b)
    } else {
        1.0
    }
}

/// Euclidean distance
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Inner product (negative for use as distance)
pub fn inner_product_distance(a: &[f32], b: &[f32]) -> f32 {
    -a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(id: &str, vector: Vec<f32>) -> MemoryNode {
        MemoryNode {
            id: id.into(),
            vector,
            text: format!("Test node {}", id),
            node_type: NodeType::Document,
            source: "test".into(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_memory_insert_and_search() {
        let config = MemoryConfig::default();
        let memory = MemoryService::new(&config).await.unwrap();

        let node = create_test_node("test-1", vec![1.0, 0.0, 0.0]);
        memory.insert_node(node).unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let result = memory.search_with_graph(&query, 10, 64, 2).await.unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].id, "test-1");
        assert!(result.candidates[0].distance < 0.001);
    }

    #[tokio::test]
    async fn test_hnsw_search_accuracy() {
        let mut config = MemoryConfig::default();
        config.hnsw_m = 16;
        config.hnsw_ef_construction = 100;
        let memory = MemoryService::new(&config).await.unwrap();

        // Insert 100 random vectors
        let dim = 128;
        let mut rng = rand::thread_rng();
        let mut vectors = Vec::new();

        for i in 0..100 {
            let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() - 0.5).collect();
            // Normalize
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            vec.iter_mut().for_each(|x| *x /= norm);
            vectors.push(vec.clone());

            let node = create_test_node(&format!("node-{}", i), vec);
            memory.insert_node(node).unwrap();
        }

        // Search for a specific vector
        let query = vectors[42].clone();
        let result = memory.search_with_graph(&query, 10, 64, 0).await.unwrap();

        // The closest should be the exact match
        assert!(!result.candidates.is_empty());
        assert_eq!(result.candidates[0].id, "node-42");
        assert!(result.candidates[0].distance < 0.001);
    }

    #[tokio::test]
    async fn test_graph_expansion() {
        let config = MemoryConfig::default();
        let memory = MemoryService::new(&config).await.unwrap();

        // Create nodes
        for i in 0..5 {
            let node = create_test_node(&format!("node-{}", i), vec![i as f32, 0.0, 0.0]);
            memory.insert_node(node).unwrap();
        }

        // Create edges: 0 -> 1 -> 2 -> 3 -> 4
        for i in 0..4 {
            let edge = MemoryEdge {
                id: format!("edge-{}", i),
                src: format!("node-{}", i),
                dst: format!("node-{}", i + 1),
                edge_type: EdgeType::Follows,
                weight: 1.0,
                metadata: HashMap::new(),
            };
            memory.insert_edge(edge).unwrap();
        }

        // Expand from node-0 with 2 hops
        let subgraph = memory.expand_neighborhood(&["node-0".into()], 2).unwrap();

        // Should include node-0, node-1, node-2
        assert_eq!(subgraph.nodes.len(), 3);
        assert_eq!(subgraph.edges.len(), 2);
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let config = MemoryConfig::default();
        let memory = MemoryService::new(&config).await.unwrap();

        let nodes: Vec<MemoryNode> = (0..10)
            .map(|i| create_test_node(&format!("batch-{}", i), vec![i as f32; 3]))
            .collect();

        let ids = memory.insert_batch(nodes).unwrap();
        assert_eq!(ids.len(), 10);
        assert_eq!(memory.node_count(), 10);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!(cosine_distance(&a, &b) < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_distance(&a, &c) - 1.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_distance(&a, &d) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_edge_weight_update() {
        let config = MemoryConfig::default();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let memory = rt.block_on(MemoryService::new(&config)).unwrap();

        let edge = MemoryEdge {
            id: "e1".into(),
            src: "n1".into(),
            dst: "n2".into(),
            edge_type: EdgeType::Cites,
            weight: 0.5,
            metadata: HashMap::new(),
        };
        memory.insert_edge(edge).unwrap();

        // Update weight
        memory.update_edge_weight("n1", "n2", 0.2).unwrap();

        let edges = memory.get_edges("n1");
        assert_eq!(edges.len(), 1);
        assert!((edges[0].weight - 0.7).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_memory_stats() {
        let config = MemoryConfig::default();
        let memory = MemoryService::new(&config).await.unwrap();

        // Insert some nodes
        for i in 0..5 {
            let node = create_test_node(&format!("stat-{}", i), vec![i as f32; 3]);
            memory.insert_node(node).unwrap();
        }

        // Perform a search
        memory
            .search_with_graph(&[0.0, 0.0, 0.0], 5, 32, 0)
            .await
            .unwrap();

        let stats = memory.get_stats();
        assert_eq!(stats.node_count, 5);
        assert_eq!(stats.total_insertions, 5);
        assert_eq!(stats.total_searches, 1);
    }
}
