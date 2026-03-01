//! Basic RVF Store — Getting Started
//!
//! Demonstrates the simplest possible RVF workflow:
//! 1. Create a store with RvfOptions (384 dims, L2 metric)
//! 2. Insert 100 random vectors with IDs
//! 3. Query top-5 nearest neighbors
//! 4. Print results (id + distance)
//! 5. Close and reopen the store
//! 6. Query again to show persistence
//! 7. Clean up

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore, SearchResult};
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// Simple pseudo-random number generator (LCG) for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn main() {
    println!("=== RVF Basic Store Example ===\n");

    let dim = 384;
    let num_vectors = 100;

    // -- Step 1: Create a temporary directory and a new store --
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("basic.rvf");

    // Note: We use L2 (squared Euclidean) metric here because the current
    // manifest format does not persist the metric choice. After reopening,
    // the store defaults to L2, so using L2 ensures consistency.
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    println!("Creating store at {:?}", store_path);
    println!("  Dimensions: {}", dim);
    println!("  Metric:     L2 (squared Euclidean)");
    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // -- Step 2: Insert 100 random vectors --
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_vectors as u64).collect();

    let ingest_result = store
        .ingest_batch(&vec_refs, &ids, None)
        .expect("failed to ingest batch");
    println!(
        "\nIngested {} vectors (rejected: {}, epoch: {})",
        ingest_result.accepted, ingest_result.rejected, ingest_result.epoch
    );

    // -- Step 3: Query top-5 nearest neighbors --
    let query_seed = 42;
    let query_vec = random_vector(dim, query_seed);
    let k = 5;

    let results = store
        .query(&query_vec, k, &QueryOptions::default())
        .expect("failed to query");

    println!("\nQuery (seed={}): top-{} nearest neighbors:", query_seed, k);
    print_results(&results);

    // -- Step 4: Show store status --
    let status = store.status();
    println!("\nStore status:");
    println!("  Total vectors: {}", status.total_vectors);
    println!("  File size:     {} bytes", status.file_size);
    println!("  Epoch:         {}", status.current_epoch);
    println!("  Segments:      {}", status.total_segments);

    // -- Step 5: Close the store --
    println!("\nClosing store...");
    store.close().expect("failed to close store");

    // -- Step 6: Reopen and query again to verify persistence --
    println!("Reopening store...");
    let reopened = RvfStore::open(&store_path).expect("failed to reopen store");

    let results_after = reopened
        .query(&query_vec, k, &QueryOptions::default())
        .expect("failed to query after reopen");

    println!("\nQuery after reopen: top-{} nearest neighbors:", k);
    print_results(&results_after);

    // Verify results match
    assert_eq!(
        results.len(),
        results_after.len(),
        "result count mismatch after reopen"
    );
    for (a, b) in results.iter().zip(results_after.iter()) {
        assert_eq!(a.id, b.id, "ID mismatch after reopen");
        assert!(
            (a.distance - b.distance).abs() < 1e-6,
            "distance mismatch after reopen"
        );
    }
    println!("\nPersistence verified: results match before and after reopen.");

    reopened.close().expect("failed to close reopened store");

    // -- Step 7: Clean up happens automatically (TempDir) --
    println!("\nDone. Temp directory will be cleaned up automatically.");
}

fn print_results(results: &[SearchResult]) {
    println!("  {:>6}  {:>12}", "ID", "Distance");
    println!("  {:->6}  {:->12}", "", "");
    for r in results {
        println!("  {:>6}  {:>12.6}", r.id, r.distance);
    }
}
