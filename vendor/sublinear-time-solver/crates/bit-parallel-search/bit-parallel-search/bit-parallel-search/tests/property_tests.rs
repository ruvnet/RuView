use bit_parallel_search::{BitParallelSearcher, find};
use proptest::prelude::*;

// Property-based tests to ensure correctness across all possible inputs
// These tests generate thousands of random test cases to find edge cases

proptest! {
    #[test]
    fn prop_find_matches_naive(
        text in prop::collection::vec(any::<u8>(), 0..1000),
        pattern in prop::collection::vec(any::<u8>(), 1..100)
    ) {
        let naive_result = naive_search(&text, &pattern);
        let bit_parallel_result = find(&text, &pattern);
        prop_assert_eq!(naive_result, bit_parallel_result);
    }

    #[test]
    fn prop_searcher_reuse_consistent(
        texts in prop::collection::vec(
            prop::collection::vec(any::<u8>(), 0..500),
            1..10
        ),
        pattern in prop::collection::vec(any::<u8>(), 1..64)
    ) {
        let searcher = BitParallelSearcher::new(&pattern);

        for text in &texts {
            let reused_result = searcher.find_in(text);
            let fresh_result = find(text, &pattern);
            prop_assert_eq!(reused_result, fresh_result);
        }
    }

    #[test]
    fn prop_count_matches_find_all(
        text in prop::collection::vec(any::<u8>(), 0..1000),
        pattern in prop::collection::vec(any::<u8>(), 1..32)
    ) {
        let searcher = BitParallelSearcher::new(&pattern);
        let count_result = searcher.count_in(&text);

        #[cfg(feature = "std")]
        {
            let find_all_count = searcher.find_all_in(&text).count();
            prop_assert_eq!(count_result, find_all_count);
        }

        // Also verify against naive counting
        let naive_count = naive_count(&text, &pattern);
        prop_assert_eq!(count_result, naive_count);
    }

    #[test]
    fn prop_empty_pattern_behavior(
        text in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        // Empty pattern should return Some(0) for any non-empty text
        let result = find(&text, &[]);
        if text.is_empty() {
            prop_assert_eq!(result, Some(0));
        } else {
            prop_assert_eq!(result, Some(0));
        }
    }

    #[test]
    fn prop_pattern_longer_than_text(
        text in prop::collection::vec(any::<u8>(), 0..50),
        pattern in prop::collection::vec(any::<u8>(), 51..100)
    ) {
        let result = find(&text, &pattern);
        prop_assert_eq!(result, None);
    }

    #[test]
    fn prop_single_byte_patterns(
        text in prop::collection::vec(any::<u8>(), 0..1000),
        byte in any::<u8>()
    ) {
        let pattern = vec![byte];
        let bit_parallel_result = find(&text, &pattern);
        let naive_result = naive_search(&text, &pattern);
        prop_assert_eq!(bit_parallel_result, naive_result);
    }

    #[test]
    fn prop_repeated_byte_patterns(
        byte in any::<u8>(),
        pattern_len in 1..64usize,
        text_len in 0..1000usize
    ) {
        let pattern = vec![byte; pattern_len];
        let text: Vec<u8> = (0..text_len).map(|_| byte).collect();

        let bit_parallel_result = find(&text, &pattern);
        let naive_result = naive_search(&text, &pattern);
        prop_assert_eq!(bit_parallel_result, naive_result);
    }

    #[test]
    fn prop_pattern_at_boundaries(
        prefix in prop::collection::vec(any::<u8>(), 0..100),
        pattern in prop::collection::vec(any::<u8>(), 1..64),
        suffix in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        // Test pattern at start
        let mut text = pattern.clone();
        text.extend_from_slice(&suffix);
        prop_assert_eq!(find(&text, &pattern), Some(0));

        // Test pattern at end
        let mut text = prefix.clone();
        text.extend_from_slice(&pattern);
        let expected_pos = if text.len() >= pattern.len() {
            Some(text.len() - pattern.len())
        } else {
            None
        };
        prop_assert_eq!(find(&text, &pattern), expected_pos);

        // Test pattern in middle
        let mut text = prefix;
        text.extend_from_slice(&pattern);
        text.extend_from_slice(&suffix);
        if text.len() >= pattern.len() {
            let result = find(&text, &pattern);
            prop_assert!(result.is_some());
        }
    }

    #[test]
    fn prop_overlapping_patterns(
        base_pattern in prop::collection::vec(any::<u8>(), 2..32),
        repeat_count in 2..10usize
    ) {
        // Create overlapping pattern like "abcabc" from "abc"
        let mut overlapping = base_pattern.clone();
        for _ in 1..repeat_count {
            overlapping.extend_from_slice(&base_pattern);
        }

        let searcher = BitParallelSearcher::new(&base_pattern);
        let count = searcher.count_in(&overlapping);

        // Should find at least repeat_count - 1 overlapping occurrences
        prop_assert!(count >= repeat_count - 1);

        // Verify against naive implementation
        let naive_count = naive_count(&overlapping, &base_pattern);
        prop_assert_eq!(count, naive_count);
    }

    #[test]
    fn prop_long_pattern_fallback(
        text in prop::collection::vec(any::<u8>(), 0..1000),
        pattern in prop::collection::vec(any::<u8>(), 65..128) // Force fallback
    ) {
        let bit_parallel_result = find(&text, &pattern);
        let naive_result = naive_search(&text, &pattern);
        prop_assert_eq!(bit_parallel_result, naive_result);
    }

    #[test]
    fn prop_exists_consistency(
        text in prop::collection::vec(any::<u8>(), 0..500),
        pattern in prop::collection::vec(any::<u8>(), 1..64)
    ) {
        let searcher = BitParallelSearcher::new(&pattern);
        let exists = searcher.exists_in(&text);
        let find_result = searcher.find_in(&text);

        prop_assert_eq!(exists, find_result.is_some());
    }
}

// Naive implementations for property test verification
fn naive_search(text: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() {
        return Some(0);
    }

    if pattern.len() > text.len() {
        return None;
    }

    (0..=text.len() - pattern.len())
        .find(|&i| &text[i..i + pattern.len()] == pattern)
}

fn naive_count(text: &[u8], pattern: &[u8]) -> usize {
    if pattern.is_empty() || pattern.len() > text.len() {
        return 0;
    }

    let mut count = 0;
    for i in 0..=text.len() - pattern.len() {
        if &text[i..i + pattern.len()] == pattern {
            count += 1;
        }
    }
    count
}

// Regression tests for specific edge cases found during development
#[cfg(test)]
mod regression_tests {
    use super::*;

    #[test]
    fn test_all_same_bytes() {
        let searcher = BitParallelSearcher::new(&[0xFF; 64]);
        let text = [0xFF; 100];

        assert_eq!(searcher.find_in(&text), Some(0));
        assert_eq!(searcher.count_in(&text), 37); // 100 - 64 + 1
    }

    #[test]
    fn test_alternating_pattern() {
        let pattern = [0xAA, 0x55, 0xAA, 0x55];
        let searcher = BitParallelSearcher::new(&pattern);
        let text = [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55];

        assert_eq!(searcher.find_in(&text), Some(0));
        assert_eq!(searcher.count_in(&text), 3); // Overlapping matches
    }

    #[test]
    fn test_pattern_near_64_byte_boundary() {
        // Test patterns at exactly 64 bytes (boundary case)
        let pattern_63 = vec![b'A'; 63];
        let pattern_64 = vec![b'A'; 64];
        let pattern_65 = vec![b'A'; 65];

        let text = vec![b'A'; 100];

        let searcher_63 = BitParallelSearcher::new(&pattern_63);
        let searcher_64 = BitParallelSearcher::new(&pattern_64);
        let searcher_65 = BitParallelSearcher::new(&pattern_65);

        assert_eq!(searcher_63.find_in(&text), Some(0));
        assert_eq!(searcher_64.find_in(&text), Some(0));
        assert_eq!(searcher_65.find_in(&text), Some(0)); // Uses fallback
    }

    #[test]
    fn test_zero_bytes() {
        let pattern = [0x00, 0x01, 0x00];
        let searcher = BitParallelSearcher::new(&pattern);
        let text = [0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00];

        assert_eq!(searcher.find_in(&text), Some(0));
        assert_eq!(searcher.count_in(&text), 2);
    }

    #[test]
    fn test_high_bit_patterns() {
        let pattern = [0x80, 0xFF, 0x7F];
        let searcher = BitParallelSearcher::new(&pattern);
        let text = [0x80, 0xFF, 0x7F, 0x00, 0x80, 0xFF, 0x7F];

        assert_eq!(searcher.find_in(&text), Some(0));
        assert_eq!(searcher.count_in(&text), 2);
    }
}