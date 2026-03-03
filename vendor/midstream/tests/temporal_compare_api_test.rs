/// Integration tests for temporal-compare pattern detection APIs
///
/// This test suite verifies that the find_similar() and detect_pattern() APIs
/// work correctly with the published crate.

use midstreamer_temporal_compare::{TemporalComparator, Pattern, SimilarityMatch};

#[test]
fn test_find_similar_with_f64() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    // Create a time series with repeating patterns
    let series = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    let pattern = vec![3.0, 4.0, 5.0];

    // Find similar patterns with a reasonable threshold
    let matches = comparator.find_similar(&series, &pattern, 1.0);

    // Verify we found the expected matches
    assert!(!matches.is_empty(), "Should find at least one match");
    assert_eq!(matches.len(), 2, "Should find exactly 2 matches");

    // Verify the indices are correct
    assert_eq!(matches[0].0, 2, "First match should be at index 2");
    assert_eq!(matches[1].0, 5, "Second match should be at index 5");

    // Verify distances are within threshold
    for (idx, distance) in &matches {
        assert!(*distance <= 1.0, "Distance {} at index {} exceeds threshold", distance, idx);
    }
}

#[test]
fn test_detect_pattern_exists() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    let series = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 4.0, 5.0];
    let pattern = vec![3.0, 4.0, 5.0];

    // Detect if pattern exists
    let found = comparator.detect_pattern(&series, &pattern, 0.5);

    assert!(found, "Pattern should be detected in the series");
}

#[test]
fn test_detect_pattern_not_exists() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    let series = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let pattern = vec![10.0, 20.0, 30.0];

    // Detect if pattern exists with strict threshold
    let found = comparator.detect_pattern(&series, &pattern, 0.5);

    assert!(!found, "Pattern should not be detected (too different)");
}

#[test]
fn test_find_similar_generic_with_integers() {
    let comparator: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

    let haystack = vec![1, 2, 3, 4, 5, 3, 4, 5, 6];
    let needle = vec![3, 4, 5];

    // Use the generic API with normalized threshold
    let matches = comparator.find_similar_generic(&haystack, &needle, 0.1).unwrap();

    assert_eq!(matches.len(), 2, "Should find 2 exact matches");
    assert_eq!(matches[0].start_index, 2);
    assert_eq!(matches[1].start_index, 5);

    // Verify similarity scores
    for m in &matches {
        assert!(m.similarity > 0.9, "Exact matches should have high similarity");
    }
}

#[test]
fn test_detect_recurring_patterns() {
    let comparator: TemporalComparator<char> = TemporalComparator::new(100, 1000);

    // Create sequence with recurring patterns
    let sequence = vec!['a', 'b', 'c', 'a', 'b', 'c', 'a', 'b', 'c'];

    // Detect patterns of length 3
    let patterns = comparator.detect_recurring_patterns(&sequence, 3, 3).unwrap();

    assert!(!patterns.is_empty(), "Should detect recurring patterns");

    // Find the 'abc' pattern
    let abc_pattern = patterns.iter()
        .find(|p| p.sequence == vec!['a', 'b', 'c']);

    assert!(abc_pattern.is_some(), "Should find 'abc' pattern");

    let pattern = abc_pattern.unwrap();
    assert_eq!(pattern.frequency(), 3, "Pattern should occur 3 times");
    assert!(pattern.confidence > 0.0, "Should have positive confidence");
}

#[test]
fn test_detect_fuzzy_patterns() {
    let comparator: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

    // Sequence with similar but not identical patterns
    let sequence = vec![1, 2, 3, 1, 2, 4, 1, 2, 3];

    // Detect fuzzy patterns (should group [1,2,3] and [1,2,4] together)
    let patterns = comparator.detect_fuzzy_patterns(&sequence, 3, 3, 0.7).unwrap();

    assert!(!patterns.is_empty(), "Should detect fuzzy patterns");

    // Should find at least one pattern that occurs multiple times
    let has_multiple = patterns.iter().any(|p| p.frequency() >= 2);
    assert!(has_multiple, "Should find patterns with multiple occurrences");
}

#[test]
fn test_pattern_struct_api() {
    let sequence = vec![1, 2, 3];
    let occurrences = vec![0, 5, 10];
    let confidence = 0.85;

    let pattern = Pattern::new(sequence.clone(), occurrences.clone(), confidence);

    // Verify Pattern API
    assert_eq!(pattern.sequence, sequence);
    assert_eq!(pattern.occurrences, occurrences);
    assert_eq!(pattern.confidence, confidence);
    assert_eq!(pattern.frequency(), 3);
    assert_eq!(pattern.length(), 3);
}

#[test]
fn test_similarity_match_struct() {
    let match1 = SimilarityMatch::new(0, 0.5);

    assert_eq!(match1.start_index, 0);
    assert_eq!(match1.distance, 0.5);
    assert!(match1.similarity > 0.0 && match1.similarity <= 1.0);

    // Lower distance should give higher similarity
    let match2 = SimilarityMatch::new(0, 0.1);
    assert!(match2.similarity > match1.similarity,
        "Lower distance should yield higher similarity");
}

#[test]
fn test_edge_case_empty_pattern() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    let series = vec![1.0, 2.0, 3.0];
    let pattern: Vec<f64> = vec![];

    let matches = comparator.find_similar(&series, &pattern, 1.0);
    assert!(matches.is_empty(), "Empty pattern should return no matches");

    let found = comparator.detect_pattern(&series, &pattern, 1.0);
    assert!(!found, "Empty pattern should not be detected");
}

#[test]
fn test_edge_case_pattern_longer_than_series() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    let series = vec![1.0, 2.0];
    let pattern = vec![1.0, 2.0, 3.0, 4.0, 5.0];

    let matches = comparator.find_similar(&series, &pattern, 1.0);
    assert!(matches.is_empty(), "Pattern longer than series should return no matches");
}

#[test]
fn test_approximate_matching_with_threshold() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    // Series with approximate match
    let series = vec![1.0, 2.0, 3.1, 4.2, 5.0, 6.0];
    let pattern = vec![3.0, 4.0, 5.0];

    // Strict threshold - should not match
    let strict_matches = comparator.find_similar(&series, &pattern, 0.1);
    assert!(strict_matches.is_empty(), "Strict threshold should reject approximate match");

    // Loose threshold - should match
    let loose_matches = comparator.find_similar(&series, &pattern, 1.5);
    assert!(!loose_matches.is_empty(), "Loose threshold should accept approximate match");
}

#[test]
fn test_results_sorted_by_quality() {
    let comparator: TemporalComparator<f64> = TemporalComparator::new(100, 1000);

    // Series with exact and approximate matches
    let series = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.5, 4.5, 5.5];
    let pattern = vec![3.0, 4.0, 5.0];

    let matches = comparator.find_similar(&series, &pattern, 2.0);

    assert!(!matches.is_empty(), "Should find matches");

    // Verify results are sorted by distance (best first)
    for i in 0..matches.len().saturating_sub(1) {
        assert!(matches[i].1 <= matches[i + 1].1,
            "Results should be sorted by distance (ascending)");
    }

    // First match should be the exact one
    assert!(matches[0].1 < 0.1, "Best match should have very low distance");
}

#[test]
fn test_caching_behavior() {
    let comparator: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

    let haystack = vec![1, 2, 3, 4, 5];
    let needle = vec![3, 4, 5];

    // Clear cache to start fresh
    comparator.clear_cache();

    // First call - should be cache miss
    let _ = comparator.find_similar_generic(&haystack, &needle, 0.1).unwrap();

    // Second call - should be cache hit
    let _ = comparator.find_similar_generic(&haystack, &needle, 0.1).unwrap();

    let stats = comparator.cache_stats();
    assert!(stats.hits > 0, "Should have cache hits");
    assert!(stats.hits + stats.misses > 0, "Should have cache activity");
}

#[test]
fn test_comprehensive_workflow() {
    let comparator: TemporalComparator<i32> = TemporalComparator::new(100, 1000);

    // Create a rich sequence
    let sequence = vec![
        1, 2, 3, 4,    // Pattern A
        1, 2, 3, 4,    // Pattern A repeat
        5, 6, 7,       // Pattern B
        5, 6, 7,       // Pattern B repeat
        1, 2, 3, 4,    // Pattern A again
    ];

    // Test 1: Exact pattern detection
    let exact_patterns = comparator.detect_recurring_patterns(&sequence, 3, 4).unwrap();
    assert!(!exact_patterns.is_empty(), "Should detect exact patterns");

    // Test 2: Fuzzy pattern detection
    let fuzzy_patterns = comparator.detect_fuzzy_patterns(&sequence, 3, 4, 0.8).unwrap();
    assert!(!fuzzy_patterns.is_empty(), "Should detect fuzzy patterns");

    // Test 3: Similarity search
    let needle = vec![1, 2, 3, 4];
    let matches = comparator.find_similar_generic(&sequence, &needle, 0.1).unwrap();
    assert_eq!(matches.len(), 3, "Should find 3 occurrences of pattern");

    // Test 4: Simple detection
    let found = comparator.detect_pattern(
        &sequence.iter().map(|&x| x as f64).collect::<Vec<_>>(),
        &needle.iter().map(|&x| x as f64).collect::<Vec<_>>(),
        1.0
    );
    assert!(found, "Pattern should be detected");

    // Test 5: Verify caching is working
    let stats = comparator.cache_stats();
    assert!(stats.size > 0, "Cache should have entries");
}
