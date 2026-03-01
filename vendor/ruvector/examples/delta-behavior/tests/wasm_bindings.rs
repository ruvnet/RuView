//! Tests for WASM bindings
//!
//! These tests verify the WASM bindings work correctly on native targets.
//! The actual WASM tests require `wasm-bindgen-test` and run in a browser/node environment.

use delta_behavior::wasm::*;

#[test]
fn test_wasm_coherence() {
    let coherence = WasmCoherence::new(0.75).unwrap();
    assert!((coherence.value() - 0.75).abs() < 0.001);
    assert!(coherence.is_above(0.5));
    assert!(coherence.is_below(0.8));
}

#[test]
fn test_wasm_coherence_bounds() {
    let bounds = WasmCoherenceBounds::new(0.3, 0.5, 0.8, 0.1).unwrap();
    assert!((bounds.min_coherence() - 0.3).abs() < 0.001);
    assert!(bounds.is_within_bounds(0.5));
    assert!(!bounds.is_within_bounds(0.2));
    assert!(bounds.should_throttle(0.4));
}

#[test]
fn test_wasm_self_limiting_reasoner() {
    let mut reasoner = WasmSelfLimitingReasoner::new(10, 5);
    assert!((reasoner.coherence() - 1.0).abs() < 0.001);

    // At full coherence, should have full depth
    assert_eq!(reasoner.allowed_depth(), 10);
    assert!(reasoner.can_write_memory());

    // Reduce coherence
    reasoner.update_coherence(-0.7);
    assert!(reasoner.allowed_depth() < 10);
}

#[test]
fn test_wasm_event_horizon() {
    let mut horizon = WasmEventHorizon::new(3, 10.0);
    assert_eq!(horizon.dimensions(), 3);
    assert!((horizon.horizon_radius() - 10.0).abs() < 0.001);
    assert!(horizon.energy_budget() > 0.0);

    // Should be able to move away from origin
    let result = horizon.move_toward("[1.0, 1.0, 1.0]");
    assert!(result.contains("moved") || result.contains("asymptotic"));
}

#[test]
fn test_wasm_homeostatic_organism() {
    let mut organism = WasmHomeostaticOrganism::new(1);
    assert!(organism.alive());
    assert!((organism.coherence() - 1.0).abs() < 0.001);

    // Should be able to eat
    let result = organism.eat(10.0);
    assert!(result.contains("success"));
}

#[test]
fn test_wasm_world_model() {
    let mut model = WasmSelfStabilizingWorldModel::new();
    assert!((model.coherence() - 1.0).abs() < 0.001);
    assert!(model.is_learning());

    // Add an observation
    let obs = r#"{"entity_id": 1, "properties": {"x": 1.0}, "source_confidence": 0.9}"#;
    let result = model.observe(obs);
    assert!(result.contains("applied") || result.contains("frozen"));
}

#[test]
fn test_wasm_bounded_creator() {
    let mut creator = WasmCoherenceBoundedCreator::new(0.5, 0.3, 0.95);
    assert!(creator.exploration_budget() > 0.0);

    // Should be able to create
    let result = creator.create(0.1);
    let status: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(status.get("status").is_some());
}

#[test]
fn test_wasm_financial_system() {
    let mut system = WasmAntiCascadeFinancialSystem::new();
    assert!((system.coherence() - 1.0).abs() < 0.001);
    assert_eq!(system.circuit_breaker_state(), WasmCircuitBreakerState::Open);

    // Small leverage should work
    let result = system.open_leverage(100.0, 2.0);
    assert!(result.contains("executed") || result.contains("queued"));
}

#[test]
fn test_wasm_aging_system() {
    let mut system = WasmGracefullyAgingSystem::new();
    assert!(system.has_capability(WasmCapability::BasicReads));
    assert!(system.has_capability(WasmCapability::SchemaMigration));

    // Age past first threshold
    system.simulate_age(400.0);

    // Schema migration should be removed
    assert!(!system.has_capability(WasmCapability::SchemaMigration));
    assert!(system.has_capability(WasmCapability::BasicReads)); // Always available
}

#[test]
fn test_wasm_coherent_swarm() {
    let mut swarm = WasmCoherentSwarm::new(0.6);
    assert_eq!(swarm.agent_count(), 0);

    // Add agents
    swarm.add_agent("a1", 0.0, 0.0);
    swarm.add_agent("a2", 1.0, 0.0);
    swarm.add_agent("a3", 0.0, 1.0);

    assert_eq!(swarm.agent_count(), 3);
    assert!(swarm.coherence() > 0.8); // Tight cluster should be coherent
}

#[test]
fn test_wasm_graceful_system() {
    let mut system = WasmGracefulSystem::new();
    assert_eq!(system.state(), WasmSystemState::Running);
    assert!(system.can_accept_work());

    // Degrade coherence
    system.apply_coherence_change(-0.5);
    assert!(matches!(
        system.state(),
        WasmSystemState::Degraded | WasmSystemState::ShuttingDown
    ));
}

#[test]
fn test_wasm_containment_substrate() {
    let mut substrate = WasmContainmentSubstrate::new();
    assert!((substrate.intelligence() - 1.0).abs() < 0.001);
    assert!(substrate.check_invariants());

    // Try to grow reasoning
    let result = substrate.attempt_growth(WasmCapabilityDomain::Reasoning, 0.3);
    let status: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        status.get("status").unwrap().as_str() == Some("approved") ||
        status.get("status").unwrap().as_str() == Some("dampened")
    );

    // Invariants should still hold
    assert!(substrate.check_invariants());
}

#[test]
fn test_wasm_version() {
    let version = delta_behavior::wasm::version();
    assert!(!version.is_empty());
}

#[test]
fn test_wasm_description() {
    let desc = delta_behavior::wasm::description();
    assert!(desc.contains("Delta-Behavior"));
}
