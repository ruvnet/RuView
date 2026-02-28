//! 64-byte segment header for the RVF format.

/// The fixed 64-byte header that precedes every segment payload.
///
/// Layout matches the wire format exactly (repr(C), little-endian fields).
/// Aligned to 64 bytes to match SIMD register width and cache-line size.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct SegmentHeader {
    /// Magic number: must be `0x52564653` ("RVFS").
    pub magic: u32,
    /// Segment format version (currently 1).
    pub version: u8,
    /// Segment type discriminator (see `SegmentType`).
    pub seg_type: u8,
    /// Bitfield flags (see `SegmentFlags`).
    pub flags: u16,
    /// Monotonically increasing segment ordinal.
    pub segment_id: u64,
    /// Byte length of payload (after header, before optional footer).
    pub payload_length: u64,
    /// Nanosecond UNIX timestamp of segment creation.
    pub timestamp_ns: u64,
    /// Hash algorithm enum: 0=CRC32C, 1=XXH3-128, 2=SHAKE-256.
    pub checksum_algo: u8,
    /// Compression enum: 0=none, 1=LZ4, 2=ZSTD, 3=custom.
    pub compression: u8,
    /// Reserved (must be zero).
    pub reserved_0: u16,
    /// Reserved (must be zero).
    pub reserved_1: u32,
    /// First 128 bits of payload hash (algorithm per `checksum_algo`).
    pub content_hash: [u8; 16],
    /// Original payload size before compression (0 if uncompressed).
    pub uncompressed_len: u32,
    /// Padding to reach the 64-byte boundary.
    pub alignment_pad: u32,
}

// Compile-time assertion: SegmentHeader must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<SegmentHeader>() == 64);

impl SegmentHeader {
    /// Create a new segment header with the given type and segment ID.
    /// All other fields are set to defaults.
    pub const fn new(seg_type: u8, segment_id: u64) -> Self {
        Self {
            magic: crate::constants::SEGMENT_MAGIC,
            version: crate::constants::SEGMENT_VERSION,
            seg_type,
            flags: 0,
            segment_id,
            payload_length: 0,
            timestamp_ns: 0,
            checksum_algo: 0,
            compression: 0,
            reserved_0: 0,
            reserved_1: 0,
            content_hash: [0u8; 16],
            uncompressed_len: 0,
            alignment_pad: 0,
        }
    }

    /// Check whether the magic field matches the expected value.
    #[inline]
    pub const fn is_valid_magic(&self) -> bool {
        self.magic == crate::constants::SEGMENT_MAGIC
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::SEGMENT_MAGIC;

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<SegmentHeader>(), 64);
    }

    #[test]
    fn header_alignment() {
        assert!(core::mem::align_of::<SegmentHeader>() <= 64);
    }

    #[test]
    fn new_header_has_valid_magic() {
        let h = SegmentHeader::new(0x01, 42);
        assert!(h.is_valid_magic());
        assert_eq!(h.magic, SEGMENT_MAGIC);
        assert_eq!(h.seg_type, 0x01);
        assert_eq!(h.segment_id, 42);
    }

    #[test]
    fn field_offsets() {
        // Verify field offsets match the wire format spec
        let h = SegmentHeader::new(0x01, 0);
        let base = &h as *const _ as usize;
        let magic_off = &h.magic as *const _ as usize - base;
        let version_off = &h.version as *const _ as usize - base;
        let seg_type_off = &h.seg_type as *const _ as usize - base;
        let flags_off = &h.flags as *const _ as usize - base;
        let segment_id_off = &h.segment_id as *const _ as usize - base;
        let payload_length_off = &h.payload_length as *const _ as usize - base;
        let timestamp_ns_off = &h.timestamp_ns as *const _ as usize - base;
        let checksum_algo_off = &h.checksum_algo as *const _ as usize - base;
        let compression_off = &h.compression as *const _ as usize - base;
        let reserved_0_off = &h.reserved_0 as *const _ as usize - base;
        let reserved_1_off = &h.reserved_1 as *const _ as usize - base;
        let content_hash_off = &h.content_hash as *const _ as usize - base;
        let uncompressed_len_off = &h.uncompressed_len as *const _ as usize - base;
        let alignment_pad_off = &h.alignment_pad as *const _ as usize - base;

        assert_eq!(magic_off, 0x00);
        assert_eq!(version_off, 0x04);
        assert_eq!(seg_type_off, 0x05);
        assert_eq!(flags_off, 0x06);
        assert_eq!(segment_id_off, 0x08);
        assert_eq!(payload_length_off, 0x10);
        assert_eq!(timestamp_ns_off, 0x18);
        assert_eq!(checksum_algo_off, 0x20);
        assert_eq!(compression_off, 0x21);
        assert_eq!(reserved_0_off, 0x22);
        assert_eq!(reserved_1_off, 0x24);
        assert_eq!(content_hash_off, 0x28);
        assert_eq!(uncompressed_len_off, 0x38);
        assert_eq!(alignment_pad_off, 0x3C);
    }
}
