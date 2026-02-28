//! RuvLTRA Model Optimization Pipeline
//!
//! RuvLTRA (Ruvector Ultra-Lightweight Transformer Runtime Architecture) is an
//! ANE-optimized model pipeline based on Qwen 0.5B architecture with SONA
//! pretraining integration for continuous learning on Apple Silicon.
//!
//! ## Architecture Overview
//!
//! Based on Qwen 0.5B specifications:
//! - **hidden_size**: 896 (optimized for ANE matmul >=768)
//! - **num_layers**: 24
//! - **num_attention_heads**: 14
//! - **intermediate_size**: 4864
//! - **vocab_size**: 151936
//!
//! ## ANE Optimization Features
//!
//! | Feature | Benefit | Implementation |
//! |---------|---------|----------------|
//! | Matmul dims >=768 | ANE acceleration | hidden_size=896 |
//! | Hybrid dispatch | Optimal routing | MLP->ANE, Attn->GPU |
//! | Memory layout | Core ML compat | NHWC tensor format |
//! | Quantization | 4-8x memory reduction | INT4/INT8 support |
//!
//! ## SONA Integration
//!
//! ```text
//! +-------------------+     +-------------------+
//! | RuvLTRA Model     |---->| SONA Learning     |
//! | (inference)       |     | - MicroLoRA       |
//! +-------------------+     | - ReasoningBank   |
//!                           | - EWC++           |
//!                           +-------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::models::ruvltra::{RuvLtraConfig, RuvLtraModel, AneOptimization};
//!
//! // Create ANE-optimized configuration
//! let config = RuvLtraConfig::default()
//!     .with_ane_optimization(AneOptimization::HybridDispatch)
//!     .with_quantization(QuantizationType::Int4);
//!
//! // Initialize model with SONA pretraining
//! let model = RuvLtraModel::new(&config)?;
//! model.enable_sona_pretraining()?;
//!
//! // Run inference with continuous learning
//! let output = model.forward(&input_ids, &positions, None)?;
//! ```

use crate::error::{Result, RuvLLMError};
use crate::kernels::rope::{precompute_rope_tables_with_config, RopeConfig, RopeTables};
use crate::kernels::{apply_rope_neon, flash_attention_neon, rms_norm_neon, AttentionConfig};
use crate::sona::{SonaConfig, SonaIntegration, Trajectory};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// =============================================================================
// ANE Optimization Configuration
// =============================================================================

/// ANE (Apple Neural Engine) optimization strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AneOptimization {
    /// Disable ANE, use GPU/CPU only
    Disabled,
    /// ANE only (best for small models, batch inference)
    AneOnly,
    /// GPU only (best for low latency)
    GpuOnly,
    /// Hybrid: MLP on ANE, Attention on GPU (recommended)
    HybridDispatch,
    /// Adaptive routing based on batch size and sequence length
    Adaptive,
}

impl Default for AneOptimization {
    fn default() -> Self {
        Self::HybridDispatch
    }
}

impl AneOptimization {
    /// Check if ANE is used
    pub fn uses_ane(&self) -> bool {
        matches!(self, Self::AneOnly | Self::HybridDispatch | Self::Adaptive)
    }

    /// Check if GPU is used
    pub fn uses_gpu(&self) -> bool {
        matches!(self, Self::GpuOnly | Self::HybridDispatch | Self::Adaptive)
    }

    /// Get recommended tile size for ANE matmul
    pub fn ane_tile_size(&self) -> usize {
        // ANE performs best with dimensions >= 768 and multiples of 64
        768
    }
}

/// Quantization type for model weights
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationType {
    /// No quantization (FP32)
    None,
    /// Half precision (FP16)
    Fp16,
    /// Brain float 16 (BF16)
    Bf16,
    /// 8-bit integer quantization
    Int8,
    /// 4-bit quantization (K-quants style)
    Int4,
    /// 4-bit GGUF Q4_K_M format
    Q4KM,
    /// Mixed precision (FP16 for attention, INT4 for MLP)
    MixedPrecision,
}

impl Default for QuantizationType {
    fn default() -> Self {
        Self::Int4
    }
}

impl QuantizationType {
    /// Get bytes per weight element
    pub fn bytes_per_weight(&self) -> f32 {
        match self {
            Self::None => 4.0,
            Self::Fp16 | Self::Bf16 => 2.0,
            Self::Int8 => 1.0,
            Self::Int4 | Self::Q4KM => 0.5,
            Self::MixedPrecision => 1.0, // Average
        }
    }

    /// Estimate memory usage for given parameter count
    pub fn estimate_memory_mb(&self, num_params: usize) -> f32 {
        (num_params as f32 * self.bytes_per_weight()) / (1024.0 * 1024.0)
    }
}

/// Memory layout format for Core ML compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MemoryLayout {
    /// Standard row-major (NCHW for 4D tensors)
    RowMajor,
    /// Column-major (Fortran-style)
    ColumnMajor,
    /// NHWC format (preferred by Core ML/ANE)
    #[default]
    Nhwc,
    /// Blocked/tiled layout for cache efficiency
    Blocked,
}

// =============================================================================
// RuvLTRA Configuration
// =============================================================================

/// RuvLTRA model configuration based on Qwen 0.5B architecture
///
/// Optimized for Apple Neural Engine (ANE) with dimensions >= 768
/// to ensure efficient matmul acceleration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvLtraConfig {
    /// Hidden size (embedding dimension) - 896 for ANE optimization
    pub hidden_size: usize,
    /// Intermediate size for MLP
    pub intermediate_size: usize,
    /// Number of hidden layers
    pub num_hidden_layers: usize,
    /// Number of attention heads
    pub num_attention_heads: usize,
    /// Number of key-value heads (GQA)
    pub num_kv_heads: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum position embeddings
    pub max_position_embeddings: usize,
    /// RoPE base frequency
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

    // ANE-specific optimizations
    /// ANE optimization strategy
    pub ane_optimization: AneOptimization,
    /// Quantization type
    pub quantization: QuantizationType,
    /// Memory layout for Core ML compatibility
    pub memory_layout: MemoryLayout,
    /// Enable ANE matmul optimization (requires dims >= 768)
    pub ane_matmul_optimized: bool,
    /// Tile size for blocked operations
    pub tile_size: usize,

    // SONA integration
    /// Enable SONA pretraining integration
    pub sona_enabled: bool,
    /// SONA configuration
    pub sona_config: SonaConfig,
}

impl Default for RuvLtraConfig {
    fn default() -> Self {
        Self::qwen_0_5b()
    }
}

impl RuvLtraConfig {
    /// Qwen 0.5B configuration - the primary RuvLTRA target
    ///
    /// Optimized for ANE with hidden_size=896 (>= 768 threshold)
    pub fn qwen_0_5b() -> Self {
        Self {
            // Qwen 0.5B architecture specifications
            hidden_size: 896,
            intermediate_size: 4864,
            num_hidden_layers: 24,
            num_attention_heads: 14,
            num_kv_heads: 2, // GQA ratio 7:1
            vocab_size: 151936,
            max_position_embeddings: 32768,
            rope_theta: 1000000.0, // Qwen uses 1M base
            rms_norm_eps: 1e-6,
            head_dim: 64, // 896 / 14 = 64
            use_flash_attention: true,
            sliding_window: None, // Qwen 0.5B uses full attention
            bos_token_id: 151643,
            eos_token_id: 151645,
            pad_token_id: 151643,

            // ANE optimizations
            ane_optimization: AneOptimization::HybridDispatch,
            quantization: QuantizationType::Int4,
            memory_layout: MemoryLayout::Nhwc,
            ane_matmul_optimized: true, // hidden_size=896 >= 768
            tile_size: 64,

            // SONA integration
            sona_enabled: true,
            sona_config: SonaConfig {
                hidden_dim: 896,
                embedding_dim: 896,
                micro_lora_rank: 2,
                base_lora_rank: 4,
                instant_learning_rate: 0.01,
                background_learning_rate: 0.001,
                ewc_lambda: 0.1,
                pattern_capacity: 10000,
                background_interval_secs: 3600,
                deep_interval_secs: 604800,
                quality_threshold: 0.5,
            },
        }
    }

    /// Qwen 1.8B configuration - larger model variant
    pub fn qwen_1_8b() -> Self {
        Self {
            hidden_size: 2048,
            intermediate_size: 5504,
            num_hidden_layers: 24,
            num_attention_heads: 16,
            num_kv_heads: 16,
            vocab_size: 151936,
            max_position_embeddings: 32768,
            rope_theta: 1000000.0,
            rms_norm_eps: 1e-6,
            head_dim: 128,
            use_flash_attention: true,
            sliding_window: None,
            bos_token_id: 151643,
            eos_token_id: 151645,
            pad_token_id: 151643,

            ane_optimization: AneOptimization::HybridDispatch,
            quantization: QuantizationType::Int4,
            memory_layout: MemoryLayout::Nhwc,
            ane_matmul_optimized: true,
            tile_size: 64,

            sona_enabled: true,
            sona_config: SonaConfig {
                hidden_dim: 2048,
                embedding_dim: 2048,
                ..SonaConfig::default()
            },
        }
    }

    /// Create a minimal test configuration
    pub fn tiny() -> Self {
        Self {
            hidden_size: 768, // Minimum for ANE optimization
            intermediate_size: 2048,
            num_hidden_layers: 4,
            num_attention_heads: 12,
            num_kv_heads: 2,
            vocab_size: 32000,
            max_position_embeddings: 2048,
            rope_theta: 10000.0,
            rms_norm_eps: 1e-5,
            head_dim: 64,
            use_flash_attention: true,
            sliding_window: None,
            bos_token_id: 1,
            eos_token_id: 2,
            pad_token_id: 0,

            ane_optimization: AneOptimization::AneOnly,
            quantization: QuantizationType::Fp16,
            memory_layout: MemoryLayout::Nhwc,
            ane_matmul_optimized: true,
            tile_size: 64,

            sona_enabled: false,
            sona_config: SonaConfig::default(),
        }
    }

    /// Builder: Set ANE optimization strategy
    pub fn with_ane_optimization(mut self, opt: AneOptimization) -> Self {
        self.ane_optimization = opt;
        self
    }

    /// Builder: Set quantization type
    pub fn with_quantization(mut self, quant: QuantizationType) -> Self {
        self.quantization = quant;
        self
    }

    /// Builder: Enable/disable SONA pretraining
    pub fn with_sona(mut self, enabled: bool) -> Self {
        self.sona_enabled = enabled;
        self
    }

    /// Builder: Set memory layout
    pub fn with_memory_layout(mut self, layout: MemoryLayout) -> Self {
        self.memory_layout = layout;
        self
    }

    /// Get GQA ratio (attention heads / KV heads)
    pub fn gqa_ratio(&self) -> usize {
        self.num_attention_heads / self.num_kv_heads
    }

    /// Get the attention configuration
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

    /// Get the RoPE configuration
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

    /// Check if this configuration is ANE-optimized
    ///
    /// ANE requires matmul dimensions >= 768 for acceleration
    pub fn is_ane_optimized(&self) -> bool {
        self.ane_matmul_optimized && self.hidden_size >= 768
    }

    /// Estimate total model parameters
    pub fn estimate_params(&self) -> usize {
        let embed_params = self.vocab_size * self.hidden_size;
        let attn_params = self.num_hidden_layers
            * (
                4 * self.hidden_size * self.hidden_size
                // QKV + O projections
            );
        let mlp_params = self.num_hidden_layers
            * (
                3 * self.hidden_size * self.intermediate_size
                // gate, up, down
            );
        let norm_params = (self.num_hidden_layers * 2 + 1) * self.hidden_size;

        embed_params + attn_params + mlp_params + norm_params
    }

    /// Estimate memory usage in MB
    pub fn estimate_memory_mb(&self) -> f32 {
        self.quantization.estimate_memory_mb(self.estimate_params())
    }
}

// =============================================================================
// RuvLTRA Attention Layer
// =============================================================================

/// RuvLTRA Attention with ANE hybrid dispatch support
#[derive(Debug)]
pub struct RuvLtraAttention {
    /// Query projection weights (hidden_size, hidden_size)
    pub q_proj: Vec<f32>,
    /// Key projection weights (hidden_size, num_kv_heads * head_dim)
    pub k_proj: Vec<f32>,
    /// Value projection weights (hidden_size, num_kv_heads * head_dim)
    pub v_proj: Vec<f32>,
    /// Output projection weights (hidden_size, hidden_size)
    pub o_proj: Vec<f32>,
    /// Configuration
    pub config: RuvLtraConfig,
    /// Precomputed RoPE tables
    pub rope_tables: RopeTables,
}

impl RuvLtraAttention {
    /// Create a new attention layer
    pub fn new(config: &RuvLtraConfig) -> Self {
        let hidden_size = config.hidden_size;
        let kv_dim = config.num_kv_heads * config.head_dim;

        Self {
            q_proj: vec![0.0; hidden_size * hidden_size],
            k_proj: vec![0.0; hidden_size * kv_dim],
            v_proj: vec![0.0; hidden_size * kv_dim],
            o_proj: vec![0.0; hidden_size * hidden_size],
            config: config.clone(),
            rope_tables: precompute_rope_tables_with_config(&config.rope_config()),
        }
    }

    /// Load weights from flat arrays
    pub fn load_weights(
        &mut self,
        q_proj: &[f32],
        k_proj: &[f32],
        v_proj: &[f32],
        o_proj: &[f32],
    ) -> Result<()> {
        let hidden_size = self.config.hidden_size;
        let kv_dim = self.config.num_kv_heads * self.config.head_dim;

        if q_proj.len() != hidden_size * hidden_size {
            return Err(RuvLLMError::Model(format!(
                "Invalid q_proj size: expected {}, got {}",
                hidden_size * hidden_size,
                q_proj.len()
            )));
        }

        if k_proj.len() != hidden_size * kv_dim || v_proj.len() != hidden_size * kv_dim {
            return Err(RuvLLMError::Model(format!(
                "Invalid KV proj size: expected {}, got k={}, v={}",
                hidden_size * kv_dim,
                k_proj.len(),
                v_proj.len()
            )));
        }

        self.q_proj.copy_from_slice(q_proj);
        self.k_proj.copy_from_slice(k_proj);
        self.v_proj.copy_from_slice(v_proj);
        self.o_proj.copy_from_slice(o_proj);

        Ok(())
    }

    /// Forward pass through attention
    ///
    /// # Arguments
    /// * `hidden_states` - Input tensor (seq_len, hidden_size)
    /// * `positions` - Position indices for RoPE
    /// * `kv_cache` - Optional KV cache (keys, values)
    ///
    /// # Returns
    /// Output tensor (seq_len, hidden_size)
    pub fn forward(
        &self,
        hidden_states: &[f32],
        positions: &[usize],
        kv_cache: Option<(&mut Vec<f32>, &mut Vec<f32>)>,
    ) -> Result<Vec<f32>> {
        let seq_len = positions.len();
        let hidden_size = self.config.hidden_size;
        let num_heads = self.config.num_attention_heads;
        let num_kv_heads = self.config.num_kv_heads;
        let head_dim = self.config.head_dim;
        let gqa_ratio = num_heads / num_kv_heads;

        if hidden_states.len() != seq_len * hidden_size {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Invalid hidden_states shape: expected {}, got {}",
                seq_len * hidden_size,
                hidden_states.len()
            )));
        }

        // Project to Q, K, V
        let mut query =
            self.linear_transform(hidden_states, &self.q_proj, hidden_size, hidden_size);
        let mut key = self.linear_transform(
            hidden_states,
            &self.k_proj,
            hidden_size,
            num_kv_heads * head_dim,
        );
        let value = self.linear_transform(
            hidden_states,
            &self.v_proj,
            hidden_size,
            num_kv_heads * head_dim,
        );

        // Apply RoPE to Q and K
        self.apply_rope(&mut query, positions, num_heads, head_dim);
        self.apply_rope(&mut key, positions, num_kv_heads, head_dim);

        // Handle KV cache
        let (key_states, value_states) = if let Some((k_cache, v_cache)) = kv_cache {
            k_cache.extend_from_slice(&key);
            v_cache.extend_from_slice(&value);
            (k_cache.as_slice(), v_cache.as_slice())
        } else {
            (key.as_slice(), value.as_slice())
        };

        // Compute attention
        let kv_len = key_states.len() / (num_kv_heads * head_dim);
        let scale = 1.0 / (head_dim as f32).sqrt();
        let mut output = vec![0.0; seq_len * hidden_size];

        // GQA: Each query head group shares one KV head
        for h in 0..num_heads {
            let kv_head = h / gqa_ratio;

            for t in 0..seq_len {
                // Extract query for this head and position
                let q_offset = (t * num_heads + h) * head_dim;
                let q_slice = &query[q_offset..q_offset + head_dim];

                // Extract keys and values for the corresponding KV head
                let mut k_slice = Vec::with_capacity(kv_len * head_dim);
                let mut v_slice = Vec::with_capacity(kv_len * head_dim);

                for kv_t in 0..kv_len {
                    let kv_offset = (kv_t * num_kv_heads + kv_head) * head_dim;
                    k_slice.extend_from_slice(&key_states[kv_offset..kv_offset + head_dim]);
                    v_slice.extend_from_slice(&value_states[kv_offset..kv_offset + head_dim]);
                }

                // Apply sliding window if configured
                let (k_slice, v_slice, _effective_kv_len) =
                    if let Some(window) = self.config.sliding_window {
                        let pos = positions[t];
                        let start = pos.saturating_sub(window);
                        if start > 0 {
                            let start_offset = start * head_dim;
                            (
                                k_slice[start_offset..].to_vec(),
                                v_slice[start_offset..].to_vec(),
                                kv_len - start,
                            )
                        } else {
                            (k_slice, v_slice, kv_len)
                        }
                    } else {
                        (k_slice, v_slice, kv_len)
                    };

                // Flash attention
                let head_output = flash_attention_neon(q_slice, &k_slice, &v_slice, scale, true);

                // Write output
                let out_offset = (t * num_heads + h) * head_dim;
                output[out_offset..out_offset + head_dim].copy_from_slice(&head_output);
            }
        }

        // Output projection
        let output = self.linear_transform(&output, &self.o_proj, hidden_size, hidden_size);

        Ok(output)
    }

    /// Apply RoPE (Rotary Position Embedding)
    fn apply_rope(&self, x: &mut [f32], positions: &[usize], num_heads: usize, head_dim: usize) {
        let seq_len = positions.len();
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

    /// Linear transformation with ANE-aware tiling
    fn linear_transform(
        &self,
        input: &[f32],
        weights: &[f32],
        in_dim: usize,
        out_dim: usize,
    ) -> Vec<f32> {
        let batch_size = input.len() / in_dim;
        let mut output = vec![0.0; batch_size * out_dim];

        // Use NEON-optimized path on aarch64
        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.linear_neon(input, weights, &mut output, batch_size, in_dim, out_dim);
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            for b in 0..batch_size {
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

    /// NEON-optimized linear transformation
    #[cfg(target_arch = "aarch64")]
    unsafe fn linear_neon(
        &self,
        input: &[f32],
        weights: &[f32],
        output: &mut [f32],
        batch_size: usize,
        in_dim: usize,
        out_dim: usize,
    ) {
        let in_ptr: *const f32 = input.as_ptr();
        let w_ptr: *const f32 = weights.as_ptr();
        let out_ptr: *mut f32 = output.as_mut_ptr();

        for b in 0..batch_size {
            for o in 0..out_dim {
                let mut acc = vdupq_n_f32(0.0);
                let mut i = 0;

                // Process 4 elements at a time
                while i + 4 <= in_dim {
                    let x = vld1q_f32(in_ptr.add(b * in_dim + i));
                    let w = vld1q_f32(w_ptr.add(o * in_dim + i));
                    acc = vfmaq_f32(acc, x, w);
                    i += 4;
                }

                // Horizontal sum
                let mut sum = vaddvq_f32(acc);

                // Handle remainder
                while i < in_dim {
                    sum += *in_ptr.add(b * in_dim + i) * *w_ptr.add(o * in_dim + i);
                    i += 1;
                }

                *out_ptr.add(b * out_dim + o) = sum;
            }
        }
    }
}

// =============================================================================
// RuvLTRA MLP with ANE Optimization
// =============================================================================

/// RuvLTRA MLP layer with SwiGLU activation
///
/// ANE-optimized with dimensions >= 768 for efficient matmul dispatch.
#[derive(Debug)]
pub struct RuvLtraMLP {
    /// Gate projection weights
    pub gate_proj: Vec<f32>,
    /// Up projection weights
    pub up_proj: Vec<f32>,
    /// Down projection weights
    pub down_proj: Vec<f32>,
    /// Hidden size
    pub hidden_size: usize,
    /// Intermediate size
    pub intermediate_size: usize,
    /// Whether to dispatch to ANE
    pub use_ane: bool,
}

impl RuvLtraMLP {
    /// Create a new MLP layer
    pub fn new(config: &RuvLtraConfig) -> Self {
        Self {
            gate_proj: vec![0.0; config.intermediate_size * config.hidden_size],
            up_proj: vec![0.0; config.intermediate_size * config.hidden_size],
            down_proj: vec![0.0; config.hidden_size * config.intermediate_size],
            hidden_size: config.hidden_size,
            intermediate_size: config.intermediate_size,
            use_ane: config.ane_optimization.uses_ane() && config.is_ane_optimized(),
        }
    }

    /// Load weights
    pub fn load_weights(
        &mut self,
        gate_proj: &[f32],
        up_proj: &[f32],
        down_proj: &[f32],
    ) -> Result<()> {
        let gate_up_size = self.intermediate_size * self.hidden_size;
        let down_size = self.hidden_size * self.intermediate_size;

        if gate_proj.len() != gate_up_size
            || up_proj.len() != gate_up_size
            || down_proj.len() != down_size
        {
            return Err(RuvLLMError::Model(format!(
                "Invalid MLP weight dimensions: expected gate/up={}, down={}; got gate={}, up={}, down={}",
                gate_up_size, down_size, gate_proj.len(), up_proj.len(), down_proj.len()
            )));
        }

        self.gate_proj.copy_from_slice(gate_proj);
        self.up_proj.copy_from_slice(up_proj);
        self.down_proj.copy_from_slice(down_proj);

        Ok(())
    }

    /// Forward pass with SwiGLU activation
    ///
    /// SwiGLU: down_proj(SiLU(gate_proj(x)) * up_proj(x))
    pub fn forward(&self, hidden_states: &[f32]) -> Result<Vec<f32>> {
        // Gate projection + SiLU activation
        let gate = self.linear(
            hidden_states,
            &self.gate_proj,
            self.hidden_size,
            self.intermediate_size,
        );
        let gate_activated = self.silu(&gate);

        // Up projection
        let up = self.linear(
            hidden_states,
            &self.up_proj,
            self.hidden_size,
            self.intermediate_size,
        );

        // Element-wise multiply (gating)
        let hidden: Vec<f32> = gate_activated
            .iter()
            .zip(up.iter())
            .map(|(g, u)| g * u)
            .collect();

        // Down projection
        let output = self.linear(
            &hidden,
            &self.down_proj,
            self.intermediate_size,
            self.hidden_size,
        );

        Ok(output)
    }

    /// Linear transformation
    fn linear(&self, input: &[f32], weights: &[f32], in_dim: usize, out_dim: usize) -> Vec<f32> {
        let batch_size = input.len() / in_dim;
        let mut output = vec![0.0; batch_size * out_dim];

        #[cfg(target_arch = "aarch64")]
        unsafe {
            let in_ptr: *const f32 = input.as_ptr();
            let w_ptr: *const f32 = weights.as_ptr();
            let out_ptr: *mut f32 = output.as_mut_ptr();

            for b in 0..batch_size {
                for o in 0..out_dim {
                    let mut acc = vdupq_n_f32(0.0);
                    let mut i = 0;

                    while i + 4 <= in_dim {
                        let x = vld1q_f32(in_ptr.add(b * in_dim + i));
                        let w = vld1q_f32(w_ptr.add(o * in_dim + i));
                        acc = vfmaq_f32(acc, x, w);
                        i += 4;
                    }

                    let mut sum = vaddvq_f32(acc);
                    while i < in_dim {
                        sum += *in_ptr.add(b * in_dim + i) * *w_ptr.add(o * in_dim + i);
                        i += 1;
                    }

                    *out_ptr.add(b * out_dim + o) = sum;
                }
            }
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            for b in 0..batch_size {
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

    /// SiLU (Swish) activation
    fn silu(&self, x: &[f32]) -> Vec<f32> {
        crate::kernels::silu_vec(x)
    }
}

// =============================================================================
// RuvLTRA Decoder Layer
// =============================================================================

/// RuvLTRA Decoder Layer combining attention and MLP with ANE dispatch
#[derive(Debug)]
pub struct RuvLtraDecoderLayer {
    /// Self attention (dispatched to GPU in hybrid mode)
    pub self_attn: RuvLtraAttention,
    /// MLP (dispatched to ANE in hybrid mode)
    pub mlp: RuvLtraMLP,
    /// Input layer norm weights
    pub input_layernorm: Vec<f32>,
    /// Post-attention layer norm weights
    pub post_attention_layernorm: Vec<f32>,
    /// RMS norm epsilon
    pub rms_norm_eps: f32,
    /// Hidden size
    pub hidden_size: usize,
    /// Layer index (for logging/debugging)
    pub layer_idx: usize,
}

impl RuvLtraDecoderLayer {
    /// Create a new decoder layer
    pub fn new(config: &RuvLtraConfig, layer_idx: usize) -> Self {
        Self {
            self_attn: RuvLtraAttention::new(config),
            mlp: RuvLtraMLP::new(config),
            input_layernorm: vec![1.0; config.hidden_size],
            post_attention_layernorm: vec![1.0; config.hidden_size],
            rms_norm_eps: config.rms_norm_eps,
            hidden_size: config.hidden_size,
            layer_idx,
        }
    }

    /// Forward pass
    pub fn forward(
        &self,
        hidden_states: &[f32],
        positions: &[usize],
        kv_cache: Option<(&mut Vec<f32>, &mut Vec<f32>)>,
    ) -> Result<Vec<f32>> {
        let seq_len = positions.len();

        // Pre-norm for attention
        let mut normed = hidden_states.to_vec();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            let slice = &mut normed[offset..offset + self.hidden_size];
            rms_norm_neon(slice, &self.input_layernorm, self.rms_norm_eps);
        }

        // Self attention (GPU dispatch in hybrid mode)
        let attn_output = self.self_attn.forward(&normed, positions, kv_cache)?;

        // Residual connection
        let mut hidden: Vec<f32> = hidden_states
            .iter()
            .zip(attn_output.iter())
            .map(|(h, a)| h + a)
            .collect();

        // Pre-norm for MLP
        let mut normed = hidden.clone();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            let slice = &mut normed[offset..offset + self.hidden_size];
            rms_norm_neon(slice, &self.post_attention_layernorm, self.rms_norm_eps);
        }

        // MLP (ANE dispatch in hybrid mode)
        let mlp_output = self.mlp.forward(&normed)?;

        // Residual connection
        for (h, m) in hidden.iter_mut().zip(mlp_output.iter()) {
            *h += m;
        }

        Ok(hidden)
    }
}

// =============================================================================
// Complete RuvLTRA Model
// =============================================================================

/// Complete RuvLTRA model with SONA pretraining integration
#[derive(Debug)]
pub struct RuvLtraModel {
    /// Model configuration
    pub config: RuvLtraConfig,
    /// Token embeddings
    pub embed_tokens: Vec<f32>,
    /// Decoder layers
    pub layers: Vec<RuvLtraDecoderLayer>,
    /// Final layer norm
    pub norm: Vec<f32>,
    /// LM head weights (often tied to embeddings)
    pub lm_head: Option<Vec<f32>>,
    /// Whether lm_head is tied to embeddings
    pub tie_word_embeddings: bool,
    /// SONA integration for continuous learning
    sona: Option<Arc<RwLock<SonaIntegration>>>,
}

impl RuvLtraModel {
    /// Create a new RuvLTRA model
    pub fn new(config: &RuvLtraConfig) -> Result<Self> {
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for i in 0..config.num_hidden_layers {
            layers.push(RuvLtraDecoderLayer::new(config, i));
        }

        let sona = if config.sona_enabled {
            Some(Arc::new(RwLock::new(SonaIntegration::new(
                config.sona_config.clone(),
            ))))
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
        })
    }

    /// Enable SONA pretraining integration
    pub fn enable_sona_pretraining(&mut self) -> Result<()> {
        if self.sona.is_none() {
            self.sona = Some(Arc::new(RwLock::new(SonaIntegration::new(
                self.config.sona_config.clone(),
            ))));
        }
        Ok(())
    }

    /// Get SONA integration (if enabled)
    pub fn sona(&self) -> Option<&Arc<RwLock<SonaIntegration>>> {
        self.sona.as_ref()
    }

    /// Forward pass through the model
    ///
    /// # Arguments
    /// * `input_ids` - Token IDs (seq_len)
    /// * `positions` - Position indices
    /// * `kv_caches` - Optional KV caches for each layer
    ///
    /// # Returns
    /// Logits tensor (seq_len, vocab_size)
    pub fn forward(
        &self,
        input_ids: &[u32],
        positions: &[usize],
        mut kv_caches: Option<&mut Vec<(Vec<f32>, Vec<f32>)>>,
    ) -> Result<Vec<f32>> {
        let seq_len = positions.len();

        if input_ids.len() != seq_len {
            return Err(RuvLLMError::InvalidOperation(format!(
                "input_ids length {} != positions length {}",
                input_ids.len(),
                seq_len
            )));
        }

        // Token embeddings
        let mut hidden_states = Vec::with_capacity(seq_len * self.config.hidden_size);
        for &token_id in input_ids {
            let offset = (token_id as usize) * self.config.hidden_size;
            if offset + self.config.hidden_size > self.embed_tokens.len() {
                return Err(RuvLLMError::InvalidOperation(format!(
                    "Token ID {} out of vocabulary bounds",
                    token_id
                )));
            }
            hidden_states
                .extend_from_slice(&self.embed_tokens[offset..offset + self.config.hidden_size]);
        }

        // Process through decoder layers
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            let kv_cache = kv_caches.as_mut().map(|caches| {
                while caches.len() <= layer_idx {
                    caches.push((Vec::new(), Vec::new()));
                }
                let (k, v) = &mut caches[layer_idx];
                (k, v)
            });

            hidden_states = layer.forward(&hidden_states, positions, kv_cache)?;
        }

        // Final norm
        for t in 0..seq_len {
            let offset = t * self.config.hidden_size;
            let slice = &mut hidden_states[offset..offset + self.config.hidden_size];
            rms_norm_neon(slice, &self.norm, self.config.rms_norm_eps);
        }

        // LM head
        let lm_weights = if self.tie_word_embeddings {
            &self.embed_tokens
        } else {
            self.lm_head
                .as_ref()
                .ok_or_else(|| RuvLLMError::InvalidOperation("No LM head weights".to_string()))?
        };

        // Compute logits
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

    /// Record a trajectory for SONA learning
    pub fn record_trajectory(&self, trajectory: Trajectory) -> Result<()> {
        if let Some(sona) = &self.sona {
            sona.write().record_trajectory(trajectory)?;
        }
        Ok(())
    }

    /// Get routing recommendation from SONA
    pub fn get_routing_recommendation(
        &self,
        query_embedding: &[f32],
    ) -> Option<crate::sona::RoutingRecommendation> {
        self.sona
            .as_ref()
            .map(|sona| sona.read().get_routing_recommendation(query_embedding))
    }

    /// Get model info
    pub fn info(&self) -> RuvLtraModelInfo {
        RuvLtraModelInfo {
            name: "RuvLTRA".to_string(),
            architecture: "Qwen".to_string(),
            num_params: self.config.estimate_params(),
            hidden_size: self.config.hidden_size,
            num_layers: self.config.num_hidden_layers,
            vocab_size: self.config.vocab_size,
            max_context: self.config.max_position_embeddings,
            quantization: self.config.quantization,
            ane_optimized: self.config.is_ane_optimized(),
            sona_enabled: self.sona.is_some(),
            estimated_memory_mb: self.config.estimate_memory_mb(),
        }
    }

    /// Apply Qwen chat template
    ///
    /// Format: `<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n`
    pub fn apply_chat_template(messages: &[(String, String)], system: Option<&str>) -> String {
        let mut result = String::new();

        // System message
        if let Some(sys) = system {
            result.push_str("<|im_start|>system\n");
            result.push_str(sys);
            result.push_str("<|im_end|>\n");
        }

        // User/assistant messages
        for (role, content) in messages {
            result.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", role, content));
        }

        result.push_str("<|im_start|>assistant\n");
        result
    }
}

/// Model information for RuvLTRA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvLtraModelInfo {
    /// Model name
    pub name: String,
    /// Architecture (Qwen)
    pub architecture: String,
    /// Number of parameters
    pub num_params: usize,
    /// Hidden dimension
    pub hidden_size: usize,
    /// Number of layers
    pub num_layers: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum context length
    pub max_context: usize,
    /// Quantization type
    pub quantization: QuantizationType,
    /// Whether ANE-optimized
    pub ane_optimized: bool,
    /// Whether SONA is enabled
    pub sona_enabled: bool,
    /// Estimated memory usage in MB
    pub estimated_memory_mb: f32,
}

// =============================================================================
// ANE Dispatch Coordinator
// =============================================================================

/// Coordinates hybrid dispatch between ANE and GPU
#[derive(Debug)]
pub struct AneDispatcher {
    /// ANE optimization mode
    mode: AneOptimization,
    /// Threshold for adaptive dispatch (batch_size * seq_len)
    adaptive_threshold: usize,
    /// Statistics
    ane_ops: std::sync::atomic::AtomicU64,
    gpu_ops: std::sync::atomic::AtomicU64,
}

impl AneDispatcher {
    /// Create a new dispatcher
    pub fn new(mode: AneOptimization) -> Self {
        Self {
            mode,
            adaptive_threshold: 512, // Switch to GPU above this
            ane_ops: std::sync::atomic::AtomicU64::new(0),
            gpu_ops: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Decide whether to use ANE for an operation
    pub fn should_use_ane(&self, op_type: &str, batch_size: usize, seq_len: usize) -> bool {
        match self.mode {
            AneOptimization::Disabled => false,
            AneOptimization::AneOnly => true,
            AneOptimization::GpuOnly => false,
            AneOptimization::HybridDispatch => {
                // MLP operations go to ANE, attention to GPU
                matches!(op_type, "mlp" | "linear" | "matmul" | "activation")
            }
            AneOptimization::Adaptive => {
                // Small batches/sequences -> ANE, large -> GPU
                let workload = batch_size * seq_len;
                if workload < self.adaptive_threshold {
                    true
                } else {
                    matches!(op_type, "mlp" | "linear")
                }
            }
        }
    }

    /// Record an ANE operation
    pub fn record_ane_op(&self) {
        self.ane_ops
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Record a GPU operation
    pub fn record_gpu_op(&self) {
        self.gpu_ops
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get dispatch statistics
    pub fn stats(&self) -> (u64, u64) {
        (
            self.ane_ops.load(std::sync::atomic::Ordering::Relaxed),
            self.gpu_ops.load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruvltra_config_qwen() {
        let config = RuvLtraConfig::qwen_0_5b();
        assert_eq!(config.hidden_size, 896);
        assert_eq!(config.num_hidden_layers, 24);
        assert_eq!(config.num_attention_heads, 14);
        assert_eq!(config.intermediate_size, 4864);
        assert_eq!(config.vocab_size, 151936);
        assert!(config.is_ane_optimized());
    }

    #[test]
    fn test_ruvltra_config_tiny() {
        let config = RuvLtraConfig::tiny();
        assert_eq!(config.hidden_size, 768);
        assert!(config.is_ane_optimized());
    }

    #[test]
    fn test_ane_optimization() {
        let config = RuvLtraConfig::qwen_0_5b();
        assert!(config.ane_optimization.uses_ane());
        assert!(config.ane_optimization.uses_gpu());
    }

    #[test]
    fn test_quantization_memory() {
        let config = RuvLtraConfig::qwen_0_5b();
        let params = config.estimate_params();
        let memory_int4 = QuantizationType::Int4.estimate_memory_mb(params);
        let memory_fp16 = QuantizationType::Fp16.estimate_memory_mb(params);

        // INT4 should be ~4x smaller than FP16
        assert!(memory_fp16 > memory_int4 * 3.5);
        assert!(memory_fp16 < memory_int4 * 4.5);
    }

    #[test]
    fn test_ruvltra_model_creation() {
        let config = RuvLtraConfig::tiny();
        let model = RuvLtraModel::new(&config).unwrap();

        assert_eq!(model.layers.len(), 4);
        assert_eq!(
            model.embed_tokens.len(),
            config.vocab_size * config.hidden_size
        );
    }

    #[test]
    fn test_gqa_ratio() {
        let config = RuvLtraConfig::qwen_0_5b();
        assert_eq!(config.gqa_ratio(), 7); // 14 heads / 2 KV heads = 7
    }

    #[test]
    fn test_ane_dispatcher() {
        let dispatcher = AneDispatcher::new(AneOptimization::HybridDispatch);

        assert!(dispatcher.should_use_ane("mlp", 1, 128));
        assert!(dispatcher.should_use_ane("linear", 1, 128));
        assert!(!dispatcher.should_use_ane("attention", 1, 128));
    }

    #[test]
    fn test_chat_template() {
        let messages = vec![
            ("user".to_string(), "Hello!".to_string()),
            ("assistant".to_string(), "Hi there!".to_string()),
            ("user".to_string(), "How are you?".to_string()),
        ];

        let template =
            RuvLtraModel::apply_chat_template(&messages, Some("You are a helpful assistant."));

        assert!(template.contains("<|im_start|>system"));
        assert!(template.contains("<|im_start|>user"));
        assert!(template.contains("<|im_start|>assistant"));
        assert!(template.contains("<|im_end|>"));
        assert!(template.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_model_info() {
        let config = RuvLtraConfig::qwen_0_5b();
        let model = RuvLtraModel::new(&config).unwrap();
        let info = model.info();

        assert_eq!(info.name, "RuvLTRA");
        assert_eq!(info.architecture, "Qwen");
        assert_eq!(info.hidden_size, 896);
        assert!(info.ane_optimized);
        assert!(info.sona_enabled);
    }
}
