//! Manifest Writer — builds a complete manifest (Level 1 TLV + Level 0 root).
//!
//! Output: Level 1 TLV payload followed by Level 0 root as last 4096 bytes.

use alloc::vec::Vec;
use rvf_types::{Level0Root, ROOT_MANIFEST_SIZE};

use crate::boot::HotsetPointers;
use crate::chain::{self, OverlayChain};
use crate::directory::{self, SegmentDirectory};
use crate::level0;
use crate::level1::{self, ManifestTag, TlvRecord};

/// Build a complete manifest from a segment directory, hotset pointers, epoch,
/// and an optional overlay chain (previous manifest link).
///
/// Returns a byte buffer containing:
///   - Level 1 TLV records (variable size)
///   - Level 0 root manifest (last 4096 bytes)
///
/// The `l1_manifest_offset` in Level 0 is set to 0 because the caller
/// must adjust it to the actual file position where this data is written.
/// Use [`build_manifest_at`] if you know the file offset ahead of time.
pub fn build_manifest(
    dir: &SegmentDirectory,
    hotset: &HotsetPointers,
    epoch: u32,
    prev_chain: Option<&OverlayChain>,
) -> Vec<u8> {
    build_manifest_at(dir, hotset, epoch, prev_chain, 0)
}

/// Like [`build_manifest`], but sets `l1_manifest_offset` to `file_offset`.
///
/// This is for when the caller knows exactly where in the file the
/// manifest payload will be written.
pub fn build_manifest_at(
    dir: &SegmentDirectory,
    hotset: &HotsetPointers,
    epoch: u32,
    prev_chain: Option<&OverlayChain>,
    file_offset: u64,
) -> Vec<u8> {
    // Build TLV records
    let mut records = Vec::new();

    // Segment directory record
    let dir_bytes = directory::write_directory(dir);
    records.push(TlvRecord {
        tag: ManifestTag::SegmentDir,
        length: dir_bytes.len() as u32,
        value: dir_bytes,
    });

    // Overlay chain record (if provided)
    if let Some(chain_ref) = prev_chain {
        let chain_bytes = chain::write_overlay_chain(chain_ref);
        records.push(TlvRecord {
            tag: ManifestTag::OverlayChain,
            length: chain_bytes.len() as u32,
            value: chain_bytes,
        });
    }

    let l1_bytes = level1::write_tlv_records(&records);
    let l1_len = l1_bytes.len() as u64;

    // Build Level 0 root
    let mut root = Level0Root::zeroed();
    root.version = 1;
    root.l1_manifest_offset = file_offset;
    root.l1_manifest_length = l1_len;
    root.epoch = epoch;
    root.entrypoint = hotset.entrypoint;
    root.toplayer = hotset.toplayer;
    root.centroid = hotset.centroid;
    root.quantdict = hotset.quantdict;
    root.hot_cache = hotset.hot_cache;
    root.prefetch_map = hotset.prefetch_map;

    let l0_bytes = level0::write_level0(&root);

    // Output: L1 TLV data + L0 root (last 4096 bytes)
    let mut out = Vec::with_capacity(l1_bytes.len() + ROOT_MANIFEST_SIZE);
    out.extend_from_slice(&l1_bytes);
    out.extend_from_slice(&l0_bytes);
    out
}

/// Write a manifest to a writer (e.g., file).
///
/// This appends the manifest bytes and flushes.
#[cfg(feature = "std")]
pub fn commit_manifest(
    file: &mut impl std::io::Write,
    manifest_bytes: &[u8],
) -> Result<(), rvf_types::RvfError> {
    file.write_all(manifest_bytes)
        .map_err(|_| rvf_types::RvfError::Code(rvf_types::ErrorCode::FsyncFailed))?;
    file.flush()
        .map_err(|_| rvf_types::RvfError::Code(rvf_types::ErrorCode::FsyncFailed))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory::SegmentDirEntry;
    use rvf_types::EntrypointPtr;

    fn sample_dir() -> SegmentDirectory {
        SegmentDirectory {
            entries: vec![
                SegmentDirEntry {
                    segment_id: 1,
                    seg_type: 0x01,
                    tier: 0,
                    file_offset: 0,
                    payload_length: 4096,
                    ..SegmentDirEntry::default()
                },
                SegmentDirEntry {
                    segment_id: 2,
                    seg_type: 0x02,
                    tier: 1,
                    file_offset: 4096,
                    payload_length: 8192,
                    ..SegmentDirEntry::default()
                },
            ],
        }
    }

    fn sample_hotset() -> HotsetPointers {
        HotsetPointers {
            entrypoint: EntrypointPtr {
                seg_offset: 0x100,
                block_offset: 0,
                count: 5,
            },
            ..Default::default()
        }
    }

    #[test]
    fn build_manifest_ends_with_level0() {
        let manifest = build_manifest(&sample_dir(), &sample_hotset(), 1, None);
        assert!(manifest.len() > ROOT_MANIFEST_SIZE);

        // Last 4096 bytes should be a valid Level 0
        let l0_start = manifest.len() - ROOT_MANIFEST_SIZE;
        let l0_data: &[u8; 4096] = manifest[l0_start..].try_into().unwrap();
        assert!(level0::validate_level0(l0_data));

        let root = level0::read_level0(l0_data).unwrap();
        assert_eq!(root.epoch, 1);
        assert_eq!(root.entrypoint.count, 5);
    }

    #[test]
    fn build_manifest_with_chain() {
        let chain = OverlayChain {
            epoch: 1,
            prev_manifest_offset: 0x1000,
            prev_manifest_id: 5,
            checkpoint_hash: [0xAB; 16],
        };

        let manifest = build_manifest(&sample_dir(), &sample_hotset(), 2, Some(&chain));
        assert!(manifest.len() > ROOT_MANIFEST_SIZE);

        let l0_start = manifest.len() - ROOT_MANIFEST_SIZE;
        let l0_data: &[u8; 4096] = manifest[l0_start..].try_into().unwrap();
        let root = level0::read_level0(l0_data).unwrap();
        assert_eq!(root.epoch, 2);
    }

    #[test]
    fn build_manifest_at_with_offset() {
        let offset = 0x1_0000u64;
        let manifest = build_manifest_at(&sample_dir(), &sample_hotset(), 3, None, offset);

        let l0_start = manifest.len() - ROOT_MANIFEST_SIZE;
        let l0_data: &[u8; 4096] = manifest[l0_start..].try_into().unwrap();
        let root = level0::read_level0(l0_data).unwrap();
        assert_eq!(root.l1_manifest_offset, offset);
    }

    #[cfg(feature = "std")]
    #[test]
    fn commit_manifest_writes_to_vec() {
        let manifest = build_manifest(&sample_dir(), &sample_hotset(), 1, None);
        let mut output = Vec::new();
        commit_manifest(&mut output, &manifest).unwrap();
        assert_eq!(output, manifest);
    }
}
