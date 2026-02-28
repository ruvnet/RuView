//! WITNESS_SEG support for cryptographic audit trails.
//!
//! Each witness entry chains to the previous via hashes, forming a
//! tamper-evident log. The chain uses SHAKE-256 for hash binding.

use alloc::vec::Vec;
use rvf_types::{ErrorCode, RvfError};

use crate::hash::shake256_256;

/// A single entry in a witness chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessEntry {
    /// Hash of the previous entry (zero for the first entry).
    pub prev_hash: [u8; 32],
    /// Hash of the action being witnessed.
    pub action_hash: [u8; 32],
    /// Nanosecond UNIX timestamp.
    pub timestamp_ns: u64,
    /// Witness type: 0x01=PROVENANCE, 0x02=COMPUTATION, etc.
    pub witness_type: u8,
}

/// Size of one serialized witness entry: 32 + 32 + 8 + 1 = 73 bytes.
const ENTRY_SIZE: usize = 73;

/// Serialize a `WitnessEntry` into bytes.
fn encode_entry(entry: &WitnessEntry) -> [u8; ENTRY_SIZE] {
    let mut buf = [0u8; ENTRY_SIZE];
    buf[0..32].copy_from_slice(&entry.prev_hash);
    buf[32..64].copy_from_slice(&entry.action_hash);
    buf[64..72].copy_from_slice(&entry.timestamp_ns.to_le_bytes());
    buf[72] = entry.witness_type;
    buf
}

/// Deserialize a `WitnessEntry` from bytes.
///
/// # Errors
///
/// Returns `TruncatedSegment` if `data` is shorter than `ENTRY_SIZE` (73) bytes.
fn decode_entry(data: &[u8]) -> Result<WitnessEntry, RvfError> {
    if data.len() < ENTRY_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let mut prev_hash = [0u8; 32];
    prev_hash.copy_from_slice(&data[0..32]);
    let mut action_hash = [0u8; 32];
    action_hash.copy_from_slice(&data[32..64]);
    let timestamp_ns = u64::from_le_bytes(data[64..72].try_into().unwrap());
    let witness_type = data[72];
    Ok(WitnessEntry {
        prev_hash,
        action_hash,
        timestamp_ns,
        witness_type,
    })
}

/// Create a witness chain from entries, linking each to the previous via hashes.
///
/// The first entry's `prev_hash` is set to all zeros (genesis).
/// Subsequent entries have `prev_hash` = SHAKE-256(previous entry bytes).
///
/// Returns the serialized chain as a byte vector.
pub fn create_witness_chain(entries: &[WitnessEntry]) -> Vec<u8> {
    let mut chain = Vec::with_capacity(entries.len() * ENTRY_SIZE);
    let mut prev_hash = [0u8; 32];

    for entry in entries {
        let mut linked = entry.clone();
        linked.prev_hash = prev_hash;
        let encoded = encode_entry(&linked);
        prev_hash = shake256_256(&encoded);
        chain.extend_from_slice(&encoded);
    }

    chain
}

/// Verify a witness chain's integrity.
///
/// Checks that each entry's `prev_hash` matches the SHAKE-256 hash of the
/// preceding entry. Returns the decoded entries if valid.
pub fn verify_witness_chain(data: &[u8]) -> Result<Vec<WitnessEntry>, RvfError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if !data.len().is_multiple_of(ENTRY_SIZE) {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let count = data.len() / ENTRY_SIZE;
    let mut entries = Vec::with_capacity(count);
    let mut expected_prev = [0u8; 32];

    for i in 0..count {
        let offset = i * ENTRY_SIZE;
        let entry_bytes = &data[offset..offset + ENTRY_SIZE];
        let entry = decode_entry(entry_bytes)?;

        if entry.prev_hash != expected_prev {
            return Err(RvfError::Code(ErrorCode::InvalidChecksum));
        }

        expected_prev = shake256_256(entry_bytes);
        entries.push(entry);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries(n: usize) -> Vec<WitnessEntry> {
        (0..n)
            .map(|i| WitnessEntry {
                prev_hash: [0u8; 32], // will be overwritten by create_witness_chain
                action_hash: shake256_256(&[i as u8]),
                timestamp_ns: 1_000_000_000 + i as u64,
                witness_type: 0x01,
            })
            .collect()
    }

    #[test]
    fn empty_chain() {
        let chain = create_witness_chain(&[]);
        assert!(chain.is_empty());
        let result = verify_witness_chain(&chain).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_entry_chain() {
        let entries = make_entries(1);
        let chain = create_witness_chain(&entries);
        assert_eq!(chain.len(), ENTRY_SIZE);
        let verified = verify_witness_chain(&chain).unwrap();
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].prev_hash, [0u8; 32]);
    }

    #[test]
    fn multi_entry_chain() {
        let entries = make_entries(5);
        let chain = create_witness_chain(&entries);
        assert_eq!(chain.len(), 5 * ENTRY_SIZE);
        let verified = verify_witness_chain(&chain).unwrap();
        assert_eq!(verified.len(), 5);
        for (i, entry) in verified.iter().enumerate() {
            assert_eq!(entry.action_hash, entries[i].action_hash);
            assert_eq!(entry.timestamp_ns, entries[i].timestamp_ns);
        }
    }

    #[test]
    fn tampered_chain_detected() {
        let entries = make_entries(3);
        let mut chain = create_witness_chain(&entries);
        // Tamper with the second entry's action_hash byte
        chain[ENTRY_SIZE + 32] ^= 0xFF;
        let result = verify_witness_chain(&chain);
        assert!(result.is_err());
    }

    #[test]
    fn truncated_chain_detected() {
        let entries = make_entries(2);
        let chain = create_witness_chain(&entries);
        let result = verify_witness_chain(&chain[..ENTRY_SIZE + 10]);
        assert!(result.is_err());
    }

    #[test]
    fn chain_links_are_correct() {
        let entries = make_entries(3);
        let chain = create_witness_chain(&entries);
        let verified = verify_witness_chain(&chain).unwrap();
        // First entry has zero prev_hash
        assert_eq!(verified[0].prev_hash, [0u8; 32]);
        // Second entry's prev_hash should equal hash of first entry's bytes
        let first_bytes = &chain[0..ENTRY_SIZE];
        let expected = shake256_256(first_bytes);
        assert_eq!(verified[1].prev_hash, expected);
    }
}
