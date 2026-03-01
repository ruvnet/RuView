//! Segment header reader and validator.
//!
//! Reads the fixed 64-byte segment header from a byte slice, validates
//! magic and version fields, and optionally verifies the content hash.

use crate::hash::verify_content_hash;
use rvf_types::{
    ErrorCode, RvfError, SegmentHeader, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC, SEGMENT_VERSION,
};

/// Read and parse a segment header from the first 64 bytes of `data`.
///
/// Validates the magic number and format version. Does not verify the
/// content hash (use `validate_segment` for that).
///
/// # Errors
///
/// - `InvalidMagic` if the magic number does not match `RVFS`.
/// - `InvalidVersion` if the version is not supported.
/// - `TruncatedSegment` if `data` is shorter than 64 bytes.
pub fn read_segment_header(data: &[u8]) -> Result<SegmentHeader, RvfError> {
    if data.len() < SEGMENT_HEADER_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != SEGMENT_MAGIC {
        return Err(RvfError::BadMagic {
            expected: SEGMENT_MAGIC,
            got: magic,
        });
    }

    let version = data[4];
    if version != SEGMENT_VERSION {
        return Err(RvfError::Code(ErrorCode::InvalidVersion));
    }

    let seg_type = data[5];
    let flags = u16::from_le_bytes([data[6], data[7]]);
    let segment_id = u64::from_le_bytes(data[0x08..0x10].try_into().unwrap());
    let payload_length = u64::from_le_bytes(data[0x10..0x18].try_into().unwrap());
    let timestamp_ns = u64::from_le_bytes(data[0x18..0x20].try_into().unwrap());
    let checksum_algo = data[0x20];
    let compression = data[0x21];
    let reserved_0 = u16::from_le_bytes([data[0x22], data[0x23]]);
    let reserved_1 = u32::from_le_bytes(data[0x24..0x28].try_into().unwrap());
    let mut content_hash = [0u8; 16];
    content_hash.copy_from_slice(&data[0x28..0x38]);
    let uncompressed_len = u32::from_le_bytes(data[0x38..0x3C].try_into().unwrap());
    let alignment_pad = u32::from_le_bytes(data[0x3C..0x40].try_into().unwrap());

    Ok(SegmentHeader {
        magic,
        version,
        seg_type,
        flags,
        segment_id,
        payload_length,
        timestamp_ns,
        checksum_algo,
        compression,
        reserved_0,
        reserved_1,
        content_hash,
        uncompressed_len,
        alignment_pad,
    })
}

/// Validate the content hash of a segment.
///
/// Computes the hash of `payload` using the algorithm specified in
/// `header.checksum_algo` and compares it against `header.content_hash`.
///
/// The SEALED flag (0x0008) is intentionally NOT treated as a bypass for hash
/// verification. A segment being sealed only means it was verified during
/// compaction, but an attacker could set the flag on a corrupted segment to
/// skip validation. Always verify the content hash regardless of flags.
///
/// # Errors
///
/// - `InvalidChecksum` if the computed hash does not match.
pub fn validate_segment(header: &SegmentHeader, payload: &[u8]) -> Result<(), RvfError> {
    // Always verify the content hash. The SEALED flag is not a reason to skip
    // verification -- an attacker could set the flag on a tampered segment to
    // bypass integrity checks.
    if !verify_content_hash(header, payload) {
        return Err(RvfError::Code(ErrorCode::InvalidChecksum));
    }
    Ok(())
}

/// Read a complete segment: header + payload slice.
///
/// Returns the parsed header and a sub-slice of `data` containing the
/// payload bytes. The payload slice starts at offset 64 and extends for
/// `header.payload_length` bytes.
///
/// # Errors
///
/// - Any error from `read_segment_header`.
/// - `TruncatedSegment` if `data` does not contain enough bytes for the
///   declared payload length.
pub fn read_segment(data: &[u8]) -> Result<(SegmentHeader, &[u8]), RvfError> {
    let header = read_segment_header(data)?;
    let payload_end = SEGMENT_HEADER_SIZE + header.payload_length as usize;
    if data.len() < payload_end {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let payload = &data[SEGMENT_HEADER_SIZE..payload_end];
    Ok((header, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::write_segment;
    use rvf_types::{SegmentFlags, SegmentType};

    #[test]
    fn read_write_round_trip() {
        let payload = b"hello vector world";
        let flags = SegmentFlags::empty();
        let seg = write_segment(SegmentType::Vec as u8, payload, flags, 42);
        let (header, decoded_payload) = read_segment(&seg).unwrap();
        assert_eq!(header.magic, SEGMENT_MAGIC);
        assert_eq!(header.version, SEGMENT_VERSION);
        assert_eq!(header.seg_type, SegmentType::Vec as u8);
        assert_eq!(header.segment_id, 42);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn validate_segment_succeeds() {
        let payload = b"data for validation";
        let flags = SegmentFlags::empty();
        let seg = write_segment(SegmentType::Index as u8, payload, flags, 1);
        let (header, decoded_payload) = read_segment(&seg).unwrap();
        assert!(validate_segment(&header, decoded_payload).is_ok());
    }

    #[test]
    fn validate_segment_detects_corruption() {
        let payload = b"original data";
        let flags = SegmentFlags::empty();
        let seg = write_segment(SegmentType::Vec as u8, payload, flags, 1);
        let (header, _) = read_segment(&seg).unwrap();
        assert!(validate_segment(&header, b"corrupted data").is_err());
    }

    #[test]
    fn truncated_header_returns_error() {
        let result = read_segment_header(&[0u8; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_magic_returns_error() {
        let mut data = [0u8; 64];
        data[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        let result = read_segment_header(&data);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_version_returns_error() {
        let mut data = [0u8; 64];
        data[0..4].copy_from_slice(&SEGMENT_MAGIC.to_le_bytes());
        data[4] = 99; // bad version
        let result = read_segment_header(&data);
        assert!(result.is_err());
    }
}
