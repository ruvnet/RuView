//! Basic usage example for aimds-response

use aimds_response::{ResponseSystem, FeedbackSignal};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== AIMDS Response Layer - Basic Usage ===\n");

    // Create response system
    println!("Creating response system...");
    let response_system = ResponseSystem::new().await?;

    // Simulate threat detection
    println!("Detecting threat...");
    let threat = create_sample_threat();

    // Apply mitigation
    println!("Applying mitigation...");
    let outcome = response_system.mitigate(&threat).await?;

    println!("✓ Mitigation applied successfully!");
    println!("  Strategy: {}", outcome.strategy_id);
    println!("  Actions: {}", outcome.actions_applied.len());
    println!("  Duration: {:?}", outcome.duration);
    println!("  Success: {}", outcome.success);

    // Learn from outcome
    println!("\nLearning from outcome...");
    response_system.learn_from_result(&outcome).await?;

    // Generate feedback
    let feedback = vec![FeedbackSignal {
        strategy_id: outcome.strategy_id.clone(),
        success: outcome.success,
        effectiveness_score: outcome.effectiveness_score(),
        timestamp: chrono::Utc::now(),
        context: Some("basic_usage_example".to_string()),
    }];

    // Optimize strategies
    println!("Optimizing strategies...");
    response_system.optimize(&feedback).await?;

    // Display metrics
    let metrics = response_system.metrics().await;
    println!("\n=== System Metrics ===");
    println!("Learned patterns: {}", metrics.learned_patterns);
    println!("Active strategies: {}", metrics.active_strategies);
    println!("Total mitigations: {}", metrics.total_mitigations);
    println!("Successful mitigations: {}", metrics.successful_mitigations);
    println!("Optimization level: {}", metrics.optimization_level);

    if metrics.total_mitigations > 0 {
        let success_rate =
            metrics.successful_mitigations as f64 / metrics.total_mitigations as f64 * 100.0;
        println!("Success rate: {:.2}%", success_rate);
    }

    println!("\n✓ Example completed successfully!");

    Ok(())
}

fn create_sample_threat() -> aimds_response::meta_learning::ThreatIncident {
    use aimds_response::meta_learning::{AttackType, ThreatIncident, ThreatType};

    ThreatIncident {
        id: "example-threat-001".to_string(),
        threat_type: ThreatType::Attack(AttackType::SqlInjection),
        severity: 8,
        confidence: 0.92,
        timestamp: chrono::Utc::now(),
    }
}
