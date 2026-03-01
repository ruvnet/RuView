//! Stress and fuzz-like tests for temporal tensor compression.
//!
//! Exercises the storage engine, delta chains, and checksum integrity under
//! heavy random workloads using a deterministic PRNG. No external dependencies.
//!
//! Run with:
//! ```sh
//! cargo test --release -p ruvector-temporal-tensor --test stress_tests -- --nocapture
//! ```

use ruvector_temporal_tensor::delta::{compute_delta, DeltaChain};
use ruvector_temporal_tensor::store::{BlockKey, ReconstructPolicy, StoreError, Tier, TieredStore};

// ---------------------------------------------------------------------------
// Deterministic PRNG (LCG) -- same as other test files, no external deps
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

    fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    fn next_usize_range(&mut self, lo: usize, hi: usize) -> usize {
        let range = (hi - lo) as u64;
        if range == 0 {
            return lo;
        }
        lo + (self.next_u64() % range) as usize
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

fn random_tier(rng: &mut SimpleRng) -> Tier {
    match rng.next_usize_range(0, 3) {
        0 => Tier::Tier1,
        1 => Tier::Tier2,
        _ => Tier::Tier3,
    }
}

fn random_data(rng: &mut SimpleRng, len: usize) -> Vec<f32> {
    (0..len).map(|_| rng.next_f32_range(-1.0, 1.0)).collect()
}

// ===========================================================================
// 1. Random put/get/evict cycle
// ===========================================================================

/// Exercises the store with 5000 random operations (put 40%, get 30%,
/// touch 20%, evict 10%) on a pool of 200 block keys.  After all
/// iterations the block count must equal `inserted - evicted`.
#[test]
fn test_random_put_get_evict_cycle() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0xDEAD_BEEF);

    const NUM_KEYS: usize = 200;
    const NUM_ITERS: usize = 5_000;
    const ELEM_COUNT: usize = 64;

    // Track which keys have been inserted and not yet evicted.
    let mut inserted: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut evicted: std::collections::HashSet<u32> = std::collections::HashSet::new();

    for iter in 0..NUM_ITERS {
        let roll = rng.next_usize_range(0, 100);
        let key_idx = rng.next_usize_range(0, NUM_KEYS) as u32;
        let key = make_key(1, key_idx);
        let tick = iter as u64;

        if roll < 40 {
            // PUT (40%)
            let data = random_data(&mut rng, ELEM_COUNT);
            let tier = random_tier(&mut rng);
            store.put(key, &data, tier, tick).unwrap();
            inserted.insert(key_idx);
            evicted.remove(&key_idx);
        } else if roll < 70 {
            // GET (30%)
            let mut out = vec![0.0f32; ELEM_COUNT];
            match store.get(key, &mut out, tick) {
                Ok(n) => {
                    assert!(n > 0, "get returned 0 elements for an existing block");
                    assert!(n <= ELEM_COUNT);
                }
                Err(StoreError::BlockNotFound) => {
                    // Key was never inserted or was evicted -- valid.
                }
                Err(StoreError::TensorEvicted) => {
                    // Block was evicted to Tier0 -- valid.
                    assert!(
                        evicted.contains(&key_idx),
                        "TensorEvicted for key not in evicted set"
                    );
                }
                Err(e) => {
                    panic!("unexpected error on get at iter {}: {:?}", iter, e);
                }
            }
        } else if roll < 90 {
            // TOUCH (20%)
            store.touch(key, tick);
        } else {
            // EVICT (10%)
            match store.evict(key, ReconstructPolicy::None) {
                Ok(()) => {
                    if inserted.contains(&key_idx) {
                        evicted.insert(key_idx);
                    }
                }
                Err(StoreError::BlockNotFound) => {
                    // Key never existed -- valid.
                }
                Err(e) => {
                    panic!("unexpected error on evict at iter {}: {:?}", iter, e);
                }
            }
        }
    }

    // Final invariant: block_count = all unique keys ever put (including evicted ones,
    // since eviction keeps metadata).
    let all_known: std::collections::HashSet<u32> = inserted.union(&evicted).copied().collect();
    assert_eq!(
        store.block_count(),
        all_known.len(),
        "block_count mismatch after random cycle"
    );

    // Verify: non-evicted blocks are readable.
    let live_keys: Vec<u32> = inserted.difference(&evicted).copied().collect();
    for &kid in &live_keys {
        let mut out = vec![0.0f32; ELEM_COUNT];
        let key = make_key(1, kid);
        let result = store.get(key, &mut out, NUM_ITERS as u64);
        assert!(
            result.is_ok(),
            "live block {} should be readable, got {:?}",
            kid,
            result
        );
    }

    println!(
        "random_put_get_evict_cycle: {} iters, {} live blocks, {} evicted",
        NUM_ITERS,
        live_keys.len(),
        evicted.len()
    );
}

// ===========================================================================
// 2. Rapid tier oscillation (stress hysteresis)
// ===========================================================================

/// Puts 50 blocks at Tier1, then alternately touches 25 blocks intensively
/// (50 touches/tick) and ignores them for 500 ticks. Verifies that all
/// blocks remain readable and no panics occur during rapid access-pattern
/// changes.
#[test]
fn test_rapid_tier_oscillation() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0xCAFE_BABE);

    const NUM_BLOCKS: usize = 50;
    const ELEM_COUNT: usize = 64;
    const TOTAL_TICKS: u64 = 500;
    const HOT_COUNT: usize = 25;
    const TOUCHES_PER_TICK: usize = 50;

    // Insert all blocks at Tier1.
    let block_data: Vec<Vec<f32>> = (0..NUM_BLOCKS)
        .map(|_| random_data(&mut rng, ELEM_COUNT))
        .collect();

    for i in 0..NUM_BLOCKS {
        store
            .put(make_key(2, i as u32), &block_data[i], Tier::Tier1, 0)
            .unwrap();
    }
    assert_eq!(store.block_count(), NUM_BLOCKS);

    // Oscillate: even ticks -> heavy touching of first HOT_COUNT blocks,
    //            odd ticks  -> no touching (cold period).
    for tick in 1..=TOTAL_TICKS {
        if tick % 2 == 0 {
            // Hot phase: touch first HOT_COUNT blocks repeatedly.
            for _ in 0..TOUCHES_PER_TICK {
                let idx = rng.next_usize_range(0, HOT_COUNT) as u32;
                store.touch(make_key(2, idx), tick);
            }
        }
        // Odd ticks: silence (no touches).
    }

    // All blocks must remain readable.
    for i in 0..NUM_BLOCKS {
        let key = make_key(2, i as u32);
        let mut out = vec![0.0f32; ELEM_COUNT];
        let n = store
            .get(key, &mut out, TOTAL_TICKS + 1)
            .unwrap_or_else(|e| panic!("block {} unreadable after oscillation: {:?}", i, e));
        assert_eq!(n, ELEM_COUNT);
        // Values must be finite.
        for (j, &v) in out.iter().enumerate() {
            assert!(v.is_finite(), "block {} elem {} is non-finite: {}", i, j, v);
        }
    }

    // Verify metadata is intact for all blocks.
    for i in 0..NUM_BLOCKS {
        let m = store.meta(make_key(2, i as u32)).expect("meta missing");
        assert!(
            m.tier == Tier::Tier1 || m.tier == Tier::Tier2 || m.tier == Tier::Tier3,
            "block {} has unexpected tier {:?}",
            i,
            m.tier
        );
    }

    println!(
        "rapid_tier_oscillation: {} ticks, {} blocks, no panics",
        TOTAL_TICKS, NUM_BLOCKS
    );
}

// ===========================================================================
// 3. Large block stress (memory pressure)
// ===========================================================================

/// Puts 500 blocks of 4096 elements each (total ~8MB at 8-bit), touches
/// them randomly, reads them all back verifying finite values, evicts half,
/// and verifies the other half is still readable and total_bytes decreased.
#[test]
fn test_large_block_stress() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0x1234_5678);

    const NUM_BLOCKS: usize = 500;
    const ELEM_COUNT: usize = 4096;

    // Insert all blocks at Tier1 (8-bit = 1 byte/elem = 4096 bytes/block).
    for i in 0..NUM_BLOCKS {
        let data = random_data(&mut rng, ELEM_COUNT);
        store
            .put(make_key(3, i as u32), &data, Tier::Tier1, i as u64)
            .unwrap();
    }
    assert_eq!(store.block_count(), NUM_BLOCKS);

    let bytes_before = store.total_bytes();
    assert!(
        bytes_before > 0,
        "total_bytes should be positive after inserting {} blocks",
        NUM_BLOCKS
    );
    println!(
        "large_block_stress: {} blocks inserted, total_bytes = {}",
        NUM_BLOCKS, bytes_before
    );

    // Touch all blocks randomly.
    for _ in 0..NUM_BLOCKS {
        let idx = rng.next_usize_range(0, NUM_BLOCKS) as u32;
        store.touch(make_key(3, idx), NUM_BLOCKS as u64 + 1);
    }

    // Read all blocks back and verify finite values.
    for i in 0..NUM_BLOCKS {
        let key = make_key(3, i as u32);
        let mut out = vec![0.0f32; ELEM_COUNT];
        let n = store
            .get(key, &mut out, NUM_BLOCKS as u64 + 2)
            .unwrap_or_else(|e| panic!("block {} unreadable: {:?}", i, e));
        assert_eq!(n, ELEM_COUNT);
        for (j, &v) in out.iter().enumerate() {
            assert!(v.is_finite(), "block {} elem {} is non-finite: {}", i, j, v);
        }
    }

    // Evict the first half.
    for i in 0..(NUM_BLOCKS / 2) {
        store
            .evict(make_key(3, i as u32), ReconstructPolicy::None)
            .unwrap();
    }

    let bytes_after = store.total_bytes();
    assert!(
        bytes_after < bytes_before,
        "total_bytes should decrease after evicting half: before={}, after={}",
        bytes_before,
        bytes_after
    );

    // Verify the second half is still readable.
    for i in (NUM_BLOCKS / 2)..NUM_BLOCKS {
        let key = make_key(3, i as u32);
        let mut out = vec![0.0f32; ELEM_COUNT];
        let n = store
            .get(key, &mut out, NUM_BLOCKS as u64 + 3)
            .unwrap_or_else(|e| {
                panic!(
                    "block {} should still be readable after evicting first half: {:?}",
                    i, e
                )
            });
        assert_eq!(n, ELEM_COUNT);
    }

    // Verify evicted blocks return TensorEvicted.
    for i in 0..(NUM_BLOCKS / 2) {
        let key = make_key(3, i as u32);
        let mut out = vec![0.0f32; ELEM_COUNT];
        let result = store.get(key, &mut out, NUM_BLOCKS as u64 + 4);
        assert_eq!(
            result,
            Err(StoreError::TensorEvicted),
            "evicted block {} should return TensorEvicted",
            i
        );
    }

    println!(
        "large_block_stress: bytes before={}, after={}, reduction={}%",
        bytes_before,
        bytes_after,
        ((bytes_before - bytes_after) as f64 / bytes_before as f64 * 100.0) as u32
    );
}

// ===========================================================================
// 4. Delta chain stress
// ===========================================================================

/// Creates a 1024-element base vector, builds a DeltaChain with max_depth=8,
/// appends 8 deltas each modifying ~5% of values, reconstructs and verifies
/// error < 1%, compacts, rebuilds to max, and checks that an extra append
/// yields DeltaChainTooLong.
#[test]
fn test_delta_chain_stress() {
    let mut rng = SimpleRng::new(0xABCD_EF01);

    const DIM: usize = 1024;
    const MAX_DEPTH: u8 = 8;
    const CHANGE_FRACTION: f32 = 0.05; // ~5% of values per delta

    // Create a base vector with random values in [-1, 1].
    let base: Vec<f32> = (0..DIM).map(|_| rng.next_f32_range(-1.0, 1.0)).collect();
    let mut chain = DeltaChain::new(base.clone(), MAX_DEPTH);

    // Build the expected ground-truth by applying modifications cumulatively.
    let mut truth = base.clone();

    // Append MAX_DEPTH deltas, each modifying ~5% of elements.
    for epoch in 0..MAX_DEPTH {
        let mut modified = truth.clone();
        let num_changes = (DIM as f32 * CHANGE_FRACTION) as usize;
        for _ in 0..num_changes {
            let idx = rng.next_usize_range(0, DIM);
            let perturbation = rng.next_f32_range(-0.1, 0.1);
            modified[idx] += perturbation;
        }

        let delta = compute_delta(
            &truth,
            &modified,
            42,           // tensor_id
            0,            // block_index
            epoch as u64, // base_epoch
            1e-8,         // threshold (very small to capture all changes)
            1.0,          // max_change_fraction (allow up to 100%)
        )
        .expect("compute_delta should succeed for small changes");

        chain
            .append(delta)
            .unwrap_or_else(|e| panic!("append should succeed at depth {}: {:?}", epoch, e));

        truth = modified;
    }

    assert_eq!(chain.chain_len(), MAX_DEPTH as usize);

    // Reconstruct and verify error < 1%.
    let reconstructed = chain.reconstruct();
    assert_eq!(reconstructed.len(), DIM);
    let mut max_err: f32 = 0.0;
    for i in 0..DIM {
        let err = (reconstructed[i] - truth[i]).abs();
        if err > max_err {
            max_err = err;
        }
    }
    // The error comes from i16 quantization of deltas; for small perturbations
    // the relative error should be well under 1% of the value range.
    let value_range = truth.iter().fold(0.0f32, |acc, &v| acc.max(v.abs()));
    let relative_max_err = if value_range > 0.0 {
        max_err / value_range
    } else {
        0.0
    };
    assert!(
        relative_max_err < 0.01,
        "reconstruction error {:.6} ({:.4}%) exceeds 1% of value range {:.4}",
        max_err,
        relative_max_err * 100.0,
        value_range
    );
    println!(
        "delta_chain_stress: max reconstruction error = {:.6} ({:.4}% of range {:.4})",
        max_err,
        relative_max_err * 100.0,
        value_range
    );

    // Compact: apply all deltas to base, chain_len should become 0.
    chain.compact();
    assert_eq!(
        chain.chain_len(),
        0,
        "chain_len should be 0 after compaction"
    );

    // Verify reconstruction after compaction still yields correct data.
    let after_compact = chain.reconstruct();
    for i in 0..DIM {
        let err = (after_compact[i] - truth[i]).abs();
        assert!(
            err < 0.01,
            "post-compaction error at elem {}: {:.6}",
            i,
            err
        );
    }

    // Rebuild chain to max depth.
    let compacted_base = after_compact.clone();
    let mut chain2 = DeltaChain::new(compacted_base.clone(), MAX_DEPTH);
    let mut truth2 = compacted_base.clone();
    for epoch in 0..MAX_DEPTH {
        let mut modified = truth2.clone();
        let num_changes = (DIM as f32 * CHANGE_FRACTION) as usize;
        for _ in 0..num_changes {
            let idx = rng.next_usize_range(0, DIM);
            modified[idx] += rng.next_f32_range(-0.05, 0.05);
        }
        let delta = compute_delta(&truth2, &modified, 42, 0, epoch as u64, 1e-8, 1.0)
            .expect("compute_delta should succeed");
        chain2.append(delta).unwrap();
        truth2 = modified;
    }
    assert_eq!(chain2.chain_len(), MAX_DEPTH as usize);

    // One more append should fail with DeltaChainTooLong.
    let mut overflow_modified = truth2.clone();
    overflow_modified[0] += 0.01;
    let overflow_delta = compute_delta(
        &truth2,
        &overflow_modified,
        42,
        0,
        MAX_DEPTH as u64,
        1e-8,
        1.0,
    )
    .expect("compute_delta for overflow");
    let result = chain2.append(overflow_delta);
    assert_eq!(
        result,
        Err(StoreError::DeltaChainTooLong),
        "appending beyond max_depth should return DeltaChainTooLong"
    );

    // Reconstruct should still work after the failed append.
    let after_fail = chain2.reconstruct();
    assert_eq!(after_fail.len(), DIM);
    for i in 0..DIM {
        let err = (after_fail[i] - truth2[i]).abs();
        assert!(
            err < 0.01,
            "reconstruction after failed append: elem {} error {:.6}",
            i,
            err
        );
    }

    println!("delta_chain_stress: all chain operations verified");
}

// ===========================================================================
// 5. Checksum sensitivity
// ===========================================================================

/// Verifies that the checksum stored in block metadata is deterministic
/// and sensitive to even tiny changes in input data.
#[test]
fn test_checksum_sensitivity() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0xFEED_FACE);

    const ELEM_COUNT: usize = 128;
    let data: Vec<f32> = (0..ELEM_COUNT)
        .map(|_| rng.next_f32_range(-1.0, 1.0))
        .collect();

    let key = make_key(5, 0);

    // Put and record the checksum.
    store.put(key, &data, Tier::Tier1, 0).unwrap();
    let checksum1 = store.meta(key).unwrap().checksum;

    // Put the same data again with the same key -> same checksum.
    store.put(key, &data, Tier::Tier1, 1).unwrap();
    let checksum2 = store.meta(key).unwrap().checksum;
    assert_eq!(
        checksum1, checksum2,
        "identical data should produce identical checksums"
    );

    // Modify one element by a tiny amount (1e-6), put again.
    let mut data_tiny = data.clone();
    data_tiny[ELEM_COUNT / 2] += 1e-6;
    store.put(key, &data_tiny, Tier::Tier1, 2).unwrap();
    let checksum3 = store.meta(key).unwrap().checksum;
    // Note: due to 8-bit quantization, a 1e-6 change on values in [-1,1]
    // might not change the quantized representation. If it does, checksums
    // differ; if not, they are the same. We test a larger perturbation below
    // to guarantee a difference.

    // Modify one element by a larger amount that will definitely change quantized value.
    let mut data_modified = data.clone();
    data_modified[ELEM_COUNT / 2] += 0.1;
    store.put(key, &data_modified, Tier::Tier1, 3).unwrap();
    let checksum4 = store.meta(key).unwrap().checksum;
    assert_ne!(
        checksum1, checksum4,
        "modifying one element by 0.1 should change the checksum"
    );

    // Put very different data -> very different checksum.
    let data_different: Vec<f32> = (0..ELEM_COUNT)
        .map(|_| rng.next_f32_range(-10.0, 10.0))
        .collect();
    store.put(key, &data_different, Tier::Tier1, 4).unwrap();
    let checksum5 = store.meta(key).unwrap().checksum;
    assert_ne!(
        checksum1, checksum5,
        "very different data should produce a different checksum"
    );
    // Also verify it differs from the slightly-modified version.
    assert_ne!(
        checksum4, checksum5,
        "two different datasets should have different checksums"
    );

    println!(
        "checksum_sensitivity: c1={:#010X} c2={:#010X} c3={:#010X} c4={:#010X} c5={:#010X}",
        checksum1, checksum2, checksum3, checksum4, checksum5
    );
}

// ===========================================================================
// 6. Concurrent simulation (simulated multi-reader)
// ===========================================================================

/// Puts 100 blocks, then runs 10 simulated "reader threads" (sequential
/// loops) each performing 100 iterations of random touches and reads.
/// Verifies all reads succeed and return finite data, and metadata remains
/// consistent.
#[test]
fn test_concurrent_simulation() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0xC0DE_C0DE);

    const NUM_BLOCKS: usize = 100;
    const NUM_READERS: usize = 10;
    const ITERS_PER_READER: usize = 100;
    const ELEM_COUNT: usize = 64;

    // Insert all blocks.
    for i in 0..NUM_BLOCKS {
        let data = random_data(&mut rng, ELEM_COUNT);
        store
            .put(make_key(6, i as u32), &data, Tier::Tier1, 0)
            .unwrap();
    }
    assert_eq!(store.block_count(), NUM_BLOCKS);

    let mut total_reads: usize = 0;
    let mut total_touches: usize = 0;

    // Simulate NUM_READERS concurrent readers.
    for reader_id in 0..NUM_READERS {
        let base_tick = (reader_id as u64 + 1) * 1000;
        for iter in 0..ITERS_PER_READER {
            let key_idx = rng.next_usize_range(0, NUM_BLOCKS) as u32;
            let key = make_key(6, key_idx);
            let tick = base_tick + iter as u64;

            // Touch the block.
            store.touch(key, tick);
            total_touches += 1;

            // Read the block.
            let mut out = vec![0.0f32; ELEM_COUNT];
            let n = store.get(key, &mut out, tick).unwrap_or_else(|e| {
                panic!(
                    "reader {} iter {} key {} failed: {:?}",
                    reader_id, iter, key_idx, e
                )
            });
            assert_eq!(n, ELEM_COUNT);
            total_reads += 1;

            // Verify finite values.
            for (j, &v) in out.iter().enumerate() {
                assert!(
                    v.is_finite(),
                    "reader {} iter {} block {} elem {} non-finite: {}",
                    reader_id,
                    iter,
                    key_idx,
                    j,
                    v
                );
            }
        }
    }

    // Verify metadata integrity for all blocks.
    for i in 0..NUM_BLOCKS {
        let key = make_key(6, i as u32);
        let m = store.meta(key).expect("meta should exist");
        assert!(
            m.tier == Tier::Tier1 || m.tier == Tier::Tier2 || m.tier == Tier::Tier3,
            "block {} has invalid tier {:?}",
            i,
            m.tier
        );
        assert!(
            m.access_count > 0,
            "block {} should have been accessed at least once",
            i
        );
    }

    println!(
        "concurrent_simulation: {} readers x {} iters = {} reads, {} touches",
        NUM_READERS, ITERS_PER_READER, total_reads, total_touches
    );
}

// ===========================================================================
// 7. Extreme tick values
// ===========================================================================

/// Tests behavior at tick value boundaries: 0, u64::MAX-1, and u64::MAX.
/// Verifies no overflow or underflow panics in access-pattern tracking.
#[test]
fn test_extreme_tick_values() {
    let mut store = TieredStore::new(4096);

    const ELEM_COUNT: usize = 32;
    let data = vec![0.5f32; ELEM_COUNT];

    // -- Test 1: Put at tick=0, touch at tick=u64::MAX-1 --
    let key_a = make_key(7, 0);
    store.put(key_a, &data, Tier::Tier1, 0).unwrap();
    store.touch(key_a, u64::MAX - 1);

    let meta_a = store.meta(key_a).unwrap();
    assert_eq!(meta_a.last_access_at, u64::MAX - 1);
    assert!(
        meta_a.access_count >= 2,
        "access_count should reflect put + touch"
    );

    // Read should still work.
    let mut out = vec![0.0f32; ELEM_COUNT];
    let n = store.get(key_a, &mut out, u64::MAX - 1).unwrap();
    assert_eq!(n, ELEM_COUNT);

    // -- Test 2: Put at tick=u64::MAX --
    let key_b = make_key(7, 1);
    store.put(key_b, &data, Tier::Tier1, u64::MAX).unwrap();
    let meta_b = store.meta(key_b).unwrap();
    assert_eq!(meta_b.created_at, u64::MAX);
    assert_eq!(meta_b.last_access_at, u64::MAX);

    // Read at u64::MAX.
    let mut out2 = vec![0.0f32; ELEM_COUNT];
    let n2 = store.get(key_b, &mut out2, u64::MAX).unwrap();
    assert_eq!(n2, ELEM_COUNT);

    // -- Test 3: Touch at tick=0 when last_access=u64::MAX --
    // This tests that saturating_sub prevents underflow.
    store.touch(key_b, 0);
    let meta_b2 = store.meta(key_b).unwrap();
    // last_access should update to 0 (the tick we passed).
    // The delta computation uses saturating_sub, so 0 - u64::MAX saturates to 0,
    // meaning delta=0 and the window/ema are handled without panic.
    assert_eq!(meta_b2.last_access_at, 0);

    // -- Test 4: Touch at tick=u64::MAX after last_access=0 --
    store.touch(key_b, u64::MAX);
    let meta_b3 = store.meta(key_b).unwrap();
    assert_eq!(meta_b3.last_access_at, u64::MAX);
    // The delta is u64::MAX, which is >= 64, so window resets to 1.
    assert_eq!(meta_b3.window, 1);

    // Verify all blocks still readable after extreme tick gymnastics.
    for i in 0..2u32 {
        let key = make_key(7, i);
        let mut out = vec![0.0f32; ELEM_COUNT];
        let result = store.get(key, &mut out, u64::MAX);
        assert!(
            result.is_ok(),
            "block {} should be readable after extreme ticks: {:?}",
            i,
            result
        );
    }

    println!("extreme_tick_values: all boundary conditions passed without panic");
}

// ===========================================================================
// 8. All tiers coexist
// ===========================================================================

/// Puts 100 blocks in each of Tier1, Tier2, Tier3 (300 total), verifies
/// tier counts, reads all blocks verifying accuracy matches tier expectations
/// (higher tiers = less quantization error), evicts all Tier3 blocks, and
/// verifies Tier1 and Tier2 are still readable.
#[test]
fn test_all_tiers_coexist() {
    let mut store = TieredStore::new(4096);
    let mut rng = SimpleRng::new(0xBAAD_F00D);

    const BLOCKS_PER_TIER: usize = 100;
    const ELEM_COUNT: usize = 128;

    // Store original data for roundtrip error comparison.
    let mut originals: Vec<Vec<f32>> = Vec::new();

    // Insert 100 blocks at Tier1 (tensor_id=81).
    for i in 0..BLOCKS_PER_TIER {
        let data = random_data(&mut rng, ELEM_COUNT);
        store
            .put(make_key(81, i as u32), &data, Tier::Tier1, 0)
            .unwrap();
        originals.push(data);
    }

    // Insert 100 blocks at Tier2 (tensor_id=82).
    for i in 0..BLOCKS_PER_TIER {
        let data = random_data(&mut rng, ELEM_COUNT);
        store
            .put(make_key(82, i as u32), &data, Tier::Tier2, 0)
            .unwrap();
        originals.push(data);
    }

    // Insert 100 blocks at Tier3 (tensor_id=83).
    for i in 0..BLOCKS_PER_TIER {
        let data = random_data(&mut rng, ELEM_COUNT);
        store
            .put(make_key(83, i as u32), &data, Tier::Tier3, 0)
            .unwrap();
        originals.push(data);
    }

    // Verify tier counts.
    assert_eq!(store.tier_count(Tier::Tier1), BLOCKS_PER_TIER);
    assert_eq!(store.tier_count(Tier::Tier2), BLOCKS_PER_TIER);
    assert_eq!(store.tier_count(Tier::Tier3), BLOCKS_PER_TIER);
    assert_eq!(store.block_count(), 3 * BLOCKS_PER_TIER);

    // Read all blocks and compute per-tier max roundtrip error.
    let mut tier1_max_err: f32 = 0.0;
    let mut tier2_max_err: f32 = 0.0;
    let mut tier3_max_err: f32 = 0.0;

    for i in 0..BLOCKS_PER_TIER {
        // Tier1
        let key = make_key(81, i as u32);
        let mut out = vec![0.0f32; ELEM_COUNT];
        store.get(key, &mut out, 1).unwrap();
        let orig = &originals[i];
        for j in 0..ELEM_COUNT {
            let err = (out[j] - orig[j]).abs();
            if err > tier1_max_err {
                tier1_max_err = err;
            }
        }

        // Tier2
        let key = make_key(82, i as u32);
        store.get(key, &mut out, 1).unwrap();
        let orig = &originals[BLOCKS_PER_TIER + i];
        for j in 0..ELEM_COUNT {
            let err = (out[j] - orig[j]).abs();
            if err > tier2_max_err {
                tier2_max_err = err;
            }
        }

        // Tier3
        let key = make_key(83, i as u32);
        store.get(key, &mut out, 1).unwrap();
        let orig = &originals[2 * BLOCKS_PER_TIER + i];
        for j in 0..ELEM_COUNT {
            let err = (out[j] - orig[j]).abs();
            if err > tier3_max_err {
                tier3_max_err = err;
            }
        }
    }

    // Tier1 (8-bit) should have the lowest error, Tier3 (3-bit) the highest.
    // Values are in [-1, 1], so 8-bit qmax=127 -> step ~0.0079, 3-bit qmax=3 -> step ~0.33.
    assert!(
        tier1_max_err <= tier3_max_err,
        "Tier1 error ({:.6}) should not exceed Tier3 error ({:.6})",
        tier1_max_err,
        tier3_max_err
    );
    // Tier3 with 3-bit quantization has significant error for [-1,1] data.
    assert!(
        tier3_max_err > 0.0,
        "Tier3 (3-bit) should have nonzero quantization error"
    );

    println!(
        "all_tiers_coexist: tier1_err={:.6}, tier2_err={:.6}, tier3_err={:.6}",
        tier1_max_err, tier2_max_err, tier3_max_err
    );

    // Evict all Tier3 blocks.
    for i in 0..BLOCKS_PER_TIER {
        store
            .evict(make_key(83, i as u32), ReconstructPolicy::None)
            .unwrap();
    }

    assert_eq!(store.tier_count(Tier::Tier3), 0);
    assert_eq!(store.tier_count(Tier::Tier0), BLOCKS_PER_TIER);
    // Total blocks unchanged (eviction preserves metadata).
    assert_eq!(store.block_count(), 3 * BLOCKS_PER_TIER);

    // Tier1 and Tier2 must still be readable.
    for i in 0..BLOCKS_PER_TIER {
        let mut out = vec![0.0f32; ELEM_COUNT];

        let key1 = make_key(81, i as u32);
        store.get(key1, &mut out, 2).unwrap_or_else(|e| {
            panic!("Tier1 block {} unreadable after Tier3 eviction: {:?}", i, e)
        });

        let key2 = make_key(82, i as u32);
        store.get(key2, &mut out, 2).unwrap_or_else(|e| {
            panic!("Tier2 block {} unreadable after Tier3 eviction: {:?}", i, e)
        });
    }

    // Evicted Tier3 blocks should return TensorEvicted.
    for i in 0..BLOCKS_PER_TIER {
        let key = make_key(83, i as u32);
        let mut out = vec![0.0f32; ELEM_COUNT];
        let result = store.get(key, &mut out, 2);
        assert_eq!(
            result,
            Err(StoreError::TensorEvicted),
            "evicted Tier3 block {} should return TensorEvicted",
            i
        );
    }

    println!(
        "all_tiers_coexist: evicted Tier3, Tier1 ({}) and Tier2 ({}) still intact",
        store.tier_count(Tier::Tier1),
        store.tier_count(Tier::Tier2)
    );
}
