//! Integration tests for provenance chain / lineage verification.
//!
//! Tests FileIdentity creation, derivation chains, lineage depth,
//! parent_id/parent_hash linkage, and multi-level derivation.

use rvf_runtime::options::{DistanceMetric, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::lineage::{LineageRecord, WITNESS_DERIVATION};
use rvf_types::{DerivationType, FileIdentity};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

// ===========================================================================
// TEST 1: root_file_has_zero_lineage
// ===========================================================================

/// A freshly created RVF file should be a root with lineage_depth=0 and
/// a zero parent_id.
#[test]
fn root_file_has_zero_lineage() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.rvf");
    let dim: u16 = 4;

    let store = RvfStore::create(&path, make_options(dim)).unwrap();

    assert_eq!(store.lineage_depth(), 0, "root should have lineage_depth 0");
    assert_eq!(
        store.parent_id(),
        &[0u8; 16],
        "root parent_id should be all zeros"
    );
    assert_ne!(
        store.file_id(),
        &[0u8; 16],
        "root file_id should be non-zero"
    );
    assert!(
        store.file_identity().is_root(),
        "root identity should report is_root()"
    );

    store.close().unwrap();

    println!("PASS: root_file_has_zero_lineage");
}

// ===========================================================================
// TEST 2: derive_sets_parent_id
// ===========================================================================

/// Deriving a child from a parent should set the child's parent_id to
/// the parent's file_id.
#[test]
fn derive_sets_parent_id() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.rvf");
    let child_path = dir.path().join("child.rvf");
    let dim: u16 = 4;

    let mut parent = RvfStore::create(&parent_path, make_options(dim)).unwrap();
    let v = vec![1.0f32; dim as usize];
    parent.ingest_batch(&[v.as_slice()], &[1], None).unwrap();

    let parent_file_id = *parent.file_id();

    let child = parent
        .derive(&child_path, DerivationType::Clone, Some(make_options(dim)))
        .unwrap();

    assert_eq!(
        child.parent_id(),
        &parent_file_id,
        "child's parent_id should equal parent's file_id"
    );
    assert_ne!(
        child.file_id(),
        &parent_file_id,
        "child should have its own unique file_id"
    );

    child.close().unwrap();
    parent.close().unwrap();

    println!("PASS: derive_sets_parent_id");
}

// ===========================================================================
// TEST 3: derive_increments_lineage_depth
// ===========================================================================

/// Each derivation should increment lineage_depth by 1.
#[test]
fn derive_increments_lineage_depth() {
    let dir = TempDir::new().unwrap();
    let root_path = dir.path().join("root.rvf");
    let child1_path = dir.path().join("child1.rvf");
    let child2_path = dir.path().join("child2.rvf");
    let dim: u16 = 4;

    let mut root = RvfStore::create(&root_path, make_options(dim)).unwrap();
    let v = vec![1.0f32; dim as usize];
    root.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
    assert_eq!(root.lineage_depth(), 0);

    let mut child1 = root
        .derive(&child1_path, DerivationType::Clone, Some(make_options(dim)))
        .unwrap();
    assert_eq!(child1.lineage_depth(), 1);

    // Need to ingest something so the child has content for hash computation
    let v2 = vec![2.0f32; dim as usize];
    child1.ingest_batch(&[v2.as_slice()], &[2], None).unwrap();

    let child2 = child1
        .derive(&child2_path, DerivationType::Clone, Some(make_options(dim)))
        .unwrap();
    assert_eq!(child2.lineage_depth(), 2);

    child2.close().unwrap();
    child1.close().unwrap();
    root.close().unwrap();

    println!("PASS: derive_increments_lineage_depth");
}

// ===========================================================================
// TEST 4: parent_hash_is_nonzero_for_derived
// ===========================================================================

/// A derived file should have a non-zero parent_hash (hash of parent manifest).
#[test]
fn parent_hash_is_nonzero_for_derived() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent_hash.rvf");
    let child_path = dir.path().join("child_hash.rvf");
    let dim: u16 = 4;

    let mut parent = RvfStore::create(&parent_path, make_options(dim)).unwrap();
    let v = vec![1.0f32; dim as usize];
    parent.ingest_batch(&[v.as_slice()], &[1], None).unwrap();

    let child = parent
        .derive(&child_path, DerivationType::Clone, Some(make_options(dim)))
        .unwrap();

    let parent_hash = child.file_identity().parent_hash;
    assert_ne!(
        parent_hash, [0u8; 32],
        "derived file's parent_hash should be non-zero"
    );

    child.close().unwrap();
    parent.close().unwrap();

    println!("PASS: parent_hash_is_nonzero_for_derived");
}

// ===========================================================================
// TEST 5: lineage_persists_through_reopen
// ===========================================================================

/// Derive a child, close both, reopen child, and verify lineage is intact.
#[test]
fn lineage_persists_through_reopen() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent_persist.rvf");
    let child_path = dir.path().join("child_persist.rvf");
    let dim: u16 = 4;

    let parent_file_id;
    let child_file_id;
    let child_parent_hash;

    {
        let mut parent = RvfStore::create(&parent_path, make_options(dim)).unwrap();
        let v = vec![1.0f32; dim as usize];
        parent.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
        parent_file_id = *parent.file_id();

        let child = parent
            .derive(&child_path, DerivationType::Clone, Some(make_options(dim)))
            .unwrap();
        child_file_id = *child.file_id();
        child_parent_hash = child.file_identity().parent_hash;
        child.close().unwrap();
        parent.close().unwrap();
    }

    // Reopen child
    {
        let child = RvfStore::open_readonly(&child_path).unwrap();
        assert_eq!(
            child.file_id(),
            &child_file_id,
            "file_id should persist through reopen"
        );
        assert_eq!(
            child.parent_id(),
            &parent_file_id,
            "parent_id should persist through reopen"
        );
        assert_eq!(
            child.lineage_depth(),
            1,
            "lineage_depth should persist through reopen"
        );
        assert_eq!(
            child.file_identity().parent_hash,
            child_parent_hash,
            "parent_hash should persist through reopen"
        );
    }

    println!("PASS: lineage_persists_through_reopen");
}

// ===========================================================================
// TEST 6: file_identity_type_round_trip
// ===========================================================================

/// Test FileIdentity serialization / deserialization directly.
#[test]
fn file_identity_type_round_trip() {
    let fi = FileIdentity {
        file_id: [0x11; 16],
        parent_id: [0x22; 16],
        parent_hash: [0x33; 32],
        lineage_depth: 42,
    };

    let bytes = fi.to_bytes();
    assert_eq!(bytes.len(), 68);

    let decoded = FileIdentity::from_bytes(&bytes);
    assert_eq!(decoded, fi);
    assert_eq!(decoded.file_id, [0x11; 16]);
    assert_eq!(decoded.parent_id, [0x22; 16]);
    assert_eq!(decoded.parent_hash, [0x33; 32]);
    assert_eq!(decoded.lineage_depth, 42);
    assert!(!decoded.is_root());

    println!("PASS: file_identity_type_round_trip");
}

// ===========================================================================
// TEST 7: lineage_record_round_trip
// ===========================================================================

/// Test LineageRecord creation and field access.
#[test]
fn lineage_record_round_trip() {
    let record = LineageRecord::new(
        [0xAA; 16],
        [0xBB; 16],
        [0xCC; 32],
        DerivationType::Filter,
        100,
        1_700_000_000_000_000_000,
        "filtered by embedding cluster",
    );

    assert_eq!(record.file_id, [0xAA; 16]);
    assert_eq!(record.parent_id, [0xBB; 16]);
    assert_eq!(record.parent_hash, [0xCC; 32]);
    assert_eq!(record.derivation_type, DerivationType::Filter);
    assert_eq!(record.mutation_count, 100);
    assert_eq!(record.timestamp_ns, 1_700_000_000_000_000_000);
    assert_eq!(record.description_str(), "filtered by embedding cluster");

    println!("PASS: lineage_record_round_trip");
}

// ===========================================================================
// TEST 8: witness_derivation_constant
// ===========================================================================

/// Verify the witness type constant for derivation events.
#[test]
fn witness_derivation_constant() {
    assert_eq!(WITNESS_DERIVATION, 0x09);

    println!("PASS: witness_derivation_constant");
}

// ===========================================================================
// TEST 9: derivation_type_enum_coverage
// ===========================================================================

/// Verify all DerivationType variants serialize correctly.
#[test]
fn derivation_type_enum_coverage() {
    let cases: &[(u8, DerivationType)] = &[
        (0, DerivationType::Clone),
        (1, DerivationType::Filter),
        (2, DerivationType::Merge),
        (3, DerivationType::Quantize),
        (4, DerivationType::Reindex),
        (5, DerivationType::Transform),
        (6, DerivationType::Snapshot),
        (0xFF, DerivationType::UserDefined),
    ];

    for &(raw, expected) in cases {
        let decoded = DerivationType::try_from(raw);
        assert_eq!(
            decoded,
            Ok(expected),
            "DerivationType::try_from({raw}) should be {expected:?}"
        );
        assert_eq!(expected as u8, raw);
    }

    // Invalid values should error
    assert!(DerivationType::try_from(7).is_err());
    assert!(DerivationType::try_from(0xFE).is_err());

    println!("PASS: derivation_type_enum_coverage");
}

// ===========================================================================
// TEST 10: three_level_lineage_chain
// ===========================================================================

/// Build a three-level lineage chain: root -> child -> grandchild,
/// and verify the entire chain is correct.
#[test]
fn three_level_lineage_chain() {
    let dir = TempDir::new().unwrap();
    let root_path = dir.path().join("root_chain.rvf");
    let child_path = dir.path().join("child_chain.rvf");
    let grandchild_path = dir.path().join("grandchild_chain.rvf");
    let dim: u16 = 4;

    // Root
    let mut root = RvfStore::create(&root_path, make_options(dim)).unwrap();
    let v = vec![1.0f32; dim as usize];
    root.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
    let root_id = *root.file_id();

    // Child
    let mut child = root
        .derive(&child_path, DerivationType::Clone, Some(make_options(dim)))
        .unwrap();
    let child_id = *child.file_id();
    let v2 = vec![2.0f32; dim as usize];
    child.ingest_batch(&[v2.as_slice()], &[2], None).unwrap();

    // Grandchild
    let grandchild = child
        .derive(
            &grandchild_path,
            DerivationType::Filter,
            Some(make_options(dim)),
        )
        .unwrap();
    let grandchild_id = *grandchild.file_id();

    // Verify chain
    assert_eq!(root.lineage_depth(), 0);
    assert_eq!(child.lineage_depth(), 1);
    assert_eq!(grandchild.lineage_depth(), 2);

    assert_eq!(root.parent_id(), &[0u8; 16]);
    assert_eq!(child.parent_id(), &root_id);
    assert_eq!(grandchild.parent_id(), &child_id);

    // All file_ids should be unique
    assert_ne!(root_id, child_id);
    assert_ne!(child_id, grandchild_id);
    assert_ne!(root_id, grandchild_id);

    // Parent hashes should be non-zero for derived files
    assert_ne!(child.file_identity().parent_hash, [0u8; 32]);
    assert_ne!(grandchild.file_identity().parent_hash, [0u8; 32]);

    grandchild.close().unwrap();
    child.close().unwrap();
    root.close().unwrap();

    println!("PASS: three_level_lineage_chain");
}

// ===========================================================================
// TEST 11: lineage_record_long_description_truncation
// ===========================================================================

/// Verify that LineageRecord truncates descriptions longer than 47 bytes.
#[test]
fn lineage_record_long_description_truncation() {
    let long_desc = "a".repeat(100);
    let record = LineageRecord::new(
        [0u8; 16],
        [0u8; 16],
        [0u8; 32],
        DerivationType::Clone,
        0,
        0,
        &long_desc,
    );

    assert_eq!(record.description_len, 47, "should be truncated to 47");
    assert_eq!(record.description_str(), &"a".repeat(47));

    println!("PASS: lineage_record_long_description_truncation");
}
