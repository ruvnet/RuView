//! Distributed Learning Example
//!
//! Demonstrates distributed Q-learning across multiple agents.

use ruvector_edge::prelude::*;
use ruvector_edge::IntelligenceSync;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("Distributed Learning Example");
    println!("============================\n");

    // Create intelligence sync for aggregated learning
    let sync = Arc::new(RwLock::new(IntelligenceSync::new("swarm-coordinator")));

    // Simulate multiple learning agents with their own experiences
    let scenarios = vec![
        ("learner-001", "edit_code", "coder", 0.9),
        ("learner-001", "review_code", "reviewer", 0.85),
        ("learner-002", "test_code", "tester", 0.88),
        ("learner-002", "debug_error", "debugger", 0.92),
        ("learner-003", "deploy_app", "devops", 0.87),
        ("learner-003", "edit_code", "coder", 0.95),  // Another agent learns edit_code
    ];

    println!("Distributed learning phase:");
    for (agent, state, action, reward) in &scenarios {
        let sync_guard = sync.write().await;
        sync_guard.update_pattern(state, action, *reward);
        println!("  {} learned: {} -> {} ({:.2})", agent, state, action, reward);
    }

    // Query merged intelligence
    let sync_guard = sync.read().await;
    let states_to_query = vec!["edit_code", "review_code", "test_code", "debug_error", "deploy_app"];

    println!("\nMerged intelligence queries:");
    for state in states_to_query {
        if let Some((action, confidence)) = sync_guard.get_best_action(
            state,
            &["coder", "reviewer", "tester", "debugger", "devops"]
                .iter().map(|s| s.to_string()).collect::<Vec<_>>()
        ) {
            println!("  {} -> {} (confidence: {:.1}%)", state, action, confidence * 100.0);
        }
    }

    // Get swarm stats
    let stats = sync_guard.get_swarm_stats();
    println!("\nSwarm statistics:");
    println!("  Total patterns: {}", stats.total_patterns);
    println!("  Total visits: {}", stats.total_visits);
    println!("  Avg confidence: {:.1}%", stats.avg_confidence * 100.0);

    println!("\nDistributed learning example complete!");

    Ok(())
}
