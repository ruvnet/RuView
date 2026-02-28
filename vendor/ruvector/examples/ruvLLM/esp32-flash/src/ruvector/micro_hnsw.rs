//! Micro HNSW - Approximate Nearest Neighbor for ESP32

use heapless::Vec as HVec;
use heapless::BinaryHeap;
use heapless::binary_heap::Min;
use super::{MicroVector, DistanceMetric, euclidean_distance_i8, MAX_NEIGHBORS};

pub const INDEX_CAPACITY: usize = 256;
pub const MAX_LAYERS: usize = 4;
pub const DEFAULT_M: usize = 8;
pub const EF_SEARCH: usize = 16;

#[derive(Debug, Clone)]
pub struct HNSWConfig {
    pub m: usize,
    pub m_max0: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub metric: DistanceMetric,
    pub binary_mode: bool,
}

impl Default for HNSWConfig {
    fn default() -> Self {
        Self { m: 8, m_max0: 16, ef_construction: 32, ef_search: 16, metric: DistanceMetric::Euclidean, binary_mode: false }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    pub id: u32,
    pub distance: i32,
    pub index: usize,
}

impl PartialEq for SearchResult { fn eq(&self, other: &Self) -> bool { self.distance == other.distance } }
impl Eq for SearchResult {}
impl PartialOrd for SearchResult { fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> { Some(self.cmp(other)) } }
impl Ord for SearchResult { fn cmp(&self, other: &Self) -> core::cmp::Ordering { self.distance.cmp(&other.distance) } }

#[derive(Debug, Clone)]
struct HNSWNode<const DIM: usize> {
    vector: HVec<i8, DIM>,
    id: u32,
    neighbors: [HVec<u16, MAX_NEIGHBORS>; MAX_LAYERS],
    max_layer: u8,
}

impl<const DIM: usize> Default for HNSWNode<DIM> {
    fn default() -> Self {
        Self { vector: HVec::new(), id: 0, neighbors: Default::default(), max_layer: 0 }
    }
}

pub struct MicroHNSW<const DIM: usize, const CAPACITY: usize> {
    config: HNSWConfig,
    nodes: HVec<HNSWNode<DIM>, CAPACITY>,
    entry_point: Option<usize>,
    max_layer: u8,
    rng_state: u32,
}

impl<const DIM: usize, const CAPACITY: usize> MicroHNSW<DIM, CAPACITY> {
    pub fn new(config: HNSWConfig) -> Self {
        Self { config, nodes: HVec::new(), entry_point: None, max_layer: 0, rng_state: 12345 }
    }

    pub fn with_seed(mut self, seed: u32) -> Self { self.rng_state = seed; self }
    pub fn len(&self) -> usize { self.nodes.len() }
    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }
    pub fn memory_bytes(&self) -> usize { self.nodes.len() * (DIM + MAX_LAYERS * MAX_NEIGHBORS * 2 + 8) }

    pub fn insert(&mut self, vector: &MicroVector<DIM>) -> Result<usize, &'static str> {
        if self.nodes.len() >= CAPACITY { return Err("Index full"); }

        let new_idx = self.nodes.len();
        let new_layer = self.random_layer();

        let mut node = HNSWNode::<DIM>::default();
        node.vector = vector.data.clone();
        node.id = vector.id;
        node.max_layer = new_layer;

        if self.entry_point.is_none() {
            self.nodes.push(node).map_err(|_| "Push failed")?;
            self.entry_point = Some(new_idx);
            self.max_layer = new_layer;
            return Ok(new_idx);
        }

        let entry = self.entry_point.unwrap();
        self.nodes.push(node).map_err(|_| "Push failed")?;

        let mut current = entry;
        for layer in (new_layer as usize + 1..=self.max_layer as usize).rev() {
            current = self.greedy_search_layer(current, &vector.data, layer);
        }

        for layer in (0..=(new_layer as usize).min(self.max_layer as usize)).rev() {
            let neighbors = self.search_layer(current, &vector.data, layer, self.config.ef_construction);
            let max_n = if layer == 0 { self.config.m_max0 } else { self.config.m };
            let mut added = 0;

            for result in neighbors.iter().take(max_n) {
                if added >= MAX_NEIGHBORS { break; }
                if let Some(new_node) = self.nodes.get_mut(new_idx) {
                    let _ = new_node.neighbors[layer].push(result.index as u16);
                }
                if let Some(neighbor) = self.nodes.get_mut(result.index) {
                    if neighbor.neighbors[layer].len() < MAX_NEIGHBORS {
                        let _ = neighbor.neighbors[layer].push(new_idx as u16);
                    }
                }
                added += 1;
            }
            if !neighbors.is_empty() { current = neighbors[0].index; }
        }

        if new_layer > self.max_layer {
            self.entry_point = Some(new_idx);
            self.max_layer = new_layer;
        }
        Ok(new_idx)
    }

    pub fn search(&self, query: &[i8], k: usize) -> HVec<SearchResult, 32> {
        let mut results = HVec::new();
        if self.entry_point.is_none() || k == 0 { return results; }

        let entry = self.entry_point.unwrap();
        let mut current = entry;
        for layer in (1..=self.max_layer as usize).rev() {
            current = self.greedy_search_layer(current, query, layer);
        }

        let candidates = self.search_layer(current, query, 0, self.config.ef_search);
        for result in candidates.into_iter().take(k) {
            let _ = results.push(result);
        }
        results
    }

    fn search_layer(&self, entry: usize, query: &[i8], layer: usize, ef: usize) -> HVec<SearchResult, 64> {
        let mut visited = [false; CAPACITY];
        let mut candidates: BinaryHeap<SearchResult, Min, 64> = BinaryHeap::new();
        let mut results: HVec<SearchResult, 64> = HVec::new();

        visited[entry] = true;
        let entry_dist = self.distance(query, entry);
        let _ = candidates.push(SearchResult { id: self.nodes[entry].id, distance: entry_dist, index: entry });
        let _ = results.push(SearchResult { id: self.nodes[entry].id, distance: entry_dist, index: entry });

        while let Some(current) = candidates.pop() {
            if results.len() >= ef {
                if let Some(worst) = results.iter().max_by_key(|r| r.distance) {
                    if current.distance > worst.distance { break; }
                }
            }

            if let Some(node) = self.nodes.get(current.index) {
                if layer < node.neighbors.len() {
                    for &neighbor_idx in node.neighbors[layer].iter() {
                        let idx = neighbor_idx as usize;
                        if idx < CAPACITY && !visited[idx] {
                            visited[idx] = true;
                            let dist = self.distance(query, idx);
                            let should_add = results.len() < ef || results.iter().any(|r| dist < r.distance);

                            if should_add {
                                let r = SearchResult { id: self.nodes[idx].id, distance: dist, index: idx };
                                let _ = candidates.push(r);
                                let _ = results.push(r);
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

        results.sort_by_key(|r| r.distance);
        results
    }

    fn greedy_search_layer(&self, entry: usize, query: &[i8], layer: usize) -> usize {
        let mut current = entry;
        let mut current_dist = self.distance(query, current);

        loop {
            let mut improved = false;
            if let Some(node) = self.nodes.get(current) {
                if layer < node.neighbors.len() {
                    for &neighbor_idx in node.neighbors[layer].iter() {
                        let idx = neighbor_idx as usize;
                        if idx < self.nodes.len() {
                            let dist = self.distance(query, idx);
                            if dist < current_dist {
                                current = idx;
                                current_dist = dist;
                                improved = true;
                            }
                        }
                    }
                }
            }
            if !improved { break; }
        }
        current
    }

    fn distance(&self, query: &[i8], idx: usize) -> i32 {
        self.nodes.get(idx).map(|n| self.config.metric.distance(query, &n.vector)).unwrap_or(i32::MAX)
    }

    fn random_layer(&mut self) -> u8 {
        self.rng_state = self.rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        let layer = (self.rng_state.leading_zeros() / 4) as u8;
        layer.min(MAX_LAYERS as u8 - 1)
    }

    pub fn get(&self, idx: usize) -> Option<&[i8]> { self.nodes.get(idx).map(|n| n.vector.as_slice()) }
    pub fn get_id(&self, idx: usize) -> Option<u32> { self.nodes.get(idx).map(|n| n.id) }
}
