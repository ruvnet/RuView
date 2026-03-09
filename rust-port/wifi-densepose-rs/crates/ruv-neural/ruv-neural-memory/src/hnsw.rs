//! Simplified HNSW (Hierarchical Navigable Small World) index for approximate
//! nearest neighbor search on embedding vectors.

use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;

/// A scored neighbor for use in the priority queue.
#[derive(Debug, Clone)]
struct ScoredNode {
    id: usize,
    distance: f64,
}

impl PartialEq for ScoredNode {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for ScoredNode {}

impl PartialOrd for ScoredNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Max-heap scored node (furthest first).
#[derive(Debug, Clone)]
struct FurthestNode {
    id: usize,
    distance: f64,
}

impl PartialEq for FurthestNode {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for FurthestNode {}

impl PartialOrd for FurthestNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FurthestNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Hierarchical Navigable Small World graph for approximate nearest neighbor search.
///
/// This is a simplified single-layer HNSW implementation suitable for moderate-scale
/// embedding stores (up to ~100k vectors).
pub struct HnswIndex {
    /// Adjacency list per layer: layers[layer][node] = [(neighbor_id, distance)]
    layers: Vec<Vec<Vec<(usize, f64)>>>,
    /// Entry point node for search.
    entry_point: usize,
    /// Maximum layer index currently in the graph.
    max_layer: usize,
    /// Number of neighbors to consider during construction.
    ef_construction: usize,
    /// Maximum number of connections per node per layer.
    m: usize,
    /// Stored embedding vectors.
    embeddings: Vec<Vec<f64>>,
}

impl HnswIndex {
    /// Create a new empty HNSW index.
    ///
    /// - `m`: maximum connections per node per layer (typical: 16)
    /// - `ef_construction`: search width during construction (typical: 200)
    pub fn new(m: usize, ef_construction: usize) -> Self {
        Self {
            layers: vec![Vec::new()], // Start with layer 0
            entry_point: 0,
            max_layer: 0,
            ef_construction,
            m,
            embeddings: Vec::new(),
        }
    }

    /// Insert a vector and return its index.
    pub fn insert(&mut self, vector: &[f64]) -> usize {
        let id = self.embeddings.len();
        self.embeddings.push(vector.to_vec());

        let insert_layer = self.select_layer();

        // Ensure we have enough layers
        while self.layers.len() <= insert_layer {
            self.layers.push(Vec::new());
        }

        // Add empty adjacency lists for this node in all layers up to insert_layer
        for layer in 0..=insert_layer {
            while self.layers[layer].len() <= id {
                self.layers[layer].push(Vec::new());
            }
        }

        // Also ensure layer 0 has an entry for this node
        while self.layers[0].len() <= id {
            self.layers[0].push(Vec::new());
        }

        if id == 0 {
            // First node, just set as entry point
            self.entry_point = 0;
            self.max_layer = insert_layer;
            return id;
        }

        // Greedy search from top layer down to insert_layer+1
        let mut current_entry = self.entry_point;
        for layer in (insert_layer + 1..=self.max_layer).rev() {
            if layer < self.layers.len() {
                let neighbors = self.search_layer(vector, current_entry, 1, layer);
                if let Some((nearest, _)) = neighbors.first() {
                    current_entry = *nearest;
                }
            }
        }

        // Insert into layers from insert_layer down to 0
        for layer in (0..=insert_layer.min(self.max_layer)).rev() {
            let neighbors =
                self.search_layer(vector, current_entry, self.ef_construction, layer);

            // Select up to m neighbors
            let selected: Vec<(usize, f64)> =
                neighbors.into_iter().take(self.m).collect();

            // Ensure adjacency list exists for this node at this layer
            while self.layers[layer].len() <= id {
                self.layers[layer].push(Vec::new());
            }

            // Add bidirectional connections
            for &(neighbor_id, dist) in &selected {
                self.layers[layer][id].push((neighbor_id, dist));

                while self.layers[layer].len() <= neighbor_id {
                    self.layers[layer].push(Vec::new());
                }
                self.layers[layer][neighbor_id].push((id, dist));

                // Prune if over capacity
                if self.layers[layer][neighbor_id].len() > self.m * 2 {
                    self.layers[layer][neighbor_id]
                        .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                    self.layers[layer][neighbor_id].truncate(self.m * 2);
                }
            }

            if let Some((nearest, _)) = selected.first() {
                current_entry = *nearest;
            }
        }

        if insert_layer > self.max_layer {
            self.max_layer = insert_layer;
            self.entry_point = id;
        }

        id
    }

    /// Search for the k nearest neighbors of `query`.
    ///
    /// - `k`: number of nearest neighbors to return
    /// - `ef`: search width (larger = more accurate, slower; typical: 50-200)
    ///
    /// Returns (index, distance) pairs sorted by ascending distance.
    pub fn search(&self, query: &[f64], k: usize, ef: usize) -> Vec<(usize, f64)> {
        if self.embeddings.is_empty() {
            return Vec::new();
        }

        // Bounds-check the entry point
        if self.entry_point >= self.embeddings.len() {
            return Vec::new();
        }

        let mut current_entry = self.entry_point;

        // Greedy search from top layer down to layer 1
        for layer in (1..=self.max_layer).rev() {
            if layer < self.layers.len() {
                let neighbors = self.search_layer(query, current_entry, 1, layer);
                if let Some((nearest, _)) = neighbors.first() {
                    current_entry = *nearest;
                }
            }
        }

        // Search layer 0 with ef candidates
        let mut results = self.search_layer(query, current_entry, ef.max(k), 0);
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Number of vectors in the index.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Returns true if the index has no vectors.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Euclidean distance between two vectors.
    fn distance(a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f64>()
            .sqrt()
    }

    /// Select a random layer for insertion using an exponential distribution.
    fn select_layer(&self) -> usize {
        // Deterministic level assignment based on node count for reproducibility.
        // Uses a simple hash-like scheme: most nodes go to layer 0.
        let n = self.embeddings.len();
        let ml = 1.0 / (self.m as f64).ln();
        // Use a simple deterministic pseudo-random based on n
        let hash = ((n.wrapping_mul(2654435761)) >> 16) as f64 / 65536.0;
        let level = (-hash.ln() * ml).floor() as usize;
        level.min(4) // Cap at 4 layers
    }

    /// Search a single layer starting from `entry`, returning `ef` nearest candidates.
    fn search_layer(
        &self,
        query: &[f64],
        entry: usize,
        ef: usize,
        layer: usize,
    ) -> Vec<(usize, f64)> {
        if layer >= self.layers.len() {
            return Vec::new();
        }

        // Bounds-check entry against embeddings
        if entry >= self.embeddings.len() {
            return Vec::new();
        }

        let mut visited = HashSet::new();
        let entry_dist = Self::distance(query, &self.embeddings[entry]);

        // Candidates: min-heap (closest first)
        let mut candidates = BinaryHeap::new();
        candidates.push(ScoredNode {
            id: entry,
            distance: entry_dist,
        });

        // Results: max-heap (furthest first, for pruning)
        let mut results = BinaryHeap::new();
        results.push(FurthestNode {
            id: entry,
            distance: entry_dist,
        });

        visited.insert(entry);

        while let Some(ScoredNode { id: current, distance: current_dist }) = candidates.pop() {
            // If current candidate is further than the worst result and we have enough, stop
            if let Some(worst) = results.peek() {
                if current_dist > worst.distance && results.len() >= ef {
                    break;
                }
            }

            // Explore neighbors
            if current < self.layers[layer].len() {
                for &(neighbor, _) in &self.layers[layer][current] {
                    if neighbor < self.embeddings.len() && visited.insert(neighbor) {
                        let dist = Self::distance(query, &self.embeddings[neighbor]);

                        let should_add = results.len() < ef
                            || results
                                .peek()
                                .map(|w| dist < w.distance)
                                .unwrap_or(true);

                        if should_add {
                            candidates.push(ScoredNode {
                                id: neighbor,
                                distance: dist,
                            });
                            results.push(FurthestNode {
                                id: neighbor,
                                distance: dist,
                            });

                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        // Collect results sorted by distance
        let mut result_vec: Vec<(usize, f64)> =
            results.into_iter().map(|n| (n.id, n.distance)).collect();
        result_vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        result_vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_search_basic() {
        let mut index = HnswIndex::new(4, 20);
        index.insert(&[0.0, 0.0]);
        index.insert(&[1.0, 0.0]);
        index.insert(&[0.0, 1.0]);
        index.insert(&[10.0, 10.0]);

        let results = index.search(&[0.1, 0.1], 2, 10);
        assert_eq!(results.len(), 2);
        // Closest should be [0,0]
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn empty_index_returns_empty() {
        let index = HnswIndex::new(4, 20);
        let results = index.search(&[1.0, 2.0], 5, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn single_element() {
        let mut index = HnswIndex::new(4, 20);
        index.insert(&[5.0, 5.0]);

        let results = index.search(&[0.0, 0.0], 1, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn hnsw_recall_vs_brute_force() {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        let dim = 8;
        let n = 200;
        let k = 10;

        let mut index = HnswIndex::new(16, 100);
        let mut vectors: Vec<Vec<f64>> = Vec::new();

        for _ in 0..n {
            let v: Vec<f64> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            index.insert(&v);
            vectors.push(v);
        }

        // Run multiple queries and check average recall
        let num_queries = 20;
        let mut total_recall = 0.0;

        for _ in 0..num_queries {
            let query: Vec<f64> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

            // Brute force ground truth
            let mut bf_distances: Vec<(usize, f64)> = vectors
                .iter()
                .enumerate()
                .map(|(i, v)| (i, HnswIndex::distance(&query, v)))
                .collect();
            bf_distances
                .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let bf_top_k: Vec<usize> = bf_distances.iter().take(k).map(|(i, _)| *i).collect();

            // HNSW search
            let hnsw_results = index.search(&query, k, 50);
            let hnsw_top_k: Vec<usize> = hnsw_results.iter().map(|(i, _)| *i).collect();

            // Compute recall
            let hits = hnsw_top_k
                .iter()
                .filter(|id| bf_top_k.contains(id))
                .count();
            total_recall += hits as f64 / k as f64;
        }

        let avg_recall = total_recall / num_queries as f64;
        assert!(
            avg_recall > 0.9,
            "HNSW recall {} should be > 0.9",
            avg_recall
        );
    }

    #[test]
    fn distance_is_euclidean() {
        let d = HnswIndex::distance(&[0.0, 0.0], &[3.0, 4.0]);
        assert!((d - 5.0).abs() < 1e-10);
    }
}
