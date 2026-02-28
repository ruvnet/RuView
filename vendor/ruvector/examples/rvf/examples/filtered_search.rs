//! Metadata Filtered Search
//!
//! Demonstrates filtered vector search using RvfStore:
//! 1. Create a store with 500 vectors
//! 2. Add metadata (category: "A"/"B"/"C", score: 0.0-1.0)
//! 3. Query with filter: category == "A"
//! 4. Query with filter: score > 0.5 (using U64 encoding)
//! 5. Query with combined filter: category == "B" AND score > 70
//! 6. Show filtered results vs unfiltered

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// LCG-based pseudo-random vector generator.
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
    println!("=== RVF Filtered Search Example ===\n");

    let dim = 64;
    let num_vectors = 500;

    // -- Step 1: Create store --
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("filtered.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    println!("Creating store with {} vectors ({} dims, L2 metric)...", num_vectors, dim);
    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // -- Step 2: Insert vectors with metadata --
    // Metadata fields:
    //   field_id 0: category (String: "A", "B", or "C")
    //   field_id 1: score (U64: 0-100, representing percentage)
    let categories = ["A", "B", "C"];
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    // Ingest in batches of 50 to demonstrate repeated ingestion.
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

        // Build metadata: 2 entries per vector (category + score).
        let mut metadata = Vec::with_capacity(batch_size * 2);
        for i in start..end {
            // Assign category based on modulo.
            let cat = categories[i % 3];
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(cat.to_string()),
            });
            // Assign score: deterministic based on index.
            let score = ((i * 7 + 13) % 101) as u64;
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(score),
            });
        }

        store
            .ingest_batch(&batch_vecs, &batch_ids, Some(&metadata))
            .expect("failed to ingest batch");
    }

    println!("Ingested {} vectors across {} batches.", num_vectors, num_batches);

    // Print metadata distribution.
    let cat_a_count = (0..num_vectors).filter(|i| i % 3 == 0).count();
    let cat_b_count = (0..num_vectors).filter(|i| i % 3 == 1).count();
    let cat_c_count = (0..num_vectors).filter(|i| i % 3 == 2).count();
    let high_score_count = (0..num_vectors)
        .filter(|&i| ((i * 7 + 13) % 101) > 50)
        .count();

    println!("\nMetadata distribution:");
    println!("  Category A: {} vectors", cat_a_count);
    println!("  Category B: {} vectors", cat_b_count);
    println!("  Category C: {} vectors", cat_c_count);
    println!("  Score > 50: {} vectors", high_score_count);

    // -- Common query vector --
    let query = random_vector(dim, 999);
    let k = 10;

    // ====================================================================
    // 3. Unfiltered query (baseline)
    // ====================================================================
    println!("\n--- Unfiltered Query (baseline) ---");
    let results_all = store
        .query(&query, k, &QueryOptions::default())
        .expect("query failed");
    println!("Top-{} results (no filter):", k);
    print_results_with_meta(&results_all, num_vectors);

    // ====================================================================
    // 4. Filter: category == "A"
    // ====================================================================
    println!("\n--- Filter: category == \"A\" ---");
    let filter_cat_a = FilterExpr::Eq(0, FilterValue::String("A".to_string()));
    let opts_cat_a = QueryOptions {
        filter: Some(filter_cat_a),
        ..Default::default()
    };
    let results_cat_a = store
        .query(&query, k, &opts_cat_a)
        .expect("filtered query failed");
    println!("Top-{} results (category == A):", k);
    print_results_with_meta(&results_cat_a, num_vectors);

    // Verify all results are category A.
    for r in &results_cat_a {
        assert_eq!(
            (r.id as usize) % 3,
            0,
            "ID {} should be category A",
            r.id
        );
    }
    println!("  All results verified as category A.");

    // ====================================================================
    // 5. Filter: score > 50
    // ====================================================================
    println!("\n--- Filter: score > 50 ---");
    let filter_high_score = FilterExpr::Gt(1, FilterValue::U64(50));
    let opts_score = QueryOptions {
        filter: Some(filter_high_score),
        ..Default::default()
    };
    let results_high_score = store
        .query(&query, k, &opts_score)
        .expect("filtered query failed");
    println!("Top-{} results (score > 50):", k);
    print_results_with_meta(&results_high_score, num_vectors);

    // Verify all results have score > 50.
    for r in &results_high_score {
        let score = ((r.id as usize) * 7 + 13) % 101;
        assert!(
            score > 50,
            "ID {} has score {} which is not > 50",
            r.id,
            score
        );
    }
    println!("  All results verified with score > 50.");

    // ====================================================================
    // 6. Combined filter: category == "B" AND score > 70
    // ====================================================================
    println!("\n--- Filter: category == \"B\" AND score > 70 ---");
    let filter_combined = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("B".to_string())),
        FilterExpr::Gt(1, FilterValue::U64(70)),
    ]);
    let opts_combined = QueryOptions {
        filter: Some(filter_combined),
        ..Default::default()
    };
    let results_combined = store
        .query(&query, k, &opts_combined)
        .expect("filtered query failed");
    println!(
        "Top-{} results (category == B AND score > 70):",
        k
    );
    print_results_with_meta(&results_combined, num_vectors);

    // Verify all results match both conditions.
    for r in &results_combined {
        let cat_idx = (r.id as usize) % 3;
        let score = ((r.id as usize) * 7 + 13) % 101;
        assert_eq!(cat_idx, 1, "ID {} should be category B", r.id);
        assert!(score > 70, "ID {} has score {} which is not > 70", r.id, score);
    }
    if !results_combined.is_empty() {
        println!("  All results verified as category B with score > 70.");
    }

    // Count how many vectors match the combined filter.
    let combined_eligible = (0..num_vectors)
        .filter(|&i| i % 3 == 1 && ((i * 7 + 13) % 101) > 70)
        .count();
    println!(
        "  Eligible vectors: {} out of {} ({:.1}% selectivity)",
        combined_eligible,
        num_vectors,
        combined_eligible as f64 / num_vectors as f64 * 100.0
    );

    // ====================================================================
    // 7. Additional filter demonstrations
    // ====================================================================
    println!("\n--- Additional Filters ---");

    // NOT filter: category != "C"
    let filter_not_c = FilterExpr::Ne(0, FilterValue::String("C".to_string()));
    let opts_not_c = QueryOptions {
        filter: Some(filter_not_c),
        ..Default::default()
    };
    let results_not_c = store
        .query(&query, k, &opts_not_c)
        .expect("query failed");
    println!("category != \"C\": {} results", results_not_c.len());
    for r in &results_not_c {
        assert_ne!((r.id as usize) % 3, 2);
    }
    println!("  Verified: no category C in results.");

    // IN filter: category IN ("A", "C")
    let filter_in = FilterExpr::In(
        0,
        vec![
            FilterValue::String("A".to_string()),
            FilterValue::String("C".to_string()),
        ],
    );
    let opts_in = QueryOptions {
        filter: Some(filter_in),
        ..Default::default()
    };
    let results_in = store
        .query(&query, k, &opts_in)
        .expect("query failed");
    println!("category IN (A, C): {} results", results_in.len());
    for r in &results_in {
        let cat_idx = (r.id as usize) % 3;
        assert!(cat_idx == 0 || cat_idx == 2);
    }
    println!("  Verified: only categories A and C in results.");

    // Range filter: score in [30, 60)
    let filter_range = FilterExpr::Range(1, FilterValue::U64(30), FilterValue::U64(60));
    let opts_range = QueryOptions {
        filter: Some(filter_range),
        ..Default::default()
    };
    let results_range = store
        .query(&query, k, &opts_range)
        .expect("query failed");
    println!("score in [30, 60): {} results", results_range.len());
    for r in &results_range {
        let score = ((r.id as usize) * 7 + 13) % 101;
        assert!((30..60).contains(&score), "ID {} score {} out of range", r.id, score);
    }
    println!("  Verified: all scores in [30, 60).");

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Filter Summary ===\n");
    println!(
        "  {:>30}  {:>10}",
        "Filter", "Results"
    );
    println!("  {:->30}  {:->10}", "", "");
    println!("  {:>30}  {:>10}", "No filter", results_all.len());
    println!("  {:>30}  {:>10}", "category == A", results_cat_a.len());
    println!("  {:>30}  {:>10}", "score > 50", results_high_score.len());
    println!("  {:>30}  {:>10}", "cat == B AND score > 70", results_combined.len());
    println!("  {:>30}  {:>10}", "category != C", results_not_c.len());
    println!("  {:>30}  {:>10}", "category IN (A, C)", results_in.len());
    println!("  {:>30}  {:>10}", "score in [30, 60)", results_range.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_results_with_meta(results: &[SearchResult], _num_vectors: usize) {
    let categories = ["A", "B", "C"];
    println!(
        "  {:>6}  {:>12}  {:>8}  {:>6}",
        "ID", "Distance", "Category", "Score"
    );
    println!("  {:->6}  {:->12}  {:->8}  {:->6}", "", "", "", "");
    for r in results {
        let cat = categories[(r.id as usize) % 3];
        let score = ((r.id as usize) * 7 + 13) % 101;
        println!(
            "  {:>6}  {:>12.6}  {:>8}  {:>6}",
            r.id, r.distance, cat, score
        );
    }
}
