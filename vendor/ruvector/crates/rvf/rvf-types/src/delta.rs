//! DELTA_SEG (0x23) types for the RVF computational container.
//!
//! Defines the 64-byte `DeltaHeader` and associated enums per ADR-031.
//! The DELTA_SEG stores sparse delta patches between clusters,
//! enabling efficient incremental updates without full cluster rewrites.

use crate::error::RvfError;

/// Magic number for `DeltaHeader`: "RVDL" in big-endian.
pub const DELTA_MAGIC: u32 = 0x5256_444C;

/// Delta encoding strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DeltaEncoding {
    /// Sparse row patches (individual vector updates).
    SparseRows = 0,
    /// Low-rank approximation of the delta.
    LowRank = 1,
    /// Full cluster patch (complete replacement).
    FullPatch = 2,
}

impl TryFrom<u8> for DeltaEncoding {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::SparseRows),
            1 => Ok(Self::LowRank),
            2 => Ok(Self::FullPatch),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "DeltaEncoding",
                value: value as u64,
            }),
        }
    }
}

/// 64-byte header for DELTA_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct DeltaHeader {
    /// Magic: `DELTA_MAGIC` (0x5256444C, "RVDL").
    pub magic: u32,
    /// DeltaHeader format version (currently 1).
    pub version: u16,
    /// Delta encoding strategy (see `DeltaEncoding`).
    pub encoding: u8,
    /// Padding (must be zero).
    pub _pad: u8,
    /// Cluster ID that this delta applies to.
    pub base_cluster_id: u32,
    /// Number of vectors affected by this delta.
    pub affected_count: u32,
    /// Size of the delta payload in bytes.
    pub delta_size: u64,
    /// SHAKE-256-256 hash of the delta payload.
    pub delta_hash: [u8; 32],
    /// Reserved (must be zero).
    pub _reserved: [u8; 8],
}

// Compile-time assertion: DeltaHeader must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<DeltaHeader>() == 64);

impl DeltaHeader {
    /// Serialize the header to a 64-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0x00..0x04].copy_from_slice(&self.magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.version.to_le_bytes());
        buf[0x06] = self.encoding;
        buf[0x07] = self._pad;
        buf[0x08..0x0C].copy_from_slice(&self.base_cluster_id.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.affected_count.to_le_bytes());
        buf[0x10..0x18].copy_from_slice(&self.delta_size.to_le_bytes());
        buf[0x18..0x38].copy_from_slice(&self.delta_hash);
        buf[0x38..0x40].copy_from_slice(&self._reserved);
        buf
    }

    /// Deserialize a `DeltaHeader` from a 64-byte slice.
    pub fn from_bytes(data: &[u8; 64]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != DELTA_MAGIC {
            return Err(RvfError::BadMagic {
                expected: DELTA_MAGIC,
                got: magic,
            });
        }

        Ok(Self {
            magic,
            version: u16::from_le_bytes([data[0x04], data[0x05]]),
            encoding: data[0x06],
            _pad: data[0x07],
            base_cluster_id: u32::from_le_bytes([data[0x08], data[0x09], data[0x0A], data[0x0B]]),
            affected_count: u32::from_le_bytes([data[0x0C], data[0x0D], data[0x0E], data[0x0F]]),
            delta_size: u64::from_le_bytes([
                data[0x10], data[0x11], data[0x12], data[0x13], data[0x14], data[0x15], data[0x16],
                data[0x17],
            ]),
            delta_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x18..0x38]);
                h
            },
            _reserved: {
                let mut r = [0u8; 8];
                r.copy_from_slice(&data[0x38..0x40]);
                r
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> DeltaHeader {
        DeltaHeader {
            magic: DELTA_MAGIC,
            version: 1,
            encoding: DeltaEncoding::SparseRows as u8,
            _pad: 0,
            base_cluster_id: 42,
            affected_count: 10,
            delta_size: 2048,
            delta_hash: [0xDD; 32],
            _reserved: [0; 8],
        }
    }

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<DeltaHeader>(), 64);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = DELTA_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVDL");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = DeltaHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.magic, DELTA_MAGIC);
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.encoding, DeltaEncoding::SparseRows as u8);
        assert_eq!(decoded._pad, 0);
        assert_eq!(decoded.base_cluster_id, 42);
        assert_eq!(decoded.affected_count, 10);
        assert_eq!(decoded.delta_size, 2048);
        assert_eq!(decoded.delta_hash, [0xDD; 32]);
        assert_eq!(decoded._reserved, [0; 8]);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = DeltaHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, DELTA_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.version as *const _ as usize - base, 0x04);
        assert_eq!(&h.encoding as *const _ as usize - base, 0x06);
        assert_eq!(&h._pad as *const _ as usize - base, 0x07);
        assert_eq!(&h.base_cluster_id as *const _ as usize - base, 0x08);
        assert_eq!(&h.affected_count as *const _ as usize - base, 0x0C);
        assert_eq!(&h.delta_size as *const _ as usize - base, 0x10);
        assert_eq!(&h.delta_hash as *const _ as usize - base, 0x18);
        assert_eq!(&h._reserved as *const _ as usize - base, 0x38);
    }

    #[test]
    fn delta_encoding_try_from() {
        assert_eq!(DeltaEncoding::try_from(0), Ok(DeltaEncoding::SparseRows));
        assert_eq!(DeltaEncoding::try_from(1), Ok(DeltaEncoding::LowRank));
        assert_eq!(DeltaEncoding::try_from(2), Ok(DeltaEncoding::FullPatch));
        assert!(DeltaEncoding::try_from(3).is_err());
        assert!(DeltaEncoding::try_from(0xFF).is_err());
    }
}
