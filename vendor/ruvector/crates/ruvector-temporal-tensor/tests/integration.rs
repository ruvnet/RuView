//! End-to-end integration tests for the temporal tensor store.
//!
//! Exercises the full lifecycle: put, get, tier migration, delta compression,
//! quantization quality, eviction, checksums, witness logging, and factor
//! reconstruction.
//!
//! Run via: `cargo test -p ruvector-temporal-tensor --test integration`

use ruvector_temporal_tensor::delta::{
    compute_delta, decode_delta, encode_delta, DeltaChain, FactorSet,
};
use ruvector_temporal_tensor::metrics::{TierChangeReason, WitnessEvent, WitnessLog};
use ruvector_temporal_tensor::quantizer;
use ruvector_temporal_tensor::segment;
use ruvector_temporal_tensor::store::{BlockKey, ReconstructPolicy, StoreError, Tier, TieredStore};
use ruvector_temporal_tensor::tiering::{self, TierConfig};
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

// ---------------------------------------------------------------------------
// Deterministic PRNG (LCG) -- no external deps
// ---------------------------------------------------------------------------

/// Simple linear congruential generator. Constants from Knuth MMIX.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_key(tid: u128, idx: u32) -> BlockKey {
    BlockKey {
        tensor_id: tid,
        block_index: idx,
    }
}

/// Map tiering module Tier to store module Tier.
fn tiering_to_store_tier(t: tiering::Tier) -> Tier {
    match t {
        tiering::Tier::Tier0 => Tier::Tier0,
        tiering::Tier::Tier1 => Tier::Tier1,
        tiering::Tier::Tier2 => Tier::Tier2,
        tiering::Tier::Tier3 => Tier::Tier3,
    }
}

// ===========================================================================
// 1. Full Lifecycle Test
// ===========================================================================

/// Put 100 blocks as hot, simulate 1000 ticks touching only 10, then verify
/// that the 90 untouched blocks migrate to colder tiers.
#[test]
fn test_full_lifecycle() {
    let mut store = TieredStore::new(4096);
    let tier_config = TierConfig::default();
    let n_elems = 64;

    let mut rng = SimpleRng::new(42);
    let block_data: Vec<Vec<f32>> = (0..100)
        .map(|_| (0..n_elems).map(|_| rng.next_f32() * 2.0 - 1.0).collect())
        .collect();

    // Put 100 blocks as Tier1 (hot).
    for i in 0..100u32 {
        store
            .put(make_key(1, i), &block_data[i as usize], Tier::Tier1, 0)
            .unwrap();
    }
    assert_eq!(store.tier_count(Tier::Tier1), 100);
    assert_eq!(store.block_count(), 100);

    // Parallel tiering metadata for migration scoring.
    let mut tiering_metas: Vec<tiering::BlockMeta> =
        (0..100).map(|_| tiering::BlockMeta::new(0)).collect();

    // Simulate 1000 ticks -- only blocks 0..10 are accessed.
    for tick in 1..=1000u64 {
        for i in 0..10 {
            store.touch(make_key(1, i as u32), tick);
            tiering::touch(&tier_config, tick, &mut tiering_metas[i]);
        }
        for i in 10..100 {
            tiering::tick_decay(&tier_config, &mut tiering_metas[i]);
        }
    }

    // Apply tier migration decisions.
    let mut migrated = 0u32;
    for i in 0..100u32 {
        if let Some(target) = tiering::choose_tier(&tier_config, 1000, &tiering_metas[i as usize]) {
            let st = tiering_to_store_tier(target);
            if st != Tier::Tier0 {
                store
                    .put(make_key(1, i), &block_data[i as usize], st, 1000)
                    .unwrap();
                migrated += 1;
            }
        }
    }

    let tier1 = store.tier_count(Tier::Tier1);
    let tier2 = store.tier_count(Tier::Tier2);
    let tier3 = store.tier_count(Tier::Tier3);

    assert!(migrated > 0, "expected migrations, got none");
    assert!(
        tier1 < 100,
        "expected fewer Tier1 blocks after migration, got {}",
        tier1
    );
    assert!(tier1 <= 20, "hot blocks should be ~10, got {}", tier1);
    assert!(
        tier2 + tier3 >= 80,
        "expected >=80 in lower tiers, got {} + {}",
        tier2,
        tier3
    );
    assert_eq!(store.block_count(), 100);
}

// ===========================================================================
// 2. Delta Chain Lifecycle Test
// ===========================================================================

/// Build a delta chain with 5 incremental deltas, reconstruct, compact,
/// verify encode/decode roundtrip.
#[test]
fn test_delta_chain_lifecycle() {
    let n = 256;
    let mut rng = SimpleRng::new(99);
    let base: Vec<f32> = (0..n).map(|_| rng.next_f32() * 2.0 - 1.0).collect();
    let mut chain = DeltaChain::new(base.clone(), 8);

    // Build 5 incremental deltas (~10% change each).
    let mut current = base.clone();
    for epoch in 0..5u64 {
        let mut next = current.clone();
        for i in 0..n {
            if (rng.next_u64() % 10) == 0 {
                next[i] += (rng.next_f32() - 0.5) * 0.1;
            }
        }
        let delta = compute_delta(&current, &next, 1, 0, epoch, 0.001, 0.5)
            .expect("delta should be computable for ~10% change");
        chain.append(delta).unwrap();
        current = next;
    }
    assert_eq!(chain.chain_len(), 5);

    // Reconstruct and verify accuracy against the final state.
    let reconstructed = chain.reconstruct();
    assert_eq!(reconstructed.len(), n);
    for i in 0..n {
        let err = (reconstructed[i] - current[i]).abs();
        assert!(
            err < 0.01,
            "recon err at {}: {} vs {} (err={})",
            i,
            reconstructed[i],
            current[i],
            err
        );
    }

    // Encode/decode the last delta and verify roundtrip.
    let last_delta = compute_delta(&base, &current, 1, 0, 99, 0.001, 1.1).unwrap();
    let encoded = encode_delta(&last_delta);
    let decoded = decode_delta(&encoded).unwrap();
    assert_eq!(decoded.header.tensor_id, 1);
    assert_eq!(decoded.entries.len(), last_delta.entries.len());

    // Compact the chain; delta list drops to 0 but state is preserved.
    let before_compact = reconstructed.clone();
    chain.compact();
    assert_eq!(chain.chain_len(), 0);

    let after_compact = chain.reconstruct();
    for i in 0..n {
        let err = (after_compact[i] - before_compact[i]).abs();
        assert!(
            err < 1e-6,
            "compact mismatch at {}: {} vs {}",
            i,
            after_compact[i],
            before_compact[i]
        );
    }
}

// ===========================================================================
// 3. Quantization Quality Sweep
// ===========================================================================

/// For each bit width (8, 7, 5, 3) verify MSE and max relative error
/// stay within ADR-023 bounds.
#[test]
fn test_quality_sweep_all_tiers() {
    let n_elems = 256;
    let mut rng = SimpleRng::new(7777);

    // Sinusoidal + noise with guaranteed minimum magnitude.
    let data: Vec<f32> = (0..n_elems)
        .map(|i| {
            let base = (i as f32 * 0.05).sin();
            let noise = (rng.next_f32() - 0.5) * 0.1;
            let val = base + noise;
            if val.abs() < 0.05 {
                if val >= 0.0 {
                    0.05 + rng.next_f32() * 0.1
                } else {
                    -0.05 - rng.next_f32() * 0.1
                }
            } else {
                val
            }
        })
        .collect();

    let max_abs: f32 = data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);

    // Store-backed tiers: (tier, bound_vs_max, label).
    let store_configs: &[(Tier, f64, &str)] = &[
        (Tier::Tier1, 0.01, "8-bit/Tier1"),
        (Tier::Tier2, 0.02, "7-bit/Tier2"),
        (Tier::Tier3, 0.35, "3-bit/Tier3"),
    ];

    let mut store = TieredStore::new(4096);
    for &(tier, bound, label) in store_configs {
        let key = make_key(tier as u128 + 100, 0);
        store.put(key, &data, tier, 0).unwrap();

        let mut out = vec![0.0f32; n_elems];
        let n = store.get(key, &mut out, 0).unwrap();
        assert_eq!(n, n_elems);

        let mut max_rel = 0.0f64;
        let mut mse = 0.0f64;
        for i in 0..n_elems {
            let err = (data[i] - out[i]) as f64;
            mse += err * err;
            let rel = err.abs() / max_abs as f64;
            if rel > max_rel {
                max_rel = rel;
            }
        }
        mse /= n_elems as f64;

        assert!(
            max_rel < bound,
            "{}: max_rel {:.4} >= bound {:.4} (MSE={:.8})",
            label,
            max_rel,
            bound,
            mse
        );
    }

    // 5-bit via groupwise quantizer directly (no store tier for 5-bit).
    {
        let scales = quantizer::compute_scales(&data, 64, 5);
        let mut packed = Vec::new();
        quantizer::quantize_and_pack(&data, &scales, 64, 5, &mut packed);
        let mut decoded = Vec::new();
        quantizer::dequantize(&packed, &scales, 64, 5, n_elems, 1, &mut decoded);

        let mut max_rel = 0.0f64;
        for i in 0..n_elems {
            let err = (data[i] - decoded[i]) as f64;
            let rel = err.abs() / max_abs as f64;
            if rel > max_rel {
                max_rel = rel;
            }
        }
        assert!(max_rel < 0.07, "5-bit: max_rel {:.4} >= 0.07", max_rel);
    }
}

// ===========================================================================
// 4. Store Persistence Roundtrip
// ===========================================================================

/// Put 50 blocks with varied data and tiers, get each back and verify data
/// and metadata.
#[test]
fn test_store_put_get_roundtrip() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(1234);
    let n_elems = 64;
    let tiers = [Tier::Tier1, Tier::Tier2, Tier::Tier3];

    let mut block_data: Vec<Vec<f32>> = Vec::new();
    let mut block_tiers: Vec<Tier> = Vec::new();

    for i in 0..50u32 {
        let d: Vec<f32> = (0..n_elems).map(|_| rng.next_f32() * 2.0 - 1.0).collect();
        let tier = tiers[(i % 3) as usize];
        store.put(make_key(42, i), &d, tier, i as u64).unwrap();
        block_data.push(d);
        block_tiers.push(tier);
    }
    assert_eq!(store.block_count(), 50);

    for i in 0..50u32 {
        let key = make_key(42, i);
        let mut out = vec![0.0f32; n_elems];
        let n = store.get(key, &mut out, i as u64).unwrap();
        assert_eq!(n, n_elems);

        let meta = store.meta(key).unwrap();
        assert_eq!(meta.tier, block_tiers[i as usize]);
        assert_eq!(meta.created_at, i as u64);

        let max_abs: f32 = block_data[i as usize]
            .iter()
            .map(|v| v.abs())
            .fold(0.0f32, f32::max);
        let tol = match block_tiers[i as usize] {
            Tier::Tier1 => max_abs * 0.01,
            Tier::Tier2 => max_abs * 0.02,
            Tier::Tier3 => max_abs * 0.35,
            Tier::Tier0 => unreachable!(),
        }
        .max(1e-6);

        for j in 0..n_elems {
            let err = (block_data[i as usize][j] - out[j]).abs();
            assert!(err < tol, "block {} elem {}: err={} tol={}", i, j, err, tol);
        }
    }
}

// ===========================================================================
// 5. Eviction and Tier0
// ===========================================================================

/// Put a block at Tier1, evict it, verify reads fail and metadata reflects
/// eviction state.
#[test]
fn test_eviction_to_tier0() {
    let mut store = TieredStore::new(4096);
    let key = make_key(1, 0);
    let data = vec![1.0f32; 64];

    store.put(key, &data, Tier::Tier1, 0).unwrap();
    assert_eq!(store.tier_count(Tier::Tier1), 1);
    assert!(store.total_bytes() > 0);

    store.evict(key, ReconstructPolicy::None).unwrap();

    // Read should fail.
    let mut out = vec![0.0f32; 64];
    assert_eq!(store.get(key, &mut out, 1), Err(StoreError::TensorEvicted));

    // Metadata should reflect Tier0.
    let meta = store.meta(key).unwrap();
    assert_eq!(meta.tier, Tier::Tier0);
    assert_eq!(meta.bits, 0);
    assert_eq!(meta.block_bytes, 0);
    assert_eq!(meta.reconstruct, ReconstructPolicy::None);

    assert_eq!(store.tier_count(Tier::Tier1), 0);
    assert_eq!(store.tier_count(Tier::Tier0), 1);
    assert_eq!(store.block_count(), 1);
    assert_eq!(store.total_bytes(), 0);
}

// ===========================================================================
// 6. Checksum Integrity
// ===========================================================================

/// Verify that checksums are non-zero and deterministic for the same data.
#[test]
fn test_checksum_integrity() {
    let mut store = TieredStore::new(4096);
    let data: Vec<f32> = (0..128).map(|i| (i as f32) * 0.1).collect();

    let key1 = make_key(1, 0);
    store.put(key1, &data, Tier::Tier1, 0).unwrap();
    let cksum1 = store.meta(key1).unwrap().checksum;
    assert_ne!(
        cksum1, 0,
        "checksum should be non-zero for non-trivial data"
    );

    // Same data under a different key produces the same checksum.
    let key2 = make_key(1, 1);
    store.put(key2, &data, Tier::Tier1, 0).unwrap();
    assert_eq!(store.meta(key2).unwrap().checksum, cksum1);

    // Different data produces a different checksum.
    let other: Vec<f32> = (0..128).map(|i| (i as f32) * 0.2).collect();
    let key3 = make_key(1, 2);
    store.put(key3, &other, Tier::Tier1, 0).unwrap();
    assert_ne!(store.meta(key3).unwrap().checksum, cksum1);
}

// ===========================================================================
// 7. Multi-Tensor Store
// ===========================================================================

/// Blocks from 3 different tensor_ids are stored and retrieved independently.
#[test]
fn test_multiple_tensors() {
    let mut store = TieredStore::new(4096);
    let n_elems = 32;
    let mut rng = SimpleRng::new(555);

    let tensor_ids: [u128; 3] = [100, 200, 300];
    let mut all_data: Vec<Vec<Vec<f32>>> = Vec::new();

    for &tid in &tensor_ids {
        let mut tensor_blocks = Vec::new();
        for blk in 0..5u32 {
            let d: Vec<f32> = (0..n_elems).map(|_| rng.next_f32() * 2.0 - 1.0).collect();
            store.put(make_key(tid, blk), &d, Tier::Tier1, 0).unwrap();
            tensor_blocks.push(d);
        }
        all_data.push(tensor_blocks);
    }
    assert_eq!(store.block_count(), 15);

    for (t_idx, &tid) in tensor_ids.iter().enumerate() {
        for blk in 0..5u32 {
            let key = make_key(tid, blk);
            let mut out = vec![0.0f32; n_elems];
            let n = store.get(key, &mut out, 0).unwrap();
            assert_eq!(n, n_elems);

            let meta = store.meta(key).unwrap();
            assert_eq!(meta.key.tensor_id, tid);
            assert_eq!(meta.key.block_index, blk);

            let orig = &all_data[t_idx][blk as usize];
            let max_abs: f32 = orig.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
            let tol = (max_abs * 0.01).max(1e-6);
            for j in 0..n_elems {
                let err = (orig[j] - out[j]).abs();
                assert!(err < tol, "tid={} blk={} j={}: err={}", tid, blk, j, err);
            }
        }
    }
}

// ===========================================================================
// 8. Stress Test
// ===========================================================================

/// Put 1000 blocks with random tiers, touch random blocks 10000 times,
/// verify no panics and all blocks remain readable.
#[test]
fn test_stress_1000_blocks() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0xDEADBEEF);
    let n_elems = 32;
    let tiers = [Tier::Tier1, Tier::Tier2, Tier::Tier3];

    for i in 0..1000u32 {
        let d: Vec<f32> = (0..n_elems).map(|_| rng.next_f32() * 2.0 - 1.0).collect();
        let tier = tiers[(rng.next_u64() % 3) as usize];
        store.put(make_key(1, i), &d, tier, i as u64).unwrap();
    }
    assert_eq!(store.block_count(), 1000);
    assert!(store.total_bytes() > 0);

    for t in 0..10_000u64 {
        let idx = (rng.next_u64() % 1000) as u32;
        store.touch(make_key(1, idx), 1000 + t);
    }

    for i in 0..1000u32 {
        let mut out = vec![0.0f32; n_elems];
        let n = store.get(make_key(1, i), &mut out, 20_000).unwrap();
        assert_eq!(n, n_elems);
        for j in 0..n_elems {
            assert!(out[j].is_finite(), "block {} elem {} not finite", i, j);
        }
    }
    assert!(store.total_bytes() > 0);
}

// ===========================================================================
// 9. Compressor + Store Integration
// ===========================================================================

/// Compress frames via TemporalTensorCompressor, decode the segment, store
/// each decoded frame as a block, and verify roundtrip.
#[test]
fn test_compressor_to_store() {
    let tensor_len = 128u32;
    let policy = TierPolicy::default();
    let mut comp = TemporalTensorCompressor::new(policy, tensor_len, 0);
    comp.set_access(100, 0); // hot -> 8-bit

    let mut rng = SimpleRng::new(0xCAFE);
    let n_frames = 10usize;

    let frames: Vec<Vec<f32>> = (0..n_frames)
        .map(|_| {
            (0..tensor_len as usize)
                .map(|_| rng.next_f32() * 2.0 - 1.0)
                .collect()
        })
        .collect();

    let mut seg = Vec::new();
    for (i, frame) in frames.iter().enumerate() {
        comp.push_frame(frame, (i + 1) as u32, &mut seg);
    }
    comp.flush(&mut seg);
    assert!(!seg.is_empty(), "compressor should produce a segment");

    let mut decoded = Vec::new();
    segment::decode(&seg, &mut decoded);
    assert_eq!(decoded.len(), tensor_len as usize * n_frames);

    // Store each decoded frame as a block.
    let mut store = TieredStore::new(4096);
    for i in 0..n_frames {
        let start = i * tensor_len as usize;
        let end = start + tensor_len as usize;
        store
            .put(
                make_key(50, i as u32),
                &decoded[start..end],
                Tier::Tier1,
                i as u64,
            )
            .unwrap();
    }
    assert_eq!(store.block_count(), n_frames);

    // Read back and verify against the decoded data (double quantization).
    for i in 0..n_frames {
        let mut out = vec![0.0f32; tensor_len as usize];
        let n = store
            .get(make_key(50, i as u32), &mut out, n_frames as u64)
            .unwrap();
        assert_eq!(n, tensor_len as usize);

        let start = i * tensor_len as usize;
        for j in 0..tensor_len as usize {
            let expected = decoded[start + j];
            let err = (expected - out[j]).abs();
            // Double quantization (compressor + store) compounds error.
            let tol = if expected.abs() > 0.01 {
                expected.abs() * 0.04
            } else {
                0.05
            };
            assert!(
                err < tol,
                "frame {} elem {}: exp={} got={} err={}",
                i,
                j,
                expected,
                out[j],
                err
            );
        }
    }
}

// ===========================================================================
// 10. Factor Reconstruction Quality
// ===========================================================================

/// Create a low-rank matrix, factor it, reconstruct, and verify error is low.
#[test]
fn test_factor_reconstruction_quality() {
    let m = 16;
    let n = 16;

    // Rank-1 matrix: data[i][j] = (i+1)*(j+1) / (m*n).
    let data: Vec<f32> = (0..m * n)
        .map(|idx| {
            let (i, j) = (idx / n, idx % n);
            (i as f32 + 1.0) * (j as f32 + 1.0) / (m * n) as f32
        })
        .collect();

    let factors = FactorSet::from_data(&data, m, n, 1);
    assert_eq!(factors.m, m);
    assert_eq!(factors.n, n);
    assert_eq!(factors.k, 1);

    let reconstructed = factors.reconstruct();
    assert_eq!(reconstructed.len(), m * n);

    let max_abs: f32 = data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    let mut max_err = 0.0f32;
    for i in 0..m * n {
        let err = (data[i] - reconstructed[i]).abs();
        if err > max_err {
            max_err = err;
        }
    }

    assert!(
        max_err < max_abs * 0.01,
        "factor reconstruction error too high: max_err={} (max_abs={})",
        max_err,
        max_abs
    );

    // Factor storage should be smaller than the full matrix.
    assert!(factors.storage_bytes() > 0);
    assert!(
        factors.storage_bytes() < m * n * 4,
        "factor storage {} should be < original {}",
        factors.storage_bytes(),
        m * n * 4
    );
}

// ===========================================================================
// 11. Witness Logging Integration
// ===========================================================================

/// Record access, tier-change, and eviction events; verify counters and
/// flip-rate calculation.
#[test]
fn test_witness_logging() {
    let mut log = WitnessLog::new(256);
    let mut store = TieredStore::new(4096);

    let key = make_key(1, 0);
    store.put(key, &vec![1.0f32; 64], Tier::Tier1, 0).unwrap();

    log.record(
        0,
        WitnessEvent::Access {
            key,
            score: 0.95,
            tier: Tier::Tier1,
        },
    );
    log.record(
        100,
        WitnessEvent::TierChange {
            key,
            from_tier: Tier::Tier1,
            to_tier: Tier::Tier2,
            score: 0.45,
            reason: TierChangeReason::ScoreDowngrade,
        },
    );

    store.evict(key, ReconstructPolicy::None).unwrap();
    log.record(
        200,
        WitnessEvent::Eviction {
            key,
            score: 0.05,
            bytes_freed: 64,
        },
    );

    assert_eq!(log.len(), 3);
    assert_eq!(log.count_tier_changes(), 1);
    assert_eq!(log.count_evictions(), 1);
    assert_eq!(log.count_checksum_failures(), 0);

    let recent = log.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].timestamp, 100);
    assert_eq!(recent[1].timestamp, 200);

    // One tier change across 1 block in the window = flip rate 1.0.
    let rate = log.tier_flip_rate(300, 1);
    assert!(
        (rate - 1.0).abs() < 1e-6,
        "expected flip rate 1.0, got {}",
        rate
    );
}
