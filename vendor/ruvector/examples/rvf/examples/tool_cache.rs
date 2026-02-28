//! Tool Call Result Cache — Agentic AI
//!
//! Demonstrates using an RVF store as a semantic cache for tool invocation results:
//! 1. Create a store for caching tool call input embeddings
//! 2. Insert embeddings with metadata: tool_name, call_hash, ttl_seconds, result_size
//! 3. Query to find cached results for similar tool calls
//! 4. Demonstrate metadata filtering: find all "search_web" results with ttl > 3600
//! 5. Show cache hit vs miss patterns
//! 6. Delete expired entries with delete_by_filter
//! 7. Compact to reclaim space
//! 8. Print cache stats
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, JOURNAL_SEG (via RvfStore)
//!
//! Run with:
//!   cargo run --example tool_cache

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
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
    println!("=== RVF Tool Call Result Cache Example ===\n");

    let dim = 64;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("tool_cache.rvf");

    // -- Step 1: Create tool cache store --
    println!("--- 1. Creating Tool Cache Store ---");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Cache store created ({} dims, L2 metric)", dim);

    // -- Step 2: Insert cached tool call results --
    // Metadata fields:
    //   field_id 0: tool_name    (String)
    //   field_id 1: call_hash    (U64: hash of the call arguments)
    //   field_id 2: ttl_seconds  (U64: time-to-live)
    //   field_id 3: result_size  (U64: size of cached result in bytes)
    println!("\n--- 2. Populating Tool Cache ---");

    let tools = [
        ("search_web", 7200u64, 4096u64),     // 2h TTL, ~4KB results
        ("read_file", 86400, 1024),            // 24h TTL, ~1KB results
        ("execute_code", 300, 2048),           // 5min TTL, ~2KB results
        ("query_database", 1800, 8192),        // 30min TTL, ~8KB results
        ("call_api", 3600, 512),               // 1h TTL, ~512B results
    ];

    let entries_per_tool = 10;
    let total_entries = tools.len() * entries_per_tool;
    let mut next_id: u64 = 0;

    for (tool_name, base_ttl, base_result_size) in &tools {
        let vectors: Vec<Vec<f32>> = (0..entries_per_tool)
            .map(|i| random_vector(dim, next_id + i as u64))
            .collect();
        let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (next_id..next_id + entries_per_tool as u64).collect();

        let mut metadata = Vec::with_capacity(entries_per_tool * 4);
        for i in 0..entries_per_tool {
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(tool_name.to_string()),
            });
            // Deterministic call hash
            let call_hash = (next_id + i as u64) * 0xDEADBEEF;
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(call_hash),
            });
            // Vary TTL slightly per entry
            let ttl = base_ttl + (i as u64) * 100;
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(ttl),
            });
            // Vary result size
            let result_size = base_result_size + (i as u64) * 128;
            metadata.push(MetadataEntry {
                field_id: 3,
                value: MetadataValue::U64(result_size),
            });
        }

        store
            .ingest_batch(&vec_refs, &ids, Some(&metadata))
            .expect("failed to ingest cache entries");

        println!(
            "  Tool '{}': {} entries cached (base TTL: {}s)",
            tool_name, entries_per_tool, base_ttl
        );

        next_id += entries_per_tool as u64;
    }

    println!("  Total cache entries: {}", total_entries);

    // -- Step 3: Cache lookup (similarity search) --
    println!("\n--- 3. Cache Lookup (Semantic Similarity) ---");

    // Look up a cached result for a search_web call
    let query_seed = 3; // close to the 4th search_web entry
    let query = random_vector(dim, query_seed);
    let k = 5;

    let all_results = store
        .query(&query, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Query (seed={}): top-{} cache hits:", query_seed, k);
    print_cache_results(&all_results, &tools, entries_per_tool);

    // Determine hit/miss
    let closest = &all_results[0];
    let hit_threshold = 0.5;
    if closest.distance < hit_threshold {
        println!(
            "\n  CACHE HIT: ID {} (distance {:.6} < threshold {:.1})",
            closest.id, closest.distance, hit_threshold
        );
    } else {
        println!(
            "\n  CACHE MISS: closest distance {:.6} >= threshold {:.1}",
            closest.distance, hit_threshold
        );
    }

    // -- Step 4: Filtered search — search_web with ttl > 3600 --
    println!("\n--- 4. Filtered Search: search_web with TTL > 3600 ---");

    let filter_web_long_ttl = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("search_web".to_string())),
        FilterExpr::Gt(2, FilterValue::U64(3600)),
    ]);
    let opts_filtered = QueryOptions {
        filter: Some(filter_web_long_ttl),
        ..Default::default()
    };
    let filtered_results = store
        .query(&query, k, &opts_filtered)
        .expect("filtered query failed");

    println!(
        "  search_web entries with TTL > 3600: {} results",
        filtered_results.len()
    );
    print_cache_results(&filtered_results, &tools, entries_per_tool);

    // Verify all results match the filter
    for r in &filtered_results {
        let tool_idx = (r.id as usize) / entries_per_tool;
        assert_eq!(
            tools[tool_idx].0, "search_web",
            "ID {} should be search_web",
            r.id
        );
        let entry_idx = (r.id as usize) % entries_per_tool;
        let ttl = tools[tool_idx].1 + (entry_idx as u64) * 100;
        assert!(ttl > 3600, "ID {} TTL {} should be > 3600", r.id, ttl);
    }
    if !filtered_results.is_empty() {
        println!("  All results verified: search_web with TTL > 3600.");
    }

    // -- Step 5: Cache hit vs miss pattern --
    println!("\n--- 5. Cache Hit/Miss Pattern ---");

    let test_queries: Vec<(u64, &str)> = vec![
        (0, "exact match for search_web[0]"),
        (15, "exact match for execute_code[5]"),
        (9999, "no match (random query)"),
        (25, "exact match for read_file[5]"),
    ];

    println!(
        "  {:>40}  {:>12}  {:>8}",
        "Query Description", "Distance", "Hit?"
    );
    println!("  {:->40}  {:->12}  {:->8}", "", "", "");

    for (seed, desc) in &test_queries {
        let q = random_vector(dim, *seed);
        let results = store
            .query(&q, 1, &QueryOptions::default())
            .expect("query failed");
        if let Some(r) = results.first() {
            let is_hit = r.distance < hit_threshold;
            println!(
                "  {:>40}  {:>12.6}  {:>8}",
                desc,
                r.distance,
                if is_hit { "HIT" } else { "MISS" }
            );
        }
    }

    // -- Step 6: Delete expired entries --
    println!("\n--- 6. Deleting Expired Cache Entries ---");

    // Delete all entries with TTL <= 500 (short-lived execute_code entries)
    let status_before = store.status();
    println!("  Before deletion: {} vectors", status_before.total_vectors);

    let filter_expired = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("execute_code".to_string())),
        FilterExpr::Le(2, FilterValue::U64(500)),
    ]);
    let del_result = store
        .delete_by_filter(&filter_expired)
        .expect("delete failed");
    println!(
        "  Deleted {} expired execute_code entries (TTL <= 500)",
        del_result.deleted
    );

    let status_after_delete = store.status();
    println!(
        "  After deletion: {} vectors (epoch {})",
        status_after_delete.total_vectors, status_after_delete.current_epoch
    );

    // -- Step 7: Compact to reclaim space --
    println!("\n--- 7. Compaction ---");

    println!(
        "  Dead space ratio: {:.2}%",
        status_after_delete.dead_space_ratio * 100.0
    );

    let compact_result = store.compact().expect("compaction failed");
    println!(
        "  Compacted: {} segments, {} bytes reclaimed (epoch {})",
        compact_result.segments_compacted,
        compact_result.bytes_reclaimed,
        compact_result.epoch
    );

    let status_after_compact = store.status();
    println!(
        "  After compaction: {} vectors, {} bytes",
        status_after_compact.total_vectors, status_after_compact.file_size
    );

    // Verify queries still work after compaction
    let post_compact_results = store
        .query(&query, k, &QueryOptions::default())
        .expect("post-compact query failed");
    println!(
        "  Post-compaction query: {} results (cache still functional)",
        post_compact_results.len()
    );

    store.close().expect("failed to close store");

    // -- Summary --
    println!("\n=== Tool Cache Summary ===\n");
    println!(
        "  {:>16}  {:>8}  {:>10}  {:>12}",
        "Tool", "Entries", "Base TTL", "Result Size"
    );
    println!(
        "  {:->16}  {:->8}  {:->10}  {:->12}",
        "", "", "", ""
    );
    for (name, ttl, size) in &tools {
        println!(
            "  {:>16}  {:>8}  {:>9}s  {:>11}B",
            name, entries_per_tool, ttl, size
        );
    }
    println!("\n  Initial entries:   {}", total_entries);
    println!("  Deleted (expired): {}", del_result.deleted);
    println!(
        "  Final entries:     {}",
        status_after_compact.total_vectors
    );
    println!("  Compaction:        {} bytes reclaimed", compact_result.bytes_reclaimed);

    println!("\nDone.");
}

fn print_cache_results(
    results: &[SearchResult],
    tools: &[(&str, u64, u64)],
    entries_per_tool: usize,
) {
    println!(
        "  {:>6}  {:>12}  {:>16}  {:>10}  {:>12}",
        "ID", "Distance", "Tool", "TTL (s)", "Result (B)"
    );
    println!(
        "  {:->6}  {:->12}  {:->16}  {:->10}  {:->12}",
        "", "", "", "", ""
    );
    for r in results {
        let tool_idx = (r.id as usize) / entries_per_tool;
        let entry_idx = (r.id as usize) % entries_per_tool;
        let (tool_name, base_ttl, base_size) = if tool_idx < tools.len() {
            tools[tool_idx]
        } else {
            ("unknown", 0, 0)
        };
        let ttl = base_ttl + (entry_idx as u64) * 100;
        let result_size = base_size + (entry_idx as u64) * 128;
        println!(
            "  {:>6}  {:>12.6}  {:>16}  {:>10}  {:>12}",
            r.id, r.distance, tool_name, ttl, result_size
        );
    }
}
