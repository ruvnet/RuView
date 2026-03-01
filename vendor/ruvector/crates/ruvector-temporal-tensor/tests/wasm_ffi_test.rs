//! FFI interface tests for the temporal tensor store.
//!
//! These tests exercise the `tts_*` extern "C" functions exposed by
//! `store_ffi.rs` through their public API.  Because the FFI layer uses
//! a single global `STORE_STATE`, tests **must** run sequentially:
//!
//! ```bash
//! cargo test -p ruvector-temporal-tensor --test wasm_ffi_test --features ffi -- --test-threads=1
//! ```
#![cfg(feature = "ffi")]

use ruvector_temporal_tensor::store_ffi::{
    tts_block_count, tts_evict, tts_get, tts_init, tts_put, tts_stats, tts_tier_count, tts_touch,
};

// ── Constants mirrored from store_ffi.rs ────────────────────────────────

const ERR_BLOCK_NOT_FOUND: i32 = -4;
const ERR_BUFFER_TOO_SMALL: i32 = -5;

/// Binary stats size: 5 * u32 + 2 * u64 = 36 bytes.
const STATS_SIZE: usize = 5 * 4 + 2 * 8;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Re-initialize the global store with default config before each test.
/// This replaces whatever state was left by a previous test.
fn reset() {
    let rc = tts_init(std::ptr::null(), 0);
    assert_eq!(rc, 0, "tts_init with default config must succeed");
}

/// Read a little-endian u32 from `buf` at the given byte offset.
fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// Read a little-endian u64 from `buf` at the given byte offset.
fn read_u64_le(buf: &[u8], off: usize) -> u64 {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(arr)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[test]
fn test_ffi_init_and_destroy() {
    // Calling tts_init with a null pointer and zero length should use
    // the default TierConfig and return success (0).
    let rc = tts_init(std::ptr::null(), 0);
    assert_eq!(rc, 0, "tts_init should return 0 on success");

    // The freshly initialized store must contain zero blocks.
    assert_eq!(tts_block_count(), 0, "new store should have 0 blocks");

    // Re-initializing must also succeed (replaces old state).
    let rc2 = tts_init(std::ptr::null(), 0);
    assert_eq!(rc2, 0, "re-init should succeed");
    assert_eq!(tts_block_count(), 0, "re-init should reset block count");
}

#[test]
fn test_ffi_put_get_roundtrip() {
    reset();

    // Create 64 f32 values with a clear pattern.
    let data: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.1).collect();

    let rc = tts_put(0, 1, 0, data.as_ptr(), data.len());
    assert_eq!(rc, 0, "tts_put should return 0 on success");

    let mut out = vec![0.0f32; 64];
    let n = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n, 64, "tts_get should return 64 elements");

    // Verify accuracy.  New blocks default to Hot (8-bit quantization)
    // so the error should be small.
    let max_abs = data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    for (i, (&orig, &dec)) in data.iter().zip(out.iter()).enumerate() {
        let err = (orig - dec).abs();
        assert!(
            err < max_abs * 0.05,
            "element {i}: orig={orig}, decoded={dec}, err={err}, tolerance={}",
            max_abs * 0.05,
        );
    }
}

#[test]
fn test_ffi_multi_tensor() {
    reset();

    let data_a: Vec<f32> = (0..64).map(|i| i as f32 * 0.5).collect();
    let data_b: Vec<f32> = (0..64).map(|i| -(i as f32) * 0.3).collect();
    let data_c: Vec<f32> = (0..64).map(|i| (i as f32).sin()).collect();

    // Three different tensor IDs using hi/lo split for u128:
    //   tensor A: hi=0, lo=1  -> tensor_id = 1
    //   tensor B: hi=0, lo=2  -> tensor_id = 2
    //   tensor C: hi=1, lo=0  -> tensor_id = 1 << 64
    assert_eq!(tts_put(0, 1, 0, data_a.as_ptr(), data_a.len()), 0);
    assert_eq!(tts_put(0, 2, 0, data_b.as_ptr(), data_b.len()), 0);
    assert_eq!(tts_put(1, 0, 0, data_c.as_ptr(), data_c.len()), 0);

    assert_eq!(tts_block_count(), 3, "should have 3 blocks total");

    // Read back each tensor independently.
    let mut out = vec![0.0f32; 64];

    let n_a = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n_a, 64);
    // Spot-check first element of tensor A.
    assert!(
        (out[0] - data_a[0]).abs() < 0.5,
        "tensor A readback mismatch"
    );

    let n_b = tts_get(0, 2, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n_b, 64);
    assert!(
        (out[0] - data_b[0]).abs() < 0.5,
        "tensor B readback mismatch"
    );

    let n_c = tts_get(1, 0, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n_c, 64);
    assert!(
        (out[0] - data_c[0]).abs() < 0.5,
        "tensor C readback mismatch"
    );
}

#[test]
fn test_ffi_eviction() {
    reset();

    let data = vec![1.0f32; 64];
    assert_eq!(tts_put(0, 42, 0, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_block_count(), 1);

    // Evict the block.
    let rc = tts_evict(0, 42, 0);
    assert_eq!(rc, 0, "tts_evict should return 0 on success");
    assert_eq!(tts_block_count(), 0, "evicted block should be gone");

    // A subsequent get should return ERR_BLOCK_NOT_FOUND.
    let mut out = vec![0.0f32; 64];
    let rc_get = tts_get(0, 42, 0, out.as_mut_ptr(), out.len());
    assert_eq!(
        rc_get, ERR_BLOCK_NOT_FOUND,
        "get after evict should return block-not-found"
    );

    // Evicting again should also return block-not-found.
    let rc2 = tts_evict(0, 42, 0);
    assert_eq!(rc2, ERR_BLOCK_NOT_FOUND);
}

#[test]
fn test_ffi_touch_updates_access() {
    reset();

    let data = vec![1.0f32; 64];
    assert_eq!(tts_put(0, 7, 3, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_block_count(), 1);

    // Touch the block multiple times.
    for _ in 0..5 {
        let rc = tts_touch(0, 7, 3);
        assert_eq!(rc, 0, "tts_touch should return 0 on success");
    }

    // Block count should remain unchanged (touch does not add/remove blocks).
    assert_eq!(tts_block_count(), 1, "touch should not change block count");

    // The block should still be readable.
    let mut out = vec![0.0f32; 64];
    let n = tts_get(0, 7, 3, out.as_mut_ptr(), out.len());
    assert_eq!(n, 64, "block should still be readable after touches");

    // Touching a non-existent block should fail.
    let rc_missing = tts_touch(0, 99, 0);
    assert_eq!(rc_missing, ERR_BLOCK_NOT_FOUND);
}

#[test]
fn test_ffi_tier_counts() {
    reset();

    // All new blocks are placed in Hot (tier 0) by default.
    let data = vec![1.0f32; 64];
    assert_eq!(tts_put(0, 1, 0, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_put(0, 1, 1, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_put(0, 2, 0, data.as_ptr(), data.len()), 0);

    assert_eq!(tts_block_count(), 3);
    assert_eq!(tts_tier_count(0), 3, "all blocks should be Hot");
    assert_eq!(tts_tier_count(1), 0, "no Warm blocks");
    assert_eq!(tts_tier_count(2), 0, "no Cool blocks");
    assert_eq!(tts_tier_count(3), 0, "no Cold blocks");

    // Invalid tier should return an error.
    assert!(tts_tier_count(99) < 0, "invalid tier should return error");
}

#[test]
fn test_ffi_stats_output() {
    reset();

    let data = vec![1.0f32; 64];
    assert_eq!(tts_put(0, 1, 0, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_put(0, 1, 1, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_put(0, 2, 0, data.as_ptr(), data.len()), 0);

    let mut buf = vec![0u8; STATS_SIZE];
    let written = tts_stats(buf.as_mut_ptr(), buf.len());
    assert_eq!(
        written, STATS_SIZE as i32,
        "tts_stats should write exactly {STATS_SIZE} bytes"
    );

    // Parse the binary stats layout:
    //   [block_count:u32][hot:u32][warm:u32][cool:u32][cold:u32]
    //   [total_bytes:u64][tick_count:u64]
    let block_count = read_u32_le(&buf, 0);
    let hot = read_u32_le(&buf, 4);
    let warm = read_u32_le(&buf, 8);
    let cool = read_u32_le(&buf, 12);
    let cold = read_u32_le(&buf, 16);
    let total_bytes = read_u64_le(&buf, 20);
    let _tick_count = read_u64_le(&buf, 28);

    assert_eq!(block_count, 3, "block_count mismatch");
    assert_eq!(hot, 3, "hot count mismatch");
    assert_eq!(warm, 0, "warm count mismatch");
    assert_eq!(cool, 0, "cool count mismatch");
    assert_eq!(cold, 0, "cold count mismatch");
    assert!(total_bytes > 0, "total_bytes should be > 0 after puts");

    // Verify stats rejects a too-small buffer.
    let mut small_buf = vec![0u8; 4];
    let rc = tts_stats(small_buf.as_mut_ptr(), small_buf.len());
    assert_eq!(rc, ERR_BUFFER_TOO_SMALL);
}

#[test]
fn test_ffi_put_multiple_blocks_same_tensor() {
    reset();

    let data = vec![2.5f32; 64];

    // Put 5 blocks for the same tensor (different block indices).
    for idx in 0..5u32 {
        let rc = tts_put(0, 10, idx, data.as_ptr(), data.len());
        assert_eq!(rc, 0, "put block_index={idx} should succeed");
    }

    assert_eq!(tts_block_count(), 5);

    // Each block should be independently readable.
    let mut out = vec![0.0f32; 64];
    for idx in 0..5u32 {
        let n = tts_get(0, 10, idx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 64, "block_index={idx} should return 64 elements");
    }
}

#[test]
fn test_ffi_overwrite_block() {
    reset();

    let data1 = vec![1.0f32; 64];
    assert_eq!(tts_put(0, 5, 0, data1.as_ptr(), data1.len()), 0);

    let data2 = vec![9.0f32; 64];
    assert_eq!(tts_put(0, 5, 0, data2.as_ptr(), data2.len()), 0);

    // Block count should still be 1 (overwrite, not insert).
    assert_eq!(tts_block_count(), 1);

    // Should read back the second write.
    let mut out = vec![0.0f32; 64];
    let n = tts_get(0, 5, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n, 64);
    for &v in &out {
        assert!(
            (v - 9.0).abs() < 0.5,
            "expected ~9.0 after overwrite, got {v}"
        );
    }
}

#[test]
fn test_ffi_get_buffer_too_small() {
    reset();

    let data = vec![1.0f32; 64];
    assert_eq!(tts_put(0, 1, 0, data.as_ptr(), data.len()), 0);

    let mut small_out = vec![0.0f32; 2];
    let rc = tts_get(0, 1, 0, small_out.as_mut_ptr(), small_out.len());
    assert_eq!(
        rc, ERR_BUFFER_TOO_SMALL,
        "get with undersized buffer should return buffer-too-small"
    );
}

#[test]
fn test_ffi_evict_then_reinsert() {
    reset();

    let data = vec![3.0f32; 64];
    assert_eq!(tts_put(0, 1, 0, data.as_ptr(), data.len()), 0);
    assert_eq!(tts_block_count(), 1);

    // Evict.
    assert_eq!(tts_evict(0, 1, 0), 0);
    assert_eq!(tts_block_count(), 0);

    // Re-insert at the same key.
    let data2 = vec![7.0f32; 64];
    assert_eq!(tts_put(0, 1, 0, data2.as_ptr(), data2.len()), 0);
    assert_eq!(tts_block_count(), 1);

    // Should read back the new data.
    let mut out = vec![0.0f32; 64];
    let n = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n, 64);
    for &v in &out {
        assert!(
            (v - 7.0).abs() < 0.5,
            "expected ~7.0 after re-insert, got {v}"
        );
    }
}

#[test]
fn test_ffi_large_tensor_id() {
    reset();

    // Use the full u128 range: hi=u64::MAX, lo=u64::MAX -> tensor_id = u128::MAX.
    let data = vec![0.5f32; 64];
    assert_eq!(
        tts_put(u64::MAX, u64::MAX, 0, data.as_ptr(), data.len()),
        0,
        "put with max tensor_id should succeed"
    );

    let mut out = vec![0.0f32; 64];
    let n = tts_get(u64::MAX, u64::MAX, 0, out.as_mut_ptr(), out.len());
    assert_eq!(n, 64, "get with max tensor_id should succeed");
}
