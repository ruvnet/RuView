//! DNA-style lineage provenance types for RVF files.
//!
//! Each RVF file carries a `FileIdentity` in the Level0Root reserved area,
//! enabling provenance chains: parent→child→grandchild with hash verification.

/// Derivation type describing how a child file was produced from its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DerivationType {
    /// Exact copy of the parent.
    Clone = 0,
    /// Subset of parent data (filtered).
    Filter = 1,
    /// Multiple parents merged into one.
    Merge = 2,
    /// Re-quantized from parent.
    Quantize = 3,
    /// Re-indexed (HNSW rebuild, etc.).
    Reindex = 4,
    /// Arbitrary transformation.
    Transform = 5,
    /// Point-in-time snapshot.
    Snapshot = 6,
    /// User-defined derivation.
    UserDefined = 0xFF,
}

impl TryFrom<u8> for DerivationType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Clone),
            1 => Ok(Self::Filter),
            2 => Ok(Self::Merge),
            3 => Ok(Self::Quantize),
            4 => Ok(Self::Reindex),
            5 => Ok(Self::Transform),
            6 => Ok(Self::Snapshot),
            0xFF => Ok(Self::UserDefined),
            other => Err(other),
        }
    }
}

/// File identity embedded in the Level0Root reserved area at offset 0xF00.
///
/// Exactly 68 bytes, fitting within the 252-byte reserved area.
/// Old readers that ignore the reserved area see zeros and continue working.
///
/// Layout:
/// | Offset | Size | Field          |
/// |--------|------|----------------|
/// | 0x00   | 16   | file_id        |
/// | 0x10   | 16   | parent_id      |
/// | 0x20   | 32   | parent_hash    |
/// | 0x40   | 4    | lineage_depth  |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct FileIdentity {
    /// Unique identifier for this file (UUID-style, 16 bytes).
    pub file_id: [u8; 16],
    /// Identifier of the parent file (all zeros for root files).
    pub parent_id: [u8; 16],
    /// SHAKE-256-256 hash of the parent's manifest (all zeros for root).
    pub parent_hash: [u8; 32],
    /// Lineage depth: 0 for root, incremented for each derivation.
    pub lineage_depth: u32,
}

// Compile-time assertion: FileIdentity must be exactly 68 bytes.
const _: () = assert!(core::mem::size_of::<FileIdentity>() == 68);

impl FileIdentity {
    /// Create a root identity (no parent) with the given file_id.
    pub const fn new_root(file_id: [u8; 16]) -> Self {
        Self {
            file_id,
            parent_id: [0u8; 16],
            parent_hash: [0u8; 32],
            lineage_depth: 0,
        }
    }

    /// Returns true if this is a root identity (no parent).
    pub fn is_root(&self) -> bool {
        self.parent_id == [0u8; 16] && self.lineage_depth == 0
    }

    /// Create an all-zero identity (default for files without lineage).
    pub const fn zeroed() -> Self {
        Self {
            file_id: [0u8; 16],
            parent_id: [0u8; 16],
            parent_hash: [0u8; 32],
            lineage_depth: 0,
        }
    }

    /// Serialize to a 68-byte array.
    pub fn to_bytes(&self) -> [u8; 68] {
        let mut buf = [0u8; 68];
        buf[0..16].copy_from_slice(&self.file_id);
        buf[16..32].copy_from_slice(&self.parent_id);
        buf[32..64].copy_from_slice(&self.parent_hash);
        buf[64..68].copy_from_slice(&self.lineage_depth.to_le_bytes());
        buf
    }

    /// Deserialize from a 68-byte slice.
    pub fn from_bytes(data: &[u8; 68]) -> Self {
        let mut file_id = [0u8; 16];
        file_id.copy_from_slice(&data[0..16]);
        let mut parent_id = [0u8; 16];
        parent_id.copy_from_slice(&data[16..32]);
        let mut parent_hash = [0u8; 32];
        parent_hash.copy_from_slice(&data[32..64]);
        // Safety: data is &[u8; 68], so data[64..68] is always exactly 4 bytes.
        // Use an explicit array conversion to avoid the unwrap.
        let lineage_depth = u32::from_le_bytes([data[64], data[65], data[66], data[67]]);
        Self {
            file_id,
            parent_id,
            parent_hash,
            lineage_depth,
        }
    }
}

/// A lineage record for witness chain entries.
///
/// Fixed 128 bytes with a 47-byte description field.
///
/// Layout:
/// | Offset | Size | Field            |
/// |--------|------|------------------|
/// | 0x00   | 16   | file_id          |
/// | 0x10   | 16   | parent_id        |
/// | 0x20   | 32   | parent_hash      |
/// | 0x40   | 1    | derivation_type  |
/// | 0x41   | 3    | _pad             |
/// | 0x44   | 4    | mutation_count   |
/// | 0x48   | 8    | timestamp_ns     |
/// | 0x50   | 1    | description_len  |
/// | 0x51   | 47   | description      |
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineageRecord {
    /// Unique identifier for this file.
    pub file_id: [u8; 16],
    /// Identifier of the parent file.
    pub parent_id: [u8; 16],
    /// SHAKE-256-256 hash of the parent's manifest.
    pub parent_hash: [u8; 32],
    /// How the child was derived from the parent.
    pub derivation_type: DerivationType,
    /// Number of mutations/changes applied.
    pub mutation_count: u32,
    /// Nanosecond UNIX timestamp of derivation.
    pub timestamp_ns: u64,
    /// Length of the description (max 47).
    pub description_len: u8,
    /// UTF-8 description of the derivation (47-byte buffer).
    pub description: [u8; 47],
}

/// Size of a serialized LineageRecord.
pub const LINEAGE_RECORD_SIZE: usize = 128;

impl LineageRecord {
    /// Create a new lineage record with a description string.
    pub fn new(
        file_id: [u8; 16],
        parent_id: [u8; 16],
        parent_hash: [u8; 32],
        derivation_type: DerivationType,
        mutation_count: u32,
        timestamp_ns: u64,
        desc: &str,
    ) -> Self {
        let desc_bytes = desc.as_bytes();
        let desc_len = desc_bytes.len().min(47) as u8;
        let mut description = [0u8; 47];
        description[..desc_len as usize].copy_from_slice(&desc_bytes[..desc_len as usize]);
        Self {
            file_id,
            parent_id,
            parent_hash,
            derivation_type,
            mutation_count,
            timestamp_ns,
            description_len: desc_len,
            description,
        }
    }

    /// Get the description as a string slice.
    pub fn description_str(&self) -> &str {
        let len = (self.description_len as usize).min(47);
        core::str::from_utf8(&self.description[..len]).unwrap_or("")
    }
}

// ---- Witness type constants for lineage entries ----

/// Witness type: file derivation event.
pub const WITNESS_DERIVATION: u8 = 0x09;
/// Witness type: lineage merge (multi-parent).
pub const WITNESS_LINEAGE_MERGE: u8 = 0x0A;
/// Witness type: lineage snapshot.
pub const WITNESS_LINEAGE_SNAPSHOT: u8 = 0x0B;
/// Witness type: lineage transform.
pub const WITNESS_LINEAGE_TRANSFORM: u8 = 0x0C;
/// Witness type: lineage verification.
pub const WITNESS_LINEAGE_VERIFY: u8 = 0x0D;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_identity_size() {
        assert_eq!(core::mem::size_of::<FileIdentity>(), 68);
    }

    #[test]
    fn file_identity_fits_in_reserved() {
        // Level0Root reserved area is 252 bytes; FileIdentity is 68 bytes
        assert!(core::mem::size_of::<FileIdentity>() <= 252);
    }

    #[test]
    fn file_identity_root() {
        let id = [0x42u8; 16];
        let fi = FileIdentity::new_root(id);
        assert!(fi.is_root());
        assert_eq!(fi.file_id, id);
        assert_eq!(fi.parent_id, [0u8; 16]);
        assert_eq!(fi.parent_hash, [0u8; 32]);
        assert_eq!(fi.lineage_depth, 0);
    }

    #[test]
    fn file_identity_zeroed_is_root() {
        let fi = FileIdentity::zeroed();
        assert!(fi.is_root());
    }

    #[test]
    fn file_identity_round_trip() {
        let fi = FileIdentity {
            file_id: [1u8; 16],
            parent_id: [2u8; 16],
            parent_hash: [3u8; 32],
            lineage_depth: 42,
        };
        let bytes = fi.to_bytes();
        let decoded = FileIdentity::from_bytes(&bytes);
        assert_eq!(fi, decoded);
    }

    #[test]
    fn file_identity_non_root() {
        let fi = FileIdentity {
            file_id: [1u8; 16],
            parent_id: [2u8; 16],
            parent_hash: [3u8; 32],
            lineage_depth: 1,
        };
        assert!(!fi.is_root());
    }

    #[test]
    fn derivation_type_round_trip() {
        let cases: &[(u8, DerivationType)] = &[
            (0, DerivationType::Clone),
            (1, DerivationType::Filter),
            (2, DerivationType::Merge),
            (3, DerivationType::Quantize),
            (4, DerivationType::Reindex),
            (5, DerivationType::Transform),
            (6, DerivationType::Snapshot),
            (0xFF, DerivationType::UserDefined),
        ];
        for &(raw, expected) in cases {
            assert_eq!(DerivationType::try_from(raw), Ok(expected));
            assert_eq!(expected as u8, raw);
        }
    }

    #[test]
    fn derivation_type_unknown() {
        assert_eq!(DerivationType::try_from(7), Err(7));
        assert_eq!(DerivationType::try_from(0xFE), Err(0xFE));
    }

    #[test]
    fn lineage_record_description() {
        let record = LineageRecord::new(
            [1u8; 16],
            [2u8; 16],
            [3u8; 32],
            DerivationType::Filter,
            5,
            1_000_000_000,
            "filtered by category",
        );
        assert_eq!(record.description_str(), "filtered by category");
        assert_eq!(record.description_len, 20);
    }

    #[test]
    fn lineage_record_long_description_truncated() {
        let long_desc = "a]".repeat(50); // 100 chars, way over 47
        let record = LineageRecord::new(
            [0u8; 16],
            [0u8; 16],
            [0u8; 32],
            DerivationType::Clone,
            0,
            0,
            &long_desc,
        );
        assert_eq!(record.description_len, 47);
    }

    #[test]
    fn witness_type_constants() {
        assert_eq!(WITNESS_DERIVATION, 0x09);
        assert_eq!(WITNESS_LINEAGE_MERGE, 0x0A);
        assert_eq!(WITNESS_LINEAGE_SNAPSHOT, 0x0B);
        assert_eq!(WITNESS_LINEAGE_TRANSFORM, 0x0C);
        assert_eq!(WITNESS_LINEAGE_VERIFY, 0x0D);
    }
}
