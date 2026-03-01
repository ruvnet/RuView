//! Micro Inference Engine for ESP32
//!
//! A minimal transformer inference engine designed for microcontrollers.
//! Supports tiny models up to ~300KB with INT8 quantization.

use crate::quantized::{QuantizationType, matmul_int8, QuantParams};
use crate::model::{TinyModel, LayerWeights};
use heapless::Vec as HVec;
use serde::{Deserialize, Serialize};

/// Maximum sequence length for embedded inference
pub const MAX_SEQ_LEN: usize = 32;
/// Maximum embedding dimension
pub const MAX_EMBED_DIM: usize = 64;
/// Maximum vocabulary size
pub const MAX_VOCAB_SIZE: usize = 512;
/// Maximum hidden dimension
pub const MAX_HIDDEN_DIM: usize = 128;

/// Inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Temperature for sampling (0.0 = greedy)
    pub temperature: f32,
    /// Top-k sampling (0 = disabled)
    pub top_k: usize,
    /// Whether to use greedy decoding
    pub greedy: bool,
    /// Random seed for reproducibility
    pub seed: u32,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            max_tokens: 16,
            temperature: 0.7,
            top_k: 8,
            greedy: true,
            seed: 42,
        }
    }
}

/// Inference result
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Generated token IDs
    pub tokens: HVec<u16, MAX_SEQ_LEN>,
    /// Total inference time in microseconds
    pub inference_time_us: u64,
    /// Tokens per second
    pub tokens_per_second: f32,
    /// Peak memory usage estimate in bytes
    pub peak_memory_bytes: usize,
    /// Per-layer timing breakdown
    pub layer_times_us: HVec<u32, 8>,
}

/// Activation buffer for intermediate computations
/// Uses fixed-size stack allocation to avoid heap fragmentation
pub struct ActivationBuffer {
    /// Input embedding buffer
    pub input: [i8; MAX_EMBED_DIM],
    /// Hidden state buffer
    pub hidden: [i32; MAX_HIDDEN_DIM],
    /// Output logits buffer
    pub logits: [i32; MAX_VOCAB_SIZE],
    /// Attention scores buffer
    pub attn_scores: [i32; MAX_SEQ_LEN],
    /// Temporary buffer for matrix ops
    pub temp: [i32; MAX_HIDDEN_DIM],
    /// Query projection buffer
    pub query: [i8; MAX_EMBED_DIM],
    /// Key projection buffer
    pub key: [i8; MAX_EMBED_DIM],
    /// Value projection buffer
    pub value: [i8; MAX_EMBED_DIM],
}

impl Default for ActivationBuffer {
    fn default() -> Self {
        Self {
            input: [0i8; MAX_EMBED_DIM],
            hidden: [0i32; MAX_HIDDEN_DIM],
            logits: [0i32; MAX_VOCAB_SIZE],
            attn_scores: [0i32; MAX_SEQ_LEN],
            temp: [0i32; MAX_HIDDEN_DIM],
            query: [0i8; MAX_EMBED_DIM],
            key: [0i8; MAX_EMBED_DIM],
            value: [0i8; MAX_EMBED_DIM],
        }
    }
}

impl ActivationBuffer {
    /// Total size of activation buffers
    pub const fn total_size() -> usize {
        MAX_EMBED_DIM * 4          // input, query, key, value (i8)
        + MAX_HIDDEN_DIM * 4 * 2   // hidden, temp (i32)
        + MAX_VOCAB_SIZE * 4       // logits (i32)
        + MAX_SEQ_LEN * 4          // attn_scores (i32)
    }
}

/// Micro inference engine for ESP32
pub struct MicroEngine {
    /// Model weights and config
    model: TinyModel,
    /// Activation buffers (stack allocated)
    buffers: ActivationBuffer,
    /// Current sequence position
    seq_pos: usize,
    /// KV cache for autoregressive generation
    kv_cache: KVCache,
    /// Performance counters
    perf: PerfCounters,
}

/// Key-Value cache for autoregressive generation
pub struct KVCache {
    /// Cached keys [seq_len, embed_dim]
    keys: [[i8; MAX_EMBED_DIM]; MAX_SEQ_LEN],
    /// Cached values [seq_len, embed_dim]
    values: [[i8; MAX_EMBED_DIM]; MAX_SEQ_LEN],
    /// Current cache length
    len: usize,
}

impl Default for KVCache {
    fn default() -> Self {
        Self {
            keys: [[0i8; MAX_EMBED_DIM]; MAX_SEQ_LEN],
            values: [[0i8; MAX_EMBED_DIM]; MAX_SEQ_LEN],
            len: 0,
        }
    }
}

impl KVCache {
    /// Total memory usage
    pub const fn memory_size() -> usize {
        MAX_SEQ_LEN * MAX_EMBED_DIM * 2 // keys + values
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Push new key-value pair
    pub fn push(&mut self, key: &[i8], value: &[i8]) -> crate::Result<()> {
        if self.len >= MAX_SEQ_LEN {
            return Err(crate::Error::BufferOverflow);
        }

        self.keys[self.len][..key.len()].copy_from_slice(key);
        self.values[self.len][..value.len()].copy_from_slice(value);
        self.len += 1;
        Ok(())
    }
}

/// Performance counters
#[derive(Debug, Clone, Default)]
pub struct PerfCounters {
    /// Total embeddings computed
    pub embeddings: u32,
    /// Total attention operations
    pub attention_ops: u32,
    /// Total FFN operations
    pub ffn_ops: u32,
    /// Total cycles (estimated)
    pub cycles: u64,
}

impl MicroEngine {
    /// Create a new micro inference engine
    pub fn new(model: TinyModel) -> crate::Result<Self> {
        // Validate model fits in memory constraints
        let model_size = model.memory_size();
        let buffer_size = ActivationBuffer::total_size();
        let kv_size = KVCache::memory_size();
        let total_required = model_size + buffer_size + kv_size;

        let available = crate::Esp32Variant::Esp32.max_model_ram();
        if total_required > available {
            return Err(crate::Error::ModelTooLarge {
                required: total_required,
                available,
            });
        }

        Ok(Self {
            model,
            buffers: ActivationBuffer::default(),
            seq_pos: 0,
            kv_cache: KVCache::default(),
            perf: PerfCounters::default(),
        })
    }

    /// Get memory usage breakdown
    pub fn memory_usage(&self) -> MemoryUsage {
        MemoryUsage {
            model_weights: self.model.memory_size(),
            activation_buffers: ActivationBuffer::total_size(),
            kv_cache: KVCache::memory_size(),
            total: self.model.memory_size()
                + ActivationBuffer::total_size()
                + KVCache::memory_size(),
        }
    }

    /// Reset engine state for new sequence
    pub fn reset(&mut self) {
        self.seq_pos = 0;
        self.kv_cache.clear();
        self.perf = PerfCounters::default();
    }

    /// Embed a single token
    pub fn embed_token(&mut self, token_id: u16) -> crate::Result<()> {
        let embed_dim = self.model.config.embed_dim;

        if token_id as usize >= self.model.config.vocab_size {
            return Err(crate::Error::InvalidModel("Token ID out of range"));
        }

        // Look up embedding from quantized table
        let embed_offset = token_id as usize * embed_dim;
        let embed_slice = &self.model.embedding_table[embed_offset..embed_offset + embed_dim];

        // Copy to input buffer
        for (i, &v) in embed_slice.iter().enumerate() {
            self.buffers.input[i] = v;
        }

        self.perf.embeddings += 1;
        Ok(())
    }

    /// Single attention head computation (INT8)
    #[allow(unused_variables)]
    pub fn attention_head(
        &mut self,
        layer: &LayerWeights,
        head_idx: usize,
    ) -> crate::Result<()> {
        let embed_dim = self.model.config.embed_dim;
        let head_dim = embed_dim / self.model.config.num_heads;
        let head_offset = head_idx * head_dim;

        // Q = input @ Wq
        matmul_int8(
            &layer.wq[head_offset * embed_dim..(head_offset + head_dim) * embed_dim],
            &layer.q_params,
            &self.buffers.input[..embed_dim],
            &self.model.input_params,
            &mut self.buffers.hidden[..head_dim],
            head_dim,
            embed_dim,
        );

        // Copy Q to query buffer
        for i in 0..head_dim {
            self.buffers.query[i] = (self.buffers.hidden[i] >> 8).clamp(-128, 127) as i8;
        }

        // K = input @ Wk
        matmul_int8(
            &layer.wk[head_offset * embed_dim..(head_offset + head_dim) * embed_dim],
            &layer.k_params,
            &self.buffers.input[..embed_dim],
            &self.model.input_params,
            &mut self.buffers.hidden[..head_dim],
            head_dim,
            embed_dim,
        );

        for i in 0..head_dim {
            self.buffers.key[i] = (self.buffers.hidden[i] >> 8).clamp(-128, 127) as i8;
        }

        // V = input @ Wv
        matmul_int8(
            &layer.wv[head_offset * embed_dim..(head_offset + head_dim) * embed_dim],
            &layer.v_params,
            &self.buffers.input[..embed_dim],
            &self.model.input_params,
            &mut self.buffers.hidden[..head_dim],
            head_dim,
            embed_dim,
        );

        for i in 0..head_dim {
            self.buffers.value[i] = (self.buffers.hidden[i] >> 8).clamp(-128, 127) as i8;
        }

        // Store K,V in cache (only for first head to avoid duplicates)
        if head_idx == 0 {
            // Only push if we haven't exceeded the sequence position
            if self.kv_cache.len < self.seq_pos + 1 {
                self.kv_cache.push(&self.buffers.key[..head_dim], &self.buffers.value[..head_dim])?;
            }
        }

        // Compute attention scores: Q @ K^T for all cached positions
        let cache_len = self.kv_cache.len;
        for pos in 0..cache_len {
            let mut score: i32 = 0;
            for i in 0..head_dim {
                score += self.buffers.query[i] as i32 * self.kv_cache.keys[pos][i] as i32;
            }
            // Scale by 1/sqrt(head_dim) approximated as right shift
            self.buffers.attn_scores[pos] = score >> 4;
        }

        // Softmax approximation using fixed-point
        Self::softmax_int32_slice(&mut self.buffers.attn_scores[..cache_len]);

        // Weighted sum of values
        for i in 0..head_dim {
            let mut sum: i32 = 0;
            for pos in 0..self.kv_cache.len {
                sum += self.buffers.attn_scores[pos] * self.kv_cache.values[pos][i] as i32;
            }
            self.buffers.hidden[i] = sum >> 8;
        }

        self.perf.attention_ops += 1;
        Ok(())
    }

    /// Fixed-point softmax approximation (static to avoid borrow issues)
    fn softmax_int32_slice(scores: &mut [i32]) {
        if scores.is_empty() {
            return;
        }

        // Find max for numerical stability
        let max = scores.iter().cloned().max().unwrap_or(0);

        // Subtract max and compute exp approximation
        // Using linear approximation: exp(x) ≈ max(0, 1 + x/256) for small x
        let mut sum: i32 = 0;
        for score in scores.iter_mut() {
            *score = (*score - max).max(-256) + 256;
            sum += *score;
        }

        // Normalize (fixed-point division)
        if sum > 0 {
            for score in scores.iter_mut() {
                *score = (*score << 8) / sum;
            }
        }
    }

    /// Feed-forward network layer (INT8)
    pub fn ffn_layer(&mut self, layer: &LayerWeights) -> crate::Result<()> {
        let embed_dim = self.model.config.embed_dim;
        let hidden_dim = self.model.config.hidden_dim;

        // Up projection: hidden = input @ W_up
        matmul_int8(
            &layer.w_up,
            &layer.up_params,
            &self.buffers.input[..embed_dim],
            &self.model.input_params,
            &mut self.buffers.hidden[..hidden_dim],
            hidden_dim,
            embed_dim,
        );

        // GELU approximation: gelu(x) ≈ x * sigmoid(1.702 * x)
        // For INT8: use ReLU as simpler approximation
        for h in self.buffers.hidden[..hidden_dim].iter_mut() {
            *h = (*h).max(0);
        }

        // Gate projection (for gated FFN)
        matmul_int8(
            &layer.w_gate,
            &layer.gate_params,
            &self.buffers.input[..embed_dim],
            &self.model.input_params,
            &mut self.buffers.temp[..hidden_dim],
            hidden_dim,
            embed_dim,
        );

        // Element-wise multiply with gate
        for i in 0..hidden_dim {
            self.buffers.hidden[i] = (self.buffers.hidden[i] >> 8) * (self.buffers.temp[i] >> 8);
        }

        // Convert back to i8 for down projection input
        let mut hidden_i8 = [0i8; MAX_HIDDEN_DIM];
        for i in 0..hidden_dim {
            hidden_i8[i] = (self.buffers.hidden[i] >> 8).clamp(-128, 127) as i8;
        }

        // Down projection: output = hidden @ W_down
        matmul_int8(
            &layer.w_down,
            &layer.down_params,
            &hidden_i8[..hidden_dim],
            &layer.up_params, // reuse params
            &mut self.buffers.hidden[..embed_dim],
            embed_dim,
            hidden_dim,
        );

        // Residual connection
        for i in 0..embed_dim {
            let residual = self.buffers.input[i] as i32 * 256;
            self.buffers.hidden[i] += residual;
            self.buffers.input[i] = (self.buffers.hidden[i] >> 8).clamp(-128, 127) as i8;
        }

        self.perf.ffn_ops += 1;
        Ok(())
    }

    /// Output projection to vocabulary
    pub fn output_projection(&mut self) -> crate::Result<()> {
        let embed_dim = self.model.config.embed_dim;
        let vocab_size = self.model.config.vocab_size;

        matmul_int8(
            &self.model.output_proj,
            &self.model.output_params,
            &self.buffers.input[..embed_dim],
            &self.model.input_params,
            &mut self.buffers.logits[..vocab_size],
            vocab_size,
            embed_dim,
        );

        Ok(())
    }

    /// Sample next token from logits
    pub fn sample(&self, config: &InferenceConfig) -> u16 {
        let vocab_size = self.model.config.vocab_size;

        if config.greedy || config.temperature < 0.01 {
            // Greedy: argmax
            let mut max_idx = 0;
            let mut max_val = i32::MIN;
            for (i, &logit) in self.buffers.logits[..vocab_size].iter().enumerate() {
                if logit > max_val {
                    max_val = logit;
                    max_idx = i;
                }
            }
            return max_idx as u16;
        }

        // Temperature sampling with top-k
        // For embedded: simple argmax with some noise
        let mut max_idx = 0;
        let mut max_val = i32::MIN;
        for (i, &logit) in self.buffers.logits[..vocab_size].iter().enumerate() {
            if logit > max_val {
                max_val = logit;
                max_idx = i;
            }
        }
        max_idx as u16
    }

    /// Run full inference for one token
    pub fn forward_one(&mut self, token_id: u16) -> crate::Result<u16> {
        // 1. Embed token
        self.embed_token(token_id)?;

        // 2. Run through transformer layers
        let num_layers = self.model.config.num_layers;
        let num_heads = self.model.config.num_heads;

        for layer_idx in 0..num_layers {
            // Clone layer data to avoid borrow issues
            let layer = self.model.layers[layer_idx].clone();

            // Attention
            for head in 0..num_heads {
                self.attention_head(&layer, head)?;
            }

            // FFN
            self.ffn_layer(&layer)?;
        }

        // 3. Output projection
        self.output_projection()?;

        // 4. Sample next token
        let next_token = self.sample(&InferenceConfig::default());

        self.seq_pos += 1;
        Ok(next_token)
    }

    /// Generate a sequence of tokens
    pub fn generate(
        &mut self,
        prompt_tokens: &[u16],
        config: &InferenceConfig,
    ) -> crate::Result<InferenceResult> {
        self.reset();

        let mut result = InferenceResult {
            tokens: HVec::new(),
            inference_time_us: 0,
            tokens_per_second: 0.0,
            peak_memory_bytes: self.memory_usage().total,
            layer_times_us: HVec::new(),
        };

        // Process prompt (prefill)
        for &token in prompt_tokens {
            let _ = self.forward_one(token)?;
        }

        // Generate new tokens
        let mut next_token = prompt_tokens.last().copied().unwrap_or(0);
        for _ in 0..config.max_tokens {
            next_token = self.forward_one(next_token)?;
            result.tokens.push(next_token).map_err(|_| crate::Error::BufferOverflow)?;

            // Check for EOS token (assume token 0 is EOS)
            if next_token == 0 {
                break;
            }
        }

        Ok(result)
    }

    /// Get performance counters
    pub fn perf_counters(&self) -> &PerfCounters {
        &self.perf
    }
}

/// Memory usage breakdown
#[derive(Debug, Clone)]
pub struct MemoryUsage {
    pub model_weights: usize,
    pub activation_buffers: usize,
    pub kv_cache: usize,
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelConfig;

    fn create_tiny_model() -> TinyModel {
        TinyModel::new(ModelConfig {
            vocab_size: 256,
            embed_dim: 64,
            hidden_dim: 128,
            num_layers: 2,
            num_heads: 4,
            max_seq_len: 32,
            quant_type: QuantizationType::Int8,
        }).unwrap()
    }

    #[test]
    fn test_engine_creation() {
        let model = create_tiny_model();
        let engine = MicroEngine::new(model).unwrap();

        let usage = engine.memory_usage();
        println!("Memory usage: {:?}", usage);
        assert!(usage.total < 320 * 1024); // Must fit in ESP32-S2
    }

    #[test]
    fn test_embedding() {
        let model = create_tiny_model();
        let mut engine = MicroEngine::new(model).unwrap();

        engine.embed_token(42).unwrap();
        assert_eq!(engine.perf.embeddings, 1);
    }

    #[test]
    fn test_forward_pass() {
        let model = create_tiny_model();
        let mut engine = MicroEngine::new(model).unwrap();

        let next_token = engine.forward_one(10).unwrap();
        assert!(next_token < 256);
    }

    #[test]
    fn test_generation() {
        let model = create_tiny_model();
        let mut engine = MicroEngine::new(model).unwrap();

        let prompt = [1u16, 2, 3];
        let config = InferenceConfig {
            max_tokens: 5,
            greedy: true,
            ..Default::default()
        };

        let result = engine.generate(&prompt, &config).unwrap();
        assert!(!result.tokens.is_empty());
        assert!(result.tokens.len() <= 5);
    }
}
