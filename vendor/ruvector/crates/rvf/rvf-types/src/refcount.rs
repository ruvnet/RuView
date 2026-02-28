//! REFCOUNT_SEG (0x21) types for the RVF computational container.
//!
//! Defines the 32-byte `RefcountHeader` per ADR-031.
//! The REFCOUNT_SEG tracks reference counts for shared clusters,
//! enabling safe snapshot deletion and garbage collection.

use crate::error::RvfError;

/// Magic number for `RefcountHeader`: "RVRC" in big-endian.
pub const REFCOUNT_MAGIC: u32 = 0x5256_5243;

/// 32-byte header for REFCOUNT_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct RefcountHeader {
    /// Magic: `REFCOUNT_MAGIC` (0x52565243, "RVRC").
    pub magic: u32,
    /// RefcountHeader format version (currently 1).
    pub version: u16,
    /// Width of each refcount entry in bytes (1, 2, or 4).
    pub refcount_width: u8,
    /// Padding (must be zero).
    pub _pad: u8,
    /// Number of clusters tracked.
    pub cluster_count: u32,
    /// Maximum refcount value before overflow.
    pub max_refcount: u32,
    /// Offset to the refcount array within the segment payload.
    pub array_offset: u64,
    /// Snapshot epoch: 0 = mutable, >0 = frozen at this epoch.
    pub snapshot_epoch: u32,
    /// Reserved (must be zero).
    pub _reserved: u32,
}

// Compile-time assertion: RefcountHeader must be exactly 32 bytes.
const _: () = assert!(core::mem::size_of::<RefcountHeader>() == 32);

impl RefcountHeader {
    /// Serialize the header to a 32-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0x00..0x04].copy_from_slice(&self.magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.version.to_le_bytes());
        buf[0x06] = self.refcount_width;
        buf[0x07] = self._pad;
        buf[0x08..0x0C].copy_from_slice(&self.cluster_count.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.max_refcount.to_le_bytes());
        buf[0x10..0x18].copy_from_slice(&self.array_offset.to_le_bytes());
        buf[0x18..0x1C].copy_from_slice(&self.snapshot_epoch.to_le_bytes());
        buf[0x1C..0x20].copy_from_slice(&self._reserved.to_le_bytes());
        buf
    }

    /// Deserialize a `RefcountHeader` from a 32-byte slice.
    pub fn from_bytes(data: &[u8; 32]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != REFCOUNT_MAGIC {
            return Err(RvfError::BadMagic {
                expected: REFCOUNT_MAGIC,
                got: magic,
            });
        }

        let refcount_width = data[0x06];
        let pad = data[0x07];
        let reserved = u32::from_le_bytes([data[0x1C], data[0x1D], data[0x1E], data[0x1F]]);

        // Validate refcount_width is 1, 2, or 4 as specified
        if refcount_width != 1 && refcount_width != 2 && refcount_width != 4 {
            return Err(RvfError::InvalidEnumValue {
                type_name: "RefcountHeader::refcount_width",
                value: refcount_width as u64,
            });
        }

        // Validate padding and reserved fields are zero (spec requirement)
        if pad != 0 {
            return Err(RvfError::InvalidEnumValue {
                type_name: "RefcountHeader::_pad",
                value: pad as u64,
            });
        }
        if reserved != 0 {
            return Err(RvfError::InvalidEnumValue {
                type_name: "RefcountHeader::_reserved",
                value: reserved as u64,
            });
        }

        Ok(Self {
            magic,
            version: u16::from_le_bytes([data[0x04], data[0x05]]),
            refcount_width,
            _pad: pad,
            cluster_count: u32::from_le_bytes([data[0x08], data[0x09], data[0x0A], data[0x0B]]),
            max_refcount: u32::from_le_bytes([data[0x0C], data[0x0D], data[0x0E], data[0x0F]]),
            array_offset: u64::from_le_bytes([
                data[0x10], data[0x11], data[0x12], data[0x13], data[0x14], data[0x15], data[0x16],
                data[0x17],
            ]),
            snapshot_epoch: u32::from_le_bytes([data[0x18], data[0x19], data[0x1A], data[0x1B]]),
            _reserved: reserved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> RefcountHeader {
        RefcountHeader {
            magic: REFCOUNT_MAGIC,
            version: 1,
            refcount_width: 2,
            _pad: 0,
            cluster_count: 1024,
            max_refcount: 65535,
            array_offset: 64,
            snapshot_epoch: 0,
            _reserved: 0,
        }
    }

    #[test]
    fn header_size_is_32() {
        assert_eq!(core::mem::size_of::<RefcountHeader>(), 32);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = REFCOUNT_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVRC");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = RefcountHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.magic, REFCOUNT_MAGIC);
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.refcount_width, 2);
        assert_eq!(decoded._pad, 0);
        assert_eq!(decoded.cluster_count, 1024);
        assert_eq!(decoded.max_refcount, 65535);
        assert_eq!(decoded.array_offset, 64);
        assert_eq!(decoded.snapshot_epoch, 0);
        assert_eq!(decoded._reserved, 0);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = RefcountHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, REFCOUNT_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.version as *const _ as usize - base, 0x04);
        assert_eq!(&h.refcount_width as *const _ as usize - base, 0x06);
        assert_eq!(&h._pad as *const _ as usize - base, 0x07);
        assert_eq!(&h.cluster_count as *const _ as usize - base, 0x08);
        assert_eq!(&h.max_refcount as *const _ as usize - base, 0x0C);
        assert_eq!(&h.array_offset as *const _ as usize - base, 0x10);
        assert_eq!(&h.snapshot_epoch as *const _ as usize - base, 0x18);
        assert_eq!(&h._reserved as *const _ as usize - base, 0x1C);
    }

    #[test]
    fn frozen_snapshot_epoch() {
        let mut h = sample_header();
        h.snapshot_epoch = 42;
        let bytes = h.to_bytes();
        let decoded = RefcountHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.snapshot_epoch, 42);
    }
}
