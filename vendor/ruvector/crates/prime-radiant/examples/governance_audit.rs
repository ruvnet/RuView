//! Governance and Audit Trail Example
//!
//! This example demonstrates Prime-Radiant's governance features:
//! - Creating policy bundles with rules and scopes
//! - Generating witness records for audit trails
//! - Verifying witness integrity
//! - Policy lifecycle management
//!
//! Run with: `cargo run --example governance_audit`

use prime_radiant::execution::{
    Action, ActionImpact, ActionMetadata, CoherenceGate, EnergySnapshot, LaneThresholds,
    PolicyBundleRef as ExecutionPolicyRef, ScopeId, WitnessRecord,
};
use prime_radiant::governance::{
    ApprovalSignature, ApproverId, EscalationCondition, EscalationRule, Hash, PolicyBundle,
    PolicyBundleBuilder, PolicyBundleRef, PolicyBundleStatus, PolicyError, ThresholdConfig,
    Timestamp, Version,
};
use std::time::Duration;

fn main() {
    println!("=== Prime-Radiant: Governance & Audit Trail Example ===\n");

    // Example 1: Create a policy bundle
    println!("--- Example 1: Policy Bundle Creation ---");
    let policy_bundle = run_policy_bundle_example();

    println!();

    // Example 2: Policy lifecycle
    println!("--- Example 2: Policy Lifecycle Management ---");
    run_policy_lifecycle_example();

    println!();

    // Example 3: Generate witness records
    println!("--- Example 3: Witness Record Generation ---");
    run_witness_generation_example();

    println!();

    // Example 4: Verify witness chain integrity
    println!("--- Example 4: Witness Chain Integrity ---");
    run_chain_verification_example();

    println!();

    // Example 5: Tamper detection
    println!("--- Example 5: Tamper Detection ---");
    run_tamper_detection_example();
}

fn run_policy_bundle_example() -> PolicyBundle {
    println!("Creating a policy bundle for LLM governance...");
    println!();

    // Create the policy bundle using the builder
    let policy = PolicyBundleBuilder::new()
        .name("llm-safety-policy")
        .description("Safety policies for LLM deployments")
        .with_threshold("default", ThresholdConfig::default())
        .with_threshold("safety", ThresholdConfig::strict())
        .with_threshold("quality", ThresholdConfig::new(0.4, 0.7, 0.9))
        .with_escalation_rule(EscalationRule::new(
            "high-energy-escalation",
            EscalationCondition::EnergyAbove(0.8),
            3, // Human lane
        ))
        .with_escalation_rule(
            EscalationRule::new(
                "persistent-incoherence",
                EscalationCondition::PersistentEnergy {
                    threshold: 0.5,
                    duration_secs: 30,
                },
                2, // Heavy lane
            )
            .with_notify("ops-team"),
        )
        .with_required_approvals(2)
        .with_approver(ApproverId::new("admin@example.com"))
        .with_approver(ApproverId::new("security@example.com"))
        .build()
        .expect("Failed to build policy");

    println!("Policy Bundle Created:");
    println!("  ID: {}", policy.id);
    println!("  Name: {}", policy.name);
    println!("  Version: {}", policy.version);
    println!("  Status: {:?}", policy.status);
    println!("  Required approvals: {}", policy.required_approvals);
    println!();
    println!("Threshold Configurations:");
    for (scope, config) in &policy.thresholds {
        println!(
            "  {}: reflex={:.2}, retrieval={:.2}, heavy={:.2}",
            scope, config.reflex, config.retrieval, config.heavy
        );
    }
    println!();
    println!("Escalation Rules:");
    for rule in &policy.escalation_rules {
        println!("  - {} -> lane {}", rule.name, rule.target_lane);
    }

    policy
}

fn run_policy_lifecycle_example() {
    println!("Demonstrating policy lifecycle transitions...");
    println!();

    // Create a new policy
    let mut policy = PolicyBundle::new("lifecycle-demo");
    println!("1. Created policy in {:?} status", policy.status);
    println!("   Editable: {}", policy.status.is_editable());

    // Add configuration while in draft
    policy
        .add_threshold("default", ThresholdConfig::default())
        .expect("Failed to add threshold");
    policy
        .set_required_approvals(2)
        .expect("Failed to set approvals");
    println!("2. Added configuration (still in Draft)");

    // Submit for approval
    policy.submit_for_approval().expect("Failed to submit");
    println!("3. Submitted for approval -> {:?}", policy.status);

    // Add first approval
    let approval1 = ApprovalSignature::placeholder(ApproverId::new("approver1"));
    policy
        .add_approval(approval1)
        .expect("Failed to add approval");
    println!(
        "4. Added first approval -> {:?} (approvals: {}/{})",
        policy.status,
        policy.approvals.len(),
        policy.required_approvals
    );

    // Add second approval (triggers activation)
    let approval2 = ApprovalSignature::placeholder(ApproverId::new("approver2"));
    policy
        .add_approval(approval2)
        .expect("Failed to add approval");
    println!("5. Added second approval -> {:?}", policy.status);
    println!(
        "   Activated at: {:?}",
        policy.activated_at.map(|t| t.to_string())
    );

    // Try to modify (should fail)
    let result = policy.add_threshold("new-scope", ThresholdConfig::strict());
    println!("6. Attempted modification: {:?}", result.err());

    // Create new version
    let new_version = policy.create_new_version();
    println!("7. Created new version:");
    println!("   New ID: {}", new_version.id);
    println!("   New version: {}", new_version.version);
    println!("   Supersedes: {:?}", new_version.supersedes);
    println!("   Status: {:?}", new_version.status);
}

/// Simple error type for audit actions
#[derive(Debug)]
struct AuditError(String);

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AuditError {}

/// Example action for witness generation
struct AuditAction {
    name: String,
    scope: ScopeId,
    metadata: ActionMetadata,
}

impl AuditAction {
    fn new(name: &str, scope: &str) -> Self {
        Self {
            name: name.to_string(),
            scope: ScopeId::new(scope),
            metadata: ActionMetadata::new("AuditAction", name, "audit-example"),
        }
    }
}

impl Action for AuditAction {
    type Output = String;
    type Error = AuditError;

    fn scope(&self) -> &ScopeId {
        &self.scope
    }

    fn impact(&self) -> ActionImpact {
        ActionImpact::medium()
    }

    fn metadata(&self) -> &ActionMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _ctx: &prime_radiant::execution::ExecutionContext,
    ) -> Result<Self::Output, Self::Error> {
        Ok(format!("Executed: {}", self.name))
    }

    fn content_hash(&self) -> [u8; 32] {
        let hash = blake3::hash(self.name.as_bytes());
        let mut result = [0u8; 32];
        result.copy_from_slice(hash.as_bytes());
        result
    }

    fn make_rollback_not_supported_error() -> Self::Error {
        AuditError("Rollback not supported".to_string())
    }
}

fn run_witness_generation_example() {
    println!("Generating witness records for gate decisions...");
    println!();

    let policy_ref = ExecutionPolicyRef::placeholder();
    let mut gate = CoherenceGate::with_defaults(policy_ref);

    // Simulate several gate decisions
    let scenarios = [
        ("Query about Rust programming", "knowledge", 0.15),
        ("Complex code generation", "generation", 0.45),
        ("Ambiguous safety question", "safety", 0.72),
        ("Potentially harmful request", "safety/critical", 0.92),
        ("Follow-up clarification", "chat", 0.25),
    ];

    for (i, (description, scope, energy)) in scenarios.iter().enumerate() {
        let action = AuditAction::new(description, scope);
        let energy_snapshot = EnergySnapshot::new(*energy, *energy, ScopeId::new(*scope));

        let (decision, witness) = gate.evaluate_with_witness(&action, &energy_snapshot);

        println!("Decision #{}: {}", i + 1, description);
        println!("  Allowed: {}", decision.allow);
        println!("  Lane: {:?}", decision.lane);
        println!("  Energy: {:.2}", energy);
        println!("  Witness ID: {}", witness.id);
        println!(
            "  Previous witness: {}",
            witness
                .previous_witness
                .as_ref()
                .map(|w| w.to_string())
                .unwrap_or_else(|| "None (genesis)".to_string())
        );
        println!();
    }
}

fn run_chain_verification_example() {
    println!("Verifying witness chain integrity...");
    println!();

    let policy_ref = ExecutionPolicyRef::placeholder();
    let mut gate = CoherenceGate::with_defaults(policy_ref);

    // Generate a chain of witnesses
    let mut witnesses = Vec::new();

    for i in 0..5 {
        let action = AuditAction::new(&format!("action_{}", i), "test");
        let energy = EnergySnapshot::new(0.2, 0.2, ScopeId::new("test"));
        let (_, witness) = gate.evaluate_with_witness(&action, &energy);
        witnesses.push(witness);
    }

    // Verify each witness's content hash
    println!("Content Hash Verification:");
    for (i, witness) in witnesses.iter().enumerate() {
        let verified = witness.verify_integrity();
        println!(
            "  Witness #{}: {} (ID: {})",
            i + 1,
            if verified { "VALID" } else { "INVALID" },
            witness.id
        );
    }
    println!();

    // Verify chain linkage
    println!("Chain Linkage:");
    println!(
        "  Witness #1: Genesis (previous: {})",
        witnesses[0]
            .previous_witness
            .as_ref()
            .map(|_| "linked")
            .unwrap_or("none")
    );

    for i in 1..witnesses.len() {
        let current = &witnesses[i];
        let previous = &witnesses[i - 1];

        let linked = current
            .previous_witness
            .as_ref()
            .map(|prev| prev == &previous.id)
            .unwrap_or(false);

        println!(
            "  Witness #{}: {} (links to #{})",
            i + 1,
            if linked { "LINKED" } else { "BROKEN" },
            i
        );
    }
}

fn run_tamper_detection_example() {
    println!("Demonstrating tamper detection...");
    println!();

    let policy_ref = ExecutionPolicyRef::placeholder();
    let mut gate = CoherenceGate::with_defaults(policy_ref);

    // Create a witness
    let action = AuditAction::new("test_action", "test");
    let energy = EnergySnapshot::new(0.5, 0.5, ScopeId::new("test"));
    let (_, mut witness) = gate.evaluate_with_witness(&action, &energy);

    // Verify original
    println!("Original Witness:");
    println!("  ID: {}", witness.id);
    println!("  Content hash: {:x?}", &witness.content_hash[..8]);
    println!("  Integrity verified: {}", witness.verify_integrity());
    println!();

    // Tamper with the witness by modifying the decision
    println!("Tampering with witness (changing allowed status)...");
    witness.decision.allow = !witness.decision.allow;

    // Verify after tampering
    println!();
    println!("After Tampering:");
    println!("  Decision.allow changed to: {}", witness.decision.allow);
    println!("  Integrity verified: {}", witness.verify_integrity());
    println!();

    if !witness.verify_integrity() {
        println!("  >> TAMPER DETECTED <<");
        println!("  The witness content has been modified after creation.");
        println!("  This breaks the audit trail integrity.");
        println!();
        println!("  In a production system, this would:");
        println!("  - Trigger security alerts");
        println!("  - Invalidate the entire chain from this point");
        println!("  - Require investigation and remediation");
    }
}
