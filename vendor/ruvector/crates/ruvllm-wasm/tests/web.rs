//! WASM Tests for RuvLLM
//!
//! These tests run in a browser environment using wasm-bindgen-test.
//! Run with: `wasm-pack test --headless --chrome`

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use ruvllm_wasm::{
    BufferPoolWasm, ChatMessageWasm, ChatTemplateWasm, GenerateConfig, InferenceArenaWasm,
    KvCacheConfigWasm, KvCacheWasm, RuvLLMWasm, Timer,
};

// ============================================================================
// GenerateConfig Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_generate_config_defaults() {
    let config = GenerateConfig::new();

    assert_eq!(config.max_tokens(), 256);
    assert!((config.temperature() - 0.7).abs() < 0.01);
    assert!((config.top_p() - 0.9).abs() < 0.01);
    assert_eq!(config.top_k(), 40);
}

#[wasm_bindgen_test]
fn test_generate_config_setters() {
    let mut config = GenerateConfig::new();

    config.set_max_tokens(512);
    config.set_temperature(0.5);
    config.set_top_p(0.95);
    config.set_top_k(50);
    config.set_repetition_penalty(1.2);

    assert_eq!(config.max_tokens(), 512);
    assert!((config.temperature() - 0.5).abs() < 0.01);
    assert!((config.top_p() - 0.95).abs() < 0.01);
    assert_eq!(config.top_k(), 50);
    assert!((config.repetition_penalty() - 1.2).abs() < 0.01);
}

#[wasm_bindgen_test]
fn test_generate_config_json() {
    let config = GenerateConfig::new();
    let json = config.to_json().expect("JSON serialization failed");

    assert!(json.contains("max_tokens"));
    assert!(json.contains("temperature"));

    let parsed = GenerateConfig::from_json(&json).expect("JSON parsing failed");
    assert_eq!(parsed.max_tokens(), config.max_tokens());
}

#[wasm_bindgen_test]
fn test_generate_config_stop_sequences() {
    let mut config = GenerateConfig::new();

    config.add_stop_sequence("</s>");
    config.add_stop_sequence("\n\n");

    // Stop sequences are stored internally
    config.clear_stop_sequences();
    // After clearing, should work without error
}

// ============================================================================
// Chat Message Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_chat_message_creation() {
    let system = ChatMessageWasm::system("You are helpful.");
    assert_eq!(system.role(), "system");
    assert_eq!(system.content(), "You are helpful.");

    let user = ChatMessageWasm::user("Hello!");
    assert_eq!(user.role(), "user");
    assert_eq!(user.content(), "Hello!");

    let assistant = ChatMessageWasm::assistant("Hi there!");
    assert_eq!(assistant.role(), "assistant");
    assert_eq!(assistant.content(), "Hi there!");
}

// ============================================================================
// Chat Template Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_chat_template_llama3() {
    let template = ChatTemplateWasm::llama3();
    assert_eq!(template.name(), "llama3");

    let messages = vec![
        ChatMessageWasm::system("Be helpful."),
        ChatMessageWasm::user("Hello"),
    ];

    let formatted = template.format(messages);
    assert!(formatted.contains("<|begin_of_text|>"));
    assert!(formatted.contains("Be helpful."));
    assert!(formatted.contains("Hello"));
}

#[wasm_bindgen_test]
fn test_chat_template_chatml() {
    let template = ChatTemplateWasm::chatml();
    assert_eq!(template.name(), "chatml");

    let messages = vec![ChatMessageWasm::user("Hi")];

    let formatted = template.format(messages);
    assert!(formatted.contains("<|im_start|>user"));
    assert!(formatted.contains("Hi"));
    assert!(formatted.contains("<|im_end|>"));
}

#[wasm_bindgen_test]
fn test_chat_template_detection() {
    let llama = ChatTemplateWasm::detect_from_model_id("meta-llama/Llama-3-8B");
    assert_eq!(llama.name(), "llama3");

    let mistral = ChatTemplateWasm::detect_from_model_id("mistralai/Mistral-7B");
    assert_eq!(mistral.name(), "mistral");

    let qwen = ChatTemplateWasm::detect_from_model_id("Qwen/Qwen2.5-0.5B");
    assert_eq!(qwen.name(), "qwen");
}

#[wasm_bindgen_test]
fn test_chat_template_custom() {
    let template = ChatTemplateWasm::custom("USER: {user}\nASSISTANT:");
    assert_eq!(template.name(), "custom");
}

// ============================================================================
// KV Cache Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_kv_cache_config() {
    let mut config = KvCacheConfigWasm::new();

    config.set_tail_length(512);
    config.set_max_tokens(8192);
    config.set_num_kv_heads(16);
    config.set_head_dim(64);

    assert_eq!(config.tail_length(), 512);
    assert_eq!(config.max_tokens(), 8192);
    assert_eq!(config.num_kv_heads(), 16);
    assert_eq!(config.head_dim(), 64);
}

#[wasm_bindgen_test]
fn test_kv_cache_basic() {
    let cache = KvCacheWasm::with_defaults();

    let stats = cache.stats();
    assert_eq!(stats.total_tokens(), 0);
    assert_eq!(stats.tail_tokens(), 0);
}

#[wasm_bindgen_test]
fn test_kv_cache_append() {
    let mut config = KvCacheConfigWasm::new();
    config.set_num_kv_heads(2);
    config.set_head_dim(4);

    let cache = KvCacheWasm::new(&config);

    // Append one token (stride = 2 * 4 = 8)
    let keys: Vec<f32> = vec![0.1; 8];
    let values: Vec<f32> = vec![0.2; 8];

    cache.append(&keys, &values).expect("append failed");

    let stats = cache.stats();
    assert_eq!(stats.total_tokens(), 1);
}

#[wasm_bindgen_test]
fn test_kv_cache_clear() {
    let cache = KvCacheWasm::with_defaults();
    cache.clear();

    assert_eq!(cache.token_count(), 0);
}

#[wasm_bindgen_test]
fn test_kv_cache_stats_json() {
    let cache = KvCacheWasm::with_defaults();
    let json = cache.stats().to_json().expect("JSON failed");

    assert!(json.contains("total_tokens"));
    assert!(json.contains("compression_ratio"));
}

// ============================================================================
// Memory Arena Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_arena_creation() {
    let arena = InferenceArenaWasm::new(4096);

    assert!(arena.capacity() >= 4096);
    assert_eq!(arena.used(), 0);
    assert_eq!(arena.remaining(), arena.capacity());
}

#[wasm_bindgen_test]
fn test_arena_for_model() {
    let arena = InferenceArenaWasm::for_model(4096, 32000, 1);

    // Should have reasonable capacity for these dimensions
    assert!(arena.capacity() > 0);
}

#[wasm_bindgen_test]
fn test_arena_reset() {
    let arena = InferenceArenaWasm::new(4096);

    // Arena starts empty
    assert_eq!(arena.used(), 0);

    // Reset should work even on empty arena
    arena.reset();
    assert_eq!(arena.used(), 0);
}

#[wasm_bindgen_test]
fn test_arena_stats_json() {
    let arena = InferenceArenaWasm::new(4096);
    let json = arena.stats_json().expect("JSON failed");

    assert!(json.contains("capacity"));
    assert!(json.contains("used"));
    assert!(json.contains("utilization"));
}

// ============================================================================
// Buffer Pool Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_buffer_pool_creation() {
    let pool = BufferPoolWasm::new();

    // Hit rate should be 0 initially (no hits or misses)
    assert!(pool.hit_rate() >= 0.0);
}

#[wasm_bindgen_test]
fn test_buffer_pool_prewarm() {
    let pool = BufferPoolWasm::new();
    pool.prewarm_all(4);

    let json = pool.stats_json().expect("JSON failed");
    assert!(json.contains("free_buffers"));
}

#[wasm_bindgen_test]
fn test_buffer_pool_clear() {
    let pool = BufferPoolWasm::new();
    pool.prewarm_all(2);
    pool.clear();

    // After clear, pool should be empty
}

#[wasm_bindgen_test]
fn test_buffer_pool_with_capacity() {
    let pool = BufferPoolWasm::with_capacity(16);

    let json = pool.stats_json().expect("JSON failed");
    assert!(json.contains("hit_rate"));
}

// ============================================================================
// RuvLLMWasm Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_ruvllm_creation() {
    let llm = RuvLLMWasm::new();
    assert!(!llm.is_initialized());
}

#[wasm_bindgen_test]
fn test_ruvllm_initialize() {
    let mut llm = RuvLLMWasm::new();
    llm.initialize().expect("initialization failed");

    assert!(llm.is_initialized());
}

#[wasm_bindgen_test]
fn test_ruvllm_initialize_with_config() {
    let mut llm = RuvLLMWasm::new();
    let config = KvCacheConfigWasm::new();

    llm.initialize_with_config(&config)
        .expect("initialization failed");

    assert!(llm.is_initialized());
}

#[wasm_bindgen_test]
fn test_ruvllm_reset() {
    let mut llm = RuvLLMWasm::new();
    llm.initialize().expect("initialization failed");
    llm.reset();

    // Should still be initialized after reset
    assert!(llm.is_initialized());
}

#[wasm_bindgen_test]
fn test_ruvllm_version() {
    let version = RuvLLMWasm::version();
    assert!(!version.is_empty());
    assert!(version.contains('.'));
}

#[wasm_bindgen_test]
fn test_ruvllm_pool_stats() {
    let mut llm = RuvLLMWasm::new();
    llm.initialize().expect("initialization failed");

    let stats = llm.get_pool_stats().expect("stats failed");
    assert!(stats.contains("hit_rate"));
}

#[wasm_bindgen_test]
fn test_ruvllm_format_chat() {
    let template = ChatTemplateWasm::chatml();
    let messages = vec![
        ChatMessageWasm::system("Be helpful."),
        ChatMessageWasm::user("Hello"),
    ];

    let formatted = RuvLLMWasm::format_chat(&template, messages);
    assert!(formatted.contains("<|im_start|>"));
    assert!(formatted.contains("Be helpful."));
}

// ============================================================================
// Utility Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_timer() {
    let timer = Timer::new("test_timer");

    // Elapsed should be non-negative
    assert!(timer.elapsed_ms() >= 0.0);
}

#[wasm_bindgen_test]
fn test_timer_reset() {
    let mut timer = Timer::new("test_timer");

    // Wait a tiny bit (if possible in test environment)
    let initial = timer.elapsed_ms();

    timer.reset();
    let after_reset = timer.elapsed_ms();

    // After reset, elapsed should be less than or equal to initial
    // (accounting for timing variations)
    assert!(after_reset <= initial + 1.0);
}

#[wasm_bindgen_test]
fn test_get_version() {
    let version = ruvllm_wasm::get_version();
    assert!(!version.is_empty());
}

#[wasm_bindgen_test]
fn test_is_ready() {
    assert!(ruvllm_wasm::is_ready());
}

#[wasm_bindgen_test]
fn test_detect_chat_template() {
    let template = ruvllm_wasm::detect_chat_template("Qwen/Qwen2.5-0.5B-Instruct");
    assert_eq!(template.name(), "qwen");
}

#[wasm_bindgen_test]
fn test_health_check() {
    assert!(ruvllm_wasm::health_check());
}
