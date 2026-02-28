//! Integration tests for MembershipFilter with HNSW-like traversal semantics.
//!
//! Tests include/exclude modes, bitmap operations, serialization round-trips,
//! and edge cases around word boundaries and empty filters.

use rvf_runtime::MembershipFilter;
use rvf_types::membership::{FilterMode, MembershipHeader, MEMBERSHIP_MAGIC};

// ===========================================================================
// TEST 1: include_mode_empty_filter_is_empty_view
// ===========================================================================

/// An empty include-mode filter means nothing is visible (fail-safe).
#[test]
fn include_mode_empty_filter_is_empty_view() {
    let filter = MembershipFilter::new_include(1000);
    for id in 0..1000 {
        assert!(
            !filter.contains(id),
            "empty include filter should not contain vector {id}"
        );
    }
    assert_eq!(filter.member_count(), 0);
    assert_eq!(filter.vector_count(), 1000);
    assert_eq!(filter.mode(), FilterMode::Include);

    println!("PASS: include_mode_empty_filter_is_empty_view");
}

// ===========================================================================
// TEST 2: include_mode_subset
// ===========================================================================

/// Add a subset of vector IDs to an include-mode filter, verify membership.
#[test]
fn include_mode_subset() {
    let mut filter = MembershipFilter::new_include(500);

    // Add specific IDs
    let included_ids: Vec<u64> = vec![0, 10, 50, 100, 200, 499];
    for &id in &included_ids {
        filter.add(id);
    }

    // Verify included
    for &id in &included_ids {
        assert!(filter.contains(id), "filter should contain {id}");
    }

    // Verify excluded
    let excluded_ids: Vec<u64> = vec![1, 9, 11, 49, 51, 99, 101, 199, 201, 498];
    for &id in &excluded_ids {
        assert!(!filter.contains(id), "filter should not contain {id}");
    }

    assert_eq!(filter.member_count(), included_ids.len() as u64);

    println!("PASS: include_mode_subset");
}

// ===========================================================================
// TEST 3: exclude_mode_basics
// ===========================================================================

/// In exclude mode, all vectors are visible by default; adding an ID
/// to the bitmap excludes it.
#[test]
fn exclude_mode_basics() {
    let mut filter = MembershipFilter::new_exclude(100);

    // Initially everything is visible
    for id in 0..100 {
        assert!(
            filter.contains(id),
            "exclude filter should contain {id} initially"
        );
    }

    // Exclude some vectors
    filter.add(10);
    filter.add(50);
    filter.add(90);

    assert!(!filter.contains(10), "vector 10 should be excluded");
    assert!(!filter.contains(50), "vector 50 should be excluded");
    assert!(!filter.contains(90), "vector 90 should be excluded");
    assert!(filter.contains(0), "vector 0 should still be visible");
    assert!(filter.contains(49), "vector 49 should still be visible");
    assert!(filter.contains(99), "vector 99 should still be visible");

    assert_eq!(filter.member_count(), 3); // 3 bits set = 3 excluded
    assert_eq!(filter.mode(), FilterMode::Exclude);

    println!("PASS: exclude_mode_basics");
}

// ===========================================================================
// TEST 4: add_remove_roundtrip
// ===========================================================================

/// Adding then removing a vector should restore the original state.
#[test]
fn add_remove_roundtrip() {
    let mut filter = MembershipFilter::new_include(64);

    filter.add(10);
    assert!(filter.contains(10));
    assert_eq!(filter.member_count(), 1);

    filter.remove(10);
    assert!(!filter.contains(10));
    assert_eq!(filter.member_count(), 0);

    // Double remove should be a no-op
    filter.remove(10);
    assert_eq!(filter.member_count(), 0);

    // Double add should not double-count
    filter.add(20);
    filter.add(20);
    assert_eq!(filter.member_count(), 1);

    println!("PASS: add_remove_roundtrip");
}

// ===========================================================================
// TEST 5: out_of_bounds_ignored
// ===========================================================================

/// Adding a vector ID beyond vector_count should be silently ignored.
#[test]
fn out_of_bounds_ignored() {
    let mut filter = MembershipFilter::new_include(10);

    filter.add(100); // way out of bounds
    assert_eq!(filter.member_count(), 0);
    assert!(!filter.contains(100));

    filter.add(10); // at boundary (0-indexed, so 10 is out of range for count=10)
    assert_eq!(filter.member_count(), 0);

    filter.add(9); // last valid
    assert_eq!(filter.member_count(), 1);
    assert!(filter.contains(9));

    println!("PASS: out_of_bounds_ignored");
}

// ===========================================================================
// TEST 6: bitmap_word_boundaries
// ===========================================================================

/// Test vectors at the 64-bit word boundaries (0, 63, 64, 127, 128, etc.).
#[test]
fn bitmap_word_boundaries() {
    let mut filter = MembershipFilter::new_include(256);

    let boundary_ids: Vec<u64> = vec![0, 1, 62, 63, 64, 65, 126, 127, 128, 129, 191, 192, 255];
    for &id in &boundary_ids {
        filter.add(id);
    }

    for &id in &boundary_ids {
        assert!(filter.contains(id), "boundary ID {id} should be in filter");
    }

    // Verify IDs adjacent to boundaries are NOT in filter
    let non_boundary: Vec<u64> = vec![2, 61, 66, 125, 130, 190, 193, 254];
    for &id in &non_boundary {
        assert!(
            !filter.contains(id),
            "non-boundary ID {id} should NOT be in filter"
        );
    }

    assert_eq!(filter.member_count(), boundary_ids.len() as u64);

    println!("PASS: bitmap_word_boundaries");
}

// ===========================================================================
// TEST 7: serialization_round_trip_include
// ===========================================================================

/// Serialize an include-mode filter to bytes, reconstruct it, and verify
/// all membership is preserved.
#[test]
fn serialization_round_trip_include() {
    let mut filter = MembershipFilter::new_include(300);
    let test_ids: Vec<u64> = vec![0, 1, 63, 64, 127, 128, 199, 250, 299];
    for &id in &test_ids {
        filter.add(id);
    }
    filter.bump_generation();
    filter.bump_generation();

    let header = filter.to_header();
    let bitmap_data = filter.serialize();

    // Verify header fields
    assert_eq!(header.magic, MEMBERSHIP_MAGIC);
    assert_eq!(header.version, 1);
    assert_eq!(header.filter_mode, FilterMode::Include as u8);
    assert_eq!(header.vector_count, 300);
    assert_eq!(header.member_count, test_ids.len() as u64);
    assert_eq!(header.generation_id, 2);

    // Deserialize
    let filter2 = MembershipFilter::deserialize(&bitmap_data, &header).unwrap();

    assert_eq!(filter2.vector_count(), 300);
    assert_eq!(filter2.member_count(), test_ids.len() as u64);
    assert_eq!(filter2.generation_id(), 2);
    assert_eq!(filter2.mode(), FilterMode::Include);

    for &id in &test_ids {
        assert!(
            filter2.contains(id),
            "deserialized filter should contain {id}"
        );
    }

    // Non-members should still be excluded
    assert!(!filter2.contains(2));
    assert!(!filter2.contains(100));
    assert!(!filter2.contains(200));

    println!("PASS: serialization_round_trip_include");
}

// ===========================================================================
// TEST 8: serialization_round_trip_exclude
// ===========================================================================

/// Serialize an exclude-mode filter and verify round-trip.
#[test]
fn serialization_round_trip_exclude() {
    let mut filter = MembershipFilter::new_exclude(200);
    filter.add(10); // exclude vector 10
    filter.add(100); // exclude vector 100

    let header = filter.to_header();
    let bitmap_data = filter.serialize();

    let filter2 = MembershipFilter::deserialize(&bitmap_data, &header).unwrap();

    assert_eq!(filter2.mode(), FilterMode::Exclude);
    assert_eq!(filter2.vector_count(), 200);
    assert_eq!(filter2.member_count(), 2);

    // In exclude mode: set bits mean excluded
    assert!(!filter2.contains(10), "vector 10 should be excluded");
    assert!(!filter2.contains(100), "vector 100 should be excluded");
    assert!(filter2.contains(0), "vector 0 should be visible");
    assert!(filter2.contains(50), "vector 50 should be visible");
    assert!(filter2.contains(199), "vector 199 should be visible");

    println!("PASS: serialization_round_trip_exclude");
}

// ===========================================================================
// TEST 9: generation_id_tracking
// ===========================================================================

/// Verify that generation_id increments correctly and survives serialization.
#[test]
fn generation_id_tracking() {
    let mut filter = MembershipFilter::new_include(64);
    assert_eq!(filter.generation_id(), 0);

    filter.bump_generation();
    assert_eq!(filter.generation_id(), 1);

    filter.bump_generation();
    filter.bump_generation();
    assert_eq!(filter.generation_id(), 3);

    // Serialize and verify generation survives
    let header = filter.to_header();
    let bitmap_data = filter.serialize();
    let filter2 = MembershipFilter::deserialize(&bitmap_data, &header).unwrap();
    assert_eq!(filter2.generation_id(), 3);

    println!("PASS: generation_id_tracking");
}

// ===========================================================================
// TEST 10: large_filter_stress
// ===========================================================================

/// Stress test with a large number of vectors to verify bitmap correctness.
#[test]
fn large_filter_stress() {
    let total = 10_000u64;
    let mut filter = MembershipFilter::new_include(total);

    // Add every 3rd vector
    let mut expected_count = 0u64;
    for id in (0..total).step_by(3) {
        filter.add(id);
        expected_count += 1;
    }

    assert_eq!(filter.member_count(), expected_count);

    // Verify membership
    for id in 0..total {
        let expected = id % 3 == 0;
        assert_eq!(
            filter.contains(id),
            expected,
            "vector {id}: expected contains={expected}"
        );
    }

    // Serialize and round-trip
    let header = filter.to_header();
    let bitmap_data = filter.serialize();
    let filter2 = MembershipFilter::deserialize(&bitmap_data, &header).unwrap();

    assert_eq!(filter2.member_count(), expected_count);

    // Spot-check a few IDs after round-trip
    assert!(filter2.contains(0));
    assert!(!filter2.contains(1));
    assert!(!filter2.contains(2));
    assert!(filter2.contains(3));
    assert!(filter2.contains(9999));
    assert!(!filter2.contains(9998));

    println!("PASS: large_filter_stress");
}

// ===========================================================================
// TEST 11: membership_header_round_trip
// ===========================================================================

/// Test that MembershipHeader serializes and deserializes correctly.
#[test]
fn membership_header_round_trip() {
    let header = MembershipHeader {
        magic: MEMBERSHIP_MAGIC,
        version: 1,
        filter_type: 0, // Bitmap
        filter_mode: FilterMode::Include as u8,
        vector_count: 100_000,
        member_count: 50_000,
        filter_offset: 96,
        filter_size: 12_500,
        generation_id: 7,
        filter_hash: [0xAB; 32],
        bloom_offset: 0,
        bloom_size: 0,
        _reserved: 0,
        _reserved2: [0u8; 8],
    };

    let bytes = header.to_bytes();
    let decoded = MembershipHeader::from_bytes(&bytes).unwrap();

    assert_eq!(decoded.magic, MEMBERSHIP_MAGIC);
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.filter_mode, FilterMode::Include as u8);
    assert_eq!(decoded.vector_count, 100_000);
    assert_eq!(decoded.member_count, 50_000);
    assert_eq!(decoded.filter_size, 12_500);
    assert_eq!(decoded.generation_id, 7);
    assert_eq!(decoded.filter_hash, [0xAB; 32]);

    println!("PASS: membership_header_round_trip");
}
