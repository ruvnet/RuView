//! Identity Continuity Tracking tests
//!
//! This module validates identity continuity tracking, feature extraction,
//! similarity calculations, and continuity break detection.

use super::*;

/// Test basic identity snapshot functionality
#[cfg(test)]
mod identity_snapshot_tests {
    use super::*;

    #[test]
    fn test_identity_snapshot_creation() {
        let timestamp = TscTimestamp::now();
        let state = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let snapshot = IdentitySnapshot::new(timestamp, &state);

        assert_eq!(snapshot.timestamp, timestamp);
        assert!(!snapshot.feature_vector.is_empty());
        assert!(snapshot.coherence_score >= 0.0 && snapshot.coherence_score <= 1.0);
        assert_eq!(snapshot.stability_metric, 1.0); // Initial value
        assert_eq!(snapshot.memory_fingerprint, state);
    }

    #[test]
    fn test_feature_extraction_consistency() {
        let state = vec![1, 2, 3, 4, 5, 4, 3, 2, 1];

        // Extract features multiple times
        let features1 = IdentitySnapshot::extract_features(&state);
        let features2 = IdentitySnapshot::extract_features(&state);

        assert_eq!(features1.len(), features2.len());
        assert_eq!(features1, features2, "Feature extraction should be deterministic");

        // Features should be normalized
        for &feature in &features1 {
            assert!(feature >= -1.0 && feature <= 1.0,
                "Feature not normalized: {}", feature);
        }
    }

    #[test]
    fn test_feature_extraction_with_different_patterns() {
        let test_cases = [
            (vec![0; 20], "constant"),
            ((0..20).map(|i| i as u8).collect(), "sequential"),
            (vec![255, 0, 255, 0, 255, 0, 255, 0], "alternating"),
            ((0..20).map(|i| fastrand::u8(..)).collect(), "random"),
        ];

        for (state, description) in test_cases.iter() {
            let features = IdentitySnapshot::extract_features(state);

            println!("{} pattern features: {:?}", description, &features[..3]);

            assert_eq!(features.len(), 16, "Should extract 16 features");

            // All features should be bounded
            for &feature in &features {
                assert!(feature >= -1.0 && feature <= 1.0,
                    "Feature out of bounds in {} pattern: {}", description, feature);
            }
        }
    }

    #[test]
    fn test_feature_extraction_with_empty_state() {
        let empty_state = vec![];
        let features = IdentitySnapshot::extract_features(&empty_state);

        assert_eq!(features.len(), 16);
        assert_eq!(features, vec![0.0; 16], "Empty state should produce zero features");
    }

    #[test]
    fn test_hash_computation_consistency() {
        let state1 = vec![1, 2, 3, 4, 5];
        let state2 = vec![1, 2, 3, 4, 5]; // Identical
        let state3 = vec![1, 2, 3, 4, 6]; // Different

        let hash1 = IdentitySnapshot::compute_hash(&state1);
        let hash2 = IdentitySnapshot::compute_hash(&state2);
        let hash3 = IdentitySnapshot::compute_hash(&state3);

        assert_eq!(hash1, hash2, "Identical states should have same hash");
        assert_ne!(hash1, hash3, "Different states should have different hashes");
    }

    #[test]
    fn test_coherence_calculation() {
        // Test coherent features (low variance)
        let coherent_features = vec![0.1, 0.1, 0.1, 0.1, 0.1];
        let coherence1 = IdentitySnapshot::calculate_coherence(&coherent_features);

        // Test incoherent features (high variance)
        let incoherent_features = vec![-1.0, 1.0, -1.0, 1.0, -1.0];
        let coherence2 = IdentitySnapshot::calculate_coherence(&incoherent_features);

        println!("Coherent features coherence: {:.6}", coherence1);
        println!("Incoherent features coherence: {:.6}", coherence2);

        assert!(coherence1 > coherence2, "Coherent features should have higher coherence");
        assert!(coherence1 >= 0.0 && coherence1 <= 1.0);
        assert!(coherence2 >= 0.0 && coherence2 <= 1.0);
    }

    #[test]
    fn test_similarity_calculation() {
        let timestamp = TscTimestamp::now();

        let state1 = vec![1, 2, 3, 4, 5];
        let state2 = vec![1, 2, 3, 4, 5]; // Identical
        let state3 = vec![5, 4, 3, 2, 1]; // Reversed

        let snapshot1 = IdentitySnapshot::new(timestamp, &state1);
        let snapshot2 = IdentitySnapshot::new(timestamp, &state2);
        let snapshot3 = IdentitySnapshot::new(timestamp, &state3);

        let sim12 = snapshot1.calculate_similarity(&snapshot2);
        let sim13 = snapshot1.calculate_similarity(&snapshot3);

        println!("Similarity identical: {:.6}", sim12);
        println!("Similarity reversed: {:.6}", sim13);

        assert!(sim12 > sim13, "Identical states should be more similar");
        assert!(sim12 >= 0.0 && sim12 <= 1.0);
        assert!(sim13 >= 0.0 && sim13 <= 1.0);
    }

    #[test]
    fn test_similarity_edge_cases() {
        let timestamp = TscTimestamp::now();

        let state1 = vec![1, 2, 3];
        let state2 = vec![]; // Empty state

        let snapshot1 = IdentitySnapshot::new(timestamp, &state1);
        let snapshot2 = IdentitySnapshot::new(timestamp, &state2);

        // Different feature vector lengths should return 0 similarity
        let similarity = snapshot1.calculate_similarity(&snapshot2);
        assert_eq!(similarity, 0.0, "Different vector lengths should have 0 similarity");
    }
}

/// Test identity continuity tracker functionality
#[cfg(test)]
mod continuity_tracker_tests {
    use super::*;

    #[test]
    fn test_tracker_creation() {
        let tracker = IdentityContinuityTracker::new();

        assert!(tracker.snapshots.is_empty());
        assert!(tracker.identity_baseline.is_none());
        assert!(tracker.last_validation_time.is_none());
        assert_eq!(tracker.continuity_threshold, 0.7);
        assert_eq!(tracker.gap_tolerance_ns, 1_000_000);

        let metrics = tracker.get_metrics().unwrap();
        assert_eq!(metrics.continuity_score, 0.0);
        assert_eq!(metrics.identity_stability, 0.0);
        assert_eq!(metrics.continuity_breaks, 0);
    }

    #[test]
    fn test_continuity_tracking_basic() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let state = vec![1, 2, 3, 4, 5];

        tracker.track_continuity(timestamp, &state).unwrap();

        assert_eq!(tracker.snapshots.len(), 1);
        assert!(tracker.identity_baseline.is_some());
        assert!(tracker.last_validation_time.is_some());

        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_score >= 0.0);
    }

    #[test]
    fn test_continuity_tracking_sequence() {
        let mut tracker = IdentityContinuityTracker::new();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        let base_timestamp = TscTimestamp::now();
        let states = [
            vec![1, 2, 3, 4, 5],
            vec![1, 2, 3, 4, 6], // Slight change
            vec![1, 2, 3, 5, 6], // More change
            vec![1, 2, 4, 5, 6], // Further change
        ];

        for (i, state) in states.iter().enumerate() {
            let timestamp = base_timestamp.add_nanos(i as u64 * 100_000, tsc_freq); // 100μs apart
            tracker.track_continuity(timestamp, state).unwrap();
        }

        assert_eq!(tracker.snapshots.len(), 4);

        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_score > 0.0);
        assert!(metrics.identity_stability >= 0.0);

        println!("Sequence tracking metrics:");
        println!("  Continuity score: {:.6}", metrics.continuity_score);
        println!("  Identity stability: {:.6}", metrics.identity_stability);
        println!("  Continuity breaks: {}", metrics.continuity_breaks);
    }

    #[test]
    fn test_continuity_break_detection() {
        let mut tracker = IdentityContinuityTracker::new();
        tracker.set_continuity_threshold(0.9); // High threshold

        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Start with one state
        let state1 = vec![1, 2, 3, 4, 5];
        tracker.track_continuity(timestamp, &state1).unwrap();

        // Dramatically different state
        let state2 = vec![100, 200, 50, 75, 25];
        let timestamp2 = timestamp.add_nanos(100_000, tsc_freq);
        tracker.track_continuity(timestamp2, &state2).unwrap();

        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_breaks > 0, "Should detect continuity break");

        println!("Continuity break test:");
        println!("  Breaks detected: {}", metrics.continuity_breaks);
        println!("  Final continuity score: {:.6}", metrics.continuity_score);
    }

    #[test]
    fn test_temporal_gap_detection() {
        let mut tracker = IdentityContinuityTracker::new();
        tracker.gap_tolerance_ns = 50_000; // 50μs tolerance

        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();
        let state = vec![1, 2, 3, 4, 5];

        // First snapshot
        tracker.track_continuity(timestamp, &state).unwrap();

        // Large temporal gap (1ms > 50μs tolerance)
        let timestamp2 = timestamp.add_nanos(1_000_000, tsc_freq);
        tracker.track_continuity(timestamp2, &state).unwrap();

        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_breaks > 0, "Should detect temporal gap");
        assert!(metrics.max_gap_duration_ns >= 1_000_000);
    }

    #[test]
    fn test_identity_stability_calculation() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Track stable states
        let stable_state = vec![10, 20, 30, 40, 50];
        for i in 0..15 {
            let ts = timestamp.add_nanos(i * 10_000, tsc_freq);
            let mut state = stable_state.clone();
            // Add tiny variations
            state[0] += (i % 3) as u8;
            tracker.track_continuity(ts, &state).unwrap();
        }

        let stability = tracker.get_identity_stability();
        println!("Identity stability with stable states: {:.6}", stability);

        assert!(stability > 0.5, "Should show high stability with similar states");
        assert!(stability <= 1.0, "Stability should be bounded");
    }

    #[test]
    fn test_continuity_validation() {
        let mut tracker = IdentityContinuityTracker::new();
        tracker.set_continuity_threshold(0.8);

        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Track highly similar states
        let base_state = vec![50, 60, 70, 80, 90];
        for i in 0..10 {
            let ts = timestamp.add_nanos(i * 5_000, tsc_freq);
            let mut state = base_state.clone();
            state[0] += i as u8; // Small variations
            tracker.track_continuity(ts, &state).unwrap();
        }

        // Should pass validation
        let validation_result = tracker.validate_continuity();
        assert!(validation_result.is_ok(), "Should pass continuity validation");

        // Now add a dramatic break
        let breaking_state = vec![200, 210, 220, 230, 240];
        let ts_break = timestamp.add_nanos(100_000, tsc_freq);
        tracker.track_continuity(ts_break, &breaking_state).unwrap();

        // May now fail validation depending on overall score
        let validation_result2 = tracker.validate_continuity();
        if validation_result2.is_err() {
            match validation_result2.unwrap_err() {
                TemporalError::IdentityContinuityBreak { gap_ns } => {
                    println!("Detected continuity break: {}ns gap", gap_ns);
                },
                _ => panic!("Unexpected error type"),
            }
        }
    }

    #[test]
    fn test_continuity_score_calculation() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create sequence with known similarity pattern
        let states = [
            vec![10, 20, 30],      // Base
            vec![11, 21, 31],      // Very similar
            vec![12, 22, 32],      // Very similar
            vec![15, 25, 35],      // Moderately similar
            vec![20, 30, 40],      // Less similar
        ];

        for (i, state) in states.iter().enumerate() {
            let ts = timestamp.add_nanos(i as u64 * 10_000, tsc_freq);
            tracker.track_continuity(ts, state).unwrap();
        }

        let continuity_score = tracker.get_continuity_score();
        println!("Continuity score for graded similarity: {:.6}", continuity_score);

        assert!(continuity_score >= 0.0 && continuity_score <= 1.0);
        assert!(continuity_score > 0.0, "Should detect some continuity");
    }

    #[test]
    fn test_identity_drift_calculation() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Establish baseline
        let baseline = vec![100, 100, 100, 100, 100];
        tracker.track_continuity(timestamp, &baseline).unwrap();

        // Gradually drift away
        for i in 1..=10 {
            let ts = timestamp.add_nanos(i * 10_000, tsc_freq);
            let drifted_state = vec![
                100 + i as u8,
                100,
                100,
                100,
                100 - i as u8,
            ];
            tracker.track_continuity(ts, &drifted_state).unwrap();
        }

        let drift = tracker.calculate_identity_drift();
        println!("Identity drift after 10 steps: {:.6}", drift);

        assert!(drift > 0.0, "Should detect drift from baseline");
        assert!(drift <= 1.0, "Drift should be bounded");
    }

    #[test]
    fn test_identity_trajectory() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create trajectory with varying coherence
        for i in 0..20 {
            let ts = timestamp.add_nanos(i * 5_000, tsc_freq);
            let coherence_factor = (i as f64 * 0.3).sin().abs(); // Varying coherence

            let state: Vec<u8> = (0..10).map(|j| {
                (100.0 + coherence_factor * 50.0 * j as f64) as u8
            }).collect();

            tracker.track_continuity(ts, &state).unwrap();
        }

        let trajectory = tracker.get_identity_trajectory(10);
        println!("Identity trajectory (last 10): {:?}", trajectory);

        assert_eq!(trajectory.len(), 10);

        // All coherence scores should be bounded
        for &score in &trajectory {
            assert!(score >= 0.0 && score <= 1.0);
        }
    }

    #[test]
    fn test_tracker_reset() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();

        // Build some state
        for i in 0..5 {
            let state = vec![i as u8; 10];
            tracker.track_continuity(timestamp, &state).unwrap();
        }

        assert!(!tracker.snapshots.is_empty());
        assert!(tracker.identity_baseline.is_some());

        // Reset and verify clean state
        tracker.reset();

        assert!(tracker.snapshots.is_empty());
        assert!(tracker.identity_baseline.is_none());
        assert!(tracker.last_validation_time.is_none());

        let metrics = tracker.get_metrics().unwrap();
        assert_eq!(metrics.continuity_score, 0.0);
        assert_eq!(metrics.identity_stability, 0.0);
        assert_eq!(metrics.continuity_breaks, 0);
    }
}

/// Test advanced continuity analysis
#[cfg(test)]
mod advanced_continuity_tests {
    use super::*;

    #[test]
    fn test_temporal_consistency_analysis() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create smooth temporal progression
        for i in 0..20 {
            let ts = timestamp.add_nanos(i * 10_000, tsc_freq);

            // Smooth progression in feature space
            let progression_factor = i as f64 / 20.0;
            let state: Vec<u8> = (0..8).map(|j| {
                (100.0 + progression_factor * 50.0 + j as f64 * 5.0) as u8
            }).collect();

            tracker.track_continuity(ts, &state).unwrap();
        }

        let metrics = tracker.get_metrics().unwrap();
        println!("Temporal consistency: {:.6}", metrics.temporal_consistency);

        assert!(metrics.temporal_consistency >= -1.0 && metrics.temporal_consistency <= 1.0);
    }

    #[test]
    fn test_preservation_efficiency() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        let baseline_state = vec![50, 60, 70, 80, 90, 100, 110, 120];

        // Track baseline
        tracker.track_continuity(timestamp, &baseline_state).unwrap();

        // Add states that preserve identity well
        for i in 1..15 {
            let ts = timestamp.add_nanos(i * 20_000, tsc_freq);
            let preserved_state: Vec<u8> = baseline_state.iter()
                .map(|&x| x + (i % 3) as u8) // Small, systematic variations
                .collect();

            tracker.track_continuity(ts, &preserved_state).unwrap();
        }

        let metrics = tracker.get_metrics().unwrap();
        println!("Preservation efficiency: {:.6}", metrics.preservation_efficiency);

        assert!(metrics.preservation_efficiency >= 0.0);
        assert!(metrics.preservation_efficiency <= 1.0);
        assert!(metrics.preservation_efficiency > 0.3, "Should show good preservation");
    }

    #[test]
    fn test_coherence_tracking() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Track states with varying coherence patterns
        let coherence_patterns = [
            vec![10; 8],                    // Highly coherent
            vec![10, 11, 10, 11, 10, 11, 10, 11], // Pattern coherent
            (0..8).collect(),               // Sequential pattern
            vec![10, 50, 20, 60, 30, 70, 40, 80], // Less coherent
        ];

        for (i, pattern) in coherence_patterns.iter().enumerate() {
            let ts = timestamp.add_nanos(i as u64 * 50_000, tsc_freq);
            tracker.track_continuity(ts, pattern).unwrap();
        }

        let metrics = tracker.get_metrics().unwrap();
        println!("Identity coherence across patterns: {:.6}", metrics.identity_coherence);

        assert!(metrics.identity_coherence >= 0.0);
        assert!(metrics.identity_coherence <= 1.0);
    }

    #[test]
    fn test_gap_metrics_accuracy() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        let state = vec![1, 2, 3, 4, 5];
        let gaps = [10_000, 50_000, 100_000, 200_000, 500_000]; // Various gap sizes

        tracker.track_continuity(timestamp, &state).unwrap();

        let mut expected_max_gap = 0;
        let mut expected_total_gap = 0;

        for (i, &gap_ns) in gaps.iter().enumerate() {
            expected_max_gap = expected_max_gap.max(gap_ns);
            expected_total_gap += gap_ns;

            let ts = timestamp.add_nanos(
                gaps[..=i].iter().sum::<u64>() + (i as u64 + 1) * 1000,
                tsc_freq
            );
            tracker.track_continuity(ts, &state).unwrap();
        }

        let metrics = tracker.get_metrics().unwrap();
        let expected_avg_gap = expected_total_gap as f64 / gaps.len() as f64;

        println!("Gap metrics:");
        println!("  Max gap: {} ns (expected: {} ns)", metrics.max_gap_duration_ns, expected_max_gap);
        println!("  Avg gap: {:.1} ns (expected: {:.1} ns)", metrics.average_gap_duration_ns, expected_avg_gap);

        // Should be reasonably close (within 20% tolerance for timing precision)
        let tolerance = 0.2;
        assert!((metrics.average_gap_duration_ns - expected_avg_gap).abs() <= expected_avg_gap * tolerance,
            "Average gap calculation inaccurate");
    }

    #[test]
    fn test_feature_stability_over_time() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create state that should extract stable features
        let base_pattern = vec![100, 150, 200, 175, 125, 225, 250, 275];

        for i in 0..25 {
            let ts = timestamp.add_nanos(i * 8_000, tsc_freq);

            // Apply small, consistent transformations
            let transformed_state: Vec<u8> = base_pattern.iter()
                .map(|&x| ((x as f64 + (i as f64 * 0.5).sin() * 5.0) as u8))
                .collect();

            tracker.track_continuity(ts, &transformed_state).unwrap();
        }

        // Check that identity remains stable despite transformations
        let final_stability = tracker.get_identity_stability();
        let final_continuity = tracker.get_continuity_score();

        println!("Feature stability over transformations:");
        println!("  Identity stability: {:.6}", final_stability);
        println!("  Continuity score: {:.6}", final_continuity);

        assert!(final_stability > 0.6, "Should maintain stability with small transformations");
        assert!(final_continuity > 0.5, "Should maintain continuity with small transformations");
    }

    #[test]
    fn test_boundary_condition_handling() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();

        // Test with boundary conditions
        let boundary_states = [
            vec![],                 // Empty state
            vec![0],                // Single byte
            vec![255; 1000],        // Large uniform state
            vec![0, 255, 0, 255],   // Extreme alternating
        ];

        for (i, state) in boundary_states.iter().enumerate() {
            println!("Testing boundary condition {}: {} bytes", i, state.len());

            let result = tracker.track_continuity(timestamp, state);

            // Should handle all boundary conditions gracefully
            assert!(result.is_ok(), "Failed to handle boundary condition {}", i);
        }

        // Should still provide meaningful metrics
        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_score >= 0.0);
        assert!(metrics.identity_stability >= 0.0);
    }

    #[test]
    fn test_memory_bounded_tracking() {
        let mut tracker = IdentityContinuityTracker::new();
        tracker.max_snapshots = 50; // Limit to test memory management

        let timestamp = TscTimestamp::now();
        let tsc_freq = TestConfig::detect_tsc_frequency();

        // Create more snapshots than the limit
        for i in 0..100 {
            let ts = timestamp.add_nanos(i * 5_000, tsc_freq);
            let state = vec![i as u8; 10];
            tracker.track_continuity(ts, &state).unwrap();
        }

        // Should not exceed memory limit
        assert!(tracker.snapshots.len() <= tracker.max_snapshots);

        // Should still maintain meaningful metrics
        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_score >= 0.0);

        println!("Memory bounded tracking:");
        println!("  Snapshots stored: {}/{}", tracker.snapshots.len(), tracker.max_snapshots);
        println!("  Final continuity: {:.6}", metrics.continuity_score);
    }
}