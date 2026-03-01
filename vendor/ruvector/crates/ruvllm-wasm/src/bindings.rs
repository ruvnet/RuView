//! JavaScript/WASM Bindings for RuvLLM
//!
//! This module provides JavaScript-friendly wrappers around the RuvLLM
//! inference runtime. All types are designed to work seamlessly with
//! JavaScript through wasm-bindgen.
//!
//! # Example (JavaScript)
//!
//! ```javascript
//! import init, { RuvLLMWasm, GenerateConfig, KvCacheWasm } from 'ruvllm-wasm';
//!
//! await init();
//!
//! // Create inference engine
//! const llm = new RuvLLMWasm();
//!
//! // Configure generation
//! const config = new GenerateConfig();
//! config.maxTokens = 256;
//! config.temperature = 0.7;
//!
//! // Format a chat conversation
//! const template = ChatTemplateWasm.llama3();
//! const messages = [
//!     ChatMessageWasm.system("You are helpful."),
//!     ChatMessageWasm.user("Hello!"),
//! ];
//! const prompt = template.format(messages);
//! ```

use crate::utils::log;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use wasm_bindgen::prelude::*;

// ============================================================================
// Types (re-implemented for WASM self-containment)
// ============================================================================

/// Model size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelSize {
    Tiny,
    Small,
    Medium,
    Large,
}

impl Default for ModelSize {
    fn default() -> Self {
        Self::Small
    }
}

/// Precision levels for quantization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Precision {
    FP32,
    FP16,
    Q8,
    Q4K,
    Q4,
}

impl Default for Precision {
    fn default() -> Self {
        Self::FP16
    }
}

impl Precision {
    pub fn bytes_per_element(&self) -> f32 {
        match self {
            Self::FP32 => 4.0,
            Self::FP16 => 2.0,
            Self::Q8 => 1.0,
            Self::Q4K | Self::Q4 => 0.5,
        }
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Generation configuration for text generation.
///
/// Controls sampling parameters and output constraints.
/// TypeScript-friendly with getter/setter methods.
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateConfig {
    /// Maximum tokens to generate
    #[wasm_bindgen(skip)]
    pub max_tokens: usize,
    /// Temperature for sampling (0.0 = deterministic)
    #[wasm_bindgen(skip)]
    pub temperature: f32,
    /// Top-p (nucleus) sampling threshold
    #[wasm_bindgen(skip)]
    pub top_p: f32,
    /// Top-k sampling (0 = disabled)
    #[wasm_bindgen(skip)]
    pub top_k: usize,
    /// Repetition penalty (1.0 = no penalty)
    #[wasm_bindgen(skip)]
    pub repetition_penalty: f32,
    /// Stop sequences (JSON array of strings)
    #[wasm_bindgen(skip)]
    pub stop_sequences: Vec<String>,
}

#[wasm_bindgen]
impl GenerateConfig {
    /// Create a new GenerateConfig with default values.
    #[wasm_bindgen(constructor)]
    pub fn new() -> GenerateConfig {
        GenerateConfig {
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repetition_penalty: 1.1,
            stop_sequences: Vec::new(),
        }
    }

    /// Get maximum tokens.
    #[wasm_bindgen(getter, js_name = maxTokens)]
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Set maximum tokens.
    #[wasm_bindgen(setter, js_name = maxTokens)]
    pub fn set_max_tokens(&mut self, value: usize) {
        self.max_tokens = value;
    }

    /// Get temperature.
    #[wasm_bindgen(getter)]
    pub fn temperature(&self) -> f32 {
        self.temperature
    }

    /// Set temperature.
    #[wasm_bindgen(setter)]
    pub fn set_temperature(&mut self, value: f32) {
        self.temperature = value;
    }

    /// Get top-p value.
    #[wasm_bindgen(getter, js_name = topP)]
    pub fn top_p(&self) -> f32 {
        self.top_p
    }

    /// Set top-p value.
    #[wasm_bindgen(setter, js_name = topP)]
    pub fn set_top_p(&mut self, value: f32) {
        self.top_p = value;
    }

    /// Get top-k value.
    #[wasm_bindgen(getter, js_name = topK)]
    pub fn top_k(&self) -> usize {
        self.top_k
    }

    /// Set top-k value.
    #[wasm_bindgen(setter, js_name = topK)]
    pub fn set_top_k(&mut self, value: usize) {
        self.top_k = value;
    }

    /// Get repetition penalty.
    #[wasm_bindgen(getter, js_name = repetitionPenalty)]
    pub fn repetition_penalty(&self) -> f32 {
        self.repetition_penalty
    }

    /// Set repetition penalty.
    #[wasm_bindgen(setter, js_name = repetitionPenalty)]
    pub fn set_repetition_penalty(&mut self, value: f32) {
        self.repetition_penalty = value;
    }

    /// Add a stop sequence.
    #[wasm_bindgen(js_name = addStopSequence)]
    pub fn add_stop_sequence(&mut self, sequence: &str) {
        self.stop_sequences.push(sequence.to_string());
    }

    /// Clear all stop sequences.
    #[wasm_bindgen(js_name = clearStopSequences)]
    pub fn clear_stop_sequences(&mut self) {
        self.stop_sequences.clear();
    }

    /// Convert to JSON string.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create from JSON string.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<GenerateConfig, JsValue> {
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

impl Default for GenerateConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Chat Message Types
// ============================================================================

/// Message role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// Internal chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: Role::System,
            content: content.to_string(),
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: Role::User,
            content: content.to_string(),
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: Role::Assistant,
            content: content.to_string(),
        }
    }
}

/// Chat message for instruction-tuned models.
///
/// Used to construct conversations for chat-based inference.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ChatMessageWasm {
    inner: ChatMessage,
}

#[wasm_bindgen]
impl ChatMessageWasm {
    /// Create a system message.
    #[wasm_bindgen(js_name = system)]
    pub fn system(content: &str) -> ChatMessageWasm {
        ChatMessageWasm {
            inner: ChatMessage::system(content),
        }
    }

    /// Create a user message.
    #[wasm_bindgen(js_name = user)]
    pub fn user(content: &str) -> ChatMessageWasm {
        ChatMessageWasm {
            inner: ChatMessage::user(content),
        }
    }

    /// Create an assistant message.
    #[wasm_bindgen(js_name = assistant)]
    pub fn assistant(content: &str) -> ChatMessageWasm {
        ChatMessageWasm {
            inner: ChatMessage::assistant(content),
        }
    }

    /// Get the role as a string.
    #[wasm_bindgen(getter)]
    pub fn role(&self) -> String {
        self.inner.role.as_str().to_string()
    }

    /// Get the message content.
    #[wasm_bindgen(getter)]
    pub fn content(&self) -> String {
        self.inner.content.clone()
    }
}

// ============================================================================
// Chat Templates
// ============================================================================

/// Chat template variants
#[derive(Debug, Clone)]
pub enum ChatTemplate {
    Llama3,
    Llama2,
    Mistral,
    Qwen,
    ChatML,
    Phi,
    Gemma,
    Custom(String),
}

impl ChatTemplate {
    /// Detect template from model ID
    pub fn detect_from_model_id(model_id: &str) -> Self {
        let model_lower = model_id.to_lowercase();
        if model_lower.contains("llama-3") || model_lower.contains("llama3") {
            Self::Llama3
        } else if model_lower.contains("llama-2") || model_lower.contains("llama2") {
            Self::Llama2
        } else if model_lower.contains("mistral") || model_lower.contains("mixtral") {
            Self::Mistral
        } else if model_lower.contains("qwen") {
            Self::Qwen
        } else if model_lower.contains("phi") {
            Self::Phi
        } else if model_lower.contains("gemma") {
            Self::Gemma
        } else {
            Self::ChatML
        }
    }

    /// Format messages using this template
    pub fn format(&self, messages: &[ChatMessage]) -> String {
        match self {
            Self::Llama3 => self.format_llama3(messages),
            Self::Llama2 => self.format_llama2(messages),
            Self::Mistral => self.format_mistral(messages),
            Self::Qwen => self.format_qwen(messages),
            Self::ChatML => self.format_chatml(messages),
            Self::Phi => self.format_phi(messages),
            Self::Gemma => self.format_gemma(messages),
            Self::Custom(template) => self.format_custom(messages, template),
        }
    }

    fn format_llama3(&self, messages: &[ChatMessage]) -> String {
        let mut output = String::from("<|begin_of_text|>");

        for msg in messages {
            let role = msg.role.as_str();
            output.push_str(&format!(
                "<|start_header_id|>{}<|end_header_id|>\n\n{}<|eot_id|>",
                role, msg.content
            ));
        }

        output.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        output
    }

    fn format_llama2(&self, messages: &[ChatMessage]) -> String {
        let mut output = String::new();
        let mut system_msg = String::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    system_msg = msg.content.clone();
                }
                Role::User => {
                    if !system_msg.is_empty() {
                        output.push_str(&format!(
                            "<s>[INST] <<SYS>>\n{}\n<</SYS>>\n\n{} [/INST]",
                            system_msg, msg.content
                        ));
                        system_msg.clear();
                    } else {
                        output.push_str(&format!("<s>[INST] {} [/INST]", msg.content));
                    }
                }
                Role::Assistant => {
                    output.push_str(&format!(" {} </s>", msg.content));
                }
            }
        }

        output
    }

    fn format_mistral(&self, messages: &[ChatMessage]) -> String {
        let mut output = String::new();

        for msg in messages {
            match msg.role {
                Role::System | Role::User => {
                    output.push_str(&format!("[INST] {} [/INST]", msg.content));
                }
                Role::Assistant => {
                    output.push_str(&format!("{}</s>", msg.content));
                }
            }
        }

        output
    }

    fn format_qwen(&self, messages: &[ChatMessage]) -> String {
        self.format_chatml(messages)
    }

    fn format_chatml(&self, messages: &[ChatMessage]) -> String {
        let mut output = String::new();

        for msg in messages {
            output.push_str(&format!(
                "<|im_start|>{}\n{}<|im_end|>\n",
                msg.role.as_str(),
                msg.content
            ));
        }

        output.push_str("<|im_start|>assistant\n");
        output
    }

    fn format_phi(&self, messages: &[ChatMessage]) -> String {
        let mut output = String::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    output.push_str(&format!("<|system|>\n{}<|end|>\n", msg.content));
                }
                Role::User => {
                    output.push_str(&format!("<|user|>\n{}<|end|>\n", msg.content));
                }
                Role::Assistant => {
                    output.push_str(&format!("<|assistant|>\n{}<|end|>\n", msg.content));
                }
            }
        }

        output.push_str("<|assistant|>\n");
        output
    }

    fn format_gemma(&self, messages: &[ChatMessage]) -> String {
        let mut output = String::new();

        for msg in messages {
            match msg.role {
                Role::User => {
                    output.push_str(&format!(
                        "<start_of_turn>user\n{}<end_of_turn>\n",
                        msg.content
                    ));
                }
                Role::Assistant => {
                    output.push_str(&format!(
                        "<start_of_turn>model\n{}<end_of_turn>\n",
                        msg.content
                    ));
                }
                Role::System => {
                    // Gemma doesn't have native system support, prepend to first user
                    output.push_str(&format!("<start_of_turn>user\n{}\n", msg.content));
                }
            }
        }

        output.push_str("<start_of_turn>model\n");
        output
    }

    fn format_custom(&self, _messages: &[ChatMessage], _template: &str) -> String {
        // Simplified custom template support
        String::new()
    }
}

/// Chat template for formatting conversations.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ChatTemplateWasm {
    inner: ChatTemplate,
}

#[wasm_bindgen]
impl ChatTemplateWasm {
    /// Create a Llama 3 chat template.
    #[wasm_bindgen(js_name = llama3)]
    pub fn llama3() -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::Llama3,
        }
    }

    /// Create a Mistral chat template.
    #[wasm_bindgen(js_name = mistral)]
    pub fn mistral() -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::Mistral,
        }
    }

    /// Create a Qwen/ChatML chat template.
    #[wasm_bindgen(js_name = chatml)]
    pub fn chatml() -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::ChatML,
        }
    }

    /// Create a Phi chat template.
    #[wasm_bindgen(js_name = phi)]
    pub fn phi() -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::Phi,
        }
    }

    /// Create a Gemma chat template.
    #[wasm_bindgen(js_name = gemma)]
    pub fn gemma() -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::Gemma,
        }
    }

    /// Create a custom chat template.
    #[wasm_bindgen(js_name = custom)]
    pub fn custom(template: &str) -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::Custom(template.to_string()),
        }
    }

    /// Detect template from model ID.
    #[wasm_bindgen(js_name = detectFromModelId)]
    pub fn detect_from_model_id(model_id: &str) -> ChatTemplateWasm {
        ChatTemplateWasm {
            inner: ChatTemplate::detect_from_model_id(model_id),
        }
    }

    /// Format messages using this template.
    #[wasm_bindgen(js_name = format)]
    pub fn format(&self, messages: Vec<ChatMessageWasm>) -> String {
        let inner_messages: Vec<ChatMessage> = messages.into_iter().map(|m| m.inner).collect();
        self.inner.format(&inner_messages)
    }

    /// Get the template name.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        match &self.inner {
            ChatTemplate::Llama3 => "llama3".to_string(),
            ChatTemplate::Llama2 => "llama2".to_string(),
            ChatTemplate::Mistral => "mistral".to_string(),
            ChatTemplate::Qwen => "qwen".to_string(),
            ChatTemplate::ChatML => "chatml".to_string(),
            ChatTemplate::Phi => "phi".to_string(),
            ChatTemplate::Gemma => "gemma".to_string(),
            ChatTemplate::Custom(_) => "custom".to_string(),
        }
    }
}

// ============================================================================
// KV Cache
// ============================================================================

/// KV cache configuration for WASM.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct KvCacheConfigWasm {
    tail_length: usize,
    max_tokens: usize,
    num_kv_heads: usize,
    head_dim: usize,
}

#[wasm_bindgen]
impl KvCacheConfigWasm {
    /// Create a new KV cache configuration.
    #[wasm_bindgen(constructor)]
    pub fn new() -> KvCacheConfigWasm {
        KvCacheConfigWasm {
            tail_length: 256,
            max_tokens: 4096,
            num_kv_heads: 8,
            head_dim: 128,
        }
    }

    /// Get tail length.
    #[wasm_bindgen(getter, js_name = tailLength)]
    pub fn tail_length(&self) -> usize {
        self.tail_length
    }

    /// Set tail length.
    #[wasm_bindgen(setter, js_name = tailLength)]
    pub fn set_tail_length(&mut self, value: usize) {
        self.tail_length = value;
    }

    /// Get max tokens.
    #[wasm_bindgen(getter, js_name = maxTokens)]
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Set max tokens.
    #[wasm_bindgen(setter, js_name = maxTokens)]
    pub fn set_max_tokens(&mut self, value: usize) {
        self.max_tokens = value;
    }

    /// Get number of KV heads.
    #[wasm_bindgen(getter, js_name = numKvHeads)]
    pub fn num_kv_heads(&self) -> usize {
        self.num_kv_heads
    }

    /// Set number of KV heads.
    #[wasm_bindgen(setter, js_name = numKvHeads)]
    pub fn set_num_kv_heads(&mut self, value: usize) {
        self.num_kv_heads = value;
    }

    /// Get head dimension.
    #[wasm_bindgen(getter, js_name = headDim)]
    pub fn head_dim(&self) -> usize {
        self.head_dim
    }

    /// Set head dimension.
    #[wasm_bindgen(setter, js_name = headDim)]
    pub fn set_head_dim(&mut self, value: usize) {
        self.head_dim = value;
    }
}

impl Default for KvCacheConfigWasm {
    fn default() -> Self {
        Self::new()
    }
}

/// KV cache statistics.
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvCacheStatsWasm {
    total_tokens: usize,
    tail_tokens: usize,
    store_tokens: usize,
    tail_bytes: usize,
    store_bytes: usize,
    compression_ratio: f32,
}

#[wasm_bindgen]
impl KvCacheStatsWasm {
    /// Get total tokens.
    #[wasm_bindgen(getter, js_name = totalTokens)]
    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    /// Get tail tokens.
    #[wasm_bindgen(getter, js_name = tailTokens)]
    pub fn tail_tokens(&self) -> usize {
        self.tail_tokens
    }

    /// Get store tokens.
    #[wasm_bindgen(getter, js_name = storeTokens)]
    pub fn store_tokens(&self) -> usize {
        self.store_tokens
    }

    /// Get compression ratio.
    #[wasm_bindgen(getter, js_name = compressionRatio)]
    pub fn compression_ratio(&self) -> f32 {
        self.compression_ratio
    }

    /// Convert to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Two-tier KV cache for WASM.
///
/// Provides memory-efficient caching with a high-precision tail
/// and quantized store for older tokens.
#[wasm_bindgen]
pub struct KvCacheWasm {
    // FP16 tail cache (recent tokens)
    tail_keys: RefCell<VecDeque<Vec<f32>>>,
    tail_values: RefCell<VecDeque<Vec<f32>>>,
    // Quantized store (older tokens)
    store_keys: RefCell<VecDeque<Vec<u8>>>,
    store_values: RefCell<VecDeque<Vec<u8>>>,
    // Configuration
    config: KvCacheConfigWasm,
}

#[wasm_bindgen]
impl KvCacheWasm {
    /// Create a new KV cache with the given configuration.
    #[wasm_bindgen(constructor)]
    pub fn new(config: &KvCacheConfigWasm) -> KvCacheWasm {
        KvCacheWasm {
            tail_keys: RefCell::new(VecDeque::new()),
            tail_values: RefCell::new(VecDeque::new()),
            store_keys: RefCell::new(VecDeque::new()),
            store_values: RefCell::new(VecDeque::new()),
            config: config.clone(),
        }
    }

    /// Create with default configuration.
    #[wasm_bindgen(js_name = withDefaults)]
    pub fn with_defaults() -> KvCacheWasm {
        KvCacheWasm::new(&KvCacheConfigWasm::default())
    }

    /// Append KV pairs to the cache.
    #[wasm_bindgen]
    pub fn append(&self, keys: &[f32], values: &[f32]) -> Result<(), JsValue> {
        let stride = self.config.num_kv_heads * self.config.head_dim;

        if keys.len() != stride || values.len() != stride {
            return Err(JsValue::from_str(&format!(
                "Key/value length must be {} (num_kv_heads * head_dim)",
                stride
            )));
        }

        let mut tail_keys = self.tail_keys.borrow_mut();
        let mut tail_values = self.tail_values.borrow_mut();

        // Add to tail
        tail_keys.push_back(keys.to_vec());
        tail_values.push_back(values.to_vec());

        // Migrate from tail to store if needed
        while tail_keys.len() > self.config.tail_length {
            if let (Some(k), Some(v)) = (tail_keys.pop_front(), tail_values.pop_front()) {
                // Simple quantization: convert f32 to u8
                let quantized_k: Vec<u8> = k.iter().map(|&x| ((x + 1.0) * 127.5) as u8).collect();
                let quantized_v: Vec<u8> = v.iter().map(|&x| ((x + 1.0) * 127.5) as u8).collect();

                self.store_keys.borrow_mut().push_back(quantized_k);
                self.store_values.borrow_mut().push_back(quantized_v);
            }
        }

        // Evict from store if exceeds max tokens
        let total = tail_keys.len() + self.store_keys.borrow().len();
        if total > self.config.max_tokens {
            let excess = total - self.config.max_tokens;
            for _ in 0..excess {
                self.store_keys.borrow_mut().pop_front();
                self.store_values.borrow_mut().pop_front();
            }
        }

        Ok(())
    }

    /// Get all cached KV pairs.
    #[wasm_bindgen(js_name = getAllKv)]
    pub fn get_all_kv(&self) -> Result<JsValue, JsValue> {
        let stride = self.config.num_kv_heads * self.config.head_dim;

        // Dequantize store
        let store_keys = self.store_keys.borrow();
        let store_values = self.store_values.borrow();
        let tail_keys = self.tail_keys.borrow();
        let tail_values = self.tail_values.borrow();

        let total_tokens = store_keys.len() + tail_keys.len();
        let mut all_keys = Vec::with_capacity(total_tokens * stride);
        let mut all_values = Vec::with_capacity(total_tokens * stride);

        // Dequantize store
        for k in store_keys.iter() {
            for &b in k {
                all_keys.push((b as f32 / 127.5) - 1.0);
            }
        }
        for v in store_values.iter() {
            for &b in v {
                all_values.push((b as f32 / 127.5) - 1.0);
            }
        }

        // Add tail (already f32)
        for k in tail_keys.iter() {
            all_keys.extend(k);
        }
        for v in tail_values.iter() {
            all_values.extend(v);
        }

        let obj = js_sys::Object::new();
        let keys_array = js_sys::Float32Array::from(all_keys.as_slice());
        let values_array = js_sys::Float32Array::from(all_values.as_slice());

        js_sys::Reflect::set(&obj, &"keys".into(), &keys_array)?;
        js_sys::Reflect::set(&obj, &"values".into(), &values_array)?;

        Ok(obj.into())
    }

    /// Get cache statistics.
    #[wasm_bindgen]
    pub fn stats(&self) -> KvCacheStatsWasm {
        let stride = self.config.num_kv_heads * self.config.head_dim;
        let tail_tokens = self.tail_keys.borrow().len();
        let store_tokens = self.store_keys.borrow().len();

        let tail_bytes = tail_tokens * stride * 4; // f32
        let store_bytes = store_tokens * stride * 1; // u8

        let full_precision_bytes = (tail_tokens + store_tokens) * stride * 4;
        let actual_bytes = tail_bytes + store_bytes;
        let compression_ratio = if actual_bytes > 0 {
            full_precision_bytes as f32 / actual_bytes as f32
        } else {
            1.0
        };

        KvCacheStatsWasm {
            total_tokens: tail_tokens + store_tokens,
            tail_tokens,
            store_tokens,
            tail_bytes,
            store_bytes,
            compression_ratio,
        }
    }

    /// Clear the cache.
    #[wasm_bindgen]
    pub fn clear(&self) {
        self.tail_keys.borrow_mut().clear();
        self.tail_values.borrow_mut().clear();
        self.store_keys.borrow_mut().clear();
        self.store_values.borrow_mut().clear();
    }

    /// Get the total number of cached tokens.
    #[wasm_bindgen(getter, js_name = tokenCount)]
    pub fn token_count(&self) -> usize {
        self.tail_keys.borrow().len() + self.store_keys.borrow().len()
    }
}

// ============================================================================
// Memory Arena
// ============================================================================

const DEFAULT_ALIGNMENT: usize = 64;

/// Arena allocator for inference buffers.
///
/// Provides fast bump allocation with O(1) reset for
/// generation-step temporaries.
#[wasm_bindgen]
pub struct InferenceArenaWasm {
    data: RefCell<Vec<u8>>,
    offset: AtomicUsize,
    high_water_mark: AtomicUsize,
    allocation_count: AtomicUsize,
}

#[wasm_bindgen]
impl InferenceArenaWasm {
    /// Create a new arena with the specified capacity in bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: usize) -> InferenceArenaWasm {
        let aligned_capacity = (capacity + DEFAULT_ALIGNMENT - 1) & !(DEFAULT_ALIGNMENT - 1);
        InferenceArenaWasm {
            data: RefCell::new(vec![0u8; aligned_capacity]),
            offset: AtomicUsize::new(0),
            high_water_mark: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
        }
    }

    /// Create an arena sized for model dimensions.
    #[wasm_bindgen(js_name = forModel)]
    pub fn for_model(
        hidden_dim: usize,
        vocab_size: usize,
        batch_size: usize,
    ) -> InferenceArenaWasm {
        let activations = hidden_dim * batch_size * 4;
        let logits = vocab_size * batch_size * 4;
        let scratch = hidden_dim * 4 * 4;
        let total = (activations + logits + scratch) * 2;
        InferenceArenaWasm::new(total)
    }

    /// Reset the arena, making all memory available for reuse.
    #[wasm_bindgen]
    pub fn reset(&self) {
        self.offset.store(0, Ordering::Release);
        self.allocation_count.store(0, Ordering::Relaxed);
    }

    /// Get current bytes used.
    #[wasm_bindgen(getter)]
    pub fn used(&self) -> usize {
        self.offset.load(Ordering::Acquire)
    }

    /// Get total capacity.
    #[wasm_bindgen(getter)]
    pub fn capacity(&self) -> usize {
        self.data.borrow().len()
    }

    /// Get remaining available bytes.
    #[wasm_bindgen(getter)]
    pub fn remaining(&self) -> usize {
        self.capacity() - self.used()
    }

    /// Get high water mark (maximum bytes ever used).
    #[wasm_bindgen(getter, js_name = highWaterMark)]
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark.load(Ordering::Relaxed)
    }

    /// Get statistics as JSON.
    #[wasm_bindgen(js_name = statsJson)]
    pub fn stats_json(&self) -> Result<String, JsValue> {
        let capacity = self.capacity();
        let used = self.used();

        let stats = serde_json::json!({
            "capacity": capacity,
            "used": used,
            "remaining": capacity - used,
            "high_water_mark": self.high_water_mark(),
            "allocation_count": self.allocation_count.load(Ordering::Relaxed),
            "utilization": if capacity > 0 { used as f64 / capacity as f64 } else { 0.0 }
        });

        serde_json::to_string(&stats).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// Buffer Pool
// ============================================================================

/// Buffer pool for efficient memory reuse.
#[wasm_bindgen]
pub struct BufferPoolWasm {
    free_lists: RefCell<[Vec<Vec<u8>>; 5]>,
    max_per_class: usize,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

const BUFFER_SIZES: [usize; 5] = [1024, 4096, 16384, 65536, 262144];

#[wasm_bindgen]
impl BufferPoolWasm {
    /// Create a new buffer pool with default settings.
    #[wasm_bindgen(constructor)]
    pub fn new() -> BufferPoolWasm {
        BufferPoolWasm::with_capacity(32)
    }

    /// Create with specified max buffers per size class.
    #[wasm_bindgen(js_name = withCapacity)]
    pub fn with_capacity(max_buffers_per_class: usize) -> BufferPoolWasm {
        BufferPoolWasm {
            free_lists: RefCell::new([
                Vec::with_capacity(max_buffers_per_class),
                Vec::with_capacity(max_buffers_per_class),
                Vec::with_capacity(max_buffers_per_class),
                Vec::with_capacity(max_buffers_per_class),
                Vec::with_capacity(max_buffers_per_class),
            ]),
            max_per_class: max_buffers_per_class,
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }

    /// Pre-warm the pool by allocating buffers.
    #[wasm_bindgen(js_name = prewarmAll)]
    pub fn prewarm_all(&self, count_per_class: usize) {
        let mut lists = self.free_lists.borrow_mut();
        for (i, size) in BUFFER_SIZES.iter().enumerate() {
            for _ in 0..count_per_class.min(self.max_per_class) {
                if lists[i].len() < self.max_per_class {
                    lists[i].push(vec![0u8; *size]);
                }
            }
        }
    }

    /// Get pool statistics as JSON.
    #[wasm_bindgen(js_name = statsJson)]
    pub fn stats_json(&self) -> Result<String, JsValue> {
        let lists = self.free_lists.borrow();
        let free_buffers: Vec<usize> = lists.iter().map(|l| l.len()).collect();
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        let stats = serde_json::json!({
            "hits": hits,
            "misses": misses,
            "allocations": misses,
            "returns": hits,
            "drops": 0,
            "free_buffers": free_buffers,
            "hit_rate": if total > 0 { hits as f64 / total as f64 } else { 0.0 }
        });

        serde_json::to_string(&stats).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get the hit rate (0.0 - 1.0).
    #[wasm_bindgen(getter, js_name = hitRate)]
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let total = hits + self.misses.load(Ordering::Relaxed);
        if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        }
    }

    /// Clear all pooled buffers.
    #[wasm_bindgen]
    pub fn clear(&self) {
        let mut lists = self.free_lists.borrow_mut();
        for list in lists.iter_mut() {
            list.clear();
        }
    }
}

impl Default for BufferPoolWasm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Main RuvLLM WASM Interface
// ============================================================================

/// Main RuvLLM WASM interface.
///
/// Provides the primary entry point for LLM inference in the browser.
/// Manages KV cache, memory pools, and inference state.
#[wasm_bindgen]
pub struct RuvLLMWasm {
    kv_cache: Option<KvCacheWasm>,
    buffer_pool: BufferPoolWasm,
    initialized: bool,
}

#[wasm_bindgen]
impl RuvLLMWasm {
    /// Create a new RuvLLM WASM instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> RuvLLMWasm {
        crate::utils::set_panic_hook();

        RuvLLMWasm {
            kv_cache: None,
            buffer_pool: BufferPoolWasm::new(),
            initialized: false,
        }
    }

    /// Initialize the engine with default configuration.
    #[wasm_bindgen]
    pub fn initialize(&mut self) -> Result<(), JsValue> {
        self.initialize_with_config(&KvCacheConfigWasm::default())
    }

    /// Initialize with custom KV cache configuration.
    #[wasm_bindgen(js_name = initializeWithConfig)]
    pub fn initialize_with_config(&mut self, config: &KvCacheConfigWasm) -> Result<(), JsValue> {
        log("Initializing RuvLLM WASM...");

        self.kv_cache = Some(KvCacheWasm::new(config));
        self.buffer_pool.prewarm_all(4);
        self.initialized = true;

        log("RuvLLM WASM initialized successfully");
        Ok(())
    }

    /// Check if the engine is initialized.
    #[wasm_bindgen(getter, js_name = isInitialized)]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get buffer pool statistics.
    #[wasm_bindgen(js_name = getPoolStats)]
    pub fn get_pool_stats(&self) -> Result<String, JsValue> {
        self.buffer_pool.stats_json()
    }

    /// Clear all caches and reset state.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        if let Some(cache) = &self.kv_cache {
            cache.clear();
        }
        self.buffer_pool.clear();
        log("RuvLLM WASM state reset");
    }

    /// Get version information.
    #[wasm_bindgen(js_name = version)]
    pub fn version() -> String {
        "2.0.0".to_string()
    }

    /// Format a chat conversation using a template.
    #[wasm_bindgen(js_name = formatChat)]
    pub fn format_chat(template: &ChatTemplateWasm, messages: Vec<ChatMessageWasm>) -> String {
        let inner_messages: Vec<ChatMessage> = messages.into_iter().map(|m| m.inner).collect();
        template.inner.format(&inner_messages)
    }
}

impl Default for RuvLLMWasm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Utility Exports
// ============================================================================

/// Get the WASM module version.
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    "2.0.0".to_string()
}

/// Check if the WASM module is ready.
#[wasm_bindgen(js_name = isReady)]
pub fn is_ready() -> bool {
    true
}

/// Detect chat template from model ID.
#[wasm_bindgen(js_name = detectChatTemplate)]
pub fn detect_chat_template(model_id: &str) -> ChatTemplateWasm {
    ChatTemplateWasm::detect_from_model_id(model_id)
}
