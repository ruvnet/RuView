//! Profile compatibility tests.
//!
//! Verifies that a generic RVF reader can open files written with different
//! profiles, and that unknown segment types are gracefully skipped.

use rvf_types::{SegmentFlags, SegmentType};
use rvf_wire::{read_segment, validate_segment, write_segment};

#[test]
fn generic_reader_handles_unknown_segment_type() {
    // Write a segment with a hypothetical future segment type (0xFE).
    let future_type: u8 = 0xFE;
    let payload = b"future segment data";
    let encoded = write_segment(future_type, payload, SegmentFlags::empty(), 1);

    // The reader should still parse the header and payload.
    let (header, decoded_payload) = read_segment(&encoded).unwrap();
    assert_eq!(header.seg_type, future_type);
    assert_eq!(decoded_payload, payload);

    // Hash validation should still work.
    assert!(validate_segment(&header, decoded_payload).is_ok());
}

#[test]
fn multi_profile_file_readable() {
    // Simulate a file with segments tagged with different profile hints.
    // The generic reader should read all of them without caring about profile.
    let mut file = Vec::new();
    let mut offsets = Vec::new();

    // "RVText" segment (just VEC_SEG with text embedding payload).
    let text_payload = b"text embedding vectors";
    offsets.push(file.len());
    file.extend_from_slice(&write_segment(
        SegmentType::Vec as u8,
        text_payload,
        SegmentFlags::empty(),
        1,
    ));

    // "RVDNA" segment (VEC_SEG with genomic data payload).
    let dna_payload = b"genomic sequence vectors";
    offsets.push(file.len());
    file.extend_from_slice(&write_segment(
        SegmentType::Vec as u8,
        dna_payload,
        SegmentFlags::empty(),
        2,
    ));

    // "RVGraph" segment (VEC_SEG with graph embedding payload).
    let graph_payload = b"graph node embedding vectors";
    offsets.push(file.len());
    file.extend_from_slice(&write_segment(
        SegmentType::Vec as u8,
        graph_payload,
        SegmentFlags::empty(),
        3,
    ));

    // Generic reader can read all segments.
    let expected_payloads: Vec<&[u8]> = vec![text_payload, dna_payload, graph_payload];
    for (i, &offset) in offsets.iter().enumerate() {
        let (header, payload) = read_segment(&file[offset..]).unwrap();
        assert_eq!(header.segment_id, (i + 1) as u64);
        assert_eq!(payload, expected_payloads[i]);
        assert!(validate_segment(&header, payload).is_ok());
    }
}

#[test]
fn version_forward_compatibility_unknown_tags_skipped() {
    // A file might contain known + unknown segment types.
    // The reader should skip unknown ones and still find known segments.
    let mut file = Vec::new();

    // Known VEC_SEG.
    let vec_offset = file.len();
    file.extend_from_slice(&write_segment(
        SegmentType::Vec as u8,
        b"vector data",
        SegmentFlags::empty(),
        1,
    ));

    // Unknown future segment type.
    file.extend_from_slice(&write_segment(
        0xFD,
        b"future extension data",
        SegmentFlags::empty(),
        2,
    ));

    // Known INDEX_SEG.
    let index_offset = file.len();
    file.extend_from_slice(&write_segment(
        SegmentType::Index as u8,
        b"index data",
        SegmentFlags::empty(),
        3,
    ));

    // Reader can still read known segments.
    let (hdr_vec, payload_vec) = read_segment(&file[vec_offset..]).unwrap();
    assert_eq!(hdr_vec.seg_type, SegmentType::Vec as u8);
    assert_eq!(payload_vec, b"vector data");

    let (hdr_idx, payload_idx) = read_segment(&file[index_offset..]).unwrap();
    assert_eq!(hdr_idx.seg_type, SegmentType::Index as u8);
    assert_eq!(payload_idx, b"index data");
}

#[test]
fn sealed_segment_flag_preserved() {
    let flags = SegmentFlags::empty().with(SegmentFlags::SEALED);
    let encoded = write_segment(SegmentType::Vec as u8, b"sealed data", flags, 1);
    let (header, _) = read_segment(&encoded).unwrap();
    assert!(
        header.flags & SegmentFlags::SEALED != 0,
        "SEALED flag should be preserved"
    );
}

#[test]
fn compressed_flag_preserved() {
    let flags = SegmentFlags::empty().with(SegmentFlags::COMPRESSED);
    let encoded = write_segment(SegmentType::Quant as u8, b"compressed quant", flags, 5);
    let (header, _) = read_segment(&encoded).unwrap();
    assert!(
        header.flags & SegmentFlags::COMPRESSED != 0,
        "COMPRESSED flag should be preserved"
    );
}
