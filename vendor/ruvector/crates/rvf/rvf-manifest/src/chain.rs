//! Overlay Chain — manifest rollback pointers for point-in-time recovery.
//!
//! Each `OVERLAY_CHAIN` TLV record stores the epoch, a pointer to the
//! previous MANIFEST_SEG, and a checkpoint hash for bisection debugging.

use alloc::vec::Vec;
use rvf_types::RvfError;

/// Fixed size of the serialized overlay chain record.
pub const OVERLAY_CHAIN_SIZE: usize = 40;

/// An overlay chain entry linking to the previous manifest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverlayChain {
    /// Current epoch number.
    pub epoch: u32,
    /// Byte offset of the previous MANIFEST_SEG in the file.
    pub prev_manifest_offset: u64,
    /// Segment ID of the previous MANIFEST_SEG.
    pub prev_manifest_id: u64,
    /// Hash of the complete state at this epoch (first 128 bits).
    pub checkpoint_hash: [u8; 16],
}

/// Deserialize an overlay chain record.
///
/// Layout (36 bytes):
/// ```text
/// 0x00  u32      epoch
/// 0x04  u32      padding (must be zero)
/// 0x08  u64      prev_manifest_offset
/// 0x10  u64      prev_manifest_id
/// 0x18  [u8;16]  checkpoint_hash
/// ```
pub fn read_overlay_chain(data: &[u8]) -> Result<OverlayChain, RvfError> {
    if data.len() < OVERLAY_CHAIN_SIZE {
        return Err(RvfError::SizeMismatch {
            expected: OVERLAY_CHAIN_SIZE,
            got: data.len(),
        });
    }

    let epoch = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let mut off8 = [0u8; 8];
    off8.copy_from_slice(&data[0x08..0x10]);
    let prev_manifest_offset = u64::from_le_bytes(off8);
    off8.copy_from_slice(&data[0x10..0x18]);
    let prev_manifest_id = u64::from_le_bytes(off8);
    let mut checkpoint_hash = [0u8; 16];
    checkpoint_hash.copy_from_slice(&data[0x18..0x28]);

    Ok(OverlayChain {
        epoch,
        prev_manifest_offset,
        prev_manifest_id,
        checkpoint_hash,
    })
}

/// Serialize an overlay chain record to bytes.
pub fn write_overlay_chain(chain: &OverlayChain) -> Vec<u8> {
    let mut buf = vec![0u8; OVERLAY_CHAIN_SIZE];

    buf[0..4].copy_from_slice(&chain.epoch.to_le_bytes());
    // bytes 4..8 are padding (zero)
    buf[0x08..0x10].copy_from_slice(&chain.prev_manifest_offset.to_le_bytes());
    buf[0x10..0x18].copy_from_slice(&chain.prev_manifest_id.to_le_bytes());
    buf[0x18..0x28].copy_from_slice(&chain.checkpoint_hash);

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let chain = OverlayChain {
            epoch: 42,
            prev_manifest_offset: 0x1_0000,
            prev_manifest_id: 7,
            checkpoint_hash: [
                0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
                0x0B, 0x0C,
            ],
        };

        let bytes = write_overlay_chain(&chain);
        assert_eq!(bytes.len(), OVERLAY_CHAIN_SIZE);

        let decoded = read_overlay_chain(&bytes).unwrap();
        assert_eq!(decoded, chain);
    }

    #[test]
    fn truncated_data() {
        let result = read_overlay_chain(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn default_chain() {
        let chain = OverlayChain::default();
        let bytes = write_overlay_chain(&chain);
        let decoded = read_overlay_chain(&bytes).unwrap();
        assert_eq!(decoded.epoch, 0);
        assert_eq!(decoded.prev_manifest_offset, 0);
        assert_eq!(decoded.prev_manifest_id, 0);
        assert_eq!(decoded.checkpoint_hash, [0u8; 16]);
    }

    #[test]
    fn chain_sequence() {
        let chain1 = OverlayChain {
            epoch: 1,
            prev_manifest_offset: 0,
            prev_manifest_id: 0,
            checkpoint_hash: [0x01; 16],
        };
        let chain2 = OverlayChain {
            epoch: 2,
            prev_manifest_offset: 0x1000,
            prev_manifest_id: 1,
            checkpoint_hash: [0x02; 16],
        };
        let chain3 = OverlayChain {
            epoch: 3,
            prev_manifest_offset: 0x2000,
            prev_manifest_id: 2,
            checkpoint_hash: [0x03; 16],
        };

        assert_eq!(chain3.prev_manifest_offset, 0x2000);
        assert_eq!(chain3.prev_manifest_id, 2);
        assert_eq!(chain2.prev_manifest_offset, 0x1000);
        assert_eq!(chain2.prev_manifest_id, 1);
        assert_eq!(chain1.prev_manifest_offset, 0);

        for chain in [chain1, chain2, chain3] {
            let bytes = write_overlay_chain(&chain);
            let decoded = read_overlay_chain(&bytes).unwrap();
            assert_eq!(decoded, chain);
        }
    }
}
