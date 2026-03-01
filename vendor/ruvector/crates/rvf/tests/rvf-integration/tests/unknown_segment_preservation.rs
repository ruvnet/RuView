//! Unknown segment type preservation during compaction.
//!
//! Forward-compatibility guarantee: older RVF tools MUST NOT silently
//! discard segment types they do not recognize. This test verifies that
//! unknown segment types (e.g., a future KERNEL_SEG 0x0E or EBPF_SEG 0x0F)
//! survive a compact/rewrite cycle byte-for-byte.
//!
//! If this test fails, it means the compaction implementation only rewrites
//! known segment types and drops everything else -- a valid finding that
//! should be fixed before shipping a format version bump.

use rvf_runtime::options::{DistanceMetric, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use tempfile::TempDir;

/// The RVF segment header magic: "RVFS" as a little-endian u32.
const SEGMENT_MAGIC: u32 = 0x5256_4653;

/// Size of the 64-byte segment header.
const SEGMENT_HEADER_SIZE: usize = 64;

/// A hypothetical future segment type not yet defined in SegmentType.
const UNKNOWN_SEG_TYPE_KERNEL: u8 = 0x0E;

/// Another hypothetical future segment type (vendor extension range).
const UNKNOWN_SEG_TYPE_VENDOR: u8 = 0xFE;

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

/// Build a raw 64-byte segment header for an unknown segment type.
fn build_raw_segment_header(
    seg_type: u8,
    seg_id: u64,
    payload_len: u64,
) -> [u8; SEGMENT_HEADER_SIZE] {
    let mut buf = [0u8; SEGMENT_HEADER_SIZE];
    // magic (offset 0x00): RVFS
    buf[0x00..0x04].copy_from_slice(&SEGMENT_MAGIC.to_le_bytes());
    // version (offset 0x04): 1
    buf[0x04] = 1;
    // seg_type (offset 0x05)
    buf[0x05] = seg_type;
    // flags (offset 0x06): 0
    // segment_id (offset 0x08)
    buf[0x08..0x10].copy_from_slice(&seg_id.to_le_bytes());
    // payload_length (offset 0x10)
    buf[0x10..0x18].copy_from_slice(&payload_len.to_le_bytes());
    // remaining fields stay zeroed (timestamp, checksum, compression, etc.)
    buf
}

/// Scan a file for all segment headers and return (offset, seg_type, seg_id, payload_len)
/// for each segment found.
fn scan_segments(file_bytes: &[u8]) -> Vec<(usize, u8, u64, u64)> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut segments = Vec::new();

    if file_bytes.len() < SEGMENT_HEADER_SIZE {
        return segments;
    }

    let last_possible = file_bytes.len() - SEGMENT_HEADER_SIZE;
    for i in 0..=last_possible {
        if file_bytes[i..i + 4] == magic_bytes {
            let seg_type = file_bytes[i + 5];
            let seg_id = u64::from_le_bytes([
                file_bytes[i + 0x08],
                file_bytes[i + 0x09],
                file_bytes[i + 0x0A],
                file_bytes[i + 0x0B],
                file_bytes[i + 0x0C],
                file_bytes[i + 0x0D],
                file_bytes[i + 0x0E],
                file_bytes[i + 0x0F],
            ]);
            let payload_len = u64::from_le_bytes([
                file_bytes[i + 0x10],
                file_bytes[i + 0x11],
                file_bytes[i + 0x12],
                file_bytes[i + 0x13],
                file_bytes[i + 0x14],
                file_bytes[i + 0x15],
                file_bytes[i + 0x16],
                file_bytes[i + 0x17],
            ]);
            segments.push((i, seg_type, seg_id, payload_len));
        }
    }

    segments
}

/// Read entire file into a byte vector.
fn read_file_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut file = OpenOptions::new().read(true).open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

/// Extract the full segment bytes (header + payload) for a segment at a given
/// offset, given the file content.
fn extract_segment_bytes(file_bytes: &[u8], offset: usize, payload_len: u64) -> &[u8] {
    let end = offset + SEGMENT_HEADER_SIZE + payload_len as usize;
    &file_bytes[offset..end]
}

// --------------------------------------------------------------------------
// 1. Unknown segment is preserved after compaction (KERNEL_SEG 0x0E)
// --------------------------------------------------------------------------
//
// NOTE: The current compaction implementation in store.rs rewrites the file
// by creating a temp file containing only the live VEC_SEGs and a new
// manifest. It does NOT preserve unknown/unrecognized segment types.
// Therefore this test documents the EXPECTED behavior (unknown segments
// should be preserved) but is anticipated to FAIL against the current
// implementation. This is a known gap -- not a bug in the test.
#[test]
fn unknown_segment_preserved_after_compaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("unknown_seg.rvf");
    let dim: u16 = 4;

    // --- Step 1: Create a store and ingest some vectors -----------------------
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=20).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // --- Step 2: Manually append an unknown segment (KERNEL_SEG 0x0E) ---------
    // The payload is arbitrary opaque data -- perhaps a future eBPF bytecode
    // blob or kernel routing table. We use a recognizable pattern so we can
    // verify byte-for-byte preservation.
    let unknown_payload: Vec<u8> = (0..128u8).collect(); // 128 bytes of 0x00..0x7F
    let unknown_seg_id: u64 = 9999;
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let header = build_raw_segment_header(
            UNKNOWN_SEG_TYPE_KERNEL,
            unknown_seg_id,
            unknown_payload.len() as u64,
        );
        file.write_all(&header).unwrap();
        file.write_all(&unknown_payload).unwrap();
        file.sync_all().unwrap();
    }

    // --- Step 3: Verify the unknown segment is present in the file ------------
    let bytes_before = read_file_bytes(&path);
    let segments_before = scan_segments(&bytes_before);

    let unknown_before: Vec<_> = segments_before
        .iter()
        .filter(|&&(_, seg_type, _, _)| seg_type == UNKNOWN_SEG_TYPE_KERNEL)
        .collect();

    assert_eq!(
        unknown_before.len(),
        1,
        "expected exactly 1 unknown segment (type 0x{:02X}) before compaction, found {}",
        UNKNOWN_SEG_TYPE_KERNEL,
        unknown_before.len()
    );

    let &(off_before, _, sid_before, plen_before) = unknown_before[0];
    assert_eq!(sid_before, unknown_seg_id);
    assert_eq!(plen_before, unknown_payload.len() as u64);

    // Save the full segment bytes for later comparison.
    let seg_bytes_before = extract_segment_bytes(&bytes_before, off_before, plen_before).to_vec();
    println!(
        "Before compaction: unknown segment at offset {}, {} total bytes (header+payload)",
        off_before,
        seg_bytes_before.len()
    );

    // --- Step 4: Delete some vectors and compact ------------------------------
    {
        let mut store = RvfStore::open(&path).unwrap();

        // Delete a few vectors to give compaction something to do.
        let del_ids: Vec<u64> = (1..=5).collect();
        store.delete(&del_ids).unwrap();

        let compact_result = store.compact().unwrap();
        println!(
            "Compaction: segments_compacted={}, bytes_reclaimed={}",
            compact_result.segments_compacted, compact_result.bytes_reclaimed
        );
        store.close().unwrap();
    }

    // --- Step 5: Verify the unknown segment still exists after compaction -----
    let bytes_after = read_file_bytes(&path);
    let segments_after = scan_segments(&bytes_after);

    println!(
        "After compaction: {} total segments found in file scan",
        segments_after.len()
    );
    for &(off, stype, sid, plen) in &segments_after {
        println!(
            "  offset={}, type=0x{:02X}, seg_id={}, payload_len={}",
            off, stype, sid, plen
        );
    }

    let unknown_after: Vec<_> = segments_after
        .iter()
        .filter(|&&(_, seg_type, _, _)| seg_type == UNKNOWN_SEG_TYPE_KERNEL)
        .collect();

    // CRITICAL ASSERTION: The unknown segment must survive compaction.
    // If this fails, the compaction implementation is dropping segments it
    // does not understand, which breaks forward compatibility.
    assert_eq!(
        unknown_after.len(),
        1,
        "FORWARD COMPATIBILITY VIOLATION: unknown segment type 0x{:02X} was dropped \
         during compaction. Older tools must preserve segment types they do not recognize. \
         Found {} unknown segments after compaction (expected 1).",
        UNKNOWN_SEG_TYPE_KERNEL,
        unknown_after.len()
    );

    // Verify byte-for-byte preservation of the segment (header + payload).
    let &(off_after, _, _, plen_after) = unknown_after[0];
    let seg_bytes_after = extract_segment_bytes(&bytes_after, off_after, plen_after).to_vec();

    assert_eq!(
        seg_bytes_before,
        seg_bytes_after,
        "Unknown segment was NOT preserved byte-for-byte. \
         Before: {} bytes at offset {}, After: {} bytes at offset {}",
        seg_bytes_before.len(),
        off_before,
        seg_bytes_after.len(),
        off_after
    );

    println!(
        "PASS: unknown segment type 0x{:02X} preserved byte-for-byte after compaction",
        UNKNOWN_SEG_TYPE_KERNEL
    );
}

// --------------------------------------------------------------------------
// 2. Multiple unknown segment types are all preserved
// --------------------------------------------------------------------------
//
// Same forward-compatibility concern as above: if compaction drops one
// unknown type it probably drops all of them.
#[test]
fn multiple_unknown_segment_types_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_unknown.rvf");
    let dim: u16 = 4;

    // Create store with some vectors.
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=10).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Append two different unknown segment types.
    let kernel_payload: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF]; // 4 bytes
    let vendor_payload: Vec<u8> = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0xFF]; // 6 bytes
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();

        // KERNEL_SEG 0x0E
        let h1 =
            build_raw_segment_header(UNKNOWN_SEG_TYPE_KERNEL, 8001, kernel_payload.len() as u64);
        file.write_all(&h1).unwrap();
        file.write_all(&kernel_payload).unwrap();

        // VENDOR_SEG 0xFE
        let h2 =
            build_raw_segment_header(UNKNOWN_SEG_TYPE_VENDOR, 8002, vendor_payload.len() as u64);
        file.write_all(&h2).unwrap();
        file.write_all(&vendor_payload).unwrap();

        file.sync_all().unwrap();
    }

    // Verify both are present before compaction.
    let bytes_before = read_file_bytes(&path);
    let segs_before = scan_segments(&bytes_before);

    let kernel_before = segs_before
        .iter()
        .filter(|s| s.1 == UNKNOWN_SEG_TYPE_KERNEL)
        .count();
    let vendor_before = segs_before
        .iter()
        .filter(|s| s.1 == UNKNOWN_SEG_TYPE_VENDOR)
        .count();
    assert_eq!(
        kernel_before, 1,
        "KERNEL_SEG should exist before compaction"
    );
    assert_eq!(
        vendor_before, 1,
        "VENDOR_SEG should exist before compaction"
    );

    // Compact.
    {
        let mut store = RvfStore::open(&path).unwrap();
        store.delete(&[1, 2]).unwrap();
        store.compact().unwrap();
        store.close().unwrap();
    }

    // Verify both unknown types survived.
    let bytes_after = read_file_bytes(&path);
    let segs_after = scan_segments(&bytes_after);

    let kernel_after = segs_after
        .iter()
        .filter(|s| s.1 == UNKNOWN_SEG_TYPE_KERNEL)
        .count();
    let vendor_after = segs_after
        .iter()
        .filter(|s| s.1 == UNKNOWN_SEG_TYPE_VENDOR)
        .count();

    println!(
        "After compaction: KERNEL_SEG(0x0E) count={}, VENDOR_SEG(0xFE) count={}",
        kernel_after, vendor_after
    );

    assert_eq!(
        kernel_after, 1,
        "FORWARD COMPATIBILITY VIOLATION: KERNEL_SEG (0x{:02X}) was dropped during compaction",
        UNKNOWN_SEG_TYPE_KERNEL
    );
    assert_eq!(
        vendor_after, 1,
        "FORWARD COMPATIBILITY VIOLATION: VENDOR_SEG (0x{:02X}) was dropped during compaction",
        UNKNOWN_SEG_TYPE_VENDOR
    );
}

// --------------------------------------------------------------------------
// 3. Unknown segment does not break store open/query (read tolerance)
// --------------------------------------------------------------------------
//
// Even if compaction does not preserve unknown segments, the store should
// at least be able to OPEN and QUERY a file that contains them, without
// panicking or returning errors.
#[test]
fn unknown_segment_does_not_break_read_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("read_tolerance.rvf");
    let dim: u16 = 4;

    // Create and populate.
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=10).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Append an unknown segment type before the final manifest so the file
    // has: [manifest] [vec_seg] [manifest] [UNKNOWN] at the tail.
    // The manifest scanner should skip past it.
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let payload = vec![0xABu8; 64];
        let header = build_raw_segment_header(0x0F, 7777, payload.len() as u64);
        file.write_all(&header).unwrap();
        file.write_all(&payload).unwrap();
        file.sync_all().unwrap();
    }

    // Re-open the store. The manifest scan reads from the tail and should
    // skip the unknown segment header (it checks for manifest type 0x05).
    // This should NOT panic or error.
    let store = RvfStore::open_readonly(&path).unwrap();
    let status = store.status();
    assert_eq!(
        status.total_vectors, 10,
        "store should still report 10 vectors even with unknown segment appended"
    );

    // Query should still work.
    let query = vec![5.0f32; dim as usize];
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();
    assert!(
        !results.is_empty(),
        "query should return results despite unknown segment in file"
    );
    assert_eq!(
        results[0].id, 6,
        "closest vector to [5,5,5,5] should be id=6 (value [5,5,5,5])"
    );

    println!("PASS: store opens and queries correctly with unknown segment type 0x0F in file");
}
