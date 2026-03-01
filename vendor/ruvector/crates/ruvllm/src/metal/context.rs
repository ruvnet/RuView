//! Metal context and device management
//!
//! Provides the main interface for Metal GPU operations.

use metal::{
    Buffer, CommandQueue, ComputeCommandEncoder, ComputePipelineState, Device, Library,
    MTLResourceOptions, MTLSize,
};
use std::sync::Arc;

use super::{
    shader_source, tile_sizes, AttentionParams, FusedAttentionParams, FusedNormParams, GemmParams,
    Int4GemvParams, MetalPipelines, NormParams, PagedAttentionParams, RopeAttentionParams,
    RopeParams, SwiGLUParams, YarnAttentionParams,
};
use crate::error::{Result, RuvLLMError};
use crate::kernels::AttentionConfig;

/// Configuration for Metal context
#[derive(Debug, Clone)]
pub struct MetalConfig {
    /// Maximum buffer pool size in bytes
    pub max_buffer_pool_size: usize,
    /// Enable profiling
    pub enable_profiling: bool,
    /// Preferred threadgroup size for compute
    pub preferred_threadgroup_size: usize,
}

impl Default for MetalConfig {
    fn default() -> Self {
        Self {
            max_buffer_pool_size: 1024 * 1024 * 1024, // 1GB
            enable_profiling: false,
            preferred_threadgroup_size: 256,
        }
    }
}

/// Metal context for GPU operations
pub struct MetalContext {
    /// Metal device
    device: Device,
    /// Command queue
    queue: CommandQueue,
    /// Compiled pipelines
    pipelines: MetalPipelines,
    /// Configuration
    config: MetalConfig,
    /// Shader library
    library: Library,
}

impl MetalContext {
    /// Create a new Metal context
    pub fn new(config: MetalConfig) -> Result<Self> {
        let device = Device::system_default()
            .ok_or_else(|| RuvLLMError::Backend("No Metal device found".to_string()))?;

        let queue = device.new_command_queue();

        // Compile shader library from embedded sources (including M4 Pro optimized)
        let shader_source = format!(
            "{}\n{}\n{}\n{}\n{}",
            shader_source::ATTENTION,
            shader_source::GEMM,
            shader_source::NORM,
            shader_source::ROPE,
            shader_source::all_optimized_shaders(),
        );

        let library = device
            .new_library_with_source(&shader_source, &metal::CompileOptions::new())
            .map_err(|e| RuvLLMError::Backend(format!("Failed to compile shaders: {}", e)))?;

        let pipelines = MetalPipelines::new(&device, &library)?;

        Ok(Self {
            device,
            queue,
            pipelines,
            config,
            library,
        })
    }

    /// Get the Metal device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get the command queue
    pub fn queue(&self) -> &CommandQueue {
        &self.queue
    }

    /// Flash Attention operation
    ///
    /// Computes attention(Q, K, V) = softmax(Q @ K^T / sqrt(d)) @ V
    /// using a memory-efficient tiled algorithm.
    ///
    /// # Arguments
    /// * `query` - Query tensor [seq_len, num_heads, head_dim]
    /// * `key` - Key tensor [kv_len, num_kv_heads, head_dim]
    /// * `value` - Value tensor [kv_len, num_kv_heads, head_dim]
    /// * `config` - Attention configuration
    ///
    /// # Returns
    /// Output tensor [seq_len, num_heads, head_dim]
    pub fn flash_attention(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        config: &AttentionConfig,
    ) -> Result<Vec<f32>> {
        let seq_len = query.len() / (config.num_heads * config.head_dim);
        let kv_len = key.len() / (config.num_kv_heads * config.head_dim);

        if seq_len == 0 || kv_len == 0 {
            return Ok(vec![0.0; query.len()]);
        }

        let params = AttentionParams::from_config(config, seq_len, kv_len);
        let output_size = seq_len * config.num_heads * config.head_dim;

        // Create Metal buffers
        let q_buffer = self.create_buffer_with_data(query)?;
        let k_buffer = self.create_buffer_with_data(key)?;
        let v_buffer = self.create_buffer_with_data(value)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;
        let output_buffer = self.create_buffer(output_size * std::mem::size_of::<f32>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&self.pipelines.attention);
        encoder.set_buffer(0, Some(&q_buffer), 0);
        encoder.set_buffer(1, Some(&k_buffer), 0);
        encoder.set_buffer(2, Some(&v_buffer), 0);
        encoder.set_buffer(3, Some(&output_buffer), 0);
        encoder.set_buffer(4, Some(&params_buffer), 0);

        // Calculate grid and threadgroup sizes
        let threads_per_head = config.head_dim.min(tile_sizes::MAX_THREADS_PER_THREADGROUP);
        let threadgroup_size = MTLSize::new(threads_per_head as u64, 1, 1);
        let grid_size = MTLSize::new(
            threads_per_head as u64,
            config.num_heads as u64,
            seq_len as u64,
        );

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        self.read_buffer(&output_buffer, output_size)
    }

    /// GEMM operation with FP16
    ///
    /// Computes C = alpha * A @ B + beta * C using FP16 precision
    /// with simdgroup_matrix acceleration on M4 Pro.
    ///
    /// # Arguments
    /// * `a` - Matrix A [m, k] in FP16
    /// * `b` - Matrix B [k, n] in FP16
    /// * `m` - Rows of A and C
    /// * `n` - Columns of B and C
    /// * `k` - Columns of A, rows of B
    ///
    /// # Returns
    /// Matrix C [m, n] in FP16
    pub fn gemm_f16(
        &self,
        a: &[half::f16],
        b: &[half::f16],
        m: usize,
        n: usize,
        k: usize,
    ) -> Result<Vec<half::f16>> {
        if a.len() != m * k || b.len() != k * n {
            return Err(RuvLLMError::InvalidOperation(format!(
                "GEMM dimension mismatch: A[{}] != {}x{}, B[{}] != {}x{}",
                a.len(),
                m,
                k,
                b.len(),
                k,
                n
            )));
        }

        let params = GemmParams::new(m, n, k);
        let output_size = m * n;

        // Create buffers
        let a_buffer = self.create_buffer_with_data_raw(a)?;
        let b_buffer = self.create_buffer_with_data_raw(b)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;
        let c_buffer = self.create_buffer(output_size * std::mem::size_of::<half::f16>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&self.pipelines.gemm);
        encoder.set_buffer(0, Some(&a_buffer), 0);
        encoder.set_buffer(1, Some(&b_buffer), 0);
        encoder.set_buffer(2, Some(&c_buffer), 0);
        encoder.set_buffer(3, Some(&params_buffer), 0);

        // Grid: one threadgroup per output tile
        let tiles_m = (m + tile_sizes::GEMM_TILE_M - 1) / tile_sizes::GEMM_TILE_M;
        let tiles_n = (n + tile_sizes::GEMM_TILE_N - 1) / tile_sizes::GEMM_TILE_N;

        let threadgroup_size = MTLSize::new(
            tile_sizes::GEMM_TILE_M as u64,
            tile_sizes::GEMM_TILE_N as u64 / 8, // 8 threads per N tile with simdgroup
            1,
        );
        let grid_size = MTLSize::new(tiles_m as u64, tiles_n as u64, 1);

        encoder.dispatch_thread_groups(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        self.read_buffer_raw(&c_buffer, output_size)
    }

    /// GEMM operation with FP32
    ///
    /// Computes C = A @ B using FP32 precision.
    pub fn gemm_f32(&self, a: &[f32], b: &[f32], m: usize, n: usize, k: usize) -> Result<Vec<f32>> {
        if a.len() != m * k || b.len() != k * n {
            return Err(RuvLLMError::InvalidOperation(format!(
                "GEMM dimension mismatch: A[{}] != {}x{}, B[{}] != {}x{}",
                a.len(),
                m,
                k,
                b.len(),
                k,
                n
            )));
        }

        let params = GemmParams::new(m, n, k);
        let output_size = m * n;

        // Create buffers
        let a_buffer = self.create_buffer_with_data(a)?;
        let b_buffer = self.create_buffer_with_data(b)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;
        let c_buffer = self.create_buffer(output_size * std::mem::size_of::<f32>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&self.pipelines.gemm_f32);
        encoder.set_buffer(0, Some(&a_buffer), 0);
        encoder.set_buffer(1, Some(&b_buffer), 0);
        encoder.set_buffer(2, Some(&c_buffer), 0);
        encoder.set_buffer(3, Some(&params_buffer), 0);

        // Grid: one threadgroup per output tile
        let tiles_m = (m + tile_sizes::GEMM_TILE_M - 1) / tile_sizes::GEMM_TILE_M;
        let tiles_n = (n + tile_sizes::GEMM_TILE_N - 1) / tile_sizes::GEMM_TILE_N;

        let threadgroup_size = MTLSize::new(16, 16, 1);
        let grid_size = MTLSize::new((tiles_m * 16) as u64, (tiles_n * 16) as u64, 1);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        self.read_buffer(&c_buffer, output_size)
    }

    /// RMSNorm operation
    ///
    /// Computes RMSNorm(x) = x * weight / sqrt(mean(x^2) + eps)
    ///
    /// # Arguments
    /// * `x` - Input tensor [batch, hidden_size]
    /// * `weight` - Weight tensor [hidden_size]
    /// * `eps` - Epsilon for numerical stability
    ///
    /// # Returns
    /// Normalized tensor (in-place modification, also returns copy)
    pub fn rms_norm(&self, x: &mut [f32], weight: &[f32], eps: f32) -> Result<()> {
        let hidden_size = weight.len();
        let batch_size = x.len() / hidden_size;

        if x.len() != batch_size * hidden_size {
            return Err(RuvLLMError::InvalidOperation(
                "RMSNorm dimension mismatch".to_string(),
            ));
        }

        let params = NormParams::new(hidden_size, eps);

        // Create buffers
        let x_buffer = self.create_buffer_with_data(x)?;
        let weight_buffer = self.create_buffer_with_data(weight)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&self.pipelines.rms_norm);
        encoder.set_buffer(0, Some(&x_buffer), 0);
        encoder.set_buffer(1, Some(&weight_buffer), 0);
        encoder.set_buffer(2, Some(&params_buffer), 0);

        // One threadgroup per batch element
        let threads_per_group = hidden_size.min(tile_sizes::MAX_THREADS_PER_THREADGROUP);
        let threadgroup_size = MTLSize::new(threads_per_group as u64, 1, 1);
        let grid_size = MTLSize::new(threads_per_group as u64, batch_size as u64, 1);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        let result = self.read_buffer(&x_buffer, x.len())?;
        x.copy_from_slice(&result);

        Ok(())
    }

    /// Apply RoPE (Rotary Position Embeddings)
    ///
    /// Applies rotary embeddings to query and key tensors.
    ///
    /// # Arguments
    /// * `x` - Input tensor [batch, num_heads, head_dim]
    /// * `position` - Position index
    /// * `head_dim` - Dimension per head
    /// * `theta_base` - Base for frequency calculation
    pub fn apply_rope(
        &self,
        x: &mut [f32],
        position: usize,
        num_heads: usize,
        head_dim: usize,
        theta_base: f32,
    ) -> Result<()> {
        let batch_size = x.len() / (num_heads * head_dim);

        if x.len() != batch_size * num_heads * head_dim {
            return Err(RuvLLMError::InvalidOperation(
                "RoPE dimension mismatch".to_string(),
            ));
        }

        let params = RopeParams::new(head_dim, num_heads, position, theta_base);

        // Precompute cos/sin tables
        let half_dim = head_dim / 2;
        let mut cos_table = vec![0.0f32; half_dim];
        let mut sin_table = vec![0.0f32; half_dim];

        for i in 0..half_dim {
            let freq = 1.0 / theta_base.powf(2.0 * i as f32 / head_dim as f32);
            let angle = position as f32 * freq;
            cos_table[i] = angle.cos();
            sin_table[i] = angle.sin();
        }

        // Create buffers
        let x_buffer = self.create_buffer_with_data(x)?;
        let cos_buffer = self.create_buffer_with_data(&cos_table)?;
        let sin_buffer = self.create_buffer_with_data(&sin_table)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(&self.pipelines.rope);
        encoder.set_buffer(0, Some(&x_buffer), 0);
        encoder.set_buffer(1, Some(&cos_buffer), 0);
        encoder.set_buffer(2, Some(&sin_buffer), 0);
        encoder.set_buffer(3, Some(&params_buffer), 0);

        // One thread per head dimension element
        let threadgroup_size = MTLSize::new(head_dim as u64, 1, 1);
        let grid_size = MTLSize::new(head_dim as u64, num_heads as u64, batch_size as u64);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        let result = self.read_buffer(&x_buffer, x.len())?;
        x.copy_from_slice(&result);

        Ok(())
    }

    // ============ M4 Pro Optimized Operations ============

    /// Check if M4 Pro optimizations are available
    pub fn has_m4_pro_optimizations(&self) -> bool {
        self.pipelines.has_m4_pro_optimizations()
    }

    /// Get list of available M4 Pro optimized operations
    pub fn available_optimizations(&self) -> Vec<&'static str> {
        self.pipelines.available_optimizations()
    }

    /// Optimized GEMM using M4 Pro tuned parameters (BM=128, BN=128, BK=32)
    ///
    /// Uses triple-buffered software pipelining and simdgroup_matrix for
    /// maximum throughput on M4 Pro's matrix coprocessor.
    pub fn gemm_optimized(
        &self,
        a: &[half::f16],
        b: &[half::f16],
        m: usize,
        n: usize,
        k: usize,
    ) -> Result<Vec<half::f16>> {
        // Fall back to standard GEMM if optimized pipeline not available
        let pipeline = match &self.pipelines.gemm_optimized {
            Some(p) => p,
            None => return self.gemm_f16(a, b, m, n, k),
        };

        if a.len() != m * k || b.len() != k * n {
            return Err(RuvLLMError::InvalidOperation(format!(
                "GEMM dimension mismatch: A[{}] != {}x{}, B[{}] != {}x{}",
                a.len(),
                m,
                k,
                b.len(),
                k,
                n
            )));
        }

        let output_size = m * n;

        // Create buffers
        let a_buffer = self.create_buffer_with_data_raw(a)?;
        let b_buffer = self.create_buffer_with_data_raw(b)?;
        let c_buffer = self.create_buffer(output_size * std::mem::size_of::<half::f16>())?;

        // Dimension buffer: [M, N, K, 0]
        let dims: [u32; 4] = [m as u32, n as u32, k as u32, 0];
        let dims_buffer = self.create_buffer_with_data(&dims)?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&a_buffer), 0);
        encoder.set_buffer(1, Some(&b_buffer), 0);
        encoder.set_buffer(2, Some(&c_buffer), 0);
        encoder.set_buffer(3, Some(&dims_buffer), 0);

        // M4 Pro optimized grid: 128x128 output tiles
        let tiles_m = (m + tile_sizes::M4_GEMM_TILE_M - 1) / tile_sizes::M4_GEMM_TILE_M;
        let tiles_n = (n + tile_sizes::M4_GEMM_TILE_N - 1) / tile_sizes::M4_GEMM_TILE_N;

        // 1024 threads per threadgroup for M4 Pro
        let threadgroup_size = MTLSize::new(32, 32, 1);
        let grid_size = MTLSize::new(tiles_n as u64, tiles_m as u64, 1);

        encoder.dispatch_thread_groups(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        self.read_buffer_raw(&c_buffer, output_size)
    }

    /// Fused Flash Attention 2 with online softmax
    ///
    /// Implements the Flash Attention 2 algorithm with O(N) memory complexity
    /// using online softmax and tiled matrix multiplication.
    pub fn fused_attention(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        causal: bool,
    ) -> Result<Vec<f32>> {
        // Fall back to standard attention if fused pipeline not available
        let pipeline = match &self.pipelines.fused_attention {
            Some(p) => p,
            None => {
                let config = AttentionConfig {
                    num_heads,
                    num_kv_heads,
                    head_dim,
                    max_seq_len: 4096,
                    causal,
                    scale: 0.0,
                };
                return self.flash_attention(query, key, value, &config);
            }
        };

        let seq_len = query.len() / (num_heads * head_dim);
        let kv_len = key.len() / (num_kv_heads * head_dim);

        if seq_len == 0 || kv_len == 0 {
            return Ok(vec![0.0; query.len()]);
        }

        let params =
            FusedAttentionParams::new(num_heads, num_kv_heads, head_dim, seq_len, kv_len, causal);
        let output_size = seq_len * num_heads * head_dim;

        // Create Metal buffers
        let q_buffer = self.create_buffer_with_data(query)?;
        let k_buffer = self.create_buffer_with_data(key)?;
        let v_buffer = self.create_buffer_with_data(value)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;
        let output_buffer = self.create_buffer(output_size * std::mem::size_of::<f32>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&q_buffer), 0);
        encoder.set_buffer(1, Some(&k_buffer), 0);
        encoder.set_buffer(2, Some(&v_buffer), 0);
        encoder.set_buffer(3, Some(&output_buffer), 0);
        encoder.set_buffer(4, Some(&params_buffer), 0);

        // Flash Attention 2 grid: one threadgroup per head per query block
        let q_blocks =
            (seq_len + tile_sizes::FLASH_ATTENTION_BLOCK - 1) / tile_sizes::FLASH_ATTENTION_BLOCK;
        let threadgroup_size = MTLSize::new(tile_sizes::FLASH_ATTENTION_BLOCK as u64, 1, 1);
        let grid_size = MTLSize::new(
            tile_sizes::FLASH_ATTENTION_BLOCK as u64,
            num_heads as u64,
            q_blocks as u64,
        );

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        self.read_buffer(&output_buffer, output_size)
    }

    /// Fused LayerNorm + Residual connection
    ///
    /// Computes: output = LayerNorm(x + residual) in a single pass
    pub fn fused_layernorm_residual(
        &self,
        x: &mut [f32],
        residual: &[f32],
        weight: &[f32],
        bias: &[f32],
        eps: f32,
    ) -> Result<()> {
        let pipeline = self
            .pipelines
            .fused_layernorm_residual
            .as_ref()
            .ok_or_else(|| {
                RuvLLMError::Backend(
                    "Fused LayerNorm+Residual not available on this device".to_string(),
                )
            })?;

        let hidden_size = weight.len();
        let batch_size = x.len() / hidden_size;

        if x.len() != batch_size * hidden_size || residual.len() != x.len() {
            return Err(RuvLLMError::InvalidOperation(
                "Fused LayerNorm dimension mismatch".to_string(),
            ));
        }

        let params = FusedNormParams::new(hidden_size, eps);

        // Create buffers
        let x_buffer = self.create_buffer_with_data(x)?;
        let residual_buffer = self.create_buffer_with_data(residual)?;
        let weight_buffer = self.create_buffer_with_data(weight)?;
        let bias_buffer = self.create_buffer_with_data(bias)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&x_buffer), 0);
        encoder.set_buffer(1, Some(&residual_buffer), 0);
        encoder.set_buffer(2, Some(&weight_buffer), 0);
        encoder.set_buffer(3, Some(&bias_buffer), 0);
        encoder.set_buffer(4, Some(&params_buffer), 0);

        let threads_per_group = hidden_size.min(tile_sizes::MAX_THREADS_PER_THREADGROUP);
        let threadgroup_size = MTLSize::new(threads_per_group as u64, 1, 1);
        let grid_size = MTLSize::new(threads_per_group as u64, batch_size as u64, 1);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        let result = self.read_buffer(&x_buffer, x.len())?;
        x.copy_from_slice(&result);

        Ok(())
    }

    /// Fused RMSNorm + Residual connection
    ///
    /// Computes: output = RMSNorm(x + residual) in a single pass
    pub fn fused_rmsnorm_residual(
        &self,
        x: &mut [f32],
        residual: &[f32],
        weight: &[f32],
        eps: f32,
    ) -> Result<()> {
        let pipeline = self
            .pipelines
            .fused_rmsnorm_residual
            .as_ref()
            .ok_or_else(|| {
                RuvLLMError::Backend(
                    "Fused RMSNorm+Residual not available on this device".to_string(),
                )
            })?;

        let hidden_size = weight.len();
        let batch_size = x.len() / hidden_size;

        if x.len() != batch_size * hidden_size || residual.len() != x.len() {
            return Err(RuvLLMError::InvalidOperation(
                "Fused RMSNorm dimension mismatch".to_string(),
            ));
        }

        let params = FusedNormParams::new(hidden_size, eps);

        // Create buffers
        let x_buffer = self.create_buffer_with_data(x)?;
        let residual_buffer = self.create_buffer_with_data(residual)?;
        let weight_buffer = self.create_buffer_with_data(weight)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&x_buffer), 0);
        encoder.set_buffer(1, Some(&residual_buffer), 0);
        encoder.set_buffer(2, Some(&weight_buffer), 0);
        encoder.set_buffer(3, Some(&params_buffer), 0);

        let threads_per_group = hidden_size.min(tile_sizes::MAX_THREADS_PER_THREADGROUP);
        let threadgroup_size = MTLSize::new(threads_per_group as u64, 1, 1);
        let grid_size = MTLSize::new(threads_per_group as u64, batch_size as u64, 1);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        // Read back results
        let result = self.read_buffer(&x_buffer, x.len())?;
        x.copy_from_slice(&result);

        Ok(())
    }

    /// Fused SwiGLU activation
    ///
    /// Computes: output = Swish(gate) * up in a single kernel
    pub fn fused_swiglu(&self, gate: &[f32], up: &[f32]) -> Result<Vec<f32>> {
        let pipeline = self.pipelines.fused_swiglu.as_ref().ok_or_else(|| {
            RuvLLMError::Backend("Fused SwiGLU not available on this device".to_string())
        })?;

        if gate.len() != up.len() {
            return Err(RuvLLMError::InvalidOperation(
                "SwiGLU dimension mismatch".to_string(),
            ));
        }

        let size = gate.len();

        // Create buffers
        let gate_buffer = self.create_buffer_with_data(gate)?;
        let up_buffer = self.create_buffer_with_data(up)?;
        let output_buffer = self.create_buffer(size * std::mem::size_of::<f32>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&gate_buffer), 0);
        encoder.set_buffer(1, Some(&up_buffer), 0);
        encoder.set_buffer(2, Some(&output_buffer), 0);

        let size_buffer = self.create_buffer_with_data(&[size as u32])?;
        encoder.set_buffer(3, Some(&size_buffer), 0);

        let threads_per_group = 256.min(size);
        let num_groups = (size + threads_per_group - 1) / threads_per_group;
        let threadgroup_size = MTLSize::new(threads_per_group as u64, 1, 1);
        let grid_size = MTLSize::new((num_groups * threads_per_group) as u64, 1, 1);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        self.read_buffer(&output_buffer, size)
    }

    /// INT4 quantized GEMV (matrix-vector multiply)
    ///
    /// Performs y = A * x where A is quantized to INT4 with group-wise scales.
    /// 4x memory reduction compared to FP16.
    pub fn int4_gemv(
        &self,
        weights_int4: &[u8], // Packed INT4 weights (2 values per byte)
        scales: &[f32],      // Per-group scale factors
        zeros: &[f32],       // Per-group zero points
        input: &[f32],       // Input vector
        m: usize,            // Output dimension
        n: usize,            // Input dimension
        group_size: usize,   // Quantization group size
    ) -> Result<Vec<f32>> {
        // Prefer SIMD-optimized version if available
        let pipeline = self
            .pipelines
            .int4_gemv_simd
            .as_ref()
            .or(self.pipelines.int4_gemv.as_ref())
            .ok_or_else(|| {
                RuvLLMError::Backend("INT4 GEMV not available on this device".to_string())
            })?;

        let expected_weights = (m * n + 1) / 2; // 2 values per byte
        if weights_int4.len() != expected_weights {
            return Err(RuvLLMError::InvalidOperation(format!(
                "INT4 weight size mismatch: expected {} bytes, got {}",
                expected_weights,
                weights_int4.len()
            )));
        }

        let params = Int4GemvParams::new(m, n, group_size);

        // Create buffers
        let weights_buffer = self.create_buffer_with_data(weights_int4)?;
        let scales_buffer = self.create_buffer_with_data(scales)?;
        let zeros_buffer = self.create_buffer_with_data(zeros)?;
        let input_buffer = self.create_buffer_with_data(input)?;
        let output_buffer = self.create_buffer(m * std::mem::size_of::<f32>())?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&weights_buffer), 0);
        encoder.set_buffer(1, Some(&scales_buffer), 0);
        encoder.set_buffer(2, Some(&zeros_buffer), 0);
        encoder.set_buffer(3, Some(&input_buffer), 0);
        encoder.set_buffer(4, Some(&output_buffer), 0);
        encoder.set_buffer(5, Some(&params_buffer), 0);

        // One thread per output element
        let threads_per_group = 256.min(m);
        let num_groups = (m + threads_per_group - 1) / threads_per_group;
        let threadgroup_size = MTLSize::new(threads_per_group as u64, 1, 1);
        let grid_size = MTLSize::new((num_groups * threads_per_group) as u64, 1, 1);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        self.read_buffer(&output_buffer, m)
    }

    /// RoPE + Attention fusion
    ///
    /// Applies RoPE to Q/K tensors and performs attention in a single fused kernel.
    /// Reduces memory traffic by avoiding intermediate tensor materialization.
    pub fn rope_then_attention(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        position_offset: usize,
        rope_theta: f32,
        causal: bool,
    ) -> Result<Vec<f32>> {
        // Fall back to separate operations if fused pipeline not available
        let pipeline = match &self.pipelines.rope_then_attention {
            Some(p) => p,
            None => {
                // Fallback: apply RoPE then attention separately
                let mut q = query.to_vec();
                let mut k = key.to_vec();
                self.apply_rope(&mut q, position_offset, num_heads, head_dim, rope_theta)?;
                self.apply_rope(&mut k, position_offset, num_kv_heads, head_dim, rope_theta)?;
                return self.fused_attention(
                    &q,
                    &k,
                    value,
                    num_heads,
                    num_kv_heads,
                    head_dim,
                    causal,
                );
            }
        };

        let seq_len = query.len() / (num_heads * head_dim);
        let kv_len = key.len() / (num_kv_heads * head_dim);

        if seq_len == 0 || kv_len == 0 {
            return Ok(vec![0.0; query.len()]);
        }

        let params = RopeAttentionParams::new(
            num_heads,
            num_kv_heads,
            head_dim,
            seq_len,
            kv_len,
            position_offset,
            rope_theta,
            causal,
        );
        let output_size = seq_len * num_heads * head_dim;

        // Create Metal buffers
        let q_buffer = self.create_buffer_with_data(query)?;
        let k_buffer = self.create_buffer_with_data(key)?;
        let v_buffer = self.create_buffer_with_data(value)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;
        let output_buffer = self.create_buffer(output_size * std::mem::size_of::<f32>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&q_buffer), 0);
        encoder.set_buffer(1, Some(&k_buffer), 0);
        encoder.set_buffer(2, Some(&v_buffer), 0);
        encoder.set_buffer(3, Some(&output_buffer), 0);
        encoder.set_buffer(4, Some(&params_buffer), 0);

        let threadgroup_size = MTLSize::new(head_dim as u64, 1, 1);
        let grid_size = MTLSize::new(head_dim as u64, num_heads as u64, seq_len as u64);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        self.read_buffer(&output_buffer, output_size)
    }

    /// YaRN Attention for extended context
    ///
    /// Uses YaRN (Yet another RoPE extensioN) scaling for models with
    /// extended context windows beyond their training length.
    pub fn yarn_attention(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        position_offset: usize,
        rope_theta: f32,
        original_max_position: usize,
        target_max_position: usize,
        causal: bool,
    ) -> Result<Vec<f32>> {
        let pipeline = self.pipelines.yarn_attention.as_ref().ok_or_else(|| {
            RuvLLMError::Backend("YaRN attention not available on this device".to_string())
        })?;

        let seq_len = query.len() / (num_heads * head_dim);
        let kv_len = key.len() / (num_kv_heads * head_dim);

        if seq_len == 0 || kv_len == 0 {
            return Ok(vec![0.0; query.len()]);
        }

        let params = YarnAttentionParams::new(
            num_heads,
            num_kv_heads,
            head_dim,
            seq_len,
            kv_len,
            position_offset,
            rope_theta,
            original_max_position,
            target_max_position,
            causal,
        );
        let output_size = seq_len * num_heads * head_dim;

        // Create Metal buffers
        let q_buffer = self.create_buffer_with_data(query)?;
        let k_buffer = self.create_buffer_with_data(key)?;
        let v_buffer = self.create_buffer_with_data(value)?;
        let params_buffer = self.create_buffer_with_data(std::slice::from_ref(&params))?;
        let output_buffer = self.create_buffer(output_size * std::mem::size_of::<f32>())?;

        // Execute kernel
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_compute_pipeline_state(pipeline);
        encoder.set_buffer(0, Some(&q_buffer), 0);
        encoder.set_buffer(1, Some(&k_buffer), 0);
        encoder.set_buffer(2, Some(&v_buffer), 0);
        encoder.set_buffer(3, Some(&output_buffer), 0);
        encoder.set_buffer(4, Some(&params_buffer), 0);

        let threadgroup_size = MTLSize::new(head_dim as u64, 1, 1);
        let grid_size = MTLSize::new(head_dim as u64, num_heads as u64, seq_len as u64);

        encoder.dispatch_threads(grid_size, threadgroup_size);
        encoder.end_encoding();

        command_buffer.commit();
        command_buffer.wait_until_completed();

        self.read_buffer(&output_buffer, output_size)
    }

    /// Create a Metal buffer with specified size
    fn create_buffer(&self, size: usize) -> Result<Buffer> {
        Ok(self
            .device
            .new_buffer(size as u64, MTLResourceOptions::StorageModeShared))
    }

    /// Create a Metal buffer with data
    fn create_buffer_with_data<T: Copy>(&self, data: &[T]) -> Result<Buffer> {
        let size = data.len() * std::mem::size_of::<T>();
        let buffer = self.device.new_buffer_with_data(
            data.as_ptr() as *const _,
            size as u64,
            MTLResourceOptions::StorageModeShared,
        );
        Ok(buffer)
    }

    /// Create a Metal buffer with raw data (for FP16)
    fn create_buffer_with_data_raw<T: Copy>(&self, data: &[T]) -> Result<Buffer> {
        self.create_buffer_with_data(data)
    }

    /// Read data from a Metal buffer
    fn read_buffer<T: Copy + Default>(&self, buffer: &Buffer, count: usize) -> Result<Vec<T>> {
        let ptr = buffer.contents() as *const T;
        let mut result = vec![T::default(); count];
        unsafe {
            std::ptr::copy_nonoverlapping(ptr, result.as_mut_ptr(), count);
        }
        Ok(result)
    }

    /// Read raw data from a Metal buffer
    fn read_buffer_raw<T: Copy + Default>(&self, buffer: &Buffer, count: usize) -> Result<Vec<T>> {
        self.read_buffer(buffer, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metal_context_creation() {
        if !super::super::is_metal_available() {
            println!("Metal not available, skipping test");
            return;
        }

        let config = MetalConfig::default();
        let ctx = MetalContext::new(config);
        assert!(
            ctx.is_ok(),
            "Failed to create Metal context: {:?}",
            ctx.err()
        );
    }

    #[test]
    fn test_flash_attention() {
        if !super::super::is_metal_available() {
            println!("Metal not available, skipping test");
            return;
        }

        let ctx = MetalContext::new(MetalConfig::default()).unwrap();

        let config = AttentionConfig {
            num_heads: 4,
            num_kv_heads: 4,
            head_dim: 64,
            max_seq_len: 1024,
            causal: false,
            scale: 0.0,
        };

        let seq_len = 4;
        let kv_len = 8;

        let query: Vec<f32> = (0..seq_len * config.num_heads * config.head_dim)
            .map(|i| (i as f32) * 0.01)
            .collect();
        let key: Vec<f32> = (0..kv_len * config.num_kv_heads * config.head_dim)
            .map(|i| (i as f32) * 0.01)
            .collect();
        let value: Vec<f32> = (0..kv_len * config.num_kv_heads * config.head_dim)
            .map(|i| (i as f32) * 0.02)
            .collect();

        let output = ctx.flash_attention(&query, &key, &value, &config);
        assert!(output.is_ok());

        let output = output.unwrap();
        assert_eq!(output.len(), seq_len * config.num_heads * config.head_dim);
        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_rms_norm() {
        if !super::super::is_metal_available() {
            println!("Metal not available, skipping test");
            return;
        }

        let ctx = MetalContext::new(MetalConfig::default()).unwrap();

        let hidden_size = 256;
        let batch_size = 4;

        let mut x: Vec<f32> = (0..batch_size * hidden_size)
            .map(|i| (i as f32) * 0.01)
            .collect();
        let weight: Vec<f32> = vec![1.0; hidden_size];

        let result = ctx.rms_norm(&mut x, &weight, 1e-6);
        assert!(result.is_ok());
        assert!(x.iter().all(|&v| v.is_finite()));
    }
}
