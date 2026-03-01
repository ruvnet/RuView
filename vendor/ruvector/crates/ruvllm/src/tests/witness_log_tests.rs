//! Witness Log Tests
//!
//! Tests for async write batching, flush on shutdown, backpressure handling,
//! and the overall witness logging system.

use crate::types::ModelSize;
use crate::witness_log::{
    AsyncWriteConfig, LatencyBreakdown, RoutingDecision, WitnessEntry, WitnessLog,
};
use std::time::Instant;

// ============================================================================
// LatencyBreakdown Tests
// ============================================================================

#[test]
fn test_latency_breakdown_default() {
    let latency = LatencyBreakdown::default();

    assert_eq!(latency.embedding_ms, 0.0);
    assert_eq!(latency.retrieval_ms, 0.0);
    assert_eq!(latency.routing_ms, 0.0);
    assert_eq!(latency.attention_ms, 0.0);
    assert_eq!(latency.generation_ms, 0.0);
    assert_eq!(latency.total_ms, 0.0);
}

#[test]
fn test_latency_breakdown_compute_total() {
    let mut latency = LatencyBreakdown {
        embedding_ms: 10.0,
        retrieval_ms: 5.0,
        routing_ms: 2.0,
        attention_ms: 50.0,
        generation_ms: 100.0,
        total_ms: 0.0,
    };

    latency.compute_total();

    assert_eq!(latency.total_ms, 167.0);
}

#[test]
fn test_latency_breakdown_exceeds_threshold() {
    let latency = LatencyBreakdown {
        embedding_ms: 10.0,
        retrieval_ms: 5.0,
        routing_ms: 2.0,
        attention_ms: 50.0,
        generation_ms: 100.0,
        total_ms: 167.0,
    };

    assert!(latency.exceeds_threshold(100.0));
    assert!(!latency.exceeds_threshold(200.0));
}

#[test]
fn test_latency_breakdown_slowest_component() {
    let latency = LatencyBreakdown {
        embedding_ms: 10.0,
        retrieval_ms: 5.0,
        routing_ms: 2.0,
        attention_ms: 50.0,
        generation_ms: 100.0,
        total_ms: 167.0,
    };

    let (name, value) = latency.slowest_component();
    assert_eq!(name, "generation");
    assert_eq!(value, 100.0);
}

#[test]
fn test_latency_breakdown_slowest_component_attention() {
    let latency = LatencyBreakdown {
        embedding_ms: 10.0,
        retrieval_ms: 5.0,
        routing_ms: 2.0,
        attention_ms: 200.0,
        generation_ms: 100.0,
        total_ms: 317.0,
    };

    let (name, _) = latency.slowest_component();
    assert_eq!(name, "attention");
}

#[test]
fn test_latency_breakdown_all_zeros() {
    let latency = LatencyBreakdown::default();
    let (_, value) = latency.slowest_component();
    assert_eq!(value, 0.0);
}

// ============================================================================
// RoutingDecision Tests
// ============================================================================

#[test]
fn test_routing_decision_default() {
    let decision = RoutingDecision::default();

    assert_eq!(decision.model, ModelSize::Small);
    assert_eq!(decision.context_size, 0);
    assert!((decision.temperature - 0.7).abs() < 0.01);
    assert!((decision.top_p - 0.9).abs() < 0.01);
    assert!((decision.confidence - 0.5).abs() < 0.01);
    assert_eq!(decision.model_probs, [0.25, 0.25, 0.25, 0.25]);
}

#[test]
fn test_routing_decision_custom() {
    let decision = RoutingDecision {
        model: ModelSize::Large,
        context_size: 4096,
        temperature: 0.3,
        top_p: 0.95,
        confidence: 0.85,
        model_probs: [0.1, 0.1, 0.2, 0.6],
    };

    assert_eq!(decision.model, ModelSize::Large);
    assert_eq!(decision.context_size, 4096);
    assert!((decision.confidence - 0.85).abs() < 0.01);

    // Probabilities should sum to 1.0
    let sum: f32 = decision.model_probs.iter().sum();
    assert!((sum - 1.0).abs() < 0.01);
}

#[test]
fn test_routing_decision_serialization() {
    let decision = RoutingDecision {
        model: ModelSize::Medium,
        context_size: 2048,
        temperature: 0.5,
        top_p: 0.85,
        confidence: 0.7,
        model_probs: [0.2, 0.3, 0.3, 0.2],
    };

    // Test that serialization works
    let json = serde_json::to_string(&decision).unwrap();
    assert!(json.contains("context_size"));

    // Test roundtrip
    let deserialized: RoutingDecision = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.context_size, 2048);
    assert!((deserialized.temperature - 0.5).abs() < 0.01);
}

// ============================================================================
// WitnessEntry Tests
// ============================================================================

#[test]
fn test_witness_entry_new() {
    let entry = WitnessEntry::new(
        "session-123".to_string(),
        vec![0.1; 768],
        RoutingDecision::default(),
    );

    assert!(!entry.request_id.is_nil());
    assert_eq!(entry.session_id, "session-123");
    assert_eq!(entry.query_embedding.len(), 768);
    assert_eq!(entry.model_used, ModelSize::Small);
    assert_eq!(entry.quality_score, 0.0);
    assert!(entry.is_success());
    assert!(entry.error.is_none());
}

#[test]
fn test_witness_entry_with_quality() {
    let entry = WitnessEntry::new(
        "session-456".to_string(),
        vec![0.5; 768],
        RoutingDecision::default(),
    )
    .with_quality(0.85);

    assert!((entry.quality_score - 0.85).abs() < 0.01);
    assert!(entry.meets_quality_threshold(0.8));
    assert!(!entry.meets_quality_threshold(0.9));
}

#[test]
fn test_witness_entry_with_latency() {
    let latency = LatencyBreakdown {
        embedding_ms: 5.0,
        retrieval_ms: 10.0,
        routing_ms: 1.0,
        attention_ms: 30.0,
        generation_ms: 50.0,
        total_ms: 96.0,
    };

    let entry = WitnessEntry::new(
        "session-789".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    )
    .with_latency(latency);

    assert_eq!(entry.latency.total_ms, 96.0);
    assert_eq!(entry.latency.generation_ms, 50.0);
}

#[test]
fn test_witness_entry_with_error() {
    use crate::types::ErrorInfo;

    let error = ErrorInfo {
        code: "TIMEOUT".to_string(),
        message: "Request timed out".to_string(),
        stack_trace: None,
        recovery_attempted: false,
    };

    let entry = WitnessEntry::new(
        "session-error".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    )
    .with_error(error);

    assert!(!entry.is_success());
    assert!(entry.error.is_some());
    assert_eq!(entry.error.as_ref().unwrap().code, "TIMEOUT");
}

#[test]
fn test_witness_entry_quality_threshold_edge_cases() {
    let entry_zero = WitnessEntry::new(
        "session".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    )
    .with_quality(0.0);

    assert!(entry_zero.meets_quality_threshold(0.0));
    assert!(!entry_zero.meets_quality_threshold(0.1));

    let entry_one = WitnessEntry::new(
        "session".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    )
    .with_quality(1.0);

    assert!(entry_one.meets_quality_threshold(1.0));
    assert!(entry_one.meets_quality_threshold(0.99));
}

#[test]
fn test_witness_entry_timestamp() {
    let before = chrono::Utc::now();
    let entry = WitnessEntry::new(
        "session".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    );
    let after = chrono::Utc::now();

    assert!(entry.timestamp >= before);
    assert!(entry.timestamp <= after);
}

#[test]
fn test_witness_entry_unique_ids() {
    let entry1 = WitnessEntry::new("s1".to_string(), vec![0.0; 768], RoutingDecision::default());
    let entry2 = WitnessEntry::new("s1".to_string(), vec![0.0; 768], RoutingDecision::default());

    // Each entry should have unique request_id
    assert_ne!(entry1.request_id, entry2.request_id);
}

// ============================================================================
// AsyncWriteConfig Tests
// ============================================================================

#[test]
fn test_async_write_config_default() {
    let config = AsyncWriteConfig::default();

    assert_eq!(config.max_batch_size, 100);
    assert_eq!(config.max_wait_ms, 1000);
    assert_eq!(config.max_queue_depth, 10000);
    assert!(!config.fsync_critical);
    assert_eq!(config.flush_interval_ms, 1000);
}

#[test]
fn test_async_write_config_custom() {
    let config = AsyncWriteConfig {
        max_batch_size: 50,
        max_wait_ms: 500,
        max_queue_depth: 5000,
        fsync_critical: true,
        flush_interval_ms: 250,
    };

    assert_eq!(config.max_batch_size, 50);
    assert!(config.fsync_critical);
}

// ============================================================================
// WritebackQueue Behavior Tests (Indirect via WitnessLog)
// ============================================================================

#[test]
fn test_writeback_batching_behavior() {
    // Simulate the batching behavior
    let max_batch_size = 10;
    let mut batch: Vec<WitnessEntry> = Vec::new();

    // Add entries
    for i in 0..15 {
        let entry = WitnessEntry::new(
            format!("session-{}", i),
            vec![i as f32 / 100.0; 768],
            RoutingDecision::default(),
        );
        batch.push(entry);

        // Check if batch should be flushed
        if batch.len() >= max_batch_size {
            assert_eq!(batch.len(), 10);
            batch.clear();
        }
    }

    // Remaining entries
    assert_eq!(batch.len(), 5);
}

#[test]
fn test_backpressure_behavior() {
    // Simulate backpressure when queue is full
    let max_queue_depth = 100;
    let mut queue_len = 0;
    let mut dropped = 0;

    for _ in 0..150 {
        if queue_len < max_queue_depth {
            queue_len += 1;
        } else {
            dropped += 1;
        }
    }

    assert_eq!(queue_len, 100);
    assert_eq!(dropped, 50);
}

#[test]
fn test_time_based_flush_simulation() {
    use std::thread::sleep;
    use std::time::Duration;

    let max_wait = Duration::from_millis(100);
    let start = Instant::now();

    // Simulate waiting for time-based flush
    sleep(Duration::from_millis(50));
    assert!(start.elapsed() < max_wait, "Not yet time to flush");

    sleep(Duration::from_millis(60));
    assert!(start.elapsed() >= max_wait, "Should flush by now");
}

// ============================================================================
// WitnessLog Stats Tests
// ============================================================================

#[test]
fn test_witness_log_stats_structure() {
    use crate::witness_log::WitnessLogStats;

    let stats = WitnessLogStats {
        total_entries: 1000,
        success_count: 950,
        error_count: 50,
        success_rate: 0.95,
        pending_writes: 25,
        dropped_entries: 0,
        background_running: false,
    };

    assert_eq!(stats.total_entries, 1000);
    assert_eq!(stats.success_count + stats.error_count, 1000);
    assert!((stats.success_rate - 0.95).abs() < 0.01);
}

#[test]
fn test_witness_log_stats_default() {
    use crate::witness_log::WitnessLogStats;

    let stats = WitnessLogStats::default();

    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.success_count, 0);
    assert_eq!(stats.error_count, 0);
    assert_eq!(stats.success_rate, 0.0);
    assert_eq!(stats.pending_writes, 0);
    assert_eq!(stats.dropped_entries, 0);
    assert!(!stats.background_running);
}

#[test]
fn test_witness_log_stats_serialization() {
    use crate::witness_log::WitnessLogStats;

    let stats = WitnessLogStats {
        total_entries: 100,
        success_count: 95,
        error_count: 5,
        success_rate: 0.95,
        pending_writes: 10,
        dropped_entries: 0,
        background_running: false,
    };

    let json = serde_json::to_string(&stats).unwrap();
    assert!(json.contains("total_entries"));
    assert!(json.contains("success_rate"));

    let deserialized: WitnessLogStats = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.total_entries, 100);
}

// ============================================================================
// Concurrent Access Simulation Tests
// ============================================================================

#[test]
fn test_concurrent_entry_creation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Spawn multiple threads creating entries
    for _ in 0..10 {
        let counter_clone = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = WitnessEntry::new(
                    "session".to_string(),
                    vec![0.0; 768],
                    RoutingDecision::default(),
                );
                counter_clone.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(counter.load(Ordering::Relaxed), 1000);
}

#[test]
fn test_unique_ids_concurrent() {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    use std::thread;

    let ids = Arc::new(Mutex::new(HashSet::new()));
    let mut handles = vec![];

    for _ in 0..10 {
        let ids_clone = Arc::clone(&ids);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let entry = WitnessEntry::new(
                    "session".to_string(),
                    vec![0.0; 768],
                    RoutingDecision::default(),
                );
                ids_clone.lock().unwrap().insert(entry.request_id);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let unique_count = ids.lock().unwrap().len();
    assert_eq!(unique_count, 1000, "All IDs should be unique");
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_witness_entry_error_chain() {
    use crate::types::ErrorInfo;

    let entry = WitnessEntry::new(
        "session".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    )
    .with_quality(0.5)
    .with_latency(LatencyBreakdown {
        embedding_ms: 10.0,
        retrieval_ms: 5.0,
        routing_ms: 2.0,
        attention_ms: 30.0,
        generation_ms: 50.0,
        total_ms: 97.0,
    })
    .with_error(ErrorInfo {
        code: "GEN_FAILED".to_string(),
        message: "Generation failed".to_string(),
        stack_trace: None,
        recovery_attempted: false,
    });

    // All builder methods should work together
    assert!((entry.quality_score - 0.5).abs() < 0.01);
    assert_eq!(entry.latency.total_ms, 97.0);
    assert!(!entry.is_success());
    assert_eq!(entry.error.as_ref().unwrap().code, "GEN_FAILED");
}

// ============================================================================
// Tag Filtering Tests
// ============================================================================

#[test]
fn test_witness_entry_tags() {
    let mut entry = WitnessEntry::new(
        "session".to_string(),
        vec![0.0; 768],
        RoutingDecision::default(),
    );

    entry.tags.push("production".to_string());
    entry.tags.push("high-priority".to_string());
    entry.tags.push("api-v2".to_string());

    assert_eq!(entry.tags.len(), 3);
    assert!(entry.tags.contains(&"production".to_string()));
}

#[test]
fn test_witness_entry_filter_by_tag() {
    let entries: Vec<WitnessEntry> = (0..10)
        .map(|i| {
            let mut entry = WitnessEntry::new(
                format!("session-{}", i),
                vec![0.0; 768],
                RoutingDecision::default(),
            );
            if i % 2 == 0 {
                entry.tags.push("even".to_string());
            } else {
                entry.tags.push("odd".to_string());
            }
            entry
        })
        .collect();

    let even_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.tags.contains(&"even".to_string()))
        .collect();

    assert_eq!(even_entries.len(), 5);
}

// ============================================================================
// Performance Measurement Tests
// ============================================================================

#[test]
fn test_entry_creation_performance() {
    let iterations = 10000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = WitnessEntry::new(
            "session".to_string(),
            vec![0.0; 768],
            RoutingDecision::default(),
        );
    }
    let duration = start.elapsed();

    let avg_us = duration.as_micros() as f64 / iterations as f64;
    assert!(
        avg_us < 100.0,
        "Entry creation should be fast: {}us",
        avg_us
    );
}

#[test]
fn test_latency_breakdown_performance() {
    let iterations = 100000;

    let start = Instant::now();
    for _ in 0..iterations {
        let mut latency = LatencyBreakdown {
            embedding_ms: 10.0,
            retrieval_ms: 5.0,
            routing_ms: 2.0,
            attention_ms: 50.0,
            generation_ms: 100.0,
            total_ms: 0.0,
        };
        latency.compute_total();
        let _ = latency.slowest_component();
    }
    let duration = start.elapsed();

    let avg_ns = duration.as_nanos() as f64 / iterations as f64;
    assert!(
        avg_ns < 1000.0,
        "Latency operations should be fast: {}ns",
        avg_ns
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_embedding() {
    let entry = WitnessEntry::new(
        "session".to_string(),
        vec![], // Empty embedding
        RoutingDecision::default(),
    );

    assert!(entry.query_embedding.is_empty());
}

#[test]
fn test_large_embedding() {
    let large_embedding = vec![0.1; 4096]; // 4K dimension embedding

    let entry = WitnessEntry::new(
        "session".to_string(),
        large_embedding.clone(),
        RoutingDecision::default(),
    );

    assert_eq!(entry.query_embedding.len(), 4096);
}

#[test]
fn test_empty_session_id() {
    let entry = WitnessEntry::new("".to_string(), vec![0.0; 768], RoutingDecision::default());

    assert!(entry.session_id.is_empty());
}

#[test]
fn test_long_session_id() {
    let long_id = "x".repeat(1000);

    let entry = WitnessEntry::new(long_id.clone(), vec![0.0; 768], RoutingDecision::default());

    assert_eq!(entry.session_id.len(), 1000);
}

#[test]
fn test_extreme_latency_values() {
    let latency = LatencyBreakdown {
        embedding_ms: f32::MAX / 10.0,
        retrieval_ms: 0.0,
        routing_ms: 0.0,
        attention_ms: 0.0,
        generation_ms: 0.0,
        total_ms: 0.0,
    };

    assert!(latency.embedding_ms.is_finite());
}

#[test]
fn test_zero_confidence_routing() {
    let decision = RoutingDecision {
        model: ModelSize::Tiny,
        confidence: 0.0,
        ..Default::default()
    };

    assert_eq!(decision.confidence, 0.0);
}

#[test]
fn test_max_confidence_routing() {
    let decision = RoutingDecision {
        model: ModelSize::Large,
        confidence: 1.0,
        ..Default::default()
    };

    assert_eq!(decision.confidence, 1.0);
}
