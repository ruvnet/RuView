//! Disk-backed BlockIO and MetaLog implementations.
//!
//! Gated behind the `persistence` feature flag. Uses raw file I/O
//! with a simple binary format. No external dependencies.

#![cfg(feature = "persistence")]

use crate::store::{
    BlockIO, BlockKey, BlockMeta, DType, MetaLog, ReconstructPolicy, StoreError, Tier,
};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Fixed size of a single encoded [`BlockMeta`] record in bytes.
///
/// Layout (all little-endian):
///
/// | Offset | Size | Field           |
/// |--------|------|-----------------|
/// | 0      | 16   | tensor_id       |
/// | 16     | 4    | block_index     |
/// | 20     | 1    | dtype           |
/// | 21     | 1    | tier            |
/// | 22     | 1    | bits            |
/// | 23     | 4    | scale           |
/// | 27     | 2    | zero_point      |
/// | 29     | 8    | created_at      |
/// | 37     | 8    | last_access_at  |
/// | 45     | 4    | access_count    |
/// | 49     | 4    | ema_rate        |
/// | 53     | 8    | window          |
/// | 61     | 4    | checksum        |
/// | 65     | 1    | reconstruct     |
/// | 66     | 4    | tier_age        |
/// | 70     | 1    | has_lineage     |
/// | 71     | 16   | lineage_parent  |
/// | 87     | 4    | block_bytes     |
const RECORD_SIZE: usize = 91;

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Serialize a [`BlockMeta`] into a fixed-size byte vector.
///
/// The encoding uses little-endian byte order for all multi-byte fields
/// and occupies exactly [`RECORD_SIZE`] bytes.
pub fn encode_meta(meta: &BlockMeta) -> Vec<u8> {
    let mut buf = Vec::with_capacity(RECORD_SIZE);

    // key
    buf.extend_from_slice(&meta.key.tensor_id.to_le_bytes());
    buf.extend_from_slice(&meta.key.block_index.to_le_bytes());

    // scalar metadata
    buf.push(meta.dtype as u8);
    buf.push(meta.tier as u8);
    buf.push(meta.bits);
    buf.extend_from_slice(&meta.scale.to_le_bytes());
    buf.extend_from_slice(&meta.zero_point.to_le_bytes());

    // timestamps and counters
    buf.extend_from_slice(&meta.created_at.to_le_bytes());
    buf.extend_from_slice(&meta.last_access_at.to_le_bytes());
    buf.extend_from_slice(&meta.access_count.to_le_bytes());
    buf.extend_from_slice(&meta.ema_rate.to_le_bytes());
    buf.extend_from_slice(&meta.window.to_le_bytes());
    buf.extend_from_slice(&meta.checksum.to_le_bytes());

    // policy and age
    buf.push(meta.reconstruct as u8);
    buf.extend_from_slice(&meta.tier_age.to_le_bytes());

    // optional lineage parent
    match meta.lineage_parent {
        Some(parent) => {
            buf.push(1);
            buf.extend_from_slice(&parent.to_le_bytes());
        }
        None => {
            buf.push(0);
            buf.extend_from_slice(&0u128.to_le_bytes());
        }
    }

    // payload size
    buf.extend_from_slice(&meta.block_bytes.to_le_bytes());

    debug_assert_eq!(buf.len(), RECORD_SIZE);
    buf
}

/// Deserialize a [`BlockMeta`] from a byte slice of at least [`RECORD_SIZE`] bytes.
///
/// Returns [`StoreError::InvalidData`] if the slice is too short or
/// contains invalid enum discriminants.
pub fn decode_meta(bytes: &[u8]) -> Result<BlockMeta, StoreError> {
    if bytes.len() < RECORD_SIZE {
        return Err(StoreError::InvalidData);
    }

    let tensor_id = u128::from_le_bytes(
        bytes[0..16]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let block_index = u32::from_le_bytes(
        bytes[16..20]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );

    let dtype = match bytes[20] {
        0 => DType::F32,
        1 => DType::F16,
        2 => DType::BF16,
        _ => return Err(StoreError::InvalidData),
    };
    let tier = match bytes[21] {
        0 => Tier::Tier0,
        1 => Tier::Tier1,
        2 => Tier::Tier2,
        3 => Tier::Tier3,
        _ => return Err(StoreError::InvalidData),
    };
    let bits = bytes[22];

    let scale = f32::from_le_bytes(
        bytes[23..27]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let zero_point = i16::from_le_bytes(
        bytes[27..29]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let created_at = u64::from_le_bytes(
        bytes[29..37]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let last_access_at = u64::from_le_bytes(
        bytes[37..45]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let access_count = u32::from_le_bytes(
        bytes[45..49]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let ema_rate = f32::from_le_bytes(
        bytes[49..53]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let window = u64::from_le_bytes(
        bytes[53..61]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let checksum = u32::from_le_bytes(
        bytes[61..65]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );

    let reconstruct = match bytes[65] {
        0 => ReconstructPolicy::None,
        1 => ReconstructPolicy::Delta,
        2 => ReconstructPolicy::Factor,
        _ => return Err(StoreError::InvalidData),
    };
    let tier_age = u32::from_le_bytes(
        bytes[66..70]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );

    let has_lineage = bytes[70];
    let lineage_value = u128::from_le_bytes(
        bytes[71..87]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );
    let lineage_parent = if has_lineage != 0 {
        Some(lineage_value)
    } else {
        None
    };

    let block_bytes = u32::from_le_bytes(
        bytes[87..91]
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    );

    Ok(BlockMeta {
        key: BlockKey {
            tensor_id,
            block_index,
        },
        dtype,
        tier,
        bits,
        scale,
        zero_point,
        created_at,
        last_access_at,
        access_count,
        ema_rate,
        window,
        checksum,
        reconstruct,
        tier_age,
        lineage_parent,
        block_bytes,
    })
}

// ---------------------------------------------------------------------------
// FileBlockIO
// ---------------------------------------------------------------------------

/// Disk-backed [`BlockIO`] that stores each block as a separate file.
///
/// Directory layout:
/// ```text
/// {base_dir}/
///   tier0/
///   tier1/
///   tier2/
///   tier3/
/// ```
///
/// Each block file is named `{tensor_id_hex}_{block_index}.bin`.
pub struct FileBlockIO {
    base_dir: PathBuf,
}

impl FileBlockIO {
    /// Create a new `FileBlockIO` rooted at `base_dir`.
    ///
    /// Creates the tier subdirectories if they do not already exist.
    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let base_dir = base_dir.into();
        for tier_num in 0..=3u8 {
            let tier_dir = base_dir.join(format!("tier{}", tier_num));
            fs::create_dir_all(&tier_dir).map_err(|_| StoreError::IOError)?;
        }
        Ok(Self { base_dir })
    }

    /// Return the filesystem path for a given block.
    fn block_path(&self, tier: Tier, key: BlockKey) -> PathBuf {
        self.base_dir
            .join(format!("tier{}", tier as u8))
            .join(format!("{:032x}_{}.bin", key.tensor_id, key.block_index))
    }

    /// Return the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl BlockIO for FileBlockIO {
    fn read_block(&self, tier: Tier, key: BlockKey, dst: &mut [u8]) -> Result<usize, StoreError> {
        let path = self.block_path(tier, key);
        let data = fs::read(&path).map_err(|_| StoreError::BlockNotFound)?;
        let n = data.len().min(dst.len());
        dst[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    fn write_block(&mut self, tier: Tier, key: BlockKey, src: &[u8]) -> Result<(), StoreError> {
        if tier == Tier::Tier0 {
            return Err(StoreError::InvalidBlock);
        }
        let path = self.block_path(tier, key);
        fs::write(&path, src).map_err(|_| StoreError::IOError)
    }

    fn delete_block(&mut self, tier: Tier, key: BlockKey) -> Result<(), StoreError> {
        let path = self.block_path(tier, key);
        fs::remove_file(&path).map_err(|_| StoreError::BlockNotFound)
    }
}

// ---------------------------------------------------------------------------
// FileMetaLog
// ---------------------------------------------------------------------------

/// Append-only file-backed [`MetaLog`].
///
/// Each [`append`](MetaLog::append) call writes a fixed-size binary record
/// to `{base_dir}/meta.log`. On construction the log is replayed into an
/// in-memory [`HashMap`] so that [`get`](MetaLog::get) is a simple lookup.
///
/// Because the log is append-only, multiple records for the same key may
/// exist on disk. The last record wins when the log is replayed.
pub struct FileMetaLog {
    log_path: PathBuf,
    index: HashMap<BlockKey, BlockMeta>,
}

impl FileMetaLog {
    /// Open (or create) a `FileMetaLog` rooted at `base_dir`.
    ///
    /// If `{base_dir}/meta.log` already exists it is replayed to populate
    /// the in-memory index.
    pub fn new(base_dir: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let base_dir = base_dir.into();
        fs::create_dir_all(&base_dir).map_err(|_| StoreError::IOError)?;
        let log_path = base_dir.join("meta.log");

        let mut index = HashMap::new();

        if log_path.exists() {
            let data = fs::read(&log_path).map_err(|_| StoreError::IOError)?;
            let mut offset = 0;
            while offset + RECORD_SIZE <= data.len() {
                if let Ok(meta) = decode_meta(&data[offset..offset + RECORD_SIZE]) {
                    index.insert(meta.key, meta);
                }
                offset += RECORD_SIZE;
            }
        }

        Ok(Self { log_path, index })
    }

    /// Return the path to the underlying log file.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Number of unique blocks tracked in the in-memory index.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Returns `true` if no metadata records are tracked.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

impl MetaLog for FileMetaLog {
    fn append(&mut self, rec: &BlockMeta) -> Result<(), StoreError> {
        let encoded = encode_meta(rec);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|_| StoreError::IOError)?;
        file.write_all(&encoded).map_err(|_| StoreError::IOError)?;
        file.flush().map_err(|_| StoreError::IOError)?;
        self.index.insert(rec.key, rec.clone());
        Ok(())
    }

    fn get(&self, key: BlockKey) -> Option<&BlockMeta> {
        self.index.get(&key)
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &BlockMeta> + '_> {
        Box::new(self.index.values())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Monotonic counter for unique test directory names.
    static TEST_ID: AtomicU32 = AtomicU32::new(0);

    /// Create a unique temporary directory for a test.
    fn test_dir(prefix: &str) -> PathBuf {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let dir =
            std::env::temp_dir().join(format!("ruvector_persistence_{}_{}_{}", prefix, pid, id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Clean up a test directory (best-effort).
    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    fn make_key(tid: u128, idx: u32) -> BlockKey {
        BlockKey {
            tensor_id: tid,
            block_index: idx,
        }
    }

    fn sample_meta(key: BlockKey) -> BlockMeta {
        BlockMeta {
            key,
            dtype: DType::F32,
            tier: Tier::Tier1,
            bits: 8,
            scale: 0.03125,
            zero_point: 0,
            created_at: 1000,
            last_access_at: 2000,
            access_count: 42,
            ema_rate: 0.75,
            window: 0xAAAA_BBBB_CCCC_DDDD,
            checksum: 0xDEAD_BEEF,
            reconstruct: ReconstructPolicy::None,
            tier_age: 15,
            lineage_parent: None,
            block_bytes: 512,
        }
    }

    // -- encode / decode roundtrip -----------------------------------------

    #[test]
    fn encode_decode_roundtrip_basic() {
        let key = make_key(0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210, 7);
        let meta = sample_meta(key);
        let encoded = encode_meta(&meta);
        assert_eq!(encoded.len(), RECORD_SIZE);

        let decoded = decode_meta(&encoded).unwrap();
        assert_eq!(decoded.key, meta.key);
        assert_eq!(decoded.dtype, meta.dtype);
        assert_eq!(decoded.tier, meta.tier);
        assert_eq!(decoded.bits, meta.bits);
        assert!((decoded.scale - meta.scale).abs() < 1e-10);
        assert_eq!(decoded.zero_point, meta.zero_point);
        assert_eq!(decoded.created_at, meta.created_at);
        assert_eq!(decoded.last_access_at, meta.last_access_at);
        assert_eq!(decoded.access_count, meta.access_count);
        assert!((decoded.ema_rate - meta.ema_rate).abs() < 1e-6);
        assert_eq!(decoded.window, meta.window);
        assert_eq!(decoded.checksum, meta.checksum);
        assert_eq!(decoded.reconstruct, meta.reconstruct);
        assert_eq!(decoded.tier_age, meta.tier_age);
        assert_eq!(decoded.lineage_parent, meta.lineage_parent);
        assert_eq!(decoded.block_bytes, meta.block_bytes);
    }

    #[test]
    fn encode_decode_with_lineage() {
        let key = make_key(1, 0);
        let mut meta = sample_meta(key);
        meta.lineage_parent = Some(0xFFFF_FFFF_FFFF_FFFF_0000_0000_0000_0001);

        let encoded = encode_meta(&meta);
        let decoded = decode_meta(&encoded).unwrap();
        assert_eq!(
            decoded.lineage_parent,
            Some(0xFFFF_FFFF_FFFF_FFFF_0000_0000_0000_0001)
        );
    }

    #[test]
    fn encode_decode_all_dtypes() {
        for (dtype_val, expected) in [(0u8, DType::F32), (1, DType::F16), (2, DType::BF16)] {
            let key = make_key(dtype_val as u128, 0);
            let mut meta = sample_meta(key);
            meta.dtype = expected;
            let decoded = decode_meta(&encode_meta(&meta)).unwrap();
            assert_eq!(decoded.dtype, expected);
        }
    }

    #[test]
    fn encode_decode_all_tiers() {
        for (tier_val, expected) in [
            (0u8, Tier::Tier0),
            (1, Tier::Tier1),
            (2, Tier::Tier2),
            (3, Tier::Tier3),
        ] {
            let key = make_key(tier_val as u128, 0);
            let mut meta = sample_meta(key);
            meta.tier = expected;
            let decoded = decode_meta(&encode_meta(&meta)).unwrap();
            assert_eq!(decoded.tier, expected);
        }
    }

    #[test]
    fn encode_decode_all_reconstruct_policies() {
        for (_, expected) in [
            (0u8, ReconstructPolicy::None),
            (1, ReconstructPolicy::Delta),
            (2, ReconstructPolicy::Factor),
        ] {
            let key = make_key(1, 0);
            let mut meta = sample_meta(key);
            meta.reconstruct = expected;
            let decoded = decode_meta(&encode_meta(&meta)).unwrap();
            assert_eq!(decoded.reconstruct, expected);
        }
    }

    #[test]
    fn decode_too_short() {
        let result = decode_meta(&[0u8; RECORD_SIZE - 1]);
        assert!(
            matches!(result, Err(StoreError::InvalidData)),
            "expected InvalidData, got {:?}",
            result.err()
        );
    }

    #[test]
    fn decode_invalid_dtype() {
        let key = make_key(1, 0);
        let mut encoded = encode_meta(&sample_meta(key));
        encoded[20] = 255; // invalid dtype
        assert!(
            matches!(decode_meta(&encoded), Err(StoreError::InvalidData)),
            "expected InvalidData for bad dtype"
        );
    }

    #[test]
    fn decode_invalid_tier() {
        let key = make_key(1, 0);
        let mut encoded = encode_meta(&sample_meta(key));
        encoded[21] = 99; // invalid tier
        assert!(
            matches!(decode_meta(&encoded), Err(StoreError::InvalidData)),
            "expected InvalidData for bad tier"
        );
    }

    #[test]
    fn decode_invalid_reconstruct() {
        let key = make_key(1, 0);
        let mut encoded = encode_meta(&sample_meta(key));
        encoded[65] = 77; // invalid reconstruct policy
        assert!(
            matches!(decode_meta(&encoded), Err(StoreError::InvalidData)),
            "expected InvalidData for bad reconstruct"
        );
    }

    // -- FileBlockIO -------------------------------------------------------

    #[test]
    fn file_block_io_write_read() {
        let dir = test_dir("bio_wr");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(0xABCD, 3);
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];

        io.write_block(Tier::Tier1, key, &data).unwrap();

        let mut dst = vec![0u8; 16];
        let n = io.read_block(Tier::Tier1, key, &mut dst).unwrap();
        assert_eq!(n, 8);
        assert_eq!(&dst[..8], &data);

        cleanup(&dir);
    }

    #[test]
    fn file_block_io_write_tier0_rejected() {
        let dir = test_dir("bio_t0");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(1, 0);
        assert_eq!(
            io.write_block(Tier::Tier0, key, &[1]),
            Err(StoreError::InvalidBlock)
        );
        cleanup(&dir);
    }

    #[test]
    fn file_block_io_read_not_found() {
        let dir = test_dir("bio_nf");
        let io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(99, 99);
        let mut dst = vec![0u8; 4];
        assert_eq!(
            io.read_block(Tier::Tier2, key, &mut dst),
            Err(StoreError::BlockNotFound)
        );
        cleanup(&dir);
    }

    #[test]
    fn file_block_io_delete() {
        let dir = test_dir("bio_del");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(5, 0);

        io.write_block(Tier::Tier2, key, &[10, 20, 30]).unwrap();
        io.delete_block(Tier::Tier2, key).unwrap();

        let mut dst = vec![0u8; 4];
        assert_eq!(
            io.read_block(Tier::Tier2, key, &mut dst),
            Err(StoreError::BlockNotFound)
        );
        cleanup(&dir);
    }

    #[test]
    fn file_block_io_delete_not_found() {
        let dir = test_dir("bio_del_nf");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(1, 0);
        assert_eq!(
            io.delete_block(Tier::Tier1, key),
            Err(StoreError::BlockNotFound)
        );
        cleanup(&dir);
    }

    #[test]
    fn file_block_io_overwrite() {
        let dir = test_dir("bio_ow");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(1, 0);

        io.write_block(Tier::Tier1, key, &[1, 2, 3]).unwrap();
        io.write_block(Tier::Tier1, key, &[4, 5, 6, 7]).unwrap();

        let mut dst = vec![0u8; 8];
        let n = io.read_block(Tier::Tier1, key, &mut dst).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&dst[..4], &[4, 5, 6, 7]);

        cleanup(&dir);
    }

    #[test]
    fn file_block_io_multiple_tiers() {
        let dir = test_dir("bio_mt");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(1, 0);

        io.write_block(Tier::Tier1, key, &[1]).unwrap();
        io.write_block(Tier::Tier2, key, &[2]).unwrap();
        io.write_block(Tier::Tier3, key, &[3]).unwrap();

        let mut dst = [0u8; 1];
        let n = io.read_block(Tier::Tier1, key, &mut dst).unwrap();
        assert_eq!(n, 1);
        assert_eq!(dst[0], 1);

        let n = io.read_block(Tier::Tier2, key, &mut dst).unwrap();
        assert_eq!(n, 1);
        assert_eq!(dst[0], 2);

        let n = io.read_block(Tier::Tier3, key, &mut dst).unwrap();
        assert_eq!(n, 1);
        assert_eq!(dst[0], 3);

        cleanup(&dir);
    }

    #[test]
    fn file_block_io_path_format() {
        let dir = test_dir("bio_path");
        let io = FileBlockIO::new(&dir).unwrap();
        let key = make_key(0xFF, 42);
        let path = io.block_path(Tier::Tier1, key);
        let expected = dir
            .join("tier1")
            .join("000000000000000000000000000000ff_42.bin");
        assert_eq!(path, expected);
        cleanup(&dir);
    }

    // -- FileMetaLog -------------------------------------------------------

    #[test]
    fn file_meta_log_append_get() {
        let dir = test_dir("ml_ag");
        let mut log = FileMetaLog::new(&dir).unwrap();
        let key = make_key(1, 0);
        let meta = sample_meta(key);

        log.append(&meta).unwrap();

        let retrieved = log.get(key).unwrap();
        assert_eq!(retrieved.key, key);
        assert_eq!(retrieved.created_at, 1000);
        assert_eq!(log.len(), 1);

        cleanup(&dir);
    }

    #[test]
    fn file_meta_log_get_missing() {
        let dir = test_dir("ml_miss");
        let log = FileMetaLog::new(&dir).unwrap();
        assert!(log.get(make_key(99, 0)).is_none());
        cleanup(&dir);
    }

    #[test]
    fn file_meta_log_upsert() {
        let dir = test_dir("ml_ups");
        let mut log = FileMetaLog::new(&dir).unwrap();
        let key = make_key(1, 0);

        let mut meta = sample_meta(key);
        meta.access_count = 10;
        log.append(&meta).unwrap();

        meta.access_count = 20;
        log.append(&meta).unwrap();

        // In-memory should reflect the latest write.
        let retrieved = log.get(key).unwrap();
        assert_eq!(retrieved.access_count, 20);
        assert_eq!(log.len(), 1);

        cleanup(&dir);
    }

    #[test]
    fn file_meta_log_iter() {
        let dir = test_dir("ml_iter");
        let mut log = FileMetaLog::new(&dir).unwrap();

        for i in 0..5u32 {
            let key = make_key(i as u128, 0);
            log.append(&sample_meta(key)).unwrap();
        }

        let entries: Vec<_> = log.iter().collect();
        assert_eq!(entries.len(), 5);

        cleanup(&dir);
    }

    #[test]
    fn file_meta_log_persistence_across_opens() {
        let dir = test_dir("ml_persist");
        let key1 = make_key(1, 0);
        let key2 = make_key(2, 5);

        // First open: write two records.
        {
            let mut log = FileMetaLog::new(&dir).unwrap();
            log.append(&sample_meta(key1)).unwrap();

            let mut meta2 = sample_meta(key2);
            meta2.tier = Tier::Tier3;
            meta2.bits = 3;
            meta2.lineage_parent = Some(0x42);
            log.append(&meta2).unwrap();
            assert_eq!(log.len(), 2);
        }

        // Second open: records should be recovered from disk.
        {
            let log = FileMetaLog::new(&dir).unwrap();
            assert_eq!(log.len(), 2);

            let r1 = log.get(key1).unwrap();
            assert_eq!(r1.tier, Tier::Tier1);

            let r2 = log.get(key2).unwrap();
            assert_eq!(r2.tier, Tier::Tier3);
            assert_eq!(r2.lineage_parent, Some(0x42));
        }

        cleanup(&dir);
    }

    #[test]
    fn file_meta_log_replay_last_wins() {
        let dir = test_dir("ml_lw");
        let key = make_key(1, 0);

        // Write two versions of the same key.
        {
            let mut log = FileMetaLog::new(&dir).unwrap();
            let mut meta = sample_meta(key);
            meta.access_count = 100;
            log.append(&meta).unwrap();
            meta.access_count = 200;
            log.append(&meta).unwrap();
        }

        // Reopen: last record should win during replay.
        {
            let log = FileMetaLog::new(&dir).unwrap();
            assert_eq!(log.len(), 1);
            let retrieved = log.get(key).unwrap();
            assert_eq!(retrieved.access_count, 200);
        }

        cleanup(&dir);
    }

    #[test]
    fn file_meta_log_empty_on_fresh_dir() {
        let dir = test_dir("ml_empty");
        let log = FileMetaLog::new(&dir).unwrap();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.iter().count(), 0);
        cleanup(&dir);
    }

    // -- Integration: FileBlockIO + FileMetaLog ----------------------------

    #[test]
    fn integration_block_io_and_meta_log() {
        let dir = test_dir("integ");
        let mut io = FileBlockIO::new(&dir).unwrap();
        let mut log = FileMetaLog::new(&dir).unwrap();

        let key = make_key(0x1234, 0);
        let block_data = vec![0xFFu8; 256];

        // Write block and metadata.
        io.write_block(Tier::Tier1, key, &block_data).unwrap();

        let mut meta = sample_meta(key);
        meta.block_bytes = 256;
        log.append(&meta).unwrap();

        // Read back and verify.
        let mut dst = vec![0u8; 512];
        let n = io.read_block(Tier::Tier1, key, &mut dst).unwrap();
        assert_eq!(n, 256);
        assert!(dst[..256].iter().all(|&b| b == 0xFF));

        let retrieved = log.get(key).unwrap();
        assert_eq!(retrieved.block_bytes, 256);

        cleanup(&dir);
    }

    #[test]
    fn record_size_constant_matches() {
        // Verify that RECORD_SIZE matches the actual encoded size.
        let meta = sample_meta(make_key(0, 0));
        let encoded = encode_meta(&meta);
        assert_eq!(encoded.len(), RECORD_SIZE);
    }
}
