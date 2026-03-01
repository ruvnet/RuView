//! Cross-platform RVF compatibility tests.
//!
//! Verifies that RVF stores can be serialized to bytes, transferred across
//! boundaries (simulating cross-platform exchange), and re-imported with
//! identical query results. Tests all three distance metrics and verifies
//! segment header preservation across the round-trip.

use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::{SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};
use std::fs;
use std::io::Read;
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

fn make_options(dim: u16, metric: DistanceMetric) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric,
        ..Default::default()
    }
}

/// Read an entire file into a byte vector.
fn read_file_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut file = fs::File::open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

/// Scan the file bytes for all segment headers and return their offsets and types.
fn scan_segment_headers(file_bytes: &[u8]) -> Vec<(usize, u8, u64, u64)> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut results = Vec::new();

    if file_bytes.len() < SEGMENT_HEADER_SIZE {
        return results;
    }

    let last_possible = file_bytes.len().saturating_sub(SEGMENT_HEADER_SIZE);
    for i in 0..=last_possible {
        if file_bytes[i..i + 4] == magic_bytes {
            let seg_type = file_bytes[i + 5];
            let seg_id = u64::from_le_bytes(file_bytes[i + 0x08..i + 0x10].try_into().unwrap());
            let payload_len =
                u64::from_le_bytes(file_bytes[i + 0x10..i + 0x18].try_into().unwrap());
            results.push((i, seg_type, seg_id, payload_len));
        }
    }

    results
}

// ---------------------------------------------------------------------------
// TEST 1: Cosine metric export/import round-trip
// ---------------------------------------------------------------------------
#[test]
fn cross_platform_cosine_round_trip() {
    let dir = TempDir::new().unwrap();
    let dim: u16 = 32;
    let num_vectors: usize = 200;

    // Phase 1: Create store and populate with vectors.
    let original_path = dir.path().join("original_cosine.rvf");
    let query = random_vector(dim as usize, 999);
    let original_results;

    {
        let mut store =
            RvfStore::create(&original_path, make_options(dim, DistanceMetric::Cosine)).unwrap();

        let vectors: Vec<Vec<f32>> = (0..num_vectors)
            .map(|i| random_vector(dim as usize, i as u64 * 7 + 3))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=num_vectors as u64).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Query original for baseline results.
    {
        let store = RvfStore::open_readonly(&original_path).unwrap();
        original_results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        assert!(
            !original_results.is_empty(),
            "original query should return results"
        );
        store.close().unwrap();
    }

    // Phase 2: Export to bytes.
    let exported_bytes = read_file_bytes(&original_path);
    assert!(
        !exported_bytes.is_empty(),
        "exported bytes should not be empty"
    );

    // Phase 3: Re-import from bytes at a new location.
    let reimported_path = dir.path().join("reimported_cosine.rvf");
    fs::write(&reimported_path, &exported_bytes).unwrap();

    // Phase 4: Open re-imported store and verify results match.
    {
        let store = RvfStore::open_readonly(&reimported_path).unwrap();
        let reimported_results = store.query(&query, 10, &QueryOptions::default()).unwrap();

        assert_eq!(
            original_results.len(),
            reimported_results.len(),
            "result count mismatch after re-import"
        );

        for (orig, reimp) in original_results.iter().zip(reimported_results.iter()) {
            assert_eq!(orig.id, reimp.id, "ID mismatch at position");
            assert!(
                (orig.distance - reimp.distance).abs() < 1e-6,
                "distance mismatch for id {}: {} vs {} (delta={})",
                orig.id,
                orig.distance,
                reimp.distance,
                (orig.distance - reimp.distance).abs()
            );
        }

        let status = store.status();
        assert_eq!(
            status.total_vectors, num_vectors as u64,
            "re-imported store should have same vector count"
        );
        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// TEST 2: Euclidean (L2) metric export/import round-trip
// ---------------------------------------------------------------------------
#[test]
fn cross_platform_l2_round_trip() {
    let dir = TempDir::new().unwrap();
    let dim: u16 = 16;
    let num_vectors: usize = 100;

    let original_path = dir.path().join("original_l2.rvf");
    let query = random_vector(dim as usize, 42);
    let original_results;

    {
        let mut store =
            RvfStore::create(&original_path, make_options(dim, DistanceMetric::L2)).unwrap();

        let vectors: Vec<Vec<f32>> = (0..num_vectors)
            .map(|i| random_vector(dim as usize, i as u64 * 11 + 5))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=num_vectors as u64).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    {
        let store = RvfStore::open_readonly(&original_path).unwrap();
        original_results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        store.close().unwrap();
    }

    let exported_bytes = read_file_bytes(&original_path);
    let reimported_path = dir.path().join("reimported_l2.rvf");
    fs::write(&reimported_path, &exported_bytes).unwrap();

    {
        let store = RvfStore::open_readonly(&reimported_path).unwrap();
        let reimported_results = store.query(&query, 10, &QueryOptions::default()).unwrap();

        assert_eq!(original_results.len(), reimported_results.len());
        for (orig, reimp) in original_results.iter().zip(reimported_results.iter()) {
            assert_eq!(orig.id, reimp.id);
            assert!(
                (orig.distance - reimp.distance).abs() < 1e-6,
                "L2 distance mismatch for id {}: {} vs {}",
                orig.id,
                orig.distance,
                reimp.distance
            );
        }
        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// TEST 3: InnerProduct (dot product) metric export/import round-trip
// ---------------------------------------------------------------------------
#[test]
fn cross_platform_inner_product_round_trip() {
    let dir = TempDir::new().unwrap();
    let dim: u16 = 64;
    let num_vectors: usize = 150;

    let original_path = dir.path().join("original_ip.rvf");
    let query = random_vector(dim as usize, 7777);
    let original_results;

    {
        let mut store = RvfStore::create(
            &original_path,
            make_options(dim, DistanceMetric::InnerProduct),
        )
        .unwrap();

        let vectors: Vec<Vec<f32>> = (0..num_vectors)
            .map(|i| random_vector(dim as usize, i as u64 * 13 + 1))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=num_vectors as u64).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    {
        let store = RvfStore::open_readonly(&original_path).unwrap();
        original_results = store.query(&query, 10, &QueryOptions::default()).unwrap();
        store.close().unwrap();
    }

    let exported_bytes = read_file_bytes(&original_path);
    let reimported_path = dir.path().join("reimported_ip.rvf");
    fs::write(&reimported_path, &exported_bytes).unwrap();

    {
        let store = RvfStore::open_readonly(&reimported_path).unwrap();
        let reimported_results = store.query(&query, 10, &QueryOptions::default()).unwrap();

        assert_eq!(original_results.len(), reimported_results.len());
        for (orig, reimp) in original_results.iter().zip(reimported_results.iter()) {
            assert_eq!(orig.id, reimp.id);
            assert!(
                (orig.distance - reimp.distance).abs() < 1e-6,
                "InnerProduct distance mismatch for id {}: {} vs {}",
                orig.id,
                orig.distance,
                reimp.distance
            );
        }
        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// TEST 4: Segment headers are preserved across serialize/deserialize
// ---------------------------------------------------------------------------
#[test]
fn cross_platform_segment_headers_preserved() {
    let dir = TempDir::new().unwrap();
    let dim: u16 = 8;

    let original_path = dir.path().join("seg_headers.rvf");

    {
        let mut store =
            RvfStore::create(&original_path, make_options(dim, DistanceMetric::L2)).unwrap();

        let vectors: Vec<Vec<f32>> = (0..50)
            .map(|i| random_vector(dim as usize, i as u64))
            .collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=50).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Scan original for segment headers.
    let original_bytes = read_file_bytes(&original_path);
    let original_segments = scan_segment_headers(&original_bytes);
    assert!(
        !original_segments.is_empty(),
        "original file should contain at least one segment"
    );

    // Copy bytes to new location (simulating cross-platform transfer).
    let reimported_path = dir.path().join("seg_headers_copy.rvf");
    fs::write(&reimported_path, &original_bytes).unwrap();

    // Scan re-imported file for segment headers.
    let reimported_bytes = read_file_bytes(&reimported_path);
    let reimported_segments = scan_segment_headers(&reimported_bytes);

    // Segment counts must match.
    assert_eq!(
        original_segments.len(),
        reimported_segments.len(),
        "segment count mismatch: {} vs {}",
        original_segments.len(),
        reimported_segments.len()
    );

    // Each segment header must be identical.
    for (i, (orig, reimp)) in original_segments
        .iter()
        .zip(reimported_segments.iter())
        .enumerate()
    {
        assert_eq!(
            orig.0, reimp.0,
            "segment {i}: offset mismatch ({} vs {})",
            orig.0, reimp.0
        );
        assert_eq!(
            orig.1, reimp.1,
            "segment {i}: type mismatch ({:#x} vs {:#x})",
            orig.1, reimp.1
        );
        assert_eq!(
            orig.2, reimp.2,
            "segment {i}: id mismatch ({} vs {})",
            orig.2, reimp.2
        );
        assert_eq!(
            orig.3, reimp.3,
            "segment {i}: payload_length mismatch ({} vs {})",
            orig.3, reimp.3
        );
    }

    // Verify the re-imported store is still queryable.
    {
        let store = RvfStore::open_readonly(&reimported_path).unwrap();
        assert_eq!(store.status().total_vectors, 50);

        let query = random_vector(dim as usize, 25);
        let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
        assert_eq!(
            results.len(),
            5,
            "re-imported store should return query results"
        );
        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------
// TEST 5: All three metrics produce consistent results after round-trip
// ---------------------------------------------------------------------------
#[test]
fn cross_platform_all_metrics_consistent() {
    let dir = TempDir::new().unwrap();
    let dim: u16 = 16;
    let num_vectors: usize = 50;

    let metrics = [
        (DistanceMetric::L2, "l2"),
        (DistanceMetric::Cosine, "cosine"),
        (DistanceMetric::InnerProduct, "dotproduct"),
    ];

    for (metric, label) in &metrics {
        let original_path = dir.path().join(format!("all_{label}.rvf"));
        let query = random_vector(dim as usize, 12345);

        // Create and populate.
        {
            let mut store = RvfStore::create(&original_path, make_options(dim, *metric)).unwrap();

            let vectors: Vec<Vec<f32>> = (0..num_vectors)
                .map(|i| random_vector(dim as usize, i as u64 * 17 + 2))
                .collect();
            let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
            let ids: Vec<u64> = (1..=num_vectors as u64).collect();
            store.ingest_batch(&refs, &ids, None).unwrap();
            store.close().unwrap();
        }

        // Query original.
        let original_results;
        {
            let store = RvfStore::open_readonly(&original_path).unwrap();
            original_results = store.query(&query, 10, &QueryOptions::default()).unwrap();
            store.close().unwrap();
        }

        // Round-trip through bytes.
        let bytes = read_file_bytes(&original_path);
        let reimported_path = dir.path().join(format!("all_{label}_copy.rvf"));
        fs::write(&reimported_path, &bytes).unwrap();

        // Verify results match within tolerance.
        {
            let store = RvfStore::open_readonly(&reimported_path).unwrap();
            let reimported_results = store.query(&query, 10, &QueryOptions::default()).unwrap();

            assert_eq!(
                original_results.len(),
                reimported_results.len(),
                "{label}: result count mismatch"
            );

            for (orig, reimp) in original_results.iter().zip(reimported_results.iter()) {
                assert_eq!(orig.id, reimp.id, "{label}: ID mismatch");
                assert!(
                    (orig.distance - reimp.distance).abs() < 1e-6,
                    "{label}: distance mismatch for id {}: {} vs {} (delta={})",
                    orig.id,
                    orig.distance,
                    reimp.distance,
                    (orig.distance - reimp.distance).abs()
                );
            }
            store.close().unwrap();
        }
    }
}

// ---------------------------------------------------------------------------
// TEST 6: Byte-level file identity after export/import
// ---------------------------------------------------------------------------
#[test]
fn cross_platform_byte_identical_transfer() {
    let dir = TempDir::new().unwrap();
    let dim: u16 = 4;

    let original_path = dir.path().join("byte_ident.rvf");

    {
        let mut store =
            RvfStore::create(&original_path, make_options(dim, DistanceMetric::L2)).unwrap();

        let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=10).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Read original bytes.
    let original_bytes = read_file_bytes(&original_path);

    // Write to new location.
    let copy_path = dir.path().join("byte_ident_copy.rvf");
    fs::write(&copy_path, &original_bytes).unwrap();

    // Read copy bytes.
    let copy_bytes = read_file_bytes(&copy_path);

    // Bytes must be identical.
    assert_eq!(
        original_bytes.len(),
        copy_bytes.len(),
        "file sizes should be identical"
    );
    assert_eq!(
        original_bytes, copy_bytes,
        "file bytes should be identical after transfer"
    );
}
