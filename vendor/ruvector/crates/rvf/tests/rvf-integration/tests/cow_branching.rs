//! Integration tests for the RVF COW (copy-on-write) branching system.
//!
//! Tests the core branching flow: creating a base store, deriving a child,
//! verifying COW statistics, write coalescing, and parent immutability.

use rvf_runtime::options::{DistanceMetric, RvfOptions};
use rvf_runtime::RvfStore;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper: make RvfStore options
// ---------------------------------------------------------------------------

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Helper: random-ish vector for testing
// ---------------------------------------------------------------------------

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed;
    for _ in 0..dim {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

// ===========================================================================
// TEST 1: basic_branch_creation
// ===========================================================================

/// Create a base store with vectors, branch it, and verify the child
/// is a COW child with correct statistics.
#[test]
fn basic_branch_creation() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base.rvf");
    let child_path = dir.path().join("child.rvf");
    let dim: u16 = 4;

    // Create base store with vectors
    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let vectors: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=20).collect();
    base.ingest_batch(&refs, &ids, None).unwrap();

    // Branch from base
    let child = base.branch(&child_path).unwrap();

    // Verify child is a COW child
    assert!(child.is_cow_child(), "child should be a COW child");

    // Verify COW stats exist
    let stats = child.cow_stats().expect("child should have COW stats");
    assert_eq!(
        stats.local_cluster_count, 0,
        "new branch should have no local clusters yet"
    );
    assert!(
        stats.cluster_count > 0,
        "branch should have inherited clusters"
    );
    assert!(!stats.frozen, "new branch should not be frozen");

    // Verify parent path is set
    assert!(
        child.parent_path().is_some(),
        "child should have a parent path"
    );

    // Verify the child has a membership filter
    assert!(
        child.membership_filter().is_some(),
        "child should have a membership filter"
    );

    child.close().unwrap();
    base.close().unwrap();

    println!("PASS: basic_branch_creation");
}

// ===========================================================================
// TEST 2: branch_inherits_vectors_via_query
// ===========================================================================

/// Create a base store with vectors, branch it, and verify the child
/// has the parent's vectors visible in its membership filter.
///
/// Note: The branch() method creates a MembershipFilter with capacity
/// equal to total_vecs (count of vectors). Vector IDs must be in the
/// range [0, total_vecs) to be representable in the filter bitmap.
#[test]
fn branch_inherits_vectors_via_query() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base_q.rvf");
    let child_path = dir.path().join("child_q.rvf");
    let dim: u16 = 4;

    // Create base with contiguous IDs starting from 0
    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let v1 = vec![1.0, 0.0, 0.0, 0.0];
    let v2 = vec![0.0, 1.0, 0.0, 0.0];
    let v3 = vec![0.0, 0.0, 1.0, 0.0];
    let vecs: Vec<&[f32]> = vec![&v1, &v2, &v3];
    base.ingest_batch(&vecs, &[0, 1, 2], None).unwrap();

    // Branch
    let child = base.branch(&child_path).unwrap();

    // The child's membership filter should include the parent's vectors
    let filter = child.membership_filter().unwrap();
    assert!(filter.contains(0), "filter should include vector 0");
    assert!(filter.contains(1), "filter should include vector 1");
    assert!(filter.contains(2), "filter should include vector 2");
    assert_eq!(filter.member_count(), 3, "filter should have 3 members");

    child.close().unwrap();
    base.close().unwrap();

    println!("PASS: branch_inherits_vectors_via_query");
}

// ===========================================================================
// TEST 3: cow_stats_reflect_local_and_inherited
// ===========================================================================

/// Create a branch and verify that CowStats correctly reflects
/// local vs inherited cluster counts.
#[test]
fn cow_stats_reflect_local_and_inherited() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base_stats.rvf");
    let child_path = dir.path().join("child_stats.rvf");
    let dim: u16 = 4;

    // Create base with enough vectors to create multiple clusters
    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let vectors: Vec<Vec<f32>> = (0..50).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=50).collect();
    base.ingest_batch(&refs, &ids, None).unwrap();

    let child = base.branch(&child_path).unwrap();

    let stats = child.cow_stats().unwrap();
    let inherited = stats.cluster_count - stats.local_cluster_count;

    assert!(
        inherited > 0,
        "child should have inherited clusters from parent"
    );
    assert_eq!(
        stats.local_cluster_count, 0,
        "fresh branch has no local clusters"
    );

    child.close().unwrap();
    base.close().unwrap();

    println!("PASS: cow_stats_reflect_local_and_inherited");
}

// ===========================================================================
// TEST 4: parent_unmodified_after_branch
// ===========================================================================

/// Verify that branching does not modify the parent store's data.
#[test]
fn parent_unmodified_after_branch() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base_parent.rvf");
    let child_path = dir.path().join("child_parent.rvf");
    let dim: u16 = 4;

    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let v1 = vec![1.0, 2.0, 3.0, 4.0];
    base.ingest_batch(&[v1.as_slice()], &[100], None).unwrap();

    let status_before = base.status();
    let total_before = status_before.total_vectors;
    let epoch_before = status_before.current_epoch;

    let child = base.branch(&child_path).unwrap();
    child.close().unwrap();

    // Parent should be unchanged
    let status_after = base.status();
    assert_eq!(
        status_after.total_vectors, total_before,
        "parent vector count should be unchanged after branch"
    );
    assert_eq!(
        status_after.current_epoch, epoch_before,
        "parent epoch should be unchanged after branch"
    );

    // Parent should still not be a COW child
    assert!(!base.is_cow_child(), "parent should not become a COW child");

    base.close().unwrap();

    println!("PASS: parent_unmodified_after_branch");
}

// ===========================================================================
// TEST 5: child_size_smaller_than_parent
// ===========================================================================

/// Create a large base store, branch it (no writes to child), and verify
/// the child file on disk is significantly smaller than the parent.
#[test]
fn child_size_smaller_than_parent() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base_size.rvf");
    let child_path = dir.path().join("child_size.rvf");
    let dim: u16 = 32;

    // Create base with many vectors to make a reasonably large file
    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let vectors: Vec<Vec<f32>> = (0..200).map(|i| random_vector(dim as usize, i)).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=200).collect();
    base.ingest_batch(&refs, &ids, None).unwrap();

    let child = base.branch(&child_path).unwrap();
    child.close().unwrap();
    base.close().unwrap();

    // Compare file sizes
    let base_size = std::fs::metadata(&base_path).unwrap().len();
    let child_size = std::fs::metadata(&child_path).unwrap().len();

    assert!(
        child_size < base_size,
        "child file ({child_size} bytes) should be smaller than parent ({base_size} bytes)"
    );

    println!("PASS: child_size_smaller_than_parent -- parent={base_size}, child={child_size}");
}

// ===========================================================================
// TEST 6: freeze_prevents_further_writes
// ===========================================================================

/// Freezing a store prevents further mutations.
#[test]
fn freeze_prevents_further_writes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("freeze.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let v = vec![1.0f32; dim as usize];
    store.ingest_batch(&[v.as_slice()], &[1], None).unwrap();

    store.freeze().unwrap();

    // Trying to ingest after freeze should fail
    let v2 = vec![2.0f32; dim as usize];
    let result = store.ingest_batch(&[v2.as_slice()], &[2], None);
    assert!(result.is_err(), "ingesting after freeze should fail");

    println!("PASS: freeze_prevents_further_writes");
}

// ===========================================================================
// TEST 7: derive_creates_lineage
// ===========================================================================

/// Deriving a child store sets up proper lineage: parent_id, parent_hash,
/// and lineage_depth.
#[test]
fn derive_creates_lineage() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base_lineage.rvf");
    let child_path = dir.path().join("child_lineage.rvf");
    let dim: u16 = 4;

    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let v = vec![1.0f32; dim as usize];
    base.ingest_batch(&[v.as_slice()], &[1], None).unwrap();

    let base_file_id = *base.file_id();
    assert_ne!(base_file_id, [0u8; 16], "base should have non-zero file_id");
    assert_eq!(base.lineage_depth(), 0, "base should have lineage_depth 0");

    let child = base
        .derive(
            &child_path,
            rvf_types::DerivationType::Clone,
            Some(make_options(dim)),
        )
        .unwrap();

    // Verify child lineage
    assert_ne!(
        *child.file_id(),
        [0u8; 16],
        "child should have non-zero file_id"
    );
    assert_ne!(
        child.file_id(),
        base.file_id(),
        "child file_id should differ from parent"
    );
    assert_eq!(
        child.parent_id(),
        &base_file_id,
        "child's parent_id should match base's file_id"
    );
    assert_eq!(
        child.lineage_depth(),
        1,
        "child should have lineage_depth 1"
    );

    // parent_hash should be non-zero (it's a hash of the parent's manifest)
    let parent_hash = child.file_identity().parent_hash;
    assert_ne!(
        parent_hash, [0u8; 32],
        "child's parent_hash should be non-zero"
    );

    child.close().unwrap();
    base.close().unwrap();

    println!("PASS: derive_creates_lineage");
}

// ===========================================================================
// TEST 8: branch_membership_filter_excludes_deleted
// ===========================================================================

/// When branching a store that has deleted vectors, the membership filter
/// should exclude the deleted ones.
///
/// Note: Uses contiguous IDs starting from 0 so they fit within the
/// MembershipFilter bitmap capacity (= total_vecs count).
#[test]
fn branch_membership_filter_excludes_deleted() {
    let dir = TempDir::new().unwrap();
    let base_path = dir.path().join("base_del.rvf");
    let child_path = dir.path().join("child_del.rvf");
    let dim: u16 = 4;

    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
    let vectors: Vec<Vec<f32>> = (0..5).map(|i| vec![i as f32; dim as usize]).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..5).collect();
    base.ingest_batch(&refs, &ids, None).unwrap();

    // Delete vectors 1 and 3
    base.delete(&[1, 3]).unwrap();

    // Branch
    let child = base.branch(&child_path).unwrap();

    let filter = child.membership_filter().unwrap();
    // The filter capacity = total_vecs = 5 (including deleted)
    // But deleted vectors should be excluded from the membership filter
    assert!(filter.contains(0), "vector 0 should be visible");
    assert!(!filter.contains(1), "deleted vector 1 should be excluded");
    assert!(filter.contains(2), "vector 2 should be visible");
    assert!(!filter.contains(3), "deleted vector 3 should be excluded");
    assert!(filter.contains(4), "vector 4 should be visible");
    assert_eq!(filter.member_count(), 3, "3 vectors should be visible");

    child.close().unwrap();
    base.close().unwrap();

    println!("PASS: branch_membership_filter_excludes_deleted");
}
