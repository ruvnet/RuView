//! Vertical Domain: Radiology Embedding Search with .rvvis
//!
//! Demonstrates RVF as a medical imaging retrieval substrate using the
//! DomainProfile::RvVision profile, triggered by the `.rvvis` file extension.
//!
//! Features:
//!   - 150 image embedding vectors (512 dims) with radiology metadata
//!   - Filtered search by modality and finding
//!   - Combined filter: modality AND finding for targeted case retrieval
//!   - Witness chain for audit trail (PROVENANCE + DATA_PROVENANCE)
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example medical_imaging

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
    println!("=== Medical Imaging Retrieval (.rvvis) ===\n");

    let dim = 512;
    let num_images = 150;

    let modalities = ["CT", "MRI", "XRay", "Ultrasound"];
    let body_regions = ["chest", "abdomen", "head", "spine", "pelvis"];
    let findings = ["normal", "fracture", "tumor", "pneumonia"];

    // ====================================================================
    // 1. Create store with .rvvis extension (DomainProfile::RvVision)
    // ====================================================================
    println!("--- 1. Create Radiology Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("radiology.rvvis");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created with .rvvis extension (DomainProfile::RvVision)");
    println!("  Dimensions: {} (image embedding space)", dim);

    // ====================================================================
    // 2. Insert 150 image embeddings with radiology metadata
    // ====================================================================
    println!("\n--- 2. Ingest Image Embeddings ---");

    let vectors: Vec<Vec<f32>> = (0..num_images)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_images as u64).collect();

    // Metadata: modality (0), body_region (1), patient_age (2), finding (3)
    let mut metadata = Vec::with_capacity(num_images * 4);
    for i in 0..num_images {
        let modality = modalities[i % modalities.len()];
        let region = body_regions[i % body_regions.len()];
        let age = (20 + (i * 3 + 7) % 61) as u64; // ages 20-80
        let finding = findings[i % findings.len()];

        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(modality.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(region.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(age),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::String(finding.to_string()),
        });
    }

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!("  Ingested {} image embeddings (rejected: {})", ingest.accepted, ingest.rejected);

    // Print distribution
    for m in &modalities {
        let count = (0..num_images).filter(|i| modalities[i % modalities.len()] == *m).count();
        println!("    {}: {} images", m, count);
    }

    // ====================================================================
    // 3. Similar case search
    // ====================================================================
    println!("\n--- 3. Similar Case Search ---");

    let query_vec = random_vector(dim, 42);
    let k = 10;

    let results = store
        .query(&query_vec, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Top-{} similar cases (unfiltered):", k);
    print_imaging_results(&results, &modalities, &body_regions, &findings);

    // ====================================================================
    // 4. Filter by modality: MRI only
    // ====================================================================
    println!("\n--- 4. MRI Cases Only ---");

    let filter_mri = FilterExpr::Eq(0, FilterValue::String("MRI".to_string()));
    let opts_mri = QueryOptions {
        filter: Some(filter_mri),
        ..Default::default()
    };
    let results_mri = store
        .query(&query_vec, k, &opts_mri)
        .expect("filtered query failed");

    println!("  Top-{} MRI cases:", k);
    print_imaging_results(&results_mri, &modalities, &body_regions, &findings);

    for r in &results_mri {
        let mod_idx = (r.id as usize) % modalities.len();
        assert_eq!(modalities[mod_idx], "MRI");
    }
    println!("  All results verified: modality == MRI.");

    // ====================================================================
    // 5. Combined filter: CT AND tumor
    // ====================================================================
    println!("\n--- 5. CT Tumor Cases ---");

    let filter_ct_tumor = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("CT".to_string())),
        FilterExpr::Eq(3, FilterValue::String("tumor".to_string())),
    ]);
    let opts_ct_tumor = QueryOptions {
        filter: Some(filter_ct_tumor),
        ..Default::default()
    };
    let results_ct_tumor = store
        .query(&query_vec, k, &opts_ct_tumor)
        .expect("filtered query failed");

    println!("  CT + tumor cases found: {}", results_ct_tumor.len());
    if !results_ct_tumor.is_empty() {
        print_imaging_results(&results_ct_tumor, &modalities, &body_regions, &findings);
    }

    let eligible_ct_tumor = (0..num_images)
        .filter(|&i| {
            modalities[i % modalities.len()] == "CT"
                && findings[i % findings.len()] == "tumor"
        })
        .count();
    println!(
        "  Eligible in dataset: {} ({:.1}% selectivity)",
        eligible_ct_tumor,
        eligible_ct_tumor as f64 / num_images as f64 * 100.0
    );

    // ====================================================================
    // 6. Filter by finding: pneumonia cases
    // ====================================================================
    println!("\n--- 6. Pneumonia Cases ---");

    let filter_pneumonia = FilterExpr::Eq(3, FilterValue::String("pneumonia".to_string()));
    let opts_pneumonia = QueryOptions {
        filter: Some(filter_pneumonia),
        ..Default::default()
    };
    let results_pneumonia = store
        .query(&query_vec, k, &opts_pneumonia)
        .expect("filtered query failed");

    println!("  Pneumonia cases found: {}", results_pneumonia.len());
    if !results_pneumonia.is_empty() {
        print_imaging_results(&results_pneumonia, &modalities, &body_regions, &findings);
    }

    // ====================================================================
    // 7. Audit trail witness chain
    // ====================================================================
    println!("\n--- 7. Audit Trail (Witness Chain) ---");

    let audit_steps = [
        ("image_acquisition", 0x01u8),     // PROVENANCE
        ("dicom_parsing", 0x02),           // COMPUTATION
        ("embedding_extraction", 0x02),    // COMPUTATION
        ("case_indexing", 0x08),           // DATA_PROVENANCE
        ("similarity_search", 0x02),       // COMPUTATION
        ("report_generation", 0x08),       // DATA_PROVENANCE
    ];

    let entries: Vec<WitnessEntry> = audit_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("radiology:{}:{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 30_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Audit chain: {} entries, VALID", verified.len());

    println!("\n  Audit steps:");
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
    println!("\n=== Medical Imaging Summary ===\n");
    println!("  Domain profile:    RvVision (.rvvis)");
    println!("  Images indexed:    {}", num_images);
    println!("  Embedding dims:    {}", dim);
    println!("  Unfiltered:        {} results", results.len());
    println!("  MRI only:          {} results", results_mri.len());
    println!("  CT + tumor:        {} results", results_ct_tumor.len());
    println!("  Pneumonia:         {} results", results_pneumonia.len());
    println!("  Audit trail:       {} entries", audit_steps.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_imaging_results(
    results: &[SearchResult],
    modalities: &[&str],
    body_regions: &[&str],
    findings: &[&str],
) {
    println!(
        "    {:>6}  {:>12}  {:>10}  {:>8}  {:>4}  {:>10}",
        "ID", "Distance", "Modality", "Region", "Age", "Finding"
    );
    println!(
        "    {:->6}  {:->12}  {:->10}  {:->8}  {:->4}  {:->10}",
        "", "", "", "", "", ""
    );
    for r in results {
        let idx = r.id as usize;
        let modality = modalities[idx % modalities.len()];
        let region = body_regions[idx % body_regions.len()];
        let age = 20 + (idx * 3 + 7) % 61;
        let finding = findings[idx % findings.len()];
        println!(
            "    {:>6}  {:>12.6}  {:>10}  {:>8}  {:>4}  {:>10}",
            r.id, r.distance, modality, region, age, finding
        );
    }
}
