//! Level 0 Root Manifest — fixed 4096 bytes at EOF.
//!
//! Provides read/write/validate functions that operate on raw byte arrays,
//! using the `Level0Root` repr(C) struct from `rvf_types`.

use rvf_types::{
    CentroidPtr, EntrypointPtr, ErrorCode, FileIdentity, HotCachePtr, Level0Root, PrefetchMapPtr,
    QuantDictPtr, RvfError, TopLayerPtr, ROOT_MANIFEST_MAGIC, ROOT_MANIFEST_SIZE,
};

// ---------- helpers for little-endian read/write ----------

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

// ---------- Offsets matching the spec ----------

const OFF_MAGIC: usize = 0x000;
const OFF_VERSION: usize = 0x004;
const OFF_FLAGS: usize = 0x006;
const OFF_L1_OFFSET: usize = 0x008;
const OFF_L1_LENGTH: usize = 0x010;
const OFF_TOTAL_VEC: usize = 0x018;
const OFF_DIM: usize = 0x020;
const OFF_DTYPE: usize = 0x022;
const OFF_PROFILE: usize = 0x023;
const OFF_EPOCH: usize = 0x024;
const OFF_CREATED: usize = 0x028;
const OFF_MODIFIED: usize = 0x030;

const OFF_ENTRYPOINT: usize = 0x038;
const OFF_TOPLAYER: usize = 0x048;
const OFF_CENTROID: usize = 0x058;
const OFF_QUANTDICT: usize = 0x068;
const OFF_HOT_CACHE: usize = 0x078;
const OFF_PREFETCH: usize = 0x088;

const OFF_SIG_ALGO: usize = 0x098;
const OFF_SIG_LEN: usize = 0x09A;
const OFF_SIGNATURE: usize = 0x09C;

// FileIdentity offsets within the reserved area (0xF00..0xF44)
const OFF_FILE_ID: usize = 0xF00;
const OFF_PARENT_ID: usize = 0xF10;
const OFF_PARENT_HASH: usize = 0xF20;
const OFF_LINEAGE_DEPTH: usize = 0xF40;

// COW pointer offsets within the reserved area (0xF44..0xF84)
// These follow FileIdentity and are backward-compatible (zeros = no COW).
const OFF_COW_MAP_OFFSET: usize = 0xF44;
const OFF_COW_MAP_GENERATION: usize = 0xF4C;
const OFF_MEMBERSHIP_OFFSET: usize = 0xF50;
const OFF_MEMBERSHIP_GENERATION: usize = 0xF58;
const OFF_SNAPSHOT_EPOCH: usize = 0xF5C;
const OFF_DOUBLE_ROOT_GENERATION: usize = 0xF60;
const OFF_DOUBLE_ROOT_HASH: usize = 0xF64;

const OFF_CHECKSUM: usize = 0xFFC;

/// Deserialize a Level 0 root manifest from exactly 4096 bytes.
pub fn read_level0(data: &[u8; ROOT_MANIFEST_SIZE]) -> Result<Level0Root, RvfError> {
    let magic = read_u32_le(data, OFF_MAGIC);
    if magic != ROOT_MANIFEST_MAGIC {
        return Err(RvfError::BadMagic {
            expected: ROOT_MANIFEST_MAGIC,
            got: magic,
        });
    }

    let stored_crc = read_u32_le(data, OFF_CHECKSUM);
    let computed_crc = crc32c::crc32c(&data[..OFF_CHECKSUM]);
    if stored_crc != computed_crc {
        return Err(RvfError::Code(ErrorCode::InvalidChecksum));
    }

    let sig_length = read_u16_le(data, OFF_SIG_LEN);

    let mut root = Level0Root::zeroed();
    root.magic = magic;
    root.version = read_u16_le(data, OFF_VERSION);
    root.flags = read_u16_le(data, OFF_FLAGS);
    root.l1_manifest_offset = read_u64_le(data, OFF_L1_OFFSET);
    root.l1_manifest_length = read_u64_le(data, OFF_L1_LENGTH);
    root.total_vector_count = read_u64_le(data, OFF_TOTAL_VEC);
    root.dimension = read_u16_le(data, OFF_DIM);
    root.base_dtype = data[OFF_DTYPE];
    root.profile_id = data[OFF_PROFILE];
    root.epoch = read_u32_le(data, OFF_EPOCH);
    root.created_ns = read_u64_le(data, OFF_CREATED);
    root.modified_ns = read_u64_le(data, OFF_MODIFIED);

    root.entrypoint = EntrypointPtr {
        seg_offset: read_u64_le(data, OFF_ENTRYPOINT),
        block_offset: read_u32_le(data, OFF_ENTRYPOINT + 8),
        count: read_u32_le(data, OFF_ENTRYPOINT + 12),
    };
    root.toplayer = TopLayerPtr {
        seg_offset: read_u64_le(data, OFF_TOPLAYER),
        block_offset: read_u32_le(data, OFF_TOPLAYER + 8),
        node_count: read_u32_le(data, OFF_TOPLAYER + 12),
    };
    root.centroid = CentroidPtr {
        seg_offset: read_u64_le(data, OFF_CENTROID),
        block_offset: read_u32_le(data, OFF_CENTROID + 8),
        count: read_u32_le(data, OFF_CENTROID + 12),
    };
    root.quantdict = QuantDictPtr {
        seg_offset: read_u64_le(data, OFF_QUANTDICT),
        block_offset: read_u32_le(data, OFF_QUANTDICT + 8),
        size: read_u32_le(data, OFF_QUANTDICT + 12),
    };
    root.hot_cache = HotCachePtr {
        seg_offset: read_u64_le(data, OFF_HOT_CACHE),
        block_offset: read_u32_le(data, OFF_HOT_CACHE + 8),
        vector_count: read_u32_le(data, OFF_HOT_CACHE + 12),
    };
    root.prefetch_map = PrefetchMapPtr {
        offset: read_u64_le(data, OFF_PREFETCH),
        entries: read_u32_le(data, OFF_PREFETCH + 8),
        _pad: 0,
    };

    root.sig_algo = read_u16_le(data, OFF_SIG_ALGO);
    root.sig_length = sig_length;

    let sig_len = sig_length as usize;
    let sig_max = Level0Root::SIG_BUF_SIZE.min(sig_len);
    root.signature_buf[..sig_max].copy_from_slice(&data[OFF_SIGNATURE..OFF_SIGNATURE + sig_max]);

    // Read FileIdentity from the reserved area
    let mut file_id = [0u8; 16];
    file_id.copy_from_slice(&data[OFF_FILE_ID..OFF_FILE_ID + 16]);
    let mut parent_id = [0u8; 16];
    parent_id.copy_from_slice(&data[OFF_PARENT_ID..OFF_PARENT_ID + 16]);
    let mut parent_hash = [0u8; 32];
    parent_hash.copy_from_slice(&data[OFF_PARENT_HASH..OFF_PARENT_HASH + 32]);
    let lineage_depth = read_u32_le(data, OFF_LINEAGE_DEPTH);

    let fi = FileIdentity {
        file_id,
        parent_id,
        parent_hash,
        lineage_depth,
    };
    let fi_bytes = fi.to_bytes();
    root.reserved[..68].copy_from_slice(&fi_bytes);

    // Read COW pointers from the reserved area (backward-compatible: zeros = no COW).
    // These are stored as raw bytes in reserved[68..136].
    let cow_map_offset = read_u64_le(data, OFF_COW_MAP_OFFSET);
    let cow_map_generation = read_u32_le(data, OFF_COW_MAP_GENERATION);
    let membership_offset = read_u64_le(data, OFF_MEMBERSHIP_OFFSET);
    let membership_generation = read_u32_le(data, OFF_MEMBERSHIP_GENERATION);
    let snapshot_epoch = read_u32_le(data, OFF_SNAPSHOT_EPOCH);
    let double_root_generation = read_u32_le(data, OFF_DOUBLE_ROOT_GENERATION);
    let mut double_root_hash = [0u8; 32];
    double_root_hash.copy_from_slice(&data[OFF_DOUBLE_ROOT_HASH..OFF_DOUBLE_ROOT_HASH + 32]);

    // Pack COW pointers into reserved[68..136]
    let cow_off = 68;
    root.reserved[cow_off..cow_off + 8].copy_from_slice(&cow_map_offset.to_le_bytes());
    root.reserved[cow_off + 8..cow_off + 12].copy_from_slice(&cow_map_generation.to_le_bytes());
    root.reserved[cow_off + 12..cow_off + 20].copy_from_slice(&membership_offset.to_le_bytes());
    root.reserved[cow_off + 20..cow_off + 24].copy_from_slice(&membership_generation.to_le_bytes());
    root.reserved[cow_off + 24..cow_off + 28].copy_from_slice(&snapshot_epoch.to_le_bytes());
    root.reserved[cow_off + 28..cow_off + 32]
        .copy_from_slice(&double_root_generation.to_le_bytes());
    root.reserved[cow_off + 32..cow_off + 64].copy_from_slice(&double_root_hash);

    root.root_checksum = stored_crc;

    Ok(root)
}

/// Serialize a Level 0 root manifest into exactly 4096 bytes.
///
/// The `root_checksum` field on the input is ignored; the checksum is
/// computed over bytes 0x000..0xFFC and written at offset 0xFFC.
pub fn write_level0(root: &Level0Root) -> [u8; ROOT_MANIFEST_SIZE] {
    let mut buf = [0u8; ROOT_MANIFEST_SIZE];

    write_u32_le(&mut buf, OFF_MAGIC, root.magic);
    write_u16_le(&mut buf, OFF_VERSION, root.version);
    write_u16_le(&mut buf, OFF_FLAGS, root.flags);
    write_u64_le(&mut buf, OFF_L1_OFFSET, root.l1_manifest_offset);
    write_u64_le(&mut buf, OFF_L1_LENGTH, root.l1_manifest_length);
    write_u64_le(&mut buf, OFF_TOTAL_VEC, root.total_vector_count);
    write_u16_le(&mut buf, OFF_DIM, root.dimension);
    buf[OFF_DTYPE] = root.base_dtype;
    buf[OFF_PROFILE] = root.profile_id;
    write_u32_le(&mut buf, OFF_EPOCH, root.epoch);
    write_u64_le(&mut buf, OFF_CREATED, root.created_ns);
    write_u64_le(&mut buf, OFF_MODIFIED, root.modified_ns);

    // Entrypoint (16 bytes)
    write_u64_le(&mut buf, OFF_ENTRYPOINT, root.entrypoint.seg_offset);
    write_u32_le(&mut buf, OFF_ENTRYPOINT + 8, root.entrypoint.block_offset);
    write_u32_le(&mut buf, OFF_ENTRYPOINT + 12, root.entrypoint.count);

    // Top layer (16 bytes)
    write_u64_le(&mut buf, OFF_TOPLAYER, root.toplayer.seg_offset);
    write_u32_le(&mut buf, OFF_TOPLAYER + 8, root.toplayer.block_offset);
    write_u32_le(&mut buf, OFF_TOPLAYER + 12, root.toplayer.node_count);

    // Centroid (16 bytes)
    write_u64_le(&mut buf, OFF_CENTROID, root.centroid.seg_offset);
    write_u32_le(&mut buf, OFF_CENTROID + 8, root.centroid.block_offset);
    write_u32_le(&mut buf, OFF_CENTROID + 12, root.centroid.count);

    // Quant dict (16 bytes)
    write_u64_le(&mut buf, OFF_QUANTDICT, root.quantdict.seg_offset);
    write_u32_le(&mut buf, OFF_QUANTDICT + 8, root.quantdict.block_offset);
    write_u32_le(&mut buf, OFF_QUANTDICT + 12, root.quantdict.size);

    // Hot cache (16 bytes)
    write_u64_le(&mut buf, OFF_HOT_CACHE, root.hot_cache.seg_offset);
    write_u32_le(&mut buf, OFF_HOT_CACHE + 8, root.hot_cache.block_offset);
    write_u32_le(&mut buf, OFF_HOT_CACHE + 12, root.hot_cache.vector_count);

    // Prefetch map (12 bytes: u64 offset + u32 entries)
    write_u64_le(&mut buf, OFF_PREFETCH, root.prefetch_map.offset);
    write_u32_le(&mut buf, OFF_PREFETCH + 8, root.prefetch_map.entries);

    write_u16_le(&mut buf, OFF_SIG_ALGO, root.sig_algo);
    let sig_len = (root.sig_length as usize).min(Level0Root::SIG_BUF_SIZE);
    write_u16_le(&mut buf, OFF_SIG_LEN, sig_len as u16);
    buf[OFF_SIGNATURE..OFF_SIGNATURE + sig_len].copy_from_slice(&root.signature_buf[..sig_len]);

    // Write FileIdentity from reserved area into the buffer
    if root.reserved.len() >= 68 {
        let fi = FileIdentity::from_bytes(root.reserved[..68].try_into().unwrap());
        buf[OFF_FILE_ID..OFF_FILE_ID + 16].copy_from_slice(&fi.file_id);
        buf[OFF_PARENT_ID..OFF_PARENT_ID + 16].copy_from_slice(&fi.parent_id);
        buf[OFF_PARENT_HASH..OFF_PARENT_HASH + 32].copy_from_slice(&fi.parent_hash);
        write_u32_le(&mut buf, OFF_LINEAGE_DEPTH, fi.lineage_depth);
    }

    // Write COW pointers from reserved[68..136] into the buffer
    // Backward-compatible: zeros mean no COW.
    if root.reserved.len() >= 132 {
        let cow_off = 68;
        buf[OFF_COW_MAP_OFFSET..OFF_COW_MAP_OFFSET + 8]
            .copy_from_slice(&root.reserved[cow_off..cow_off + 8]);
        buf[OFF_COW_MAP_GENERATION..OFF_COW_MAP_GENERATION + 4]
            .copy_from_slice(&root.reserved[cow_off + 8..cow_off + 12]);
        buf[OFF_MEMBERSHIP_OFFSET..OFF_MEMBERSHIP_OFFSET + 8]
            .copy_from_slice(&root.reserved[cow_off + 12..cow_off + 20]);
        buf[OFF_MEMBERSHIP_GENERATION..OFF_MEMBERSHIP_GENERATION + 4]
            .copy_from_slice(&root.reserved[cow_off + 20..cow_off + 24]);
        buf[OFF_SNAPSHOT_EPOCH..OFF_SNAPSHOT_EPOCH + 4]
            .copy_from_slice(&root.reserved[cow_off + 24..cow_off + 28]);
        buf[OFF_DOUBLE_ROOT_GENERATION..OFF_DOUBLE_ROOT_GENERATION + 4]
            .copy_from_slice(&root.reserved[cow_off + 28..cow_off + 32]);
        buf[OFF_DOUBLE_ROOT_HASH..OFF_DOUBLE_ROOT_HASH + 32]
            .copy_from_slice(&root.reserved[cow_off + 32..cow_off + 64]);
    }

    // CRC32C over first 4092 bytes
    let crc = crc32c::crc32c(&buf[..OFF_CHECKSUM]);
    write_u32_le(&mut buf, OFF_CHECKSUM, crc);

    buf
}

/// Fast validation: check magic + CRC32C without full deserialization.
pub fn validate_level0(data: &[u8; ROOT_MANIFEST_SIZE]) -> bool {
    let magic = read_u32_le(data, OFF_MAGIC);
    if magic != ROOT_MANIFEST_MAGIC {
        return false;
    }
    let stored_crc = read_u32_le(data, OFF_CHECKSUM);
    let computed_crc = crc32c::crc32c(&data[..OFF_CHECKSUM]);
    stored_crc == computed_crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_root() -> Level0Root {
        let mut root = Level0Root::zeroed();
        root.version = 1;
        root.flags = 0x0004; // SIGNED
        root.l1_manifest_offset = 0x1_0000;
        root.l1_manifest_length = 0x2000;
        root.total_vector_count = 10_000_000;
        root.dimension = 384;
        root.base_dtype = 1; // f16
        root.profile_id = 2; // text
        root.epoch = 42;
        root.created_ns = 1_700_000_000_000_000_000;
        root.modified_ns = 1_700_000_001_000_000_000;
        root.entrypoint = EntrypointPtr {
            seg_offset: 0x1000,
            block_offset: 64,
            count: 3,
        };
        root.toplayer = TopLayerPtr {
            seg_offset: 0x2000,
            block_offset: 128,
            node_count: 500,
        };
        root.centroid = CentroidPtr {
            seg_offset: 0x3000,
            block_offset: 0,
            count: 256,
        };
        root.quantdict = QuantDictPtr {
            seg_offset: 0x4000,
            block_offset: 0,
            size: 8192,
        };
        root.hot_cache = HotCachePtr {
            seg_offset: 0x5000,
            block_offset: 0,
            vector_count: 1000,
        };
        root.prefetch_map = PrefetchMapPtr {
            offset: 0x6000,
            entries: 200,
            _pad: 0,
        };
        root.sig_algo = 0; // Ed25519
        root.sig_length = 4;
        root.signature_buf[0] = 0xDE;
        root.signature_buf[1] = 0xAD;
        root.signature_buf[2] = 0xBE;
        root.signature_buf[3] = 0xEF;
        root
    }

    #[test]
    fn round_trip() {
        let original = sample_root();
        let bytes = write_level0(&original);
        let decoded = read_level0(&bytes).expect("read_level0 should succeed");

        assert_eq!(decoded.magic, original.magic);
        assert_eq!(decoded.version, original.version);
        assert_eq!(decoded.flags, original.flags);
        assert_eq!(decoded.l1_manifest_offset, original.l1_manifest_offset);
        assert_eq!(decoded.l1_manifest_length, original.l1_manifest_length);
        assert_eq!(decoded.total_vector_count, original.total_vector_count);
        assert_eq!(decoded.dimension, original.dimension);
        assert_eq!(decoded.base_dtype, original.base_dtype);
        assert_eq!(decoded.profile_id, original.profile_id);
        assert_eq!(decoded.epoch, original.epoch);
        assert_eq!(decoded.created_ns, original.created_ns);
        assert_eq!(decoded.modified_ns, original.modified_ns);

        assert_eq!(
            decoded.entrypoint.seg_offset,
            original.entrypoint.seg_offset
        );
        assert_eq!(
            decoded.entrypoint.block_offset,
            original.entrypoint.block_offset
        );
        assert_eq!(decoded.entrypoint.count, original.entrypoint.count);

        assert_eq!(decoded.toplayer.seg_offset, original.toplayer.seg_offset);
        assert_eq!(decoded.toplayer.node_count, original.toplayer.node_count);

        assert_eq!(decoded.centroid.seg_offset, original.centroid.seg_offset);
        assert_eq!(decoded.centroid.count, original.centroid.count);

        assert_eq!(decoded.quantdict.seg_offset, original.quantdict.seg_offset);
        assert_eq!(decoded.quantdict.size, original.quantdict.size);

        assert_eq!(decoded.hot_cache.seg_offset, original.hot_cache.seg_offset);
        assert_eq!(
            decoded.hot_cache.vector_count,
            original.hot_cache.vector_count
        );

        assert_eq!(decoded.prefetch_map.offset, original.prefetch_map.offset);
        assert_eq!(decoded.prefetch_map.entries, original.prefetch_map.entries);

        assert_eq!(decoded.sig_algo, original.sig_algo);
        assert_eq!(decoded.sig_length, original.sig_length);
        assert_eq!(decoded.signature_buf[..4], original.signature_buf[..4]);
    }

    #[test]
    fn crc_detects_corruption() {
        let root = sample_root();
        let mut bytes = write_level0(&root);
        assert!(validate_level0(&bytes));

        // Corrupt a byte in the middle
        bytes[0x050] ^= 0xFF;
        assert!(!validate_level0(&bytes));

        // read_level0 should also fail
        assert!(read_level0(&bytes).is_err());
    }

    #[test]
    fn invalid_magic_rejected() {
        let mut bytes = write_level0(&sample_root());
        // Overwrite magic
        bytes[0] = 0x00;
        bytes[1] = 0x00;
        bytes[2] = 0x00;
        bytes[3] = 0x00;
        // Fix CRC so only magic check fails
        let crc = crc32c::crc32c(&bytes[..OFF_CHECKSUM]);
        write_u32_le(&mut bytes, OFF_CHECKSUM, crc);

        let err = read_level0(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, got } => {
                assert_eq!(expected, ROOT_MANIFEST_MAGIC);
                assert_eq!(got, 0);
            }
            other => panic!("expected BadMagic, got {:?}", other),
        }
    }

    #[test]
    fn default_root_round_trips() {
        let root = Level0Root::zeroed();
        let bytes = write_level0(&root);
        let decoded = read_level0(&bytes).unwrap();
        assert_eq!(decoded.magic, ROOT_MANIFEST_MAGIC);
        assert_eq!(decoded.total_vector_count, 0);
        assert_eq!(decoded.dimension, 0);
    }

    #[test]
    fn output_is_exactly_4096_bytes() {
        let bytes = write_level0(&Level0Root::zeroed());
        assert_eq!(bytes.len(), 4096);
    }

    #[test]
    fn cow_pointers_round_trip() {
        let mut root = sample_root();

        // Set COW pointers in the reserved area (offsets 68..132)
        let cow_off = 68;
        let cow_map_offset: u64 = 0x1234_5678_9ABC_DEF0;
        let cow_map_generation: u32 = 42;
        let membership_offset: u64 = 0xFEDC_BA98_7654_3210;
        let membership_generation: u32 = 7;
        let snapshot_epoch: u32 = 100;
        let double_root_generation: u32 = 3;
        let double_root_hash = [0xEE; 32];

        root.reserved[cow_off..cow_off + 8].copy_from_slice(&cow_map_offset.to_le_bytes());
        root.reserved[cow_off + 8..cow_off + 12].copy_from_slice(&cow_map_generation.to_le_bytes());
        root.reserved[cow_off + 12..cow_off + 20].copy_from_slice(&membership_offset.to_le_bytes());
        root.reserved[cow_off + 20..cow_off + 24]
            .copy_from_slice(&membership_generation.to_le_bytes());
        root.reserved[cow_off + 24..cow_off + 28].copy_from_slice(&snapshot_epoch.to_le_bytes());
        root.reserved[cow_off + 28..cow_off + 32]
            .copy_from_slice(&double_root_generation.to_le_bytes());
        root.reserved[cow_off + 32..cow_off + 64].copy_from_slice(&double_root_hash);

        let bytes = write_level0(&root);
        let decoded = read_level0(&bytes).expect("read_level0 should succeed");

        // Verify COW pointers survived round-trip
        let d_cow_off = 68;
        let d_cow_map_offset = u64::from_le_bytes(
            decoded.reserved[d_cow_off..d_cow_off + 8]
                .try_into()
                .unwrap(),
        );
        let d_cow_map_generation = u32::from_le_bytes(
            decoded.reserved[d_cow_off + 8..d_cow_off + 12]
                .try_into()
                .unwrap(),
        );
        let d_membership_offset = u64::from_le_bytes(
            decoded.reserved[d_cow_off + 12..d_cow_off + 20]
                .try_into()
                .unwrap(),
        );
        let d_membership_generation = u32::from_le_bytes(
            decoded.reserved[d_cow_off + 20..d_cow_off + 24]
                .try_into()
                .unwrap(),
        );
        let d_snapshot_epoch = u32::from_le_bytes(
            decoded.reserved[d_cow_off + 24..d_cow_off + 28]
                .try_into()
                .unwrap(),
        );
        let d_double_root_generation = u32::from_le_bytes(
            decoded.reserved[d_cow_off + 28..d_cow_off + 32]
                .try_into()
                .unwrap(),
        );
        let d_double_root_hash = &decoded.reserved[d_cow_off + 32..d_cow_off + 64];

        assert_eq!(d_cow_map_offset, cow_map_offset);
        assert_eq!(d_cow_map_generation, cow_map_generation);
        assert_eq!(d_membership_offset, membership_offset);
        assert_eq!(d_membership_generation, membership_generation);
        assert_eq!(d_snapshot_epoch, snapshot_epoch);
        assert_eq!(d_double_root_generation, double_root_generation);
        assert_eq!(d_double_root_hash, &double_root_hash[..]);
    }

    #[test]
    fn cow_pointers_default_to_zero() {
        // Verify that a root with no COW pointers still round-trips correctly
        let root = Level0Root::zeroed();
        let bytes = write_level0(&root);
        let decoded = read_level0(&bytes).unwrap();

        let cow_off = 68;
        let cow_map_offset =
            u64::from_le_bytes(decoded.reserved[cow_off..cow_off + 8].try_into().unwrap());
        let snapshot_epoch = u32::from_le_bytes(
            decoded.reserved[cow_off + 24..cow_off + 28]
                .try_into()
                .unwrap(),
        );

        assert_eq!(cow_map_offset, 0);
        assert_eq!(snapshot_epoch, 0);
    }
}
