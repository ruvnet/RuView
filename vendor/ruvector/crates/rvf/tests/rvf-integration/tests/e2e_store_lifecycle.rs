//! Full Store Lifecycle end-to-end acceptance tests.
//!
//! Exercises the complete RVF pipeline: create -> ingest -> query -> close ->
//! reopen -> query -> delete -> compact -> verify. Based on the primary
//! acceptance test from the RVF spec.

use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use tempfile::TempDir;

/// Deterministic pseudo-random vector generation using an LCG.
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

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

// --------------------------------------------------------------------------
// 1. Create store, ingest 10 batches of 100 vectors, query after each
// --------------------------------------------------------------------------
#[test]
fn lifecycle_batch_ingest_with_progressive_queries() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("progressive.rvf");
    let dim: u16 = 32;
    let batch_size: usize = 100;
    let num_batches: usize = 10;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Fixed query vector that we check against after each batch.
    let query = random_vector(dim as usize, 999999);
    let mut prev_result_count = 0usize;

    for batch in 0..num_batches {
        let base_id = (batch * batch_size + 1) as u64;
        let vectors: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| random_vector(dim as usize, (base_id + i as u64) * 7 + 3))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + batch_size as u64).collect();

        let result = store.ingest_batch(&refs, &ids, None).unwrap();
        assert_eq!(
            result.accepted, batch_size as u64,
            "batch {batch}: expected {batch_size} accepted"
        );

        // Query after each batch.
        let results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert!(
            results.len() >= prev_result_count.min(10),
            "batch {batch}: result count should not decrease"
        );
        prev_result_count = results.len();

        // Status should reflect cumulative count.
        let status = store.status();
        let expected = ((batch + 1) * batch_size) as u64;
        assert_eq!(
            status.total_vectors, expected,
            "batch {batch}: expected {expected} total vectors, got {}",
            status.total_vectors
        );
    }

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 2. Close and reopen store (progressive boot test)
// --------------------------------------------------------------------------
#[test]
fn lifecycle_close_reopen_data_persists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("reopen.rvf");
    let dim: u16 = 16;

    // Phase 1: create and populate.
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (1..=500)
            .map(|i| random_vector(dim as usize, i * 13 + 7))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=500).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Phase 2: reopen and verify.
    {
        let store = RvfStore::open(&path).unwrap();
        let status = store.status();
        assert_eq!(
            status.total_vectors, 500,
            "all 500 vectors should persist after reopen"
        );

        // Query immediately after reopen.
        let query = random_vector(dim as usize, 13 + 7); // same as vector id=1
        let results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert_eq!(results.len(), 10);
        // The closest result should be the matching vector.
        assert_eq!(
            results[0].id, 1,
            "exact match vector should be first result"
        );
        assert!(
            results[0].distance < 1e-6,
            "exact match should have near-zero distance, got {}",
            results[0].distance
        );
        store.close().unwrap();
    }
}

// --------------------------------------------------------------------------
// 3. Query immediately on reopen (Layer A availability)
// --------------------------------------------------------------------------
#[test]
fn lifecycle_first_query_after_reopen_returns_results() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("first_query.rvf");
    let dim: u16 = 8;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..200).map(|i| random_vector(dim as usize, i)).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=200).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    let store = RvfStore::open_readonly(&path).unwrap();
    let query = random_vector(dim as usize, 50); // matches vector 51
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
    assert!(
        !results.is_empty(),
        "first query after reopen should return results"
    );
    // Verify sorting.
    for i in 1..results.len() {
        assert!(
            results[i - 1].distance <= results[i].distance,
            "results not sorted: {} > {}",
            results[i - 1].distance,
            results[i].distance
        );
    }
}

// --------------------------------------------------------------------------
// 4. Delete vectors and verify exclusion from results
// --------------------------------------------------------------------------
#[test]
fn lifecycle_delete_vectors_excluded_from_query() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete_excl.rvf");
    let dim: u16 = 8;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let vectors: Vec<Vec<f32>> = (0..100).map(|i| random_vector(dim as usize, i)).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=100).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Delete the first 10 vectors.
    let delete_ids: Vec<u64> = (1..=10).collect();
    let del_result = store.delete(&delete_ids).unwrap();
    assert_eq!(del_result.deleted, 10);

    // Query and verify no deleted IDs appear.
    let query = random_vector(dim as usize, 0); // close to vector 1
    let results = store.query(&query, 100, &QueryOptions::default()).unwrap();
    for r in &results {
        assert!(
            r.id > 10,
            "deleted vector {} should not appear in results",
            r.id
        );
    }
    assert_eq!(
        results.len(),
        90,
        "should have 90 results after deleting 10"
    );

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 5. Delete persists through close/reopen
// --------------------------------------------------------------------------
#[test]
fn lifecycle_delete_persists_after_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("del_persist.rvf");
    let dim: u16 = 4;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=20).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.delete(&[5, 10, 15]).unwrap();
        store.close().unwrap();
    }

    {
        let store = RvfStore::open_readonly(&path).unwrap();
        let status = store.status();
        assert_eq!(
            status.total_vectors, 17,
            "17 vectors should remain after deleting 3"
        );

        let query = vec![5.0f32; dim as usize];
        let results = store.query(&query, 20, &QueryOptions::default()).unwrap();
        for r in &results {
            assert!(
                r.id != 5 && r.id != 10 && r.id != 15,
                "deleted vector {} appeared after reopen",
                r.id
            );
        }
    }
}

// --------------------------------------------------------------------------
// 6. Compact and verify results unchanged
// --------------------------------------------------------------------------
#[test]
fn lifecycle_compact_preserves_query_results() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact_preserves.rvf");
    let dim: u16 = 8;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let vectors: Vec<Vec<f32>> = (0..50).map(|i| random_vector(dim as usize, i)).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=50).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Delete first 20.
    let delete_ids: Vec<u64> = (1..=20).collect();
    store.delete(&delete_ids).unwrap();

    // Query before compaction.
    let query = random_vector(dim as usize, 30); // matches vector 31
    let before = store.query(&query, 10, &QueryOptions::default()).unwrap();

    // Compact.
    let compact_result = store.compact().unwrap();
    assert!(
        compact_result.segments_compacted > 0 || compact_result.bytes_reclaimed > 0,
        "compaction should reclaim space"
    );

    // Query after compaction should return same results.
    let after = store.query(&query, 10, &QueryOptions::default()).unwrap();
    assert_eq!(
        before.len(),
        after.len(),
        "result count should be the same before and after compaction"
    );
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(
            b.id, a.id,
            "result IDs should match before/after compaction"
        );
        assert!(
            (b.distance - a.distance).abs() < 1e-6,
            "distances should match before/after compaction"
        );
    }

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 7. Status reports correct counts through lifecycle
// --------------------------------------------------------------------------
#[test]
fn lifecycle_status_reports_correct_counts() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("status.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Empty store.
    assert_eq!(store.status().total_vectors, 0);
    assert!(!store.status().read_only);

    // After ingest.
    let vectors: Vec<Vec<f32>> = (0..100).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=100).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();
    assert_eq!(store.status().total_vectors, 100);
    assert!(store.status().file_size > 0);

    // After delete.
    store.delete(&[50, 51, 52]).unwrap();
    assert_eq!(store.status().total_vectors, 97);
    assert!(
        store.status().dead_space_ratio > 0.0,
        "dead space should be > 0 after delete"
    );

    // After compact.
    store.compact().unwrap();
    assert_eq!(store.status().total_vectors, 97);

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 8. Multiple ingest-delete-query cycles
// --------------------------------------------------------------------------
#[test]
fn lifecycle_multiple_ingest_delete_cycles() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("cycles.rvf");
    let dim: u16 = 8;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    let mut total_live = 0u64;

    for cycle in 0..5u64 {
        // Ingest 50 vectors.
        let base_id = cycle * 100 + 1;
        let vectors: Vec<Vec<f32>> = (0..50)
            .map(|i| random_vector(dim as usize, base_id + i as u64))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + 50).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        total_live += 50;

        // Delete 10 from this batch.
        let del_ids: Vec<u64> = (base_id..base_id + 10).collect();
        store.delete(&del_ids).unwrap();
        total_live -= 10;

        assert_eq!(
            store.status().total_vectors,
            total_live,
            "cycle {cycle}: expected {total_live} live vectors"
        );

        // Query should return results.
        let query = random_vector(dim as usize, base_id + 25);
        let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
        assert!(
            !results.is_empty(),
            "cycle {cycle}: query should return results"
        );
    }

    assert_eq!(store.status().total_vectors, 200); // 5 * 40

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 9. Large dimension vectors
// --------------------------------------------------------------------------
#[test]
fn lifecycle_high_dimension_384() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("highdim.rvf");
    let dim: u16 = 384; // sentence embedding size from spec

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest 100 vectors of dim 384.
    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|i| random_vector(dim as usize, i * 42 + 7))
        .collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=100).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Query with known vector.
    let query = vectors[49].clone(); // should match id=50
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0].id, 50, "exact match should be first");
    assert!(results[0].distance < 1e-6);

    store.close().unwrap();

    // Reopen and verify.
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(store.status().total_vectors, 100);
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
    assert_eq!(results[0].id, 50);
}

// --------------------------------------------------------------------------
// 10. Compact then reopen
// --------------------------------------------------------------------------
#[test]
fn lifecycle_compact_then_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact_reopen.rvf");
    let dim: u16 = 8;

    // Create, populate, delete, compact.
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

        let vectors: Vec<Vec<f32>> = (0..100).map(|i| random_vector(dim as usize, i)).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=100).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();

        // Delete half.
        let del_ids: Vec<u64> = (1..=50).collect();
        store.delete(&del_ids).unwrap();

        // Compact.
        store.compact().unwrap();
        assert_eq!(store.status().total_vectors, 50);

        store.close().unwrap();
    }

    // Reopen and verify.
    {
        let store = RvfStore::open_readonly(&path).unwrap();
        assert_eq!(store.status().total_vectors, 50);

        let query = random_vector(dim as usize, 75); // matches vector 76
        let results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert!(!results.is_empty());
        // All results should have id > 50.
        for r in &results {
            assert!(r.id > 50, "post-compact reopen: id {} should be > 50", r.id);
        }
    }
}

// --------------------------------------------------------------------------
// 11. Epoch advances correctly
// --------------------------------------------------------------------------
#[test]
fn lifecycle_epoch_advances() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("epoch.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let initial_epoch = store.status().current_epoch;

    // Ingest should advance epoch.
    let v = vec![1.0f32; dim as usize];
    let ingest_result = store.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
    assert!(
        ingest_result.epoch > initial_epoch,
        "epoch should advance after ingest"
    );

    // Delete should advance epoch.
    let del_result = store.delete(&[1]).unwrap();
    assert!(
        del_result.epoch > ingest_result.epoch,
        "epoch should advance after delete"
    );

    // Compact should advance epoch.
    let compact_result = store.compact().unwrap();
    assert!(
        compact_result.epoch > del_result.epoch,
        "epoch should advance after compact"
    );

    store.close().unwrap();
}

// --------------------------------------------------------------------------
// 12. Dimension mismatch rejected
// --------------------------------------------------------------------------
#[test]
fn lifecycle_dimension_mismatch_rejected() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("dim_mismatch.rvf");
    let dim: u16 = 8;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Correct dimension.
    let good = vec![1.0f32; dim as usize];
    let result = store.ingest_batch(&[good.as_slice()], &[1], None).unwrap();
    assert_eq!(result.accepted, 1);

    // Wrong dimension: should be rejected.
    let bad = vec![1.0f32; 4]; // dim=4 when store expects dim=8
    let result = store.ingest_batch(&[bad.as_slice()], &[2], None).unwrap();
    assert_eq!(
        result.accepted, 0,
        "wrong-dimension vector should be rejected"
    );
    assert_eq!(result.rejected, 1);

    // Query with wrong dimension should fail.
    let bad_query = vec![1.0f32; 4];
    assert!(
        store
            .query(&bad_query, 5, &QueryOptions::default())
            .is_err(),
        "query with wrong dimension should fail"
    );

    store.close().unwrap();
}
