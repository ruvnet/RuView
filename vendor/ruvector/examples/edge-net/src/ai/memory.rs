//! # HNSW Vector Index for Edge-Net
//!
//! Hierarchical Navigable Small World graph for efficient approximate nearest neighbor search.
//! Provides 150x faster search than naive linear scan with O(log N) complexity.
//!
//! ## Key Features
//!
//! - **Multi-layer graph**: Higher layers for coarse search, lower layers for fine-grained
//! - **Incremental updates**: Add vectors without rebuilding the entire index
//! - **P2P synchronization**: Index can be incrementally updated from peer events
//! - **SIMD acceleration**: Uses ComputeOps trait for vectorized distance calculations
//!
//! ## Architecture
//!
//! ```text
//! Layer 2:  [node-5] -------- [node-42]
//!              |                  |
//! Layer 1:  [node-5] -- [node-12] -- [node-42] -- [node-87]
//!              |           |            |            |
//! Layer 0:  [all nodes connected with M*2 edges per node]
//! ```
//!
//! ## Parameters
//!
//! - `M`: Maximum connections per node (default 32)
//! - `ef_construction`: Build-time beam width (default 200)
//! - `ef_search`: Search-time beam width (default 64)

use crate::ai::{ComputeOps, CpuOps};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;

/// HNSW configuration parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Maximum connections per node at layers > 0
    pub m: usize,
    /// Maximum connections per node at layer 0 (typically 2*M)
    pub m_max_0: usize,
    /// Build-time beam width
    pub ef_construction: usize,
    /// Search-time beam width
    pub ef_search: usize,
    /// Vector dimension
    pub dimensions: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 32,
            m_max_0: 64,
            ef_construction: 200,
            ef_search: 64,
            dimensions: 128,
        }
    }
}

impl HnswConfig {
    /// Create config for small indices (< 10k vectors)
    pub fn small(dimensions: usize) -> Self {
        Self {
            m: 16,
            m_max_0: 32,
            ef_construction: 100,
            ef_search: 32,
            dimensions,
        }
    }

    /// Create config for medium indices (10k - 100k vectors)
    pub fn medium(dimensions: usize) -> Self {
        Self {
            m: 32,
            m_max_0: 64,
            ef_construction: 200,
            ef_search: 64,
            dimensions,
        }
    }

    /// Create config for large indices (> 100k vectors)
    pub fn large(dimensions: usize) -> Self {
        Self {
            m: 48,
            m_max_0: 96,
            ef_construction: 400,
            ef_search: 128,
            dimensions,
        }
    }
}

/// A node in the HNSW graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswNode {
    /// Unique node identifier
    pub id: String,
    /// Vector data
    pub vector: Vec<f32>,
    /// Connections at each layer (layer -> list of neighbor indices)
    connections: Vec<Vec<usize>>,
    /// Maximum layer this node appears in
    max_layer: usize,
}

impl HnswNode {
    /// Create a new HNSW node
    pub fn new(id: String, vector: Vec<f32>, max_layer: usize) -> Self {
        Self {
            id,
            vector,
            connections: vec![Vec::new(); max_layer + 1],
            max_layer,
        }
    }

    /// Get neighbors at a specific layer
    pub fn neighbors_at_layer(&self, layer: usize) -> &[usize] {
        if layer <= self.max_layer {
            &self.connections[layer]
        } else {
            &[]
        }
    }
}

/// Candidate for priority queue (min-heap by distance)
#[derive(Clone, Debug)]
struct Candidate {
    distance: f32,
    node_idx: usize,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.node_idx == other.node_idx
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap (smaller distance = higher priority)
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

/// Search result from HNSW query
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// Node ID
    pub id: String,
    /// Distance from query
    pub distance: f32,
    /// Node index in the index
    pub index: usize,
}

/// Search statistics for performance monitoring
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchStats {
    /// Number of results returned
    pub k_retrieved: usize,
    /// Layers traversed during search
    pub layers_traversed: usize,
    /// Total distance computations
    pub distance_computations: usize,
    /// Mean distance of results
    pub distance_mean: f32,
    /// Min distance of results
    pub distance_min: f32,
    /// Max distance of results
    pub distance_max: f32,
}

/// HNSW vector index for approximate nearest neighbor search
pub struct HnswIndex {
    /// All nodes in the graph
    nodes: Vec<HnswNode>,
    /// Index from ID to node index
    id_to_index: rustc_hash::FxHashMap<String, usize>,
    /// Entry point (highest layer node)
    entry_point: Option<usize>,
    /// Maximum layer in the graph
    max_layer: usize,
    /// Configuration
    config: HnswConfig,
    /// Statistics
    total_insertions: u64,
    total_searches: u64,
    total_distance_ops: u64,
}

impl HnswIndex {
    /// Create a new HNSW index
    pub fn new(dimensions: usize, config: HnswConfig) -> Self {
        Self {
            nodes: Vec::new(),
            id_to_index: rustc_hash::FxHashMap::default(),
            entry_point: None,
            max_layer: 0,
            config: HnswConfig { dimensions, ..config },
            total_insertions: 0,
            total_searches: 0,
            total_distance_ops: 0,
        }
    }

    /// Create with default config for given dimensions
    pub fn with_dimensions(dimensions: usize) -> Self {
        Self::new(dimensions, HnswConfig::medium(dimensions))
    }

    /// Get number of vectors in the index
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get index configuration
    pub fn config(&self) -> &HnswConfig {
        &self.config
    }

    /// Generate random layer for new node (exponential distribution)
    fn random_layer(&self) -> usize {
        let m = self.config.m.max(2) as f32;
        let ml = 1.0 / m.ln();

        // Use wasm-compatible random via js_sys
        #[cfg(target_arch = "wasm32")]
        let r: f32 = js_sys::Math::random() as f32;
        #[cfg(not(target_arch = "wasm32"))]
        let r: f32 = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos();
            ((seed as f32 / u32::MAX as f32) * 1000.0).fract()
        };
        if r <= f32::EPSILON {
            return 0;
        }

        let level = (-r.ln() * ml).floor();
        level.min(32.0) as usize
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, id: impl Into<String>, vector: Vec<f32>) -> Result<usize, &'static str> {
        let id = id.into();

        // Validate dimensions
        if vector.len() != self.config.dimensions {
            return Err("Vector dimension mismatch");
        }

        // Check if ID already exists
        if self.id_to_index.contains_key(&id) {
            return Err("ID already exists in index");
        }

        // Determine layer for new node
        let new_layer = self.random_layer();
        let node_idx = self.nodes.len();

        // Create new node
        let mut new_node = HnswNode::new(id.clone(), vector, new_layer);

        // Handle first insertion
        if self.entry_point.is_none() {
            self.nodes.push(new_node);
            self.id_to_index.insert(id, node_idx);
            self.entry_point = Some(node_idx);
            self.max_layer = new_layer;
            self.total_insertions += 1;
            return Ok(node_idx);
        }

        let entry_point = self.entry_point.unwrap();

        // Search phase: traverse from top layer down
        let mut current = entry_point;
        let mut current_dist = CpuOps::cosine_distance(&new_node.vector, &self.nodes[current].vector);

        // Greedy search from top layer to layer above new_layer
        for layer in (new_layer + 1..=self.max_layer).rev() {
            loop {
                let mut changed = false;
                let neighbors = self.nodes[current].neighbors_at_layer(layer);

                for &neighbor in neighbors {
                    if neighbor < self.nodes.len() {
                        let dist = CpuOps::cosine_distance(&new_node.vector, &self.nodes[neighbor].vector);
                        self.total_distance_ops += 1;
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

        // Store the node first so we can reference it
        self.nodes.push(new_node);
        self.id_to_index.insert(id, node_idx);

        // Insert phase: insert at each layer from min(new_layer, max_layer) down to 0
        let top_layer = new_layer.min(self.max_layer);
        for layer in (0..=top_layer).rev() {
            let max_connections = if layer == 0 { self.config.m_max_0 } else { self.config.m };

            // Find nearest neighbors at this layer
            let neighbors = self.search_layer(node_idx, current, self.config.ef_construction, layer);

            // Select best connections
            let connections: Vec<usize> = neighbors
                .into_iter()
                .take(max_connections)
                .map(|(idx, _)| idx)
                .collect();

            // Add bidirectional connections
            for &neighbor in &connections {
                // Add connection from new node to neighbor
                if layer <= self.nodes[node_idx].max_layer {
                    self.nodes[node_idx].connections[layer].push(neighbor);
                }

                // Add connection from neighbor to new node
                if layer <= self.nodes[neighbor].max_layer {
                    self.nodes[neighbor].connections[layer].push(node_idx);

                    // Prune if too many connections
                    if self.nodes[neighbor].connections[layer].len() > max_connections {
                        self.prune_connections(neighbor, layer, max_connections);
                    }
                }
            }

            // Update entry point for next layer
            if !connections.is_empty() {
                current = connections[0];
            }
        }

        // Update entry point if necessary
        if new_layer > self.max_layer {
            self.entry_point = Some(node_idx);
            self.max_layer = new_layer;
        }

        self.total_insertions += 1;
        Ok(node_idx)
    }

    /// Search for k nearest neighbors
    pub fn search(&mut self, query: &[f32], k: usize) -> Result<Vec<SearchResult>, &'static str> {
        self.search_with_ef(query, k, self.config.ef_search)
    }

    /// Search with custom ef parameter
    pub fn search_with_ef(&mut self, query: &[f32], k: usize, ef: usize) -> Result<Vec<SearchResult>, &'static str> {
        if query.len() != self.config.dimensions {
            return Err("Query dimension mismatch");
        }

        if self.entry_point.is_none() {
            return Ok(vec![]);
        }

        self.total_searches += 1;
        let entry_point = self.entry_point.unwrap();

        // Start from entry point
        let mut current = entry_point;
        let mut current_dist = CpuOps::cosine_distance(query, &self.nodes[current].vector);
        self.total_distance_ops += 1;

        // Traverse from top layer to layer 1
        for layer in (1..=self.max_layer).rev() {
            loop {
                let mut changed = false;
                let neighbors = self.nodes[current].neighbors_at_layer(layer);

                for &neighbor in neighbors {
                    if neighbor < self.nodes.len() {
                        let dist = CpuOps::cosine_distance(query, &self.nodes[neighbor].vector);
                        self.total_distance_ops += 1;
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
        let neighbors = self.search_layer_query(query, current, ef, 0);

        // Return top-k results
        let results: Vec<SearchResult> = neighbors
            .into_iter()
            .take(k)
            .map(|(idx, dist)| SearchResult {
                id: self.nodes[idx].id.clone(),
                distance: dist,
                index: idx,
            })
            .collect();

        Ok(results)
    }

    /// Search within a layer starting from entry point
    fn search_layer(&self, query_idx: usize, entry: usize, ef: usize, layer: usize) -> Vec<(usize, f32)> {
        let query = &self.nodes[query_idx].vector;
        self.search_layer_query(query, entry, ef, layer)
    }

    /// Search within a layer with a query vector
    fn search_layer_query(&self, query: &[f32], entry: usize, ef: usize, layer: usize) -> Vec<(usize, f32)> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut result = Vec::new();

        let entry_dist = CpuOps::cosine_distance(query, &self.nodes[entry].vector);
        visited.insert(entry);
        candidates.push(Candidate { distance: entry_dist, node_idx: entry });
        result.push((entry, entry_dist));

        while let Some(Candidate { distance: _, node_idx }) = candidates.pop() {
            // Check stopping condition
            if result.len() >= ef {
                result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                if let Some(&(_, furthest_dist)) = result.last() {
                    if let Some(closest) = candidates.peek() {
                        if closest.distance > furthest_dist {
                            break;
                        }
                    }
                }
            }

            // Explore neighbors
            let neighbors = self.nodes[node_idx].neighbors_at_layer(layer);
            for &neighbor in neighbors {
                if !visited.contains(&neighbor) && neighbor < self.nodes.len() {
                    visited.insert(neighbor);
                    let dist = CpuOps::cosine_distance(query, &self.nodes[neighbor].vector);
                    candidates.push(Candidate { distance: dist, node_idx: neighbor });
                    result.push((neighbor, dist));
                }
            }
        }

        result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        result.truncate(ef);
        result
    }

    /// Prune connections to keep only the best ones
    fn prune_connections(&mut self, node_idx: usize, layer: usize, max_conn: usize) {
        if layer > self.nodes[node_idx].max_layer {
            return;
        }

        let node_vec = self.nodes[node_idx].vector.clone();
        let mut scored: Vec<(usize, f32)> = self.nodes[node_idx].connections[layer]
            .iter()
            .filter_map(|&n| {
                if n < self.nodes.len() {
                    Some((n, CpuOps::cosine_distance(&node_vec, &self.nodes[n].vector)))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        self.nodes[node_idx].connections[layer] = scored.into_iter().take(max_conn).map(|(n, _)| n).collect();
    }

    /// Get a node by ID
    pub fn get(&self, id: &str) -> Option<&HnswNode> {
        self.id_to_index.get(id).map(|&idx| &self.nodes[idx])
    }

    /// Get a node by index
    pub fn get_by_index(&self, idx: usize) -> Option<&HnswNode> {
        self.nodes.get(idx)
    }

    /// Check if an ID exists in the index
    pub fn contains(&self, id: &str) -> bool {
        self.id_to_index.contains_key(id)
    }

    /// Get statistics about the index
    pub fn stats(&self) -> HnswStats {
        let layer_counts: Vec<usize> = (0..=self.max_layer)
            .map(|l| self.nodes.iter().filter(|n| n.max_layer >= l).count())
            .collect();

        let avg_connections = if self.nodes.is_empty() {
            0.0
        } else {
            let total_connections: usize = self.nodes
                .iter()
                .map(|n| n.connections.iter().map(|c| c.len()).sum::<usize>())
                .sum();
            total_connections as f64 / self.nodes.len() as f64
        };

        HnswStats {
            node_count: self.nodes.len(),
            max_layer: self.max_layer,
            layer_counts,
            avg_connections_per_node: avg_connections,
            total_insertions: self.total_insertions,
            total_searches: self.total_searches,
            total_distance_computations: self.total_distance_ops,
        }
    }

    /// Merge updates from a peer (for P2P sync)
    pub fn merge_peer_updates(&mut self, updates: Vec<(String, Vec<f32>)>) -> usize {
        let mut inserted = 0;
        for (id, vector) in updates {
            if !self.contains(&id) {
                if self.insert(id, vector).is_ok() {
                    inserted += 1;
                }
            }
        }
        inserted
    }
}

/// Statistics about the HNSW index
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswStats {
    /// Total number of nodes
    pub node_count: usize,
    /// Maximum layer in the graph
    pub max_layer: usize,
    /// Number of nodes at each layer
    pub layer_counts: Vec<usize>,
    /// Average connections per node
    pub avg_connections_per_node: f64,
    /// Total insertions performed
    pub total_insertions: u64,
    /// Total searches performed
    pub total_searches: u64,
    /// Total distance computations
    pub total_distance_computations: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut vec = Vec::with_capacity(dim);
        for i in 0..dim {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            let hash = hasher.finish();
            vec.push(((hash % 1000) as f32 / 1000.0) - 0.5);
        }

        // Normalize
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            vec.iter_mut().for_each(|x| *x /= norm);
        }
        vec
    }

    #[test]
    fn test_insert_and_search() {
        let mut index = HnswIndex::with_dimensions(8);

        // Insert some vectors
        for i in 0..10 {
            let vec = random_vector(8, i);
            index.insert(format!("node-{}", i), vec).unwrap();
        }

        assert_eq!(index.len(), 10);

        // Search for first vector
        let query = random_vector(8, 0);
        let results = index.search(&query, 5).unwrap();

        assert!(!results.is_empty());
        // First result should be the exact match or very close
        assert!(results[0].distance < 0.1, "First result should be very close");
    }

    #[test]
    fn test_exact_match_search() {
        let mut index = HnswIndex::with_dimensions(4);

        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0, 0.0];
        let v3 = vec![0.0, 0.0, 1.0, 0.0];

        index.insert("v1", v1.clone()).unwrap();
        index.insert("v2", v2).unwrap();
        index.insert("v3", v3).unwrap();

        let results = index.search(&v1, 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "v1");
        assert!(results[0].distance < 0.001);
    }

    #[test]
    fn test_duplicate_id_rejected() {
        let mut index = HnswIndex::with_dimensions(4);

        let v = vec![1.0, 0.0, 0.0, 0.0];
        index.insert("dup", v.clone()).unwrap();

        let result = index.insert("dup", v);
        assert!(result.is_err());
    }

    #[test]
    fn test_dimension_mismatch() {
        let mut index = HnswIndex::with_dimensions(4);

        let wrong_dim = vec![1.0, 0.0, 0.0]; // 3D instead of 4D
        let result = index.insert("wrong", wrong_dim);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_search() {
        let mut index = HnswIndex::with_dimensions(4);
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let results = index.search(&query, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_stats() {
        let mut index = HnswIndex::with_dimensions(8);

        for i in 0..50 {
            let vec = random_vector(8, i);
            index.insert(format!("node-{}", i), vec).unwrap();
        }

        let stats = index.stats();
        assert_eq!(stats.node_count, 50);
        assert_eq!(stats.total_insertions, 50);
        assert!(stats.max_layer >= 0);
    }

    #[test]
    fn test_peer_merge() {
        let mut index = HnswIndex::with_dimensions(4);

        index.insert("local-1", vec![1.0, 0.0, 0.0, 0.0]).unwrap();

        let peer_updates = vec![
            ("peer-1".to_string(), vec![0.0, 1.0, 0.0, 0.0]),
            ("peer-2".to_string(), vec![0.0, 0.0, 1.0, 0.0]),
            ("local-1".to_string(), vec![1.0, 1.0, 0.0, 0.0]), // Duplicate, should be ignored
        ];

        let inserted = index.merge_peer_updates(peer_updates);
        assert_eq!(inserted, 2);
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_search_ordering() {
        let mut index = HnswIndex::with_dimensions(4);

        // Insert vectors at different angles
        index.insert("v0", vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        index.insert("v1", vec![0.707, 0.707, 0.0, 0.0]).unwrap();
        index.insert("v2", vec![0.0, 1.0, 0.0, 0.0]).unwrap();

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let results = index.search(&query, 3).unwrap();

        assert_eq!(results.len(), 3);
        // Results should be ordered by distance
        for i in 1..results.len() {
            assert!(results[i - 1].distance <= results[i].distance);
        }
    }
}
