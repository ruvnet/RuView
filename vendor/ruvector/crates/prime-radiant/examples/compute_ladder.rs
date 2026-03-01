//! Compute Ladder Example
//!
//! This example demonstrates Prime-Radiant's 4-lane compute ladder
//! for energy-based routing and escalation.
//!
//! The compute ladder routes actions to different processing lanes based on
//! coherence energy:
//! - Lane 0 (Reflex): Instant, low-cost processing for coherent actions
//! - Lane 1 (Retrieval): Light reasoning with evidence fetching
//! - Lane 2 (Heavy): Multi-step planning, spectral analysis
//! - Lane 3 (Human): Escalation for sustained incoherence
//!
//! Run with: `cargo run --example compute_ladder`

use prime_radiant::execution::{
    Action, ActionImpact, ActionMetadata, CoherenceGate, ComputeLane, EnergySnapshot, GateDecision,
    LaneThresholds, PolicyBundleRef, ScopeId,
};
use std::time::Duration;

fn main() {
    println!("=== Prime-Radiant: Compute Ladder Example ===\n");

    // Example 1: Low energy - Reflex lane
    println!("--- Example 1: Low Energy -> Reflex Lane ---");
    run_reflex_example();

    println!();

    // Example 2: Medium energy - Retrieval lane
    println!("--- Example 2: Medium Energy -> Retrieval Lane ---");
    run_retrieval_example();

    println!();

    // Example 3: High energy - Heavy lane
    println!("--- Example 3: High Energy -> Heavy Lane ---");
    run_heavy_example();

    println!();

    // Example 4: Very high energy - Human escalation
    println!("--- Example 4: Very High Energy -> Human Escalation ---");
    run_human_escalation_example();

    println!();

    // Example 5: Custom thresholds
    println!("--- Example 5: Custom Threshold Configuration ---");
    run_custom_thresholds_example();
}

/// Simple error type for example actions
#[derive(Debug)]
struct ExampleError(String);

impl std::fmt::Display for ExampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ExampleError {}

/// Example action for demonstration
struct ExampleAction {
    name: String,
    scope: ScopeId,
    impact: ActionImpact,
    metadata: ActionMetadata,
}

impl ExampleAction {
    fn new(name: &str, scope: &str, impact: ActionImpact) -> Self {
        Self {
            name: name.to_string(),
            scope: ScopeId::new(scope),
            impact,
            metadata: ActionMetadata::new("ExampleAction", name, "example"),
        }
    }
}

/// Execution context for actions
struct ExampleContext;

impl Action for ExampleAction {
    type Output = String;
    type Error = ExampleError;

    fn scope(&self) -> &ScopeId {
        &self.scope
    }

    fn impact(&self) -> ActionImpact {
        self.impact
    }

    fn metadata(&self) -> &ActionMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _ctx: &prime_radiant::execution::ExecutionContext,
    ) -> Result<Self::Output, Self::Error> {
        Ok(format!("Executed action: {}", self.name))
    }

    fn content_hash(&self) -> [u8; 32] {
        let mut hash = [0u8; 32];
        let name_bytes = self.name.as_bytes();
        for (i, &b) in name_bytes.iter().enumerate().take(32) {
            hash[i] = b;
        }
        hash
    }

    fn make_rollback_not_supported_error() -> Self::Error {
        ExampleError("Rollback not supported".to_string())
    }
}

fn create_test_gate() -> CoherenceGate {
    let policy_ref = PolicyBundleRef::placeholder();
    CoherenceGate::with_defaults(policy_ref)
}

fn run_reflex_example() {
    let mut gate = create_test_gate();

    // Create an action
    let action = ExampleAction::new("simple_query", "knowledge/facts", ActionImpact::low());

    // Create a LOW energy snapshot
    // Low energy = system is coherent = fast reflex processing
    let energy_snapshot = EnergySnapshot::new(
        0.1,  // total_energy: Very low (coherent)
        0.05, // scope_energy: Also very low
        ScopeId::new("knowledge/facts"),
    );

    println!("Action: {}", action.name);
    println!("Energy Snapshot:");
    println!("  Total energy: {:.2}", energy_snapshot.total_energy);
    println!("  Scope energy: {:.2}", energy_snapshot.scope_energy);
    println!();

    // Evaluate with the gate
    let (decision, witness) = gate.evaluate_with_witness(&action, &energy_snapshot);

    println!("Gate Decision:");
    println!("  Allowed: {}", decision.allow);
    println!(
        "  Compute Lane: {:?} ({})",
        decision.lane,
        lane_description(decision.lane)
    );
    if let Some(reason) = &decision.reason {
        println!("  Reason: {}", reason);
    }
    println!();
    println!("Witness Record:");
    println!("  ID: {}", witness.id);
    println!("  Integrity verified: {}", witness.verify_integrity());
    println!();

    explain_decision(decision.lane);
}

fn run_retrieval_example() {
    let mut gate = create_test_gate();

    let action = ExampleAction::new(
        "complex_query",
        "reasoning/inference",
        ActionImpact::medium(),
    );

    // Create MEDIUM energy snapshot
    // Moderate energy = some inconsistency = needs evidence retrieval
    let energy_snapshot = EnergySnapshot::new(
        0.45, // total_energy: Medium
        0.35, // scope_energy: Medium (above reflex threshold)
        ScopeId::new("reasoning/inference"),
    );

    println!("Action: {}", action.name);
    println!("Energy Snapshot:");
    println!("  Total energy: {:.2}", energy_snapshot.total_energy);
    println!("  Scope energy: {:.2}", energy_snapshot.scope_energy);
    println!();

    let (decision, _) = gate.evaluate_with_witness(&action, &energy_snapshot);

    println!("Gate Decision:");
    println!("  Allowed: {}", decision.allow);
    println!(
        "  Compute Lane: {:?} ({})",
        decision.lane,
        lane_description(decision.lane)
    );
    if let Some(reason) = &decision.reason {
        println!("  Reason: {}", reason);
    }
    println!();

    explain_decision(decision.lane);
}

fn run_heavy_example() {
    let mut gate = create_test_gate();

    let action = ExampleAction::new(
        "multi_step_planning",
        "planning/complex",
        ActionImpact::high(),
    );

    // Create HIGH energy snapshot
    // High energy = significant inconsistency = needs heavy computation
    let energy_snapshot = EnergySnapshot::new(
        0.75, // total_energy: High
        0.65, // scope_energy: High (above retrieval threshold)
        ScopeId::new("planning/complex"),
    );

    println!("Action: {}", action.name);
    println!("Energy Snapshot:");
    println!("  Total energy: {:.2}", energy_snapshot.total_energy);
    println!("  Scope energy: {:.2}", energy_snapshot.scope_energy);
    println!();

    let (decision, _) = gate.evaluate_with_witness(&action, &energy_snapshot);

    println!("Gate Decision:");
    println!("  Allowed: {}", decision.allow);
    println!(
        "  Compute Lane: {:?} ({})",
        decision.lane,
        lane_description(decision.lane)
    );
    if let Some(reason) = &decision.reason {
        println!("  Reason: {}", reason);
    }
    println!();

    explain_decision(decision.lane);
}

fn run_human_escalation_example() {
    let mut gate = create_test_gate();

    let action = ExampleAction::new(
        "critical_decision",
        "safety/critical",
        ActionImpact::critical(),
    );

    // Create VERY HIGH energy snapshot
    // Very high energy = sustained incoherence = requires human intervention
    let energy_snapshot = EnergySnapshot::new(
        0.95, // total_energy: Very high (near 1.0)
        0.92, // scope_energy: Very high (above heavy threshold)
        ScopeId::new("safety/critical"),
    );

    println!("Action: {}", action.name);
    println!("Energy Snapshot:");
    println!("  Total energy: {:.2}", energy_snapshot.total_energy);
    println!("  Scope energy: {:.2}", energy_snapshot.scope_energy);
    println!();

    let (decision, _) = gate.evaluate_with_witness(&action, &energy_snapshot);

    println!("Gate Decision:");
    println!("  Allowed: {}", decision.allow);
    println!(
        "  Compute Lane: {:?} ({})",
        decision.lane,
        lane_description(decision.lane)
    );
    if let Some(reason) = &decision.reason {
        println!("  Reason: {}", reason);
    }
    println!();

    explain_decision(decision.lane);

    if decision.lane == ComputeLane::Human {
        println!();
        println!("  >> HUMAN ESCALATION TRIGGERED <<");
        println!("  The system has detected sustained incoherence that");
        println!("  requires human review before proceeding.");
    }
}

fn run_custom_thresholds_example() {
    // Create a gate with custom thresholds
    let policy_ref = PolicyBundleRef::placeholder();

    // Use custom thresholds: more lenient for reflex, stricter for escalation
    let custom_thresholds = LaneThresholds::new(0.4, 0.7, 0.9);

    let mut gate = CoherenceGate::new(custom_thresholds, Duration::from_secs(10), policy_ref);

    println!("Custom Threshold Configuration:");
    println!("  Reflex threshold: 0.40 (more lenient)");
    println!("  Retrieval threshold: 0.70");
    println!("  Heavy threshold: 0.90");
    println!();

    // Test with energy that would trigger retrieval with default thresholds
    // but stays in reflex with custom thresholds
    let action = ExampleAction::new("test_action", "test/scope", ActionImpact::medium());

    let energy_snapshot = EnergySnapshot::new(0.35, 0.35, ScopeId::new("test/scope"));

    let (decision, _) = gate.evaluate_with_witness(&action, &energy_snapshot);

    println!("With energy 0.35:");
    println!("  Default thresholds (reflex=0.3) would route to: Retrieval");
    println!(
        "  Custom thresholds (reflex=0.4) route to: {:?} ({})",
        decision.lane,
        lane_description(decision.lane)
    );
    println!();
    println!("Custom thresholds allow you to:");
    println!("  - Tune sensitivity based on domain requirements");
    println!("  - Make critical scopes more conservative");
    println!("  - Allow more autonomy in low-risk areas");
}

fn lane_description(lane: ComputeLane) -> &'static str {
    match lane {
        ComputeLane::Reflex => "instant processing, <1ms",
        ComputeLane::Retrieval => "evidence fetching, ~10ms",
        ComputeLane::Heavy => "multi-step reasoning, ~100ms",
        ComputeLane::Human => "human escalation",
    }
}

fn explain_decision(lane: ComputeLane) {
    println!("Lane Explanation:");
    match lane {
        ComputeLane::Reflex => {
            println!("  The system is highly coherent (low energy).");
            println!("  Action can proceed with minimal computation.");
            println!("  Typical use: Simple queries, cached responses, routine actions.");
        }
        ComputeLane::Retrieval => {
            println!("  The system shows some uncertainty (medium energy).");
            println!("  Additional evidence retrieval is recommended.");
            println!("  Typical use: Questions needing context lookup, clarification.");
        }
        ComputeLane::Heavy => {
            println!("  The system shows significant inconsistency (high energy).");
            println!("  Multi-step reasoning or spectral analysis is needed.");
            println!("  Typical use: Complex planning, conflict resolution, deep analysis.");
        }
        ComputeLane::Human => {
            println!("  The system shows sustained incoherence (very high energy).");
            println!("  Human intervention is required before proceeding.");
            println!("  Typical use: Safety-critical decisions, policy violations, edge cases.");
        }
    }
}
