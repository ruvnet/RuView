//! COW cluster map for vector-addressed cluster resolution.
//!
//! Supports three formats: flat array (default), ART tree, and extent list.
//! Currently only flat_array is implemented; ART tree and extent list are
//! reserved for future optimization of sparse mappings.

use rvf_types::cow_map::{CowMapEntry, MapFormat};
use rvf_types::{ErrorCode, RvfError};

/// Adaptive cluster map for cluster_id -> location resolution.
///
/// Each cluster is either local (written to this file), inherited from the
/// parent (ParentRef), or unallocated.
pub struct CowMap {
    format: MapFormat,
    entries: Vec<CowMapEntry>,
}

impl CowMap {
    /// Create a new flat-array map with `cluster_count` entries, all Unallocated.
    pub fn new_flat(cluster_count: u32) -> Self {
        Self {
            format: MapFormat::FlatArray,
            entries: vec![CowMapEntry::Unallocated; cluster_count as usize],
        }
    }

    /// Create a new flat-array map with all entries set to ParentRef.
    pub fn new_parent_ref(cluster_count: u32) -> Self {
        Self {
            format: MapFormat::FlatArray,
            entries: vec![CowMapEntry::ParentRef; cluster_count as usize],
        }
    }

    /// Look up a cluster by ID.
    pub fn lookup(&self, cluster_id: u32) -> CowMapEntry {
        self.entries
            .get(cluster_id as usize)
            .copied()
            .unwrap_or(CowMapEntry::Unallocated)
    }

    /// Update a cluster entry.
    pub fn update(&mut self, cluster_id: u32, entry: CowMapEntry) {
        let idx = cluster_id as usize;
        if idx >= self.entries.len() {
            self.entries.resize(idx + 1, CowMapEntry::Unallocated);
        }
        self.entries[idx] = entry;
    }

    /// Serialize the map to bytes.
    ///
    /// Wire format (flat_array):
    ///   format(u8) | cluster_count(u32) | entries[cluster_count]
    /// Each entry: tag(u8) | offset(u64)
    ///   tag 0x00 = Unallocated, tag 0x01 = ParentRef, tag 0x02 = LocalOffset
    pub fn serialize(&self) -> Vec<u8> {
        let count = self.entries.len() as u32;
        // 1 (format) + 4 (count) + count * 9 (tag + offset)
        let mut buf = Vec::with_capacity(5 + self.entries.len() * 9);
        buf.push(self.format as u8);
        buf.extend_from_slice(&count.to_le_bytes());
        for entry in &self.entries {
            match entry {
                CowMapEntry::Unallocated => {
                    buf.push(0x00);
                    buf.extend_from_slice(&0u64.to_le_bytes());
                }
                CowMapEntry::ParentRef => {
                    buf.push(0x01);
                    buf.extend_from_slice(&0u64.to_le_bytes());
                }
                CowMapEntry::LocalOffset(off) => {
                    buf.push(0x02);
                    buf.extend_from_slice(&off.to_le_bytes());
                }
            }
        }
        buf
    }

    /// Deserialize a CowMap from bytes.
    pub fn deserialize(data: &[u8], format: MapFormat) -> Result<Self, RvfError> {
        if data.len() < 5 {
            return Err(RvfError::Code(ErrorCode::CowMapCorrupt));
        }
        let stored_format = data[0];
        if stored_format != format as u8 {
            return Err(RvfError::Code(ErrorCode::CowMapCorrupt));
        }
        let count = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
        let expected_len = count
            .checked_mul(9)
            .and_then(|v| v.checked_add(5))
            .ok_or(RvfError::Code(ErrorCode::CowMapCorrupt))?;
        if data.len() < expected_len {
            return Err(RvfError::Code(ErrorCode::CowMapCorrupt));
        }
        let mut entries = Vec::with_capacity(count);
        let mut offset = 5;
        for _ in 0..count {
            let tag = data[offset];
            let val = u64::from_le_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
                data[offset + 8],
            ]);
            let entry = match tag {
                0x00 => CowMapEntry::Unallocated,
                0x01 => CowMapEntry::ParentRef,
                0x02 => CowMapEntry::LocalOffset(val),
                _ => return Err(RvfError::Code(ErrorCode::CowMapCorrupt)),
            };
            entries.push(entry);
            offset += 9;
        }
        Ok(Self { format, entries })
    }

    /// Count of clusters that have local data.
    pub fn local_cluster_count(&self) -> u32 {
        self.entries
            .iter()
            .filter(|e| matches!(e, CowMapEntry::LocalOffset(_)))
            .count() as u32
    }

    /// Total number of clusters in the map.
    pub fn cluster_count(&self) -> u32 {
        self.entries.len() as u32
    }

    /// Get the map format.
    pub fn format(&self) -> MapFormat {
        self.format
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_flat_all_unallocated() {
        let map = CowMap::new_flat(10);
        assert_eq!(map.cluster_count(), 10);
        assert_eq!(map.local_cluster_count(), 0);
        for i in 0..10 {
            assert_eq!(map.lookup(i), CowMapEntry::Unallocated);
        }
    }

    #[test]
    fn new_parent_ref_all_parent() {
        let map = CowMap::new_parent_ref(5);
        assert_eq!(map.cluster_count(), 5);
        for i in 0..5 {
            assert_eq!(map.lookup(i), CowMapEntry::ParentRef);
        }
    }

    #[test]
    fn update_and_lookup() {
        let mut map = CowMap::new_flat(4);
        map.update(1, CowMapEntry::LocalOffset(0x1000));
        map.update(3, CowMapEntry::ParentRef);
        assert_eq!(map.lookup(0), CowMapEntry::Unallocated);
        assert_eq!(map.lookup(1), CowMapEntry::LocalOffset(0x1000));
        assert_eq!(map.lookup(2), CowMapEntry::Unallocated);
        assert_eq!(map.lookup(3), CowMapEntry::ParentRef);
        assert_eq!(map.local_cluster_count(), 1);
    }

    #[test]
    fn update_grows_map() {
        let mut map = CowMap::new_flat(2);
        map.update(5, CowMapEntry::LocalOffset(0x2000));
        assert_eq!(map.cluster_count(), 6);
        assert_eq!(map.lookup(5), CowMapEntry::LocalOffset(0x2000));
    }

    #[test]
    fn out_of_bounds_lookup_returns_unallocated() {
        let map = CowMap::new_flat(2);
        assert_eq!(map.lookup(100), CowMapEntry::Unallocated);
    }

    #[test]
    fn serialize_deserialize_round_trip() {
        let mut map = CowMap::new_flat(4);
        map.update(0, CowMapEntry::LocalOffset(0x100));
        map.update(1, CowMapEntry::ParentRef);
        // 2 stays Unallocated
        map.update(3, CowMapEntry::LocalOffset(0x200));

        let bytes = map.serialize();
        let map2 = CowMap::deserialize(&bytes, MapFormat::FlatArray).unwrap();

        assert_eq!(map2.cluster_count(), 4);
        assert_eq!(map2.lookup(0), CowMapEntry::LocalOffset(0x100));
        assert_eq!(map2.lookup(1), CowMapEntry::ParentRef);
        assert_eq!(map2.lookup(2), CowMapEntry::Unallocated);
        assert_eq!(map2.lookup(3), CowMapEntry::LocalOffset(0x200));
    }

    #[test]
    fn deserialize_corrupt_data() {
        let result = CowMap::deserialize(&[0x00, 0x01], MapFormat::FlatArray);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_wrong_format() {
        let map = CowMap::new_flat(1);
        let bytes = map.serialize();
        let result = CowMap::deserialize(&bytes, MapFormat::ArtTree);
        assert!(result.is_err());
    }
}
