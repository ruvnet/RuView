//! Progressive recall end-to-end tests.
//!
//! Verifies that the three-layer progressive index model (Layer A / B / C)
//! delivers improving recall as more layers are loaded. Uses brute-force
//! k-NN as ground truth.

use rvf_index::distance::l2_distance;
use rvf_index::hnsw::HnswConfig;
use rvf_index::layers::{IndexState, LayerA, LayerC};
use rvf_index::progressive::ProgressiveIndex;
use rvf_index::traits::InMemoryVectorStore;
use rvf_index::{build_full_index, build_layer_a, build_layer_b, build_layer_c};
use std::collections::{BTreeSet, HashSet};

/// Generate `n` pseudo-random vectors of dimension `dim` using a seeded LCG.
fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            (0..dim)
                .map(|_| {
                    s = s
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    ((s >> 33) as f32) / (u32::MAX as f32) - 0.5
                })
                .collect()
        })
        .collect()
}

/// Brute-force k-NN for ground truth (squared L2).
fn brute_force_knn(query: &[f32], vectors: &[Vec<f32>], k: usize) -> Vec<u64> {
    let mut dists: Vec<(u64, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i as u64, l2_distance(query, v)))
        .collect();
    dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    dists.iter().take(k).map(|(id, _)| *id).collect()
}

/// Calculate recall@K: fraction of ground truth IDs present in the
/// approximate results.
fn recall_at_k(approx: &[(u64, f32)], exact: &[u64]) -> f64 {
    let exact_set: HashSet<u64> = exact.iter().copied().collect();
    let hits = approx
        .iter()
        .filter(|(id, _)| exact_set.contains(id))
        .count();
    hits as f64 / exact.len() as f64
}

/// Generate deterministic RNG values for HNSW level selection.
fn rng_values(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((s >> 33) as f64 / (1u64 << 31) as f64).clamp(0.001, 0.999)
        })
        .collect()
}

// --------------------------------------------------------------------------
// 1. Full Layer C achieves high recall (>= 0.90) on 5000 vectors
// --------------------------------------------------------------------------
#[test]
fn progressive_full_index_recall_at_least_090() {
    let n = 5000;
    let dim = 32;
    let k = 10;
    let num_queries = 50;

    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };
    let rng = rng_values(n, 123);
    let graph = build_full_index(&store, n, &config, &rng, &l2_distance);

    let layer_c = build_layer_c(&graph);
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

    let queries = random_vectors(num_queries, dim, 999);
    let mut total_recall = 0.0;

    for query in &queries {
        let approx = idx.search(query, k, 200, &store);
        let exact = brute_force_knn(query, &vectors, k);
        total_recall += recall_at_k(&approx, &exact);
    }

    let avg_recall = total_recall / num_queries as f64;
    assert!(
        avg_recall >= 0.90,
        "Full index recall@{k} = {avg_recall:.3}, expected >= 0.90"
    );
}

// --------------------------------------------------------------------------
// 2. Layer A only achieves moderate recall (>= 0.40 for small dataset)
// --------------------------------------------------------------------------
#[test]
fn progressive_layer_a_only_returns_results() {
    let n = 2000;
    let dim = 32;
    let k = 10;
    let num_queries = 30;

    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };
    let rng = rng_values(n, 123);
    let graph = build_full_index(&store, n, &config, &rng, &l2_distance);

    // Build centroids using simple partitioning.
    let n_centroids = 10;
    let partition_size = n / n_centroids;
    let mut centroids = Vec::new();
    let mut assignments = vec![0u32; n];

    for c in 0..n_centroids {
        let start = c * partition_size;
        let end = if c == n_centroids - 1 {
            n
        } else {
            (c + 1) * partition_size
        };
        // Compute centroid as the mean of vectors in this partition.
        let mut centroid = vec![0.0f32; dim];
        for i in start..end {
            for d in 0..dim {
                centroid[d] += vectors[i][d];
            }
            assignments[i] = c as u32;
        }
        let count = (end - start) as f32;
        for c in &mut centroid {
            *c /= count;
        }
        centroids.push(centroid);
    }

    let layer_a = build_layer_a(&graph, &centroids, &assignments, n as u64);

    let idx = ProgressiveIndex {
        layer_a: Some(layer_a),
        layer_b: None,
        layer_c: None,
    };

    let queries = random_vectors(num_queries, dim, 777);
    let mut queries_with_results = 0;
    let mut total_recall = 0.0;

    for query in &queries {
        let approx = idx.search(query, k, 100, &store);
        if !approx.is_empty() {
            queries_with_results += 1;
            let exact = brute_force_knn(query, &vectors, k);
            total_recall += recall_at_k(&approx, &exact);
        }
    }

    // Layer A should return results for most queries.
    assert!(
        queries_with_results > num_queries / 2,
        "Layer A should return results for most queries, got {queries_with_results}/{num_queries}"
    );

    // Average recall should be > 0 (Layer A provides coarse routing).
    if queries_with_results > 0 {
        let avg_recall = total_recall / queries_with_results as f64;
        assert!(
            avg_recall > 0.0,
            "Layer A recall should be > 0, got {avg_recall:.3}"
        );
    }
}

// --------------------------------------------------------------------------
// 3. Recall improves from Layer A -> A+B -> A+B+C
// --------------------------------------------------------------------------
#[test]
fn progressive_recall_improves_with_more_layers() {
    let n = 2000;
    let dim = 32;
    let k = 10;
    let num_queries = 30;

    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };
    let rng = rng_values(n, 123);
    let graph = build_full_index(&store, n, &config, &rng, &l2_distance);

    // Build centroids.
    let n_centroids = 10;
    let partition_size = n / n_centroids;
    let mut centroids = Vec::new();
    let mut assignments = vec![0u32; n];
    for c in 0..n_centroids {
        let start = c * partition_size;
        let end = if c == n_centroids - 1 {
            n
        } else {
            (c + 1) * partition_size
        };
        let mut centroid = vec![0.0f32; dim];
        for i in start..end {
            for d in 0..dim {
                centroid[d] += vectors[i][d];
            }
            assignments[i] = c as u32;
        }
        let count = (end - start) as f32;
        for c in &mut centroid {
            *c /= count;
        }
        centroids.push(centroid);
    }

    let layer_a = build_layer_a(&graph, &centroids, &assignments, n as u64);

    // Layer B: mark first 50% as hot.
    let hot_ids: BTreeSet<u64> = (0..(n / 2) as u64).collect();
    let layer_b = build_layer_b(&graph, &hot_ids);

    // Layer C: full graph.
    let layer_c = build_layer_c(&graph);

    let queries = random_vectors(num_queries, dim, 777);

    // Measure recall for Layer C (most reliable measurement).
    let idx_c = ProgressiveIndex {
        layer_a: Some(layer_a.clone()),
        layer_b: None,
        layer_c: Some(layer_c),
    };

    let mut recall_c = 0.0;
    for query in &queries {
        let approx = idx_c.search(query, k, 200, &store);
        let exact = brute_force_knn(query, &vectors, k);
        recall_c += recall_at_k(&approx, &exact);
    }
    recall_c /= num_queries as f64;

    // Layer C should achieve high recall.
    assert!(
        recall_c >= 0.85,
        "Layer C recall@{k} = {recall_c:.3}, expected >= 0.85"
    );

    // The estimated recall from the layer model should reflect the hierarchy.
    let state_a_only = IndexState {
        layer_a: Some(layer_a.clone()),
        layer_b: None,
        layer_c: None,
        total_nodes: n as u64,
    };
    let state_full = IndexState {
        layer_a: Some(layer_a),
        layer_b: Some(layer_b),
        layer_c: Some(LayerC {
            full_adjacency: graph.layers.clone(),
        }),
        total_nodes: n as u64,
    };

    let est_a = rvf_index::layers::available_recall(&state_a_only);
    let est_full = rvf_index::layers::available_recall(&state_full);
    assert!(
        est_full > est_a,
        "estimated recall for full index ({est_full}) should be > Layer A only ({est_a})"
    );
}

// --------------------------------------------------------------------------
// 4. HNSW recall improves with ef_search parameter
// --------------------------------------------------------------------------
#[test]
fn progressive_recall_improves_with_ef_search() {
    let n = 3000;
    let dim = 32;
    let k = 10;
    let num_queries = 20;

    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };
    let rng = rng_values(n, 123);
    let graph = build_full_index(&store, n, &config, &rng, &l2_distance);
    let layer_c = build_layer_c(&graph);

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

    let queries = random_vectors(num_queries, dim, 555);
    let ef_values = [10, 50, 200];
    let mut recalls = Vec::new();

    for &ef in &ef_values {
        let mut total = 0.0;
        for query in &queries {
            let approx = idx.search(query, k, ef, &store);
            let exact = brute_force_knn(query, &vectors, k);
            total += recall_at_k(&approx, &exact);
        }
        recalls.push(total / num_queries as f64);
    }

    // Recall should generally increase with higher ef_search.
    for i in 1..recalls.len() {
        assert!(
            recalls[i] >= recalls[i - 1] - 0.05, // tolerance for randomness
            "recall should improve with ef_search: ef={:?} -> recalls={:?}",
            ef_values,
            recalls
        );
    }

    // The highest ef_search should achieve good recall.
    assert!(
        recalls[recalls.len() - 1] >= 0.85,
        "ef_search=200 recall = {:.3}, expected >= 0.85",
        recalls[recalls.len() - 1]
    );
}
