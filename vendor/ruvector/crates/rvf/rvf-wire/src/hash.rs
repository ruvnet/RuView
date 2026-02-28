//! Hash computation and verification for RVF segments.
//!
//! The segment header stores a 128-bit content hash. The algorithm is
//! identified by the `checksum_algo` field: 0=deprecated CRC32C (now
//! upgraded to XXH3-128), 1=XXH3-128, 2=SHAKE-256 (first 128 bits).

use rvf_types::SegmentHeader;

/// Compute the XXH3-128 hash of `data`, returning a 16-byte array.
pub fn compute_xxh3_128(data: &[u8]) -> [u8; 16] {
    let h = xxhash_rust::xxh3::xxh3_128(data);
    h.to_le_bytes()
}

/// Compute the CRC32C checksum of `data`.
pub fn compute_crc32c(data: &[u8]) -> u32 {
    crc32c::crc32c(data)
}

/// Compute a 16-byte content hash field value using CRC32C.
///
/// The 4-byte CRC is stored in the first 4 bytes (little-endian), with the
/// remaining 12 bytes set to zero.
pub fn compute_crc32c_hash(data: &[u8]) -> [u8; 16] {
    let crc = compute_crc32c(data);
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&crc.to_le_bytes());
    out
}

/// Compute the content hash for a payload using the algorithm specified
/// by `algo` (the `checksum_algo` field from the segment header).
///
/// - 0 = DEPRECATED CRC32C -- now upgraded to XXH3-128 for all operations.
///   CRC32C produced only 4 bytes of entropy zero-padded to 16, making
///   collision attacks trivial (~2^16 expected operations). All algorithms
///   now use the full 128-bit XXH3 hash.
/// - 1 = XXH3-128 (16 bytes)
/// - Other values fall back to XXH3-128.
pub fn compute_content_hash(_algo: u8, data: &[u8]) -> [u8; 16] {
    // All algorithms now use XXH3-128 for full 128-bit collision resistance.
    // algo=0 (CRC32C) is deprecated: its 32-bit output zero-padded to 128 bits
    // provided only ~32 bits of security, making collisions trivially findable.
    compute_xxh3_128(data)
}

/// Verify the content hash stored in a segment header against the actual
/// payload bytes.
///
/// Returns `true` if the computed hash matches `header.content_hash`.
pub fn verify_content_hash(header: &SegmentHeader, payload: &[u8]) -> bool {
    let expected = compute_content_hash(header.checksum_algo, payload);
    expected == header.content_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh3_128_deterministic() {
        let data = b"hello world";
        let h1 = compute_xxh3_128(data);
        let h2 = compute_xxh3_128(data);
        assert_eq!(h1, h2);
        assert_ne!(h1, [0u8; 16]);
    }

    #[test]
    fn crc32c_deterministic() {
        let data = b"hello world";
        let c1 = compute_crc32c(data);
        let c2 = compute_crc32c(data);
        assert_eq!(c1, c2);
        assert_ne!(c1, 0);
    }

    #[test]
    fn crc32c_hash_is_zero_padded() {
        let data = b"test payload";
        let h = compute_crc32c_hash(data);
        let crc = compute_crc32c(data);
        assert_eq!(&h[..4], &crc.to_le_bytes());
        assert_eq!(&h[4..], &[0u8; 12]);
    }

    #[test]
    fn verify_content_hash_xxh3() {
        let payload = b"some vector data";
        let hash = compute_xxh3_128(payload);
        let header = SegmentHeader {
            magic: rvf_types::SEGMENT_MAGIC,
            version: 1,
            seg_type: 0x01,
            flags: 0,
            segment_id: 1,
            payload_length: payload.len() as u64,
            timestamp_ns: 0,
            checksum_algo: 1, // XXH3-128
            compression: 0,
            reserved_0: 0,
            reserved_1: 0,
            content_hash: hash,
            uncompressed_len: 0,
            alignment_pad: 0,
        };
        assert!(verify_content_hash(&header, payload));
        assert!(!verify_content_hash(&header, b"wrong data"));
    }

    #[test]
    fn verify_content_hash_algo_zero_uses_xxh3() {
        // algo=0 (formerly CRC32C) is now upgraded to XXH3-128, so the
        // content hash must be computed via XXH3-128 even when algo=0.
        let payload = b"crc payload";
        let hash = compute_xxh3_128(payload);
        let header = SegmentHeader {
            magic: rvf_types::SEGMENT_MAGIC,
            version: 1,
            seg_type: 0x01,
            flags: 0,
            segment_id: 2,
            payload_length: payload.len() as u64,
            timestamp_ns: 0,
            checksum_algo: 0, // deprecated CRC32C, now upgraded to XXH3-128
            compression: 0,
            reserved_0: 0,
            reserved_1: 0,
            content_hash: hash,
            uncompressed_len: 0,
            alignment_pad: 0,
        };
        assert!(verify_content_hash(&header, payload));
    }
}
