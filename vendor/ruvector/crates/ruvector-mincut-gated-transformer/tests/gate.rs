//! Gate decision tests.
//!
//! Verifies that synthetic lambda traces produce expected tier changes.

use ruvector_mincut_gated_transformer::{
    gate::GateController, GateDecision, GatePacket, GatePolicy, GateReason, SpikePacket,
};

fn create_controller() -> GateController {
    GateController::new(GatePolicy::default())
}

// ============ Lambda-based decisions ============

#[test]
fn test_lambda_above_min_allows() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100, // Well above min (30)
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::Allow);
    assert_eq!(decision.tier, 0);
}

#[test]
fn test_lambda_below_min_quarantines() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 20, // Below min (30)
        lambda_prev: 100,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::QuarantineUpdates);
    assert_eq!(decision.reason, GateReason::LambdaBelowMin);
    assert_eq!(decision.tier, 2);
}

#[test]
fn test_lambda_drop_flushes_kv() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 40,
        lambda_prev: 100, // 60% drop - exceeds default max ~37.5%
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::FlushKv);
    assert_eq!(decision.reason, GateReason::LambdaDroppedFast);
}

#[test]
fn test_lambda_gradual_drop_allows() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 90,
        lambda_prev: 100, // 10% drop - within tolerance
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::Allow);
}

// ============ Boundary-based decisions ============

#[test]
fn test_boundary_spike_reduces_scope() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Above max (20)
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::ReduceScope);
    assert_eq!(decision.reason, GateReason::BoundarySpike);
    assert_eq!(decision.tier, 1);
}

#[test]
fn test_boundary_concentration_reduces_scope() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 25000, // Above max (~62.5%)
        partition_count: 3,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::ReduceScope);
    assert_eq!(decision.reason, GateReason::BoundaryConcentrationSpike);
}

// ============ Partition-based decisions ============

#[test]
fn test_partition_drift_reduces_scope() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 15, // Above max (10)
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::ReduceScope);
    assert_eq!(decision.reason, GateReason::PartitionDrift);
}

// ============ Flag-based decisions ============

#[test]
fn test_force_safe_flag() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.decision, GateDecision::FreezeWrites);
    assert_eq!(decision.reason, GateReason::ForcedByFlag);
    assert_eq!(decision.tier, 2);
}

#[test]
fn test_skip_flag() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_SKIP,
    };

    let decision = controller.evaluate(&gate, None);
    assert!(decision.skip);
    assert_eq!(decision.tier, 3);
}

// ============ Spike-based decisions ============

#[test]
fn test_spike_inactive_skips() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let spike = SpikePacket {
        fired: 0, // Not fired
        rate_q15: 10000,
        novelty_q15: 15000,
        ..Default::default()
    };

    let decision = controller.evaluate(&gate, Some(&spike));
    assert!(decision.skip);
    assert_eq!(decision.tier, 3);
}

#[test]
fn test_spike_active_allows() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let spike = SpikePacket {
        fired: 1,        // Fired
        rate_q15: 10000, // Normal rate
        novelty_q15: 15000,
        ..Default::default()
    };

    let decision = controller.evaluate(&gate, Some(&spike));
    assert!(!decision.skip);
    assert_eq!(decision.decision, GateDecision::Allow);
}

#[test]
fn test_spike_storm_freezes() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    let spike = SpikePacket {
        fired: 1,
        rate_q15: 30000, // Very high - exceeds max
        novelty_q15: 5000,
        ..Default::default()
    };

    let decision = controller.evaluate(&gate, Some(&spike));
    assert_eq!(decision.decision, GateDecision::FreezeWrites);
    assert_eq!(decision.reason, GateReason::SpikeStorm);
    assert_eq!(decision.tier, 2);
}

// ============ Policy variants ============

#[test]
fn test_conservative_policy() {
    let controller = GateController::new(GatePolicy::conservative());

    // Same conditions that would pass with default policy
    let gate = GatePacket {
        lambda: 40, // Below conservative min (50) but above default (30)
        lambda_prev: 45,
        boundary_edges: 8, // Below conservative max (10)
        boundary_concentration_q15: 12000,
        partition_count: 4,
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    // Conservative should intervene
    assert_eq!(decision.decision, GateDecision::QuarantineUpdates);
}

#[test]
fn test_permissive_policy() {
    let controller = GateController::new(GatePolicy::permissive());

    // Conditions that would trigger intervention with default policy
    let gate = GatePacket {
        lambda: 25, // Above permissive min (20) but below default (30)
        lambda_prev: 35,
        boundary_edges: 40, // Above default max (20) but below permissive (50)
        boundary_concentration_q15: 20000,
        partition_count: 15, // Above default max (10) but below permissive (20)
        flags: 0,
    };

    let decision = controller.evaluate(&gate, None);
    // Permissive should allow
    assert_eq!(decision.decision, GateDecision::Allow);
}

// ============ Tier decision properties ============

#[test]
fn test_tier0_full_layers() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        ..Default::default()
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.tier, 0);
    assert!(!decision.skip);
    // In default controller, layers_to_run should be layers_normal (4)
    assert_eq!(decision.layers_to_run, 4);
}

#[test]
fn test_tier1_reduced_layers() {
    let controller = create_controller();

    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Triggers ReduceScope
        ..Default::default()
    };

    let decision = controller.evaluate(&gate, None);
    assert_eq!(decision.tier, 1);
    // Should have reduced layers
    assert!(decision.layers_to_run < 4);
}

#[test]
fn test_kv_writes_permission() {
    let controller = create_controller();

    // Allow case - KV writes enabled
    let gate_allow = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        ..Default::default()
    };
    let decision = controller.evaluate(&gate_allow, None);
    assert!(decision.decision.allows_kv_writes());

    // FreezeWrites case - KV writes disabled
    let gate_freeze = GatePacket {
        lambda: 100,
        flags: GatePacket::FLAG_FORCE_SAFE,
        ..Default::default()
    };
    let decision = controller.evaluate(&gate_freeze, None);
    assert!(!decision.decision.allows_kv_writes());
}

#[test]
fn test_external_writes_permission() {
    let controller = create_controller();

    // Allow case - external writes enabled
    let gate_allow = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        ..Default::default()
    };
    let decision = controller.evaluate(&gate_allow, None);
    assert!(decision.decision.allows_external_writes());

    // ReduceScope case - external writes disabled
    let gate_reduce = GatePacket {
        lambda: 100,
        boundary_edges: 30,
        ..Default::default()
    };
    let decision = controller.evaluate(&gate_reduce, None);
    assert!(!decision.decision.allows_external_writes());
}

// ============ Lambda delta calculation ============

#[test]
fn test_lambda_delta_positive() {
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 80,
        ..Default::default()
    };
    assert_eq!(gate.lambda_delta(), 20);
}

#[test]
fn test_lambda_delta_negative() {
    let gate = GatePacket {
        lambda: 80,
        lambda_prev: 100,
        ..Default::default()
    };
    assert_eq!(gate.lambda_delta(), -20);
}

#[test]
fn test_drop_ratio_calculation() {
    let gate = GatePacket {
        lambda: 50,
        lambda_prev: 100, // 50% drop
        ..Default::default()
    };

    let ratio = gate.drop_ratio_q15();
    // Should be around 16384 (50% of 32768)
    assert!(ratio > 16000 && ratio < 17000);
}

#[test]
fn test_drop_ratio_no_drop() {
    let gate = GatePacket {
        lambda: 100,
        lambda_prev: 80, // Increase, not drop
        ..Default::default()
    };

    let ratio = gate.drop_ratio_q15();
    assert_eq!(ratio, 0);
}
