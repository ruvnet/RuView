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
//! Integration tests for mistral-rs backend
//!
//! Tests the mistral-rs backend integration including:
//! - Backend creation and configuration
//! - PagedAttention integration
//! - X-LoRA adapter management
//! - ISQ (In-Situ Quantization) configuration
//! - Model loading and generation (requires model files)
//!
//! ## Running Tests
//!
//! ```bash
//! # Run basic tests (no model required)
//! cargo test --features mistral-rs mistral_backend
//!
//! # Run all tests including model-dependent ones
//! cargo test --features mistral-rs mistral_backend -- --include-ignored
//!
//! # Run with Metal acceleration
//! cargo test --features mistral-rs-metal mistral_backend
//! ```

#![cfg(feature = "mistral-rs")]

use ruvllm::backends::mistral_backend::{
    IsqConfig, IsqMethod, MistralBackend, MistralBackendConfig, PagedAttentionConfigExt,
    XLoraConfig, XLoraManager, XLoraMixingMode,
};
use ruvllm::backends::{
    DType, DeviceType, GenerateParams, LlmBackend, ModelArchitecture, ModelConfig, Quantization,
};
use std::path::Path;

// ============================================================================
// Backend Creation Tests
// ============================================================================

#[test]
fn test_mistral_backend_creation() {
    let backend = MistralBackend::new().unwrap();
    assert!(!backend.is_model_loaded());
    assert!(backend.model_info().is_none());
}

#[test]
fn test_mistral_backend_default() {
    let backend = MistralBackend::default();
    assert!(!backend.is_model_loaded());
}

#[test]
fn test_mistral_backend_for_metal() {
    let result = MistralBackend::for_metal();
    assert!(result.is_ok());
    let backend = result.unwrap();
    assert!(!backend.is_model_loaded());
}

#[test]
fn test_mistral_backend_for_cuda() {
    let result = MistralBackend::for_cuda(0);
    assert!(result.is_ok());
    let backend = result.unwrap();
    assert!(!backend.is_model_loaded());
}

#[test]
fn test_mistral_backend_with_custom_config() {
    let config = MistralBackendConfig::default()
        .with_max_seq_len(16384)
        .with_max_batch_size(64);

    let backend = MistralBackend::with_config(config).unwrap();
    assert!(!backend.is_model_loaded());
}

// ============================================================================
// Configuration Builder Tests
// ============================================================================

#[test]
fn test_mistral_config_builder() {
    let config = MistralBackendConfig::default()
        .with_paged_attention(16, 4096)
        .with_xlora_adapters(vec!["code", "chat"])
        .with_isq(4);

    assert!(config.paged_attention.is_some());
    assert!(config.xlora.is_some());
    assert!(config.isq.is_some());
}

#[test]
fn test_mistral_config_paged_attention() {
    let config = MistralBackendConfig::default().with_paged_attention(32, 8192);

    let pa = config.paged_attention.unwrap();
    assert_eq!(pa.block_size, 32);
    assert_eq!(pa.max_pages, 8192);
    assert!(pa.enable_prefix_caching);
    assert!((pa.gpu_memory_fraction - 0.9).abs() < f32::EPSILON);
}

#[test]
fn test_mistral_config_xlora() {
    let config = MistralBackendConfig::default().with_xlora_adapters(vec!["code", "chat", "math"]);

    let xlora = config.xlora.unwrap();
    assert_eq!(xlora.adapter_names.len(), 3);
    assert!(xlora.adapter_names.contains(&"code".to_string()));
    assert!(xlora.adapter_names.contains(&"chat".to_string()));
    assert!(xlora.adapter_names.contains(&"math".to_string()));
}

#[test]
fn test_mistral_config_isq() {
    let config = MistralBackendConfig::default().with_isq(4);

    let isq = config.isq.unwrap();
    assert_eq!(isq.bits, 4);
    assert!(matches!(isq.method, IsqMethod::AWQ));
    assert!(!isq.symmetric);
    assert!(isq.per_channel);
}

#[test]
fn test_mistral_config_chained() {
    let config = MistralBackendConfig::default()
        .with_paged_attention(16, 4096)
        .with_xlora_adapters(vec!["adapter1", "adapter2"])
        .with_isq(8)
        .with_max_seq_len(32768)
        .with_max_batch_size(128);

    assert!(config.paged_attention.is_some());
    assert!(config.xlora.is_some());
    assert!(config.isq.is_some());
    assert_eq!(config.max_seq_len, 32768);
    assert_eq!(config.max_batch_size, 128);
}

#[test]
fn test_mistral_config_for_metal() {
    let config = MistralBackendConfig::for_metal();

    assert!(matches!(config.device, DeviceType::Metal));
    assert!(matches!(config.dtype, DType::F16));
    assert!(config.use_flash_attn);
}

#[test]
fn test_mistral_config_for_cuda() {
    let config = MistralBackendConfig::for_cuda(1);

    if let DeviceType::Cuda(id) = config.device {
        assert_eq!(id, 1);
    } else {
        panic!("Expected CUDA device type");
    }
    assert!(matches!(config.dtype, DType::F16));
    assert!(config.use_flash_attn);
}

// ============================================================================
// PagedAttention Configuration Tests
// ============================================================================

#[test]
fn test_paged_attention_config_default() {
    let config = PagedAttentionConfigExt::default();

    assert_eq!(config.block_size, 16);
    assert_eq!(config.max_pages, 4096);
    assert!((config.gpu_memory_fraction - 0.9).abs() < f32::EPSILON);
    assert!(config.enable_prefix_caching);
    assert!((config.recomputation_threshold - 0.1).abs() < f32::EPSILON);
}

#[test]
fn test_paged_attention_stats() {
    let backend = MistralBackend::new().unwrap();
    let stats = backend.paged_attention_stats();

    // Default config enables PagedAttention
    assert!(stats.is_some());
    let stats = stats.unwrap();
    assert!(stats.total_blocks > 0);
    assert_eq!(stats.active_sequences, 0);
}

#[test]
fn test_paged_attention_disabled() {
    let mut config = MistralBackendConfig::default();
    config.paged_attention = None;

    let backend = MistralBackend::with_config(config).unwrap();
    let stats = backend.paged_attention_stats();

    assert!(stats.is_none());
}

// ============================================================================
// X-LoRA Manager Tests
// ============================================================================

#[test]
fn test_xlora_manager_creation() {
    let xlora_config = XLoraConfig {
        adapter_names: vec!["test".to_string()],
        top_k: 1,
        ..Default::default()
    };

    let manager = XLoraManager::new(xlora_config);
    let stats = manager.stats();

    assert_eq!(stats.loaded_adapters, 0);
    assert_eq!(stats.forward_count, 0);
}

#[test]
fn test_xlora_manager_routing() {
    let xlora_config = XLoraConfig {
        adapter_names: vec!["code".to_string(), "chat".to_string()],
        top_k: 2,
        use_learned_routing: false,
        ..Default::default()
    };

    let manager = XLoraManager::new(xlora_config);

    // Route without adapters - returns empty
    let routing = manager.route(&[0.1, 0.2, 0.3]);
    assert!(routing.is_empty()); // No adapters loaded

    let stats = manager.stats();
    assert_eq!(stats.forward_count, 1);
}

#[test]
fn test_xlora_config_defaults() {
    let config = XLoraConfig::default();

    assert!(config.adapter_names.is_empty());
    assert!(config.base_adapter.is_none());
    assert!(config.adapter_scales.is_none());
    assert_eq!(config.router_hidden_dim, 64);
    assert_eq!(config.router_layers, 2);
    assert_eq!(config.top_k, 2);
    assert!((config.temperature - 1.0).abs() < f32::EPSILON);
    assert!(config.use_learned_routing);
    assert!(matches!(config.mixing_mode, XLoraMixingMode::Additive));
}

#[test]
fn test_xlora_mixing_modes() {
    let additive = XLoraMixingMode::Additive;
    let concat = XLoraMixingMode::Concatenate;
    let gated = XLoraMixingMode::Gated;
    let attention = XLoraMixingMode::Attention;

    assert!(matches!(additive, XLoraMixingMode::Additive));
    assert!(matches!(concat, XLoraMixingMode::Concatenate));
    assert!(matches!(gated, XLoraMixingMode::Gated));
    assert!(matches!(attention, XLoraMixingMode::Attention));
}

#[test]
fn test_xlora_stats_from_backend() {
    let config = MistralBackendConfig::default().with_xlora_adapters(vec!["code", "chat"]);
    let backend = MistralBackend::with_config(config).unwrap();

    let stats = backend.xlora_stats();
    assert!(stats.is_some());

    let stats = stats.unwrap();
    assert_eq!(stats.loaded_adapters, 0); // No adapters actually loaded from disk
    assert_eq!(stats.forward_count, 0);
}

#[test]
fn test_xlora_stats_none_when_not_configured() {
    let mut config = MistralBackendConfig::default();
    config.xlora = None;

    let backend = MistralBackend::with_config(config).unwrap();
    let stats = backend.xlora_stats();

    assert!(stats.is_none());
}

// ============================================================================
// ISQ Configuration Tests
// ============================================================================

#[test]
fn test_isq_config_defaults() {
    let config = IsqConfig::default();

    assert_eq!(config.bits, 4);
    assert!(matches!(config.method, IsqMethod::AWQ));
    assert!(!config.symmetric);
    assert!(config.per_channel);
    assert_eq!(config.calibration_samples, 128);
}

#[test]
fn test_isq_methods() {
    let awq = IsqMethod::AWQ;
    let gptq = IsqMethod::GPTQ;
    let rtn = IsqMethod::RTN;
    let smooth = IsqMethod::SmoothQuant;

    assert!(matches!(awq, IsqMethod::AWQ));
    assert!(matches!(gptq, IsqMethod::GPTQ));
    assert!(matches!(rtn, IsqMethod::RTN));
    assert!(matches!(smooth, IsqMethod::SmoothQuant));
}

#[test]
fn test_isq_with_different_bits() {
    for bits in [2, 4, 8] {
        let config = MistralBackendConfig::default().with_isq(bits);
        let isq = config.isq.unwrap();
        assert_eq!(isq.bits, bits);
    }
}

// ============================================================================
// Backend Operation Tests (Without Model)
// ============================================================================

#[test]
fn test_generate_requires_loaded_model() {
    let backend = MistralBackend::new().unwrap();

    let result = backend.generate("Hello", GenerateParams::default());
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("No model loaded"));
}

#[test]
fn test_generate_stream_requires_loaded_model() {
    let backend = MistralBackend::new().unwrap();

    let result = backend.generate_stream("Hello", GenerateParams::default());
    assert!(result.is_err());
}

#[test]
fn test_embeddings_require_loaded_model() {
    let backend = MistralBackend::new().unwrap();

    let result = backend.get_embeddings("Test text");
    assert!(result.is_err());
}

#[test]
fn test_tokenizer_none_before_load() {
    let backend = MistralBackend::new().unwrap();
    assert!(backend.tokenizer().is_none());
}

#[test]
fn test_model_info_none_before_load() {
    let backend = MistralBackend::new().unwrap();
    assert!(backend.model_info().is_none());
}

#[test]
fn test_unload_model_when_not_loaded() {
    let mut backend = MistralBackend::new().unwrap();

    // Should not panic when called on unloaded backend
    backend.unload_model();
    assert!(!backend.is_model_loaded());
}

#[test]
fn test_xlora_adapter_operations_require_config() {
    let mut config = MistralBackendConfig::default();
    config.xlora = None;

    let backend = MistralBackend::with_config(config).unwrap();

    // Loading adapter should fail without X-LoRA configured
    let result = backend.load_xlora_adapter("test", Path::new("/nonexistent"));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("X-LoRA not configured"));

    // Setting adapters should fail
    let result = backend.set_xlora_adapters(vec![("test", 1.0)]);
    assert!(result.is_err());
}

#[test]
fn test_isq_requires_loaded_model() {
    let config = MistralBackendConfig::default().with_isq(4);
    let mut backend = MistralBackend::with_config(config).unwrap();

    let result = backend.apply_isq();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No model loaded"));
}

// ============================================================================
// Model Loading Tests (Requires Model Files - Ignored by Default)
// ============================================================================

#[test]
#[ignore = "Requires model file - run with --include-ignored"]
fn test_model_loading() {
    let mut backend = MistralBackend::for_metal().unwrap();

    // Note: Replace with actual model path for testing
    let result = backend.load_model(
        "models/test-model.gguf",
        ModelConfig {
            architecture: ModelArchitecture::Mistral,
            device: DeviceType::Metal,
            ..Default::default()
        },
    );

    if result.is_ok() {
        assert!(backend.is_model_loaded());
        assert!(backend.model_info().is_some());
        assert!(backend.tokenizer().is_some());
    }
}

#[test]
#[ignore = "Requires model file - run with --include-ignored"]
fn test_generation() {
    let mut backend = MistralBackend::new().unwrap();

    let load_result = backend.load_model("models/test-model.gguf", ModelConfig::default());
    if load_result.is_err() {
        return; // Skip if model not available
    }

    let output = backend.generate(
        "Hello",
        GenerateParams {
            max_tokens: 10,
            temperature: 0.7,
            ..Default::default()
        },
    );

    match output {
        Ok(text) => {
            assert!(!text.is_empty());
        }
        Err(e) => {
            panic!("Generation failed: {}", e);
        }
    }
}

#[test]
#[ignore = "Requires model file - run with --include-ignored"]
fn test_streaming_generation() {
    let mut backend = MistralBackend::new().unwrap();

    let load_result = backend.load_model("models/test-model.gguf", ModelConfig::default());
    if load_result.is_err() {
        return;
    }

    let stream = backend.generate_stream("Hello", GenerateParams::default());
    match stream {
        Ok(stream) => {
            let tokens: Vec<_> = stream.collect();
            assert!(!tokens.is_empty());
        }
        Err(e) => {
            panic!("Streaming generation failed: {}", e);
        }
    }
}

#[test]
#[ignore = "Requires model file - run with --include-ignored"]
fn test_embeddings_extraction() {
    let mut backend = MistralBackend::new().unwrap();

    let load_result = backend.load_model("models/test-model.gguf", ModelConfig::default());
    if load_result.is_err() {
        return;
    }

    let embeddings = backend.get_embeddings("Test text for embedding");
    match embeddings {
        Ok(emb) => {
            assert!(!emb.is_empty());
            assert!(emb.iter().all(|&v| v.is_finite()));
        }
        Err(e) => {
            panic!("Embedding extraction failed: {}", e);
        }
    }
}

#[test]
#[ignore = "Requires model file - run with --include-ignored"]
fn test_model_unload_and_reload() {
    let mut backend = MistralBackend::new().unwrap();

    // Load model
    let load_result = backend.load_model("models/test-model.gguf", ModelConfig::default());
    if load_result.is_err() {
        return;
    }
    assert!(backend.is_model_loaded());

    // Unload
    backend.unload_model();
    assert!(!backend.is_model_loaded());
    assert!(backend.model_info().is_none());

    // Reload
    let reload_result = backend.load_model("models/test-model.gguf", ModelConfig::default());
    if reload_result.is_ok() {
        assert!(backend.is_model_loaded());
    }
}

// ============================================================================
// Integration Tests with PagedAttention
// ============================================================================

#[test]
fn test_backend_paged_attention_integration() {
    let config = MistralBackendConfig::default().with_paged_attention(16, 4096);
    let backend = MistralBackend::with_config(config).unwrap();

    // Verify PagedAttention is configured
    let stats = backend.paged_attention_stats().unwrap();
    assert!(stats.total_blocks > 0);
    assert!(stats.free_blocks > 0);
    assert_eq!(stats.active_sequences, 0);
}

#[test]
fn test_backend_xlora_integration() {
    let config = MistralBackendConfig::default().with_xlora_adapters(vec!["code", "math", "chat"]);
    let backend = MistralBackend::with_config(config).unwrap();

    // Verify X-LoRA is configured
    let stats = backend.xlora_stats().unwrap();
    assert_eq!(stats.loaded_adapters, 0); // None loaded yet
    assert!(stats.adapter_usage.is_empty());
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_config_serialization() {
    let config = MistralBackendConfig::default()
        .with_paged_attention(32, 8192)
        .with_xlora_adapters(vec!["test"])
        .with_isq(4);

    // Test serialization roundtrip
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: MistralBackendConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.max_seq_len, config.max_seq_len);
    assert_eq!(deserialized.max_batch_size, config.max_batch_size);
    assert!(deserialized.paged_attention.is_some());
    assert!(deserialized.xlora.is_some());
    assert!(deserialized.isq.is_some());
}

#[test]
fn test_paged_attention_config_serialization() {
    let config = PagedAttentionConfigExt::default();

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: PagedAttentionConfigExt = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.block_size, config.block_size);
    assert_eq!(deserialized.max_pages, config.max_pages);
    assert_eq!(
        deserialized.enable_prefix_caching,
        config.enable_prefix_caching
    );
}

#[test]
fn test_xlora_config_serialization() {
    let config = XLoraConfig {
        adapter_names: vec!["a".to_string(), "b".to_string()],
        top_k: 3,
        temperature: 0.5,
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: XLoraConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.adapter_names.len(), 2);
    assert_eq!(deserialized.top_k, 3);
    assert!((deserialized.temperature - 0.5).abs() < f32::EPSILON);
}

#[test]
fn test_isq_config_serialization() {
    let config = IsqConfig {
        bits: 8,
        method: IsqMethod::GPTQ,
        symmetric: true,
        per_channel: false,
        calibration_samples: 256,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: IsqConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.bits, 8);
    assert!(matches!(deserialized.method, IsqMethod::GPTQ));
    assert!(deserialized.symmetric);
    assert!(!deserialized.per_channel);
    assert_eq!(deserialized.calibration_samples, 256);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_xlora_adapters() {
    let config = MistralBackendConfig::default().with_xlora_adapters(vec![]);

    let xlora = config.xlora.unwrap();
    assert!(xlora.adapter_names.is_empty());
}

#[test]
fn test_large_page_config() {
    let config = MistralBackendConfig::default().with_paged_attention(256, 65536);

    let pa = config.paged_attention.unwrap();
    assert_eq!(pa.block_size, 256);
    assert_eq!(pa.max_pages, 65536);
}

#[test]
fn test_multiple_backend_instances() {
    let backend1 = MistralBackend::new().unwrap();
    let backend2 = MistralBackend::for_metal().unwrap();
    let backend3 = MistralBackend::for_cuda(0).unwrap();

    assert!(!backend1.is_model_loaded());
    assert!(!backend2.is_model_loaded());
    assert!(!backend3.is_model_loaded());
}

#[test]
fn test_generate_params_integration() {
    let params = GenerateParams {
        max_tokens: 256,
        temperature: 0.8,
        top_p: 0.95,
        top_k: 50,
        repetition_penalty: 1.1,
        frequency_penalty: 0.1,
        presence_penalty: 0.1,
        stop_sequences: vec!["STOP".to_string(), "\n\n".to_string()],
        seed: Some(12345),
    };

    assert_eq!(params.max_tokens, 256);
    assert!((params.temperature - 0.8).abs() < f32::EPSILON);
    assert_eq!(params.stop_sequences.len(), 2);
    assert_eq!(params.seed, Some(12345));
}
