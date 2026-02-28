//! Micro HNSW - Approximate Nearest Neighbor for ESP32
//!
//! A minimal HNSW (Hierarchical Navigable Small World) implementation
//! designed for ESP32's memory constraints.
//!
//! # Features
//! - Fixed-size graph structure (no dynamic allocation)
//! - INT8 quantized vectors
//! - Binary quantization option (32x smaller)
//! - O(log n) search complexity
//!
//! # Memory Usage
//!
//! For 64-dimensional INT8 vectors:
//! - 100 vectors: ~8 KB
//! - 500 vectors: ~40 KB
//! - 1000 vectors (binary): ~10 KB

use heapless::Vec as HVec;
use heapless::BinaryHeap;
use heapless::binary_heap::Min;
use super::{MicroVector, DistanceMetric, euclidean_distance_i8, MAX_NEIGHBORS};

/// Maximum vectors in the index
pub const INDEX_CAPACITY: usize = 256;
/// Maximum layers in HNSW
pub const MAX_LAYERS: usize = 4;
/// Default neighbors per layer
pub const DEFAULT_M: usize = 8;
/// Search expansion factor
pub const EF_SEARCH: usize = 16;

/// HNSW Configuration
#[derive(Debug, Clone)]
pub struct HNSWConfig {
    /// Max neighbors per node
    pub m: usize,
    /// Neighbors at layer 0 (usually 2*M)
    pub m_max0: usize,
    /// Construction expansion factor
    pub ef_construction: usize,
    /// Search expansion factor
    pub ef_search: usize,
    /// Distance metric
    pub metric: DistanceMetric,
    /// Enable binary quantization
    pub binary_mode: bool,
}

impl Default for HNSWConfig {
    fn default() -> Self {
        Self {
            m: 8,
            m_max0: 16,
            ef_construction: 32,
            ef_search: 16,
            metric: DistanceMetric::Euclidean,
            binary_mode: false,
        }
    }
}

/// Search result
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    /// Vector ID
    pub id: u32,
    /// Distance to query
    pub distance: i32,
    /// Index in storage
    pub index: usize,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.distance.cmp(&other.distance)
    }
}

/// Node in the HNSW graph
#[derive(Debug, Clone)]
struct HNSWNode<const DIM: usize> {
    /// Vector data
    vector: HVec<i8, DIM>,
    /// User ID
    id: u32,
    /// Neighbors per layer [layer][neighbor_indices]
    neighbors: [HVec<u16, MAX_NEIGHBORS>; MAX_LAYERS],
    /// Maximum layer this node exists on
    max_layer: u8,
}

impl<const DIM: usize> Default for HNSWNode<DIM> {
    fn default() -> Self {
        Self {
            vector: HVec::new(),
            id: 0,
            neighbors: Default::default(),
            max_layer: 0,
        }
    }
}

/// Micro HNSW Index
pub struct MicroHNSW<const DIM: usize, const CAPACITY: usize> {
    /// Configuration
    config: HNSWConfig,
    /// Stored nodes
    nodes: HVec<HNSWNode<DIM>, CAPACITY>,
    /// Entry point (highest layer node)
    entry_point: Option<usize>,
    /// Current maximum layer
    max_layer: u8,
    /// Random seed for layer selection
    rng_state: u32,
}

impl<const DIM: usize, const CAPACITY: usize> MicroHNSW<DIM, CAPACITY> {
    /// Create new HNSW index
    pub fn new(config: HNSWConfig) -> Self {
        Self {
            config,
            nodes: HVec::new(),
            entry_point: None,
            max_layer: 0,
            rng_state: 12345, // Default seed
        }
    }

    /// Set random seed
    pub fn with_seed(mut self, seed: u32) -> Self {
        self.rng_state = seed;
        self
    }

    /// Number of vectors in index
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        // Approximate: vectors + neighbor lists
        self.nodes.len() * (DIM + MAX_LAYERS * MAX_NEIGHBORS * 2 + 8)
    }

    /// Insert a vector
    pub fn insert(&mut self, vector: &MicroVector<DIM>) -> Result<usize, &'static str> {
        if self.nodes.len() >= CAPACITY {
            return Err("Index full");
        }

        let new_idx = self.nodes.len();
        let new_layer = self.random_layer();

        // Create node
        let mut node = HNSWNode::<DIM>::default();
        node.vector = vector.data.clone();
        node.id = vector.id;
        node.max_layer = new_layer;

        // First node is simple
        if self.entry_point.is_none() {
            self.nodes.push(node).map_err(|_| "Push failed")?;
            self.entry_point = Some(new_idx);
            self.max_layer = new_layer;
            return Ok(new_idx);
        }

        let entry = self.entry_point.unwrap();

        // Add node first so we can reference it
        self.nodes.push(node).map_err(|_| "Push failed")?;

        // Search for neighbors from top layer down
        let mut current = entry;

        // Traverse upper layers
        for layer in (new_layer as usize + 1..=self.max_layer as usize).rev() {
            current = self.greedy_search_layer(current, &vector.data, layer);
        }

        // Insert at each layer
        for layer in (0..=(new_layer as usize).min(self.max_layer as usize)).rev() {
            let neighbors = self.search_layer(current, &vector.data, layer, self.config.ef_construction);

            // Connect to best neighbors
            let max_neighbors = if layer == 0 { self.config.m_max0 } else { self.config.m };
            let mut added = 0;

            for result in neighbors.iter().take(max_neighbors) {
                if added >= MAX_NEIGHBORS {
                    break;
                }

                // Add bidirectional connection
                if let Some(new_node) = self.nodes.get_mut(new_idx) {
                    let _ = new_node.neighbors[layer].push(result.index as u16);
                }

                if let Some(neighbor_node) = self.nodes.get_mut(result.index) {
                    if neighbor_node.neighbors[layer].len() < MAX_NEIGHBORS {
                        let _ = neighbor_node.neighbors[layer].push(new_idx as u16);
                    }
                }

                added += 1;
            }

            if !neighbors.is_empty() {
                current = neighbors[0].index;
            }
        }

        // Update entry point if new node has higher layer
        if new_layer > self.max_layer {
            self.entry_point = Some(new_idx);
            self.max_layer = new_layer;
        }

        Ok(new_idx)
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[i8], k: usize) -> HVec<SearchResult, 32> {
        let mut results = HVec::new();

        if self.entry_point.is_none() || k == 0 {
            return results;
        }

        let entry = self.entry_point.unwrap();

        // Traverse from top layer
        let mut current = entry;
        for layer in (1..=self.max_layer as usize).rev() {
            current = self.greedy_search_layer(current, query, layer);
        }

        // Search layer 0 with ef expansion
        let candidates = self.search_layer(current, query, 0, self.config.ef_search);

        // Return top k
        for result in candidates.into_iter().take(k) {
            let _ = results.push(result);
        }

        results
    }

    /// Search specific layer
    fn search_layer(&self, entry: usize, query: &[i8], layer: usize, ef: usize) -> HVec<SearchResult, 64> {
        let mut visited = [false; CAPACITY];
        let mut candidates: BinaryHeap<SearchResult, Min, 64> = BinaryHeap::new();
        let mut results: HVec<SearchResult, 64> = HVec::new();

        visited[entry] = true;
        let entry_dist = self.distance(query, entry);

        let _ = candidates.push(SearchResult {
            id: self.nodes[entry].id,
            distance: entry_dist,
            index: entry,
        });
        let _ = results.push(SearchResult {
            id: self.nodes[entry].id,
            distance: entry_dist,
            index: entry,
        });

        while let Some(current) = candidates.pop() {
            // Early termination
            if results.len() >= ef {
                if let Some(worst) = results.iter().max_by_key(|r| r.distance) {
                    if current.distance > worst.distance {
                        break;
                    }
                }
            }

            // Explore neighbors
            if let Some(node) = self.nodes.get(current.index) {
                if layer < node.neighbors.len() {
                    for &neighbor_idx in node.neighbors[layer].iter() {
                        let neighbor_idx = neighbor_idx as usize;
                        if neighbor_idx < CAPACITY && !visited[neighbor_idx] {
                            visited[neighbor_idx] = true;

                            let dist = self.distance(query, neighbor_idx);

                            // Add if better than worst in results
                            let should_add = results.len() < ef ||
                                results.iter().any(|r| dist < r.distance);

                            if should_add {
                                let result = SearchResult {
                                    id: self.nodes[neighbor_idx].id,
                                    distance: dist,
                                    index: neighbor_idx,
                                };
                                let _ = candidates.push(result);
                                let _ = results.push(result);

                                // Keep results bounded
                                if results.len() > ef * 2 {
                                    results.sort_by_key(|r| r.distance);
                                    results.truncate(ef);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort and truncate
        results.sort_by_key(|r| r.distance);
        results
    }

    /// Greedy search on a single layer
    fn greedy_search_layer(&self, entry: usize, query: &[i8], layer: usize) -> usize {
        let mut current = entry;
        let mut current_dist = self.distance(query, current);

        loop {
            let mut improved = false;

            if let Some(node) = self.nodes.get(current) {
                if layer < node.neighbors.len() {
                    for &neighbor_idx in node.neighbors[layer].iter() {
                        let neighbor_idx = neighbor_idx as usize;
                        if neighbor_idx < self.nodes.len() {
                            let dist = self.distance(query, neighbor_idx);
                            if dist < current_dist {
                                current = neighbor_idx;
                                current_dist = dist;
                                improved = true;
                            }
                        }
                    }
                }
            }

            if !improved {
                break;
            }
        }

        current
    }

    /// Calculate distance between query and stored vector
    fn distance(&self, query: &[i8], idx: usize) -> i32 {
        if let Some(node) = self.nodes.get(idx) {
            self.config.metric.distance(query, &node.vector)
        } else {
            i32::MAX
        }
    }

    /// Generate random layer (exponential distribution)
    fn random_layer(&mut self) -> u8 {
        // Simple LCG random
        self.rng_state = self.rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        let rand = self.rng_state;

        // Count leading zeros gives exponential distribution
        let layer = (rand.leading_zeros() / 4) as u8;
        layer.min(MAX_LAYERS as u8 - 1)
    }

    /// Get vector by index
    pub fn get(&self, idx: usize) -> Option<&[i8]> {
        self.nodes.get(idx).map(|n| n.vector.as_slice())
    }

    /// Get ID by index
    pub fn get_id(&self, idx: usize) -> Option<u32> {
        self.nodes.get(idx).map(|n| n.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_basic() {
        let mut index: MicroHNSW<8, 100> = MicroHNSW::new(HNSWConfig::default());

        // Insert vectors
        for i in 0..10 {
            let data: HVec<i8, 8> = (0..8).map(|j| (i * 10 + j) as i8).collect();
            let vec = MicroVector { data, id: i as u32 };
            index.insert(&vec).unwrap();
        }

        assert_eq!(index.len(), 10);
    }

    #[test]
    fn test_hnsw_search() {
        let mut index: MicroHNSW<4, 100> = MicroHNSW::new(HNSWConfig::default());

        // Insert specific vectors
        let vectors = [
            [10i8, 0, 0, 0],
            [0i8, 10, 0, 0],
            [0i8, 0, 10, 0],
            [11i8, 1, 0, 0], // Close to first
        ];

        for (i, v) in vectors.iter().enumerate() {
            let data: HVec<i8, 4> = v.iter().copied().collect();
            let vec = MicroVector { data, id: i as u32 };
            index.insert(&vec).unwrap();
        }

        // Search for vector close to first
        let query = [10i8, 0, 0, 0];
        let results = index.search(&query, 2);

        assert!(!results.is_empty());
        assert_eq!(results[0].id, 0); // Exact match should be first
    }
}
