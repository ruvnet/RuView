//! Metal compute pipeline management
//!
//! Handles compilation and caching of Metal compute pipelines.
//! Includes optimized M4 Pro pipelines for maximum performance.

use metal::{ComputePipelineState, Device, Library};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::error::{Result, RuvLLMError};

/// Collection of compiled Metal pipelines
pub struct MetalPipelines {
    // ============ Core Pipelines ============
    /// Flash attention pipeline (legacy)
    pub attention: ComputePipelineState,
    /// GEMM FP16 pipeline (legacy)
    pub gemm: ComputePipelineState,
    /// GEMM FP32 pipeline (legacy)
    pub gemm_f32: ComputePipelineState,
    /// RMSNorm pipeline
    pub rms_norm: ComputePipelineState,
    /// LayerNorm pipeline
    pub layer_norm: ComputePipelineState,
    /// RoPE pipeline
    pub rope: ComputePipelineState,
    /// Softmax pipeline
    pub softmax: ComputePipelineState,
    /// Element-wise add pipeline
    pub add: ComputePipelineState,
    /// Element-wise multiply pipeline
    pub mul: ComputePipelineState,
    /// SiLU activation pipeline
    pub silu: ComputePipelineState,

    // ============ M4 Pro Optimized Pipelines ============
    /// M4 Pro optimized GEMM (BM=128, BN=128, BK=32)
    pub gemm_optimized: Option<ComputePipelineState>,
    /// Fused attention with online softmax
    pub fused_attention: Option<ComputePipelineState>,
    /// Fused attention FP16
    pub fused_attention_f16: Option<ComputePipelineState>,
    /// Paged attention for KV cache
    pub paged_attention: Option<ComputePipelineState>,
    /// Fused LayerNorm + Residual
    pub fused_layernorm_residual: Option<ComputePipelineState>,
    /// Fused RMSNorm + Residual
    pub fused_rmsnorm_residual: Option<ComputePipelineState>,
    /// Fused SwiGLU activation
    pub fused_swiglu: Option<ComputePipelineState>,
    /// INT4 GEMV with dequantization
    pub int4_gemv: Option<ComputePipelineState>,
    /// INT4 GEMV SIMD optimized
    pub int4_gemv_simd: Option<ComputePipelineState>,
    /// INT4 GEMM
    pub int4_gemm: Option<ComputePipelineState>,
    /// INT8 GEMV
    pub int8_gemv: Option<ComputePipelineState>,
    /// RoPE + Attention fusion
    pub rope_then_attention: Option<ComputePipelineState>,
    /// YaRN attention (extended context)
    pub yarn_attention: Option<ComputePipelineState>,
    /// In-place Q/K RoPE application
    pub apply_rope_qk_inplace: Option<ComputePipelineState>,
}

impl MetalPipelines {
    /// Create all pipelines from a compiled library
    pub fn new(device: &Device, library: &Library) -> Result<Self> {
        Ok(Self {
            // Core pipelines (required)
            attention: Self::create_pipeline(device, library, "flash_attention")?,
            gemm: Self::create_pipeline(device, library, "gemm_f16")?,
            gemm_f32: Self::create_pipeline(device, library, "gemm_f32")?,
            rms_norm: Self::create_pipeline(device, library, "rms_norm")?,
            layer_norm: Self::create_pipeline(device, library, "layer_norm")?,
            rope: Self::create_pipeline(device, library, "apply_rope")?,
            softmax: Self::create_pipeline(device, library, "softmax")?,
            add: Self::create_pipeline(device, library, "elementwise_add")?,
            mul: Self::create_pipeline(device, library, "elementwise_mul")?,
            silu: Self::create_pipeline(device, library, "silu")?,

            // M4 Pro optimized pipelines (optional - may fail on older hardware)
            gemm_optimized: Self::try_create_pipeline(device, library, "gemm_optimized"),
            fused_attention: Self::try_create_pipeline(device, library, "fused_attention"),
            fused_attention_f16: Self::try_create_pipeline(device, library, "fused_attention_f16"),
            paged_attention: Self::try_create_pipeline(device, library, "paged_attention"),
            fused_layernorm_residual: Self::try_create_pipeline(
                device,
                library,
                "fused_layernorm_residual",
            ),
            fused_rmsnorm_residual: Self::try_create_pipeline(
                device,
                library,
                "fused_rmsnorm_residual",
            ),
            fused_swiglu: Self::try_create_pipeline(device, library, "fused_swiglu"),
            int4_gemv: Self::try_create_pipeline(device, library, "int4_gemv"),
            int4_gemv_simd: Self::try_create_pipeline(device, library, "int4_gemv_simd"),
            int4_gemm: Self::try_create_pipeline(device, library, "int4_gemm"),
            int8_gemv: Self::try_create_pipeline(device, library, "int8_gemv"),
            rope_then_attention: Self::try_create_pipeline(device, library, "rope_then_attention"),
            yarn_attention: Self::try_create_pipeline(device, library, "yarn_attention"),
            apply_rope_qk_inplace: Self::try_create_pipeline(
                device,
                library,
                "apply_rope_qk_inplace",
            ),
        })
    }

    /// Check if M4 Pro optimized pipelines are available
    pub fn has_m4_pro_optimizations(&self) -> bool {
        self.gemm_optimized.is_some() && self.fused_attention.is_some()
    }

    /// Get list of available optimized pipelines
    pub fn available_optimizations(&self) -> Vec<&'static str> {
        let mut available = Vec::new();
        if self.gemm_optimized.is_some() {
            available.push("gemm_optimized");
        }
        if self.fused_attention.is_some() {
            available.push("fused_attention");
        }
        if self.fused_attention_f16.is_some() {
            available.push("fused_attention_f16");
        }
        if self.paged_attention.is_some() {
            available.push("paged_attention");
        }
        if self.fused_layernorm_residual.is_some() {
            available.push("fused_layernorm_residual");
        }
        if self.fused_rmsnorm_residual.is_some() {
            available.push("fused_rmsnorm_residual");
        }
        if self.fused_swiglu.is_some() {
            available.push("fused_swiglu");
        }
        if self.int4_gemv.is_some() {
            available.push("int4_gemv");
        }
        if self.int4_gemv_simd.is_some() {
            available.push("int4_gemv_simd");
        }
        if self.int4_gemm.is_some() {
            available.push("int4_gemm");
        }
        if self.int8_gemv.is_some() {
            available.push("int8_gemv");
        }
        if self.rope_then_attention.is_some() {
            available.push("rope_then_attention");
        }
        if self.yarn_attention.is_some() {
            available.push("yarn_attention");
        }
        if self.apply_rope_qk_inplace.is_some() {
            available.push("apply_rope_qk_inplace");
        }
        available
    }

    /// Try to create a pipeline, returning None if it fails
    fn try_create_pipeline(
        device: &Device,
        library: &Library,
        function_name: &str,
    ) -> Option<ComputePipelineState> {
        Self::create_pipeline(device, library, function_name).ok()
    }

    /// Create a single pipeline from a function name
    fn create_pipeline(
        device: &Device,
        library: &Library,
        function_name: &str,
    ) -> Result<ComputePipelineState> {
        let function = library.get_function(function_name, None).map_err(|e| {
            RuvLLMError::Backend(format!("Failed to get function '{}': {}", function_name, e))
        })?;

        device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|e| {
                RuvLLMError::Backend(format!(
                    "Failed to create pipeline for '{}': {}",
                    function_name, e
                ))
            })
    }
}

/// Cache for dynamically compiled pipelines
pub struct PipelineCache {
    /// Device for compilation
    device: Device,
    /// Cached pipelines by source hash
    cache: RwLock<HashMap<u64, ComputePipelineState>>,
}

impl PipelineCache {
    /// Create a new pipeline cache
    pub fn new(device: Device) -> Self {
        Self {
            device,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get or compile a pipeline
    pub fn get_or_compile(
        &self,
        source: &str,
        function_name: &str,
    ) -> Result<ComputePipelineState> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        function_name.hash(&mut hasher);
        let key = hasher.finish();

        // Check cache
        {
            let cache = self.cache.read().unwrap();
            if let Some(pipeline) = cache.get(&key) {
                return Ok(pipeline.clone());
            }
        }

        // Compile
        let library = self
            .device
            .new_library_with_source(source, &metal::CompileOptions::new())
            .map_err(|e| RuvLLMError::Backend(format!("Shader compilation failed: {}", e)))?;

        let function = library
            .get_function(function_name, None)
            .map_err(|e| RuvLLMError::Backend(format!("Function not found: {}", e)))?;

        let pipeline = self
            .device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|e| RuvLLMError::Backend(format!("Pipeline creation failed: {}", e)))?;

        // Cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(key, pipeline.clone());
        }

        Ok(pipeline)
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }
}

/// Pipeline configuration for specialized kernels
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PipelineConfig {
    /// Tile size M
    pub tile_m: usize,
    /// Tile size N
    pub tile_n: usize,
    /// Tile size K
    pub tile_k: usize,
    /// Use FP16
    pub use_fp16: bool,
    /// Number of warps
    pub num_warps: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            tile_m: 64,
            tile_n: 64,
            tile_k: 32,
            use_fp16: true,
            num_warps: 4,
        }
    }
}

impl PipelineConfig {
    /// Generate specialized shader source
    pub fn generate_gemm_shader(&self) -> String {
        format!(
            r#"
#include <metal_stdlib>
using namespace metal;

#define TILE_M {}
#define TILE_N {}
#define TILE_K {}

kernel void gemm_specialized(
    device const {} *A [[buffer(0)]],
    device const {} *B [[buffer(1)]],
    device {} *C [[buffer(2)]],
    constant uint4 &dims [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]]
) {{
    // Specialized GEMM implementation
    uint M = dims.x;
    uint N = dims.y;
    uint K = dims.z;

    uint row = gid.y * TILE_M + tid.y;
    uint col = gid.x * TILE_N + tid.x;

    if (row >= M || col >= N) return;

    {} sum = 0;
    for (uint k = 0; k < K; k++) {{
        sum += A[row * K + k] * B[k * N + col];
    }}
    C[row * N + col] = sum;
}}
"#,
            self.tile_m,
            self.tile_n,
            self.tile_k,
            if self.use_fp16 { "half" } else { "float" },
            if self.use_fp16 { "half" } else { "float" },
            if self.use_fp16 { "half" } else { "float" },
            if self.use_fp16 { "half" } else { "float" },
        )
    }
}
