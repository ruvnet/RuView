//! Comprehensive tests for deterministic replay
//!
//! Tests cover:
//! - Replay engine creation and configuration
//! - Checkpoint management
//! - Decision replay and verification
//! - Security tests (ensuring determinism)

use cognitum_gate_tilezero::replay::{
    ReplayDifference, ReplayEngine, ReplayError, ReplayResult, SequenceVerification,
    StateSnapshot, TileSnapshot,
};
use cognitum_gate_tilezero::receipt::{
    EvidentialWitness, PredictiveWitness, StructuralWitness, TimestampProof, WitnessReceipt,
    WitnessSummary,
};
use cognitum_gate_tilezero::permit::PermitToken;
use cognitum_gate_tilezero::GateDecision;
use std::collections::HashMap;

fn create_test_receipt(
    sequence: u64,
    decision: GateDecision,
    witness: WitnessSummary,
) -> WitnessReceipt {
    WitnessReceipt {
        sequence,
        token: PermitToken {
            decision,
            action_id: format!("action-{}", sequence),
            timestamp: 1000000000 + sequence * 1000,
            ttl_ns: 60_000_000_000,
            witness_hash: [0u8; 32],
            sequence,
            signature: [0u8; 64],
        },
        previous_hash: [0u8; 32],
        witness_summary: witness,
        timestamp_proof: TimestampProof {
            timestamp: 1000000000 + sequence * 1000,
            previous_receipt_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
    }
}

fn create_permit_witness() -> WitnessSummary {
    WitnessSummary {
        structural: StructuralWitness {
            cut_value: 10.0,
            partition: "stable".to_string(),
            critical_edges: 2,
            boundary: vec![],
        },
        predictive: PredictiveWitness {
            set_size: 5,
            coverage: 0.9,
        },
        evidential: EvidentialWitness {
            e_value: 150.0,
            verdict: "accept".to_string(),
        },
    }
}

fn create_defer_witness() -> WitnessSummary {
    WitnessSummary {
        structural: StructuralWitness {
            cut_value: 10.0,
            partition: "stable".to_string(),
            critical_edges: 5,
            boundary: vec![],
        },
        predictive: PredictiveWitness {
            set_size: 25, // Large set size -> defer
            coverage: 0.9,
        },
        evidential: EvidentialWitness {
            e_value: 50.0,
            verdict: "continue".to_string(),
        },
    }
}

fn create_deny_witness() -> WitnessSummary {
    WitnessSummary {
        structural: StructuralWitness {
            cut_value: 2.0,
            partition: "fragile".to_string(), // Fragile -> deny
            critical_edges: 10,
            boundary: vec![],
        },
        predictive: PredictiveWitness {
            set_size: 5,
            coverage: 0.9,
        },
        evidential: EvidentialWitness {
            e_value: 0.001,
            verdict: "reject".to_string(),
        },
    }
}

#[cfg(test)]
mod engine_creation {
    use super::*;

    #[test]
    fn test_default_engine() {
        let engine = ReplayEngine::default();
        assert_eq!(engine.checkpoint_count(), 0);
    }

    #[test]
    fn test_engine_with_interval() {
        let engine = ReplayEngine::new(50);
        assert_eq!(engine.checkpoint_count(), 0);
    }
}

#[cfg(test)]
mod checkpoint_management {
    use super::*;

    #[test]
    fn test_save_checkpoint() {
        let mut engine = ReplayEngine::new(10);

        let snapshot = StateSnapshot {
            sequence: 0,
            timestamp: 1000,
            global_min_cut: 10.0,
            aggregate_e_value: 100.0,
            min_coherence: 256,
            tile_states: HashMap::new(),
        };

        engine.save_checkpoint(0, snapshot);
        assert_eq!(engine.checkpoint_count(), 1);
    }

    #[test]
    fn test_checkpoint_at_interval() {
        let mut engine = ReplayEngine::new(10);

        // Checkpoint at 0, 10, 20 should be saved
        for seq in [0, 5, 10, 15, 20] {
            let snapshot = StateSnapshot {
                sequence: seq,
                timestamp: 1000 + seq,
                global_min_cut: 10.0,
                aggregate_e_value: 100.0,
                min_coherence: 256,
                tile_states: HashMap::new(),
            };
            engine.save_checkpoint(seq, snapshot);
        }

        // Only 0, 10, 20 should be saved (multiples of 10)
        assert_eq!(engine.checkpoint_count(), 3);
    }

    #[test]
    fn test_find_nearest_checkpoint() {
        let mut engine = ReplayEngine::new(10);

        for seq in [0, 10, 20] {
            let snapshot = StateSnapshot {
                sequence: seq,
                timestamp: 1000 + seq,
                global_min_cut: seq as f64,
                aggregate_e_value: 100.0,
                min_coherence: 256,
                tile_states: HashMap::new(),
            };
            engine.save_checkpoint(seq, snapshot);
        }

        // Find nearest for 15 -> should be 10
        let (found_seq, snapshot) = engine.find_nearest_checkpoint(15).unwrap();
        assert_eq!(found_seq, 10);
        assert_eq!(snapshot.global_min_cut, 10.0);

        // Find nearest for 25 -> should be 20
        let (found_seq, _) = engine.find_nearest_checkpoint(25).unwrap();
        assert_eq!(found_seq, 20);

        // Find nearest for 5 -> should be 0
        let (found_seq, _) = engine.find_nearest_checkpoint(5).unwrap();
        assert_eq!(found_seq, 0);
    }

    #[test]
    fn test_no_checkpoint_found() {
        let engine = ReplayEngine::new(10);
        assert!(engine.find_nearest_checkpoint(5).is_none());
    }

    #[test]
    fn test_prune_checkpoints() {
        let mut engine = ReplayEngine::new(10);

        for seq in [0, 10, 20, 30, 40, 50] {
            let snapshot = StateSnapshot {
                sequence: seq,
                timestamp: 1000 + seq,
                global_min_cut: 10.0,
                aggregate_e_value: 100.0,
                min_coherence: 256,
                tile_states: HashMap::new(),
            };
            engine.save_checkpoint(seq, snapshot);
        }

        assert_eq!(engine.checkpoint_count(), 6);

        engine.prune_before(30);

        assert_eq!(engine.checkpoint_count(), 3); // 30, 40, 50 remain
        assert!(engine.find_nearest_checkpoint(20).is_none());
        assert!(engine.find_nearest_checkpoint(30).is_some());
    }
}

#[cfg(test)]
mod decision_replay {
    use super::*;

    #[test]
    fn test_replay_permit() {
        let engine = ReplayEngine::new(100);
        let receipt = create_test_receipt(0, GateDecision::Permit, create_permit_witness());

        let result = engine.replay(&receipt);

        assert!(result.matched);
        assert_eq!(result.decision, GateDecision::Permit);
        assert_eq!(result.original_decision, GateDecision::Permit);
        assert!(result.differences.is_empty());
    }

    #[test]
    fn test_replay_defer() {
        let engine = ReplayEngine::new(100);
        let receipt = create_test_receipt(0, GateDecision::Defer, create_defer_witness());

        let result = engine.replay(&receipt);

        assert!(result.matched);
        assert_eq!(result.decision, GateDecision::Defer);
    }

    #[test]
    fn test_replay_deny() {
        let engine = ReplayEngine::new(100);
        let receipt = create_test_receipt(0, GateDecision::Deny, create_deny_witness());

        let result = engine.replay(&receipt);

        assert!(result.matched);
        assert_eq!(result.decision, GateDecision::Deny);
    }

    #[test]
    fn test_replay_mismatch() {
        let engine = ReplayEngine::new(100);

        // Create a receipt where the decision doesn't match the witness
        // Witness indicates DENY (fragile partition), but token says PERMIT
        let receipt = create_test_receipt(0, GateDecision::Permit, create_deny_witness());

        let result = engine.replay(&receipt);

        assert!(!result.matched);
        assert_eq!(result.decision, GateDecision::Deny); // Reconstructed from witness
        assert_eq!(result.original_decision, GateDecision::Permit); // From token
        assert!(!result.differences.is_empty());
    }

    #[test]
    fn test_replay_preserves_snapshot() {
        let engine = ReplayEngine::new(100);
        let witness = create_permit_witness();
        let receipt = create_test_receipt(0, GateDecision::Permit, witness.clone());

        let result = engine.replay(&receipt);

        assert_eq!(result.state_snapshot.structural.cut_value, witness.structural.cut_value);
        assert_eq!(result.state_snapshot.evidential.e_value, witness.evidential.e_value);
    }
}

#[cfg(test)]
mod sequence_verification {
    use super::*;

    #[test]
    fn test_verify_empty_sequence() {
        let engine = ReplayEngine::new(100);
        let verification = engine.verify_sequence(&[]);

        assert_eq!(verification.total_receipts, 0);
        assert!(verification.all_matched);
        assert_eq!(verification.mismatch_count(), 0);
    }

    #[test]
    fn test_verify_single_receipt() {
        let engine = ReplayEngine::new(100);
        let receipts = vec![create_test_receipt(0, GateDecision::Permit, create_permit_witness())];

        let verification = engine.verify_sequence(&receipts);

        assert_eq!(verification.total_receipts, 1);
        assert!(verification.all_matched);
    }

    #[test]
    fn test_verify_multiple_receipts() {
        let engine = ReplayEngine::new(100);
        let receipts = vec![
            create_test_receipt(0, GateDecision::Permit, create_permit_witness()),
            create_test_receipt(1, GateDecision::Defer, create_defer_witness()),
            create_test_receipt(2, GateDecision::Deny, create_deny_witness()),
        ];

        let verification = engine.verify_sequence(&receipts);

        assert_eq!(verification.total_receipts, 3);
        assert!(verification.all_matched);
        assert_eq!(verification.mismatch_count(), 0);
    }

    #[test]
    fn test_verify_with_mismatches() {
        let engine = ReplayEngine::new(100);
        let receipts = vec![
            create_test_receipt(0, GateDecision::Permit, create_permit_witness()),
            create_test_receipt(1, GateDecision::Permit, create_deny_witness()), // Mismatch!
            create_test_receipt(2, GateDecision::Deny, create_deny_witness()),
        ];

        let verification = engine.verify_sequence(&receipts);

        assert_eq!(verification.total_receipts, 3);
        assert!(!verification.all_matched);
        assert_eq!(verification.mismatch_count(), 1);

        let mismatches: Vec<_> = verification.mismatches().collect();
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].0, 1); // Sequence 1 mismatched
    }

    #[test]
    fn test_mismatches_iterator() {
        let engine = ReplayEngine::new(100);
        let receipts = vec![
            create_test_receipt(0, GateDecision::Permit, create_deny_witness()), // Mismatch
            create_test_receipt(1, GateDecision::Permit, create_permit_witness()),
            create_test_receipt(2, GateDecision::Defer, create_deny_witness()), // Mismatch
        ];

        let verification = engine.verify_sequence(&receipts);
        let mismatches: Vec<_> = verification.mismatches().collect();

        assert_eq!(mismatches.len(), 2);
    }
}

#[cfg(test)]
mod checkpoint_export_import {
    use super::*;

    #[test]
    fn test_export_checkpoint() {
        let mut engine = ReplayEngine::new(10);

        let snapshot = StateSnapshot {
            sequence: 0,
            timestamp: 1000,
            global_min_cut: 15.0,
            aggregate_e_value: 200.0,
            min_coherence: 512,
            tile_states: HashMap::new(),
        };

        engine.save_checkpoint(0, snapshot);

        let exported = engine.export_checkpoint(0);
        assert!(exported.is_some());

        let data = exported.unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_export_nonexistent() {
        let engine = ReplayEngine::new(10);
        assert!(engine.export_checkpoint(0).is_none());
    }

    #[test]
    fn test_import_checkpoint() {
        let mut engine1 = ReplayEngine::new(10);

        let snapshot = StateSnapshot {
            sequence: 0,
            timestamp: 1000,
            global_min_cut: 25.0,
            aggregate_e_value: 300.0,
            min_coherence: 768,
            tile_states: HashMap::new(),
        };

        engine1.save_checkpoint(0, snapshot);
        let exported = engine1.export_checkpoint(0).unwrap();

        let mut engine2 = ReplayEngine::new(10);
        assert!(engine2.import_checkpoint(0, &exported).is_ok());
        assert_eq!(engine2.checkpoint_count(), 1);

        let (_, imported) = engine2.find_nearest_checkpoint(0).unwrap();
        assert_eq!(imported.global_min_cut, 25.0);
    }

    #[test]
    fn test_import_invalid_data() {
        let mut engine = ReplayEngine::new(10);
        let result = engine.import_checkpoint(0, b"invalid json");
        assert!(matches!(result, Err(ReplayError::InvalidCheckpoint)));
    }
}

#[cfg(test)]
mod tile_snapshot {
    use super::*;

    #[test]
    fn test_tile_snapshot_in_state() {
        let mut tile_states = HashMap::new();
        tile_states.insert(
            1,
            TileSnapshot {
                tile_id: 1,
                coherence: 256,
                e_value: 10.0,
                boundary_edges: 5,
            },
        );
        tile_states.insert(
            2,
            TileSnapshot {
                tile_id: 2,
                coherence: 512,
                e_value: 20.0,
                boundary_edges: 3,
            },
        );

        let snapshot = StateSnapshot {
            sequence: 0,
            timestamp: 1000,
            global_min_cut: 10.0,
            aggregate_e_value: 100.0,
            min_coherence: 256,
            tile_states,
        };

        assert_eq!(snapshot.tile_states.len(), 2);
        assert_eq!(snapshot.tile_states.get(&1).unwrap().coherence, 256);
        assert_eq!(snapshot.tile_states.get(&2).unwrap().e_value, 20.0);
    }
}

#[cfg(test)]
mod replay_difference {
    use super::*;

    #[test]
    fn test_difference_structure() {
        let diff = ReplayDifference {
            field: "decision".to_string(),
            original: "permit".to_string(),
            replayed: "deny".to_string(),
        };

        assert_eq!(diff.field, "decision");
        assert_eq!(diff.original, "permit");
        assert_eq!(diff.replayed, "deny");
    }
}

#[cfg(test)]
mod determinism {
    use super::*;

    /// Test that replaying the same receipt always produces the same result
    #[test]
    fn test_replay_deterministic() {
        let engine = ReplayEngine::new(100);
        let receipt = create_test_receipt(0, GateDecision::Permit, create_permit_witness());

        let result1 = engine.replay(&receipt);
        let result2 = engine.replay(&receipt);

        assert_eq!(result1.decision, result2.decision);
        assert_eq!(result1.matched, result2.matched);
        assert_eq!(result1.differences.len(), result2.differences.len());
    }

    /// Test that different engines produce same results
    #[test]
    fn test_cross_engine_determinism() {
        let engine1 = ReplayEngine::new(100);
        let engine2 = ReplayEngine::new(50); // Different checkpoint interval

        let receipt = create_test_receipt(0, GateDecision::Defer, create_defer_witness());

        let result1 = engine1.replay(&receipt);
        let result2 = engine2.replay(&receipt);

        assert_eq!(result1.decision, result2.decision);
        assert_eq!(result1.matched, result2.matched);
    }

    /// Test sequence verification is deterministic
    #[test]
    fn test_sequence_verification_deterministic() {
        let engine = ReplayEngine::new(100);
        let receipts = vec![
            create_test_receipt(0, GateDecision::Permit, create_permit_witness()),
            create_test_receipt(1, GateDecision::Deny, create_deny_witness()),
        ];

        let v1 = engine.verify_sequence(&receipts);
        let v2 = engine.verify_sequence(&receipts);

        assert_eq!(v1.total_receipts, v2.total_receipts);
        assert_eq!(v1.all_matched, v2.all_matched);
        assert_eq!(v1.mismatch_count(), v2.mismatch_count());
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    /// Test that modified witness produces different replay result
    #[test]
    fn test_witness_tampering_detected() {
        let engine = ReplayEngine::new(100);

        let original = create_test_receipt(0, GateDecision::Permit, create_permit_witness());
        let original_result = engine.replay(&original);

        // Create tampered receipt with modified witness
        let mut tampered_witness = create_permit_witness();
        tampered_witness.structural.partition = "fragile".to_string();
        let tampered = create_test_receipt(0, GateDecision::Permit, tampered_witness);
        let tampered_result = engine.replay(&tampered);

        // Tampered one should fail replay
        assert!(original_result.matched);
        assert!(!tampered_result.matched);
    }

    /// Test audit trail completeness
    #[test]
    fn test_audit_trail() {
        let engine = ReplayEngine::new(100);
        let mut receipts = Vec::new();

        // Build a sequence of decisions
        for i in 0..10 {
            let witness = if i % 3 == 0 {
                create_permit_witness()
            } else if i % 3 == 1 {
                create_defer_witness()
            } else {
                create_deny_witness()
            };

            let decision = if i % 3 == 0 {
                GateDecision::Permit
            } else if i % 3 == 1 {
                GateDecision::Defer
            } else {
                GateDecision::Deny
            };

            receipts.push(create_test_receipt(i, decision, witness));
        }

        let verification = engine.verify_sequence(&receipts);

        // All should match since we built them consistently
        assert!(verification.all_matched);
        assert_eq!(verification.total_receipts, 10);
    }
}

// Property-based tests
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_replay_always_produces_result(sequence in 0u64..1000) {
            let engine = ReplayEngine::new(100);
            let receipt = create_test_receipt(
                sequence,
                GateDecision::Permit,
                create_permit_witness()
            );

            let result = engine.replay(&receipt);
            // Should always produce a valid result
            assert!(result.decision == GateDecision::Permit ||
                    result.decision == GateDecision::Defer ||
                    result.decision == GateDecision::Deny);
        }

        #[test]
        fn prop_checkpoint_interval_works(interval in 1u64..100) {
            let mut engine = ReplayEngine::new(interval);

            for seq in 0..interval * 3 {
                let snapshot = StateSnapshot {
                    sequence: seq,
                    timestamp: 1000 + seq,
                    global_min_cut: 10.0,
                    aggregate_e_value: 100.0,
                    min_coherence: 256,
                    tile_states: HashMap::new(),
                };
                engine.save_checkpoint(seq, snapshot);
            }

            // Should have saved at least 3 checkpoints
            assert!(engine.checkpoint_count() >= 3);
        }

        #[test]
        fn prop_matching_decisions_have_empty_differences(seq in 0u64..100) {
            let engine = ReplayEngine::new(100);

            // Create receipts where decision matches witness
            let receipts = vec![
                (GateDecision::Permit, create_permit_witness()),
                (GateDecision::Defer, create_defer_witness()),
                (GateDecision::Deny, create_deny_witness()),
            ];

            for (decision, witness) in receipts {
                let receipt = create_test_receipt(seq, decision, witness);
                let result = engine.replay(&receipt);
                if result.matched {
                    assert!(result.differences.is_empty());
                }
            }
        }
    }
}
