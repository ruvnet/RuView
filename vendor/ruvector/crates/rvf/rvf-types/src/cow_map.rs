//! COW_MAP_SEG (0x20) types for the RVF computational container.
//!
//! Defines the 64-byte `CowMapHeader` and associated enums per ADR-031.
//! The COW_MAP_SEG tracks copy-on-write cluster mappings, enabling
//! branching and snapshotting of vector data without full duplication.

use crate::error::RvfError;

/// Magic number for `CowMapHeader`: "RVCM" in big-endian.
pub const COWMAP_MAGIC: u32 = 0x5256_434D;

/// Cluster map storage format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum MapFormat {
    /// Simple flat array of cluster entries.
    FlatArray = 0,
    /// Adaptive Radix Tree for sparse mappings.
    ArtTree = 1,
    /// Extent list for contiguous ranges.
    ExtentList = 2,
}

impl TryFrom<u8> for MapFormat {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::FlatArray),
            1 => Ok(Self::ArtTree),
            2 => Ok(Self::ExtentList),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "MapFormat",
                value: value as u64,
            }),
        }
    }
}

/// Entry in the COW cluster map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CowMapEntry {
    /// Cluster has been written locally at the given offset.
    LocalOffset(u64),
    /// Cluster data lives in the parent file.
    ParentRef,
    /// Cluster has not been allocated.
    Unallocated,
}

/// 64-byte header for COW_MAP_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CowMapHeader {
    /// Magic: `COWMAP_MAGIC` (0x5256434D, "RVCM").
    pub magic: u32,
    /// CowMapHeader format version (currently 1).
    pub version: u16,
    /// Map storage format (see `MapFormat`).
    pub map_format: u8,
    /// Compression policy for COW clusters.
    pub compression_policy: u8,
    /// Cluster size in bytes (must be power of 2, SIMD aligned).
    pub cluster_size_bytes: u32,
    /// Number of vectors per cluster.
    pub vectors_per_cluster: u32,
    /// UUID of the base (parent) file.
    pub base_file_id: [u8; 16],
    /// SHAKE-256-256 hash of the base file.
    pub base_file_hash: [u8; 32],
}

// Compile-time assertion: CowMapHeader must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<CowMapHeader>() == 64);

impl CowMapHeader {
    /// Serialize the header to a 64-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0x00..0x04].copy_from_slice(&self.magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.version.to_le_bytes());
        buf[0x06] = self.map_format;
        buf[0x07] = self.compression_policy;
        buf[0x08..0x0C].copy_from_slice(&self.cluster_size_bytes.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.vectors_per_cluster.to_le_bytes());
        buf[0x10..0x20].copy_from_slice(&self.base_file_id);
        buf[0x20..0x40].copy_from_slice(&self.base_file_hash);
        buf
    }

    /// Deserialize a `CowMapHeader` from a 64-byte slice.
    pub fn from_bytes(data: &[u8; 64]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != COWMAP_MAGIC {
            return Err(RvfError::BadMagic {
                expected: COWMAP_MAGIC,
                got: magic,
            });
        }

        let version = u16::from_le_bytes([data[0x04], data[0x05]]);
        let map_format = data[0x06];
        let cluster_size_bytes =
            u32::from_le_bytes([data[0x08], data[0x09], data[0x0A], data[0x0B]]);
        let vectors_per_cluster =
            u32::from_le_bytes([data[0x0C], data[0x0D], data[0x0E], data[0x0F]]);

        // Validate map_format is a known enum value
        let _ = MapFormat::try_from(map_format)?;

        // Validate cluster_size_bytes is a power of 2 and non-zero
        if cluster_size_bytes == 0 || !cluster_size_bytes.is_power_of_two() {
            return Err(RvfError::InvalidEnumValue {
                type_name: "CowMapHeader::cluster_size_bytes",
                value: cluster_size_bytes as u64,
            });
        }

        // Validate vectors_per_cluster is non-zero (prevents division by zero)
        if vectors_per_cluster == 0 {
            return Err(RvfError::InvalidEnumValue {
                type_name: "CowMapHeader::vectors_per_cluster",
                value: 0,
            });
        }

        Ok(Self {
            magic,
            version,
            map_format,
            compression_policy: data[0x07],
            cluster_size_bytes,
            vectors_per_cluster,
            base_file_id: {
                let mut id = [0u8; 16];
                id.copy_from_slice(&data[0x10..0x20]);
                id
            },
            base_file_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x20..0x40]);
                h
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> CowMapHeader {
        CowMapHeader {
            magic: COWMAP_MAGIC,
            version: 1,
            map_format: MapFormat::FlatArray as u8,
            compression_policy: 0,
            cluster_size_bytes: 4096,
            vectors_per_cluster: 64,
            base_file_id: [0xAA; 16],
            base_file_hash: [0xBB; 32],
        }
    }

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<CowMapHeader>(), 64);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = COWMAP_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVCM");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = CowMapHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.magic, COWMAP_MAGIC);
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.map_format, MapFormat::FlatArray as u8);
        assert_eq!(decoded.compression_policy, 0);
        assert_eq!(decoded.cluster_size_bytes, 4096);
        assert_eq!(decoded.vectors_per_cluster, 64);
        assert_eq!(decoded.base_file_id, [0xAA; 16]);
        assert_eq!(decoded.base_file_hash, [0xBB; 32]);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = CowMapHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, COWMAP_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.version as *const _ as usize - base, 0x04);
        assert_eq!(&h.map_format as *const _ as usize - base, 0x06);
        assert_eq!(&h.compression_policy as *const _ as usize - base, 0x07);
        assert_eq!(&h.cluster_size_bytes as *const _ as usize - base, 0x08);
        assert_eq!(&h.vectors_per_cluster as *const _ as usize - base, 0x0C);
        assert_eq!(&h.base_file_id as *const _ as usize - base, 0x10);
        assert_eq!(&h.base_file_hash as *const _ as usize - base, 0x20);
    }

    #[test]
    fn map_format_try_from() {
        assert_eq!(MapFormat::try_from(0), Ok(MapFormat::FlatArray));
        assert_eq!(MapFormat::try_from(1), Ok(MapFormat::ArtTree));
        assert_eq!(MapFormat::try_from(2), Ok(MapFormat::ExtentList));
        assert!(MapFormat::try_from(3).is_err());
        assert!(MapFormat::try_from(0xFF).is_err());
    }

    #[test]
    fn cow_map_entry_variants() {
        let local = CowMapEntry::LocalOffset(0x1000);
        let parent = CowMapEntry::ParentRef;
        let unalloc = CowMapEntry::Unallocated;

        assert_eq!(local, CowMapEntry::LocalOffset(0x1000));
        assert_eq!(parent, CowMapEntry::ParentRef);
        assert_eq!(unalloc, CowMapEntry::Unallocated);
        assert_ne!(local, parent);
    }
}
