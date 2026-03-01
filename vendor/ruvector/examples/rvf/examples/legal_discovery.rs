//! Vertical Domain: Legal Document Similarity with .rvtext
//!
//! Demonstrates RVF as a legal discovery substrate using the
//! DomainProfile::RvText profile, triggered by the `.rvtext` file extension.
//!
//! Features:
//!   - 300 document embedding vectors (768 dims) with legal metadata
//!   - Filtered search by doc_type, jurisdiction, and year
//!   - Range filter on relevance_score
//!   - Witness chain for discovery audit trail
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example legal_discovery

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
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
    println!("=== Legal Discovery (.rvtext) ===\n");

    let dim = 768;
    let num_docs = 300;

    let doc_types = ["contract", "brief", "motion", "deposition", "statute"];
    let jurisdictions = ["NY", "CA", "TX", "DE", "IL", "FL"];

    // ====================================================================
    // 1. Create store with .rvtext extension (DomainProfile::RvText)
    // ====================================================================
    println!("--- 1. Create Legal Document Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("discovery.rvtext");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created with .rvtext extension (DomainProfile::RvText)");
    println!("  Dimensions: {} (document embedding space)", dim);

    // ====================================================================
    // 2. Insert 300 document embeddings with legal metadata
    // ====================================================================
    println!("\n--- 2. Ingest Document Embeddings ---");

    let vectors: Vec<Vec<f32>> = (0..num_docs)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_docs as u64).collect();

    // Metadata: doc_type (0), jurisdiction (1), year (2), relevance_score (3)
    let mut metadata = Vec::with_capacity(num_docs * 4);
    for i in 0..num_docs {
        let doc_type = doc_types[i % doc_types.len()];
        let jurisdiction = jurisdictions[i % jurisdictions.len()];
        let year = (2015 + (i % 11)) as u64; // 2015-2025
        let relevance = ((i * 17 + 23) % 101) as u64;

        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(doc_type.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(jurisdiction.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(year),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::U64(relevance),
        });
    }

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!("  Ingested {} document embeddings (rejected: {})", ingest.accepted, ingest.rejected);

    // Print distribution
    for dt in &doc_types {
        let count = (0..num_docs).filter(|i| doc_types[i % doc_types.len()] == *dt).count();
        println!("    {}: {} documents", dt, count);
    }

    // ====================================================================
    // 3. General similarity search
    // ====================================================================
    println!("\n--- 3. Document Similarity Search ---");

    let query_vec = random_vector(dim, 999);
    let k = 10;

    let results = store
        .query(&query_vec, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Top-{} similar documents:", k);
    print_legal_results(&results, &doc_types, &jurisdictions);

    // ====================================================================
    // 4. Filter: contracts filed after 2020
    // ====================================================================
    println!("\n--- 4. Recent Contracts (year > 2020) ---");

    let filter_contracts = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("contract".to_string())),
        FilterExpr::Gt(2, FilterValue::U64(2020)),
    ]);
    let opts_contracts = QueryOptions {
        filter: Some(filter_contracts),
        ..Default::default()
    };
    let results_contracts = store
        .query(&query_vec, k, &opts_contracts)
        .expect("filtered query failed");

    println!("  Contracts after 2020: {}", results_contracts.len());
    if !results_contracts.is_empty() {
        print_legal_results(&results_contracts, &doc_types, &jurisdictions);
    }

    for r in &results_contracts {
        let idx = r.id as usize;
        assert_eq!(doc_types[idx % doc_types.len()], "contract");
        assert!((2015 + (idx % 11)) > 2020);
    }
    if !results_contracts.is_empty() {
        println!("  All results verified: contract AND year > 2020.");
    }

    // ====================================================================
    // 5. Range filter: relevance_score in [70, 100]
    // ====================================================================
    println!("\n--- 5. High-Relevance Documents (score 70-100) ---");

    let filter_relevance = FilterExpr::Range(3, FilterValue::U64(70), FilterValue::U64(101));
    let opts_relevance = QueryOptions {
        filter: Some(filter_relevance),
        ..Default::default()
    };
    let results_relevance = store
        .query(&query_vec, k, &opts_relevance)
        .expect("filtered query failed");

    println!("  High-relevance documents: {}", results_relevance.len());
    if !results_relevance.is_empty() {
        print_legal_results(&results_relevance, &doc_types, &jurisdictions);
    }

    for r in &results_relevance {
        let score = ((r.id as usize) * 17 + 23) % 101;
        assert!(
            (70..101).contains(&score),
            "ID {} has relevance {} not in [70, 101)",
            r.id,
            score
        );
    }
    if !results_relevance.is_empty() {
        println!("  All results verified: relevance in [70, 100].");
    }

    // ====================================================================
    // 6. Filter by jurisdiction
    // ====================================================================
    println!("\n--- 6. Jurisdiction Filter (Delaware) ---");

    let filter_de = FilterExpr::Eq(1, FilterValue::String("DE".to_string()));
    let opts_de = QueryOptions {
        filter: Some(filter_de),
        ..Default::default()
    };
    let results_de = store
        .query(&query_vec, k, &opts_de)
        .expect("filtered query failed");

    println!("  Delaware documents: {}", results_de.len());
    if !results_de.is_empty() {
        print_legal_results(&results_de, &doc_types, &jurisdictions);
    }

    // ====================================================================
    // 7. Discovery audit trail
    // ====================================================================
    println!("\n--- 7. Discovery Audit Trail ---");

    let audit_steps = [
        ("collection_init", 0x01u8),
        ("document_ingestion", 0x02),
        ("embedding_generation", 0x02),
        ("relevance_scoring", 0x02),
        ("privilege_review", 0x08),
        ("production_export", 0x08),
        ("audit_seal", 0x01),
    ];

    let entries: Vec<WitnessEntry> = audit_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("ediscovery:case_2024_001:{}:{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 3_600_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Audit chain: {} entries, {} bytes, VALID", verified.len(), chain_bytes.len());

    println!("\n  Discovery workflow:");
    for (i, (step, _)) in audit_steps.iter().enumerate() {
        let wtype_name = match verified[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            0x08 => "DATA",
            _ => "????",
        };
        println!("    [{}] {} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Legal Discovery Summary ===\n");
    println!("  Domain profile:        RvText (.rvtext)");
    println!("  Documents indexed:     {}", num_docs);
    println!("  Embedding dims:        {}", dim);
    println!("  Unfiltered:            {} results", results.len());
    println!("  Contracts (post-2020): {} results", results_contracts.len());
    println!("  High relevance:        {} results", results_relevance.len());
    println!("  Delaware:              {} results", results_de.len());
    println!("  Audit trail:           {} entries", audit_steps.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_legal_results(
    results: &[SearchResult],
    doc_types: &[&str],
    jurisdictions: &[&str],
) {
    println!(
        "    {:>6}  {:>12}  {:>12}  {:>6}  {:>6}  {:>6}",
        "ID", "Distance", "DocType", "Juris", "Year", "Rel"
    );
    println!(
        "    {:->6}  {:->12}  {:->12}  {:->6}  {:->6}  {:->6}",
        "", "", "", "", "", ""
    );
    for r in results {
        let idx = r.id as usize;
        let doc_type = doc_types[idx % doc_types.len()];
        let jurisdiction = jurisdictions[idx % jurisdictions.len()];
        let year = 2015 + (idx % 11);
        let relevance = (idx * 17 + 23) % 101;
        println!(
            "    {:>6}  {:>12.6}  {:>12}  {:>6}  {:>6}  {:>6}",
            r.id, r.distance, doc_type, jurisdiction, year, relevance
        );
    }
}
