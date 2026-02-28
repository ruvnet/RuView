//! Item Recommendation Engine — Practical Production
//!
//! Demonstrates building an item recommendation system with RVF:
//! 1. Create a store for item embeddings (128 dims, L2 metric)
//! 2. Insert 200 item vectors with metadata: item_type, rating, popularity
//! 3. Build user preference vectors by averaging item embeddings
//! 4. Query for recommendations similar to user preferences
//! 5. Filter by item_type for genre-specific recommendations
//! 6. Filter by rating > 70 for quality recommendations
//! 7. Print grouped recommendation results
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example recommendation

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

const ITEM_TYPES: [&str; 4] = ["movie", "book", "music", "game"];

/// Deterministic item type for a given item index.
fn item_type(i: usize) -> &'static str {
    ITEM_TYPES[i % ITEM_TYPES.len()]
}

/// Deterministic rating (1-100) for a given item index.
fn item_rating(i: usize) -> u64 {
    ((i * 17 + 23) % 100 + 1) as u64
}

/// Deterministic popularity score for a given item index.
fn item_popularity(i: usize) -> u64 {
    ((i * 41 + 7) % 10000) as u64
}

/// Average multiple vectors element-wise.
fn average_vectors(vecs: &[Vec<f32>]) -> Vec<f32> {
    if vecs.is_empty() {
        return Vec::new();
    }
    let dim = vecs[0].len();
    let mut avg = vec![0.0f32; dim];
    for v in vecs {
        for (j, &val) in v.iter().enumerate() {
            avg[j] += val;
        }
    }
    let n = vecs.len() as f32;
    for val in &mut avg {
        *val /= n;
    }
    avg
}

fn main() {
    println!("=== RVF Item Recommendation Engine ===\n");

    let dim = 128;
    let num_items = 200;

    // -- Step 1: Create store --
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("items.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    println!("Creating item store...");
    println!("  Dimensions: {}", dim);
    println!("  Items:      {}", num_items);
    println!("  Types:      {:?}\n", ITEM_TYPES);

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // -- Step 2: Insert item vectors with metadata --
    // Metadata fields:
    //   field_id 0: item_type (String)
    //   field_id 1: rating (U64: 1-100)
    //   field_id 2: popularity (U64)
    let vectors: Vec<Vec<f32>> = (0..num_items)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    let batch_vecs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let batch_ids: Vec<u64> = (0..num_items as u64).collect();

    let mut metadata = Vec::with_capacity(num_items * 3);
    for i in 0..num_items {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(item_type(i).to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(item_rating(i)),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(item_popularity(i)),
        });
    }

    store
        .ingest_batch(&batch_vecs, &batch_ids, Some(&metadata))
        .expect("failed to ingest items");

    println!("Ingested {} items.\n", num_items);

    // Print item type distribution
    println!("=== Item Distribution ===\n");
    for itype in &ITEM_TYPES {
        let count = (0..num_items).filter(|&i| item_type(i) == *itype).count();
        let avg_rating: f64 = (0..num_items)
            .filter(|&i| item_type(i) == *itype)
            .map(|i| item_rating(i) as f64)
            .sum::<f64>()
            / count as f64;
        let high_rated = (0..num_items)
            .filter(|&i| item_type(i) == *itype && item_rating(i) > 70)
            .count();
        println!(
            "  {:>6}: {} items, avg rating: {:.1}, high-rated (>70): {}",
            itype, count, avg_rating, high_rated
        );
    }

    // -- Step 3: Create user preference vectors --
    // Create a user who likes movies and books with IDs 0, 4, 8, 12, 16
    // (these happen to be movies since 0 % 4 == 0)
    let liked_ids: Vec<usize> = vec![0, 4, 8, 12, 16, 1, 5, 9]; // mix of movies and books
    let liked_vecs: Vec<Vec<f32>> = liked_ids.iter().map(|&i| vectors[i].clone()).collect();
    let user_preference = average_vectors(&liked_vecs);

    println!("\n=== User Profile ===\n");
    println!("  Liked items: {:?}", liked_ids);
    println!("  Liked types: {:?}",
        liked_ids.iter().map(|&i| item_type(i)).collect::<Vec<_>>()
    );
    println!("  Liked ratings: {:?}",
        liked_ids.iter().map(|&i| item_rating(i)).collect::<Vec<_>>()
    );

    let k = 10;

    // ====================================================================
    // 4. General recommendations (all types)
    // ====================================================================
    println!("\n=== General Recommendations (Top-{}) ===\n", k);

    let results_all = store
        .query(&user_preference, k, &QueryOptions::default())
        .expect("query failed");
    print_recommendation_table(&results_all);

    // ====================================================================
    // 5. Genre-specific recommendations
    // ====================================================================
    for itype in &ITEM_TYPES {
        println!("\n=== {} Recommendations (Top-5) ===\n", capitalize(itype));

        let filter = FilterExpr::Eq(0, FilterValue::String(itype.to_string()));
        let opts = QueryOptions {
            filter: Some(filter),
            ..Default::default()
        };
        let results = store
            .query(&user_preference, 5, &opts)
            .expect("filtered query failed");
        print_recommendation_table(&results);

        // Verify filter correctness
        for r in &results {
            assert_eq!(
                item_type(r.id as usize), *itype,
                "ID {} should be type {}",
                r.id, itype
            );
        }
    }

    // ====================================================================
    // 6. Quality recommendations: rating > 70
    // ====================================================================
    println!("\n=== Quality Recommendations (rating > 70, Top-{}) ===\n", k);

    let filter_quality = FilterExpr::Gt(1, FilterValue::U64(70));
    let opts_quality = QueryOptions {
        filter: Some(filter_quality),
        ..Default::default()
    };
    let results_quality = store
        .query(&user_preference, k, &opts_quality)
        .expect("quality query failed");
    print_recommendation_table(&results_quality);

    // Verify all results have rating > 70
    for r in &results_quality {
        let rating = item_rating(r.id as usize);
        assert!(
            rating > 70,
            "ID {} has rating {} which is not > 70",
            r.id, rating
        );
    }
    println!("  All {} results verified with rating > 70.", results_quality.len());

    // Quality + genre combined
    println!("\n=== High-Rated Movies (rating > 70 AND type == \"movie\", Top-5) ===\n");

    let filter_hq_movie = FilterExpr::And(vec![
        FilterExpr::Gt(1, FilterValue::U64(70)),
        FilterExpr::Eq(0, FilterValue::String("movie".to_string())),
    ]);
    let opts_hq_movie = QueryOptions {
        filter: Some(filter_hq_movie),
        ..Default::default()
    };
    let results_hq_movie = store
        .query(&user_preference, 5, &opts_hq_movie)
        .expect("combined query failed");
    print_recommendation_table(&results_hq_movie);

    // ====================================================================
    // Summary: grouped by type
    // ====================================================================
    println!("\n=== Recommendation Summary (grouped by type) ===\n");

    let results_20 = store
        .query(&user_preference, 20, &QueryOptions::default())
        .expect("query failed");

    for itype in &ITEM_TYPES {
        let typed: Vec<&SearchResult> = results_20.iter()
            .filter(|r| item_type(r.id as usize) == *itype)
            .collect();
        println!("  {} ({} found in top-20):", capitalize(itype), typed.len());
        for r in &typed {
            let id = r.id as usize;
            println!(
                "    ID {:>4} | rating: {:>3} | popularity: {:>5} | distance: {:.6}",
                r.id, item_rating(id), item_popularity(id), r.distance
            );
        }
    }

    let status = store.status();
    println!("\n  Store status:");
    println!("    Total items: {}", status.total_vectors);
    println!("    File size:   {} bytes", status.file_size);

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_recommendation_table(results: &[SearchResult]) {
    println!(
        "  {:>6}  {:>12}  {:>8}  {:>8}  {:>10}",
        "ID", "Distance", "Type", "Rating", "Popularity"
    );
    println!(
        "  {:->6}  {:->12}  {:->8}  {:->8}  {:->10}",
        "", "", "", "", ""
    );
    for r in results {
        let id = r.id as usize;
        println!(
            "  {:>6}  {:>12.6}  {:>8}  {:>8}  {:>10}",
            r.id, r.distance, item_type(id), item_rating(id), item_popularity(id)
        );
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
