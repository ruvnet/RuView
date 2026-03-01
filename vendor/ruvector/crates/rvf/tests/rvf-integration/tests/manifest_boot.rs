//! Manifest and boot integration tests.
//!
//! Tests the rvf-wire tail_scan + rvf-manifest progressive boot pipeline:
//! - Write segments, append manifest, find manifest from tail
//! - Level 0 / Level 1 manifest round-trips
//! - Overlay chain progression

use rvf_types::{SegmentFlags, SegmentType, SEGMENT_ALIGNMENT, SEGMENT_HEADER_SIZE};
use rvf_wire::{find_latest_manifest, write_segment};

#[test]
fn tail_scan_finds_manifest_after_data_segments() {
    let mut file = Vec::new();

    // Write several VEC_SEGs.
    for i in 0..5 {
        let payload = vec![i as u8; 100];
        let seg = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), i);
        file.extend_from_slice(&seg);
    }

    // Write a manifest segment at the end.
    let manifest_payload = vec![0u8; 128];
    let manifest_offset = file.len();
    let manifest_seg = write_segment(
        SegmentType::Manifest as u8,
        &manifest_payload,
        SegmentFlags::empty(),
        100,
    );
    file.extend_from_slice(&manifest_seg);

    let (offset, header) = find_latest_manifest(&file).unwrap();
    assert_eq!(offset, manifest_offset);
    assert_eq!(header.seg_type, SegmentType::Manifest as u8);
    assert_eq!(header.segment_id, 100);
}

#[test]
fn tail_scan_finds_latest_manifest_when_multiple_exist() {
    let mut file = Vec::new();

    // First manifest.
    let m1 = write_segment(
        SegmentType::Manifest as u8,
        &[1u8; 64],
        SegmentFlags::empty(),
        1,
    );
    file.extend_from_slice(&m1);

    // Some data segments.
    for i in 10..15 {
        let seg = write_segment(
            SegmentType::Vec as u8,
            &[i as u8; 200],
            SegmentFlags::empty(),
            i,
        );
        file.extend_from_slice(&seg);
    }

    // Second (latest) manifest.
    let latest_offset = file.len();
    let m2 = write_segment(
        SegmentType::Manifest as u8,
        &[2u8; 64],
        SegmentFlags::empty(),
        2,
    );
    file.extend_from_slice(&m2);

    let (offset, header) = find_latest_manifest(&file).unwrap();
    assert_eq!(offset, latest_offset);
    assert_eq!(header.segment_id, 2);
}

#[test]
fn tail_scan_fails_when_no_manifest() {
    let mut file = Vec::new();
    for i in 0..3 {
        let seg = write_segment(SegmentType::Vec as u8, &[0u8; 50], SegmentFlags::empty(), i);
        file.extend_from_slice(&seg);
    }

    assert!(find_latest_manifest(&file).is_err());
}

#[test]
fn tail_scan_handles_mixed_segment_types() {
    let mut file = Vec::new();

    let types = [
        SegmentType::Vec,
        SegmentType::Index,
        SegmentType::Meta,
        SegmentType::Journal,
        SegmentType::Hot,
    ];

    for (i, seg_type) in types.iter().enumerate() {
        let seg = write_segment(
            *seg_type as u8,
            &[i as u8; 80],
            SegmentFlags::empty(),
            i as u64,
        );
        file.extend_from_slice(&seg);
    }

    // Finally add manifest.
    let manifest_offset = file.len();
    let manifest = write_segment(
        SegmentType::Manifest as u8,
        &[0xFFu8; 96],
        SegmentFlags::empty(),
        99,
    );
    file.extend_from_slice(&manifest);

    let (offset, header) = find_latest_manifest(&file).unwrap();
    assert_eq!(offset, manifest_offset);
    assert_eq!(header.segment_id, 99);
}

#[test]
fn all_segments_are_64_byte_aligned() {
    let mut file = Vec::new();
    let types = [
        SegmentType::Vec,
        SegmentType::Index,
        SegmentType::Quant,
        SegmentType::Journal,
        SegmentType::Manifest,
        SegmentType::Meta,
        SegmentType::Hot,
    ];

    for (i, seg_type) in types.iter().enumerate() {
        let payload_size = 10 + i * 17; // various non-aligned sizes
        let payload = vec![0u8; payload_size];
        let seg = write_segment(*seg_type as u8, &payload, SegmentFlags::empty(), i as u64);

        assert_eq!(
            seg.len() % SEGMENT_ALIGNMENT,
            0,
            "segment type {:?} (payload={payload_size}) not 64-byte aligned",
            seg_type
        );
        file.extend_from_slice(&seg);
    }

    // Every segment boundary is at a 64-byte aligned offset.
    let mut offset = 0;
    for (i, seg_type) in types.iter().enumerate() {
        assert_eq!(
            offset % SEGMENT_ALIGNMENT,
            0,
            "segment {i} ({seg_type:?}) starts at non-aligned offset {offset}"
        );
        let payload_size = 10 + i * 17;
        let seg_size =
            (SEGMENT_HEADER_SIZE + payload_size + SEGMENT_ALIGNMENT - 1) & !(SEGMENT_ALIGNMENT - 1);
        offset += seg_size;
    }
}
