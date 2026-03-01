//! Integration test: .rvdna extension → Rvdna profile; .rvf → Generic.
//!
//! Verifies from_extension() / extension() round-trip for all profiles.

use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{RvfOptions, RvfStore};
use rvf_types::DomainProfile;
use tempfile::TempDir;

#[test]
fn extension_round_trip_all_profiles() {
    let profiles = [
        (DomainProfile::Generic, "rvf"),
        (DomainProfile::Rvdna, "rvdna"),
        (DomainProfile::RvText, "rvtext"),
        (DomainProfile::RvGraph, "rvgraph"),
        (DomainProfile::RvVision, "rvvis"),
    ];

    for (profile, ext) in profiles {
        assert_eq!(
            profile.extension(),
            ext,
            "extension mismatch for {profile:?}"
        );
        let back = DomainProfile::from_extension(ext).unwrap();
        assert_eq!(back, profile, "from_extension round-trip failed for {ext}");
    }
}

#[test]
fn extension_case_insensitive() {
    assert_eq!(
        DomainProfile::from_extension("RVDNA"),
        Some(DomainProfile::Rvdna)
    );
    assert_eq!(
        DomainProfile::from_extension("Rvf"),
        Some(DomainProfile::Generic)
    );
    assert_eq!(
        DomainProfile::from_extension("RVTEXT"),
        Some(DomainProfile::RvText)
    );
    assert_eq!(
        DomainProfile::from_extension("RvGraph"),
        Some(DomainProfile::RvGraph)
    );
    assert_eq!(
        DomainProfile::from_extension("RVVIS"),
        Some(DomainProfile::RvVision)
    );
}

#[test]
fn unknown_extension_returns_none() {
    assert_eq!(DomainProfile::from_extension("txt"), None);
    assert_eq!(DomainProfile::from_extension("bin"), None);
    assert_eq!(DomainProfile::from_extension(""), None);
    assert_eq!(DomainProfile::from_extension("rvf2"), None);
}

#[test]
fn rvdna_file_creates_successfully() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.rvdna");

    let options = RvfOptions {
        dimension: 4,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let store = RvfStore::create(&path, options).unwrap();
    assert_ne!(*store.file_id(), [0u8; 16]);
    store.close().unwrap();

    // Reopen and verify it works
    let store = RvfStore::open(&path).unwrap();
    let query = vec![1.0, 0.0, 0.0, 0.0];
    let results = store
        .query(&query, 1, &rvf_runtime::QueryOptions::default())
        .unwrap();
    assert!(results.is_empty());
    store.close().unwrap();
}

#[test]
fn derive_parent_rvf_to_child_rvdna() {
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.rvf");
    let child_path = dir.path().join("child.rvdna");

    let options = RvfOptions {
        dimension: 4,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let parent = RvfStore::create(&parent_path, options).unwrap();
    let child = parent
        .derive(&child_path, rvf_types::DerivationType::Clone, None)
        .unwrap();

    // Child should have parent linkage
    assert_eq!(child.parent_id(), parent.file_id());
    assert_eq!(child.lineage_depth(), 1);

    child.close().unwrap();
    parent.close().unwrap();
}
