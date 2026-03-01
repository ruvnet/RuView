//! Metal GPU Acceleration for Apple Silicon M4 Pro
//!
//! This module provides GPU-accelerated compute shaders for LLM operations,
//! specifically optimized for Apple Silicon's Metal Performance Shaders and
//! the M4 Pro's matrix coprocessor (AMX/SME).
//!
//! ## Features
//!
//! - **Flash Attention**: Tiled attention with O(N) memory complexity
//! - **GEMM**: Optimized matrix multiplication using simdgroup_matrix
//! - **RMSNorm/LayerNorm**: Parallel normalization with warp-level reductions
//! - **RoPE**: Rotary position embedding application
//!
//! ## M4 Pro Optimizations
//!
//! - Uses `simdgroup_half8x8` for tensor core acceleration
//! - Optimized for 16KB threadgroup memory
//! - FP16 operations for 2x throughput
//! - Coalesced memory access patterns
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::metal::{MetalContext, MetalConfig};
//!
//! let ctx = MetalContext::new(MetalConfig::default())?;
//!
//! // Flash attention
//! let output = ctx.flash_attention(&q, &k, &v, &config)?;
//!
//! // Matrix multiplication
//! let c = ctx.gemm_f16(&a, &b, m, n, k)?;
//! ```

#[cfg(target_os = "macos")]
mod buffers;
#[cfg(target_os = "macos")]
mod context;
#[cfg(target_os = "macos")]
mod operations;
#[cfg(target_os = "macos")]
mod pipelines;

#[cfg(target_os = "macos")]
pub use buffers::{MetalBuffer, MetalBufferPool};
#[cfg(target_os = "macos")]
pub use context::{MetalConfig, MetalContext};
#[cfg(target_os = "macos")]
pub use operations::{
    // GEMM Metal GPU functions
    batched_gemm_metal,
    dequantize_int8,
    fp16_to_fp32,
    // FP16/Quantization utilities
    fp32_to_fp16,
    gemv_batched_metal,
    gemv_metal,
    gemv_metal_f16,
    gemv_metal_with_params,
    quantize_int8,
    verify_speculative_tokens,
    // GEMV Metal GPU functions
    GemvParams,
};
#[cfg(target_os = "macos")]
pub use pipelines::{MetalPipelines, PipelineCache};

use crate::error::{Result, RuvLLMError};
use crate::kernels::AttentionConfig;

/// Attention parameters for Metal shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AttentionParams {
    /// Number of query heads
    pub num_heads: u32,
    /// Number of key-value heads
    pub num_kv_heads: u32,
    /// Dimension per head
    pub head_dim: u32,
    /// Sequence length (query)
    pub seq_len: u32,
    /// KV sequence length
    pub kv_len: u32,
    /// Softmax scale factor
    pub scale: f32,
    /// Whether to apply causal mask
    pub causal: u32,
    /// Padding for alignment
    pub _padding: u32,
}

impl AttentionParams {
    /// Create attention params from config
    pub fn from_config(config: &AttentionConfig, seq_len: usize, kv_len: usize) -> Self {
        Self {
            num_heads: config.num_heads as u32,
            num_kv_heads: config.num_kv_heads as u32,
            head_dim: config.head_dim as u32,
            seq_len: seq_len as u32,
            kv_len: kv_len as u32,
            scale: config.effective_scale(),
            causal: config.causal as u32,
            _padding: 0,
        }
    }
}

/// GEMM parameters for Metal shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GemmParams {
    /// M dimension (rows of A/C)
    pub m: u32,
    /// N dimension (cols of B/C)
    pub n: u32,
    /// K dimension (cols of A, rows of B)
    pub k: u32,
    /// Leading dimension of A
    pub lda: u32,
    /// Leading dimension of B
    pub ldb: u32,
    /// Leading dimension of C
    pub ldc: u32,
    /// Alpha scalar
    pub alpha: f32,
    /// Beta scalar
    pub beta: f32,
}

impl GemmParams {
    /// Create GEMM params for C = alpha * A @ B + beta * C
    pub fn new(m: usize, n: usize, k: usize) -> Self {
        Self {
            m: m as u32,
            n: n as u32,
            k: k as u32,
            lda: k as u32, // Row-major
            ldb: n as u32,
            ldc: n as u32,
            alpha: 1.0,
            beta: 0.0,
        }
    }
}

/// Normalization parameters for Metal shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NormParams {
    /// Hidden dimension
    pub hidden_size: u32,
    /// Epsilon for numerical stability
    pub eps: f32,
    /// Number of elements per thread
    pub elements_per_thread: u32,
    /// Padding for alignment
    pub _padding: u32,
}

impl NormParams {
    /// Create norm params
    pub fn new(hidden_size: usize, eps: f32) -> Self {
        let elements_per_thread = (hidden_size + 255) / 256; // Distribute across 256 threads
        Self {
            hidden_size: hidden_size as u32,
            eps,
            elements_per_thread: elements_per_thread as u32,
            _padding: 0,
        }
    }
}

/// RoPE parameters for Metal shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RopeParams {
    /// Head dimension (must be even)
    pub head_dim: u32,
    /// Number of heads
    pub num_heads: u32,
    /// Position offset
    pub position: u32,
    /// Base for frequency calculation (default 10000)
    pub theta_base: f32,
}

impl RopeParams {
    /// Create RoPE params
    pub fn new(head_dim: usize, num_heads: usize, position: usize, theta_base: f32) -> Self {
        Self {
            head_dim: head_dim as u32,
            num_heads: num_heads as u32,
            position: position as u32,
            theta_base,
        }
    }
}

// ============ M4 Pro Optimized Parameter Structures ============

/// Fused Attention parameters for Flash Attention 2
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FusedAttentionParams {
    /// Number of query heads
    pub num_heads: u32,
    /// Number of key-value heads (for GQA)
    pub num_kv_heads: u32,
    /// Dimension per head
    pub head_dim: u32,
    /// Query sequence length
    pub seq_len: u32,
    /// KV sequence length
    pub kv_len: u32,
    /// Softmax scale factor (1/sqrt(head_dim))
    pub scale: f32,
    /// Whether to apply causal mask
    pub causal: u32,
    /// Block size for tiled computation
    pub block_size: u32,
}

impl FusedAttentionParams {
    /// Create fused attention params with M4 Pro optimal settings
    pub fn new(
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        seq_len: usize,
        kv_len: usize,
        causal: bool,
    ) -> Self {
        Self {
            num_heads: num_heads as u32,
            num_kv_heads: num_kv_heads as u32,
            head_dim: head_dim as u32,
            seq_len: seq_len as u32,
            kv_len: kv_len as u32,
            scale: 1.0 / (head_dim as f32).sqrt(),
            causal: causal as u32,
            block_size: 64, // Optimal for M4 Pro 16KB threadgroup memory
        }
    }
}

/// Fused LayerNorm + Residual parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FusedNormParams {
    /// Hidden dimension
    pub hidden_size: u32,
    /// Epsilon for numerical stability
    pub eps: f32,
    /// Residual scaling factor (default 1.0)
    pub residual_scale: f32,
    /// Padding for alignment
    pub _padding: u32,
}

impl FusedNormParams {
    /// Create fused norm params
    pub fn new(hidden_size: usize, eps: f32) -> Self {
        Self {
            hidden_size: hidden_size as u32,
            eps,
            residual_scale: 1.0,
            _padding: 0,
        }
    }

    /// Create fused norm params with custom residual scale
    pub fn with_residual_scale(hidden_size: usize, eps: f32, residual_scale: f32) -> Self {
        Self {
            hidden_size: hidden_size as u32,
            eps,
            residual_scale,
            _padding: 0,
        }
    }
}

/// INT4 GEMV parameters for quantized inference
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Int4GemvParams {
    /// Number of rows in matrix
    pub m: u32,
    /// Number of columns (input dimension)
    pub n: u32,
    /// Group size for quantization (typically 32 or 128)
    pub group_size: u32,
    /// Number of groups
    pub num_groups: u32,
}

impl Int4GemvParams {
    /// Create INT4 GEMV params
    pub fn new(m: usize, n: usize, group_size: usize) -> Self {
        let num_groups = (n + group_size - 1) / group_size;
        Self {
            m: m as u32,
            n: n as u32,
            group_size: group_size as u32,
            num_groups: num_groups as u32,
        }
    }
}

/// RoPE + Attention fusion parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RopeAttentionParams {
    /// Number of query heads
    pub num_heads: u32,
    /// Number of key-value heads (for GQA)
    pub num_kv_heads: u32,
    /// Head dimension
    pub head_dim: u32,
    /// Sequence length
    pub seq_len: u32,
    /// KV sequence length
    pub kv_len: u32,
    /// Position offset for RoPE
    pub position_offset: u32,
    /// RoPE base frequency
    pub rope_theta: f32,
    /// Softmax scale
    pub scale: f32,
    /// Whether to apply causal mask
    pub causal: u32,
    /// Padding for alignment
    pub _padding: [u32; 3],
}

impl RopeAttentionParams {
    /// Create RoPE + Attention fusion params
    pub fn new(
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        seq_len: usize,
        kv_len: usize,
        position_offset: usize,
        rope_theta: f32,
        causal: bool,
    ) -> Self {
        Self {
            num_heads: num_heads as u32,
            num_kv_heads: num_kv_heads as u32,
            head_dim: head_dim as u32,
            seq_len: seq_len as u32,
            kv_len: kv_len as u32,
            position_offset: position_offset as u32,
            rope_theta,
            scale: 1.0 / (head_dim as f32).sqrt(),
            causal: causal as u32,
            _padding: [0; 3],
        }
    }
}

/// YaRN attention parameters for extended context
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct YarnAttentionParams {
    /// Number of query heads
    pub num_heads: u32,
    /// Number of key-value heads
    pub num_kv_heads: u32,
    /// Head dimension
    pub head_dim: u32,
    /// Sequence length
    pub seq_len: u32,
    /// KV sequence length
    pub kv_len: u32,
    /// Position offset
    pub position_offset: u32,
    /// Base RoPE theta
    pub rope_theta: f32,
    /// YaRN attention scale
    pub attn_scale: f32,
    /// YaRN interpolation factor (context extension)
    pub yarn_scale: f32,
    /// Original context length (for YaRN scaling)
    pub original_max_position: u32,
    /// Whether to apply causal mask
    pub causal: u32,
    /// Padding for alignment
    pub _padding: u32,
}

impl YarnAttentionParams {
    /// Create YaRN attention params for extended context
    pub fn new(
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        seq_len: usize,
        kv_len: usize,
        position_offset: usize,
        rope_theta: f32,
        original_max_position: usize,
        target_max_position: usize,
        causal: bool,
    ) -> Self {
        // YaRN scale factor for context extension
        let yarn_scale = (target_max_position as f32) / (original_max_position as f32);

        Self {
            num_heads: num_heads as u32,
            num_kv_heads: num_kv_heads as u32,
            head_dim: head_dim as u32,
            seq_len: seq_len as u32,
            kv_len: kv_len as u32,
            position_offset: position_offset as u32,
            rope_theta,
            attn_scale: 1.0 / (head_dim as f32).sqrt(),
            yarn_scale,
            original_max_position: original_max_position as u32,
            causal: causal as u32,
            _padding: 0,
        }
    }
}

/// Paged attention parameters for KV cache management
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PagedAttentionParams {
    /// Number of query heads
    pub num_heads: u32,
    /// Number of key-value heads
    pub num_kv_heads: u32,
    /// Head dimension
    pub head_dim: u32,
    /// Query sequence length
    pub seq_len: u32,
    /// Block size for paging (tokens per page)
    pub block_size: u32,
    /// Number of pages in K/V cache
    pub num_pages: u32,
    /// Softmax scale
    pub scale: f32,
    /// Causal masking
    pub causal: u32,
}

impl PagedAttentionParams {
    /// Create paged attention params
    pub fn new(
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        seq_len: usize,
        block_size: usize,
        num_pages: usize,
        causal: bool,
    ) -> Self {
        Self {
            num_heads: num_heads as u32,
            num_kv_heads: num_kv_heads as u32,
            head_dim: head_dim as u32,
            seq_len: seq_len as u32,
            block_size: block_size as u32,
            num_pages: num_pages as u32,
            scale: 1.0 / (head_dim as f32).sqrt(),
            causal: causal as u32,
        }
    }
}

/// Quantization parameters for INT4/INT8 operations
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct QuantParams {
    /// Group size for quantization
    pub group_size: u32,
    /// Number of groups
    pub num_groups: u32,
    /// Zero-point offset mode (0=symmetric, 1=asymmetric)
    pub zero_point_mode: u32,
    /// Padding for alignment
    pub _padding: u32,
}

impl QuantParams {
    /// Create quantization params
    pub fn new(group_size: usize, num_elements: usize, asymmetric: bool) -> Self {
        let num_groups = (num_elements + group_size - 1) / group_size;
        Self {
            group_size: group_size as u32,
            num_groups: num_groups as u32,
            zero_point_mode: asymmetric as u32,
            _padding: 0,
        }
    }
}

/// SwiGLU activation parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SwiGLUParams {
    /// Hidden size (input dimension)
    pub hidden_size: u32,
    /// Intermediate size (gate dimension)
    pub intermediate_size: u32,
    /// Padding for alignment
    pub _padding: [u32; 2],
}

impl SwiGLUParams {
    /// Create SwiGLU params
    pub fn new(hidden_size: usize, intermediate_size: usize) -> Self {
        Self {
            hidden_size: hidden_size as u32,
            intermediate_size: intermediate_size as u32,
            _padding: [0; 2],
        }
    }
}

/// Tile sizes optimized for M4 Pro
pub mod tile_sizes {
    /// Attention tile size (fits in 16KB threadgroup memory)
    pub const ATTENTION_TILE: usize = 64;
    /// GEMM tile M dimension (legacy)
    pub const GEMM_TILE_M: usize = 64;
    /// GEMM tile N dimension (legacy)
    pub const GEMM_TILE_N: usize = 64;
    /// GEMM tile K dimension (legacy)
    pub const GEMM_TILE_K: usize = 32;
    /// Number of threads per SIMD group
    pub const SIMD_SIZE: usize = 32;
    /// Maximum threads per threadgroup
    pub const MAX_THREADS_PER_THREADGROUP: usize = 1024;

    // ============ M4 Pro Optimized Constants ============

    /// M4 Pro optimized GEMM tile M (128x128 output tiles)
    pub const M4_GEMM_TILE_M: usize = 128;
    /// M4 Pro optimized GEMM tile N
    pub const M4_GEMM_TILE_N: usize = 128;
    /// M4 Pro optimized GEMM tile K
    pub const M4_GEMM_TILE_K: usize = 32;
    /// Flash Attention 2 block size
    pub const FLASH_ATTENTION_BLOCK: usize = 64;
    /// Fused attention query block size
    pub const FUSED_ATTENTION_Q_BLOCK: usize = 64;
    /// Fused attention KV block size
    pub const FUSED_ATTENTION_KV_BLOCK: usize = 64;
    /// INT4 quantization group size
    pub const INT4_GROUP_SIZE: usize = 32;
    /// INT8 quantization group size
    pub const INT8_GROUP_SIZE: usize = 128;
    /// Warps per M4 Pro threadgroup (1024 threads / 64)
    pub const M4_WARPS_PER_BLOCK: usize = 16;
    /// Threads per warp on Metal
    pub const THREADS_PER_WARP: usize = 32;
    /// M4 Pro L1 cache size per core
    pub const M4_L1_CACHE_SIZE: usize = 16 * 1024;
    /// M4 Pro L2 cache size per core
    pub const M4_L2_CACHE_SIZE: usize = 192 * 1024;
    /// Optimal threadgroup memory for M4 Pro
    pub const M4_THREADGROUP_MEMORY: usize = 16 * 1024;
}

/// Check if Metal is available on this system
#[cfg(target_os = "macos")]
pub fn is_metal_available() -> bool {
    metal::Device::system_default().is_some()
}

#[cfg(not(target_os = "macos"))]
pub fn is_metal_available() -> bool {
    false
}

/// Get Metal device information
#[cfg(target_os = "macos")]
pub fn get_device_info() -> Option<MetalDeviceInfo> {
    metal::Device::system_default().map(|device| MetalDeviceInfo {
        name: device.name().to_string(),
        registry_id: device.registry_id(),
        max_threads_per_threadgroup: device.max_threads_per_threadgroup().width as usize,
        max_buffer_length: device.max_buffer_length() as usize,
        has_unified_memory: device.has_unified_memory(),
        recommended_max_working_set_size: device.recommended_max_working_set_size() as usize,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn get_device_info() -> Option<MetalDeviceInfo> {
    None
}

/// Metal device information
#[derive(Debug, Clone)]
pub struct MetalDeviceInfo {
    /// Device name (e.g., "Apple M4 Pro")
    pub name: String,
    /// Registry ID
    pub registry_id: u64,
    /// Maximum threads per threadgroup
    pub max_threads_per_threadgroup: usize,
    /// Maximum buffer length
    pub max_buffer_length: usize,
    /// Whether device has unified memory
    pub has_unified_memory: bool,
    /// Recommended working set size
    pub recommended_max_working_set_size: usize,
}

/// Embedded shader source code
pub mod shader_source {
    /// Flash Attention shader source
    pub const ATTENTION: &str = include_str!("shaders/attention.metal");
    /// GEMM shader source
    pub const GEMM: &str = include_str!("shaders/gemm.metal");
    /// Normalization shader source
    pub const NORM: &str = include_str!("shaders/norm.metal");
    /// RoPE shader source
    pub const ROPE: &str = include_str!("shaders/rope.metal");

    // ============ M4 Pro Optimized Shaders ============

    /// Fused Attention shader (Flash Attention 2 with online softmax)
    pub const ATTENTION_FUSED: &str = include_str!("shaders/attention_fused.metal");
    /// Fused operations shader (LayerNorm+Residual, SwiGLU, etc.)
    pub const FUSED_OPS: &str = include_str!("shaders/fused_ops.metal");
    /// Quantized operations shader (INT4/INT8 GEMV/GEMM)
    pub const QUANTIZED: &str = include_str!("shaders/quantized.metal");
    /// RoPE + Attention fusion shader (YaRN, NTK-aware)
    pub const ROPE_ATTENTION: &str = include_str!("shaders/rope_attention.metal");

    /// Combined M4 Pro optimized shader source
    pub fn all_optimized_shaders() -> String {
        format!(
            "{}\n{}\n{}\n{}",
            ATTENTION_FUSED, FUSED_OPS, QUANTIZED, ROPE_ATTENTION
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_params() {
        let config = AttentionConfig {
            num_heads: 32,
            num_kv_heads: 8,
            head_dim: 128,
            max_seq_len: 4096,
            causal: true,
            scale: 0.0,
        };

        let params = AttentionParams::from_config(&config, 1, 100);
        assert_eq!(params.num_heads, 32);
        assert_eq!(params.num_kv_heads, 8);
        assert_eq!(params.head_dim, 128);
        assert!(params.scale > 0.0);
    }

    #[test]
    fn test_gemm_params() {
        let params = GemmParams::new(64, 128, 256);
        assert_eq!(params.m, 64);
        assert_eq!(params.n, 128);
        assert_eq!(params.k, 256);
        assert_eq!(params.alpha, 1.0);
        assert_eq!(params.beta, 0.0);
    }

    #[test]
    fn test_norm_params() {
        let params = NormParams::new(4096, 1e-6);
        assert_eq!(params.hidden_size, 4096);
        assert!((params.eps - 1e-6).abs() < 1e-10);
    }

    #[test]
    fn test_rope_params() {
        let params = RopeParams::new(128, 32, 0, 10000.0);
        assert_eq!(params.head_dim, 128);
        assert_eq!(params.num_heads, 32);
        assert_eq!(params.theta_base, 10000.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_metal_available() {
        // Metal should be available on macOS
        let available = is_metal_available();
        println!("Metal available: {}", available);

        if available {
            let info = get_device_info().unwrap();
            println!("Device: {}", info.name);
            println!("Unified memory: {}", info.has_unified_memory);
        }
    }

    // ============ M4 Pro Optimized Parameter Tests ============

    #[test]
    fn test_fused_attention_params() {
        let params = FusedAttentionParams::new(32, 8, 128, 16, 2048, true);
        assert_eq!(params.num_heads, 32);
        assert_eq!(params.num_kv_heads, 8);
        assert_eq!(params.head_dim, 128);
        assert_eq!(params.seq_len, 16);
        assert_eq!(params.kv_len, 2048);
        assert_eq!(params.causal, 1);
        assert_eq!(params.block_size, 64); // M4 Pro optimal
                                           // Check scale = 1/sqrt(128) ≈ 0.0884
        assert!((params.scale - 0.0884).abs() < 0.001);
    }

    #[test]
    fn test_fused_norm_params() {
        let params = FusedNormParams::new(4096, 1e-5);
        assert_eq!(params.hidden_size, 4096);
        assert!((params.eps - 1e-5).abs() < 1e-10);
        assert!((params.residual_scale - 1.0).abs() < 1e-10);

        let params_scaled = FusedNormParams::with_residual_scale(4096, 1e-5, 0.5);
        assert!((params_scaled.residual_scale - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_int4_gemv_params() {
        let params = Int4GemvParams::new(4096, 4096, 32);
        assert_eq!(params.m, 4096);
        assert_eq!(params.n, 4096);
        assert_eq!(params.group_size, 32);
        assert_eq!(params.num_groups, 128); // 4096 / 32
    }

    #[test]
    fn test_rope_attention_params() {
        let params = RopeAttentionParams::new(32, 8, 128, 16, 2048, 100, 10000.0, true);
        assert_eq!(params.num_heads, 32);
        assert_eq!(params.num_kv_heads, 8);
        assert_eq!(params.head_dim, 128);
        assert_eq!(params.position_offset, 100);
        assert_eq!(params.rope_theta, 10000.0);
        assert_eq!(params.causal, 1);
    }

    #[test]
    fn test_yarn_attention_params() {
        // Test YaRN for 4x context extension (4096 -> 16384)
        let params = YarnAttentionParams::new(32, 8, 128, 16, 2048, 0, 10000.0, 4096, 16384, true);
        assert_eq!(params.num_heads, 32);
        assert_eq!(params.original_max_position, 4096);
        // yarn_scale = 16384 / 4096 = 4.0
        assert!((params.yarn_scale - 4.0).abs() < 1e-5);
    }

    #[test]
    fn test_paged_attention_params() {
        let params = PagedAttentionParams::new(32, 8, 128, 16, 64, 32, true);
        assert_eq!(params.num_heads, 32);
        assert_eq!(params.num_kv_heads, 8);
        assert_eq!(params.block_size, 64);
        assert_eq!(params.num_pages, 32);
        assert_eq!(params.causal, 1);
    }

    #[test]
    fn test_quant_params() {
        let params = QuantParams::new(32, 4096, false);
        assert_eq!(params.group_size, 32);
        assert_eq!(params.num_groups, 128); // 4096 / 32
        assert_eq!(params.zero_point_mode, 0); // symmetric

        let params_asym = QuantParams::new(128, 4096, true);
        assert_eq!(params_asym.group_size, 128);
        assert_eq!(params_asym.num_groups, 32); // 4096 / 128
        assert_eq!(params_asym.zero_point_mode, 1); // asymmetric
    }

    #[test]
    fn test_swiglu_params() {
        let params = SwiGLUParams::new(4096, 11008);
        assert_eq!(params.hidden_size, 4096);
        assert_eq!(params.intermediate_size, 11008);
    }

    #[test]
    fn test_m4_pro_tile_sizes() {
        // Verify M4 Pro optimized constants
        assert_eq!(tile_sizes::M4_GEMM_TILE_M, 128);
        assert_eq!(tile_sizes::M4_GEMM_TILE_N, 128);
        assert_eq!(tile_sizes::M4_GEMM_TILE_K, 32);
        assert_eq!(tile_sizes::FLASH_ATTENTION_BLOCK, 64);
        assert_eq!(tile_sizes::INT4_GROUP_SIZE, 32);
        assert_eq!(tile_sizes::M4_THREADGROUP_MEMORY, 16 * 1024);
        assert_eq!(tile_sizes::MAX_THREADS_PER_THREADGROUP, 1024);
    }
}
