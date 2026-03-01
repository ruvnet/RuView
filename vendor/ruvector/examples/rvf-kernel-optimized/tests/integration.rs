//! Integration tests for the verified RVF kernel-optimized example.
//! All tests use tempfile and from_builtin_minimal() — no QEMU required.

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};

fn temp_store(dim: u16) -> (tempfile::TempDir, std::path::PathBuf, RvfStore) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rvf");
    let options = RvfOptions {
        dimension: dim,
        ..RvfOptions::default()
    };
    let store = RvfStore::create(&path, options).unwrap();
    let p = path.clone();
    (dir, p, store)
}

#[test]
fn test_kernel_embed() {
    let (_dir, _path, mut store) = temp_store(384);
    let result = rvf_kernel_optimized::kernel_embed::embed_optimized_kernel(
        &mut store,
        rvf_kernel_optimized::KERNEL_CMDLINE,
        false, // no eBPF
        384,
    )
    .unwrap();

    assert!(result.kernel_size > 0);
    assert_eq!(result.ebpf_programs, 0);
    assert!(result.kernel_hash.iter().any(|&b| b != 0));
    store.close().unwrap();
}

#[test]
fn test_ebpf_embed_all_three() {
    let (_dir, _path, mut store) = temp_store(384);
    let result = rvf_kernel_optimized::kernel_embed::embed_optimized_kernel(
        &mut store,
        "console=ttyS0",
        true,
        384,
    )
    .unwrap();

    assert_eq!(result.ebpf_programs, 3);
    store.close().unwrap();
}

#[test]
fn test_verified_ingest_small_batch() {
    let (_dir, _path, mut store) = temp_store(384);

    let mut pipeline = rvf_kernel_optimized::verified_ingest::VerifiedIngestPipeline::new(384);

    let vectors: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5f32; 384]).collect();
    let ids: Vec<u64> = (0..10).collect();

    let ingested = pipeline
        .verify_and_ingest(&mut store, &vectors, &ids)
        .unwrap();
    assert_eq!(ingested, 10);

    let stats = pipeline.stats();
    assert!(stats.proofs_generated > 0);
    assert_eq!(stats.attestations_created, 1);
    store.close().unwrap();
}

#[test]
fn test_verified_ingest_dim_mismatch() {
    let (_dir, _path, mut store) = temp_store(384);

    let mut pipeline = rvf_kernel_optimized::verified_ingest::VerifiedIngestPipeline::new(384);

    // Wrong dimension: 128 instead of 384
    let vectors: Vec<Vec<f32>> = vec![vec![0.5f32; 128]];
    let ids: Vec<u64> = vec![0];

    let result = pipeline.verify_and_ingest(&mut store, &vectors, &ids);
    assert!(result.is_err());
    store.close().unwrap();
}

#[test]
fn test_gated_routing_reflex() {
    use ruvector_verified::gated::{self, ProofKind, ProofTier};

    let env = ruvector_verified::ProofEnvironment::new();
    let decision = gated::route_proof(ProofKind::Reflexivity, &env);
    assert!(matches!(decision.tier, ProofTier::Reflex));
}

#[test]
fn test_arena_dedup_rate() {
    let arena = ruvector_verified::fast_arena::FastTermArena::with_capacity(256);

    // First intern is a miss
    let (_, was_cached) = arena.intern(42);
    assert!(!was_cached);

    // Subsequent interns of same hash are hits
    for _ in 0..99 {
        let (_, was_cached) = arena.intern(42);
        assert!(was_cached);
    }

    let stats = arena.stats();
    assert!(stats.cache_hit_rate() > 0.98);
}

#[test]
fn test_attestation_serialization() {
    let env = ruvector_verified::ProofEnvironment::new();
    let att = ruvector_verified::proof_store::create_attestation(&env, 0);
    let bytes = att.to_bytes();

    assert!(!bytes.is_empty());

    let recovered = ruvector_verified::ProofAttestation::from_bytes(&bytes).unwrap();
    assert_eq!(att.content_hash(), recovered.content_hash());
}

#[test]
fn test_full_pipeline() {
    let (_dir, path, mut store) = temp_store(384);

    // Embed kernel
    rvf_kernel_optimized::kernel_embed::embed_optimized_kernel(
        &mut store,
        rvf_kernel_optimized::KERNEL_CMDLINE,
        true,
        384,
    )
    .unwrap();

    // Verified ingest with 100 vectors
    let (stats, store_size) =
        rvf_kernel_optimized::verified_ingest::run_verified_ingest(&mut store, &path, 384, 100, 42)
            .unwrap();

    assert_eq!(stats.vectors_verified, 100);
    assert!(stats.proofs_generated > 0);
    assert!(stats.attestations_created > 0);
    assert!(store_size > 0);

    // Query
    let query = vec![0.5f32; 384];
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
    assert!(!results.is_empty());

    store.close().unwrap();
}
