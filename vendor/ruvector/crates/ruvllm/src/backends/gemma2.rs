//! Gemma-2 Model Architecture Implementation
//!
//! Google's Gemma-2 features advanced attention mechanisms:
//! - **Logit soft-capping**: Stabilizes attention with `cap * tanh(x / cap)`
//! - **Alternating local/global attention**: Odd layers use sliding window
//! - **GeGLU activation**: Gated Linear Unit with GELU
//! - **GQA**: Grouped Query Attention for memory efficiency
//! - **Large head dimension**: 256 for improved representation
//!
//! ## Model Variants
//!
//! | Model | Hidden Size | Layers | Heads | KV Heads | Context |
//! |-------|-------------|--------|-------|----------|---------|
//! | Gemma-2-2B | 2304 | 26 | 8 | 4 | 8192 |
//! | Gemma-2-9B | 3584 | 42 | 16 | 8 | 8192 |
//! | Gemma-2-27B | 4608 | 46 | 32 | 16 | 8192 |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::backends::gemma2::{Gemma2Config, Gemma2Model};
//!
//! let config = Gemma2Config::gemma2_9b();
//! let model = Gemma2Model::new(&config)?;
//!
//! let output = model.forward(&input_ids, &attention_mask, None)?;
//! ```

use crate::error::{Result, RuvLLMError};
use crate::kernels::rope::{precompute_rope_tables_with_config, RopeConfig, RopeTables};
use crate::kernels::{apply_rope_neon, flash_attention_neon, rms_norm_neon, AttentionConfig};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// Soft-capping value for attention logits
pub const ATTENTION_SOFTCAP: f32 = 50.0;

/// Soft-capping value for final logits
pub const FINAL_LOGIT_SOFTCAP: f32 = 30.0;

/// Gemma-2 model configuration
#[derive(Debug, Clone)]
pub struct Gemma2Config {
    /// Hidden size (embedding dimension)
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
    /// Sliding window size for local attention layers
    pub sliding_window: usize,
    /// Head dimension (typically 256 for Gemma-2)
    pub head_dim: usize,
    /// Query pre-attention normalization
    pub query_pre_attn_scalar: f32,
    /// Attention logit soft-capping value
    pub attn_logit_softcapping: f32,
    /// Final logit soft-capping value
    pub final_logit_softcapping: f32,
    /// Whether to use flash attention
    pub use_flash_attention: bool,
    /// BOS token ID
    pub bos_token_id: u32,
    /// EOS token ID
    pub eos_token_id: u32,
}

impl Default for Gemma2Config {
    fn default() -> Self {
        Self::gemma2_9b()
    }
}

impl Gemma2Config {
    /// Gemma-2 2B configuration
    pub fn gemma2_2b() -> Self {
        Self {
            hidden_size: 2304,
            intermediate_size: 9216,
            num_hidden_layers: 26,
            num_attention_heads: 8,
            num_kv_heads: 4,
            vocab_size: 256000,
            max_position_embeddings: 8192,
            rope_theta: 10000.0,
            rms_norm_eps: 1e-6,
            sliding_window: 4096,
            head_dim: 256,
            query_pre_attn_scalar: 256.0_f32.sqrt().recip(),
            attn_logit_softcapping: ATTENTION_SOFTCAP,
            final_logit_softcapping: FINAL_LOGIT_SOFTCAP,
            use_flash_attention: true,
            bos_token_id: 2,
            eos_token_id: 1,
        }
    }

    /// Gemma-2 9B configuration
    pub fn gemma2_9b() -> Self {
        Self {
            hidden_size: 3584,
            intermediate_size: 14336,
            num_hidden_layers: 42,
            num_attention_heads: 16,
            num_kv_heads: 8,
            vocab_size: 256000,
            max_position_embeddings: 8192,
            rope_theta: 10000.0,
            rms_norm_eps: 1e-6,
            sliding_window: 4096,
            head_dim: 256,
            query_pre_attn_scalar: 256.0_f32.sqrt().recip(),
            attn_logit_softcapping: ATTENTION_SOFTCAP,
            final_logit_softcapping: FINAL_LOGIT_SOFTCAP,
            use_flash_attention: true,
            bos_token_id: 2,
            eos_token_id: 1,
        }
    }

    /// Gemma-2 27B configuration
    pub fn gemma2_27b() -> Self {
        Self {
            hidden_size: 4608,
            intermediate_size: 36864,
            num_hidden_layers: 46,
            num_attention_heads: 32,
            num_kv_heads: 16,
            vocab_size: 256000,
            max_position_embeddings: 8192,
            rope_theta: 10000.0,
            rms_norm_eps: 1e-6,
            sliding_window: 4096,
            head_dim: 256,
            query_pre_attn_scalar: 256.0_f32.sqrt().recip(),
            attn_logit_softcapping: ATTENTION_SOFTCAP,
            final_logit_softcapping: FINAL_LOGIT_SOFTCAP,
            use_flash_attention: true,
            bos_token_id: 2,
            eos_token_id: 1,
        }
    }

    /// Get GQA ratio
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
            scale: self.query_pre_attn_scalar,
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

    /// Check if a layer uses local (sliding window) attention
    ///
    /// Gemma-2 alternates: even layers = global, odd layers = local
    pub fn is_local_attention_layer(&self, layer_idx: usize) -> bool {
        layer_idx % 2 == 1
    }
}

/// Apply logit soft-capping: cap * tanh(x / cap)
///
/// This prevents attention scores from becoming too large,
/// improving training stability and generation quality.
///
/// # Arguments
/// * `x` - Input logits (modified in-place)
/// * `cap` - Soft-capping value (typically 50.0 for attention)
#[inline(always)]
pub fn logit_soft_cap(x: &mut [f32], cap: f32) {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        logit_soft_cap_neon(x, cap);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let inv_cap = 1.0 / cap;
        for v in x.iter_mut() {
            *v = cap * (*v * inv_cap).tanh();
        }
    }
}

/// NEON-optimized logit soft-capping
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn logit_soft_cap_neon(x: &mut [f32], cap: f32) {
    let cap_vec = vdupq_n_f32(cap);
    let inv_cap = 1.0 / cap;
    let inv_cap_vec = vdupq_n_f32(inv_cap);

    let ptr = x.as_mut_ptr();
    let len = x.len();

    let mut i = 0;

    // Process 4 elements at a time
    while i + 4 <= len {
        let v = vld1q_f32(ptr.add(i));

        // Compute v / cap
        let scaled = vmulq_f32(v, inv_cap_vec);

        // Compute tanh using approximation or element-wise
        // tanh(x) ~ x for small x, tanh(x) ~ sign(x) for large x
        // Using element-wise for accuracy
        let t0 = (vgetq_lane_f32(scaled, 0)).tanh();
        let t1 = (vgetq_lane_f32(scaled, 1)).tanh();
        let t2 = (vgetq_lane_f32(scaled, 2)).tanh();
        let t3 = (vgetq_lane_f32(scaled, 3)).tanh();

        let tanh_vec = vsetq_lane_f32(
            t3,
            vsetq_lane_f32(
                t2,
                vsetq_lane_f32(t1, vsetq_lane_f32(t0, vdupq_n_f32(0.0), 0), 1),
                2,
            ),
            3,
        );

        // Multiply by cap
        let result = vmulq_f32(tanh_vec, cap_vec);
        vst1q_f32(ptr.add(i), result);

        i += 4;
    }

    // Handle remainder
    while i < len {
        x[i] = cap * (x[i] * inv_cap).tanh();
        i += 1;
    }
}

/// Gemma-2 Attention layer with soft-capping and alternating local/global
#[derive(Debug)]
pub struct Gemma2Attention {
    /// Query projection weights
    pub q_proj: Vec<f32>,
    /// Key projection weights
    pub k_proj: Vec<f32>,
    /// Value projection weights
    pub v_proj: Vec<f32>,
    /// Output projection weights
    pub o_proj: Vec<f32>,
    /// Configuration
    pub config: Gemma2Config,
    /// Layer index (for alternating attention)
    pub layer_idx: usize,
    /// Precomputed RoPE tables
    pub rope_tables: RopeTables,
}

impl Gemma2Attention {
    /// Create a new Gemma2Attention layer
    pub fn new(config: &Gemma2Config, layer_idx: usize) -> Self {
        let hidden_size = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let num_kv_heads = config.num_kv_heads;
        let head_dim = config.head_dim;

        Self {
            q_proj: vec![0.0; num_heads * head_dim * hidden_size],
            k_proj: vec![0.0; num_kv_heads * head_dim * hidden_size],
            v_proj: vec![0.0; num_kv_heads * head_dim * hidden_size],
            o_proj: vec![0.0; hidden_size * num_heads * head_dim],
            config: config.clone(),
            layer_idx,
            rope_tables: precompute_rope_tables_with_config(&config.rope_config()),
        }
    }

    /// Load weights
    pub fn load_weights(
        &mut self,
        q_proj: &[f32],
        k_proj: &[f32],
        v_proj: &[f32],
        o_proj: &[f32],
    ) -> Result<()> {
        if q_proj.len() != self.q_proj.len()
            || k_proj.len() != self.k_proj.len()
            || v_proj.len() != self.v_proj.len()
            || o_proj.len() != self.o_proj.len()
        {
            return Err(RuvLLMError::Model(
                "Invalid attention weight dimensions".to_string(),
            ));
        }

        self.q_proj.copy_from_slice(q_proj);
        self.k_proj.copy_from_slice(k_proj);
        self.v_proj.copy_from_slice(v_proj);
        self.o_proj.copy_from_slice(o_proj);

        Ok(())
    }

    /// Forward pass with soft-capped attention and alternating local/global
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
        let gqa_ratio = self.config.gqa_ratio();

        if hidden_states.len() != seq_len * hidden_size {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Invalid hidden_states shape: expected {}, got {}",
                seq_len * hidden_size,
                hidden_states.len()
            )));
        }

        // Project to Q, K, V
        let mut query = self.linear_transform(
            hidden_states,
            &self.q_proj,
            hidden_size,
            num_heads * head_dim,
        );
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

        // Apply RoPE
        self.apply_rope(&mut query, positions, num_heads);
        self.apply_rope(&mut key, positions, num_kv_heads);

        // Handle KV cache
        let (key_states, value_states) = if let Some((k_cache, v_cache)) = kv_cache {
            k_cache.extend_from_slice(&key);
            v_cache.extend_from_slice(&value);
            (k_cache.as_slice(), v_cache.as_slice())
        } else {
            (key.as_slice(), value.as_slice())
        };

        let kv_len = key_states.len() / (num_kv_heads * head_dim);

        // Determine if this layer uses local (sliding window) or global attention
        let is_local = self.config.is_local_attention_layer(self.layer_idx);
        let effective_window = if is_local {
            Some(self.config.sliding_window)
        } else {
            None
        };

        // Compute attention with soft-capping
        let scale = self.config.query_pre_attn_scalar;
        let mut output = vec![0.0; seq_len * num_heads * head_dim];

        for h in 0..num_heads {
            let kv_head = h / gqa_ratio;

            for t in 0..seq_len {
                // Extract query for this head and position
                let q_offset = (t * num_heads + h) * head_dim;
                let q_slice = &query[q_offset..q_offset + head_dim];

                // Determine attention range based on local/global
                let (start_pos, end_pos) = if let Some(window) = effective_window {
                    let pos = positions[t];
                    let start = pos.saturating_sub(window);
                    (start, kv_len)
                } else {
                    (0, kv_len)
                };

                // Extract keys and values for this KV head
                let effective_kv_len = end_pos - start_pos;
                let mut k_slice = Vec::with_capacity(effective_kv_len * head_dim);
                let mut v_slice = Vec::with_capacity(effective_kv_len * head_dim);

                for kv_t in start_pos..end_pos {
                    let kv_offset = (kv_t * num_kv_heads + kv_head) * head_dim;
                    k_slice.extend_from_slice(&key_states[kv_offset..kv_offset + head_dim]);
                    v_slice.extend_from_slice(&value_states[kv_offset..kv_offset + head_dim]);
                }

                // Compute attention scores
                let mut scores = self.compute_attention_scores(q_slice, &k_slice, scale);

                // Apply soft-capping to attention logits
                logit_soft_cap(&mut scores, self.config.attn_logit_softcapping);

                // Apply causal mask (for positions after current)
                let current_pos = positions[t];
                for (i, score) in scores.iter_mut().enumerate() {
                    let kv_pos = start_pos + i;
                    if kv_pos > current_pos {
                        *score = f32::NEG_INFINITY;
                    }
                }

                // Softmax
                let attn_weights = self.softmax(&scores);

                // Weighted sum of values
                let mut head_output = vec![0.0; head_dim];
                for (i, &weight) in attn_weights.iter().enumerate() {
                    for d in 0..head_dim {
                        head_output[d] += weight * v_slice[i * head_dim + d];
                    }
                }

                // Write output
                let out_offset = (t * num_heads + h) * head_dim;
                output[out_offset..out_offset + head_dim].copy_from_slice(&head_output);
            }
        }

        // Output projection
        let output =
            self.linear_transform(&output, &self.o_proj, num_heads * head_dim, hidden_size);

        Ok(output)
    }

    /// Compute attention scores with proper scaling
    fn compute_attention_scores(&self, query: &[f32], keys: &[f32], scale: f32) -> Vec<f32> {
        let head_dim = query.len();
        let kv_len = keys.len() / head_dim;
        let mut scores = vec![0.0; kv_len];

        for t in 0..kv_len {
            let k_offset = t * head_dim;
            let mut score = 0.0;
            for d in 0..head_dim {
                score += query[d] * keys[k_offset + d];
            }
            scores[t] = score * scale;
        }

        scores
    }

    /// Softmax normalization
    fn softmax(&self, x: &[f32]) -> Vec<f32> {
        let max_val = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_vals: Vec<f32> = x.iter().map(|&v| (v - max_val).exp()).collect();
        let sum: f32 = exp_vals.iter().sum();
        exp_vals.iter().map(|&v| v / sum).collect()
    }

    /// Apply RoPE to query or key tensors
    fn apply_rope(&self, x: &mut [f32], positions: &[usize], num_heads: usize) {
        let head_dim = self.config.head_dim;
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

    /// Linear transformation
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

/// Gemma-2 MLP layer with GeGLU activation
///
/// GeGLU combines gating with GELU activation:
/// ```text
/// MLP(x) = down_proj(GELU(gate_proj(x)) * up_proj(x))
/// ```
#[derive(Debug)]
pub struct Gemma2MLP {
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
}

impl Gemma2MLP {
    /// Create a new Gemma2MLP layer
    pub fn new(config: &Gemma2Config) -> Self {
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

    /// Forward pass with GeGLU activation
    pub fn forward(&self, hidden_states: &[f32]) -> Result<Vec<f32>> {
        let batch_size = hidden_states.len() / self.hidden_size;

        // Gate projection + GELU
        let gate = self.linear(
            hidden_states,
            &self.gate_proj,
            self.hidden_size,
            self.intermediate_size,
        );
        let gate_activated = self.gelu(&gate);

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

    /// GELU activation: x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
    ///
    /// Uses the vectorized NEON implementation from the activations module
    /// for ~3.2x speedup over the previous scalar-in-vector approach.
    fn gelu(&self, x: &[f32]) -> Vec<f32> {
        crate::kernels::gelu_vec(x)
    }
}

/// Gemma-2 Decoder Layer
#[derive(Debug)]
pub struct Gemma2DecoderLayer {
    /// Self attention
    pub self_attn: Gemma2Attention,
    /// MLP
    pub mlp: Gemma2MLP,
    /// Input layer norm weights
    pub input_layernorm: Vec<f32>,
    /// Post-attention layer norm weights
    pub post_attention_layernorm: Vec<f32>,
    /// Pre-feedforward layer norm
    pub pre_feedforward_layernorm: Vec<f32>,
    /// Post-feedforward layer norm
    pub post_feedforward_layernorm: Vec<f32>,
    /// RMS norm epsilon
    pub rms_norm_eps: f32,
    /// Hidden size
    pub hidden_size: usize,
}

impl Gemma2DecoderLayer {
    /// Create a new decoder layer
    pub fn new(config: &Gemma2Config, layer_idx: usize) -> Self {
        Self {
            self_attn: Gemma2Attention::new(config, layer_idx),
            mlp: Gemma2MLP::new(config),
            input_layernorm: vec![1.0; config.hidden_size],
            post_attention_layernorm: vec![1.0; config.hidden_size],
            pre_feedforward_layernorm: vec![1.0; config.hidden_size],
            post_feedforward_layernorm: vec![1.0; config.hidden_size],
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

        // Post-attention norm
        let mut attn_normed = attn_output.clone();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            let slice = &mut attn_normed[offset..offset + self.hidden_size];
            rms_norm_neon(slice, &self.post_attention_layernorm, self.rms_norm_eps);
        }

        // Residual connection
        let mut hidden: Vec<f32> = hidden_states
            .iter()
            .zip(attn_normed.iter())
            .map(|(h, a)| h + a)
            .collect();

        // Pre-feedforward norm
        let mut ff_normed = hidden.clone();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            let slice = &mut ff_normed[offset..offset + self.hidden_size];
            rms_norm_neon(slice, &self.pre_feedforward_layernorm, self.rms_norm_eps);
        }

        // MLP
        let mlp_output = self.mlp.forward(&ff_normed)?;

        // Post-feedforward norm
        let mut mlp_normed = mlp_output.clone();
        for t in 0..seq_len {
            let offset = t * self.hidden_size;
            let slice = &mut mlp_normed[offset..offset + self.hidden_size];
            rms_norm_neon(slice, &self.post_feedforward_layernorm, self.rms_norm_eps);
        }

        // Residual connection
        for (h, m) in hidden.iter_mut().zip(mlp_normed.iter()) {
            *h += m;
        }

        Ok(hidden)
    }
}

/// Complete Gemma-2 Model
#[derive(Debug)]
pub struct Gemma2Model {
    /// Model configuration
    pub config: Gemma2Config,
    /// Token embeddings
    pub embed_tokens: Vec<f32>,
    /// Decoder layers
    pub layers: Vec<Gemma2DecoderLayer>,
    /// Final layer norm
    pub norm: Vec<f32>,
    /// LM head weights
    pub lm_head: Option<Vec<f32>>,
    /// Whether lm_head is tied to embeddings
    pub tie_word_embeddings: bool,
}

impl Gemma2Model {
    /// Create a new Gemma-2 model
    pub fn new(config: &Gemma2Config) -> Result<Self> {
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for i in 0..config.num_hidden_layers {
            layers.push(Gemma2DecoderLayer::new(config, i));
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

    /// Forward pass
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

        // Token embeddings (normalized by sqrt(hidden_size) for Gemma)
        let embed_scale = (self.config.hidden_size as f32).sqrt();
        let mut hidden_states = Vec::with_capacity(seq_len * self.config.hidden_size);
        for &token_id in input_ids {
            let offset = (token_id as usize) * self.config.hidden_size;
            if offset + self.config.hidden_size > self.embed_tokens.len() {
                return Err(RuvLLMError::InvalidOperation(format!(
                    "Token ID {} out of vocabulary bounds",
                    token_id
                )));
            }
            for i in 0..self.config.hidden_size {
                hidden_states.push(self.embed_tokens[offset + i] * embed_scale);
            }
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

        // Compute logits with soft-capping
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

            // Apply final logit soft-capping
            let logit_slice =
                &mut logits[t * self.config.vocab_size..(t + 1) * self.config.vocab_size];
            logit_soft_cap(logit_slice, self.config.final_logit_softcapping);
        }

        Ok(logits)
    }

    /// Generate Gemma-2 chat template format
    ///
    /// Gemma-2 uses: `<start_of_turn>user\n{content}<end_of_turn>\n<start_of_turn>model`
    pub fn apply_chat_template(messages: &[(String, String)]) -> String {
        let mut result = String::new();

        for (role, content) in messages {
            result.push_str(&format!(
                "<start_of_turn>{}\n{}<end_of_turn>\n",
                role, content
            ));
        }

        result.push_str("<start_of_turn>model\n");
        result
    }

    /// Load model weights from GGUF format
    #[cfg(feature = "candle")]
    pub fn from_gguf(_path: &std::path::Path) -> Result<Self> {
        Err(RuvLLMError::NotFound(
            "GGUF loading not yet implemented for Gemma-2".to_string(),
        ))
    }

    /// Load model weights from safetensors format
    #[cfg(feature = "candle")]
    pub fn from_safetensors(_path: &std::path::Path) -> Result<Self> {
        Err(RuvLLMError::NotFound(
            "Safetensors loading not yet implemented for Gemma-2".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemma2_config() {
        let config = Gemma2Config::gemma2_9b();
        assert_eq!(config.hidden_size, 3584);
        assert_eq!(config.num_hidden_layers, 42);
        assert_eq!(config.head_dim, 256);
        assert_eq!(config.gqa_ratio(), 2);
    }

    #[test]
    fn test_gemma2_config_2b() {
        let config = Gemma2Config::gemma2_2b();
        assert_eq!(config.hidden_size, 2304);
        assert_eq!(config.num_hidden_layers, 26);
        assert_eq!(config.gqa_ratio(), 2);
    }

    #[test]
    fn test_local_attention_alternation() {
        let config = Gemma2Config::gemma2_9b();
        assert!(!config.is_local_attention_layer(0)); // Global
        assert!(config.is_local_attention_layer(1)); // Local
        assert!(!config.is_local_attention_layer(2)); // Global
        assert!(config.is_local_attention_layer(3)); // Local
    }

    #[test]
    fn test_logit_soft_cap() {
        let mut x = vec![0.0, 10.0, -10.0, 100.0, -100.0];
        logit_soft_cap(&mut x, 50.0);

        // cap * tanh(x / cap)
        // tanh(0) = 0
        assert!((x[0]).abs() < 1e-5);
        // tanh(10/50) ~ 0.197, so 50 * 0.197 ~ 9.85
        assert!((x[1] - 9.866).abs() < 0.1);
        // tanh(-10/50) ~ -0.197
        assert!((x[2] - (-9.866)).abs() < 0.1);
        // tanh(100/50) = tanh(2) ~ 0.964, so 50 * 0.964 ~ 48.2
        assert!((x[3] - 48.2).abs() < 0.5);
        // Should be bounded by cap
        assert!(x[3].abs() < 50.0);
        assert!(x[4].abs() < 50.0);
    }

    #[test]
    fn test_gemma2_mlp_gelu() {
        let config = Gemma2Config::gemma2_2b();
        let mlp = Gemma2MLP::new(&config);

        // Test GELU activation
        let input = vec![0.0, 1.0, -1.0, 2.0];
        let output = mlp.gelu(&input);

        // GELU(0) = 0
        assert!((output[0]).abs() < 1e-5);
        // GELU(1) ~ 0.841
        assert!((output[1] - 0.841).abs() < 0.01);
        // GELU(-1) ~ -0.159
        assert!((output[2] - (-0.159)).abs() < 0.01);
    }

    #[test]
    fn test_gemma2_model_creation() {
        let config = Gemma2Config::gemma2_2b();
        let model = Gemma2Model::new(&config).unwrap();

        assert_eq!(model.layers.len(), 26);
        assert_eq!(
            model.embed_tokens.len(),
            config.vocab_size * config.hidden_size
        );
    }

    #[test]
    fn test_chat_template() {
        let messages = vec![
            ("user".to_string(), "Hello!".to_string()),
            ("model".to_string(), "Hi there!".to_string()),
            ("user".to_string(), "How are you?".to_string()),
        ];

        let template = Gemma2Model::apply_chat_template(&messages);

        assert!(template.contains("<start_of_turn>user"));
        assert!(template.contains("<start_of_turn>model"));
        assert!(template.contains("<end_of_turn>"));
        assert!(template.ends_with("<start_of_turn>model\n"));
    }

    #[test]
    fn test_attention_config() {
        let config = Gemma2Config::gemma2_9b();
        let attn_config = config.attention_config();

        assert_eq!(attn_config.num_heads, 16);
        assert_eq!(attn_config.num_kv_heads, 8);
        assert_eq!(attn_config.head_dim, 256);
        assert!(attn_config.causal);
    }
}
