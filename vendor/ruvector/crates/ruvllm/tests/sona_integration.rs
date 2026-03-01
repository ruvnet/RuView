#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Integration tests for SONA (Self-Optimizing Neural Architecture)
//!
//! Tests the three-tier learning loop: instant adaptation, background consolidation,
//! and deep loop processing.

use ruvllm::{
    error::Result,
    sona::{
        LearningLoop, RoutingRecommendation, SonaConfig, SonaIntegration, SonaStats, Trajectory,
    },
};
use std::time::Duration;

/// Create a test SONA configuration
fn create_test_sona_config() -> SonaConfig {
    SonaConfig {
        hidden_dim: 64,
        embedding_dim: 128,
        micro_lora_rank: 2,
        base_lora_rank: 4,
        instant_learning_rate: 0.01,
        background_learning_rate: 0.001,
        ewc_lambda: 0.1,
        pattern_capacity: 100,
        background_interval_secs: 3600,
        deep_interval_secs: 604800,
        quality_threshold: 0.5,
    }
}

/// Create a test trajectory
fn create_test_trajectory(request_id: &str, quality: f32) -> Trajectory {
    Trajectory {
        request_id: request_id.to_string(),
        session_id: "test-session".to_string(),
        query_embedding: vec![0.1; 128],
        response_embedding: vec![0.2; 128],
        quality_score: quality,
        routing_features: vec![0.7, 0.9, 0.5, 0.5],
        model_index: 1,
        timestamp: chrono::Utc::now(),
    }
}

#[test]
fn test_sona_config_default() {
    let config = SonaConfig::default();

    assert_eq!(config.hidden_dim, 256);
    assert_eq!(config.embedding_dim, 768);
    assert_eq!(config.micro_lora_rank, 2);
    assert_eq!(config.base_lora_rank, 8);
    assert!(config.instant_learning_rate > 0.0);
    assert!(config.ewc_lambda > 0.0);
    assert!(config.quality_threshold > 0.0);
}

#[test]
fn test_sona_integration_creation() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 0);
    assert_eq!(stats.instant_updates, 0);
    assert_eq!(stats.background_updates, 0);
    assert_eq!(stats.deep_updates, 0);
}

#[test]
fn test_learning_loop_variants() {
    assert!(matches!(LearningLoop::Instant, LearningLoop::Instant));
    assert!(matches!(LearningLoop::Background, LearningLoop::Background));
    assert!(matches!(LearningLoop::Deep, LearningLoop::Deep));
}

#[test]
fn test_trajectory_creation() {
    let trajectory = create_test_trajectory("req-001", 0.8);

    assert_eq!(trajectory.request_id, "req-001");
    assert_eq!(trajectory.session_id, "test-session");
    assert_eq!(trajectory.quality_score, 0.8);
    assert_eq!(trajectory.query_embedding.len(), 128);
    assert_eq!(trajectory.response_embedding.len(), 128);
    assert_eq!(trajectory.routing_features.len(), 4);
}

#[test]
fn test_sona_record_trajectory() {
    let config = SonaConfig {
        quality_threshold: 0.0, // Accept all trajectories
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    let trajectory = create_test_trajectory("req-001", 0.8);
    sona.record_trajectory(trajectory).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 1);
    assert_eq!(stats.instant_updates, 1); // Should run instant loop
}

#[test]
fn test_sona_quality_threshold() {
    let config = SonaConfig {
        quality_threshold: 0.7,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // High quality - should trigger instant loop
    let high_quality = create_test_trajectory("req-001", 0.9);
    sona.record_trajectory(high_quality).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 1);
    assert_eq!(stats.instant_updates, 1);

    // Low quality - should not trigger instant loop
    let low_quality = create_test_trajectory("req-002", 0.5);
    sona.record_trajectory(low_quality).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 2);
    assert_eq!(stats.instant_updates, 1); // Still 1
}

#[test]
fn test_sona_multiple_trajectories() {
    let config = SonaConfig {
        quality_threshold: 0.0,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    for i in 0..10 {
        let trajectory = create_test_trajectory(&format!("req-{:03}", i), 0.8);
        sona.record_trajectory(trajectory).unwrap();
    }

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 10);
    assert_eq!(stats.instant_updates, 10);
}

#[test]
fn test_sona_routing_recommendation_no_patterns() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    let query = vec![0.1; 128];
    let rec = sona.get_routing_recommendation(&query);

    // With no patterns, should return defaults
    assert_eq!(rec.based_on_patterns, 0);
}

#[test]
fn test_routing_recommendation_default() {
    let rec = RoutingRecommendation::default();

    assert_eq!(rec.suggested_model, 0);
    assert_eq!(rec.confidence, 0.0);
    assert_eq!(rec.based_on_patterns, 0);
    assert_eq!(rec.average_quality, 0.0);
}

#[test]
fn test_sona_search_patterns_empty() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    let query = vec![0.1; 128];
    let patterns = sona.search_patterns(&query, 5);

    assert!(patterns.is_empty());
}

#[test]
fn test_sona_apply_transform() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    let input = vec![0.1; 64]; // Must match hidden_dim
    let output = sona.apply_transform(&input);

    assert_eq!(output.len(), input.len());
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_sona_stats() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    let stats = sona.stats();

    assert_eq!(stats.total_trajectories, 0);
    assert_eq!(stats.instant_updates, 0);
    assert_eq!(stats.background_updates, 0);
    assert_eq!(stats.deep_updates, 0);
    assert_eq!(stats.patterns_learned, 0);
    assert_eq!(stats.buffer_size, 0);
}

#[test]
fn test_sona_stats_after_learning() {
    let config = SonaConfig {
        quality_threshold: 0.0,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // Record some trajectories
    for i in 0..5 {
        let trajectory = create_test_trajectory(&format!("req-{}", i), 0.8);
        sona.record_trajectory(trajectory).unwrap();
    }

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 5);
    assert!(stats.buffer_size > 0);
}

#[test]
fn test_sona_trigger_background_loop() {
    let config = SonaConfig {
        quality_threshold: 0.0,
        background_interval_secs: 0, // Allow immediate trigger
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // Record trajectories
    for i in 0..5 {
        let trajectory = create_test_trajectory(&format!("req-{}", i), 0.8);
        sona.record_trajectory(trajectory).unwrap();
    }

    // Trigger background loop
    sona.trigger_background_loop().unwrap();

    let stats = sona.stats();
    assert!(stats.background_updates >= 1);
}

#[test]
fn test_sona_trigger_deep_loop() {
    let config = SonaConfig {
        quality_threshold: 0.0,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // Record trajectories (this may trigger deep loop automatically if interval elapsed)
    for i in 0..5 {
        let trajectory = create_test_trajectory(&format!("req-{}", i), 0.8);
        sona.record_trajectory(trajectory).unwrap();
    }

    let stats_before = sona.stats();
    let deep_updates_before = stats_before.deep_updates;

    // Trigger background loop first (to populate patterns)
    sona.trigger_background_loop().unwrap();

    // Trigger deep loop explicitly
    sona.trigger_deep_loop().unwrap();

    let stats = sona.stats();
    // At least one more deep update after explicit trigger
    assert!(
        stats.deep_updates >= deep_updates_before + 1,
        "Expected at least {} deep updates, got {}",
        deep_updates_before + 1,
        stats.deep_updates
    );
}

#[test]
fn test_trajectory_timestamp() {
    let trajectory = create_test_trajectory("req-001", 0.8);
    let now = chrono::Utc::now();

    // Timestamp should be recent
    let diff = now - trajectory.timestamp;
    assert!(diff.num_seconds() < 1);
}

#[test]
fn test_sona_varying_quality_trajectories() {
    let config = SonaConfig {
        quality_threshold: 0.5,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // Record trajectories with varying quality
    let qualities = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1];
    for (i, &quality) in qualities.iter().enumerate() {
        let trajectory = create_test_trajectory(&format!("req-{}", i), quality);
        sona.record_trajectory(trajectory).unwrap();
    }

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 9);
    // Only 5 have quality >= 0.5 threshold
    assert_eq!(stats.instant_updates, 5);
}

#[test]
fn test_sona_empty_background_loop() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    // Trigger background loop with no trajectories
    // Note: The implementation returns early without incrementing counter
    // if there are no high-quality trajectories to process
    let result = sona.trigger_background_loop();
    assert!(result.is_ok());

    let stats = sona.stats();
    // With no trajectories meeting quality threshold, background_updates is 0
    assert_eq!(
        stats.background_updates, 0,
        "Background loop with no trajectories should not count as an update"
    );
}

#[test]
fn test_sona_empty_deep_loop() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    // Trigger deep loop with no patterns
    let result = sona.trigger_deep_loop();
    assert!(result.is_ok());

    let stats = sona.stats();
    assert_eq!(stats.deep_updates, 1);
}

#[test]
fn test_sona_large_embedding() {
    let config = SonaConfig {
        embedding_dim: 768,
        hidden_dim: 256,
        quality_threshold: 0.0,
        ..SonaConfig::default()
    };
    let sona = SonaIntegration::new(config);

    let trajectory = Trajectory {
        request_id: "large-001".to_string(),
        session_id: "test".to_string(),
        query_embedding: vec![0.1; 768],
        response_embedding: vec![0.2; 768],
        quality_score: 0.9,
        routing_features: vec![0.5; 4],
        model_index: 0,
        timestamp: chrono::Utc::now(),
    };

    sona.record_trajectory(trajectory).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 1);
}

#[test]
fn test_sona_model_index_mapping() {
    let config = SonaConfig {
        quality_threshold: 0.0,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // Test different model indices
    for model_idx in 0..4 {
        let trajectory = Trajectory {
            request_id: format!("model-{}", model_idx),
            session_id: "test".to_string(),
            query_embedding: vec![0.1; 128],
            response_embedding: vec![0.2; 128],
            quality_score: 0.8,
            routing_features: vec![0.5; 4],
            model_index: model_idx,
            timestamp: chrono::Utc::now(),
        };

        sona.record_trajectory(trajectory).unwrap();
    }

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 4);
}

#[test]
fn test_sona_concurrent_safe() {
    use std::sync::Arc;
    use std::thread;

    let config = SonaConfig {
        quality_threshold: 0.0,
        ..create_test_sona_config()
    };
    let sona = Arc::new(SonaIntegration::new(config));

    let mut handles = vec![];

    // Spawn multiple threads recording trajectories
    for thread_id in 0..4 {
        let sona_clone = Arc::clone(&sona);
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let trajectory = Trajectory {
                    request_id: format!("thread-{}-req-{}", thread_id, i),
                    session_id: format!("thread-{}", thread_id),
                    query_embedding: vec![0.1; 128],
                    response_embedding: vec![0.2; 128],
                    quality_score: 0.8,
                    routing_features: vec![0.5; 4],
                    model_index: 0,
                    timestamp: chrono::Utc::now(),
                };
                sona_clone.record_trajectory(trajectory).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 40);
}

#[test]
fn test_sona_stats_struct() {
    let stats = SonaStats {
        total_trajectories: 100,
        instant_updates: 80,
        background_updates: 5,
        deep_updates: 1,
        patterns_learned: 50,
        buffer_size: 20,
        last_background_secs_ago: 3600,
        last_deep_secs_ago: 86400,
    };

    assert_eq!(stats.total_trajectories, 100);
    assert_eq!(stats.instant_updates, 80);
    assert_eq!(stats.background_updates, 5);
    assert_eq!(stats.deep_updates, 1);
    assert_eq!(stats.patterns_learned, 50);
    assert_eq!(stats.buffer_size, 20);
}

#[test]
fn test_sona_routing_features() {
    let trajectory = Trajectory {
        request_id: "routing-test".to_string(),
        session_id: "test".to_string(),
        query_embedding: vec![0.1; 128],
        response_embedding: vec![0.2; 128],
        quality_score: 0.9,
        routing_features: vec![0.7, 0.9, 0.8, 0.5], // temperature, top_p, confidence, context_ratio
        model_index: 1,
        timestamp: chrono::Utc::now(),
    };

    assert_eq!(trajectory.routing_features.len(), 4);
    assert_eq!(trajectory.routing_features[0], 0.7); // temperature
    assert_eq!(trajectory.routing_features[1], 0.9); // top_p
}

#[test]
fn test_sona_boundary_quality() {
    let config = SonaConfig {
        quality_threshold: 0.5,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    // Exactly at threshold
    let trajectory = create_test_trajectory("boundary", 0.5);
    sona.record_trajectory(trajectory).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.instant_updates, 1); // Should still trigger
}

#[test]
fn test_sona_zero_quality() {
    let config = SonaConfig {
        quality_threshold: 0.0,
        ..create_test_sona_config()
    };
    let sona = SonaIntegration::new(config);

    let trajectory = create_test_trajectory("zero-quality", 0.0);
    sona.record_trajectory(trajectory).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 1);
    // With threshold 0.0, even quality 0.0 should trigger (0.0 >= 0.0)
    assert_eq!(stats.instant_updates, 1);
}

#[test]
fn test_sona_negative_quality_handling() {
    let config = create_test_sona_config();
    let sona = SonaIntegration::new(config);

    // Negative quality should still be recorded but not trigger learning
    let trajectory = Trajectory {
        request_id: "negative".to_string(),
        session_id: "test".to_string(),
        query_embedding: vec![0.1; 128],
        response_embedding: vec![0.2; 128],
        quality_score: -0.5, // Negative
        routing_features: vec![0.5; 4],
        model_index: 0,
        timestamp: chrono::Utc::now(),
    };

    sona.record_trajectory(trajectory).unwrap();

    let stats = sona.stats();
    assert_eq!(stats.total_trajectories, 1);
    assert_eq!(stats.instant_updates, 0); // Should not trigger
}
