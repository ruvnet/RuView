//! Progressive HNSW Index — Layer A / B / C
//!
//! Demonstrates the three-layer progressive indexing model:
//! 1. Build a full HNSW graph with 5000 random vectors (128 dims)
//! 2. Build Layer A (entry points + centroids) and search
//! 3. Build Layer B (hot region) and search with A+B
//! 4. Build Layer C (full graph) and search with A+B+C
//! 5. Show how recall improves at each stage

use std::collections::BTreeSet;

use rvf_index::{
    build_full_index, build_layer_a, build_layer_b, build_layer_c,
    HnswConfig, InMemoryVectorStore, ProgressiveIndex,
    l2_distance,
};

/// LCG-based pseudo-random vector generator for deterministic results.
fn random_vectors(n: usize, dim: usize, base_seed: u64) -> Vec<Vec<f32>> {
    let mut seed = base_seed;
    (0..n)
        .map(|_| {
            (0..dim)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    ((seed >> 33) as f32) / (u32::MAX as f32) - 0.5
                })
                .collect()
        })
        .collect()
}

/// Compute brute-force top-k for a single query.
fn brute_force_top_k(query: &[f32], vectors: &[Vec<f32>], k: usize) -> BTreeSet<u64> {
    let mut dists: Vec<(u64, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i as u64, l2_distance(query, v)))
        .collect();
    dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    dists.iter().take(k).map(|&(id, _)| id).collect()
}

/// Measure recall@k for the progressive index vs brute force.
fn measure_recall(
    index: &ProgressiveIndex,
    store: &InMemoryVectorStore,
    vectors: &[Vec<f32>],
    k: usize,
    ef_search: usize,
    num_queries: usize,
    query_seed: u64,
) -> f64 {
    let queries = random_vectors(num_queries, vectors[0].len(), query_seed);
    let mut total_recall = 0.0;

    for query in &queries {
        let gt = brute_force_top_k(query, vectors, k);
        let results = index.search(query, k, ef_search, store);
        let result_ids: BTreeSet<u64> = results.iter().map(|&(id, _)| id).collect();
        let overlap = gt.intersection(&result_ids).count();
        total_recall += overlap as f64 / k as f64;
    }

    total_recall / num_queries as f64
}

fn main() {
    println!("=== RVF Progressive Index Example ===\n");

    let n = 5000;
    let dim = 128;
    let k = 10;
    let ef_search = 200;
    let num_queries = 50;

    // -- Step 1: Generate random vectors --
    println!("Generating {} random vectors ({} dims)...", n, dim);
    let vectors = random_vectors(n, dim, 42);
    let store = InMemoryVectorStore::new(vectors.clone());

    // -- Step 2: Build full HNSW graph --
    println!("Building full HNSW graph (M=16, ef_construction=200)...");
    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };

    // Generate deterministic RNG values for level selection.
    let mut rng_seed: u64 = 12345;
    let rng_values: Vec<f64> = (0..n)
        .map(|_| {
            rng_seed = rng_seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let val = (rng_seed >> 33) as f64 / (1u64 << 31) as f64;
            val.clamp(0.001, 0.999)
        })
        .collect();

    let graph = build_full_index(&store, n, &config, &rng_values, &l2_distance);
    println!(
        "  Graph built: {} nodes, max_layer={}, entry_point={:?}",
        graph.node_count(),
        graph.max_layer,
        graph.entry_point,
    );

    // -- Step 3: Build Layer A (entry points + centroids) --
    println!("\n--- Layer A: Entry Points + Coarse Routing ---");

    // Simple 2-centroid clustering: split vectors in half.
    let mid = n / 2;
    let centroid_0: Vec<f32> = (0..dim)
        .map(|d| vectors[..mid].iter().map(|v| v[d]).sum::<f32>() / mid as f32)
        .collect();
    let centroid_1: Vec<f32> = (0..dim)
        .map(|d| vectors[mid..].iter().map(|v| v[d]).sum::<f32>() / (n - mid) as f32)
        .collect();
    let centroids = vec![centroid_0, centroid_1];
    let assignments: Vec<u32> = (0..n).map(|i| if i < mid { 0 } else { 1 }).collect();

    let layer_a = build_layer_a(&graph, &centroids, &assignments, n as u64);
    println!(
        "  Entry points: {}, Centroids: {}, Partitions: {}",
        layer_a.entry_points.len(),
        layer_a.centroids.len(),
        layer_a.partition_map.len(),
    );

    let index_a = ProgressiveIndex {
        layer_a: Some(layer_a.clone()),
        layer_b: None,
        layer_c: None,
    };

    let recall_a = measure_recall(&index_a, &store, &vectors, k, ef_search, num_queries, 999);
    println!("  Recall@{}: {:.3}", k, recall_a);

    // -- Step 4: Build Layer B (hot region) --
    println!("\n--- Layer A + B: Hot Region Partial Adjacency ---");

    // Mark the first 30% of nodes as "hot".
    let hot_count = (n as f64 * 0.30) as u64;
    let hot_nodes: BTreeSet<u64> = (0..hot_count).collect();
    let layer_b = build_layer_b(&graph, &hot_nodes);
    println!(
        "  Hot nodes: {}, Covered ranges: {}",
        layer_b.partial_adjacency.len(),
        layer_b.covered_ranges.len(),
    );

    let index_ab = ProgressiveIndex {
        layer_a: Some(layer_a.clone()),
        layer_b: Some(layer_b.clone()),
        layer_c: None,
    };

    let recall_ab = measure_recall(&index_ab, &store, &vectors, k, ef_search, num_queries, 999);
    println!("  Recall@{}: {:.3}", k, recall_ab);

    // -- Step 5: Build Layer C (full graph) --
    println!("\n--- Layer A + B + C: Full HNSW Graph ---");

    let layer_c = build_layer_c(&graph);
    println!(
        "  Full adjacency layers: {}",
        layer_c.full_adjacency.len(),
    );

    let index_abc = ProgressiveIndex {
        layer_a: Some(layer_a),
        layer_b: Some(layer_b),
        layer_c: Some(layer_c),
    };

    let recall_abc =
        measure_recall(&index_abc, &store, &vectors, k, ef_search, num_queries, 999);
    println!("  Recall@{}: {:.3}", k, recall_abc);

    // -- Summary --
    println!("\n=== Recall Progression Summary ===");
    println!("  {:>12}  {:>10}", "Layers", "Recall@10");
    println!("  {:->12}  {:->10}", "", "");
    println!("  {:>12}  {:>10.3}", "A only", recall_a);
    println!("  {:>12}  {:>10.3}", "A + B", recall_ab);
    println!("  {:>12}  {:>10.3}", "A + B + C", recall_abc);
    println!();

    if recall_abc > recall_ab && recall_ab >= recall_a {
        println!("  Recall improved progressively as expected.");
    } else {
        println!("  Note: recall progression may vary with random data.");
    }

    println!("\nDone.");
}
