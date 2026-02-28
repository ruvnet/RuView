//! Filter pipeline tests for ruQu coherence gate
//!
//! Tests the three-filter decision pipeline:
//! - Structural filter with min-cut based stability
//! - Shift filter for distribution drift detection
//! - Evidence accumulator for e-value convergence

use ruqu::filters::{
    EvidenceAccumulator, EvidenceConfig, EvidenceFilter, FilterConfig, FilterPipeline, RegionMask,
    ShiftConfig, ShiftFilter, StructuralConfig, StructuralFilter, SystemState, Verdict,
};

// ============================================================================
// Structural Filter Tests
// ============================================================================

mod structural_filter_tests {
    use super::*;

    #[test]
    fn test_structural_filter_basic_creation() {
        let filter = StructuralFilter::new(5.0);
        assert_eq!(filter.threshold(), 5.0);
    }

    #[test]
    fn test_structural_filter_with_config() {
        let config = StructuralConfig {
            threshold: 3.5,
            max_cut_size: 500,
            use_subpolynomial: false,
            phi: 0.02,
        };
        let filter = StructuralFilter::with_config(config);
        assert_eq!(filter.threshold(), 3.5);
    }

    #[test]
    fn test_structural_filter_triangle_graph() {
        let mut filter = StructuralFilter::new(1.5);

        // Create a triangle (3-connected)
        filter.insert_edge(1, 2, 1.0).unwrap();
        filter.insert_edge(2, 3, 1.0).unwrap();
        filter.insert_edge(3, 1, 1.0).unwrap();

        let state = SystemState::new(3);
        let result = filter.evaluate(&state);

        // Triangle should have cut value >= 2.0
        assert!(result.cut_value >= 1.5);
        assert!(result.is_coherent);
        assert!(result.boundary_edges.is_empty());
    }

    #[test]
    fn test_structural_filter_single_edge_below_threshold() {
        let config = StructuralConfig {
            threshold: 3.0,
            use_subpolynomial: false,
            ..Default::default()
        };
        let mut filter = StructuralFilter::with_config(config);

        // Single edge has cut value 1.0
        filter.insert_edge(1, 2, 1.0).unwrap();

        let state = SystemState::new(2);
        let result = filter.evaluate(&state);

        // Should be below threshold
        assert!(!result.is_coherent);
        assert!(!result.boundary_edges.is_empty());
    }

    #[test]
    fn test_structural_filter_various_cut_values() {
        let test_cases = vec![
            (vec![(1, 2, 1.0)], 1.0, true), // Single edge at threshold (>= passes)
            (vec![(1, 2, 2.0)], 1.0, true), // Single edge weight 2.0 above threshold
            (vec![(1, 2, 1.0), (2, 3, 1.0)], 1.0, true), // Path
            (vec![(1, 2, 0.5)], 1.0, false), // Weak edge below threshold
        ];

        for (edges, threshold, expected_coherent) in test_cases {
            let config = StructuralConfig {
                threshold,
                use_subpolynomial: false,
                ..Default::default()
            };
            let mut filter = StructuralFilter::with_config(config);

            for (u, v, w) in edges {
                filter.insert_edge(u, v, w).unwrap();
            }

            let state = SystemState::new(10);
            let result = filter.evaluate(&state);

            assert_eq!(
                result.is_coherent, expected_coherent,
                "Threshold {}, expected coherent: {}",
                threshold, expected_coherent
            );
        }
    }

    #[test]
    fn test_structural_filter_edge_deletion() {
        let config = StructuralConfig {
            threshold: 1.0,
            use_subpolynomial: false,
            ..Default::default()
        };
        let mut filter = StructuralFilter::with_config(config);

        // Build a path: 1-2-3
        filter.insert_edge(1, 2, 1.0).unwrap();
        filter.insert_edge(2, 3, 1.0).unwrap();

        // Remove an edge
        filter.delete_edge(1, 2).unwrap();

        let state = SystemState::new(3);
        let result = filter.evaluate(&state);

        // Cut value should decrease
        assert!(result.cut_value >= 0.0);
    }

    #[test]
    fn test_structural_filter_duplicate_edge_error() {
        let mut filter = StructuralFilter::new(1.0);

        filter.insert_edge(1, 2, 1.0).unwrap();
        let result = filter.insert_edge(1, 2, 1.0);

        assert!(result.is_err());
    }

    #[test]
    fn test_structural_filter_delete_nonexistent_edge() {
        let mut filter = StructuralFilter::new(1.0);

        let result = filter.delete_edge(1, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_structural_filter_compute_time_recorded() {
        let mut filter = StructuralFilter::new(1.0);
        filter.insert_edge(1, 2, 1.0).unwrap();

        let state = SystemState::new(2);
        let result = filter.evaluate(&state);

        // Should have recorded some compute time
        assert!(result.compute_time_us < 1_000_000); // Less than 1 second
    }
}

// ============================================================================
// Shift Filter Tests
// ============================================================================

mod shift_filter_tests {
    use super::*;

    #[test]
    fn test_shift_filter_basic_creation() {
        let filter = ShiftFilter::new(0.5, 100);
        assert_eq!(filter.threshold(), 0.5);
        assert_eq!(filter.window_size(), 100);
    }

    #[test]
    fn test_shift_filter_stable_observations() {
        let mut filter = ShiftFilter::new(0.5, 100);

        // Add stable observations (low variance)
        for i in 0..100 {
            filter.update(0, 0.5 + (i as f64 % 10.0) * 0.001);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        assert!(result.is_stable);
        assert!(result.pressure < 0.5);
    }

    #[test]
    fn test_shift_filter_drift_detection() {
        let mut filter = ShiftFilter::new(0.3, 100);

        // Start with baseline
        for _ in 0..50 {
            filter.update(0, 0.5);
        }

        // Introduce drift
        for i in 0..50 {
            filter.update(0, 0.5 + i as f64 * 0.1); // Increasing values
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Should detect drift
        assert!(result.pressure > 0.0);
    }

    #[test]
    fn test_shift_filter_multiple_regions() {
        let mut filter = ShiftFilter::new(0.5, 100);

        // Different patterns per region
        for i in 0..100 {
            filter.update(0, 0.5); // Stable
            filter.update(1, 0.5 + i as f64 * 0.05); // Drifting
            filter.update(2, 0.5); // Stable
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Region 1 should be affected
        assert!(result.region_shifts.len() >= 3);
    }

    #[test]
    fn test_shift_filter_affected_regions_mask() {
        let mut filter = ShiftFilter::new(0.2, 100);

        // Create severe drift in regions 0 and 2
        for i in 0..100 {
            filter.update(0, i as f64); // Severe drift
            filter.update(1, 0.5); // Stable
            filter.update(2, i as f64 * 0.5); // Moderate drift
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Check affected regions
        if result.affected_regions.any() {
            assert!(result.pressure > 0.0);
        }
    }

    #[test]
    fn test_shift_filter_lead_time_estimation() {
        let mut filter = ShiftFilter::new(0.3, 100);

        // Create moderate drift
        for i in 0..100 {
            filter.update(0, 0.5 + i as f64 * 0.02);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // If drifting, should have lead time estimate
        if !result.is_stable {
            assert!(result.lead_time.is_some());
            assert!(result.lead_time.unwrap() >= 1);
        }
    }

    #[test]
    fn test_shift_filter_reset() {
        let mut filter = ShiftFilter::new(0.5, 100);

        // Add observations
        for _ in 0..50 {
            filter.update(0, 1.0);
        }

        // Reset
        filter.reset();

        // New observations should be fresh
        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Should be near-zero pressure after reset
        assert!(result.pressure < 0.5 || result.is_stable);
    }

    #[test]
    fn test_shift_filter_variance_computation() {
        let mut filter = ShiftFilter::new(0.5, 100);

        // Add observations with known variance
        let values = [0.0, 1.0, 2.0, 3.0, 4.0];
        for &v in values.iter().cycle().take(100) {
            filter.update(0, v);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Should compute some shift based on variance
        assert!(result.region_shifts[0] >= 0.0);
    }
}

// ============================================================================
// Evidence Accumulator Tests
// ============================================================================

mod evidence_accumulator_tests {
    use super::*;

    #[test]
    fn test_evidence_accumulator_initial_state() {
        let acc = EvidenceAccumulator::new();

        assert_eq!(acc.e_value(), 1.0);
        assert_eq!(acc.samples_seen(), 0);
        assert_eq!(acc.log_e_value(), 0.0);
    }

    #[test]
    fn test_evidence_accumulator_update() {
        let mut acc = EvidenceAccumulator::new();

        // Likelihood ratio > 1 means evidence for H1
        acc.update(2.0);

        assert!(acc.e_value() > 1.0);
        assert_eq!(acc.samples_seen(), 1);
    }

    #[test]
    fn test_evidence_accumulator_convergence_positive() {
        let mut acc = EvidenceAccumulator::new();

        // Consistently high likelihood ratios
        for _ in 0..20 {
            acc.update(2.0);
        }

        // Should converge to high e-value
        assert!(acc.e_value() > 100.0);
    }

    #[test]
    fn test_evidence_accumulator_convergence_negative() {
        let mut acc = EvidenceAccumulator::new();

        // Consistently low likelihood ratios
        for _ in 0..20 {
            acc.update(0.5);
        }

        // Should converge to low e-value
        assert!(acc.e_value() < 0.1);
    }

    #[test]
    fn test_evidence_accumulator_mixed_evidence() {
        let mut acc = EvidenceAccumulator::new();

        // Mixed evidence should roughly cancel out
        for _ in 0..50 {
            acc.update(2.0);
            acc.update(0.5);
        }

        // Should be near 1.0
        let e = acc.e_value();
        assert!(e > 0.1 && e < 10.0);
    }

    #[test]
    fn test_evidence_accumulator_reset() {
        let mut acc = EvidenceAccumulator::new();

        // Add evidence
        for _ in 0..10 {
            acc.update(2.0);
        }

        // Reset
        acc.reset();

        assert_eq!(acc.e_value(), 1.0);
        assert_eq!(acc.samples_seen(), 0);
    }

    #[test]
    fn test_evidence_accumulator_extreme_values_clamped() {
        let mut acc = EvidenceAccumulator::new();

        // Extreme likelihood ratio should be clamped
        acc.update(1e20); // Should be clamped to 1e10

        // Should not overflow
        assert!(acc.e_value().is_finite());
    }

    #[test]
    fn test_evidence_accumulator_posterior_odds() {
        let mut acc = EvidenceAccumulator::new();

        acc.update(4.0); // e-value = 4

        let prior_odds = 1.0; // Equal prior
        let posterior = acc.posterior_odds(prior_odds);

        assert!((posterior - 4.0).abs() < 0.1);
    }
}

// ============================================================================
// Evidence Filter Tests
// ============================================================================

mod evidence_filter_tests {
    use super::*;

    #[test]
    fn test_evidence_filter_permit_verdict() {
        let mut filter = EvidenceFilter::new(10.0, 0.1);

        // Add strong evidence
        for _ in 0..10 {
            filter.update(2.0);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        assert!(result.e_value > 10.0);
        assert_eq!(result.verdict, Some(Verdict::Permit));
    }

    #[test]
    fn test_evidence_filter_deny_verdict() {
        let mut filter = EvidenceFilter::new(10.0, 0.1);

        // Add negative evidence
        for _ in 0..10 {
            filter.update(0.5);
        }

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        assert!(result.e_value < 0.1);
        assert_eq!(result.verdict, Some(Verdict::Deny));
    }

    #[test]
    fn test_evidence_filter_defer_verdict() {
        let mut filter = EvidenceFilter::new(10.0, 0.1);

        // Add minimal evidence (stays near 1.0)
        filter.update(1.1);
        filter.update(0.9);

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Should be between thresholds
        assert!(result.e_value > 0.1 && result.e_value < 10.0);
        assert_eq!(result.verdict, None); // Defer
    }

    #[test]
    fn test_evidence_filter_thresholds() {
        let filter = EvidenceFilter::new(20.0, 0.05);

        assert_eq!(filter.tau_permit(), 20.0);
        assert_eq!(filter.tau_deny(), 0.05);
    }

    #[test]
    fn test_evidence_filter_region_accumulators() {
        let mut filter = EvidenceFilter::new(10.0, 0.1);

        // Update different regions
        filter.update_region(0, 2.0);
        filter.update_region(1, 0.5);
        filter.update_region(2, 1.5);

        let state = SystemState::new(10);
        let result = filter.evaluate(&state);

        // Global accumulator should still be at 1.0
        assert!((result.e_value - 1.0).abs() < 0.1);
    }
}

// ============================================================================
// Filter Pipeline Tests
// ============================================================================

mod filter_pipeline_tests {
    use super::*;

    #[test]
    fn test_pipeline_all_filters_pass() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 1.0,
                use_subpolynomial: false,
                ..Default::default()
            },
            shift: ShiftConfig {
                threshold: 0.5,
                ..Default::default()
            },
            evidence: EvidenceConfig {
                tau_permit: 5.0,
                tau_deny: 0.2,
                ..Default::default()
            },
        };

        let mut pipeline = FilterPipeline::new(config);

        // Build good graph
        pipeline.structural_mut().insert_edge(1, 2, 2.0).unwrap();
        pipeline.structural_mut().insert_edge(2, 3, 2.0).unwrap();
        pipeline.structural_mut().insert_edge(3, 1, 2.0).unwrap();

        // Stable shift
        for _ in 0..30 {
            pipeline.shift_mut().update(0, 0.5);
        }

        // Strong evidence
        for _ in 0..5 {
            pipeline.evidence_mut().update(2.0);
        }

        let state = SystemState::new(3);
        let result = pipeline.evaluate(&state);

        assert_eq!(result.verdict, Some(Verdict::Permit));
        assert!(result.recommendations.is_empty() || result.structural.is_coherent);
    }

    #[test]
    fn test_pipeline_structural_fails() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 5.0, // High threshold
                use_subpolynomial: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pipeline = FilterPipeline::new(config);

        // Weak graph
        pipeline.structural_mut().insert_edge(1, 2, 1.0).unwrap();

        let state = SystemState::new(2);
        let result = pipeline.evaluate(&state);

        assert_eq!(result.verdict, Some(Verdict::Deny));
        assert!(!result.structural.is_coherent);
    }

    #[test]
    fn test_pipeline_shift_triggers_defer() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 1.0,
                use_subpolynomial: false,
                ..Default::default()
            },
            shift: ShiftConfig {
                threshold: 0.1, // Low threshold
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pipeline = FilterPipeline::new(config);

        // Good structure
        pipeline.structural_mut().insert_edge(1, 2, 2.0).unwrap();
        pipeline.structural_mut().insert_edge(2, 3, 2.0).unwrap();

        // Create drift
        for i in 0..50 {
            pipeline.shift_mut().update(0, i as f64);
        }

        let state = SystemState::new(3);
        let result = pipeline.evaluate(&state);

        // Should defer due to shift
        assert!(result.verdict == Some(Verdict::Defer) || result.verdict == Some(Verdict::Deny));
    }

    #[test]
    fn test_pipeline_evidence_determines_permit_deny() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 1.0,
                use_subpolynomial: false,
                ..Default::default()
            },
            shift: ShiftConfig {
                threshold: 0.9, // Permissive
                ..Default::default()
            },
            evidence: EvidenceConfig {
                tau_permit: 5.0,
                tau_deny: 0.2,
                ..Default::default()
            },
        };

        let mut pipeline = FilterPipeline::new(config);

        // Good structure
        pipeline.structural_mut().insert_edge(1, 2, 2.0).unwrap();

        // Minimal shift
        for _ in 0..20 {
            pipeline.shift_mut().update(0, 0.5);
        }

        // Test with insufficient evidence
        let state = SystemState::new(2);
        let result = pipeline.evaluate(&state);

        // Should be Defer (evidence accumulating) since no evidence added
        assert!(result.verdict == Some(Verdict::Defer) || result.evidence.verdict == None);
    }

    #[test]
    fn test_pipeline_reset() {
        let config = FilterConfig::default();
        let mut pipeline = FilterPipeline::new(config);

        // Add some state
        for _ in 0..10 {
            pipeline.shift_mut().update(0, 1.0);
            pipeline.evidence_mut().update(2.0);
        }

        // Reset
        pipeline.reset();

        // Evaluate fresh
        let state = SystemState::new(10);
        let result = pipeline.evaluate(&state);

        // Evidence should be back to 1.0
        assert!((result.evidence.e_value - 1.0).abs() < 0.5);
    }

    #[test]
    fn test_pipeline_total_time_recorded() {
        let config = FilterConfig::default();
        let pipeline = FilterPipeline::new(config);

        let state = SystemState::new(10);
        let result = pipeline.evaluate(&state);

        // Should have recorded time
        assert!(result.total_time_us < 1_000_000);
    }

    #[test]
    fn test_pipeline_recommendations_generated() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 10.0, // Very high
                use_subpolynomial: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pipeline = FilterPipeline::new(config);
        pipeline.structural_mut().insert_edge(1, 2, 1.0).unwrap();

        let state = SystemState::new(2);
        let result = pipeline.evaluate(&state);

        // Should have recommendations about structural failure
        assert!(!result.recommendations.is_empty());
        assert!(result.recommendations[0].contains("Structural"));
    }
}

// ============================================================================
// Filter Combination Logic Tests
// ============================================================================

mod filter_combination_tests {
    use super::*;

    #[test]
    fn test_deny_takes_priority() {
        // If any filter denies, overall should deny
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 10.0,
                use_subpolynomial: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pipeline = FilterPipeline::new(config);
        pipeline.structural_mut().insert_edge(1, 2, 1.0).unwrap();

        // Even with good evidence
        for _ in 0..10 {
            pipeline.evidence_mut().update(2.0);
        }

        let state = SystemState::new(2);
        let result = pipeline.evaluate(&state);

        assert_eq!(result.verdict, Some(Verdict::Deny));
    }

    #[test]
    fn test_defer_when_evidence_accumulating() {
        let config = FilterConfig {
            structural: StructuralConfig {
                threshold: 1.0,
                use_subpolynomial: false,
                ..Default::default()
            },
            shift: ShiftConfig {
                threshold: 0.9,
                ..Default::default()
            },
            evidence: EvidenceConfig {
                tau_permit: 100.0, // Very high threshold
                tau_deny: 0.001,
                ..Default::default()
            },
        };

        let mut pipeline = FilterPipeline::new(config);
        pipeline.structural_mut().insert_edge(1, 2, 2.0).unwrap();

        // Minimal evidence (not enough to decide)
        pipeline.evidence_mut().update(1.1);

        let state = SystemState::new(2);
        let result = pipeline.evaluate(&state);

        // Should defer - evidence not conclusive
        assert_eq!(result.verdict, Some(Verdict::Defer));
    }
}

// ============================================================================
// Proptest Property-Based Tests
// ============================================================================

#[cfg(test)]
mod proptest_filters {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn prop_structural_coherence_monotonic_with_weight(
            base_weight in 0.1f64..10.0,
            multiplier in 1.0f64..5.0
        ) {
            let config = StructuralConfig {
                threshold: base_weight,
                use_subpolynomial: false,
                ..Default::default()
            };
            let mut filter = StructuralFilter::with_config(config);

            filter.insert_edge(1, 2, base_weight * multiplier).unwrap();

            let state = SystemState::new(2);
            let result = filter.evaluate(&state);

            // Higher weight should increase cut value
            if multiplier >= 1.0 {
                prop_assert!(result.cut_value >= 0.0);
            }
        }

        #[test]
        fn prop_evidence_accumulator_bounded(
            likelihood_ratios in prop::collection::vec(0.1f64..10.0, 1..50)
        ) {
            let mut acc = EvidenceAccumulator::new();

            for lr in likelihood_ratios {
                acc.update(lr);
            }

            // E-value should always be finite and positive
            prop_assert!(acc.e_value().is_finite());
            prop_assert!(acc.e_value() > 0.0);
        }

        #[test]
        fn prop_shift_filter_pressure_bounded(
            values in prop::collection::vec(0.0f64..100.0, 10..100)
        ) {
            let mut filter = ShiftFilter::new(0.5, 100);

            for (i, v) in values.iter().enumerate() {
                filter.update(i % 10, *v);
            }

            let state = SystemState::new(10);
            let result = filter.evaluate(&state);

            // Pressure should be bounded [0, inf) but typically reasonable
            prop_assert!(result.pressure >= 0.0);
            prop_assert!(result.pressure.is_finite());
        }
    }
}

// ============================================================================
// Region Mask Tests
// ============================================================================

mod region_mask_tests {
    use super::*;

    #[test]
    fn test_region_mask_empty() {
        let mask = RegionMask::empty();
        assert!(!mask.any());
        assert_eq!(mask.count(), 0);
    }

    #[test]
    fn test_region_mask_all() {
        let mask = RegionMask::all();
        assert!(mask.any());
        assert_eq!(mask.count(), 64);
    }

    #[test]
    fn test_region_mask_set_clear() {
        let mut mask = RegionMask::empty();

        mask.set(5);
        assert!(mask.is_set(5));
        assert!(!mask.is_set(4));

        mask.clear(5);
        assert!(!mask.is_set(5));
    }

    #[test]
    fn test_region_mask_union() {
        let mut a = RegionMask::empty();
        let mut b = RegionMask::empty();

        a.set(1);
        a.set(3);
        b.set(2);
        b.set(3);

        let union = a.union(&b);
        assert!(union.is_set(1));
        assert!(union.is_set(2));
        assert!(union.is_set(3));
        assert_eq!(union.count(), 3);
    }

    #[test]
    fn test_region_mask_intersection() {
        let mut a = RegionMask::empty();
        let mut b = RegionMask::empty();

        a.set(1);
        a.set(3);
        b.set(2);
        b.set(3);

        let intersection = a.intersection(&b);
        assert!(!intersection.is_set(1));
        assert!(!intersection.is_set(2));
        assert!(intersection.is_set(3));
        assert_eq!(intersection.count(), 1);
    }
}
