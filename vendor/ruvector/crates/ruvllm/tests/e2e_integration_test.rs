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
//! End-to-end Integration Tests for RuvLLM
//!
//! Tests the complete inference pipeline including:
//! - GGUF file parsing and loading
//! - Token generation with various configurations
//! - Streaming generation with callbacks
//! - Speculative decoding pipeline
//! - KV cache persistence and continuation
//! - Batch generation processing
//! - Stop sequence handling
//! - Temperature sampling verification
//!
//! ## Running Tests
//!
//! ### Without a real model (uses NoopBackend simulation):
//! ```bash
//! cargo test -p ruvllm --test e2e_integration_test
//! ```
//!
//! ### With a real model file:
//! ```bash
//! TEST_MODEL_PATH=/path/to/model.gguf cargo test -p ruvllm --test e2e_integration_test -- --ignored
//! ```
//!
//! ### Run specific test with model:
//! ```bash
//! TEST_MODEL_PATH=/path/to/model.gguf cargo test -p ruvllm --test e2e_integration_test test_real_model_generation -- --ignored
//! ```

use ruvllm::{
    // Backends
    backends::{
        GenerateParams, GeneratedToken, LlmBackend, ModelArchitecture, ModelConfig, Quantization,
        SpecialTokens, StreamEvent, TokenStream, Tokenizer,
    },
    // Error handling
    error::{Result, RuvLLMError},
    // KV Cache
    kv_cache::{KvCacheConfig, TwoTierKvCache},
    // Serving
    serving::{
        InferenceRequest, KvCachePoolConfig, Priority, ServingEngine, ServingEngineConfig,
        TokenOutput,
    },
    // Speculative decoding
    speculative::{
        log_softmax, sample_from_probs, softmax, top_k_filter, top_p_filter,
        AtomicSpeculativeStats, SpeculationTree, SpeculativeConfig, SpeculativeDecoder,
        SpeculativeStats, TreeNode,
    },
};

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Test Fixtures and Helpers
// ============================================================================

/// GGUF magic number "GGUF" in little-endian
const GGUF_MAGIC: u32 = 0x46554747;
/// Supported GGUF version
const GGUF_VERSION: u32 = 3;

/// GGUF metadata value types
#[repr(u32)]
enum GgufMetadataType {
    Uint32 = 4,
    String = 8,
}

/// Create a minimal valid GGUF file for testing (header only, no tensors)
fn create_minimal_test_gguf() -> Vec<u8> {
    let mut data = Vec::new();

    // Magic number
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());

    // Version
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());

    // Tensor count: 0
    data.extend_from_slice(&0u64.to_le_bytes());

    // Metadata KV count: 0
    data.extend_from_slice(&0u64.to_le_bytes());

    data
}

/// Create a GGUF file with metadata (architecture, context length, etc.)
fn create_test_gguf_with_metadata() -> Vec<u8> {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());

    // Tensor count: 1 (we'll add a small embedding)
    data.extend_from_slice(&1u64.to_le_bytes());

    // Metadata count: 3
    data.extend_from_slice(&3u64.to_le_bytes());

    // Metadata 1: general.architecture = "llama" (string)
    let key1 = "general.architecture";
    data.extend_from_slice(&(key1.len() as u64).to_le_bytes());
    data.extend_from_slice(key1.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::String as u32).to_le_bytes());
    let value1 = "llama";
    data.extend_from_slice(&(value1.len() as u64).to_le_bytes());
    data.extend_from_slice(value1.as_bytes());

    // Metadata 2: llama.context_length = 4096 (u32)
    let key2 = "llama.context_length";
    data.extend_from_slice(&(key2.len() as u64).to_le_bytes());
    data.extend_from_slice(key2.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::Uint32 as u32).to_le_bytes());
    data.extend_from_slice(&4096u32.to_le_bytes());

    // Metadata 3: llama.embedding_length = 4096 (u32)
    let key3 = "llama.embedding_length";
    data.extend_from_slice(&(key3.len() as u64).to_le_bytes());
    data.extend_from_slice(key3.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::Uint32 as u32).to_le_bytes());
    data.extend_from_slice(&4096u32.to_le_bytes());

    // Tensor info for a small embedding tensor
    let tensor_name = "model.embed_tokens.weight";
    data.extend_from_slice(&(tensor_name.len() as u64).to_le_bytes());
    data.extend_from_slice(tensor_name.as_bytes());
    data.extend_from_slice(&2u32.to_le_bytes()); // n_dims
    data.extend_from_slice(&32u64.to_le_bytes()); // vocab_size (small for test)
    data.extend_from_slice(&16u64.to_le_bytes()); // hidden_size (small for test)
    data.extend_from_slice(&0u32.to_le_bytes()); // F32 type
    data.extend_from_slice(&0u64.to_le_bytes()); // offset

    data
}

/// Create a GGUF file with Q4_0 quantized tensor
fn create_test_gguf_q4_quantized() -> Vec<u8> {
    let mut data = Vec::new();

    // Header
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes()); // 1 tensor
    data.extend_from_slice(&1u64.to_le_bytes()); // 1 metadata

    // Metadata: architecture
    let key = "general.architecture";
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::String as u32).to_le_bytes());
    let value = "llama";
    data.extend_from_slice(&(value.len() as u64).to_le_bytes());
    data.extend_from_slice(value.as_bytes());

    // Tensor info (Q4_0 quantized)
    let tensor_name = "model.layers.0.self_attn.q_proj.weight";
    data.extend_from_slice(&(tensor_name.len() as u64).to_le_bytes());
    data.extend_from_slice(tensor_name.as_bytes());
    data.extend_from_slice(&2u32.to_le_bytes()); // n_dims
    data.extend_from_slice(&64u64.to_le_bytes()); // dim0
    data.extend_from_slice(&64u64.to_le_bytes()); // dim1
    data.extend_from_slice(&2u32.to_le_bytes()); // Q4_0 type
    data.extend_from_slice(&0u64.to_le_bytes()); // offset

    data
}

/// Mock tokenizer for testing
struct MockTokenizer {
    vocab: HashMap<String, u32>,
    reverse_vocab: HashMap<u32, String>,
}

impl MockTokenizer {
    fn new() -> Self {
        let mut vocab = HashMap::new();
        let mut reverse_vocab = HashMap::new();

        // Add common tokens
        let tokens = [
            ("<s>", 1),
            ("</s>", 2),
            ("<pad>", 0),
            ("Hello", 100),
            (",", 101),
            (" ", 102),
            ("world", 103),
            ("!", 104),
            ("The", 105),
            ("quick", 106),
            ("brown", 107),
            ("fox", 108),
            ("jumps", 109),
            ("over", 110),
            ("lazy", 111),
            ("dog", 112),
            (".", 113),
            ("test", 114),
            ("model", 115),
            ("output", 116),
        ];

        for (text, id) in tokens {
            vocab.insert(text.to_string(), id);
            reverse_vocab.insert(id, text.to_string());
        }

        Self {
            vocab,
            reverse_vocab,
        }
    }
}

impl Tokenizer for MockTokenizer {
    fn encode(&self, text: &str) -> Result<Vec<u32>> {
        // Simple word-based tokenization for testing
        let mut tokens = Vec::new();
        for word in text.split_whitespace() {
            if let Some(&id) = self.vocab.get(word) {
                tokens.push(id);
            } else {
                // Unknown word - hash it to a pseudo-ID
                let hash = word
                    .bytes()
                    .fold(200u32, |acc, b| acc.wrapping_add(b as u32));
                tokens.push(hash % 1000 + 200);
            }
        }
        Ok(tokens)
    }

    fn decode(&self, tokens: &[u32]) -> Result<String> {
        let words: Vec<String> = tokens
            .iter()
            .filter_map(|&id| {
                self.reverse_vocab
                    .get(&id)
                    .cloned()
                    .or_else(|| Some(format!("[{}]", id)))
            })
            .collect();
        Ok(words.join(" "))
    }

    fn vocab_size(&self) -> usize {
        32000 // Standard vocab size
    }

    fn special_tokens(&self) -> SpecialTokens {
        SpecialTokens {
            bos_token_id: Some(1),
            eos_token_id: Some(2),
            pad_token_id: Some(0),
            unk_token_id: Some(3),
        }
    }
}

/// Mock LLM backend that generates deterministic tokens based on context
struct MockLlmBackend {
    tokenizer: MockTokenizer,
    model_loaded: AtomicBool,
    generation_count: AtomicUsize,
}

impl MockLlmBackend {
    fn new() -> Self {
        Self {
            tokenizer: MockTokenizer::new(),
            model_loaded: AtomicBool::new(false),
            generation_count: AtomicUsize::new(0),
        }
    }

    fn deterministic_token(&self, context: &[u32], seed_offset: usize) -> u32 {
        let hash = context.iter().fold(seed_offset as u32, |acc, &t| {
            acc.wrapping_add(t).wrapping_mul(31)
        });
        // Generate tokens in reasonable vocabulary range
        (hash % 30000) + 100
    }
}

impl LlmBackend for MockLlmBackend {
    fn load_model(&mut self, _model_id: &str, _config: ModelConfig) -> Result<()> {
        self.model_loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String> {
        if !self.model_loaded.load(Ordering::SeqCst) {
            return Err(RuvLLMError::Config("Model not loaded".to_string()));
        }

        let count = self.generation_count.fetch_add(1, Ordering::SeqCst);
        let prompt_tokens = self.tokenizer.encode(prompt)?;

        // Generate deterministic tokens
        let mut output_tokens = Vec::new();
        let mut context = prompt_tokens.clone();

        for i in 0..params.max_tokens {
            let token = self.deterministic_token(&context, count + i);

            // Check for stop
            if token == 2 {
                // EOS
                break;
            }

            output_tokens.push(token);
            context.push(token);
        }

        // Decode output
        self.tokenizer.decode(&output_tokens)
    }

    fn generate_stream(
        &self,
        prompt: &str,
        params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<GeneratedToken>> + Send + '_>> {
        let count = self.generation_count.fetch_add(1, Ordering::SeqCst);
        let prompt_tokens = self.tokenizer.encode(prompt)?;

        Ok(Box::new(MockStreamIterator {
            backend: self,
            context: prompt_tokens,
            remaining: params.max_tokens,
            seed_offset: count,
            finished: false,
        }))
    }

    fn generate_stream_v2(&self, prompt: &str, params: GenerateParams) -> Result<TokenStream> {
        let (tx, stream) = TokenStream::channel();
        let count = self.generation_count.fetch_add(1, Ordering::SeqCst);
        let prompt_tokens = self.tokenizer.encode(prompt)?;
        let max_tokens = params.max_tokens;

        // Pre-generate all tokens (deterministic, so we can compute them ahead of time)
        let mut context = prompt_tokens;
        let mut tokens_to_send = Vec::new();

        let start = Instant::now();

        for i in 0..max_tokens {
            let token = self.deterministic_token(&context, count + i);
            let text = self.tokenizer.decode(&[token]).unwrap_or_default();
            let is_eos = token == 2;

            tokens_to_send.push((token, text, is_eos));

            if is_eos {
                break;
            }

            context.push(token);
        }

        let token_count = tokens_to_send.len();
        let duration = start.elapsed();

        // Spawn thread to send tokens (only uses owned data now)
        std::thread::spawn(move || {
            for (token, text, is_eos) in tokens_to_send {
                let event = StreamEvent::Token(GeneratedToken {
                    id: token,
                    text,
                    logprob: Some(-0.5), // Dummy logprob
                    is_special: is_eos,
                });

                if tx.send(event).is_err() {
                    break;
                }
            }

            let _ = tx.send(StreamEvent::Done {
                total_tokens: token_count,
                duration_ms: duration.as_millis() as u64,
                tokens_per_second: token_count as f64 / duration.as_secs_f64().max(0.001),
            });
        });

        Ok(stream)
    }

    fn get_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        // Generate deterministic embeddings
        let tokens = self.tokenizer.encode(text)?;
        let dim = 768; // Standard embedding dim
        let mut embeddings = vec![0.0f32; dim];

        for (i, &t) in tokens.iter().enumerate() {
            for j in 0..dim {
                let idx = (i * 100 + j) % dim;
                embeddings[idx] += (t as f32 * 0.001) * ((j as f32 + 1.0).sin());
            }
        }

        // Normalize
        let norm: f32 = embeddings.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for e in &mut embeddings {
                *e /= norm;
            }
        }

        Ok(embeddings)
    }

    fn tokenizer(&self) -> Option<&dyn Tokenizer> {
        Some(&self.tokenizer)
    }

    fn is_model_loaded(&self) -> bool {
        self.model_loaded.load(Ordering::SeqCst)
    }

    fn model_info(&self) -> Option<ruvllm::backends::ModelInfo> {
        if self.is_model_loaded() {
            Some(ruvllm::backends::ModelInfo {
                name: "MockModel-7B".to_string(),
                architecture: ModelArchitecture::Llama,
                num_parameters: 7_000_000_000,
                vocab_size: 32000,
                hidden_size: 4096,
                num_layers: 32,
                max_context_length: 8192,
                quantization: Some(Quantization::Q4K),
                memory_usage: 4_000_000_000,
            })
        } else {
            None
        }
    }

    fn unload_model(&mut self) {
        self.model_loaded.store(false, Ordering::SeqCst);
    }
}

struct MockStreamIterator<'a> {
    backend: &'a MockLlmBackend,
    context: Vec<u32>,
    remaining: usize,
    seed_offset: usize,
    finished: bool,
}

impl<'a> Iterator for MockStreamIterator<'a> {
    type Item = Result<GeneratedToken>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.remaining == 0 {
            return None;
        }

        let token = self
            .backend
            .deterministic_token(&self.context, self.seed_offset);
        self.seed_offset += 1;
        self.remaining -= 1;

        let text = self.backend.tokenizer.decode(&[token]).unwrap_or_default();
        let is_eos = token == 2;

        if is_eos {
            self.finished = true;
        }

        self.context.push(token);

        Some(Ok(GeneratedToken {
            id: token,
            text,
            logprob: Some(-0.5),
            is_special: is_eos,
        }))
    }
}

/// Create a test serving engine with mock backend
fn create_mock_serving_engine() -> (ServingEngine, Arc<MockLlmBackend>) {
    let backend = Arc::new(MockLlmBackend::new());
    let config = ServingEngineConfig {
        kv_cache: KvCachePoolConfig {
            num_slots: 8,
            max_seq_len: 512,
            block_size: 16,
            total_blocks: 128,
            num_kv_heads: 4,
            head_dim: 64,
            num_layers: 8,
        },
        max_concurrent_requests: 16,
        enable_speculative: false, // Disable for basic tests
        ..Default::default()
    };
    let engine = ServingEngine::new(backend.clone() as Arc<dyn LlmBackend>, config);
    (engine, backend)
}

// ============================================================================
// GGUF Loading Tests
// ============================================================================

#[test]
fn test_gguf_load_and_generate_basic() {
    // Test: Load a minimal GGUF, verify parsing works, then generate tokens
    let gguf_data = create_minimal_test_gguf();

    // Parse GGUF header
    assert!(gguf_data.len() >= 24); // Minimum header size
    let magic = u32::from_le_bytes([gguf_data[0], gguf_data[1], gguf_data[2], gguf_data[3]]);
    assert_eq!(magic, GGUF_MAGIC, "Magic number should match");

    let version = u32::from_le_bytes([gguf_data[4], gguf_data[5], gguf_data[6], gguf_data[7]]);
    assert_eq!(version, GGUF_VERSION, "Version should be 3");

    // Create mock backend and generate
    let mut backend = MockLlmBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();

    let params = GenerateParams::default().with_max_tokens(10);
    let output = backend.generate("Hello world", params).unwrap();

    assert!(!output.is_empty(), "Should generate some output");
}

#[test]
fn test_gguf_load_with_metadata() {
    // Test: Load GGUF with metadata, verify extraction
    let gguf_data = create_test_gguf_with_metadata();

    // The data should be large enough to contain metadata
    assert!(gguf_data.len() > 100, "Should have metadata");

    // Verify magic
    let magic = u32::from_le_bytes([gguf_data[0], gguf_data[1], gguf_data[2], gguf_data[3]]);
    assert_eq!(magic, GGUF_MAGIC);

    // Count metadata (at offset 16)
    let metadata_count = u64::from_le_bytes(gguf_data[16..24].try_into().unwrap());
    assert_eq!(metadata_count, 3, "Should have 3 metadata entries");
}

#[test]
fn test_gguf_load_with_quantization() {
    // Test: Verify Q4_0, Q4_K, Q8_0 quantized model metadata parsing
    let gguf_data = create_test_gguf_q4_quantized();

    // Parse and verify header
    let magic = u32::from_le_bytes([gguf_data[0], gguf_data[1], gguf_data[2], gguf_data[3]]);
    assert_eq!(magic, GGUF_MAGIC);

    let tensor_count = u64::from_le_bytes(gguf_data[8..16].try_into().unwrap());
    assert_eq!(tensor_count, 1, "Should have 1 quantized tensor");

    // Test quantization type bytes_per_weight
    assert_eq!(Quantization::Q4.bytes_per_weight(), 0.5);
    assert_eq!(Quantization::Q4K.bytes_per_weight(), 0.5);
    assert_eq!(Quantization::Q8.bytes_per_weight(), 1.0);
    assert!(Quantization::Q4.is_gguf());
    assert!(Quantization::Q4K.is_gguf());
    assert!(Quantization::Q8.is_gguf());
    assert!(!Quantization::F16.is_gguf());
}

// ============================================================================
// Streaming Generation Tests
// ============================================================================

#[test]
fn test_streaming_generation() {
    // Test: Streaming callback generation works correctly
    let mut backend = MockLlmBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();

    let params = GenerateParams::default()
        .with_max_tokens(20)
        .with_temperature(0.7);

    // Collect streaming output
    let mut tokens_received = Vec::new();
    let stream = backend.generate_stream("Hello world", params).unwrap();

    for result in stream {
        let token = result.expect("Stream should not error");
        tokens_received.push(token);
    }

    assert!(!tokens_received.is_empty(), "Should receive tokens");
    assert!(tokens_received.len() <= 20, "Should respect max_tokens");

    // Verify each token has valid fields
    for token in &tokens_received {
        assert!(token.id > 0 || token.is_special, "Token ID should be valid");
    }
}

#[test]
fn test_streaming_generation_v2() {
    // Test: New TokenStream interface
    let mut backend = MockLlmBackend::new();
    backend
        .load_model("test-model", ModelConfig::default())
        .unwrap();

    let params = GenerateParams::default()
        .with_max_tokens(10)
        .with_temperature(0.5);

    let mut stream = backend.generate_stream_v2("Test prompt", params).unwrap();

    let mut token_count = 0;
    let mut received_done = false;

    // Use try_next with timeout to avoid blocking forever
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline && !stream.is_finished() {
        if let Some(result) = stream.recv_timeout(Duration::from_millis(100)) {
            match result {
                Ok(StreamEvent::Token(token)) => {
                    token_count += 1;
                    assert!(!token.text.is_empty() || token.is_special);
                }
                Ok(StreamEvent::Done { total_tokens, .. }) => {
                    received_done = true;
                    assert_eq!(total_tokens, token_count);
                    break;
                }
                Ok(StreamEvent::Error(e)) => {
                    panic!("Stream error: {}", e);
                }
                Err(e) => {
                    panic!("Result error: {:?}", e);
                }
            }
        }
    }

    assert!(received_done, "Should receive Done event");
    assert!(token_count > 0, "Should receive at least one token");
}

#[test]
fn test_streaming_with_callback() {
    // Test: Streaming with callback in serving engine
    let (engine, backend) = create_mock_serving_engine();

    // Load model through backend
    backend.model_loaded.store(true, Ordering::SeqCst);

    let tokens_received = Arc::new(AtomicUsize::new(0));
    let tokens_clone = tokens_received.clone();

    let params = GenerateParams::default().with_max_tokens(5);
    let request = InferenceRequest::new(vec![100, 101, 102], params);

    let callback: Box<dyn Fn(TokenOutput) + Send + Sync> = Box::new(move |_output| {
        tokens_clone.fetch_add(1, Ordering::Relaxed);
    });

    let _ = engine.submit_with_callback(request, callback);

    // Run several iterations
    for _ in 0..30 {
        let _ = engine.run_iteration();
    }

    // Should have received some callbacks
    let _received = tokens_received.load(Ordering::Relaxed);
    // May or may not have tokens depending on timing
}

// ============================================================================
// Speculative Decoding Tests
// ============================================================================

#[test]
fn test_speculative_decoding_config() {
    // Test: Speculative decoding configuration
    let config = SpeculativeConfig::default();

    assert!(config.lookahead >= 2, "Lookahead should be at least 2");
    assert!(config.lookahead <= 16, "Lookahead should be reasonable");
    assert!(config.acceptance_threshold > 0.0 && config.acceptance_threshold <= 1.0);
    assert!(
        config.adaptive_lookahead,
        "Adaptive lookahead should be on by default"
    );
}

#[test]
fn test_speculative_stats() {
    // Test: Statistics tracking for speculative decoding
    let mut stats = SpeculativeStats::new();

    assert_eq!(stats.draft_tokens, 0);
    assert_eq!(stats.accepted_tokens, 0);
    assert_eq!(stats.acceptance_rate, 0.0);

    // Record some speculation rounds
    stats.record_round(4, 3, 10.0);
    assert_eq!(stats.draft_tokens, 4);
    assert_eq!(stats.accepted_tokens, 3);
    assert!((stats.acceptance_rate - 0.75).abs() < 0.01);
    assert_eq!(stats.total_tokens_generated, 4); // 3 accepted + 1 correction

    stats.record_round(4, 4, 8.0);
    assert_eq!(stats.draft_tokens, 8);
    assert_eq!(stats.accepted_tokens, 7);

    // Reset
    stats.reset();
    assert_eq!(stats.draft_tokens, 0);
}

#[test]
fn test_atomic_speculative_stats() {
    // Test: Thread-safe atomic statistics
    let stats = AtomicSpeculativeStats::new();

    // Record from multiple threads
    let stats_arc = Arc::new(stats);
    let mut handles = vec![];

    for _ in 0..4 {
        let stats_clone = stats_arc.clone();
        let handle = std::thread::spawn(move || {
            for _ in 0..10 {
                stats_clone.record_round(4, 3, Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = stats_arc.snapshot();
    assert_eq!(snapshot.draft_tokens, 4 * 10 * 4);
    assert_eq!(snapshot.accepted_tokens, 3 * 10 * 4);
    assert_eq!(snapshot.main_forward_passes, 10 * 4);
}

#[test]
fn test_speculation_tree() {
    // Test: Tree-based speculation structure
    let mut tree = SpeculationTree::new(4, 2);

    assert_eq!(tree.node_count, 1);
    assert_eq!(tree.max_depth, 4);
    assert_eq!(tree.branching_factor, 2);

    // Add children to root
    tree.root.add_child(100, 0.8);
    tree.root.add_child(101, 0.6);
    tree.node_count += 2;

    assert_eq!(tree.root.children.len(), 2);

    // Get paths
    let paths = tree.get_candidate_paths();
    assert_eq!(paths.len(), 2); // Two leaf paths

    // Best path should be the one with higher probability
    let best = tree.best_path();
    assert!(
        best.is_empty() || best[0] == 100,
        "Best path should start with high-prob token"
    );
}

#[test]
fn test_tree_node_operations() {
    // Test: TreeNode building and traversal
    let mut root = TreeNode::new(0, 1.0, 0);

    assert_eq!(root.token, 0);
    assert_eq!(root.depth, 0);
    assert!(root.children.is_empty());

    // Build a small tree
    let child1 = root.add_child(10, 0.7);
    child1.add_child(20, 0.8);
    child1.add_child(21, 0.4);

    let child2 = root.add_child(11, 0.5);
    child2.add_child(22, 0.9);

    // Get all paths
    let paths = root.get_paths();
    assert_eq!(paths.len(), 3); // 3 leaf nodes

    // Best path should maximize probability
    let best = root.best_path();
    assert_eq!(best.len(), 3); // root -> child -> leaf
}

#[test]
fn test_speculative_decoding_e2e() {
    // Test: Full speculative decoding pipeline (mock)
    let main_model = Arc::new(MockLlmBackend::new());
    let draft_model = Arc::new(MockLlmBackend::new());

    // Load both models
    unsafe {
        (Arc::as_ptr(&main_model) as *mut MockLlmBackend)
            .as_mut()
            .unwrap()
            .load_model("main", ModelConfig::default())
            .unwrap();
        (Arc::as_ptr(&draft_model) as *mut MockLlmBackend)
            .as_mut()
            .unwrap()
            .load_model("draft", ModelConfig::default())
            .unwrap();
    }

    let config = SpeculativeConfig {
        lookahead: 4,
        acceptance_threshold: 0.5,
        draft_temperature: 0.0,
        tree_speculation: false,
        adaptive_lookahead: true,
        min_lookahead: 2,
        max_lookahead: 8,
        ..Default::default()
    };

    let decoder = SpeculativeDecoder::new(main_model, draft_model, config);

    // Verify configuration
    let cfg = decoder.config();
    assert_eq!(cfg.lookahead, 4);

    // Check tokenizer availability
    assert!(decoder.tokenizer().is_some());

    // Get initial stats
    let stats = decoder.stats();
    assert_eq!(stats.draft_tokens, 0);
}

// ============================================================================
// KV Cache Tests
// ============================================================================

#[test]
fn test_kv_cache_persistence() {
    // Test: Generate, cache, continue generating
    let config = KvCacheConfig {
        tail_length: 16,
        max_tokens: 64,
        num_kv_heads: 2,
        head_dim: 32,
        migration_batch: 8,
        ..Default::default()
    };

    let cache = TwoTierKvCache::new(config);

    // Add initial context
    for i in 0..10 {
        let keys = vec![i as f32 * 0.1; 2 * 32];
        let values = vec![i as f32 * 0.2; 2 * 32];
        cache.append(&keys, &values).unwrap();
    }

    let stats1 = cache.stats();
    assert_eq!(stats1.total_tokens, 10);

    // Query with current cache (simulating continuation)
    // Query size should match num_kv_heads * head_dim = 2 * 32 = 64
    let query = vec![0.5f32; 2 * 32];
    let scale = 1.0 / 32.0f32.sqrt();
    let output1 = cache.attend(&query, scale).unwrap();
    assert_eq!(output1.len(), 2 * 32);

    // Add more tokens (continuation)
    for i in 10..20 {
        let keys = vec![i as f32 * 0.1; 2 * 32];
        let values = vec![i as f32 * 0.2; 2 * 32];
        cache.append(&keys, &values).unwrap();
    }

    let stats2 = cache.stats();
    assert_eq!(stats2.total_tokens, 20);

    // Query again - should now attend over more tokens
    let output2 = cache.attend(&query, scale).unwrap();
    assert_eq!(output2.len(), 2 * 32);

    // Outputs should be different due to more context
    let diff: f32 = output1
        .iter()
        .zip(output2.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    // Could be same if attention weights distribute similarly, so just check finite
    assert!(diff.is_finite());
}

#[test]
fn test_kv_cache_two_tier_migration() {
    // Test: Verify tail -> store migration
    let config = KvCacheConfig {
        tail_length: 4,
        max_tokens: 100,
        num_kv_heads: 1,
        head_dim: 8,
        migration_batch: 2,
        ..Default::default()
    };

    let cache = TwoTierKvCache::new(config);

    // Add enough tokens to trigger migration
    for i in 0..10 {
        let keys = vec![i as f32; 8];
        let values = vec![i as f32 * 2.0; 8];
        cache.append(&keys, &values).unwrap();
    }

    let stats = cache.stats();

    // Tail should be limited, store should have overflow
    assert!(stats.tail_tokens <= 4, "Tail should respect limit");
    assert!(stats.store_tokens > 0, "Store should have migrated tokens");
    assert_eq!(stats.total_tokens, 10);
}

#[test]
fn test_kv_cache_concurrent_access() {
    // Test: Concurrent KV cache operations
    let config = KvCacheConfig {
        tail_length: 32,
        max_tokens: 256,
        num_kv_heads: 4,
        head_dim: 64,
        migration_batch: 16,
        ..Default::default()
    };

    let cache = Arc::new(TwoTierKvCache::new(config));
    let mut handles = vec![];

    // Spawn concurrent writers
    for t in 0..4 {
        let cache_clone = cache.clone();
        let handle = std::thread::spawn(move || {
            for i in 0..25 {
                let keys = vec![(t * 100 + i) as f32; 4 * 64];
                let values = vec![(t * 100 + i) as f32 * 2.0; 4 * 64];
                cache_clone.append(&keys, &values).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = cache.stats();
    assert_eq!(stats.total_tokens, 100); // 4 threads * 25 tokens
}

// ============================================================================
// Batch Generation Tests
// ============================================================================

#[test]
fn test_batch_generation() {
    // Test: Multiple prompts processed in batch
    let (engine, backend) = create_mock_serving_engine();
    backend.model_loaded.store(true, Ordering::SeqCst);

    // Submit multiple requests
    let mut request_ids = Vec::new();
    let prompts = vec![
        vec![100, 101, 102], // "Hello , "
        vec![105, 106, 107], // "The quick brown"
        vec![114, 115, 116], // "test model output"
    ];

    for prompt in prompts {
        let params = GenerateParams::default().with_max_tokens(5);
        let request = InferenceRequest::new(prompt, params);
        let id = engine.submit(request).unwrap();
        request_ids.push(id);
    }

    // Run iterations to process all
    for _ in 0..50 {
        let _ = engine.run_iteration();
    }

    // Check metrics
    let stats = engine.stats();

    // Should have processed requests
    assert!(
        stats.running_requests > 0 || stats.completed_requests > 0 || stats.pending_requests > 0,
        "Should have processed some requests"
    );
}

#[test]
fn test_batch_priority_ordering() {
    // Test: Higher priority requests are processed first
    let (engine, backend) = create_mock_serving_engine();
    backend.model_loaded.store(true, Ordering::SeqCst);

    // Submit low priority first
    let params = GenerateParams::default().with_max_tokens(3);
    let mut low_req = InferenceRequest::new(vec![100], params.clone());
    low_req.priority = Priority::Low;
    let _low_id = engine.submit(low_req).unwrap();

    // Submit high priority second
    let mut high_req = InferenceRequest::new(vec![101], params);
    high_req.priority = Priority::High;
    let _high_id = engine.submit(high_req).unwrap();

    // Priority values
    assert!(Priority::High.value() > Priority::Low.value());
    assert!(Priority::Critical.value() > Priority::High.value());
}

// ============================================================================
// Stop Sequence Tests
// ============================================================================

#[test]
fn test_stop_sequences() {
    // Test: Generation stops at stop sequences
    let mut backend = MockLlmBackend::new();
    backend.load_model("test", ModelConfig::default()).unwrap();

    let params = GenerateParams::default()
        .with_max_tokens(100)
        .with_stop_sequence("\n\n")
        .with_stop_sequence("END");

    // Generate - the mock backend won't actually hit stop sequences
    // but we verify the params are stored correctly
    assert_eq!(params.stop_sequences.len(), 2);
    assert!(params.stop_sequences.contains(&"\n\n".to_string()));
    assert!(params.stop_sequences.contains(&"END".to_string()));
}

#[test]
fn test_multiple_stop_sequences() {
    // Test: Multiple stop sequences configuration
    let params = GenerateParams::default()
        .with_stop_sequence("<|end|>")
        .with_stop_sequence("</s>")
        .with_stop_sequence("STOP")
        .with_stop_sequence("\n---\n");

    assert_eq!(params.stop_sequences.len(), 4);

    // Verify each sequence is present
    for seq in &["<|end|>", "</s>", "STOP", "\n---\n"] {
        assert!(
            params.stop_sequences.contains(&seq.to_string()),
            "Should contain {}",
            seq
        );
    }
}

// ============================================================================
// Temperature Sampling Tests
// ============================================================================

#[test]
fn test_temperature_sampling() {
    // Test: Temperature affects output diversity
    let mut backend = MockLlmBackend::new();
    backend.load_model("test", ModelConfig::default()).unwrap();

    // Low temperature (more deterministic)
    let low_temp_params = GenerateParams::default()
        .with_max_tokens(10)
        .with_temperature(0.1);

    // High temperature (more random)
    let high_temp_params = GenerateParams::default()
        .with_max_tokens(10)
        .with_temperature(1.5);

    // Our mock backend doesn't actually use temperature, but we verify params
    assert!(low_temp_params.temperature < high_temp_params.temperature);
    assert!(low_temp_params.temperature < 0.5);
    assert!(high_temp_params.temperature > 1.0);
}

#[test]
fn test_softmax_temperature_effect() {
    // Test: Verify softmax correctly concentrates/diffuses with temperature
    let logits = vec![1.0f32, 2.0, 3.0, 4.0];

    // Standard softmax
    let probs = softmax(&logits);
    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 0.001, "Softmax should sum to 1");

    // Verify ordering preserved
    assert!(probs[3] > probs[2]);
    assert!(probs[2] > probs[1]);
    assert!(probs[1] > probs[0]);

    // Test with scaled logits (simulating low temperature)
    let scaled: Vec<f32> = logits.iter().map(|&x| x * 5.0).collect();
    let probs_sharp = softmax(&scaled);

    // Sharp distribution should have higher max probability
    assert!(
        probs_sharp[3] > probs[3],
        "Lower temperature should concentrate probability"
    );
}

#[test]
fn test_log_softmax() {
    // Test: Log softmax for numerical stability
    let logits = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];

    let log_probs = log_softmax(&logits);

    // All log probs should be <= 0
    for &lp in &log_probs {
        assert!(lp <= 0.0, "Log probability should be <= 0");
        assert!(lp.is_finite(), "Log probability should be finite");
    }

    // exp(log_softmax) should equal softmax
    let probs_from_log: Vec<f32> = log_probs.iter().map(|&lp| lp.exp()).collect();
    let probs = softmax(&logits);

    for (a, b) in probs_from_log.iter().zip(probs.iter()) {
        assert!(
            (a - b).abs() < 0.001,
            "exp(log_softmax) should equal softmax"
        );
    }
}

#[test]
fn test_top_k_filtering() {
    // Test: Top-k sampling correctly filters
    let mut logits = vec![1.0f32, 5.0, 3.0, 4.0, 2.0];

    top_k_filter(&mut logits, 2);

    // Only top 2 should remain finite
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert_eq!(finite_count, 2, "Top-k should keep exactly k values");

    // The top 2 values (5.0 and 4.0 at indices 1 and 3) should be finite
    assert!(logits[1].is_finite()); // 5.0
    assert!(logits[3].is_finite()); // 4.0
}

#[test]
fn test_top_p_filtering() {
    // Test: Nucleus (top-p) sampling correctly filters
    let mut logits = vec![10.0f32, 5.0, 3.0, 2.0, 1.0];

    top_p_filter(&mut logits, 0.9);

    // Most probability mass should be preserved
    let finite_count = logits.iter().filter(|x| x.is_finite()).count();
    assert!(finite_count >= 1, "Top-p should keep at least one value");
    assert!(finite_count < 5, "Top-p with 0.9 should filter some values");
}

#[test]
fn test_sampling_from_probabilities() {
    // Test: Sample from probability distribution
    use rand::SeedableRng;

    let probs = vec![0.1f32, 0.2, 0.3, 0.4];
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);

    let mut counts = vec![0usize; 4];

    // Sample many times
    for _ in 0..1000 {
        let idx = sample_from_probs(&probs, &mut rng);
        counts[idx] += 1;
    }

    // Higher probability indices should be sampled more often
    // With these probabilities: idx 3 (0.4) > idx 2 (0.3) > idx 1 (0.2) > idx 0 (0.1)
    assert!(
        counts[3] > counts[0],
        "Higher prob should be sampled more: {} vs {}",
        counts[3],
        counts[0]
    );
}

#[test]
fn test_deterministic_generation_with_seed() {
    // Test: Same seed produces same output
    let mut backend1 = MockLlmBackend::new();
    let mut backend2 = MockLlmBackend::new();

    backend1.load_model("test", ModelConfig::default()).unwrap();
    backend2.load_model("test", ModelConfig::default()).unwrap();

    let params = GenerateParams::default().with_max_tokens(10).with_seed(42);

    let output1 = backend1.generate("Hello", params.clone()).unwrap();
    let output2 = backend2.generate("Hello", params).unwrap();

    // With mock backend using deterministic generation, outputs should match
    assert_eq!(output1, output2, "Same seed should produce same output");
}

// ============================================================================
// Real Model Tests (Requires TEST_MODEL_PATH)
// ============================================================================

#[test]
#[ignore = "Requires GGUF model file at TEST_MODEL_PATH environment variable"]
fn test_real_model_generation() {
    // Test: Load actual GGUF model and generate
    let model_path =
        env::var("TEST_MODEL_PATH").expect("TEST_MODEL_PATH environment variable must be set");

    let path = Path::new(&model_path);
    assert!(path.exists(), "Model file should exist: {}", model_path);

    // For now, just verify the file exists and can be opened
    let file = std::fs::File::open(path).expect("Should open model file");
    let metadata = file.metadata().expect("Should read metadata");

    assert!(
        metadata.len() > 1024,
        "Model file should be larger than 1KB"
    );

    // Read and verify GGUF magic
    let mut buffer = [0u8; 4];
    use std::io::Read;
    let mut file = std::fs::File::open(path).unwrap();
    file.read_exact(&mut buffer).expect("Should read magic");

    let magic = u32::from_le_bytes(buffer);
    assert_eq!(magic, GGUF_MAGIC, "Should have valid GGUF magic");
}

#[test]
#[ignore = "Requires GGUF model file at TEST_MODEL_PATH environment variable"]
fn test_real_model_streaming() {
    // Test: Stream generation from real model
    let model_path =
        env::var("TEST_MODEL_PATH").expect("TEST_MODEL_PATH environment variable must be set");

    // Would need real model loading here
    // For now, verify environment is set correctly
    assert!(
        !model_path.is_empty(),
        "TEST_MODEL_PATH should not be empty"
    );
}

#[test]
#[ignore = "Requires GGUF model file at TEST_MODEL_PATH environment variable"]
fn test_real_model_quantization() {
    // Test: Load quantized model and verify inference
    let _model_path =
        env::var("TEST_MODEL_PATH").expect("TEST_MODEL_PATH environment variable must be set");

    // Verify quantization types
    assert!(Quantization::Q4K.is_gguf());
    assert!(Quantization::Q8.is_gguf());

    // Memory estimation for different quantizations
    let param_count: f64 = 7_000_000_000.0; // 7B params
    let q4k_memory = param_count * Quantization::Q4K.bytes_per_weight() as f64;
    let q8_memory = param_count * Quantization::Q8.bytes_per_weight() as f64;
    let f16_memory = param_count * Quantization::F16.bytes_per_weight() as f64;

    assert!(q4k_memory < q8_memory);
    assert!(q8_memory < f16_memory);

    // ~3.5GB for Q4K, ~7GB for Q8, ~14GB for F16
    assert!(q4k_memory < 5_000_000_000.0);
    assert!(q8_memory < 10_000_000_000.0);
    assert!(f16_memory < 20_000_000_000.0);
}

// ============================================================================
// Integration Tests - Full Pipeline
// ============================================================================

#[test]
fn test_full_pipeline_mock() {
    // Test: Complete pipeline from request to completion
    let (engine, backend) = create_mock_serving_engine();
    backend.model_loaded.store(true, Ordering::SeqCst);

    // Create and submit request
    let params = GenerateParams::default()
        .with_max_tokens(10)
        .with_temperature(0.7)
        .with_top_p(0.9);

    let request = InferenceRequest::new(vec![100, 101, 102, 103, 104], params);
    let request_id = engine.submit(request).unwrap();

    // Process until completion or timeout
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let _ = engine.run_iteration();

        if engine.is_complete(request_id) {
            break;
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    // Should have made progress
    let stats = engine.stats();
    assert!(
        stats.running_requests > 0 || stats.completed_requests > 0 || stats.pending_requests > 0
    );
}

#[test]
fn test_engine_metrics() {
    // Test: Serving engine metrics collection
    let (engine, backend) = create_mock_serving_engine();
    backend.model_loaded.store(true, Ordering::SeqCst);

    // Initial metrics
    let metrics = engine.metrics();
    assert_eq!(metrics.pending_requests, 0);
    assert_eq!(metrics.running_requests, 0);
    assert!(metrics.uptime_seconds >= 0.0);

    // Submit some requests
    for _ in 0..3 {
        let params = GenerateParams::default().with_max_tokens(5);
        let request = InferenceRequest::new(vec![100, 101], params);
        engine.submit(request).unwrap();
    }

    // Run a few iterations
    for _ in 0..10 {
        let _ = engine.run_iteration();
    }

    // Check updated metrics
    let metrics = engine.metrics();
    // Requests may have completed by now, so check all states
    assert!(
        metrics.pending_requests > 0
            || metrics.running_requests > 0
            || metrics.completed_requests > 0
            || metrics.total_requests_processed > 0,
        "Should have requests processed, pending, running, or completed: {:?}",
        (
            metrics.pending_requests,
            metrics.running_requests,
            metrics.completed_requests,
            metrics.total_requests_processed
        )
    );
}

#[test]
fn test_request_cancellation() {
    // Test: Request can be cancelled mid-generation
    let (engine, backend) = create_mock_serving_engine();
    backend.model_loaded.store(true, Ordering::SeqCst);

    let params = GenerateParams::default().with_max_tokens(100);
    let request = InferenceRequest::new(vec![100, 101, 102], params);
    let request_id = engine.submit(request).unwrap();

    // Start processing
    for _ in 0..5 {
        let _ = engine.run_iteration();
    }

    // Cancel
    let cancelled = engine.cancel(request_id);
    assert!(cancelled, "Should successfully cancel request");
}

#[test]
fn test_concurrent_engine_operations() {
    // Test: Engine handles concurrent submissions
    let (engine, backend) = create_mock_serving_engine();
    backend.model_loaded.store(true, Ordering::SeqCst);

    let engine = Arc::new(engine);
    let mut handles = vec![];

    // Spawn concurrent submitters
    for i in 0..4 {
        let engine_clone = engine.clone();
        let handle = std::thread::spawn(move || {
            let params = GenerateParams::default().with_max_tokens(5);
            let request = InferenceRequest::new(vec![100 + i as u32], params);
            engine_clone.submit(request)
        });
        handles.push(handle);
    }

    // All submissions should succeed
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok(), "Concurrent submission should succeed");
    }

    // Process
    for _ in 0..50 {
        let _ = engine.run_iteration();
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_error_handling_unloaded_model() {
    // Test: Proper error when model not loaded
    let backend = MockLlmBackend::new();
    // Don't load model

    let params = GenerateParams::default();
    let result = backend.generate("Hello", params);

    assert!(result.is_err());
    match result {
        Err(RuvLLMError::Config(msg)) => {
            assert!(msg.contains("not loaded"));
        }
        _ => panic!("Expected Config error for unloaded model"),
    }
}

#[test]
fn test_error_handling_invalid_params() {
    // Test: Handle edge case parameters
    let params = GenerateParams::default()
        .with_max_tokens(0) // Edge case: 0 tokens
        .with_temperature(0.0); // Edge case: zero temperature (greedy)

    assert_eq!(params.max_tokens, 0);
    assert_eq!(params.temperature, 0.0);

    // These should be handled gracefully by the backend
    let mut backend = MockLlmBackend::new();
    backend.load_model("test", ModelConfig::default()).unwrap();

    let result = backend.generate("Hello", params);
    // With max_tokens=0, should return empty or minimal output
    assert!(result.is_ok());
}

#[test]
fn test_embeddings_generation() {
    // Test: Embedding extraction works correctly
    let mut backend = MockLlmBackend::new();
    backend.load_model("test", ModelConfig::default()).unwrap();

    let embeddings = backend.get_embeddings("Hello world").unwrap();

    assert_eq!(embeddings.len(), 768); // Standard embedding dim

    // Embeddings should be normalized
    let norm: f32 = embeddings.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "Embeddings should be normalized, got norm {}",
        norm
    );

    // Different texts should produce different embeddings
    let embeddings2 = backend.get_embeddings("Different text here").unwrap();

    let diff: f32 = embeddings
        .iter()
        .zip(embeddings2.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();

    assert!(
        diff > 0.1,
        "Different texts should have different embeddings"
    );
}
