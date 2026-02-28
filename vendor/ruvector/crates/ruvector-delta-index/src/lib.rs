//! # RuVector Delta Index
//!
//! Delta-aware HNSW index with incremental updates and repair strategies.
//! Optimized for scenarios with frequent small changes to vector embeddings.
//!
//! ## Key Features
//!
//! - Incremental index updates without full rebuild
//! - Repair strategies for maintaining graph quality
//! - Recall quality monitoring
//! - Delta-based versioning
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvector_delta_index::{DeltaHnsw, DeltaHnswConfig, RepairStrategy};
//! use ruvector_delta_core::VectorDelta;
//!
//! let config = DeltaHnswConfig::default();
//! let mut index = DeltaHnsw::new(384, config);
//!
//! // Insert vectors
//! index.insert("vec1", vec![1.0; 384]);
//!
//! // Apply delta update
//! let delta = VectorDelta::from_dense(vec![0.1; 384]);
//! index.apply_delta("vec1", &delta);
//!
//! // Search (uses repaired graph)
//! let results = index.search(&query, 10);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod incremental;
pub mod quality;
pub mod repair;

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;
use priority_queue::PriorityQueue;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use smallvec::SmallVec;

use ruvector_delta_core::{Delta, DeltaStream, VectorDelta};

pub use error::{IndexError, Result};
pub use incremental::IncrementalUpdater;
pub use quality::{QualityMetrics, QualityMonitor, RecallEstimate};
pub use repair::{GraphRepairer, RepairConfig, RepairStrategy};

/// Configuration for Delta HNSW index
#[derive(Debug, Clone)]
pub struct DeltaHnswConfig {
    /// Number of connections per node
    pub m: usize,
    /// Maximum connections per node at layer 0
    pub m0: usize,
    /// Construction ef (neighbor search budget)
    pub ef_construction: usize,
    /// Search ef (query-time search budget)
    pub ef_search: usize,
    /// Maximum elements
    pub max_elements: usize,
    /// Level multiplier for layer assignment
    pub level_mult: f64,
    /// Delta threshold for triggering repair
    pub repair_threshold: f32,
    /// Maximum deltas before compaction
    pub max_deltas: usize,
    /// Enable automatic quality monitoring
    pub auto_monitor: bool,
}

impl Default for DeltaHnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 200,
            ef_search: 100,
            max_elements: 1_000_000,
            level_mult: 1.0 / (16.0_f64).ln(),
            repair_threshold: 0.5,
            max_deltas: 100,
            auto_monitor: true,
        }
    }
}

/// A node in the HNSW graph
#[derive(Clone)]
struct HnswNode {
    /// Vector ID
    id: String,
    /// Vector data
    vector: Vec<f32>,
    /// Neighbors at each level (level -> neighbors)
    neighbors: Vec<SmallVec<[u32; 32]>>,
    /// Maximum level for this node
    level: usize,
    /// Delta stream for this node
    delta_stream: DeltaStream<VectorDelta>,
}

impl HnswNode {
    fn new(id: String, vector: Vec<f32>, level: usize) -> Self {
        Self {
            id,
            vector: vector.clone(),
            neighbors: vec![SmallVec::new(); level + 1],
            level,
            delta_stream: DeltaStream::for_vectors(vector.len()),
        }
    }
}

/// Entry point for the HNSW graph
#[derive(Clone)]
struct EntryPoint {
    node_idx: u32,
    level: usize,
}

/// Delta-aware HNSW index
pub struct DeltaHnsw {
    /// Configuration
    config: DeltaHnswConfig,
    /// Vector dimensions
    dimensions: usize,
    /// All nodes
    nodes: Vec<RwLock<HnswNode>>,
    /// ID to node index mapping
    id_to_idx: DashMap<String, u32>,
    /// Entry point
    entry_point: RwLock<Option<EntryPoint>>,
    /// Random number generator for level assignment
    rng: RwLock<XorShiftRng>,
    /// Quality monitor
    quality_monitor: Option<QualityMonitor>,
    /// Graph repairer
    repairer: GraphRepairer,
}

impl DeltaHnsw {
    /// Create a new Delta HNSW index
    pub fn new(dimensions: usize, config: DeltaHnswConfig) -> Self {
        let quality_monitor = if config.auto_monitor {
            Some(QualityMonitor::new(dimensions))
        } else {
            None
        };

        let repair_config = RepairConfig {
            strategy: RepairStrategy::Lazy,
            batch_size: 100,
            quality_threshold: 0.95,
        };

        Self {
            config: config.clone(),
            dimensions,
            nodes: Vec::with_capacity(config.max_elements),
            id_to_idx: DashMap::new(),
            entry_point: RwLock::new(None),
            rng: RwLock::new(XorShiftRng::seed_from_u64(42)),
            quality_monitor,
            repairer: GraphRepairer::new(repair_config),
        }
    }

    /// Get configuration
    pub fn config(&self) -> &DeltaHnswConfig {
        &self.config
    }

    /// Get dimensions
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Get number of elements
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Insert a new vector
    pub fn insert(&mut self, id: &str, vector: Vec<f32>) -> Result<()> {
        if vector.len() != self.dimensions {
            return Err(IndexError::DimensionMismatch {
                expected: self.dimensions,
                actual: vector.len(),
            });
        }

        if self.id_to_idx.contains_key(id) {
            return Err(IndexError::DuplicateId(id.to_string()));
        }

        // Assign level
        let level = self.random_level();
        let node_idx = self.nodes.len() as u32;

        // Create node
        let node = HnswNode::new(id.to_string(), vector.clone(), level);
        self.nodes.push(RwLock::new(node));
        self.id_to_idx.insert(id.to_string(), node_idx);

        // Connect to graph
        self.connect_node(node_idx, &vector, level)?;

        // Update entry point if needed
        let mut entry = self.entry_point.write();
        if entry.is_none() || level > entry.as_ref().unwrap().level {
            *entry = Some(EntryPoint { node_idx, level });
        }

        Ok(())
    }

    /// Apply a delta update to a vector
    pub fn apply_delta(&mut self, id: &str, delta: &VectorDelta) -> Result<()> {
        let node_idx = *self
            .id_to_idx
            .get(id)
            .ok_or_else(|| IndexError::NotFound(id.to_string()))?;

        let mut node = self.nodes[node_idx as usize].write();

        // Apply delta to vector
        delta
            .apply(&mut node.vector)
            .map_err(|e| IndexError::DeltaError(format!("{:?}", e)))?;

        // Record in stream
        node.delta_stream.push(delta.clone());

        // Check if repair is needed
        let cumulative_change = self.estimate_cumulative_change(&node);
        if cumulative_change > self.config.repair_threshold {
            drop(node);
            self.repair_node(node_idx)?;
        }

        Ok(())
    }

    /// Batch apply deltas
    pub fn apply_deltas_batch(&mut self, updates: &[(String, VectorDelta)]) -> Result<Vec<u32>> {
        let mut repaired = Vec::new();

        for (id, delta) in updates {
            let node_idx = *self
                .id_to_idx
                .get(id)
                .ok_or_else(|| IndexError::NotFound(id.clone()))?;

            let mut node = self.nodes[node_idx as usize].write();
            delta
                .apply(&mut node.vector)
                .map_err(|e| IndexError::DeltaError(format!("{:?}", e)))?;
            node.delta_stream.push(delta.clone());

            let change = self.estimate_cumulative_change(&node);
            if change > self.config.repair_threshold {
                repaired.push(node_idx);
            }
        }

        // Batch repair
        for node_idx in &repaired {
            drop(self.nodes[*node_idx as usize].write());
            self.repair_node(*node_idx)?;
        }

        Ok(repaired)
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        if query.len() != self.dimensions {
            return Err(IndexError::DimensionMismatch {
                expected: self.dimensions,
                actual: query.len(),
            });
        }

        let entry = self.entry_point.read();
        if entry.is_none() {
            return Ok(Vec::new());
        }

        let entry = entry.as_ref().unwrap();
        let mut current_node = entry.node_idx;

        // Greedy search from top to layer 1
        for level in (1..=entry.level).rev() {
            current_node = self.greedy_search(query, current_node, level);
        }

        // Layer 0: ef_search neighbors
        let candidates = self.search_layer(query, current_node, 0, self.config.ef_search);

        // Take top-k
        let results: Vec<SearchResult> = candidates
            .into_iter()
            .take(k)
            .map(|(idx, dist)| {
                let node = self.nodes[idx as usize].read();
                SearchResult {
                    id: node.id.clone(),
                    distance: dist,
                    vector: Some(node.vector.clone()),
                }
            })
            .collect();

        // Update quality monitor
        if let Some(monitor) = &self.quality_monitor {
            monitor.record_search(query, &results);
        }

        Ok(results)
    }

    /// Get current quality metrics
    pub fn quality_metrics(&self) -> Option<QualityMetrics> {
        self.quality_monitor.as_ref().map(|m| m.metrics())
    }

    /// Force repair of entire graph
    pub fn force_repair(&mut self) -> Result<usize> {
        let node_count = self.nodes.len();
        let mut repaired = 0;

        for idx in 0..node_count {
            if self.repair_node(idx as u32)? {
                repaired += 1;
            }
        }

        Ok(repaired)
    }

    /// Delete a vector by ID
    pub fn delete(&mut self, id: &str) -> Result<bool> {
        let node_idx = match self.id_to_idx.remove(id) {
            Some((_, idx)) => idx,
            None => return Ok(false),
        };

        // Mark node as deleted (we don't physically remove to preserve indices)
        let mut node = self.nodes[node_idx as usize].write();
        node.id = String::new();
        node.vector.clear();
        node.neighbors.clear();

        // Remove from other nodes' neighbor lists
        for i in 0..self.nodes.len() {
            if i == node_idx as usize {
                continue;
            }

            let mut other = self.nodes[i].write();
            for level_neighbors in &mut other.neighbors {
                level_neighbors.retain(|n| *n != node_idx);
            }
        }

        Ok(true)
    }

    /// Compact delta streams for all nodes
    pub fn compact_deltas(&mut self) -> usize {
        let mut total_compacted = 0;

        for node in &self.nodes {
            let mut node = node.write();
            total_compacted += node.delta_stream.compact().unwrap_or(0);
        }

        total_compacted
    }

    // Private methods

    fn random_level(&self) -> usize {
        let mut rng = self.rng.write();
        let r: f64 = rand::Rng::gen(&mut *rng);
        (-r.ln() * self.config.level_mult).floor() as usize
    }

    fn connect_node(&mut self, node_idx: u32, vector: &[f32], level: usize) -> Result<()> {
        let entry = self.entry_point.read().clone();

        if entry.is_none() {
            return Ok(());
        }

        let entry = entry.unwrap();
        let mut current = entry.node_idx;

        // Navigate from top level
        for l in (level + 1..=entry.level).rev() {
            current = self.greedy_search(vector, current, l);
        }

        // Connect at each level
        for l in (0..=level.min(entry.level)).rev() {
            let neighbors = self.search_layer(vector, current, l, self.config.ef_construction);

            let max_conn = if l == 0 {
                self.config.m0
            } else {
                self.config.m
            };

            // Select best neighbors
            let selected: Vec<u32> = neighbors
                .into_iter()
                .take(max_conn)
                .map(|(idx, _)| idx)
                .collect();

            // Update node's neighbors
            {
                let mut node = self.nodes[node_idx as usize].write();
                if l < node.neighbors.len() {
                    node.neighbors[l] = selected.iter().cloned().collect();
                }
            }

            // Add reverse connections
            for &neighbor_idx in &selected {
                let mut neighbor = self.nodes[neighbor_idx as usize].write();
                if l < neighbor.neighbors.len() {
                    neighbor.neighbors[l].push(node_idx);

                    // Prune if over limit
                    if neighbor.neighbors[l].len() > max_conn {
                        let node_vec = self.nodes[neighbor_idx as usize].read().vector.clone();
                        self.prune_neighbors(&mut neighbor.neighbors[l], &node_vec, max_conn);
                    }
                }
            }

            if !selected.is_empty() {
                current = selected[0];
            }
        }

        Ok(())
    }

    fn greedy_search(&self, query: &[f32], start: u32, level: usize) -> u32 {
        let mut current = start;
        let mut current_dist = self.distance(query, current);

        loop {
            let node = self.nodes[current as usize].read();
            if level >= node.neighbors.len() {
                break;
            }

            let mut improved = false;

            for &neighbor in &node.neighbors[level] {
                let dist = self.distance(query, neighbor);
                if dist < current_dist {
                    current = neighbor;
                    current_dist = dist;
                    improved = true;
                }
            }

            if !improved {
                break;
            }
        }

        current
    }

    fn search_layer(&self, query: &[f32], start: u32, level: usize, ef: usize) -> Vec<(u32, f32)> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;
        use std::collections::HashSet;

        #[derive(Clone, Copy)]
        struct Candidate {
            idx: u32,
            dist: f32,
        }

        impl PartialEq for Candidate {
            fn eq(&self, other: &Self) -> bool {
                self.dist == other.dist
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
                // Min-heap by distance
                other
                    .dist
                    .partial_cmp(&self.dist)
                    .unwrap_or(Ordering::Equal)
            }
        }

        let start_dist = self.distance(query, start);
        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();
        let mut visited = HashSet::new();

        candidates.push(Candidate {
            idx: start,
            dist: start_dist,
        });
        results.push(Candidate {
            idx: start,
            dist: -start_dist, // Max-heap for worst result
        });
        visited.insert(start);

        while let Some(current) = candidates.pop() {
            // Check if we can stop
            if !results.is_empty() {
                let worst = results.peek().unwrap();
                if current.dist > -worst.dist {
                    break;
                }
            }

            let node = self.nodes[current.idx as usize].read();
            if level >= node.neighbors.len() {
                continue;
            }

            for &neighbor in &node.neighbors[level] {
                if visited.contains(&neighbor) {
                    continue;
                }
                visited.insert(neighbor);

                let dist = self.distance(query, neighbor);

                let should_add = results.len() < ef || dist < -results.peek().unwrap().dist;

                if should_add {
                    candidates.push(Candidate {
                        idx: neighbor,
                        dist,
                    });
                    results.push(Candidate {
                        idx: neighbor,
                        dist: -dist,
                    });

                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }

        results.into_iter().map(|c| (c.idx, -c.dist)).collect()
    }

    fn distance(&self, query: &[f32], node_idx: u32) -> f32 {
        let node = self.nodes[node_idx as usize].read();
        if node.vector.is_empty() {
            return f32::MAX;
        }

        // L2 distance squared
        query
            .iter()
            .zip(node.vector.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    fn prune_neighbors(&self, neighbors: &mut SmallVec<[u32; 32]>, node_vec: &[f32], max: usize) {
        if neighbors.len() <= max {
            return;
        }

        // Sort by distance and keep closest
        let mut with_dist: Vec<(u32, f32)> = neighbors
            .iter()
            .map(|&n| (n, self.distance(node_vec, n)))
            .collect();

        with_dist.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        neighbors.clear();
        for (idx, _) in with_dist.into_iter().take(max) {
            neighbors.push(idx);
        }
    }

    fn estimate_cumulative_change(&self, node: &HnswNode) -> f32 {
        // Estimate change based on delta stream
        let mut total_change = 0.0f32;

        for (_, delta) in node.delta_stream.iter() {
            total_change += delta.l2_norm();
        }

        total_change
    }

    fn repair_node(&mut self, node_idx: u32) -> Result<bool> {
        let node = self.nodes[node_idx as usize].read();
        if node.vector.is_empty() {
            return Ok(false);
        }

        let vector = node.vector.clone();
        let level = node.level;
        drop(node);

        // Reconnect based on current vector
        self.reconnect_node(node_idx, &vector, level)?;

        // Compact delta stream
        {
            let mut node = self.nodes[node_idx as usize].write();
            node.delta_stream.compact().ok();
        }

        Ok(true)
    }

    fn reconnect_node(&mut self, node_idx: u32, vector: &[f32], level: usize) -> Result<()> {
        // Find new neighbors at each level
        let entry = self.entry_point.read().clone();
        if entry.is_none() {
            return Ok(());
        }

        let entry = entry.unwrap();
        let mut current = entry.node_idx;

        for l in (level + 1..=entry.level).rev() {
            current = self.greedy_search(vector, current, l);
        }

        for l in (0..=level.min(entry.level)).rev() {
            let neighbors = self.search_layer(vector, current, l, self.config.ef_construction);

            let max_conn = if l == 0 {
                self.config.m0
            } else {
                self.config.m
            };

            // Filter out self
            let selected: Vec<u32> = neighbors
                .into_iter()
                .filter(|(idx, _)| *idx != node_idx)
                .take(max_conn)
                .map(|(idx, _)| idx)
                .collect();

            // Update neighbors
            {
                let mut node = self.nodes[node_idx as usize].write();
                if l < node.neighbors.len() {
                    node.neighbors[l] = selected.iter().cloned().collect();
                }
            }

            if !selected.is_empty() {
                current = selected[0];
            }
        }

        Ok(())
    }
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Vector ID
    pub id: String,
    /// Distance to query
    pub distance: f32,
    /// Optional vector data
    pub vector: Option<Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dim: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..dim).map(|_| rng.gen()).collect()
    }

    #[test]
    fn test_insert_and_search() {
        let mut index = DeltaHnsw::new(128, DeltaHnswConfig::default());

        // Insert some vectors
        for i in 0..100 {
            let vec = random_vector(128);
            index.insert(&format!("vec_{}", i), vec).unwrap();
        }

        assert_eq!(index.len(), 100);

        // Search
        let query = random_vector(128);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_delta_update() {
        let mut index = DeltaHnsw::new(4, DeltaHnswConfig::default());

        let original = vec![1.0, 2.0, 3.0, 4.0];
        index.insert("test", original.clone()).unwrap();

        let delta = VectorDelta::from_dense(vec![0.5, 0.0, -0.5, 0.0]);
        index.apply_delta("test", &delta).unwrap();

        // Search should still work
        let results = index.search(&[1.5, 2.0, 2.5, 4.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test");
    }

    #[test]
    fn test_delete() {
        let mut index = DeltaHnsw::new(4, DeltaHnswConfig::default());

        index.insert("a", vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        index.insert("b", vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        index.insert("c", vec![0.0, 0.0, 1.0, 0.0]).unwrap();

        assert!(index.delete("b").unwrap());
        assert!(!index.delete("nonexistent").unwrap());

        let results = index.search(&[0.0, 1.0, 0.0, 0.0], 10).unwrap();
        assert!(results.iter().all(|r| r.id != "b"));
    }
}
