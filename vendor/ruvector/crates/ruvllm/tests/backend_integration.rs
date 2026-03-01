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
//! Integration tests for LLM backends
//!
//! Tests the LLM backend infrastructure including model loading,
//! text generation, streaming, and embeddings extraction.

use ruvllm::{
    backends::{
        create_backend, DType, DeviceType, GenerateParams, LlmBackend, ModelArchitecture,
        ModelConfig, ModelInfo, Quantization, SpecialTokens, TokenStream, Tokenizer,
    },
    error::Result,
};
use std::sync::Arc;

/// Mock backend for testing without requiring actual model files
#[derive(Debug)]
struct MockBackend {
    model_info: Option<ModelInfo>,
    loaded: bool,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            model_info: None,
            loaded: false,
        }
    }
}

impl LlmBackend for MockBackend {
    fn load_model(&mut self, model_id: &str, config: ModelConfig) -> Result<()> {
        self.model_info = Some(ModelInfo {
            name: model_id.to_string(),
            architecture: config.architecture,
            num_parameters: 100_000,
            vocab_size: 32000,
            hidden_size: 768,
            num_layers: 12,
            max_context_length: config.max_sequence_length,
            quantization: config.quantization,
            memory_usage: 1024 * 1024 * 100, // 100MB
        });
        self.loaded = true;
        Ok(())
    }

    fn generate(&self, prompt: &str, _params: GenerateParams) -> Result<String> {
        if !self.loaded {
            return Err(ruvllm::RuvLLMError::Backend("Model not loaded".to_string()));
        }
        Ok(format!("Response to: {}", prompt))
    }

    fn generate_stream(
        &self,
        _prompt: &str,
        _params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<ruvllm::backends::GeneratedToken>> + Send + '_>>
    {
        if !self.loaded {
            return Err(ruvllm::RuvLLMError::Backend("Model not loaded".to_string()));
        }

        let tokens = vec![
            ruvllm::backends::GeneratedToken {
                id: 1,
                text: "Hello".to_string(),
                logprob: Some(-0.5),
                is_special: false,
            },
            ruvllm::backends::GeneratedToken {
                id: 2,
                text: " world".to_string(),
                logprob: Some(-0.3),
                is_special: false,
            },
            ruvllm::backends::GeneratedToken {
                id: 3,
                text: "!".to_string(),
                logprob: Some(-0.1),
                is_special: false,
            },
        ];

        Ok(Box::new(tokens.into_iter().map(Ok)))
    }

    fn generate_stream_v2(&self, _prompt: &str, _params: GenerateParams) -> Result<TokenStream> {
        if !self.loaded {
            return Err(ruvllm::RuvLLMError::Backend("Model not loaded".to_string()));
        }
        // Return a mock stream using channel
        let (tx, stream) = TokenStream::channel();
        // Drop tx immediately since we don't need to send anything for this mock
        drop(tx);
        Ok(stream)
    }

    fn get_embeddings(&self, _text: &str) -> Result<Vec<f32>> {
        if !self.loaded {
            return Err(ruvllm::RuvLLMError::Backend("Model not loaded".to_string()));
        }
        // Return a mock embedding
        Ok(vec![0.1; 768])
    }

    fn tokenizer(&self) -> Option<&dyn Tokenizer> {
        None
    }

    fn is_model_loaded(&self) -> bool {
        self.loaded
    }

    fn model_info(&self) -> Option<ModelInfo> {
        self.model_info.clone()
    }

    fn unload_model(&mut self) {
        self.loaded = false;
        self.model_info = None;
    }
}

#[test]
fn test_mock_backend_load_model() {
    let mut backend = MockBackend::new();

    // Initially not loaded
    assert!(!backend.is_model_loaded());
    assert!(backend.model_info().is_none());

    // Load model
    let config = ModelConfig::default();
    let result = backend.load_model("test-model", config);
    assert!(result.is_ok());
    assert!(backend.is_model_loaded());
    assert!(backend.model_info().is_some());
}

#[test]
fn test_backend_generate_basic() {
    let mut backend = MockBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();

    let params = GenerateParams {
        max_tokens: 100,
        temperature: 0.7,
        top_p: 0.9,
        top_k: 40,
        repetition_penalty: 1.1,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        stop_sequences: vec![],
        seed: Some(42),
    };

    let result = backend.generate("Hello, how are you?", params);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(!output.is_empty());
    assert!(output.contains("Hello"));
}

#[test]
fn test_backend_generate_requires_loaded_model() {
    let backend = MockBackend::new();

    let params = GenerateParams::default();
    let result = backend.generate("Test prompt", params);

    assert!(result.is_err());
}

#[test]
fn test_backend_streaming() {
    let mut backend = MockBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();

    let params = GenerateParams::default();
    let stream = backend.generate_stream("Hello", params).unwrap();

    let tokens: Vec<_> = stream.collect();
    assert_eq!(tokens.len(), 3);

    let first = tokens[0].as_ref().unwrap();
    assert_eq!(first.text, "Hello");
    assert_eq!(first.id, 1);
    assert!(!first.is_special);
}

#[test]
fn test_backend_embeddings() {
    let mut backend = MockBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();

    let embedding = backend.get_embeddings("Test text for embedding").unwrap();

    assert_eq!(embedding.len(), 768);
    assert!(embedding.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_backend_model_info() {
    let mut backend = MockBackend::new();

    let config = ModelConfig {
        architecture: ModelArchitecture::Llama,
        max_sequence_length: 4096,
        quantization: Some(Quantization::Q4K),
        ..Default::default()
    };

    backend.load_model("llama-test", config).unwrap();
    let info = backend.model_info().unwrap();

    assert_eq!(info.name, "llama-test");
    assert_eq!(info.max_context_length, 4096);
    assert!(matches!(info.architecture, ModelArchitecture::Llama));
    assert!(matches!(info.quantization, Some(Quantization::Q4K)));
}

#[test]
fn test_backend_unload() {
    let mut backend = MockBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();
    assert!(backend.is_model_loaded());

    backend.unload_model();
    assert!(!backend.is_model_loaded());
    assert!(backend.model_info().is_none());

    // Should fail after unload
    let result = backend.generate("Test", GenerateParams::default());
    assert!(result.is_err());
}

#[test]
fn test_model_config() {
    let config = ModelConfig {
        architecture: ModelArchitecture::Mistral,
        device: DeviceType::Cpu,
        dtype: DType::F32,
        quantization: Some(Quantization::Q4K),
        use_flash_attention: true,
        max_sequence_length: 4096,
        num_kv_heads: Some(8),
        hidden_size: Some(4096),
        num_layers: Some(32),
        vocab_size: Some(32000),
        rope_theta: Some(10000.0),
        sliding_window: None,
    };

    assert!(matches!(config.device, DeviceType::Cpu));
    assert!(matches!(config.dtype, DType::F32));
    assert!(matches!(config.quantization, Some(Quantization::Q4K)));
    assert!(config.use_flash_attention);
    assert_eq!(config.max_sequence_length, 4096);
}

#[test]
fn test_generate_params_default() {
    let params = GenerateParams::default();

    assert!(params.max_tokens > 0);
    assert!(params.temperature > 0.0);
    assert!(params.top_p <= 1.0);
    assert!(params.top_k > 0);
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

#[test]
fn test_special_tokens() {
    let tokens = SpecialTokens {
        bos_token_id: Some(1),
        eos_token_id: Some(2),
        pad_token_id: Some(0),
        unk_token_id: Some(3),
    };

    assert_eq!(tokens.bos_token_id, Some(1));
    assert_eq!(tokens.eos_token_id, Some(2));
    assert_eq!(tokens.pad_token_id, Some(0));
    assert_eq!(tokens.unk_token_id, Some(3));
}

#[test]
fn test_create_backend() {
    // This creates a NoopBackend when candle feature is not enabled
    let backend = create_backend();

    // Without the candle feature, the backend should not be able to load models
    #[cfg(not(feature = "candle"))]
    {
        assert!(!backend.is_model_loaded());
    }
}

// Candle backend tests (only run when the feature is enabled)
#[cfg(feature = "candle")]
mod candle_tests {
    use super::*;
    use ruvllm::backends::CandleBackend;

    #[test]
    #[ignore] // Requires model download
    fn test_candle_backend_creation() {
        let backend = CandleBackend::new();
        assert!(backend.is_ok());
    }

    #[test]
    #[ignore] // Requires model download
    fn test_candle_backend_load_model() {
        let mut backend = CandleBackend::new().unwrap();
        let config = ModelConfig {
            architecture: ModelArchitecture::Phi,
            device: DeviceType::Cpu,
            ..Default::default()
        };

        // This would require an actual model file
        // let result = backend.load_model("microsoft/phi-2", config);
        // assert!(result.is_ok());
    }
}

// ========== V2 Feature Tests: Memory Pool Integration ==========

mod memory_pool_tests {
    use ruvllm::memory_pool::{
        BufferPool, BufferSize, InferenceArena, MemoryManager, MemoryManagerConfig,
        ScratchSpaceManager,
    };

    /// Test memory pool integration with streaming generation
    #[test]
    fn test_memory_pool_integration() {
        let pool = BufferPool::new();

        // Pre-warm the pool
        pool.prewarm_all(4).expect("prewarm failed");

        // Simulate multiple generation steps
        for step in 0..10 {
            // Acquire buffers for KV cache
            let kv_buffer = pool.acquire(BufferSize::KB64).expect("acquire failed");
            assert_eq!(kv_buffer.capacity(), 65536);

            // Simulate processing
            let data = kv_buffer.as_slice::<f32>();
            assert!(!data.is_empty());

            // Buffer returns to pool when dropped
        }

        // Check pool statistics
        let stats = pool.stats();
        assert!(stats.hits + stats.misses > 0, "Pool should have been used");

        // Hit rate should be decent after warm-up
        if stats.hits + stats.misses >= 10 {
            assert!(
                stats.hit_rate > 0.5,
                "Pool hit rate should be decent: {:.2}",
                stats.hit_rate
            );
        }
    }

    /// Test streaming with memory pool
    #[test]
    fn test_streaming_with_pool() {
        let manager = MemoryManager::new().expect("manager creation failed");

        // Simulate streaming generation
        for token_idx in 0..100 {
            // Reset arena at start of each step
            manager.reset_step();

            // Allocate temporary buffers from arena
            let activations: &mut [f32] = manager.arena.alloc(1024).expect("arena alloc failed");
            activations[0] = token_idx as f32;

            let logits: &mut [f32] = manager.arena.alloc(32000).expect("arena alloc for logits");
            logits[0] = token_idx as f32 * 0.1;

            // Acquire KV cache buffer from pool
            let kv_buf = manager
                .pool
                .acquire(BufferSize::KB16)
                .expect("acquire failed");
            assert!(kv_buf.capacity() >= 16384);

            // Use scratch space for intermediate computations
            let mut scratch = manager.scratch.get_scratch().expect("get_scratch failed");
            if let Some(temp) = scratch.get::<f32>(256) {
                temp.fill(1.0);
                assert_eq!(temp.len(), 256);
            }

            // Verify arena usage grows
            assert!(manager.arena.used() > 0);
        }

        // Verify final statistics
        let stats = manager.stats();
        assert!(stats.pool.hits + stats.pool.misses > 0);
        assert!(stats.arena.high_water_mark > 0);
    }

    /// Test arena allocation and reset cycle
    #[test]
    fn test_arena_allocation_cycle() {
        let arena = InferenceArena::new(4 * 1024 * 1024).expect("arena creation failed"); // 4MB

        for cycle in 0..50 {
            // Allocate various buffer sizes
            let buf1: &mut [f32] = arena.alloc(4096).expect("alloc 4096");
            let buf2: &mut [f32] = arena.alloc(8192).expect("alloc 8192");
            let buf3: &mut [f32] = arena.alloc(1024).expect("alloc 1024");

            // Write to buffers
            buf1[0] = cycle as f32;
            buf2[0] = cycle as f32 * 2.0;
            buf3[0] = cycle as f32 * 3.0;

            // Verify allocations
            assert_eq!(arena.allocation_count(), 3);
            assert!(arena.used() > 0);

            // Reset for next cycle
            arena.reset();
            assert_eq!(arena.used(), 0);
            assert_eq!(arena.allocation_count(), 0);
        }

        // High water mark should be set
        assert!(arena.high_water_mark() > 0);
    }

    /// Test buffer pool reuse efficiency
    #[test]
    fn test_buffer_pool_reuse() {
        let pool = BufferPool::with_capacity(8);

        // Acquire and release same size multiple times
        for _ in 0..20 {
            let buf = pool.acquire(BufferSize::KB4).expect("acquire failed");
            assert_eq!(buf.capacity(), 4096);
            // Buffer returns to pool on drop
        }

        let stats = pool.stats();
        // After first allocation, subsequent ones should hit the pool
        assert!(
            stats.hits >= 19,
            "Expected at least 19 hits, got {}",
            stats.hits
        );
    }

    /// Test scratch space thread isolation
    #[test]
    fn test_scratch_space_isolation() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(ScratchSpaceManager::new(8192, 8).expect("manager creation failed"));

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    for _ in 0..10 {
                        let mut scratch = manager.get_scratch().expect("get_scratch failed");

                        // Each thread writes its ID
                        if let Some(buf) = scratch.get::<u32>(100) {
                            buf.fill(thread_id);
                            // Verify no cross-thread contamination
                            assert!(buf.iter().all(|&v| v == thread_id));
                        }

                        scratch.reset();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify 4 threads were tracked
        assert_eq!(manager.active_threads(), 4);
    }

    /// Test memory manager configuration for model
    #[test]
    fn test_memory_manager_for_model() {
        // Configure for a small LLM (e.g., Phi-2)
        let config = MemoryManagerConfig::for_model(
            2560,  // hidden_dim
            51200, // vocab_size
            1,     // batch_size
        );

        let manager = MemoryManager::with_config(config).expect("manager creation failed");

        // Verify adequate capacity
        assert!(manager.arena.capacity() > 2560 * 4 * 4); // At least hidden_dim * 4 * sizeof(f32)

        // Simulate inference
        let activations: &mut [f32] = manager.arena.alloc(2560).expect("alloc activations");
        let logits: &mut [f32] = manager.arena.alloc(51200).expect("alloc logits");

        assert_eq!(activations.len(), 2560);
        assert_eq!(logits.len(), 51200);

        // Reset for next step
        manager.reset_step();
        assert_eq!(manager.arena.used(), 0);
    }

    /// Test buffer size class selection
    #[test]
    fn test_buffer_size_selection() {
        let pool = BufferPool::new();

        // Test automatic size class selection
        if let Some(buf) = pool.acquire_for_size(500).ok().flatten() {
            assert!(buf.capacity() >= 500);
            assert_eq!(buf.size_class(), BufferSize::KB1);
        }

        if let Some(buf) = pool.acquire_for_size(3000).ok().flatten() {
            assert!(buf.capacity() >= 3000);
            assert_eq!(buf.size_class(), BufferSize::KB4);
        }

        if let Some(buf) = pool.acquire_for_size(100000).ok().flatten() {
            assert!(buf.capacity() >= 100000);
            assert_eq!(buf.size_class(), BufferSize::KB256);
        }

        // Size too large should return None
        let too_large = pool.acquire_for_size(500000).ok().flatten();
        assert!(too_large.is_none(), "Should not find buffer for 500KB");
    }
}
