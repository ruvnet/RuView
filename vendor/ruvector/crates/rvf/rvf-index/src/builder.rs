//! Index construction: building Layer A, B, C from vectors and an HNSW graph.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::hnsw::{HnswConfig, HnswGraph, HnswLayer};
use crate::layers::{LayerA, LayerB, LayerC, PartitionEntry};
use crate::traits::VectorStore;

/// Build the full HNSW graph from a set of vectors.
///
/// `rng_values`: one random value per vector for level selection.
/// These should be uniform in (0, 1).
pub fn build_full_index(
    vectors: &dyn VectorStore,
    num_vectors: usize,
    config: &HnswConfig,
    rng_values: &[f64],
    distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
) -> HnswGraph {
    assert!(
        rng_values.len() >= num_vectors,
        "Need at least one rng value per vector"
    );

    let mut graph = HnswGraph::new(config);
    for (i, &rng_val) in rng_values.iter().enumerate().take(num_vectors) {
        graph.insert(i as u64, rng_val, vectors, distance_fn);
    }
    graph
}

/// Build Layer A from an existing HNSW graph.
///
/// Extracts entry points, top-layer adjacency, centroids, and a partition map.
///
/// `centroids`: precomputed cluster centroids.
/// `assignments`: for each vector ID, the centroid index it's assigned to.
pub fn build_layer_a(
    graph: &HnswGraph,
    centroids: &[Vec<f32>],
    assignments: &[u32],
    _num_vectors: u64,
) -> LayerA {
    let entry_points = match graph.entry_point {
        Some(ep) => vec![(ep, graph.max_layer as u32)],
        None => vec![],
    };

    // Extract top layers. "Top" = layers above the threshold.
    // For progressive indexing, we take layers >= max_layer - 1 (at least
    // the top 2 layers). Adjust based on graph size.
    let threshold = graph.max_layer.saturating_sub(1);

    let top_layers: Vec<HnswLayer> = graph.layers[threshold..].to_vec();

    // Build partition map from assignments.
    let mut partitions: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    for (vid, &centroid_id) in assignments.iter().enumerate() {
        let entry = partitions
            .entry(centroid_id)
            .or_insert((vid as u64, vid as u64));
        entry.0 = entry.0.min(vid as u64);
        entry.1 = entry.1.max(vid as u64 + 1);
    }

    let partition_map: Vec<PartitionEntry> = partitions
        .into_iter()
        .map(|(centroid_id, (start, end))| PartitionEntry {
            centroid_id,
            vector_id_start: start,
            vector_id_end: end,
            segment_ref: 0,
            block_ref: 0,
        })
        .collect();

    LayerA {
        entry_points,
        top_layers,
        top_layer_start: threshold,
        centroids: centroids.to_vec(),
        partition_map,
    }
}

/// Build Layer B from an existing HNSW graph, keeping only hot nodes.
///
/// `hot_node_ids`: the set of node IDs in the hot working set.
pub fn build_layer_b(graph: &HnswGraph, hot_node_ids: &BTreeSet<u64>) -> LayerB {
    let mut partial_adjacency = BTreeMap::new();

    // For each hot node, include its layer 0 neighbors.
    if let Some(layer0) = graph.layers.first() {
        for &nid in hot_node_ids {
            if let Some(neighbors) = layer0.adjacency.get(&nid) {
                partial_adjacency.insert(nid, neighbors.clone());
            }
        }
    }

    // Compute covered ranges from the hot node set.
    let covered_ranges = compute_ranges(hot_node_ids);

    LayerB {
        partial_adjacency,
        covered_ranges,
    }
}

/// Build Layer C from the full HNSW graph (just wraps all adjacency).
pub fn build_layer_c(graph: &HnswGraph) -> LayerC {
    LayerC {
        full_adjacency: graph.layers.clone(),
    }
}

/// Incrementally add a vector to an existing HNSW graph.
pub fn incremental_insert(
    graph: &mut HnswGraph,
    id: u64,
    rng_val: f64,
    vectors: &dyn VectorStore,
    distance_fn: &dyn Fn(&[f32], &[f32]) -> f32,
) {
    graph.insert(id, rng_val, vectors, distance_fn);
}

/// Compute contiguous ranges from a sorted set of IDs.
fn compute_ranges(ids: &BTreeSet<u64>) -> Vec<(u64, u64)> {
    if ids.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut iter = ids.iter();
    let &first = iter.next().unwrap();
    let mut start = first;
    let mut end = first + 1;

    for &id in iter {
        if id == end {
            end = id + 1;
        } else {
            ranges.push((start, end));
            start = id;
            end = id + 1;
        }
    }
    ranges.push((start, end));
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distance::l2_distance;
    use crate::traits::InMemoryVectorStore;

    #[test]
    fn build_full_index_basic() {
        let n = 50;
        let dim = 4;
        let vecs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..dim).map(|d| (i * dim + d) as f32).collect())
            .collect();
        let store = InMemoryVectorStore::new(vecs);

        let config = HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 50,
        };
        let rng_vals: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 100) as f64 / 100.0).collect();

        let graph = build_full_index(&store, n, &config, &rng_vals, &l2_distance);
        assert_eq!(graph.node_count(), n);
        assert!(graph.entry_point.is_some());
    }

    #[test]
    fn build_layer_a_from_graph() {
        let n = 100;
        let dim = 4;
        let vecs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..dim).map(|d| (i * dim + d) as f32).collect())
            .collect();
        let store = InMemoryVectorStore::new(vecs.clone());

        let config = HnswConfig::default();
        let rng_vals: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 100) as f64 / 100.0).collect();
        let graph = build_full_index(&store, n, &config, &rng_vals, &l2_distance);

        let centroids = vec![vecs[25].clone(), vecs[75].clone()];
        let assignments: Vec<u32> = (0..n).map(|i| if i < 50 { 0 } else { 1 }).collect();

        let layer_a = build_layer_a(&graph, &centroids, &assignments, n as u64);
        assert!(!layer_a.entry_points.is_empty());
        assert_eq!(layer_a.centroids.len(), 2);
        assert!(!layer_a.partition_map.is_empty());
    }

    #[test]
    fn build_layer_b_from_graph() {
        let n = 50;
        let dim = 4;
        let vecs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..dim).map(|d| (i * dim + d) as f32).collect())
            .collect();
        let store = InMemoryVectorStore::new(vecs);

        let config = HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 50,
        };
        let rng_vals: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 100) as f64 / 100.0).collect();
        let graph = build_full_index(&store, n, &config, &rng_vals, &l2_distance);

        // Mark first 25 nodes as hot.
        let hot: BTreeSet<u64> = (0..25).collect();
        let layer_b = build_layer_b(&graph, &hot);

        assert!(!layer_b.partial_adjacency.is_empty());
        assert!(layer_b.has_node(0));
        assert!(!layer_b.has_node(49));
    }

    #[test]
    fn compute_ranges_basic() {
        let ids: BTreeSet<u64> = [1, 2, 3, 5, 6, 10].into_iter().collect();
        let ranges = compute_ranges(&ids);
        assert_eq!(ranges, vec![(1, 4), (5, 7), (10, 11)]);
    }

    #[test]
    fn compute_ranges_empty() {
        let ids: BTreeSet<u64> = BTreeSet::new();
        assert!(compute_ranges(&ids).is_empty());
    }

    #[test]
    fn incremental_insert_works() {
        let n = 20;
        let dim = 4;
        let mut vecs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..dim).map(|d| (i * dim + d) as f32).collect())
            .collect();

        let store = InMemoryVectorStore::new(vecs.clone());
        let config = HnswConfig {
            m: 8,
            m0: 16,
            ef_construction: 50,
        };
        let rng_vals: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 100) as f64 / 100.0).collect();
        let mut graph = build_full_index(&store, n, &config, &rng_vals, &l2_distance);

        assert_eq!(graph.node_count(), n);

        // Add one more vector.
        vecs.push((0..dim).map(|d| (n * dim + d) as f32).collect());
        let store2 = InMemoryVectorStore::new(vecs);
        incremental_insert(&mut graph, n as u64, 0.5, &store2, &l2_distance);

        assert_eq!(graph.node_count(), n + 1);
    }
}
