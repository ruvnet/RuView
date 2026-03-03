//! Integration tests for temporal comparison and scheduling
//!
//! Tests real-world scenarios combining temporal analysis and scheduling

use midstream::{
    TemporalComparator, Sequence, ComparisonAlgorithm,
    RealtimeScheduler, SchedulingPolicy, Priority,
    Action, AgentContext, AgenticLoop, LeanAgenticConfig,
};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_temporal_conversation_pattern_matching() {
    // Simulate detecting similar conversation patterns
    let mut comparator = TemporalComparator::<String>::new();

    // Add historical conversation sequences
    comparator.add_sequence(Sequence {
        data: vec![
            "greeting".to_string(),
            "weather_query".to_string(),
            "location_query".to_string(),
            "weather_response".to_string(),
        ],
        timestamp: 1000,
        id: "conv1".to_string(),
    });

    comparator.add_sequence(Sequence {
        data: vec![
            "greeting".to_string(),
            "weather_query".to_string(),
            "location_query".to_string(),
            "weather_response".to_string(),
            "followup".to_string(),
        ],
        timestamp: 2000,
        id: "conv2".to_string(),
    });

    comparator.add_sequence(Sequence {
        data: vec![
            "greeting".to_string(),
            "calendar_query".to_string(),
            "calendar_response".to_string(),
        ],
        timestamp: 3000,
        id: "conv3".to_string(),
    });

    // Query with new conversation
    let query = vec![
        "greeting".to_string(),
        "weather_query".to_string(),
        "location_query".to_string(),
    ];

    let similar = comparator.find_similar(&query, 0.7, ComparisonAlgorithm::LCS);

    // Should find conv1 and conv2 as similar (weather conversations)
    assert!(similar.len() >= 2);
    println!("Found {} similar conversations", similar.len());

    for (idx, score) in similar.iter() {
        println!("Conversation {}: similarity = {}", idx, score);
        assert!(*score >= 0.7);
    }
}

#[tokio::test]
async fn test_temporal_action_sequence_analysis() {
    // Test analyzing agent action sequences over time
    let mut comparator = TemporalComparator::<String>::new();

    // Normal behavior pattern
    let normal_sequence = vec![
        "plan".to_string(),
        "verify".to_string(),
        "execute".to_string(),
        "observe".to_string(),
        "learn".to_string(),
    ];

    // Anomalous behavior (skips verification)
    let anomalous_sequence = vec![
        "plan".to_string(),
        "execute".to_string(),
        "observe".to_string(),
        "learn".to_string(),
    ];

    // Compare sequences
    let similarity = comparator.compare(
        &normal_sequence,
        &anomalous_sequence,
        ComparisonAlgorithm::LCS,
    );

    println!("Similarity between normal and anomalous: {}", similarity);

    // LCS should show high similarity but not perfect
    assert!(similarity > 0.6);
    assert!(similarity < 1.0);

    // Edit distance should show difference
    let distance = comparator.compare(
        &normal_sequence,
        &anomalous_sequence,
        ComparisonAlgorithm::EditDistance,
    );

    println!("Edit distance: {}", distance);
    assert!(distance > 0.0); // Should detect the missing step
}

#[tokio::test]
async fn test_scheduler_with_deadlines() {
    // Test real-time scheduling with various deadline constraints
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

    // Schedule critical task with tight deadline
    let critical_action = Action {
        action_type: "critical_response".to_string(),
        description: "User safety check".to_string(),
        parameters: HashMap::new(),
        tool_calls: vec![],
        expected_outcome: Some("safe".to_string()),
        expected_reward: 1.0,
    };

    let critical_id = scheduler.schedule(
        critical_action,
        Priority::Critical,
        Duration::from_millis(50), // Very tight deadline
        Duration::from_millis(10),
    ).await;

    // Schedule normal task with relaxed deadline
    let normal_action = Action {
        action_type: "normal_query".to_string(),
        description: "Regular information request".to_string(),
        parameters: HashMap::new(),
        tool_calls: vec![],
        expected_outcome: None,
        expected_reward: 0.7,
    };

    scheduler.schedule(
        normal_action,
        Priority::Medium,
        Duration::from_secs(5), // Relaxed deadline
        Duration::from_millis(100),
    ).await;

    // EDF should prioritize the critical task due to earlier deadline
    let next = scheduler.next_task().await.unwrap();
    assert_eq!(next.id, critical_id);
    assert_eq!(next.action.action_type, "critical_response");

    println!("Scheduler correctly prioritized critical task with tight deadline");
}

#[tokio::test]
async fn test_scheduler_priority_override() {
    // Test that priority scheduling overrides based on priority level
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::FixedPriority);

    // Schedule low priority task first
    scheduler.schedule(
        Action {
            action_type: "background_task".to_string(),
            description: "Background processing".to_string(),
            parameters: HashMap::new(),
            tool_calls: vec![],
            expected_outcome: None,
            expected_reward: 0.3,
        },
        Priority::Background,
        Duration::from_secs(10),
        Duration::from_millis(100),
    ).await;

    // Schedule high priority task second
    scheduler.schedule(
        Action {
            action_type: "urgent_task".to_string(),
            description: "Urgent response needed".to_string(),
            parameters: HashMap::new(),
            tool_calls: vec![],
            expected_outcome: None,
            expected_reward: 0.9,
        },
        Priority::Critical,
        Duration::from_secs(10),
        Duration::from_millis(50),
    ).await;

    // Should get high priority task first despite being scheduled later
    let next = scheduler.next_task().await.unwrap();
    assert_eq!(next.action.action_type, "urgent_task");

    println!("Priority scheduling correctly prioritized critical task");
}

#[tokio::test]
async fn test_combined_temporal_and_scheduling() {
    // Integration test: Use temporal patterns to inform scheduling decisions
    let mut comparator = TemporalComparator::<String>::new();
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

    // Historical pattern: queries that led to good outcomes
    comparator.add_sequence(Sequence {
        data: vec![
            "user_query".to_string(),
            "context_check".to_string(),
            "knowledge_lookup".to_string(),
            "response".to_string(),
        ],
        timestamp: 1000,
        id: "good_pattern".to_string(),
    });

    // Current query sequence
    let current = vec!["user_query".to_string(), "context_check".to_string()];

    // Find similar patterns
    let similar = comparator.find_similar(&current, 0.5, ComparisonAlgorithm::LCS);

    if !similar.is_empty() {
        println!("Found similar successful pattern, scheduling with high priority");

        // Schedule next expected action with higher priority
        scheduler.schedule(
            Action {
                action_type: "knowledge_lookup".to_string(),
                description: "Predicted next action from pattern".to_string(),
                parameters: HashMap::new(),
                tool_calls: vec![],
                expected_outcome: Some("success".to_string()),
                expected_reward: 0.85,
            },
            Priority::High, // Higher priority based on pattern match
            Duration::from_millis(100),
            Duration::from_millis(20),
        ).await;
    }

    let stats = scheduler.get_stats().await;
    assert_eq!(stats.total_scheduled, 1);

    println!("Successfully combined temporal pattern matching with scheduling");
}

#[tokio::test]
async fn test_scheduler_deadline_checking() {
    // Test the can_meet_deadline functionality
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

    // Empty queue - should be able to meet deadline
    let can_meet = scheduler.can_meet_deadline(
        Duration::from_millis(10),
        Duration::from_secs(1),
    ).await;
    assert!(can_meet);

    // Add many tasks
    for i in 0..50 {
        scheduler.schedule(
            Action {
                action_type: format!("task_{}", i),
                description: format!("Task {}", i),
                parameters: HashMap::new(),
                tool_calls: vec![],
                expected_outcome: None,
                expected_reward: 0.7,
            },
            Priority::Medium,
            Duration::from_secs(10),
            Duration::from_millis(50), // Each task takes 50ms
        ).await;
    }

    // Now with 50 tasks * 50ms = 2500ms pending work
    let can_meet_tight = scheduler.can_meet_deadline(
        Duration::from_millis(10),
        Duration::from_millis(100), // Want to finish in 100ms
    ).await;

    assert!(!can_meet_tight); // Should not be able to meet tight deadline

    let can_meet_loose = scheduler.can_meet_deadline(
        Duration::from_millis(10),
        Duration::from_secs(10), // Generous deadline
    ).await;

    assert!(can_meet_loose); // Should be able to meet loose deadline

    println!("Deadline checking correctly estimates feasibility");
}

#[tokio::test]
async fn test_temporal_caching() {
    // Test that temporal comparison caching works correctly
    let mut comparator = TemporalComparator::<i32>::new();

    let seq1: Vec<i32> = (0..100).collect();
    let seq2: Vec<i32> = (0..100).map(|x| x + 1).collect();

    // First comparison - not cached
    let result1 = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW);

    // Second comparison - should be cached
    let result2 = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW);

    assert_eq!(result1, result2);

    let stats = comparator.cache_stats();
    println!("Cache stats: {:?}", stats);

    // Should have cached the result
    assert_eq!(stats.dtw_count, 1); // Only computed once

    // Try different algorithm - should compute again
    let _result3 = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::LCS);

    let stats2 = comparator.cache_stats();
    assert_eq!(stats2.lcs_count, 1);
    assert_eq!(stats2.total_comparisons, 2); // DTW + LCS

    println!("Caching working correctly: {} total comparisons", stats2.total_comparisons);
}

#[tokio::test]
async fn test_pattern_detection_in_stream() {
    // Simulate detecting recurring patterns in a stream
    let comparator = TemporalComparator::<String>::new();

    // Simulated stream of user intents
    let intent_stream = vec![
        "weather", "location", "weather", "news", "sports",
        "weather", "location", "weather", "calendar", "weather",
        "location", "weather",
    ].into_iter().map(|s| s.to_string()).collect::<Vec<_>>();

    // Pattern we're looking for
    let pattern = vec!["weather".to_string(), "location".to_string(), "weather".to_string()];

    let positions = comparator.detect_pattern(&intent_stream, &pattern);

    println!("Found pattern at positions: {:?}", positions);
    assert!(!positions.is_empty());

    // Should find the pattern at position 0 and position 9
    assert!(positions.contains(&0));
    assert!(positions.contains(&9));

    println!("Successfully detected {} pattern occurrences in stream", positions.len());
}

#[tokio::test]
async fn test_scheduler_stats_tracking() {
    // Test that scheduler correctly tracks statistics
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

    // Schedule and execute several tasks
    for i in 0..10 {
        let task_id = scheduler.schedule(
            Action {
                action_type: format!("task_{}", i),
                description: format!("Task {}", i),
                parameters: HashMap::new(),
                tool_calls: vec![],
                expected_outcome: None,
                expected_reward: 0.7,
            },
            Priority::Medium,
            Duration::from_secs(1),
            Duration::from_millis(10),
        ).await;

        // Mark as executed with varying durations
        scheduler.mark_executed(task_id, Duration::from_micros(100 * (i + 1))).await;
    }

    let stats = scheduler.get_stats().await;

    assert_eq!(stats.total_scheduled, 10);
    assert_eq!(stats.total_executed, 10);
    assert!(stats.average_latency_ns > 0);
    assert!(stats.max_latency_ns >= stats.min_latency_ns);

    println!("Scheduler stats: {:?}", stats);
    println!("Average latency: {} μs", stats.average_latency_ns / 1000);
}

#[tokio::test]
async fn test_real_world_conversation_flow() {
    // Simulate a realistic conversation flow with scheduling
    let mut comparator = TemporalComparator::<String>::new();
    let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

    // Add historical successful conversation patterns
    comparator.add_sequence(Sequence {
        data: vec![
            "greeting".to_string(),
            "clarification".to_string(),
            "action".to_string(),
            "confirmation".to_string(),
        ],
        timestamp: 1000,
        id: "success_pattern".to_string(),
    });

    // Current conversation
    let current_flow = vec!["greeting".to_string(), "clarification".to_string()];

    // Check similarity to successful patterns
    let similar = comparator.find_similar(&current_flow, 0.6, ComparisonAlgorithm::LCS);

    if !similar.is_empty() {
        // We found a similar successful pattern, schedule next actions accordingly

        // Schedule the predicted next action (from pattern)
        scheduler.schedule(
            Action {
                action_type: "action".to_string(),
                description: "Execute predicted action from pattern".to_string(),
                parameters: HashMap::new(),
                tool_calls: vec![],
                expected_outcome: Some("confirmation".to_string()),
                expected_reward: 0.8,
            },
            Priority::High,
            Duration::from_millis(200),
            Duration::from_millis(50),
        ).await;

        // Schedule confirmation as follow-up
        scheduler.schedule(
            Action {
                action_type: "confirmation".to_string(),
                description: "Confirm action completion".to_string(),
                parameters: HashMap::new(),
                tool_calls: vec![],
                expected_outcome: Some("success".to_string()),
                expected_reward: 0.9,
            },
            Priority::Medium,
            Duration::from_millis(500),
            Duration::from_millis(30),
        ).await;

        println!("Scheduled actions based on historical success pattern");
    }

    // Execute scheduled tasks
    let mut executed_count = 0;
    while let Some(task) = scheduler.next_task().await {
        println!("Executing: {}", task.action.action_type);
        scheduler.mark_executed(task.id, Duration::from_millis(10)).await;
        executed_count += 1;
    }

    assert_eq!(executed_count, 2);
    println!("Successfully completed conversation flow based on patterns");
}
