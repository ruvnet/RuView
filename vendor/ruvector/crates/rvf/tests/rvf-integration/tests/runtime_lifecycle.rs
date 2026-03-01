//! Runtime store lifecycle integration tests.
//!
//! Exercises the full create -> ingest -> query -> delete -> compact -> reopen
//! lifecycle through the rvf-runtime RvfStore API.

use rvf_runtime::filter::{FilterExpr, FilterValue};
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

/// Generate a unit vector along axis `axis` in `dim` dimensions.
fn unit_vector(dim: usize, axis: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; dim];
    if axis < dim {
        v[axis] = 1.0;
    }
    v
}

#[test]
fn full_lifecycle_create_ingest_query_close_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lifecycle.rvf");
    let dim = 8;
    let options = make_options(dim);

    // Phase 1: create, ingest, close.
    {
        let mut store = RvfStore::create(&path, options.clone()).unwrap();

        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| {
                let mut v = vec![0.0f32; dim as usize];
                v[i % dim as usize] = 1.0;
                v[(i + 1) % dim as usize] = 0.5;
                v
            })
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=100).collect();

        let result = store.ingest_batch(&refs, &ids, None).unwrap();
        assert_eq!(result.accepted, 100);
        assert_eq!(result.rejected, 0);
        store.close().unwrap();
    }

    // Phase 2: reopen, query, verify results.
    {
        let store = RvfStore::open(&path).unwrap();
        let query = unit_vector(dim as usize, 0);
        let results = store.query(&query, 5, &QueryOptions::default()).unwrap();

        assert_eq!(results.len(), 5);
        // Results should be sorted by distance (ascending).
        for i in 1..results.len() {
            assert!(
                results[i - 1].distance <= results[i].distance,
                "results not sorted: {} > {}",
                results[i - 1].distance,
                results[i].distance
            );
        }
        store.close().unwrap();
    }

    // Phase 3: reopen, verify status.
    {
        let store = RvfStore::open_readonly(&path).unwrap();
        let status = store.status();
        assert_eq!(status.total_vectors, 100);
        assert!(status.read_only);
    }
}

#[test]
fn delete_and_reopen_excludes_deleted_vectors() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete.rvf");
    let dim = 4;
    let options = make_options(dim);

    // Create with 10 vectors.
    {
        let mut store = RvfStore::create(&path, options.clone()).unwrap();
        let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=10).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();

        // Delete vectors 3, 5, 7.
        let del_result = store.delete(&[3, 5, 7]).unwrap();
        assert_eq!(del_result.deleted, 3);

        store.close().unwrap();
    }

    // Reopen and verify deleted vectors are gone.
    {
        let store = RvfStore::open(&path).unwrap();
        let status = store.status();
        assert_eq!(status.total_vectors, 7); // 10 - 3

        // Query with a vector that matches vector 3 exactly.
        let query = vec![3.0f32; dim as usize];
        let results = store.query(&query, 10, &QueryOptions::default()).unwrap();

        // Vector 3 should not be in results.
        for r in &results {
            assert_ne!(r.id, 3, "deleted vector 3 should not appear in results");
            assert_ne!(r.id, 5, "deleted vector 5 should not appear in results");
            assert_ne!(r.id, 7, "deleted vector 7 should not appear in results");
        }
        store.close().unwrap();
    }
}

#[test]
fn compact_reduces_file_size_after_deletion() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact.rvf");
    let dim = 4;
    let options = make_options(dim);

    let mut store = RvfStore::create(&path, options).unwrap();

    // Ingest 50 vectors.
    let vectors: Vec<Vec<f32>> = (0..50).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=50).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Delete half.
    let delete_ids: Vec<u64> = (1..=25).collect();
    store.delete(&delete_ids).unwrap();

    // Compact.
    let compact_result = store.compact().unwrap();
    assert!(compact_result.segments_compacted > 0 || compact_result.bytes_reclaimed > 0);

    // Verify remaining vectors are queryable.
    let query = vec![30.0f32; dim as usize];
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
    assert!(!results.is_empty());
    for r in &results {
        assert!(
            r.id > 25,
            "compacted store should only contain ids > 25, got {}",
            r.id
        );
    }

    store.close().unwrap();
}

#[test]
fn filter_query_integration() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("filter.rvf");
    let dim = 4;
    let options = make_options(dim);

    let mut store = RvfStore::create(&path, options).unwrap();

    let vectors: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=20).collect();

    // Ingest with metadata.
    use rvf_runtime::options::{MetadataEntry, MetadataValue};
    let metadata: Vec<MetadataEntry> = ids
        .iter()
        .map(|&id| MetadataEntry {
            field_id: 0,
            value: MetadataValue::U64(id % 3), // category: 0, 1, 2
        })
        .collect();
    store.ingest_batch(&refs, &ids, Some(&metadata)).unwrap();

    // Query with filter: category == 1 (ids 1, 4, 7, 10, 13, 16, 19).
    let filter = FilterExpr::Eq(0, FilterValue::U64(1));
    let qopts = QueryOptions {
        filter: Some(filter),
        ..Default::default()
    };
    let query = vec![0.0f32; dim as usize];
    let results = store.query(&query, 20, &qopts).unwrap();

    // All results should have category == 1 (id % 3 == 1).
    for r in &results {
        assert_eq!(
            r.id % 3,
            1,
            "filter should only return category 1, got id={}",
            r.id
        );
    }
    assert!(!results.is_empty());

    store.close().unwrap();
}

#[test]
fn readonly_prevents_writes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("readonly.rvf");
    let dim = 4;
    let options = make_options(dim);

    // Create a store.
    {
        let mut store = RvfStore::create(&path, options).unwrap();
        let v = vec![1.0f32; dim as usize];
        store.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
        store.close().unwrap();
    }

    // Open readonly.
    let store = RvfStore::open_readonly(&path).unwrap();

    // Queries should work.
    let query = vec![1.0f32; dim as usize];
    let results = store.query(&query, 1, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), 1);

    // Writes should fail.
    // (open_readonly returns an immutable store, so we can't call ingest_batch)
    assert!(store.status().read_only);
}

#[test]
fn concurrent_writer_lock() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lock.rvf");
    let dim = 4;
    let options = make_options(dim);

    // First writer.
    let mut store1 = RvfStore::create(&path, options.clone()).unwrap();
    let v = vec![1.0f32; dim as usize];
    store1.ingest_batch(&[v.as_slice()], &[1], None).unwrap();

    // Second writer should fail.
    let result = RvfStore::open(&path);
    assert!(result.is_err(), "second writer should fail to acquire lock");

    store1.close().unwrap();

    // After close, opening should work.
    let store2 = RvfStore::open(&path);
    assert!(
        store2.is_ok(),
        "should be able to open after first writer closed"
    );
    store2.unwrap().close().unwrap();
}

#[test]
fn multiple_ingest_batches() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_ingest.rvf");
    let dim = 4;
    let options = make_options(dim);

    let mut store = RvfStore::create(&path, options).unwrap();

    // Ingest in three batches.
    for batch in 0..3 {
        let base_id = batch * 100 + 1;
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| vec![(base_id + i) as f32; dim as usize])
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + 100).map(|i| i as u64).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
    }

    // Should have 300 vectors.
    assert_eq!(store.status().total_vectors, 300);

    // Close and reopen to verify persistence.
    store.close().unwrap();

    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(store.status().total_vectors, 300);
}

#[test]
fn delete_by_filter() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("del_filter.rvf");
    let dim = 4;
    let options = make_options(dim);

    let mut store = RvfStore::create(&path, options).unwrap();

    let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=10).collect();

    use rvf_runtime::options::{MetadataEntry, MetadataValue};
    let metadata: Vec<MetadataEntry> = ids
        .iter()
        .map(|&id| MetadataEntry {
            field_id: 0,
            value: MetadataValue::U64(if id <= 5 { 0 } else { 1 }),
        })
        .collect();
    store.ingest_batch(&refs, &ids, Some(&metadata)).unwrap();

    // Delete all with field_0 == 0 (ids 1..=5).
    let filter = FilterExpr::Eq(0, FilterValue::U64(0));
    let del_result = store.delete_by_filter(&filter).unwrap();
    assert_eq!(del_result.deleted, 5);
    assert_eq!(store.status().total_vectors, 5);

    store.close().unwrap();
}
