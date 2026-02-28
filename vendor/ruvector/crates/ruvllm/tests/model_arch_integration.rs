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
//! Integration tests for v2.1 model architectures (Phi-3, Gemma-2)
//!
//! Tests cover:
//! - Model configuration creation and validation
//! - Chat template formatting
//! - Sliding window attention
//! - Logit soft capping
//! - Grouped Query Attention (GQA)
//! - RoPE (Rotary Position Embedding) configurations

// =============================================================================
// Model Configuration Types
// =============================================================================

/// Attention type for different model architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionType {
    /// Multi-Head Attention (standard)
    MHA,
    /// Grouped Query Attention (fewer KV heads)
    GQA,
    /// Multi-Query Attention (single KV head)
    MQA,
}

/// RoPE scaling configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RopeScaling {
    pub scaling_type: RopeScalingType,
    pub factor: f32,
    pub low_freq_factor: Option<f32>,
    pub high_freq_factor: Option<f32>,
    pub original_max_position_embeddings: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RopeScalingType {
    Linear,
    Dynamic,
    Yarn,
    Longrope,
    Su,
}

impl Default for RopeScaling {
    fn default() -> Self {
        Self {
            scaling_type: RopeScalingType::Linear,
            factor: 1.0,
            low_freq_factor: None,
            high_freq_factor: None,
            original_max_position_embeddings: None,
        }
    }
}

// =============================================================================
// Phi-3 Configuration
// =============================================================================

/// Configuration for Phi-3 family models
#[derive(Debug, Clone)]
pub struct Phi3Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub hidden_act: String,
    pub max_position_embeddings: usize,
    pub original_max_position_embeddings: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub rope_scaling: Option<RopeScaling>,
    pub sliding_window: Option<usize>,
    pub attention_bias: bool,
    pub attention_dropout: f32,
    pub bos_token_id: u32,
    pub eos_token_id: u32,
}

impl Phi3Config {
    /// Create configuration for Phi-3-mini (3.8B parameters)
    pub fn phi3_mini() -> Self {
        Self {
            vocab_size: 32064,
            hidden_size: 3072,
            intermediate_size: 8192,
            num_hidden_layers: 32,
            num_attention_heads: 32,
            num_key_value_heads: 32,
            hidden_act: "silu".to_string(),
            max_position_embeddings: 131072,
            original_max_position_embeddings: 4096,
            rms_norm_eps: 1e-5,
            rope_theta: 10000.0,
            rope_scaling: Some(RopeScaling {
                scaling_type: RopeScalingType::Longrope,
                factor: 1.0,
                low_freq_factor: Some(1.0),
                high_freq_factor: Some(4.0),
                original_max_position_embeddings: Some(4096),
            }),
            sliding_window: None,
            attention_bias: false,
            attention_dropout: 0.0,
            bos_token_id: 1,
            eos_token_id: 32000,
        }
    }

    /// Create configuration for Phi-3-small (7B parameters)
    pub fn phi3_small() -> Self {
        Self {
            vocab_size: 100352,
            hidden_size: 4096,
            intermediate_size: 14336,
            num_hidden_layers: 32,
            num_attention_heads: 32,
            num_key_value_heads: 8, // GQA with 4:1 ratio
            hidden_act: "silu".to_string(),
            max_position_embeddings: 131072,
            original_max_position_embeddings: 8192,
            rms_norm_eps: 1e-5,
            rope_theta: 10000.0,
            rope_scaling: Some(RopeScaling {
                scaling_type: RopeScalingType::Longrope,
                factor: 1.0,
                low_freq_factor: Some(1.0),
                high_freq_factor: Some(4.0),
                original_max_position_embeddings: Some(8192),
            }),
            sliding_window: None,
            attention_bias: false,
            attention_dropout: 0.0,
            bos_token_id: 100257,
            eos_token_id: 100257,
        }
    }

    /// Create configuration for Phi-3-medium (14B parameters)
    pub fn phi3_medium() -> Self {
        Self {
            vocab_size: 32064,
            hidden_size: 5120,
            intermediate_size: 17920,
            num_hidden_layers: 40,
            num_attention_heads: 40,
            num_key_value_heads: 10, // GQA with 4:1 ratio
            hidden_act: "silu".to_string(),
            max_position_embeddings: 131072,
            original_max_position_embeddings: 4096,
            rms_norm_eps: 1e-5,
            rope_theta: 10000.0,
            rope_scaling: Some(RopeScaling {
                scaling_type: RopeScalingType::Longrope,
                factor: 1.0,
                low_freq_factor: Some(1.0),
                high_freq_factor: Some(4.0),
                original_max_position_embeddings: Some(4096),
            }),
            sliding_window: None,
            attention_bias: false,
            attention_dropout: 0.0,
            bos_token_id: 1,
            eos_token_id: 32000,
        }
    }

    /// Get attention type based on head configuration
    pub fn attention_type(&self) -> AttentionType {
        if self.num_key_value_heads == 1 {
            AttentionType::MQA
        } else if self.num_key_value_heads < self.num_attention_heads {
            AttentionType::GQA
        } else {
            AttentionType::MHA
        }
    }

    /// Calculate head dimension
    pub fn head_dim(&self) -> usize {
        self.hidden_size / self.num_attention_heads
    }

    /// Calculate KV groups for GQA
    pub fn num_kv_groups(&self) -> usize {
        self.num_attention_heads / self.num_key_value_heads
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.hidden_size % self.num_attention_heads != 0 {
            return Err("hidden_size must be divisible by num_attention_heads".to_string());
        }
        if self.num_attention_heads % self.num_key_value_heads != 0 {
            return Err("num_attention_heads must be divisible by num_key_value_heads".to_string());
        }
        if self.vocab_size == 0 {
            return Err("vocab_size must be greater than 0".to_string());
        }
        Ok(())
    }
}

// =============================================================================
// Gemma-2 Configuration
// =============================================================================

/// Configuration for Gemma-2 family models
#[derive(Debug, Clone)]
pub struct Gemma2Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub head_dim: usize,
    pub hidden_act: String,
    pub hidden_activation: String,
    pub max_position_embeddings: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub attention_bias: bool,
    pub attention_dropout: f32,
    /// Sliding window size for local attention layers
    pub sliding_window: usize,
    /// Query pre-attention scalar
    pub query_pre_attn_scalar: f32,
    /// Logit soft capping value for attention
    pub attn_logit_softcapping: f32,
    /// Logit soft capping value for final logits
    pub final_logit_softcapping: f32,
    pub bos_token_id: u32,
    pub eos_token_id: u32,
    pub pad_token_id: u32,
}

impl Gemma2Config {
    /// Create configuration for Gemma-2-2B
    pub fn gemma2_2b() -> Self {
        Self {
            vocab_size: 256000,
            hidden_size: 2304,
            intermediate_size: 9216,
            num_hidden_layers: 26,
            num_attention_heads: 8,
            num_key_value_heads: 4,
            head_dim: 256,
            hidden_act: "gelu_pytorch_tanh".to_string(),
            hidden_activation: "gelu_pytorch_tanh".to_string(),
            max_position_embeddings: 8192,
            rms_norm_eps: 1e-6,
            rope_theta: 10000.0,
            attention_bias: false,
            attention_dropout: 0.0,
            sliding_window: 4096,
            query_pre_attn_scalar: 256.0,
            attn_logit_softcapping: 50.0,
            final_logit_softcapping: 30.0,
            bos_token_id: 2,
            eos_token_id: 1,
            pad_token_id: 0,
        }
    }

    /// Create configuration for Gemma-2-9B
    pub fn gemma2_9b() -> Self {
        Self {
            vocab_size: 256000,
            hidden_size: 3584,
            intermediate_size: 14336,
            num_hidden_layers: 42,
            num_attention_heads: 16,
            num_key_value_heads: 8,
            head_dim: 256,
            hidden_act: "gelu_pytorch_tanh".to_string(),
            hidden_activation: "gelu_pytorch_tanh".to_string(),
            max_position_embeddings: 8192,
            rms_norm_eps: 1e-6,
            rope_theta: 10000.0,
            attention_bias: false,
            attention_dropout: 0.0,
            sliding_window: 4096,
            query_pre_attn_scalar: 256.0,
            attn_logit_softcapping: 50.0,
            final_logit_softcapping: 30.0,
            bos_token_id: 2,
            eos_token_id: 1,
            pad_token_id: 0,
        }
    }

    /// Create configuration for Gemma-2-27B
    pub fn gemma2_27b() -> Self {
        Self {
            vocab_size: 256000,
            hidden_size: 4608,
            intermediate_size: 36864,
            num_hidden_layers: 46,
            num_attention_heads: 32,
            num_key_value_heads: 16,
            head_dim: 128,
            hidden_act: "gelu_pytorch_tanh".to_string(),
            hidden_activation: "gelu_pytorch_tanh".to_string(),
            max_position_embeddings: 8192,
            rms_norm_eps: 1e-6,
            rope_theta: 10000.0,
            attention_bias: false,
            attention_dropout: 0.0,
            sliding_window: 4096,
            query_pre_attn_scalar: 128.0,
            attn_logit_softcapping: 50.0,
            final_logit_softcapping: 30.0,
            bos_token_id: 2,
            eos_token_id: 1,
            pad_token_id: 0,
        }
    }

    /// Get attention type based on head configuration
    pub fn attention_type(&self) -> AttentionType {
        if self.num_key_value_heads == 1 {
            AttentionType::MQA
        } else if self.num_key_value_heads < self.num_attention_heads {
            AttentionType::GQA
        } else {
            AttentionType::MHA
        }
    }

    /// Calculate KV groups for GQA
    pub fn num_kv_groups(&self) -> usize {
        self.num_attention_heads / self.num_key_value_heads
    }

    /// Check if a layer uses sliding window attention
    pub fn uses_sliding_window(&self, layer_idx: usize) -> bool {
        // Gemma-2 uses alternating global and local attention
        // Even layers use global, odd layers use sliding window
        layer_idx % 2 == 1
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.num_attention_heads % self.num_key_value_heads != 0 {
            return Err("num_attention_heads must be divisible by num_key_value_heads".to_string());
        }
        if self.vocab_size == 0 {
            return Err("vocab_size must be greater than 0".to_string());
        }
        if self.attn_logit_softcapping <= 0.0 {
            return Err("attn_logit_softcapping must be positive".to_string());
        }
        if self.final_logit_softcapping <= 0.0 {
            return Err("final_logit_softcapping must be positive".to_string());
        }
        Ok(())
    }
}

// =============================================================================
// Chat Template System
// =============================================================================

/// Chat message role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Chat template for formatting conversations
pub trait ChatTemplate {
    /// Format a list of messages into a prompt string
    fn format(&self, messages: &[ChatMessage], add_generation_prompt: bool) -> String;

    /// Get the template name
    fn name(&self) -> &str;
}

/// Phi-3 chat template
pub struct Phi3ChatTemplate;

impl ChatTemplate for Phi3ChatTemplate {
    fn format(&self, messages: &[ChatMessage], add_generation_prompt: bool) -> String {
        let mut result = String::new();

        for message in messages {
            match message.role {
                Role::System => {
                    result.push_str(&format!("<|system|>\n{}<|end|>\n", message.content));
                }
                Role::User => {
                    result.push_str(&format!("<|user|>\n{}<|end|>\n", message.content));
                }
                Role::Assistant => {
                    result.push_str(&format!("<|assistant|>\n{}<|end|>\n", message.content));
                }
            }
        }

        if add_generation_prompt {
            result.push_str("<|assistant|>\n");
        }

        result
    }

    fn name(&self) -> &str {
        "phi3"
    }
}

/// Gemma chat template
pub struct GemmaChatTemplate;

impl ChatTemplate for GemmaChatTemplate {
    fn format(&self, messages: &[ChatMessage], add_generation_prompt: bool) -> String {
        let mut result = String::new();

        for message in messages {
            match message.role {
                Role::System => {
                    // Gemma doesn't have a system role, prepend to first user message
                    result.push_str(&format!("<start_of_turn>user\n{}", message.content));
                }
                Role::User => {
                    if result.ends_with("<start_of_turn>user\n") {
                        // System message was added, append to it
                        result.push_str(&format!("\n\n{}<end_of_turn>\n", message.content));
                    } else {
                        result.push_str(&format!(
                            "<start_of_turn>user\n{}<end_of_turn>\n",
                            message.content
                        ));
                    }
                }
                Role::Assistant => {
                    result.push_str(&format!(
                        "<start_of_turn>model\n{}<end_of_turn>\n",
                        message.content
                    ));
                }
            }
        }

        if add_generation_prompt {
            result.push_str("<start_of_turn>model\n");
        }

        result
    }

    fn name(&self) -> &str {
        "gemma"
    }
}

/// ChatML template (used by many models)
pub struct ChatMLTemplate;

impl ChatTemplate for ChatMLTemplate {
    fn format(&self, messages: &[ChatMessage], add_generation_prompt: bool) -> String {
        let mut result = String::new();

        for message in messages {
            let role = match message.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            result.push_str(&format!(
                "<|im_start|>{}\n{}<|im_end|>\n",
                role, message.content
            ));
        }

        if add_generation_prompt {
            result.push_str("<|im_start|>assistant\n");
        }

        result
    }

    fn name(&self) -> &str {
        "chatml"
    }
}

// =============================================================================
// Sliding Window Attention
// =============================================================================

/// Sliding window attention mask generator
pub struct SlidingWindowMask {
    window_size: usize,
}

impl SlidingWindowMask {
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }

    /// Generate attention mask for a given sequence length
    /// Returns a 2D mask where true = attend, false = mask
    pub fn generate_mask(&self, seq_len: usize) -> Vec<Vec<bool>> {
        let mut mask = vec![vec![false; seq_len]; seq_len];

        for i in 0..seq_len {
            let start = if i >= self.window_size {
                i - self.window_size + 1
            } else {
                0
            };
            for j in start..=i {
                mask[i][j] = true;
            }
        }

        mask
    }

    /// Check if position j is visible from position i
    pub fn is_visible(&self, i: usize, j: usize) -> bool {
        j <= i && i - j < self.window_size
    }

    /// Get the effective context length for a position
    pub fn effective_context(&self, position: usize) -> usize {
        std::cmp::min(position + 1, self.window_size)
    }
}

// =============================================================================
// Logit Soft Capping
// =============================================================================

/// Logit soft capping implementation
pub struct LogitSoftCap {
    cap: f32,
}

impl LogitSoftCap {
    pub fn new(cap: f32) -> Self {
        assert!(cap > 0.0, "Cap must be positive");
        Self { cap }
    }

    /// Apply soft capping to a single logit
    /// Formula: cap * tanh(logit / cap)
    pub fn apply(&self, logit: f32) -> f32 {
        self.cap * (logit / self.cap).tanh()
    }

    /// Apply soft capping to a slice of logits
    pub fn apply_to_slice(&self, logits: &mut [f32]) {
        for logit in logits.iter_mut() {
            *logit = self.apply(*logit);
        }
    }

    /// Check if a logit would be capped
    pub fn would_cap(&self, logit: f32) -> bool {
        logit.abs() > self.cap * 0.9 // Approximately where tanh starts saturating
    }
}

// =============================================================================
// RoPE (Rotary Position Embedding)
// =============================================================================

/// RoPE implementation for position encoding
pub struct RoPE {
    dim: usize,
    theta: f64,
    max_seq_len: usize,
    cos_cache: Vec<Vec<f32>>,
    sin_cache: Vec<Vec<f32>>,
}

impl RoPE {
    pub fn new(dim: usize, theta: f64, max_seq_len: usize) -> Self {
        let mut rope = Self {
            dim,
            theta,
            max_seq_len,
            cos_cache: Vec::new(),
            sin_cache: Vec::new(),
        };
        rope.build_cache();
        rope
    }

    fn build_cache(&mut self) {
        self.cos_cache = vec![vec![0.0; self.dim / 2]; self.max_seq_len];
        self.sin_cache = vec![vec![0.0; self.dim / 2]; self.max_seq_len];

        for pos in 0..self.max_seq_len {
            for i in 0..self.dim / 2 {
                let freq = 1.0 / self.theta.powf(2.0 * i as f64 / self.dim as f64);
                let angle = pos as f64 * freq;
                self.cos_cache[pos][i] = angle.cos() as f32;
                self.sin_cache[pos][i] = angle.sin() as f32;
            }
        }
    }

    /// Apply RoPE to query/key vectors at a given position
    pub fn apply(&self, x: &[f32], position: usize) -> Vec<f32> {
        assert!(position < self.max_seq_len, "Position exceeds max_seq_len");
        assert_eq!(x.len(), self.dim, "Input dimension mismatch");

        let mut result = vec![0.0; self.dim];

        for i in 0..self.dim / 2 {
            let cos = self.cos_cache[position][i];
            let sin = self.sin_cache[position][i];

            let x0 = x[2 * i];
            let x1 = x[2 * i + 1];

            result[2 * i] = x0 * cos - x1 * sin;
            result[2 * i + 1] = x0 * sin + x1 * cos;
        }

        result
    }

    /// Get the dimension
    pub fn dim(&self) -> usize {
        self.dim
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Phi-3 Configuration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_phi3_config_creation() {
        let config = Phi3Config::phi3_mini();
        assert_eq!(config.hidden_size, 3072);
        assert_eq!(config.num_hidden_layers, 32);
        assert_eq!(config.vocab_size, 32064);
    }

    #[test]
    fn test_phi3_mini_dimensions() {
        let config = Phi3Config::phi3_mini();
        assert_eq!(config.hidden_size, 3072);
        assert_eq!(config.intermediate_size, 8192);
        assert_eq!(config.num_attention_heads, 32);
        assert_eq!(config.num_key_value_heads, 32);
        assert_eq!(config.head_dim(), 96); // 3072 / 32
    }

    #[test]
    fn test_phi3_small_gqa() {
        let config = Phi3Config::phi3_small();
        assert_eq!(config.attention_type(), AttentionType::GQA);
        assert_eq!(config.num_kv_groups(), 4); // 32 / 8
    }

    #[test]
    fn test_phi3_medium_gqa() {
        let config = Phi3Config::phi3_medium();
        assert_eq!(config.attention_type(), AttentionType::GQA);
        assert_eq!(config.num_kv_groups(), 4); // 40 / 10
        assert_eq!(config.head_dim(), 128); // 5120 / 40
    }

    #[test]
    fn test_phi3_mini_mha() {
        let config = Phi3Config::phi3_mini();
        assert_eq!(config.attention_type(), AttentionType::MHA);
        assert_eq!(config.num_kv_groups(), 1);
    }

    #[test]
    fn test_phi3_rope_scaling() {
        let config = Phi3Config::phi3_mini();
        let rope_scaling = config.rope_scaling.as_ref().unwrap();
        assert_eq!(rope_scaling.scaling_type, RopeScalingType::Longrope);
        assert_eq!(rope_scaling.low_freq_factor, Some(1.0));
        assert_eq!(rope_scaling.high_freq_factor, Some(4.0));
    }

    #[test]
    fn test_phi3_config_validation() {
        let config = Phi3Config::phi3_mini();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_phi3_invalid_config() {
        let mut config = Phi3Config::phi3_mini();
        config.num_key_value_heads = 3; // Not a divisor of 32
        assert!(config.validate().is_err());
    }

    // -------------------------------------------------------------------------
    // Gemma-2 Configuration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_gemma2_config_creation() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.hidden_size, 3584);
        assert_eq!(config.head_dim, 256);
        assert_eq!(config.attn_logit_softcapping, 50.0);
    }

    #[test]
    fn test_gemma2_2b_dimensions() {
        let config = Gemma2Config::gemma2_2b();
        assert_eq!(config.hidden_size, 2304);
        assert_eq!(config.num_hidden_layers, 26);
        assert_eq!(config.num_attention_heads, 8);
        assert_eq!(config.num_key_value_heads, 4);
        assert_eq!(config.vocab_size, 256000);
    }

    #[test]
    fn test_gemma2_9b_dimensions() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.hidden_size, 3584);
        assert_eq!(config.num_hidden_layers, 42);
        assert_eq!(config.num_attention_heads, 16);
        assert_eq!(config.num_key_value_heads, 8);
    }

    #[test]
    fn test_gemma2_27b_dimensions() {
        let config = Gemma2Config::gemma2_27b();
        assert_eq!(config.hidden_size, 4608);
        assert_eq!(config.num_hidden_layers, 46);
        assert_eq!(config.num_attention_heads, 32);
        assert_eq!(config.num_key_value_heads, 16);
        assert_eq!(config.head_dim, 128);
    }

    #[test]
    fn test_gemma2_gqa() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.attention_type(), AttentionType::GQA);
        assert_eq!(config.num_kv_groups(), 2); // 16 / 8
    }

    #[test]
    fn test_gemma2_sliding_window() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.sliding_window, 4096);

        // Alternating layers
        assert!(!config.uses_sliding_window(0)); // Global
        assert!(config.uses_sliding_window(1)); // Local (sliding window)
        assert!(!config.uses_sliding_window(2)); // Global
        assert!(config.uses_sliding_window(3)); // Local
    }

    #[test]
    fn test_gemma2_logit_softcapping_values() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.attn_logit_softcapping, 50.0);
        assert_eq!(config.final_logit_softcapping, 30.0);
    }

    #[test]
    fn test_gemma2_query_pre_attn_scalar() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.query_pre_attn_scalar, 256.0);
        assert_eq!(config.query_pre_attn_scalar, config.head_dim as f32);
    }

    #[test]
    fn test_gemma2_config_validation() {
        let config = Gemma2Config::gemma2_9b();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_gemma2_invalid_softcapping() {
        let mut config = Gemma2Config::gemma2_9b();
        config.attn_logit_softcapping = 0.0;
        assert!(config.validate().is_err());
    }

    // -------------------------------------------------------------------------
    // Chat Template Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_phi3_chat_template_single_user() {
        let template = Phi3ChatTemplate;
        let messages = vec![ChatMessage::user("Hello, how are you?")];

        let result = template.format(&messages, true);

        assert!(result.contains("<|user|>"));
        assert!(result.contains("Hello, how are you?"));
        assert!(result.contains("<|end|>"));
        assert!(result.ends_with("<|assistant|>\n"));
    }

    #[test]
    fn test_phi3_chat_template_with_system() {
        let template = Phi3ChatTemplate;
        let messages = vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("What is 2+2?"),
        ];

        let result = template.format(&messages, true);

        assert!(result.contains("<|system|>"));
        assert!(result.contains("You are a helpful assistant."));
        assert!(result.contains("<|user|>"));
        assert!(result.contains("What is 2+2?"));
    }

    #[test]
    fn test_phi3_chat_template_conversation() {
        let template = Phi3ChatTemplate;
        let messages = vec![
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there!"),
            ChatMessage::user("How are you?"),
        ];

        let result = template.format(&messages, true);

        assert!(result.contains("<|assistant|>\nHi there!<|end|>"));
    }

    #[test]
    fn test_gemma_chat_template_single_user() {
        let template = GemmaChatTemplate;
        let messages = vec![ChatMessage::user("Hello, how are you?")];

        let result = template.format(&messages, true);

        assert!(result.contains("<start_of_turn>user"));
        assert!(result.contains("Hello, how are you?"));
        assert!(result.contains("<end_of_turn>"));
        assert!(result.ends_with("<start_of_turn>model\n"));
    }

    #[test]
    fn test_gemma_chat_template_with_system() {
        let template = GemmaChatTemplate;
        let messages = vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("What is 2+2?"),
        ];

        let result = template.format(&messages, true);

        // System message should be prepended to user message
        assert!(result.contains("You are a helpful assistant."));
        assert!(result.contains("What is 2+2?"));
    }

    #[test]
    fn test_gemma_chat_template_uses_model_role() {
        let template = GemmaChatTemplate;
        let messages = vec![ChatMessage::user("Hello"), ChatMessage::assistant("Hi!")];

        let result = template.format(&messages, false);

        assert!(result.contains("<start_of_turn>model\nHi!<end_of_turn>"));
    }

    #[test]
    fn test_chatml_template() {
        let template = ChatMLTemplate;
        let messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("Hello"),
        ];

        let result = template.format(&messages, true);

        assert!(result.contains("<|im_start|>system\nYou are helpful.<|im_end|>"));
        assert!(result.contains("<|im_start|>user\nHello<|im_end|>"));
        assert!(result.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_chat_template_no_generation_prompt() {
        let template = Phi3ChatTemplate;
        let messages = vec![ChatMessage::user("Hello")];

        let result = template.format(&messages, false);

        assert!(!result.ends_with("<|assistant|>\n"));
    }

    // -------------------------------------------------------------------------
    // Sliding Window Attention Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sliding_window_mask_basic() {
        let window = SlidingWindowMask::new(3);
        let mask = window.generate_mask(5);

        // Position 0 can only see itself
        assert!(mask[0][0]);

        // Position 2 can see positions 0, 1, 2
        assert!(mask[2][0]);
        assert!(mask[2][1]);
        assert!(mask[2][2]);

        // Position 4 can see positions 2, 3, 4 (not 0, 1)
        assert!(!mask[4][0]);
        assert!(!mask[4][1]);
        assert!(mask[4][2]);
        assert!(mask[4][3]);
        assert!(mask[4][4]);
    }

    #[test]
    fn test_sliding_window_visibility() {
        let window = SlidingWindowMask::new(4096);

        // Within window
        assert!(window.is_visible(100, 50));
        assert!(window.is_visible(4095, 0));

        // Just outside window
        assert!(!window.is_visible(4096, 0));
        assert!(window.is_visible(4096, 1));
    }

    #[test]
    fn test_sliding_window_effective_context() {
        let window = SlidingWindowMask::new(4096);

        assert_eq!(window.effective_context(0), 1);
        assert_eq!(window.effective_context(100), 101);
        assert_eq!(window.effective_context(4095), 4096);
        assert_eq!(window.effective_context(5000), 4096);
        assert_eq!(window.effective_context(10000), 4096);
    }

    #[test]
    fn test_sliding_window_causal() {
        let window = SlidingWindowMask::new(100);

        // Cannot see future positions
        assert!(!window.is_visible(5, 10));
        assert!(!window.is_visible(0, 1));
    }

    // -------------------------------------------------------------------------
    // Logit Soft Capping Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_logit_softcap_basic() {
        let cap = LogitSoftCap::new(50.0);

        // Small values pass through approximately unchanged
        let small = cap.apply(1.0);
        assert!((small - 1.0).abs() < 0.1);

        // Large values get capped
        let large = cap.apply(100.0);
        assert!(large < 50.0);
        assert!(large > 45.0); // tanh(2) ≈ 0.964
    }

    #[test]
    fn test_logit_softcap_symmetry() {
        let cap = LogitSoftCap::new(50.0);

        let pos = cap.apply(30.0);
        let neg = cap.apply(-30.0);

        assert!((pos + neg).abs() < 0.001);
    }

    #[test]
    fn test_logit_softcap_bounds() {
        let cap = LogitSoftCap::new(50.0);

        // Even extreme values stay bounded
        // tanh(1000/50) = tanh(20) is essentially 1.0
        // So result = 50 * 1.0 = 50.0 (at the cap, not below)
        let extreme = cap.apply(1000.0);
        assert!(extreme <= 50.0);
        assert!(extreme > 49.9); // Very close to cap

        let neg_extreme = cap.apply(-1000.0);
        assert!(neg_extreme >= -50.0);
        assert!(neg_extreme < -49.9);
    }

    #[test]
    fn test_logit_softcap_slice() {
        let cap = LogitSoftCap::new(30.0);
        let mut logits = vec![1.0, 50.0, -50.0, 100.0];

        cap.apply_to_slice(&mut logits);

        assert!((logits[0] - 1.0).abs() < 0.1);
        assert!(logits[1] < 30.0);
        assert!(logits[2] > -30.0);
        assert!(logits[3] < 30.0);
    }

    #[test]
    fn test_logit_softcap_would_cap() {
        let cap = LogitSoftCap::new(50.0);

        assert!(!cap.would_cap(10.0));
        assert!(!cap.would_cap(30.0));
        assert!(cap.would_cap(50.0));
        assert!(cap.would_cap(100.0));
        assert!(cap.would_cap(-60.0));
    }

    #[test]
    fn test_gemma2_attention_softcap() {
        let config = Gemma2Config::gemma2_9b();
        let cap = LogitSoftCap::new(config.attn_logit_softcapping);

        // Simulate attention scores
        let scores = vec![10.0, 25.0, 60.0, -40.0, 100.0];
        let mut capped = scores.clone();
        cap.apply_to_slice(&mut capped);

        // All should be within bounds
        for &score in &capped {
            assert!(score.abs() < config.attn_logit_softcapping);
        }
    }

    #[test]
    fn test_gemma2_final_softcap() {
        let config = Gemma2Config::gemma2_9b();
        let cap = LogitSoftCap::new(config.final_logit_softcapping);

        let extreme_logit = 1000.0;
        let capped = cap.apply(extreme_logit);

        // tanh approaches 1.0 for large inputs, so capped approaches the cap value
        assert!(capped <= config.final_logit_softcapping);
        assert!(capped > config.final_logit_softcapping - 0.1);
    }

    // -------------------------------------------------------------------------
    // RoPE Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_rope_creation() {
        let rope = RoPE::new(64, 10000.0, 2048);
        assert_eq!(rope.dim(), 64);
    }

    #[test]
    fn test_rope_apply_preserves_length() {
        let rope = RoPE::new(64, 10000.0, 2048);
        let input = vec![1.0; 64];

        let output = rope.apply(&input, 0);
        assert_eq!(output.len(), 64);
    }

    #[test]
    fn test_rope_position_zero() {
        let rope = RoPE::new(64, 10000.0, 2048);
        let input = vec![1.0; 64];

        let output = rope.apply(&input, 0);

        // At position 0, cos(0)=1 and sin(0)=0
        // So output should be close to input
        for i in 0..32 {
            assert!((output[2 * i] - input[2 * i]).abs() < 0.01);
        }
    }

    #[test]
    fn test_rope_different_positions() {
        let rope = RoPE::new(64, 10000.0, 2048);
        let input = vec![1.0; 64];

        let out0 = rope.apply(&input, 0);
        let out1 = rope.apply(&input, 1);
        let out100 = rope.apply(&input, 100);

        // Different positions should give different outputs
        assert!(out0 != out1);
        assert!(out1 != out100);
    }

    #[test]
    fn test_rope_norm_preservation() {
        let rope = RoPE::new(64, 10000.0, 2048);
        let input: Vec<f32> = (0..64).map(|i| (i as f32) * 0.1).collect();

        let output = rope.apply(&input, 50);

        // RoPE is a rotation, so it should preserve the norm of each 2D pair
        for i in 0..32 {
            let in_norm = (input[2 * i].powi(2) + input[2 * i + 1].powi(2)).sqrt();
            let out_norm = (output[2 * i].powi(2) + output[2 * i + 1].powi(2)).sqrt();
            assert!((in_norm - out_norm).abs() < 0.0001);
        }
    }

    // -------------------------------------------------------------------------
    // Integration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_phi3_full_pipeline_setup() {
        let config = Phi3Config::phi3_mini();
        let template = Phi3ChatTemplate;
        let rope = RoPE::new(
            config.head_dim(),
            config.rope_theta,
            config.max_position_embeddings,
        );

        // Validate config
        assert!(config.validate().is_ok());

        // Format a message
        let messages = vec![
            ChatMessage::system("You are a helpful AI."),
            ChatMessage::user("Hello!"),
        ];
        let prompt = template.format(&messages, true);
        assert!(!prompt.is_empty());

        // RoPE is ready
        assert_eq!(rope.dim(), config.head_dim());
    }

    #[test]
    fn test_gemma2_full_pipeline_setup() {
        let config = Gemma2Config::gemma2_9b();
        let template = GemmaChatTemplate;
        let sliding_window = SlidingWindowMask::new(config.sliding_window);
        let attn_cap = LogitSoftCap::new(config.attn_logit_softcapping);
        let final_cap = LogitSoftCap::new(config.final_logit_softcapping);

        // Validate config
        assert!(config.validate().is_ok());

        // Format a message
        let messages = vec![ChatMessage::user("What is the capital of France?")];
        let prompt = template.format(&messages, true);
        assert!(!prompt.is_empty());

        // Sliding window is ready
        assert_eq!(
            sliding_window.effective_context(10000),
            config.sliding_window
        );

        // Soft caps are ready
        assert!(attn_cap.apply(100.0) < config.attn_logit_softcapping);
        assert!(final_cap.apply(100.0) < config.final_logit_softcapping);
    }

    #[test]
    fn test_model_comparison() {
        let phi3 = Phi3Config::phi3_mini();
        let gemma2 = Gemma2Config::gemma2_9b();

        // Different vocab sizes
        assert!(gemma2.vocab_size > phi3.vocab_size);

        // Both use GeLU variants
        assert!(phi3.hidden_act.contains("silu") || phi3.hidden_act.contains("gelu"));
        assert!(gemma2.hidden_act.contains("gelu"));

        // Gemma-2 has soft capping, Phi-3 doesn't need it
        assert!(gemma2.attn_logit_softcapping > 0.0);
    }

    #[test]
    fn test_attention_type_detection() {
        // MHA
        let phi3_mini = Phi3Config::phi3_mini();
        assert_eq!(phi3_mini.attention_type(), AttentionType::MHA);

        // GQA
        let phi3_small = Phi3Config::phi3_small();
        assert_eq!(phi3_small.attention_type(), AttentionType::GQA);

        let gemma2 = Gemma2Config::gemma2_9b();
        assert_eq!(gemma2.attention_type(), AttentionType::GQA);
    }
}
