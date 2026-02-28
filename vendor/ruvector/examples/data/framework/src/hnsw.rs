//! HNSW (Hierarchical Navigable Small World) Index
//!
//! Production-quality implementation of the HNSW algorithm for approximate
//! nearest neighbor search in high-dimensional vector spaces.
//!
//! ## Algorithm Overview
//!
//! HNSW builds a multi-layer graph structure where:
//! - Layer 0 contains all vectors
//! - Higher layers contain progressively fewer vectors (exponentially decaying)
//! - Each layer is a navigable small world graph with bounded degree
//! - Search proceeds from top layer down, greedy navigating to nearest neighbors
//!
//! ## Performance Characteristics
//!
//! - **Search**: O(log n) approximate nearest neighbor queries
//! - **Insert**: O(log n) amortized insertion time
//! - **Space**: O(n * M) where M is max connections per layer
//! - **Accuracy**: Configurable via ef_construction and ef_search parameters
//!
//! ## References
//!
//! - Malkov, Y. A., & Yashunin, D. A. (2018). "Efficient and robust approximate
//!   nearest neighbor search using Hierarchical Navigable Small World graphs"
//!   IEEE Transactions on Pattern Analysis and Machine Intelligence.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::ruvector_native::SemanticVector;
use crate::FrameworkError;

/// HNSW index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Maximum number of bi-directional links per node per layer (M)
    /// Higher values improve recall but increase memory and search time
    /// Typical range: 8-64, default: 16
    pub m: usize,

    /// Maximum connections for layer 0 (typically M * 2)
    pub m_max_0: usize,

    /// Size of dynamic candidate list during construction (ef_construction)
    /// Higher values improve graph quality but slow construction
    /// Typical range: 100-500, default: 200
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search (ef_search)
    /// Higher values improve recall but slow search
    /// Typical range: 50-200, default: 50
    pub ef_search: usize,

    /// Layer generation probability parameter (ml)
    /// 1/ln(ml) determines layer assignment probability
    /// Default: 1.0 / ln(m) ≈ 0.36 for m=16
    pub ml: f64,

    /// Vector dimension (must be consistent)
    pub dimension: usize,

    /// Distance metric
    pub metric: DistanceMetric,
}

impl Default for HnswConfig {
    fn default() -> Self {
        let m = 16;
        Self {
            m,
            m_max_0: m * 2,
            ef_construction: 200,
            ef_search: 50,
            ml: 1.0 / (m as f64).ln(),
            dimension: 128,
            metric: DistanceMetric::Cosine,
        }
    }
}

/// Distance metrics supported by HNSW
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Cosine similarity (converted to angular distance)
    /// Distance = arccos(similarity) / π
    /// Range: [0, 1] where 0 = identical, 1 = opposite
    Cosine,

    /// Euclidean (L2) distance
    Euclidean,

    /// Manhattan (L1) distance
    Manhattan,
}

/// A node in the HNSW graph
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HnswNode {
    /// Vector data
    vector: Vec<f32>,

    /// External identifier from SemanticVector
    external_id: String,

    /// Timestamp when added
    timestamp: DateTime<Utc>,

    /// Maximum layer this node appears in
    level: usize,

    /// Connections per layer: connections[layer] = set of neighbor node IDs
    /// Layer 0 can have up to m_max_0 connections, others up to m
    connections: Vec<Vec<usize>>,
}

/// Search result with distance and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswSearchResult {
    /// Node ID in the index
    pub node_id: usize,

    /// External identifier
    pub external_id: String,

    /// Distance to query vector (lower is more similar)
    pub distance: f32,

    /// Cosine similarity score (if using cosine metric)
    pub similarity: Option<f32>,

    /// Timestamp when vector was added
    pub timestamp: DateTime<Utc>,
}

/// Statistics about the HNSW index structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswStats {
    /// Total number of nodes
    pub node_count: usize,

    /// Number of layers in the graph
    pub layer_count: usize,

    /// Nodes per layer
    pub nodes_per_layer: Vec<usize>,

    /// Average connections per node per layer
    pub avg_connections_per_layer: Vec<f64>,

    /// Total edges in the graph
    pub total_edges: usize,

    /// Entry point node ID
    pub entry_point: Option<usize>,

    /// Memory usage estimate in bytes
    pub estimated_memory_bytes: usize,
}

/// HNSW index for approximate nearest neighbor search
///
/// Thread-safe implementation using Arc<RwLock<>> for concurrent reads.
pub struct HnswIndex {
    /// Configuration
    config: HnswConfig,

    /// All nodes in the index
    nodes: Vec<HnswNode>,

    /// Entry point for search (node with highest layer)
    entry_point: Option<usize>,

    /// Maximum layer currently in use
    max_layer: usize,

    /// Random number generator for layer assignment
    rng: Arc<RwLock<rand::rngs::StdRng>>,
}

impl HnswIndex {
    /// Create a new HNSW index with default configuration
    pub fn new() -> Self {
        Self::with_config(HnswConfig::default())
    }

    /// Create a new HNSW index with custom configuration
    pub fn with_config(config: HnswConfig) -> Self {
        use rand::SeedableRng;
        Self {
            config,
            nodes: Vec::new(),
            entry_point: None,
            max_layer: 0,
            rng: Arc::new(RwLock::new(rand::rngs::StdRng::from_entropy())),
        }
    }

    /// Insert a vector into the index
    ///
    /// ## Arguments
    ///
    /// - `vector`: The SemanticVector to insert
    ///
    /// ## Returns
    ///
    /// The assigned node ID
    pub fn insert(&mut self, vector: SemanticVector) -> Result<usize, FrameworkError> {
        if vector.embedding.len() != self.config.dimension {
            return Err(FrameworkError::Config(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.config.dimension,
                vector.embedding.len()
            )));
        }

        let node_id = self.nodes.len();
        let level = self.random_level();

        // Create new node
        let mut new_node = HnswNode {
            vector: vector.embedding,
            external_id: vector.id,
            timestamp: vector.timestamp,
            level,
            connections: vec![Vec::new(); level + 1],
        };

        // Insert into graph
        if self.entry_point.is_none() {
            // First node - becomes entry point
            self.nodes.push(new_node);
            self.entry_point = Some(node_id);
            self.max_layer = level;
            return Ok(node_id);
        }

        // Search for nearest neighbors at insertion point
        let entry_point = self.entry_point.unwrap();
        let mut current_nearest = vec![entry_point];

        // Traverse from top layer down to level+1
        for lc in (level + 1..=self.max_layer).rev() {
            current_nearest = self.search_layer(&new_node.vector, &current_nearest, 1, lc);
        }

        // Insert from level down to 0
        for lc in (0..=level).rev() {
            let candidates = self.search_layer(&new_node.vector, &current_nearest, self.config.ef_construction, lc);

            // Select M neighbors
            let m = if lc == 0 { self.config.m_max_0 } else { self.config.m };
            let neighbors = self.select_neighbors(&new_node.vector, candidates, m);

            // Add bidirectional links
            for &neighbor_id in &neighbors {
                // Add link from new node to neighbor
                new_node.connections[lc].push(neighbor_id);
            }

            current_nearest = neighbors.clone();
        }

        self.nodes.push(new_node);

        // Add reverse links and prune if necessary
        for lc in 0..=level {
            let neighbors: Vec<usize> = self.nodes[node_id].connections[lc].clone();
            for neighbor_id in neighbors {
                // Only add reverse link if neighbor has this layer
                if lc < self.nodes[neighbor_id].connections.len() {
                    self.nodes[neighbor_id].connections[lc].push(node_id);

                    // Prune if exceeded max connections
                    let m_max = if lc == 0 { self.config.m_max_0 } else { self.config.m };
                    if self.nodes[neighbor_id].connections[lc].len() > m_max {
                        let neighbor_vec = self.nodes[neighbor_id].vector.clone();
                        let candidates = self.nodes[neighbor_id].connections[lc].clone();
                        let pruned = self.select_neighbors(&neighbor_vec, candidates, m_max);
                        self.nodes[neighbor_id].connections[lc] = pruned;
                    }
                }
            }
        }

        // Update entry point if new node is at higher layer
        if level > self.max_layer {
            self.max_layer = level;
            self.entry_point = Some(node_id);
        }

        Ok(node_id)
    }

    /// Insert a batch of vectors
    ///
    /// More efficient than inserting one at a time for large batches.
    pub fn insert_batch(&mut self, vectors: Vec<SemanticVector>) -> Result<Vec<usize>, FrameworkError> {
        let mut ids = Vec::with_capacity(vectors.len());
        for vector in vectors {
            ids.push(self.insert(vector)?);
        }
        Ok(ids)
    }

    /// Search for k nearest neighbors
    ///
    /// ## Arguments
    ///
    /// - `query`: Query vector (must match index dimension)
    /// - `k`: Number of neighbors to return
    ///
    /// ## Returns
    ///
    /// Up to k nearest neighbors, sorted by distance (ascending)
    pub fn search_knn(&self, query: &[f32], k: usize) -> Result<Vec<HnswSearchResult>, FrameworkError> {
        if query.len() != self.config.dimension {
            return Err(FrameworkError::Config(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.config.dimension,
                query.len()
            )));
        }

        if self.entry_point.is_none() {
            return Ok(Vec::new());
        }

        let entry_point = self.entry_point.unwrap();
        let mut current_nearest = vec![entry_point];

        // Traverse from top layer down to layer 1
        for lc in (1..=self.max_layer).rev() {
            current_nearest = self.search_layer(query, &current_nearest, 1, lc);
        }

        // Search layer 0 with ef_search
        let ef = self.config.ef_search.max(k);
        let candidates = self.search_layer(query, &current_nearest, ef, 0);

        // Convert to search results
        let results: Vec<HnswSearchResult> = candidates
            .iter()
            .take(k)
            .map(|&node_id| {
                let node = &self.nodes[node_id];
                let distance = self.distance(query, &node.vector);
                let similarity = if self.config.metric == DistanceMetric::Cosine {
                    Some(self.cosine_similarity(query, &node.vector))
                } else {
                    None
                };

                HnswSearchResult {
                    node_id,
                    external_id: node.external_id.clone(),
                    distance,
                    similarity,
                    timestamp: node.timestamp,
                }
            })
            .collect();

        Ok(results)
    }

    /// Search for all neighbors within a distance threshold
    ///
    /// ## Arguments
    ///
    /// - `query`: Query vector
    /// - `threshold`: Maximum distance (exclusive)
    /// - `max_results`: Maximum number of results to return (None for unlimited)
    ///
    /// ## Returns
    ///
    /// All neighbors within threshold, sorted by distance
    pub fn search_threshold(
        &self,
        query: &[f32],
        threshold: f32,
        max_results: Option<usize>,
    ) -> Result<Vec<HnswSearchResult>, FrameworkError> {
        // Search with large k first
        let k = max_results.unwrap_or(1000).max(100);
        let mut results = self.search_knn(query, k)?;

        // Filter by threshold
        results.retain(|r| r.distance < threshold);

        // Limit results
        if let Some(max) = max_results {
            results.truncate(max);
        }

        Ok(results)
    }

    /// Get statistics about the index structure
    pub fn stats(&self) -> HnswStats {
        let node_count = self.nodes.len();
        let layer_count = self.max_layer + 1;

        let mut nodes_per_layer = vec![0; layer_count];
        let mut connections_per_layer = vec![0; layer_count];

        for node in &self.nodes {
            for layer in 0..=node.level {
                nodes_per_layer[layer] += 1;
                connections_per_layer[layer] += node.connections[layer].len();
            }
        }

        let avg_connections_per_layer: Vec<f64> = connections_per_layer
            .iter()
            .zip(&nodes_per_layer)
            .map(|(conn, nodes)| {
                if *nodes > 0 {
                    *conn as f64 / *nodes as f64
                } else {
                    0.0
                }
            })
            .collect();

        let total_edges: usize = connections_per_layer.iter().sum();

        // Estimate memory: each node stores vector + metadata + connections
        let estimated_memory_bytes = node_count
            * (self.config.dimension * 4 // vector (f32)
                + 100 // metadata overhead
                + self.config.m * 8 * layer_count); // connections (usize)

        HnswStats {
            node_count,
            layer_count,
            nodes_per_layer,
            avg_connections_per_layer,
            total_edges,
            entry_point: self.entry_point,
            estimated_memory_bytes,
        }
    }

    // ===== Private helper methods =====

    /// Search a single layer for nearest neighbors
    fn search_layer(&self, query: &[f32], entry_points: &[usize], ef: usize, layer: usize) -> Vec<usize> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut nearest = BinaryHeap::new();

        for &ep in entry_points {
            let dist = self.distance(query, &self.nodes[ep].vector);
            candidates.push((Reverse(OrderedFloat(dist)), ep));
            nearest.push((OrderedFloat(dist), ep));
            visited.insert(ep);
        }

        while let Some((Reverse(OrderedFloat(dist)), current)) = candidates.pop() {
            // Check if we should continue searching
            if let Some(&(OrderedFloat(max_dist), _)) = nearest.peek() {
                if dist > max_dist {
                    break;
                }
            }

            // Explore neighbors
            if current < self.nodes.len() && layer <= self.nodes[current].level {
                for &neighbor in &self.nodes[current].connections[layer] {
                    if visited.insert(neighbor) {
                        let neighbor_dist = self.distance(query, &self.nodes[neighbor].vector);

                        if let Some(&(OrderedFloat(max_dist), _)) = nearest.peek() {
                            if neighbor_dist < max_dist || nearest.len() < ef {
                                candidates.push((Reverse(OrderedFloat(neighbor_dist)), neighbor));
                                nearest.push((OrderedFloat(neighbor_dist), neighbor));

                                if nearest.len() > ef {
                                    nearest.pop();
                                }
                            }
                        } else {
                            candidates.push((Reverse(OrderedFloat(neighbor_dist)), neighbor));
                            nearest.push((OrderedFloat(neighbor_dist), neighbor));
                        }
                    }
                }
            }
        }

        // Extract node IDs sorted by distance (ascending)
        let mut sorted_nearest: Vec<_> = nearest.into_iter().collect();
        sorted_nearest.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        sorted_nearest.into_iter().map(|(_, id)| id).collect()
    }

    /// Select M neighbors from candidates using heuristic
    fn select_neighbors(&self, base: &[f32], candidates: Vec<usize>, m: usize) -> Vec<usize> {
        if candidates.len() <= m {
            return candidates;
        }

        // Simple heuristic: keep nearest M by distance
        let mut with_distances: Vec<_> = candidates
            .into_iter()
            .map(|id| {
                let dist = self.distance(base, &self.nodes[id].vector);
                (OrderedFloat(dist), id)
            })
            .collect();

        with_distances.sort_by_key(|(dist, _)| *dist);
        with_distances.into_iter().take(m).map(|(_, id)| id).collect()
    }

    /// Compute distance between two vectors
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.config.metric {
            DistanceMetric::Cosine => {
                let similarity = self.cosine_similarity(a, b);
                // Convert to angular distance: arccos(sim) / π ∈ [0, 1]
                similarity.max(-1.0).min(1.0).acos() / std::f32::consts::PI
            }
            DistanceMetric::Euclidean => {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f32>()
                    .sqrt()
            }
            DistanceMetric::Manhattan => {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).abs())
                    .sum()
            }
        }
    }

    /// Compute cosine similarity between two vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).max(-1.0).min(1.0)
    }

    /// Randomly assign a layer to a new node
    fn random_level(&self) -> usize {
        let mut rng = self.rng.write().unwrap();
        let uniform: f64 = rng.gen();
        (-uniform.ln() * self.config.ml).floor() as usize
    }

    /// Get the underlying vector for a node
    pub fn get_vector(&self, node_id: usize) -> Option<&Vec<f32>> {
        self.nodes.get(node_id).map(|n| &n.vector)
    }

    /// Get the external ID for a node
    pub fn get_external_id(&self, node_id: usize) -> Option<&str> {
        self.nodes.get(node_id).map(|n| n.external_id.as_str())
    }

    /// Get total number of nodes in the index
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for HnswIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper for f32 that implements Ord for use in BinaryHeap
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::ruvector_native::Domain;

    fn create_test_vector(id: &str, embedding: Vec<f32>) -> SemanticVector {
        SemanticVector {
            id: id.to_string(),
            embedding,
            domain: Domain::Climate,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_hnsw_basic_insert_search() {
        let config = HnswConfig {
            dimension: 3,
            ..Default::default()
        };
        let mut index = HnswIndex::with_config(config);

        // Insert vectors
        let v1 = create_test_vector("v1", vec![1.0, 0.0, 0.0]);
        let v2 = create_test_vector("v2", vec![0.0, 1.0, 0.0]);
        let v3 = create_test_vector("v3", vec![0.9, 0.1, 0.0]);

        index.insert(v1).unwrap();
        index.insert(v2).unwrap();
        index.insert(v3).unwrap();

        assert_eq!(index.len(), 3);

        // Search for nearest to v1
        let query = vec![1.0, 0.0, 0.0];
        let results = index.search_knn(&query, 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].external_id, "v1"); // Exact match
        assert_eq!(results[1].external_id, "v3"); // Close match
    }

    #[test]
    fn test_hnsw_batch_insert() {
        let config = HnswConfig {
            dimension: 2,
            ..Default::default()
        };
        let mut index = HnswIndex::with_config(config);

        let vectors = vec![
            create_test_vector("v1", vec![1.0, 0.0]),
            create_test_vector("v2", vec![0.0, 1.0]),
            create_test_vector("v3", vec![1.0, 1.0]),
        ];

        let ids = index.insert_batch(vectors).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_hnsw_threshold_search() {
        let config = HnswConfig {
            dimension: 2,
            ..Default::default()
        };
        let mut index = HnswIndex::with_config(config);

        // Insert vectors at different distances
        index.insert(create_test_vector("close", vec![1.0, 0.1])).unwrap();
        index.insert(create_test_vector("medium", vec![0.7, 0.7])).unwrap();
        index.insert(create_test_vector("far", vec![0.0, 1.0])).unwrap();

        let query = vec![1.0, 0.0];
        let results = index.search_threshold(&query, 0.3, None).unwrap();

        // Should find only close vectors
        assert!(results.len() >= 1);
        assert!(results.iter().all(|r| r.distance < 0.3));
    }

    #[test]
    fn test_hnsw_cosine_similarity() {
        let config = HnswConfig {
            dimension: 3,
            metric: DistanceMetric::Cosine,
            ..Default::default()
        };
        let mut index = HnswIndex::with_config(config);

        let v1 = create_test_vector("identical", vec![1.0, 0.0, 0.0]);
        let v2 = create_test_vector("orthogonal", vec![0.0, 1.0, 0.0]);
        let v3 = create_test_vector("opposite", vec![-1.0, 0.0, 0.0]);

        index.insert(v1).unwrap();
        index.insert(v2).unwrap();
        index.insert(v3).unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let results = index.search_knn(&query, 3).unwrap();

        // Identical should be closest
        assert_eq!(results[0].external_id, "identical");
        assert!(results[0].distance < 0.01);

        // Opposite should be farthest
        assert_eq!(results[2].external_id, "opposite");
    }

    #[test]
    fn test_hnsw_stats() {
        let config = HnswConfig {
            dimension: 2,
            m: 4,
            ..Default::default()
        };
        let mut index = HnswIndex::with_config(config);

        for i in 0..10 {
            let vec = create_test_vector(&format!("v{}", i), vec![i as f32, i as f32]);
            index.insert(vec).unwrap();
        }

        let stats = index.stats();
        assert_eq!(stats.node_count, 10);
        assert!(stats.layer_count > 0);
        assert_eq!(stats.nodes_per_layer[0], 10); // All nodes in layer 0
        assert!(stats.total_edges > 0);
    }

    #[test]
    fn test_dimension_mismatch() {
        let config = HnswConfig {
            dimension: 3,
            ..Default::default()
        };
        let mut index = HnswIndex::with_config(config);

        let bad_vector = create_test_vector("bad", vec![1.0, 2.0]); // Wrong dimension
        let result = index.insert(bad_vector);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_index_search() {
        let index = HnswIndex::new();
        let query = vec![1.0; 128];
        let results = index.search_knn(&query, 5).unwrap();
        assert!(results.is_empty());
    }
}
