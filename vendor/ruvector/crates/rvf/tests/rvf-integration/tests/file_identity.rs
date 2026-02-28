//! Integration test: FileIdentity write → read round-trip via Level0Root.
//!
//! Tests the Level0Root codec's FileIdentity read/write in the reserved area,
//! backward compatibility (zeros parse as valid root), and the type itself.

use rvf_manifest::{read_level0, write_level0};
use rvf_types::{FileIdentity, Level0Root};

#[test]
fn file_identity_write_read_round_trip() {
    let mut root = Level0Root::zeroed();
    root.version = 1;
    root.dimension = 128;

    // Set a FileIdentity in the reserved area
    let fi = FileIdentity {
        file_id: [0xAA; 16],
        parent_id: [0xBB; 16],
        parent_hash: [0xCC; 32],
        lineage_depth: 3,
    };
    root.reserved[..68].copy_from_slice(&fi.to_bytes());

    // Write and read back
    let bytes = write_level0(&root);
    let decoded = read_level0(&bytes).unwrap();

    // Extract FileIdentity from decoded reserved area
    let decoded_fi = FileIdentity::from_bytes(decoded.reserved[..68].try_into().unwrap());
    assert_eq!(decoded_fi, fi);
    assert_eq!(decoded_fi.file_id, [0xAA; 16]);
    assert_eq!(decoded_fi.parent_id, [0xBB; 16]);
    assert_eq!(decoded_fi.parent_hash, [0xCC; 32]);
    assert_eq!(decoded_fi.lineage_depth, 3);
}

#[test]
fn zeroed_reserved_parses_as_root_identity() {
    let root = Level0Root::zeroed();
    let bytes = write_level0(&root);
    let decoded = read_level0(&bytes).unwrap();

    let fi = FileIdentity::from_bytes(decoded.reserved[..68].try_into().unwrap());
    assert!(fi.is_root());
    assert_eq!(fi.file_id, [0u8; 16]);
    assert_eq!(fi.parent_id, [0u8; 16]);
    assert_eq!(fi.parent_hash, [0u8; 32]);
    assert_eq!(fi.lineage_depth, 0);
}

#[test]
fn backward_compat_old_files_still_work() {
    // Simulate an old file with no lineage data (all zeros in reserved)
    let root = Level0Root::zeroed();
    let bytes = write_level0(&root);

    // Should parse successfully
    let decoded = read_level0(&bytes).unwrap();
    assert_eq!(decoded.magic, rvf_types::ROOT_MANIFEST_MAGIC);

    // FileIdentity should be all zeros = valid root
    let fi = FileIdentity::from_bytes(decoded.reserved[..68].try_into().unwrap());
    assert!(fi.is_root());
}

#[test]
fn file_identity_type_assertions() {
    // Compile-time verified, but test runtime too
    assert_eq!(core::mem::size_of::<FileIdentity>(), 68);
    assert!(
        68 <= 252,
        "FileIdentity must fit in Level0Root reserved area"
    );
}

#[test]
fn file_identity_to_bytes_from_bytes_round_trip() {
    let cases = [
        FileIdentity::zeroed(),
        FileIdentity::new_root([0xFF; 16]),
        FileIdentity {
            file_id: [1; 16],
            parent_id: [2; 16],
            parent_hash: [3; 32],
            lineage_depth: u32::MAX,
        },
    ];

    for fi in &cases {
        let bytes = fi.to_bytes();
        let decoded = FileIdentity::from_bytes(&bytes);
        assert_eq!(&decoded, fi);
    }
}

#[test]
fn root_identity_detection() {
    // Root: all-zero parent + depth 0
    let root = FileIdentity::new_root([0x42; 16]);
    assert!(root.is_root());

    // Non-root: has parent_id
    let child = FileIdentity {
        file_id: [1; 16],
        parent_id: [2; 16],
        parent_hash: [3; 32],
        lineage_depth: 1,
    };
    assert!(!child.is_root());

    // Edge case: zero parent_id but non-zero depth → not root
    let weird = FileIdentity {
        file_id: [1; 16],
        parent_id: [0; 16],
        parent_hash: [0; 32],
        lineage_depth: 5,
    };
    assert!(!weird.is_root());
}

#[test]
fn level0_root_preserves_other_fields_with_identity() {
    let mut root = Level0Root::zeroed();
    root.version = 1;
    root.flags = 0x0804; // SIGNED + HAS_LINEAGE
    root.total_vector_count = 1_000_000;
    root.dimension = 384;
    root.epoch = 42;

    let fi = FileIdentity {
        file_id: [0x11; 16],
        parent_id: [0x22; 16],
        parent_hash: [0x33; 32],
        lineage_depth: 7,
    };
    root.reserved[..68].copy_from_slice(&fi.to_bytes());

    let bytes = write_level0(&root);
    let decoded = read_level0(&bytes).unwrap();

    // Original fields preserved
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.flags, 0x0804);
    assert_eq!(decoded.total_vector_count, 1_000_000);
    assert_eq!(decoded.dimension, 384);
    assert_eq!(decoded.epoch, 42);

    // FileIdentity preserved
    let decoded_fi = FileIdentity::from_bytes(decoded.reserved[..68].try_into().unwrap());
    assert_eq!(decoded_fi, fi);
}
