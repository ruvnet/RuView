//! RVF CLI / persistence smoke tests -- Phase 1 acceptance criteria.
//!
//! Validates the end-to-end lifecycle that the Node.js CLI wraps:
//!   1. Create an RVF store
//!   2. Ingest vectors
//!   3. Query and verify results
//!   4. Close (simulating process exit)
//!   5. Reopen (simulating process restart)
//!   6. Query again and verify identical results
//!
//! Also exercises the rvlite adapter layer for the same persistence
//! guarantee and tests that error paths produce clear messages.

use std::path::Path;

use rvf_adapter_rvlite::{RvliteCollection, RvliteConfig, RvliteMetric};
use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 1. Core RVF store: create -> ingest -> query -> close -> reopen -> query
// ---------------------------------------------------------------------------
#[test]
fn smoke_rvf_persistence_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("smoke.rvf");
    let dim: u16 = 32;
    let k = 5;

    // -- Phase 1: create, populate, query, record results, close ----------
    let results_before;
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

        // Ingest 200 vectors.
        let vectors: Vec<Vec<f32>> = (1..=200)
            .map(|i| random_vector(dim as usize, i * 13 + 7))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=200).collect();

        let ingest = store.ingest_batch(&refs, &ids, None).unwrap();
        assert_eq!(ingest.accepted, 200, "all 200 vectors should be accepted");

        // Query with a known vector (seed for id=100).
        let query = random_vector(dim as usize, 100 * 13 + 7);
        results_before = store.query(&query, k, &QueryOptions::default()).unwrap();
        assert_eq!(results_before.len(), k);
        assert_eq!(
            results_before[0].id, 100,
            "exact-match vector should be first"
        );
        assert!(
            results_before[0].distance < 1e-6,
            "exact-match distance should be near zero"
        );

        // Verify status before closing.
        let status = store.status();
        assert_eq!(status.total_vectors, 200);

        store.close().unwrap();
    }

    // -- Phase 2: reopen and verify identical results ---------------------
    {
        let store = RvfStore::open(&path).unwrap();

        // Status should reflect the same count.
        assert_eq!(
            store.status().total_vectors,
            200,
            "vector count must survive restart"
        );

        // Same query must produce identical results.
        let query = random_vector(dim as usize, 100 * 13 + 7);
        let results_after = store.query(&query, k, &QueryOptions::default()).unwrap();
        assert_eq!(results_after.len(), results_before.len());

        for (before, after) in results_before.iter().zip(results_after.iter()) {
            assert_eq!(before.id, after.id, "result IDs must match across restart");
            assert!(
                (before.distance - after.distance).abs() < 1e-6,
                "distances must match across restart: {} vs {}",
                before.distance,
                after.distance
            );
        }

        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// 2. Rvlite adapter: same persistence guarantee through the adapter API
// ---------------------------------------------------------------------------
#[test]
fn smoke_rvlite_adapter_persistence() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("adapter_smoke.rvf");
    let dim: u16 = 8;

    // -- Phase 1: create via adapter, add vectors, search, close ----------
    let results_before;
    {
        let config = RvliteConfig::new(path.clone(), dim).with_metric(RvliteMetric::L2);
        let mut col = RvliteCollection::create(config).unwrap();

        col.add(1, &[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
            .unwrap();
        col.add(2, &[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
            .unwrap();
        col.add(3, &[0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0])
            .unwrap();
        col.add(4, &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
            .unwrap();
        col.add(5, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0])
            .unwrap();

        assert_eq!(col.len(), 5);

        results_before = col.search(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 3);
        assert_eq!(results_before.len(), 3);
        assert_eq!(results_before[0].id, 1, "exact match should be first");
        assert!(results_before[0].distance < f32::EPSILON);

        col.close().unwrap();
    }

    // -- Phase 2: reopen via adapter, verify same results -----------------
    {
        let col = RvliteCollection::open(&path).unwrap();
        assert_eq!(col.len(), 5, "vector count must survive adapter restart");
        assert_eq!(col.dimension(), dim);

        let results_after = col.search(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 3);
        assert_eq!(results_after.len(), results_before.len());

        for (before, after) in results_before.iter().zip(results_after.iter()) {
            assert_eq!(
                before.id, after.id,
                "adapter result IDs must match across restart"
            );
            assert!(
                (before.distance - after.distance).abs() < 1e-6,
                "adapter distances must match across restart"
            );
        }

        col.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// 3. Delete-then-restart: deletions survive process restart
// ---------------------------------------------------------------------------
#[test]
fn smoke_deletions_persist_across_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("del_persist_smoke.rvf");
    let dim: u16 = 4;

    // Phase 1: create, populate, delete some, close.
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=20).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();

        store.delete(&[5, 10, 15]).unwrap();
        assert_eq!(store.status().total_vectors, 17);
        store.close().unwrap();
    }

    // Phase 2: reopen and verify deletions survived.
    {
        let store = RvfStore::open(&path).unwrap();
        assert_eq!(
            store.status().total_vectors,
            17,
            "17 vectors should remain after restart"
        );

        // Query with high k to get all results; deleted IDs must be absent.
        let query = vec![5.0f32; dim as usize];
        let results = store.query(&query, 20, &QueryOptions::default()).unwrap();
        for r in &results {
            assert!(
                r.id != 5 && r.id != 10 && r.id != 15,
                "deleted vector {} appeared after restart",
                r.id
            );
        }
        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// 4. Compact-then-restart: compacted store reopens correctly
// ---------------------------------------------------------------------------
#[test]
fn smoke_compact_then_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact_restart_smoke.rvf");
    let dim: u16 = 8;

    // Phase 1: create, populate, delete half, compact, record query, close.
    let results_before;
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..100).map(|i| random_vector(dim as usize, i)).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=100).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();

        let del_ids: Vec<u64> = (1..=50).collect();
        store.delete(&del_ids).unwrap();
        store.compact().unwrap();
        assert_eq!(store.status().total_vectors, 50);

        let query = random_vector(dim as usize, 75); // close to vector 76
        results_before = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert!(!results_before.is_empty());

        store.close().unwrap();
    }

    // Phase 2: reopen and verify same results.
    {
        let store = RvfStore::open(&path).unwrap();
        assert_eq!(store.status().total_vectors, 50);

        let query = random_vector(dim as usize, 75);
        let results_after = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert_eq!(results_before.len(), results_after.len());

        for (b, a) in results_before.iter().zip(results_after.iter()) {
            assert_eq!(b.id, a.id, "post-compact restart: IDs must match");
            assert!(
                (b.distance - a.distance).abs() < 1e-6,
                "post-compact restart: distances must match"
            );
        }

        // All results should have id > 50 (deleted ids were 1..=50).
        for r in &results_after {
            assert!(
                r.id > 50,
                "post-compact restart: deleted id {} should not appear",
                r.id
            );
        }

        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// 5. Missing dependency produces clear error message
// ---------------------------------------------------------------------------
#[test]
fn smoke_nonexistent_store_gives_clear_error() {
    // Opening a path that does not exist should produce a meaningful error,
    // not a panic. This mirrors the "missing @ruvector/rvf" scenario at the
    // Rust level -- the file simply doesn't exist.
    let result = RvfStore::open(Path::new("/tmp/nonexistent_rvf_smoke_test_12345.rvf"));
    assert!(result.is_err(), "opening nonexistent store should fail");
    let err_msg = match result {
        Err(e) => format!("{e}"),
        Ok(_) => panic!("expected error, got Ok"),
    };
    // The error message should be informative (not empty or cryptic).
    assert!(!err_msg.is_empty(), "error message should not be empty");
}
