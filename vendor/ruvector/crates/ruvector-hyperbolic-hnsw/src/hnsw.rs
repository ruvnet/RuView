//! HNSW Adapter with Hyperbolic Distance Support
//!
//! This module provides HNSW (Hierarchical Navigable Small World) graph
//! implementation optimized for hyperbolic space using the Poincaré ball model.
//!
//! # Key Features
//!
//! - Hyperbolic distance metric for neighbor selection
//! - Tangent space pruning for accelerated search
//! - Configurable curvature per index
//! - Dual-space search (Euclidean fallback)

use crate::error::{HyperbolicError, HyperbolicResult};
use crate::poincare::{fused_norms, norm_squared, poincare_distance, poincare_distance_from_norms, project_to_ball, EPS};
use crate::tangent::TangentCache;
use serde::{Deserialize, Serialize};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Distance metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Poincaré ball hyperbolic distance
    Poincare,
    /// Standard Euclidean distance
    Euclidean,
    /// Cosine similarity (converted to distance)
    Cosine,
    /// Hybrid: Euclidean for pruning, Poincaré for ranking
    Hybrid,
}

/// HNSW configuration for hyperbolic space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicHnswConfig {
    /// Maximum number of connections per node (M parameter)
    pub max_connections: usize,
    /// Maximum connections for layer 0 (M0 = 2*M typically)
    pub max_connections_0: usize,
    /// Size of dynamic candidate list during construction (ef_construction)
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search (ef)
    pub ef_search: usize,
    /// Level multiplier for layer selection (ml = 1/ln(M))
    pub level_mult: f32,
    /// Curvature parameter for Poincaré ball
    pub curvature: f32,
    /// Distance metric
    pub metric: DistanceMetric,
    /// Pruning factor for tangent space optimization
    pub prune_factor: usize,
    /// Whether to use tangent space pruning
    pub use_tangent_pruning: bool,
}

impl Default for HyperbolicHnswConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            max_connections_0: 32,
            ef_construction: 200,
            ef_search: 50,
            level_mult: 1.0 / (16.0_f32).ln(),
            curvature: 1.0,
            metric: DistanceMetric::Poincare,
            prune_factor: 10,
            use_tangent_pruning: true,
        }
    }
}

/// A node in the HNSW graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswNode {
    /// Node ID
    pub id: usize,
    /// Vector in Poincaré ball
    pub vector: Vec<f32>,
    /// Connections at each level (level -> neighbor ids)
    pub connections: Vec<Vec<usize>>,
    /// Maximum level this node appears in
    pub level: usize,
}

impl HnswNode {
    pub fn new(id: usize, vector: Vec<f32>, max_level: usize) -> Self {
        let connections = (0..=max_level).map(|_| Vec::new()).collect();
        Self {
            id,
            vector,
            connections,
            level: max_level,
        }
    }
}

/// Search result with distance
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: usize,
    pub distance: f32,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.distance.partial_cmp(&other.distance)
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance.partial_cmp(&other.distance).unwrap()
    }
}

/// Hyperbolic HNSW Index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicHnsw {
    /// Configuration
    pub config: HyperbolicHnswConfig,
    /// All nodes in the graph
    nodes: Vec<HnswNode>,
    /// Entry point node ID
    entry_point: Option<usize>,
    /// Maximum level in the graph
    max_level: usize,
    /// Tangent cache for pruning (not serialized)
    #[serde(skip)]
    tangent_cache: Option<TangentCache>,
}

impl HyperbolicHnsw {
    /// Create a new empty HNSW index
    pub fn new(config: HyperbolicHnswConfig) -> Self {
        Self {
            config,
            nodes: Vec::new(),
            entry_point: None,
            max_level: 0,
            tangent_cache: None,
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(HyperbolicHnswConfig::default())
    }

    /// Get the number of nodes in the index
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get the dimension of vectors
    pub fn dim(&self) -> Option<usize> {
        self.nodes.first().map(|n| n.vector.len())
    }

    /// Compute distance between two vectors (optimized with fused norms)
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.config.metric {
            DistanceMetric::Poincare | DistanceMetric::Hybrid => {
                // Use fused_norms for single-pass computation
                let (diff_sq, norm_a_sq, norm_b_sq) = fused_norms(a, b);
                poincare_distance_from_norms(diff_sq, norm_a_sq, norm_b_sq, self.config.curvature)
            }
            DistanceMetric::Euclidean => {
                let (diff_sq, _, _) = fused_norms(a, b);
                diff_sq.sqrt()
            }
            DistanceMetric::Cosine => {
                let len = a.len().min(b.len());
                let mut dot_ab = 0.0f32;
                let mut norm_a_sq = 0.0f32;
                let mut norm_b_sq = 0.0f32;

                // Fused computation
                for i in 0..len {
                    let ai = a[i];
                    let bi = b[i];
                    dot_ab += ai * bi;
                    norm_a_sq += ai * ai;
                    norm_b_sq += bi * bi;
                }

                let norm_prod = (norm_a_sq * norm_b_sq).sqrt();
                1.0 - dot_ab / (norm_prod + EPS)
            }
        }
    }

    /// Compute distance with pre-computed query norm (for batch search)
    #[inline]
    fn distance_with_query_norm(&self, query: &[f32], query_norm_sq: f32, point: &[f32]) -> f32 {
        match self.config.metric {
            DistanceMetric::Poincare | DistanceMetric::Hybrid => {
                let (diff_sq, _, point_norm_sq) = fused_norms(query, point);
                poincare_distance_from_norms(diff_sq, query_norm_sq, point_norm_sq, self.config.curvature)
            }
            _ => self.distance(query, point)
        }
    }

    /// Generate random level for a new node
    fn random_level(&self) -> usize {
        let r: f32 = rand::random();
        (-r.ln() * self.config.level_mult) as usize
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, vector: Vec<f32>) -> HyperbolicResult<usize> {
        // Project to ball for safety
        let vector = project_to_ball(&vector, self.config.curvature, EPS);

        let id = self.nodes.len();
        let level = self.random_level();

        // Create new node
        let node = HnswNode::new(id, vector.clone(), level);
        self.nodes.push(node);

        if self.entry_point.is_none() {
            self.entry_point = Some(id);
            self.max_level = level;
            return Ok(id);
        }

        let entry_id = self.entry_point.unwrap();

        // Search for entry point at top levels
        let mut current = entry_id;
        for l in (level + 1..=self.max_level).rev() {
            current = self.search_layer_single(&vector, current, l)?;
        }

        // Insert at levels [0, min(level, max_level)]
        let insert_level = level.min(self.max_level);
        for l in (0..=insert_level).rev() {
            let neighbors = self.search_layer(&vector, current, self.config.ef_construction, l)?;

            // Select best neighbors
            let max_conn = if l == 0 {
                self.config.max_connections_0
            } else {
                self.config.max_connections
            };

            let selected: Vec<usize> = neighbors.iter().take(max_conn).map(|r| r.id).collect();

            // Add bidirectional connections
            self.nodes[id].connections[l] = selected.clone();

            for &neighbor_id in &selected {
                self.nodes[neighbor_id].connections[l].push(id);

                // Prune if too many connections
                if self.nodes[neighbor_id].connections[l].len() > max_conn {
                    self.prune_connections(neighbor_id, l, max_conn)?;
                }
            }

            if !neighbors.is_empty() {
                current = neighbors[0].id;
            }
        }

        // Update entry point if new node has higher level
        if level > self.max_level {
            self.entry_point = Some(id);
            self.max_level = level;
        }

        // Invalidate tangent cache
        self.tangent_cache = None;

        Ok(id)
    }

    /// Insert batch of vectors
    pub fn insert_batch(&mut self, vectors: Vec<Vec<f32>>) -> HyperbolicResult<Vec<usize>> {
        let mut ids = Vec::with_capacity(vectors.len());
        for vector in vectors {
            ids.push(self.insert(vector)?);
        }
        Ok(ids)
    }

    /// Search for single nearest neighbor at a layer (greedy)
    fn search_layer_single(&self, query: &[f32], entry: usize, level: usize) -> HyperbolicResult<usize> {
        let mut current = entry;
        let mut current_dist = self.distance(query, &self.nodes[current].vector);

        loop {
            let mut changed = false;

            for &neighbor in &self.nodes[current].connections[level] {
                let dist = self.distance(query, &self.nodes[neighbor].vector);
                if dist < current_dist {
                    current_dist = dist;
                    current = neighbor;
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        Ok(current)
    }

    /// Search layer with ef candidates
    fn search_layer(
        &self,
        query: &[f32],
        entry: usize,
        ef: usize,
        level: usize,
    ) -> HyperbolicResult<Vec<SearchResult>> {
        use std::collections::{BinaryHeap, HashSet};

        let entry_dist = self.distance(query, &self.nodes[entry].vector);

        let mut visited = HashSet::new();
        visited.insert(entry);

        // Candidates (min-heap by distance)
        let mut candidates: BinaryHeap<std::cmp::Reverse<SearchResult>> = BinaryHeap::new();
        candidates.push(std::cmp::Reverse(SearchResult {
            id: entry,
            distance: entry_dist,
        }));

        // Results (max-heap by distance for easy pruning)
        let mut results: BinaryHeap<SearchResult> = BinaryHeap::new();
        results.push(SearchResult {
            id: entry,
            distance: entry_dist,
        });

        while let Some(std::cmp::Reverse(current)) = candidates.pop() {
            // Check if we can stop early
            if let Some(furthest) = results.peek() {
                if current.distance > furthest.distance && results.len() >= ef {
                    break;
                }
            }

            // Explore neighbors
            for &neighbor in &self.nodes[current.id].connections[level] {
                if visited.contains(&neighbor) {
                    continue;
                }
                visited.insert(neighbor);

                let dist = self.distance(query, &self.nodes[neighbor].vector);

                let should_add = results.len() < ef
                    || results
                        .peek()
                        .map(|r| dist < r.distance)
                        .unwrap_or(true);

                if should_add {
                    candidates.push(std::cmp::Reverse(SearchResult {
                        id: neighbor,
                        distance: dist,
                    }));
                    results.push(SearchResult {
                        id: neighbor,
                        distance: dist,
                    });

                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }

        let mut result_vec: Vec<SearchResult> = results.into_iter().collect();
        result_vec.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());

        Ok(result_vec)
    }

    /// Prune connections to keep only the best
    fn prune_connections(
        &mut self,
        node_id: usize,
        level: usize,
        max_conn: usize,
    ) -> HyperbolicResult<()> {
        let node_vector = self.nodes[node_id].vector.clone();
        let connections = &self.nodes[node_id].connections[level];

        let mut scored: Vec<(usize, f32)> = connections
            .iter()
            .map(|&id| (id, self.distance(&node_vector, &self.nodes[id].vector)))
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        self.nodes[node_id].connections[level] =
            scored.into_iter().take(max_conn).map(|(id, _)| id).collect();

        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> HyperbolicResult<Vec<SearchResult>> {
        if self.is_empty() {
            return Ok(Vec::new());
        }

        let query = project_to_ball(query, self.config.curvature, EPS);
        let entry = self.entry_point.unwrap();

        // Navigate to lowest level from top
        let mut current = entry;
        for l in (1..=self.max_level).rev() {
            current = self.search_layer_single(&query, current, l)?;
        }

        // Search at layer 0 with ef_search candidates
        let ef = self.config.ef_search.max(k);
        let mut results = self.search_layer(&query, current, ef, 0)?;

        results.truncate(k);
        Ok(results)
    }

    /// Search with tangent space pruning (optimized for hyperbolic)
    pub fn search_with_pruning(&self, query: &[f32], k: usize) -> HyperbolicResult<Vec<SearchResult>> {
        // Fall back to regular search if no tangent cache
        if self.tangent_cache.is_none() || !self.config.use_tangent_pruning {
            return self.search(query, k);
        }

        let cache = self.tangent_cache.as_ref().unwrap();
        let query = project_to_ball(query, self.config.curvature, EPS);

        // Phase 1: Fast tangent space filtering
        let query_tangent = cache.query_tangent(&query);

        let mut candidates: Vec<(usize, f32)> = (0..cache.len())
            .map(|i| {
                let tangent_dist = cache.tangent_distance_squared(&query_tangent, i);
                (cache.point_indices[i], tangent_dist)
            })
            .collect();

        // Sort by tangent distance
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Keep top prune_factor * k candidates
        let num_candidates = (k * self.config.prune_factor).min(candidates.len());
        candidates.truncate(num_candidates);

        // Phase 2: Exact Poincaré distance for finalists
        let mut results: Vec<SearchResult> = candidates
            .into_iter()
            .map(|(id, _)| {
                let dist = self.distance(&query, &self.nodes[id].vector);
                SearchResult { id, distance: dist }
            })
            .collect();

        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        results.truncate(k);

        Ok(results)
    }

    /// Build tangent cache for all points
    pub fn build_tangent_cache(&mut self) -> HyperbolicResult<()> {
        if self.is_empty() {
            return Ok(());
        }

        let vectors: Vec<Vec<f32>> = self.nodes.iter().map(|n| n.vector.clone()).collect();
        let indices: Vec<usize> = (0..self.nodes.len()).collect();

        self.tangent_cache = Some(TangentCache::new(&vectors, &indices, self.config.curvature)?);

        Ok(())
    }

    /// Get a reference to a node's vector
    pub fn get_vector(&self, id: usize) -> Option<&[f32]> {
        self.nodes.get(id).map(|n| n.vector.as_slice())
    }

    /// Update curvature and rebuild tangent cache
    pub fn set_curvature(&mut self, curvature: f32) -> HyperbolicResult<()> {
        if curvature <= 0.0 {
            return Err(HyperbolicError::InvalidCurvature(curvature));
        }

        self.config.curvature = curvature;

        // Reproject all vectors
        for node in &mut self.nodes {
            node.vector = project_to_ball(&node.vector, curvature, EPS);
        }

        // Rebuild tangent cache
        if self.tangent_cache.is_some() {
            self.build_tangent_cache()?;
        }

        Ok(())
    }

    /// Get all vectors as a slice
    pub fn vectors(&self) -> Vec<&[f32]> {
        self.nodes.iter().map(|n| n.vector.as_slice()).collect()
    }
}

/// Dual-space index for fallback and mutual ranking fusion
#[derive(Debug)]
pub struct DualSpaceIndex {
    /// Hyperbolic index (primary)
    pub hyperbolic: HyperbolicHnsw,
    /// Euclidean index (fallback)
    pub euclidean: HyperbolicHnsw,
    /// Fusion weight for hyperbolic results (0-1)
    pub fusion_weight: f32,
}

impl DualSpaceIndex {
    /// Create a new dual-space index
    pub fn new(curvature: f32, fusion_weight: f32) -> Self {
        let mut hyp_config = HyperbolicHnswConfig::default();
        hyp_config.curvature = curvature;
        hyp_config.metric = DistanceMetric::Poincare;

        let mut euc_config = HyperbolicHnswConfig::default();
        euc_config.metric = DistanceMetric::Euclidean;

        Self {
            hyperbolic: HyperbolicHnsw::new(hyp_config),
            euclidean: HyperbolicHnsw::new(euc_config),
            fusion_weight: fusion_weight.clamp(0.0, 1.0),
        }
    }

    /// Insert into both indices
    pub fn insert(&mut self, vector: Vec<f32>) -> HyperbolicResult<usize> {
        self.euclidean.insert(vector.clone())?;
        self.hyperbolic.insert(vector)
    }

    /// Search with mutual ranking fusion
    pub fn search(&self, query: &[f32], k: usize) -> HyperbolicResult<Vec<SearchResult>> {
        let hyp_results = self.hyperbolic.search(query, k * 2)?;
        let euc_results = self.euclidean.search(query, k * 2)?;

        // Combine and re-rank using fusion
        use std::collections::HashMap;

        let mut scores: HashMap<usize, f32> = HashMap::new();

        // Add hyperbolic scores
        for (rank, r) in hyp_results.iter().enumerate() {
            let score = self.fusion_weight * (1.0 / (rank as f32 + 1.0));
            *scores.entry(r.id).or_insert(0.0) += score;
        }

        // Add Euclidean scores
        for (rank, r) in euc_results.iter().enumerate() {
            let score = (1.0 - self.fusion_weight) * (1.0 / (rank as f32 + 1.0));
            *scores.entry(r.id).or_insert(0.0) += score;
        }

        // Sort by combined score (higher is better)
        let mut combined: Vec<(usize, f32)> = scores.into_iter().collect();
        combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Return top k with hyperbolic distances
        Ok(combined
            .into_iter()
            .take(k)
            .map(|(id, _)| {
                let dist = self.hyperbolic.distance(
                    query,
                    self.hyperbolic.get_vector(id).unwrap_or(&[]),
                );
                SearchResult { id, distance: dist }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_insert_search() {
        let mut hnsw = HyperbolicHnsw::default_config();

        // Insert some vectors
        for i in 0..10 {
            let v = vec![0.1 * i as f32, 0.05 * i as f32];
            hnsw.insert(v).unwrap();
        }

        assert_eq!(hnsw.len(), 10);

        // Search
        let query = vec![0.3, 0.15];
        let results = hnsw.search(&query, 3).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].distance <= results[1].distance);
    }

    #[test]
    fn test_dual_space() {
        let mut dual = DualSpaceIndex::new(1.0, 0.5);

        for i in 0..10 {
            let v = vec![0.1 * i as f32, 0.05 * i as f32];
            dual.insert(v).unwrap();
        }

        let query = vec![0.3, 0.15];
        let results = dual.search(&query, 3).unwrap();

        assert_eq!(results.len(), 3);
    }
}
