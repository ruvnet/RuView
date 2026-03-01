//! Level 1 Full Manifest — variable-size TLV records.
//!
//! Level 1 is encoded as a sequence of tag-length-value records,
//! each 8-byte aligned, for forward compatibility.

use alloc::vec::Vec;
use rvf_types::{ErrorCode, RvfError};

/// Tag values for Level 1 manifest records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ManifestTag {
    /// Array of segment directory entries.
    SegmentDir = 0x0001,
    /// Temperature tier assignments per block.
    TempTierMap = 0x0002,
    /// Index layer availability bitmap.
    IndexLayers = 0x0003,
    /// Epoch chain with rollback pointers.
    OverlayChain = 0x0004,
    /// Active/tombstoned segment sets.
    CompactionState = 0x0005,
    /// Multi-file shard references.
    ShardRefs = 0x0006,
    /// What this file can do (features, limits).
    CapabilityManifest = 0x0007,
    /// Domain-specific configuration.
    ProfileConfig = 0x0008,
    /// Pointer to latest SKETCH_SEG.
    AccessSketchRef = 0x0009,
    /// Full prefetch hint table.
    PrefetchTable = 0x000A,
    /// Restart point index for varint delta IDs.
    IdRestartPoints = 0x000B,
    /// Proof-of-computation witness chain.
    WitnessChain = 0x000C,
    /// Encryption key references (not keys themselves).
    KeyDirectory = 0x000D,
}

impl ManifestTag {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0001 => Some(Self::SegmentDir),
            0x0002 => Some(Self::TempTierMap),
            0x0003 => Some(Self::IndexLayers),
            0x0004 => Some(Self::OverlayChain),
            0x0005 => Some(Self::CompactionState),
            0x0006 => Some(Self::ShardRefs),
            0x0007 => Some(Self::CapabilityManifest),
            0x0008 => Some(Self::ProfileConfig),
            0x0009 => Some(Self::AccessSketchRef),
            0x000A => Some(Self::PrefetchTable),
            0x000B => Some(Self::IdRestartPoints),
            0x000C => Some(Self::WitnessChain),
            0x000D => Some(Self::KeyDirectory),
            _ => None,
        }
    }
}

/// A single TLV record from the Level 1 manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlvRecord {
    pub tag: ManifestTag,
    pub length: u32,
    pub value: Vec<u8>,
}

/// Parsed Level 1 manifest: a collection of TLV records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Level1Manifest {
    pub records: Vec<TlvRecord>,
}

impl Level1Manifest {
    /// Find the first record with the given tag.
    pub fn find(&self, tag: ManifestTag) -> Option<&TlvRecord> {
        self.records.iter().find(|r| r.tag == tag)
    }

    /// Find all records with the given tag.
    pub fn find_all(&self, tag: ManifestTag) -> Vec<&TlvRecord> {
        self.records.iter().filter(|r| r.tag == tag).collect()
    }
}

// ---------- helpers ----------

fn read_u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn write_u16_le(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u32_le(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Round up to the next 8-byte boundary.
fn align8(n: usize) -> usize {
    (n + 7) & !7
}

/// TLV record header layout:
///   tag: u16     (2 bytes)
///   length: u32  (4 bytes)
///   pad: u16     (2 bytes, to reach 8-byte alignment)
///   value: [u8; length]
///   [padding to 8-byte boundary]
const TLV_HEADER_SIZE: usize = 8; // tag(2) + length(4) + pad(2)

/// Deserialize a sequence of TLV records from raw bytes.
pub fn read_tlv_records(data: &[u8]) -> Result<Vec<TlvRecord>, RvfError> {
    let mut records = Vec::new();
    let mut pos = 0;

    while pos + TLV_HEADER_SIZE <= data.len() {
        let tag_raw = read_u16_le(data, pos);
        let length = read_u32_le(data, pos + 2);
        // pad at pos+6 is ignored on read

        let tag = ManifestTag::from_u16(tag_raw).ok_or(RvfError::InvalidEnumValue {
            type_name: "ManifestTag",
            value: tag_raw as u64,
        })?;

        let value_start = pos + TLV_HEADER_SIZE;
        let value_end = value_start + length as usize;

        if value_end > data.len() {
            return Err(RvfError::Code(ErrorCode::TruncatedSegment));
        }

        let value = data[value_start..value_end].to_vec();
        records.push(TlvRecord { tag, length, value });

        // Advance to next 8-byte aligned position
        pos = align8(value_end);
    }

    Ok(records)
}

/// Serialize a sequence of TLV records into bytes (8-byte aligned).
pub fn write_tlv_records(records: &[TlvRecord]) -> Vec<u8> {
    let mut buf = Vec::new();

    for rec in records {
        write_u16_le(&mut buf, rec.tag as u16);
        write_u32_le(&mut buf, rec.value.len() as u32);
        // pad field (2 bytes)
        buf.extend_from_slice(&[0u8; 2]);

        buf.extend_from_slice(&rec.value);

        // Pad to 8-byte boundary
        let padded = align8(buf.len());
        buf.resize(padded, 0);
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_from_u16_known() {
        assert_eq!(ManifestTag::from_u16(0x0001), Some(ManifestTag::SegmentDir));
        assert_eq!(
            ManifestTag::from_u16(0x000D),
            Some(ManifestTag::KeyDirectory)
        );
    }

    #[test]
    fn tag_from_u16_unknown() {
        assert_eq!(ManifestTag::from_u16(0x0000), None);
        assert_eq!(ManifestTag::from_u16(0x000E), None);
        assert_eq!(ManifestTag::from_u16(0xFFFF), None);
    }

    #[test]
    fn round_trip_single_record() {
        let records = vec![TlvRecord {
            tag: ManifestTag::SegmentDir,
            length: 5,
            value: vec![1, 2, 3, 4, 5],
        }];

        let bytes = write_tlv_records(&records);
        assert_eq!(bytes.len() % 8, 0, "output must be 8-byte aligned");

        let decoded = read_tlv_records(&bytes).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].tag, ManifestTag::SegmentDir);
        assert_eq!(decoded[0].value, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn round_trip_multiple_records() {
        let records = vec![
            TlvRecord {
                tag: ManifestTag::SegmentDir,
                length: 3,
                value: vec![0xAA, 0xBB, 0xCC],
            },
            TlvRecord {
                tag: ManifestTag::OverlayChain,
                length: 8,
                value: vec![1, 2, 3, 4, 5, 6, 7, 8],
            },
            TlvRecord {
                tag: ManifestTag::CapabilityManifest,
                length: 1,
                value: vec![0xFF],
            },
        ];

        let bytes = write_tlv_records(&records);
        assert_eq!(bytes.len() % 8, 0);

        let decoded = read_tlv_records(&bytes).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].tag, ManifestTag::SegmentDir);
        assert_eq!(decoded[0].value, vec![0xAA, 0xBB, 0xCC]);
        assert_eq!(decoded[1].tag, ManifestTag::OverlayChain);
        assert_eq!(decoded[1].value, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(decoded[2].tag, ManifestTag::CapabilityManifest);
        assert_eq!(decoded[2].value, vec![0xFF]);
    }

    #[test]
    fn empty_records() {
        let bytes = write_tlv_records(&[]);
        assert!(bytes.is_empty());
        let decoded = read_tlv_records(&bytes).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn truncated_value_returns_error() {
        let mut buf = Vec::new();
        write_u16_le(&mut buf, ManifestTag::SegmentDir as u16);
        write_u32_le(&mut buf, 100); // claims 100 bytes
        buf.extend_from_slice(&[0u8; 2]); // pad
        buf.extend_from_slice(&[0u8; 10]); // only 10 bytes

        let result = read_tlv_records(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn level1_manifest_find() {
        let manifest = Level1Manifest {
            records: vec![
                TlvRecord {
                    tag: ManifestTag::SegmentDir,
                    length: 3,
                    value: vec![1, 2, 3],
                },
                TlvRecord {
                    tag: ManifestTag::OverlayChain,
                    length: 2,
                    value: vec![4, 5],
                },
            ],
        };

        assert!(manifest.find(ManifestTag::SegmentDir).is_some());
        assert!(manifest.find(ManifestTag::OverlayChain).is_some());
        assert!(manifest.find(ManifestTag::CompactionState).is_none());
    }
}
