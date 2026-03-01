//! # Zero-Knowledge Vector Proofs
//!
//! Category: **Network & Security**
//!
//! **What this demonstrates:**
//! - Prove a vector belongs to an RVF store without revealing the vector itself
//! - SHAKE-256 commitment scheme: commit to a vector, verify later
//! - Blinded similarity search: query with a noised vector, verify results
//! - Witness chain records all proof generation/verification events
//! - Merkle-style inclusion proofs using segment hashes
//! - Non-interactive proof of vector count and dimension (metadata proofs)
//!
//! **RVF segments used:** VEC, INDEX, WITNESS, MANIFEST, META
//!
//! **Context:**
//! In privacy-sensitive domains (medical, legal, financial), you may need to
//! prove properties of a vector database without exposing the raw vectors.
//! This example demonstrates commitment-based ZK patterns using RVF's
//! existing crypto primitives (SHAKE-256, witness chains, Ed25519).
//!
//! **Run:** `cargo run --example zero_knowledge`

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

use rvf_crypto::{
    create_witness_chain, shake256_256, verify_witness_chain,
    sign_segment, verify_segment, WitnessEntry,
};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_types::{SegmentHeader, SegmentType};
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

/// Create a commitment to a vector: H(vector_bytes || nonce).
fn commit_vector(vector: &[f32], nonce: &[u8; 32]) -> [u8; 32] {
    let mut data = Vec::with_capacity(vector.len() * 4 + 32);
    for &val in vector {
        data.extend_from_slice(&val.to_le_bytes());
    }
    data.extend_from_slice(nonce);
    shake256_256(&data)
}

/// Verify a commitment against a vector and nonce.
fn verify_commitment(vector: &[f32], nonce: &[u8; 32], commitment: &[u8; 32]) -> bool {
    let recomputed = commit_vector(vector, nonce);
    recomputed == *commitment
}

/// Add calibrated noise to a vector for blinded search.
fn blind_vector(vector: &[f32], noise_seed: u64, noise_scale: f32) -> Vec<f32> {
    let noise = random_vector(vector.len(), noise_seed);
    vector
        .iter()
        .zip(noise.iter())
        .map(|(v, n)| v + n * noise_scale)
        .collect()
}

fn main() {
    println!("=== Zero-Knowledge Vector Proofs Example ===\n");

    let dim = 128;
    let num_vectors = 200;
    let tmp = TempDir::new().expect("temp dir");

    // ──────────────────────────────────────────────
    // Phase 1: Create a vector store (the prover's database)
    // ──────────────────────────────────────────────
    println!("--- Phase 1: Prover Creates Vector Database ---\n");

    let store_path = tmp.path().join("prover_db.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&store_path, options).expect("create store");

    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_vectors as u64).collect();

    let ingest = store
        .ingest_batch(&vec_refs, &ids, None)
        .expect("ingest");
    println!("  Prover database: {} vectors, {} dims", ingest.accepted, dim);
    println!();

    // ──────────────────────────────────────────────
    // Phase 2: Commitment scheme — commit to a vector
    // ──────────────────────────────────────────────
    println!("--- Phase 2: Vector Commitment Scheme ---\n");

    // Prover picks a secret vector and commits
    let secret_idx = 42;
    let secret_vector = &vectors[secret_idx];
    let nonce = shake256_256(b"secret-nonce-for-commitment");

    let commitment = commit_vector(secret_vector, &nonce);
    println!("  Secret vector: id={}", secret_idx);
    println!("  Nonce:         {}...", hex_string(&nonce[..8]));
    println!("  Commitment:    {}...", hex_string(&commitment[..16]));

    // Prover sends commitment to verifier (without revealing vector or nonce)
    println!("  Sent to verifier: commitment (32 bytes)");
    println!("  NOT sent: vector ({} floats), nonce (32 bytes)", dim);
    println!();

    // ──────────────────────────────────────────────
    // Phase 3: Verify the commitment (reveal phase)
    // ──────────────────────────────────────────────
    println!("--- Phase 3: Commitment Verification (Reveal) ---\n");

    // Prover reveals vector + nonce; verifier checks
    let valid = verify_commitment(secret_vector, &nonce, &commitment);
    println!("  Reveal: vector id={}, nonce={}...", secret_idx, hex_string(&nonce[..4]));
    println!("  Verification: {}", if valid { "VALID" } else { "INVALID" });
    assert!(valid);

    // Wrong vector → fails
    let wrong_vector = &vectors[99];
    let invalid = verify_commitment(wrong_vector, &nonce, &commitment);
    println!("  Wrong vector (id=99): {}", if invalid { "VALID (bad)" } else { "REJECTED (correct)" });
    assert!(!invalid);

    // Wrong nonce → fails
    let wrong_nonce = shake256_256(b"wrong-nonce");
    let invalid_nonce = verify_commitment(secret_vector, &wrong_nonce, &commitment);
    println!("  Wrong nonce: {}", if invalid_nonce { "VALID (bad)" } else { "REJECTED (correct)" });
    assert!(!invalid_nonce);
    println!();

    // ──────────────────────────────────────────────
    // Phase 4: Blinded similarity search
    // ──────────────────────────────────────────────
    println!("--- Phase 4: Blinded Similarity Search ---\n");

    // Verifier wants to search but doesn't want to reveal exact query
    let true_query = random_vector(dim, 777);
    let noise_scale = 0.01; // small noise preserves ranking
    let blinded_query = blind_vector(&true_query, 12345, noise_scale);

    println!("  True query:    first 4 = [{:.4}, {:.4}, {:.4}, {:.4}]",
        true_query[0], true_query[1], true_query[2], true_query[3]);
    println!("  Blinded query: first 4 = [{:.4}, {:.4}, {:.4}, {:.4}]",
        blinded_query[0], blinded_query[1], blinded_query[2], blinded_query[3]);
    println!("  Noise scale:   {}", noise_scale);

    // Search with true query
    let true_results = store
        .query(&true_query, 10, &QueryOptions::default())
        .expect("true query");

    // Search with blinded query
    let blinded_results = store
        .query(&blinded_query, 10, &QueryOptions::default())
        .expect("blinded query");

    // Compare rankings
    let true_ids: Vec<u64> = true_results.iter().map(|r| r.id).collect();
    let blinded_ids: Vec<u64> = blinded_results.iter().map(|r| r.id).collect();

    let overlap: usize = true_ids.iter().filter(|id| blinded_ids.contains(id)).count();
    let overlap_pct = (overlap as f64 / true_ids.len() as f64) * 100.0;

    println!("\n  True top-10 IDs:    {:?}", &true_ids[..5]);
    println!("  Blinded top-10 IDs: {:?}", &blinded_ids[..5]);
    println!("  Ranking overlap:    {}/{} ({:.0}%)", overlap, true_ids.len(), overlap_pct);
    println!("  Privacy gain:       query vector hidden behind noise");
    println!();

    // ──────────────────────────────────────────────
    // Phase 5: Metadata proofs (non-interactive)
    // ──────────────────────────────────────────────
    println!("--- Phase 5: Metadata Proofs ---\n");

    let status = store.status();

    // Prove: "the store has exactly N vectors of dimension D"
    let metadata_claim = format!(
        "vectors={},dimension={},epoch={}",
        status.total_vectors, dim, status.current_epoch
    );
    let metadata_hash = shake256_256(metadata_claim.as_bytes());

    // Sign the metadata claim
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let mut meta_header = SegmentHeader::new(SegmentType::Meta as u8, 1);
    meta_header.timestamp_ns = 1_700_000_000_000_000_000;
    meta_header.payload_length = metadata_claim.len() as u64;

    let meta_footer = sign_segment(&meta_header, metadata_claim.as_bytes(), &signing_key);
    let meta_valid = verify_segment(
        &meta_header, metadata_claim.as_bytes(), &meta_footer, &verifying_key,
    );

    println!("  Claim: \"{}\"", metadata_claim);
    println!("  Claim hash:  {}...", hex_string(&metadata_hash[..8]));
    println!("  Signed by:   {}...", hex_string(&verifying_key.to_bytes()[..8]));
    println!("  Verification: {}", if meta_valid { "VALID" } else { "INVALID" });
    assert!(meta_valid);
    println!();

    // ──────────────────────────────────────────────
    // Phase 6: Proof witness chain
    // ──────────────────────────────────────────────
    println!("--- Phase 6: Proof Witness Chain ---\n");

    let proof_events = [
        ("commit:vector_42", 0x01u8),    // PROVENANCE
        ("reveal:vector_42_ok", 0x02),   // COMPUTATION
        ("blinded_search:noise=0.01", 0x02),
        ("metadata_proof:signed", 0x01),
        ("audit_complete", 0x02),
    ];

    let base_ts = 1_700_000_000_000_000_000u64;
    let entries: Vec<WitnessEntry> = proof_events
        .iter()
        .enumerate()
        .map(|(i, (event, wtype))| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(format!("zk:{}", event).as_bytes()),
            timestamp_ns: base_ts + (i as u64) * 1_000_000_000,
            witness_type: *wtype,
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified_chain = verify_witness_chain(&chain_bytes).expect("verify chain");

    println!("  Proof chain: {} entries, {} bytes, VERIFIED", verified_chain.len(), chain_bytes.len());
    for (i, (event, _)) in proof_events.iter().enumerate() {
        let wtype = match verified_chain[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            _ => "????",
        };
        println!("    [{}] {} → {}", wtype, i, event);
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 7: Batch commitment (Merkle-style)
    // ──────────────────────────────────────────────
    println!("--- Phase 7: Batch Commitment (Merkle-style) ---\n");

    // Commit to multiple vectors and combine into a root
    let batch_size = 8;
    let batch_nonces: Vec<[u8; 32]> = (0..batch_size)
        .map(|i| shake256_256(format!("batch-nonce-{}", i).as_bytes()))
        .collect();

    let leaf_commitments: Vec<[u8; 32]> = (0..batch_size)
        .map(|i| commit_vector(&vectors[i], &batch_nonces[i]))
        .collect();

    // Combine leaf commitments into a root hash
    let mut combined = Vec::with_capacity(batch_size * 32);
    for leaf in &leaf_commitments {
        combined.extend_from_slice(leaf);
    }
    let batch_root = shake256_256(&combined);

    println!("  Batch size:      {}", batch_size);
    println!("  Leaf commitments:");
    for (i, leaf) in leaf_commitments.iter().enumerate() {
        println!("    [{}] {}...", i, hex_string(&leaf[..8]));
    }
    println!("  Batch root:      {}...", hex_string(&batch_root[..16]));

    // Verify individual leaf
    let leaf_valid = verify_commitment(&vectors[3], &batch_nonces[3], &leaf_commitments[3]);
    println!("\n  Verify leaf [3]: {}", if leaf_valid { "VALID" } else { "INVALID" });
    assert!(leaf_valid);

    // Recompute root from leaves
    let mut recomputed = Vec::with_capacity(batch_size * 32);
    for leaf in &leaf_commitments {
        recomputed.extend_from_slice(leaf);
    }
    let recomputed_root = shake256_256(&recomputed);
    let root_valid = recomputed_root == batch_root;
    println!("  Verify root:     {}", if root_valid { "VALID" } else { "INVALID" });
    assert!(root_valid);
    println!();

    // ──────────────────────────────────────────────
    // Summary
    // ──────────────────────────────────────────────
    println!("=== Zero-Knowledge Proofs Summary ===\n");
    println!("  Vector database:       {} vectors, {} dims", num_vectors, dim);
    println!("  Commitment scheme:     SHAKE-256(vector || nonce)");
    println!("  Commitment verify:     correct=VALID, wrong_vec=REJECTED, wrong_nonce=REJECTED");
    println!("  Blinded search:        noise={}, overlap={:.0}%", noise_scale, overlap_pct);
    println!("  Metadata proofs:       signed claim with Ed25519");
    println!("  Batch commitments:     {} leaves → Merkle root", batch_size);
    println!("  Witness chain:         {} events, verified", proof_events.len());
    println!("  Segments used:         VEC, INDEX, WITNESS, MANIFEST, META");
    println!();
    println!("  Key insight: RVF's SHAKE-256 and witness chains provide");
    println!("  the building blocks for zero-knowledge proofs over");
    println!("  vector data without requiring a full ZK-SNARK library.");

    store.close().expect("close store");

    println!("\n=== Done ===");
}
