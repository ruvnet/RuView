//! NEON-Optimized LLM Kernels for Mac M4 Pro
//!
//! This module provides highly optimized SIMD kernels for LLM operations,
//! specifically tuned for Apple Silicon (M1/M2/M3/M4) using ARM NEON intrinsics.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvllm::kernels::{
//!     flash_attention_neon, apply_rope_neon, rms_norm_neon,
//!     AttentionConfig, is_neon_available,
//! };
//!
//! // Check NEON availability
//! assert!(is_neon_available(), "NEON required for optimal performance");
//!
//! // Configure attention
//! let config = AttentionConfig {
//!     num_heads: 32,
//!     num_kv_heads: 8,  // GQA with 4:1 ratio
//!     head_dim: 128,
//!     causal: true,
//!     ..Default::default()
//! };
//!
//! // Flash attention with NEON SIMD
//! let output = flash_attention_neon(
//!     &query, &key, &value,
//!     config.effective_scale(),
//!     config.causal
//! );
//!
//! // Apply RoPE to query/key tensors
//! apply_rope_neon(&mut qk, &positions, config.head_dim, 10000.0);
//!
//! // RMSNorm normalization
//! rms_norm_neon(&mut hidden, &weight, 1e-6);
//! ```
//!
//! ## Kernel Categories
//!
//! - [`attention`]: Flash Attention 2, Paged Attention, MQA/GQA
//! - [`rope`]: Rotary Position Embeddings (RoPE)
//! - [`norm`]: RMSNorm, LayerNorm
//! - [`matmul`]: Batched GEMM operations
//! - [`quantized`]: INT8/INT4 quantized inference kernels
//! - [`activations`]: Vectorized SiLU, GELU, ReLU, Softmax
//!
//! ## Performance Characteristics
//!
//! | Kernel | Sequence Length | Throughput | vs. Naive |
//! |--------|-----------------|------------|-----------|
//! | `flash_attention_neon` | 4096 | 2.5 GFLOPS | 3.2x |
//! | `paged_attention_neon` | 8192+ | 2.1 GFLOPS | 2.8x |
//! | `rms_norm_neon` | Any | 4.8 GFLOPS | 4.1x |
//! | `gemm_neon` | 4096x4096 | 1.2 GFLOPS | 2.4x |
//! | `silu` | Any | 5.2 GFLOPS | 3.5x |
//! | `gelu` | Any | 4.5 GFLOPS | 3.2x |
//! | `softmax` | Any | 3.8 GFLOPS | 2.8x |
//!
//! ## Performance Optimizations
//!
//! All kernels implement:
//! - 4x loop unrolling for instruction-level parallelism
//! - FMA instructions for improved throughput
//! - Pointer caching to reduce address calculations
//! - Efficient horizontal reductions via `vaddvq_f32`
//! - Software prefetching for large tensors
//!
//! ## Memory Layout
//!
//! Kernels expect contiguous memory in the following layouts:
//!
//! - **Query/Key/Value**: `[batch, seq_len, num_heads, head_dim]`
//! - **KV Cache**: `[batch, num_kv_heads, seq_len, head_dim]`
//! - **Hidden states**: `[batch, seq_len, hidden_dim]`

pub mod activations;
pub mod attention;
pub mod matmul;
pub mod norm;
pub mod quantized;
pub mod rope;

// Apple Accelerate framework integration (macOS only)
#[cfg(any(target_os = "macos", doc))]
pub mod accelerate;

// Apple Neural Engine (ANE) optimized operations
// Uses BNNS (Basic Neural Network Subroutines) which routes to ANE on macOS.
// Decision-logic functions (should_use_ane, get_ane_recommendation, etc.) and
// platform-fallback stubs are available on all targets so that tests and
// cross-platform code can reference the module unconditionally.
pub mod ane_ops;

// Re-exports for convenience
pub use attention::{
    flash_attention_auto,
    // TD-009: Zero-allocation attention functions and scratch buffers
    flash_attention_into,
    flash_attention_neon,
    flash_attention_v2,
    flash_attention_with_scratch,
    grouped_query_attention_neon,
    multi_query_attention_neon,
    paged_attention_neon,
    select_block_size,
    AttentionScratch,
    PagedKvCache,
    BLOCK_SIZE_LARGE,
    BLOCK_SIZE_MEDIUM,
    BLOCK_SIZE_SMALL,
};
// Thread-local scratch buffer for zero-allocation attention (non-WASM only)
#[cfg(not(target_arch = "wasm32"))]
pub use attention::THREAD_LOCAL_SCRATCH;
#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
pub use attention::{
    grouped_query_attention_parallel, multi_head_attention_parallel, multi_query_attention_parallel,
};
pub use matmul::{batched_gemm_neon, gemm_neon, gemv_neon};
#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
pub use matmul::{
    batched_gemm_parallel, configure_thread_pool, gemm_parallel, gemv_parallel, get_physical_cores,
};
pub use norm::{layer_norm_neon, rms_norm_neon};
pub use quantized::{
    dequantize_int4, dequantize_int8, int4_gemv_neon, int8_gemv_neon, q4k_gemv_neon,
    quantize_to_int4, quantize_to_int8, quantize_to_q4k, BlockQ4K, QuantizedInt4, QuantizedInt8,
    INT4_BLOCK_SIZE, Q4K_SUPER_BLOCK_SIZE,
};
pub use rope::{apply_rope_neon, precompute_rope_tables, RopeConfig};

// Activation function exports
pub use activations::{
    batch_gelu, batch_silu, batch_softmax, gelu, gelu_exact, gelu_vec, leaky_relu, relu, relu_vec,
    silu, silu_vec, softmax, softmax_temperature, softmax_vec,
};

// Accelerate framework exports (macOS only)
#[cfg(all(target_os = "macos", feature = "accelerate"))]
pub use accelerate::{
    axpy_accelerate, dot_accelerate, gemm_accelerate, gemv_accelerate, gemv_scaled_accelerate,
    gemv_transpose_accelerate, is_accelerate_available, scal_accelerate, should_use_accelerate,
    MatrixLayout,
};

// Fallback availability check for non-macOS platforms
#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn is_accelerate_available() -> bool {
    false
}

// ANE (Apple Neural Engine) ops exports (macOS only with coreml feature)
#[cfg(all(target_os = "macos", feature = "coreml"))]
pub use ane_ops::{
    batched_matmul_ane,
    gelu_ane,
    gelu_auto,
    // Strategy recommendations (M4 Pro optimized)
    get_ane_recommendation,
    // Availability checks
    is_ane_available,
    layer_norm_ane,
    layer_norm_auto,
    // Direct ANE operations
    matmul_ane,
    // Auto-dispatch functions
    matmul_auto,
    rms_norm_ane,
    rms_norm_auto,
    should_use_ane,
    should_use_ane_activation,
    should_use_ane_matmul,
    silu_ane,
    silu_auto,
    softmax_ane,
    softmax_auto,
    AneRecommendation,
};

// Re-export ANE availability check for all platforms without coreml feature
// (the ane_ops module is now unconditionally available)
#[cfg(not(all(target_os = "macos", feature = "coreml")))]
pub use ane_ops::is_ane_available;

/// SIMD lane width for NEON (128-bit = 4 floats).
///
/// ARM NEON registers are 128 bits wide, holding 4 single-precision floats.
/// This constant is used for loop unrolling and vectorization decisions.
pub const NEON_LANE_WIDTH: usize = 4;

/// Optimal unroll factor for M4 Pro's 6-wide superscalar core.
///
/// The M4 Pro can execute up to 6 operations per cycle. Using a 4x unroll
/// factor with FMA instructions achieves near-optimal utilization.
pub const UNROLL_FACTOR: usize = 4;

/// Prefetch distance in cache lines (64 bytes = 16 floats)
pub const PREFETCH_DISTANCE: usize = 64;

/// Check if NEON is available at runtime
#[inline(always)]
pub fn is_neon_available() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        true // NEON is always available on aarch64
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        false
    }
}

/// Kernel configuration for attention operations.
///
/// Configures the attention mechanism including head counts, dimensions,
/// and masking behavior. Supports both standard multi-head attention and
/// grouped-query attention (GQA).
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::kernels::AttentionConfig;
///
/// // Standard Mistral-7B configuration with GQA
/// let config = AttentionConfig {
///     num_heads: 32,
///     num_kv_heads: 8,  // 4:1 GQA ratio
///     head_dim: 128,
///     max_seq_len: 4096,
///     causal: true,
///     scale: 0.0,  // Auto-computed as 1/sqrt(head_dim)
/// };
///
/// assert_eq!(config.gqa_ratio(), 4);
/// assert!((config.effective_scale() - 0.0884).abs() < 0.001);
/// ```
///
/// # GQA (Grouped-Query Attention)
///
/// GQA reduces memory usage by sharing key-value heads across query heads:
///
/// | GQA Ratio | KV Memory | Quality |
/// |-----------|-----------|---------|
/// | 1:1 (MHA) | 100% | Best |
/// | 4:1 | 25% | Excellent |
/// | 8:1 | 12.5% | Good |
#[derive(Debug, Clone, Copy)]
pub struct AttentionConfig {
    /// Number of query heads
    pub num_heads: usize,
    /// Number of key-value heads (for GQA)
    pub num_kv_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Whether to use causal masking
    pub causal: bool,
    /// Softmax scale factor (typically 1/sqrt(head_dim))
    pub scale: f32,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            num_heads: 32,
            num_kv_heads: 8,
            head_dim: 128,
            max_seq_len: 4096,
            causal: true,
            scale: 0.0, // Will be computed from head_dim if 0
        }
    }
}

impl AttentionConfig {
    /// Get the effective scale (computes from head_dim if not set)
    #[inline(always)]
    pub fn effective_scale(&self) -> f32 {
        if self.scale == 0.0 {
            1.0 / (self.head_dim as f32).sqrt()
        } else {
            self.scale
        }
    }

    /// Get the GQA ratio (num_heads / num_kv_heads)
    #[inline(always)]
    pub fn gqa_ratio(&self) -> usize {
        self.num_heads / self.num_kv_heads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_config() {
        let config = AttentionConfig::default();
        assert_eq!(config.gqa_ratio(), 4);
        assert!((config.effective_scale() - 0.088388).abs() < 0.001);
    }

    #[test]
    fn test_neon_available() {
        #[cfg(target_arch = "aarch64")]
        assert!(is_neon_available());
    }
}
