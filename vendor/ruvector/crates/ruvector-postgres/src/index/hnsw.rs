//! HNSW (Hierarchical Navigable Small World) index implementation
//!
//! Provides fast approximate nearest neighbor search with O(log n) complexity.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use dashmap::DashMap;
use parking_lot::RwLock;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::distance::{distance, DistanceMetric};

/// Maximum supported layers in HNSW graph (can be configured via max_layers)
pub const DEFAULT_MAX_LAYERS: usize = 32;

/// HNSW configuration parameters
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Maximum number of connections per layer (default: 16)
    pub m: usize,
    /// Maximum connections for layer 0 (default: 2*m)
    pub m0: usize,
    /// Build-time candidate list size (default: 64)
    pub ef_construction: usize,
    /// Query-time candidate list size (default: 40)
    pub ef_search: usize,
    /// Maximum elements (for pre-allocation)
    pub max_elements: usize,
    /// Distance metric
    pub metric: DistanceMetric,
    /// Random seed for reproducibility
    pub seed: u64,
    /// Maximum number of layers in the graph (default: 32)
    pub max_layers: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 64,
            ef_search: 40,
            max_elements: 1_000_000,
            metric: DistanceMetric::Euclidean,
            seed: 42,
            max_layers: DEFAULT_MAX_LAYERS,
        }
    }
}

/// Node ID type
pub type NodeId = u64;

/// Neighbor entry with distance
#[derive(Debug, Clone, Copy)]
struct Neighbor {
    id: NodeId,
    distance: f32,
}

impl PartialEq for Neighbor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Neighbor {}

impl PartialOrd for Neighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Neighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for max-heap (we want min distances first)
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Node in the HNSW graph
struct HnswNode {
    /// Vector data
    vector: Vec<f32>,
    /// Neighbors at each layer
    neighbors: Vec<RwLock<Vec<NodeId>>>,
    /// Maximum layer this node is present in
    #[allow(dead_code)]
    max_layer: usize,
}

/// HNSW Index
pub struct HnswIndex {
    /// Configuration
    config: HnswConfig,
    /// All nodes
    nodes: DashMap<NodeId, HnswNode>,
    /// Entry point (node at highest layer)
    entry_point: RwLock<Option<NodeId>>,
    /// Maximum layer in the index
    max_layer: AtomicUsize,
    /// Node counter
    node_count: AtomicUsize,
    /// Next node ID
    next_id: AtomicUsize,
    /// Random number generator
    rng: RwLock<ChaCha8Rng>,
    /// Dimensions
    dimensions: usize,
}

impl HnswIndex {
    /// Create a new HNSW index
    pub fn new(dimensions: usize, config: HnswConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);

        Self {
            config,
            nodes: DashMap::new(),
            entry_point: RwLock::new(None),
            max_layer: AtomicUsize::new(0),
            node_count: AtomicUsize::new(0),
            next_id: AtomicUsize::new(0),
            rng: RwLock::new(rng),
            dimensions,
        }
    }

    /// Get number of vectors in the index
    pub fn len(&self) -> usize {
        self.node_count.load(AtomicOrdering::Relaxed)
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Calculate random level for new node
    #[inline]
    fn random_level(&self) -> usize {
        let ml = 1.0 / (self.config.m as f64).ln();
        let mut rng = self.rng.write();
        let r: f64 = rng.gen();
        let level = (-r.ln() * ml).floor() as usize;
        level.min(self.config.max_layers) // Use configurable max layers
    }

    /// Calculate distance between two vectors
    #[inline]
    fn calc_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        distance(a, b, self.config.metric)
    }

    /// Insert a vector into the index
    ///
    /// Returns the assigned NodeId, or panics if the node ID space is exhausted.
    pub fn insert(&self, vector: Vec<f32>) -> NodeId {
        assert_eq!(vector.len(), self.dimensions, "Vector dimension mismatch");

        // Use checked arithmetic to detect overflow (theoretical for u64, but safe)
        let next_id = self.next_id.fetch_add(1, AtomicOrdering::Relaxed);
        if next_id == usize::MAX {
            panic!("HNSW index node ID overflow - maximum capacity reached");
        }
        let id = next_id as NodeId;
        let level = self.random_level();

        // Handle empty index (fast path - no searching needed, can avoid clone)
        let current_entry = *self.entry_point.read();
        if current_entry.is_none() {
            // Create node with empty neighbor lists for each layer
            let mut neighbors = Vec::with_capacity(level + 1);
            for _ in 0..=level {
                neighbors.push(RwLock::new(Vec::new()));
            }

            let node = HnswNode {
                vector, // Move without clone - first node doesn't need search
                neighbors,
                max_layer: level,
            };

            self.nodes.insert(id, node);
            *self.entry_point.write() = Some(id);
            self.max_layer.store(level, AtomicOrdering::Relaxed);
            self.node_count.fetch_add(1, AtomicOrdering::Relaxed);
            return id;
        }

        // For non-empty index: search FIRST with borrowed vector, then insert
        // This avoids cloning the vector entirely - zero-copy insert path
        let entry_point_id = current_entry.unwrap();
        let current_max_layer = self.max_layer.load(AtomicOrdering::Relaxed);

        // Search down from top layer to find entry point for insertion
        let mut curr_id = entry_point_id;

        // Descend through layers above the new node's max layer
        for layer in (level + 1..=current_max_layer).rev() {
            curr_id = self.search_layer_single(&vector, curr_id, layer);
        }

        // Collect all neighbor selections before inserting the node
        // This allows us to search with borrowed vector, then move it
        let mut layer_neighbors: Vec<Vec<NodeId>> =
            Vec::with_capacity(level.min(current_max_layer) + 1);

        for layer in (0..=level.min(current_max_layer)).rev() {
            let neighbors = self.search_layer(&vector, curr_id, self.config.ef_construction, layer);

            // Select best neighbors
            let max_connections = if layer == 0 {
                self.config.m0
            } else {
                self.config.m
            };
            let selected: Vec<NodeId> = neighbors
                .into_iter()
                .take(max_connections)
                .map(|n| n.id)
                .collect();

            // Update curr_id for next layer
            if !selected.is_empty() {
                curr_id = selected[0];
            }

            layer_neighbors.push(selected);
        }

        // Reverse since we collected in reverse order
        layer_neighbors.reverse();

        // NOW create and insert the node (moving the vector - no clone needed)
        let mut neighbors_vec = Vec::with_capacity(level + 1);
        for _ in 0..=level {
            neighbors_vec.push(RwLock::new(Vec::new()));
        }

        let node = HnswNode {
            vector, // Move original into node - zero copy!
            neighbors: neighbors_vec,
            max_layer: level,
        };
        self.nodes.insert(id, node);

        // Apply the pre-computed neighbor connections
        for (layer_idx, selected) in layer_neighbors.iter().enumerate() {
            let layer = layer_idx;

            // Set neighbors for new node
            if let Some(node) = self.nodes.get(&id) {
                if layer < node.neighbors.len() {
                    *node.neighbors[layer].write() = selected.clone();
                }
            }

            // Add bidirectional connections
            for &neighbor_id in selected {
                self.connect(neighbor_id, id, layer);
            }
        }

        // Update entry point if necessary
        if level > current_max_layer {
            self.max_layer.store(level, AtomicOrdering::Relaxed);
            *self.entry_point.write() = Some(id);
        }

        self.node_count.fetch_add(1, AtomicOrdering::Relaxed);
        id
    }

    /// Search for the single nearest neighbor in a layer (for descending)
    #[inline]
    fn search_layer_single(&self, query: &[f32], entry_id: NodeId, layer: usize) -> NodeId {
        let entry_node = self.nodes.get(&entry_id).unwrap();
        let mut best_id = entry_id;
        let mut best_dist = self.calc_distance(query, &entry_node.vector);
        drop(entry_node);

        loop {
            let mut changed = false;
            let node = self.nodes.get(&best_id).unwrap();

            if layer >= node.neighbors.len() {
                break;
            }

            let neighbors = node.neighbors[layer].read().clone();
            drop(node);

            for &neighbor_id in &neighbors {
                if let Some(neighbor) = self.nodes.get(&neighbor_id) {
                    let dist = self.calc_distance(query, &neighbor.vector);
                    if dist < best_dist {
                        best_dist = dist;
                        best_id = neighbor_id;
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        best_id
    }

    /// Search layer with beam search
    #[inline]
    fn search_layer(
        &self,
        query: &[f32],
        entry_id: NodeId,
        ef: usize,
        layer: usize,
    ) -> Vec<Neighbor> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();

        let entry_node = self.nodes.get(&entry_id).unwrap();
        let entry_dist = self.calc_distance(query, &entry_node.vector);
        drop(entry_node);

        visited.insert(entry_id);
        candidates.push(Neighbor {
            id: entry_id,
            distance: entry_dist,
        });
        results.push(Neighbor {
            id: entry_id,
            distance: -entry_dist,
        }); // Negative for max-heap

        while let Some(current) = candidates.pop() {
            let furthest_result = results.peek().map(|n| -n.distance).unwrap_or(f32::MAX);

            if current.distance > furthest_result && results.len() >= ef {
                break;
            }

            let node = match self.nodes.get(&current.id) {
                Some(n) => n,
                None => continue,
            };

            if layer >= node.neighbors.len() {
                continue;
            }

            let neighbors = node.neighbors[layer].read().clone();
            drop(node);

            for neighbor_id in neighbors {
                if visited.contains(&neighbor_id) {
                    continue;
                }
                visited.insert(neighbor_id);

                let neighbor = match self.nodes.get(&neighbor_id) {
                    Some(n) => n,
                    None => continue,
                };

                let dist = self.calc_distance(query, &neighbor.vector);
                drop(neighbor);

                let furthest_result = results.peek().map(|n| -n.distance).unwrap_or(f32::MAX);

                if dist < furthest_result || results.len() < ef {
                    candidates.push(Neighbor {
                        id: neighbor_id,
                        distance: dist,
                    });
                    results.push(Neighbor {
                        id: neighbor_id,
                        distance: -dist,
                    });

                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }

        // Convert to positive distances and sort
        let mut result_vec: Vec<Neighbor> = results
            .into_iter()
            .map(|n| Neighbor {
                id: n.id,
                distance: -n.distance,
            })
            .collect();
        result_vec.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(Ordering::Equal)
        });
        result_vec
    }

    /// Connect two nodes at a layer
    fn connect(&self, from_id: NodeId, to_id: NodeId, layer: usize) {
        if let Some(node) = self.nodes.get(&from_id) {
            if layer < node.neighbors.len() {
                let mut neighbors = node.neighbors[layer].write();
                let max_connections = if layer == 0 {
                    self.config.m0
                } else {
                    self.config.m
                };

                if neighbors.len() < max_connections {
                    if !neighbors.contains(&to_id) {
                        neighbors.push(to_id);
                    }
                } else {
                    // Need to prune - add new connection and remove worst
                    if !neighbors.contains(&to_id) {
                        neighbors.push(to_id);

                        // Calculate distances and prune
                        let mut with_dist: Vec<(NodeId, f32)> = neighbors
                            .iter()
                            .filter_map(|&id| {
                                self.nodes.get(&id).map(|n| {
                                    let dist = self.calc_distance(&node.vector, &n.vector);
                                    (id, dist)
                                })
                            })
                            .collect();

                        with_dist.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                        *neighbors = with_dist
                            .into_iter()
                            .take(max_connections)
                            .map(|(id, _)| id)
                            .collect();
                    }
                }
            }
        }
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize, ef_search: Option<usize>) -> Vec<(NodeId, f32)> {
        assert_eq!(query.len(), self.dimensions, "Query dimension mismatch");

        let ef = ef_search.unwrap_or(self.config.ef_search).max(k);

        let entry_point = match *self.entry_point.read() {
            Some(ep) => ep,
            None => return Vec::new(),
        };

        let max_layer = self.max_layer.load(AtomicOrdering::Relaxed);

        // Descend through layers
        let mut curr_id = entry_point;
        for layer in (1..=max_layer).rev() {
            curr_id = self.search_layer_single(query, curr_id, layer);
        }

        // Search at layer 0
        let results = self.search_layer(query, curr_id, ef, 0);

        // Return top k
        results
            .into_iter()
            .take(k)
            .map(|n| (n.id, n.distance))
            .collect()
    }

    /// Get vector by ID
    pub fn get_vector(&self, id: NodeId) -> Option<Vec<f32>> {
        self.nodes.get(&id).map(|n| n.vector.clone())
    }

    /// Delete a vector (marks as deleted, doesn't reclaim space)
    pub fn delete(&self, id: NodeId) -> bool {
        self.nodes.remove(&id).is_some()
    }

    /// Get approximate memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        let vector_bytes = self.len() * self.dimensions * 4;
        let neighbor_overhead = self.len() * self.config.m * 8 * 2; // Rough estimate
        vector_bytes + neighbor_overhead
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_search() {
        let config = HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 32,
            ef_search: 20,
            max_elements: 1000,
            metric: DistanceMetric::Euclidean,
            seed: 42,
            max_layers: 16,
        };

        let index = HnswIndex::new(3, config);

        // Insert vectors
        index.insert(vec![0.0, 0.0, 0.0]);
        index.insert(vec![1.0, 0.0, 0.0]);
        index.insert(vec![0.0, 1.0, 0.0]);
        index.insert(vec![0.0, 0.0, 1.0]);
        index.insert(vec![1.0, 1.0, 1.0]);

        assert_eq!(index.len(), 5);

        // Search
        let results = index.search(&[0.1, 0.1, 0.1], 3, None);
        assert!(!results.is_empty());

        // First result should be closest to query
        let (id, dist) = results[0];
        assert!(dist < 0.5, "Expected close match, got distance {}", dist);
    }

    #[test]
    fn test_empty_index() {
        let index = HnswIndex::new(3, HnswConfig::default());
        assert!(index.is_empty());

        let results = index.search(&[0.0, 0.0, 0.0], 10, None);
        assert!(results.is_empty());
    }

    #[test]
    fn test_cosine_metric() {
        let mut config = HnswConfig::default();
        config.metric = DistanceMetric::Cosine;

        let index = HnswIndex::new(3, config);

        index.insert(vec![1.0, 0.0, 0.0]);
        index.insert(vec![0.0, 1.0, 0.0]);
        index.insert(vec![0.0, 0.0, 1.0]);

        let results = index.search(&[1.0, 0.0, 0.0], 1, None);
        assert_eq!(results.len(), 1);

        // Distance should be ~0 for same direction
        assert!(results[0].1 < 0.01);
    }

    #[test]
    fn test_high_dimensional() {
        let dims = 128;
        let config = HnswConfig {
            m: 16,
            m0: 32,
            ef_construction: 64,
            ef_search: 40,
            max_elements: 10000,
            metric: DistanceMetric::Euclidean,
            seed: 42,
            max_layers: 16,
        };

        let index = HnswIndex::new(dims, config);

        // Insert 100 random vectors
        for i in 0..100 {
            let vector: Vec<f32> = (0..dims).map(|j| (i + j) as f32 * 0.01).collect();
            index.insert(vector);
        }

        assert_eq!(index.len(), 100);

        // Search
        let query: Vec<f32> = (0..dims).map(|i| i as f32 * 0.01).collect();
        let results = index.search(&query, 10, None);

        assert_eq!(results.len(), 10);
    }
}
