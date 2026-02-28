#![cfg(feature = "persistence")]

use ruvector_temporal_tensor::persistence::{FileBlockIO, FileMetaLog};
use ruvector_temporal_tensor::store::{
    BlockIO, BlockKey, BlockMeta, DType, MetaLog, ReconstructPolicy, Tier,
};
use std::path::PathBuf;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ruvector_test_{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

fn make_key(id: u128, idx: u32) -> BlockKey {
    BlockKey {
        tensor_id: id,
        block_index: idx,
    }
}

fn make_meta(key: BlockKey, tier: Tier) -> BlockMeta {
    BlockMeta {
        key,
        dtype: DType::F32,
        tier,
        bits: 8,
        scale: 0.5,
        zero_point: 0,
        created_at: 100,
        last_access_at: 200,
        access_count: 5,
        ema_rate: 0.1,
        window: 0xFF,
        checksum: 0xDEADBEEF,
        reconstruct: ReconstructPolicy::None,
        tier_age: 10,
        lineage_parent: None,
        block_bytes: 64,
    }
}

// -----------------------------------------------------------------------
// FileBlockIO tests
// -----------------------------------------------------------------------

#[test]
fn test_file_block_io_write_read() {
    let dir = test_dir("block_io_write_read");
    let mut bio = FileBlockIO::new(&dir).unwrap();

    let key = make_key(1, 0);
    let data = vec![0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89];
    bio.write_block(Tier::Tier1, key, &data).unwrap();

    let mut dst = vec![0u8; 32];
    let n = bio.read_block(Tier::Tier1, key, &mut dst).unwrap();
    assert_eq!(n, data.len());
    assert_eq!(&dst[..n], &data[..]);

    cleanup(&dir);
}

#[test]
fn test_file_block_io_different_tiers() {
    let dir = test_dir("block_io_tiers");
    let mut bio = FileBlockIO::new(&dir).unwrap();

    let key = make_key(1, 0);
    let data1 = vec![1u8; 16];
    let data2 = vec![2u8; 8];
    let data3 = vec![3u8; 4];

    bio.write_block(Tier::Tier1, key, &data1).unwrap();
    bio.write_block(Tier::Tier2, key, &data2).unwrap();
    bio.write_block(Tier::Tier3, key, &data3).unwrap();

    let mut buf = vec![0u8; 32];

    let n1 = bio.read_block(Tier::Tier1, key, &mut buf).unwrap();
    assert_eq!(&buf[..n1], &data1[..]);

    let n2 = bio.read_block(Tier::Tier2, key, &mut buf).unwrap();
    assert_eq!(&buf[..n2], &data2[..]);

    let n3 = bio.read_block(Tier::Tier3, key, &mut buf).unwrap();
    assert_eq!(&buf[..n3], &data3[..]);

    cleanup(&dir);
}

#[test]
fn test_file_block_io_delete() {
    let dir = test_dir("block_io_delete");
    let mut bio = FileBlockIO::new(&dir).unwrap();

    let key = make_key(1, 0);
    bio.write_block(Tier::Tier1, key, &[1, 2, 3]).unwrap();
    bio.delete_block(Tier::Tier1, key).unwrap();

    let mut buf = vec![0u8; 32];
    let result = bio.read_block(Tier::Tier1, key, &mut buf);
    assert!(result.is_err() || result.unwrap() == 0);

    cleanup(&dir);
}

#[test]
fn test_file_block_io_overwrite() {
    let dir = test_dir("block_io_overwrite");
    let mut bio = FileBlockIO::new(&dir).unwrap();

    let key = make_key(1, 0);
    bio.write_block(Tier::Tier1, key, &[1, 2, 3]).unwrap();
    bio.write_block(Tier::Tier1, key, &[4, 5, 6, 7]).unwrap();

    let mut buf = vec![0u8; 32];
    let n = bio.read_block(Tier::Tier1, key, &mut buf).unwrap();
    assert_eq!(&buf[..n], &[4, 5, 6, 7]);

    cleanup(&dir);
}

#[test]
fn test_file_block_io_missing_key() {
    let dir = test_dir("block_io_missing");
    let bio = FileBlockIO::new(&dir).unwrap();

    let mut buf = vec![0u8; 32];
    let result = bio.read_block(Tier::Tier1, make_key(99, 0), &mut buf);
    assert!(result.is_err() || result.unwrap() == 0);

    cleanup(&dir);
}

// -----------------------------------------------------------------------
// FileMetaLog tests
// -----------------------------------------------------------------------

#[test]
fn test_file_meta_log_append_get() {
    let dir = test_dir("meta_log_append");
    let mut log = FileMetaLog::new(&dir).unwrap();

    let key = make_key(1, 0);
    let meta = make_meta(key, Tier::Tier1);
    log.append(&meta).unwrap();

    let retrieved = log.get(key).unwrap();
    assert_eq!(retrieved.key, key);
    assert_eq!(retrieved.tier, Tier::Tier1);
    assert_eq!(retrieved.bits, 8);
    assert!((retrieved.scale - 0.5).abs() < 1e-6);
    assert_eq!(retrieved.checksum, 0xDEADBEEF);

    cleanup(&dir);
}

#[test]
fn test_file_meta_log_upsert() {
    let dir = test_dir("meta_log_upsert");
    let mut log = FileMetaLog::new(&dir).unwrap();

    let key = make_key(1, 0);
    let meta1 = make_meta(key, Tier::Tier1);
    log.append(&meta1).unwrap();

    let mut meta2 = make_meta(key, Tier::Tier2);
    meta2.bits = 7;
    log.append(&meta2).unwrap();

    let retrieved = log.get(key).unwrap();
    assert_eq!(retrieved.tier, Tier::Tier2);
    assert_eq!(retrieved.bits, 7);

    cleanup(&dir);
}

#[test]
fn test_file_meta_log_iter() {
    let dir = test_dir("meta_log_iter");
    let mut log = FileMetaLog::new(&dir).unwrap();

    for i in 0..5u128 {
        let key = make_key(i, 0);
        log.append(&make_meta(key, Tier::Tier1)).unwrap();
    }

    let count = log.iter().count();
    assert_eq!(count, 5);

    cleanup(&dir);
}

#[test]
fn test_file_meta_log_missing_key() {
    let dir = test_dir("meta_log_missing");
    let log = FileMetaLog::new(&dir).unwrap();
    assert!(log.get(make_key(99, 0)).is_none());

    cleanup(&dir);
}

#[test]
fn test_file_meta_log_multiple_blocks_same_tensor() {
    let dir = test_dir("meta_log_multi_block");
    let mut log = FileMetaLog::new(&dir).unwrap();

    for idx in 0..3u32 {
        let key = make_key(1, idx);
        log.append(&make_meta(key, Tier::Tier1)).unwrap();
    }

    assert!(log.get(make_key(1, 0)).is_some());
    assert!(log.get(make_key(1, 1)).is_some());
    assert!(log.get(make_key(1, 2)).is_some());
    assert!(log.get(make_key(1, 3)).is_none());

    cleanup(&dir);
}
