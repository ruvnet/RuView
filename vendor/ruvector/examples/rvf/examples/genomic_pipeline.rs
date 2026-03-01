//! Vertical Domain: Genomic Analysis with .rvdna
//!
//! Demonstrates RVF as a genomics substrate using the DomainProfile::Rvdna
//! profile, triggered by the `.rvdna` file extension.
//!
//! Pipeline stages:
//!   k-mer encoding -> embedding -> similarity search -> variant detection
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG (via rvf-crypto)
//! Lineage: parent -> child derivation with DerivationType::Filter
//!
//! Run: cargo run --example genomic_pipeline

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_types::DerivationType;
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
    println!("=== Genomic Pipeline (.rvdna) ===\n");

    let dim = 64;
    let num_kmers = 100;

    // Gene names for metadata
    let gene_names = [
        "BRCA1", "TP53", "EGFR", "KRAS", "PIK3CA",
        "BRAF", "PTEN", "APC", "CDKN2A", "MYC",
    ];
    let chromosomes: [u64; 10] = [17, 17, 7, 12, 3, 7, 10, 5, 9, 8];

    // ====================================================================
    // 1. Create store with .rvdna extension (triggers DomainProfile::Rvdna)
    // ====================================================================
    println!("--- 1. Create Genomic Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("genome.rvdna");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created with .rvdna extension");
    println!("  Dimensions: {} (k-mer embedding space)", dim);

    // ====================================================================
    // 2. Produce k-mer encoding -> embedding pipeline
    // ====================================================================
    println!("\n--- 2. K-mer Encoding and Embedding ---");

    let vectors: Vec<Vec<f32>> = (0..num_kmers)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_kmers as u64).collect();

    // Build metadata: gene_name (field 0), chromosome (field 1),
    // position (field 2), kmer_length (field 3)
    let mut metadata = Vec::with_capacity(num_kmers * 4);
    for i in 0..num_kmers {
        let gene_idx = i % gene_names.len();
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(gene_names[gene_idx].to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(chromosomes[gene_idx]),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64((i * 1000 + 50000) as u64),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::U64(31),
        });
    }

    let ingest_result = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("failed to ingest k-mer embeddings");
    println!(
        "  Ingested {} k-mer embeddings (rejected: {})",
        ingest_result.accepted, ingest_result.rejected
    );

    // ====================================================================
    // 3. Similarity search: find similar k-mers
    // ====================================================================
    println!("\n--- 3. K-mer Similarity Search ---");

    let query_seed = 5; // query based on the 6th k-mer (BRAF gene)
    let query_vec = random_vector(dim, query_seed);
    let k = 10;

    let results = store
        .query(&query_vec, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Query k-mer (seed={}): top-{} similar k-mers:", query_seed, k);
    print_genomic_results(&results, gene_names.as_slice(), &chromosomes);

    // ====================================================================
    // 4. Filtered search: chromosome-specific analysis
    // ====================================================================
    println!("\n--- 4. Chromosome-Specific Search ---");

    // Filter: chromosome == 7 (EGFR and BRAF)
    let filter_chr7 = FilterExpr::Eq(1, FilterValue::U64(7));
    let opts_chr7 = QueryOptions {
        filter: Some(filter_chr7),
        ..Default::default()
    };
    let results_chr7 = store
        .query(&query_vec, k, &opts_chr7)
        .expect("filtered query failed");

    println!("  Chromosome 7 k-mers (EGFR/BRAF region):");
    print_genomic_results(&results_chr7, gene_names.as_slice(), &chromosomes);

    // Verify all results are chromosome 7
    for r in &results_chr7 {
        let gene_idx = (r.id as usize) % gene_names.len();
        assert_eq!(chromosomes[gene_idx], 7, "expected chromosome 7");
    }
    println!("  All results verified as chromosome 7.");

    // ====================================================================
    // 5. Variant detection: filter by gene
    // ====================================================================
    println!("\n--- 5. Variant Detection (Gene-Specific) ---");

    // Filter: gene_name == "TP53"
    let filter_tp53 = FilterExpr::Eq(0, FilterValue::String("TP53".to_string()));
    let opts_tp53 = QueryOptions {
        filter: Some(filter_tp53),
        ..Default::default()
    };
    let results_tp53 = store
        .query(&query_vec, k, &opts_tp53)
        .expect("filtered query failed");

    println!("  TP53 variants found: {}", results_tp53.len());
    print_genomic_results(&results_tp53, gene_names.as_slice(), &chromosomes);

    // ====================================================================
    // 6. Build witness chain recording pipeline steps
    // ====================================================================
    println!("\n--- 6. Pipeline Audit Trail (Witness Chain) ---");

    let pipeline_steps = [
        "kmer_extraction",
        "embedding_generation",
        "similarity_search",
        "variant_detection",
        "annotation",
    ];

    let entries: Vec<WitnessEntry> = pipeline_steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let action_data = format!("genomic_pipeline:step_{}:{}", i, step);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 60_000_000_000,
                witness_type: if i == 0 { 0x01 } else { 0x02 }, // PROVENANCE then COMPUTATION
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    println!("  Created witness chain: {} entries, {} bytes", entries.len(), chain_bytes.len());

    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Chain integrity: VALID ({} entries verified)", verified.len());

    println!("\n  Pipeline steps recorded:");
    for (i, step) in pipeline_steps.iter().enumerate() {
        let wtype = if i == 0 { "PROV" } else { "COMP" };
        println!("    [{}] {} -> {}", wtype, i, step);
    }

    // ====================================================================
    // 7. Derive child store with filtered variants
    // ====================================================================
    println!("\n--- 7. Derive Filtered Variant Store ---");

    let child_path = tmp_dir.path().join("variants_chr7.rvdna");
    let child_store = store
        .derive(&child_path, DerivationType::Filter, None)
        .expect("failed to derive child store");

    // Verify lineage
    let parent_id = store.file_id();
    let child_parent_id = child_store.parent_id();
    assert_eq!(parent_id, child_parent_id, "lineage parent mismatch");
    assert_eq!(child_store.lineage_depth(), 1, "child depth should be 1");

    println!("  Parent file_id:    {}", hex_string(parent_id));
    println!("  Child parent_id:   {}", hex_string(child_parent_id));
    println!("  Lineage depth:     {}", child_store.lineage_depth());
    println!("  Lineage verified:  parent_id matches");

    child_store.close().expect("failed to close child");

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Genomic Pipeline Summary ===\n");
    println!("  Domain profile:     Rvdna (.rvdna)");
    println!("  K-mers embedded:    {}", num_kmers);
    println!("  Embedding dims:     {}", dim);
    println!("  Similarity results: {}", results.len());
    println!("  Chr7 results:       {}", results_chr7.len());
    println!("  TP53 variants:      {}", results_tp53.len());
    println!("  Audit trail:        {} pipeline steps", pipeline_steps.len());
    println!("  Lineage chain:      parent -> filtered child");

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_genomic_results(results: &[SearchResult], genes: &[&str], chromosomes: &[u64]) {
    println!(
        "    {:>6}  {:>12}  {:>8}  {:>5}  {:>8}",
        "ID", "Distance", "Gene", "Chr", "Position"
    );
    println!("    {:->6}  {:->12}  {:->8}  {:->5}  {:->8}", "", "", "", "", "");
    for r in results {
        let gene_idx = (r.id as usize) % genes.len();
        let pos = (r.id as usize) * 1000 + 50000;
        println!(
            "    {:>6}  {:>12.6}  {:>8}  {:>5}  {:>8}",
            r.id, r.distance, genes[gene_idx], chromosomes[gene_idx], pos
        );
    }
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
