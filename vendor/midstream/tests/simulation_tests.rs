//! Comprehensive simulation tests for various real-world scenarios

use midstream::{
    LeanAgenticSystem, LeanAgenticConfig, AgentContext,
    KnowledgeGraph, Entity, EntityType, Relation,
};
use std::time::Instant;

#[tokio::test]
async fn test_weather_intent_simulation() {
    let config = LeanAgenticConfig::default();
    let system = LeanAgenticSystem::new(config);
    let mut context = AgentContext::new("weather_session".to_string());

    let messages = vec![
        "What's the weather like today?",
        "How about tomorrow?",
        "Will it rain this weekend?",
        "Should I bring an umbrella?",
    ];

    for (i, msg) in messages.iter().enumerate() {
        let result = system.process_stream_chunk(msg, context.clone()).await;
        assert!(result.is_ok(), "Message {} failed: {:?}", i, result);

        let res = result.unwrap();
        println!("Message: {}", msg);
        println!("  Action: {}", res.action.description);
        println!("  Reward: {:.3}", res.reward);
        println!("  Verified: {}", res.verified);

        context.add_message(msg.to_string());
    }

    // Verify learning occurred
    let stats = system.get_stats().await;
    assert!(stats.total_actions >= messages.len() as u64);
    println!("\nFinal stats: {:?}", stats);
}

#[tokio::test]
async fn test_knowledge_accumulation_simulation() {
    let config = LeanAgenticConfig::default();
    let system = LeanAgenticSystem::new(config);
    let mut context = AgentContext::new("learning_session".to_string());

    let learning_sequence = vec![
        "My name is Alice and I work at Google",
        "I live in San Francisco",
        "I prefer detailed weather forecasts",
        "My favorite color is blue",
        "I usually wake up at 7 AM",
    ];

    for msg in &learning_sequence {
        let result = system.process_stream_chunk(msg, context.clone()).await.unwrap();
        context.add_message(msg.to_string());
        context.set_preference("detail_level".to_string(), 0.9);
    }

    let stats = system.get_stats().await;

    // Verify knowledge was accumulated
    assert!(stats.total_entities > 0, "No entities extracted");
    assert!(stats.learning_iterations > 0, "No learning occurred");

    println!("Knowledge accumulation results:");
    println!("  Entities: {}", stats.total_entities);
    println!("  Learning iterations: {}", stats.learning_iterations);
    println!("  Average reward: {:.3}", stats.average_reward);
}

#[tokio::test]
async fn test_high_frequency_streaming_simulation() {
    let config = LeanAgenticConfig {
        enable_formal_verification: false, // Disable for speed
        learning_rate: 0.05,
        ..Default::default()
    };

    let system = LeanAgenticSystem::new(config);
    let context = AgentContext::new("streaming_session".to_string());

    let start = Instant::now();
    let num_chunks = 1000;

    for i in 0..num_chunks {
        let chunk = format!("Stream chunk {}", i);
        let result = system.process_stream_chunk(&chunk, context.clone()).await;
        assert!(result.is_ok(), "Chunk {} failed", i);
    }

    let duration = start.elapsed();
    let chunks_per_sec = num_chunks as f64 / duration.as_secs_f64();

    println!("\nHigh-frequency streaming results:");
    println!("  Total chunks: {}", num_chunks);
    println!("  Duration: {:?}", duration);
    println!("  Throughput: {:.2} chunks/sec", chunks_per_sec);
    println!("  Avg latency: {:.2} ms/chunk", duration.as_millis() as f64 / num_chunks as f64);

    // Verify minimum throughput
    assert!(chunks_per_sec > 50.0, "Throughput too low: {:.2} chunks/sec", chunks_per_sec);
}

#[tokio::test]
async fn test_concurrent_sessions_simulation() {
    let config = LeanAgenticConfig::default();
    let system = LeanAgenticSystem::new(config);

    let num_sessions = 100;
    let mut handles = vec![];

    let start = Instant::now();

    for i in 0..num_sessions {
        let sys = &system;
        let handle = tokio::spawn(async move {
            let context = AgentContext::new(format!("session_{}", i));
            let messages = vec![
                "Hello",
                "What's the weather?",
                "Thank you",
            ];

            for msg in messages {
                sys.process_stream_chunk(msg, context.clone()).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let duration = start.elapsed();

    println!("\nConcurrent sessions results:");
    println!("  Sessions: {}", num_sessions);
    println!("  Duration: {:?}", duration);
    println!("  Avg per session: {:.2} ms", duration.as_millis() as f64 / num_sessions as f64);

    let stats = system.get_stats().await;
    println!("  Total actions: {}", stats.total_actions);
}

#[tokio::test]
async fn test_learning_convergence_simulation() {
    let config = LeanAgenticConfig {
        learning_rate: 0.1, // Higher learning rate for faster convergence
        ..Default::default()
    };

    let system = LeanAgenticSystem::new(config);
    let mut context = AgentContext::new("convergence_session".to_string());

    // Repeat the same pattern to test learning convergence
    let pattern = "What is the weather in Tokyo?";
    let mut rewards = vec![];

    for iteration in 0..100 {
        let result = system.process_stream_chunk(pattern, context.clone()).await.unwrap();
        rewards.push(result.reward);

        if iteration % 10 == 0 {
            println!("Iteration {}: reward = {:.3}", iteration, result.reward);
        }

        context.add_message(pattern.to_string());
    }

    // Check if rewards are improving (basic convergence check)
    let early_avg: f64 = rewards[0..20].iter().sum::<f64>() / 20.0;
    let late_avg: f64 = rewards[80..100].iter().sum::<f64>() / 20.0;

    println!("\nLearning convergence results:");
    println!("  Early average reward (0-20): {:.3}", early_avg);
    println!("  Late average reward (80-100): {:.3}", late_avg);
    println!("  Improvement: {:.3}", late_avg - early_avg);

    // Rewards should stabilize or improve
    assert!(late_avg >= early_avg * 0.8, "Learning degraded significantly");
}

#[tokio::test]
async fn test_knowledge_graph_scaling() {
    let mut kg = KnowledgeGraph::new();

    let start = Instant::now();
    let num_entities = 10000;

    // Add many entities
    for i in 0..num_entities {
        let entity = Entity {
            id: format!("entity_{}", i),
            name: format!("Entity {}", i),
            entity_type: if i % 3 == 0 {
                EntityType::Person
            } else if i % 3 == 1 {
                EntityType::Organization
            } else {
                EntityType::Concept
            },
            attributes: std::collections::HashMap::new(),
            confidence: 0.9,
        };

        kg.update(vec![entity]).await.unwrap();
    }

    let insert_duration = start.elapsed();

    // Add relations
    let relation_start = Instant::now();
    for i in 0..1000 {
        kg.add_relation(Relation {
            id: format!("rel_{}", i),
            subject: format!("entity_{}", i * 10),
            predicate: "relates_to".to_string(),
            object: format!("entity_{}", i * 10 + 1),
            confidence: 0.85,
            source: "test".to_string(),
        });
    }
    let relation_duration = relation_start.elapsed();

    // Query performance
    let query_start = Instant::now();
    let results = kg.query_entities(EntityType::Person);
    let query_duration = query_start.elapsed();

    println!("\nKnowledge graph scaling results:");
    println!("  Entities inserted: {}", num_entities);
    println!("  Insert time: {:?}", insert_duration);
    println!("  Insert rate: {:.2} entities/sec", num_entities as f64 / insert_duration.as_secs_f64());
    println!("  Relations added: 1000");
    println!("  Relation time: {:?}", relation_duration);
    println!("  Query time: {:?}", query_duration);
    println!("  Results found: {}", results.len());

    assert_eq!(kg.entity_count(), num_entities);
    assert_eq!(kg.relation_count(), 1000);
}

#[tokio::test]
async fn test_adaptive_behavior_simulation() {
    let config = LeanAgenticConfig {
        learning_rate: 0.05,
        ..Default::default()
    };

    let system = LeanAgenticSystem::new(config);
    let mut context = AgentContext::new("adaptive_session".to_string());

    // Phase 1: Weather queries
    println!("\nPhase 1: Weather queries");
    for i in 0..10 {
        let msg = format!("What's the weather in city {}?", i);
        let result = system.process_stream_chunk(&msg, context.clone()).await.unwrap();
        context.add_message(msg);
        if i == 0 || i == 9 {
            println!("  Iteration {}: reward = {:.3}", i, result.reward);
        }
    }

    // Phase 2: Switch to learning/memory queries
    println!("\nPhase 2: Learning queries");
    for i in 0..10 {
        let msg = format!("Remember that I like {}", i);
        let result = system.process_stream_chunk(&msg, context.clone()).await.unwrap();
        context.add_message(msg);
        if i == 0 || i == 9 {
            println!("  Iteration {}: reward = {:.3}", i, result.reward);
        }
    }

    let stats = system.get_stats().await;
    println!("\nAdaptive behavior stats:");
    println!("  Total actions: {}", stats.total_actions);
    println!("  Average reward: {:.3}", stats.average_reward);
    println!("  Entities learned: {}", stats.total_entities);

    assert!(stats.total_actions >= 20);
}

#[tokio::test]
async fn test_memory_efficiency() {
    use std::mem::size_of;

    println!("\nMemory efficiency analysis:");
    println!("  AgentContext: {} bytes", size_of::<AgentContext>());
    println!("  Entity: {} bytes", size_of::<Entity>());
    println!("  Relation: {} bytes", size_of::<Relation>());

    // Test memory growth with many sessions
    let config = LeanAgenticConfig::default();
    let system = LeanAgenticSystem::new(config);

    for i in 0..100 {
        let context = AgentContext::new(format!("session_{}", i));
        system.process_stream_chunk("test", context).await.unwrap();
    }

    let stats = system.get_stats().await;
    println!("  Sessions processed: 100");
    println!("  Total entities: {}", stats.total_entities);
    println!("  Estimated memory per session: ~{} KB",
             (stats.total_entities * size_of::<Entity>()) / 100 / 1024);
}
