//! Bit-flip detection tests: verify that hash/CRC catches random corruption.
//!
//! From acceptance spec section 4: "Bit Flip Detection"
//! Pass criteria: 100% detection of single-bit flips. Corruption isolated to
//! affected segment.

use rvf_types::{SegmentFlags, SegmentType, SEGMENT_HEADER_SIZE};
use rvf_wire::{read_segment, validate_segment, write_segment};

#[test]
fn single_bit_flip_in_payload_detected() {
    let payload = b"important vector data that must not be corrupted";
    let encoded = write_segment(SegmentType::Vec as u8, payload, SegmentFlags::empty(), 1);
    let (header, _) = read_segment(&encoded).unwrap();

    // Flip each bit in the payload region and verify detection.
    let payload_start = SEGMENT_HEADER_SIZE;
    let payload_end = payload_start + payload.len();
    let mut detected = 0;
    let total = (payload_end - payload_start) * 8;

    for byte_idx in payload_start..payload_end {
        for bit in 0..8 {
            let mut corrupted = encoded.clone();
            corrupted[byte_idx] ^= 1 << bit;
            let corrupted_payload = &corrupted[payload_start..payload_end];

            if validate_segment(&header, corrupted_payload).is_err() {
                detected += 1;
            }
        }
    }

    assert_eq!(
        detected, total,
        "detected {detected}/{total} single-bit flips in payload"
    );
}

#[test]
fn multi_bit_corruption_detected() {
    let payload: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();
    let encoded = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), 2);
    let (header, _) = read_segment(&encoded).unwrap();

    // Corrupt multiple bytes.
    let payload_start = SEGMENT_HEADER_SIZE;
    let mut corrupted = encoded.clone();
    corrupted[payload_start] ^= 0xFF;
    corrupted[payload_start + 100] ^= 0x55;
    corrupted[payload_start + 200] ^= 0xAA;

    let corrupted_payload = &corrupted[payload_start..payload_start + payload.len()];
    assert!(
        validate_segment(&header, corrupted_payload).is_err(),
        "multi-byte corruption should be detected"
    );
}

#[test]
fn corruption_in_one_segment_does_not_affect_another() {
    // Build two segments.
    let payload_a = b"segment A vector data";
    let payload_b = b"segment B vector data";

    let seg_a = write_segment(SegmentType::Vec as u8, payload_a, SegmentFlags::empty(), 1);
    let seg_b = write_segment(SegmentType::Vec as u8, payload_b, SegmentFlags::empty(), 2);

    let mut file = seg_a.clone();
    let seg_b_offset = file.len();
    file.extend_from_slice(&seg_b);

    // Corrupt segment A's payload.
    let mut corrupted = file.clone();
    corrupted[SEGMENT_HEADER_SIZE] ^= 0xFF;

    // Segment A should fail validation.
    let (hdr_a, _) = read_segment(&seg_a).unwrap();
    let corrupted_payload_a =
        &corrupted[SEGMENT_HEADER_SIZE..SEGMENT_HEADER_SIZE + payload_a.len()];
    assert!(
        validate_segment(&hdr_a, corrupted_payload_a).is_err(),
        "corrupted segment A should fail"
    );

    // Segment B should still validate fine.
    let (hdr_b, payload_b_decoded) = read_segment(&corrupted[seg_b_offset..]).unwrap();
    assert!(
        validate_segment(&hdr_b, payload_b_decoded).is_ok(),
        "uncorrupted segment B should still pass"
    );
}

#[test]
fn header_magic_corruption_detected() {
    let encoded = write_segment(SegmentType::Vec as u8, b"data", SegmentFlags::empty(), 1);
    let mut corrupted = encoded.clone();
    // Corrupt the magic bytes.
    corrupted[0] ^= 0x01;

    assert!(
        read_segment(&corrupted).is_err(),
        "corrupted magic should cause read failure"
    );
}

#[test]
fn zero_payload_hash_is_valid() {
    // Even an empty payload should have a valid hash.
    let encoded = write_segment(SegmentType::Meta as u8, &[], SegmentFlags::empty(), 0);
    let (header, payload) = read_segment(&encoded).unwrap();
    assert!(validate_segment(&header, payload).is_ok());
}
