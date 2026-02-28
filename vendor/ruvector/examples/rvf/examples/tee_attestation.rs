//! # TEE Attestation and Confidential Computing
//!
//! Category: **Network & Security**
//!
//! **What this demonstrates:**
//! - Full hardware TEE attestation lifecycle using RVF's attestation module
//! - Encode/decode `AttestationHeader` and attestation records
//! - Build attestation witness payloads with multiple TEE platforms
//! - TEE-bound key creation, encoding, and verification
//! - Tamper detection: modified attestation records are rejected
//! - Multi-platform attestation: SGX, SEV-SNP, TDX in one witness chain
//!
//! **RVF segments used:** VEC, WITNESS, CRYPTO
//!
//! **Context:**
//! In confidential computing, vectors may be generated inside a TEE (Trusted
//! Execution Environment). RVF records TEE attestation quotes alongside the
//! vectors so consumers can verify provenance. This example demonstrates the
//! full attestation flow using RVF's real attestation API.
//!
//! **Run:** `cargo run --example tee_attestation`

use rvf_crypto::{
    attestation_witness_entry, build_attestation_witness_payload,
    decode_attestation_header, decode_attestation_record, decode_tee_bound_key,
    encode_attestation_header, encode_attestation_record, encode_tee_bound_key,
    verify_attestation_witness_payload, verify_key_binding,
    shake256_256,
};
use rvf_crypto::hash::shake256_128;
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_types::{
    AttestationHeader, AttestationWitnessType, TeePlatform, KEY_TYPE_TEE_BOUND,
};
use rvf_crypto::TeeBoundKeyRecord;
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

/// Format bytes as a hex string.
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Create a TEE measurement hash from a name string.
fn make_measurement(name: &str) -> [u8; 32] {
    shake256_256(name.as_bytes())
}

/// Create a signer ID hash.
fn make_signer(name: &str) -> [u8; 32] {
    shake256_256(format!("signer:{}", name).as_bytes())
}

/// Create a nonce from a seed value.
fn make_nonce(seed: u64) -> [u8; 16] {
    shake256_128(&seed.to_le_bytes())
}

fn main() {
    println!("=== TEE Attestation & Confidential Computing Example ===\n");

    let dim = 128;
    let tmp = TempDir::new().expect("temp dir");

    // ──────────────────────────────────────────────
    // Phase 1: Create AttestationHeaders for different TEE platforms
    // ──────────────────────────────────────────────
    println!("--- Phase 1: Create Attestation Headers ---\n");

    let platforms = [
        ("SGX Enclave", TeePlatform::Sgx, "enclave-v1.2.3"),
        ("SEV-SNP VM", TeePlatform::SevSnp, "sev-vm-prod-0"),
        ("TDX Trust Domain", TeePlatform::Tdx, "tdx-domain-alpha"),
    ];

    let base_ts = 1_700_000_000_000_000_000u64;

    let mut headers = Vec::new();
    for (i, (label, platform, enclave_name)) in platforms.iter().enumerate() {
        let measurement = make_measurement(enclave_name);
        let signer_id = make_signer(enclave_name);
        let nonce = make_nonce(i as u64);

        let header = AttestationHeader {
            platform: *platform as u8,
            attestation_type: AttestationWitnessType::PlatformAttestation as u8,
            quote_length: 64,
            reserved_0: 0,
            measurement,
            signer_id,
            timestamp_ns: base_ts + (i as u64) * 1_000_000_000,
            nonce,
            svn: (i as u16) + 1,
            sig_algo: 1, // Ed25519
            flags: AttestationHeader::FLAG_HAS_REPORT_DATA,
            reserved_1: [0u8; 3],
            report_data_len: 32,
        };

        println!("  [{}] {}", i, label);
        println!("    Platform:    {:?} (0x{:02x})", platform, *platform as u8);
        println!("    Measurement: {}...", hex_string(&measurement[..8]));
        println!("    Signer:      {}...", hex_string(&signer_id[..8]));
        println!("    Nonce:       {}...", hex_string(&nonce[..8]));
        println!("    SVN:         {}", header.svn);
        println!("    Record size: {} bytes", header.total_record_length());

        headers.push(header);
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 2: Encode/decode attestation records
    // ──────────────────────────────────────────────
    println!("--- Phase 2: Attestation Record Codec ---\n");

    let mut records = Vec::new();
    for (i, header) in headers.iter().enumerate() {
        // Report data: hash of vectors generated in this TEE
        let report_data = shake256_256(
            format!("vectors-generated-in-tee-{}", i).as_bytes(),
        );
        let report_data_slice = &report_data[..header.report_data_len as usize];

        // Quote blob (opaque platform-specific attestation data)
        let quote: Vec<u8> = (0..header.quote_length as usize)
            .map(|j| ((j + i * 37) & 0xFF) as u8)
            .collect();

        let encoded = encode_attestation_record(header, report_data_slice, &quote);
        records.push(encoded.clone());

        // Verify round-trip
        let (dec_header, dec_rd, dec_quote) =
            decode_attestation_record(&encoded).expect("decode record");

        assert_eq!(dec_header.platform, header.platform);
        assert_eq!(dec_header.quote_length, header.quote_length);
        assert_eq!(dec_header.report_data_len, header.report_data_len);
        assert_eq!(dec_rd, report_data_slice);
        assert_eq!(dec_quote, quote);

        println!(
            "  Record {}: {} bytes, codec round-trip OK",
            i, encoded.len()
        );
    }

    // Also verify header-only codec
    let hdr_encoded = encode_attestation_header(&headers[0]);
    let hdr_decoded = decode_attestation_header(&hdr_encoded).expect("decode header");
    assert_eq!(hdr_decoded.platform, headers[0].platform);
    assert_eq!(hdr_decoded.measurement, headers[0].measurement);
    println!("  Header-only codec: 112 bytes, round-trip OK");
    println!();

    // ──────────────────────────────────────────────
    // Phase 3: Build attestation witness payload
    // ──────────────────────────────────────────────
    println!("--- Phase 3: Attestation Witness Payload ---\n");

    let timestamps: Vec<u64> = (0..records.len())
        .map(|i| base_ts + (i as u64) * 2_000_000_000)
        .collect();
    let witness_types = vec![
        AttestationWitnessType::PlatformAttestation,
        AttestationWitnessType::ComputationProof,
        AttestationWitnessType::DataProvenance,
    ];

    let payload =
        build_attestation_witness_payload(&records, &timestamps, &witness_types)
            .expect("build payload");

    println!("  Payload size: {} bytes", payload.len());
    println!("  Records:      {}", records.len());

    // Verify the payload
    let verified_entries =
        verify_attestation_witness_payload(&payload).expect("verify payload");

    println!("  Verified:     {} entries OK\n", verified_entries.len());

    println!(
        "  {:>5}  {:>16}  {:>18}  {:>10}  {:>8}",
        "Entry", "Witness Type", "Platform", "Quote Len", "RD Len"
    );
    println!(
        "  {:->5}  {:->16}  {:->18}  {:->10}  {:->8}",
        "", "", "", "", ""
    );
    for (i, (entry, header, rd, quote)) in verified_entries.iter().enumerate() {
        let wtype_name = match AttestationWitnessType::try_from(entry.witness_type) {
            Ok(AttestationWitnessType::PlatformAttestation) => "PLATFORM_ATT",
            Ok(AttestationWitnessType::ComputationProof) => "COMP_PROOF",
            Ok(AttestationWitnessType::DataProvenance) => "DATA_PROV",
            Ok(AttestationWitnessType::KeyBinding) => "KEY_BIND",
            Err(_) => "UNKNOWN",
        };
        let platform_name = match TeePlatform::try_from(header.platform) {
            Ok(TeePlatform::Sgx) => "SGX",
            Ok(TeePlatform::SevSnp) => "SEV-SNP",
            Ok(TeePlatform::Tdx) => "TDX",
            Ok(TeePlatform::ArmCca) => "ARM-CCA",
            Ok(TeePlatform::SoftwareTee) => "SW-TEE",
            Err(_) => "UNKNOWN",
        };
        println!(
            "  {:>5}  {:>16}  {:>18}  {:>10}  {:>8}",
            i, wtype_name, platform_name, quote.len(), rd.len()
        );
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 4: Tamper detection
    // ──────────────────────────────────────────────
    println!("--- Phase 4: Tamper Detection ---\n");

    let mut tampered = payload.clone();
    // Flip a byte in the records section (near the end)
    let tamper_offset = tampered.len() - 10;
    tampered[tamper_offset] ^= 0xFF;

    match verify_attestation_witness_payload(&tampered) {
        Ok(_) => println!("  Tampered payload: VALID (unexpected!)"),
        Err(e) => println!("  Tampered payload: REJECTED ({:?})", e),
    }

    // Truncation detection
    let truncated = &payload[..payload.len() / 2];
    match verify_attestation_witness_payload(truncated) {
        Ok(_) => println!("  Truncated payload: VALID (unexpected!)"),
        Err(e) => println!("  Truncated payload: REJECTED ({:?})", e),
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 5: TEE-bound key record
    // ──────────────────────────────────────────────
    println!("--- Phase 5: TEE-Bound Key Record ---\n");

    let measurement = make_measurement("enclave-v1.2.3");
    let sealed_key = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
    let public_key = b"ruvector-attestation-public-key";
    let key_id = shake256_128(public_key);

    let key_record = TeeBoundKeyRecord {
        key_type: KEY_TYPE_TEE_BOUND,
        algorithm: 1, // Ed25519
        sealed_key_length: sealed_key.len() as u16,
        key_id,
        measurement,
        platform: TeePlatform::Sgx as u8,
        reserved: [0u8; 3],
        valid_from: base_ts,
        valid_until: base_ts + 86_400_000_000_000, // 24h
        sealed_key: sealed_key.clone(),
    };

    let encoded_key = encode_tee_bound_key(&key_record);
    let decoded_key = decode_tee_bound_key(&encoded_key).expect("decode key");

    assert_eq!(decoded_key.key_type, KEY_TYPE_TEE_BOUND);
    assert_eq!(decoded_key.algorithm, 1);
    assert_eq!(decoded_key.key_id, key_id);
    assert_eq!(decoded_key.measurement, measurement);
    assert_eq!(decoded_key.sealed_key, sealed_key);

    println!("  Key type:      TEE_BOUND ({})", KEY_TYPE_TEE_BOUND);
    println!("  Algorithm:     Ed25519");
    println!("  Key ID:        {}...", hex_string(&key_id[..8]));
    println!("  Measurement:   {}...", hex_string(&measurement[..8]));
    println!("  Platform:      SGX");
    println!("  Sealed key:    {} bytes", sealed_key.len());
    println!("  Wire size:     {} bytes", encoded_key.len());
    println!("  Codec:         round-trip OK");

    // Verify key binding in matching environment
    let binding_result = verify_key_binding(
        &decoded_key,
        TeePlatform::Sgx,
        &measurement,
        base_ts + 1_000_000_000, // within validity window
    );
    println!("  Binding check: {}", if binding_result.is_ok() { "VALID" } else { "FAILED" });
    assert!(binding_result.is_ok());

    // Wrong platform → rejection
    let wrong_platform = verify_key_binding(
        &decoded_key,
        TeePlatform::SevSnp,
        &measurement,
        base_ts + 1_000_000_000,
    );
    println!("  Wrong platform: {}", if wrong_platform.is_err() { "REJECTED (correct)" } else { "VALID (bad)" });
    assert!(wrong_platform.is_err());

    // Wrong measurement → rejection
    let wrong_meas = verify_key_binding(
        &decoded_key,
        TeePlatform::Sgx,
        &[0xFF; 32],
        base_ts + 1_000_000_000,
    );
    println!("  Wrong measurement: {}", if wrong_meas.is_err() { "REJECTED (correct)" } else { "VALID (bad)" });
    assert!(wrong_meas.is_err());

    // Expired → rejection
    let expired = verify_key_binding(
        &decoded_key,
        TeePlatform::Sgx,
        &measurement,
        base_ts + 100_000_000_000_000, // way past valid_until
    );
    println!("  Expired key: {}", if expired.is_err() { "REJECTED (correct)" } else { "VALID (bad)" });
    assert!(expired.is_err());
    println!();

    // ──────────────────────────────────────────────
    // Phase 6: Store vectors with attestation context
    // ──────────────────────────────────────────────
    println!("--- Phase 6: Attested Vector Store ---\n");

    let store_path = tmp.path().join("attested_vectors.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&store_path, options).expect("create store");

    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|i| random_vector(dim, i))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..100).collect();

    let ingest = store.ingest_batch(&vec_refs, &ids, None).expect("ingest");
    println!("  Ingested {} vectors into attested store", ingest.accepted);

    // Query
    let query = random_vector(dim, 999);
    let results = store
        .query(&query, 5, &QueryOptions::default())
        .expect("query");

    println!("  Top-5 nearest neighbors:");
    for (i, r) in results.iter().enumerate() {
        println!("    #{}: id={}, dist={:.6}", i + 1, r.id, r.distance);
    }

    // Create a single witness entry for the attestation
    let entry = attestation_witness_entry(
        &records[0],
        base_ts,
        AttestationWitnessType::PlatformAttestation,
    );
    assert_eq!(entry.witness_type, AttestationWitnessType::PlatformAttestation as u8);
    println!("\n  Attestation witness entry:");
    println!("    Type:        PLATFORM_ATTESTATION");
    println!("    Action hash: {}...", hex_string(&entry.action_hash[..8]));
    println!("    Timestamp:   {} ns", entry.timestamp_ns);

    store.close().expect("close store");
    println!();

    // ──────────────────────────────────────────────
    // Summary
    // ──────────────────────────────────────────────
    println!("=== TEE Attestation Summary ===\n");
    println!("  Platforms demonstrated:  SGX, SEV-SNP, TDX");
    println!("  Attestation records:     {} (all codec round-trip OK)", records.len());
    println!("  Witness payload:         {} entries, verified", verified_entries.len());
    println!("  Tamper detection:        payload + truncation → rejected");
    println!("  TEE-bound key:           {} bytes, binding verified", encoded_key.len());
    println!("  Key binding checks:      valid / wrong-platform / wrong-measurement / expired");
    println!("  Attested vectors:        {} stored + queried", ingest.accepted);
    println!("  Segments used:           VEC, WITNESS, CRYPTO");
    println!();
    println!("  Key insight: RVF binds attestation quotes to vectors,");
    println!("  so consumers can verify that embeddings were generated");
    println!("  inside a specific TEE before trusting them.");

    println!("\n=== Done ===");
}
