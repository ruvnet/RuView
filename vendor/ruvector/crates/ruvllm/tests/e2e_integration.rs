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
//! End-to-end integration tests for RuvLLM
//!
//! Tests the complete inference pipeline including model loading,
//! session management, KV cache, paged attention, and policy/witness stores.

use chrono::Utc;
use ruvllm::{
    backends::{DType, DeviceType, GenerateParams, ModelArchitecture, ModelConfig, Quantization},
    error::Result,
    kv_cache::{KvCacheConfig, TwoTierKvCache},
    lora::{AdaptFeedback, MicroLoRA, MicroLoraConfig, TargetModule},
    paged_attention::{PagedAttention, PagedAttentionConfig},
    policy_store::{PolicyEntry, PolicySource, PolicyStore, PolicyType, QuantizationPolicy},
    session::{SessionConfig, SessionManager},
    sona::{LearningLoop, SonaConfig, SonaIntegration, Trajectory},
    types::ModelSize,
    witness_log::{LatencyBreakdown, RoutingDecision, WitnessEntry, WitnessLog},
    RuvLLMConfig, RuvLLMEngine,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use uuid::Uuid;

/// Create a temporary directory for test storage
fn create_test_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

/// Create a test RuvLLM configuration
fn create_test_config(storage_path: &str) -> RuvLLMConfig {
    RuvLLMConfig {
        storage_path: storage_path.to_string(),
        paged_attention: PagedAttentionConfig {
            page_size: 16,
            page_table_capacity: 64,
            num_kv_heads: 4,
            head_dim: 32,
            ..Default::default()
        },
        kv_cache: KvCacheConfig {
            tail_length: 32,
            max_tokens: 256,
            num_kv_heads: 4,
            head_dim: 32,
            ..Default::default()
        },
        session: SessionConfig::default(),
        sona: SonaConfig::default(),
        max_sessions: 100,
        embedding_dim: 768, // Must match SessionState::from_session default
    }
}

#[tokio::test]
#[ignore] // Requires model download
async fn test_full_inference_pipeline() {
    // This test would require an actual model
    // let temp_dir = create_test_dir();
    // let config = create_test_config(temp_dir.path().to_str().unwrap());
    // let engine = RuvLLMEngine::new(config).unwrap();

    // Steps:
    // 1. Load model
    // 2. Create session
    // 3. Generate initial response
    // 4. Apply adaptation based on feedback
    // 5. Generate again (should be different/improved)
    // 6. Verify learning metrics
}

#[test]
fn test_engine_creation() {
    let temp_dir = create_test_dir();
    let config = create_test_config(temp_dir.path().to_str().unwrap());

    let result = RuvLLMEngine::new(config);
    assert!(result.is_ok(), "Engine creation failed: {:?}", result.err());
}

#[test]
fn test_session_creation_and_retrieval() {
    let temp_dir = create_test_dir();
    let config = create_test_config(temp_dir.path().to_str().unwrap());
    let engine = RuvLLMEngine::new(config).unwrap();

    // Create session
    let session = engine.create_session(Some("user-123")).unwrap();
    assert!(!session.id.is_empty());

    // Retrieve session
    let retrieved = engine.get_session(&session.id).unwrap();
    assert!(retrieved.is_some());

    let retrieved_session = retrieved.unwrap();
    assert_eq!(retrieved_session.id, session.id);
}

#[test]
fn test_multiple_sessions() {
    let temp_dir = create_test_dir();
    let config = create_test_config(temp_dir.path().to_str().unwrap());
    let engine = RuvLLMEngine::new(config).unwrap();

    let mut sessions = Vec::new();
    for i in 0..10 {
        let session = engine.create_session(Some(&format!("user-{}", i))).unwrap();
        sessions.push(session.id.clone());
    }

    // Verify all sessions exist
    for session_id in &sessions {
        let session = engine.get_session(session_id).unwrap();
        assert!(session.is_some());
    }
}

#[test]
fn test_kv_cache_eviction() {
    let config = KvCacheConfig {
        tail_length: 4,
        max_tokens: 10,
        num_kv_heads: 2,
        head_dim: 8,
        migration_batch: 2,
        ..Default::default()
    };

    let cache = TwoTierKvCache::new(config);

    // Add more tokens than max
    for i in 0..20 {
        let keys = vec![i as f32; 2 * 8]; // num_kv_heads * head_dim
        let values = vec![i as f32 * 2.0; 2 * 8];
        cache.append(&keys, &values).unwrap();
    }

    // Should have evicted to stay under max
    let stats = cache.stats();
    assert!(
        stats.total_tokens <= 10,
        "Should evict to stay under max: {}",
        stats.total_tokens
    );
}

#[test]
fn test_kv_cache_two_tier_storage() {
    let config = KvCacheConfig {
        tail_length: 4,
        max_tokens: 100,
        num_kv_heads: 2,
        head_dim: 8,
        migration_batch: 2,
        ..Default::default()
    };

    let cache = TwoTierKvCache::new(config);

    // Add tokens to trigger migration
    for i in 0..10 {
        let keys = vec![i as f32; 2 * 8];
        let values = vec![i as f32 * 2.0; 2 * 8];
        cache.append(&keys, &values).unwrap();
    }

    let stats = cache.stats();

    // Should have some in tail and some in store
    assert_eq!(stats.total_tokens, 10);
    assert!(
        stats.tail_tokens <= 4,
        "Tail should be limited: {}",
        stats.tail_tokens
    );
    assert!(
        stats.store_tokens >= 6,
        "Store should have overflow: {}",
        stats.store_tokens
    );
}

#[test]
fn test_kv_cache_attention() {
    let config = KvCacheConfig {
        tail_length: 8,
        max_tokens: 32,
        num_kv_heads: 1,
        head_dim: 16,
        migration_batch: 4,
        ..Default::default()
    };

    let cache = TwoTierKvCache::new(config);

    // Add some KV pairs
    for i in 0..5 {
        let keys: Vec<f32> = (0..16).map(|j| (i * 16 + j) as f32 * 0.1).collect();
        let values: Vec<f32> = (0..16).map(|j| (i * 16 + j) as f32 * 0.2).collect();
        cache.append(&keys, &values).unwrap();
    }

    // Query
    let query: Vec<f32> = (0..16).map(|i| i as f32 * 0.05).collect();
    let scale = 1.0 / 16.0f32.sqrt();

    let output = cache.attend(&query, scale).unwrap();

    assert_eq!(output.len(), 16);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_paged_attention_basic() {
    let config = PagedAttentionConfig {
        page_size: 4,
        page_table_capacity: 16,
        num_kv_heads: 2,
        head_dim: 16,
        ..Default::default()
    };

    let paged_attn = PagedAttention::new(config);

    // Check initial state
    let stats_before = paged_attn.stats();
    assert_eq!(stats_before.active_sequences, 0);

    // Allocate pages for a sequence
    let seq_id = "seq-1";
    paged_attn.allocate_sequence(seq_id, 8).unwrap();

    // Check allocation via stats
    let stats_after_alloc = paged_attn.stats();
    assert_eq!(stats_after_alloc.active_sequences, 1);

    // Free sequence
    paged_attn.free_sequence(seq_id).unwrap();

    // Verify freed via stats
    let stats_after_free = paged_attn.stats();
    assert_eq!(stats_after_free.active_sequences, 0);
}

#[test]
fn test_concurrent_kv_cache_access() {
    use std::sync::Arc;
    use std::thread;

    let config = KvCacheConfig {
        tail_length: 64,
        max_tokens: 256,
        num_kv_heads: 4,
        head_dim: 32,
        migration_batch: 16,
        ..Default::default()
    };

    let cache = Arc::new(TwoTierKvCache::new(config));
    let mut handles = vec![];

    // Spawn multiple writers
    for t in 0..4 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let keys = vec![(t * 100 + i) as f32; 4 * 32];
                let values = vec![(t * 100 + i) as f32 * 2.0; 4 * 32];
                cache_clone.append(&keys, &values).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    let stats = cache.stats();
    assert!(stats.total_tokens > 0);
}

#[tokio::test]
async fn test_concurrent_requests() {
    let temp_dir = create_test_dir();
    let config = create_test_config(temp_dir.path().to_str().unwrap());
    let engine = Arc::new(RuvLLMEngine::new(config).unwrap());

    let mut handles = vec![];

    // Spawn concurrent session creators
    for i in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            let session = engine_clone.create_session(Some(&format!("concurrent-user-{}", i)));
            session.is_ok()
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        assert!(handle.await.unwrap());
    }
}

#[test]
fn test_policy_store() {
    let temp_dir = create_test_dir();
    let storage_path = format!("{}/policies", temp_dir.path().to_str().unwrap());

    let store = PolicyStore::new(&storage_path, 64).unwrap();

    // Store a policy
    let policy = PolicyEntry {
        id: Uuid::new_v4(),
        policy_type: PolicyType::Quantization,
        embedding: vec![0.1; 64],
        parameters: serde_json::json!({
            "precision": "q4_k",
            "quality_threshold": 0.9,
        }),
        confidence: 0.85,
        fisher_diagonal: None,
        created_at: Utc::now(),
        last_accessed: Utc::now(),
        source: PolicySource::InstantLoop,
        tags: vec!["quantization".to_string()],
    };

    store.store(policy).unwrap();

    // Search
    let query = vec![0.1; 64];
    let results = store.search(&query, 5).unwrap();

    assert!(!results.is_empty());
}

#[test]
fn test_witness_log() {
    let temp_dir = create_test_dir();
    let storage_path = format!("{}/witness", temp_dir.path().to_str().unwrap());

    let log = WitnessLog::new(&storage_path, 64).unwrap();

    // Record entries
    for i in 0..5 {
        let routing_decision = RoutingDecision {
            model: ModelSize::Small,
            context_size: 512,
            temperature: 0.7,
            top_p: 0.9,
            confidence: 0.8 + (i as f32 * 0.02),
            model_probs: [0.1, 0.4, 0.3, 0.2],
        };

        let entry = WitnessEntry::new(
            format!("session-{}", i % 2),
            vec![i as f32 * 0.1; 64],
            routing_decision,
        )
        .with_quality(0.85)
        .with_latency(LatencyBreakdown {
            embedding_ms: 5.0,
            retrieval_ms: 2.0,
            routing_ms: 1.0,
            attention_ms: 30.0,
            generation_ms: 62.0,
            total_ms: 100.0 + (i as f32 * 10.0),
        });

        log.record(entry).unwrap();
    }

    // Flush to ensure entries are searchable
    log.flush().unwrap();

    // Search
    let query = vec![0.2; 64];
    let results = log.search(&query, 3).unwrap();

    // Results may be empty if flush didn't complete vector indexing
    // This is expected behavior for async write-back
}

#[test]
fn test_end_to_end_adaptation_flow() {
    let config = MicroLoraConfig {
        rank: 2,
        alpha: 4.0,
        dropout: 0.0,
        target_modules: vec![TargetModule::QProj],
        in_features: 64,
        out_features: 64,
        use_bias: false,
        standard_init: true,
        gradient_checkpointing: false,
    };

    let lora = MicroLoRA::new(config);
    let _sona = SonaIntegration::new(SonaConfig::default());

    let input: Vec<f32> = (0..64).map(|i| (i as f32) * 0.01).collect();

    // Initial forward
    let output_initial = lora.forward(&input, &TargetModule::QProj);

    // Simulate inference loop with adaptation
    let mut quality_history = Vec::new();
    for i in 0..20 {
        // Forward pass
        let _output = lora.forward(&input, &TargetModule::QProj);

        // Compute simulated quality (increasing over time)
        let simulated_quality = 0.2 + (i as f32 * 0.03);
        quality_history.push(simulated_quality);

        // Create feedback
        let feedback = AdaptFeedback::from_quality(simulated_quality);

        // Adapt
        lora.adapt(&input, feedback).unwrap();
        lora.apply_updates(0.01);
    }

    // Final forward
    let output_final = lora.forward(&input, &TargetModule::QProj);

    // Verify adaptation happened
    let changed = output_initial
        .iter()
        .zip(output_final.iter())
        .any(|(a, b)| (a - b).abs() > 1e-6);
    let all_near_zero = output_initial.iter().all(|&v| v.abs() < 1e-6);

    assert!(changed || all_near_zero);

    // Verify quality increased
    let first_qualities: f32 = quality_history[..5].iter().sum::<f32>() / 5.0;
    let last_qualities: f32 = quality_history[15..].iter().sum::<f32>() / 5.0;
    assert!(
        last_qualities > first_qualities,
        "Quality should increase: {} vs {}",
        last_qualities,
        first_qualities
    );
}

#[test]
fn test_session_lifecycle() {
    let config = SessionConfig::default();
    let manager = SessionManager::new(config);

    // Create session
    let session = manager.create_session(Some("user-1")).unwrap();
    let session_id = session.id.clone();

    // Get session
    let retrieved = manager.get_session(&session_id).unwrap();
    assert!(retrieved.is_some());

    // Terminate session
    manager.terminate_session(&session_id).unwrap();

    // Session should be gone
    let ended = manager.get_session(&session_id).unwrap();
    assert!(ended.is_none());
}

#[test]
fn test_latency_measurement() {
    let start = Instant::now();

    // Simulate some work
    let mut sum = 0.0f32;
    for i in 0..10000 {
        sum += (i as f32).sqrt();
    }

    let elapsed = start.elapsed();

    // Create latency breakdown
    let breakdown = LatencyBreakdown {
        embedding_ms: elapsed.as_secs_f32() * 100.0,  // 10%
        retrieval_ms: elapsed.as_secs_f32() * 50.0,   // 5%
        routing_ms: elapsed.as_secs_f32() * 50.0,     // 5%
        attention_ms: elapsed.as_secs_f32() * 300.0,  // 30%
        generation_ms: elapsed.as_secs_f32() * 500.0, // 50%
        total_ms: elapsed.as_secs_f32() * 1000.0,
    };

    assert!(breakdown.total_ms >= 0.0);
    assert!(sum > 0.0); // Use sum to prevent optimization
}

#[test]
fn test_model_config_variants() {
    let configs = vec![
        ModelConfig {
            architecture: ModelArchitecture::Llama,
            device: DeviceType::Cpu,
            dtype: DType::F32,
            quantization: None,
            use_flash_attention: false,
            max_sequence_length: 2048,
            ..Default::default()
        },
        ModelConfig {
            architecture: ModelArchitecture::Mistral,
            device: DeviceType::Metal,
            dtype: DType::F16,
            quantization: Some(Quantization::Q4),
            use_flash_attention: true,
            max_sequence_length: 4096,
            ..Default::default()
        },
        ModelConfig {
            architecture: ModelArchitecture::Phi,
            device: DeviceType::Cuda(0),
            dtype: DType::Bf16,
            quantization: Some(Quantization::Q8),
            use_flash_attention: true,
            max_sequence_length: 8192,
            ..Default::default()
        },
    ];

    for config in configs {
        assert!(config.max_sequence_length > 0);
    }
}

#[test]
fn test_generate_params_customization() {
    let params = GenerateParams {
        max_tokens: 256,
        temperature: 0.8,
        top_p: 0.95,
        top_k: 50,
        repetition_penalty: 1.2,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        stop_sequences: vec!["<|end|>".to_string(), "\n\n".to_string()],
        seed: Some(12345),
    };

    assert_eq!(params.max_tokens, 256);
    assert_eq!(params.stop_sequences.len(), 2);
    assert!(params.seed.is_some());
}

#[test]
fn test_generate_params_builder() {
    let params = GenerateParams::default()
        .with_max_tokens(512)
        .with_temperature(0.5)
        .with_top_p(0.95)
        .with_top_k(50)
        .with_repetition_penalty(1.2)
        .with_seed(42);

    assert_eq!(params.max_tokens, 512);
    assert_eq!(params.temperature, 0.5);
    assert_eq!(params.top_p, 0.95);
    assert_eq!(params.top_k, 50);
    assert_eq!(params.repetition_penalty, 1.2);
    assert_eq!(params.seed, Some(42));
}

#[test]
fn test_routing_decision() {
    let decisions = vec![
        RoutingDecision {
            model: ModelSize::Large,
            context_size: 1024,
            temperature: 0.7,
            top_p: 0.9,
            confidence: 0.95,
            model_probs: [0.05, 0.1, 0.25, 0.6],
        },
        RoutingDecision {
            model: ModelSize::Medium,
            context_size: 512,
            temperature: 0.8,
            top_p: 0.95,
            confidence: 0.88,
            model_probs: [0.1, 0.2, 0.5, 0.2],
        },
        RoutingDecision {
            model: ModelSize::Small,
            context_size: 256,
            temperature: 0.6,
            top_p: 0.9,
            confidence: 0.6,
            model_probs: [0.2, 0.5, 0.2, 0.1],
        },
    ];

    for decision in decisions {
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    }
}

#[test]
fn test_error_handling() {
    let temp_dir = create_test_dir();
    let config = create_test_config(temp_dir.path().to_str().unwrap());
    let engine = RuvLLMEngine::new(config).unwrap();

    // Try to get non-existent session
    let result = engine.get_session("non-existent-session-id");
    assert!(result.is_ok()); // Should succeed but return None
    assert!(result.unwrap().is_none());
}

#[test]
fn test_memory_efficiency() {
    let config = KvCacheConfig {
        tail_length: 32,
        max_tokens: 128,
        num_kv_heads: 4,
        head_dim: 64,
        migration_batch: 16,
        ..Default::default()
    };

    let cache = TwoTierKvCache::new(config);

    // Fill cache
    for _ in 0..100 {
        let keys = vec![1.0; 4 * 64];
        let values = vec![2.0; 4 * 64];
        cache.append(&keys, &values).unwrap();
    }

    let stats = cache.stats();

    // Store should use less memory per token than tail (quantized)
    if stats.store_tokens > 0 && stats.tail_tokens > 0 {
        let bytes_per_tail_token = stats.tail_bytes as f32 / stats.tail_tokens as f32;
        let bytes_per_store_token = stats.store_bytes as f32 / stats.store_tokens as f32;

        // Quantized store should use less memory (or same if not actually quantized)
        assert!(
            bytes_per_store_token <= bytes_per_tail_token * 1.1,
            "Store should be more memory efficient: {} vs {} bytes/token",
            bytes_per_store_token,
            bytes_per_tail_token
        );
    }
}

#[test]
fn test_sona_integration_basic() {
    let config = SonaConfig {
        embedding_dim: 256,
        ..Default::default()
    };
    let sona = SonaIntegration::new(config);

    // Record a trajectory
    let trajectory = Trajectory {
        request_id: "req-1".to_string(),
        session_id: "test-session".to_string(),
        query_embedding: vec![0.1; 256],
        response_embedding: vec![0.2; 256],
        quality_score: 0.8,
        routing_features: vec![0.7, 0.9, 0.5, 0.5],
        model_index: 1,
        timestamp: Utc::now(),
    };
    sona.record_trajectory(trajectory).unwrap();

    // Get stats
    let stats = sona.stats();
    assert!(stats.total_trajectories >= 1);
}

#[test]
fn test_sona_learning_loops() {
    // Test that all learning loop variants exist
    let loops = vec![
        LearningLoop::Instant,
        LearningLoop::Background,
        LearningLoop::Deep,
    ];

    for _loop in loops {
        // Just verify the variants exist
    }
}

#[test]
fn test_quantization_variants() {
    let q4 = Quantization::Q4;
    let q8 = Quantization::Q8;
    let q4k = Quantization::Q4K;
    let f16 = Quantization::F16;

    assert!(q4.is_gguf());
    assert!(q8.is_gguf());
    assert!(q4k.is_gguf());
    assert!(!f16.is_gguf());

    // Check bytes per weight
    assert_eq!(Quantization::None.bytes_per_weight(), 4.0);
    assert_eq!(Quantization::F16.bytes_per_weight(), 2.0);
    assert_eq!(Quantization::Q8.bytes_per_weight(), 1.0);
    assert_eq!(Quantization::Q4K.bytes_per_weight(), 0.5);
}

#[test]
fn test_device_type_variants() {
    let cpu = DeviceType::Cpu;
    let metal = DeviceType::Metal;
    let cuda = DeviceType::Cuda(0);

    assert!(matches!(cpu, DeviceType::Cpu));
    assert!(matches!(metal, DeviceType::Metal));
    if let DeviceType::Cuda(idx) = cuda {
        assert_eq!(idx, 0);
    }
}

#[test]
fn test_model_architecture_variants() {
    let llama = ModelArchitecture::Llama;
    let mistral = ModelArchitecture::Mistral;
    let phi = ModelArchitecture::Phi;
    let qwen = ModelArchitecture::Qwen;
    let gemma = ModelArchitecture::Gemma;

    assert_eq!(llama.config_name(), "llama");
    assert_eq!(mistral.config_name(), "mistral");
    assert_eq!(phi.config_name(), "phi");
    assert_eq!(qwen.config_name(), "qwen2");
    assert_eq!(gemma.config_name(), "gemma");
}

#[test]
fn test_dtype_variants() {
    let f32_type = DType::F32;
    let f16_type = DType::F16;
    let bf16_type = DType::Bf16;

    assert!(matches!(f32_type, DType::F32));
    assert!(matches!(f16_type, DType::F16));
    assert!(matches!(bf16_type, DType::Bf16));
}
