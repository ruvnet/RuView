//! Syndrome processing tests for ruQu coherence gate
//!
//! Tests for detector bitmap operations with SIMD-like performance,
//! syndrome buffer ring behavior, delta computation accuracy,
//! and buffer overflow handling.

use ruqu::syndrome::{
    BufferStatistics, DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound,
};
use ruqu::MAX_DETECTORS;

// ============================================================================
// DetectorBitmap Tests - SIMD-like Performance
// ============================================================================

mod detector_bitmap_tests {
    use super::*;

    #[test]
    fn test_bitmap_creation() {
        let bitmap = DetectorBitmap::new(64);

        assert_eq!(bitmap.detector_count(), 64);
        assert_eq!(bitmap.fired_count(), 0);
        assert!(bitmap.is_empty());
    }

    #[test]
    fn test_bitmap_max_detectors() {
        let bitmap = DetectorBitmap::new(MAX_DETECTORS);

        assert_eq!(bitmap.detector_count(), MAX_DETECTORS);
        assert_eq!(bitmap.fired_count(), 0);
    }

    #[test]
    #[should_panic(expected = "count exceeds maximum")]
    fn test_bitmap_overflow_panics() {
        DetectorBitmap::new(MAX_DETECTORS + 1);
    }

    #[test]
    fn test_bitmap_set_get() {
        let mut bitmap = DetectorBitmap::new(128);

        bitmap.set(0, true);
        bitmap.set(63, true);
        bitmap.set(64, true);
        bitmap.set(127, true);

        assert!(bitmap.get(0));
        assert!(bitmap.get(63));
        assert!(bitmap.get(64));
        assert!(bitmap.get(127));
        assert!(!bitmap.get(1));
        assert!(!bitmap.get(100));
    }

    #[test]
    fn test_bitmap_set_clear() {
        let mut bitmap = DetectorBitmap::new(64);

        bitmap.set(10, true);
        assert!(bitmap.get(10));

        bitmap.set(10, false);
        assert!(!bitmap.get(10));
    }

    #[test]
    fn test_bitmap_fired_count_popcount() {
        let mut bitmap = DetectorBitmap::new(256);

        // Set every 10th detector
        for i in (0..256).step_by(10) {
            bitmap.set(i, true);
        }

        assert_eq!(bitmap.fired_count(), 26); // 0, 10, 20, ..., 250
    }

    #[test]
    fn test_bitmap_fired_count_all() {
        let mut bitmap = DetectorBitmap::new(64);

        for i in 0..64 {
            bitmap.set(i, true);
        }

        assert_eq!(bitmap.fired_count(), 64);
    }

    #[test]
    fn test_bitmap_iter_fired() {
        let mut bitmap = DetectorBitmap::new(128);

        bitmap.set(5, true);
        bitmap.set(64, true);
        bitmap.set(100, true);

        let fired: Vec<usize> = bitmap.iter_fired().collect();

        assert_eq!(fired, vec![5, 64, 100]);
    }

    #[test]
    fn test_bitmap_iter_fired_empty() {
        let bitmap = DetectorBitmap::new(64);

        let fired: Vec<usize> = bitmap.iter_fired().collect();

        assert!(fired.is_empty());
    }

    #[test]
    fn test_bitmap_iter_fired_all() {
        let mut bitmap = DetectorBitmap::new(64);

        for i in 0..64 {
            bitmap.set(i, true);
        }

        let fired: Vec<usize> = bitmap.iter_fired().collect();

        assert_eq!(fired.len(), 64);
        for (i, &val) in fired.iter().enumerate() {
            assert_eq!(val, i);
        }
    }

    #[test]
    fn test_bitmap_xor() {
        let mut a = DetectorBitmap::new(64);
        a.set(0, true);
        a.set(5, true);
        a.set(10, true);

        let mut b = DetectorBitmap::new(64);
        b.set(5, true);
        b.set(10, true);
        b.set(20, true);

        let result = a.xor(&b);

        assert!(result.get(0)); // Only in a
        assert!(!result.get(5)); // In both
        assert!(!result.get(10)); // In both
        assert!(result.get(20)); // Only in b
        assert_eq!(result.fired_count(), 2);
    }

    #[test]
    fn test_bitmap_and() {
        let mut a = DetectorBitmap::new(64);
        a.set(0, true);
        a.set(5, true);

        let mut b = DetectorBitmap::new(64);
        b.set(5, true);
        b.set(10, true);

        let result = a.and(&b);

        assert!(!result.get(0));
        assert!(result.get(5));
        assert!(!result.get(10));
        assert_eq!(result.fired_count(), 1);
    }

    #[test]
    fn test_bitmap_or() {
        let mut a = DetectorBitmap::new(64);
        a.set(0, true);
        a.set(5, true);

        let mut b = DetectorBitmap::new(64);
        b.set(5, true);
        b.set(10, true);

        let result = a.or(&b);

        assert!(result.get(0));
        assert!(result.get(5));
        assert!(result.get(10));
        assert_eq!(result.fired_count(), 3);
    }

    #[test]
    fn test_bitmap_clear() {
        let mut bitmap = DetectorBitmap::new(64);

        bitmap.set(0, true);
        bitmap.set(10, true);
        assert_eq!(bitmap.fired_count(), 2);

        bitmap.clear();

        assert_eq!(bitmap.fired_count(), 0);
        assert!(bitmap.is_empty());
    }

    #[test]
    fn test_bitmap_from_raw() {
        let bits = [0x0101_0101_0101_0101u64; 16];
        let bitmap = DetectorBitmap::from_raw(bits, 1024);

        // Each word has 8 bits set (every 8th bit)
        assert_eq!(bitmap.fired_count(), 128); // 8 * 16
    }

    #[test]
    fn test_bitmap_raw_bits() {
        let mut bitmap = DetectorBitmap::new(128);
        bitmap.set(0, true);
        bitmap.set(64, true);

        let bits = bitmap.raw_bits();

        assert_eq!(bits[0], 1); // Bit 0 set
        assert_eq!(bits[1], 1); // Bit 0 of word 1 (detector 64)
    }

    // Performance-oriented tests for SIMD-like behavior
    #[test]
    fn test_bitmap_bulk_operations_performance() {
        let mut a = DetectorBitmap::new(1024);
        let mut b = DetectorBitmap::new(1024);

        // Set alternating bits
        for i in (0..1024).step_by(2) {
            a.set(i, true);
        }
        for i in (1..1024).step_by(2) {
            b.set(i, true);
        }

        // These operations should be efficient (operating on 64 bits at a time)
        let xor_result = a.xor(&b);
        assert_eq!(xor_result.fired_count(), 1024); // All bits differ

        let and_result = a.and(&b);
        assert_eq!(and_result.fired_count(), 0); // No overlap

        let or_result = a.or(&b);
        assert_eq!(or_result.fired_count(), 1024); // All bits set
    }

    #[test]
    fn test_bitmap_popcount_performance() {
        let mut bitmap = DetectorBitmap::new(1024);

        // Set all bits
        for i in 0..1024 {
            bitmap.set(i, true);
        }

        // Popcount should use hardware instructions
        assert_eq!(bitmap.popcount(), 1024);
    }
}

// ============================================================================
// SyndromeRound Tests
// ============================================================================

mod syndrome_round_tests {
    use super::*;

    #[test]
    fn test_round_creation() {
        let detectors = DetectorBitmap::new(64);
        let round = SyndromeRound::new(1, 100, 1_000_000, detectors, 5);

        assert_eq!(round.round_id, 1);
        assert_eq!(round.cycle, 100);
        assert_eq!(round.timestamp, 1_000_000);
        assert_eq!(round.source_tile, 5);
        assert_eq!(round.fired_count(), 0);
    }

    #[test]
    fn test_round_struct_syntax() {
        let mut detectors = DetectorBitmap::new(64);
        detectors.set(10, true);

        let round = SyndromeRound {
            round_id: 42,
            cycle: 200,
            timestamp: 2_000_000,
            detectors,
            source_tile: 0,
        };

        assert_eq!(round.round_id, 42);
        assert_eq!(round.fired_count(), 1);
    }

    #[test]
    fn test_round_fired_count() {
        let mut detectors = DetectorBitmap::new(64);
        detectors.set(0, true);
        detectors.set(10, true);
        detectors.set(63, true);

        let round = SyndromeRound::new(1, 100, 1_000_000, detectors, 0);

        assert_eq!(round.fired_count(), 3);
    }

    #[test]
    fn test_round_iter_fired() {
        let mut detectors = DetectorBitmap::new(64);
        detectors.set(5, true);
        detectors.set(10, true);

        let round = SyndromeRound::new(1, 100, 1_000_000, detectors, 0);

        let fired: Vec<usize> = round.iter_fired().collect();
        assert_eq!(fired, vec![5, 10]);
    }

    #[test]
    fn test_round_delta_to() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(0, true);
        d1.set(5, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(5, true);
        d2.set(10, true);

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = round1.delta_to(&round2);

        assert_eq!(delta.from_round, 1);
        assert_eq!(delta.to_round, 2);
        assert_eq!(delta.flip_count(), 2); // 0 and 10 flipped
    }
}

// ============================================================================
// SyndromeBuffer Ring Behavior Tests
// ============================================================================

mod syndrome_buffer_tests {
    use super::*;

    #[test]
    fn test_buffer_creation() {
        let buffer = SyndromeBuffer::new(100);

        assert_eq!(buffer.capacity(), 100);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }

    #[test]
    #[should_panic(expected = "capacity must be positive")]
    fn test_buffer_zero_capacity() {
        SyndromeBuffer::new(0);
    }

    #[test]
    fn test_buffer_push_single() {
        let mut buffer = SyndromeBuffer::new(10);

        let round = SyndromeRound::new(1, 100, 1_000, DetectorBitmap::new(64), 0);
        buffer.push(round);

        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_buffer_push_to_capacity() {
        let mut buffer = SyndromeBuffer::new(10);

        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        assert_eq!(buffer.len(), 10);
        assert!(buffer.is_full());
    }

    #[test]
    fn test_buffer_ring_overflow() {
        let mut buffer = SyndromeBuffer::new(5);

        // Push 10 rounds into buffer of capacity 5
        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        // Should still have capacity 5
        assert_eq!(buffer.len(), 5);

        // Oldest should be round 5 (rounds 0-4 evicted)
        assert!(buffer.get(4).is_none());
        assert!(buffer.get(5).is_some());
    }

    #[test]
    fn test_buffer_watermark_updates() {
        let mut buffer = SyndromeBuffer::new(5);

        // Fill buffer
        for i in 0..5 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let initial_watermark = buffer.watermark();

        // Overflow
        for i in 5..10 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        // Watermark should have advanced
        assert!(buffer.watermark() > initial_watermark);
    }

    #[test]
    fn test_buffer_window_basic() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..50 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let window = buffer.window(10);

        assert_eq!(window.len(), 10);
        assert_eq!(window[0].round_id, 40); // Oldest in window
        assert_eq!(window[9].round_id, 49); // Newest in window
    }

    #[test]
    fn test_buffer_window_larger_than_available() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..5 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let window = buffer.window(100);

        assert_eq!(window.len(), 5); // Only 5 available
    }

    #[test]
    fn test_buffer_window_empty() {
        let buffer = SyndromeBuffer::new(100);

        let window = buffer.window(10);

        assert!(window.is_empty());
    }

    #[test]
    fn test_buffer_get_by_round_id() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..50 {
            let mut detectors = DetectorBitmap::new(64);
            detectors.set(i as usize % 64, true);
            let round = SyndromeRound::new(i, i, i * 1_000, detectors, 0);
            buffer.push(round);
        }

        let round = buffer.get(25);
        assert!(round.is_some());
        assert_eq!(round.unwrap().round_id, 25);

        let nonexistent = buffer.get(999);
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_buffer_get_evicted_round() {
        let mut buffer = SyndromeBuffer::new(5);

        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        // Rounds 0-4 should be evicted
        for i in 0..5 {
            assert!(buffer.get(i).is_none());
        }

        // Rounds 5-9 should exist
        for i in 5..10 {
            assert!(buffer.get(i).is_some());
        }
    }

    #[test]
    fn test_buffer_iter() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let ids: Vec<u64> = buffer.iter().map(|r| r.round_id).collect();

        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_buffer_iter_after_overflow() {
        let mut buffer = SyndromeBuffer::new(5);

        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let ids: Vec<u64> = buffer.iter().map(|r| r.round_id).collect();

        assert_eq!(ids, vec![5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..50 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        buffer.clear();

        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_buffer_statistics() {
        let mut buffer = SyndromeBuffer::new(10);

        for i in 0..20 {
            let mut detectors = DetectorBitmap::new(64);
            for j in 0..(i % 5) as usize {
                detectors.set(j, true);
            }
            let round = SyndromeRound::new(i, i, i * 1_000, detectors, 0);
            buffer.push(round);
        }

        let stats = buffer.statistics();

        assert_eq!(stats.total_rounds, 20);
        assert_eq!(stats.current_size, 10);
        assert_eq!(stats.capacity, 10);
        assert_eq!(stats.evicted_rounds, 10);
        assert!(stats.avg_firing_rate >= 0.0);
    }
}

// ============================================================================
// SyndromeDelta Computation Tests
// ============================================================================

mod syndrome_delta_tests {
    use super::*;

    #[test]
    fn test_delta_compute_basic() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(0, true);
        d1.set(5, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(5, true);
        d2.set(10, true);

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.from_round, 1);
        assert_eq!(delta.to_round, 2);
        assert_eq!(delta.flip_count(), 2);
    }

    #[test]
    fn test_delta_quiet() {
        let mut detectors = DetectorBitmap::new(64);
        detectors.set(5, true);

        let round1 = SyndromeRound::new(1, 100, 1_000, detectors.clone(), 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, detectors, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert!(delta.is_quiet());
        assert_eq!(delta.flip_count(), 0);
    }

    #[test]
    fn test_delta_not_quiet() {
        let d1 = DetectorBitmap::new(64);
        let mut d2 = DetectorBitmap::new(64);
        d2.set(0, true);

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert!(!delta.is_quiet());
    }

    #[test]
    fn test_delta_activity_level() {
        let d1 = DetectorBitmap::new(100);
        let mut d2 = DetectorBitmap::new(100);

        for i in 0..10 {
            d2.set(i, true);
        }

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        // 10 out of 100 detectors flipped = 0.1
        assert!((delta.activity_level() - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_delta_activity_level_zero() {
        let d1 = DetectorBitmap::new(0);
        let d2 = DetectorBitmap::new(0);

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.activity_level(), 0.0);
    }

    #[test]
    fn test_delta_span() {
        let d1 = DetectorBitmap::new(64);
        let d2 = DetectorBitmap::new(64);

        let round1 = SyndromeRound::new(100, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(110, 110, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.span(), 10);
    }

    #[test]
    fn test_delta_iter_flipped() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(0, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(10, true);
        d2.set(20, true);

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);
        let flipped: Vec<usize> = delta.iter_flipped().collect();

        assert_eq!(flipped, vec![0, 10, 20]);
    }

    #[test]
    fn test_delta_new_constructor() {
        let flipped = DetectorBitmap::new(64);
        let delta = SyndromeDelta::new(1, 5, flipped);

        assert_eq!(delta.from_round, 1);
        assert_eq!(delta.to_round, 5);
        assert_eq!(delta.span(), 4);
    }

    #[test]
    fn test_delta_accuracy_all_bits_flip() {
        let mut d1 = DetectorBitmap::new(64);
        for i in 0..64 {
            d1.set(i, true);
        }

        let d2 = DetectorBitmap::new(64);

        let round1 = SyndromeRound::new(1, 100, 1_000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2_000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.flip_count(), 64);
        assert_eq!(delta.activity_level(), 1.0);
    }
}

// ============================================================================
// Buffer Overflow Handling Tests
// ============================================================================

mod buffer_overflow_tests {
    use super::*;

    #[test]
    fn test_buffer_graceful_overflow() {
        let mut buffer = SyndromeBuffer::new(100);

        // Push 1000 rounds
        for i in 0..1000 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        // Should still work
        assert_eq!(buffer.len(), 100);
        assert!(buffer.is_full());

        // Most recent 100 should be available
        for i in 900..1000 {
            assert!(buffer.get(i).is_some());
        }
    }

    #[test]
    fn test_buffer_statistics_after_overflow() {
        let mut buffer = SyndromeBuffer::new(10);

        for i in 0..100 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let stats = buffer.statistics();

        assert_eq!(stats.total_rounds, 100);
        assert_eq!(stats.evicted_rounds, 90);
        assert_eq!(stats.current_size, 10);
    }

    #[test]
    fn test_buffer_continuous_operation() {
        let mut buffer = SyndromeBuffer::new(50);

        // Simulate long-running operation
        for i in 0..10_000 {
            let mut detectors = DetectorBitmap::new(64);
            if i % 100 == 0 {
                detectors.set(0, true); // Occasional syndrome
            }
            let round = SyndromeRound::new(i, i, i * 1_000, detectors, 0);
            buffer.push(round);

            // Periodically access window
            if i % 1000 == 0 {
                let window = buffer.window(10);
                assert_eq!(window.len(), std::cmp::min(10, buffer.len()));
            }
        }

        // Buffer should still be functional
        assert_eq!(buffer.len(), 50);
    }

    #[test]
    fn test_buffer_window_wrap_around() {
        let mut buffer = SyndromeBuffer::new(10);

        // Push 15 rounds to wrap around
        for i in 0..15 {
            let round = SyndromeRound::new(i, i, i * 1_000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        // Window should correctly handle wrap-around
        let window = buffer.window(10);

        assert_eq!(window.len(), 10);
        assert_eq!(window[0].round_id, 5); // Oldest available
        assert_eq!(window[9].round_id, 14); // Most recent
    }
}

// ============================================================================
// Proptest Property-Based Tests
// ============================================================================

#[cfg(test)]
mod proptest_syndrome {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_bitmap_popcount_equals_set_count(
            detector_indices in prop::collection::vec(0usize..1024, 0..100)
        ) {
            let mut bitmap = DetectorBitmap::new(1024);
            let mut unique_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

            for idx in detector_indices {
                bitmap.set(idx, true);
                unique_indices.insert(idx);
            }

            prop_assert_eq!(bitmap.fired_count(), unique_indices.len());
        }

        #[test]
        fn prop_xor_commutative(
            indices_a in prop::collection::vec(0usize..64, 0..10),
            indices_b in prop::collection::vec(0usize..64, 0..10)
        ) {
            let mut a = DetectorBitmap::new(64);
            let mut b = DetectorBitmap::new(64);

            for idx in indices_a {
                a.set(idx, true);
            }
            for idx in indices_b {
                b.set(idx, true);
            }

            let ab = a.xor(&b);
            let ba = b.xor(&a);

            // XOR should be commutative
            prop_assert_eq!(ab.fired_count(), ba.fired_count());
            for i in 0..64 {
                prop_assert_eq!(ab.get(i), ba.get(i));
            }
        }

        #[test]
        fn prop_buffer_window_size_bounded(
            capacity in 10usize..100,
            push_count in 0usize..200,
            window_size in 1usize..50
        ) {
            let mut buffer = SyndromeBuffer::new(capacity);

            for i in 0..push_count as u64 {
                let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
                buffer.push(round);
            }

            let window = buffer.window(window_size);

            // Window size should be min(requested, available)
            let expected_size = window_size.min(push_count).min(capacity);
            prop_assert_eq!(window.len(), expected_size);
        }

        #[test]
        fn prop_delta_flip_count_bounded(
            set_a in prop::collection::vec(0usize..64, 0..64),
            set_b in prop::collection::vec(0usize..64, 0..64)
        ) {
            let mut d1 = DetectorBitmap::new(64);
            let mut d2 = DetectorBitmap::new(64);

            for idx in set_a {
                d1.set(idx, true);
            }
            for idx in set_b {
                d2.set(idx, true);
            }

            let round1 = SyndromeRound::new(0, 0, 0, d1, 0);
            let round2 = SyndromeRound::new(1, 1, 1, d2, 0);

            let delta = SyndromeDelta::compute(&round1, &round2);

            // Flip count should be bounded by detector count
            prop_assert!(delta.flip_count() <= 64);
        }
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn test_bitmap_single_detector() {
        let bitmap = DetectorBitmap::new(1);

        assert_eq!(bitmap.detector_count(), 1);
    }

    #[test]
    fn test_bitmap_boundary_word_crossing() {
        let mut bitmap = DetectorBitmap::new(128);

        // Set bits around word boundary (63, 64, 65)
        bitmap.set(63, true);
        bitmap.set(64, true);
        bitmap.set(65, true);

        assert!(bitmap.get(63));
        assert!(bitmap.get(64));
        assert!(bitmap.get(65));
        assert_eq!(bitmap.fired_count(), 3);
    }

    #[test]
    fn test_buffer_single_capacity() {
        let mut buffer = SyndromeBuffer::new(1);

        buffer.push(SyndromeRound::new(0, 0, 0, DetectorBitmap::new(64), 0));
        assert_eq!(buffer.len(), 1);

        buffer.push(SyndromeRound::new(1, 1, 1, DetectorBitmap::new(64), 0));
        assert_eq!(buffer.len(), 1); // Still 1, oldest evicted

        assert!(buffer.get(0).is_none());
        assert!(buffer.get(1).is_some());
    }

    #[test]
    fn test_delta_same_round() {
        let detectors = DetectorBitmap::new(64);
        let round = SyndromeRound::new(1, 100, 1_000, detectors, 0);

        let delta = SyndromeDelta::compute(&round, &round);

        assert!(delta.is_quiet());
        assert_eq!(delta.span(), 0);
    }
}
