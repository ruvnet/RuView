//! Serverless Cold-Start Optimized
//!
//! Category: Runtime Targets
//!
//! Demonstrates how RVF files are optimized for serverless function
//! cold starts. The manifest-based tail scan enables sub-5ms boot:
//! the runtime reads only the last segment (manifest) to discover the
//! full file layout, then progressively loads vectors on demand.
//!
//! Lifecycle demonstrated:
//!   1. Deployment: create an RVF file with vectors (done once)
//!   2. Cold start: open existing file (manifest tail scan)
//!   3. Handle request: query with filter
//!   4. Audit: add witness chain entry for each request
//!   5. Warm request: query again (vectors already loaded)
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore), wire-level
//! tail_scan (via rvf_wire::find_latest_manifest), WITNESS_SEG (via
//! rvf_crypto)
//!
//! Run with:
//!   cargo run --example serverless_function

use std::time::Instant;

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
use rvf_wire::tail_scan::find_latest_manifest;
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
    println!("=== RVF Serverless Cold-Start Optimized ===\n");

    let dim = 128;
    let num_vectors = 500;
    let k = 5;

    // ====================================================================
    // 1. DEPLOYMENT PHASE: Create the RVF file with vectors
    // ====================================================================
    println!("--- 1. Deployment Phase: Create RVF Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("serverless.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // Insert vectors with metadata (representing a product catalog)
    // field_id 0: category (String: "electronics", "clothing", "food", "books")
    // field_id 1: price_tier (U64: 1=budget, 2=mid, 3=premium)
    let categories = ["electronics", "clothing", "food", "books"];
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    let batch_size = 100;
    let num_batches = num_vectors / batch_size;

    for batch_idx in 0..num_batches {
        let start = batch_idx * batch_size;
        let end = start + batch_size;
        let batch_vecs: Vec<&[f32]> = vectors[start..end]
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let batch_ids: Vec<u64> = (start as u64..end as u64).collect();

        let mut metadata = Vec::with_capacity(batch_size * 2);
        for i in start..end {
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(categories[i % 4].to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(((i * 3 + 1) % 3 + 1) as u64),
            });
        }

        store
            .ingest_batch(&batch_vecs, &batch_ids, Some(&metadata))
            .expect("failed to ingest batch");
    }

    let deploy_status = store.status();
    println!("  Vectors:    {}", deploy_status.total_vectors);
    println!("  Segments:   {}", deploy_status.total_segments);
    println!("  File size:  {} bytes ({:.1} KB)", deploy_status.file_size, deploy_status.file_size as f64 / 1024.0);
    println!("  Epoch:      {}", deploy_status.current_epoch);

    // Close the store (represents deployment upload)
    store.close().expect("failed to close store");
    println!("  Store closed (deployment artifact ready).");

    // ====================================================================
    // 2. COLD START: Reopen the store (demonstrates function boot)
    // ====================================================================
    println!("\n--- 2. Cold Start: Manifest Tail Scan ---");

    // Read the raw file to demonstrate tail scan timing
    let file_bytes = std::fs::read(&store_path).expect("failed to read file");

    let scan_start = Instant::now();
    let (manifest_offset, manifest_header) = find_latest_manifest(&file_bytes)
        .expect("manifest not found");
    let scan_elapsed = scan_start.elapsed();

    println!("  File size:        {} bytes", file_bytes.len());
    println!("  Manifest found:   offset {} (seg_id={})", manifest_offset, manifest_header.segment_id);
    println!("  Manifest payload: {} bytes", manifest_header.payload_length);
    println!("  Tail scan time:   {:?}", scan_elapsed);
    println!("  Scan strategy:    backward from EOF at 64-byte boundaries");

    // Now open via RvfStore (which internally does the same tail scan + boot)
    let cold_start = Instant::now();
    let cold_store = RvfStore::open(&store_path).expect("failed to open store");
    let cold_elapsed = cold_start.elapsed();

    let cold_status = cold_store.status();
    println!("\n  Full cold start (open + load vectors):");
    println!("    Time:           {:?}", cold_elapsed);
    println!("    Vectors loaded: {}", cold_status.total_vectors);
    println!("    Epoch:          {}", cold_status.current_epoch);

    // ====================================================================
    // 3. HANDLE REQUEST: Query with filter
    // ====================================================================
    println!("\n--- 3. Request: Filtered Query ---");

    let mut witness_entries: Vec<WitnessEntry> = Vec::new();
    let base_timestamp = 1_700_000_000_000_000_000u64;

    // Request 1: Find electronics in premium tier
    let query1 = random_vector(dim, 42);
    let filter1 = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("electronics".to_string())),
        FilterExpr::Eq(1, FilterValue::U64(3)), // premium
    ]);
    let opts1 = QueryOptions {
        filter: Some(filter1),
        ..Default::default()
    };

    let req1_start = Instant::now();
    let results1 = cold_store.query(&query1, k, &opts1).expect("query failed");
    let req1_elapsed = req1_start.elapsed();

    println!("  Request 1: electronics + premium tier");
    println!("    Query time: {:?}", req1_elapsed);
    println!("    Results:    {} of {}", results1.len(), k);
    print_results_with_meta(&results1, &categories);

    // Record witness entry for request 1
    let action1 = format!(
        "SEARCH:filter=electronics+premium:k={}:results={}",
        k,
        results1.len()
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(action1.as_bytes()),
        timestamp_ns: base_timestamp,
        witness_type: 0x02, // COMPUTATION
    });

    // ====================================================================
    // 4. WARM REQUEST: Query again (index already loaded)
    // ====================================================================
    println!("\n--- 4. Warm Request (Vectors Already Loaded) ---");

    // Request 2: Find food items in budget tier
    let query2 = random_vector(dim, 77);
    let filter2 = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("food".to_string())),
        FilterExpr::Eq(1, FilterValue::U64(1)), // budget
    ]);
    let opts2 = QueryOptions {
        filter: Some(filter2),
        ..Default::default()
    };

    let req2_start = Instant::now();
    let results2 = cold_store.query(&query2, k, &opts2).expect("query failed");
    let req2_elapsed = req2_start.elapsed();

    println!("  Request 2: food + budget tier");
    println!("    Query time: {:?} (warm path)", req2_elapsed);
    println!("    Results:    {} of {}", results2.len(), k);
    print_results_with_meta(&results2, &categories);

    // Record witness entry for request 2
    let action2 = format!(
        "SEARCH:filter=food+budget:k={}:results={}",
        k,
        results2.len()
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(action2.as_bytes()),
        timestamp_ns: base_timestamp + 1_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // Request 3: Unfiltered query
    let query3 = random_vector(dim, 123);
    let req3_start = Instant::now();
    let results3 = cold_store
        .query(&query3, k, &QueryOptions::default())
        .expect("query failed");
    let req3_elapsed = req3_start.elapsed();

    println!("\n  Request 3: unfiltered (all categories)");
    println!("    Query time: {:?} (warm path)", req3_elapsed);
    println!("    Results:    {} of {}", results3.len(), k);
    print_results_with_meta(&results3, &categories);

    // Record witness entry for request 3
    let action3 = format!(
        "SEARCH:unfiltered:k={}:results={}",
        k,
        results3.len()
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(action3.as_bytes()),
        timestamp_ns: base_timestamp + 2_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // ====================================================================
    // 5. AUDIT: Witness chain for request tracking
    // ====================================================================
    println!("\n--- 5. Witness Audit Trail ---");

    let chain_bytes = create_witness_chain(&witness_entries);
    println!(
        "  Created witness chain: {} entries, {} bytes",
        witness_entries.len(),
        chain_bytes.len()
    );

    match verify_witness_chain(&chain_bytes) {
        Ok(verified) => {
            println!(
                "  Chain integrity: VALID ({} entries verified)\n",
                verified.len()
            );
            println!(
                "  {:>5}  {:>8}  {:>20}  {:>32}",
                "Index", "Type", "Timestamp (ns)", "Action Hash (first 16 bytes)"
            );
            println!("  {:->5}  {:->8}  {:->20}  {:->32}", "", "", "", "");
            for (i, entry) in verified.iter().enumerate() {
                let wtype = match entry.witness_type {
                    0x01 => "PROV",
                    0x02 => "SEARCH",
                    _ => "????",
                };
                let hash_hex: String = entry
                    .action_hash[..16]
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                println!(
                    "  {:>5}  {:>8}  {:>20}  {}",
                    i, wtype, entry.timestamp_ns, hash_hex
                );
            }
        }
        Err(e) => println!("  Chain integrity: FAILED ({:?})", e),
    }

    // ====================================================================
    // 6. Progressive loading concept (Layer A instant results)
    // ====================================================================
    println!("\n--- 6. Progressive Loading Concept ---");
    println!("  In production serverless with large RVF files:");
    println!("    Layer A (manifest + entry points): instant, < 1ms");
    println!("      -> Provides coarse routing to correct partition");
    println!("    Layer B (hot region adjacency):    fast, < 5ms");
    println!("      -> Covers frequently-accessed vectors");
    println!("    Layer C (full HNSW graph):         deferred, < 50ms");
    println!("      -> Full recall, loaded in background");
    println!("  This example uses flat scan (all layers loaded at open).");
    println!("  For large datasets, progressive loading enables instant");
    println!("  results even before the full index is in memory.");

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Serverless Function Summary ===\n");
    println!("  {:>28}  {:>16}", "Metric", "Value");
    println!("  {:->28}  {:->16}", "", "");
    println!("  {:>28}  {:>16}", "Vectors", num_vectors);
    println!("  {:>28}  {:>16}", "Dimensions", dim);
    println!("  {:>28}  {:>14.1} KB", "File size", deploy_status.file_size as f64 / 1024.0);
    println!("  {:>28}  {:>16?}", "Tail scan time", scan_elapsed);
    println!("  {:>28}  {:>16?}", "Full cold start", cold_elapsed);
    println!("  {:>28}  {:>16?}", "Request 1 (cold, filtered)", req1_elapsed);
    println!("  {:>28}  {:>16?}", "Request 2 (warm, filtered)", req2_elapsed);
    println!("  {:>28}  {:>16?}", "Request 3 (warm, unfiltered)", req3_elapsed);
    println!("  {:>28}  {:>16}", "Witness entries", witness_entries.len());

    cold_store.close().expect("failed to close store");

    println!("\nDone.");
}

fn print_results_with_meta(results: &[SearchResult], categories: &[&str]) {
    println!("    {:>6}  {:>12}  {:>14}  {:>10}", "ID", "Distance", "Category", "Price Tier");
    println!("    {:->6}  {:->12}  {:->14}  {:->10}", "", "", "", "");
    for r in results {
        let cat = categories[(r.id as usize) % 4];
        let tier = match ((r.id as usize) * 3 + 1) % 3 + 1 {
            1 => "budget",
            2 => "mid",
            3 => "premium",
            _ => "?",
        };
        println!(
            "    {:>6}  {:>12.6}  {:>14}  {:>10}",
            r.id, r.distance, cat, tier
        );
    }
}
