//! Index recall integration tests.
//!
//! Tests the rvf-index HNSW graph to verify recall@K targets.

use rvf_index::distance::{cosine_distance, dot_product, l2_distance};
use rvf_index::hnsw::{HnswConfig, HnswGraph};
use rvf_index::traits::InMemoryVectorStore;

/// Generate `n` pseudo-random vectors of dimension `dim` using a simple LCG.
fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            (0..dim)
                .map(|_| {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (s >> 33) as f32 / (1u64 << 31) as f32
                })
                .collect()
        })
        .collect()
}

/// Brute-force k-NN for ground truth (using squared L2).
fn brute_force_knn(query: &[f32], vectors: &[Vec<f32>], k: usize) -> Vec<u64> {
    let mut distances: Vec<(u64, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i as u64, l2_distance(query, v)))
        .collect();
    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    distances.iter().take(k).map(|(i, _)| *i).collect()
}

/// Calculate recall@K.
fn recall_at_k(approx: &[(u64, f32)], exact: &[u64]) -> f64 {
    let exact_set: std::collections::HashSet<u64> = exact.iter().copied().collect();
    let hits = approx
        .iter()
        .filter(|(id, _)| exact_set.contains(id))
        .count();
    hits as f64 / exact.len() as f64
}

#[test]
fn hnsw_build_and_query_recall() {
    let dim = 32;
    let n = 1000;
    let k = 10;
    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };

    let mut graph = HnswGraph::new(&config);

    // Insert all vectors.
    let mut rng_seed: u64 = 123;
    for i in 0..n as u64 {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rng_val = ((rng_seed >> 33) as f64 / (1u64 << 31) as f64).clamp(0.001, 0.999);
        graph.insert(i, rng_val, &store, &l2_distance);
    }

    // Run 50 queries and measure average recall.
    let queries = random_vectors(50, dim, 999);
    let mut total_recall = 0.0;

    for query in &queries {
        let approx_results = graph.search(query, k, 200, &store, &l2_distance);
        let exact_results = brute_force_knn(query, &vectors, k);
        total_recall += recall_at_k(&approx_results, &exact_results);
    }

    let avg_recall = total_recall / queries.len() as f64;
    assert!(
        avg_recall >= 0.90,
        "HNSW recall@{k} = {avg_recall:.3}, expected >= 0.90"
    );
}

#[test]
fn hnsw_recall_improves_with_ef_search() {
    let dim = 32;
    let n = 500;
    let k = 10;
    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };

    let mut graph = HnswGraph::new(&config);
    let mut rng_seed: u64 = 77;
    for i in 0..n as u64 {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rng_val = ((rng_seed >> 33) as f64 / (1u64 << 31) as f64).clamp(0.001, 0.999);
        graph.insert(i, rng_val, &store, &l2_distance);
    }

    let queries = random_vectors(20, dim, 555);

    let mut recalls = Vec::new();
    for ef_search in [10, 50, 200] {
        let mut total = 0.0;
        for query in &queries {
            let approx = graph.search(query, k, ef_search, &store, &l2_distance);
            let exact = brute_force_knn(query, &vectors, k);
            total += recall_at_k(&approx, &exact);
        }
        recalls.push(total / queries.len() as f64);
    }

    // Recall should generally increase with higher ef_search.
    for i in 1..recalls.len() {
        assert!(
            recalls[i] >= recalls[i - 1] - 0.05, // tolerance for randomness
            "recall should improve with ef_search: {:?}",
            recalls
        );
    }
}

#[test]
fn distance_functions_are_consistent() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![5.0, 6.0, 7.0, 8.0];

    // l2_distance returns squared L2 (no sqrt).
    let l2 = l2_distance(&a, &b);
    let expected_sq = 4.0 * 4.0 + 4.0 * 4.0 + 4.0 * 4.0 + 4.0 * 4.0;
    assert!(
        (l2 - expected_sq).abs() < 1e-5,
        "L2 squared distance mismatch: {l2} != {expected_sq}"
    );

    // dot_product returns -dot(a,b).
    let dp = dot_product(&a, &b);
    let expected_dot = -(1.0 * 5.0 + 2.0 * 6.0 + 3.0 * 7.0 + 4.0 * 8.0);
    assert!(
        (dp - expected_dot).abs() < 1e-5,
        "dot product mismatch: {dp} != {expected_dot}"
    );

    // cosine_distance returns 1 - cosine_similarity.
    let cos = cosine_distance(&a, &b);
    assert!(
        (0.0..=2.0).contains(&cos),
        "cosine distance out of range: {cos}"
    );
}
