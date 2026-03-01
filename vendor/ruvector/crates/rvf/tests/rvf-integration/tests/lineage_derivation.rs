//! Integration test: parent → child → grandchild derivation chain.
//!
//! Verifies file_id, parent_id, parent_hash, lineage_depth at each level,
//! and that HAS_LINEAGE flag + DERIVATION witness semantics work end-to-end.

use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{RvfOptions, RvfStore};
use rvf_types::DerivationType;
use tempfile::TempDir;

#[test]
fn parent_child_grandchild_derivation() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.rvf");
    let child_path = dir.path().join("child.rvf");
    let grandchild_path = dir.path().join("grandchild.rvdna");

    let options = RvfOptions {
        dimension: 4,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    // Create parent
    let parent = RvfStore::create(&parent_path, options.clone()).unwrap();
    let parent_file_id = *parent.file_id();
    assert_eq!(parent.lineage_depth(), 0);
    assert_eq!(parent.parent_id(), &[0u8; 16]);
    assert!(parent.file_identity().is_root());
    assert_ne!(parent_file_id, [0u8; 16]); // should have a real ID

    // Derive child from parent
    let child = parent
        .derive(&child_path, DerivationType::Filter, None)
        .unwrap();
    let child_file_id = *child.file_id();
    assert_eq!(child.lineage_depth(), 1);
    assert_eq!(child.parent_id(), &parent_file_id);
    assert!(!child.file_identity().is_root());
    assert_ne!(child_file_id, parent_file_id); // different file IDs
    assert_ne!(child.file_identity().parent_hash, [0u8; 32]); // non-zero parent hash

    // Derive grandchild from child
    let grandchild = child
        .derive(&grandchild_path, DerivationType::Transform, None)
        .unwrap();
    assert_eq!(grandchild.lineage_depth(), 2);
    assert_eq!(grandchild.parent_id(), &child_file_id);
    assert!(!grandchild.file_identity().is_root());
    assert_ne!(grandchild.file_identity().parent_hash, [0u8; 32]);

    // Verify the chain is properly linked
    assert_ne!(grandchild.file_id(), child.file_id());
    assert_ne!(grandchild.file_id(), parent.file_id());

    grandchild.close().unwrap();
    child.close().unwrap();
    parent.close().unwrap();
}

#[test]
fn derived_store_inherits_dimension() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.rvf");
    let child_path = dir.path().join("child.rvf");

    let options = RvfOptions {
        dimension: 128,
        metric: DistanceMetric::Cosine,
        ..Default::default()
    };

    let parent = RvfStore::create(&parent_path, options).unwrap();
    let child = parent
        .derive(&child_path, DerivationType::Clone, None)
        .unwrap();

    // Child should be queryable with same dimension
    let query = vec![0.0f32; 128];
    let results = child
        .query(&query, 10, &rvf_runtime::QueryOptions::default())
        .unwrap();
    assert!(results.is_empty()); // no vectors ingested yet

    child.close().unwrap();
    parent.close().unwrap();
}

#[test]
fn file_identity_persists_through_reopen() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.rvf");
    let child_path = dir.path().join("child.rvf");

    let options = RvfOptions {
        dimension: 4,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let parent = RvfStore::create(&parent_path, options).unwrap();
    let parent_file_id = *parent.file_id();

    let child = parent
        .derive(&child_path, DerivationType::Snapshot, None)
        .unwrap();
    let child_file_id = *child.file_id();
    let child_depth = child.lineage_depth();
    let child_parent_id = *child.parent_id();
    let child_parent_hash = child.file_identity().parent_hash;
    child.close().unwrap();
    parent.close().unwrap();

    // Reopen child and verify identity persists
    let reopened = RvfStore::open(&child_path).unwrap();
    assert_eq!(*reopened.file_id(), child_file_id);
    assert_eq!(reopened.lineage_depth(), child_depth);
    assert_eq!(*reopened.parent_id(), child_parent_id);
    assert_eq!(reopened.file_identity().parent_hash, child_parent_hash);
    assert_eq!(*reopened.parent_id(), parent_file_id);
    reopened.close().unwrap();
}

#[test]
fn root_file_identity_persists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("root.rvf");

    let options = RvfOptions {
        dimension: 4,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let store = RvfStore::create(&path, options).unwrap();
    let original_id = *store.file_id();
    assert!(store.file_identity().is_root());
    store.close().unwrap();

    let reopened = RvfStore::open(&path).unwrap();
    assert_eq!(*reopened.file_id(), original_id);
    assert!(reopened.file_identity().is_root());
    assert_eq!(reopened.lineage_depth(), 0);
    reopened.close().unwrap();
}
