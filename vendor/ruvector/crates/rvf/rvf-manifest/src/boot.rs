//! Progressive Boot Sequence — read Level 0 from EOF, then Level 1.
//!
//! Phase 1: Read last 4 KB -> hotset pointers -> system is queryable.
//! Phase 2: Read Level 1 at l1_manifest_offset -> full directory.

use rvf_types::{
    CentroidPtr, EntrypointPtr, ErrorCode, HotCachePtr, Level0Root, PrefetchMapPtr, QuantDictPtr,
    RvfError, TopLayerPtr, ROOT_MANIFEST_SIZE,
};

use crate::directory::SegmentDirectory;
use crate::level0;
use crate::level1::{self, Level1Manifest};

/// Collected hotset offsets extracted from the Level 0 root.
#[derive(Clone, Debug)]
pub struct HotsetPointers {
    pub entrypoint: EntrypointPtr,
    pub toplayer: TopLayerPtr,
    pub centroid: CentroidPtr,
    pub quantdict: QuantDictPtr,
    pub hot_cache: HotCachePtr,
    pub prefetch_map: PrefetchMapPtr,
}

impl Default for HotsetPointers {
    fn default() -> Self {
        // Extract from a zeroed Level0Root
        let root = Level0Root::zeroed();
        Self {
            entrypoint: root.entrypoint,
            toplayer: root.toplayer,
            centroid: root.centroid,
            quantdict: root.quantdict,
            hot_cache: root.hot_cache,
            prefetch_map: root.prefetch_map,
        }
    }
}

/// Full boot state, progressively populated.
#[derive(Clone, Debug)]
pub struct BootState {
    pub level0: Level0Root,
    pub level1: Option<Level1Manifest>,
    pub segment_dir: Option<SegmentDirectory>,
}

/// Boot phase 1: read the last 4096 bytes from `file_data` and parse Level 0.
///
/// After this call the system has hotset pointers and can answer approximate queries.
pub fn boot_phase1(file_data: &[u8]) -> Result<Level0Root, RvfError> {
    if file_data.len() < ROOT_MANIFEST_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let start = file_data.len() - ROOT_MANIFEST_SIZE;
    let tail: &[u8; ROOT_MANIFEST_SIZE] =
        file_data[start..start + ROOT_MANIFEST_SIZE]
            .try_into()
            .map_err(|_| RvfError::Code(ErrorCode::TruncatedSegment))?;

    level0::read_level0(tail)
}

/// Boot phase 2: using the Level 0 root, read and parse Level 1 (TLV records).
///
/// After this call the system has the full segment directory.
pub fn boot_phase2(file_data: &[u8], root: &Level0Root) -> Result<Level1Manifest, RvfError> {
    let offset = root.l1_manifest_offset as usize;
    let length = root.l1_manifest_length as usize;

    if length == 0 {
        return Ok(Level1Manifest::default());
    }

    let end = offset
        .checked_add(length)
        .ok_or(RvfError::Code(ErrorCode::TruncatedSegment))?;
    if end > file_data.len() {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let records = level1::read_tlv_records(&file_data[offset..end])?;
    Ok(Level1Manifest { records })
}

/// Extract the six hotset pointers from a Level 0 root.
pub fn extract_hotset_offsets(root: &Level0Root) -> HotsetPointers {
    HotsetPointers {
        entrypoint: root.entrypoint,
        toplayer: root.toplayer,
        centroid: root.centroid,
        quantdict: root.quantdict,
        hot_cache: root.hot_cache,
        prefetch_map: root.prefetch_map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory::{self, SegmentDirEntry};
    use crate::level0;
    use crate::level1::{ManifestTag, TlvRecord};

    fn make_test_file() -> Vec<u8> {
        // Build a segment directory with a few entries
        let dir = SegmentDirectory {
            entries: vec![
                SegmentDirEntry {
                    segment_id: 1,
                    seg_type: 0x01, // VEC
                    tier: 0,
                    file_offset: 0,
                    payload_length: 4096,
                    ..SegmentDirEntry::default()
                },
                SegmentDirEntry {
                    segment_id: 2,
                    seg_type: 0x02, // INDEX
                    tier: 1,
                    file_offset: 4096,
                    payload_length: 8192,
                    ..SegmentDirEntry::default()
                },
            ],
        };

        // Build Level 1 TLV records
        let dir_bytes = directory::write_directory(&dir);
        let tlv_records = vec![TlvRecord {
            tag: ManifestTag::SegmentDir,
            length: dir_bytes.len() as u32,
            value: dir_bytes,
        }];
        let l1_bytes = crate::level1::write_tlv_records(&tlv_records);

        // Start with dummy segments data
        let mut file_data = vec![0u8; 16384];
        let l1_offset = file_data.len();
        file_data.extend_from_slice(&l1_bytes);

        // Build Level 0 pointing to L1
        let mut root = Level0Root::zeroed();
        root.version = 1;
        root.l1_manifest_offset = l1_offset as u64;
        root.l1_manifest_length = l1_bytes.len() as u64;
        root.total_vector_count = 10_000;
        root.dimension = 384;
        root.base_dtype = 1;
        root.profile_id = 2;
        root.epoch = 1;
        root.entrypoint = EntrypointPtr {
            seg_offset: 0x100,
            block_offset: 0,
            count: 3,
        };
        root.toplayer = TopLayerPtr {
            seg_offset: 0x200,
            block_offset: 64,
            node_count: 500,
        };
        root.centroid = CentroidPtr {
            seg_offset: 0x300,
            block_offset: 0,
            count: 128,
        };
        root.quantdict = QuantDictPtr {
            seg_offset: 0x400,
            block_offset: 0,
            size: 4096,
        };
        root.hot_cache = HotCachePtr {
            seg_offset: 0x500,
            block_offset: 0,
            vector_count: 1000,
        };
        root.prefetch_map = PrefetchMapPtr {
            offset: 0x600,
            entries: 200,
            _pad: 0,
        };

        let l0_bytes = level0::write_level0(&root);
        file_data.extend_from_slice(&l0_bytes);
        file_data
    }

    #[test]
    fn boot_phase1_extracts_hotset() {
        let file_data = make_test_file();
        let l0 = boot_phase1(&file_data).unwrap();

        assert_eq!(l0.dimension, 384);
        assert_eq!(l0.total_vector_count, 10_000);
        assert_eq!(l0.epoch, 1);
        assert_eq!(l0.entrypoint.count, 3);
        assert_eq!(l0.toplayer.node_count, 500);
        assert_eq!(l0.centroid.count, 128);
    }

    #[test]
    fn boot_phase2_loads_directory() {
        let file_data = make_test_file();
        let l0 = boot_phase1(&file_data).unwrap();
        let l1 = boot_phase2(&file_data, &l0).unwrap();

        assert!(!l1.records.is_empty());
        let dir_rec = l1.find(ManifestTag::SegmentDir).unwrap();
        let dir = directory::read_directory(&dir_rec.value).unwrap();
        assert_eq!(dir.entries.len(), 2);
        assert_eq!(dir.entries[0].segment_id, 1);
        assert_eq!(dir.entries[1].segment_id, 2);
    }

    #[test]
    fn extract_hotset_offsets_works() {
        let file_data = make_test_file();
        let l0 = boot_phase1(&file_data).unwrap();
        let hotset = extract_hotset_offsets(&l0);

        assert_eq!(hotset.entrypoint.seg_offset, 0x100);
        assert_eq!(hotset.toplayer.seg_offset, 0x200);
        assert_eq!(hotset.centroid.seg_offset, 0x300);
        assert_eq!(hotset.quantdict.seg_offset, 0x400);
        assert_eq!(hotset.hot_cache.seg_offset, 0x500);
        assert_eq!(hotset.prefetch_map.offset, 0x600);
    }

    #[test]
    fn boot_phase1_rejects_short_data() {
        let result = boot_phase1(&[0u8; 100]);
        assert!(result.is_err());
    }

    #[test]
    fn full_boot_state() {
        let file_data = make_test_file();
        let l0 = boot_phase1(&file_data).unwrap();
        let l1 = boot_phase2(&file_data, &l0).unwrap();

        let dir_rec = l1.find(ManifestTag::SegmentDir).unwrap();
        let dir = directory::read_directory(&dir_rec.value).unwrap();

        let state = BootState {
            level0: l0,
            level1: Some(l1),
            segment_dir: Some(dir),
        };

        assert_eq!(state.level0.epoch, 1);
        assert_eq!(state.segment_dir.as_ref().unwrap().len(), 2);
    }
}
