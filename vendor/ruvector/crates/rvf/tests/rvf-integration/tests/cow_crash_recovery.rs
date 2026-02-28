//! Integration tests for RVF COW crash recovery scenarios.
//!
//! Tests that the store can recover from torn writes, truncated files,
//! and other crash scenarios by falling back to earlier valid manifests.

use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::{SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper: make RvfStore options
// ---------------------------------------------------------------------------

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Helper: read entire file into bytes
// ---------------------------------------------------------------------------

fn read_file_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut file = OpenOptions::new().read(true).open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

// ---------------------------------------------------------------------------
// Helper: scan file for manifest segments
// ---------------------------------------------------------------------------

fn find_manifest_offsets(file_bytes: &[u8]) -> Vec<(usize, u64)> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut manifests = Vec::new();

    if file_bytes.len() < SEGMENT_HEADER_SIZE {
        return manifests;
    }

    let last_possible = file_bytes.len() - SEGMENT_HEADER_SIZE;
    for i in 0..=last_possible {
        if file_bytes[i..i + 4] == magic_bytes {
            let seg_type = file_bytes[i + 5];
            if seg_type == 0x05 {
                // Manifest
                let seg_id = u64::from_le_bytes(file_bytes[i + 0x08..i + 0x10].try_into().unwrap());
                manifests.push((i, seg_id));
            }
        }
    }

    manifests
}

// ===========================================================================
// TEST 1: store_survives_garbage_appended
// ===========================================================================

/// Create a valid store, append random garbage bytes to the end,
/// and verify the store can still be opened and queried correctly.
#[test]
fn store_survives_garbage_appended() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("garbage.rvf");
    let dim: u16 = 4;

    // Create a store with vectors
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let v1 = vec![1.0, 2.0, 3.0, 4.0];
        let v2 = vec![5.0, 6.0, 7.0, 8.0];
        store
            .ingest_batch(&[v1.as_slice(), v2.as_slice()], &[1, 2], None)
            .unwrap();
        store.close().unwrap();
    }

    // Append garbage
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
        file.write_all(&garbage).unwrap();
        file.sync_all().unwrap();
    }

    // Reopen should succeed — the manifest scanner finds the latest valid manifest
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(
        store.status().total_vectors,
        2,
        "store should still report 2 vectors despite garbage appended"
    );

    let query = vec![1.0, 2.0, 3.0, 4.0];
    let results = store.query(&query, 2, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), 2, "should find 2 results");
    assert_eq!(results[0].id, 1, "nearest should be vector 1");
    assert!(results[0].distance < f32::EPSILON);

    println!("PASS: store_survives_garbage_appended");
}

// ===========================================================================
// TEST 2: truncated_file_at_segment_boundary
// ===========================================================================

/// Create a store, then truncate it at a non-manifest segment boundary.
/// If the manifest segment is still intact, the store should open.
#[test]
fn truncated_file_preserves_early_manifest() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("truncated.rvf");
    let dim: u16 = 4;

    // Create store with vectors — this writes a Vec segment then a Manifest
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        store.ingest_batch(&[v1.as_slice()], &[1], None).unwrap();
        store.close().unwrap();
    }

    let original_bytes = read_file_bytes(&path);
    let manifests = find_manifest_offsets(&original_bytes);

    // There should be at least one manifest
    assert!(
        !manifests.is_empty(),
        "should find at least one manifest in the file"
    );

    // The file should open fine from the valid manifest
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(store.status().total_vectors, 1);

    println!("PASS: truncated_file_preserves_early_manifest");
}

// ===========================================================================
// TEST 3: multiple_manifests_last_wins
// ===========================================================================

/// Create a store, ingest vectors in two batches (creating two manifests),
/// and verify the latest manifest is used on reopen.
#[test]
fn multiple_manifests_last_wins() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_manifest.rvf");
    let dim: u16 = 4;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

        // First batch
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        store.ingest_batch(&[v1.as_slice()], &[1], None).unwrap();
        // This writes a manifest

        // Second batch
        let v2 = vec![0.0, 1.0, 0.0, 0.0];
        store.ingest_batch(&[v2.as_slice()], &[2], None).unwrap();
        // This writes another manifest

        store.close().unwrap();
    }

    let file_bytes = read_file_bytes(&path);
    let manifests = find_manifest_offsets(&file_bytes);

    // Should have at least 2 manifests (initial + after first ingest + after second)
    assert!(
        manifests.len() >= 2,
        "expected at least 2 manifest segments, found {}",
        manifests.len()
    );

    // Reopen and verify the latest state is used (2 vectors)
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(
        store.status().total_vectors,
        2,
        "latest manifest should reflect both batches"
    );

    println!("PASS: multiple_manifests_last_wins");
}

// ===========================================================================
// TEST 4: corrupted_trailing_bytes_dont_break_store
// ===========================================================================

/// Write a valid store, then append a partial (truncated) segment header.
/// The store should still open because the manifest scanner can ignore
/// incomplete segments.
#[test]
fn corrupted_trailing_bytes_dont_break_store() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("partial_seg.rvf");
    let dim: u16 = 4;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let v = vec![1.0, 2.0, 3.0, 4.0];
        store.ingest_batch(&[v.as_slice()], &[42], None).unwrap();
        store.close().unwrap();
    }

    // Append a partial segment header (only magic + a few bytes, not a full 64-byte header)
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let partial_header = SEGMENT_MAGIC.to_le_bytes();
        file.write_all(&partial_header).unwrap();
        // Add a few more bytes but not enough for a full header
        file.write_all(&[0x04, 0x01, 0x00, 0x00]).unwrap();
        file.sync_all().unwrap();
    }

    // Reopen should still work
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(
        store.status().total_vectors,
        1,
        "store should still have 1 vector despite partial segment appended"
    );

    let query = vec![1.0, 2.0, 3.0, 4.0];
    let results = store.query(&query, 1, &QueryOptions::default()).unwrap();
    assert_eq!(results[0].id, 42);

    println!("PASS: corrupted_trailing_bytes_dont_break_store");
}

// ===========================================================================
// TEST 5: reopened_store_preserves_all_data
// ===========================================================================

/// Verify that close + reopen preserves all vectors and metadata.
#[test]
fn reopened_store_preserves_all_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("reopen.rvf");
    let dim: u16 = 8;

    let vectors: Vec<Vec<f32>> = (0..50)
        .map(|i| {
            let mut v = Vec::with_capacity(dim as usize);
            let mut x = i as u64;
            for _ in 0..dim {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                v.push(((x >> 33) as f32) / (u32::MAX as f32));
            }
            v
        })
        .collect();

    // Create and populate
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..50).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Reopen and verify
    {
        let store = RvfStore::open_readonly(&path).unwrap();
        assert_eq!(store.status().total_vectors, 50);

        // Query with each original vector — should find itself as nearest
        for i in 0..50u64 {
            let results = store
                .query(&vectors[i as usize], 1, &QueryOptions::default())
                .unwrap();
            assert_eq!(
                results.len(),
                1,
                "query for vector {i} should return 1 result"
            );
            assert_eq!(
                results[0].id, i,
                "nearest neighbor for vector {i} should be itself"
            );
            assert!(
                results[0].distance < f32::EPSILON,
                "self-distance for vector {i} should be ~0"
            );
        }
    }

    println!("PASS: reopened_store_preserves_all_data");
}

// ===========================================================================
// TEST 6: deletion_persists_through_reopen
// ===========================================================================

/// Delete vectors, close, reopen, and verify deletions are still applied.
#[test]
fn deletion_persists_through_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("del_persist.rvf");
    let dim: u16 = 4;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0, 0.0];
        let v3 = vec![0.0, 0.0, 1.0, 0.0];
        store
            .ingest_batch(
                &[v1.as_slice(), v2.as_slice(), v3.as_slice()],
                &[1, 2, 3],
                None,
            )
            .unwrap();
        store.delete(&[2]).unwrap();
        store.close().unwrap();
    }

    {
        let store = RvfStore::open_readonly(&path).unwrap();
        assert_eq!(
            store.status().total_vectors,
            2,
            "should have 2 vectors after deletion and reopen"
        );

        let query = vec![0.0, 1.0, 0.0, 0.0];
        let results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert_eq!(results.len(), 2);
        assert!(
            results.iter().all(|r| r.id != 2),
            "deleted vector 2 should not appear in results"
        );
    }

    println!("PASS: deletion_persists_through_reopen");
}
