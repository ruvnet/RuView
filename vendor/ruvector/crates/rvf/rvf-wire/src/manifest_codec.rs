//! Level 0 root manifest codec.
//!
//! The root manifest is always exactly 4096 bytes, found at the tail of the
//! file (or at the tail of a MANIFEST_SEG payload). It contains hotset
//! pointers for instant boot and a CRC32C checksum at the last 4 bytes.

use crate::hash::compute_crc32c;
use rvf_types::{ErrorCode, RvfError, ROOT_MANIFEST_MAGIC, ROOT_MANIFEST_SIZE};

/// Parsed Level 0 root manifest.
#[derive(Clone, Debug)]
pub struct Level0Root {
    pub magic: u32,
    pub version: u16,
    pub flags: u16,
    pub l1_manifest_offset: u64,
    pub l1_manifest_length: u64,
    pub total_vector_count: u64,
    pub dimension: u16,
    pub base_dtype: u8,
    pub profile_id: u8,
    pub epoch: u32,
    pub created_ns: u64,
    pub modified_ns: u64,
    // Hotset pointers
    pub entrypoint_seg_offset: u64,
    pub entrypoint_block_offset: u32,
    pub entrypoint_count: u32,
    pub toplayer_seg_offset: u64,
    pub toplayer_block_offset: u32,
    pub toplayer_node_count: u32,
    pub centroid_seg_offset: u64,
    pub centroid_block_offset: u32,
    pub centroid_count: u32,
    pub quantdict_seg_offset: u64,
    pub quantdict_block_offset: u32,
    pub quantdict_size: u32,
    pub hot_cache_seg_offset: u64,
    pub hot_cache_block_offset: u32,
    pub hot_cache_vector_count: u32,
    pub prefetch_map_offset: u64,
    pub prefetch_map_entries: u32,
    // Checksum
    pub root_checksum: u32,
}

impl Default for Level0Root {
    fn default() -> Self {
        Self {
            magic: ROOT_MANIFEST_MAGIC,
            version: 1,
            flags: 0,
            l1_manifest_offset: 0,
            l1_manifest_length: 0,
            total_vector_count: 0,
            dimension: 0,
            base_dtype: 0,
            profile_id: 0,
            epoch: 0,
            created_ns: 0,
            modified_ns: 0,
            entrypoint_seg_offset: 0,
            entrypoint_block_offset: 0,
            entrypoint_count: 0,
            toplayer_seg_offset: 0,
            toplayer_block_offset: 0,
            toplayer_node_count: 0,
            centroid_seg_offset: 0,
            centroid_block_offset: 0,
            centroid_count: 0,
            quantdict_seg_offset: 0,
            quantdict_block_offset: 0,
            quantdict_size: 0,
            hot_cache_seg_offset: 0,
            hot_cache_block_offset: 0,
            hot_cache_vector_count: 0,
            prefetch_map_offset: 0,
            prefetch_map_entries: 0,
            root_checksum: 0,
        }
    }
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}

fn write_u16_le(buf: &mut [u8], offset: usize, val: u16) {
    buf[offset..offset + 2].copy_from_slice(&val.to_le_bytes());
}

fn write_u32_le(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
}

fn write_u64_le(buf: &mut [u8], offset: usize, val: u64) {
    buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
}

/// Read and parse a Level 0 root manifest from a 4096-byte slice.
///
/// Validates the magic (`RVM0`) and CRC32C checksum.
///
/// # Errors
///
/// - `InvalidManifest` if the magic is wrong or the checksum doesn't match.
/// - `TruncatedSegment` if `data` is shorter than 4096 bytes.
pub fn read_root_manifest(data: &[u8]) -> Result<Level0Root, RvfError> {
    if data.len() < ROOT_MANIFEST_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let magic = read_u32_le(data, 0x000);
    if magic != ROOT_MANIFEST_MAGIC {
        return Err(RvfError::Code(ErrorCode::InvalidManifest));
    }

    // Verify CRC32C: checksum covers bytes 0x000..0xFFC
    let stored_checksum = read_u32_le(data, 0xFFC);
    let computed_checksum = compute_crc32c(&data[..0xFFC]);
    if stored_checksum != computed_checksum {
        return Err(RvfError::Code(ErrorCode::InvalidChecksum));
    }

    Ok(Level0Root {
        magic,
        version: read_u16_le(data, 0x004),
        flags: read_u16_le(data, 0x006),
        l1_manifest_offset: read_u64_le(data, 0x008),
        l1_manifest_length: read_u64_le(data, 0x010),
        total_vector_count: read_u64_le(data, 0x018),
        dimension: read_u16_le(data, 0x020),
        base_dtype: data[0x022],
        profile_id: data[0x023],
        epoch: read_u32_le(data, 0x024),
        created_ns: read_u64_le(data, 0x028),
        modified_ns: read_u64_le(data, 0x030),
        // Hotset pointers
        entrypoint_seg_offset: read_u64_le(data, 0x038),
        entrypoint_block_offset: read_u32_le(data, 0x040),
        entrypoint_count: read_u32_le(data, 0x044),
        toplayer_seg_offset: read_u64_le(data, 0x048),
        toplayer_block_offset: read_u32_le(data, 0x050),
        toplayer_node_count: read_u32_le(data, 0x054),
        centroid_seg_offset: read_u64_le(data, 0x058),
        centroid_block_offset: read_u32_le(data, 0x060),
        centroid_count: read_u32_le(data, 0x064),
        quantdict_seg_offset: read_u64_le(data, 0x068),
        quantdict_block_offset: read_u32_le(data, 0x070),
        quantdict_size: read_u32_le(data, 0x074),
        hot_cache_seg_offset: read_u64_le(data, 0x078),
        hot_cache_block_offset: read_u32_le(data, 0x080),
        hot_cache_vector_count: read_u32_le(data, 0x084),
        prefetch_map_offset: read_u64_le(data, 0x088),
        prefetch_map_entries: read_u32_le(data, 0x090),
        root_checksum: stored_checksum,
    })
}

/// Serialize a Level 0 root manifest into a 4096-byte array.
///
/// Computes and stores the CRC32C checksum at offset 0xFFC.
pub fn write_root_manifest(root: &Level0Root) -> [u8; ROOT_MANIFEST_SIZE] {
    let mut buf = [0u8; ROOT_MANIFEST_SIZE];

    write_u32_le(&mut buf, 0x000, root.magic);
    write_u16_le(&mut buf, 0x004, root.version);
    write_u16_le(&mut buf, 0x006, root.flags);
    write_u64_le(&mut buf, 0x008, root.l1_manifest_offset);
    write_u64_le(&mut buf, 0x010, root.l1_manifest_length);
    write_u64_le(&mut buf, 0x018, root.total_vector_count);
    write_u16_le(&mut buf, 0x020, root.dimension);
    buf[0x022] = root.base_dtype;
    buf[0x023] = root.profile_id;
    write_u32_le(&mut buf, 0x024, root.epoch);
    write_u64_le(&mut buf, 0x028, root.created_ns);
    write_u64_le(&mut buf, 0x030, root.modified_ns);
    // Hotset pointers
    write_u64_le(&mut buf, 0x038, root.entrypoint_seg_offset);
    write_u32_le(&mut buf, 0x040, root.entrypoint_block_offset);
    write_u32_le(&mut buf, 0x044, root.entrypoint_count);
    write_u64_le(&mut buf, 0x048, root.toplayer_seg_offset);
    write_u32_le(&mut buf, 0x050, root.toplayer_block_offset);
    write_u32_le(&mut buf, 0x054, root.toplayer_node_count);
    write_u64_le(&mut buf, 0x058, root.centroid_seg_offset);
    write_u32_le(&mut buf, 0x060, root.centroid_block_offset);
    write_u32_le(&mut buf, 0x064, root.centroid_count);
    write_u64_le(&mut buf, 0x068, root.quantdict_seg_offset);
    write_u32_le(&mut buf, 0x070, root.quantdict_block_offset);
    write_u32_le(&mut buf, 0x074, root.quantdict_size);
    write_u64_le(&mut buf, 0x078, root.hot_cache_seg_offset);
    write_u32_le(&mut buf, 0x080, root.hot_cache_block_offset);
    write_u32_le(&mut buf, 0x084, root.hot_cache_vector_count);
    write_u64_le(&mut buf, 0x088, root.prefetch_map_offset);
    write_u32_le(&mut buf, 0x090, root.prefetch_map_entries);

    // Compute and write CRC32C
    let checksum = compute_crc32c(&buf[..0xFFC]);
    write_u32_le(&mut buf, 0xFFC, checksum);

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_default() {
        let root = Level0Root::default();
        let buf = write_root_manifest(&root);
        assert_eq!(buf.len(), ROOT_MANIFEST_SIZE);
        let decoded = read_root_manifest(&buf).unwrap();
        assert_eq!(decoded.magic, ROOT_MANIFEST_MAGIC);
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.total_vector_count, 0);
    }

    #[test]
    fn round_trip_with_values() {
        let root = Level0Root {
            magic: ROOT_MANIFEST_MAGIC,
            version: 1,
            flags: 0,
            l1_manifest_offset: 4096,
            l1_manifest_length: 2048,
            total_vector_count: 1_000_000,
            dimension: 384,
            base_dtype: 1, // f16
            profile_id: 2, // text
            epoch: 42,
            created_ns: 1700000000000000000,
            modified_ns: 1700000001000000000,
            entrypoint_seg_offset: 8192,
            entrypoint_block_offset: 64,
            entrypoint_count: 10,
            toplayer_seg_offset: 16384,
            toplayer_block_offset: 128,
            toplayer_node_count: 100,
            centroid_seg_offset: 32768,
            centroid_block_offset: 0,
            centroid_count: 256,
            quantdict_seg_offset: 65536,
            quantdict_block_offset: 0,
            quantdict_size: 4096,
            hot_cache_seg_offset: 131072,
            hot_cache_block_offset: 0,
            hot_cache_vector_count: 1000,
            prefetch_map_offset: 262144,
            prefetch_map_entries: 50,
            root_checksum: 0, // will be computed
        };
        let buf = write_root_manifest(&root);
        let decoded = read_root_manifest(&buf).unwrap();
        assert_eq!(decoded.total_vector_count, 1_000_000);
        assert_eq!(decoded.dimension, 384);
        assert_eq!(decoded.base_dtype, 1);
        assert_eq!(decoded.epoch, 42);
        assert_eq!(decoded.entrypoint_count, 10);
        assert_eq!(decoded.hot_cache_vector_count, 1000);
    }

    #[test]
    fn invalid_magic_rejected() {
        let mut buf = [0u8; ROOT_MANIFEST_SIZE];
        buf[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        let result = read_root_manifest(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn corrupted_checksum_rejected() {
        let root = Level0Root::default();
        let mut buf = write_root_manifest(&root);
        // Corrupt one byte in the data area
        buf[0x020] ^= 0xFF;
        let result = read_root_manifest(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn truncated_data_rejected() {
        let result = read_root_manifest(&[0u8; 100]);
        assert!(result.is_err());
    }
}
