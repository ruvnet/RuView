//! Vertical Domain: Market Signal Analysis with Attestation
//!
//! Demonstrates RVF for financial signal embeddings with cryptographic
//! attestation chains and Ed25519 segment signing.
//!
//! Features:
//!   - 200 signal vectors with market metadata
//!   - PLATFORM_ATTESTATION witness chain entries (software TEE)
//!   - Ed25519 keypair generation and segment signing
//!   - Filtered queries by confidence and signal type
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG, CRYPTO_SEG
//!
//! Run: cargo run --example financial_signals

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_types::{SegmentHeader, SegmentType};
use rvf_crypto::{
    sign_segment, verify_segment,
    create_witness_chain, verify_witness_chain, WitnessEntry,
    shake256_256,
};
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
    println!("=== Financial Signal Analysis with Attestation ===\n");

    let dim = 256;
    let num_signals = 200;

    let tickers = ["AAPL", "GOOGL", "MSFT", "AMZN", "TSLA",
                    "META", "NVDA", "JPM", "GS", "BAC"];
    let signal_types = ["momentum", "mean_revert", "volatility", "sentiment"];

    // ====================================================================
    // 1. Create store for market signal embeddings
    // ====================================================================
    println!("--- 1. Create Market Signal Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("signals.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created: {} dims, L2 metric", dim);

    // ====================================================================
    // 2. Insert 200 signal vectors with metadata
    // ====================================================================
    println!("\n--- 2. Ingest Signal Embeddings ---");

    let vectors: Vec<Vec<f32>> = (0..num_signals)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_signals as u64).collect();

    // Metadata: ticker (0), signal_type (1), timestamp (2), confidence (3)
    let mut metadata = Vec::with_capacity(num_signals * 4);
    for i in 0..num_signals {
        let ticker = tickers[i % tickers.len()];
        let sig_type = signal_types[i % signal_types.len()];
        let confidence = ((i * 13 + 37) % 101) as u64;
        let timestamp = 1_700_000_000 + (i as u64) * 300; // 5-min intervals

        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(ticker.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(sig_type.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(timestamp),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::U64(confidence),
        });
    }

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!("  Ingested {} signals (rejected: {})", ingest.accepted, ingest.rejected);

    // Print signal distribution
    let momentum_count = (0..num_signals).filter(|i| i % signal_types.len() == 0).count();
    let high_conf_count = (0..num_signals).filter(|&i| ((i * 13 + 37) % 101) > 80).count();
    println!("  Momentum signals: {}", momentum_count);
    println!("  High confidence (>80): {}", high_conf_count);

    // ====================================================================
    // 3. Build witness chain with PLATFORM_ATTESTATION entries
    // ====================================================================
    println!("\n--- 3. Attestation Witness Chain ---");

    let attestation_steps = [
        ("tee_init", 0x05u8),           // PLATFORM_ATTESTATION
        ("data_ingest", 0x08),          // DATA_PROVENANCE
        ("signal_compute", 0x07),       // COMPUTATION_PROOF
        ("result_attest", 0x05),        // PLATFORM_ATTESTATION
        ("chain_seal", 0x08),           // DATA_PROVENANCE
    ];

    let entries: Vec<WitnessEntry> = attestation_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("fintech:{}:epoch_{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 1_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Chain created: {} entries, {} bytes", verified.len(), chain_bytes.len());

    println!("\n  Attestation chain:");
    for (i, (step, _)) in attestation_steps.iter().enumerate() {
        let wtype_name = match verified[i].witness_type {
            0x05 => "PLAT_ATT",
            0x07 => "COMP_PRF",
            0x08 => "DATA_PRV",
            _ => "UNKNOWN",
        };
        println!("    [{}] {} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // 4. Ed25519 signing
    // ====================================================================
    println!("\n--- 4. Ed25519 Segment Signing ---");

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let mut header = SegmentHeader::new(SegmentType::Vec as u8, 1);
    header.timestamp_ns = 1_700_000_000_000_000_000;
    header.payload_length = 1024;

    let payload = b"Market signal embeddings for AAPL momentum cluster";

    let footer = sign_segment(&header, payload, &signing_key);
    let valid = verify_segment(&header, payload, &footer, &verifying_key);

    println!("  Algorithm:     Ed25519");
    println!("  Public key:    {}...", hex_string(&verifying_key.to_bytes()[..16]));
    println!("  Signature:     {}...", hex_string(&footer.signature[..16]));
    println!("  Verification:  {}", if valid { "VALID" } else { "INVALID" });
    assert!(valid, "signature should be valid");

    // ====================================================================
    // 5. Query high-confidence signals (confidence > 80)
    // ====================================================================
    println!("\n--- 5. High-Confidence Signal Query ---");

    let query_vec = random_vector(dim, 42);
    let k = 10;

    let filter_high_conf = FilterExpr::Gt(3, FilterValue::U64(80));
    let opts_conf = QueryOptions {
        filter: Some(filter_high_conf),
        ..Default::default()
    };
    let results_high = store
        .query(&query_vec, k, &opts_conf)
        .expect("query failed");

    println!("  Top-{} signals with confidence > 80:", k);
    print_signal_results(&results_high, &tickers, &signal_types);

    for r in &results_high {
        let conf = ((r.id as usize) * 13 + 37) % 101;
        assert!(conf > 80, "ID {} has confidence {} <= 80", r.id, conf);
    }
    println!("  All results verified: confidence > 80.");

    // ====================================================================
    // 6. Filter by signal type: momentum signals only
    // ====================================================================
    println!("\n--- 6. Strategy-Specific Filter (Momentum) ---");

    let filter_momentum = FilterExpr::Eq(1, FilterValue::String("momentum".to_string()));
    let opts_momentum = QueryOptions {
        filter: Some(filter_momentum),
        ..Default::default()
    };
    let results_momentum = store
        .query(&query_vec, k, &opts_momentum)
        .expect("query failed");

    println!("  Top-{} momentum signals:", k);
    print_signal_results(&results_momentum, &tickers, &signal_types);

    for r in &results_momentum {
        let sig_idx = (r.id as usize) % signal_types.len();
        assert_eq!(signal_types[sig_idx], "momentum");
    }
    println!("  All results verified: signal_type == momentum.");

    // ====================================================================
    // 7. Combined filter: momentum AND high confidence
    // ====================================================================
    println!("\n--- 7. Combined Filter (Momentum + High Confidence) ---");

    let filter_combined = FilterExpr::And(vec![
        FilterExpr::Eq(1, FilterValue::String("momentum".to_string())),
        FilterExpr::Gt(3, FilterValue::U64(80)),
    ]);
    let opts_combined = QueryOptions {
        filter: Some(filter_combined),
        ..Default::default()
    };
    let results_combined = store
        .query(&query_vec, k, &opts_combined)
        .expect("query failed");

    println!("  Momentum signals with confidence > 80: {}", results_combined.len());
    if !results_combined.is_empty() {
        print_signal_results(&results_combined, &tickers, &signal_types);
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Financial Signals Summary ===\n");
    println!("  Total signals:           {}", num_signals);
    println!("  Embedding dimensions:    {}", dim);
    println!("  Attestation chain:       {} entries", attestation_steps.len());
    println!("  Ed25519 signature:       VALID");
    println!("  High confidence (>80):   {} results", results_high.len());
    println!("  Momentum signals:        {} results", results_momentum.len());
    println!("  Combined filter:         {} results", results_combined.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_signal_results(
    results: &[SearchResult],
    tickers: &[&str],
    signal_types: &[&str],
) {
    println!(
        "    {:>6}  {:>12}  {:>6}  {:>12}  {:>6}",
        "ID", "Distance", "Ticker", "Signal", "Conf"
    );
    println!("    {:->6}  {:->12}  {:->6}  {:->12}  {:->6}", "", "", "", "", "");
    for r in results {
        let idx = r.id as usize;
        let ticker = tickers[idx % tickers.len()];
        let sig_type = signal_types[idx % signal_types.len()];
        let conf = (idx * 13 + 37) % 101;
        println!(
            "    {:>6}  {:>12.6}  {:>6}  {:>12}  {:>6}",
            r.id, r.distance, ticker, sig_type, conf
        );
    }
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
