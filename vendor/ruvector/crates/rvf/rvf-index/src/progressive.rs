//! Progressive search combining Layers A, B, and C.
//!
//! Depending on which layers are loaded, the search adapts:
//! - Layer A only: centroid routing + top-layer HNSW + hot cache scan
//! - A + B: HNSW through hot region, fallback centroid scan for cold
//! - A + B + C: full HNSW at all layers

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::distance::l2_distance;
use crate::layers::{LayerA, LayerB, LayerC};
use crate::traits::VectorStore;

/// Progressive index that adapts search quality based on loaded layers.
#[derive(Clone, Debug)]
pub struct ProgressiveIndex {
    pub layer_a: Option<LayerA>,
    pub layer_b: Option<LayerB>,
    pub layer_c: Option<LayerC>,
}

impl ProgressiveIndex {
    /// Create a new empty progressive index.
    pub fn new() -> Self {
        Self {
            layer_a: None,
            layer_b: None,
            layer_c: None,
        }
    }

    /// Search using whatever layers are available.
    ///
    /// Returns `(node_id, distance)` pairs sorted by distance ascending.
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        vectors: &dyn VectorStore,
    ) -> Vec<(u64, f32)> {
        self.search_with_distance(query, k, ef_search, vectors, &l2_distance)
    }

    /// Search with a custom distance function.
    pub fn search_with_distance(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> Vec<(u64, f32)> {
        match (&self.layer_a, &self.layer_b, &self.layer_c) {
            (None, _, _) => Vec::new(),
            (Some(a), None, None) => self.search_layer_a_only(query, k, a, vectors, distance_fn),
            (Some(a), Some(b), None) => {
                self.search_a_plus_b(query, k, ef_search, a, b, vectors, distance_fn)
            }
            (Some(_a), _, Some(c)) => {
                self.search_full(query, k, ef_search, c, vectors, distance_fn)
            }
        }
    }

    /// Search using only Layer A: centroid routing + top-layer HNSW traversal.
    fn search_layer_a_only(
        &self,
        query: &[f32],
        k: usize,
        layer_a: &LayerA,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> Vec<(u64, f32)> {
        let mut candidates: Vec<(u64, f32)> = Vec::new();

        // Step 1: find nearest centroids.
        let n_probe = 10.min(layer_a.centroids.len());
        let mut centroid_dists: Vec<(usize, f32)> = layer_a
            .centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, distance_fn(query, c)))
            .collect();
        centroid_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
        centroid_dists.truncate(n_probe);

        // Step 2: HNSW search through top layers using Layer A entry points.
        if let Some(&(ep, _)) = layer_a.entry_points.first() {
            let mut current = ep;
            // Greedy walk through top layers.
            for tl in &layer_a.top_layers {
                current = greedy_walk(query, current, tl, vectors, distance_fn);
            }
            if let Some(v) = vectors.get_vector(current) {
                candidates.push((current, distance_fn(query, v)));
            }

            // Also check neighbors of the landing node at the lowest top layer.
            if let Some(last_tl) = layer_a.top_layers.last() {
                for &nid in last_tl.neighbors(current) {
                    if let Some(nv) = vectors.get_vector(nid) {
                        candidates.push((nid, distance_fn(query, nv)));
                    }
                }
            }
        }

        // Step 3: scan vectors in the nearest centroid partitions.
        for &(ci, _) in &centroid_dists {
            for part in &layer_a.partition_map {
                if part.centroid_id == ci as u32 {
                    // Scan vectors in this partition.
                    for vid in part.vector_id_start..part.vector_id_end {
                        if let Some(v) = vectors.get_vector(vid) {
                            candidates.push((vid, distance_fn(query, v)));
                        }
                    }
                }
            }
        }

        // Deduplicate and return top-k.
        dedup_top_k(&mut candidates, k)
    }

    /// Search using Layers A + B: HNSW through hot region, fallback for cold.
    #[allow(clippy::too_many_arguments)]
    fn search_a_plus_b(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        layer_a: &LayerA,
        layer_b: &LayerB,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> Vec<(u64, f32)> {
        let ef = ef_search.max(k);
        let mut visited = BTreeSet::new();
        let mut results: Vec<(u64, f32)> = Vec::new();

        // Start with Layer A routing to find the best entry into hot region.
        let entry = layer_a.entry_points.first().map(|&(ep, _)| ep).unwrap_or(0);

        let mut current = entry;
        for tl in &layer_a.top_layers {
            current = greedy_walk(query, current, tl, vectors, distance_fn);
        }

        // Beam search through Layer B's partial adjacency.
        let mut candidates: Vec<(u64, f32)> = Vec::new();
        if let Some(v) = vectors.get_vector(current) {
            let d = distance_fn(query, v);
            candidates.push((current, d));
            results.push((current, d));
            visited.insert(current);
        }

        let mut idx = 0;
        while idx < candidates.len() {
            let (cid, cdist) = candidates[idx];
            idx += 1;

            if results.len() >= ef {
                let worst = results.last().map_or(f32::MAX, |r| r.1);
                if cdist > worst {
                    break;
                }
            }

            // Get neighbors: prefer Layer B, fallback to Layer A's top layers.
            let neighbor_ids: Vec<u64> = if let Some(neighbors) = layer_b.neighbors(cid) {
                neighbors.to_vec()
            } else {
                // Fallback: check top layers for any adjacency.
                let mut fallback = Vec::new();
                for tl in &layer_a.top_layers {
                    fallback.extend_from_slice(tl.neighbors(cid));
                }
                fallback
            };

            for nid in neighbor_ids {
                if !visited.insert(nid) {
                    continue;
                }
                if let Some(nv) = vectors.get_vector(nid) {
                    let d = distance_fn(query, nv);
                    let worst = if results.len() >= ef {
                        results.last().map_or(f32::MAX, |r| r.1)
                    } else {
                        f32::MAX
                    };

                    if d < worst || results.len() < ef {
                        let pos = candidates[idx..]
                            .binary_search_by(|p| {
                                p.1.partial_cmp(&d).unwrap_or(core::cmp::Ordering::Equal)
                            })
                            .unwrap_or_else(|e| e);
                        candidates.insert(idx + pos, (nid, d));

                        let rpos = results
                            .binary_search_by(|p| {
                                p.1.partial_cmp(&d).unwrap_or(core::cmp::Ordering::Equal)
                            })
                            .unwrap_or_else(|e| e);
                        results.insert(rpos, (nid, d));

                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        results.truncate(k);
        results
    }

    /// Search using full Layer C HNSW graph.
    fn search_full(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        layer_c: &LayerC,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> Vec<(u64, f32)> {
        let ef = ef_search.max(k);
        let max_layer = if layer_c.full_adjacency.is_empty() {
            return Vec::new();
        } else {
            layer_c.full_adjacency.len() - 1
        };

        // Find the entry point: any node at the highest layer.
        let entry = match layer_c.full_adjacency[max_layer].adjacency.keys().next() {
            Some(&ep) => ep,
            None => return Vec::new(),
        };

        // Phase 1: greedy descent through upper layers.
        let mut current = entry;
        for l in (1..=max_layer).rev() {
            current = greedy_walk(
                query,
                current,
                &layer_c.full_adjacency[l],
                vectors,
                distance_fn,
            );
        }

        // Phase 2: beam search at layer 0.
        beam_search_layer(
            query,
            &[current],
            ef,
            k,
            &layer_c.full_adjacency[0],
            vectors,
            distance_fn,
        )
    }
}

impl Default for ProgressiveIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────

/// Greedy walk to the closest node in a single HNSW layer.
fn greedy_walk(
    query: &[f32],
    start: u64,
    layer: &crate::hnsw::HnswLayer,
    vectors: &dyn VectorStore,
    distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
) -> u64 {
    let mut current = start;
    let mut current_dist = match vectors.get_vector(start) {
        Some(v) => distance_fn(query, v),
        None => return start,
    };

    loop {
        let mut improved = false;
        for &nid in layer.neighbors(current) {
            if let Some(nv) = vectors.get_vector(nid) {
                let d = distance_fn(query, nv);
                if d < current_dist {
                    current = nid;
                    current_dist = d;
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
    current
}

/// Beam search at a single HNSW layer. Returns top-k results sorted by distance.
fn beam_search_layer(
    query: &[f32],
    entry_points: &[u64],
    ef: usize,
    k: usize,
    layer: &crate::hnsw::HnswLayer,
    vectors: &dyn VectorStore,
    distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
) -> Vec<(u64, f32)> {
    let mut visited = BTreeSet::new();
    let mut candidates: Vec<(u64, f32)> = Vec::new();
    let mut results: Vec<(u64, f32)> = Vec::new();

    for &ep in entry_points {
        if visited.insert(ep) {
            if let Some(v) = vectors.get_vector(ep) {
                let d = distance_fn(query, v);
                candidates.push((ep, d));
                results.push((ep, d));
            }
        }
    }

    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
    results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));

    let mut idx = 0;
    while idx < candidates.len() {
        let (cid, cdist) = candidates[idx];
        idx += 1;

        if results.len() >= ef {
            let worst = results.last().map_or(f32::MAX, |r| r.1);
            if cdist > worst {
                break;
            }
        }

        for &nid in layer.neighbors(cid) {
            if !visited.insert(nid) {
                continue;
            }
            if let Some(nv) = vectors.get_vector(nid) {
                let d = distance_fn(query, nv);
                let worst = if results.len() >= ef {
                    results.last().map_or(f32::MAX, |r| r.1)
                } else {
                    f32::MAX
                };

                if d < worst || results.len() < ef {
                    let pos = candidates[idx..]
                        .binary_search_by(|p| {
                            p.1.partial_cmp(&d).unwrap_or(core::cmp::Ordering::Equal)
                        })
                        .unwrap_or_else(|e| e);
                    candidates.insert(idx + pos, (nid, d));

                    let rpos = results
                        .binary_search_by(|p| {
                            p.1.partial_cmp(&d).unwrap_or(core::cmp::Ordering::Equal)
                        })
                        .unwrap_or_else(|e| e);
                    results.insert(rpos, (nid, d));

                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }
    }

    results.truncate(k);
    results
}

/// Deduplicate candidates by node ID and return top-k by distance.
fn dedup_top_k(candidates: &mut [(u64, f32)], k: usize) -> Vec<(u64, f32)> {
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(k);
    for &(id, dist) in candidates.iter() {
        if seen.insert(id) {
            result.push((id, dist));
            if result.len() == k {
                break;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hnsw::{HnswConfig, HnswGraph};
    use crate::layers::{LayerA, LayerC, PartitionEntry};
    use crate::traits::InMemoryVectorStore;

    fn make_test_vectors(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n)
            .map(|i| (0..dim).map(|d| (i * dim + d) as f32).collect())
            .collect()
    }

    #[test]
    fn progressive_empty_returns_empty() {
        let idx = ProgressiveIndex::new();
        let store = InMemoryVectorStore::new(vec![vec![0.0; 4]]);
        let results = idx.search(&[0.0; 4], 5, 50, &store);
        assert!(results.is_empty());
    }

    #[test]
    fn progressive_layer_a_only() {
        let vectors = make_test_vectors(100, 4);
        let store = InMemoryVectorStore::new(vectors.clone());

        // Build a centroid from first 50 vectors (partition 0) and last 50 (partition 1).
        let centroid_0: Vec<f32> = (0..4)
            .map(|d| (0..50).map(|i| vectors[i][d]).sum::<f32>() / 50.0)
            .collect();
        let centroid_1: Vec<f32> = (0..4)
            .map(|d| (50..100).map(|i| vectors[i][d]).sum::<f32>() / 50.0)
            .collect();

        let idx = ProgressiveIndex {
            layer_a: Some(LayerA {
                entry_points: vec![(0, 0)],
                top_layers: vec![],
                top_layer_start: 0,
                centroids: vec![centroid_0, centroid_1],
                partition_map: vec![
                    PartitionEntry {
                        centroid_id: 0,
                        vector_id_start: 0,
                        vector_id_end: 50,
                        segment_ref: 0,
                        block_ref: 0,
                    },
                    PartitionEntry {
                        centroid_id: 1,
                        vector_id_start: 50,
                        vector_id_end: 100,
                        segment_ref: 0,
                        block_ref: 0,
                    },
                ],
            }),
            layer_b: None,
            layer_c: None,
        };

        let query = vectors[25].clone();
        let results = idx.search(&query, 5, 50, &store);
        assert!(!results.is_empty());
        // The exact match should be found.
        assert_eq!(results[0].0, 25);
    }

    #[test]
    fn progressive_full_layer_c() {
        let n = 200;
        let dim = 4;
        let vectors = make_test_vectors(n, dim);
        let store = InMemoryVectorStore::new(vectors.clone());

        // Build a full HNSW graph, then extract it as Layer C.
        let config = HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 100,
        };
        let mut graph = HnswGraph::new(&config);
        for i in 0..n as u64 {
            let rng = ((i * 7 + 3) % 100) as f64 / 100.0;
            graph.insert(i, rng, &store, &l2_distance);
        }

        let layer_c = LayerC {
            full_adjacency: graph.layers.clone(),
        };

        let idx = ProgressiveIndex {
            layer_a: Some(LayerA {
                entry_points: vec![(graph.entry_point.unwrap(), graph.max_layer as u32)],
                top_layers: vec![],
                top_layer_start: 0,
                centroids: vec![],
                partition_map: vec![],
            }),
            layer_b: None,
            layer_c: Some(layer_c),
        };

        // Query for a known vector.
        let target = 100;
        let query = vectors[target].clone();
        let results = idx.search(&query, 10, 100, &store);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, target as u64);
    }
}
