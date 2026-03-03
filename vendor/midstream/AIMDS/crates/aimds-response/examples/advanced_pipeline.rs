//! Advanced mitigation pipeline example

use aimds_response::{
    AdaptiveMitigator, AuditLogger, FeedbackSignal, MetaLearningEngine, ResponseSystem,
    RollbackManager,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== AIMDS Response Layer - Advanced Pipeline ===\n");

    // Create components
    let system = ResponseSystem::new().await?;
    let mut meta_learner = MetaLearningEngine::new();
    let audit_logger = AuditLogger::new();
    let rollback_mgr = RollbackManager::new();

    // Simulate multiple threat scenarios
    let threats = create_threat_scenarios();

    println!("Processing {} threat scenarios...\n", threats.len());

    for (i, threat) in threats.iter().enumerate() {
        println!("--- Scenario {} ---", i + 1);
        println!("Threat ID: {}", threat.id);
        println!("Severity: {}", threat.severity);
        println!("Confidence: {:.2}", threat.confidence);

        // Apply mitigation
        let outcome = system.mitigate(threat).await?;

        println!("✓ Mitigation applied: {}", outcome.strategy_id);
        println!("  Actions: {:?}", outcome.actions_applied);

        // Learn from outcome
        meta_learner.learn_from_incident(threat).await;

        // Create feedback
        let feedback = FeedbackSignal {
            strategy_id: outcome.strategy_id.clone(),
            success: outcome.success,
            effectiveness_score: outcome.effectiveness_score(),
            timestamp: chrono::Utc::now(),
            context: Some(format!("scenario_{}", i + 1)),
        };

        // Optimize based on feedback
        meta_learner.optimize_strategy(&[feedback]);

        println!(
            "  Optimization level: {}\n",
            meta_learner.current_optimization_level()
        );

        // Small delay between scenarios
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Display final statistics
    println!("\n=== Final Statistics ===");

    let metrics = system.metrics().await;
    println!("Total mitigations: {}", metrics.total_mitigations);
    println!("Successful: {}", metrics.successful_mitigations);
    println!("Learned patterns: {}", metrics.learned_patterns);
    println!("Active strategies: {}", metrics.active_strategies);
    println!(
        "Optimization level: {}/25",
        metrics.optimization_level
    );

    let audit_stats = audit_logger.statistics().await;
    println!("\n=== Audit Statistics ===");
    println!("Total mitigations: {}", audit_stats.total_mitigations);
    println!("Success rate: {:.2}%", audit_stats.success_rate() * 100.0);
    println!("Total actions: {}", audit_stats.total_actions_applied);

    println!("\n✓ Advanced pipeline completed!");

    Ok(())
}

fn create_threat_scenarios() -> Vec<aimds_response::meta_learning::ThreatIncident> {
    use aimds_response::meta_learning::{AttackType, ThreatIncident, ThreatType};

    vec![
        ThreatIncident {
            id: "threat-001".to_string(),
            threat_type: ThreatType::Attack(AttackType::SqlInjection),
            severity: 9,
            confidence: 0.95,
            timestamp: chrono::Utc::now(),
        },
        ThreatIncident {
            id: "threat-002".to_string(),
            threat_type: ThreatType::Attack(AttackType::XSS),
            severity: 7,
            confidence: 0.88,
            timestamp: chrono::Utc::now(),
        },
        ThreatIncident {
            id: "threat-003".to_string(),
            threat_type: ThreatType::Anomaly(0.92),
            severity: 6,
            confidence: 0.85,
            timestamp: chrono::Utc::now(),
        },
        ThreatIncident {
            id: "threat-004".to_string(),
            threat_type: ThreatType::Attack(AttackType::DDoS),
            severity: 10,
            confidence: 0.98,
            timestamp: chrono::Utc::now(),
        },
        ThreatIncident {
            id: "threat-005".to_string(),
            threat_type: ThreatType::Intrusion(8),
            severity: 8,
            confidence: 0.91,
            timestamp: chrono::Utc::now(),
        },
    ]
}
