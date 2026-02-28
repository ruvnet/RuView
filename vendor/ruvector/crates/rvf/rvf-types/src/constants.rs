//! Magic numbers, alignment requirements, and size limits for the RVF format.

/// Segment header magic: "RVFS" in ASCII (little-endian u32).
pub const SEGMENT_MAGIC: u32 = 0x5256_4653;

/// Root manifest magic: "RVM0" in ASCII (little-endian u32).
pub const ROOT_MANIFEST_MAGIC: u32 = 0x5256_4D30;

/// All segments must start at a 64-byte aligned boundary (AVX-512 / cache-line width).
pub const SEGMENT_ALIGNMENT: usize = 64;

/// The Level 0 root manifest is always exactly 4096 bytes (one OS page / disk sector).
pub const ROOT_MANIFEST_SIZE: usize = 4096;

/// Maximum payload size for a single segment (4 GiB).
pub const MAX_SEGMENT_PAYLOAD: u64 = 4 * 1024 * 1024 * 1024;

/// Size of the segment header in bytes.
pub const SEGMENT_HEADER_SIZE: usize = 64;

/// Current segment format version.
pub const SEGMENT_VERSION: u8 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_bytes_match_ascii() {
        // "RVFS" => 0x52 0x56 0x46 0x53
        let bytes = SEGMENT_MAGIC.to_le_bytes();
        assert_eq!(&bytes, b"SFVR"); // LE representation: 0x53, 0x46, 0x56, 0x52
        let bytes_be = SEGMENT_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVFS");
    }

    #[test]
    fn root_manifest_magic_bytes() {
        let bytes_be = ROOT_MANIFEST_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVM0");
    }

    #[test]
    fn alignment_is_power_of_two() {
        assert!(SEGMENT_ALIGNMENT.is_power_of_two());
    }

    #[test]
    fn max_payload_is_4gb() {
        assert_eq!(MAX_SEGMENT_PAYLOAD, 0x1_0000_0000);
    }
}
