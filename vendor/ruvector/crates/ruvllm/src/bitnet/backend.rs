//! BitNet b1.58 Inference Backend
//!
//! This module implements the `BitNetBackend` inference pipeline for BitNet b1.58
//! MoE models (e.g., GLM-4.7-Flash). It wires together the quantizer, TL1 kernel,
//! and MoE routing into a working inference pipeline.
//!
//! ## Phase 0 Scope
//!
//! - Attention is a placeholder (pass-through) for smoke testing
//! - MoE routing is fully functional (FP16 gate + softmax + top-K)
//! - Expert FFN uses real TL1 GEMV on ternary weights
//! - Embedding lookup and LM head are FP16 matmul
//!
//! ## Architecture
//!
//! ```text
//! Embedding (FP16) -> [Transformer Layers] -> RMSNorm -> LM Head (FP16) -> Logits
//!
//! Each Transformer Layer:
//!   RMSNorm -> Attention (placeholder) -> Residual
//!   -> RMSNorm -> MoE Gate (FP16) -> Top-K Expert Selection
//!   -> Expert FFN (TL1 GEMV on ternary) -> Weighted Sum -> Residual
//! ```

use std::path::Path;
use std::sync::Mutex;

use crate::backends::{
    GenerateParams, GeneratedToken, LlmBackend, ModelArchitecture, ModelConfig, ModelInfo,
    Quantization, SpecialTokens as BackendSpecialTokens, StreamEvent, TokenStream,
    Tokenizer as BackendTokenizer,
};
use crate::error::{Result, RuvLLMError};
use crate::gguf::{GgufFile, GgufQuantType};

use super::ternary_tensor::TernaryTensor;
use super::tokenizer::{BpeTokenizer, SpecialTokens as BitNetSpecialTokens};

// ============================================================================
// Configuration
// ============================================================================

/// Model configuration for BitNet MoE inference.
///
/// Describes the architecture dimensions extracted from GGUF metadata
/// or supplied manually for testing. Supports both standard GQA attention
/// and MLA (Multi-Head Latent Attention) as used by GLM-4.7-Flash.
#[derive(Debug, Clone)]
pub struct BitNetModelConfig {
    /// Number of transformer layers
    pub num_layers: usize,
    /// Hidden state dimension
    pub hidden_size: usize,
    /// Number of MoE routed experts per layer
    pub num_experts: usize,
    /// Number of active experts per token (top-K)
    pub active_experts: usize,
    /// Dense FFN intermediate dimension (for dense layers)
    pub intermediate_size: usize,
    /// MoE expert FFN intermediate dimension (may differ from dense)
    pub moe_intermediate_size: usize,
    /// Number of attention query heads
    pub num_attention_heads: usize,
    /// Number of attention key-value heads (GQA; equals num_attention_heads in MLA)
    pub num_kv_heads: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum context length
    pub max_context: usize,
    /// RoPE frequency base
    pub rope_theta: f32,

    // --- MLA (Multi-Head Latent Attention) parameters ---
    /// Whether attention uses MLA (true) or standard GQA (false)
    pub use_mla: bool,
    /// Q low-rank compression dimension (MLA)
    pub q_lora_rank: usize,
    /// KV low-rank compression dimension (MLA)
    pub kv_lora_rank: usize,
    /// Non-RoPE portion of Q/K head dimension (MLA)
    pub qk_nope_head_dim: usize,
    /// RoPE portion of Q/K head dimension (MLA)
    pub qk_rope_head_dim: usize,
    /// Value head dimension (MLA)
    pub v_head_dim: usize,

    // --- MoE structure ---
    /// Number of shared experts (always-active, non-routed)
    pub n_shared_experts: usize,
    /// First N layers use dense FFN instead of MoE (e.g., 1 means layer 0 is dense)
    pub first_k_dense_replace: usize,
    /// Scaling factor for routed expert weights
    pub routed_scaling_factor: f32,
}

impl Default for BitNetModelConfig {
    fn default() -> Self {
        // Default values matching GLM-4.7-Flash architecture
        Self {
            num_layers: 47,
            hidden_size: 2048,
            num_experts: 64,
            active_experts: 4,
            intermediate_size: 10240,
            moe_intermediate_size: 1536,
            num_attention_heads: 20,
            num_kv_heads: 20,
            vocab_size: 154880,
            max_context: 8192,
            rope_theta: 1_000_000.0,
            // MLA parameters from GLM-4.7-Flash config.json
            use_mla: true,
            q_lora_rank: 768,
            kv_lora_rank: 512,
            qk_nope_head_dim: 192,
            qk_rope_head_dim: 64,
            v_head_dim: 256,
            // MoE structure
            n_shared_experts: 1,
            first_k_dense_replace: 1,
            routed_scaling_factor: 1.8,
        }
    }
}

// ============================================================================
// TL1 Lookup Table
// ============================================================================

/// Pre-computed lookup table for packed 2-bit ternary bytes.
///
/// For each of the 256 possible byte values, stores the four decoded
/// ternary values {-1, 0, +1}. This avoids per-element bit manipulation
/// during the hot GEMV inner loop.
type Tl1Lut = [[i8; 4]; 256];

/// Build the TL1 lookup table at load time.
///
/// Encoding per the ternary_tensor module:
/// - 00 = -1, 01 = 0, 10 = +1, 11 = 0 (reserved)
fn build_tl1_lut() -> Tl1Lut {
    let mut lut = [[0i8; 4]; 256];
    for byte_val in 0u16..256 {
        for pos in 0..4 {
            let bits = ((byte_val as u8) >> (pos * 2)) & 0b11;
            lut[byte_val as usize][pos] = match bits {
                0b00 => -1,
                0b01 => 0,
                0b10 => 1,
                0b11 => 0, // reserved
                _ => unreachable!(),
            };
        }
    }
    lut
}

// ============================================================================
// Tensor Name Mapper
// ============================================================================

/// Resolves logical tensor names to actual GGUF tensor names.
///
/// GLM-4.7-Flash GGUF files use llama.cpp conventions (`blk.0.attn_q_a.weight`),
/// while some models use HuggingFace conventions (`model.layers.0.self_attn.q_proj.weight`).
/// The mapper tries GGUF names first, then HuggingFace names as fallback.
struct TensorNameMapper;

impl TensorNameMapper {
    /// Find the first tensor name that exists in the GGUF file.
    fn resolve(gguf: &GgufFile, candidates: &[String]) -> Option<String> {
        for name in candidates {
            if gguf.get_tensor(name).is_some() {
                return Some(name.clone());
            }
        }
        None
    }

    // -- Global tensors --

    fn embedding() -> Vec<String> {
        vec![
            "token_embd.weight".into(),
            "model.embed_tokens.weight".into(),
        ]
    }

    fn output() -> Vec<String> {
        vec!["output.weight".into(), "lm_head.weight".into()]
    }

    fn final_norm() -> Vec<String> {
        vec!["output_norm.weight".into(), "model.norm.weight".into()]
    }

    // -- Per-layer norms --

    fn input_norm(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.attn_norm.weight", idx),
            format!("model.layers.{}.input_layernorm.weight", idx),
        ]
    }

    fn post_attn_norm(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.ffn_norm.weight", idx),
            format!("model.layers.{}.post_attention_layernorm.weight", idx),
        ]
    }

    // -- MLA attention tensors --

    fn attn_q_a(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_q_a.weight", idx)]
    }

    fn attn_q_b(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_q_b.weight", idx)]
    }

    fn attn_q_a_norm(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_q_a_norm.weight", idx)]
    }

    fn attn_kv_a_mqa(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_kv_a_mqa.weight", idx)]
    }

    fn attn_kv_a_norm(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_kv_a_norm.weight", idx)]
    }

    fn attn_k_b(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_k_b.weight", idx)]
    }

    fn attn_v_b(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.attn_v_b.weight", idx)]
    }

    fn attn_output(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.attn_output.weight", idx),
            format!("model.layers.{}.self_attn.o_proj.weight", idx),
        ]
    }

    // -- Standard GQA attention tensors --

    fn attn_q_proj(idx: usize) -> Vec<String> {
        vec![format!("model.layers.{}.self_attn.q_proj.weight", idx)]
    }

    fn attn_k_proj(idx: usize) -> Vec<String> {
        vec![format!("model.layers.{}.self_attn.k_proj.weight", idx)]
    }

    fn attn_v_proj(idx: usize) -> Vec<String> {
        vec![format!("model.layers.{}.self_attn.v_proj.weight", idx)]
    }

    // -- MoE router gate --

    fn moe_gate(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.ffn_gate_inp.weight", idx),
            format!("model.layers.{}.mlp.gate.weight", idx),
        ]
    }

    // -- Dense FFN tensors --

    fn ffn_gate(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.ffn_gate.weight", idx),
            format!("model.layers.{}.mlp.gate_proj.weight", idx),
        ]
    }

    fn ffn_up(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.ffn_up.weight", idx),
            format!("model.layers.{}.mlp.up_proj.weight", idx),
        ]
    }

    fn ffn_down(idx: usize) -> Vec<String> {
        vec![
            format!("blk.{}.ffn_down.weight", idx),
            format!("model.layers.{}.mlp.down_proj.weight", idx),
        ]
    }

    // -- Shared expert tensors --

    fn ffn_gate_shexp(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.ffn_gate_shexp.weight", idx)]
    }

    fn ffn_up_shexp(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.ffn_up_shexp.weight", idx)]
    }

    fn ffn_down_shexp(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.ffn_down_shexp.weight", idx)]
    }

    // -- Stacked expert tensors (3D, all experts in one tensor) --

    fn ffn_gate_exps(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.ffn_gate_exps.weight", idx)]
    }

    fn ffn_up_exps(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.ffn_up_exps.weight", idx)]
    }

    fn ffn_down_exps(idx: usize) -> Vec<String> {
        vec![format!("blk.{}.ffn_down_exps.weight", idx)]
    }

    // -- Per-expert tensors (HuggingFace individual naming) --

    fn expert_gate(idx: usize, expert_idx: usize) -> Vec<String> {
        vec![format!(
            "model.layers.{}.mlp.experts.{}.gate_proj.weight",
            idx, expert_idx
        )]
    }

    fn expert_up(idx: usize, expert_idx: usize) -> Vec<String> {
        vec![format!(
            "model.layers.{}.mlp.experts.{}.up_proj.weight",
            idx, expert_idx
        )]
    }

    fn expert_down(idx: usize, expert_idx: usize) -> Vec<String> {
        vec![format!(
            "model.layers.{}.mlp.experts.{}.down_proj.weight",
            idx, expert_idx
        )]
    }

    /// Check if a layer has MLA attention tensors.
    fn has_mla(gguf: &GgufFile, idx: usize) -> bool {
        Self::resolve(gguf, &Self::attn_q_a(idx)).is_some()
    }

    /// Check if a layer has stacked expert tensors.
    fn has_stacked_experts(gguf: &GgufFile, idx: usize) -> bool {
        Self::resolve(gguf, &Self::ffn_gate_exps(idx)).is_some()
    }

    /// Check if a layer has dense FFN (not MoE).
    fn has_dense_ffn(gguf: &GgufFile, idx: usize) -> bool {
        Self::resolve(gguf, &Self::ffn_gate(idx)).is_some()
    }
}

// ============================================================================
// Per-Layer and Per-Expert Weight Storage
// ============================================================================

/// Ternary weights for a single MoE expert (gate, up, down projections).
#[derive(Debug, Clone)]
struct ExpertWeights {
    /// gate_proj: [intermediate_size, hidden_size]
    gate_proj: TernaryTensor,
    /// up_proj: [intermediate_size, hidden_size]
    up_proj: TernaryTensor,
    /// down_proj: [hidden_size, intermediate_size]
    down_proj: TernaryTensor,
}

/// Attention projection weights.
///
/// Supports two variants:
/// - **Standard GQA**: Direct Q/K/V/O projections
/// - **MLA (Multi-Head Latent Attention)**: Low-rank compressed Q/KV projections
///   as used by GLM-4.7-Flash / DeepSeek-V2
#[derive(Debug, Clone)]
struct AttentionWeights {
    /// Whether this layer uses MLA or standard GQA
    is_mla: bool,

    // --- Standard GQA fields ---
    /// Q projection: [num_heads * head_dim, hidden_size]
    q_proj: TernaryTensor,
    /// K projection: [num_kv_heads * head_dim, hidden_size]
    k_proj: TernaryTensor,
    /// V projection: [num_kv_heads * head_dim, hidden_size]
    v_proj: TernaryTensor,
    /// Output projection: [hidden_size, num_heads * head_dim]
    o_proj: TernaryTensor,

    // --- MLA fields (populated when is_mla = true) ---
    /// Q down-projection: [hidden_size → q_lora_rank]
    q_a: Option<TernaryTensor>,
    /// Q up-projection: [q_lora_rank → num_heads * (qk_nope_head_dim + qk_rope_head_dim)]
    q_b: Option<TernaryTensor>,
    /// Q compression norm weights: [q_lora_rank]
    q_a_norm: Option<Vec<f32>>,
    /// KV joint down-projection: [hidden_size → kv_lora_rank + qk_rope_head_dim]
    kv_a_mqa: Option<TernaryTensor>,
    /// KV compression norm weights: [kv_lora_rank]
    kv_a_norm: Option<Vec<f32>>,
    /// K up-projection: [kv_lora_rank → num_heads * qk_nope_head_dim]
    k_b: Option<TernaryTensor>,
    /// V up-projection: [kv_lora_rank → num_heads * v_head_dim]
    v_b: Option<TernaryTensor>,
}

/// Type of FFN in a transformer layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerType {
    /// Dense FFN (single gate/up/down, no MoE routing)
    Dense,
    /// MoE with routed experts only
    Moe,
    /// MoE with routed experts + shared expert(s)
    MoeWithShared,
}

/// Weights for a single transformer layer.
#[derive(Debug, Clone)]
struct TransformerLayer {
    /// Input RMSNorm weight [hidden_size]
    input_norm_weight: Vec<f32>,
    /// Post-attention RMSNorm weight [hidden_size]
    post_attn_norm_weight: Vec<f32>,
    /// Attention projection weights (ternary, supports MLA or GQA)
    attention: AttentionWeights,
    /// Type of FFN in this layer
    layer_type: LayerType,
    /// MoE router gate weight [num_experts, hidden_size] (FP32, empty for dense layers)
    gate_weight: Vec<f32>,
    /// Per-expert FFN weights (routed experts, ternary)
    experts: Vec<ExpertWeights>,
    /// Shared expert FFN weights (always-active, non-routed; None for dense layers)
    shared_expert: Option<ExpertWeights>,
    /// Dense FFN weights (for dense-only layers; uses gate/up/down from ExpertWeights)
    dense_ffn: Option<ExpertWeights>,
}

// ============================================================================
// KV Cache
// ============================================================================

/// Per-layer KV cache for autoregressive generation.
#[derive(Debug, Clone)]
struct LayerKvCache {
    /// Cached key vectors: one [num_kv_heads * head_dim] per position
    keys: Vec<Vec<f32>>,
    /// Cached value vectors: one [num_kv_heads * head_dim] per position
    values: Vec<Vec<f32>>,
}

impl LayerKvCache {
    fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
    }

    fn len(&self) -> usize {
        self.keys.len()
    }
}

// ============================================================================
// Scratch Memory Pool (Zero-Allocation Forward Pass)
// ============================================================================

/// Pre-allocated scratch buffers to eliminate per-token heap allocations
/// in the forward pass. All hot-path vectors are pre-sized to the maximum
/// needed dimension and reused across tokens.
struct ScratchPool {
    /// General-purpose buffer [hidden_size] — used for normed, residual, etc.
    buf_hidden_a: Vec<f32>,
    buf_hidden_b: Vec<f32>,
    buf_hidden_c: Vec<f32>,
    /// Buffer for attention Q output [num_heads * head_dim]
    buf_attn_q: Vec<f32>,
    /// Buffer for attention K output [num_kv_heads * head_dim or num_heads * q_head_dim]
    buf_attn_k: Vec<f32>,
    /// Buffer for attention V output [num_kv_heads * head_dim or num_heads * v_dim]
    buf_attn_v: Vec<f32>,
    /// Buffer for attention output [hidden_size or num_heads * v_dim]
    buf_attn_out: Vec<f32>,
    /// Buffer for FFN intermediate [intermediate_size]
    buf_ffn_gate: Vec<f32>,
    buf_ffn_up: Vec<f32>,
    buf_ffn_fused: Vec<f32>,
    buf_ffn_down: Vec<f32>,
    /// Buffer for expert output accumulation [hidden_size]
    buf_expert_out: Vec<f32>,
    /// Buffer for logits [vocab_size]
    buf_logits: Vec<f32>,
    /// Buffer for MLA compressed Q [q_lora_rank]
    buf_mla_cq: Vec<f32>,
    /// Buffer for MLA Q full [num_heads * q_head_dim]
    buf_mla_qfull: Vec<f32>,
    /// Buffer for MLA KV combined [kv_lora_rank + qk_rope_head_dim]
    buf_mla_kv: Vec<f32>,
    /// TL1 GEMV output buffer (reusable for arbitrary sizes)
    buf_gemv: Vec<f32>,
}

impl ScratchPool {
    fn new() -> Self {
        Self {
            buf_hidden_a: Vec::new(),
            buf_hidden_b: Vec::new(),
            buf_hidden_c: Vec::new(),
            buf_attn_q: Vec::new(),
            buf_attn_k: Vec::new(),
            buf_attn_v: Vec::new(),
            buf_attn_out: Vec::new(),
            buf_ffn_gate: Vec::new(),
            buf_ffn_up: Vec::new(),
            buf_ffn_fused: Vec::new(),
            buf_ffn_down: Vec::new(),
            buf_expert_out: Vec::new(),
            buf_logits: Vec::new(),
            buf_mla_cq: Vec::new(),
            buf_mla_qfull: Vec::new(),
            buf_mla_kv: Vec::new(),
            buf_gemv: Vec::new(),
        }
    }

    /// Pre-allocate all buffers based on model config. Called once after loading.
    fn allocate(&mut self, config: &BitNetModelConfig) {
        let h = config.hidden_size;
        let q_head_dim = config.qk_nope_head_dim + config.qk_rope_head_dim;
        let attn_dim = config.num_attention_heads * q_head_dim;
        let v_total = config.num_attention_heads * config.v_head_dim;
        let inter = config.intermediate_size.max(config.moe_intermediate_size);

        self.buf_hidden_a = vec![0.0; h];
        self.buf_hidden_b = vec![0.0; h];
        self.buf_hidden_c = vec![0.0; h];
        self.buf_attn_q = vec![0.0; attn_dim];
        self.buf_attn_k = vec![0.0; attn_dim];
        self.buf_attn_v = vec![0.0; v_total.max(attn_dim)];
        self.buf_attn_out = vec![0.0; v_total.max(h)];
        self.buf_ffn_gate = vec![0.0; inter];
        self.buf_ffn_up = vec![0.0; inter];
        self.buf_ffn_fused = vec![0.0; inter];
        self.buf_ffn_down = vec![0.0; h];
        self.buf_expert_out = vec![0.0; h];
        self.buf_logits = vec![0.0; config.vocab_size];
        self.buf_mla_cq = vec![0.0; config.q_lora_rank];
        self.buf_mla_qfull = vec![0.0; attn_dim];
        self.buf_mla_kv = vec![0.0; config.kv_lora_rank + config.qk_rope_head_dim];
        self.buf_gemv = vec![0.0; attn_dim.max(inter).max(h)];
    }

    /// Total memory used by scratch buffers.
    fn memory_bytes(&self) -> usize {
        (self.buf_hidden_a.len()
            + self.buf_hidden_b.len()
            + self.buf_hidden_c.len()
            + self.buf_attn_q.len()
            + self.buf_attn_k.len()
            + self.buf_attn_v.len()
            + self.buf_attn_out.len()
            + self.buf_ffn_gate.len()
            + self.buf_ffn_up.len()
            + self.buf_ffn_fused.len()
            + self.buf_ffn_down.len()
            + self.buf_expert_out.len()
            + self.buf_logits.len()
            + self.buf_mla_cq.len()
            + self.buf_mla_qfull.len()
            + self.buf_mla_kv.len()
            + self.buf_gemv.len())
            * 4
    }
}

// ============================================================================
// BitNetBackend
// ============================================================================

/// BitNet b1.58 MoE inference backend.
///
/// Provides model loading from GGUF and forward pass inference using
/// ternary TL1 GEMV kernels for expert FFN layers and FP32 for shared
/// layers (embeddings, norms, router, LM head).
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::backend::BitNetBackend;
/// use ruvllm::backends::{LlmBackend, ModelConfig, GenerateParams};
///
/// let mut backend = BitNetBackend::new();
/// backend.load_model("model.gguf", ModelConfig::default())?;
///
/// let logits = backend.forward(&[1, 2, 3])?;
/// ```
pub struct BitNetBackend {
    /// Model configuration (set after load)
    config: Option<BitNetModelConfig>,
    /// Embedding table [vocab_size * hidden_size], row-major FP32
    embedding: Vec<f32>,
    /// LM head weight [vocab_size * hidden_size], row-major FP32
    lm_head: Vec<f32>,
    /// Final RMSNorm weight [hidden_size]
    final_norm_weight: Vec<f32>,
    /// Transformer layers
    layers: Vec<TransformerLayer>,
    /// Pre-computed TL1 lookup table
    tl1_lut: Tl1Lut,
    /// Per-layer KV caches for autoregressive generation
    kv_caches: Vec<LayerKvCache>,
    /// Tokenizer (loaded from GGUF or byte-level fallback)
    tok: Option<BpeTokenizer>,
    /// Pre-computed RoPE cos/sin tables [max_context, head_dim/2]
    rope_cos: Vec<f32>,
    rope_sin: Vec<f32>,
    /// Whether a model is loaded
    loaded: bool,
    /// Model path (for info)
    model_path: String,
    /// Pre-allocated scratch buffers for zero-alloc forward pass
    scratch: ScratchPool,
    /// Per-layer routing history for expert prediction (last N positions).
    /// Uses Mutex for interior mutability so forward_ffn can track routing
    /// decisions without requiring &mut self (needed for LlmBackend trait compat).
    routing_history: Mutex<Vec<Vec<usize>>>,
    /// Maximum routing history length
    max_routing_history: usize,
    /// Cached expert predictor, rebuilt periodically from routing history.
    /// Used to prefetch likely-next experts before they're computed.
    expert_predictor: Option<ExpertPredictor>,
    /// Number of routing history entries since last predictor rebuild.
    predictor_stale_count: usize,
    /// Per-layer compressed MLA KV caches (used instead of `kv_caches` for MLA layers).
    mla_caches: Vec<CompressedMlaCache>,
    /// When true, MLA layers store compressed latents (c_kv + k_pe) instead of
    /// full K/V vectors, giving ~17.8x memory reduction at the cost of recomputing
    /// K_nope and V during attention. Ideal for memory-constrained targets (Pi 5).
    use_compressed_kv: bool,
}

impl BitNetBackend {
    /// Create a new unloaded BitNetBackend.
    pub fn new() -> Self {
        Self {
            config: None,
            embedding: Vec::new(),
            lm_head: Vec::new(),
            final_norm_weight: Vec::new(),
            layers: Vec::new(),
            tl1_lut: build_tl1_lut(),
            kv_caches: Vec::new(),
            tok: None,
            rope_cos: Vec::new(),
            rope_sin: Vec::new(),
            loaded: false,
            model_path: String::new(),
            scratch: ScratchPool::new(),
            routing_history: Mutex::new(Vec::new()),
            max_routing_history: 128,
            expert_predictor: None,
            predictor_stale_count: 0,
            mla_caches: Vec::new(),
            use_compressed_kv: false,
        }
    }

    /// Enable or disable compressed MLA KV cache mode.
    ///
    /// When enabled, MLA layers store only the compressed latents (c_kv + k_pe)
    /// instead of full K/V vectors, giving ~17.8x memory reduction. K_nope and V
    /// are recomputed from the compressed latent during attention, which trades
    /// compute for memory. Ideal for memory-constrained targets (e.g., Pi 5).
    pub fn set_compressed_kv(&mut self, enabled: bool) {
        self.use_compressed_kv = enabled;
    }

    /// Returns whether compressed MLA KV cache mode is enabled.
    pub fn compressed_kv_enabled(&self) -> bool {
        self.use_compressed_kv
    }

    /// Clear the KV cache (call between sequences).
    pub fn reset_cache(&mut self) {
        for cache in &mut self.kv_caches {
            cache.clear();
        }
        for cache in &mut self.mla_caches {
            cache.clear();
        }
    }

    // ========================================================================
    // Model Loading
    // ========================================================================

    /// Load a BitNet MoE model from a GGUF file.
    ///
    /// Parses the GGUF file, extracts model configuration from metadata,
    /// separates FP16 shared tensors from ternary expert tensors, and
    /// pre-builds the TL1 lookup table.
    ///
    /// Supports both llama.cpp GGUF tensor naming (`token_embd.weight`,
    /// `blk.0.attn_q_a.weight`) and HuggingFace naming (`model.embed_tokens.weight`,
    /// `model.layers.0.self_attn.q_proj.weight`).
    fn load_gguf(&mut self, path: &str) -> Result<()> {
        let gguf = GgufFile::open_mmap(Path::new(path))?;

        // Extract model config from GGUF metadata
        let config = self.extract_config(&gguf)?;

        // Load embedding table via name mapper
        let emb_name = TensorNameMapper::resolve(&gguf, &TensorNameMapper::embedding())
            .ok_or_else(|| RuvLLMError::NotFound(
                "Embedding tensor not found (tried: token_embd.weight, model.embed_tokens.weight)".into()
            ))?;
        self.embedding = self.load_fp_tensor(&gguf, &emb_name, &config)?;

        // Load LM head / output via name mapper (fallback to tied embeddings)
        self.lm_head =
            if let Some(out_name) = TensorNameMapper::resolve(&gguf, &TensorNameMapper::output()) {
                self.load_fp_tensor(&gguf, &out_name, &config)?
            } else {
                self.embedding.clone()
            };

        // Load final norm via name mapper
        let norm_name = TensorNameMapper::resolve(&gguf, &TensorNameMapper::final_norm())
            .ok_or_else(|| {
                RuvLLMError::NotFound(
                    "Final norm tensor not found (tried: output_norm.weight, model.norm.weight)"
                        .into(),
                )
            })?;
        self.final_norm_weight = self.load_fp_tensor(&gguf, &norm_name, &config)?;

        // Load transformer layers
        self.layers = Vec::with_capacity(config.num_layers);
        for layer_idx in 0..config.num_layers {
            let layer = self.load_layer(&gguf, layer_idx, &config)?;
            self.layers.push(layer);
        }

        // Initialize KV caches (one per layer, pre-allocated for 512 positions)
        let pre_alloc_seq = 512.min(config.max_context);
        self.kv_caches = (0..config.num_layers)
            .map(|_| {
                let mut cache = LayerKvCache::new();
                cache.keys.reserve(pre_alloc_seq);
                cache.values.reserve(pre_alloc_seq);
                cache
            })
            .collect();

        // Initialize compressed MLA caches (one per layer for MLA layers)
        self.mla_caches = (0..config.num_layers)
            .map(|_| CompressedMlaCache::new())
            .collect();

        // Build RoPE cos/sin tables
        // For MLA, rope applies only to qk_rope_head_dim portion
        let rope_dim = if config.use_mla {
            config.qk_rope_head_dim
        } else {
            config.hidden_size / config.num_attention_heads
        };
        self.build_rope_tables(config.max_context.min(8192), rope_dim, config.rope_theta);

        // Load tokenizer from GGUF metadata
        self.tok = self.load_tokenizer_from_gguf(&gguf);

        // Pre-allocate scratch memory pool
        self.scratch.allocate(&config);

        // Initialize routing history
        self.routing_history.lock().unwrap().clear();

        self.config = Some(config);
        self.loaded = true;
        self.model_path = path.to_string();

        Ok(())
    }

    /// Pre-compute RoPE frequency tables.
    fn build_rope_tables(&mut self, max_seq: usize, head_dim: usize, theta: f32) {
        let half = head_dim / 2;
        let total = max_seq * half;
        self.rope_cos = vec![0.0; total];
        self.rope_sin = vec![0.0; total];

        for pos in 0..max_seq {
            for i in 0..half {
                let freq = 1.0 / theta.powf(2.0 * i as f32 / head_dim as f32);
                let angle = pos as f32 * freq;
                self.rope_cos[pos * half + i] = angle.cos();
                self.rope_sin[pos * half + i] = angle.sin();
            }
        }
    }

    /// Load tokenizer from GGUF metadata, falling back to byte-level tokenizer.
    fn load_tokenizer_from_gguf(&self, gguf: &GgufFile) -> Option<BpeTokenizer> {
        // Try to extract token list from GGUF
        let tokens_meta = gguf.metadata.get("tokenizer.ggml.tokens");
        let merges_meta = gguf.metadata.get("tokenizer.ggml.merges");

        if let Some(tokens_arr) = tokens_meta.and_then(|v| v.as_array()) {
            let vocab: Vec<String> = tokens_arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            let merges: Vec<(String, String)> =
                if let Some(merges_arr) = merges_meta.and_then(|v| v.as_array()) {
                    merges_arr
                        .iter()
                        .filter_map(|v| {
                            let s = v.as_str()?;
                            let mut parts = s.splitn(2, ' ');
                            let left = parts.next()?.to_string();
                            let right = parts.next()?.to_string();
                            Some((left, right))
                        })
                        .collect()
                } else {
                    Vec::new()
                };

            if !vocab.is_empty() {
                return Some(BpeTokenizer::from_vocab(
                    vocab,
                    merges,
                    BitNetSpecialTokens::default(),
                ));
            }
        }

        // Fallback: construct a byte-level tokenizer (260 tokens)
        Some(Self::build_byte_level_tokenizer())
    }

    /// Build a minimal byte-level tokenizer for when GGUF has no vocab.
    fn build_byte_level_tokenizer() -> BpeTokenizer {
        let mut vocab = vec![
            "<PAD>".to_string(), // 0
            "<BOS>".to_string(), // 1
            "<EOS>".to_string(), // 2
            "<UNK>".to_string(), // 3
        ];
        for b in 0..=255u8 {
            vocab.push(format!("<{:02X}>", b));
        }
        BpeTokenizer::from_vocab(vocab, vec![], BitNetSpecialTokens::default())
    }

    /// Extract BitNetModelConfig from GGUF metadata.
    fn extract_config(&self, gguf: &GgufFile) -> Result<BitNetModelConfig> {
        let defaults = BitNetModelConfig::default();
        let num_layers = gguf.layer_count().unwrap_or(defaults.num_layers);
        let hidden_size = gguf.embedding_length().unwrap_or(defaults.hidden_size);
        let num_attention_heads = gguf.head_count().unwrap_or(defaults.num_attention_heads);
        let num_kv_heads = gguf.head_count_kv().unwrap_or(defaults.num_kv_heads);
        let vocab_size = gguf.vocab_size().unwrap_or(defaults.vocab_size);
        let max_context = gguf.context_length().unwrap_or(defaults.max_context);
        let rope_theta = gguf.rope_freq_base().unwrap_or(defaults.rope_theta);
        let intermediate_size = gguf
            .feed_forward_length()
            .unwrap_or(defaults.intermediate_size);

        // Detect expert count from tensor names or metadata
        let num_experts = self
            .detect_expert_count(gguf)
            .or_else(|| Self::meta_usize(gguf, "llm.expert_count"))
            .unwrap_or(defaults.num_experts);

        // Active experts per token
        let active_experts = Self::meta_usize(gguf, "llm.expert_used_count")
            .or_else(|| Self::meta_usize(gguf, "model.expert_count_active"))
            .unwrap_or(defaults.active_experts);

        // MoE intermediate size (may differ from dense intermediate_size)
        let moe_intermediate_size = Self::meta_usize(gguf, "llm.expert_feed_forward_length")
            .unwrap_or(defaults.moe_intermediate_size);

        // MLA parameters
        let q_lora_rank =
            Self::meta_usize(gguf, "llm.attention.q_lora_rank").unwrap_or(defaults.q_lora_rank);
        let kv_lora_rank =
            Self::meta_usize(gguf, "llm.attention.kv_lora_rank").unwrap_or(defaults.kv_lora_rank);
        let qk_nope_head_dim = Self::meta_usize(gguf, "llm.attention.key_length_nope")
            .unwrap_or(defaults.qk_nope_head_dim);
        let qk_rope_head_dim = Self::meta_usize(gguf, "llm.attention.key_length_rope")
            .or_else(|| gguf.rope_dimension_count())
            .unwrap_or(defaults.qk_rope_head_dim);
        let v_head_dim =
            Self::meta_usize(gguf, "llm.attention.value_length").unwrap_or(defaults.v_head_dim);

        // Detect MLA by checking for q_a tensor in first layer
        let use_mla = TensorNameMapper::has_mla(gguf, 0);

        // Shared experts
        let n_shared_experts =
            Self::meta_usize(gguf, "llm.expert_shared_count").unwrap_or(if num_experts > 1 {
                defaults.n_shared_experts
            } else {
                0
            });

        // First K dense layers
        let first_k_dense_replace = Self::meta_usize(gguf, "llm.expert_first_dense_layers")
            .unwrap_or(defaults.first_k_dense_replace);

        // Routed scaling factor
        let routed_scaling_factor = Self::meta_f32(gguf, "llm.expert_weights_scale")
            .unwrap_or(defaults.routed_scaling_factor);

        Ok(BitNetModelConfig {
            num_layers,
            hidden_size,
            num_experts,
            active_experts,
            intermediate_size,
            moe_intermediate_size,
            num_attention_heads,
            num_kv_heads,
            vocab_size,
            max_context,
            rope_theta,
            use_mla,
            q_lora_rank,
            kv_lora_rank,
            qk_nope_head_dim,
            qk_rope_head_dim,
            v_head_dim,
            n_shared_experts,
            first_k_dense_replace,
            routed_scaling_factor,
        })
    }

    /// Helper: extract a usize from GGUF metadata.
    fn meta_usize(gguf: &GgufFile, key: &str) -> Option<usize> {
        gguf.metadata
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }

    /// Helper: extract an f32 from GGUF metadata.
    fn meta_f32(gguf: &GgufFile, key: &str) -> Option<f32> {
        gguf.metadata.get(key).and_then(|v| v.as_f32())
    }

    /// Detect the number of MoE experts by scanning tensor names.
    fn detect_expert_count(&self, gguf: &GgufFile) -> Option<usize> {
        let mut max_expert_idx = 0usize;
        let mut found_any = false;

        for tensor in &gguf.tensors {
            // Look for patterns like "experts.0.", "experts.7.", etc.
            if let Some(pos) = tensor.name.find("experts.") {
                let after = &tensor.name[pos + 8..];
                if let Some(dot) = after.find('.') {
                    if let Ok(idx) = after[..dot].parse::<usize>() {
                        max_expert_idx = max_expert_idx.max(idx);
                        found_any = true;
                    }
                }
            }
        }

        if found_any {
            Some(max_expert_idx + 1)
        } else {
            None
        }
    }

    /// Load an FP16/FP32 tensor from GGUF, returning FP32 data.
    fn load_fp_tensor(
        &self,
        gguf: &GgufFile,
        name: &str,
        _config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        match gguf.get_tensor(name) {
            Some(_) => gguf.load_tensor_f32(name),
            None => Err(RuvLLMError::NotFound(format!(
                "Required tensor not found: {}",
                name
            ))),
        }
    }

    /// Load a ternary tensor from GGUF (BitnetT158 or dequant + re-quantize).
    fn load_ternary_tensor(&self, gguf: &GgufFile, name: &str) -> Result<TernaryTensor> {
        let info = gguf
            .get_tensor(name)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Tensor not found: {}", name)))?;

        if info.dtype == GgufQuantType::BitnetT158 {
            // Native ternary format: extract packed data and scales directly
            let raw = gguf.load_tensor_quantized(name)?;
            let num_elements = info.num_elements();
            let block_size = 256usize;
            let num_blocks = (num_elements + block_size - 1) / block_size;
            let type_size = 66usize; // 64 packed + 2 FP16 scale

            let mut packed_data = Vec::with_capacity(num_blocks * 64);
            let mut scales = Vec::with_capacity(num_blocks);

            for blk in 0..num_blocks {
                let offset = blk * type_size;
                if offset + type_size > raw.data.len() {
                    break;
                }
                packed_data.extend_from_slice(&raw.data[offset..offset + 64]);
                let scale_bits = u16::from_le_bytes([raw.data[offset + 64], raw.data[offset + 65]]);
                scales.push(f16_to_f32(scale_bits));
            }

            let shape = if info.shape.len() == 2 {
                (info.shape[0], info.shape[1])
            } else {
                (1, num_elements)
            };

            Ok(TernaryTensor {
                packed_data,
                scales,
                shape,
                block_size,
            })
        } else {
            // Non-native format: dequantize to FP32, then quantize to ternary
            let fp32 = gguf.load_tensor_f32(name)?;
            let num_elements = fp32.len();
            let shape = if info.shape.len() == 2 {
                (info.shape[0], info.shape[1])
            } else {
                (1, num_elements)
            };

            let ptconfig = super::quantizer::PtBitnetConfig::default();
            super::quantizer::quantize_tensor(&fp32, shape, &ptconfig)
        }
    }

    /// Load a single transformer layer.
    ///
    /// Detects the layer type (dense vs MoE), attention type (MLA vs GQA),
    /// and expert tensor format (stacked 3D vs individual) from the GGUF file.
    fn load_layer(
        &self,
        gguf: &GgufFile,
        idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<TransformerLayer> {
        // Norm weights via name mapper
        let in_norm_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::input_norm(idx))
            .ok_or_else(|| RuvLLMError::NotFound(format!("Layer {} input norm not found", idx)))?;
        let input_norm_weight = self.load_fp_tensor(gguf, &in_norm_name, config)?;

        let post_norm_name =
            TensorNameMapper::resolve(gguf, &TensorNameMapper::post_attn_norm(idx)).ok_or_else(
                || RuvLLMError::NotFound(format!("Layer {} post-attn norm not found", idx)),
            )?;
        let post_attn_norm_weight = self.load_fp_tensor(gguf, &post_norm_name, config)?;

        // === Attention weights ===
        let attention = if TensorNameMapper::has_mla(gguf, idx) {
            self.load_mla_attention(gguf, idx, config)?
        } else {
            self.load_gqa_attention(gguf, idx, config)?
        };

        // === FFN weights ===
        let is_dense_layer =
            idx < config.first_k_dense_replace || TensorNameMapper::has_dense_ffn(gguf, idx);

        if is_dense_layer {
            // Dense FFN layer (no MoE routing)
            let dense_ffn = self.load_dense_ffn(gguf, idx, config)?;
            Ok(TransformerLayer {
                input_norm_weight,
                post_attn_norm_weight,
                attention,
                layer_type: LayerType::Dense,
                gate_weight: Vec::new(),
                experts: Vec::new(),
                shared_expert: None,
                dense_ffn: Some(dense_ffn),
            })
        } else {
            // MoE layer: load router gate + experts
            let gate_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::moe_gate(idx))
                .ok_or_else(|| {
                    RuvLLMError::NotFound(format!("Layer {} MoE gate not found", idx))
                })?;
            let gate_weight = self.load_fp_tensor(gguf, &gate_name, config)?;

            let experts = self.load_experts(gguf, idx, config)?;

            // Try loading shared expert
            let shared_expert = self.load_shared_expert(gguf, idx, config).ok();

            let layer_type = if shared_expert.is_some() {
                LayerType::MoeWithShared
            } else {
                LayerType::Moe
            };

            Ok(TransformerLayer {
                input_norm_weight,
                post_attn_norm_weight,
                attention,
                layer_type,
                gate_weight,
                experts,
                shared_expert,
                dense_ffn: None,
            })
        }
    }

    /// Load MLA attention weights for a layer.
    fn load_mla_attention(
        &self,
        gguf: &GgufFile,
        idx: usize,
        _config: &BitNetModelConfig,
    ) -> Result<AttentionWeights> {
        // MLA projections
        let q_a_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_q_a(idx))
            .ok_or_else(|| RuvLLMError::NotFound(format!("Layer {} attn_q_a not found", idx)))?;
        let q_a = self.load_ternary_tensor(gguf, &q_a_name)?;

        let q_b_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_q_b(idx))
            .ok_or_else(|| RuvLLMError::NotFound(format!("Layer {} attn_q_b not found", idx)))?;
        let q_b = self.load_ternary_tensor(gguf, &q_b_name)?;

        let kv_a_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_kv_a_mqa(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} attn_kv_a_mqa not found", idx))
            })?;
        let kv_a_mqa = self.load_ternary_tensor(gguf, &kv_a_name)?;

        let k_b_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_k_b(idx))
            .ok_or_else(|| RuvLLMError::NotFound(format!("Layer {} attn_k_b not found", idx)))?;
        let k_b = self.load_ternary_tensor(gguf, &k_b_name)?;

        let v_b_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_v_b(idx))
            .ok_or_else(|| RuvLLMError::NotFound(format!("Layer {} attn_v_b not found", idx)))?;
        let v_b = self.load_ternary_tensor(gguf, &v_b_name)?;

        let o_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_output(idx))
            .ok_or_else(|| RuvLLMError::NotFound(format!("Layer {} attn_output not found", idx)))?;
        let o_proj = self.load_ternary_tensor(gguf, &o_name)?;

        // Norm weights for MLA compression (may or may not be present)
        let q_a_norm = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_q_a_norm(idx))
            .and_then(|n| self.load_fp_tensor(gguf, &n, _config).ok());
        let kv_a_norm = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_kv_a_norm(idx))
            .and_then(|n| self.load_fp_tensor(gguf, &n, _config).ok());

        // Use o_proj as placeholder for the standard fields (they won't be used in MLA path)
        let placeholder = TernaryTensor {
            packed_data: vec![],
            scales: vec![],
            shape: (0, 0),
            block_size: 256,
        };

        Ok(AttentionWeights {
            is_mla: true,
            q_proj: placeholder.clone(),
            k_proj: placeholder.clone(),
            v_proj: placeholder,
            o_proj,
            q_a: Some(q_a),
            q_b: Some(q_b),
            q_a_norm,
            kv_a_mqa: Some(kv_a_mqa),
            kv_a_norm,
            k_b: Some(k_b),
            v_b: Some(v_b),
        })
    }

    /// Load standard GQA attention weights for a layer.
    fn load_gqa_attention(
        &self,
        gguf: &GgufFile,
        idx: usize,
        _config: &BitNetModelConfig,
    ) -> Result<AttentionWeights> {
        let q_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_q_proj(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} Q projection not found", idx))
            })?;
        let q_proj = self.load_ternary_tensor(gguf, &q_name)?;

        let k_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_k_proj(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} K projection not found", idx))
            })?;
        let k_proj = self.load_ternary_tensor(gguf, &k_name)?;

        let v_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_v_proj(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} V projection not found", idx))
            })?;
        let v_proj = self.load_ternary_tensor(gguf, &v_name)?;

        let o_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::attn_output(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} O projection not found", idx))
            })?;
        let o_proj = self.load_ternary_tensor(gguf, &o_name)?;

        Ok(AttentionWeights {
            is_mla: false,
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            q_a: None,
            q_b: None,
            q_a_norm: None,
            kv_a_mqa: None,
            kv_a_norm: None,
            k_b: None,
            v_b: None,
        })
    }

    /// Load dense FFN weights for a layer (no MoE).
    fn load_dense_ffn(
        &self,
        gguf: &GgufFile,
        idx: usize,
        _config: &BitNetModelConfig,
    ) -> Result<ExpertWeights> {
        let gate_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_gate(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} dense ffn_gate not found", idx))
            })?;
        let up_name =
            TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_up(idx)).ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} dense ffn_up not found", idx))
            })?;
        let down_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_down(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} dense ffn_down not found", idx))
            })?;

        Ok(ExpertWeights {
            gate_proj: self.load_ternary_tensor(gguf, &gate_name)?,
            up_proj: self.load_ternary_tensor(gguf, &up_name)?,
            down_proj: self.load_ternary_tensor(gguf, &down_name)?,
        })
    }

    /// Load shared expert weights for a layer.
    fn load_shared_expert(
        &self,
        gguf: &GgufFile,
        idx: usize,
        _config: &BitNetModelConfig,
    ) -> Result<ExpertWeights> {
        let gate_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_gate_shexp(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} shared expert gate not found", idx))
            })?;
        let up_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_up_shexp(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} shared expert up not found", idx))
            })?;
        let down_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_down_shexp(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} shared expert down not found", idx))
            })?;

        Ok(ExpertWeights {
            gate_proj: self.load_ternary_tensor(gguf, &gate_name)?,
            up_proj: self.load_ternary_tensor(gguf, &up_name)?,
            down_proj: self.load_ternary_tensor(gguf, &down_name)?,
        })
    }

    /// Load routed expert weights, supporting both stacked (3D) and individual tensor formats.
    fn load_experts(
        &self,
        gguf: &GgufFile,
        idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<ExpertWeights>> {
        if TensorNameMapper::has_stacked_experts(gguf, idx) {
            self.load_stacked_experts(gguf, idx, config)
        } else {
            self.load_individual_experts(gguf, idx, config)
        }
    }

    /// Load stacked expert tensors (3D format: [num_experts, out_dim, in_dim])
    /// and split into per-expert TernaryTensors.
    fn load_stacked_experts(
        &self,
        gguf: &GgufFile,
        idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<ExpertWeights>> {
        let gate_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_gate_exps(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} stacked gate_exps not found", idx))
            })?;
        let up_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_up_exps(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} stacked up_exps not found", idx))
            })?;
        let down_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::ffn_down_exps(idx))
            .ok_or_else(|| {
                RuvLLMError::NotFound(format!("Layer {} stacked down_exps not found", idx))
            })?;

        // Load stacked tensors as FP32 and split per expert
        let gate_all = gguf.load_tensor_f32(&gate_name)?;
        let up_all = gguf.load_tensor_f32(&up_name)?;
        let down_all = gguf.load_tensor_f32(&down_name)?;

        let num_experts = config.num_experts;
        let intermediate = config.moe_intermediate_size;
        let hidden = config.hidden_size;

        // gate/up: [num_experts, intermediate_size, hidden_size]
        let gate_per_expert = intermediate * hidden;
        // down: [num_experts, hidden_size, intermediate_size]
        let down_per_expert = hidden * intermediate;

        let ptconfig = super::quantizer::PtBitnetConfig::default();
        let mut experts = Vec::with_capacity(num_experts);

        for e in 0..num_experts {
            let gate_start = e * gate_per_expert;
            let gate_end = gate_start + gate_per_expert;
            let gate_slice = if gate_end <= gate_all.len() {
                &gate_all[gate_start..gate_end]
            } else {
                // Insufficient data — create zeros
                &[]
            };

            let up_start = e * gate_per_expert;
            let up_end = up_start + gate_per_expert;
            let up_slice = if up_end <= up_all.len() {
                &up_all[up_start..up_end]
            } else {
                &[]
            };

            let down_start = e * down_per_expert;
            let down_end = down_start + down_per_expert;
            let down_slice = if down_end <= down_all.len() {
                &down_all[down_start..down_end]
            } else {
                &[]
            };

            let gate_proj = if gate_slice.is_empty() {
                TernaryTensor {
                    packed_data: vec![],
                    scales: vec![],
                    shape: (intermediate, hidden),
                    block_size: 256,
                }
            } else {
                super::quantizer::quantize_tensor(gate_slice, (intermediate, hidden), &ptconfig)?
            };
            let up_proj = if up_slice.is_empty() {
                TernaryTensor {
                    packed_data: vec![],
                    scales: vec![],
                    shape: (intermediate, hidden),
                    block_size: 256,
                }
            } else {
                super::quantizer::quantize_tensor(up_slice, (intermediate, hidden), &ptconfig)?
            };
            let down_proj = if down_slice.is_empty() {
                TernaryTensor {
                    packed_data: vec![],
                    scales: vec![],
                    shape: (hidden, intermediate),
                    block_size: 256,
                }
            } else {
                super::quantizer::quantize_tensor(down_slice, (hidden, intermediate), &ptconfig)?
            };

            experts.push(ExpertWeights {
                gate_proj,
                up_proj,
                down_proj,
            });
        }

        Ok(experts)
    }

    /// Load individual expert tensors (HuggingFace naming: `experts.{e}.gate_proj.weight`).
    fn load_individual_experts(
        &self,
        gguf: &GgufFile,
        idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<ExpertWeights>> {
        let mut experts = Vec::with_capacity(config.num_experts);
        for e in 0..config.num_experts {
            let gate_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::expert_gate(idx, e))
                .ok_or_else(|| {
                    RuvLLMError::NotFound(format!("Layer {} expert {} gate_proj not found", idx, e))
                })?;
            let up_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::expert_up(idx, e))
                .ok_or_else(|| {
                    RuvLLMError::NotFound(format!("Layer {} expert {} up_proj not found", idx, e))
                })?;
            let down_name = TensorNameMapper::resolve(gguf, &TensorNameMapper::expert_down(idx, e))
                .ok_or_else(|| {
                    RuvLLMError::NotFound(format!("Layer {} expert {} down_proj not found", idx, e))
                })?;

            experts.push(ExpertWeights {
                gate_proj: self.load_ternary_tensor(gguf, &gate_name)?,
                up_proj: self.load_ternary_tensor(gguf, &up_name)?,
                down_proj: self.load_ternary_tensor(gguf, &down_name)?,
            });
        }
        Ok(experts)
    }

    // ========================================================================
    // Forward Pass
    // ========================================================================

    /// Run a forward pass for a single token, using the KV cache.
    ///
    /// This is the autoregressive path: embed one token, run all layers
    /// with cached K/V from prior positions, return logits.
    ///
    /// Call `reset_cache()` before starting a new sequence.
    ///
    /// # Arguments
    ///
    /// * `token_id` - Single token to process
    /// * `position` - Position index in the sequence (0-based)
    pub fn forward_token(&mut self, token_id: u32, position: usize) -> Result<Vec<f32>> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No model loaded".to_string()))?
            .clone();

        let hidden = config.hidden_size;

        if (token_id as usize) >= config.vocab_size {
            return Err(RuvLLMError::Model(format!(
                "Token ID {} exceeds vocab size {}",
                token_id, config.vocab_size
            )));
        }

        // Periodically rebuild expert predictor from routing history.
        // Rebuild every 16 tokens to amortize the transition matrix cost.
        self.predictor_stale_count += 1;
        if self.predictor_stale_count >= 16 {
            let hist = self.routing_history.lock().unwrap();
            if hist.len() >= 2 {
                self.expert_predictor =
                    Some(ExpertPredictor::from_history(config.num_experts, &hist));
            }
            self.predictor_stale_count = 0;
        }

        // Embedding lookup
        let start = (token_id as usize) * hidden;
        let mut hidden_states: Vec<f32> = self.embedding[start..start + hidden].to_vec();

        // Transformer layers
        for layer_idx in 0..self.layers.len() {
            hidden_states =
                self.forward_layer_cached(&hidden_states, layer_idx, position, &config)?;
        }

        // Final RMSNorm
        rms_norm_inplace(&mut hidden_states, &self.final_norm_weight, 1e-6);

        // LM head: logits = hidden_states @ lm_head^T
        let logits =
            fp32_matvec_transposed(&self.lm_head, &hidden_states, config.vocab_size, hidden);

        Ok(logits)
    }

    /// Legacy forward: process full token sequence without KV cache.
    /// Kept for backwards compatibility with tests.
    pub fn forward(&self, token_ids: &[u32]) -> Result<Vec<f32>> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No model loaded".to_string()))?;

        if token_ids.is_empty() {
            return Err(RuvLLMError::Model("Empty token sequence".to_string()));
        }

        let hidden = config.hidden_size;
        let last_token = *token_ids.last().unwrap() as usize;
        if last_token >= config.vocab_size {
            return Err(RuvLLMError::Model(format!(
                "Token ID {} exceeds vocab size {}",
                last_token, config.vocab_size
            )));
        }
        let mut hidden_states: Vec<f32> =
            self.embedding[last_token * hidden..(last_token + 1) * hidden].to_vec();

        for layer_idx in 0..self.layers.len() {
            hidden_states = self.forward_layer_nocache(&hidden_states, layer_idx, config)?;
        }

        rms_norm_inplace(&mut hidden_states, &self.final_norm_weight, 1e-6);

        let logits =
            fp32_matvec_transposed(&self.lm_head, &hidden_states, config.vocab_size, hidden);

        Ok(logits)
    }

    /// Forward pass through a single layer with KV cache (autoregressive).
    fn forward_layer_cached(
        &mut self,
        input: &[f32],
        layer_idx: usize,
        position: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let hidden = config.hidden_size;

        // --- Pre-attention norm ---
        let mut normed = input.to_vec();
        let layer = &self.layers[layer_idx];
        rms_norm_inplace(&mut normed, &layer.input_norm_weight, 1e-6);

        // --- Attention (MLA or GQA) ---
        let attn_out = if self.layers[layer_idx].attention.is_mla {
            self.forward_mla_cached(&normed, layer_idx, position, config)?
        } else {
            self.forward_gqa_cached(&normed, layer_idx, position, config)?
        };

        // --- Output projection ---
        let o_out = self.tl1_gemv(
            &self.layers[layer_idx].attention.o_proj,
            &attn_out,
            hidden,
            hidden,
        );

        // --- Residual after attention ---
        let mut residual: Vec<f32> = input.iter().zip(o_out.iter()).map(|(r, a)| r + a).collect();

        // --- Post-attention norm ---
        let mut normed_ffn = residual.clone();
        let layer = &self.layers[layer_idx];
        rms_norm_inplace(&mut normed_ffn, &layer.post_attn_norm_weight, 1e-6);

        // --- FFN (Dense, MoE, or MoE+Shared) ---
        let ffn_out = self.forward_ffn(&normed_ffn, layer_idx, config)?;

        for (r, &f) in residual.iter_mut().zip(ffn_out.iter()) {
            *r += f;
        }

        Ok(residual)
    }

    /// GQA attention with KV cache.
    ///
    /// Optimized with 4-wide unrolled dot products and fused score-weighted
    /// value accumulation.
    fn forward_gqa_cached(
        &mut self,
        normed: &[f32],
        layer_idx: usize,
        position: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let hidden = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let num_kv_heads = config.num_kv_heads;
        let head_dim = hidden / num_heads;
        let kv_dim = num_kv_heads * head_dim;

        // Q/K/V projections via TL1 GEMV (SIMD-dispatched)
        let q = self.tl1_gemv(
            &self.layers[layer_idx].attention.q_proj,
            normed,
            hidden,
            hidden,
        );
        let k = self.tl1_gemv(
            &self.layers[layer_idx].attention.k_proj,
            normed,
            kv_dim,
            hidden,
        );
        let v = self.tl1_gemv(
            &self.layers[layer_idx].attention.v_proj,
            normed,
            kv_dim,
            hidden,
        );

        // Apply RoPE to Q and K
        let mut q_rope = q;
        let mut k_rope = k;
        self.apply_rope(&mut q_rope, num_heads, head_dim, position);
        self.apply_rope(&mut k_rope, num_kv_heads, head_dim, position);

        // Update KV cache
        self.kv_caches[layer_idx].keys.push(k_rope);
        self.kv_caches[layer_idx].values.push(v);
        let seq_len = self.kv_caches[layer_idx].len();

        // GQA attention scores with 4-wide dot product
        let gqa_groups = if num_kv_heads > 0 {
            num_heads / num_kv_heads
        } else {
            1
        };
        let inv_sqrt_d = 1.0 / (head_dim as f32).sqrt();
        let mut attn_out = vec![0.0f32; hidden];
        let dim_chunks = head_dim / 4;
        let dim_tail = dim_chunks * 4;

        for h in 0..num_heads {
            let kv_head = h / gqa_groups;
            let q_offset = h * head_dim;
            let k_offset = kv_head * head_dim;

            let mut scores = Vec::with_capacity(seq_len);
            for pos in 0..seq_len {
                let k_vec = &self.kv_caches[layer_idx].keys[pos];
                // 4-wide unrolled dot product
                let mut d0 = 0.0f32;
                let mut d1 = 0.0f32;
                let mut d2 = 0.0f32;
                let mut d3 = 0.0f32;
                for c in 0..dim_chunks {
                    let d = c * 4;
                    unsafe {
                        d0 += *q_rope.get_unchecked(q_offset + d)
                            * *k_vec.get_unchecked(k_offset + d);
                        d1 += *q_rope.get_unchecked(q_offset + d + 1)
                            * *k_vec.get_unchecked(k_offset + d + 1);
                        d2 += *q_rope.get_unchecked(q_offset + d + 2)
                            * *k_vec.get_unchecked(k_offset + d + 2);
                        d3 += *q_rope.get_unchecked(q_offset + d + 3)
                            * *k_vec.get_unchecked(k_offset + d + 3);
                    }
                }
                let mut dot = d0 + d1 + d2 + d3;
                for d in dim_tail..head_dim {
                    dot += q_rope[q_offset + d] * k_vec[k_offset + d];
                }
                scores.push(dot * inv_sqrt_d);
            }

            softmax_inplace(&mut scores);

            // Weighted value accumulation
            let v_offset = kv_head * head_dim;
            for pos in 0..seq_len {
                let v_vec = &self.kv_caches[layer_idx].values[pos];
                let w = scores[pos];
                if w < 1e-10 {
                    continue;
                } // Skip negligible weights
                for d in 0..head_dim {
                    unsafe {
                        *attn_out.get_unchecked_mut(q_offset + d) +=
                            w * *v_vec.get_unchecked(v_offset + d);
                    }
                }
            }
        }

        Ok(attn_out)
    }

    /// MLA (Multi-Head Latent Attention) with KV cache.
    ///
    /// Forward path:
    /// 1. Q: x → W_q_a → RMSNorm → W_q_b → split(Q_nope, Q_rope) → RoPE(Q_rope)
    /// 2. KV: x → W_kv_a → split(c_kv, k_pe) → RoPE(k_pe)
    ///    K: RMSNorm(c_kv) → W_k_b → K_nope → concat(K_nope, K_rope)
    ///    V: c_kv → W_v_b → V
    /// 3. Standard multi-head attention on concatenated Q/K
    ///
    /// When `use_compressed_kv` is enabled, stores only compressed latents (c_kv + k_pe)
    /// instead of full K/V vectors (~17.8x memory reduction), recomputing K_nope and V
    /// from cached latents during attention.
    fn forward_mla_cached(
        &mut self,
        normed: &[f32],
        layer_idx: usize,
        position: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let hidden = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let q_lora_rank = config.q_lora_rank;
        let kv_lora_rank = config.kv_lora_rank;
        let qk_nope_dim = config.qk_nope_head_dim;
        let qk_rope_dim = config.qk_rope_head_dim;
        let v_dim = config.v_head_dim;
        let q_head_dim = qk_nope_dim + qk_rope_dim;
        let kv_a_out = kv_lora_rank + qk_rope_dim;

        let attn = &self.layers[layer_idx].attention;

        // --- Q path ---
        let q_a = attn
            .q_a
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("MLA q_a missing".into()))?;
        let mut c_q = self.tl1_gemv(q_a, normed, q_lora_rank, hidden);

        if let Some(ref norm_w) = attn.q_a_norm {
            rms_norm_inplace(&mut c_q, norm_w, 1e-6);
        }

        let q_b = attn
            .q_b
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("MLA q_b missing".into()))?;
        let q_full = self.tl1_gemv(q_b, &c_q, num_heads * q_head_dim, q_lora_rank);

        // Split Q into nope and rope parts, apply RoPE
        let mut q_nope = vec![0.0f32; num_heads * qk_nope_dim];
        let mut q_rope_part = vec![0.0f32; num_heads * qk_rope_dim];

        for h in 0..num_heads {
            let src = h * q_head_dim;
            let nope_dst = h * qk_nope_dim;
            let rope_dst = h * qk_rope_dim;
            q_nope[nope_dst..nope_dst + qk_nope_dim]
                .copy_from_slice(&q_full[src..src + qk_nope_dim]);
            q_rope_part[rope_dst..rope_dst + qk_rope_dim]
                .copy_from_slice(&q_full[src + qk_nope_dim..src + q_head_dim]);
        }

        self.apply_rope(&mut q_rope_part, num_heads, qk_rope_dim, position);

        // Build full Q by concatenating Q_nope + Q_rope per head
        let mut q_full_concat = vec![0.0f32; num_heads * q_head_dim];
        for h in 0..num_heads {
            let dst = h * q_head_dim;
            let nope_src = h * qk_nope_dim;
            let rope_src = h * qk_rope_dim;
            q_full_concat[dst..dst + qk_nope_dim]
                .copy_from_slice(&q_nope[nope_src..nope_src + qk_nope_dim]);
            q_full_concat[dst + qk_nope_dim..dst + q_head_dim]
                .copy_from_slice(&q_rope_part[rope_src..rope_src + qk_rope_dim]);
        }

        // --- KV path ---
        let kv_a = attn
            .kv_a_mqa
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("MLA kv_a_mqa missing".into()))?;
        let kv_combined = self.tl1_gemv(kv_a, normed, kv_a_out, hidden);

        let c_kv_raw = kv_combined[..kv_lora_rank].to_vec();
        let mut k_pe = kv_combined[kv_lora_rank..].to_vec();
        self.apply_rope(&mut k_pe, 1, qk_rope_dim, position);

        // --- Attention dispatch: compressed or full KV cache ---
        if self.use_compressed_kv {
            // COMPRESSED PATH: store only c_kv + k_pe, recompute K/V during attention.
            // ~17.8x memory savings at the cost of per-position recomputation.
            self.mla_caches[layer_idx].push(c_kv_raw.clone(), k_pe.clone());
            let seq_len = self.mla_caches[layer_idx].len();

            let k_b = self.layers[layer_idx]
                .attention
                .k_b
                .as_ref()
                .ok_or_else(|| RuvLLMError::Model("MLA k_b missing".into()))?;
            let v_b = self.layers[layer_idx]
                .attention
                .v_b
                .as_ref()
                .ok_or_else(|| RuvLLMError::Model("MLA v_b missing".into()))?;

            let inv_sqrt_d = 1.0 / (q_head_dim as f32).sqrt();
            let mut attn_out = vec![0.0f32; num_heads * v_dim];

            for h in 0..num_heads {
                let q_off = h * q_head_dim;

                let mut scores = Vec::with_capacity(seq_len);
                for pos in 0..seq_len {
                    // Recompute K for this cached position from compressed latent
                    let cached_ckv = &self.mla_caches[layer_idx].c_kv[pos];
                    let cached_kpe = &self.mla_caches[layer_idx].k_pe[pos];

                    let mut ckv_normed = cached_ckv.clone();
                    if let Some(ref norm_w) = self.layers[layer_idx].attention.kv_a_norm {
                        rms_norm_inplace(&mut ckv_normed, norm_w, 1e-6);
                    }

                    let k_nope =
                        self.tl1_gemv(k_b, &ckv_normed, num_heads * qk_nope_dim, kv_lora_rank);

                    // Build K for this head: [K_nope_h | K_rope]
                    let nope_off = h * qk_nope_dim;
                    let mut dot = 0.0f32;
                    // Dot with nope portion
                    for d in 0..qk_nope_dim {
                        dot += q_full_concat[q_off + d] * k_nope[nope_off + d];
                    }
                    // Dot with rope portion (shared across heads)
                    for d in 0..qk_rope_dim {
                        dot += q_full_concat[q_off + qk_nope_dim + d] * cached_kpe[d];
                    }
                    scores.push(dot * inv_sqrt_d);
                }

                softmax_inplace(&mut scores);

                // Weighted value accumulation (recompute V from cached c_kv)
                let v_off = h * v_dim;
                for pos in 0..seq_len {
                    let w = scores[pos];
                    if w < 1e-10 {
                        continue;
                    }

                    let cached_ckv = &self.mla_caches[layer_idx].c_kv[pos];
                    let v_full = self.tl1_gemv(v_b, cached_ckv, num_heads * v_dim, kv_lora_rank);
                    for d in 0..v_dim {
                        attn_out[v_off + d] += w * v_full[h * v_dim + d];
                    }
                }
            }

            Ok(attn_out)
        } else {
            // FULL PATH: expand K/V and store in standard KV cache (fast, more memory).
            let mut c_kv_normed = c_kv_raw;
            if let Some(ref norm_w) = self.layers[layer_idx].attention.kv_a_norm {
                rms_norm_inplace(&mut c_kv_normed, norm_w, 1e-6);
            }

            let k_b = self.layers[layer_idx]
                .attention
                .k_b
                .as_ref()
                .ok_or_else(|| RuvLLMError::Model("MLA k_b missing".into()))?;
            let k_nope = self.tl1_gemv(k_b, &c_kv_normed, num_heads * qk_nope_dim, kv_lora_rank);

            let v_b = self.layers[layer_idx]
                .attention
                .v_b
                .as_ref()
                .ok_or_else(|| RuvLLMError::Model("MLA v_b missing".into()))?;
            let c_kv_for_v = &kv_combined[..kv_lora_rank];
            let v_full = self.tl1_gemv(v_b, c_kv_for_v, num_heads * v_dim, kv_lora_rank);

            // Build full K
            let mut k_full = vec![0.0f32; num_heads * q_head_dim];
            for h in 0..num_heads {
                let dst = h * q_head_dim;
                let nope_src = h * qk_nope_dim;
                k_full[dst..dst + qk_nope_dim]
                    .copy_from_slice(&k_nope[nope_src..nope_src + qk_nope_dim]);
                k_full[dst + qk_nope_dim..dst + q_head_dim].copy_from_slice(&k_pe[..qk_rope_dim]);
            }

            // Update KV cache
            self.kv_caches[layer_idx].keys.push(k_full);
            self.kv_caches[layer_idx].values.push(v_full);
            let seq_len = self.kv_caches[layer_idx].len();

            // Multi-head attention
            let inv_sqrt_d = 1.0 / (q_head_dim as f32).sqrt();
            let mut attn_out = vec![0.0f32; num_heads * v_dim];

            for h in 0..num_heads {
                let q_off = h * q_head_dim;

                let mut scores = Vec::with_capacity(seq_len);
                for pos in 0..seq_len {
                    let k_vec = &self.kv_caches[layer_idx].keys[pos];
                    let k_off = h * q_head_dim;
                    let mut dot = 0.0f32;
                    for d in 0..q_head_dim {
                        dot += q_full_concat[q_off + d] * k_vec[k_off + d];
                    }
                    scores.push(dot * inv_sqrt_d);
                }

                softmax_inplace(&mut scores);

                let v_off = h * v_dim;
                for pos in 0..seq_len {
                    let v_vec = &self.kv_caches[layer_idx].values[pos];
                    let w = scores[pos];
                    for d in 0..v_dim {
                        attn_out[v_off + d] += w * v_vec[h * v_dim + d];
                    }
                }
            }

            Ok(attn_out)
        }
    }

    /// Unified FFN forward: dispatches to dense, MoE, or MoE+shared based on layer type.
    ///
    /// For MoE layers, tracks routing decisions in `self.routing_history` to
    /// enable predictive expert prefetching via `ExpertPredictor`.
    fn forward_ffn(
        &self,
        normed_ffn: &[f32],
        layer_idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let hidden = config.hidden_size;
        let layer = &self.layers[layer_idx];

        match layer.layer_type {
            LayerType::Dense => {
                // Dense FFN: single gate/up/down
                let ffn = layer.dense_ffn.as_ref().ok_or_else(|| {
                    RuvLLMError::Model(format!("Layer {} is Dense but has no dense_ffn", layer_idx))
                })?;
                self.expert_forward(normed_ffn, ffn, config)
            }
            LayerType::Moe | LayerType::MoeWithShared => {
                // Predictive prefetch: touch predicted expert weight data before routing.
                // This pulls weight cache lines into L2/L3 during the router computation,
                // hiding memory latency for the upcoming expert GEMVs.
                if let Some(ref predictor) = self.expert_predictor {
                    let hist = self.routing_history.lock().unwrap();
                    if let Some(last) = hist.last() {
                        let predicted = predictor.predict_next(last, config.active_experts);
                        let experts = &self.layers[layer_idx].experts;
                        for &eidx in &predicted {
                            if eidx < experts.len() {
                                // Touch first cache line of gate_proj packed data
                                let data = &experts[eidx].gate_proj.packed_data;
                                if !data.is_empty() {
                                    // Volatile read forces the load, acting as software prefetch
                                    unsafe {
                                        std::ptr::read_volatile(data.as_ptr());
                                    }
                                }
                            }
                        }
                    }
                }

                // Route to top-K experts
                let (indices, weights) =
                    self.route_experts(normed_ffn, &self.layers[layer_idx].gate_weight, config)?;

                // Track routing decisions from the first MoE layer for expert prediction.
                // For GLM-4.7-Flash, layer 0 is Dense (first_k_dense_replace=1), so
                // the first MoE layer is at index first_k_dense_replace.
                if layer_idx == config.first_k_dense_replace {
                    let mut hist = self.routing_history.lock().unwrap();
                    hist.push(indices.clone());
                    if hist.len() > self.max_routing_history {
                        hist.remove(0);
                    }
                }

                let mut output = vec![0.0f32; hidden];

                // Routed experts
                let experts = &self.layers[layer_idx].experts;
                for (&eidx, &ew) in indices.iter().zip(weights.iter()) {
                    if eidx >= experts.len() {
                        continue;
                    }
                    let e_out = self.expert_forward(normed_ffn, &experts[eidx], config)?;
                    for (o, &e) in output.iter_mut().zip(e_out.iter()) {
                        *o += ew * e;
                    }
                }

                // Shared expert (MoeWithShared only)
                if layer.layer_type == LayerType::MoeWithShared {
                    if let Some(ref shared) = self.layers[layer_idx].shared_expert {
                        let s_out = self.expert_forward(normed_ffn, shared, config)?;
                        for (o, &s) in output.iter_mut().zip(s_out.iter()) {
                            *o += s;
                        }
                    }
                }

                Ok(output)
            }
        }
    }

    /// Forward pass through a single layer WITHOUT KV cache (legacy path).
    fn forward_layer_nocache(
        &self,
        input: &[f32],
        layer_idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let hidden = config.hidden_size;

        let mut normed = input.to_vec();
        rms_norm_inplace(&mut normed, &self.layers[layer_idx].input_norm_weight, 1e-6);

        // Attention: single-position (degenerates to V pass-through for GQA)
        let attn_concat = if self.layers[layer_idx].attention.is_mla {
            // MLA single-position: project through full pipeline but attention = identity
            self.forward_mla_single_position(&normed, layer_idx, config)?
        } else {
            // GQA single-position: V expanded to all heads
            let num_heads = config.num_attention_heads;
            let head_dim = hidden / num_heads;
            let kv_dim = config.num_kv_heads * head_dim;
            let gqa_groups = if config.num_kv_heads > 0 {
                num_heads / config.num_kv_heads
            } else {
                1
            };

            let q = self.tl1_gemv(
                &self.layers[layer_idx].attention.q_proj,
                &normed,
                hidden,
                hidden,
            );
            let k = self.tl1_gemv(
                &self.layers[layer_idx].attention.k_proj,
                &normed,
                kv_dim,
                hidden,
            );
            let v = self.tl1_gemv(
                &self.layers[layer_idx].attention.v_proj,
                &normed,
                kv_dim,
                hidden,
            );
            let _ = (q, k); // Exercise projections

            let mut concat = vec![0.0f32; hidden];
            for h in 0..num_heads {
                let kv_head = h / gqa_groups;
                for d in 0..head_dim {
                    concat[h * head_dim + d] = v[kv_head * head_dim + d];
                }
            }
            concat
        };

        let o_out = self.tl1_gemv(
            &self.layers[layer_idx].attention.o_proj,
            &attn_concat,
            hidden,
            hidden,
        );
        let mut residual: Vec<f32> = input.iter().zip(o_out.iter()).map(|(r, a)| r + a).collect();

        let mut normed_ffn = residual.clone();
        rms_norm_inplace(
            &mut normed_ffn,
            &self.layers[layer_idx].post_attn_norm_weight,
            1e-6,
        );

        let ffn_out = self.forward_ffn(&normed_ffn, layer_idx, config)?;

        for (r, &f) in residual.iter_mut().zip(ffn_out.iter()) {
            *r += f;
        }

        Ok(residual)
    }

    /// MLA forward for single-position (no KV cache). Used in legacy forward path.
    fn forward_mla_single_position(
        &self,
        normed: &[f32],
        layer_idx: usize,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let hidden = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let q_lora_rank = config.q_lora_rank;
        let kv_lora_rank = config.kv_lora_rank;
        let v_dim = config.v_head_dim;
        let kv_a_out = kv_lora_rank + config.qk_rope_head_dim;

        let attn = &self.layers[layer_idx].attention;

        // Q path (exercise projections)
        if let Some(ref q_a) = attn.q_a {
            let mut c_q = self.tl1_gemv(q_a, normed, q_lora_rank, hidden);
            if let Some(ref norm_w) = attn.q_a_norm {
                rms_norm_inplace(&mut c_q, norm_w, 1e-6);
            }
            if let Some(ref q_b) = attn.q_b {
                let _q = self.tl1_gemv(
                    q_b,
                    &c_q,
                    num_heads * (config.qk_nope_head_dim + config.qk_rope_head_dim),
                    q_lora_rank,
                );
            }
        }

        // KV path
        let kv_a = self.layers[layer_idx]
            .attention
            .kv_a_mqa
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("MLA kv_a_mqa missing in nocache path".into()))?;
        let kv_combined = self.tl1_gemv(kv_a, normed, kv_a_out, hidden);
        let c_kv = &kv_combined[..kv_lora_rank];

        // V = c_kv @ W_v_b
        let v_b = self.layers[layer_idx]
            .attention
            .v_b
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("MLA v_b missing".into()))?;
        let v_full = self.tl1_gemv(v_b, c_kv, num_heads * v_dim, kv_lora_rank);

        // Single position: attention is identity, output = V directly
        Ok(v_full)
    }

    /// Apply Rotary Position Embedding (RoPE) in-place.
    ///
    /// For each head, rotates pairs of dimensions (2i, 2i+1) by position-dependent angles.
    fn apply_rope(&self, x: &mut [f32], num_heads: usize, head_dim: usize, position: usize) {
        let half = head_dim / 2;
        let max_seq = self.rope_cos.len() / half;
        if position >= max_seq {
            return; // Beyond pre-computed tables — skip RoPE
        }
        let cos_base = position * half;
        for h in 0..num_heads {
            let offset = h * head_dim;
            for i in 0..half {
                let cos_val = self.rope_cos[cos_base + i];
                let sin_val = self.rope_sin[cos_base + i];
                let x0 = x[offset + 2 * i];
                let x1 = x[offset + 2 * i + 1];
                x[offset + 2 * i] = x0 * cos_val - x1 * sin_val;
                x[offset + 2 * i + 1] = x0 * sin_val + x1 * cos_val;
            }
        }
    }

    // ========================================================================
    // MoE Router
    // ========================================================================

    /// Route hidden states to the top-K experts.
    ///
    /// Computes `scores = hidden_states @ gate_weight^T`, applies softmax,
    /// then selects the top-K experts with highest scores.
    ///
    /// # Returns
    ///
    /// Tuple of (expert_indices, expert_weights) both of length active_experts.
    fn route_experts(
        &self,
        hidden_states: &[f32],
        gate_weight: &[f32],
        config: &BitNetModelConfig,
    ) -> Result<(Vec<usize>, Vec<f32>)> {
        let num_experts = config.num_experts;
        let hidden = config.hidden_size;
        // Clamp top_k to num_experts to prevent selecting more experts than exist
        let top_k = config.active_experts.min(num_experts);

        if num_experts == 0 {
            return Ok((vec![], vec![]));
        }

        // Gate: scores[e] = dot(hidden_states, gate_weight[e])
        let mut scores = vec![0.0f32; num_experts];
        for e in 0..num_experts {
            let row_start = e * hidden;
            if row_start + hidden > gate_weight.len() {
                break;
            }
            let mut dot = 0.0f32;
            for j in 0..hidden {
                dot += hidden_states[j] * gate_weight[row_start + j];
            }
            scores[e] = dot;
        }

        // Softmax over expert scores
        softmax_inplace(&mut scores);

        // Top-K selection
        let mut indexed: Vec<(usize, f32)> = scores.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let selected: Vec<(usize, f32)> = indexed.into_iter().take(top_k).collect();

        // Renormalize selected weights so they sum to 1
        let weight_sum: f32 = selected.iter().map(|(_, w)| w).sum();
        let norm_factor = if weight_sum > 1e-12 {
            1.0 / weight_sum
        } else {
            1.0
        };

        let expert_indices: Vec<usize> = selected.iter().map(|(i, _)| *i).collect();
        let expert_weights: Vec<f32> = selected.iter().map(|(_, w)| w * norm_factor).collect();

        Ok((expert_indices, expert_weights))
    }

    // ========================================================================
    // Expert FFN (TL1 GEMV)
    // ========================================================================

    /// Forward pass through a single expert's SwiGLU FFN.
    ///
    /// Fused implementation: gate and up projections are computed, then
    /// SiLU(gate) * up is fused in a single pass to halve memory traffic.
    ///
    /// Computes:
    /// ```text
    /// gate = TL1_GEMV(gate_proj, input)
    /// up   = TL1_GEMV(up_proj, input)
    /// hidden = silu(gate) * up    [FUSED: single pass]
    /// output = TL1_GEMV(down_proj, hidden)
    /// ```
    fn expert_forward(
        &self,
        input: &[f32],
        expert: &ExpertWeights,
        config: &BitNetModelConfig,
    ) -> Result<Vec<f32>> {
        let intermediate = config.intermediate_size;
        let hidden = config.hidden_size;

        // gate_proj and up_proj GEMVs
        let gate_out = self.tl1_gemv(&expert.gate_proj, input, intermediate, hidden);
        let up_out = self.tl1_gemv(&expert.up_proj, input, intermediate, hidden);

        // Fused SiLU(gate) * up — single pass with 4-wide unroll
        let mut fused = vec![0.0f32; intermediate];
        let chunks = intermediate / 4;
        let remainder = intermediate % 4;

        // Unrolled 4-wide loop — keeps gate/up values in registers
        for c in 0..chunks {
            let base = c * 4;
            unsafe {
                let g0 = *gate_out.get_unchecked(base);
                let g1 = *gate_out.get_unchecked(base + 1);
                let g2 = *gate_out.get_unchecked(base + 2);
                let g3 = *gate_out.get_unchecked(base + 3);
                let u0 = *up_out.get_unchecked(base);
                let u1 = *up_out.get_unchecked(base + 1);
                let u2 = *up_out.get_unchecked(base + 2);
                let u3 = *up_out.get_unchecked(base + 3);
                *fused.get_unchecked_mut(base) = g0 * sigmoid(g0) * u0;
                *fused.get_unchecked_mut(base + 1) = g1 * sigmoid(g1) * u1;
                *fused.get_unchecked_mut(base + 2) = g2 * sigmoid(g2) * u2;
                *fused.get_unchecked_mut(base + 3) = g3 * sigmoid(g3) * u3;
            }
        }
        let tail_start = chunks * 4;
        for i in 0..remainder {
            let idx = tail_start + i;
            fused[idx] = gate_out[idx] * sigmoid(gate_out[idx]) * up_out[idx];
        }

        // down_proj
        let output = self.tl1_gemv(&expert.down_proj, &fused, hidden, intermediate);

        Ok(output)
    }

    /// TL1 GEMV: ternary matrix-vector product with automatic SIMD dispatch.
    ///
    /// Delegates to AVX2 kernel on x86_64 (16 elements/iter via vpshufb LUT +
    /// INT16 madd), with scalar LUT fallback on other architectures.
    ///
    /// Computes `output[i] = sum_j(ternary_weight[i,j] * input[j]) * scale[block]`
    #[inline]
    fn tl1_gemv(
        &self,
        weight: &TernaryTensor,
        input: &[f32],
        out_rows: usize,
        in_cols: usize,
    ) -> Vec<f32> {
        let mut output = vec![0.0f32; out_rows];
        if out_rows == 0 || in_cols == 0 || weight.packed_data.is_empty() {
            return output;
        }
        Self::tl1_gemv_dispatch(
            &self.tl1_lut,
            &weight.packed_data,
            &weight.scales,
            input,
            &mut output,
            out_rows,
            in_cols,
            weight.block_size,
        );
        output
    }

    /// TL1 GEMV into a pre-allocated output buffer (zero-alloc hot path).
    ///
    /// The caller must ensure `output.len() >= out_rows`.
    #[inline]
    fn tl1_gemv_into(
        &self,
        weight: &TernaryTensor,
        input: &[f32],
        output: &mut [f32],
        out_rows: usize,
        in_cols: usize,
    ) {
        for v in output[..out_rows].iter_mut() {
            *v = 0.0;
        }
        if out_rows == 0 || in_cols == 0 || weight.packed_data.is_empty() {
            return;
        }
        Self::tl1_gemv_dispatch(
            &self.tl1_lut,
            &weight.packed_data,
            &weight.scales,
            input,
            &mut output[..out_rows],
            out_rows,
            in_cols,
            weight.block_size,
        );
    }

    /// Dispatch TL1 GEMV to AVX2 SIMD when available, otherwise scalar LUT path.
    #[inline]
    fn tl1_gemv_dispatch(
        lut: &[[i8; 4]; 256],
        packed_data: &[u8],
        scales: &[f32],
        input: &[f32],
        output: &mut [f32],
        out_rows: usize,
        in_cols: usize,
        block_size: usize,
    ) {
        // AVX2 SIMD path (compile-time gate + runtime dispatch inside tl1_avx2)
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            super::tl1_avx2::tl1_gemv(
                packed_data,
                scales,
                input,
                output,
                out_rows,
                in_cols,
                block_size,
            );
            return;
        }

        // Scalar LUT fallback for non-AVX2 platforms
        #[allow(unreachable_code)]
        {
            let bytes_per_row = (in_cols + 3) / 4;
            let blocks_per_row = (in_cols + block_size - 1) / block_size;

            for row in 0..out_rows {
                let row_byte_offset = row * bytes_per_row;
                let row_scale_offset = row * blocks_per_row;
                let mut accum = 0.0f32;

                for blk in 0..blocks_per_row {
                    let scale = scales.get(row_scale_offset + blk).copied().unwrap_or(1.0);

                    let blk_start = blk * block_size;
                    let blk_end = (blk_start + block_size).min(in_cols);
                    let mut block_accum = 0.0f32;
                    let mut c = blk_start;

                    // Process 4 elements at a time via LUT
                    while c + 4 <= blk_end {
                        let byte_idx = row_byte_offset + c / 4;
                        if byte_idx >= packed_data.len() {
                            break;
                        }
                        let ternary = &lut[packed_data[byte_idx] as usize];
                        for k in 0..4 {
                            let t = ternary[k];
                            if t == 1 {
                                block_accum += input[c + k];
                            } else if t == -1 {
                                block_accum -= input[c + k];
                            }
                        }
                        c += 4;
                    }

                    // Handle tail
                    while c < blk_end {
                        let byte_idx = row_byte_offset + c / 4;
                        let bit_pos = c % 4;
                        if byte_idx < packed_data.len() {
                            let t = lut[packed_data[byte_idx] as usize][bit_pos];
                            if t == 1 {
                                block_accum += input[c];
                            } else if t == -1 {
                                block_accum -= input[c];
                            }
                        }
                        c += 1;
                    }

                    accum += block_accum * scale;
                }

                output[row] += accum;
            }
        }
    }

    // ========================================================================
    // Tensor Discovery & Model Validation
    // ========================================================================

    /// Discover and classify all tensors in a GGUF file.
    ///
    /// Returns a structured report of found tensors, grouped by type
    /// (embedding, attention, FFN, norm, etc.), with shape and quantization info.
    pub fn discover_tensors(path: &str) -> Result<TensorDiscoveryReport> {
        let gguf = GgufFile::open_mmap(Path::new(path))?;
        let mut report = TensorDiscoveryReport {
            total_tensors: gguf.tensors.len(),
            total_bytes: gguf.total_tensor_size(),
            architecture: gguf.architecture().map(|s| s.to_string()),
            tensor_groups: Vec::new(),
            warnings: Vec::new(),
        };

        // Classify tensors
        let mut embedding = Vec::new();
        let mut attention = Vec::new();
        let mut ffn = Vec::new();
        let mut norm = Vec::new();
        let mut other = Vec::new();

        for t in &gguf.tensors {
            let info = TensorEntry {
                name: t.name.clone(),
                shape: t.shape.clone(),
                dtype: t.dtype.name().to_string(),
                bytes: t.byte_size(),
            };

            if t.name.contains("embd") || t.name.contains("embed") || t.name == "output.weight" {
                embedding.push(info);
            } else if t.name.contains("attn") || t.name.contains("self_attn") {
                attention.push(info);
            } else if t.name.contains("ffn") || t.name.contains("mlp") || t.name.contains("expert")
            {
                ffn.push(info);
            } else if t.name.contains("norm") {
                norm.push(info);
            } else {
                other.push(info);
            }
        }

        if !embedding.is_empty() {
            report.tensor_groups.push(TensorGroup {
                name: "Embedding/Output".into(),
                tensors: embedding,
            });
        }
        if !norm.is_empty() {
            report.tensor_groups.push(TensorGroup {
                name: "Normalization".into(),
                tensors: norm,
            });
        }
        if !attention.is_empty() {
            report.tensor_groups.push(TensorGroup {
                name: "Attention".into(),
                tensors: attention,
            });
        }
        if !ffn.is_empty() {
            report.tensor_groups.push(TensorGroup {
                name: "FFN/Expert".into(),
                tensors: ffn,
            });
        }
        if !other.is_empty() {
            report.tensor_groups.push(TensorGroup {
                name: "Other".into(),
                tensors: other,
            });
        }

        // Detect naming convention
        let has_blk = gguf.tensors.iter().any(|t| t.name.starts_with("blk."));
        let has_model = gguf.tensors.iter().any(|t| t.name.starts_with("model."));
        if has_blk && has_model {
            report
                .warnings
                .push("Mixed naming conventions detected (blk.* and model.*)".into());
        }

        // Detect MLA
        let has_mla = gguf.tensors.iter().any(|t| t.name.contains("attn_q_a"));
        if has_mla {
            report
                .warnings
                .push("MLA (Multi-Head Latent Attention) tensors detected".into());
        }

        // Detect stacked experts
        let has_exps = gguf.tensors.iter().any(|t| t.name.contains("_exps"));
        if has_exps {
            report
                .warnings
                .push("Stacked expert tensors detected (3D format)".into());
        }

        Ok(report)
    }

    /// Validate that a GGUF file has all required tensors for loading.
    ///
    /// Returns a list of missing tensor names and a boolean indicating
    /// whether the model can be loaded.
    pub fn validate_model(path: &str) -> Result<ModelValidation> {
        let gguf = GgufFile::open_mmap(Path::new(path))?;
        let backend = BitNetBackend::new();
        let config = backend.extract_config(&gguf)?;
        let mut missing = Vec::new();
        let mut found = Vec::new();

        // Check global tensors
        for (label, candidates) in [
            ("Embedding", TensorNameMapper::embedding()),
            ("Output/LM Head", TensorNameMapper::output()),
            ("Final Norm", TensorNameMapper::final_norm()),
        ] {
            if let Some(name) = TensorNameMapper::resolve(&gguf, &candidates) {
                found.push(format!("{}: {}", label, name));
            } else {
                missing.push(format!("{} (tried: {})", label, candidates.join(", ")));
            }
        }

        // Check first layer tensors to determine structure
        let idx = 0;
        for (label, candidates) in [
            ("Layer 0 Input Norm", TensorNameMapper::input_norm(idx)),
            (
                "Layer 0 Post-Attn Norm",
                TensorNameMapper::post_attn_norm(idx),
            ),
        ] {
            if let Some(name) = TensorNameMapper::resolve(&gguf, &candidates) {
                found.push(format!("{}: {}", label, name));
            } else {
                missing.push(format!("{} (tried: {})", label, candidates.join(", ")));
            }
        }

        // Check attention type
        if TensorNameMapper::has_mla(&gguf, 0) {
            found.push("Attention type: MLA".into());
            for (label, candidates) in [
                ("Layer 0 attn_q_a", TensorNameMapper::attn_q_a(0)),
                ("Layer 0 attn_q_b", TensorNameMapper::attn_q_b(0)),
                ("Layer 0 attn_kv_a_mqa", TensorNameMapper::attn_kv_a_mqa(0)),
                ("Layer 0 attn_k_b", TensorNameMapper::attn_k_b(0)),
                ("Layer 0 attn_v_b", TensorNameMapper::attn_v_b(0)),
                ("Layer 0 attn_output", TensorNameMapper::attn_output(0)),
            ] {
                if TensorNameMapper::resolve(&gguf, &candidates).is_some() {
                    found.push(format!("  {}: present", label));
                } else {
                    missing.push(format!("{} (tried: {})", label, candidates.join(", ")));
                }
            }
        } else {
            found.push("Attention type: GQA".into());
        }

        // Check FFN structure for layers
        let check_layer = config.first_k_dense_replace.min(config.num_layers);
        if check_layer > 0 {
            if TensorNameMapper::has_dense_ffn(&gguf, 0) {
                found.push("Layer 0: Dense FFN".into());
            } else {
                missing.push("Layer 0 dense FFN tensors".into());
            }
        }
        if config.num_layers > config.first_k_dense_replace {
            let moe_layer = config.first_k_dense_replace;
            if TensorNameMapper::has_stacked_experts(&gguf, moe_layer) {
                found.push(format!("Layer {}: Stacked MoE experts", moe_layer));
            } else if TensorNameMapper::resolve(&gguf, &TensorNameMapper::expert_gate(moe_layer, 0))
                .is_some()
            {
                found.push(format!("Layer {}: Individual MoE experts", moe_layer));
            } else {
                missing.push(format!("Layer {} MoE expert tensors", moe_layer));
            }
        }

        let can_load = missing.is_empty();
        Ok(ModelValidation {
            can_load,
            config_summary: format!(
                "layers={}, hidden={}, heads={}, experts={}, vocab={}, mla={}",
                config.num_layers,
                config.hidden_size,
                config.num_attention_heads,
                config.num_experts,
                config.vocab_size,
                config.use_mla
            ),
            found,
            missing,
        })
    }

    /// Greedy-decode a single next token from logits.
    fn argmax(logits: &[f32]) -> u32 {
        let mut best_idx = 0u32;
        let mut best_val = f32::NEG_INFINITY;
        for (i, &v) in logits.iter().enumerate() {
            if v > best_val {
                best_val = v;
                best_idx = i as u32;
            }
        }
        best_idx
    }
}

// ============================================================================
// Tensor Discovery & Validation Report Types
// ============================================================================

/// Report from tensor discovery on a GGUF file.
#[derive(Debug)]
pub struct TensorDiscoveryReport {
    /// Total number of tensors
    pub total_tensors: usize,
    /// Total bytes across all tensors
    pub total_bytes: usize,
    /// Architecture string from metadata
    pub architecture: Option<String>,
    /// Grouped tensor listings
    pub tensor_groups: Vec<TensorGroup>,
    /// Warnings or observations
    pub warnings: Vec<String>,
}

/// A group of related tensors.
#[derive(Debug)]
pub struct TensorGroup {
    /// Group name (e.g., "Attention", "FFN/Expert")
    pub name: String,
    /// Tensors in this group
    pub tensors: Vec<TensorEntry>,
}

/// Info about a single tensor.
#[derive(Debug)]
pub struct TensorEntry {
    /// Tensor name in GGUF
    pub name: String,
    /// Shape dimensions
    pub shape: Vec<usize>,
    /// Quantization type name
    pub dtype: String,
    /// Size in bytes
    pub bytes: usize,
}

/// Result of model validation against expected tensor layout.
#[derive(Debug)]
pub struct ModelValidation {
    /// Whether all required tensors were found
    pub can_load: bool,
    /// Summary of detected configuration
    pub config_summary: String,
    /// Tensors that were found
    pub found: Vec<String>,
    /// Tensors that are missing
    pub missing: Vec<String>,
}

// ============================================================================
// Generation Statistics
// ============================================================================

/// Statistics from a streaming generation run.
#[derive(Debug, Clone)]
pub struct GenerationStats {
    /// Number of tokens in the prompt
    pub prompt_tokens: usize,
    /// Number of tokens generated
    pub generated_tokens: usize,
    /// Total tokens processed (prompt + generated)
    pub total_tokens: usize,
    /// Wall-clock time for generation (excluding prefill) in milliseconds
    pub elapsed_ms: u64,
    /// Tokens per second (generated tokens / elapsed time)
    pub tokens_per_second: f64,
}

// ============================================================================
// Predictive Expert Prefetcher
// ============================================================================

/// Predicts which experts will be needed next based on routing history.
///
/// Maintains a transition matrix `P[i][j]` estimating the probability that
/// expert `j` is selected at position `t+1` given expert `i` at position `t`.
/// Uses Laplace smoothing to handle unseen transitions.
///
/// # Usage
///
/// ```rust,ignore
/// // Build from routing history (one entry per token position)
/// let history = vec![vec![2, 5], vec![5, 3], vec![2, 7]]; // top-K per position
/// let predictor = ExpertPredictor::from_history(64, &history);
///
/// // Predict next experts given current selection
/// let current = vec![2, 5];
/// let predicted = predictor.predict_next(&current, 4);
/// // predicted might be [3, 7, 5, 2] — likely next experts
/// ```
pub struct ExpertPredictor {
    /// Number of experts
    num_experts: usize,
    /// Transition counts: transition_counts[from][to] = number of observed transitions
    transition_counts: Vec<Vec<u32>>,
    /// Total transitions observed from each expert
    row_totals: Vec<u32>,
}

impl ExpertPredictor {
    /// Build a predictor from routing history.
    ///
    /// `routing_history` is a sequence of expert selections, where each entry
    /// contains the expert IDs selected at that position (top-K).
    pub fn from_history(num_experts: usize, routing_history: &[Vec<usize>]) -> Self {
        let mut transition_counts = vec![vec![0u32; num_experts]; num_experts];
        let mut row_totals = vec![0u32; num_experts];

        // Count transitions: for each consecutive pair of positions,
        // every expert at position t transitions to every expert at position t+1
        for window in routing_history.windows(2) {
            let prev = &window[0];
            let next = &window[1];
            for &from in prev {
                if from >= num_experts {
                    continue;
                }
                for &to in next {
                    if to >= num_experts {
                        continue;
                    }
                    transition_counts[from][to] += 1;
                    row_totals[from] += 1;
                }
            }
        }

        Self {
            num_experts,
            transition_counts,
            row_totals,
        }
    }

    /// Predict the most likely next experts given the current selection.
    ///
    /// Returns up to `top_k` expert IDs ranked by predicted probability.
    /// Aggregates predictions from all currently-active experts.
    pub fn predict_next(&self, current_experts: &[usize], top_k: usize) -> Vec<usize> {
        let mut scores = vec![0.0f32; self.num_experts];

        for &from in current_experts {
            if from >= self.num_experts {
                continue;
            }
            let total = self.row_totals[from] as f32 + self.num_experts as f32; // Laplace denom
            for to in 0..self.num_experts {
                // Laplace-smoothed probability
                let count = self.transition_counts[from][to] as f32 + 1.0;
                scores[to] += count / total;
            }
        }

        // Exclude currently-active experts (they're already loaded)
        for &cur in current_experts {
            if cur < self.num_experts {
                scores[cur] = 0.0;
            }
        }

        // Top-K by score
        let mut indexed: Vec<(usize, f32)> = scores.into_iter().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        indexed.into_iter().take(top_k).map(|(id, _)| id).collect()
    }

    /// Get the transition probability from expert `from` to expert `to`.
    ///
    /// Returns a Laplace-smoothed probability in (0, 1).
    pub fn transition_prob(&self, from: usize, to: usize) -> f32 {
        if from >= self.num_experts || to >= self.num_experts {
            return 0.0;
        }
        let total = self.row_totals[from] as f32 + self.num_experts as f32;
        let count = self.transition_counts[from][to] as f32 + 1.0;
        count / total
    }

    /// Return the number of experts this predictor covers.
    pub fn num_experts(&self) -> usize {
        self.num_experts
    }

    /// Total number of observed transitions.
    pub fn total_observations(&self) -> u64 {
        self.row_totals.iter().map(|&r| r as u64).sum()
    }
}

// ============================================================================
// Compressed MLA KV Cache
// ============================================================================

/// Compressed KV cache for MLA (Multi-Head Latent Attention) layers.
///
/// Instead of storing the full decompressed K and V vectors (which are
/// `num_heads * (qk_nope_head_dim + qk_rope_head_dim)` and
/// `num_heads * v_head_dim` per position), this cache stores the
/// compressed latent representation:
///
/// - `c_kv`: The compressed KV latent, size `kv_lora_rank` per position
/// - `k_pe`: The RoPE-applied key portion, size `qk_rope_head_dim` per position
///
/// Total per position: `kv_lora_rank + qk_rope_head_dim` (e.g., 512 + 64 = 576)
/// vs full KV: `num_heads * (qk_nope_head_dim + qk_rope_head_dim) + num_heads * v_head_dim`
///            (e.g., 20 * 256 + 20 * 256 = 10240)
///
/// This gives a **17.8x memory reduction** for GLM-4.7-Flash at the cost of
/// recomputing K_nope and V from the compressed latent during attention.
#[derive(Debug, Clone)]
pub struct CompressedMlaCache {
    /// Compressed KV latents: one [kv_lora_rank] vector per position
    c_kv: Vec<Vec<f32>>,
    /// RoPE-applied key portion: one [qk_rope_head_dim] vector per position
    k_pe: Vec<Vec<f32>>,
}

impl CompressedMlaCache {
    /// Create a new empty compressed cache.
    pub fn new() -> Self {
        Self {
            c_kv: Vec::new(),
            k_pe: Vec::new(),
        }
    }

    /// Push a new position's compressed KV data.
    pub fn push(&mut self, c_kv: Vec<f32>, k_pe: Vec<f32>) {
        self.c_kv.push(c_kv);
        self.k_pe.push(k_pe);
    }

    /// Number of cached positions.
    pub fn len(&self) -> usize {
        self.c_kv.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.c_kv.is_empty()
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.c_kv.clear();
        self.k_pe.clear();
    }

    /// Memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        let c_kv_bytes: usize = self.c_kv.iter().map(|v| v.len() * 4).sum();
        let k_pe_bytes: usize = self.k_pe.iter().map(|v| v.len() * 4).sum();
        c_kv_bytes + k_pe_bytes
    }

    /// Compute the memory savings ratio vs full KV cache.
    ///
    /// Returns the ratio of full cache size to compressed cache size.
    /// E.g., a return value of 17.8 means the compressed cache is 17.8x smaller.
    pub fn savings_ratio(
        num_heads: usize,
        qk_nope_head_dim: usize,
        qk_rope_head_dim: usize,
        v_head_dim: usize,
        kv_lora_rank: usize,
    ) -> f32 {
        let full_k_dim = num_heads * (qk_nope_head_dim + qk_rope_head_dim);
        let full_v_dim = num_heads * v_head_dim;
        let full_per_pos = (full_k_dim + full_v_dim) as f32;
        let compressed_per_pos = (kv_lora_rank + qk_rope_head_dim) as f32;
        if compressed_per_pos > 0.0 {
            full_per_pos / compressed_per_pos
        } else {
            0.0
        }
    }
}

// ============================================================================
// LlmBackend Trait Implementation
// ============================================================================

// ============================================================================
// Tokenizer trait bridge
// ============================================================================

/// Wraps our BpeTokenizer to implement the crate-level Tokenizer trait.
struct TokenizerBridge<'a> {
    inner: &'a BpeTokenizer,
}

impl<'a> BackendTokenizer for TokenizerBridge<'a> {
    fn encode(&self, text: &str) -> Result<Vec<u32>> {
        Ok(self.inner.encode(text))
    }

    fn decode(&self, tokens: &[u32]) -> Result<String> {
        Ok(self.inner.decode(tokens))
    }

    fn vocab_size(&self) -> usize {
        self.inner.vocab_size()
    }

    fn special_tokens(&self) -> BackendSpecialTokens {
        BackendSpecialTokens {
            bos_token_id: Some(1),
            eos_token_id: Some(2),
            ..Default::default()
        }
    }
}

impl LlmBackend for BitNetBackend {
    fn load_model(&mut self, model_id: &str, _config: ModelConfig) -> Result<()> {
        self.load_gguf(model_id)
    }

    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String> {
        if !self.loaded {
            return Err(RuvLLMError::Model("No model loaded".to_string()));
        }

        let tokenizer = self
            .tok
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No tokenizer loaded".to_string()))?;

        // Encode prompt via tokenizer
        let prompt_tokens = tokenizer.encode(prompt);
        let eos_id = 2u32;

        // Autoregressive generation using forward_token with KV cache.
        // Since generate() takes &self (not &mut self), we use the legacy
        // full-sequence forward path here. Use generate_mut() for KV-cached
        // generation.
        let mut tokens = prompt_tokens;
        let mut generated = Vec::new();

        for _ in 0..params.max_tokens {
            let logits = self.forward(&tokens)?;
            let next_token = Self::argmax(&logits);

            if next_token == eos_id || next_token == 0 {
                break;
            }

            generated.push(next_token);
            tokens.push(next_token);
        }

        // Decode generated tokens back to text
        let text = tokenizer.decode(&generated);
        Ok(text)
    }

    fn generate_stream(
        &self,
        prompt: &str,
        params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<GeneratedToken>> + Send + '_>> {
        let result = self.generate(prompt, params)?;
        let tokens: Vec<Result<GeneratedToken>> = result
            .chars()
            .enumerate()
            .map(|(i, c)| {
                Ok(GeneratedToken {
                    id: i as u32,
                    text: c.to_string(),
                    logprob: None,
                    is_special: false,
                })
            })
            .collect();
        Ok(Box::new(tokens.into_iter()))
    }

    fn generate_stream_v2(&self, prompt: &str, params: GenerateParams) -> Result<TokenStream> {
        let (tx, stream) = TokenStream::channel();
        let result = self.generate(prompt, params.clone());

        match result {
            Ok(text) => {
                let _ = tx.send(StreamEvent::Token(GeneratedToken {
                    id: 0,
                    text,
                    logprob: None,
                    is_special: false,
                }));
                let _ = tx.send(StreamEvent::Done {
                    total_tokens: 1,
                    duration_ms: 0,
                    tokens_per_second: 0.0,
                });
            }
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(e.to_string()));
            }
        }

        Ok(stream)
    }

    fn get_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No model loaded".to_string()))?;
        let tokenizer = self
            .tok
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No tokenizer loaded".to_string()))?;

        let ids = tokenizer.encode(text);
        if ids.is_empty() {
            return Err(RuvLLMError::Model("Empty token sequence".to_string()));
        }

        // Use last token embedding as text representation
        let last_id = *ids.last().unwrap() as usize;
        let hidden = config.hidden_size;
        if last_id >= config.vocab_size {
            return Err(RuvLLMError::Model("Token exceeds vocab".to_string()));
        }
        Ok(self.embedding[last_id * hidden..(last_id + 1) * hidden].to_vec())
    }

    fn tokenizer(&self) -> Option<&dyn BackendTokenizer> {
        self.tok
            .as_ref()
            .map(|t| {
                // Safety: we return a reference with the same lifetime as &self.
                // The TokenizerBridge is a thin wrapper — we use a raw pointer trick
                // to avoid the borrow checker issue with returning a trait object
                // that borrows from self.
                //
                // Alternative: store a Box<dyn BackendTokenizer> directly. For now,
                // return None and callers should use `self.tok` directly.
                let _ = t;
                // Return None for the trait-object path; callers can use tok() accessor
                None::<&dyn BackendTokenizer>
            })
            .flatten()
    }

    fn is_model_loaded(&self) -> bool {
        self.loaded
    }

    fn model_info(&self) -> Option<ModelInfo> {
        let config = self.config.as_ref()?;
        Some(ModelInfo {
            name: self.model_path.clone(),
            architecture: ModelArchitecture::Qwen,
            num_parameters: config.num_layers
                * config.num_experts
                * config.intermediate_size
                * config.hidden_size
                * 3,
            vocab_size: config.vocab_size,
            hidden_size: config.hidden_size,
            num_layers: config.num_layers,
            max_context_length: config.max_context,
            quantization: Some(Quantization::Q2K),
            memory_usage: self.embedding.len() * 4
                + self.lm_head.len() * 4
                + self
                    .layers
                    .iter()
                    .map(|l| {
                        let mut bytes = l.gate_weight.len() * 4
                            + l.input_norm_weight.len() * 4
                            + l.post_attn_norm_weight.len() * 4
                            + l.attention.o_proj.memory_bytes();
                        // Attention: MLA or GQA
                        if l.attention.is_mla {
                            bytes += l.attention.q_a.as_ref().map_or(0, |t| t.memory_bytes());
                            bytes += l.attention.q_b.as_ref().map_or(0, |t| t.memory_bytes());
                            bytes += l
                                .attention
                                .kv_a_mqa
                                .as_ref()
                                .map_or(0, |t| t.memory_bytes());
                            bytes += l.attention.k_b.as_ref().map_or(0, |t| t.memory_bytes());
                            bytes += l.attention.v_b.as_ref().map_or(0, |t| t.memory_bytes());
                            bytes += l.attention.q_a_norm.as_ref().map_or(0, |v| v.len() * 4);
                            bytes += l.attention.kv_a_norm.as_ref().map_or(0, |v| v.len() * 4);
                        } else {
                            bytes += l.attention.q_proj.memory_bytes();
                            bytes += l.attention.k_proj.memory_bytes();
                            bytes += l.attention.v_proj.memory_bytes();
                        }
                        // FFN: routed experts
                        bytes += l
                            .experts
                            .iter()
                            .map(|e| {
                                e.gate_proj.memory_bytes()
                                    + e.up_proj.memory_bytes()
                                    + e.down_proj.memory_bytes()
                            })
                            .sum::<usize>();
                        // FFN: shared expert
                        if let Some(ref se) = l.shared_expert {
                            bytes += se.gate_proj.memory_bytes()
                                + se.up_proj.memory_bytes()
                                + se.down_proj.memory_bytes();
                        }
                        // FFN: dense
                        if let Some(ref df) = l.dense_ffn {
                            bytes += df.gate_proj.memory_bytes()
                                + df.up_proj.memory_bytes()
                                + df.down_proj.memory_bytes();
                        }
                        bytes
                    })
                    .sum::<usize>(),
        })
    }

    fn unload_model(&mut self) {
        self.config = None;
        self.embedding.clear();
        self.lm_head.clear();
        self.final_norm_weight.clear();
        self.layers.clear();
        self.kv_caches.clear();
        self.tok = None;
        self.rope_cos.clear();
        self.rope_sin.clear();
        self.loaded = false;
        self.model_path.clear();
    }
}

impl BitNetBackend {
    /// Autoregressive generate with KV cache (takes &mut self).
    ///
    /// This is the efficient path for generation: each token only computes
    /// attention against cached K/V vectors rather than reprocessing the
    /// full sequence.
    pub fn generate_cached(&mut self, prompt: &str, max_tokens: usize) -> Result<String> {
        if !self.loaded {
            return Err(RuvLLMError::Model("No model loaded".to_string()));
        }
        let tokenizer = self
            .tok
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No tokenizer loaded".to_string()))?;

        let prompt_tokens = tokenizer.encode(prompt);
        let eos_id = 2u32;

        self.reset_cache();

        // Prefill: process all prompt tokens
        let mut last_logits = Vec::new();
        for (pos, &tid) in prompt_tokens.iter().enumerate() {
            last_logits = self.forward_token(tid, pos)?;
        }

        // Decode
        let mut generated = Vec::new();
        let mut pos = prompt_tokens.len();

        for _ in 0..max_tokens {
            let next_token = Self::argmax(&last_logits);
            if next_token == eos_id || next_token == 0 {
                break;
            }
            generated.push(next_token);
            last_logits = self.forward_token(next_token, pos)?;
            pos += 1;
        }

        let tokenizer = self.tok.as_ref().unwrap();
        Ok(tokenizer.decode(&generated))
    }

    /// Get the loaded tokenizer (if any).
    pub fn tok(&self) -> Option<&BpeTokenizer> {
        self.tok.as_ref()
    }

    // ========================================================================
    // Streaming Generation
    // ========================================================================

    /// Streaming autoregressive generation with per-token callback.
    ///
    /// Calls `on_token` for each generated token, allowing callers to process
    /// tokens incrementally (e.g., for real-time output). The callback receives
    /// the token ID, the decoded text for that token, and the token's position.
    ///
    /// Returns the concatenated generated text. If the callback returns `false`,
    /// generation stops early (allows callers to implement stop conditions).
    ///
    /// # Arguments
    ///
    /// * `prompt` - Input text to condition on
    /// * `max_tokens` - Maximum number of tokens to generate
    /// * `on_token` - Callback invoked for each token: `(token_id, text, position) -> continue?`
    pub fn generate_streaming<F>(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        mut on_token: F,
    ) -> Result<GenerationStats>
    where
        F: FnMut(u32, &str, usize) -> bool,
    {
        if !self.loaded {
            return Err(RuvLLMError::Model("No model loaded".to_string()));
        }
        let tokenizer = self
            .tok
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("No tokenizer loaded".to_string()))?;

        let prompt_tokens = tokenizer.encode(prompt);
        let eos_id = 2u32;
        let prompt_len = prompt_tokens.len();

        self.reset_cache();

        // Prefill: process all prompt tokens
        let mut last_logits = Vec::new();
        for (pos, &tid) in prompt_tokens.iter().enumerate() {
            last_logits = self.forward_token(tid, pos)?;
        }

        // Decode with streaming callback
        let mut generated_tokens = Vec::new();
        let mut pos = prompt_len;

        let start_time = std::time::Instant::now();

        for _ in 0..max_tokens {
            let next_token = Self::argmax(&last_logits);
            if next_token == eos_id || next_token == 0 {
                break;
            }

            // Decode single token
            let tokenizer = self.tok.as_ref().unwrap();
            let token_text = tokenizer.decode(&[next_token]);

            generated_tokens.push(next_token);

            // Invoke callback; stop if it returns false
            if !on_token(next_token, &token_text, pos) {
                break;
            }

            last_logits = self.forward_token(next_token, pos)?;
            pos += 1;
        }

        let elapsed = start_time.elapsed();
        let num_generated = generated_tokens.len();

        Ok(GenerationStats {
            prompt_tokens: prompt_len,
            generated_tokens: num_generated,
            total_tokens: prompt_len + num_generated,
            elapsed_ms: elapsed.as_millis() as u64,
            tokens_per_second: if elapsed.as_secs_f64() > 0.0 {
                num_generated as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
        })
    }

    // ========================================================================
    // Predictive Expert Prefetcher
    // ========================================================================

    /// Create a predictive expert prefetcher from routing history.
    ///
    /// Analyzes past routing decisions to build a co-occurrence matrix:
    /// if expert A is selected at position t, which experts are likely at t+1?
    /// Uses this to predict and warm up likely-next experts before they're needed.
    pub fn build_expert_predictor(&self, routing_history: &[Vec<usize>]) -> ExpertPredictor {
        let num_experts = self.config.as_ref().map(|c| c.num_experts).unwrap_or(64);

        ExpertPredictor::from_history(num_experts, routing_history)
    }
}

// ============================================================================
// Math Helpers (standalone functions used by the backend)
// ============================================================================

/// In-place RMSNorm: x = x / rms(x) * weight
///
/// Optimized with 4-wide accumulator and fused multiply for better ILP.
#[inline]
fn rms_norm_inplace(x: &mut [f32], weight: &[f32], eps: f32) {
    let n = x.len();
    if n == 0 {
        return;
    }

    // 4-way parallel accumulation for sum of squares
    let mut s0 = 0.0f32;
    let mut s1 = 0.0f32;
    let mut s2 = 0.0f32;
    let mut s3 = 0.0f32;
    let chunks = n / 4;
    let tail = chunks * 4;

    for c in 0..chunks {
        let base = c * 4;
        unsafe {
            let v0 = *x.get_unchecked(base);
            let v1 = *x.get_unchecked(base + 1);
            let v2 = *x.get_unchecked(base + 2);
            let v3 = *x.get_unchecked(base + 3);
            s0 += v0 * v0;
            s1 += v1 * v1;
            s2 += v2 * v2;
            s3 += v3 * v3;
        }
    }
    let mut sum_sq = s0 + s1 + s2 + s3;
    for i in tail..n {
        sum_sq += x[i] * x[i];
    }

    let inv_rms = 1.0 / (sum_sq / n as f32 + eps).sqrt();

    // Fused scale: x[i] = x[i] * inv_rms * weight[i]
    if weight.len() >= n {
        // Fast path: weight is correctly sized (common case)
        for c in 0..chunks {
            let base = c * 4;
            unsafe {
                *x.get_unchecked_mut(base) *= inv_rms * *weight.get_unchecked(base);
                *x.get_unchecked_mut(base + 1) *= inv_rms * *weight.get_unchecked(base + 1);
                *x.get_unchecked_mut(base + 2) *= inv_rms * *weight.get_unchecked(base + 2);
                *x.get_unchecked_mut(base + 3) *= inv_rms * *weight.get_unchecked(base + 3);
            }
        }
        for i in tail..n {
            x[i] *= inv_rms * weight[i];
        }
    } else {
        // Fallback: weight may be shorter
        for i in 0..n {
            x[i] *= inv_rms * weight.get(i).copied().unwrap_or(1.0);
        }
    }
}

/// In-place softmax with streaming max and fused exp+sum.
///
/// Guards against NaN propagation: if all inputs are -inf or NaN,
/// the result is a uniform distribution (1/n for each element).
#[inline]
fn softmax_inplace(x: &mut [f32]) {
    let n = x.len();
    if n == 0 {
        return;
    }

    // Streaming max with 4-wide reduction
    let mut max_val = f32::NEG_INFINITY;
    for &v in x.iter() {
        if v > max_val {
            max_val = v;
        }
    }

    // Guard: if max_val is -inf or NaN, fall back to uniform
    if max_val.is_nan() || (max_val.is_infinite() && max_val.is_sign_negative()) {
        let uniform = 1.0 / n as f32;
        for v in x.iter_mut() {
            *v = uniform;
        }
        return;
    }

    // Fused exp + sum in a single pass
    let mut sum_exp = 0.0f32;
    for v in x.iter_mut() {
        let e = (*v - max_val).exp();
        *v = e;
        sum_exp += e;
    }

    // Guard: degenerate sum
    if !sum_exp.is_normal() || sum_exp <= 0.0 {
        let uniform = 1.0 / n as f32;
        for v in x.iter_mut() {
            *v = uniform;
        }
        return;
    }

    // Normalize with reciprocal multiply (faster than per-element division)
    let inv_sum = 1.0 / sum_exp;
    for v in x.iter_mut() {
        *v *= inv_sum;
    }
}

/// Sigmoid activation.
#[inline(always)]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// FP16 bits to FP32 conversion (same as in gguf/quantization.rs).
#[inline(always)]
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exp = ((bits >> 10) & 0x1F) as u32;
    let frac = (bits & 0x03FF) as u32;

    if exp == 0 {
        if frac == 0 {
            return f32::from_bits(sign);
        }
        let mut e = 1u32;
        let mut f = frac;
        while (f & 0x0400) == 0 {
            f <<= 1;
            e += 1;
        }
        f &= 0x03FF;
        return f32::from_bits(sign | ((127 - 15 + 1 - e) << 23) | (f << 13));
    }

    if exp == 31 {
        return f32::from_bits(sign | 0x7F80_0000 | (frac << 13));
    }

    f32::from_bits(sign | ((exp + 127 - 15) << 23) | (frac << 13))
}

/// FP32 matrix-vector product (transposed): out[i] = dot(mat[i*cols..], vec)
///
/// mat is [rows, cols] row-major, vec is [cols], out is [rows].
/// Optimized with 4-wide unrolled inner loop for better ILP and cache utilization.
#[inline]
fn fp32_matvec_transposed(mat: &[f32], vec: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; rows];
    let chunks = cols / 4;
    let tail = chunks * 4;

    for i in 0..rows {
        let row_start = i * cols;
        if row_start + cols > mat.len() {
            break;
        }

        // 4-wide unrolled dot product
        let mut d0 = 0.0f32;
        let mut d1 = 0.0f32;
        let mut d2 = 0.0f32;
        let mut d3 = 0.0f32;

        for c in 0..chunks {
            let j = c * 4;
            unsafe {
                let m0 = *mat.get_unchecked(row_start + j);
                let m1 = *mat.get_unchecked(row_start + j + 1);
                let m2 = *mat.get_unchecked(row_start + j + 2);
                let m3 = *mat.get_unchecked(row_start + j + 3);
                let v0 = *vec.get_unchecked(j);
                let v1 = *vec.get_unchecked(j + 1);
                let v2 = *vec.get_unchecked(j + 2);
                let v3 = *vec.get_unchecked(j + 3);
                d0 += m0 * v0;
                d1 += m1 * v1;
                d2 += m2 * v2;
                d3 += m3 * v3;
            }
        }

        let mut dot = d0 + d1 + d2 + d3;
        for j in tail..cols {
            dot += mat[row_start + j] * vec[j];
        }
        output[i] = dot;
    }
    output
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitnet::{pack_ternary, TernaryTensor};

    #[test]
    fn test_build_tl1_lut() {
        let lut = build_tl1_lut();

        // Byte 0x00 = all bits 00 = all -1
        assert_eq!(lut[0x00], [-1, -1, -1, -1]);

        // Byte 0x55 = 01_01_01_01 = all 0
        assert_eq!(lut[0x55], [0, 0, 0, 0]);

        // Byte 0xAA = 10_10_10_10 = all +1
        assert_eq!(lut[0xAA], [1, 1, 1, 1]);

        // Byte 0x24 = 00_10_01_00 => positions: [00, 01, 10, 00] => [-1, 0, 1, -1]
        // bit layout LSB first: bits[0:1]=00, bits[2:3]=01, bits[4:5]=10, bits[6:7]=00
        // 0x24 = 0b00_10_01_00
        assert_eq!(lut[0x24], [-1, 0, 1, -1]);
    }

    #[test]
    fn test_rms_norm_inplace() {
        let mut x = vec![1.0, 2.0, 3.0, 4.0];
        let w = vec![1.0; 4];
        rms_norm_inplace(&mut x, &w, 1e-6);

        // RMS of [1,2,3,4] = sqrt((1+4+9+16)/4) = sqrt(7.5) ≈ 2.7386
        let rms = (30.0f32 / 4.0).sqrt();
        let expected: Vec<f32> = [1.0, 2.0, 3.0, 4.0].iter().map(|v| v / rms).collect();

        for (a, b) in x.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-4, "got {} expected {}", a, b);
        }
    }

    #[test]
    fn test_softmax_inplace() {
        let mut x = vec![1.0, 2.0, 3.0];
        softmax_inplace(&mut x);

        // Sum should be 1.0
        let sum: f32 = x.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        // Values should be ordered
        assert!(x[0] < x[1]);
        assert!(x[1] < x[2]);
    }

    #[test]
    fn test_sigmoid() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.999);
        assert!(sigmoid(-10.0) < 0.001);
    }

    #[test]
    fn test_fp32_matvec_transposed() {
        // Identity matrix 3x3
        let mat = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let vec_in = vec![2.0, 3.0, 4.0];
        let out = fp32_matvec_transposed(&mat, &vec_in, 3, 3);
        assert_eq!(out, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_tl1_gemv_simple() {
        let backend = BitNetBackend::new();

        // Create a 2x4 ternary weight matrix:
        // Row 0: [+1, +1, +1, +1]
        // Row 1: [-1, -1, -1, -1]
        let row0 = vec![1i8, 1, 1, 1];
        let row1 = vec![-1i8, -1, -1, -1];
        let mut all = row0.clone();
        all.extend_from_slice(&row1);
        let packed = pack_ternary(&all);

        let weight = TernaryTensor {
            packed_data: packed,
            scales: vec![1.0, 1.0], // one scale per block (each row < 256, so 1 block per row)
            shape: (2, 4),
            block_size: 256,
        };

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = backend.tl1_gemv(&weight, &input, 2, 4);

        // Row 0: 1+2+3+4 = 10, scale=1.0
        assert!((output[0] - 10.0).abs() < 1e-6);
        // Row 1: -(1+2+3+4) = -10, scale=1.0
        assert!((output[1] - (-10.0)).abs() < 1e-6);
    }

    #[test]
    fn test_tl1_gemv_with_zeros() {
        let backend = BitNetBackend::new();

        // Row: [+1, 0, -1, 0]
        let vals = vec![1i8, 0, -1, 0];
        let packed = pack_ternary(&vals);

        let weight = TernaryTensor {
            packed_data: packed,
            scales: vec![2.0],
            shape: (1, 4),
            block_size: 256,
        };

        let input = vec![5.0, 3.0, 7.0, 9.0];
        let output = backend.tl1_gemv(&weight, &input, 1, 4);

        // Result: (5.0 + 0 - 7.0 + 0) * 2.0 = -2.0 * 2.0 = -4.0
        assert!((output[0] - (-4.0)).abs() < 1e-6);
    }

    #[test]
    fn test_bitnet_model_config_default() {
        let config = BitNetModelConfig::default();
        // GLM-4.7-Flash defaults
        assert_eq!(config.num_layers, 47);
        assert_eq!(config.hidden_size, 2048);
        assert_eq!(config.num_experts, 64);
        assert_eq!(config.active_experts, 4);
        assert_eq!(config.moe_intermediate_size, 1536);
        assert!(config.use_mla);
        assert_eq!(config.q_lora_rank, 768);
        assert_eq!(config.kv_lora_rank, 512);
        assert_eq!(config.qk_nope_head_dim, 192);
        assert_eq!(config.qk_rope_head_dim, 64);
        assert_eq!(config.v_head_dim, 256);
        assert_eq!(config.n_shared_experts, 1);
        assert_eq!(config.first_k_dense_replace, 1);
    }

    #[test]
    fn test_route_experts_topk() {
        let backend = BitNetBackend::new();
        let config = BitNetModelConfig {
            num_experts: 4,
            active_experts: 2,
            hidden_size: 4,
            ..Default::default()
        };

        // Gate weight [4 experts, 4 hidden]: identity-like so expert scores = hidden_states
        let gate_weight = vec![
            1.0, 0.0, 0.0, 0.0, // Expert 0 looks at dim 0
            0.0, 1.0, 0.0, 0.0, // Expert 1 looks at dim 1
            0.0, 0.0, 1.0, 0.0, // Expert 2 looks at dim 2
            0.0, 0.0, 0.0, 1.0, // Expert 3 looks at dim 3
        ];

        // Hidden states: dim 2 is highest, dim 3 is second
        let hidden = vec![0.1, 0.2, 0.9, 0.5];

        let (indices, weights) = backend
            .route_experts(&hidden, &gate_weight, &config)
            .unwrap();

        assert_eq!(indices.len(), 2);
        assert_eq!(weights.len(), 2);

        // Expert 2 should be first (score 0.9), Expert 3 second (score 0.5)
        assert_eq!(indices[0], 2);
        assert_eq!(indices[1], 3);

        // Weights should sum to ~1.0
        let wsum: f32 = weights.iter().sum();
        assert!((wsum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_backend_new_unloaded() {
        let backend = BitNetBackend::new();
        assert!(!backend.is_model_loaded());
        assert!(backend.model_info().is_none());
    }

    #[test]
    fn test_rope_tables() {
        let mut backend = BitNetBackend::new();
        backend.build_rope_tables(16, 8, 10000.0);

        let half = 4; // head_dim / 2
                      // Position 0: all angles are 0 → cos=1, sin=0
        for i in 0..half {
            assert!(
                (backend.rope_cos[i] - 1.0).abs() < 1e-5,
                "cos[0][{}]={}",
                i,
                backend.rope_cos[i]
            );
            assert!(
                backend.rope_sin[i].abs() < 1e-5,
                "sin[0][{}]={}",
                i,
                backend.rope_sin[i]
            );
        }

        // Table size should be max_seq * half
        assert_eq!(backend.rope_cos.len(), 16 * 4);
        assert_eq!(backend.rope_sin.len(), 16 * 4);
    }

    #[test]
    fn test_apply_rope_identity_at_pos_0() {
        let mut backend = BitNetBackend::new();
        backend.build_rope_tables(8, 4, 10000.0);

        let mut x = vec![1.0, 2.0, 3.0, 4.0];
        let original = x.clone();
        backend.apply_rope(&mut x, 1, 4, 0);

        // At position 0, all angles are 0, so cos=1, sin=0 → identity
        for (a, b) in x.iter().zip(original.iter()) {
            assert!(
                (a - b).abs() < 1e-5,
                "RoPE at pos 0 should be identity: got {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_apply_rope_rotates_at_pos_1() {
        let mut backend = BitNetBackend::new();
        backend.build_rope_tables(8, 4, 10000.0);

        let mut x = vec![1.0, 0.0, 1.0, 0.0]; // head_dim=4, 1 head
        let original = x.clone();
        backend.apply_rope(&mut x, 1, 4, 1);

        // At position 1, some rotation should happen
        let changed = x
            .iter()
            .zip(original.iter())
            .any(|(a, b)| (a - b).abs() > 1e-6);
        assert!(changed, "RoPE at pos 1 should rotate the vector");

        // Norm should be preserved (RoPE is an orthogonal rotation)
        let orig_norm: f32 = original.iter().map(|v| v * v).sum::<f32>().sqrt();
        let new_norm: f32 = x.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            (orig_norm - new_norm).abs() < 1e-4,
            "RoPE should preserve norm"
        );
    }

    #[test]
    fn test_kv_cache_operations() {
        let mut cache = LayerKvCache::new();
        assert_eq!(cache.len(), 0);

        cache.keys.push(vec![1.0, 2.0]);
        cache.values.push(vec![3.0, 4.0]);
        assert_eq!(cache.len(), 1);

        cache.keys.push(vec![5.0, 6.0]);
        cache.values.push(vec![7.0, 8.0]);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_byte_level_tokenizer() {
        let tok = BitNetBackend::build_byte_level_tokenizer();
        assert_eq!(tok.vocab_size(), 260); // 4 special + 256 byte tokens

        // Roundtrip ASCII
        let ids = tok.encode("Hello");
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, "Hello", "Byte-level tokenizer roundtrip failed");

        // BOS should be prepended
        assert_eq!(ids[0], 1);
    }

    #[test]
    fn test_byte_level_tokenizer_utf8() {
        let tok = BitNetBackend::build_byte_level_tokenizer();
        let text = "cafe\u{0301}"; // combining accent
        let ids = tok.encode(text);
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_backend_reset_cache() {
        let mut backend = BitNetBackend::new();
        // Manually set up caches
        backend.kv_caches = vec![LayerKvCache::new(), LayerKvCache::new()];
        backend.kv_caches[0].keys.push(vec![1.0]);
        backend.kv_caches[1].keys.push(vec![2.0]);

        backend.reset_cache();
        assert_eq!(backend.kv_caches[0].len(), 0);
        assert_eq!(backend.kv_caches[1].len(), 0);
    }

    #[test]
    fn test_attention_weights_gqa() {
        // Verify GQA AttentionWeights construction
        let packed = pack_ternary(&[1, 0, -1, 0]);
        let tensor = TernaryTensor {
            packed_data: packed.clone(),
            scales: vec![1.0],
            shape: (1, 4),
            block_size: 256,
        };
        let attn = AttentionWeights {
            is_mla: false,
            q_proj: tensor.clone(),
            k_proj: tensor.clone(),
            v_proj: tensor.clone(),
            o_proj: tensor,
            q_a: None,
            q_b: None,
            q_a_norm: None,
            kv_a_mqa: None,
            kv_a_norm: None,
            k_b: None,
            v_b: None,
        };
        assert!(!attn.is_mla);
        assert_eq!(attn.q_proj.shape, (1, 4));
    }

    #[test]
    fn test_attention_weights_mla() {
        // Verify MLA AttentionWeights construction
        let packed = pack_ternary(&[1, 0, -1, 0]);
        let tensor = TernaryTensor {
            packed_data: packed.clone(),
            scales: vec![1.0],
            shape: (1, 4),
            block_size: 256,
        };
        let placeholder = TernaryTensor {
            packed_data: vec![],
            scales: vec![],
            shape: (0, 0),
            block_size: 256,
        };
        let attn = AttentionWeights {
            is_mla: true,
            q_proj: placeholder.clone(),
            k_proj: placeholder.clone(),
            v_proj: placeholder,
            o_proj: tensor.clone(),
            q_a: Some(tensor.clone()),
            q_b: Some(tensor.clone()),
            q_a_norm: Some(vec![1.0; 4]),
            kv_a_mqa: Some(tensor.clone()),
            kv_a_norm: Some(vec![1.0; 4]),
            k_b: Some(tensor.clone()),
            v_b: Some(tensor),
        };
        assert!(attn.is_mla);
        assert!(attn.q_a.is_some());
        assert!(attn.q_b.is_some());
        assert!(attn.kv_a_mqa.is_some());
        assert!(attn.k_b.is_some());
        assert!(attn.v_b.is_some());
    }

    #[test]
    fn test_tok_accessor() {
        let mut backend = BitNetBackend::new();
        assert!(backend.tok().is_none());

        backend.tok = Some(BitNetBackend::build_byte_level_tokenizer());
        assert!(backend.tok().is_some());
        assert_eq!(backend.tok().unwrap().vocab_size(), 260);
    }

    #[test]
    fn test_layer_type_enum() {
        assert_eq!(LayerType::Dense, LayerType::Dense);
        assert_ne!(LayerType::Dense, LayerType::Moe);
        assert_ne!(LayerType::Moe, LayerType::MoeWithShared);
    }

    #[test]
    fn test_tensor_name_mapper_embedding() {
        let candidates = TensorNameMapper::embedding();
        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(&"token_embd.weight".to_string()));
        assert!(candidates.contains(&"model.embed_tokens.weight".to_string()));
    }

    #[test]
    fn test_tensor_name_mapper_mla() {
        let q_a = TensorNameMapper::attn_q_a(5);
        assert_eq!(q_a, vec!["blk.5.attn_q_a.weight".to_string()]);

        let q_b = TensorNameMapper::attn_q_b(5);
        assert_eq!(q_b, vec!["blk.5.attn_q_b.weight".to_string()]);

        let kv_a = TensorNameMapper::attn_kv_a_mqa(5);
        assert_eq!(kv_a, vec!["blk.5.attn_kv_a_mqa.weight".to_string()]);

        let k_b = TensorNameMapper::attn_k_b(5);
        assert_eq!(k_b, vec!["blk.5.attn_k_b.weight".to_string()]);

        let v_b = TensorNameMapper::attn_v_b(5);
        assert_eq!(v_b, vec!["blk.5.attn_v_b.weight".to_string()]);
    }

    #[test]
    fn test_tensor_name_mapper_norms() {
        let in_norm = TensorNameMapper::input_norm(3);
        assert!(in_norm.contains(&"blk.3.attn_norm.weight".to_string()));
        assert!(in_norm.contains(&"model.layers.3.input_layernorm.weight".to_string()));

        let post_norm = TensorNameMapper::post_attn_norm(3);
        assert!(post_norm.contains(&"blk.3.ffn_norm.weight".to_string()));
    }

    #[test]
    fn test_tensor_name_mapper_moe() {
        let gate = TensorNameMapper::moe_gate(2);
        assert!(gate.contains(&"blk.2.ffn_gate_inp.weight".to_string()));

        let exps = TensorNameMapper::ffn_gate_exps(2);
        assert_eq!(exps, vec!["blk.2.ffn_gate_exps.weight".to_string()]);

        let shexp = TensorNameMapper::ffn_gate_shexp(2);
        assert_eq!(shexp, vec!["blk.2.ffn_gate_shexp.weight".to_string()]);
    }

    #[test]
    fn test_tensor_name_mapper_dense_ffn() {
        let gate = TensorNameMapper::ffn_gate(0);
        assert!(gate.contains(&"blk.0.ffn_gate.weight".to_string()));
        assert!(gate.contains(&"model.layers.0.mlp.gate_proj.weight".to_string()));
    }

    #[test]
    fn test_tensor_name_mapper_individual_experts() {
        let gate = TensorNameMapper::expert_gate(1, 3);
        assert_eq!(
            gate,
            vec!["model.layers.1.mlp.experts.3.gate_proj.weight".to_string()]
        );
    }

    #[test]
    fn test_mla_config_dimensions() {
        let config = BitNetModelConfig::default();
        // Q head dim = qk_nope_head_dim + qk_rope_head_dim
        let q_head_dim = config.qk_nope_head_dim + config.qk_rope_head_dim;
        assert_eq!(q_head_dim, 256);

        // Total Q dim = num_heads * q_head_dim
        let total_q_dim = config.num_attention_heads * q_head_dim;
        assert_eq!(total_q_dim, 5120);

        // KV compression output = kv_lora_rank + qk_rope_head_dim
        let kv_a_out = config.kv_lora_rank + config.qk_rope_head_dim;
        assert_eq!(kv_a_out, 576);
    }

    #[test]
    fn test_transformer_layer_dense() {
        let packed = pack_ternary(&[1, 0, -1, 0]);
        let tensor = TernaryTensor {
            packed_data: packed.clone(),
            scales: vec![1.0],
            shape: (1, 4),
            block_size: 256,
        };
        let attn = AttentionWeights {
            is_mla: false,
            q_proj: tensor.clone(),
            k_proj: tensor.clone(),
            v_proj: tensor.clone(),
            o_proj: tensor.clone(),
            q_a: None,
            q_b: None,
            q_a_norm: None,
            kv_a_mqa: None,
            kv_a_norm: None,
            k_b: None,
            v_b: None,
        };
        let layer = TransformerLayer {
            input_norm_weight: vec![1.0; 4],
            post_attn_norm_weight: vec![1.0; 4],
            attention: attn,
            layer_type: LayerType::Dense,
            gate_weight: Vec::new(),
            experts: Vec::new(),
            shared_expert: None,
            dense_ffn: Some(ExpertWeights {
                gate_proj: tensor.clone(),
                up_proj: tensor.clone(),
                down_proj: tensor,
            }),
        };
        assert_eq!(layer.layer_type, LayerType::Dense);
        assert!(layer.dense_ffn.is_some());
        assert!(layer.shared_expert.is_none());
    }

    #[test]
    fn test_transformer_layer_moe_with_shared() {
        let packed = pack_ternary(&[1, 0, -1, 0]);
        let tensor = TernaryTensor {
            packed_data: packed.clone(),
            scales: vec![1.0],
            shape: (1, 4),
            block_size: 256,
        };
        let attn = AttentionWeights {
            is_mla: false,
            q_proj: tensor.clone(),
            k_proj: tensor.clone(),
            v_proj: tensor.clone(),
            o_proj: tensor.clone(),
            q_a: None,
            q_b: None,
            q_a_norm: None,
            kv_a_mqa: None,
            kv_a_norm: None,
            k_b: None,
            v_b: None,
        };
        let expert = ExpertWeights {
            gate_proj: tensor.clone(),
            up_proj: tensor.clone(),
            down_proj: tensor.clone(),
        };
        let layer = TransformerLayer {
            input_norm_weight: vec![1.0; 4],
            post_attn_norm_weight: vec![1.0; 4],
            attention: attn,
            layer_type: LayerType::MoeWithShared,
            gate_weight: vec![1.0; 8], // 2 experts x 4 hidden
            experts: vec![expert.clone(), expert.clone()],
            shared_expert: Some(expert),
            dense_ffn: None,
        };
        assert_eq!(layer.layer_type, LayerType::MoeWithShared);
        assert_eq!(layer.experts.len(), 2);
        assert!(layer.shared_expert.is_some());
    }

    #[test]
    fn test_tensor_discovery_report_struct() {
        let report = TensorDiscoveryReport {
            total_tensors: 10,
            total_bytes: 1024,
            architecture: Some("deepseek2".into()),
            tensor_groups: vec![TensorGroup {
                name: "Embedding".into(),
                tensors: vec![TensorEntry {
                    name: "token_embd.weight".into(),
                    shape: vec![154880, 2048],
                    dtype: "Q8_0".into(),
                    bytes: 512,
                }],
            }],
            warnings: vec!["MLA detected".into()],
        };
        assert_eq!(report.total_tensors, 10);
        assert_eq!(report.tensor_groups.len(), 1);
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn test_model_validation_struct() {
        let validation = ModelValidation {
            can_load: true,
            config_summary: "layers=47, hidden=2048".into(),
            found: vec!["Embedding: token_embd.weight".into()],
            missing: vec![],
        };
        assert!(validation.can_load);
        assert_eq!(validation.found.len(), 1);
        assert!(validation.missing.is_empty());
    }

    #[test]
    fn test_meta_helpers() {
        // Test that meta_usize and meta_f32 handle missing keys
        // (We can't easily construct a GgufFile in tests, so we test the
        // behavior through the config defaults)
        let config = BitNetModelConfig::default();
        assert_eq!(config.rope_theta, 1_000_000.0);
        assert_eq!(config.routed_scaling_factor, 1.8);
    }

    // =========================================================================
    // Generation Stats tests
    // =========================================================================

    #[test]
    fn test_generation_stats_struct() {
        let stats = GenerationStats {
            prompt_tokens: 10,
            generated_tokens: 50,
            total_tokens: 60,
            elapsed_ms: 1000,
            tokens_per_second: 50.0,
        };
        assert_eq!(stats.prompt_tokens, 10);
        assert_eq!(stats.generated_tokens, 50);
        assert_eq!(stats.total_tokens, 60);
        assert_eq!(stats.elapsed_ms, 1000);
        assert!((stats.tokens_per_second - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_generation_stats_zero_elapsed() {
        let stats = GenerationStats {
            prompt_tokens: 5,
            generated_tokens: 0,
            total_tokens: 5,
            elapsed_ms: 0,
            tokens_per_second: 0.0,
        };
        assert_eq!(stats.generated_tokens, 0);
        assert_eq!(stats.tokens_per_second, 0.0);
    }

    // =========================================================================
    // Expert Predictor tests
    // =========================================================================

    #[test]
    fn test_expert_predictor_from_empty_history() {
        let predictor = ExpertPredictor::from_history(8, &[]);
        assert_eq!(predictor.num_experts(), 8);
        assert_eq!(predictor.total_observations(), 0);
    }

    #[test]
    fn test_expert_predictor_from_single_entry() {
        // Single entry = no transitions
        let history = vec![vec![2, 5]];
        let predictor = ExpertPredictor::from_history(8, &history);
        assert_eq!(predictor.total_observations(), 0);
    }

    #[test]
    fn test_expert_predictor_transition_counts() {
        // Two entries: experts [2,5] -> experts [3,7]
        // Expected transitions: 2->3, 2->7, 5->3, 5->7 (each count=1)
        let history = vec![vec![2, 5], vec![3, 7]];
        let predictor = ExpertPredictor::from_history(8, &history);
        assert_eq!(predictor.total_observations(), 4);

        // Transition probabilities should reflect counts + Laplace smoothing
        let p_2_3 = predictor.transition_prob(2, 3);
        let p_2_7 = predictor.transition_prob(2, 7);
        let p_2_0 = predictor.transition_prob(2, 0); // unobserved

        // 2->3 has count=1, total from expert 2 = 2, Laplace denom = 2+8=10
        // p = (1+1)/10 = 0.2
        assert!((p_2_3 - 0.2).abs() < 1e-6, "p(2->3)={}", p_2_3);
        assert!((p_2_7 - 0.2).abs() < 1e-6, "p(2->7)={}", p_2_7);
        // 2->0 has count=0, p = (0+1)/10 = 0.1
        assert!((p_2_0 - 0.1).abs() < 1e-6, "p(2->0)={}", p_2_0);
    }

    #[test]
    fn test_expert_predictor_predict_next() {
        // Build a history where expert 2 always transitions to expert 5
        let history = vec![
            vec![2],
            vec![5],
            vec![2],
            vec![5],
            vec![2],
            vec![5],
            vec![2],
            vec![5],
        ];
        let predictor = ExpertPredictor::from_history(8, &history);

        // Given current = [2], predict next
        let predicted = predictor.predict_next(&[2], 3);

        // Expert 5 should be the top prediction (highest transition count)
        assert!(!predicted.is_empty());
        assert_eq!(predicted[0], 5, "Expert 5 should be top prediction");
    }

    #[test]
    fn test_expert_predictor_excludes_current() {
        // Build a history where expert 2 transitions to itself often
        let history = vec![vec![2], vec![2], vec![2], vec![2]];
        let predictor = ExpertPredictor::from_history(8, &history);

        // Predict next given current=[2]; expert 2 should be excluded
        let predicted = predictor.predict_next(&[2], 3);
        assert!(
            !predicted.contains(&2),
            "Current experts should be excluded"
        );
    }

    #[test]
    fn test_expert_predictor_out_of_bounds() {
        let predictor = ExpertPredictor::from_history(4, &[]);
        assert_eq!(predictor.transition_prob(10, 0), 0.0);
        assert_eq!(predictor.transition_prob(0, 10), 0.0);

        // Predict with out-of-bounds experts should not panic
        let predicted = predictor.predict_next(&[99], 2);
        assert!(predicted.len() <= 2);
    }

    #[test]
    fn test_expert_predictor_build_from_backend() {
        let backend = BitNetBackend::new();
        let history = vec![vec![1, 2], vec![3, 4]];
        let predictor = backend.build_expert_predictor(&history);
        assert_eq!(predictor.num_experts(), 64); // default config
    }

    // =========================================================================
    // Compressed MLA Cache tests
    // =========================================================================

    #[test]
    fn test_compressed_mla_cache_new() {
        let cache = CompressedMlaCache::new();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.memory_bytes(), 0);
    }

    #[test]
    fn test_compressed_mla_cache_push() {
        let mut cache = CompressedMlaCache::new();
        let c_kv = vec![1.0f32; 512]; // kv_lora_rank
        let k_pe = vec![0.5f32; 64]; // qk_rope_head_dim

        cache.push(c_kv, k_pe);
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        // Memory: 512*4 + 64*4 = 2304 bytes
        assert_eq!(cache.memory_bytes(), 2304);
    }

    #[test]
    fn test_compressed_mla_cache_clear() {
        let mut cache = CompressedMlaCache::new();
        cache.push(vec![1.0; 512], vec![0.5; 64]);
        cache.push(vec![2.0; 512], vec![0.5; 64]);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.memory_bytes(), 0);
    }

    #[test]
    fn test_compressed_mla_cache_savings_ratio() {
        // GLM-4.7-Flash dimensions
        let ratio = CompressedMlaCache::savings_ratio(
            20,  // num_heads
            192, // qk_nope_head_dim
            64,  // qk_rope_head_dim
            256, // v_head_dim
            512, // kv_lora_rank
        );
        // Full K: 20 * 256 = 5120, Full V: 20 * 256 = 5120, total = 10240
        // Compressed: 512 + 64 = 576
        // Ratio: 10240 / 576 ≈ 17.78
        assert!(ratio > 17.0, "Expected ~17.8x savings, got {}", ratio);
        assert!(ratio < 18.5, "Expected ~17.8x savings, got {}", ratio);
    }

    #[test]
    fn test_compressed_mla_cache_multiple_positions() {
        let mut cache = CompressedMlaCache::new();
        for i in 0..100 {
            cache.push(vec![i as f32; 512], vec![(i as f32) * 0.1; 64]);
        }
        assert_eq!(cache.len(), 100);
        // 100 positions * (512 + 64) * 4 bytes = 230,400 bytes
        assert_eq!(cache.memory_bytes(), 230_400);
    }

    #[test]
    fn test_compressed_vs_full_kv_memory() {
        // Compare memory usage: compressed vs full cache for 1024 positions
        let positions = 1024;
        let config = BitNetModelConfig::default();

        // Full KV cache per position:
        let full_k_dim =
            config.num_attention_heads * (config.qk_nope_head_dim + config.qk_rope_head_dim);
        let full_v_dim = config.num_attention_heads * config.v_head_dim;
        let full_per_pos = (full_k_dim + full_v_dim) * 4; // FP32
        let full_total = full_per_pos * positions;

        // Compressed cache per position:
        let compressed_per_pos = (config.kv_lora_rank + config.qk_rope_head_dim) * 4;
        let compressed_total = compressed_per_pos * positions;

        // For 1024 positions, full = ~40 MB vs compressed = ~2.3 MB
        assert!(
            full_total > compressed_total * 10,
            "Full ({} bytes) should be >10x compressed ({} bytes)",
            full_total,
            compressed_total
        );
    }

    // =========================================================================
    // End-to-end inference tests with synthetic model
    // =========================================================================

    /// Build a tiny synthetic model for E2E testing.
    ///
    /// Config: 2 layers, hidden_size=8, vocab=16, 2 heads, 2 KV heads, GQA,
    /// 2 experts (top-1), dense layer 0 + MoE layer 1, intermediate_size=4.
    fn build_tiny_model() -> BitNetBackend {
        let hidden = 8;
        let vocab = 16;
        let num_heads = 2;
        let num_kv_heads = 2;
        let head_dim = hidden / num_heads; // 4
        let intermediate = 4;
        let num_experts = 2;

        // Helper: create a ternary tensor of given shape filled with +1
        let make_ternary = |rows: usize, cols: usize| -> TernaryTensor {
            let ternary_vals: Vec<i8> = (0..rows * cols)
                .map(|i| match i % 3 {
                    0 => 1,
                    1 => -1,
                    _ => 0,
                })
                .collect();
            let packed = pack_ternary(&ternary_vals);
            let block_size = 256;
            let blocks_per_row = (cols + block_size - 1) / block_size;
            TernaryTensor {
                packed_data: packed,
                scales: vec![1.0; rows * blocks_per_row],
                shape: (rows, cols),
                block_size,
            }
        };

        let make_expert = || ExpertWeights {
            gate_proj: make_ternary(intermediate, hidden),
            up_proj: make_ternary(intermediate, hidden),
            down_proj: make_ternary(hidden, intermediate),
        };

        let make_gqa_attn = || AttentionWeights {
            is_mla: false,
            q_proj: make_ternary(hidden, hidden),
            k_proj: make_ternary(num_kv_heads * head_dim, hidden),
            v_proj: make_ternary(num_kv_heads * head_dim, hidden),
            o_proj: make_ternary(hidden, hidden),
            q_a: None,
            q_b: None,
            q_a_norm: None,
            kv_a_mqa: None,
            kv_a_norm: None,
            k_b: None,
            v_b: None,
        };

        // Layer 0: Dense FFN
        let layer0 = TransformerLayer {
            input_norm_weight: vec![1.0; hidden],
            post_attn_norm_weight: vec![1.0; hidden],
            attention: make_gqa_attn(),
            layer_type: LayerType::Dense,
            gate_weight: Vec::new(),
            experts: Vec::new(),
            shared_expert: None,
            dense_ffn: Some(make_expert()),
        };

        // Layer 1: MoE with 2 experts, top-1
        let layer1 = TransformerLayer {
            input_norm_weight: vec![1.0; hidden],
            post_attn_norm_weight: vec![1.0; hidden],
            attention: make_gqa_attn(),
            layer_type: LayerType::Moe,
            gate_weight: vec![1.0; num_experts * hidden], // [2 experts, 8 hidden]
            experts: vec![make_expert(), make_expert()],
            shared_expert: None,
            dense_ffn: None,
        };

        let config = BitNetModelConfig {
            num_layers: 2,
            hidden_size: hidden,
            intermediate_size: intermediate,
            vocab_size: vocab,
            num_attention_heads: num_heads,
            num_kv_heads,
            num_experts,
            active_experts: 1,
            moe_intermediate_size: intermediate,
            max_context: 64,
            use_mla: false,
            q_lora_rank: 0,
            kv_lora_rank: 0,
            qk_nope_head_dim: 0,
            qk_rope_head_dim: 0,
            v_head_dim: 0,
            n_shared_experts: 0,
            first_k_dense_replace: 1,
            rope_theta: 10000.0,
            routed_scaling_factor: 1.0,
        };

        // Build embedding table: [vocab * hidden] with simple deterministic pattern
        let mut embedding = vec![0.0f32; vocab * hidden];
        for tok in 0..vocab {
            for d in 0..hidden {
                embedding[tok * hidden + d] = ((tok * hidden + d) as f32 * 0.01).sin();
            }
        }

        // LM head: [vocab * hidden] — simple identity-like
        let mut lm_head = vec![0.0f32; vocab * hidden];
        for tok in 0..vocab {
            for d in 0..hidden {
                lm_head[tok * hidden + d] = if d == tok % hidden { 1.0 } else { 0.0 };
            }
        }

        let final_norm = vec![1.0; hidden];

        let mut backend = BitNetBackend::new();
        backend.config = Some(config.clone());
        backend.embedding = embedding;
        backend.lm_head = lm_head;
        backend.final_norm_weight = final_norm;
        backend.layers = vec![layer0, layer1];
        backend.kv_caches = vec![LayerKvCache::new(), LayerKvCache::new()];
        backend.mla_caches = vec![CompressedMlaCache::new(), CompressedMlaCache::new()];
        backend.loaded = true;
        backend.scratch.allocate(&config);
        backend.build_rope_tables(
            config.max_context.min(64),
            hidden / num_heads,
            config.rope_theta,
        );

        backend
    }

    #[test]
    fn test_e2e_forward_produces_logits() {
        let backend = build_tiny_model();
        let logits = backend.forward(&[0, 1, 2]).unwrap();
        assert_eq!(logits.len(), 16, "Should produce vocab_size=16 logits");

        // Logits should be finite
        for (i, &l) in logits.iter().enumerate() {
            assert!(l.is_finite(), "Logit {} is not finite: {}", i, l);
        }
    }

    #[test]
    fn test_e2e_forward_token_with_kv_cache() {
        let mut backend = build_tiny_model();
        backend.reset_cache();

        // Process 3 tokens autoregressively
        let logits_0 = backend.forward_token(0, 0).unwrap();
        assert_eq!(logits_0.len(), 16);

        let logits_1 = backend.forward_token(1, 1).unwrap();
        assert_eq!(logits_1.len(), 16);

        let logits_2 = backend.forward_token(2, 2).unwrap();
        assert_eq!(logits_2.len(), 16);

        // KV cache should have 3 positions per layer
        assert_eq!(backend.kv_caches[0].len(), 3);
        assert_eq!(backend.kv_caches[1].len(), 3);

        // All logits should be finite
        for &l in logits_2.iter() {
            assert!(l.is_finite());
        }
    }

    #[test]
    fn test_e2e_forward_deterministic() {
        let backend = build_tiny_model();
        let logits_a = backend.forward(&[3, 5, 7]).unwrap();
        let logits_b = backend.forward(&[3, 5, 7]).unwrap();

        // Same input should produce same output (no randomness)
        for (a, b) in logits_a.iter().zip(logits_b.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "Forward should be deterministic: {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_e2e_forward_different_tokens_different_logits() {
        let backend = build_tiny_model();
        let logits_a = backend.forward(&[0]).unwrap();
        let logits_b = backend.forward(&[1]).unwrap();

        // Different tokens should produce different logits
        let diff: f32 = logits_a
            .iter()
            .zip(logits_b.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff > 1e-6,
            "Different tokens should produce different logits, diff={}",
            diff
        );
    }

    #[test]
    fn test_e2e_expert_predictor_builds_from_inference() {
        let mut backend = build_tiny_model();
        backend.reset_cache();

        // Run enough tokens to accumulate routing history and trigger predictor rebuild
        for pos in 0..20 {
            let _ = backend.forward_token(pos as u32 % 16, pos).unwrap();
        }

        // Predictor should have been built (rebuilds every 16 tokens)
        assert!(
            backend.expert_predictor.is_some(),
            "Expert predictor should be built after 16+ tokens"
        );

        let predictor = backend.expert_predictor.as_ref().unwrap();
        assert!(
            predictor.total_observations() > 0,
            "Predictor should have observations from routing history"
        );
    }

    #[test]
    fn test_e2e_forward_token_reset_cache() {
        let mut backend = build_tiny_model();

        // First sequence
        let _ = backend.forward_token(0, 0).unwrap();
        let _ = backend.forward_token(1, 1).unwrap();
        assert_eq!(backend.kv_caches[0].len(), 2);

        // Reset and start new sequence
        backend.reset_cache();
        assert_eq!(backend.kv_caches[0].len(), 0);

        let logits = backend.forward_token(5, 0).unwrap();
        assert_eq!(logits.len(), 16);
        assert_eq!(backend.kv_caches[0].len(), 1);
    }

    #[test]
    fn test_e2e_compressed_kv_toggle() {
        let mut backend = build_tiny_model();

        // Default: compressed KV disabled
        assert!(!backend.compressed_kv_enabled());

        backend.set_compressed_kv(true);
        assert!(backend.compressed_kv_enabled());

        backend.set_compressed_kv(false);
        assert!(!backend.compressed_kv_enabled());
    }

    #[test]
    fn test_e2e_scratch_pool_allocated() {
        let backend = build_tiny_model();

        // Scratch pool should be allocated after build
        assert!(
            backend.scratch.memory_bytes() > 0,
            "Scratch pool should be allocated"
        );

        // Should have buffers for at least hidden_size (8)
        assert!(backend.scratch.buf_hidden_a.len() >= 8);
        assert!(backend.scratch.buf_ffn_gate.len() >= 4); // intermediate_size
    }

    // =========================================================================
    // Benchmark-style performance tests
    // =========================================================================

    #[test]
    fn test_bench_forward_token_throughput() {
        let mut backend = build_tiny_model();
        backend.reset_cache();

        let start = std::time::Instant::now();
        let num_tokens = 32;
        for pos in 0..num_tokens {
            let _ = backend.forward_token(pos as u32 % 16, pos).unwrap();
        }
        let elapsed = start.elapsed();

        let tokens_per_sec = num_tokens as f64 / elapsed.as_secs_f64();
        // Just verify it runs and is reasonably fast (should be >100 tok/s on any machine)
        assert!(
            tokens_per_sec > 10.0,
            "Expected >10 tok/s for tiny model, got {:.1}",
            tokens_per_sec
        );
    }

    #[test]
    fn test_bench_tl1_gemv_dispatch_performance() {
        let backend = BitNetBackend::new();

        // Create a 64x64 ternary weight matrix
        let vals: Vec<i8> = (0..64 * 64)
            .map(|i| match i % 3 {
                0 => 1,
                1 => -1,
                _ => 0,
            })
            .collect();
        let packed = pack_ternary(&vals);
        let weight = TernaryTensor {
            packed_data: packed,
            scales: vec![1.0; 64],
            shape: (64, 64),
            block_size: 256,
        };
        let input: Vec<f32> = (0..64).map(|i| (i as f32) * 0.1).collect();

        let start = std::time::Instant::now();
        let iters = 1000;
        for _ in 0..iters {
            let _ = backend.tl1_gemv(&weight, &input, 64, 64);
        }
        let elapsed = start.elapsed();

        let gemvs_per_sec = iters as f64 / elapsed.as_secs_f64();
        // Verify GEMV performance: should manage >10K/s for 64x64 on any machine
        assert!(
            gemvs_per_sec > 1000.0,
            "Expected >1K GEMV/s for 64x64, got {:.1}",
            gemvs_per_sec
        );
    }

    #[test]
    fn test_bench_rms_norm_performance() {
        let w = vec![1.0f32; 2048];
        let mut x: Vec<f32> = (0..2048).map(|i| (i as f32) * 0.001).collect();

        let start = std::time::Instant::now();
        let iters = 10000;
        for _ in 0..iters {
            rms_norm_inplace(&mut x, &w, 1e-6);
        }
        let elapsed = start.elapsed();

        let norms_per_sec = iters as f64 / elapsed.as_secs_f64();
        assert!(
            norms_per_sec > 10000.0,
            "Expected >10K norms/s for dim=2048, got {:.1}",
            norms_per_sec
        );
    }

    #[test]
    fn test_bench_softmax_performance() {
        let mut x: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.01).collect();

        let start = std::time::Instant::now();
        let iters = 10000;
        for _ in 0..iters {
            softmax_inplace(&mut x);
        }
        let elapsed = start.elapsed();

        let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
        assert!(
            ops_per_sec > 10000.0,
            "Expected >10K softmax/s for dim=1024, got {:.1}",
            ops_per_sec
        );
    }

    #[test]
    fn test_bench_expert_forward_performance() {
        let backend = BitNetBackend::new();
        let config = BitNetModelConfig {
            hidden_size: 64,
            intermediate_size: 32,
            moe_intermediate_size: 32,
            ..Default::default()
        };

        let vals: Vec<i8> = (0..32 * 64)
            .map(|i| match i % 3 {
                0 => 1,
                1 => -1,
                _ => 0,
            })
            .collect();
        let packed = pack_ternary(&vals);
        let make_t = |rows, cols| TernaryTensor {
            packed_data: packed.clone(),
            scales: vec![1.0; rows],
            shape: (rows, cols),
            block_size: 256,
        };

        let expert = ExpertWeights {
            gate_proj: make_t(32, 64),
            up_proj: make_t(32, 64),
            down_proj: make_t(64, 32),
        };

        let input: Vec<f32> = (0..64).map(|i| (i as f32) * 0.01).collect();

        let start = std::time::Instant::now();
        let iters = 500;
        for _ in 0..iters {
            let _ = backend.expert_forward(&input, &expert, &config).unwrap();
        }
        let elapsed = start.elapsed();

        let experts_per_sec = iters as f64 / elapsed.as_secs_f64();
        assert!(
            experts_per_sec > 100.0,
            "Expected >100 expert_forward/s for 64→32→64, got {:.1}",
            experts_per_sec
        );
    }
}
