//! Lineage witness functions for DNA-style provenance chains.
//!
//! Provides serialization, hashing, and verification for lineage records
//! that track file derivation history through witness chain entries.

use rvf_types::{
    DerivationType, ErrorCode, FileIdentity, LineageRecord, RvfError, LINEAGE_RECORD_SIZE,
    WITNESS_DERIVATION,
};

use crate::hash::shake256_256;
use crate::witness::WitnessEntry;

/// Serialize a `LineageRecord` to a fixed 128-byte array.
pub fn lineage_record_to_bytes(record: &LineageRecord) -> [u8; LINEAGE_RECORD_SIZE] {
    let mut buf = [0u8; LINEAGE_RECORD_SIZE];
    buf[0x00..0x10].copy_from_slice(&record.file_id);
    buf[0x10..0x20].copy_from_slice(&record.parent_id);
    buf[0x20..0x40].copy_from_slice(&record.parent_hash);
    buf[0x40] = record.derivation_type as u8;
    // 3 bytes padding at 0x41..0x44
    buf[0x44..0x48].copy_from_slice(&record.mutation_count.to_le_bytes());
    buf[0x48..0x50].copy_from_slice(&record.timestamp_ns.to_le_bytes());
    buf[0x50] = record.description_len;
    let desc_len = (record.description_len as usize).min(47);
    buf[0x51..0x51 + desc_len].copy_from_slice(&record.description[..desc_len]);
    buf
}

/// Deserialize a `LineageRecord` from a 128-byte slice.
pub fn lineage_record_from_bytes(
    data: &[u8; LINEAGE_RECORD_SIZE],
) -> Result<LineageRecord, RvfError> {
    let mut file_id = [0u8; 16];
    file_id.copy_from_slice(&data[0x00..0x10]);
    let mut parent_id = [0u8; 16];
    parent_id.copy_from_slice(&data[0x10..0x20]);
    let mut parent_hash = [0u8; 32];
    parent_hash.copy_from_slice(&data[0x20..0x40]);

    let derivation_type =
        DerivationType::try_from(data[0x40]).map_err(|v| RvfError::InvalidEnumValue {
            type_name: "DerivationType",
            value: v as u64,
        })?;

    let mutation_count = u32::from_le_bytes(data[0x44..0x48].try_into().unwrap());
    let timestamp_ns = u64::from_le_bytes(data[0x48..0x50].try_into().unwrap());
    let description_len = data[0x50].min(47);
    let mut description = [0u8; 47];
    description[..description_len as usize]
        .copy_from_slice(&data[0x51..0x51 + description_len as usize]);

    Ok(LineageRecord {
        file_id,
        parent_id,
        parent_hash,
        derivation_type,
        mutation_count,
        timestamp_ns,
        description_len,
        description,
    })
}

/// Create a witness entry for a lineage derivation event.
///
/// The `action_hash` is SHAKE-256-256 of the serialized record bytes.
/// Uses witness type `WITNESS_DERIVATION` (0x09).
pub fn lineage_witness_entry(record: &LineageRecord, prev_hash: [u8; 32]) -> WitnessEntry {
    let record_bytes = lineage_record_to_bytes(record);
    let action_hash = shake256_256(&record_bytes);
    WitnessEntry {
        prev_hash,
        action_hash,
        timestamp_ns: record.timestamp_ns,
        witness_type: WITNESS_DERIVATION,
    }
}

/// Compute the SHAKE-256-256 hash of a 4096-byte manifest for use as parent_hash.
pub fn compute_manifest_hash(manifest: &[u8; 4096]) -> [u8; 32] {
    shake256_256(manifest)
}

/// Verify a lineage chain: each child's parent_hash must match the
/// hash of the corresponding parent's manifest bytes.
///
/// Takes pairs of (FileIdentity, manifest_hash) in order from root to leaf.
pub fn verify_lineage_chain(entries: &[(FileIdentity, [u8; 32])]) -> Result<(), RvfError> {
    if entries.is_empty() {
        return Ok(());
    }

    // First entry must be root
    if !entries[0].0.is_root() {
        return Err(RvfError::Code(ErrorCode::LineageBroken));
    }

    for i in 1..entries.len() {
        let child = &entries[i].0;
        let parent = &entries[i - 1].0;
        let parent_manifest_hash = &entries[i - 1].1;

        // Child's parent_id must match parent's file_id
        if child.parent_id != parent.file_id {
            return Err(RvfError::Code(ErrorCode::LineageBroken));
        }

        // Child's parent_hash must match parent's manifest hash
        if child.parent_hash != *parent_manifest_hash {
            return Err(RvfError::Code(ErrorCode::ParentHashMismatch));
        }

        // Depth must increment by 1
        if child.lineage_depth != parent.lineage_depth + 1 {
            return Err(RvfError::Code(ErrorCode::LineageBroken));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> LineageRecord {
        LineageRecord::new(
            [1u8; 16],
            [2u8; 16],
            [3u8; 32],
            DerivationType::Filter,
            5,
            1_700_000_000_000_000_000,
            "test derivation",
        )
    }

    #[test]
    fn lineage_record_round_trip() {
        let record = sample_record();
        let bytes = lineage_record_to_bytes(&record);
        assert_eq!(bytes.len(), LINEAGE_RECORD_SIZE);
        let decoded = lineage_record_from_bytes(&bytes).unwrap();
        assert_eq!(decoded.file_id, record.file_id);
        assert_eq!(decoded.parent_id, record.parent_id);
        assert_eq!(decoded.parent_hash, record.parent_hash);
        assert_eq!(decoded.derivation_type, record.derivation_type);
        assert_eq!(decoded.mutation_count, record.mutation_count);
        assert_eq!(decoded.timestamp_ns, record.timestamp_ns);
        assert_eq!(decoded.description_str(), record.description_str());
    }

    #[test]
    fn lineage_record_invalid_derivation_type() {
        let record = sample_record();
        let mut bytes = lineage_record_to_bytes(&record);
        bytes[0x40] = 0xFE; // invalid derivation type
        let result = lineage_record_from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn lineage_witness_entry_creates_valid_entry() {
        let record = sample_record();
        let prev_hash = [0u8; 32];
        let entry = lineage_witness_entry(&record, prev_hash);
        assert_eq!(entry.witness_type, WITNESS_DERIVATION);
        assert_eq!(entry.prev_hash, prev_hash);
        assert_eq!(entry.timestamp_ns, record.timestamp_ns);
        assert_ne!(entry.action_hash, [0u8; 32]);
    }

    #[test]
    fn compute_manifest_hash_deterministic() {
        let manifest = [0xABu8; 4096];
        let h1 = compute_manifest_hash(&manifest);
        let h2 = compute_manifest_hash(&manifest);
        assert_eq!(h1, h2);
        assert_ne!(h1, [0u8; 32]);
    }

    #[test]
    fn verify_empty_chain() {
        assert!(verify_lineage_chain(&[]).is_ok());
    }

    #[test]
    fn verify_single_root() {
        let root = FileIdentity::new_root([1u8; 16]);
        let hash = [0xAAu8; 32];
        assert!(verify_lineage_chain(&[(root, hash)]).is_ok());
    }

    #[test]
    fn verify_parent_child_chain() {
        let root_id = [1u8; 16];
        let child_id = [2u8; 16];
        let root_hash = [0xAAu8; 32];
        let child_hash = [0xBBu8; 32];

        let root = FileIdentity::new_root(root_id);
        let child = FileIdentity {
            file_id: child_id,
            parent_id: root_id,
            parent_hash: root_hash,
            lineage_depth: 1,
        };

        assert!(verify_lineage_chain(&[(root, root_hash), (child, child_hash)]).is_ok());
    }

    #[test]
    fn verify_broken_parent_id() {
        let root = FileIdentity::new_root([1u8; 16]);
        let root_hash = [0xAAu8; 32];
        let child = FileIdentity {
            file_id: [2u8; 16],
            parent_id: [3u8; 16], // wrong parent_id
            parent_hash: root_hash,
            lineage_depth: 1,
        };
        let result = verify_lineage_chain(&[(root, root_hash), (child, [0xBBu8; 32])]);
        assert!(result.is_err());
    }

    #[test]
    fn verify_hash_mismatch() {
        let root_id = [1u8; 16];
        let root = FileIdentity::new_root(root_id);
        let root_hash = [0xAAu8; 32];
        let child = FileIdentity {
            file_id: [2u8; 16],
            parent_id: root_id,
            parent_hash: [0xCCu8; 32], // wrong hash
            lineage_depth: 1,
        };
        let result = verify_lineage_chain(&[(root, root_hash), (child, [0xBBu8; 32])]);
        assert!(matches!(
            result,
            Err(RvfError::Code(ErrorCode::ParentHashMismatch))
        ));
    }

    #[test]
    fn verify_non_root_first() {
        let non_root = FileIdentity {
            file_id: [1u8; 16],
            parent_id: [2u8; 16],
            parent_hash: [3u8; 32],
            lineage_depth: 1,
        };
        let result = verify_lineage_chain(&[(non_root, [0u8; 32])]);
        assert!(result.is_err());
    }

    #[test]
    fn verify_depth_mismatch() {
        let root_id = [1u8; 16];
        let root = FileIdentity::new_root(root_id);
        let root_hash = [0xAAu8; 32];
        let child = FileIdentity {
            file_id: [2u8; 16],
            parent_id: root_id,
            parent_hash: root_hash,
            lineage_depth: 5, // should be 1
        };
        let result = verify_lineage_chain(&[(root, root_hash), (child, [0xBBu8; 32])]);
        assert!(result.is_err());
    }
}
