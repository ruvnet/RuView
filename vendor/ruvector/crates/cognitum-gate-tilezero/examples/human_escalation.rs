//! Human Escalation Example
//!
//! This example demonstrates the hybrid agent/human workflow:
//! - Detecting when human review is needed (DEFER)
//! - Presenting the escalation context
//!
//! Run with: cargo run --example human_escalation

use cognitum_gate_tilezero::{
    ActionContext, ActionMetadata, ActionTarget, GateDecision, GateThresholds, TileZero,
};
use std::collections::HashMap;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cognitum Coherence Gate - Human Escalation Example ===\n");

    // Create TileZero with conservative thresholds to trigger DEFER
    let thresholds = GateThresholds {
        min_cut: 15.0,  // Higher threshold
        max_shift: 0.3, // Lower tolerance for shift
        tau_deny: 0.01,
        tau_permit: 100.0,
        permit_ttl_ns: 300_000_000_000, // 5 minutes
        theta_uncertainty: 10.0,
        theta_confidence: 3.0,
    };
    let tilezero = TileZero::new(thresholds);

    // Simulate a risky action
    let action = ActionContext {
        action_id: "critical-update-042".to_string(),
        action_type: "database_migration".to_string(),
        target: ActionTarget {
            device: Some("production-db-primary".to_string()),
            path: Some("/data/schema".to_string()),
            extra: HashMap::new(),
        },
        context: ActionMetadata {
            agent_id: "migration-agent".to_string(),
            session_id: Some("migration-session".to_string()),
            prior_actions: vec![],
            urgency: "high".to_string(),
        },
    };

    println!("Evaluating high-risk action:");
    println!("  Type: {}", action.action_type);
    println!("  Target: {:?}", action.target.device);
    println!();

    // Evaluate - this may trigger DEFER due to conservative thresholds
    let token = tilezero.decide(&action).await;

    if token.decision == GateDecision::Defer {
        println!("Decision: DEFER - Human review required\n");

        // Display escalation context
        println!("┌─────────────────────────────────────────────────────┐");
        println!("│  HUMAN DECISION REQUIRED                           │");
        println!("├─────────────────────────────────────────────────────┤");
        println!("│  Action: {}              │", action.action_id);
        println!("│  Target: {:?}                 │", action.target.device);
        println!("│                                                     │");
        println!("│  Why deferred:                                      │");
        println!("│  • High-risk target (production database)           │");
        println!("│  • Action type: database_migration                  │");
        println!("│                                                     │");
        println!("│  Options:                                           │");
        println!("│  [1] APPROVE - Allow the action                     │");
        println!("│  [2] DENY   - Block the action                      │");
        println!("│  [3] ESCALATE - Need more review                    │");
        println!("└─────────────────────────────────────────────────────┘");
        println!();

        // Get human input
        print!("Enter your decision (1/2/3): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => {
                println!("\nYou chose: APPROVE");
                println!("In production, this would:");
                println!("  - Record the approval with your identity");
                println!("  - Generate a new PERMIT token");
                println!("  - Log the decision to the audit trail");
            }
            "2" => {
                println!("\nYou chose: DENY");
                println!("In production, this would:");
                println!("  - Record the denial with your identity");
                println!("  - Block the action permanently");
                println!("  - Alert the requesting agent");
            }
            _ => {
                println!("\nYou chose: ESCALATE");
                println!("In production, this would:");
                println!("  - Forward to Tier 3 (policy team)");
                println!("  - Extend the timeout");
                println!("  - Provide additional context");
            }
        }
    } else {
        println!("Decision: {:?}", token.decision);
        println!("(Automatic - no human review needed)");
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
