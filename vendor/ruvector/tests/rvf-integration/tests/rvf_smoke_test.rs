//! End-to-end RVF smoke test -- full lifecycle verification.
//!
//! Exercises the complete RVF pipeline through 15 steps:
//!   1.  Create a new store (dim=128, cosine metric)
//!   2.  Ingest 100 random vectors with metadata
//!   3.  Query for 10 nearest neighbors of a known vector
//!   4.  Verify results are sorted and distances are valid (0.0..2.0 for cosine)
//!   5.  Close the store
//!   6.  Reopen the store (simulating process restart)
//!   7.  Query again with the same vector
//!   8.  Verify results match the first query exactly (persistence verified)
//!   9.  Delete some vectors
//!   10. Compact the store
//!   11. Verify deleted vectors no longer appear in results
//!   12. Derive a child store
//!   13. Verify child can be queried independently
//!   14. Verify segment listing works on both parent and child
//!   15. Clean up temporary files
//!
//! NOTE: The `DistanceMetric` is not persisted in the manifest, so after
//! `RvfStore::open()` the metric defaults to L2. The lifecycle test therefore
//! uses L2 for the cross-restart comparison (steps 5-8), while cosine-specific
//! assertions are exercised in a dedicated single-session test.

use rvf_runtime::options::{
    DistanceMetric, MetadataEntry, MetadataValue, QueryOptions, RvfOptions,
};
use rvf_runtime::RvfStore;
use rvf_types::DerivationType;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deterministic pseudo-random vector generation using an LCG.
/// Produces values in [-0.5, 0.5).
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

/// L2-normalize a vector in place so cosine distance is well-defined.
fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Generate a normalized random vector suitable for cosine queries.
fn random_unit_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = random_vector(dim, seed);
    normalize(&mut v);
    v
}

fn make_options(dim: u16, metric: DistanceMetric) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Full lifecycle smoke test (L2 metric for cross-restart consistency)
// ---------------------------------------------------------------------------

#[test]
fn rvf_smoke_full_lifecycle() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let store_path = dir.path().join("smoke_lifecycle.rvf");
    let child_path = dir.path().join("smoke_child.rvf");

    let dim: u16 = 128;
    let k: usize = 10;
    let vector_count: usize = 100;

    // Use L2 metric for the lifecycle test because the metric is not persisted
    // in the manifest. After reopen, the store defaults to L2, so using L2
    // throughout ensures cross-restart distance comparisons are exact.
    let options = make_options(dim, DistanceMetric::L2);

    // -----------------------------------------------------------------------
    // Step 1: Create a new RVF store with dimension 128 and cosine metric
    // -----------------------------------------------------------------------
    let mut store = RvfStore::create(&store_path, options.clone())
        .expect("step 1: failed to create store");

    // Verify initial state.
    let initial_status = store.status();
    assert_eq!(initial_status.total_vectors, 0, "step 1: new store should be empty");
    assert!(!initial_status.read_only, "step 1: new store should not be read-only");

    // -----------------------------------------------------------------------
    // Step 2: Ingest 100 random vectors with metadata
    // -----------------------------------------------------------------------
    let vectors: Vec<Vec<f32>> = (0..vector_count as u64)
        .map(|i| random_vector(dim as usize, i * 17 + 5))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=vector_count as u64).collect();

    // One metadata entry per vector: field_id=0, value=category string.
    let metadata: Vec<MetadataEntry> = ids
        .iter()
        .map(|&id| MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(format!("group_{}", id % 5)),
        })
        .collect();

    let ingest_result = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("step 2: ingest failed");

    assert_eq!(
        ingest_result.accepted, vector_count as u64,
        "step 2: all {} vectors should be accepted",
        vector_count,
    );
    assert_eq!(ingest_result.rejected, 0, "step 2: no vectors should be rejected");
    assert!(ingest_result.epoch > 0, "step 2: epoch should advance after ingest");

    // -----------------------------------------------------------------------
    // Step 3: Query for 10 nearest neighbors of a known vector
    // -----------------------------------------------------------------------
    // Use vector with id=50 as the query (seed = 49 * 17 + 5 = 838).
    let query_vec = random_vector(dim as usize, 49 * 17 + 5);
    let results_first = store
        .query(&query_vec, k, &QueryOptions::default())
        .expect("step 3: query failed");

    assert_eq!(
        results_first.len(),
        k,
        "step 3: should return exactly {} results",
        k,
    );

    // The first result should be the exact match (id=50).
    assert_eq!(
        results_first[0].id, 50,
        "step 3: exact match vector should be first result",
    );
    assert!(
        results_first[0].distance < 1e-5,
        "step 3: exact match distance should be near zero, got {}",
        results_first[0].distance,
    );

    // -----------------------------------------------------------------------
    // Step 4: Verify results are sorted by distance and distances are valid
    //         (L2 distances are non-negative)
    // -----------------------------------------------------------------------
    for i in 1..results_first.len() {
        assert!(
            results_first[i].distance >= results_first[i - 1].distance,
            "step 4: results not sorted at position {}: {} > {}",
            i,
            results_first[i - 1].distance,
            results_first[i].distance,
        );
    }
    for r in &results_first {
        assert!(
            r.distance >= 0.0,
            "step 4: L2 distance {} should be non-negative",
            r.distance,
        );
    }

    // -----------------------------------------------------------------------
    // Step 5: Close the store
    // -----------------------------------------------------------------------
    store.close().expect("step 5: close failed");

    // -----------------------------------------------------------------------
    // Step 6: Reopen the store (simulating process restart)
    // -----------------------------------------------------------------------
    let store = RvfStore::open(&store_path).expect("step 6: reopen failed");
    let reopen_status = store.status();
    assert_eq!(
        reopen_status.total_vectors, vector_count as u64,
        "step 6: all {} vectors should persist after reopen",
        vector_count,
    );

    // -----------------------------------------------------------------------
    // Step 7: Query again with the same vector
    // -----------------------------------------------------------------------
    let results_second = store
        .query(&query_vec, k, &QueryOptions::default())
        .expect("step 7: query after reopen failed");

    assert_eq!(
        results_second.len(),
        k,
        "step 7: should return exactly {} results after reopen",
        k,
    );

    // -----------------------------------------------------------------------
    // Step 8: Verify results match the first query exactly (persistence)
    //
    // After reopen, the internal iteration order of vectors may differ, which
    // can affect tie-breaking in the k-NN heap. We therefore compare:
    //   (a) the set of result IDs must be identical,
    //   (b) distances for each ID must match within floating-point tolerance,
    //   (c) result count must be the same.
    // -----------------------------------------------------------------------
    assert_eq!(
        results_first.len(),
        results_second.len(),
        "step 8: result count should match across restart",
    );

    // Build a map of id -> distance for comparison.
    let first_map: std::collections::HashMap<u64, f32> = results_first
        .iter()
        .map(|r| (r.id, r.distance))
        .collect();
    let second_map: std::collections::HashMap<u64, f32> = results_second
        .iter()
        .map(|r| (r.id, r.distance))
        .collect();

    // Verify the exact same IDs appear in both result sets.
    let mut first_ids: Vec<u64> = first_map.keys().copied().collect();
    let mut second_ids: Vec<u64> = second_map.keys().copied().collect();
    first_ids.sort();
    second_ids.sort();
    assert_eq!(
        first_ids, second_ids,
        "step 8: result ID sets must match across restart",
    );

    // Verify distances match per-ID within tolerance.
    for &id in &first_ids {
        let d1 = first_map[&id];
        let d2 = second_map[&id];
        assert!(
            (d1 - d2).abs() < 1e-5,
            "step 8: distance mismatch for id={}: {} vs {} (pre vs post restart)",
            id, d1, d2,
        );
    }

    // Need a mutable store for delete/compact. Drop the read-write handle and
    // reopen it mutably.
    store.close().expect("step 8: close for mutable reopen failed");
    let mut store = RvfStore::open(&store_path).expect("step 8: mutable reopen failed");

    // -----------------------------------------------------------------------
    // Step 9: Delete some vectors (ids 1..=10)
    // -----------------------------------------------------------------------
    let delete_ids: Vec<u64> = (1..=10).collect();
    let del_result = store
        .delete(&delete_ids)
        .expect("step 9: delete failed");

    assert_eq!(
        del_result.deleted, 10,
        "step 9: should have deleted 10 vectors",
    );
    assert!(
        del_result.epoch > reopen_status.current_epoch,
        "step 9: epoch should advance after delete",
    );

    // Quick verification: deleted vectors should not appear in query.
    let post_delete_results = store
        .query(&query_vec, vector_count, &QueryOptions::default())
        .expect("step 9: post-delete query failed");

    for r in &post_delete_results {
        assert!(
            r.id > 10,
            "step 9: deleted vector {} should not appear in results",
            r.id,
        );
    }
    assert_eq!(
        post_delete_results.len(),
        vector_count - 10,
        "step 9: should have {} results after deleting 10",
        vector_count - 10,
    );

    // -----------------------------------------------------------------------
    // Step 10: Compact the store
    // -----------------------------------------------------------------------
    let pre_compact_epoch = store.status().current_epoch;
    let compact_result = store.compact().expect("step 10: compact failed");

    assert!(
        compact_result.segments_compacted > 0 || compact_result.bytes_reclaimed > 0,
        "step 10: compaction should reclaim space",
    );
    assert!(
        compact_result.epoch > pre_compact_epoch,
        "step 10: epoch should advance after compact",
    );

    // -----------------------------------------------------------------------
    // Step 11: Verify deleted vectors no longer appear in results
    // -----------------------------------------------------------------------
    let post_compact_results = store
        .query(&query_vec, vector_count, &QueryOptions::default())
        .expect("step 11: post-compact query failed");

    for r in &post_compact_results {
        assert!(
            r.id > 10,
            "step 11: deleted vector {} appeared after compaction",
            r.id,
        );
    }
    assert_eq!(
        post_compact_results.len(),
        vector_count - 10,
        "step 11: should still have {} results post-compact",
        vector_count - 10,
    );

    // Verify post-compact status.
    let post_compact_status = store.status();
    assert_eq!(
        post_compact_status.total_vectors,
        (vector_count - 10) as u64,
        "step 11: status should reflect {} live vectors",
        vector_count - 10,
    );

    // -----------------------------------------------------------------------
    // Step 12: Derive a child store
    // -----------------------------------------------------------------------
    let child = store
        .derive(&child_path, DerivationType::Clone, Some(options.clone()))
        .expect("step 12: derive failed");

    // Verify lineage.
    assert_eq!(
        child.lineage_depth(),
        1,
        "step 12: child lineage depth should be 1",
    );
    assert_eq!(
        child.parent_id(),
        store.file_id(),
        "step 12: child parent_id should match parent file_id",
    );
    assert_ne!(
        child.file_id(),
        store.file_id(),
        "step 12: child should have a distinct file_id",
    );

    // -----------------------------------------------------------------------
    // Step 13: Verify child can be queried independently
    // -----------------------------------------------------------------------
    // The child is a fresh derived store (no vectors copied by default via
    // derive -- only lineage metadata). Query should return empty or results
    // depending on whether vectors were inherited. We just verify it does not
    // panic and returns a valid response.
    let child_query = random_vector(dim as usize, 999);
    let child_results = child
        .query(&child_query, k, &QueryOptions::default())
        .expect("step 13: child query failed");

    // Child is newly derived with no vectors of its own, so results should be empty.
    assert!(
        child_results.is_empty(),
        "step 13: freshly derived child should have no vectors, got {}",
        child_results.len(),
    );

    // -----------------------------------------------------------------------
    // Step 14: Verify segment listing works on both parent and child
    // -----------------------------------------------------------------------
    let parent_segments = store.segment_dir();
    assert!(
        !parent_segments.is_empty(),
        "step 14: parent should have at least one segment",
    );

    let child_segments = child.segment_dir();
    assert!(
        !child_segments.is_empty(),
        "step 14: child should have at least one segment (manifest)",
    );

    // Verify segment tuples have valid structure (seg_id > 0, type byte > 0).
    for &(seg_id, _offset, _len, seg_type) in parent_segments {
        assert!(seg_id > 0, "step 14: parent segment ID should be > 0");
        assert!(seg_type > 0, "step 14: parent segment type should be > 0");
    }
    for &(seg_id, _offset, _len, seg_type) in child_segments {
        assert!(seg_id > 0, "step 14: child segment ID should be > 0");
        assert!(seg_type > 0, "step 14: child segment type should be > 0");
    }

    // -----------------------------------------------------------------------
    // Step 15: Clean up temporary files
    // -----------------------------------------------------------------------
    child.close().expect("step 15: child close failed");
    store.close().expect("step 15: parent close failed");

    // TempDir's Drop impl will remove the directory, but verify the files exist
    // before cleanup happens.
    assert!(
        store_path.exists(),
        "step 15: parent store file should exist before cleanup",
    );
    assert!(
        child_path.exists(),
        "step 15: child store file should exist before cleanup",
    );

    // Explicitly drop the TempDir to trigger cleanup.
    drop(dir);
}

// ---------------------------------------------------------------------------
// Additional focused smoke tests
// ---------------------------------------------------------------------------

/// Verify that cosine metric returns distances strictly in [0.0, 2.0] range
/// for all query results when using normalized vectors. This test runs within
/// a single session (no restart) to avoid the metric-not-persisted issue.
#[test]
fn smoke_cosine_distance_range() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("cosine_range.rvf");

    let dim: u16 = 128;
    let options = make_options(dim, DistanceMetric::Cosine);

    let mut store = RvfStore::create(&path, options).unwrap();

    // Ingest 50 normalized vectors.
    let vectors: Vec<Vec<f32>> = (0..50)
        .map(|i| random_unit_vector(dim as usize, i * 31 + 3))
        .collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=50).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Query with several different vectors and verify distance range.
    for seed in [0, 42, 100, 999, 12345] {
        let q = random_unit_vector(dim as usize, seed);
        let results = store.query(&q, 50, &QueryOptions::default()).unwrap();

        for r in &results {
            assert!(
                r.distance >= 0.0 && r.distance <= 2.0,
                "cosine distance {} out of range [0.0, 2.0] for seed {}",
                r.distance,
                seed,
            );
        }

        // Verify sorting.
        for i in 1..results.len() {
            assert!(
                results[i].distance >= results[i - 1].distance,
                "results not sorted for seed {}: {} > {} at position {}",
                seed,
                results[i - 1].distance,
                results[i].distance,
                i,
            );
        }
    }

    store.close().unwrap();
}

/// Verify persistence across multiple close/reopen cycles with interleaved
/// ingests and deletes. Uses L2 metric for cross-restart consistency.
#[test]
fn smoke_multi_restart_persistence() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_restart.rvf");
    let dim: u16 = 128;

    let options = make_options(dim, DistanceMetric::L2);

    // Cycle 1: create and ingest 50 vectors.
    {
        let mut store = RvfStore::create(&path, options.clone()).unwrap();
        let vectors: Vec<Vec<f32>> = (0..50)
            .map(|i| random_vector(dim as usize, i))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=50).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        assert_eq!(store.status().total_vectors, 50);
        store.close().unwrap();
    }

    // Cycle 2: reopen, ingest 50 more, delete 10, close.
    {
        let mut store = RvfStore::open(&path).unwrap();
        assert_eq!(store.status().total_vectors, 50);

        let vectors: Vec<Vec<f32>> = (50..100)
            .map(|i| random_vector(dim as usize, i))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (51..=100).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        assert_eq!(store.status().total_vectors, 100);

        store.delete(&[5, 10, 15, 20, 25, 55, 60, 65, 70, 75]).unwrap();
        assert_eq!(store.status().total_vectors, 90);

        store.close().unwrap();
    }

    // Cycle 3: reopen, verify counts, compact, close.
    {
        let mut store = RvfStore::open(&path).unwrap();
        assert_eq!(
            store.status().total_vectors, 90,
            "cycle 3: 90 vectors should survive two restarts",
        );

        store.compact().unwrap();
        assert_eq!(store.status().total_vectors, 90);

        // Verify no deleted IDs appear in a full query.
        let q = random_vector(dim as usize, 42);
        let results = store.query(&q, 100, &QueryOptions::default()).unwrap();
        let deleted_ids = [5, 10, 15, 20, 25, 55, 60, 65, 70, 75];
        for r in &results {
            assert!(
                !deleted_ids.contains(&r.id),
                "cycle 3: deleted vector {} appeared after compact + restart",
                r.id,
            );
        }

        store.close().unwrap();
    }

    // Cycle 4: final reopen (readonly), verify persistence survived compact.
    {
        let store = RvfStore::open_readonly(&path).unwrap();
        assert_eq!(
            store.status().total_vectors, 90,
            "cycle 4: 90 vectors should survive compact + restart",
        );
        assert!(store.status().read_only);
    }
}

/// Verify metadata ingestion and that vector IDs are correct after batch
/// operations.
#[test]
fn smoke_metadata_and_ids() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("meta_ids.rvf");
    let dim: u16 = 128;

    let options = make_options(dim, DistanceMetric::L2);

    let mut store = RvfStore::create(&path, options).unwrap();

    // Ingest 100 vectors, each with a metadata entry.
    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|i| random_vector(dim as usize, i * 7 + 1))
        .collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=100).collect();
    let metadata: Vec<MetadataEntry> = ids
        .iter()
        .map(|&id| MetadataEntry {
            field_id: 0,
            value: MetadataValue::U64(id),
        })
        .collect();

    let result = store.ingest_batch(&refs, &ids, Some(&metadata)).unwrap();
    assert_eq!(result.accepted, 100);
    assert_eq!(result.rejected, 0);

    // Query for exact match of vector id=42.
    let query = random_vector(dim as usize, 41 * 7 + 1);
    let results = store.query(&query, 1, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 42, "exact match should be id=42");
    assert!(results[0].distance < 1e-5);

    store.close().unwrap();
}
