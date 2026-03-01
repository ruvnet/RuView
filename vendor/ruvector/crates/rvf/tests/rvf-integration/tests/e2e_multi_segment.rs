//! Multi-segment file end-to-end tests.
//!
//! Verifies correct behavior when a store contains many VEC_SEGs from
//! repeated ingest operations: all vectors are queryable, compaction
//! merges segments, and deletions work correctly across segments.

use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use tempfile::TempDir;

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed;
    for _ in 0..dim {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

// --------------------------------------------------------------------------
// 1. Ingest 100 vectors 20 times, creating 20 VEC_SEGs
// --------------------------------------------------------------------------
#[test]
fn multi_seg_twenty_batches_all_queryable() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi20.rvf");
    let dim: u16 = 8;
    let batch_size = 100usize;
    let num_batches = 20usize;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    for batch in 0..num_batches {
        let base_id = (batch * batch_size + 1) as u64;
        let vectors: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| random_vector(dim as usize, base_id + i as u64))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + batch_size as u64).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
    }

    let total = (num_batches * batch_size) as u64;
    assert_eq!(store.status().total_vectors, total);

    // Query for a vector from each batch to verify all segments are accessible.
    for batch in 0..num_batches {
        let target_id = (batch * batch_size + 50 + 1) as u64; // mid-batch vector
        let target_vec = random_vector(dim as usize, target_id);
        let results = store
            .query(&target_vec, 5, &QueryOptions::default())
            .unwrap();
        assert!(
            !results.is_empty(),
            "batch {batch}: query should return results"
        );
        assert_eq!(
            results[0].id, target_id,
            "batch {batch}: exact match should be first result"
        );
        assert!(
            results[0].distance < 1e-6,
            "batch {batch}: exact match distance should be near zero"
        );
    }

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 2. Verify segment count increases with batches
// --------------------------------------------------------------------------
#[test]
fn multi_seg_segment_count_increases() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("seg_count.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let initial_segments = store.status().total_segments;

    for batch in 0..5 {
        let base_id = (batch * 10 + 1) as u64;
        let vectors: Vec<Vec<f32>> = (0..10)
            .map(|i| vec![(base_id + i as u64) as f32; dim as usize])
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + 10).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
    }

    let final_segments = store.status().total_segments;
    assert!(
        final_segments > initial_segments,
        "segment count should increase after multiple ingests: initial={initial_segments}, final={final_segments}"
    );

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 3. Compact merges multiple segments
// --------------------------------------------------------------------------
#[test]
fn multi_seg_compact_merges_segments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("merge.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest in 10 small batches.
    for batch in 0..10 {
        let base_id = (batch * 20 + 1) as u64;
        let vectors: Vec<Vec<f32>> = (0..20)
            .map(|i| vec![(base_id + i as u64) as f32; dim as usize])
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + 20).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
    }

    assert_eq!(store.status().total_vectors, 200);

    // Delete the first 50 vectors (spanning multiple segments).
    let del_ids: Vec<u64> = (1..=50).collect();
    store.delete(&del_ids).unwrap();
    assert_eq!(store.status().total_vectors, 150);

    // Compact.
    let compact_result = store.compact().unwrap();
    assert!(
        compact_result.segments_compacted > 0 || compact_result.bytes_reclaimed > 0,
        "compaction should do some work"
    );

    // All remaining 150 vectors should still be queryable.
    assert_eq!(store.status().total_vectors, 150);

    // Spot-check: query for a vector from the middle (batch 5, id 101).
    let target_vec = vec![101.0f32; dim as usize];
    let results = store
        .query(&target_vec, 5, &QueryOptions::default())
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, 101, "vector 101 should be first result");

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 4. Delete first 500 from 2000 vectors, verify deletion bitmap
// --------------------------------------------------------------------------
#[test]
fn multi_seg_delete_first_500_from_2000() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("del500.rvf");
    let dim: u16 = 8;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest 2000 vectors in batches of 200.
    for batch in 0..10 {
        let base_id = (batch * 200 + 1) as u64;
        let vectors: Vec<Vec<f32>> = (0..200)
            .map(|i| random_vector(dim as usize, base_id + i as u64))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + 200).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
    }

    assert_eq!(store.status().total_vectors, 2000);

    // Delete first 500.
    let del_ids: Vec<u64> = (1..=500).collect();
    let del_result = store.delete(&del_ids).unwrap();
    assert_eq!(del_result.deleted, 500);
    assert_eq!(store.status().total_vectors, 1500);

    // Query for a deleted vector (id=250): should not appear in results.
    let target = random_vector(dim as usize, 250);
    let results = store.query(&target, 100, &QueryOptions::default()).unwrap();
    for r in &results {
        assert!(
            r.id > 500,
            "deleted vector {} should not appear in results",
            r.id
        );
    }

    // Query for a live vector (id=750): should appear.
    let live_target = random_vector(dim as usize, 750);
    let results = store
        .query(&live_target, 5, &QueryOptions::default())
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, 750, "live vector 750 should be found");

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 5. Compact after deletion, then verify remaining vectors
// --------------------------------------------------------------------------
#[test]
fn multi_seg_compact_after_delete_verifies_remaining() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact_del.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest 500 vectors in 5 batches.
    for batch in 0..5 {
        let base_id = (batch * 100 + 1) as u64;
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| vec![(base_id + i as u64) as f32; dim as usize])
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + 100).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
    }

    // Delete first 200.
    let del_ids: Vec<u64> = (1..=200).collect();
    store.delete(&del_ids).unwrap();
    assert_eq!(store.status().total_vectors, 300);

    // Compact.
    store.compact().unwrap();
    assert_eq!(store.status().total_vectors, 300);

    // Query: vector 300 should be findable.
    let target_vec = vec![300.0f32; dim as usize];
    let results = store
        .query(&target_vec, 10, &QueryOptions::default())
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, 300);

    // All remaining IDs should be in range [201, 500].
    let all_results = store
        .query(&vec![0.0f32; dim as usize], 300, &QueryOptions::default())
        .unwrap();
    assert_eq!(all_results.len(), 300);
    for r in &all_results {
        assert!(
            r.id >= 201 && r.id <= 500,
            "after compact, id {} should be in [201, 500]",
            r.id
        );
    }

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 6. Second compact after more deletions reclaims additional space
// --------------------------------------------------------------------------
#[test]
fn multi_seg_double_compact() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("double_compact.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    let vectors: Vec<Vec<f32>> = (0..200).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=200).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // First round: delete 50, compact.
    let del1: Vec<u64> = (1..=50).collect();
    store.delete(&del1).unwrap();
    store.compact().unwrap();
    assert_eq!(store.status().total_vectors, 150);

    // Second round: delete 50 more, compact.
    let del2: Vec<u64> = (51..=100).collect();
    store.delete(&del2).unwrap();
    store.compact().unwrap();
    assert_eq!(store.status().total_vectors, 100);

    // All remaining should be in [101, 200].
    let query = vec![150.0f32; dim as usize];
    let results = store.query(&query, 100, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), 100);
    for r in &results {
        assert!(
            r.id >= 101 && r.id <= 200,
            "after double compact, id {} should be in [101, 200]",
            r.id
        );
    }

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 7. Reopen after multi-segment ingest preserves all data
// --------------------------------------------------------------------------
#[test]
fn multi_seg_reopen_preserves_all_batches() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_reopen.rvf");
    let dim: u16 = 8;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        for batch in 0..5 {
            let base_id = (batch * 100 + 1) as u64;
            let vectors: Vec<Vec<f32>> = (0..100)
                .map(|i| random_vector(dim as usize, base_id + i as u64))
                .collect();
            let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
            let ids: Vec<u64> = (base_id..base_id + 100).collect();
            store.ingest_batch(&refs, &ids, None).unwrap();
        }
        store.close().unwrap();
    }

    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(store.status().total_vectors, 500);

    // Query for vectors from each batch.
    for batch in 0..5 {
        let target_id = (batch * 100 + 50 + 1) as u64;
        let target = random_vector(dim as usize, target_id);
        let results = store.query(&target, 1, &QueryOptions::default()).unwrap();
        assert_eq!(
            results.len(),
            1,
            "batch {batch}: should find exactly 1 result"
        );
        assert_eq!(
            results[0].id, target_id,
            "batch {batch}: found id {} instead of {}",
            results[0].id, target_id
        );
    }
}
