//! Integration tests for segment preservation during compaction.
//!
//! Tests that unknown or extension segments (Kernel, Ebpf, etc.) survive
//! compaction cycles, and that the compact operation correctly rewrites
//! vector data while preserving other segments byte-for-byte.

use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::{SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};
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
// Helper: read file bytes
// ---------------------------------------------------------------------------

fn read_file_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut file = OpenOptions::new().read(true).open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

// ---------------------------------------------------------------------------
// Helper: scan file for segments of a given type
// ---------------------------------------------------------------------------

fn scan_segments_of_type(file_bytes: &[u8], seg_type: u8) -> Vec<(usize, u64, u64)> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut results = Vec::new();

    if file_bytes.len() < SEGMENT_HEADER_SIZE {
        return results;
    }

    let last_possible = file_bytes.len() - SEGMENT_HEADER_SIZE;
    for i in 0..=last_possible {
        if file_bytes[i..i + 4] == magic_bytes {
            let found_type = file_bytes[i + 5];
            if found_type == seg_type {
                let seg_id = u64::from_le_bytes(file_bytes[i + 0x08..i + 0x10].try_into().unwrap());
                let payload_len =
                    u64::from_le_bytes(file_bytes[i + 0x10..i + 0x18].try_into().unwrap());
                results.push((i, seg_id, payload_len));
            }
        }
    }

    results
}

// ===========================================================================
// TEST 1: kernel_segment_survives_compaction
// ===========================================================================

/// Embed a kernel into a store, compact, and verify the kernel segment
/// is preserved in the compacted file.
#[test]
fn kernel_segment_survives_compaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kernel_compact.rvf");
    let dim: u16 = 4;

    let kernel_image = b"test-kernel-image-for-compaction-test";

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest vectors
    let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..10).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Embed kernel
    let _kernel_seg_id = store
        .embed_kernel(0x00, 0x00, 0, kernel_image, 8080, None)
        .unwrap();

    // Delete some vectors to trigger compaction
    store.delete(&[0, 2, 4, 6, 8]).unwrap();

    // Compact
    store.compact().unwrap();

    // Verify vectors are correct
    let status = store.status();
    assert_eq!(
        status.total_vectors, 5,
        "should have 5 vectors after compaction"
    );

    // Verify kernel segment is still present
    let bytes = read_file_bytes(&path);
    let kernel_segs = scan_segments_of_type(&bytes, SegmentType::Kernel as u8);
    assert!(
        !kernel_segs.is_empty(),
        "KERNEL_SEG should survive compaction"
    );

    // Verify the kernel can still be extracted
    let extracted = store.extract_kernel().unwrap();
    assert!(extracted.is_some(), "kernel should still be extractable");
    let (header_bytes, image_bytes) = extracted.unwrap();
    assert_eq!(header_bytes.len(), 128);
    assert_eq!(
        &image_bytes[..kernel_image.len()],
        kernel_image,
        "kernel image content should be preserved"
    );

    store.close().unwrap();

    println!("PASS: kernel_segment_survives_compaction");
}

// ===========================================================================
// TEST 2: ebpf_segment_survives_compaction
// ===========================================================================

/// Embed an eBPF program, compact, and verify it survives.
#[test]
fn ebpf_segment_survives_compaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ebpf_compact.rvf");
    let dim: u16 = 4;

    let bytecode = b"ebpf-bytecode-for-compaction-test-12345678";

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest and delete
    let vectors: Vec<Vec<f32>> = (0..6).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..6).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Embed eBPF
    store.embed_ebpf(0x01, 0x02, 128, bytecode, None).unwrap();

    // Delete and compact
    store.delete(&[0, 2, 4]).unwrap();
    store.compact().unwrap();

    // Verify eBPF is still present
    let bytes = read_file_bytes(&path);
    let ebpf_segs = scan_segments_of_type(&bytes, SegmentType::Ebpf as u8);
    assert!(!ebpf_segs.is_empty(), "EBPF_SEG should survive compaction");

    let extracted = store.extract_ebpf().unwrap();
    assert!(extracted.is_some(), "eBPF should still be extractable");
    let (header, payload) = extracted.unwrap();
    assert_eq!(header.len(), 64);
    assert_eq!(
        &payload[..bytecode.len()],
        bytecode,
        "eBPF bytecode should be preserved"
    );

    store.close().unwrap();

    println!("PASS: ebpf_segment_survives_compaction");
}

// ===========================================================================
// TEST 3: both_kernel_and_ebpf_survive_compaction
// ===========================================================================

/// Embed both kernel and eBPF segments, compact, and verify both survive.
#[test]
fn both_kernel_and_ebpf_survive_compaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("both_compact.rvf");
    let dim: u16 = 4;

    let kernel_image = b"kernel-data-for-dual-segment-test";
    let ebpf_bytecode = b"ebpf-code-for-dual-segment-test";

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    let vectors: Vec<Vec<f32>> = (0..8).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..8).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    store
        .embed_kernel(0x01, 0x00, 0x01, kernel_image, 9090, Some("quiet"))
        .unwrap();
    store
        .embed_ebpf(0x02, 0x01, 256, ebpf_bytecode, None)
        .unwrap();

    // Delete half the vectors and compact
    store.delete(&[0, 1, 2, 3]).unwrap();
    store.compact().unwrap();

    assert_eq!(store.status().total_vectors, 4);

    // Both should survive
    let bytes = read_file_bytes(&path);
    let kernel_segs = scan_segments_of_type(&bytes, SegmentType::Kernel as u8);
    let ebpf_segs = scan_segments_of_type(&bytes, SegmentType::Ebpf as u8);

    assert!(
        !kernel_segs.is_empty(),
        "KERNEL_SEG should survive compaction"
    );
    assert!(!ebpf_segs.is_empty(), "EBPF_SEG should survive compaction");

    assert!(store.extract_kernel().unwrap().is_some());
    assert!(store.extract_ebpf().unwrap().is_some());

    store.close().unwrap();

    println!("PASS: both_kernel_and_ebpf_survive_compaction");
}

// ===========================================================================
// TEST 4: unknown_segment_type_survives_compaction
// ===========================================================================

/// Manually append a segment with an unknown type code (simulating a future
/// format extension), compact, and verify it survives.
#[test]
fn unknown_segment_type_survives_compaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("unknown_seg.rvf");
    let dim: u16 = 4;

    let unknown_seg_type: u8 = 0x30; // Not defined in current SegmentType enum
    let unknown_payload = b"future-segment-payload-data-v2";

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let v = vec![1.0f32; dim as usize];
        store.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
        store.close().unwrap();
    }

    // Manually append an "unknown" segment
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let mut header = [0u8; SEGMENT_HEADER_SIZE];
        header[0..4].copy_from_slice(&SEGMENT_MAGIC.to_le_bytes());
        header[4] = 1; // version
        header[5] = unknown_seg_type;
        // flags at 6..8 stay zero
        header[0x08..0x10].copy_from_slice(&9999u64.to_le_bytes()); // seg_id
        header[0x10..0x18].copy_from_slice(&(unknown_payload.len() as u64).to_le_bytes());
        file.write_all(&header).unwrap();
        file.write_all(unknown_payload).unwrap();
        file.sync_all().unwrap();
    }

    // Verify the unknown segment is present
    let bytes_before = read_file_bytes(&path);
    let unknown_before = scan_segments_of_type(&bytes_before, unknown_seg_type);
    assert_eq!(
        unknown_before.len(),
        1,
        "should find 1 unknown segment before compaction"
    );

    // Compact
    {
        let mut store = RvfStore::open(&path).unwrap();
        store.compact().unwrap();
        store.close().unwrap();
    }

    // Verify the unknown segment survived
    let bytes_after = read_file_bytes(&path);
    let unknown_after = scan_segments_of_type(&bytes_after, unknown_seg_type);
    assert_eq!(
        unknown_after.len(),
        1,
        "unknown segment should survive compaction"
    );

    // Verify the payload is intact
    let (offset, _seg_id, payload_len) = unknown_after[0];
    let payload_start = offset + SEGMENT_HEADER_SIZE;
    let payload_end = payload_start + payload_len as usize;
    assert_eq!(
        &bytes_after[payload_start..payload_end],
        unknown_payload,
        "unknown segment payload should be preserved"
    );

    println!("PASS: unknown_segment_type_survives_compaction");
}

// ===========================================================================
// TEST 5: compaction_removes_dead_vectors_but_keeps_live
// ===========================================================================

/// Verify that compaction correctly removes deleted vectors while
/// keeping live ones, and that queries still return correct results.
#[test]
fn compaction_removes_dead_vectors_but_keeps_live() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact_live.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Ingest 10 vectors
    let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32, 0.0, 0.0, 0.0]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..10).collect();
    store.ingest_batch(&refs, &ids, None).unwrap();

    // Delete odd-indexed vectors
    store.delete(&[1, 3, 5, 7, 9]).unwrap();
    let pre_compact_size = store.status().file_size;

    // Compact
    store.compact().unwrap();
    let post_compact_size = store.status().file_size;

    // Verify compacted state
    assert_eq!(store.status().total_vectors, 5);

    // Query should only return even-indexed vectors
    let query = vec![0.0, 0.0, 0.0, 0.0];
    let results = store.query(&query, 10, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), 5);
    for r in &results {
        assert!(r.id % 2 == 0, "only even IDs should remain, got {}", r.id);
    }

    // File should be smaller (or at least not larger) after compaction
    // (may be larger due to segment overhead, but vector data should shrink)
    assert!(
        post_compact_size <= pre_compact_size + 256,
        "compacted file should not grow significantly: pre={pre_compact_size}, post={post_compact_size}"
    );

    store.close().unwrap();

    println!("PASS: compaction_removes_dead_vectors_but_keeps_live");
}

// ===========================================================================
// TEST 6: compacted_store_can_be_reopened
// ===========================================================================

/// After compaction, close and reopen the store to verify durability.
#[test]
fn compacted_store_can_be_reopened() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("compact_reopen.rvf");
    let dim: u16 = 4;

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32, 0.0, 0.0, 0.0]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..20).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();

        store.delete(&[0, 5, 10, 15]).unwrap();
        store.compact().unwrap();
        store.close().unwrap();
    }

    // Reopen
    {
        let store = RvfStore::open_readonly(&path).unwrap();
        assert_eq!(store.status().total_vectors, 16);

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
        assert_eq!(results.len(), 5);
        // Verify deleted vectors are not in results
        for r in &results {
            assert!(
                r.id != 0 && r.id != 5 && r.id != 10 && r.id != 15,
                "deleted vector {} should not appear",
                r.id
            );
        }
    }

    println!("PASS: compacted_store_can_be_reopened");
}
