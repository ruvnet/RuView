//! Basic Coherence Gate Example
//!
//! This example demonstrates:
//! - Creating a TileZero arbiter
//! - Evaluating an action
//! - Verifying the permit token
//!
//! Run with: cargo run --example basic_gate

use cognitum_gate_tilezero::{
    ActionContext, ActionMetadata, ActionTarget, GateDecision, GateThresholds, TileZero,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cognitum Coherence Gate - Basic Example ===\n");

    // Create TileZero with default thresholds
    let thresholds = GateThresholds::default();
    let tilezero = TileZero::new(thresholds);

    println!("TileZero initialized with thresholds:");
    println!("  Min cut: {}", tilezero.thresholds().min_cut);
    println!("  Max shift: {}", tilezero.thresholds().max_shift);
    println!(
        "  Deny threshold (tau_deny): {}",
        tilezero.thresholds().tau_deny
    );
    println!(
        "  Permit threshold (tau_permit): {}",
        tilezero.thresholds().tau_permit
    );
    println!();

    // Create an action context
    let action = ActionContext {
        action_id: "config-push-001".to_string(),
        action_type: "config_change".to_string(),
        target: ActionTarget {
            device: Some("router-west-03".to_string()),
            path: Some("/network/interfaces/eth0".to_string()),
            extra: HashMap::new(),
        },
        context: ActionMetadata {
            agent_id: "ops-agent-12".to_string(),
            session_id: Some("sess-abc123".to_string()),
            prior_actions: vec![],
            urgency: "normal".to_string(),
        },
    };

    println!("Evaluating action:");
    println!("  ID: {}", action.action_id);
    println!("  Type: {}", action.action_type);
    println!("  Agent: {}", action.context.agent_id);
    println!("  Target: {:?}", action.target.device);
    println!();

    // Evaluate the action
    let token = tilezero.decide(&action).await;

    // Display result
    match token.decision {
        GateDecision::Permit => {
            println!("Decision: PERMIT");
            println!("  The action is allowed to proceed.");
        }
        GateDecision::Defer => {
            println!("Decision: DEFER");
            println!("  Human review required.");
        }
        GateDecision::Deny => {
            println!("Decision: DENY");
            println!("  Action blocked due to safety concerns.");
        }
    }

    println!("\nToken details:");
    println!("  Sequence: {}", token.sequence);
    println!("  Valid until: {} ns", token.timestamp + token.ttl_ns);
    println!("  Witness hash: {:02x?}", &token.witness_hash[..8]);

    // Verify the token
    let verifier = tilezero.verifier();
    match verifier.verify(&token) {
        Ok(()) => println!("\nToken signature: VALID"),
        Err(e) => println!("\nToken signature: INVALID - {:?}", e),
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
