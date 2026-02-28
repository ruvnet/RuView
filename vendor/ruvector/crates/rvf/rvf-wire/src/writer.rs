//! Segment writer: serializes a segment header + payload into a byte buffer.
//!
//! The writer computes the content hash (XXH3-128 by default), sets the
//! timestamp, and pads the output to a 64-byte boundary.

use crate::hash::compute_content_hash;
use rvf_types::{
    SegmentFlags, SegmentHeader, SEGMENT_ALIGNMENT, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC,
    SEGMENT_VERSION,
};

/// Default checksum algorithm: XXH3-128.
const DEFAULT_CHECKSUM_ALGO: u8 = 1;

/// Calculate the total padded size of a segment (header + payload + padding).
///
/// The result is always a multiple of `SEGMENT_ALIGNMENT` (64 bytes).
pub fn calculate_padded_size(header_size: usize, payload_size: usize) -> usize {
    let raw = header_size + payload_size;
    (raw + SEGMENT_ALIGNMENT - 1) & !(SEGMENT_ALIGNMENT - 1)
}

/// Serialize a complete segment: 64-byte header + payload + zero-padding to
/// the next 64-byte boundary.
///
/// The content hash is computed over the raw payload using XXH3-128 (algo=1).
/// The timestamp is set to 0 (callers should overwrite if needed).
pub fn write_segment(
    seg_type: u8,
    payload: &[u8],
    flags: SegmentFlags,
    segment_id: u64,
) -> Vec<u8> {
    write_segment_with_algo(seg_type, payload, flags, segment_id, DEFAULT_CHECKSUM_ALGO)
}

/// Like `write_segment`, but allows specifying the checksum algorithm.
pub fn write_segment_with_algo(
    seg_type: u8,
    payload: &[u8],
    flags: SegmentFlags,
    segment_id: u64,
    checksum_algo: u8,
) -> Vec<u8> {
    let content_hash = compute_content_hash(checksum_algo, payload);
    let total_size = calculate_padded_size(SEGMENT_HEADER_SIZE, payload.len());
    let padding = total_size - SEGMENT_HEADER_SIZE - payload.len();

    let header = SegmentHeader {
        magic: SEGMENT_MAGIC,
        version: SEGMENT_VERSION,
        seg_type,
        flags: flags.bits(),
        segment_id,
        payload_length: payload.len() as u64,
        timestamp_ns: 0,
        checksum_algo,
        compression: 0,
        reserved_0: 0,
        reserved_1: 0,
        content_hash,
        uncompressed_len: 0,
        alignment_pad: padding as u32,
    };

    let mut buf = Vec::with_capacity(total_size);
    // Serialize header fields in little-endian order
    buf.extend_from_slice(&header.magic.to_le_bytes());
    buf.push(header.version);
    buf.push(header.seg_type);
    buf.extend_from_slice(&header.flags.to_le_bytes());
    buf.extend_from_slice(&header.segment_id.to_le_bytes());
    buf.extend_from_slice(&header.payload_length.to_le_bytes());
    buf.extend_from_slice(&header.timestamp_ns.to_le_bytes());
    buf.push(header.checksum_algo);
    buf.push(header.compression);
    buf.extend_from_slice(&header.reserved_0.to_le_bytes());
    buf.extend_from_slice(&header.reserved_1.to_le_bytes());
    buf.extend_from_slice(&header.content_hash);
    buf.extend_from_slice(&header.uncompressed_len.to_le_bytes());
    buf.extend_from_slice(&header.alignment_pad.to_le_bytes());

    debug_assert_eq!(buf.len(), SEGMENT_HEADER_SIZE);

    // Payload
    buf.extend_from_slice(payload);

    // Zero-padding to 64-byte alignment
    buf.resize(total_size, 0);

    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvf_types::SegmentType;

    #[test]
    fn output_is_64_byte_aligned() {
        for payload_size in [0, 1, 10, 63, 64, 65, 127, 128, 1000] {
            let payload = vec![0xABu8; payload_size];
            let seg = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), 0);
            assert_eq!(
                seg.len() % SEGMENT_ALIGNMENT,
                0,
                "not aligned for payload_size={payload_size}"
            );
        }
    }

    #[test]
    fn header_magic_and_version() {
        let seg = write_segment(SegmentType::Vec as u8, b"test", SegmentFlags::empty(), 1);
        let magic = u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]);
        assert_eq!(magic, SEGMENT_MAGIC);
        assert_eq!(seg[4], SEGMENT_VERSION);
    }

    #[test]
    fn segment_id_is_stored() {
        let seg = write_segment(
            SegmentType::Index as u8,
            b"idx",
            SegmentFlags::empty(),
            12345,
        );
        let id = u64::from_le_bytes(seg[0x08..0x10].try_into().unwrap());
        assert_eq!(id, 12345);
    }

    #[test]
    fn flags_are_stored() {
        let flags = SegmentFlags::empty()
            .with(SegmentFlags::COMPRESSED)
            .with(SegmentFlags::SEALED);
        let seg = write_segment(SegmentType::Vec as u8, b"data", flags, 0);
        let stored_flags = u16::from_le_bytes([seg[6], seg[7]]);
        assert!(stored_flags & SegmentFlags::COMPRESSED != 0);
        assert!(stored_flags & SegmentFlags::SEALED != 0);
    }

    #[test]
    fn payload_length_matches() {
        let payload = b"hello world!";
        let seg = write_segment(SegmentType::Vec as u8, payload, SegmentFlags::empty(), 0);
        let len = u64::from_le_bytes(seg[0x10..0x18].try_into().unwrap());
        assert_eq!(len, payload.len() as u64);
    }

    #[test]
    fn calculate_padded_size_examples() {
        assert_eq!(calculate_padded_size(64, 0), 64);
        assert_eq!(calculate_padded_size(64, 1), 128);
        assert_eq!(calculate_padded_size(64, 64), 128);
        assert_eq!(calculate_padded_size(64, 65), 192);
    }

    #[test]
    fn empty_payload() {
        let seg = write_segment(SegmentType::Meta as u8, &[], SegmentFlags::empty(), 0);
        assert_eq!(seg.len(), 64);
        let len = u64::from_le_bytes(seg[0x10..0x18].try_into().unwrap());
        assert_eq!(len, 0);
    }
}
