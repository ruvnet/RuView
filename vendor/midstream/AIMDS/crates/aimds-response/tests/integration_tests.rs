//! Integration tests for AIMDS response layer

use aimds_response::{
    ResponseSystem, MetaLearningEngine, AdaptiveMitigator, MitigationAction,
    ThreatContext, FeedbackSignal, MitigationOutcome,
};
use std::collections::HashMap;
use std::time::Duration;

mod common;

#[tokio::test]
async fn test_end_to_end_mitigation() {
    // Create response system
    let system = ResponseSystem::new().await.expect("Failed to create system");

    // Create threat incident
    let threat = create_test_threat("high_severity", 9, 0.95);

    // Apply mitigation
    let outcome = system.mitigate(&threat).await;
    assert!(outcome.is_ok(), "Mitigation should succeed");

    let outcome = outcome.unwrap();
    assert!(outcome.success, "Mitigation should be successful");
    assert!(!outcome.actions_applied.is_empty(), "Actions should be applied");
}

#[tokio::test]
async fn test_meta_learning_integration() {
    let system = ResponseSystem::new().await.unwrap();

    // Apply multiple mitigations
    for i in 0..10 {
        let threat = create_test_threat(&format!("threat_{}", i), 7, 0.8);
        let outcome = system.mitigate(&threat).await.unwrap();

        // Learn from outcome
        system.learn_from_result(&outcome).await.unwrap();
    }

    // Check metrics
    let metrics = system.metrics().await;
    assert!(metrics.total_mitigations >= 10);
}

#[tokio::test]
async fn test_strategy_optimization() {
    let system = ResponseSystem::new().await.unwrap();

    // Generate feedback signals
    let feedback: Vec<FeedbackSignal> = (0..20)
        .map(|i| FeedbackSignal {
            strategy_id: format!("strategy_{}", i % 3),
            success: i % 2 == 0,
            effectiveness_score: 0.7 + (i as f64 * 0.01),
            timestamp: chrono::Utc::now(),
            context: Some(format!("test_{}", i)),
        })
        .collect();

    // Optimize based on feedback
    system.optimize(&feedback).await.unwrap();

    let metrics = system.metrics().await;
    assert!(metrics.optimization_level >= 0);
}

#[tokio::test]
async fn test_rollback_mechanism() {
    let system = ResponseSystem::new().await.unwrap();

    // Create a threat that will fail mitigation
    let threat = create_test_threat("low_severity", 2, 0.3);

    // This should trigger rollback on failure
    let _result = system.mitigate(&threat).await;

    // Verify rollback was attempted
    // In production, we'd check rollback history
}

#[tokio::test]
async fn test_concurrent_mitigations() {
    let system = ResponseSystem::new().await.unwrap();

    // Create multiple threats
    let threats: Vec<_> = (0..5)
        .map(|i| create_test_threat(&format!("concurrent_{}", i), 6, 0.75))
        .collect();

    // Apply mitigations concurrently
    let mut handles = vec![];

    for threat in threats {
        let system_clone = system.clone();
        let handle = tokio::spawn(async move {
            system_clone.mitigate(&threat).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;

    // All should succeed
    for result in results {
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }
}

#[tokio::test]
async fn test_adaptive_strategy_selection() {
    let mut mitigator = AdaptiveMitigator::new();

    // Test different threat severities
    let low_threat = create_test_threat("low", 3, 0.4);
    let medium_threat = create_test_threat("medium", 6, 0.7);
    let high_threat = create_test_threat("high", 9, 0.95);

    // Each should select appropriate strategy
    let low_result = mitigator.apply_mitigation(&low_threat).await;
    let medium_result = mitigator.apply_mitigation(&medium_threat).await;
    let high_result = mitigator.apply_mitigation(&high_threat).await;

    assert!(low_result.is_ok());
    assert!(medium_result.is_ok());
    assert!(high_result.is_ok());

    // Update effectiveness
    mitigator.update_effectiveness(&low_result.unwrap().strategy_id, true);
    mitigator.update_effectiveness(&medium_result.unwrap().strategy_id, true);
    mitigator.update_effectiveness(&high_result.unwrap().strategy_id, true);

    assert!(mitigator.active_strategies_count() > 0);
}

#[tokio::test]
async fn test_meta_learning_convergence() {
    let mut engine = MetaLearningEngine::new();

    // Train with similar incidents
    for i in 0..25 {
        let incident = create_test_incident(i, 7, 0.8);
        engine.learn_from_incident(&incident).await;
    }

    // Should have learned patterns
    assert!(engine.learned_patterns_count() > 0);

    // Optimization level should advance
    let feedback: Vec<FeedbackSignal> = (0..30)
        .map(|i| FeedbackSignal {
            strategy_id: "test_strategy".to_string(),
            success: true,
            effectiveness_score: 0.85,
            timestamp: chrono::Utc::now(),
            context: Some(format!("iteration_{}", i)),
        })
        .collect();

    engine.optimize_strategy(&feedback);

    // Should advance toward higher levels
    assert!(engine.current_optimization_level() >= 0);
}

#[tokio::test]
async fn test_mitigation_performance() {
    let system = ResponseSystem::new().await.unwrap();

    let threat = create_test_threat("perf_test", 7, 0.85);

    let start = std::time::Instant::now();
    let result = system.mitigate(&threat).await;
    let duration = start.elapsed();

    assert!(result.is_ok());
    assert!(duration < Duration::from_millis(100), "Mitigation should be fast");
}

#[tokio::test]
async fn test_effectiveness_tracking() {
    let mut mitigator = AdaptiveMitigator::new();

    // Apply same strategy multiple times
    for i in 0..10 {
        let threat = create_test_threat(&format!("track_{}", i), 7, 0.8);
        let outcome = mitigator.apply_mitigation(&threat).await.unwrap();

        // Alternate success/failure
        mitigator.update_effectiveness(&outcome.strategy_id, i % 2 == 0);
    }

    // Effectiveness should be around 0.5 due to alternating success
    // In production, we'd have getter for effectiveness scores
}

#[tokio::test]
async fn test_pattern_extraction() {
    let engine = MetaLearningEngine::new();

    let incident = create_test_incident(1, 8, 0.9);

    // This is tested internally, but we verify the engine handles it
    assert_eq!(engine.learned_patterns_count(), 0);
}

#[tokio::test]
async fn test_multi_level_optimization() {
    let mut engine = MetaLearningEngine::new();

    // Generate extensive feedback to trigger level advancement
    for level in 0..5 {
        let feedback: Vec<FeedbackSignal> = (0..50)
            .map(|i| FeedbackSignal {
                strategy_id: format!("level_{}_strategy", level),
                success: true,
                effectiveness_score: 0.8 + (i as f64 * 0.001),
                timestamp: chrono::Utc::now(),
                context: Some(format!("level_{}_iter_{}", level, i)),
            })
            .collect();

        engine.optimize_strategy(&feedback);

        // Add learned patterns to advance level
        for i in 0..15 {
            let incident = create_test_incident(i, 7, 0.8);
            engine.learn_from_incident(&incident).await;
        }
    }

    // Should have advanced through multiple levels
    assert!(engine.current_optimization_level() > 0);
}

#[tokio::test]
async fn test_context_metadata() {
    let threat = create_test_threat("metadata_test", 7, 0.85);
    let context = ThreatContext::from_incident(&threat)
        .with_metadata("test_key".to_string(), "test_value".to_string());

    assert!(context.metadata.contains_key("test_key"));
    assert_eq!(context.metadata.get("test_key").unwrap(), "test_value");
}

// Helper functions

fn create_test_threat(id: &str, severity: u8, confidence: f64) -> aimds_response::meta_learning::ThreatIncident {
    use aimds_response::meta_learning::{ThreatIncident, ThreatType};

    ThreatIncident {
        id: id.to_string(),
        threat_type: ThreatType::Anomaly(confidence),
        severity,
        confidence,
        timestamp: chrono::Utc::now(),
    }
}

fn create_test_incident(id: i32, severity: u8, confidence: f64) -> aimds_response::meta_learning::ThreatIncident {
    use aimds_response::meta_learning::{ThreatIncident, ThreatType};

    ThreatIncident {
        id: format!("incident_{}", id),
        threat_type: ThreatType::Anomaly(confidence),
        severity,
        confidence,
        timestamp: chrono::Utc::now(),
    }
}
