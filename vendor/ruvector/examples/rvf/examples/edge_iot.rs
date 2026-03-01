//! IoT / Edge Device Pattern
//!
//! Category: Runtime Targets
//!
//! Demonstrates how RVF operates on constrained edge and IoT devices with
//! limited memory and compute. Uses small dimensions (32), limited vector
//! count (200), and binary quantization to minimize storage. No encryption
//! or signing overhead (resource-constrained environment).
//!
//! This mirrors the rvlite-style minimal API: insert, query, close. No
//! index building, no progressive loading -- just flat brute-force scan
//! optimized for small datasets on microcontroller-class hardware.
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore)
//! Quantization: Binary (rvf-quant encode_binary / hamming_distance)
//!
//! Run with:
//!   cargo run --example edge_iot

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore, SearchResult};
use rvf_runtime::options::DistanceMetric;
use rvf_quant::{encode_binary, hamming_distance};
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
    println!("=== RVF IoT / Edge Device Pattern ===\n");

    let dim = 32;
    let num_vectors = 200;
    let k = 5;

    // ====================================================================
    // 1. Constrained device configuration
    // ====================================================================
    println!("--- 1. Edge Device Configuration ---");
    println!("  Dimensions:     {} (small for MCU targets)", dim);
    println!("  Vector count:   {} (limited by SRAM)", num_vectors);
    println!("  Metric:         L2 (no cosine normalization overhead)");
    println!("  Encryption:     none (resource-constrained)");
    println!("  Signing:        none (resource-constrained)");
    println!("  Index type:     flat scan (no HNSW overhead)");

    // ====================================================================
    // 2. Create store (rvlite-style minimal API)
    // ====================================================================
    println!("\n--- 2. Create Store (rvlite Minimal API) ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("edge.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // ====================================================================
    // 3. Insert vectors in batches (sensor embeddings)
    // ====================================================================
    println!("\n--- 3. Batch Insert (Sensor Embeddings) ---");

    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    // Insert in small batches (representing periodic sensor reads)
    let batch_size = 50;
    let num_batches = num_vectors / batch_size;

    for batch_idx in 0..num_batches {
        let start = batch_idx * batch_size;
        let end = start + batch_size;
        let batch_vecs: Vec<&[f32]> = vectors[start..end]
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let batch_ids: Vec<u64> = (start as u64..end as u64).collect();

        let result = store
            .ingest_batch(&batch_vecs, &batch_ids, None)
            .expect("failed to ingest batch");
        println!(
            "  Batch {}: inserted {} vectors (epoch {})",
            batch_idx, result.accepted, result.epoch
        );
    }

    // ====================================================================
    // 4. Query (edge inference: find similar sensor readings)
    // ====================================================================
    println!("\n--- 4. Edge Query (Anomaly Detection) ---");

    let query = random_vector(dim, 99);
    let results = store
        .query(&query, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Query seed: 99 (synthetic sensor reading)");
    println!("  Top-{} nearest sensor patterns:", k);
    print_results(&results);

    // ====================================================================
    // 5. Binary quantization (minimize storage on flash)
    // ====================================================================
    println!("\n--- 5. Binary Quantization (Flash Storage) ---");

    let raw_bytes_per_vec = dim * 4; // fp32
    let raw_total = num_vectors * raw_bytes_per_vec;

    // Binary quantize all vectors
    let binary_vectors: Vec<Vec<u8>> = vectors.iter()
        .map(|v| encode_binary(v))
        .collect();

    let bin_bytes_per_vec = binary_vectors[0].len();
    let bin_total = num_vectors * bin_bytes_per_vec;
    let compression_ratio = raw_total as f64 / bin_total as f64;

    println!("  Raw fp32:");
    println!("    Per vector:   {} bytes ({} dims x 4)", raw_bytes_per_vec, dim);
    println!("    Total:        {} bytes ({:.1} KB)", raw_total, raw_total as f64 / 1024.0);
    println!("  Binary quantized:");
    println!("    Per vector:   {} bytes ({} dims / 8)", bin_bytes_per_vec, dim);
    println!("    Total:        {} bytes ({:.1} KB)", bin_total, bin_total as f64 / 1024.0);
    println!("  Compression:    {:.1}x", compression_ratio);

    // ====================================================================
    // 6. Hamming distance search (binary-quantized query)
    // ====================================================================
    println!("\n--- 6. Hamming Distance Search (Binary Domain) ---");

    let query_binary = encode_binary(&query);

    // Brute-force Hamming search over binary vectors
    let mut ham_results: Vec<(u64, u32)> = binary_vectors
        .iter()
        .enumerate()
        .map(|(i, bv)| (i as u64, hamming_distance(&query_binary, bv)))
        .collect();
    ham_results.sort_by_key(|&(_, d)| d);

    println!("  Top-{} by Hamming distance (binary search):", k);
    println!("  {:>6}  {:>12}", "ID", "Hamming Dist");
    println!("  {:->6}  {:->12}", "", "");
    for &(id, dist) in ham_results.iter().take(k) {
        println!("  {:>6}  {:>12}", id, dist);
    }

    // Compare with L2 results
    println!("\n  Overlap with L2 top-{}:", k);
    let l2_ids: Vec<u64> = results.iter().map(|r| r.id).collect();
    let ham_ids: Vec<u64> = ham_results.iter().take(k).map(|&(id, _)| id).collect();
    let overlap = l2_ids.iter().filter(|id| ham_ids.contains(id)).count();
    println!("    L2 top-{}: {:?}", k, l2_ids);
    println!("    Hamming top-{}: {:?}", k, ham_ids);
    println!("    Overlap: {}/{} ({:.0}%)", overlap, k, overlap as f64 / k as f64 * 100.0);

    // ====================================================================
    // 7. Memory footprint analysis
    // ====================================================================
    println!("\n--- 7. Memory Footprint Analysis ---");

    let status = store.status();
    let rvf_file_size = status.file_size;

    println!("  {:>28}  {:>10}  {:>10}", "Storage", "Bytes", "KB");
    println!("  {:->28}  {:->10}  {:->10}", "", "", "");
    println!(
        "  {:>28}  {:>10}  {:>10.1}",
        "Raw fp32 vectors",
        raw_total,
        raw_total as f64 / 1024.0
    );
    println!(
        "  {:>28}  {:>10}  {:>10.1}",
        "Binary quantized vectors",
        bin_total,
        bin_total as f64 / 1024.0
    );
    println!(
        "  {:>28}  {:>10}  {:>10.1}",
        "RVF file (with segments)",
        rvf_file_size,
        rvf_file_size as f64 / 1024.0
    );
    println!(
        "  {:>28}  {:>10}  {:>10.1}",
        "Segment overhead",
        rvf_file_size as usize - raw_total,
        (rvf_file_size as usize - raw_total) as f64 / 1024.0
    );

    // Typical MCU SRAM budgets
    println!("\n  Typical edge device SRAM budgets:");
    println!("    ESP32:           520 KB  -- fits {} fp32 vectors", 520 * 1024 / raw_bytes_per_vec);
    println!("    STM32H7:         1 MB    -- fits {} fp32 vectors", 1024 * 1024 / raw_bytes_per_vec);
    println!("    Raspberry Pi:    varies  -- fits {} fp32 vectors (1 MB budget)", 1024 * 1024 / raw_bytes_per_vec);
    println!("    With binary PQ:  520 KB  -- fits {} binary vectors", 520 * 1024 / bin_bytes_per_vec);

    // ====================================================================
    // 8. Close (rvlite lifecycle: insert -> query -> close)
    // ====================================================================
    println!("\n--- 8. Close Store ---");
    store.close().expect("failed to close store");
    println!("  Store closed. rvlite lifecycle complete.");

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Edge Device Summary ===\n");
    println!("  {:>24}  {:>12}", "Metric", "Value");
    println!("  {:->24}  {:->12}", "", "");
    println!("  {:>24}  {:>12}", "Vectors", num_vectors);
    println!("  {:>24}  {:>12}", "Dimensions", dim);
    println!("  {:>24}  {:>10} B", "Raw per vector", raw_bytes_per_vec);
    println!("  {:>24}  {:>10} B", "Binary per vector", bin_bytes_per_vec);
    println!("  {:>24}  {:>10.1}x", "Compression ratio", compression_ratio);
    println!("  {:>24}  {:>10.1} KB", "Raw total", raw_total as f64 / 1024.0);
    println!("  {:>24}  {:>10.1} KB", "Binary total", bin_total as f64 / 1024.0);
    println!("  {:>24}  {:>10.1} KB", "RVF file size", rvf_file_size as f64 / 1024.0);

    println!("\nDone.");
}

fn print_results(results: &[SearchResult]) {
    println!("  {:>6}  {:>12}", "ID", "Distance");
    println!("  {:->6}  {:->12}", "", "");
    for r in results {
        println!("  {:>6}  {:>12.6}", r.id, r.distance);
    }
}
