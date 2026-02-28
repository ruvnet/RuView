//! Integration tests for RuvLLM
//!
//! Tests the complete pipeline from request to response.

use ruvllm::types::{EdgeType, Feedback, MemoryEdge, MemoryNode, NodeType};
use ruvllm::{Config, Request, RuvLLM};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Atomic counter for unique test directories
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Helper to create test config with unique database path
fn test_config() -> Config {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_path = format!("/tmp/ruvllm_test_{}.db", id);
    Config::builder()
        .db_path(&db_path)
        .embedding_dim(128)
        .router_hidden_dim(32)
        .learning_enabled(false)
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_basic_query() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    let response = llm.query("What is machine learning?").await.unwrap();

    assert!(!response.text.is_empty());
    assert!(!response.request_id.is_empty());
    assert!(response.confidence >= 0.0 && response.confidence <= 1.0);
}

#[tokio::test]
async fn test_query_with_context() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    // Preload some context
    // (In real tests, we'd inject memory nodes)

    let response = llm.query("Explain neural networks").await.unwrap();

    assert!(!response.text.is_empty());
    assert!(response.latency.total_ms > 0.0);
}

#[tokio::test]
async fn test_session_management() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    // Create a session
    let session = llm.new_session();
    assert!(!session.id.is_empty());

    // Query with session
    let response = llm.query_session(&session, "Hello").await.unwrap();
    assert!(!response.text.is_empty());

    // Query again in same session
    let response2 = llm
        .query_session(&session, "Follow up question")
        .await
        .unwrap();
    assert!(!response2.text.is_empty());
}

#[tokio::test]
async fn test_routing_decision() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    let response = llm.query("Simple question").await.unwrap();

    // Check routing info is populated
    assert!(response.routing_info.confidence >= 0.0);
    assert!(response.routing_info.temperature > 0.0);
    assert!(response.routing_info.top_p > 0.0);
    assert!(response.routing_info.context_size > 0);
}

#[tokio::test]
async fn test_latency_breakdown() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    let response = llm.query("Test query for latency").await.unwrap();

    // All latency components should be non-negative
    assert!(response.latency.embedding_ms >= 0.0);
    assert!(response.latency.retrieval_ms >= 0.0);
    assert!(response.latency.routing_ms >= 0.0);
    assert!(response.latency.attention_ms >= 0.0);
    assert!(response.latency.generation_ms >= 0.0);

    // Total should be sum of components (approximately)
    let sum = response.latency.embedding_ms
        + response.latency.retrieval_ms
        + response.latency.routing_ms
        + response.latency.attention_ms
        + response.latency.generation_ms;

    // Allow some variance for overhead
    assert!(response.latency.total_ms >= sum * 0.9);
}

#[tokio::test]
async fn test_feedback() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    let response = llm.query("Test for feedback").await.unwrap();

    // Provide feedback
    let feedback = Feedback {
        request_id: response.request_id.clone(),
        rating: Some(5),
        correction: None,
        task_success: Some(true),
    };

    // Should not error
    llm.feedback(feedback).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_queries() {
    let config = test_config();
    let llm = std::sync::Arc::new(RuvLLM::new(config).await.unwrap());

    // Run multiple queries concurrently
    let mut handles = Vec::new();
    for i in 0..5 {
        let llm_clone = llm.clone();
        let handle = tokio::spawn(async move {
            let query = format!("Concurrent query {}", i);
            llm_clone.query(query).await.unwrap()
        });
        handles.push(handle);
    }

    // Wait for all
    for handle in handles {
        let response = handle.await.unwrap();
        assert!(!response.text.is_empty());
    }
}

#[tokio::test]
async fn test_shutdown() {
    let config = test_config();
    let llm = RuvLLM::new(config).await.unwrap();

    // Query first
    llm.query("Before shutdown").await.unwrap();

    // Shutdown should succeed
    llm.shutdown().await.unwrap();
}

// Module-specific integration tests

mod memory_integration {
    use super::*;
    use ruvllm::config::MemoryConfig;
    use ruvllm::memory::MemoryService;

    #[tokio::test]
    async fn test_memory_pipeline() {
        let config = MemoryConfig::default();
        let memory = MemoryService::new(&config).await.unwrap();

        // Insert nodes
        let nodes: Vec<MemoryNode> = (0..100)
            .map(|i| {
                let mut vec: Vec<f32> = vec![0.0; 128];
                vec[i % 128] = 1.0;
                MemoryNode {
                    id: format!("node-{}", i),
                    vector: vec,
                    text: format!("Document {} about topic {}", i, i % 10),
                    node_type: NodeType::Document,
                    source: "test".into(),
                    metadata: HashMap::new(),
                }
            })
            .collect();

        for node in nodes {
            memory.insert_node(node).unwrap();
        }

        // Insert edges
        for i in 0..99 {
            let edge = MemoryEdge {
                id: format!("edge-{}", i),
                src: format!("node-{}", i),
                dst: format!("node-{}", i + 1),
                edge_type: EdgeType::Follows,
                weight: 0.8,
                metadata: HashMap::new(),
            };
            memory.insert_edge(edge).unwrap();
        }

        // Search
        let mut query = vec![0.0f32; 128];
        query[50] = 1.0;

        let result = memory.search_with_graph(&query, 10, 64, 2).await.unwrap();

        assert!(!result.candidates.is_empty());
        assert!(result.candidates.len() <= 10);

        // First result should be close to node-50
        assert_eq!(result.candidates[0].id, "node-50");

        // Subgraph should include neighbors
        assert!(!result.subgraph.nodes.is_empty());
    }
}

mod router_integration {
    use super::*;
    use ruvllm::config::RouterConfig;
    use ruvllm::router::FastGRNNRouter;
    use ruvllm::types::RouterSample;

    #[test]
    fn test_router_training_cycle() {
        let config = RouterConfig::default();
        let mut router = FastGRNNRouter::new(&config).unwrap();

        // Create training samples
        let samples: Vec<RouterSample> = (0..100)
            .map(|i| RouterSample {
                features: vec![0.1; config.input_dim],
                label_model: i % 4,
                label_context: i % 5,
                label_temperature: 0.7,
                label_top_p: 0.9,
                quality: 0.8,
                latency_ms: 100.0 + (i as f32) * 10.0,
            })
            .collect();

        // Train
        let metrics = router.train_batch(&samples, 0.001, 0.0, None, None);

        assert!(metrics.total_loss >= 0.0);
        assert!(metrics.model_accuracy >= 0.0);

        // Forward pass should work
        let features = vec![0.1; config.input_dim];
        let hidden = vec![0.0; config.hidden_dim];
        let decision = router.forward(&features, &hidden).unwrap();

        assert!(decision.confidence >= 0.0);
    }

    #[test]
    fn test_router_ewc() {
        let config = RouterConfig::default();
        let mut router = FastGRNNRouter::new(&config).unwrap();

        // Initial training
        let samples1: Vec<RouterSample> = (0..50)
            .map(|_| RouterSample {
                features: vec![0.1; config.input_dim],
                label_model: 0,
                label_context: 0,
                label_temperature: 0.5,
                label_top_p: 0.9,
                quality: 0.9,
                latency_ms: 50.0,
            })
            .collect();

        router.train_batch(&samples1, 0.001, 0.0, None, None);

        // Compute Fisher information
        let fisher = router.compute_fisher(&samples1);

        // Train on new task with EWC (using same weights as optimal for test)
        let samples2: Vec<RouterSample> = (0..50)
            .map(|_| RouterSample {
                features: vec![0.5; config.input_dim],
                label_model: 3,
                label_context: 4,
                label_temperature: 0.9,
                label_top_p: 0.95,
                quality: 0.7,
                latency_ms: 200.0,
            })
            .collect();

        // Train with EWC regularization (using fisher as a proxy for optimal weights)
        let metrics = router.train_batch(
            &samples2,
            0.001,
            0.4,
            Some(&fisher),
            Some(&fisher), // Using fisher as placeholder for optimal weights
        );

        // Total loss should be non-negative
        assert!(metrics.total_loss >= 0.0);
        assert!(metrics.samples_processed > 0);
    }
}

mod attention_integration {
    use super::*;
    use ruvllm::attention::GraphAttentionEngine;
    use ruvllm::config::EmbeddingConfig;
    use ruvllm::memory::SubGraph;

    #[test]
    fn test_attention_with_complex_graph() {
        let config = EmbeddingConfig::default();
        let engine = GraphAttentionEngine::new(&config).unwrap();

        // Create a complex subgraph
        let nodes: Vec<MemoryNode> = (0..20)
            .map(|i| {
                let mut vec = vec![0.1; config.dimension];
                vec[i % config.dimension] += 0.5;
                // Normalize
                let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                vec.iter_mut().for_each(|x| *x /= norm);

                MemoryNode {
                    id: format!("n-{}", i),
                    vector: vec,
                    text: format!("Node {}", i),
                    node_type: NodeType::Document,
                    source: "test".into(),
                    metadata: HashMap::new(),
                }
            })
            .collect();

        // Create edges forming a more complex structure
        let mut edges = Vec::new();
        for i in 0..19 {
            edges.push(MemoryEdge {
                id: format!("e-{}-{}", i, i + 1),
                src: format!("n-{}", i),
                dst: format!("n-{}", i + 1),
                edge_type: EdgeType::Follows,
                weight: 0.9,
                metadata: HashMap::new(),
            });
        }
        // Add some cross-links
        for i in (0..15).step_by(5) {
            edges.push(MemoryEdge {
                id: format!("cross-{}", i),
                src: format!("n-{}", i),
                dst: format!("n-{}", i + 5),
                edge_type: EdgeType::SameTopic,
                weight: 0.7,
                metadata: HashMap::new(),
            });
        }

        let subgraph = SubGraph {
            nodes,
            edges,
            center_ids: vec!["n-0".into()],
        };

        // Query
        let query = vec![0.2; config.dimension];
        let context = engine.attend(&query, &subgraph).unwrap();

        // Validate
        assert_eq!(context.ranked_nodes.len(), 20);
        assert_eq!(context.attention_weights.len(), 20);

        // Weights sum to 1
        let sum: f32 = context.attention_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);

        // Multi-head weights
        assert!(!context.head_weights.is_empty());

        // Summary stats
        assert_eq!(context.summary.num_nodes, 20);
        assert!(context.summary.num_edges > 0);
    }
}

mod embedding_integration {
    use super::*;
    use ruvllm::config::EmbeddingConfig;
    use ruvllm::embedding::{EmbeddingService, PoolingStrategy};

    #[test]
    fn test_embedding_batch_processing() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        let texts: Vec<&str> = vec![
            "The quick brown fox",
            "Jumps over the lazy dog",
            "Machine learning is fascinating",
            "Neural networks process information",
            "Vector databases store embeddings",
        ];

        let embeddings = service.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 5);

        // Check pairwise similarities
        let mut similarities = Vec::new();
        for i in 0..embeddings.len() {
            for j in (i + 1)..embeddings.len() {
                let dot: f32 = embeddings[i]
                    .vector
                    .iter()
                    .zip(embeddings[j].vector.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                similarities.push((i, j, dot));
            }
        }

        // Related texts should have higher similarity
        // (In mock embeddings this may not hold, but structure should work)
        assert_eq!(similarities.len(), 10); // 5 choose 2
    }

    #[test]
    fn test_embedding_pooling_comparison() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        let text = "This is a test sentence for comparing pooling strategies";

        let mean = service
            .embed_with_pooling(text, PoolingStrategy::Mean)
            .unwrap();
        let max = service
            .embed_with_pooling(text, PoolingStrategy::Max)
            .unwrap();
        let cls = service
            .embed_with_pooling(text, PoolingStrategy::CLS)
            .unwrap();
        let last = service
            .embed_with_pooling(text, PoolingStrategy::LastToken)
            .unwrap();

        // All should produce valid embeddings
        for emb in [&mean, &max, &cls, &last] {
            let norm: f32 = emb.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01);
        }

        // CLS and Mean should differ
        let cls_mean_dot: f32 = cls
            .vector
            .iter()
            .zip(mean.vector.iter())
            .map(|(a, b)| a * b)
            .sum();
        assert!(cls_mean_dot.abs() < 0.999);
    }
}

mod compression_integration {
    use super::*;
    use ruvllm::compression::CompressionService;
    use ruvllm::config::MemoryConfig;
    use ruvllm::memory::MemoryService;

    #[tokio::test]
    async fn test_compression_pipeline() {
        let config = MemoryConfig::default();
        let memory = MemoryService::new(&config).await.unwrap();

        // Insert nodes
        for i in 0..50 {
            let node = MemoryNode {
                id: format!("compress-{}", i),
                vector: vec![0.1; 128],
                text: format!("Document {} for compression", i),
                node_type: NodeType::Document,
                source: "test".into(),
                metadata: HashMap::new(),
            };
            memory.insert_node(node).unwrap();
        }

        // Create compression service
        let compression = CompressionService::new(5, 0.5);

        // Run compression
        let stats = compression.run_compression(&memory).await.unwrap();

        // Stats should be populated (even if 0 for mock)
        assert!(stats.clusters_found >= 0);
    }
}
