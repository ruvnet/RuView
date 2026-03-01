//! Receipt Audit Trail Example
//!
//! This example demonstrates:
//! - Generating multiple decisions
//! - Accessing the receipt log
//! - Verifying hash chain integrity
//!
//! Run with: cargo run --example receipt_audit

use cognitum_gate_tilezero::{
    ActionContext, ActionMetadata, ActionTarget, GateThresholds, TileZero,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cognitum Coherence Gate - Receipt Audit Example ===\n");

    let tilezero = TileZero::new(GateThresholds::default());

    // Generate several decisions
    let actions = vec![
        ("action-001", "config_read", "agent-1", "router-1"),
        ("action-002", "config_write", "agent-1", "router-1"),
        ("action-003", "restart", "agent-2", "service-a"),
        ("action-004", "deploy", "agent-3", "cluster-prod"),
        ("action-005", "rollback", "agent-3", "cluster-prod"),
    ];

    println!("Generating decisions...\n");

    for (id, action_type, agent, target) in &actions {
        let action = ActionContext {
            action_id: id.to_string(),
            action_type: action_type.to_string(),
            target: ActionTarget {
                device: Some(target.to_string()),
                path: None,
                extra: HashMap::new(),
            },
            context: ActionMetadata {
                agent_id: agent.to_string(),
                session_id: None,
                prior_actions: vec![],
                urgency: "normal".to_string(),
            },
        };

        let token = tilezero.decide(&action).await;
        println!("  {} -> {:?}", id, token.decision);
    }

    println!("\n--- Audit Trail ---\n");

    // Verify the hash chain
    match tilezero.verify_receipt_chain().await {
        Ok(()) => println!("Hash chain: VERIFIED"),
        Err(e) => println!("Hash chain: BROKEN - {:?}", e),
    }

    // Display receipt summary
    println!("\nReceipts:");
    println!("{:-<60}", "");
    println!(
        "{:<10} {:<15} {:<12} {:<20}",
        "Seq", "Action", "Decision", "Hash (first 8)"
    );
    println!("{:-<60}", "");

    for seq in 0..actions.len() as u64 {
        if let Some(receipt) = tilezero.get_receipt(seq).await {
            let hash = receipt.hash();
            let hash_hex = hex::encode(&hash[..4]);
            println!(
                "{:<10} {:<15} {:<12} {}...",
                receipt.sequence,
                receipt.token.action_id,
                format!("{:?}", receipt.token.decision),
                hash_hex
            );
        }
    }

    println!("{:-<60}", "");

    // Export for compliance
    println!("\nExporting audit log...");

    let audit_json = tilezero.export_receipts_json().await?;
    let filename = format!(
        "audit_log_{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    println!("  Would write {} bytes to {}", audit_json.len(), filename);
    println!("  (Skipping actual file write in example)");

    println!("\n=== Example Complete ===");
    Ok(())
}
