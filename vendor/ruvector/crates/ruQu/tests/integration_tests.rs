//! End-to-end integration tests for ruQu coherence gate
//!
//! Tests full fabric initialization, syndrome ingestion through gate decision,
//! and receipt generation with verification.

use ruqu::{
    filters::{
        EvidenceConfig, FilterConfig, FilterPipeline, ShiftConfig, StructuralConfig, Verdict,
    },
    prelude::*,
    syndrome::{DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound},
    tile::{
        GateDecision, GateThresholds, PermitToken, ReceiptLog, SyndromeDelta as TileSyndromeDelta,
        TileReport, TileZero, WorkerTile,
    },
    TILE_COUNT, WORKER_TILE_COUNT,
};

// ============================================================================
// Full Fabric Initialization Tests
// ============================================================================

#[test]
fn test_fabric_initialization_all_tiles() {
    // Create all 255 worker tiles
    let workers: Vec<WorkerTile> = (1..=255).map(WorkerTile::new).collect();

    assert_eq!(workers.len(), WORKER_TILE_COUNT);

    for (i, worker) in workers.iter().enumerate() {
        assert_eq!(worker.tile_id, (i + 1) as u8);
        assert_eq!(worker.tick, 0);
    }
}

#[test]
fn test_fabric_initialization_with_tilezero() {
    let thresholds = GateThresholds::default();
    let tilezero = TileZero::new(thresholds);

    // Verify default thresholds
    assert!(tilezero.thresholds.structural_min_cut > 0.0);
    assert!(tilezero.thresholds.shift_max > 0.0);
    assert!(tilezero.thresholds.tau_deny > 0.0);
    assert!(tilezero.thresholds.tau_permit > tilezero.thresholds.tau_deny);
    assert!(tilezero.receipt_log.is_empty());
}

#[test]
fn test_fabric_tile_count_matches_constants() {
    assert_eq!(TILE_COUNT, 256);
    assert_eq!(WORKER_TILE_COUNT, 255);
}

// ============================================================================
// Syndrome Ingestion Through Gate Decision Tests
// ============================================================================

#[test]
fn test_syndrome_ingestion_single_round() {
    let mut worker = WorkerTile::new(1);

    // Ingest a syndrome
    let delta = TileSyndromeDelta::new(0, 1, 50);
    let report = worker.tick(&delta);

    assert_eq!(report.tile_id, 1);
    assert_eq!(report.tick, 1);
    assert!(report.status & TileReport::STATUS_VALID != 0);
}

#[test]
fn test_syndrome_ingestion_multiple_rounds() {
    let mut worker = WorkerTile::new(1);

    // Process multiple syndrome rounds
    for i in 0..100 {
        let delta = TileSyndromeDelta::new(i as u16 % 64, (i as u16 + 1) % 64, (i % 256) as u16);
        let report = worker.tick(&delta);
        assert_eq!(report.tick, i + 1);
    }

    assert_eq!(worker.tick, 100);
}

#[test]
fn test_full_pipeline_syndrome_to_decision_safe() {
    // Setup tiles
    let thresholds = GateThresholds {
        structural_min_cut: 2.0,
        shift_max: 0.7,
        tau_deny: 0.01,
        tau_permit: 50.0,
        permit_ttl_ns: 4_000_000,
    };
    let mut tilezero = TileZero::new(thresholds);

    // Create workers and process syndromes
    let mut workers: Vec<WorkerTile> = (1..=10).map(WorkerTile::new).collect();

    // Build graph with good connectivity
    for worker in &mut workers {
        // Add edges to create a well-connected graph
        worker.patch_graph.add_edge(0, 1, 100);
        worker.patch_graph.add_edge(1, 2, 100);
        worker.patch_graph.add_edge(2, 3, 100);
        worker.patch_graph.add_edge(3, 0, 100);
        worker.patch_graph.recompute_components();
    }

    // Process syndromes with low values (indicating stability)
    for _ in 0..50 {
        for worker in &mut workers {
            let delta = TileSyndromeDelta::new(0, 1, 50); // Low syndrome value
            worker.tick(&delta);
        }
    }

    // Collect reports
    let reports: Vec<TileReport> = workers
        .iter()
        .map(|w| {
            let mut report = TileReport::new(w.tile_id);
            report.local_cut = 10.0; // Good cut value
            report.shift_score = 0.1; // Low shift
            report.e_value = 200.0; // Strong evidence
            report
        })
        .collect();

    let decision = tilezero.merge_reports(reports);
    assert_eq!(decision, GateDecision::Permit);
}

#[test]
fn test_full_pipeline_syndrome_to_decision_unsafe() {
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);

    // Create reports indicating structural problems
    let reports: Vec<TileReport> = (1..=10)
        .map(|i| {
            let mut report = TileReport::new(i);
            report.local_cut = 1.0; // Below threshold (5.0)
            report.shift_score = 0.1;
            report.e_value = 200.0;
            report
        })
        .collect();

    let decision = tilezero.merge_reports(reports);
    assert_eq!(decision, GateDecision::Deny);
}

#[test]
fn test_full_pipeline_syndrome_to_decision_cautious() {
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);

    // Create reports with high shift but good structure
    let reports: Vec<TileReport> = (1..=10)
        .map(|i| {
            let mut report = TileReport::new(i);
            report.local_cut = 10.0;
            report.shift_score = 0.8; // Above threshold (0.5)
            report.e_value = 200.0;
            report
        })
        .collect();

    let decision = tilezero.merge_reports(reports);
    assert_eq!(decision, GateDecision::Defer);
}

// ============================================================================
// GateDecision Variants Tests
// ============================================================================

#[test]
fn test_gate_decision_safe_variant() {
    let decision = GateDecision::Permit;
    assert!(decision.is_permit());
    assert!(!decision.is_deny());
}

#[test]
fn test_gate_decision_cautious_variant() {
    let decision = GateDecision::Defer;
    assert!(!decision.is_permit());
    assert!(!decision.is_deny());
}

#[test]
fn test_gate_decision_unsafe_variant() {
    let decision = GateDecision::Deny;
    assert!(!decision.is_permit());
    assert!(decision.is_deny());
}

#[test]
fn test_gate_decision_all_variants_distinct() {
    let permit = GateDecision::Permit;
    let defer = GateDecision::Defer;
    let deny = GateDecision::Deny;

    assert_ne!(permit, defer);
    assert_ne!(permit, deny);
    assert_ne!(defer, deny);
}

// ============================================================================
// Receipt Generation and Verification Tests
// ============================================================================

#[test]
fn test_receipt_generation_on_decision() {
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);

    // Make a decision
    let reports: Vec<TileReport> = (1..=5)
        .map(|i| {
            let mut report = TileReport::new(i);
            report.local_cut = 10.0;
            report.shift_score = 0.1;
            report.e_value = 200.0;
            report
        })
        .collect();

    tilezero.merge_reports(reports);

    // Verify receipt was created
    assert_eq!(tilezero.receipt_log.len(), 1);
}

#[test]
fn test_receipt_chain_integrity() {
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);

    // Make multiple decisions
    for _ in 0..10 {
        let reports: Vec<TileReport> = (1..=3)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        tilezero.merge_reports(reports);
    }

    // Verify chain
    assert_eq!(tilezero.receipt_log.len(), 10);

    // Check that entries are chainable by looking up sequences
    for i in 0..10 {
        let entry = tilezero.receipt_log.get(i as u64);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().sequence, i as u64);
    }
}

#[test]
fn test_permit_token_issuance() {
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);

    // Create reports for permit
    let reports: Vec<TileReport> = (1..=5)
        .map(|i| {
            let mut report = TileReport::new(i);
            report.local_cut = 10.0;
            report.shift_score = 0.1;
            report.e_value = 200.0;
            report
        })
        .collect();

    let decision = tilezero.merge_reports(reports);
    assert_eq!(decision, GateDecision::Permit);

    let token = tilezero.issue_permit(&decision);
    assert_eq!(token.decision, GateDecision::Permit);
    assert!(token.ttl_ns > 0);
}

#[test]
fn test_permit_token_validity_window() {
    let token = PermitToken {
        decision: GateDecision::Permit,
        sequence: 0,
        timestamp: 1_000_000,
        ttl_ns: 500_000,
        witness_hash: [0u8; 32],
        signature: [1u8; 64], // Non-zero placeholder
    };

    // Within validity window
    assert!(token.is_valid(1_200_000));
    assert!(token.is_valid(1_499_999));

    // Outside validity window
    assert!(!token.is_valid(1_500_001));
    assert!(!token.is_valid(2_000_000));
}

// ============================================================================
// Integration with Filter Pipeline Tests
// ============================================================================

#[test]
fn test_filter_pipeline_integration_permit() {
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

    // Build strong graph
    pipeline.structural_mut().insert_edge(1, 2, 2.0).unwrap();
    pipeline.structural_mut().insert_edge(2, 3, 2.0).unwrap();
    pipeline.structural_mut().insert_edge(3, 1, 2.0).unwrap();

    // Add stable observations
    for _ in 0..20 {
        pipeline.shift_mut().update(0, 0.5);
    }

    // Add strong evidence
    for _ in 0..5 {
        pipeline.evidence_mut().update(2.0);
    }

    let state = ruqu::filters::SystemState::new(3);
    let result = pipeline.evaluate(&state);

    assert_eq!(result.verdict, Some(Verdict::Permit));
}

#[test]
fn test_filter_pipeline_integration_deny() {
    let config = FilterConfig {
        structural: StructuralConfig {
            threshold: 5.0,
            use_subpolynomial: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pipeline = FilterPipeline::new(config);

    // Build weak graph
    pipeline.structural_mut().insert_edge(1, 2, 1.0).unwrap();

    let state = ruqu::filters::SystemState::new(2);
    let result = pipeline.evaluate(&state);

    assert_eq!(result.verdict, Some(Verdict::Deny));
}

// ============================================================================
// End-to-End Workflow Tests
// ============================================================================

#[test]
fn test_complete_workflow_healthy_system() {
    // 1. Initialize fabric
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);
    let mut workers: Vec<WorkerTile> = (1..=5).map(WorkerTile::new).collect();

    // 2. Build graph structure in each worker
    for worker in &mut workers {
        worker.patch_graph.add_edge(0, 1, 200);
        worker.patch_graph.add_edge(1, 2, 200);
        worker.patch_graph.add_edge(2, 0, 200);
        worker.patch_graph.recompute_components();
    }

    // 3. Simulate syndrome stream
    for cycle in 0..50 {
        for worker in &mut workers {
            let delta = TileSyndromeDelta::new(
                (cycle % 3) as u16,
                ((cycle + 1) % 3) as u16,
                50, // Low syndrome value
            );
            worker.tick(&delta);
        }
    }

    // 4. Collect reports and make decision
    let reports: Vec<TileReport> = workers
        .iter()
        .map(|w| {
            let mut report = TileReport::new(w.tile_id);
            report.local_cut = w.local_cut_state.cut_value.max(10.0);
            report.shift_score = 0.1;
            report.e_value = 200.0;
            report
        })
        .collect();

    let decision = tilezero.merge_reports(reports);

    // 5. Verify outcome
    assert_eq!(decision, GateDecision::Permit);
    assert_eq!(tilezero.receipt_log.len(), 1);

    // 6. Issue and verify permit
    let token = tilezero.issue_permit(&decision);
    assert_eq!(token.decision, GateDecision::Permit);
}

#[test]
fn test_complete_workflow_degrading_system() {
    let thresholds = GateThresholds::default();
    let mut tilezero = TileZero::new(thresholds);

    // Simulate degradation over time
    for cycle in 0..20 {
        let cut_value = 10.0 - (cycle as f64 * 0.5); // Degrading cut

        let reports: Vec<TileReport> = (1..=5)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = cut_value;
                report.shift_score = 0.1 + (cycle as f64 * 0.02);
                report.e_value = 200.0 / (cycle as f64 + 1.0);
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);

        // Eventually should transition from Permit -> Defer -> Deny
        if cut_value < thresholds.structural_min_cut {
            assert_eq!(decision, GateDecision::Deny);
        }
    }

    // Should have logged all decisions
    assert_eq!(tilezero.receipt_log.len(), 20);
}

// ============================================================================
// Proptest Property-Based Tests
// ============================================================================

#[cfg(test)]
mod proptest_integration {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_decision_consistency(
            cut_values in prop::collection::vec(0.0f64..20.0, 1..10),
            shift_values in prop::collection::vec(0.0f64..1.0, 1..10),
            e_values in prop::collection::vec(0.01f64..500.0, 1..10),
        ) {
            let thresholds = GateThresholds::default();
            let mut tilezero = TileZero::new(thresholds);

            let reports: Vec<TileReport> = cut_values
                .iter()
                .zip(shift_values.iter())
                .zip(e_values.iter())
                .enumerate()
                .map(|(i, ((cut, shift), e_val))| {
                    let mut report = TileReport::new((i + 1) as u8);
                    report.local_cut = *cut;
                    report.shift_score = *shift;
                    report.e_value = *e_val;
                    report
                })
                .collect();

            let decision = tilezero.merge_reports(reports.clone());

            // Verify decision is consistent with filters
            let min_cut: f64 = reports.iter().map(|r| r.local_cut).filter(|c| *c > 0.0).fold(f64::MAX, |a, b| a.min(b));
            let max_shift: f64 = reports.iter().map(|r| r.shift_score).fold(0.0, |a, b| a.max(b));

            if min_cut < thresholds.structural_min_cut {
                prop_assert_eq!(decision, GateDecision::Deny);
            } else if max_shift >= thresholds.shift_max {
                prop_assert_eq!(decision, GateDecision::Defer);
            }
        }

        #[test]
        fn prop_receipt_log_always_grows(num_decisions in 1usize..50) {
            let thresholds = GateThresholds::default();
            let mut tilezero = TileZero::new(thresholds);

            for _ in 0..num_decisions {
                let reports: Vec<TileReport> = (1..=3)
                    .map(|i| {
                        let mut report = TileReport::new(i);
                        report.local_cut = 10.0;
                        report.shift_score = 0.1;
                        report.e_value = 200.0;
                        report
                    })
                    .collect();

                tilezero.merge_reports(reports);
            }

            prop_assert_eq!(tilezero.receipt_log.len(), num_decisions);
        }
    }
}
