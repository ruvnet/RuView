//! Segment Directory — the array of segment location entries
//! stored inside the `SEGMENT_DIR` TLV record of Level 1.

use alloc::vec::Vec;
use rvf_types::{RvfError, SegmentType};

/// Size of each directory entry in bytes (cache-line aligned).
pub const DIR_ENTRY_SIZE: usize = 64;

/// A single entry in the segment directory.
///
/// Binary layout (64 bytes):
/// ```text
/// 0x00  u64   segment_id
/// 0x08  u8    seg_type
/// 0x09  u8    tier  (0=hot, 1=warm, 2=cold)
/// 0x0A  u16   flags
/// 0x0C  u32   reserved (must be zero)
/// 0x10  u64   file_offset
/// 0x18  u64   payload_length (decompressed)
/// 0x20  u64   compressed_length (0 if uncompressed)
/// 0x28  u16   shard_id (0 for main file)
/// 0x2A  u16   compression
/// 0x2C  u32   block_count
/// 0x30  [u8;16] content_hash (first 128 bits)
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentDirEntry {
    pub segment_id: u64,
    pub seg_type: u8,
    pub tier: u8,
    pub flags: u16,
    pub file_offset: u64,
    pub payload_length: u64,
    pub compressed_length: u64,
    pub shard_id: u16,
    pub compression: u16,
    pub block_count: u32,
    pub content_hash: [u8; 16],
}

/// The complete segment directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SegmentDirectory {
    pub entries: Vec<SegmentDirEntry>,
}

// ---------- helpers ----------

fn read_u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_u64_le(buf: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(b)
}

fn write_u16_le(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn write_u32_le(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn write_u64_le(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn read_entry(buf: &[u8], base: usize) -> SegmentDirEntry {
    let mut content_hash = [0u8; 16];
    content_hash.copy_from_slice(&buf[base + 0x30..base + 0x40]);

    SegmentDirEntry {
        segment_id: read_u64_le(buf, base),
        seg_type: buf[base + 0x08],
        tier: buf[base + 0x09],
        flags: read_u16_le(buf, base + 0x0A),
        file_offset: read_u64_le(buf, base + 0x10),
        payload_length: read_u64_le(buf, base + 0x18),
        compressed_length: read_u64_le(buf, base + 0x20),
        shard_id: read_u16_le(buf, base + 0x28),
        compression: read_u16_le(buf, base + 0x2A),
        block_count: read_u32_le(buf, base + 0x2C),
        content_hash,
    }
}

fn write_entry(buf: &mut [u8], base: usize, e: &SegmentDirEntry) {
    write_u64_le(buf, base, e.segment_id);
    buf[base + 0x08] = e.seg_type;
    buf[base + 0x09] = e.tier;
    write_u16_le(buf, base + 0x0A, e.flags);
    write_u32_le(buf, base + 0x0C, 0); // reserved
    write_u64_le(buf, base + 0x10, e.file_offset);
    write_u64_le(buf, base + 0x18, e.payload_length);
    write_u64_le(buf, base + 0x20, e.compressed_length);
    write_u16_le(buf, base + 0x28, e.shard_id);
    write_u16_le(buf, base + 0x2A, e.compression);
    write_u32_le(buf, base + 0x2C, e.block_count);
    buf[base + 0x30..base + 0x40].copy_from_slice(&e.content_hash);
}

/// Deserialize a segment directory from raw bytes.
pub fn read_directory(data: &[u8]) -> Result<SegmentDirectory, RvfError> {
    if !data.len().is_multiple_of(DIR_ENTRY_SIZE) {
        return Err(RvfError::SizeMismatch {
            expected: (data.len() / DIR_ENTRY_SIZE + 1) * DIR_ENTRY_SIZE,
            got: data.len(),
        });
    }

    let count = data.len() / DIR_ENTRY_SIZE;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        entries.push(read_entry(data, i * DIR_ENTRY_SIZE));
    }

    Ok(SegmentDirectory { entries })
}

/// Serialize a segment directory to raw bytes.
pub fn write_directory(dir: &SegmentDirectory) -> Vec<u8> {
    let mut buf = vec![0u8; dir.entries.len() * DIR_ENTRY_SIZE];
    for (i, entry) in dir.entries.iter().enumerate() {
        write_entry(&mut buf, i * DIR_ENTRY_SIZE, entry);
    }
    buf
}

impl SegmentDirectory {
    /// Find a segment by its ID.
    pub fn find_segment(&self, id: u64) -> Option<&SegmentDirEntry> {
        self.entries.iter().find(|e| e.segment_id == id)
    }

    /// Return all segments of the given type.
    pub fn segments_by_type(&self, seg_type: SegmentType) -> Vec<&SegmentDirEntry> {
        let raw = seg_type as u8;
        self.entries.iter().filter(|e| e.seg_type == raw).collect()
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u64, seg_type: u8, tier: u8) -> SegmentDirEntry {
        let mut hash = [0u8; 16];
        hash[0] = (id & 0xFF) as u8;
        SegmentDirEntry {
            segment_id: id,
            seg_type,
            tier,
            flags: 0,
            file_offset: id * 0x1000,
            payload_length: 4096,
            compressed_length: 0,
            shard_id: 0,
            compression: 0,
            block_count: 1,
            content_hash: hash,
        }
    }

    #[test]
    fn round_trip_single_entry() {
        let dir = SegmentDirectory {
            entries: vec![make_entry(1, SegmentType::Vec as u8, 0)],
        };

        let bytes = write_directory(&dir);
        assert_eq!(bytes.len(), 64);

        let decoded = read_directory(&bytes).unwrap();
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0], dir.entries[0]);
    }

    #[test]
    fn round_trip_100_entries() {
        let entries: Vec<_> = (0..100)
            .map(|i| make_entry(i, (i % 13 + 1) as u8, (i % 3) as u8))
            .collect();

        let dir = SegmentDirectory {
            entries: entries.clone(),
        };
        let bytes = write_directory(&dir);
        assert_eq!(bytes.len(), 100 * 64);

        let decoded = read_directory(&bytes).unwrap();
        assert_eq!(decoded.entries.len(), 100);
        for (a, b) in decoded.entries.iter().zip(entries.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn find_segment_by_id() {
        let dir = SegmentDirectory {
            entries: vec![
                make_entry(10, SegmentType::Vec as u8, 0),
                make_entry(20, SegmentType::Index as u8, 1),
                make_entry(30, SegmentType::Manifest as u8, 0),
            ],
        };

        assert_eq!(
            dir.find_segment(20).unwrap().seg_type,
            SegmentType::Index as u8
        );
        assert!(dir.find_segment(99).is_none());
    }

    #[test]
    fn filter_by_type() {
        let dir = SegmentDirectory {
            entries: vec![
                make_entry(1, SegmentType::Vec as u8, 0),
                make_entry(2, SegmentType::Vec as u8, 1),
                make_entry(3, SegmentType::Index as u8, 0),
                make_entry(4, SegmentType::Vec as u8, 2),
            ],
        };

        let vecs = dir.segments_by_type(SegmentType::Vec);
        assert_eq!(vecs.len(), 3);
        let indexes = dir.segments_by_type(SegmentType::Index);
        assert_eq!(indexes.len(), 1);
        let manifests = dir.segments_by_type(SegmentType::Manifest);
        assert_eq!(manifests.len(), 0);
    }

    #[test]
    fn bad_size_returns_error() {
        let data = vec![0u8; 65]; // not a multiple of 64
        let result = read_directory(&data);
        assert!(result.is_err());
    }

    #[test]
    fn empty_directory() {
        let dir = SegmentDirectory { entries: vec![] };
        let bytes = write_directory(&dir);
        assert!(bytes.is_empty());
        let decoded = read_directory(&bytes).unwrap();
        assert!(decoded.is_empty());
    }
}
