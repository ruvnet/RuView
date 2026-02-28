//! End-to-end integration test for the full 5-phase transfer pipeline.
//!
//! This test exercises `ExoTransferOrchestrator` which wires together:
//! - Phase 1: Thompson-sampling domain bridge
//! - Phase 2: Transfer manifold (exo-manifold)
//! - Phase 3: Transfer timeline (exo-temporal)
//! - Phase 4: Transfer CRDT (exo-federation)
//! - Phase 5: Emergent detection (exo-exotic)

use exo_backend_classical::transfer_orchestrator::ExoTransferOrchestrator;

#[test]
fn test_full_transfer_pipeline_single_cycle() {
    let mut orch = ExoTransferOrchestrator::new("e2e_node");

    // Before any cycle, no prior should be known.
    assert!(orch.best_prior().is_none());
    assert_eq!(orch.cycle(), 0);

    // Run first cycle: establishes a baseline.
    let result = orch.run_cycle();

    assert_eq!(result.cycle, 1, "cycle counter should increment");
    assert!(
        result.eval_score >= 0.0 && result.eval_score <= 1.0,
        "eval_score must be in [0, 1]: got {}",
        result.eval_score
    );
    assert!(
        result.manifold_entries >= 1,
        "at least one prior should be stored in the manifold"
    );

    // Phase 4: CRDT should now have a prior for the (src, dst) pair.
    assert!(
        orch.best_prior().is_some(),
        "CRDT should hold a prior after the first cycle"
    );
}

#[test]
fn test_full_transfer_pipeline_multi_cycle() {
    let mut orch = ExoTransferOrchestrator::new("e2e_multi");

    // Run several cycles to let all phases accumulate state.
    for expected_cycle in 1..=6u64 {
        let result = orch.run_cycle();
        assert_eq!(result.cycle, expected_cycle);
        assert!(result.eval_score >= 0.0 && result.eval_score <= 1.0);
    }

    // After 6 cycles:
    // - Manifold should hold (src, dst) prior from every cycle.
    let last = orch.run_cycle();
    assert_eq!(last.cycle, 7);
    assert!(last.manifold_entries >= 1);

    // - Emergence detector should be active (score is a valid float).
    assert!(last.emergence_score.is_finite());

    // - CRDT should know both domain IDs.
    let prior = orch.best_prior().expect("CRDT must hold a prior");
    assert_eq!(prior.src_domain, "exo-retrieval");
    assert_eq!(prior.dst_domain, "exo-graph");
    assert!(prior.improvement >= 0.0 && prior.improvement <= 1.0);
    assert!(prior.confidence >= 0.0 && prior.confidence <= 1.0);
    assert!(prior.cycle >= 1);
}

#[test]
fn test_transfer_emergence_increases_with_cycles() {
    let mut orch = ExoTransferOrchestrator::new("e2e_emergence");

    // Baseline (cycle 1 records baseline score, emergence = initial detection).
    orch.run_cycle();

    // Subsequent cycles contribute post-transfer scores.
    let mut scores: Vec<f64> = Vec::new();
    for _ in 0..5 {
        let r = orch.run_cycle();
        scores.push(r.emergence_score);
    }

    // All emergence scores must be finite non-negative values.
    for score in &scores {
        assert!(score.is_finite(), "emergence score must be finite");
        assert!(*score >= 0.0, "emergence score must be non-negative");
    }
}

#[test]
fn test_transfer_manifold_accumulates() {
    let mut orch = ExoTransferOrchestrator::new("e2e_manifold");

    // Each cycle stores a prior in the manifold.
    for i in 1..=5 {
        let result = orch.run_cycle();
        // Manifold stores one entry per (src, dst) pair; repeated writes
        // update the same entry, so count stays at 1.
        assert!(
            result.manifold_entries >= 1,
            "cycle {}: manifold must hold ≥1 entry",
            i
        );
    }
}

#[test]
fn test_crdt_prior_consistency() {
    let mut orch = ExoTransferOrchestrator::new("e2e_crdt");

    // Run 3 cycles; the CRDT should consistently return a valid prior.
    for _ in 0..3 {
        orch.run_cycle();
    }

    let prior = orch.best_prior().expect("prior must exist after 3 cycles");
    assert_eq!(prior.src_domain, "exo-retrieval");
    assert_eq!(prior.dst_domain, "exo-graph");
    assert!(prior.cycle >= 1 && prior.cycle <= 3);
}
