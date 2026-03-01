//! MEMBERSHIP_SEG (0x22) types for the RVF computational container.
//!
//! Defines the 96-byte `MembershipHeader` and associated enums per ADR-031.
//! The MEMBERSHIP_SEG stores vector membership filters for branches,
//! tracking which vectors belong to a given snapshot or branch.

use crate::error::RvfError;

/// Magic number for `MembershipHeader`: "RVMB" in big-endian.
pub const MEMBERSHIP_MAGIC: u32 = 0x5256_4D42;

/// Filter storage type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum FilterType {
    /// Dense bitmap (one bit per vector).
    Bitmap = 0,
    /// Roaring bitmap (compressed sparse).
    RoaringBitmap = 1,
}

impl TryFrom<u8> for FilterType {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Bitmap),
            1 => Ok(Self::RoaringBitmap),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "FilterType",
                value: value as u64,
            }),
        }
    }
}

/// Filter mode: include-by-default or exclude-by-default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum FilterMode {
    /// Vectors are included unless filtered out.
    Include = 0,
    /// Vectors are excluded unless explicitly included.
    Exclude = 1,
}

impl TryFrom<u8> for FilterMode {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Include),
            1 => Ok(Self::Exclude),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "FilterMode",
                value: value as u64,
            }),
        }
    }
}

/// 96-byte header for MEMBERSHIP_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MembershipHeader {
    /// Magic: `MEMBERSHIP_MAGIC` (0x52564D42, "RVMB").
    pub magic: u32,
    /// MembershipHeader format version (currently 1).
    pub version: u16,
    /// Filter storage type (see `FilterType`).
    pub filter_type: u8,
    /// Filter mode (see `FilterMode`).
    pub filter_mode: u8,
    /// Total number of vectors in the dataset.
    pub vector_count: u64,
    /// Number of vectors that are members.
    pub member_count: u64,
    /// Offset to the membership filter within the segment payload.
    pub filter_offset: u64,
    /// Size of the membership filter in bytes.
    pub filter_size: u32,
    /// Generation counter for optimistic concurrency.
    pub generation_id: u32,
    /// SHAKE-256-256 hash of the filter data.
    pub filter_hash: [u8; 32],
    /// Offset to optional Bloom filter for fast negative lookups.
    pub bloom_offset: u64,
    /// Size of the Bloom filter in bytes.
    pub bloom_size: u32,
    /// Reserved (must be zero).
    pub _reserved: u32,
    /// Reserved (must be zero).
    pub _reserved2: [u8; 8],
}

// Compile-time assertion: MembershipHeader must be exactly 96 bytes.
const _: () = assert!(core::mem::size_of::<MembershipHeader>() == 96);

impl MembershipHeader {
    /// Serialize the header to a 96-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut buf = [0u8; 96];
        buf[0x00..0x04].copy_from_slice(&self.magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.version.to_le_bytes());
        buf[0x06] = self.filter_type;
        buf[0x07] = self.filter_mode;
        buf[0x08..0x10].copy_from_slice(&self.vector_count.to_le_bytes());
        buf[0x10..0x18].copy_from_slice(&self.member_count.to_le_bytes());
        buf[0x18..0x20].copy_from_slice(&self.filter_offset.to_le_bytes());
        buf[0x20..0x24].copy_from_slice(&self.filter_size.to_le_bytes());
        buf[0x24..0x28].copy_from_slice(&self.generation_id.to_le_bytes());
        buf[0x28..0x48].copy_from_slice(&self.filter_hash);
        buf[0x48..0x50].copy_from_slice(&self.bloom_offset.to_le_bytes());
        buf[0x50..0x54].copy_from_slice(&self.bloom_size.to_le_bytes());
        buf[0x54..0x58].copy_from_slice(&self._reserved.to_le_bytes());
        buf[0x58..0x60].copy_from_slice(&self._reserved2);
        buf
    }

    /// Deserialize a `MembershipHeader` from a 96-byte slice.
    pub fn from_bytes(data: &[u8; 96]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != MEMBERSHIP_MAGIC {
            return Err(RvfError::BadMagic {
                expected: MEMBERSHIP_MAGIC,
                got: magic,
            });
        }

        Ok(Self {
            magic,
            version: u16::from_le_bytes([data[0x04], data[0x05]]),
            filter_type: data[0x06],
            filter_mode: data[0x07],
            vector_count: u64::from_le_bytes([
                data[0x08], data[0x09], data[0x0A], data[0x0B], data[0x0C], data[0x0D], data[0x0E],
                data[0x0F],
            ]),
            member_count: u64::from_le_bytes([
                data[0x10], data[0x11], data[0x12], data[0x13], data[0x14], data[0x15], data[0x16],
                data[0x17],
            ]),
            filter_offset: u64::from_le_bytes([
                data[0x18], data[0x19], data[0x1A], data[0x1B], data[0x1C], data[0x1D], data[0x1E],
                data[0x1F],
            ]),
            filter_size: u32::from_le_bytes([data[0x20], data[0x21], data[0x22], data[0x23]]),
            generation_id: u32::from_le_bytes([data[0x24], data[0x25], data[0x26], data[0x27]]),
            filter_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x28..0x48]);
                h
            },
            bloom_offset: u64::from_le_bytes([
                data[0x48], data[0x49], data[0x4A], data[0x4B], data[0x4C], data[0x4D], data[0x4E],
                data[0x4F],
            ]),
            bloom_size: u32::from_le_bytes([data[0x50], data[0x51], data[0x52], data[0x53]]),
            _reserved: u32::from_le_bytes([data[0x54], data[0x55], data[0x56], data[0x57]]),
            _reserved2: {
                let mut r = [0u8; 8];
                r.copy_from_slice(&data[0x58..0x60]);
                r
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> MembershipHeader {
        MembershipHeader {
            magic: MEMBERSHIP_MAGIC,
            version: 1,
            filter_type: FilterType::Bitmap as u8,
            filter_mode: FilterMode::Include as u8,
            vector_count: 1_000_000,
            member_count: 500_000,
            filter_offset: 96,
            filter_size: 125_000,
            generation_id: 1,
            filter_hash: [0xCC; 32],
            bloom_offset: 0,
            bloom_size: 0,
            _reserved: 0,
            _reserved2: [0; 8],
        }
    }

    #[test]
    fn header_size_is_96() {
        assert_eq!(core::mem::size_of::<MembershipHeader>(), 96);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = MEMBERSHIP_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVMB");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = MembershipHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.magic, MEMBERSHIP_MAGIC);
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.filter_type, FilterType::Bitmap as u8);
        assert_eq!(decoded.filter_mode, FilterMode::Include as u8);
        assert_eq!(decoded.vector_count, 1_000_000);
        assert_eq!(decoded.member_count, 500_000);
        assert_eq!(decoded.filter_offset, 96);
        assert_eq!(decoded.filter_size, 125_000);
        assert_eq!(decoded.generation_id, 1);
        assert_eq!(decoded.filter_hash, [0xCC; 32]);
        assert_eq!(decoded.bloom_offset, 0);
        assert_eq!(decoded.bloom_size, 0);
        assert_eq!(decoded._reserved, 0);
        assert_eq!(decoded._reserved2, [0; 8]);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = MembershipHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, MEMBERSHIP_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.version as *const _ as usize - base, 0x04);
        assert_eq!(&h.filter_type as *const _ as usize - base, 0x06);
        assert_eq!(&h.filter_mode as *const _ as usize - base, 0x07);
        assert_eq!(&h.vector_count as *const _ as usize - base, 0x08);
        assert_eq!(&h.member_count as *const _ as usize - base, 0x10);
        assert_eq!(&h.filter_offset as *const _ as usize - base, 0x18);
        assert_eq!(&h.filter_size as *const _ as usize - base, 0x20);
        assert_eq!(&h.generation_id as *const _ as usize - base, 0x24);
        assert_eq!(&h.filter_hash as *const _ as usize - base, 0x28);
        assert_eq!(&h.bloom_offset as *const _ as usize - base, 0x48);
        assert_eq!(&h.bloom_size as *const _ as usize - base, 0x50);
        assert_eq!(&h._reserved as *const _ as usize - base, 0x54);
        assert_eq!(&h._reserved2 as *const _ as usize - base, 0x58);
    }

    #[test]
    fn filter_type_try_from() {
        assert_eq!(FilterType::try_from(0), Ok(FilterType::Bitmap));
        assert_eq!(FilterType::try_from(1), Ok(FilterType::RoaringBitmap));
        assert!(FilterType::try_from(2).is_err());
        assert!(FilterType::try_from(0xFF).is_err());
    }

    #[test]
    fn filter_mode_try_from() {
        assert_eq!(FilterMode::try_from(0), Ok(FilterMode::Include));
        assert_eq!(FilterMode::try_from(1), Ok(FilterMode::Exclude));
        assert!(FilterMode::try_from(2).is_err());
        assert!(FilterMode::try_from(0xFF).is_err());
    }
}
