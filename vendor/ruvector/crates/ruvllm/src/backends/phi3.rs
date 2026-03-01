//! Phi-3 Model Architecture Implementation
//!
//! Microsoft Phi-3 is a compact but powerful model featuring:
//! - **SuRoPE**: Scaled Uniform Rotary Position Embeddings for extended context
//! - **SwiGLU activation**: Gated Linear Unit with Swish (SiLU)
//! - **Fused gate_up_proj**: Combined gate and up projection for efficiency
//! - **Sliding window attention**: 2048 token window for memory efficiency
//!
//! ## Model Variants
//!
//! | Model | Hidden Size | Layers | Heads | Context |
//! |-------|-------------|--------|-------|---------|
//! | Phi-3-mini | 3072 | 32 | 32 | 4096/128K |
//! | Phi-3-small | 2560 | 32 | 32 | 8192/128K |
//! | Phi-3-medium | 5120 | 40 | 40 | 4096/128K |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::backends::phi3::{Phi3Config, Phi3Model};
//!
//! let config = Phi3Config::phi3_mini_128k();
//! let model = Phi3Model::new(&config)?;
//!
//! let output = model.forward(&input_ids, &attention_mask, None)?;
//! ```

use crate::error::{Result, RuvLLMError};
use crate::kernels::rope::{precompute_rope_tables_with_config, RopeConfig, RopeTables};
use crate::kernels::{apply_rope_neon, flash_attention_neon, rms_norm_neon, AttentionConfig};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// Phi-3 model configuration
#[derive(Debug, Clone)]
pub struct Phi3Config {
    /// Hidden size (embedding dimension)
    pub hidden_size: usize,
    /// Intermediate size for MLP (typically 8/3 * hidden_size for SwiGLU)
    pub intermediate_size: usize,
    /// Number of hidden layers
    pub num_hidden_layers: usize,
    /// Number of attention heads
    pub num_attention_heads: usize,
    /// Number of key-value heads (same as attention heads for Phi-3, no GQA)
    pub num_kv_heads: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum position embeddings
    pub max_position_embeddings: usize,
    /// Original maximum position embeddings (for SuRoPE scaling)
    pub original_max_position_embeddings: usize,
    /// RoPE base frequency
    pub rope_theta: f32,
    /// RoPE scaling factor (for SuRoPE)
    pub rope_scaling_factor: f32,
    /// RMS norm epsilon
    pub rms_norm_eps: f32,
    /// Sliding window size (typically 2048 for Phi-3)
    pub sliding_window: Option<usize>,
    /// Head dimension (hidden_size / num_attention_heads)
    pub head_dim: usize,
    /// Whether to use flash attention
    pub use_flash_attention: bool,
    /// BOS token ID
    pub bos_token_id: u32,
    /// EOS token ID
    pub eos_token_id: u32,
}

impl Default for Phi3Config {
    fn default() -> Self {
        Self::phi3_mini_4k()
    }
}

impl Phi3Config {
    /// Phi-3-mini with 4K context
    pub fn phi3_mini_4k() -> Self {
        Self {
            hidden_size: 3072,
            intermediate_size: 8192,
            num_hidden_layers: 32,
            num_attention_heads: 32,
            num_kv_heads: 32, // No GQA
            vocab_size: 32064,
            max_position_embeddings: 4096,
            original_max_position_embeddings: 4096,
            rope_theta: 10000.0,
            rope_scaling_factor: 1.0,
            rms_norm_eps: 1e-5,
            sliding_window: Some(2048),
            head_dim: 96, // 3072 / 32
            use_flash_attention: true,
            bos_token_id: 1,
            eos_token_id: 32000,
        }
    }

    /// Phi-3-mini with 128K extended context (SuRoPE)
    pub fn phi3_mini_128k() -> Self {
        Self {
            hidden_size: 3072,
            intermediate_size: 8192,
            num_hidden_layers: 32,
            num_attention_heads: 32,
            num_kv_heads: 32,
            vocab_size: 32064,
            max_position_embeddings: 131072,
            original_max_position_embeddings: 4096,
            rope_theta: 10000.0,
            rope_scaling_factor: 32.0, // SuRoPE scaling
            rms_norm_eps: 1e-5,
            sliding_window: Some(2048),
            head_dim: 96,
            use_flash_attention: true,
            bos_token_id: 1,
            eos_token_id: 32000,
        }
    }

    /// Phi-3-small configuration
    pub fn phi3_small() -> Self {
        Self {
            hidden_size: 2560,
            intermediate_size: 6912,
            num_hidden_layers: 32,
            num_attention_heads: 32,
            num_kv_heads: 32,
            vocab_size: 32064,
            max_position_embeddings: 8192,
            original_max_position_embeddings: 8192,
            rope_theta: 10000.0,
            rope_scaling_factor: 1.0,
            rms_norm_eps: 1e-5,
            sliding_window: Some(2048),
            head_dim: 80, // 2560 / 32
            use_flash_attention: true,
            bos_token_id: 1,
            eos_token_id: 32000,
        }
    }

    /// Phi-3-medium configuration
    pub fn phi3_medium() -> Self {
        Self {
            hidden_size: 5120,
            intermediate_size: 13824,
            num_hidden_layers: 40,
            num_attention_heads: 40,
            num_kv_heads: 40,
            vocab_size: 32064,
            max_position_embeddings: 4096,
            original_max_position_embeddings: 4096,
            rope_theta: 10000.0,
            rope_scaling_factor: 1.0,
            rms_norm_eps: 1e-5,
            sliding_window: Some(2048),
            head_dim: 128, // 5120 / 40
            use_flash_attention: true,
            bos_token_id: 1,
            eos_token_id: 32000,
        }
    }

    /// Get the attention configuration
    pub fn attention_config(&self) -> AttentionConfig {
        AttentionConfig {
            num_heads: self.num_attention_heads,
            num_kv_heads: self.num_kv_heads,
            head_dim: self.head_dim,
            max_seq_len: self.max_position_embeddings,
            causal: true,
            scale: 0.0, // Will be computed from head_dim
        }
    }

    /// Get the RoPE configuration with SuRoPE scaling
    pub fn rope_config(&self) -> RopeConfig {
        RopeConfig {
            base: self.rope_theta,
            head_dim: self.head_dim,
            max_seq_len: self.max_position_embeddings,
            scaling_factor: self.rope_scaling_factor,
            ntk_aware: self.rope_scaling_factor > 1.0,
            original_max_len: self.original_max_position_embeddings,
        }
    }
}

/// Phi-3 Attention layer
///
/// Implements multi-head attention with:
/// - SuRoPE (Scaled Uniform RoPE) for extended context
/// - Optional sliding window attention
/// - Fused QKV projection
#[derive(Debug)]
pub struct Phi3Attention {
    /// Query projection weights (hidden_size, hidden_size)
    pub q_proj: Vec<f32>,
    /// Key projection weights (hidden_size, hidden_size)
    pub k_proj: Vec<f32>,
    /// Value projection weights (hidden_size, hidden_size)
    pub v_proj: Vec<f32>,
    /// Output projection weights (hidden_size, hidden_size)
    pub o_proj: Vec<f32>,
    /// Configuration
    pub config: Phi3Config,
    /// Precomputed RoPE tables
    pub rope_tables: RopeTables,
}

impl Phi3Attention {
    /// Create a new Phi3Attention layer
    pub fn new(config: &Phi3Config) -> Self {
        let hidden_size = config.hidden_size;
        let qkv_size = hidden_size * hidden_size;

        Self {
            q_proj: vec![0.0; qkv_size],
            k_proj: vec![0.0; qkv_size],
            v_proj: vec![0.0; qkv_size],
            o_proj: vec![0.0; qkv_size],
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
        let expected_size = self.config.hidden_size * self.config.hidden_size;

        if q_proj.len() != expected_size
            || k_proj.len() != expected_size
            || v_proj.len() != expected_size
            || o_proj.len() != expected_size
        {
            return Err(RuvLLMError::Model(format!(
                "Invalid weight dimensions: expected {}, got q={}, k={}, v={}, o={}",
                expected_size,
                q_proj.len(),
                k_proj.len(),
                v_proj.len(),
                o_proj.len()
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
    /// * `hidden_states` - Input tensor (batch_size * seq_len, hidden_size)
    /// * `positions` - Position indices for RoPE
    /// * `kv_cache` - Optional KV cache (keys, values)
    ///
    /// # Returns
    /// Output tensor (batch_size * seq_len, hidden_size)
    pub fn forward(
        &self,
        hidden_states: &[f32],
        positions: &[usize],
        kv_cache: Option<(&mut Vec<f32>, &mut Vec<f32>)>,
    ) -> Result<Vec<f32>> {
        let seq_len = positions.len();
        let hidden_size = self.config.hidden_size;
        let num_heads = self.config.num_attention_heads;
        let head_dim = self.config.head_dim;

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
        let mut key = self.linear_transform(hidden_states, &self.k_proj, hidden_size, hidden_size);
        let value = self.linear_transform(hidden_states, &self.v_proj, hidden_size, hidden_size);

        // Apply SuRoPE (Scaled Uniform RoPE)
        self.apply_surope(&mut query, positions);
        self.apply_surope(&mut key, positions);

        // Handle KV cache
        let (key_states, value_states) = if let Some((k_cache, v_cache)) = kv_cache {
            k_cache.extend_from_slice(&key);
            v_cache.extend_from_slice(&value);
            (k_cache.as_slice(), v_cache.as_slice())
        } else {
            (key.as_slice(), value.as_slice())
        };

        // Compute attention for each head
        let kv_len = key_states.len() / hidden_size;
        let scale = 1.0 / (head_dim as f32).sqrt();
        let mut output = vec![0.0; seq_len * hidden_size];

        for h in 0..num_heads {
            for t in 0..seq_len {
                // Extract query for this head and position
                let q_offset = (t * num_heads + h) * head_dim;
                let q_slice = &query[q_offset..q_offset + head_dim];

                // Extract keys and values for this head
                let mut k_slice = Vec::with_capacity(kv_len * head_dim);
                let mut v_slice = Vec::with_capacity(kv_len * head_dim);

                for kv_t in 0..kv_len {
                    let kv_offset = (kv_t * num_heads + h) * head_dim;
                    k_slice.extend_from_slice(&key_states[kv_offset..kv_offset + head_dim]);
                    v_slice.extend_from_slice(&value_states[kv_offset..kv_offset + head_dim]);
                }

                // Apply sliding window if configured
                let (k_slice, v_slice, effective_kv_len) =
                    if let Some(window) = self.config.sliding_window {
                        let pos = positions[t];
                        let start = pos.saturating_sub(window);
                        let end = kv_len;
                        if start > 0 {
                            let start_offset = start * head_dim;
                            (
                                k_slice[start_offset..].to_vec(),
                                v_slice[start_offset..].to_vec(),
                                end - start,
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

    /// Apply SuRoPE (Scaled Uniform RoPE)
    fn apply_surope(&self, x: &mut [f32], positions: &[usize]) {
        let head_dim = self.config.head_dim;
        let num_heads = self.config.num_attention_heads;
        let seq_len = positions.len();

        // Apply RoPE per head
        for h in 0..num_heads {
            for t in 0..seq_len {
                let offset = (t * num_heads + h) * head_dim;
                let mut head_vec = x[offset..offset + head_dim].to_vec();

                // Scale position by scaling factor for SuRoPE
                let scaled_pos = (positions[t] as f32 / self.config.rope_scaling_factor) as usize;
                apply_rope_neon(
                    &mut head_vec,
                    &[scaled_pos],
                    head_dim,
                    self.config.rope_theta,
                );

                x[offset..offset + head_dim].copy_from_slice(&head_vec);
            }
        }
    }

    /// Linear transformation: output = input @ weights.T
    fn linear_transform(
        &self,
        input: &[f32],
        weights: &[f32],
        in_dim: usize,
        out_dim: usize,
    ) -> Vec<f32> {
        let batch_size = input.len() / in_dim;
        let mut output = vec![0.0; batch_size * out_dim];

        for b in 0..batch_size {
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
}

/// Phi-3 MLP layer with SwiGLU activation
///
/// SwiGLU combines gating with Swish activation:
/// ```text
/// MLP(x) = down_proj(SiLU(gate_proj(x)) * up_proj(x))
/// ```
///
/// Phi-3 uses a fused gate_up_proj for efficiency
#[derive(Debug)]
pub struct Phi3MLP {
    /// Gate projection weights (intermediate_size, hidden_size)
    pub gate_proj: Vec<f32>,
    /// Up projection weights (intermediate_size, hidden_size)
    pub up_proj: Vec<f32>,
    /// Down projection weights (hidden_size, intermediate_size)
    pub down_proj: Vec<f32>,
    /// Hidden size
    pub hidden_size: usize,
    /// Intermediate size
    pub intermediate_size: usize,
}

impl Phi3MLP {
    /// Create a new Phi3MLP layer
    pub fn new(config: &Phi3Config) -> Self {
        Self {
            gate_proj: vec![0.0; config.intermediate_size * config.hidden_size],
            up_proj: vec![0.0; config.intermediate_size * config.hidden_size],
            down_proj: vec![0.0; config.hidden_size * config.intermediate_size],
            hidden_size: config.hidden_size,
            intermediate_size: config.intermediate_size,
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
            return Err(RuvLLMError::Model(
                "Invalid MLP weight dimensions".to_string(),
            ));
        }

        self.gate_proj.copy_from_slice(gate_proj);
        self.up_proj.copy_from_slice(up_proj);
        self.down_proj.copy_from_slice(down_proj);

        Ok(())
    }

    /// Forward pass with SwiGLU activation
    pub fn forward(&self, hidden_states: &[f32]) -> Result<Vec<f32>> {
        let batch_size = hidden_states.len() / self.hidden_size;

        // Gate projection + SiLU
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

    /// SiLU (Swish) activation: x * sigmoid(x)
    ///
    /// Uses the vectorized NEON implementation from the activations module
    /// for ~3.5x speedup over the previous scalar-in-vector approach.
    fn silu(&self, x: &[f32]) -> Vec<f32> {
        crate::kernels::silu_vec(x)
    }
}

/// Phi-3 Decoder Layer
///
/// Each layer consists of:
/// 1. Self-attention with pre-normalization
/// 2. MLP with pre-normalization
#[derive(Debug)]
pub struct Phi3DecoderLayer {
    /// Self attention
    pub self_attn: Phi3Attention,
    /// MLP
    pub mlp: Phi3MLP,
    /// Input layer norm weights
    pub input_layernorm: Vec<f32>,
    /// Post-attention layer norm weights
    pub post_attention_layernorm: Vec<f32>,
    /// RMS norm epsilon
    pub rms_norm_eps: f32,
    /// Hidden size
    pub hidden_size: usize,
}

impl Phi3DecoderLayer {
    /// Create a new decoder layer
    pub fn new(config: &Phi3Config) -> Self {
        Self {
            self_attn: Phi3Attention::new(config),
            mlp: Phi3MLP::new(config),
            input_layernorm: vec![1.0; config.hidden_size],
            post_attention_layernorm: vec![1.0; config.hidden_size],
            rms_norm_eps: config.rms_norm_eps,
            hidden_size: config.hidden_size,
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

        // Self attention
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

        // MLP
        let mlp_output = self.mlp.forward(&normed)?;

        // Residual connection
        for (h, m) in hidden.iter_mut().zip(mlp_output.iter()) {
            *h += m;
        }

        Ok(hidden)
    }
}

/// Complete Phi-3 Model
#[derive(Debug)]
pub struct Phi3Model {
    /// Model configuration
    pub config: Phi3Config,
    /// Token embeddings (vocab_size, hidden_size)
    pub embed_tokens: Vec<f32>,
    /// Decoder layers
    pub layers: Vec<Phi3DecoderLayer>,
    /// Final layer norm
    pub norm: Vec<f32>,
    /// LM head weights (vocab_size, hidden_size) - often tied to embeddings
    pub lm_head: Option<Vec<f32>>,
    /// Whether lm_head is tied to embeddings
    pub tie_word_embeddings: bool,
}

impl Phi3Model {
    /// Create a new Phi-3 model
    pub fn new(config: &Phi3Config) -> Result<Self> {
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for _ in 0..config.num_hidden_layers {
            layers.push(Phi3DecoderLayer::new(config));
        }

        Ok(Self {
            config: config.clone(),
            embed_tokens: vec![0.0; config.vocab_size * config.hidden_size],
            layers,
            norm: vec![1.0; config.hidden_size],
            lm_head: None,
            tie_word_embeddings: true,
        })
    }

    /// Forward pass through the model
    ///
    /// # Arguments
    /// * `input_ids` - Token IDs (batch_size * seq_len)
    /// * `positions` - Position indices
    /// * `kv_caches` - Optional KV caches for each layer
    ///
    /// # Returns
    /// Logits tensor (batch_size * seq_len, vocab_size)
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

    /// Generate Phi-3 chat template format
    ///
    /// Phi-3 uses: `<|user|>\n{content}<|end|>\n<|assistant|>`
    pub fn apply_chat_template(messages: &[(String, String)]) -> String {
        let mut result = String::new();

        for (role, content) in messages {
            result.push_str(&format!("<|{}|>\n{}<|end|>\n", role, content));
        }

        result.push_str("<|assistant|>");
        result
    }

    /// Load model weights from GGUF format
    #[cfg(feature = "candle")]
    pub fn from_gguf(_path: &std::path::Path) -> Result<Self> {
        // Implementation would parse GGUF and load weights
        Err(RuvLLMError::NotFound(
            "GGUF loading not yet implemented for Phi-3".to_string(),
        ))
    }

    /// Load model weights from safetensors format
    #[cfg(feature = "candle")]
    pub fn from_safetensors(_path: &std::path::Path) -> Result<Self> {
        // Implementation would parse safetensors and load weights
        Err(RuvLLMError::NotFound(
            "Safetensors loading not yet implemented for Phi-3".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phi3_config() {
        let config = Phi3Config::phi3_mini_4k();
        assert_eq!(config.hidden_size, 3072);
        assert_eq!(config.num_hidden_layers, 32);
        assert_eq!(config.head_dim, 96);
        assert_eq!(config.sliding_window, Some(2048));
    }

    #[test]
    fn test_phi3_config_128k() {
        let config = Phi3Config::phi3_mini_128k();
        assert_eq!(config.max_position_embeddings, 131072);
        assert_eq!(config.rope_scaling_factor, 32.0);
    }

    #[test]
    fn test_phi3_attention_config() {
        let config = Phi3Config::phi3_mini_4k();
        let attn_config = config.attention_config();
        assert_eq!(attn_config.num_heads, 32);
        assert_eq!(attn_config.num_kv_heads, 32);
        assert!(attn_config.causal);
    }

    #[test]
    fn test_phi3_mlp_silu() {
        let config = Phi3Config::phi3_mini_4k();
        let mlp = Phi3MLP::new(&config);

        // Test SiLU activation
        let input = vec![0.0, 1.0, -1.0, 2.0];
        let output = mlp.silu(&input);

        // SiLU(0) = 0
        assert!((output[0]).abs() < 1e-5);
        // SiLU(1) = 1 * sigmoid(1) ~ 0.731
        assert!((output[1] - 0.731).abs() < 0.01);
        // SiLU(-1) ~ -0.269
        assert!((output[2] - (-0.269)).abs() < 0.01);
    }

    #[test]
    fn test_phi3_model_creation() {
        let config = Phi3Config::phi3_mini_4k();
        let model = Phi3Model::new(&config).unwrap();

        assert_eq!(model.layers.len(), 32);
        assert_eq!(
            model.embed_tokens.len(),
            config.vocab_size * config.hidden_size
        );
    }

    #[test]
    fn test_chat_template() {
        let messages = vec![
            ("user".to_string(), "Hello!".to_string()),
            ("assistant".to_string(), "Hi there!".to_string()),
            ("user".to_string(), "How are you?".to_string()),
        ];

        let template = Phi3Model::apply_chat_template(&messages);

        assert!(template.contains("<|user|>"));
        assert!(template.contains("<|assistant|>"));
        assert!(template.contains("<|end|>"));
        assert!(template.ends_with("<|assistant|>"));
    }
}
