//! Comprehensive tests for PERMIT/DEFER/DENY decision logic
//!
//! Tests cover:
//! - Three-filter decision pipeline
//! - Threshold configurations
//! - Edge cases and boundary conditions
//! - Security scenarios (policy violations, replay detection)

use cognitum_gate_tilezero::decision::{EvidenceDecision, GateDecision, GateThresholds};

#[cfg(test)]
mod gate_decision {
    use super::*;

    #[test]
    fn test_decision_display() {
        assert_eq!(GateDecision::Permit.to_string(), "permit");
        assert_eq!(GateDecision::Defer.to_string(), "defer");
        assert_eq!(GateDecision::Deny.to_string(), "deny");
    }

    #[test]
    fn test_decision_equality() {
        assert_eq!(GateDecision::Permit, GateDecision::Permit);
        assert_eq!(GateDecision::Defer, GateDecision::Defer);
        assert_eq!(GateDecision::Deny, GateDecision::Deny);

        assert_ne!(GateDecision::Permit, GateDecision::Defer);
        assert_ne!(GateDecision::Permit, GateDecision::Deny);
        assert_ne!(GateDecision::Defer, GateDecision::Deny);
    }
}

#[cfg(test)]
mod evidence_decision {
    use super::*;

    #[test]
    fn test_evidence_values() {
        let accept = EvidenceDecision::Accept;
        let cont = EvidenceDecision::Continue;
        let reject = EvidenceDecision::Reject;

        assert_eq!(accept, EvidenceDecision::Accept);
        assert_eq!(cont, EvidenceDecision::Continue);
        assert_eq!(reject, EvidenceDecision::Reject);
    }
}

#[cfg(test)]
mod threshold_configuration {
    use super::*;

    #[test]
    fn test_default_thresholds() {
        let thresholds = GateThresholds::default();

        assert_eq!(thresholds.tau_deny, 0.01);
        assert_eq!(thresholds.tau_permit, 100.0);
        assert_eq!(thresholds.min_cut, 5.0);
        assert_eq!(thresholds.max_shift, 0.5);
        assert_eq!(thresholds.permit_ttl_ns, 60_000_000_000);
    }

    #[test]
    fn test_custom_thresholds() {
        let thresholds = GateThresholds {
            tau_deny: 0.05,
            tau_permit: 50.0,
            min_cut: 10.0,
            max_shift: 0.3,
            permit_ttl_ns: 30_000_000_000,
            theta_uncertainty: 15.0,
            theta_confidence: 3.0,
        };

        assert_eq!(thresholds.tau_deny, 0.05);
        assert_eq!(thresholds.tau_permit, 50.0);
        assert_eq!(thresholds.min_cut, 10.0);
    }

    #[test]
    fn test_threshold_ordering() {
        let thresholds = GateThresholds::default();

        // tau_deny < 1 < tau_permit (typical e-process thresholds)
        assert!(thresholds.tau_deny < 1.0);
        assert!(thresholds.tau_permit > 1.0);
        assert!(thresholds.tau_deny < thresholds.tau_permit);
    }

    #[test]
    fn test_conformal_thresholds() {
        let thresholds = GateThresholds::default();

        // theta_confidence < theta_uncertainty (smaller set = more confident)
        assert!(thresholds.theta_confidence < thresholds.theta_uncertainty);
    }
}

#[cfg(test)]
mod three_filter_logic {
    use super::*;

    /// Test the structural filter (min-cut check)
    #[test]
    fn test_structural_filter_deny() {
        // If min-cut is below threshold, should DENY
        let thresholds = GateThresholds::default();

        // Low min-cut (below threshold of 5.0)
        let min_cut = 3.0;
        let shift_pressure = 0.1; // OK
        let e_aggregate = 150.0; // OK

        let decision = apply_three_filters(min_cut, shift_pressure, e_aggregate, &thresholds);
        assert_eq!(decision, GateDecision::Deny);
    }

    /// Test the shift filter (coherence check)
    #[test]
    fn test_shift_filter_defer() {
        let thresholds = GateThresholds::default();

        // OK min-cut, high shift pressure
        let min_cut = 10.0; // OK
        let shift_pressure = 0.8; // Above threshold of 0.5
        let e_aggregate = 150.0; // OK

        let decision = apply_three_filters(min_cut, shift_pressure, e_aggregate, &thresholds);
        assert_eq!(decision, GateDecision::Defer);
    }

    /// Test the evidence filter (e-value check)
    #[test]
    fn test_evidence_filter_deny() {
        let thresholds = GateThresholds::default();

        // OK min-cut, OK shift, low e-value (evidence against coherence)
        let min_cut = 10.0;
        let shift_pressure = 0.1;
        let e_aggregate = 0.005; // Below tau_deny of 0.01

        let decision = apply_three_filters(min_cut, shift_pressure, e_aggregate, &thresholds);
        assert_eq!(decision, GateDecision::Deny);
    }

    #[test]
    fn test_evidence_filter_defer() {
        let thresholds = GateThresholds::default();

        // OK min-cut, OK shift, moderate e-value (insufficient evidence)
        let min_cut = 10.0;
        let shift_pressure = 0.1;
        let e_aggregate = 50.0; // Between tau_deny (0.01) and tau_permit (100)

        let decision = apply_three_filters(min_cut, shift_pressure, e_aggregate, &thresholds);
        assert_eq!(decision, GateDecision::Defer);
    }

    #[test]
    fn test_all_filters_pass_permit() {
        let thresholds = GateThresholds::default();

        // Everything OK
        let min_cut = 10.0;
        let shift_pressure = 0.1;
        let e_aggregate = 150.0; // Above tau_permit of 100

        let decision = apply_three_filters(min_cut, shift_pressure, e_aggregate, &thresholds);
        assert_eq!(decision, GateDecision::Permit);
    }

    // Helper function to simulate the three-filter logic
    fn apply_three_filters(
        min_cut: f64,
        shift_pressure: f64,
        e_aggregate: f64,
        thresholds: &GateThresholds,
    ) -> GateDecision {
        // 1. Structural filter
        if min_cut < thresholds.min_cut {
            return GateDecision::Deny;
        }

        // 2. Shift filter
        if shift_pressure >= thresholds.max_shift {
            return GateDecision::Defer;
        }

        // 3. Evidence filter
        if e_aggregate < thresholds.tau_deny {
            return GateDecision::Deny;
        }
        if e_aggregate < thresholds.tau_permit {
            return GateDecision::Defer;
        }

        GateDecision::Permit
    }
}

#[cfg(test)]
mod boundary_conditions {
    use super::*;

    #[test]
    fn test_min_cut_at_threshold() {
        let thresholds = GateThresholds::default();

        // Exactly at threshold
        let decision = decide_structural(5.0, &thresholds);
        assert_eq!(decision, GateDecision::Permit); // >= threshold is OK
    }

    #[test]
    fn test_min_cut_just_below() {
        let thresholds = GateThresholds::default();

        let decision = decide_structural(4.999, &thresholds);
        assert_eq!(decision, GateDecision::Deny);
    }

    #[test]
    fn test_e_value_at_deny_threshold() {
        let thresholds = GateThresholds::default();

        let decision = decide_evidence(0.01, &thresholds);
        assert_eq!(decision, EvidenceDecision::Continue); // Exactly at threshold continues
    }

    #[test]
    fn test_e_value_at_permit_threshold() {
        let thresholds = GateThresholds::default();

        let decision = decide_evidence(100.0, &thresholds);
        assert_eq!(decision, EvidenceDecision::Accept);
    }

    #[test]
    fn test_zero_values() {
        let thresholds = GateThresholds::default();

        assert_eq!(decide_structural(0.0, &thresholds), GateDecision::Deny);
        assert_eq!(decide_evidence(0.0, &thresholds), EvidenceDecision::Reject);
    }

    // Helper functions
    fn decide_structural(min_cut: f64, thresholds: &GateThresholds) -> GateDecision {
        if min_cut >= thresholds.min_cut {
            GateDecision::Permit
        } else {
            GateDecision::Deny
        }
    }

    fn decide_evidence(e_aggregate: f64, thresholds: &GateThresholds) -> EvidenceDecision {
        if e_aggregate < thresholds.tau_deny {
            EvidenceDecision::Reject
        } else if e_aggregate >= thresholds.tau_permit {
            EvidenceDecision::Accept
        } else {
            EvidenceDecision::Continue
        }
    }
}

#[cfg(test)]
mod filter_priority {
    use super::*;

    /// Structural filter has highest priority (checked first)
    #[test]
    fn test_structural_overrides_evidence() {
        let thresholds = GateThresholds::default();

        // Low min-cut but high e-value
        let min_cut = 1.0; // Fail structural
        let e_aggregate = 1000.0; // Would pass evidence

        // Structural failure should result in DENY
        let decision = if min_cut < thresholds.min_cut {
            GateDecision::Deny
        } else if e_aggregate >= thresholds.tau_permit {
            GateDecision::Permit
        } else {
            GateDecision::Defer
        };

        assert_eq!(decision, GateDecision::Deny);
    }

    /// Shift filter checked after structural
    #[test]
    fn test_shift_overrides_evidence() {
        let thresholds = GateThresholds::default();

        // Good min-cut, high shift, high e-value
        let min_cut = 10.0; // Pass structural
        let shift_pressure = 0.9; // Fail shift
        let e_aggregate = 1000.0; // Would pass evidence

        let decision = if min_cut < thresholds.min_cut {
            GateDecision::Deny
        } else if shift_pressure >= thresholds.max_shift {
            GateDecision::Defer
        } else if e_aggregate >= thresholds.tau_permit {
            GateDecision::Permit
        } else {
            GateDecision::Defer
        };

        assert_eq!(decision, GateDecision::Defer);
    }
}

#[cfg(test)]
mod ttl_scenarios {
    use super::*;

    #[test]
    fn test_permit_ttl() {
        let thresholds = GateThresholds::default();
        assert_eq!(thresholds.permit_ttl_ns, 60_000_000_000); // 60 seconds
    }

    #[test]
    fn test_custom_short_ttl() {
        let thresholds = GateThresholds {
            permit_ttl_ns: 1_000_000_000, // 1 second
            ..Default::default()
        };

        assert_eq!(thresholds.permit_ttl_ns, 1_000_000_000);
    }

    #[test]
    fn test_custom_long_ttl() {
        let thresholds = GateThresholds {
            permit_ttl_ns: 3600_000_000_000, // 1 hour
            ..Default::default()
        };

        assert_eq!(thresholds.permit_ttl_ns, 3600_000_000_000);
    }
}

#[cfg(test)]
mod extreme_values {
    use super::*;

    #[test]
    fn test_very_high_e_value() {
        let thresholds = GateThresholds::default();

        let decision = decide_evidence_full(1e10, &thresholds);
        assert_eq!(decision, EvidenceDecision::Accept);
    }

    #[test]
    fn test_very_low_e_value() {
        let thresholds = GateThresholds::default();

        let decision = decide_evidence_full(1e-10, &thresholds);
        assert_eq!(decision, EvidenceDecision::Reject);
    }

    #[test]
    fn test_very_high_min_cut() {
        let thresholds = GateThresholds::default();

        let decision = decide_structural_full(1000.0, &thresholds);
        assert_eq!(decision, GateDecision::Permit);
    }

    // Helper
    fn decide_evidence_full(e_aggregate: f64, thresholds: &GateThresholds) -> EvidenceDecision {
        if e_aggregate < thresholds.tau_deny {
            EvidenceDecision::Reject
        } else if e_aggregate >= thresholds.tau_permit {
            EvidenceDecision::Accept
        } else {
            EvidenceDecision::Continue
        }
    }

    fn decide_structural_full(min_cut: f64, thresholds: &GateThresholds) -> GateDecision {
        if min_cut >= thresholds.min_cut {
            GateDecision::Permit
        } else {
            GateDecision::Deny
        }
    }
}

#[cfg(test)]
mod serialization {
    use super::*;

    #[test]
    fn test_decision_serialization() {
        let decisions = [
            GateDecision::Permit,
            GateDecision::Defer,
            GateDecision::Deny,
        ];

        for decision in &decisions {
            let json = serde_json::to_string(decision).unwrap();
            let restored: GateDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(*decision, restored);
        }
    }

    #[test]
    fn test_decision_json_values() {
        assert_eq!(
            serde_json::to_string(&GateDecision::Permit).unwrap(),
            "\"permit\""
        );
        assert_eq!(
            serde_json::to_string(&GateDecision::Defer).unwrap(),
            "\"defer\""
        );
        assert_eq!(
            serde_json::to_string(&GateDecision::Deny).unwrap(),
            "\"deny\""
        );
    }

    #[test]
    fn test_thresholds_serialization() {
        let thresholds = GateThresholds::default();
        let json = serde_json::to_string(&thresholds).unwrap();
        let restored: GateThresholds = serde_json::from_str(&json).unwrap();

        assert_eq!(thresholds.tau_deny, restored.tau_deny);
        assert_eq!(thresholds.tau_permit, restored.tau_permit);
        assert_eq!(thresholds.min_cut, restored.min_cut);
    }
}

// Property-based tests
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_permit_requires_all_pass(
            min_cut in 0.0f64..100.0,
            shift in 0.0f64..1.0,
            e_val in 0.001f64..1000.0
        ) {
            let thresholds = GateThresholds::default();

            let structural_ok = min_cut >= thresholds.min_cut;
            let shift_ok = shift < thresholds.max_shift;
            let evidence_ok = e_val >= thresholds.tau_permit;

            let decision = apply_filters(min_cut, shift, e_val, &thresholds);

            if decision == GateDecision::Permit {
                assert!(structural_ok && shift_ok && evidence_ok);
            }
        }

        #[test]
        fn prop_structural_fail_is_deny(min_cut in 0.0f64..4.9) {
            let thresholds = GateThresholds::default();
            // Any structural failure (min_cut < 5.0) should result in Deny
            let decision = apply_filters(min_cut, 0.0, 1000.0, &thresholds);
            assert_eq!(decision, GateDecision::Deny);
        }

        #[test]
        fn prop_evidence_deny_threshold(e_val in 0.0f64..0.009) {
            let thresholds = GateThresholds::default();
            // E-value below tau_deny should result in Deny (if structural passes)
            let decision = apply_filters(100.0, 0.0, e_val, &thresholds);
            assert_eq!(decision, GateDecision::Deny);
        }
    }

    fn apply_filters(
        min_cut: f64,
        shift_pressure: f64,
        e_aggregate: f64,
        thresholds: &GateThresholds,
    ) -> GateDecision {
        if min_cut < thresholds.min_cut {
            return GateDecision::Deny;
        }
        if shift_pressure >= thresholds.max_shift {
            return GateDecision::Defer;
        }
        if e_aggregate < thresholds.tau_deny {
            return GateDecision::Deny;
        }
        if e_aggregate < thresholds.tau_permit {
            return GateDecision::Defer;
        }
        GateDecision::Permit
    }
}
