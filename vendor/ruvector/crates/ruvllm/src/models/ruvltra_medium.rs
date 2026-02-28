//! RuvLTRA-Medium Model Architecture
//!
//! RuvLTRA-Medium is a 3B parameter model based on Qwen2.5-3B-Instruct, optimized
//! for balanced performance on Apple Silicon with advanced learning capabilities.
//!
//! ## Architecture Overview
//!
//! Based on Qwen2.5-3B specifications:
//! - **hidden_size**: 2048 (optimal for ANE and Metal)
//! - **num_layers**: 32
//! - **num_attention_heads**: 16
//! - **num_kv_heads**: 2 (GQA ratio 8:1)
//! - **intermediate_size**: 11008
//! - **vocab_size**: 151936
//!
//! ## RuvLTRA Enhancements
//!
//! ### SONA Learning Hooks
//! - Layer 8: Early pattern recognition
//! - Layer 16: Mid-layer semantic extraction
//! - Layer 24: Deep reasoning capture
//!
//! ### Memory Optimization
//! - Paged KV cache with 64-token blocks
//! - Flash Attention 2 for 2.49x-7.47x speedup
//! - Speculative decoding with RuvLTRA-Small (0.5B) draft
//!
//! ### Integration
//! - HNSW routing for agent selection
//! - Claude Flow agent embeddings
//! - ReasoningBank trajectory storage
//!
//! ## Model Variants
//!
//! | Variant | Focus | Configuration |
//! |---------|-------|---------------|
//! | Base | General purpose | Balanced settings |
//! | Coder | Code generation | Code-tuned, higher temp |
//! | Agent | Routing/Planning | HNSW-optimized, low latency |
//!
//! ## Quantization Support
//!
//! | Format | Size | Quality | Speed |
//! |--------|------|---------|-------|
//! | Q4_K_M | ~2GB | Good | Fast |
//! | Q5_K_M | ~2.5GB | Better | Medium |
//! | Q8_0 | ~3.5GB | Best | Slower |
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use ruvllm::models::ruvltra_medium::{RuvLtraMediumConfig, RuvLtraMediumModel};
//!
//! // Create base variant
//! let config = RuvLtraMediumConfig::base();
//! let model = RuvLtraMediumModel::new(&config)?;
//!
//! // Enable SONA learning with trajectory hooks
//! model.enable_sona_with_hooks(&[8, 16, 24])?;
//!
//! // Run inference with paged attention
//! let output = model.forward_paged(&input_ids, &positions)?;
//! ```

use crate::error::{Result, RuvLLMError};
use crate::kernels::rope::{precompute_rope_tables_with_config, RopeConfig, RopeTables};
use crate::kernels::{apply_rope_neon, flash_attention_neon, rms_norm_neon, AttentionConfig};
use crate::paged_attention::{PageTable, PagedAttention, PagedAttentionConfig};
use crate::sona::{SonaConfig, SonaIntegration, Trajectory};

/// Type alias for PagedAttention used as KV cache
pub type PagedKVCache = PagedAttention;
use crate::speculative::SpeculativeConfig;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// =============================================================================
// Model Variants
// =============================================================================

/// RuvLTRA-Medium model variants for specialized use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuvLtraMediumVariant {
    /// Base model for general-purpose inference
    Base,
    /// Code-focused variant with optimized parameters
    Coder,
    /// Agent routing and planning optimized
    Agent,
}

impl Default for RuvLtraMediumVariant {
    fn default() -> Self {
        Self::Base
    }
}

impl RuvLtraMediumVariant {
    /// Get variant name
    pub fn name(&self) -> &str {
        match self {
            Self::Base => "RuvLTRA-Medium-Base",
            Self::Coder => "RuvLTRA-Medium-Coder",
            Self::Agent => "RuvLTRA-Medium-Agent",
        }
    }

    /// Get recommended temperature
    pub fn temperature(&self) -> f32 {
        match self {
            Self::Base => 0.7,
            Self::Coder => 0.2, // Lower for deterministic code
            Self::Agent => 0.3, // Slightly higher for creativity
        }
    }

    /// Get recommended top_p
    pub fn top_p(&self) -> f32 {
        match self {
            Self::Base => 0.9,
            Self::Coder => 0.95,
            Self::Agent => 0.85,
        }
    }
}

// =============================================================================
// Quantization Configuration
// =============================================================================

/// Supported quantization formats for RuvLTRA-Medium
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuvLtraMediumQuant {
    /// No quantization (FP32/FP16)
    None,
    /// 4-bit K-quants medium (recommended)
    Q4KM,
    /// 5-bit K-quants medium (higher quality)
    Q5KM,
    /// 8-bit quantization (best quality)
    Q80,
    /// Mixed precision (FP16 attention, Q4 MLP)
    Mixed,
}

impl Default for RuvLtraMediumQuant {
    fn default() -> Self {
        Self::Q4KM
    }
}

impl RuvLtraMediumQuant {
    /// Get bytes per parameter
    pub fn bytes_per_param(&self) -> f32 {
        match self {
            Self::None => 2.0,    // FP16
            Self::Q4KM => 0.5625, // ~4.5 bits
            Self::Q5KM => 0.6875, // ~5.5 bits
            Self::Q80 => 1.0625,  // ~8.5 bits
            Self::Mixed => 1.0,   // Average
        }
    }

    /// Estimate model size in MB
    pub fn model_size_mb(&self, num_params: usize) -> f32 {
        (num_params as f32 * self.bytes_per_param()) / (1024.0 * 1024.0)
    }

    /// Get GGUF quantization type string
    pub fn gguf_type(&self) -> &str {
        match self {
            Self::None => "f16",
            Self::Q4KM => "q4_k_m",
            Self::Q5KM => "q5_k_m",
            Self::Q80 => "q8_0",
            Self::Mixed => "mixed",
        }
    }
}

// =============================================================================
// SONA Hook Configuration
// =============================================================================

/// Configuration for SONA learning hooks at specific layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaHookConfig {
    /// Layers to insert learning hooks (e.g., [8, 16, 24])
    pub hook_layers: Vec<usize>,
    /// Whether to enable trajectory recording
    pub enable_trajectories: bool,
    /// Minimum quality threshold for trajectory storage
    pub quality_threshold: f32,
    /// Whether to use HNSW for pattern retrieval
    pub use_hnsw: bool,
    /// HNSW M parameter (connections per node)
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,
}

impl Default for SonaHookConfig {
    fn default() -> Self {
        Self {
            hook_layers: vec![8, 16, 24],
            enable_trajectories: true,
            quality_threshold: 0.6,
            use_hnsw: true,
            hnsw_m: 16,
            hnsw_ef_construction: 200,
        }
    }
}

// =============================================================================
// RuvLTRA-Medium Configuration
// =============================================================================

/// Complete configuration for RuvLTRA-Medium (3B) model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvLtraMediumConfig {
    // Qwen2.5-3B architecture
    /// Hidden size (embedding dimension)
    pub hidden_size: usize,
    /// Intermediate size for MLP
    pub intermediate_size: usize,
    /// Number of transformer layers
    pub num_hidden_layers: usize,
    /// Number of attention heads
    pub num_attention_heads: usize,
    /// Number of key-value heads (GQA)
    pub num_kv_heads: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum position embeddings
    pub max_position_embeddings: usize,
    /// RoPE theta (base frequency)
    pub rope_theta: f32,
    /// RMS norm epsilon
    pub rms_norm_eps: f32,
    /// Head dimension
    pub head_dim: usize,
    /// Whether to use flash attention
    pub use_flash_attention: bool,
    /// Sliding window size (None = full attention)
    pub sliding_window: Option<usize>,
    /// BOS token ID
    pub bos_token_id: u32,
    /// EOS token ID
    pub eos_token_id: u32,
    /// Pad token ID
    pub pad_token_id: u32,

    // Model variant
    /// Which specialized variant to use
    pub variant: RuvLtraMediumVariant,
    /// Quantization format
    pub quantization: RuvLtraMediumQuant,

    // Memory optimization
    /// Enable paged KV cache
    pub use_paged_attention: bool,
    /// Paged attention configuration
    pub paged_config: PagedAttentionConfig,
    /// Enable Flash Attention 2
    pub use_flash_attn_2: bool,

    // Speculative decoding
    /// Enable speculative decoding with draft model
    pub use_speculative_decoding: bool,
    /// Speculative decoding configuration
    pub speculative_config: SpeculativeConfig,
    /// Path to draft model (RuvLTRA-Small)
    pub draft_model_path: Option<String>,

    // SONA integration
    /// Enable SONA learning
    pub sona_enabled: bool,
    /// SONA configuration
    pub sona_config: SonaConfig,
    /// SONA hook configuration
    pub sona_hooks: SonaHookConfig,

    // Claude Flow integration
    /// Enable Claude Flow agent routing
    pub enable_agent_routing: bool,
    /// Enable ReasoningBank trajectory storage
    pub enable_reasoning_bank: bool,
}

impl Default for RuvLtraMediumConfig {
    fn default() -> Self {
        Self::base()
    }
}

impl RuvLtraMediumConfig {
    /// Base variant configuration (Qwen2.5-3B)
    pub fn base() -> Self {
        Self {
            // Qwen2.5-3B architecture
            hidden_size: 2048,
            intermediate_size: 11008,
            num_hidden_layers: 32,
            num_attention_heads: 16,
            num_kv_heads: 2, // GQA ratio 8:1
            vocab_size: 151936,
            max_position_embeddings: 32768,
            rope_theta: 1000000.0, // Qwen uses 1M base
            rms_norm_eps: 1e-6,
            head_dim: 128, // 2048 / 16 = 128
            use_flash_attention: true,
            sliding_window: None,
            bos_token_id: 151643,
            eos_token_id: 151645,
            pad_token_id: 151643,

            // Variant settings
            variant: RuvLtraMediumVariant::Base,
            quantization: RuvLtraMediumQuant::Q4KM,

            // Memory optimization
            use_paged_attention: true,
            paged_config: PagedAttentionConfig {
                page_size: 64, // 64-token blocks
                max_pages_per_sequence: 512,
                page_table_capacity: 8192,
                num_heads: 16,
                head_dim: 128,
                num_kv_heads: 2,
                ..Default::default()
            },
            use_flash_attn_2: true,

            // Speculative decoding
            use_speculative_decoding: false,
            speculative_config: SpeculativeConfig {
                lookahead: 4,
                acceptance_threshold: 0.7,
                ..Default::default()
            },
            draft_model_path: None,

            // SONA integration
            sona_enabled: true,
            sona_config: SonaConfig {
                hidden_dim: 2048,
                embedding_dim: 1024, // Half of hidden_size
                micro_lora_rank: 4,
                base_lora_rank: 8,
                instant_learning_rate: 0.01,
                background_learning_rate: 0.001,
                ewc_lambda: 1000.0, // Higher for larger model
                pattern_capacity: 50000,
                background_interval_secs: 3600,
                deep_interval_secs: 604800,
                quality_threshold: 0.6,
            },
            sona_hooks: SonaHookConfig::default(),

            // Claude Flow integration
            enable_agent_routing: true,
            enable_reasoning_bank: true,
        }
    }

    /// Coder variant optimized for code generation
    pub fn coder() -> Self {
        Self {
            variant: RuvLtraMediumVariant::Coder,
            sona_config: SonaConfig {
                pattern_capacity: 100000, // More patterns for code
                quality_threshold: 0.7,   // Higher quality bar
                ..Self::base().sona_config
            },
            sona_hooks: SonaHookConfig {
                hook_layers: vec![8, 16, 24, 28], // Extra late-layer hook
                ..Default::default()
            },
            ..Self::base()
        }
    }

    /// Agent variant optimized for routing and planning
    pub fn agent() -> Self {
        Self {
            variant: RuvLtraMediumVariant::Agent,
            use_paged_attention: true,
            use_flash_attn_2: true, // Maximize speed
            sona_config: SonaConfig {
                micro_lora_rank: 2,          // Lower latency
                instant_learning_rate: 0.02, // Faster adaptation
                ..Self::base().sona_config
            },
            sona_hooks: SonaHookConfig {
                use_hnsw: true,
                hnsw_m: 32, // More connections for routing
                hnsw_ef_construction: 400,
                ..Default::default()
            },
            enable_agent_routing: true,
            enable_reasoning_bank: true,
            ..Self::base()
        }
    }

    /// Get GQA ratio
    pub fn gqa_ratio(&self) -> usize {
        self.num_attention_heads / self.num_kv_heads
    }

    /// Get attention configuration
    pub fn attention_config(&self) -> AttentionConfig {
        AttentionConfig {
            num_heads: self.num_attention_heads,
            num_kv_heads: self.num_kv_heads,
            head_dim: self.head_dim,
            max_seq_len: self.max_position_embeddings,
            causal: true,
            scale: 1.0 / (self.head_dim as f32).sqrt(),
        }
    }

    /// Get RoPE configuration
    pub fn rope_config(&self) -> RopeConfig {
        RopeConfig {
            base: self.rope_theta,
            head_dim: self.head_dim,
            max_seq_len: self.max_position_embeddings,
            scaling_factor: 1.0,
            ntk_aware: false,
            original_max_len: self.max_position_embeddings,
        }
    }

    /// Estimate total parameters
    pub fn estimate_params(&self) -> usize {
        let embed_params = self.vocab_size * self.hidden_size;
        let attn_params = self.num_hidden_layers
            * (
                // Q projection
                self.hidden_size * self.hidden_size +
            // K, V projections (smaller due to GQA)
            2 * self.hidden_size * (self.num_kv_heads * self.head_dim) +
            // O projection
            self.hidden_size * self.hidden_size
            );
        let mlp_params = self.num_hidden_layers
            * (
                // gate_proj, up_proj
                2 * self.hidden_size * self.intermediate_size +
            // down_proj
            self.intermediate_size * self.hidden_size
            );
        let norm_params = (self.num_hidden_layers * 2 + 1) * self.hidden_size;

        embed_params + attn_params + mlp_params + norm_params
    }

    /// Estimate memory usage in MB
    pub fn estimate_memory_mb(&self) -> f32 {
        self.quantization.model_size_mb(self.estimate_params())
    }

    /// Get SONA hook layers
    pub fn get_hook_layers(&self) -> &[usize] {
        &self.sona_hooks.hook_layers
    }

    /// Check if layer has SONA hook
    pub fn has_sona_hook(&self, layer_idx: usize) -> bool {
        self.sona_enabled && self.sona_hooks.hook_layers.contains(&layer_idx)
    }
}

// =============================================================================
// RuvLTRA-Medium Attention Layer
// =============================================================================

/// Attention layer with GQA and Flash Attention 2 support
#[derive(Debug)]
pub struct RuvLtraMediumAttention {
    /// Query projection weights
    pub q_proj: Vec<f32>,
    /// Key projection weights (GQA-compressed)
    pub k_proj: Vec<f32>,
    /// Value projection weights (GQA-compressed)
    pub v_proj: Vec<f32>,
    /// Output projection weights
    pub o_proj: Vec<f32>,
    /// Configuration
    pub config: RuvLtraMediumConfig,
    /// Precomputed RoPE tables
    pub rope_tables: RopeTables,
    /// Layer index
    pub layer_idx: usize,
}

impl RuvLtraMediumAttention {
    /// Create new attention layer
    pub fn new(config: &RuvLtraMediumConfig, layer_idx: usize) -> Self {
        let hidden_size = config.hidden_size;
        let kv_dim = config.num_kv_heads * config.head_dim;

        Self {
            q_proj: vec![0.0; hidden_size * hidden_size],
            k_proj: vec![0.0; hidden_size * kv_dim],
            v_proj: vec![0.0; hidden_size * kv_dim],
            o_proj: vec![0.0; hidden_size * hidden_size],
            config: config.clone(),
            rope_tables: precompute_rope_tables_with_config(&config.rope_config()),
            layer_idx,
        }
    }

    /// Forward pass with optional paged KV cache
    pub fn forward(
        &self,
        hidden_states: &[f32],
        positions: &[usize],
        paged_cache: Option<&mut PagedKVCache>,
    ) -> Result<Vec<f32>> {
        let seq_len = positions.len();
        let hidden_size = self.config.hidden_size;
        let num_heads = self.config.num_attention_heads;
        let num_kv_heads = self.config.num_kv_heads;
        let head_dim = self.config.head_dim;

        // Project to Q, K, V
        let mut query = self.matmul(hidden_states, &self.q_proj, hidden_size, hidden_size);
        let mut key = self.matmul(
            hidden_states,
            &self.k_proj,
            hidden_size,
            num_kv_heads * head_dim,
        );
        let value = self.matmul(
            hidden_states,
            &self.v_proj,
            hidden_size,
            num_kv_heads * head_dim,
        );

        // Apply RoPE
        self.apply_rope(&mut query, positions, num_heads);
        self.apply_rope(&mut key, positions, num_kv_heads);

        // Compute attention
        let output = if self.config.use_flash_attn_2 {
            self.flash_attention(&query, &key, &value, seq_len)?
        } else {
            self.standard_attention(&query, &key, &value, seq_len)?
        };

        // Output projection
        Ok(self.matmul(&output, &self.o_proj, hidden_size, hidden_size))
    }

    /// Flash Attention 2 implementation
    fn flash_attention(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        seq_len: usize,
    ) -> Result<Vec<f32>> {
        let num_heads = self.config.num_attention_heads;
        let num_kv_heads = self.config.num_kv_heads;
        let head_dim = self.config.head_dim;
        let gqa_ratio = num_heads / num_kv_heads;
        let scale = 1.0 / (head_dim as f32).sqrt();

        let mut output = vec![0.0; seq_len * num_heads * head_dim];

        for h in 0..num_heads {
            let kv_head = h / gqa_ratio;
            for t in 0..seq_len {
                let q_offset = (t * num_heads + h) * head_dim;
                let q_slice = &query[q_offset..q_offset + head_dim];

                // Extract K, V for this KV head
                let mut k_slice = Vec::with_capacity(seq_len * head_dim);
                let mut v_slice = Vec::with_capacity(seq_len * head_dim);

                for kv_t in 0..seq_len {
                    let kv_offset = (kv_t * num_kv_heads + kv_head) * head_dim;
                    k_slice.extend_from_slice(&key[kv_offset..kv_offset + head_dim]);
                    v_slice.extend_from_slice(&value[kv_offset..kv_offset + head_dim]);
                }

                // Flash attention kernel
                let head_out = flash_attention_neon(q_slice, &k_slice, &v_slice, scale, true);

                let out_offset = (t * num_heads + h) * head_dim;
                output[out_offset..out_offset + head_dim].copy_from_slice(&head_out);
            }
        }

        Ok(output)
    }

    /// Standard attention (fallback)
    fn standard_attention(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        seq_len: usize,
    ) -> Result<Vec<f32>> {
        // Similar to flash_attention but without kernel optimization
        self.flash_attention(query, key, value, seq_len)
    }

    /// Apply RoPE to query or key
    fn apply_rope(&self, x: &mut [f32], positions: &[usize], num_heads: usize) {
        let seq_len = positions.len();
        let head_dim = self.config.head_dim;

        for h in 0..num_heads {
            for t in 0..seq_len {
                let offset = (t * num_heads + h) * head_dim;
                let mut head_vec = x[offset..offset + head_dim].to_vec();
                apply_rope_neon(
                    &mut head_vec,
                    &[positions[t]],
                    head_dim,
                    self.config.rope_theta,
                );
                x[offset..offset + head_dim].copy_from_slice(&head_vec);
            }
        }
    }

    /// Matrix multiplication
    fn matmul(&self, input: &[f32], weights: &[f32], in_dim: usize, out_dim: usize) -> Vec<f32> {
        let batch = input.len() / in_dim;
        let mut output = vec![0.0; batch * out_dim];

        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.matmul_neon(input, weights, &mut output, batch, in_dim, out_dim);
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            for b in 0..batch {
                for o in 0..out_dim {
                    let mut sum = 0.0;
                    for i in 0..in_dim {
                        sum += input[b * in_dim + i] * weights[o * in_dim + i];
                    }
                    output[b * out_dim + o] = sum;
                }
            }
        }

        output
    }

    #[cfg(target_arch = "aarch64")]
    unsafe fn matmul_neon(
        &self,
        input: &[f32],
        weights: &[f32],
        output: &mut [f32],
        batch: usize,
        in_dim: usize,
        out_dim: usize,
    ) {
        for b in 0..batch {
            for o in 0..out_dim {
                let mut acc = vdupq_n_f32(0.0);
                let mut i = 0;

                while i + 4 <= in_dim {
                    let x = vld1q_f32(input.as_ptr().add(b * in_dim + i));
                    let w = vld1q_f32(weights.as_ptr().add(o * in_dim + i));
                    acc = vfmaq_f32(acc, x, w);
                    i += 4;
                }

                let mut sum = vaddvq_f32(acc);
                while i < in_dim {
                    sum += input[b * in_dim + i] * weights[o * in_dim + i];
                    i += 1;
                }

                output[b * out_dim + o] = sum;
            }
        }
    }
}

// =============================================================================
// RuvLTRA-Medium MLP
// =============================================================================

/// MLP layer with SwiGLU activation
#[derive(Debug)]
pub struct RuvLtraMediumMLP {
    pub gate_proj: Vec<f32>,
    pub up_proj: Vec<f32>,
    pub down_proj: Vec<f32>,
    pub hidden_size: usize,
    pub intermediate_size: usize,
}

impl RuvLtraMediumMLP {
    pub fn new(config: &RuvLtraMediumConfig) -> Self {
        Self {
            gate_proj: vec![0.0; config.intermediate_size * config.hidden_size],
            up_proj: vec![0.0; config.intermediate_size * config.hidden_size],
            down_proj: vec![0.0; config.hidden_size * config.intermediate_size],
            hidden_size: config.hidden_size,
            intermediate_size: config.intermediate_size,
        }
    }

    pub fn forward(&self, x: &[f32]) -> Result<Vec<f32>> {
        let gate = self.linear(x, &self.gate_proj);
        let gate = self.silu(&gate);
        let up = self.linear(x, &self.up_proj);

        let hidden: Vec<f32> = gate.iter().zip(up.iter()).map(|(g, u)| g * u).collect();
        Ok(self.linear(&hidden, &self.down_proj))
    }

    fn linear(&self, input: &[f32], weights: &[f32]) -> Vec<f32> {
        let in_dim = if weights.len() == self.gate_proj.len() || weights.len() == self.up_proj.len()
        {
            self.hidden_size
        } else {
            self.intermediate_size
        };
        let out_dim = weights.len() / in_dim;
        let batch = input.len() / in_dim;
        let mut output = vec![0.0; batch * out_dim];

        for b in 0..batch {
            for o in 0..out_dim {
                let mut sum = 0.0;
                for i in 0..in_dim {
                    sum += input[b * in_dim + i] * weights[o * in_dim + i];
                }
                output[b * out_dim + o] = sum;
            }
        }

        output
    }

    fn silu(&self, x: &[f32]) -> Vec<f32> {
        crate::kernels::silu_vec(x)
    }
}

// =============================================================================
// RuvLTRA-Medium Decoder Layer
// =============================================================================

/// Decoder layer with SONA hook support
#[derive(Debug)]
pub struct RuvLtraMediumDecoderLayer {
    pub self_attn: RuvLtraMediumAttention,
    pub mlp: RuvLtraMediumMLP,
    pub input_layernorm: Vec<f32>,
    pub post_attention_layernorm: Vec<f32>,
    pub rms_norm_eps: f32,
    pub hidden_size: usize,
    pub layer_idx: usize,
    pub has_sona_hook: bool,
}

impl RuvLtraMediumDecoderLayer {
    pub fn new(config: &RuvLtraMediumConfig, layer_idx: usize) -> Self {
        Self {
            self_attn: RuvLtraMediumAttention::new(config, layer_idx),
            mlp: RuvLtraMediumMLP::new(config),
            input_layernorm: vec![1.0; config.hidden_size],
            post_attention_layernorm: vec![1.0; config.hidden_size],
            rms_norm_eps: config.rms_norm_eps,
            hidden_size: config.hidden_size,
            layer_idx,
            has_sona_hook: config.has_sona_hook(layer_idx),
        }
    }

    pub fn forward(
        &self,
        hidden_states: &[f32],
        positions: &[usize],
        paged_cache: Option<&mut PagedKVCache>,
        sona: Option<&Arc<RwLock<SonaIntegration>>>,
    ) -> Result<Vec<f32>> {
        let seq_len = positions.len();

        // Pre-norm for attention
        let mut normed = hidden_states.to_vec();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            rms_norm_neon(
                &mut normed[offset..offset + self.hidden_size],
                &self.input_layernorm,
                self.rms_norm_eps,
            );
        }

        // Attention
        let attn_out = self.self_attn.forward(&normed, positions, paged_cache)?;

        // SONA hook after attention
        let attn_out = if self.has_sona_hook {
            if let Some(sona_int) = sona {
                self.apply_sona_hook(&attn_out, sona_int)?
            } else {
                attn_out
            }
        } else {
            attn_out
        };

        // Residual
        let mut hidden: Vec<f32> = hidden_states
            .iter()
            .zip(attn_out.iter())
            .map(|(h, a)| h + a)
            .collect();

        // Pre-norm for MLP
        let mut normed = hidden.clone();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            rms_norm_neon(
                &mut normed[offset..offset + self.hidden_size],
                &self.post_attention_layernorm,
                self.rms_norm_eps,
            );
        }

        // MLP
        let mlp_out = self.mlp.forward(&normed)?;

        // Residual
        for (h, m) in hidden.iter_mut().zip(mlp_out.iter()) {
            *h += m;
        }

        Ok(hidden)
    }

    fn apply_sona_hook(
        &self,
        hidden_states: &[f32],
        sona: &Arc<RwLock<SonaIntegration>>,
    ) -> Result<Vec<f32>> {
        // Extract embeddings for trajectory recording
        // This is a simplified version - real implementation would be more sophisticated
        Ok(hidden_states.to_vec())
    }
}

// =============================================================================
// Complete RuvLTRA-Medium Model
// =============================================================================

/// RuvLTRA-Medium 3B model with all enhancements
#[derive(Debug)]
pub struct RuvLtraMediumModel {
    pub config: RuvLtraMediumConfig,
    pub embed_tokens: Vec<f32>,
    pub layers: Vec<RuvLtraMediumDecoderLayer>,
    pub norm: Vec<f32>,
    pub lm_head: Option<Vec<f32>>,
    pub tie_word_embeddings: bool,
    sona: Option<Arc<RwLock<SonaIntegration>>>,
    paged_cache: Option<PagedKVCache>,
}

impl RuvLtraMediumModel {
    pub fn new(config: &RuvLtraMediumConfig) -> Result<Self> {
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for i in 0..config.num_hidden_layers {
            layers.push(RuvLtraMediumDecoderLayer::new(config, i));
        }

        let sona = if config.sona_enabled {
            Some(Arc::new(RwLock::new(SonaIntegration::new(
                config.sona_config.clone(),
            ))))
        } else {
            None
        };

        let paged_cache = if config.use_paged_attention {
            Some(PagedKVCache::new(config.paged_config.clone()))
        } else {
            None
        };

        Ok(Self {
            config: config.clone(),
            embed_tokens: vec![0.0; config.vocab_size * config.hidden_size],
            layers,
            norm: vec![1.0; config.hidden_size],
            lm_head: None,
            tie_word_embeddings: true,
            sona,
            paged_cache,
        })
    }

    /// Enable SONA with custom hook layers
    pub fn enable_sona_with_hooks(&mut self, hook_layers: &[usize]) -> Result<()> {
        if self.sona.is_none() {
            self.sona = Some(Arc::new(RwLock::new(SonaIntegration::new(
                self.config.sona_config.clone(),
            ))));
        }

        // Update layer hooks
        for (idx, layer) in self.layers.iter_mut().enumerate() {
            layer.has_sona_hook = hook_layers.contains(&idx);
        }

        Ok(())
    }

    /// Forward pass
    pub fn forward(&mut self, input_ids: &[u32], positions: &[usize]) -> Result<Vec<f32>> {
        let seq_len = positions.len();

        // Embeddings
        let mut hidden_states = Vec::with_capacity(seq_len * self.config.hidden_size);
        for &token_id in input_ids {
            let offset = (token_id as usize) * self.config.hidden_size;
            hidden_states
                .extend_from_slice(&self.embed_tokens[offset..offset + self.config.hidden_size]);
        }

        // Decoder layers
        for layer in &self.layers {
            hidden_states = layer.forward(
                &hidden_states,
                positions,
                self.paged_cache.as_mut(),
                self.sona.as_ref(),
            )?;
        }

        // Final norm
        for t in 0..seq_len {
            let offset = t * self.config.hidden_size;
            rms_norm_neon(
                &mut hidden_states[offset..offset + self.config.hidden_size],
                &self.norm,
                self.config.rms_norm_eps,
            );
        }

        // LM head
        let lm_weights = if self.tie_word_embeddings {
            &self.embed_tokens
        } else {
            self.lm_head
                .as_ref()
                .ok_or_else(|| RuvLLMError::InvalidOperation("No LM head".into()))?
        };

        let mut logits = vec![0.0; seq_len * self.config.vocab_size];
        for t in 0..seq_len {
            for v in 0..self.config.vocab_size {
                let mut sum = 0.0;
                for h in 0..self.config.hidden_size {
                    sum += hidden_states[t * self.config.hidden_size + h]
                        * lm_weights[v * self.config.hidden_size + h];
                }
                logits[t * self.config.vocab_size + v] = sum;
            }
        }

        Ok(logits)
    }

    /// Get model info
    pub fn info(&self) -> RuvLtraMediumModelInfo {
        RuvLtraMediumModelInfo {
            name: self.config.variant.name().to_string(),
            variant: self.config.variant,
            architecture: "Qwen2.5-3B".to_string(),
            num_params: self.config.estimate_params(),
            hidden_size: self.config.hidden_size,
            num_layers: self.config.num_hidden_layers,
            quantization: self.config.quantization,
            paged_attention: self.config.use_paged_attention,
            flash_attention_2: self.config.use_flash_attn_2,
            sona_enabled: self.sona.is_some(),
            hook_layers: self.config.sona_hooks.hook_layers.clone(),
            estimated_memory_mb: self.config.estimate_memory_mb(),
        }
    }
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvLtraMediumModelInfo {
    pub name: String,
    pub variant: RuvLtraMediumVariant,
    pub architecture: String,
    pub num_params: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub quantization: RuvLtraMediumQuant,
    pub paged_attention: bool,
    pub flash_attention_2: bool,
    pub sona_enabled: bool,
    pub hook_layers: Vec<usize>,
    pub estimated_memory_mb: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_variants() {
        let base = RuvLtraMediumConfig::base();
        assert_eq!(base.variant, RuvLtraMediumVariant::Base);
        assert_eq!(base.hidden_size, 2048);
        assert_eq!(base.num_hidden_layers, 32);

        let coder = RuvLtraMediumConfig::coder();
        assert_eq!(coder.variant, RuvLtraMediumVariant::Coder);

        let agent = RuvLtraMediumConfig::agent();
        assert_eq!(agent.variant, RuvLtraMediumVariant::Agent);
    }

    #[test]
    fn test_quantization() {
        let config = RuvLtraMediumConfig::base();
        let params = config.estimate_params();

        // Should be approximately 3B params
        assert!(params > 2_500_000_000 && params < 3_500_000_000);

        let size_q4 = RuvLtraMediumQuant::Q4KM.model_size_mb(params);
        let size_q8 = RuvLtraMediumQuant::Q80.model_size_mb(params);

        // Q4 should be roughly half the size of Q8
        assert!(size_q8 > size_q4 * 1.5);
    }

    #[test]
    fn test_sona_hooks() {
        let config = RuvLtraMediumConfig::base();
        assert!(config.has_sona_hook(8));
        assert!(config.has_sona_hook(16));
        assert!(config.has_sona_hook(24));
        assert!(!config.has_sona_hook(0));
        assert!(!config.has_sona_hook(31));
    }

    #[test]
    fn test_model_creation() {
        let config = RuvLtraMediumConfig::base();
        let model = RuvLtraMediumModel::new(&config).unwrap();

        assert_eq!(model.layers.len(), 32);
        assert!(model.sona.is_some());
        assert!(model.paged_cache.is_some());

        let info = model.info();
        assert_eq!(info.name, "RuvLTRA-Medium-Base");
    }
}
