//! Wire format interoperability end-to-end tests.
//!
//! Verifies that the wire format is correctly round-trippable between
//! rvf-wire (low-level segment I/O) and rvf-runtime (high-level store API).
//! Tests forward compatibility with unknown segment types, mixed compression
//! flags, and cross-layer interop.

use rvf_runtime::options::{DistanceMetric, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::{
    SegmentFlags, SegmentType, SEGMENT_ALIGNMENT, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC,
    SEGMENT_VERSION,
};
use rvf_wire::{
    find_latest_manifest, read_segment, read_segment_header, validate_segment, write_segment,
};
use std::fs;
use tempfile::TempDir;

// --------------------------------------------------------------------------
// 1. Create RVF file manually with rvf-wire, read with rvf-wire
// --------------------------------------------------------------------------
#[test]
fn interop_manual_wire_round_trip() {
    let mut file = Vec::new();
    let mut offsets = Vec::new();

    // Write 5 VEC_SEGs with different payloads.
    for i in 0..5u64 {
        let payload: Vec<u8> = (0..256)
            .map(|b| (i as u8).wrapping_mul(37).wrapping_add(b as u8))
            .collect();
        offsets.push(file.len());
        let seg = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), i);
        file.extend_from_slice(&seg);
    }

    // Write a manifest at the end.
    let manifest_payload = b"manifest data with segment directory";
    let manifest_offset = file.len();
    let manifest_seg = write_segment(
        SegmentType::Manifest as u8,
        manifest_payload,
        SegmentFlags::empty(),
        100,
    );
    file.extend_from_slice(&manifest_seg);

    // Read back each segment.
    for (i, &offset) in offsets.iter().enumerate() {
        let (header, payload) = read_segment(&file[offset..]).unwrap();
        assert_eq!(header.segment_id, i as u64);
        assert_eq!(header.seg_type, SegmentType::Vec as u8);
        assert_eq!(payload.len(), 256);
        validate_segment(&header, payload).unwrap();
    }

    // Find manifest via tail scan.
    let (found_offset, manifest_header) = find_latest_manifest(&file).unwrap();
    assert_eq!(found_offset, manifest_offset);
    assert_eq!(manifest_header.segment_id, 100);
    assert_eq!(manifest_header.seg_type, SegmentType::Manifest as u8);
}

// --------------------------------------------------------------------------
// 2. Verify all segment headers, hashes, alignment
// --------------------------------------------------------------------------
#[test]
fn interop_all_segments_valid_headers_hashes_alignment() {
    let segment_types = [
        (SegmentType::Vec as u8, "VEC"),
        (SegmentType::Index as u8, "INDEX"),
        (SegmentType::Quant as u8, "QUANT"),
        (SegmentType::Journal as u8, "JOURNAL"),
        (SegmentType::Manifest as u8, "MANIFEST"),
        (SegmentType::Meta as u8, "META"),
        (SegmentType::Hot as u8, "HOT"),
    ];

    let mut file = Vec::new();
    let mut offsets = Vec::new();

    for (i, (seg_type, _name)) in segment_types.iter().enumerate() {
        let payload_size = 50 + i * 31; // Various non-aligned sizes.
        let payload: Vec<u8> = (0..payload_size).map(|b| (b * 7 + i) as u8).collect();

        offsets.push(file.len());
        let seg = write_segment(*seg_type, &payload, SegmentFlags::empty(), i as u64);

        // Each segment must be 64-byte aligned.
        assert_eq!(
            seg.len() % SEGMENT_ALIGNMENT,
            0,
            "segment type {} not 64-byte aligned",
            _name
        );
        file.extend_from_slice(&seg);
    }

    // Read and validate all.
    for (i, &offset) in offsets.iter().enumerate() {
        let (header, payload) = read_segment(&file[offset..]).unwrap();

        // Header checks.
        assert_eq!(header.magic, SEGMENT_MAGIC, "segment {i}: bad magic");
        assert_eq!(header.version, SEGMENT_VERSION, "segment {i}: bad version");
        assert_eq!(
            header.seg_type, segment_types[i].0,
            "segment {i}: wrong type"
        );
        assert_eq!(header.segment_id, i as u64, "segment {i}: wrong ID");

        // Hash check.
        validate_segment(&header, payload)
            .unwrap_or_else(|e| panic!("segment {i} ({}): hash failed: {e:?}", segment_types[i].1));

        // Offset alignment check.
        assert_eq!(
            offset % SEGMENT_ALIGNMENT,
            0,
            "segment {i} starts at non-aligned offset {offset}"
        );
    }
}

// --------------------------------------------------------------------------
// 3. Forward compatibility: unknown segment type is safely skipped
// --------------------------------------------------------------------------
#[test]
fn interop_unknown_segment_type_skipped() {
    let mut file = Vec::new();

    // Known VEC_SEG.
    let vec_offset = file.len();
    let vec_payload = b"known vector data";
    file.extend_from_slice(&write_segment(
        SegmentType::Vec as u8,
        vec_payload,
        SegmentFlags::empty(),
        1,
    ));

    // Unknown future segment type (0xFE).
    let _unknown_offset = file.len();
    file.extend_from_slice(&write_segment(
        0xFE,
        b"hypothetical v2 extension data",
        SegmentFlags::empty(),
        2,
    ));

    // Another unknown type (0xFD).
    file.extend_from_slice(&write_segment(
        0xFD,
        b"another future extension",
        SegmentFlags::empty(),
        3,
    ));

    // Known MANIFEST_SEG.
    let manifest_offset = file.len();
    file.extend_from_slice(&write_segment(
        SegmentType::Manifest as u8,
        b"manifest payload",
        SegmentFlags::empty(),
        10,
    ));

    // The reader can still read and validate the unknown segments structurally.
    let (unknown_hdr, unknown_pay) = read_segment(&file[_unknown_offset..]).unwrap();
    assert_eq!(unknown_hdr.seg_type, 0xFE);
    validate_segment(&unknown_hdr, unknown_pay).unwrap();

    // The known segments are still accessible.
    let (vec_hdr, vec_pay) = read_segment(&file[vec_offset..]).unwrap();
    assert_eq!(vec_hdr.seg_type, SegmentType::Vec as u8);
    assert_eq!(vec_pay, vec_payload);

    // Manifest is still findable.
    let (found_offset, mani_hdr) = find_latest_manifest(&file).unwrap();
    assert_eq!(found_offset, manifest_offset);
    assert_eq!(mani_hdr.segment_id, 10);
}

// --------------------------------------------------------------------------
// 4. Mixed compression flags: some compressed, some not
// --------------------------------------------------------------------------
#[test]
fn interop_mixed_compression_flags() {
    let payloads: Vec<(&[u8], SegmentFlags)> = vec![
        (b"uncompressed data", SegmentFlags::empty()),
        (
            b"compressed data marker",
            SegmentFlags::empty().with(SegmentFlags::COMPRESSED),
        ),
        (b"plain data", SegmentFlags::empty()),
        (
            b"sealed compressed",
            SegmentFlags::empty()
                .with(SegmentFlags::COMPRESSED)
                .with(SegmentFlags::SEALED),
        ),
        (b"hot data", SegmentFlags::empty().with(SegmentFlags::HOT)),
    ];

    let mut file = Vec::new();
    let mut offsets = Vec::new();

    for (i, (payload, flags)) in payloads.iter().enumerate() {
        offsets.push(file.len());
        let seg = write_segment(SegmentType::Vec as u8, payload, *flags, i as u64);
        file.extend_from_slice(&seg);
    }

    // Read all segments back and verify flags are preserved.
    for (i, &offset) in offsets.iter().enumerate() {
        let (header, payload) = read_segment(&file[offset..]).unwrap();
        let expected_flags = payloads[i].1;

        if expected_flags.contains(SegmentFlags::COMPRESSED) {
            assert!(
                header.flags & SegmentFlags::COMPRESSED != 0,
                "segment {i}: COMPRESSED flag should be set"
            );
        }
        if expected_flags.contains(SegmentFlags::SEALED) {
            assert!(
                header.flags & SegmentFlags::SEALED != 0,
                "segment {i}: SEALED flag should be set"
            );
        }
        if expected_flags.contains(SegmentFlags::HOT) {
            assert!(
                header.flags & SegmentFlags::HOT != 0,
                "segment {i}: HOT flag should be set"
            );
        }

        // Payload data is still readable regardless of flags.
        assert_eq!(payload, payloads[i].0);
        validate_segment(&header, payload).unwrap();
    }
}

// --------------------------------------------------------------------------
// 5. Create file with runtime, verify structure with rvf-wire
// --------------------------------------------------------------------------
#[test]
fn interop_runtime_write_wire_read() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("rt_to_wire.rvf");
    let dim: u16 = 4;

    // Create using rvf-runtime.
    {
        let mut store = RvfStore::create(
            &path,
            RvfOptions {
                dimension: dim,
                metric: DistanceMetric::L2,
                ..Default::default()
            },
        )
        .unwrap();

        let v1 = vec![1.0f32, 2.0, 3.0, 4.0];
        let v2 = vec![5.0f32, 6.0, 7.0, 8.0];
        store
            .ingest_batch(&[v1.as_slice(), v2.as_slice()], &[10, 20], None)
            .unwrap();
        store.close().unwrap();
    }

    // Read the raw file and verify structure with rvf-wire.
    let file_bytes = fs::read(&path).unwrap();

    // The file should contain valid segments.
    assert!(
        file_bytes.len() >= SEGMENT_HEADER_SIZE,
        "file should contain at least one segment header"
    );

    // Scan for segments by walking byte-by-byte looking for RVFS magic.
    // The runtime's SegmentWriter uses its own layout (header + payload,
    // not necessarily 64-byte padded), so we scan for magic + version.
    let mut segments_found = 0u32;
    let mut manifest_found = false;
    let mut vec_seg_found = false;

    let mut offset = 0;
    while offset + SEGMENT_HEADER_SIZE <= file_bytes.len() {
        // Check for RVFS magic at this offset.
        let magic = u32::from_le_bytes([
            file_bytes[offset],
            file_bytes[offset + 1],
            file_bytes[offset + 2],
            file_bytes[offset + 3],
        ]);
        let version = file_bytes[offset + 4];

        if magic == SEGMENT_MAGIC && version == SEGMENT_VERSION {
            if let Ok(header) = read_segment_header(&file_bytes[offset..]) {
                segments_found += 1;
                match header.seg_type {
                    t if t == SegmentType::Vec as u8 => vec_seg_found = true,
                    t if t == SegmentType::Manifest as u8 => manifest_found = true,
                    _ => {}
                }
                // Move past header + payload.
                let seg_size = SEGMENT_HEADER_SIZE + header.payload_length as usize;
                offset += seg_size.max(1);
                continue;
            }
        }
        offset += 1;
    }

    assert!(vec_seg_found, "should find at least one VEC_SEG");
    assert!(manifest_found, "should find at least one MANIFEST_SEG");
    assert!(
        segments_found >= 2,
        "should find at least 2 segments (got {segments_found})"
    );
}

// --------------------------------------------------------------------------
// 6. All flag combinations preserved through round-trip
// --------------------------------------------------------------------------
#[test]
fn interop_flag_combinations_round_trip() {
    let flag_combos: Vec<SegmentFlags> = vec![
        SegmentFlags::empty(),
        SegmentFlags::empty().with(SegmentFlags::COMPRESSED),
        SegmentFlags::empty().with(SegmentFlags::ENCRYPTED),
        SegmentFlags::empty().with(SegmentFlags::SIGNED),
        SegmentFlags::empty().with(SegmentFlags::SEALED),
        SegmentFlags::empty().with(SegmentFlags::PARTIAL),
        SegmentFlags::empty().with(SegmentFlags::TOMBSTONE),
        SegmentFlags::empty().with(SegmentFlags::HOT),
        SegmentFlags::empty().with(SegmentFlags::OVERLAY),
        SegmentFlags::empty().with(SegmentFlags::SNAPSHOT),
        SegmentFlags::empty().with(SegmentFlags::CHECKPOINT),
        // Combined flags.
        SegmentFlags::empty()
            .with(SegmentFlags::COMPRESSED)
            .with(SegmentFlags::SEALED)
            .with(SegmentFlags::HOT),
        SegmentFlags::empty()
            .with(SegmentFlags::ENCRYPTED)
            .with(SegmentFlags::SIGNED)
            .with(SegmentFlags::CHECKPOINT),
    ];

    for (i, flags) in flag_combos.iter().enumerate() {
        let payload = format!("payload for flag combo {i}");
        let encoded = write_segment(SegmentType::Vec as u8, payload.as_bytes(), *flags, i as u64);
        let (header, decoded_payload) = read_segment(&encoded).unwrap();

        assert_eq!(
            SegmentFlags::from_raw(header.flags).bits(),
            flags.bits(),
            "flag combo {i}: flags not preserved"
        );
        assert_eq!(decoded_payload, payload.as_bytes());
        validate_segment(&header, decoded_payload).unwrap();
    }
}

// --------------------------------------------------------------------------
// 7. Large payload round-trip preserves all bytes
// --------------------------------------------------------------------------
#[test]
fn interop_large_payload_byte_exact() {
    // 100KB payload.
    let size = 100_000;
    let payload: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let encoded = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), 42);

    let (header, decoded) = read_segment(&encoded).unwrap();
    assert_eq!(header.payload_length, size as u64);
    assert_eq!(decoded.len(), size);
    assert_eq!(
        decoded,
        &payload[..],
        "large payload should be byte-identical"
    );
    validate_segment(&header, decoded).unwrap();

    // Verify 64-byte alignment.
    assert_eq!(encoded.len() % SEGMENT_ALIGNMENT, 0);
}
