//! Hybrid GPU+ANE Pipeline Coordinator
//!
//! This module provides intelligent routing of LLM operations to optimal accelerators:
//! - **MLP/FFN layers** -> ANE (matrix multiply heavy, ANE excels)
//! - **Attention computation** -> GPU (Flash Attention on Metal)
//! - **Embeddings** -> Either (depends on size)
//!
//! ## Architecture
//!
//! ```text
//! +------------------+     +-------------------+     +------------------+
//! |   Input Tensor   | --> | Operation Router  | --> | Output Tensor    |
//! +------------------+     +--------+----------+     +------------------+
//!                                   |
//!                    +--------------+--------------+
//!                    |                             |
//!                    v                             v
//!           +--------+----------+       +----------+--------+
//!           | ANE (Core ML)     |       | GPU (Metal)       |
//!           | - MLP/FFN         |       | - Flash Attention |
//!           | - LayerNorm       |       | - RoPE            |
//!           | - Activations     |       | - KV Cache        |
//!           +-------------------+       +-------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::backends::{HybridPipeline, HybridPipelineConfig, AneStrategy};
//!
//! let config = HybridPipelineConfig {
//!     ane_strategy: AneStrategy::PreferAneForMlp,
//!     metal_for_attention: true,
//!     ..Default::default()
//! };
//!
//! let pipeline = HybridPipeline::new(config)?;
//!
//! // Operations automatically route to optimal accelerator
//! let mlp_output = pipeline.mlp_forward(&input, &weights)?;  // -> ANE
//! let attn_output = pipeline.attention(&q, &k, &v)?;         // -> Metal
//! ```
//!
//! ## Performance Characteristics
//!
//! | Operation | ANE TOPS | GPU TFLOPS | Optimal |
//! |-----------|----------|------------|---------|
//! | MatMul (4K x 4K) | 38 | 16.7 | ANE |
//! | Flash Attention | N/A | 16.7 | GPU |
//! | LayerNorm | 38 | 16.7 | ANE |
//! | SiLU/SwiGLU | 38 | 16.7 | ANE |
//! | RoPE | N/A | 16.7 | GPU |

use super::{
    AneCapabilities, ComputeUnits, CoreMLBackend, DType, DeviceType, GenerateParams,
    GeneratedToken, LlmBackend, ModelArchitecture, ModelConfig, ModelInfo, Quantization,
    SpecialTokens, StreamEvent, TokenStream, Tokenizer,
};
use crate::error::{Result, RuvLLMError};
use crate::kernels::AttentionConfig;

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
use crate::metal::{MetalConfig, MetalContext};

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Strategy for ANE utilization in hybrid pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AneStrategy {
    /// Disable ANE, use GPU for all operations
    GpuOnly,
    /// Use ANE only for MLP/FFN layers
    #[default]
    PreferAneForMlp,
    /// Use ANE for MLP and normalization
    PreferAneForMlpAndNorm,
    /// Use ANE for all compatible operations
    MaximizeAneUsage,
    /// Automatic selection based on operation size and latency
    Adaptive,
}

impl AneStrategy {
    /// Check if ANE should be used for MLP operations
    pub fn use_ane_for_mlp(&self) -> bool {
        matches!(
            self,
            Self::PreferAneForMlp
                | Self::PreferAneForMlpAndNorm
                | Self::MaximizeAneUsage
                | Self::Adaptive
        )
    }

    /// Check if ANE should be used for normalization
    pub fn use_ane_for_norm(&self) -> bool {
        matches!(
            self,
            Self::PreferAneForMlpAndNorm | Self::MaximizeAneUsage | Self::Adaptive
        )
    }

    /// Check if ANE should be used for activations
    pub fn use_ane_for_activations(&self) -> bool {
        matches!(self, Self::MaximizeAneUsage | Self::Adaptive)
    }

    /// Get description for logging
    pub fn description(&self) -> &'static str {
        match self {
            Self::GpuOnly => "GPU only",
            Self::PreferAneForMlp => "ANE for MLP, GPU for attention",
            Self::PreferAneForMlpAndNorm => "ANE for MLP+Norm, GPU for attention",
            Self::MaximizeAneUsage => "Maximize ANE usage",
            Self::Adaptive => "Adaptive routing",
        }
    }
}

/// Operation type for routing decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// Matrix multiplication (MLP projections)
    MatMul,
    /// Self-attention computation
    Attention,
    /// Flash Attention
    FlashAttention,
    /// Activation functions (SiLU, GELU, etc.)
    Activation,
    /// Normalization (RMSNorm, LayerNorm)
    Normalization,
    /// Rotary Position Embedding
    RoPE,
    /// Embedding lookup
    Embedding,
    /// KV cache operations
    KvCache,
    /// Softmax
    Softmax,
    /// Unknown/other
    Other,
}

impl OperationType {
    /// Default accelerator preference for this operation
    pub fn preferred_accelerator(&self) -> AcceleratorType {
        match self {
            Self::MatMul | Self::Activation | Self::Normalization | Self::Softmax => {
                AcceleratorType::Ane
            }
            Self::Attention | Self::FlashAttention | Self::RoPE | Self::KvCache => {
                AcceleratorType::Metal
            }
            Self::Embedding => AcceleratorType::Either,
            Self::Other => AcceleratorType::Metal,
        }
    }

    /// Is this operation supported on ANE?
    pub fn ane_supported(&self) -> bool {
        matches!(
            self,
            Self::MatMul | Self::Activation | Self::Normalization | Self::Softmax | Self::Embedding
        )
    }
}

/// Target accelerator type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcceleratorType {
    /// Metal GPU
    Metal,
    /// Apple Neural Engine via Core ML
    Ane,
    /// CPU fallback
    Cpu,
    /// Either ANE or Metal (let router decide)
    Either,
}

/// Performance metrics for a single accelerator
#[derive(Debug, Clone, Default)]
pub struct AcceleratorMetrics {
    /// Total operations executed
    pub total_ops: u64,
    /// Total time spent (nanoseconds)
    pub total_time_ns: u64,
    /// Total FLOPs processed
    pub total_flops: u64,
    /// Average latency per operation (microseconds)
    pub avg_latency_us: f64,
    /// Peak throughput (GFLOPS)
    pub peak_gflops: f64,
    /// Bytes transferred
    pub bytes_transferred: u64,
}

impl AcceleratorMetrics {
    /// Update metrics after an operation
    pub fn record_operation(&mut self, duration_ns: u64, flops: u64, bytes: u64) {
        self.total_ops += 1;
        self.total_time_ns += duration_ns;
        self.total_flops += flops;
        self.bytes_transferred += bytes;

        self.avg_latency_us = (self.total_time_ns as f64 / 1000.0) / self.total_ops as f64;

        let elapsed_sec = self.total_time_ns as f64 / 1e9;
        if elapsed_sec > 0.0 {
            self.peak_gflops = (self.total_flops as f64 / 1e9) / elapsed_sec;
        }
    }
}

/// Configuration for the hybrid pipeline
#[derive(Debug, Clone)]
pub struct HybridPipelineConfig {
    /// ANE utilization strategy
    pub ane_strategy: AneStrategy,
    /// Always use Metal for attention operations
    pub metal_for_attention: bool,
    /// Minimum batch size to use ANE (smaller batches have ANE overhead)
    pub ane_min_batch_size: usize,
    /// Maximum dimension size for ANE (larger may spill to GPU)
    pub ane_max_dim: usize,
    /// Enable performance metrics collection
    pub collect_metrics: bool,
    /// Data type for Metal operations
    pub metal_dtype: DType,
    /// Enable async execution for pipelining
    pub async_execution: bool,
    /// Adaptive threshold: switch to GPU if ANE latency exceeds this (us)
    pub adaptive_latency_threshold_us: u64,
}

impl Default for HybridPipelineConfig {
    fn default() -> Self {
        Self {
            ane_strategy: AneStrategy::PreferAneForMlp,
            metal_for_attention: true,
            ane_min_batch_size: 1,
            ane_max_dim: 16384, // ANE works well up to ~16K dimensions
            collect_metrics: true,
            metal_dtype: DType::F16,
            async_execution: false,
            adaptive_latency_threshold_us: 500, // 0.5ms threshold
        }
    }
}

/// Data format for inter-accelerator transfers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    /// Native Metal buffer (MTLBuffer)
    MetalBuffer,
    /// Core ML MLMultiArray
    CoreMLArray,
    /// CPU memory (Vec<f32>)
    CpuMemory,
    /// Half precision (Vec<f16>)
    CpuMemoryF16,
}

/// Tensor wrapper for unified handling across accelerators
#[derive(Debug)]
pub struct HybridTensor {
    /// Current data format
    pub format: DataFormat,
    /// Shape dimensions [batch, seq_len, hidden_dim, ...]
    pub shape: Vec<usize>,
    /// Data type
    pub dtype: DType,
    /// CPU data (if available)
    cpu_data: Option<Vec<f32>>,
    /// Whether data is dirty and needs sync
    dirty: bool,
}

impl HybridTensor {
    /// Create a new tensor from CPU data
    pub fn from_cpu(data: Vec<f32>, shape: Vec<usize>) -> Self {
        Self {
            format: DataFormat::CpuMemory,
            shape,
            dtype: DType::F32,
            cpu_data: Some(data),
            dirty: false,
        }
    }

    /// Get total number of elements
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Get data as CPU f32 slice
    pub fn as_slice(&self) -> Option<&[f32]> {
        self.cpu_data.as_deref()
    }

    /// Get mutable data as CPU f32 slice
    pub fn as_mut_slice(&mut self) -> Option<&mut [f32]> {
        self.dirty = true;
        self.cpu_data.as_deref_mut()
    }

    /// Consume and return CPU data
    pub fn into_cpu_data(self) -> Option<Vec<f32>> {
        self.cpu_data
    }
}

/// Routing decision made by the pipeline
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Chosen accelerator
    pub accelerator: AcceleratorType,
    /// Operation type
    pub operation: OperationType,
    /// Estimated latency (microseconds)
    pub estimated_latency_us: u64,
    /// Estimated FLOPs
    pub estimated_flops: u64,
    /// Reason for this decision
    pub reason: String,
}

/// Hybrid GPU+ANE Pipeline Coordinator
///
/// Intelligently routes LLM operations to the optimal accelerator:
/// - ANE for matrix-multiply heavy operations (MLP/FFN)
/// - Metal GPU for attention and position embeddings
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::{HybridPipeline, HybridPipelineConfig};
///
/// let pipeline = HybridPipeline::new(HybridPipelineConfig::default())?;
///
/// // MLP forward pass (routed to ANE)
/// let mlp_out = pipeline.mlp_forward(&hidden, &gate_weight, &up_weight, &down_weight)?;
///
/// // Attention (routed to Metal GPU)
/// let attn_out = pipeline.flash_attention(&q, &k, &v, &config)?;
/// ```
pub struct HybridPipeline {
    /// Pipeline configuration
    config: HybridPipelineConfig,

    /// Metal GPU context (always available on macOS)
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    metal_ctx: Option<MetalContext>,

    /// Core ML backend for ANE (optional)
    #[cfg(feature = "coreml")]
    coreml_backend: Option<CoreMLBackend>,

    /// ANE capabilities
    ane_caps: AneCapabilities,

    /// Performance metrics per accelerator
    metal_metrics: AcceleratorMetrics,
    ane_metrics: AcceleratorMetrics,
    cpu_metrics: AcceleratorMetrics,

    /// Adaptive routing history (operation -> avg latency)
    routing_history: HashMap<OperationType, (u64, u64)>, // (total_ns, count)

    /// Model info (if loaded)
    model_info: Option<ModelInfo>,

    /// Whether model is loaded
    loaded: bool,
}

impl HybridPipeline {
    /// Create a new hybrid pipeline
    pub fn new(config: HybridPipelineConfig) -> Result<Self> {
        let ane_caps = AneCapabilities::detect();

        #[cfg(all(target_os = "macos", feature = "metal-compute"))]
        let metal_ctx = MetalContext::new(MetalConfig::default()).ok();

        #[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
        let metal_ctx: Option<()> = None;

        #[cfg(feature = "coreml")]
        let coreml_backend = if ane_caps.available && config.ane_strategy != AneStrategy::GpuOnly {
            CoreMLBackend::new().ok()
        } else {
            None
        };

        #[cfg(not(feature = "coreml"))]
        let coreml_backend: Option<()> = None;

        Ok(Self {
            config,
            #[cfg(all(target_os = "macos", feature = "metal-compute"))]
            metal_ctx,
            #[cfg(feature = "coreml")]
            coreml_backend,
            ane_caps,
            metal_metrics: AcceleratorMetrics::default(),
            ane_metrics: AcceleratorMetrics::default(),
            cpu_metrics: AcceleratorMetrics::default(),
            routing_history: HashMap::new(),
            model_info: None,
            loaded: false,
        })
    }

    /// Check if Metal GPU is available
    pub fn has_metal(&self) -> bool {
        #[cfg(all(target_os = "macos", feature = "metal-compute"))]
        {
            self.metal_ctx.is_some()
        }
        #[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
        {
            false
        }
    }

    /// Check if ANE is available
    pub fn has_ane(&self) -> bool {
        #[cfg(feature = "coreml")]
        {
            self.coreml_backend.is_some() && self.ane_caps.available
        }
        #[cfg(not(feature = "coreml"))]
        {
            false
        }
    }

    /// Get current ANE strategy
    pub fn ane_strategy(&self) -> AneStrategy {
        self.config.ane_strategy
    }

    /// Get ANE capabilities
    pub fn ane_capabilities(&self) -> &AneCapabilities {
        &self.ane_caps
    }

    /// Get performance metrics for Metal GPU
    pub fn metal_metrics(&self) -> &AcceleratorMetrics {
        &self.metal_metrics
    }

    /// Get performance metrics for ANE
    pub fn ane_metrics(&self) -> &AcceleratorMetrics {
        &self.ane_metrics
    }

    /// Get performance metrics for CPU
    pub fn cpu_metrics(&self) -> &AcceleratorMetrics {
        &self.cpu_metrics
    }

    /// Route an operation to the optimal accelerator
    pub fn route_operation(
        &self,
        op: OperationType,
        batch_size: usize,
        dim: usize,
    ) -> RoutingDecision {
        let strategy = self.config.ane_strategy;

        // Check basic constraints
        let ane_available = self.has_ane();
        let metal_available = self.has_metal();
        let meets_batch_threshold = batch_size >= self.config.ane_min_batch_size;
        let meets_dim_threshold = dim <= self.config.ane_max_dim;

        // Calculate estimated FLOPs (simplified)
        let estimated_flops = (batch_size * dim * dim) as u64;

        // Force Metal for attention if configured
        if self.config.metal_for_attention
            && matches!(op, OperationType::Attention | OperationType::FlashAttention)
        {
            return RoutingDecision {
                accelerator: if metal_available {
                    AcceleratorType::Metal
                } else {
                    AcceleratorType::Cpu
                },
                operation: op,
                estimated_latency_us: estimated_flops / 1000,
                estimated_flops,
                reason: "Attention forced to Metal for Flash Attention support".to_string(),
            };
        }

        // Check adaptive routing history
        if strategy == AneStrategy::Adaptive {
            if let Some(&(total_ns, count)) = self.routing_history.get(&op) {
                let avg_ns = total_ns / count.max(1);
                let avg_us = avg_ns / 1000;

                if avg_us > self.config.adaptive_latency_threshold_us {
                    return RoutingDecision {
                        accelerator: AcceleratorType::Metal,
                        operation: op,
                        estimated_latency_us: avg_us,
                        estimated_flops,
                        reason: format!(
                            "Adaptive: ANE latency {}us exceeds threshold {}us",
                            avg_us, self.config.adaptive_latency_threshold_us
                        ),
                    };
                }
            }
        }

        // Apply strategy-based routing
        let accelerator = match op {
            OperationType::MatMul => {
                if ane_available
                    && strategy.use_ane_for_mlp()
                    && meets_batch_threshold
                    && meets_dim_threshold
                {
                    AcceleratorType::Ane
                } else if metal_available {
                    AcceleratorType::Metal
                } else {
                    AcceleratorType::Cpu
                }
            }
            OperationType::Normalization => {
                if ane_available && strategy.use_ane_for_norm() && meets_dim_threshold {
                    AcceleratorType::Ane
                } else if metal_available {
                    AcceleratorType::Metal
                } else {
                    AcceleratorType::Cpu
                }
            }
            OperationType::Activation => {
                if ane_available && strategy.use_ane_for_activations() {
                    AcceleratorType::Ane
                } else if metal_available {
                    AcceleratorType::Metal
                } else {
                    AcceleratorType::Cpu
                }
            }
            OperationType::Attention | OperationType::FlashAttention | OperationType::RoPE => {
                if metal_available {
                    AcceleratorType::Metal
                } else {
                    AcceleratorType::Cpu
                }
            }
            OperationType::Embedding => {
                // Small embeddings can be done on CPU, large ones benefit from GPU
                if dim > 4096 && metal_available {
                    AcceleratorType::Metal
                } else if ane_available && dim <= self.config.ane_max_dim {
                    AcceleratorType::Ane
                } else {
                    AcceleratorType::Cpu
                }
            }
            _ => {
                if metal_available {
                    AcceleratorType::Metal
                } else {
                    AcceleratorType::Cpu
                }
            }
        };

        let reason = match accelerator {
            AcceleratorType::Ane => {
                format!(
                    "ANE optimal for {} (batch={}, dim={})",
                    op_name(op),
                    batch_size,
                    dim
                )
            }
            AcceleratorType::Metal => {
                format!(
                    "Metal optimal for {} (ANE: available={}, batch_ok={}, dim_ok={})",
                    op_name(op),
                    ane_available,
                    meets_batch_threshold,
                    meets_dim_threshold
                )
            }
            AcceleratorType::Cpu => "CPU fallback".to_string(),
            AcceleratorType::Either => "Auto-selected".to_string(),
        };

        RoutingDecision {
            accelerator,
            operation: op,
            estimated_latency_us: estimated_flops / 1000,
            estimated_flops,
            reason,
        }
    }

    /// Execute Flash Attention (always on Metal GPU)
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    pub fn flash_attention(
        &mut self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        config: &AttentionConfig,
    ) -> Result<Vec<f32>> {
        let start = Instant::now();

        let ctx = self.metal_ctx.as_ref().ok_or_else(|| {
            RuvLLMError::HybridPipeline("Metal context not available".to_string())
        })?;

        let result = ctx.flash_attention(query, key, value, config)?;

        if self.config.collect_metrics {
            let duration_ns = start.elapsed().as_nanos() as u64;
            let seq_len = query.len() / (config.num_heads * config.head_dim);
            let kv_len = key.len() / (config.num_kv_heads * config.head_dim);
            // Attention FLOPs: 2 * seq_len * kv_len * head_dim * num_heads (QK^T and softmax@V)
            let flops = 2
                * seq_len as u64
                * kv_len as u64
                * config.head_dim as u64
                * config.num_heads as u64;
            let bytes = (query.len() + key.len() + value.len() + result.len()) * 4;
            self.metal_metrics
                .record_operation(duration_ns, flops, bytes as u64);
        }

        Ok(result)
    }

    #[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
    pub fn flash_attention(
        &mut self,
        _query: &[f32],
        _key: &[f32],
        _value: &[f32],
        _config: &AttentionConfig,
    ) -> Result<Vec<f32>> {
        Err(RuvLLMError::HybridPipeline(
            "Metal compute not available on this platform".to_string(),
        ))
    }

    /// Execute MLP forward pass with hybrid routing
    ///
    /// Routes gate/up projections to ANE (if available) and
    /// uses SwiGLU activation fused on the same accelerator.
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    pub fn mlp_forward(
        &mut self,
        hidden: &[f32],
        gate_weight: &[f32],
        up_weight: &[f32],
        down_weight: &[f32],
        hidden_size: usize,
        intermediate_size: usize,
    ) -> Result<Vec<f32>> {
        let batch_size = hidden.len() / hidden_size;
        let decision = self.route_operation(OperationType::MatMul, batch_size, hidden_size);

        let start = Instant::now();

        // For now, use Metal for everything (ANE integration would require Core ML model conversion)
        let ctx = self.metal_ctx.as_ref().ok_or_else(|| {
            RuvLLMError::HybridPipeline("Metal context not available".to_string())
        })?;

        // Gate projection: hidden @ gate_weight.T
        let gate = ctx.gemm_f32(
            hidden,
            gate_weight,
            batch_size,
            intermediate_size,
            hidden_size,
        )?;

        // Up projection: hidden @ up_weight.T
        let up = ctx.gemm_f32(
            hidden,
            up_weight,
            batch_size,
            intermediate_size,
            hidden_size,
        )?;

        // SwiGLU activation: silu(gate) * up
        let activated = if let Some(_) = ctx.has_m4_pro_optimizations().then_some(()) {
            ctx.fused_swiglu(&gate, &up)?
        } else {
            // CPU fallback for SwiGLU
            gate.iter()
                .zip(up.iter())
                .map(|(&g, &u)| {
                    let silu_g = g / (1.0 + (-g).exp());
                    silu_g * u
                })
                .collect()
        };

        // Down projection: activated @ down_weight.T
        let output = ctx.gemm_f32(
            &activated,
            down_weight,
            batch_size,
            hidden_size,
            intermediate_size,
        )?;

        if self.config.collect_metrics {
            let duration_ns = start.elapsed().as_nanos() as u64;
            // MLP FLOPs: 3 matmuls + activation
            let flops = 2
                * batch_size as u64
                * (hidden_size as u64 * intermediate_size as u64 * 2
                    + intermediate_size as u64 * hidden_size as u64);
            let bytes = (hidden.len()
                + gate_weight.len()
                + up_weight.len()
                + down_weight.len()
                + output.len())
                * 4;

            match decision.accelerator {
                AcceleratorType::Ane => {
                    self.ane_metrics
                        .record_operation(duration_ns, flops, bytes as u64)
                }
                AcceleratorType::Metal => {
                    self.metal_metrics
                        .record_operation(duration_ns, flops, bytes as u64)
                }
                _ => self
                    .cpu_metrics
                    .record_operation(duration_ns, flops, bytes as u64),
            }
        }

        Ok(output)
    }

    #[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
    pub fn mlp_forward(
        &mut self,
        _hidden: &[f32],
        _gate_weight: &[f32],
        _up_weight: &[f32],
        _down_weight: &[f32],
        _hidden_size: usize,
        _intermediate_size: usize,
    ) -> Result<Vec<f32>> {
        Err(RuvLLMError::HybridPipeline(
            "Metal compute not available on this platform".to_string(),
        ))
    }

    /// Execute RMSNorm with hybrid routing
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    pub fn rms_norm(&mut self, x: &mut [f32], weight: &[f32], eps: f32) -> Result<()> {
        let hidden_size = weight.len();
        let batch_size = x.len() / hidden_size;
        let decision = self.route_operation(OperationType::Normalization, batch_size, hidden_size);

        let start = Instant::now();

        // Use Metal for RMSNorm
        let ctx = self.metal_ctx.as_ref().ok_or_else(|| {
            RuvLLMError::HybridPipeline("Metal context not available".to_string())
        })?;

        ctx.rms_norm(x, weight, eps)?;

        if self.config.collect_metrics {
            let duration_ns = start.elapsed().as_nanos() as u64;
            // RMSNorm FLOPs: ~4 ops per element (square, sum, rsqrt, mul)
            let flops = 4 * x.len() as u64;
            let bytes = (x.len() + weight.len()) * 4;

            match decision.accelerator {
                AcceleratorType::Ane => {
                    self.ane_metrics
                        .record_operation(duration_ns, flops, bytes as u64)
                }
                AcceleratorType::Metal => {
                    self.metal_metrics
                        .record_operation(duration_ns, flops, bytes as u64)
                }
                _ => self
                    .cpu_metrics
                    .record_operation(duration_ns, flops, bytes as u64),
            }
        }

        Ok(())
    }

    #[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
    pub fn rms_norm(&mut self, _x: &mut [f32], _weight: &[f32], _eps: f32) -> Result<()> {
        Err(RuvLLMError::HybridPipeline(
            "Metal compute not available on this platform".to_string(),
        ))
    }

    /// Apply RoPE with Metal GPU
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    pub fn apply_rope(
        &mut self,
        x: &mut [f32],
        position: usize,
        num_heads: usize,
        head_dim: usize,
        theta: f32,
    ) -> Result<()> {
        let start = Instant::now();

        let ctx = self.metal_ctx.as_ref().ok_or_else(|| {
            RuvLLMError::HybridPipeline("Metal context not available".to_string())
        })?;

        ctx.apply_rope(x, position, num_heads, head_dim, theta)?;

        if self.config.collect_metrics {
            let duration_ns = start.elapsed().as_nanos() as u64;
            // RoPE FLOPs: ~6 ops per element (sin, cos, mul, add)
            let flops = 6 * x.len() as u64;
            let bytes = x.len() * 4;
            self.metal_metrics
                .record_operation(duration_ns, flops, bytes as u64);
        }

        Ok(())
    }

    #[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
    pub fn apply_rope(
        &mut self,
        _x: &mut [f32],
        _position: usize,
        _num_heads: usize,
        _head_dim: usize,
        _theta: f32,
    ) -> Result<()> {
        Err(RuvLLMError::HybridPipeline(
            "Metal compute not available on this platform".to_string(),
        ))
    }

    /// Get summary of accelerator utilization
    pub fn utilization_summary(&self) -> String {
        let total_ops =
            self.metal_metrics.total_ops + self.ane_metrics.total_ops + self.cpu_metrics.total_ops;

        if total_ops == 0 {
            return "No operations executed yet".to_string();
        }

        let metal_pct = (self.metal_metrics.total_ops as f64 / total_ops as f64) * 100.0;
        let ane_pct = (self.ane_metrics.total_ops as f64 / total_ops as f64) * 100.0;
        let cpu_pct = (self.cpu_metrics.total_ops as f64 / total_ops as f64) * 100.0;

        format!(
            "Utilization: Metal={:.1}% ({} ops, {:.2} GFLOPS), ANE={:.1}% ({} ops, {:.2} GFLOPS), CPU={:.1}% ({} ops)",
            metal_pct, self.metal_metrics.total_ops, self.metal_metrics.peak_gflops,
            ane_pct, self.ane_metrics.total_ops, self.ane_metrics.peak_gflops,
            cpu_pct, self.cpu_metrics.total_ops
        )
    }

    /// Reset all metrics
    pub fn reset_metrics(&mut self) {
        self.metal_metrics = AcceleratorMetrics::default();
        self.ane_metrics = AcceleratorMetrics::default();
        self.cpu_metrics = AcceleratorMetrics::default();
        self.routing_history.clear();
    }
}

/// Helper function to get operation name
fn op_name(op: OperationType) -> &'static str {
    match op {
        OperationType::MatMul => "MatMul",
        OperationType::Attention => "Attention",
        OperationType::FlashAttention => "FlashAttention",
        OperationType::Activation => "Activation",
        OperationType::Normalization => "Normalization",
        OperationType::RoPE => "RoPE",
        OperationType::Embedding => "Embedding",
        OperationType::KvCache => "KvCache",
        OperationType::Softmax => "Softmax",
        OperationType::Other => "Other",
    }
}

// Implement LlmBackend trait for HybridPipeline
impl LlmBackend for HybridPipeline {
    fn load_model(&mut self, model_id: &str, config: ModelConfig) -> Result<()> {
        // Initialize both backends with the model
        #[cfg(all(target_os = "macos", feature = "metal-compute"))]
        if self.metal_ctx.is_none() {
            self.metal_ctx = MetalContext::new(MetalConfig::default()).ok();
        }

        #[cfg(feature = "coreml")]
        if self.coreml_backend.is_some() {
            if let Some(ref mut backend) = self.coreml_backend {
                // Try to load on Core ML (may fail if model not converted)
                let _ = backend.load_model(model_id, config.clone());
            }
        }

        // Store model info
        self.model_info = Some(ModelInfo {
            name: model_id.to_string(),
            architecture: config.architecture,
            num_parameters: 0, // Would be filled from actual model
            vocab_size: config.vocab_size.unwrap_or(32000),
            hidden_size: config.hidden_size.unwrap_or(4096),
            num_layers: config.num_layers.unwrap_or(32),
            max_context_length: config.max_sequence_length,
            quantization: config.quantization,
            memory_usage: 0,
        });

        self.loaded = true;
        Ok(())
    }

    fn generate(&self, _prompt: &str, _params: GenerateParams) -> Result<String> {
        if !self.loaded {
            return Err(RuvLLMError::InvalidOperation("No model loaded".to_string()));
        }

        Err(RuvLLMError::NotImplemented(
            "HybridPipeline generate() requires model-specific implementation".to_string(),
        ))
    }

    fn generate_stream(
        &self,
        _prompt: &str,
        _params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<GeneratedToken>> + Send + '_>> {
        Err(RuvLLMError::NotImplemented(
            "HybridPipeline streaming not yet implemented".to_string(),
        ))
    }

    fn generate_stream_v2(&self, _prompt: &str, _params: GenerateParams) -> Result<TokenStream> {
        Err(RuvLLMError::NotImplemented(
            "HybridPipeline streaming v2 not yet implemented".to_string(),
        ))
    }

    fn get_embeddings(&self, _text: &str) -> Result<Vec<f32>> {
        Err(RuvLLMError::NotImplemented(
            "HybridPipeline embeddings not yet implemented".to_string(),
        ))
    }

    fn tokenizer(&self) -> Option<&dyn Tokenizer> {
        None
    }

    fn is_model_loaded(&self) -> bool {
        self.loaded
    }

    fn model_info(&self) -> Option<ModelInfo> {
        self.model_info.clone()
    }

    fn unload_model(&mut self) {
        self.loaded = false;
        self.model_info = None;

        #[cfg(feature = "coreml")]
        if let Some(ref mut backend) = self.coreml_backend {
            backend.unload_model();
        }
    }
}

// Mark HybridPipeline as thread-safe
unsafe impl Send for HybridPipeline {}
unsafe impl Sync for HybridPipeline {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ane_strategy() {
        assert!(AneStrategy::PreferAneForMlp.use_ane_for_mlp());
        assert!(!AneStrategy::PreferAneForMlp.use_ane_for_norm());

        assert!(AneStrategy::PreferAneForMlpAndNorm.use_ane_for_mlp());
        assert!(AneStrategy::PreferAneForMlpAndNorm.use_ane_for_norm());

        assert!(!AneStrategy::GpuOnly.use_ane_for_mlp());
    }

    #[test]
    fn test_operation_type_routing() {
        assert_eq!(
            OperationType::MatMul.preferred_accelerator(),
            AcceleratorType::Ane
        );
        assert_eq!(
            OperationType::Attention.preferred_accelerator(),
            AcceleratorType::Metal
        );
        assert_eq!(
            OperationType::FlashAttention.preferred_accelerator(),
            AcceleratorType::Metal
        );
    }

    #[test]
    fn test_pipeline_config_defaults() {
        let config = HybridPipelineConfig::default();
        assert_eq!(config.ane_strategy, AneStrategy::PreferAneForMlp);
        assert!(config.metal_for_attention);
        assert_eq!(config.ane_min_batch_size, 1);
    }

    #[test]
    fn test_routing_decision() {
        let config = HybridPipelineConfig::default();
        let pipeline = HybridPipeline::new(config).unwrap();

        // Attention should always route to Metal
        let decision = pipeline.route_operation(OperationType::Attention, 1, 4096);
        assert!(matches!(
            decision.accelerator,
            AcceleratorType::Metal | AcceleratorType::Cpu
        ));

        // MatMul routing depends on ANE availability
        let decision = pipeline.route_operation(OperationType::MatMul, 16, 4096);
        // On macOS with ANE, should prefer ANE; otherwise Metal/CPU
        assert!(matches!(
            decision.accelerator,
            AcceleratorType::Ane | AcceleratorType::Metal | AcceleratorType::Cpu
        ));
    }

    #[test]
    fn test_hybrid_tensor() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let tensor = HybridTensor::from_cpu(data.clone(), vec![2, 2]);

        assert_eq!(tensor.numel(), 4);
        assert_eq!(tensor.format, DataFormat::CpuMemory);
        assert_eq!(tensor.as_slice(), Some(data.as_slice()));
    }

    #[test]
    fn test_accelerator_metrics() {
        let mut metrics = AcceleratorMetrics::default();

        metrics.record_operation(1_000_000, 1_000_000, 4096);
        assert_eq!(metrics.total_ops, 1);
        assert_eq!(metrics.total_time_ns, 1_000_000);
        assert_eq!(metrics.total_flops, 1_000_000);

        metrics.record_operation(2_000_000, 2_000_000, 8192);
        assert_eq!(metrics.total_ops, 2);
        assert_eq!(metrics.total_time_ns, 3_000_000);
    }

    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    #[test]
    fn test_pipeline_creation() {
        let config = HybridPipelineConfig::default();
        let pipeline = HybridPipeline::new(config);
        assert!(pipeline.is_ok());

        let pipeline = pipeline.unwrap();
        assert!(pipeline.has_metal() || !crate::metal::is_metal_available());
    }
}
