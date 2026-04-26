//! Micro-HNSW vector search -- spatial reasoning module (ADR-041).
//!
//! On-device approximate nearest-neighbour search for CSI fingerprint
//! matching.  Stores up to 64 reference vectors of dimension 8 in a
//! single-layer navigable small-world graph.  No heap, no_std.
//!
//! Event IDs: 765-768 (Spatial Reasoning series).

use libm::sqrtf;

const MAX_VECTORS: usize = 64;
const DIM: usize = 8;
const MAX_NEIGHBORS: usize = 4;

// M-06 fix: compile-time assertion that neighbor indices fit in u8.
const _: () = assert!(MAX_VECTORS <= 255, "MAX_VECTORS must fit in u8 for neighbor index storage");
const BEAM_WIDTH: usize = 4;
const MAX_HOPS: usize = 8;
const CLASS_UNKNOWN: u8 = 255;
const MATCH_THRESHOLD: f32 = 2.0;

pub const EVENT_NEAREST_MATCH_ID: i32 = 765;
pub const EVENT_MATCH_DISTANCE: i32 = 766;
pub const EVENT_CLASSIFICATION: i32 = 767;
pub const EVENT_LIBRARY_SIZE: i32 = 768;

struct HnswNode {
    vec: [f32; DIM],
    neighbors: [u8; MAX_NEIGHBORS],
    n_neighbors: u8,
    label: u8,
}

impl HnswNode {
    const fn empty() -> Self {
        Self { vec: [0.0; DIM], neighbors: [0xFF; MAX_NEIGHBORS], n_neighbors: 0, label: CLASS_UNKNOWN }
    }
}

/// Squared L2 distance between two DIM-dimensional vectors (inline helper).
fn l2_sq(a: &[f32; DIM], b: &[f32; DIM]) -> f32 {
    let mut s = 0.0f32;
    let mut i = 0;
    while i < DIM { let d = a[i] - b[i]; s += d * d; i += 1; }
    s
}

/// L2 distance between a stored vector and a query slice.
fn l2_query(stored: &[f32; DIM], query: &[f32]) -> f32 {
    let mut s = 0.0f32;
    let len = if query.len() < DIM { query.len() } else { DIM };
    let mut i = 0;
    while i < len { let d = stored[i] - query[i]; s += d * d; i += 1; }
    sqrtf(s)
}

/// Micro-HNSW on-device vector index.
pub struct MicroHnsw {
    nodes: [HnswNode; MAX_VECTORS],
    n_vectors: usize,
    entry_point: usize,
    frame_count: u32,
    last_nearest: usize,
    last_distance: f32,
}

impl MicroHnsw {
    pub const fn new() -> Self {
        const EMPTY: HnswNode = HnswNode::empty();
        Self {
            nodes: [EMPTY; MAX_VECTORS], n_vectors: 0, entry_point: usize::MAX,
            frame_count: 0, last_nearest: 0, last_distance: f32::MAX,
        }
    }

    /// Insert a reference vector with a classification label.
    pub fn insert(&mut self, vec: &[f32], label: u8) -> Option<usize> {
        if self.n_vectors >= MAX_VECTORS { return None; }
        let idx = self.n_vectors;
        let dim = vec.len().min(DIM);
        let mut i = 0;
        while i < dim { self.nodes[idx].vec[i] = vec[i]; i += 1; }
        self.nodes[idx].label = label;
        self.nodes[idx].n_neighbors = 0;
        self.n_vectors += 1;

        if self.entry_point == usize::MAX {
            self.entry_point = idx;
            return Some(idx);
        }

        // Find nearest MAX_NEIGHBORS existing nodes (linear scan, N<=64).
        let mut nearest = [(f32::MAX, 0usize); MAX_NEIGHBORS];
        let mut i = 0;
        while i < idx {
            let d = sqrtf(l2_sq(&self.nodes[idx].vec, &self.nodes[i].vec));
            let mut slot = 0;
            while slot < MAX_NEIGHBORS {
                if d < nearest[slot].0 {
                    let mut k = MAX_NEIGHBORS - 1;
                    while k > slot { nearest[k] = nearest[k - 1]; k -= 1; }
                    nearest[slot] = (d, i);
                    break;
                }
                slot += 1;
            }
            i += 1;
        }

        // Add bidirectional edges.
        let mut slot = 0;
        while slot < MAX_NEIGHBORS {
            if nearest[slot].0 >= f32::MAX { break; }
            let ni = nearest[slot].1;
            self.add_edge(idx, ni);
            self.add_edge(ni, idx);
            slot += 1;
        }
        Some(idx)
    }

    fn add_edge(&mut self, from: usize, to: usize) {
        let nn = self.nodes[from].n_neighbors as usize;
        if nn >= MAX_NEIGHBORS {
            let new_d = l2_sq(&self.nodes[from].vec, &self.nodes[to].vec);
            let mut worst_slot = 0usize;
            let mut worst_d = 0.0f32;
            let mut i = 0;
            while i < MAX_NEIGHBORS {
                let ni = self.nodes[from].neighbors[i] as usize;
                if ni < MAX_VECTORS {
                    let d = l2_sq(&self.nodes[from].vec, &self.nodes[ni].vec);
                    if d > worst_d { worst_d = d; worst_slot = i; }
                }
                i += 1;
            }
            if new_d < worst_d { self.nodes[from].neighbors[worst_slot] = to as u8; }
        } else {
            let mut i = 0;
            while i < nn {
                if self.nodes[from].neighbors[i] as usize == to { return; }
                i += 1;
            }
            self.nodes[from].neighbors[nn] = to as u8;
            self.nodes[from].n_neighbors += 1;
        }
    }

    /// Search for the nearest vector.  Returns (index, distance).
    pub fn search(&self, query: &[f32]) -> (usize, f32) {
        if self.n_vectors == 0 { return (usize::MAX, f32::MAX); }

        let mut beam = [(f32::MAX, 0usize); BEAM_WIDTH];
        beam[0] = (l2_query(&self.nodes[self.entry_point].vec, query), self.entry_point);
        let mut visited = [false; MAX_VECTORS];
        visited[self.entry_point] = true;

        let mut hop = 0;
        while hop < MAX_HOPS {
            let mut improved = false;
            let mut b = 0;
            while b < BEAM_WIDTH {
                if beam[b].0 >= f32::MAX { break; }
                let node = &self.nodes[beam[b].1];
                let mut n = 0;
                while n < node.n_neighbors as usize {
                    let ni = node.neighbors[n] as usize;
                    if ni < self.n_vectors && !visited[ni] {
                        visited[ni] = true;
                        let d = l2_query(&self.nodes[ni].vec, query);
                        let mut slot = 0;
                        while slot < BEAM_WIDTH {
                            if d < beam[slot].0 {
                                let mut k = BEAM_WIDTH - 1;
                                while k > slot { beam[k] = beam[k - 1]; k -= 1; }
                                beam[slot] = (d, ni);
                                improved = true;
                                break;
                            }
                            slot += 1;
                        }
                    }
                    n += 1;
                }
                b += 1;
            }
            if !improved { break; }
            hop += 1;
        }
        (beam[0].1, beam[0].0)
    }

    /// Process one CSI frame (top features as query).
    pub fn process_frame(&mut self, features: &[f32]) -> &[(i32, f32)] {
        self.frame_count += 1;
        if self.n_vectors == 0 {
            static mut EMPTY: [(i32, f32); 1] = [(0, 0.0); 1];
            unsafe { EMPTY[0] = (EVENT_LIBRARY_SIZE, 0.0); }
            return unsafe { &EMPTY[..1] };
        }
        let (nearest_id, distance) = self.search(features);
        self.last_nearest = nearest_id;
        self.last_distance = distance;
        let label = if nearest_id < self.n_vectors && distance < MATCH_THRESHOLD {
            self.nodes[nearest_id].label
        } else { CLASS_UNKNOWN };

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        unsafe {
            EVENTS[0] = (EVENT_NEAREST_MATCH_ID, nearest_id as f32);
            EVENTS[1] = (EVENT_MATCH_DISTANCE, distance);
            EVENTS[2] = (EVENT_CLASSIFICATION, label as f32);
            EVENTS[3] = (EVENT_LIBRARY_SIZE, self.n_vectors as f32);
        }
        unsafe { &EVENTS[..4] }
    }

    pub fn size(&self) -> usize { self.n_vectors }

    pub fn last_label(&self) -> u8 {
        if self.last_nearest < self.n_vectors && self.last_distance < MATCH_THRESHOLD {
            self.nodes[self.last_nearest].label
        } else { CLASS_UNKNOWN }
    }

    pub fn last_match_distance(&self) -> f32 { self.last_distance }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_constructor() {
        let hnsw = MicroHnsw::new();
        assert_eq!(hnsw.size(), 0);
        assert_eq!(hnsw.entry_point, usize::MAX);
    }

    #[test]
    fn test_insert_single() {
        let mut hnsw = MicroHnsw::new();
        let idx = hnsw.insert(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 1);
        assert_eq!(idx, Some(0));
        assert_eq!(hnsw.size(), 1);
    }

    #[test]
    fn test_insert_and_search_exact() {
        let mut hnsw = MicroHnsw::new();
        let v0 = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let v1 = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        hnsw.insert(&v0, 10);
        hnsw.insert(&v1, 20);
        let (id, dist) = hnsw.search(&v1);
        assert_eq!(id, 1);
        assert!(dist < 0.01);
    }

    #[test]
    fn test_search_nearest() {
        let mut hnsw = MicroHnsw::new();
        hnsw.insert(&[0.0; 8], 0);
        hnsw.insert(&[10.0; 8], 1);
        let (id, _) = hnsw.search(&[0.1, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(id, 0);
        let (id2, _) = hnsw.search(&[9.9, 9.8, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0]);
        assert_eq!(id2, 1);
    }

    #[test]
    fn test_capacity_limit() {
        let mut hnsw = MicroHnsw::new();
        for i in 0..MAX_VECTORS {
            let mut v = [0.0f32; 8];
            v[0] = i as f32;
            assert!(hnsw.insert(&v, i as u8).is_some());
        }
        assert!(hnsw.insert(&[99.0; 8], 0).is_none());
    }

    #[test]
    fn test_process_frame_empty() {
        let mut hnsw = MicroHnsw::new();
        let events = hnsw.process_frame(&[0.0f32; 8]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, EVENT_LIBRARY_SIZE);
    }

    #[test]
    fn test_process_frame_with_data() {
        let mut hnsw = MicroHnsw::new();
        hnsw.insert(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 5);
        hnsw.insert(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 10);
        let events = hnsw.process_frame(&[0.9, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].0, EVENT_NEAREST_MATCH_ID);
        assert!((events[0].1 - 0.0).abs() < 1e-6);
        assert!((events[2].1 - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_classification_unknown_far() {
        let mut hnsw = MicroHnsw::new();
        hnsw.insert(&[0.0; 8], 42);
        let (_, dist) = hnsw.search(&[100.0; 8]);
        assert!(dist > MATCH_THRESHOLD);
        let events = hnsw.process_frame(&[100.0; 8]);
        assert!((events[2].1 - CLASS_UNKNOWN as f32).abs() < 1e-6);
    }
}
