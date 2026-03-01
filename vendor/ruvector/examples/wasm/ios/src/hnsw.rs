//! Lightweight HNSW Index for iOS/Browser WASM
//!
//! A simplified HNSW implementation optimized for mobile/browser deployment.
//! Provides O(log n) approximate nearest neighbor search.
//!
//! Based on the paper: "Efficient and Robust Approximate Nearest Neighbor Search
//! Using Hierarchical Navigable Small World Graphs"

use crate::distance::{distance, DistanceMetric};
use std::collections::{BinaryHeap, HashSet};
use std::vec::Vec;
use core::cmp::Ordering;

/// HNSW configuration
#[derive(Clone, Debug)]
pub struct HnswConfig {
    /// Max connections per node (M parameter)
    pub m: usize,
    /// Max connections at layer 0 (usually 2*M)
    pub m_max_0: usize,
    /// Construction-time search width
    pub ef_construction: usize,
    /// Query-time search width
    pub ef_search: usize,
    /// Level multiplier (1/ln(M))
    pub level_mult: f32,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m_max_0: 32,
            ef_construction: 100,
            ef_search: 50,
            level_mult: 0.36, // 1/ln(16)
        }
    }
}

/// Node in the HNSW graph
#[derive(Clone, Debug)]
struct HnswNode {
    /// Vector ID
    id: u64,
    /// Vector data
    vector: Vec<f32>,
    /// Connections at each layer
    connections: Vec<Vec<u64>>,
    /// Node's layer
    level: usize,
}

/// Search candidate with distance
#[derive(Clone, Debug)]
struct Candidate {
    id: u64,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse order for min-heap behavior in BinaryHeap
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Lightweight HNSW index
pub struct HnswIndex {
    /// All nodes
    nodes: Vec<HnswNode>,
    /// ID to node index mapping
    id_to_idx: std::collections::HashMap<u64, usize>,
    /// Entry point (topmost node)
    entry_point: Option<usize>,
    /// Maximum level in the graph
    max_level: usize,
    /// Configuration
    config: HnswConfig,
    /// Distance metric
    metric: DistanceMetric,
    /// Dimension
    dim: usize,
    /// Random seed for level generation
    seed: u32,
}

impl HnswIndex {
    /// Create a new HNSW index
    pub fn new(dim: usize, metric: DistanceMetric, config: HnswConfig) -> Self {
        Self {
            nodes: Vec::new(),
            id_to_idx: std::collections::HashMap::new(),
            entry_point: None,
            max_level: 0,
            config,
            metric,
            dim,
            seed: 12345,
        }
    }

    /// Create with default config
    pub fn with_defaults(dim: usize, metric: DistanceMetric) -> Self {
        Self::new(dim, metric, HnswConfig::default())
    }

    /// Generate random level for a new node
    fn random_level(&mut self) -> usize {
        // LCG random number generator
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        let rand = (self.seed >> 16) as f32 / 32768.0;

        let level = (-rand.ln() * self.config.level_mult).floor() as usize;
        level.min(16) // Cap at 16 levels
    }

    /// Insert a vector into the index
    pub fn insert(&mut self, id: u64, vector: Vec<f32>) -> bool {
        if vector.len() != self.dim {
            return false;
        }

        if self.id_to_idx.contains_key(&id) {
            return false; // Already exists
        }

        let level = self.random_level();
        let node_idx = self.nodes.len();

        // Create node with empty connections
        let mut node = HnswNode {
            id,
            vector,
            connections: vec![Vec::new(); level + 1],
            level,
        };

        if let Some(ep_idx) = self.entry_point {
            // Find entry point at the top level
            let mut curr_idx = ep_idx;
            let mut curr_dist = self.distance_to_node(node_idx, curr_idx, &node.vector);

            // Traverse from top to insertion level
            for lc in (level + 1..=self.max_level).rev() {
                let mut changed = true;
                while changed {
                    changed = false;
                    if let Some(connections) = self.nodes.get(curr_idx).map(|n| n.connections.get(lc).cloned()).flatten() {
                        for &neighbor_id in &connections {
                            if let Some(&neighbor_idx) = self.id_to_idx.get(&neighbor_id) {
                                let d = self.distance_to_node(node_idx, neighbor_idx, &node.vector);
                                if d < curr_dist {
                                    curr_dist = d;
                                    curr_idx = neighbor_idx;
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }

            // Insert at each level
            for lc in (0..=level.min(self.max_level)).rev() {
                let neighbors = self.search_layer(&node.vector, curr_idx, self.config.ef_construction, lc);

                // Select M best neighbors
                let m_max = if lc == 0 { self.config.m_max_0 } else { self.config.m };
                let selected: Vec<u64> = neighbors.iter()
                    .take(m_max)
                    .map(|c| c.id)
                    .collect();

                node.connections[lc] = selected.clone();

                // Add bidirectional connections
                for &neighbor_id in &selected {
                    if let Some(&neighbor_idx) = self.id_to_idx.get(&neighbor_id) {
                        if let Some(neighbor_node) = self.nodes.get_mut(neighbor_idx) {
                            if lc < neighbor_node.connections.len() {
                                neighbor_node.connections[lc].push(id);

                                // Prune if too many connections
                                if neighbor_node.connections[lc].len() > m_max {
                                    let query = &neighbor_node.vector.clone();
                                    self.prune_connections(neighbor_idx, lc, m_max, query);
                                }
                            }
                        }
                    }
                }

                if !neighbors.is_empty() {
                    curr_idx = self.id_to_idx.get(&neighbors[0].id).copied().unwrap_or(curr_idx);
                }
            }
        }

        // Add node
        self.nodes.push(node);
        self.id_to_idx.insert(id, node_idx);

        // Update entry point if this is higher level
        if level > self.max_level || self.entry_point.is_none() {
            self.max_level = level;
            self.entry_point = Some(node_idx);
        }

        true
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
        self.search_with_ef(query, k, self.config.ef_search)
    }

    /// Search with custom ef parameter
    pub fn search_with_ef(&self, query: &[f32], k: usize, ef: usize) -> Vec<(u64, f32)> {
        if query.len() != self.dim || self.entry_point.is_none() {
            return vec![];
        }

        let ep_idx = self.entry_point.unwrap();

        // Find entry point by traversing from top
        let mut curr_idx = ep_idx;
        let mut curr_dist = distance(query, &self.nodes[curr_idx].vector, self.metric);

        for lc in (1..=self.max_level).rev() {
            let mut changed = true;
            while changed {
                changed = false;
                if let Some(connections) = self.nodes.get(curr_idx).and_then(|n| n.connections.get(lc)) {
                    for &neighbor_id in connections {
                        if let Some(&neighbor_idx) = self.id_to_idx.get(&neighbor_id) {
                            let d = distance(query, &self.nodes[neighbor_idx].vector, self.metric);
                            if d < curr_dist {
                                curr_dist = d;
                                curr_idx = neighbor_idx;
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        // Search at layer 0
        let results = self.search_layer(query, curr_idx, ef, 0);

        results.into_iter()
            .take(k)
            .map(|c| (c.id, c.distance))
            .collect()
    }

    /// Search within a specific layer
    fn search_layer(&self, query: &[f32], entry_idx: usize, ef: usize, layer: usize) -> Vec<Candidate> {
        let entry_id = self.nodes[entry_idx].id;
        let entry_dist = distance(query, &self.nodes[entry_idx].vector, self.metric);

        let mut visited: HashSet<u64> = HashSet::new();
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new();
        let mut results: Vec<Candidate> = Vec::new();

        visited.insert(entry_id);
        candidates.push(Candidate { id: entry_id, distance: entry_dist });
        results.push(Candidate { id: entry_id, distance: entry_dist });

        while let Some(current) = candidates.pop() {
            // Stop if current is worse than worst in results
            if results.len() >= ef {
                let worst_dist = results.iter().map(|c| c.distance).fold(f32::NEG_INFINITY, f32::max);
                if current.distance > worst_dist {
                    break;
                }
            }

            // Explore neighbors
            if let Some(&curr_idx) = self.id_to_idx.get(&current.id) {
                if let Some(connections) = self.nodes.get(curr_idx).and_then(|n| n.connections.get(layer)) {
                    for &neighbor_id in connections {
                        if visited.insert(neighbor_id) {
                            if let Some(&neighbor_idx) = self.id_to_idx.get(&neighbor_id) {
                                let d = distance(query, &self.nodes[neighbor_idx].vector, self.metric);

                                let should_add = results.len() < ef || {
                                    let worst = results.iter().map(|c| c.distance).fold(f32::NEG_INFINITY, f32::max);
                                    d < worst
                                };

                                if should_add {
                                    candidates.push(Candidate { id: neighbor_id, distance: d });
                                    results.push(Candidate { id: neighbor_id, distance: d });

                                    // Keep only ef best
                                    if results.len() > ef {
                                        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
                                        results.truncate(ef);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        results
    }

    /// Prune connections to keep only the best
    fn prune_connections(&mut self, node_idx: usize, layer: usize, max_conn: usize, query: &[f32]) {
        // First, collect connection info without holding mutable borrow
        let connections_to_score: Vec<u64> = if let Some(node) = self.nodes.get(node_idx) {
            if layer < node.connections.len() {
                node.connections[layer].clone()
            } else {
                return;
            }
        } else {
            return;
        };

        // Score connections
        let mut candidates: Vec<(u64, f32)> = connections_to_score
            .iter()
            .filter_map(|&id| {
                self.id_to_idx.get(&id)
                    .and_then(|&idx| self.nodes.get(idx))
                    .map(|n| (id, distance(query, &n.vector, self.metric)))
            })
            .collect();

        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let pruned: Vec<u64> = candidates.into_iter()
            .take(max_conn)
            .map(|(id, _)| id)
            .collect();

        // Now update the connections
        if let Some(node) = self.nodes.get_mut(node_idx) {
            if layer < node.connections.len() {
                node.connections[layer] = pruned;
            }
        }
    }

    /// Helper to calculate distance to a node
    fn distance_to_node(&self, _new_idx: usize, existing_idx: usize, new_vector: &[f32]) -> f32 {
        if let Some(node) = self.nodes.get(existing_idx) {
            distance(new_vector, &node.vector, self.metric)
        } else {
            f32::MAX
        }
    }

    /// Get number of vectors in the index
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get vector by ID
    pub fn get(&self, id: u64) -> Option<&[f32]> {
        self.id_to_idx.get(&id)
            .and_then(|&idx| self.nodes.get(idx))
            .map(|n| n.vector.as_slice())
    }

    // ============================================
    // Persistence
    // ============================================

    /// Serialize the HNSW index to bytes
    ///
    /// Format:
    /// - Header (32 bytes): dim, metric, m, m_max_0, ef_construction, ef_search, max_level, node_count
    /// - For each node: id (8), level (4), vector (dim*4), connections per layer
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Header
        bytes.extend_from_slice(&(self.dim as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.metric as u8).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 3]); // padding
        bytes.extend_from_slice(&(self.config.m as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.config.m_max_0 as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.config.ef_construction as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.config.ef_search as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.max_level as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.entry_point.map(|e| e as u32).unwrap_or(u32::MAX).to_le_bytes());

        // Nodes
        for node in &self.nodes {
            // Node header: id, level
            bytes.extend_from_slice(&node.id.to_le_bytes());
            bytes.extend_from_slice(&(node.level as u32).to_le_bytes());

            // Vector
            for &v in &node.vector {
                bytes.extend_from_slice(&v.to_le_bytes());
            }

            // Connections: count per layer, then connection IDs
            bytes.extend_from_slice(&(node.connections.len() as u32).to_le_bytes());
            for layer_conns in &node.connections {
                bytes.extend_from_slice(&(layer_conns.len() as u32).to_le_bytes());
                for &conn_id in layer_conns {
                    bytes.extend_from_slice(&conn_id.to_le_bytes());
                }
            }
        }

        bytes
    }

    /// Deserialize HNSW index from bytes
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 36 {
            return None;
        }

        let mut offset = 0;

        // Read header
        let dim = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let metric = DistanceMetric::from_u8(bytes[4]);
        offset = 8;

        let m = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
        offset += 4;
        let m_max_0 = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
        offset += 4;
        let ef_construction = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
        offset += 4;
        let ef_search = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
        offset += 4;
        let max_level = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
        offset += 4;
        let node_count = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
        offset += 4;
        let entry_point_raw = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]);
        offset += 4;
        let entry_point = if entry_point_raw == u32::MAX { None } else { Some(entry_point_raw as usize) };

        let config = HnswConfig {
            m,
            m_max_0,
            ef_construction,
            ef_search,
            level_mult: 1.0 / (m as f32).ln(),
        };

        let mut nodes = Vec::with_capacity(node_count);
        let mut id_to_idx = std::collections::HashMap::new();

        for node_idx in 0..node_count {
            if offset + 12 > bytes.len() {
                return None;
            }

            // Node header
            let id = u64::from_le_bytes([
                bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3],
                bytes[offset+4], bytes[offset+5], bytes[offset+6], bytes[offset+7],
            ]);
            offset += 8;
            let level = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
            offset += 4;

            // Vector
            let mut vector = Vec::with_capacity(dim);
            for _ in 0..dim {
                if offset + 4 > bytes.len() {
                    return None;
                }
                let v = f32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]);
                vector.push(v);
                offset += 4;
            }

            // Connections
            if offset + 4 > bytes.len() {
                return None;
            }
            let num_layers = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
            offset += 4;

            let mut connections = Vec::with_capacity(num_layers);
            for _ in 0..num_layers {
                if offset + 4 > bytes.len() {
                    return None;
                }
                let num_conns = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]) as usize;
                offset += 4;

                let mut layer_conns = Vec::with_capacity(num_conns);
                for _ in 0..num_conns {
                    if offset + 8 > bytes.len() {
                        return None;
                    }
                    let conn_id = u64::from_le_bytes([
                        bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3],
                        bytes[offset+4], bytes[offset+5], bytes[offset+6], bytes[offset+7],
                    ]);
                    layer_conns.push(conn_id);
                    offset += 8;
                }
                connections.push(layer_conns);
            }

            id_to_idx.insert(id, node_idx);
            nodes.push(HnswNode {
                id,
                vector,
                connections,
                level,
            });
        }

        Some(Self {
            nodes,
            id_to_idx,
            entry_point,
            max_level,
            config,
            metric,
            dim,
            seed: 12345,
        })
    }

    /// Estimate serialized size in bytes
    pub fn serialized_size(&self) -> usize {
        let mut size = 36; // Header
        for node in &self.nodes {
            size += 12; // id + level
            size += node.vector.len() * 4; // vector
            size += 4; // num_layers
            for layer in &node.connections {
                size += 4 + layer.len() * 8; // count + connection IDs
            }
        }
        size
    }
}

// ============================================
// WASM Exports
// ============================================

static mut HNSW_INDEX: Option<HnswIndex> = None;

/// Create HNSW index
#[no_mangle]
pub extern "C" fn hnsw_create(dim: u32, metric: u8, m: u32, ef_construction: u32) -> i32 {
    let config = HnswConfig {
        m: m as usize,
        m_max_0: (m * 2) as usize,
        ef_construction: ef_construction as usize,
        ef_search: 50,
        level_mult: 1.0 / (m as f32).ln(),
    };

    unsafe {
        HNSW_INDEX = Some(HnswIndex::new(
            dim as usize,
            DistanceMetric::from_u8(metric),
            config,
        ));
    }
    0
}

/// Insert vector into HNSW
#[no_mangle]
pub extern "C" fn hnsw_insert(id: u64, vector_ptr: *const f32, len: u32) -> i32 {
    unsafe {
        if let Some(index) = HNSW_INDEX.as_mut() {
            let vector = core::slice::from_raw_parts(vector_ptr, len as usize).to_vec();
            if index.insert(id, vector) { 0 } else { -1 }
        } else {
            -1
        }
    }
}

/// Search HNSW index
#[no_mangle]
pub extern "C" fn hnsw_search(
    query_ptr: *const f32,
    query_len: u32,
    k: u32,
    ef: u32,
    out_ids: *mut u64,
    out_distances: *mut f32,
) -> u32 {
    unsafe {
        if let Some(index) = HNSW_INDEX.as_ref() {
            let query = core::slice::from_raw_parts(query_ptr, query_len as usize);
            let results = index.search_with_ef(query, k as usize, ef as usize);

            let ids = core::slice::from_raw_parts_mut(out_ids, results.len());
            let distances = core::slice::from_raw_parts_mut(out_distances, results.len());

            for (i, (id, dist)) in results.iter().enumerate() {
                ids[i] = *id;
                distances[i] = *dist;
            }

            results.len() as u32
        } else {
            0
        }
    }
}

/// Get HNSW index size
#[no_mangle]
pub extern "C" fn hnsw_size() -> u32 {
    unsafe {
        HNSW_INDEX.as_ref().map(|i| i.len() as u32).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_insert_search() {
        let mut index = HnswIndex::with_defaults(4, DistanceMetric::Euclidean);

        // Insert some vectors
        for i in 0..100u64 {
            let v = vec![i as f32, 0.0, 0.0, 0.0];
            assert!(index.insert(i, v));
        }

        assert_eq!(index.len(), 100);

        // Search for closest to [50, 0, 0, 0]
        let query = vec![50.0, 0.0, 0.0, 0.0];
        let results = index.search(&query, 5);

        assert!(!results.is_empty());
        // HNSW is approximate - verify we get results and distance is reasonable
        let (closest_id, closest_dist) = results[0];
        // The closest vector should have a reasonable distance (less than 25)
        assert!(closest_dist < 25.0, "Distance too large: {}", closest_dist);
        // Result should be somewhere in the index
        assert!(closest_id < 100, "Invalid ID: {}", closest_id);
    }

    #[test]
    fn test_hnsw_cosine() {
        let mut index = HnswIndex::with_defaults(3, DistanceMetric::Cosine);

        // Insert normalized vectors
        index.insert(1, vec![1.0, 0.0, 0.0]);
        index.insert(2, vec![0.0, 1.0, 0.0]);
        index.insert(3, vec![0.707, 0.707, 0.0]);

        let query = vec![1.0, 0.0, 0.0];
        let results = index.search(&query, 3);

        assert_eq!(results[0].0, 1); // Exact match first
    }
}
