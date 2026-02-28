//! Round-trip tests: write + read all segment types via rvf-wire,
//! verifying data integrity across the full encode/decode pipeline.

use rvf_types::{SegmentFlags, SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC, SEGMENT_VERSION};
use rvf_wire::{read_segment, validate_segment, write_segment};

/// Helper: all segment types that exist in the spec.
fn all_segment_types() -> Vec<(u8, &'static str)> {
    vec![
        (SegmentType::Vec as u8, "VEC_SEG"),
        (SegmentType::Index as u8, "INDEX_SEG"),
        (SegmentType::Quant as u8, "QUANT_SEG"),
        (SegmentType::Journal as u8, "JOURNAL_SEG"),
        (SegmentType::Manifest as u8, "MANIFEST_SEG"),
        (SegmentType::Meta as u8, "META_SEG"),
        (SegmentType::Hot as u8, "HOT_SEG"),
    ]
}

#[test]
fn round_trip_all_segment_types() {
    for (seg_type, name) in all_segment_types() {
        let payload = format!("payload for {name}");
        let encoded = write_segment(seg_type, payload.as_bytes(), SegmentFlags::empty(), 42);

        let (header, decoded_payload) =
            read_segment(&encoded).unwrap_or_else(|e| panic!("failed to read {name}: {e:?}"));

        assert_eq!(header.magic, SEGMENT_MAGIC, "{name}: bad magic");
        assert_eq!(header.version, SEGMENT_VERSION, "{name}: bad version");
        assert_eq!(header.seg_type, seg_type, "{name}: bad seg_type");
        assert_eq!(header.segment_id, 42, "{name}: bad segment_id");
        assert_eq!(
            decoded_payload,
            payload.as_bytes(),
            "{name}: payload mismatch"
        );
    }
}

#[test]
fn round_trip_validates_content_hash() {
    for (seg_type, name) in all_segment_types() {
        let payload: Vec<u8> = (0..256).map(|i| (i & 0xFF) as u8).collect();
        let encoded = write_segment(seg_type, &payload, SegmentFlags::empty(), 1);
        let (header, decoded_payload) = read_segment(&encoded).unwrap();

        validate_segment(&header, decoded_payload)
            .unwrap_or_else(|e| panic!("{name}: hash validation failed: {e:?}"));
    }
}

#[test]
fn round_trip_preserves_flags() {
    let flags = SegmentFlags::empty()
        .with(SegmentFlags::COMPRESSED)
        .with(SegmentFlags::SEALED);
    let encoded = write_segment(SegmentType::Vec as u8, b"flagged", flags, 99);
    let (header, _) = read_segment(&encoded).unwrap();

    assert!(header.flags & SegmentFlags::COMPRESSED != 0);
    assert!(header.flags & SegmentFlags::SEALED != 0);
}

#[test]
fn round_trip_empty_payload() {
    let encoded = write_segment(SegmentType::Meta as u8, &[], SegmentFlags::empty(), 0);
    let (header, payload) = read_segment(&encoded).unwrap();

    assert_eq!(header.payload_length, 0);
    assert!(payload.is_empty());
    assert_eq!(encoded.len(), SEGMENT_HEADER_SIZE); // 64 bytes, no padding needed
}

#[test]
fn round_trip_large_payload() {
    let payload: Vec<u8> = (0..10000).map(|i| (i % 251) as u8).collect();
    let encoded = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), 7);
    let (header, decoded_payload) = read_segment(&encoded).unwrap();

    assert_eq!(header.payload_length, 10000);
    assert_eq!(decoded_payload, &payload[..]);
    validate_segment(&header, decoded_payload).unwrap();
}

#[test]
fn output_is_64_byte_aligned() {
    for size in [0, 1, 10, 63, 64, 65, 100, 127, 128, 129, 255, 256, 1000] {
        let payload = vec![0xABu8; size];
        let encoded = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), 0);
        assert_eq!(
            encoded.len() % 64,
            0,
            "not 64-byte aligned for payload size {size}"
        );
    }
}

#[test]
fn multi_segment_file() {
    // Build a file with multiple segments back-to-back.
    let mut file = Vec::new();
    let mut offsets = Vec::new();

    for i in 0..5 {
        let payload = format!("segment {i} data");
        offsets.push(file.len());
        let seg = write_segment(
            SegmentType::Vec as u8,
            payload.as_bytes(),
            SegmentFlags::empty(),
            i,
        );
        file.extend_from_slice(&seg);
    }

    // Read each segment back.
    for (i, &offset) in offsets.iter().enumerate() {
        let (header, payload) = read_segment(&file[offset..]).unwrap();
        assert_eq!(header.segment_id, i as u64);
        let expected = format!("segment {i} data");
        assert_eq!(payload, expected.as_bytes());
    }
}
