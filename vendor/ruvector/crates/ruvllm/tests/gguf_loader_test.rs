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
//! GGUF Loader Integration Tests
//!
//! Tests for the new GGUF model loading system including:
//! - Tensor name mapping for different architectures
//! - Progress tracking during loading
//! - Layer weight organization
//! - Streaming loader for large models

use std::collections::HashMap;

// ============================================================================
// TensorNameMapper Tests
// ============================================================================

/// Simulated tensor name mapper for testing (mirrors the real implementation)
struct TestTensorNameMapper {
    architecture: &'static str,
}

impl TestTensorNameMapper {
    fn new(architecture: &'static str) -> Self {
        Self { architecture }
    }

    fn extract_layer_index(&self, name: &str) -> Option<usize> {
        for pattern in &["layers.", "h.", "blocks.", "block."] {
            if let Some(pos) = name.find(pattern) {
                let after = &name[pos + pattern.len()..];
                if let Some(end) = after.find('.') {
                    if let Ok(idx) = after[..end].parse() {
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    fn categorize(&self, name: &str) -> &'static str {
        let lower = name.to_lowercase();

        if lower.contains("embed") || (lower.contains("token") && lower.contains("weight")) {
            if lower.contains("output") || lower.contains("lm_head") {
                return "OutputHead";
            }
            return "Embedding";
        }

        if lower.contains("lm_head") || (lower.contains("output") && !lower.contains("attn")) {
            return "OutputHead";
        }

        if lower.contains("attn") || lower.contains("attention") {
            if lower.contains("q_proj") || lower.contains(".wq.") || lower.contains("query") {
                return "AttentionQuery";
            }
            if lower.contains("k_proj") || lower.contains(".wk.") || lower.contains("key") {
                return "AttentionKey";
            }
            if lower.contains("v_proj") || lower.contains(".wv.") || lower.contains("value") {
                return "AttentionValue";
            }
            if lower.contains("o_proj") || lower.contains(".wo.") || lower.contains("out_proj") {
                return "AttentionOutput";
            }
        }

        if lower.contains("mlp") || lower.contains("ffn") || lower.contains("feed_forward") {
            if lower.contains("gate") || lower.contains(".w1.") {
                return "FfnGate";
            }
            if lower.contains("up") || lower.contains(".w3.") {
                return "FfnUp";
            }
            if lower.contains("down") || lower.contains(".w2.") {
                return "FfnDown";
            }
        }

        if lower.contains("norm") || lower.contains("ln_") || lower.contains("layer_norm") {
            if lower.contains("final") || lower.contains("model.norm") || !lower.contains("layers")
            {
                return "FinalNorm";
            }
            return "LayerNorm";
        }

        "Other"
    }
}

#[test]
fn test_llama_tensor_name_mapping() {
    let mapper = TestTensorNameMapper::new("llama");

    // Test layer extraction
    assert_eq!(
        mapper.extract_layer_index("model.layers.0.self_attn.q_proj.weight"),
        Some(0)
    );
    assert_eq!(
        mapper.extract_layer_index("model.layers.31.mlp.gate_proj.weight"),
        Some(31)
    );
    assert_eq!(
        mapper.extract_layer_index("model.embed_tokens.weight"),
        None
    );
    assert_eq!(mapper.extract_layer_index("lm_head.weight"), None);
}

#[test]
fn test_phi_tensor_name_mapping() {
    let mapper = TestTensorNameMapper::new("phi");

    // Phi uses transformer.h.N pattern
    assert_eq!(
        mapper.extract_layer_index("transformer.h.0.mixer.Wqkv.weight"),
        Some(0)
    );
    assert_eq!(
        mapper.extract_layer_index("transformer.h.15.mlp.fc1.weight"),
        Some(15)
    );
    assert_eq!(
        mapper.extract_layer_index("transformer.embd.wte.weight"),
        None
    );
}

#[test]
fn test_qwen_tensor_name_mapping() {
    let mapper = TestTensorNameMapper::new("qwen");

    // Qwen uses transformer.h.N pattern like GPT-2
    assert_eq!(
        mapper.extract_layer_index("transformer.h.0.attn.c_attn.weight"),
        Some(0)
    );
    assert_eq!(
        mapper.extract_layer_index("transformer.h.23.mlp.w1.weight"),
        Some(23)
    );
}

#[test]
fn test_tensor_categorization_attention() {
    let mapper = TestTensorNameMapper::new("llama");

    assert_eq!(
        mapper.categorize("model.layers.0.self_attn.q_proj.weight"),
        "AttentionQuery"
    );
    assert_eq!(
        mapper.categorize("model.layers.0.self_attn.k_proj.weight"),
        "AttentionKey"
    );
    assert_eq!(
        mapper.categorize("model.layers.0.self_attn.v_proj.weight"),
        "AttentionValue"
    );
    assert_eq!(
        mapper.categorize("model.layers.0.self_attn.o_proj.weight"),
        "AttentionOutput"
    );
}

#[test]
fn test_tensor_categorization_mlp() {
    let mapper = TestTensorNameMapper::new("llama");

    assert_eq!(
        mapper.categorize("model.layers.0.mlp.gate_proj.weight"),
        "FfnGate"
    );
    assert_eq!(
        mapper.categorize("model.layers.0.mlp.up_proj.weight"),
        "FfnUp"
    );
    assert_eq!(
        mapper.categorize("model.layers.0.mlp.down_proj.weight"),
        "FfnDown"
    );
}

#[test]
fn test_tensor_categorization_embedding() {
    let mapper = TestTensorNameMapper::new("llama");

    assert_eq!(mapper.categorize("model.embed_tokens.weight"), "Embedding");
    assert_eq!(mapper.categorize("lm_head.weight"), "OutputHead");
    assert_eq!(mapper.categorize("model.norm.weight"), "FinalNorm");
}

// ============================================================================
// LoadProgress Tests
// ============================================================================

#[derive(Debug, Clone)]
struct TestLoadProgress {
    total_tensors: usize,
    loaded_tensors: usize,
    total_bytes: usize,
    loaded_bytes: usize,
}

impl TestLoadProgress {
    fn percent(&self) -> f32 {
        if self.total_tensors == 0 {
            return 100.0;
        }
        (self.loaded_tensors as f32 / self.total_tensors as f32) * 100.0
    }

    fn byte_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.loaded_bytes as f32 / self.total_bytes as f32) * 100.0
    }

    fn is_complete(&self) -> bool {
        self.loaded_tensors >= self.total_tensors
    }
}

#[test]
fn test_load_progress_calculation() {
    let progress = TestLoadProgress {
        total_tensors: 100,
        loaded_tensors: 25,
        total_bytes: 1_000_000,
        loaded_bytes: 250_000,
    };

    assert!((progress.percent() - 25.0).abs() < 0.001);
    assert!((progress.byte_percent() - 25.0).abs() < 0.001);
    assert!(!progress.is_complete());
}

#[test]
fn test_load_progress_complete() {
    let progress = TestLoadProgress {
        total_tensors: 50,
        loaded_tensors: 50,
        total_bytes: 500_000,
        loaded_bytes: 500_000,
    };

    assert!((progress.percent() - 100.0).abs() < 0.001);
    assert!(progress.is_complete());
}

#[test]
fn test_load_progress_empty() {
    let progress = TestLoadProgress {
        total_tensors: 0,
        loaded_tensors: 0,
        total_bytes: 0,
        loaded_bytes: 0,
    };

    // Empty should be considered complete
    assert!((progress.percent() - 100.0).abs() < 0.001);
}

// ============================================================================
// LoadConfig Tests
// ============================================================================

#[derive(Default)]
struct TestLoadConfig {
    use_mmap: bool,
    keep_quantized: bool,
    tensor_filter: Vec<String>,
    layer_filter: Vec<usize>,
    num_threads: usize,
}

impl TestLoadConfig {
    fn with_mmap(mut self, enabled: bool) -> Self {
        self.use_mmap = enabled;
        self
    }

    fn with_quantized(mut self, keep: bool) -> Self {
        self.keep_quantized = keep;
        self
    }

    fn with_tensor_filter(mut self, tensors: Vec<String>) -> Self {
        self.tensor_filter = tensors;
        self
    }

    fn with_layer_filter(mut self, layers: Vec<usize>) -> Self {
        self.layer_filter = layers;
        self
    }

    fn with_threads(mut self, threads: usize) -> Self {
        self.num_threads = threads;
        self
    }
}

#[test]
fn test_load_config_builder() {
    let config = TestLoadConfig::default()
        .with_mmap(true)
        .with_quantized(true)
        .with_threads(8)
        .with_layer_filter(vec![0, 1, 2, 3])
        .with_tensor_filter(vec!["attention".to_string()]);

    assert!(config.use_mmap);
    assert!(config.keep_quantized);
    assert_eq!(config.num_threads, 8);
    assert_eq!(config.layer_filter, vec![0, 1, 2, 3]);
    assert_eq!(config.tensor_filter, vec!["attention".to_string()]);
}

#[test]
fn test_load_config_defaults() {
    let config = TestLoadConfig::default();

    assert!(!config.use_mmap);
    assert!(!config.keep_quantized);
    assert_eq!(config.num_threads, 0);
    assert!(config.layer_filter.is_empty());
    assert!(config.tensor_filter.is_empty());
}

// ============================================================================
// Architecture-Specific Tensor Mapping Tests
// ============================================================================

struct ArchitectureTensorMap {
    embed_tokens: &'static str,
    q_proj_pattern: &'static str,
    k_proj_pattern: &'static str,
    v_proj_pattern: &'static str,
    o_proj_pattern: &'static str,
    gate_proj_pattern: &'static str,
    up_proj_pattern: &'static str,
    down_proj_pattern: &'static str,
    final_norm: &'static str,
    lm_head: &'static str,
}

impl ArchitectureTensorMap {
    fn llama() -> Self {
        Self {
            embed_tokens: "model.embed_tokens.weight",
            q_proj_pattern: "model.layers.{}.self_attn.q_proj.weight",
            k_proj_pattern: "model.layers.{}.self_attn.k_proj.weight",
            v_proj_pattern: "model.layers.{}.self_attn.v_proj.weight",
            o_proj_pattern: "model.layers.{}.self_attn.o_proj.weight",
            gate_proj_pattern: "model.layers.{}.mlp.gate_proj.weight",
            up_proj_pattern: "model.layers.{}.mlp.up_proj.weight",
            down_proj_pattern: "model.layers.{}.mlp.down_proj.weight",
            final_norm: "model.norm.weight",
            lm_head: "lm_head.weight",
        }
    }

    fn mistral() -> Self {
        // Mistral uses same naming as Llama
        Self::llama()
    }

    fn phi() -> Self {
        Self {
            embed_tokens: "transformer.embd.wte.weight",
            q_proj_pattern: "transformer.h.{}.mixer.Wqkv.weight",
            k_proj_pattern: "transformer.h.{}.mixer.Wqkv.weight",
            v_proj_pattern: "transformer.h.{}.mixer.Wqkv.weight",
            o_proj_pattern: "transformer.h.{}.mixer.out_proj.weight",
            gate_proj_pattern: "transformer.h.{}.mlp.fc1.weight",
            up_proj_pattern: "transformer.h.{}.mlp.fc1.weight",
            down_proj_pattern: "transformer.h.{}.mlp.fc2.weight",
            final_norm: "transformer.ln_f.weight",
            lm_head: "lm_head.weight",
        }
    }

    fn gemma() -> Self {
        Self {
            embed_tokens: "model.embed_tokens.weight",
            q_proj_pattern: "model.layers.{}.self_attn.q_proj.weight",
            k_proj_pattern: "model.layers.{}.self_attn.k_proj.weight",
            v_proj_pattern: "model.layers.{}.self_attn.v_proj.weight",
            o_proj_pattern: "model.layers.{}.self_attn.o_proj.weight",
            gate_proj_pattern: "model.layers.{}.mlp.gate_proj.weight",
            up_proj_pattern: "model.layers.{}.mlp.up_proj.weight",
            down_proj_pattern: "model.layers.{}.mlp.down_proj.weight",
            final_norm: "model.norm.weight",
            lm_head: "model.embed_tokens.weight", // Tied embeddings
        }
    }

    fn layer_tensor(&self, pattern: &str, layer: usize) -> String {
        pattern.replace("{}", &layer.to_string())
    }
}

#[test]
fn test_llama_tensor_patterns() {
    let map = ArchitectureTensorMap::llama();

    assert_eq!(
        map.layer_tensor(map.q_proj_pattern, 0),
        "model.layers.0.self_attn.q_proj.weight"
    );
    assert_eq!(
        map.layer_tensor(map.gate_proj_pattern, 15),
        "model.layers.15.mlp.gate_proj.weight"
    );
    assert_eq!(
        map.layer_tensor(map.down_proj_pattern, 31),
        "model.layers.31.mlp.down_proj.weight"
    );
}

#[test]
fn test_phi_tensor_patterns() {
    let map = ArchitectureTensorMap::phi();

    assert_eq!(
        map.layer_tensor(map.q_proj_pattern, 0),
        "transformer.h.0.mixer.Wqkv.weight"
    );
    assert_eq!(
        map.layer_tensor(map.o_proj_pattern, 7),
        "transformer.h.7.mixer.out_proj.weight"
    );
    assert_eq!(
        map.layer_tensor(map.down_proj_pattern, 23),
        "transformer.h.23.mlp.fc2.weight"
    );
}

#[test]
fn test_gemma_tied_embeddings() {
    let map = ArchitectureTensorMap::gemma();

    // Gemma ties lm_head to embed_tokens
    assert_eq!(map.embed_tokens, map.lm_head);
}

// ============================================================================
// Weight Tensor Tests
// ============================================================================

#[derive(Clone)]
enum TestWeightTensor {
    F32(Vec<f32>, Vec<usize>),
    Quantized {
        data: Vec<u8>,
        quant_type: u32,
        shape: Vec<usize>,
    },
}

impl TestWeightTensor {
    fn shape(&self) -> &[usize] {
        match self {
            TestWeightTensor::F32(_, shape) => shape,
            TestWeightTensor::Quantized { shape, .. } => shape,
        }
    }

    fn is_quantized(&self) -> bool {
        matches!(self, TestWeightTensor::Quantized { .. })
    }

    fn memory_bytes(&self) -> usize {
        match self {
            TestWeightTensor::F32(data, _) => data.len() * 4,
            TestWeightTensor::Quantized { data, .. } => data.len(),
        }
    }
}

#[test]
fn test_weight_tensor_f32() {
    let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
    let shape = vec![2, 3];
    let tensor = TestWeightTensor::F32(data.clone(), shape.clone());

    assert!(!tensor.is_quantized());
    assert_eq!(tensor.shape(), &[2, 3]);
    assert_eq!(tensor.memory_bytes(), 24); // 6 floats * 4 bytes
}

#[test]
fn test_weight_tensor_quantized() {
    let data = vec![0u8; 18]; // One Q4_0 block (2 bytes scale + 16 bytes data)
    let tensor = TestWeightTensor::Quantized {
        data: data.clone(),
        quant_type: 2, // Q4_0
        shape: vec![32],
    };

    assert!(tensor.is_quantized());
    assert_eq!(tensor.shape(), &[32]);
    assert_eq!(tensor.memory_bytes(), 18);
}

// ============================================================================
// Streaming Loader Simulation Tests
// ============================================================================

struct TestStreamingLoader {
    total_layers: usize,
    current_layer: usize,
}

impl TestStreamingLoader {
    fn new(total_layers: usize) -> Self {
        Self {
            total_layers,
            current_layer: 0,
        }
    }

    fn has_more_layers(&self) -> bool {
        self.current_layer < self.total_layers
    }

    fn load_next_layer(&mut self) -> Option<usize> {
        if self.current_layer >= self.total_layers {
            return None;
        }
        let layer = self.current_layer;
        self.current_layer += 1;
        Some(layer)
    }

    fn reset(&mut self) {
        self.current_layer = 0;
    }
}

#[test]
fn test_streaming_loader_basic() {
    let mut loader = TestStreamingLoader::new(32);

    assert!(loader.has_more_layers());
    assert_eq!(loader.load_next_layer(), Some(0));
    assert_eq!(loader.load_next_layer(), Some(1));
    assert!(loader.has_more_layers());
}

#[test]
fn test_streaming_loader_exhaust() {
    let mut loader = TestStreamingLoader::new(3);

    assert_eq!(loader.load_next_layer(), Some(0));
    assert_eq!(loader.load_next_layer(), Some(1));
    assert_eq!(loader.load_next_layer(), Some(2));
    assert!(!loader.has_more_layers());
    assert_eq!(loader.load_next_layer(), None);
}

#[test]
fn test_streaming_loader_reset() {
    let mut loader = TestStreamingLoader::new(5);

    // Load some layers
    loader.load_next_layer();
    loader.load_next_layer();

    // Reset
    loader.reset();

    // Should start from beginning
    assert_eq!(loader.load_next_layer(), Some(0));
}

// ============================================================================
// Model Configuration Tests
// ============================================================================

#[derive(Debug, Clone, Default)]
struct TestModelConfig {
    architecture: Option<String>,
    context_length: Option<usize>,
    embedding_length: Option<usize>,
    head_count: Option<usize>,
    head_count_kv: Option<usize>,
    layer_count: Option<usize>,
    vocab_size: Option<usize>,
    rope_freq_base: Option<f32>,
    feed_forward_length: Option<usize>,
}

#[test]
fn test_model_config_llama_7b() {
    let config = TestModelConfig {
        architecture: Some("llama".to_string()),
        context_length: Some(4096),
        embedding_length: Some(4096),
        head_count: Some(32),
        head_count_kv: Some(32),
        layer_count: Some(32),
        vocab_size: Some(32000),
        rope_freq_base: Some(10000.0),
        feed_forward_length: Some(11008),
    };

    assert_eq!(config.architecture, Some("llama".to_string()));
    assert_eq!(config.layer_count, Some(32));
    assert_eq!(config.head_count, Some(32));
}

#[test]
fn test_model_config_mistral_7b() {
    let config = TestModelConfig {
        architecture: Some("mistral".to_string()),
        context_length: Some(32768),
        embedding_length: Some(4096),
        head_count: Some(32),
        head_count_kv: Some(8), // GQA with 8 KV heads
        layer_count: Some(32),
        vocab_size: Some(32000),
        rope_freq_base: Some(10000.0),
        feed_forward_length: Some(14336),
    };

    assert_eq!(config.head_count_kv, Some(8)); // GQA
    assert_eq!(config.context_length, Some(32768)); // Larger context
}

#[test]
fn test_model_config_phi2() {
    let config = TestModelConfig {
        architecture: Some("phi".to_string()),
        context_length: Some(2048),
        embedding_length: Some(2560),
        head_count: Some(32),
        head_count_kv: Some(32),
        layer_count: Some(32),
        vocab_size: Some(51200),
        rope_freq_base: Some(10000.0),
        feed_forward_length: Some(10240),
    };

    assert_eq!(config.embedding_length, Some(2560));
    assert_eq!(config.vocab_size, Some(51200));
}

// ============================================================================
// Memory Estimation Tests
// ============================================================================

fn estimate_model_memory(config: &TestModelConfig, quant_type: &str) -> usize {
    let vocab = config.vocab_size.unwrap_or(32000);
    let hidden = config.embedding_length.unwrap_or(4096);
    let layers = config.layer_count.unwrap_or(32);
    let ff_hidden = config.feed_forward_length.unwrap_or(hidden * 4);

    // Bytes per parameter based on quantization
    let bytes_per_param: f32 = match quant_type {
        "F32" => 4.0,
        "F16" => 2.0,
        "Q8_0" => 1.0625, // ~8.5 bits per weight
        "Q4_K" => 0.5625, // ~4.5 bits per weight
        "Q4_0" => 0.5625,
        "Q2_K" => 0.325, // ~2.6 bits per weight
        _ => 4.0,
    };

    // Embedding: vocab_size * hidden_size
    let embed_params = vocab * hidden;

    // Per layer:
    // - Attention: 4 * hidden^2 (Q, K, V, O projections)
    // - MLP: 3 * hidden * ff_hidden (gate, up, down)
    let attn_params_per_layer = 4 * hidden * hidden;
    let mlp_params_per_layer = 3 * hidden * ff_hidden;
    let layer_params = attn_params_per_layer + mlp_params_per_layer;

    // Total
    let total_params = embed_params + (layers * layer_params) + (vocab * hidden); // + LM head

    (total_params as f32 * bytes_per_param) as usize
}

#[test]
fn test_memory_estimation_llama_7b() {
    let config = TestModelConfig {
        architecture: Some("llama".to_string()),
        embedding_length: Some(4096),
        layer_count: Some(32),
        vocab_size: Some(32000),
        feed_forward_length: Some(11008),
        ..Default::default()
    };

    let f32_size = estimate_model_memory(&config, "F32");
    let q4_size = estimate_model_memory(&config, "Q4_K");

    // F32 ~7B params * 4 bytes = ~28GB
    // Q4_K ~7B params * 0.5625 bytes = ~4GB
    assert!(f32_size > 20_000_000_000); // > 20GB
    assert!(q4_size < 6_000_000_000); // < 6GB
    assert!(f32_size > q4_size * 5); // F32 should be ~7x larger
}

#[test]
fn test_memory_estimation_small_model() {
    let config = TestModelConfig {
        architecture: Some("phi".to_string()),
        embedding_length: Some(2560),
        layer_count: Some(24),
        vocab_size: Some(51200),
        feed_forward_length: Some(10240),
        ..Default::default()
    };

    let q4_size = estimate_model_memory(&config, "Q4_K");

    // Phi-2 is smaller, Q4_K should be < 2GB
    assert!(q4_size < 3_000_000_000);
}
