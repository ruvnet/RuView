//! Document Search Engine — Practical Production
//!
//! Demonstrates building a document search engine with filtered vector search:
//! 1. Create a store for document embeddings (384 dims, L2 metric)
//! 2. Insert 500 document vectors with metadata: doc_id, category, word_count, publish_year
//! 3. Basic semantic search (top-10 nearest neighbors)
//! 4. Filtered search: category == "science" AND publish_year > 2023
//! 5. Range search: word_count in [500, 2000]
//! 6. Multi-category search: category IN ["tech", "science"]
//! 7. Recall measurement and formatted results table
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example semantic_search

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

/// Categories assigned to documents based on index.
const CATEGORIES: [&str; 5] = ["science", "tech", "business", "health", "sports"];

/// Deterministic category for a given document index.
fn doc_category(i: usize) -> &'static str {
    CATEGORIES[i % CATEGORIES.len()]
}

/// Deterministic word count for a given document index.
fn doc_word_count(i: usize) -> u64 {
    ((i * 31 + 17) % 3000 + 100) as u64
}

/// Deterministic publish year for a given document index.
fn doc_publish_year(i: usize) -> u64 {
    (2018 + (i * 13 + 7) % 8) as u64
}

fn main() {
    println!("=== RVF Document Search Engine ===\n");

    let dim = 384;
    let num_docs = 500;

    // -- Step 1: Create store --
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("documents.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    println!("Creating document store at {:?}", store_path);
    println!("  Dimensions:  {}", dim);
    println!("  Documents:   {}", num_docs);
    println!("  Metric:      L2 (squared Euclidean)\n");

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // -- Step 2: Insert document vectors with metadata --
    // Metadata fields:
    //   field_id 0: category (String)
    //   field_id 1: word_count (U64)
    //   field_id 2: publish_year (U64)
    let vectors: Vec<Vec<f32>> = (0..num_docs)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    let batch_size = 100;
    let num_batches = num_docs / batch_size;

    for batch_idx in 0..num_batches {
        let start = batch_idx * batch_size;
        let end = start + batch_size;

        let batch_vecs: Vec<&[f32]> = vectors[start..end]
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let batch_ids: Vec<u64> = (start as u64..end as u64).collect();

        // 3 metadata entries per vector: category, word_count, publish_year
        let mut metadata = Vec::with_capacity(batch_size * 3);
        for i in start..end {
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(doc_category(i).to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(doc_word_count(i)),
            });
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(doc_publish_year(i)),
            });
        }

        store
            .ingest_batch(&batch_vecs, &batch_ids, Some(&metadata))
            .expect("failed to ingest batch");
    }

    println!("Ingested {} documents across {} batches.\n", num_docs, num_batches);

    // Print metadata distribution
    let mut cat_counts = [0usize; 5];
    let mut year_counts = std::collections::HashMap::new();
    for i in 0..num_docs {
        cat_counts[i % CATEGORIES.len()] += 1;
        *year_counts.entry(doc_publish_year(i)).or_insert(0usize) += 1;
    }

    println!("=== Metadata Distribution ===\n");
    println!("  Category distribution:");
    for (idx, cat) in CATEGORIES.iter().enumerate() {
        println!("    {:>10}: {} docs", cat, cat_counts[idx]);
    }

    let mut years: Vec<_> = year_counts.iter().collect();
    years.sort_by_key(|&(y, _)| *y);
    println!("\n  Year distribution:");
    for (year, count) in &years {
        println!("    {:>10}: {} docs", year, count);
    }

    // -- Common query vector --
    let query = random_vector(dim, 999);
    let k = 10;

    // ====================================================================
    // 3. Basic semantic search (top-10 nearest neighbors)
    // ====================================================================
    println!("\n=== Basic Semantic Search (Top-{}) ===\n", k);

    let results_all = store
        .query(&query, k, &QueryOptions::default())
        .expect("query failed");
    print_results_table(&results_all);

    // ====================================================================
    // 4. Filtered search: category == "science" AND publish_year > 2023
    // ====================================================================
    println!("\n=== Filtered Search: category == \"science\" AND publish_year > 2023 ===\n");

    let filter_sci_recent = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("science".to_string())),
        FilterExpr::Gt(2, FilterValue::U64(2023)),
    ]);
    let opts_sci = QueryOptions {
        filter: Some(filter_sci_recent),
        ..Default::default()
    };
    let results_sci = store
        .query(&query, k, &opts_sci)
        .expect("filtered query failed");

    print_results_table(&results_sci);

    // Measure recall: how many results actually match the filter
    let sci_recall = results_sci.iter().filter(|r| {
        let id = r.id as usize;
        doc_category(id) == "science" && doc_publish_year(id) > 2023
    }).count();
    println!(
        "  Filter recall: {}/{} results match (100% expected for pre-filter)",
        sci_recall, results_sci.len()
    );

    // Count eligible documents
    let sci_eligible = (0..num_docs)
        .filter(|&i| doc_category(i) == "science" && doc_publish_year(i) > 2023)
        .count();
    println!(
        "  Eligible pool: {} out of {} documents ({:.1}% selectivity)",
        sci_eligible, num_docs, sci_eligible as f64 / num_docs as f64 * 100.0
    );

    // ====================================================================
    // 5. Range search: word_count in [500, 2000]
    // ====================================================================
    println!("\n=== Range Search: word_count in [500, 2000) ===\n");

    let filter_wc = FilterExpr::Range(1, FilterValue::U64(500), FilterValue::U64(2000));
    let opts_wc = QueryOptions {
        filter: Some(filter_wc),
        ..Default::default()
    };
    let results_wc = store
        .query(&query, k, &opts_wc)
        .expect("range query failed");

    print_results_table(&results_wc);

    let wc_recall = results_wc.iter().filter(|r| {
        let wc = doc_word_count(r.id as usize);
        (500..2000).contains(&wc)
    }).count();
    println!("  Filter recall: {}/{} results match", wc_recall, results_wc.len());

    let wc_eligible = (0..num_docs)
        .filter(|&i| {
            let wc = doc_word_count(i);
            (500..2000).contains(&wc)
        })
        .count();
    println!(
        "  Eligible pool: {} out of {} documents ({:.1}% selectivity)",
        wc_eligible, num_docs, wc_eligible as f64 / num_docs as f64 * 100.0
    );

    // ====================================================================
    // 6. Multi-category: category IN ["tech", "science"]
    // ====================================================================
    println!("\n=== Multi-Category Search: category IN [\"tech\", \"science\"] ===\n");

    let filter_multi = FilterExpr::In(
        0,
        vec![
            FilterValue::String("tech".to_string()),
            FilterValue::String("science".to_string()),
        ],
    );
    let opts_multi = QueryOptions {
        filter: Some(filter_multi),
        ..Default::default()
    };
    let results_multi = store
        .query(&query, k, &opts_multi)
        .expect("multi-category query failed");

    print_results_table(&results_multi);

    let multi_recall = results_multi.iter().filter(|r| {
        let cat = doc_category(r.id as usize);
        cat == "tech" || cat == "science"
    }).count();
    println!("  Filter recall: {}/{} results match", multi_recall, results_multi.len());

    let multi_eligible = (0..num_docs)
        .filter(|&i| {
            let cat = doc_category(i);
            cat == "tech" || cat == "science"
        })
        .count();
    println!(
        "  Eligible pool: {} out of {} documents ({:.1}% selectivity)",
        multi_eligible, num_docs, multi_eligible as f64 / num_docs as f64 * 100.0
    );

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Search Summary ===\n");
    println!(
        "  {:>40}  {:>10}  {:>12}",
        "Query", "Results", "Eligible"
    );
    println!("  {:->40}  {:->10}  {:->12}", "", "", "");
    println!(
        "  {:>40}  {:>10}  {:>12}",
        "Unfiltered (baseline)", results_all.len(), num_docs
    );
    println!(
        "  {:>40}  {:>10}  {:>12}",
        "science AND year > 2023", results_sci.len(), sci_eligible
    );
    println!(
        "  {:>40}  {:>10}  {:>12}",
        "word_count in [500, 2000)", results_wc.len(), wc_eligible
    );
    println!(
        "  {:>40}  {:>10}  {:>12}",
        "category IN [tech, science]", results_multi.len(), multi_eligible
    );

    let status = store.status();
    println!("\n  Store status:");
    println!("    Total vectors: {}", status.total_vectors);
    println!("    File size:     {} bytes", status.file_size);
    println!("    Epoch:         {}", status.current_epoch);

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_results_table(results: &[SearchResult]) {
    println!(
        "  {:>6}  {:>12}  {:>10}  {:>12}  {:>6}",
        "ID", "Distance", "Category", "Word Count", "Year"
    );
    println!(
        "  {:->6}  {:->12}  {:->10}  {:->12}  {:->6}",
        "", "", "", "", ""
    );
    for r in results {
        let id = r.id as usize;
        println!(
            "  {:>6}  {:>12.6}  {:>10}  {:>12}  {:>6}",
            r.id, r.distance, doc_category(id), doc_word_count(id), doc_publish_year(id)
        );
    }
}
