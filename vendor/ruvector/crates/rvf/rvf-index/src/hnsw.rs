//! Core HNSW (Hierarchical Navigable Small World) graph implementation.
//!
//! Implements the algorithm from Malkov & Yashunin (2018) with:
//! - Configurable M (max neighbors per layer) and ef_construction
//! - Layer selection via P = 1/ln(M), level = floor(-ln(random) * P)
//! - Greedy search at upper layers, beam search at layer 0

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use crate::traits::VectorStore;

/// Configuration for HNSW graph construction.
#[derive(Clone, Debug)]
pub struct HnswConfig {
    /// Maximum number of neighbors per node per layer (layer > 0).
    pub m: usize,
    /// Maximum number of neighbors at layer 0 (typically 2*M).
    pub m0: usize,
    /// Size of the dynamic candidate list during construction.
    pub ef_construction: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 200,
        }
    }
}

/// A single layer of the HNSW graph, mapping node IDs to their neighbor lists.
#[derive(Clone, Debug, Default)]
pub struct HnswLayer {
    /// Node ID -> sorted list of neighbor IDs.
    pub adjacency: BTreeMap<u64, Vec<u64>>,
}

impl HnswLayer {
    /// Returns true if this layer contains the given node.
    #[inline]
    pub fn contains(&self, id: u64) -> bool {
        self.adjacency.contains_key(&id)
    }

    /// Returns the neighbors of a node, or an empty slice if not present.
    #[inline]
    pub fn neighbors(&self, id: u64) -> &[u64] {
        self.adjacency.get(&id).map_or(&[], |v| v.as_slice())
    }

    /// Number of nodes in this layer.
    #[inline]
    pub fn len(&self) -> usize {
        self.adjacency.len()
    }

    /// Returns true if the layer has no nodes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.adjacency.is_empty()
    }
}

/// The full HNSW graph structure.
#[derive(Clone, Debug)]
pub struct HnswGraph {
    /// Layers from bottom (0) to top.
    pub layers: Vec<HnswLayer>,
    /// The entry point node ID (node at the highest layer).
    pub entry_point: Option<u64>,
    /// The highest occupied layer index.
    pub max_layer: usize,
    /// Max neighbors per layer (> 0).
    pub m: usize,
    /// Max neighbors at layer 0.
    pub m0: usize,
    /// ef_construction parameter.
    pub ef_construction: usize,
    /// Level normalization factor: 1 / ln(M).
    ml: f64,
}

impl HnswGraph {
    /// Create a new empty HNSW graph with the given configuration.
    pub fn new(config: &HnswConfig) -> Self {
        Self {
            layers: vec![HnswLayer::default()],
            entry_point: None,
            max_layer: 0,
            m: config.m,
            m0: config.m0,
            ef_construction: config.ef_construction,
            ml: 1.0 / (config.m as f64).ln(),
        }
    }

    /// Select a random level for a new node.
    /// Level = floor(-ln(uniform(0,1)) * ml).
    fn random_level(&self, rng_val: f64) -> usize {
        let r = if rng_val <= 0.0 { 1e-10 } else { rng_val };
        (-r.ln() * self.ml).floor() as usize
    }

    /// Insert a new node into the HNSW graph.
    ///
    /// `id`: the node ID to insert.
    /// `rng_val`: a uniform random value in (0, 1) for level selection.
    /// `vectors`: provides access to all vectors by ID.
    /// `distance_fn`: distance function between two vectors.
    pub fn insert(
        &mut self,
        id: u64,
        rng_val: f64,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) {
        let level = self.random_level(rng_val);

        // Ensure we have enough layers.
        while self.layers.len() <= level {
            self.layers.push(HnswLayer::default());
        }

        // Add the node to each layer from 0 to `level`.
        for l in 0..=level {
            self.layers[l].adjacency.entry(id).or_default();
        }

        let query_vec = match vectors.get_vector(id) {
            Some(v) => v,
            None => return,
        };

        if self.entry_point.is_none() {
            // First node.
            self.entry_point = Some(id);
            self.max_layer = level;
            return;
        }

        let ep = self.entry_point.unwrap();

        // Phase 1: greedy search from top layer down to level+1.
        let mut current_ep = ep;
        let top = self.max_layer;
        if top > level {
            for l in (level + 1..=top).rev() {
                current_ep = self.greedy_closest(query_vec, current_ep, l, vectors, distance_fn);
            }
        }

        // Phase 2: at each layer from min(level, max_layer) down to 0,
        // do a beam search and connect neighbors.
        let start_layer = level.min(top);
        let mut entry_points = vec![current_ep];

        for l in (0..=start_layer).rev() {
            let max_neighbors = if l == 0 { self.m0 } else { self.m };

            let candidates = self.search_layer(
                query_vec,
                &entry_points,
                self.ef_construction,
                l,
                vectors,
                distance_fn,
            );

            // Select the closest `max_neighbors` candidates.
            let selected: Vec<(u64, f32)> =
                candidates.iter().take(max_neighbors).cloned().collect();

            // Connect the new node to selected neighbors.
            let neighbor_ids: Vec<u64> = selected.iter().map(|&(nid, _)| nid).collect();
            self.layers[l].adjacency.insert(id, neighbor_ids.clone());

            // Bidirectional: add the new node as a neighbor of each selected node,
            // then prune if over the limit.
            for &nid in &neighbor_ids {
                let nlist = self.layers[l].adjacency.entry(nid).or_default();
                if !nlist.contains(&id) {
                    nlist.push(id);
                }
                if nlist.len() > max_neighbors {
                    // Prune: keep only the closest max_neighbors.
                    self.prune_neighbors(nid, l, max_neighbors, vectors, distance_fn);
                }
            }

            // Use the selected candidates as entry points for the next layer down.
            entry_points = selected.iter().map(|&(nid, _)| nid).collect();
        }

        // Update entry point if the new node is at a higher layer.
        if level > self.max_layer {
            self.entry_point = Some(id);
            self.max_layer = level;
        }
    }

    /// Greedy search: starting from `ep`, walk to the closest node at `layer`.
    fn greedy_closest(
        &self,
        query: &[f32],
        ep: u64,
        layer: usize,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> u64 {
        let mut current = ep;
        let mut current_dist = match vectors.get_vector(ep) {
            Some(v) => distance_fn(query, v),
            None => return ep,
        };

        loop {
            let mut changed = false;
            let neighbors = self.layers[layer].neighbors(current);
            for &nid in neighbors {
                if let Some(nv) = vectors.get_vector(nid) {
                    let d = distance_fn(query, nv);
                    if d < current_dist {
                        current = nid;
                        current_dist = d;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        current
    }

    /// Beam search at a given layer. Returns candidates sorted by distance (ascending).
    fn search_layer(
        &self,
        query: &[f32],
        entry_points: &[u64],
        ef: usize,
        layer: usize,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> Vec<(u64, f32)> {
        #[cfg(not(feature = "std"))]
        use alloc::collections::BTreeSet as HashSet;
        #[cfg(feature = "std")]
        use std::collections::HashSet;

        let mut visited = HashSet::new();
        // candidates sorted by (distance, id) — acts as a min-heap.
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

        let mut candidate_idx = 0;

        while candidate_idx < candidates.len() {
            let (cid, cdist) = candidates[candidate_idx];
            candidate_idx += 1;

            // If the closest candidate is farther than the worst result and
            // we already have `ef` results, stop.
            if results.len() >= ef {
                let worst_dist = results.last().map_or(f32::MAX, |r| r.1);
                if cdist > worst_dist {
                    break;
                }
            }

            let neighbors = self.layers[layer].neighbors(cid);
            for &nid in neighbors {
                if !visited.insert(nid) {
                    continue;
                }
                if let Some(nv) = vectors.get_vector(nid) {
                    let d = distance_fn(query, nv);
                    let worst_dist = if results.len() >= ef {
                        results.last().map_or(f32::MAX, |r| r.1)
                    } else {
                        f32::MAX
                    };

                    if d < worst_dist || results.len() < ef {
                        // Insert into candidates (sorted).
                        let pos = candidates[candidate_idx..]
                            .binary_search_by(|probe| {
                                probe
                                    .1
                                    .partial_cmp(&d)
                                    .unwrap_or(core::cmp::Ordering::Equal)
                            })
                            .unwrap_or_else(|e| e);
                        candidates.insert(candidate_idx + pos, (nid, d));

                        // Insert into results (sorted).
                        let rpos = results
                            .binary_search_by(|probe| {
                                probe
                                    .1
                                    .partial_cmp(&d)
                                    .unwrap_or(core::cmp::Ordering::Equal)
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

        results
    }

    /// Prune neighbors of a node to keep only the closest `max_neighbors`.
    fn prune_neighbors(
        &mut self,
        node: u64,
        layer: usize,
        max_neighbors: usize,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) {
        let node_vec = match vectors.get_vector(node) {
            Some(v) => v,
            None => return,
        };
        let neighbors = match self.layers[layer].adjacency.get(&node) {
            Some(n) => n.clone(),
            None => return,
        };

        let mut scored: Vec<(u64, f32)> = neighbors
            .iter()
            .filter_map(|&nid| {
                vectors
                    .get_vector(nid)
                    .map(|nv| (nid, distance_fn(node_vec, nv)))
            })
            .collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
        scored.truncate(max_neighbors);

        let pruned: Vec<u64> = scored.into_iter().map(|(nid, _)| nid).collect();
        self.layers[layer].adjacency.insert(node, pruned);
    }

    /// Search the HNSW graph for the `k` nearest neighbors of `query`.
    ///
    /// `ef_search`: size of the dynamic candidate list during search.
    /// Returns a list of `(node_id, distance)` sorted by distance (ascending).
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        vectors: &dyn VectorStore,
        distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
    ) -> Vec<(u64, f32)> {
        let ep = match self.entry_point {
            Some(ep) => ep,
            None => return Vec::new(),
        };

        let ef = ef_search.max(k);

        // Phase 1: greedy search from top layer down to layer 1.
        let mut current_ep = ep;
        for l in (1..=self.max_layer).rev() {
            current_ep = self.greedy_closest(query, current_ep, l, vectors, distance_fn);
        }

        // Phase 2: beam search at layer 0.
        let mut results = self.search_layer(query, &[current_ep], ef, 0, vectors, distance_fn);
        results.truncate(k);
        results
    }

    /// Returns the total number of nodes across all layers.
    pub fn node_count(&self) -> usize {
        self.layers.first().map_or(0, |l| l.adjacency.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distance::l2_distance;
    use crate::traits::InMemoryVectorStore;

    fn make_config() -> HnswConfig {
        HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 100,
        }
    }

    #[test]
    fn empty_graph_search_returns_empty() {
        let config = make_config();
        let graph = HnswGraph::new(&config);
        let store = InMemoryVectorStore::new(vec![vec![0.0; 4]]);
        let results = graph.search(&[0.0; 4], 5, 50, &store, &l2_distance);
        assert!(results.is_empty());
    }

    #[test]
    fn insert_single_node() {
        let config = make_config();
        let mut graph = HnswGraph::new(&config);
        let store = InMemoryVectorStore::new(vec![vec![1.0, 2.0, 3.0]]);
        graph.insert(0, 0.5, &store, &l2_distance);

        assert_eq!(graph.entry_point, Some(0));
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn insert_and_search_small() {
        let config = make_config();
        let mut graph = HnswGraph::new(&config);

        let vectors: Vec<Vec<f32>> = (0..20)
            .map(|i| vec![i as f32, (i * 2) as f32, (i * 3) as f32])
            .collect();
        let store = InMemoryVectorStore::new(vectors);

        // Insert all with deterministic pseudo-random values.
        for i in 0..20u64 {
            let rng = ((i * 7 + 3) % 100) as f64 / 100.0;
            graph.insert(i, rng, &store, &l2_distance);
        }

        // Search for a query near node 10.
        let query = [10.0, 20.0, 30.0];
        let results = graph.search(&query, 3, 50, &store, &l2_distance);
        assert!(!results.is_empty());
        // Node 10 should be the closest (exact match).
        assert_eq!(results[0].0, 10);
    }

    /// Build HNSW with 1000 random vectors, verify recall@10 >= 0.95.
    #[test]
    fn recall_at_10_1000_vectors() {
        use alloc::collections::BTreeSet;

        let n = 1000;
        let dim = 32;

        // Generate deterministic pseudo-random vectors using a simple LCG.
        let mut seed: u64 = 42;
        let vectors: Vec<Vec<f32>> = (0..n)
            .map(|_| {
                (0..dim)
                    .map(|_| {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        (seed >> 33) as f32 / (1u64 << 31) as f32
                    })
                    .collect()
            })
            .collect();
        let store = InMemoryVectorStore::new(vectors.clone());

        // Build the graph.
        let config = HnswConfig {
            m: 16,
            m0: 32,
            ef_construction: 200,
        };
        let mut graph = HnswGraph::new(&config);
        let mut rng_seed: u64 = 123;
        for i in 0..n as u64 {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let rng_val = (rng_seed >> 33) as f64 / (1u64 << 31) as f64;
            let rng_val = rng_val.clamp(0.001, 0.999);
            graph.insert(i, rng_val, &store, &l2_distance);
        }

        // Compute brute-force ground truth and measure recall.
        let num_queries = 50;
        let k = 10;
        let ef_search = 200;
        let mut total_recall = 0.0;

        let mut query_seed: u64 = 999;
        for _ in 0..num_queries {
            // Generate a random query.
            let query: Vec<f32> = (0..dim)
                .map(|_| {
                    query_seed = query_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (query_seed >> 33) as f32 / (1u64 << 31) as f32
                })
                .collect();

            // Brute-force top-k.
            let mut all_dists: Vec<(u64, f32)> = (0..n as u64)
                .map(|i| (i, l2_distance(&query, &vectors[i as usize])))
                .collect();
            all_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let gt_set: BTreeSet<u64> = all_dists.iter().take(k).map(|&(id, _)| id).collect();

            // HNSW search.
            let results = graph.search(&query, k, ef_search, &store, &l2_distance);
            let result_set: BTreeSet<u64> = results.iter().map(|&(id, _)| id).collect();

            let overlap = gt_set.intersection(&result_set).count();
            total_recall += overlap as f64 / k as f64;
        }

        let avg_recall = total_recall / num_queries as f64;
        assert!(
            avg_recall >= 0.95,
            "Recall@10 = {:.3}, expected >= 0.95",
            avg_recall
        );
    }
}
