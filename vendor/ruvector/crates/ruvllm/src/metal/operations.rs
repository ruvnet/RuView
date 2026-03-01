//! High-level Metal operations
//!
//! Provides convenient wrappers around Metal compute operations.

use super::{AttentionParams, GemmParams, MetalConfig, MetalContext, NormParams, RopeParams};
use crate::error::{Result, RuvLLMError};
use crate::kernels::AttentionConfig;

#[cfg(target_os = "macos")]
use metal::{Buffer, MTLSize};

// ============================================================================
// GEMV Parameters for Metal (matches shader struct)
// ============================================================================

/// GEMV parameters for Metal shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GemvParams {
    /// Rows of A (output dimension)
    pub m: u32,
    /// Columns of A (input dimension)
    pub n: u32,
    /// Leading dimension of A
    pub lda: u32,
    /// Alpha scalar
    pub alpha: f32,
    /// Beta scalar for y = alpha*A*x + beta*y
    pub beta: f32,
}

impl GemvParams {
    /// Create GEMV params for y = A * x
    pub fn new(m: usize, n: usize) -> Self {
        Self {
            m: m as u32,
            n: n as u32,
            lda: n as u32, // Row-major
            alpha: 1.0,
            beta: 0.0,
        }
    }

    /// Create GEMV params with scaling: y = alpha * A * x + beta * y
    pub fn with_scaling(m: usize, n: usize, alpha: f32, beta: f32) -> Self {
        Self {
            m: m as u32,
            n: n as u32,
            lda: n as u32,
            alpha,
            beta,
        }
    }
}

// ============================================================================
// Metal Buffer Wrapper for GEMV
// ============================================================================

/// Metal buffer wrapper for GEMV operations
#[cfg(target_os = "macos")]
pub struct MetalGemvBuffer {
    /// Underlying Metal buffer
    pub buffer: Buffer,
    /// Size in elements
    pub size: usize,
}

#[cfg(target_os = "macos")]
impl MetalGemvBuffer {
    /// Get buffer contents as f32 slice
    pub fn as_slice(&self) -> &[f32] {
        let ptr = self.buffer.contents() as *const f32;
        unsafe { std::slice::from_raw_parts(ptr, self.size) }
    }

    /// Copy data to buffer
    pub fn copy_from_slice(&mut self, data: &[f32]) {
        let ptr = self.buffer.contents() as *mut f32;
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len().min(self.size));
        }
    }
}

/// Batch matrix multiplication with Metal
///
/// Computes batched C = A @ B for multiple matrices.
pub fn batched_gemm_metal(
    ctx: &MetalContext,
    a: &[f32],
    b: &[f32],
    batch_size: usize,
    m: usize,
    n: usize,
    k: usize,
) -> Result<Vec<f32>> {
    if a.len() != batch_size * m * k {
        return Err(RuvLLMError::InvalidOperation(format!(
            "Batched GEMM A size mismatch: {} != {}",
            a.len(),
            batch_size * m * k
        )));
    }
    if b.len() != batch_size * k * n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "Batched GEMM B size mismatch: {} != {}",
            b.len(),
            batch_size * k * n
        )));
    }

    let mut results = Vec::with_capacity(batch_size * m * n);

    // Process each batch
    for batch in 0..batch_size {
        let a_start = batch * m * k;
        let a_end = a_start + m * k;
        let b_start = batch * k * n;
        let b_end = b_start + k * n;

        let c = ctx.gemm_f32(&a[a_start..a_end], &b[b_start..b_end], m, n, k)?;
        results.extend_from_slice(&c);
    }

    Ok(results)
}

/// Fused attention operation
///
/// Computes attention with fused softmax for efficiency.
pub fn fused_attention_metal(
    ctx: &MetalContext,
    query: &[f32],
    key: &[f32],
    value: &[f32],
    config: &AttentionConfig,
) -> Result<Vec<f32>> {
    // Validate inputs
    let q_size = config.num_heads * config.head_dim;
    let kv_size = config.num_kv_heads * config.head_dim;

    if query.len() % q_size != 0 {
        return Err(RuvLLMError::InvalidOperation(format!(
            "Query size {} not divisible by head size {}",
            query.len(),
            q_size
        )));
    }

    ctx.flash_attention(query, key, value, config)
}

/// Layer normalization with Metal
pub fn layer_norm_metal(
    ctx: &MetalContext,
    x: &mut [f32],
    weight: &[f32],
    bias: Option<&[f32]>,
    eps: f32,
) -> Result<()> {
    // RMSNorm as base
    ctx.rms_norm(x, weight, eps)?;

    // Apply bias if provided
    if let Some(bias) = bias {
        for (xi, &bi) in x.iter_mut().zip(bias.iter()) {
            *xi += bi;
        }
    }

    Ok(())
}

/// Fused MLP operation
///
/// Computes: output = down_proj(silu(gate_proj(x)) * up_proj(x))
pub fn fused_mlp_metal(
    ctx: &MetalContext,
    x: &[f32],
    gate_weight: &[f32],
    up_weight: &[f32],
    down_weight: &[f32],
    hidden_size: usize,
    intermediate_size: usize,
) -> Result<Vec<f32>> {
    let batch_size = x.len() / hidden_size;

    // Gate projection: x @ gate_weight^T
    let gate = ctx.gemm_f32(x, gate_weight, batch_size, intermediate_size, hidden_size)?;

    // Up projection: x @ up_weight^T
    let up = ctx.gemm_f32(x, up_weight, batch_size, intermediate_size, hidden_size)?;

    // SiLU and multiply
    let mut hidden: Vec<f32> = gate
        .iter()
        .zip(up.iter())
        .map(|(&g, &u)| {
            let silu = g / (1.0 + (-g).exp());
            silu * u
        })
        .collect();

    // Down projection: hidden @ down_weight^T
    ctx.gemm_f32(
        &hidden,
        down_weight,
        batch_size,
        hidden_size,
        intermediate_size,
    )
}

/// Convert FP32 to FP16
pub fn fp32_to_fp16(data: &[f32]) -> Vec<half::f16> {
    data.iter().map(|&x| half::f16::from_f32(x)).collect()
}

/// Convert FP16 to FP32
pub fn fp16_to_fp32(data: &[half::f16]) -> Vec<f32> {
    data.iter().map(|x| x.to_f32()).collect()
}

/// Quantize to INT8 with scale
pub fn quantize_int8(data: &[f32]) -> (Vec<i8>, f32) {
    let max_abs = data.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let scale = max_abs / 127.0;
    let inv_scale = if scale > 0.0 { 1.0 / scale } else { 0.0 };

    let quantized: Vec<i8> = data
        .iter()
        .map(|&x| (x * inv_scale).round().clamp(-127.0, 127.0) as i8)
        .collect();

    (quantized, scale)
}

/// Dequantize from INT8
pub fn dequantize_int8(data: &[i8], scale: f32) -> Vec<f32> {
    data.iter().map(|&x| x as f32 * scale).collect()
}

/// Memory-efficient attention with chunking
///
/// Processes attention in chunks to reduce peak memory usage.
pub fn chunked_attention_metal(
    ctx: &MetalContext,
    query: &[f32],
    key: &[f32],
    value: &[f32],
    config: &AttentionConfig,
    chunk_size: usize,
) -> Result<Vec<f32>> {
    let q_size = config.num_heads * config.head_dim;
    let kv_size = config.num_kv_heads * config.head_dim;
    let seq_len = query.len() / q_size;
    let kv_len = key.len() / kv_size;

    if seq_len <= chunk_size {
        // No chunking needed
        return ctx.flash_attention(query, key, value, config);
    }

    let mut output = vec![0.0f32; query.len()];

    // Process in chunks
    for chunk_start in (0..seq_len).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(seq_len);
        let chunk_len = chunk_end - chunk_start;

        let q_start = chunk_start * q_size;
        let q_end = chunk_end * q_size;
        let chunk_query = &query[q_start..q_end];

        let chunk_config = AttentionConfig {
            num_heads: config.num_heads,
            num_kv_heads: config.num_kv_heads,
            head_dim: config.head_dim,
            max_seq_len: chunk_len,
            causal: config.causal,
            scale: config.scale,
        };

        let chunk_output = ctx.flash_attention(chunk_query, key, value, &chunk_config)?;

        output[q_start..q_end].copy_from_slice(&chunk_output);
    }

    Ok(output)
}

/// Speculative decoding helper
///
/// Verifies draft tokens against target model.
pub fn verify_speculative_tokens(
    draft_logits: &[f32],
    target_logits: &[f32],
    vocab_size: usize,
    num_draft_tokens: usize,
) -> (usize, Vec<usize>) {
    let mut accepted = Vec::with_capacity(num_draft_tokens);

    for i in 0..num_draft_tokens {
        let draft_start = i * vocab_size;
        let target_start = i * vocab_size;

        // Find argmax for both
        let draft_token = draft_logits[draft_start..draft_start + vocab_size]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let target_token = target_logits[target_start..target_start + vocab_size]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        if draft_token == target_token {
            accepted.push(draft_token);
        } else {
            // First mismatch - accept target token and stop
            accepted.push(target_token);
            break;
        }
    }

    (accepted.len(), accepted)
}

// ============================================================================
// GEMV Metal GPU Operations
// ============================================================================

/// GEMV operation on Metal GPU
///
/// Computes y = A * x where A is (m x n), x is (n), y is (m)
///
/// # Arguments
/// * `context` - Metal context with compiled pipelines
/// * `a` - Matrix A as a slice (m * n elements, row-major)
/// * `x` - Input vector x (n elements)
/// * `m` - Number of rows in A (output dimension)
/// * `n` - Number of columns in A (input dimension)
///
/// # Returns
/// Output vector y (m elements)
///
/// # Performance
/// Target: 100+ GFLOPS on M4 Pro GPU (vs ~35 GFLOPS CPU)
///
/// # Example
/// ```ignore
/// use ruvllm::metal::{MetalContext, MetalConfig, gemv_metal};
///
/// let ctx = MetalContext::new(MetalConfig::default())?;
/// let a = vec![1.0f32; 4096 * 4096]; // 4096x4096 matrix
/// let x = vec![1.0f32; 4096];        // Input vector
/// let y = gemv_metal(&ctx, &a, &x, 4096, 4096)?;
/// ```
#[cfg(target_os = "macos")]
pub fn gemv_metal(
    context: &MetalContext,
    a: &[f32],
    x: &[f32],
    m: usize,
    n: usize,
) -> Result<Vec<f32>> {
    gemv_metal_with_params(context, a, x, m, n, 1.0, 0.0)
}

/// GEMV operation on Metal GPU with alpha/beta scaling
///
/// Computes y = alpha * A * x + beta * y
///
/// # Arguments
/// * `context` - Metal context with compiled pipelines
/// * `a` - Matrix A (m x n), row-major
/// * `x` - Input vector (n)
/// * `m` - Rows of A
/// * `n` - Columns of A
/// * `alpha` - Scale factor for A * x
/// * `beta` - Scale factor for existing y (use 0.0 if y is uninitialized)
///
/// # Returns
/// Output vector y (m)
#[cfg(target_os = "macos")]
pub fn gemv_metal_with_params(
    context: &MetalContext,
    a: &[f32],
    x: &[f32],
    m: usize,
    n: usize,
    alpha: f32,
    beta: f32,
) -> Result<Vec<f32>> {
    use metal::MTLResourceOptions;

    if a.len() != m * n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "GEMV matrix size mismatch: A[{}] != {}x{}",
            a.len(),
            m,
            n
        )));
    }
    if x.len() != n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "GEMV vector size mismatch: x[{}] != {}",
            x.len(),
            n
        )));
    }

    let params = GemvParams::with_scaling(m, n, alpha, beta);

    // Create Metal buffers
    let device = context.device();
    let queue = context.queue();

    let a_buffer = device.new_buffer_with_data(
        a.as_ptr() as *const _,
        (m * n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let x_buffer = device.new_buffer_with_data(
        x.as_ptr() as *const _,
        (n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let y_buffer = device.new_buffer(
        (m * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let params_buffer = device.new_buffer_with_data(
        &params as *const _ as *const _,
        std::mem::size_of::<GemvParams>() as u64,
        MTLResourceOptions::StorageModeShared,
    );

    // Get pipeline - use gemv_optimized_f32 if available, else gemv_simple_f32
    let shader_source = include_str!("shaders/gemv.metal");
    let library = device
        .new_library_with_source(shader_source, &metal::CompileOptions::new())
        .map_err(|e| RuvLLMError::Backend(format!("Failed to compile GEMV shader: {}", e)))?;

    // Try optimized kernel first, fall back to simple
    let function_name = if m >= 4 {
        "gemv_optimized_f32"
    } else {
        "gemv_simple_f32"
    };
    let function = library
        .get_function(function_name, None)
        .map_err(|e| RuvLLMError::Backend(format!("Failed to get GEMV function: {}", e)))?;

    let pipeline = device
        .new_compute_pipeline_state_with_function(&function)
        .map_err(|e| RuvLLMError::Backend(format!("Failed to create GEMV pipeline: {}", e)))?;

    // Execute kernel
    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();

    encoder.set_compute_pipeline_state(&pipeline);
    encoder.set_buffer(0, Some(&a_buffer), 0);
    encoder.set_buffer(1, Some(&x_buffer), 0);
    encoder.set_buffer(2, Some(&y_buffer), 0);
    encoder.set_buffer(3, Some(&params_buffer), 0);

    // Grid and threadgroup configuration
    // gemv_optimized_f32: 32 threads per row, 4 rows per block
    let rows_per_block = 4;
    let threads_per_row = 32;
    let num_blocks = (m + rows_per_block - 1) / rows_per_block;

    if m >= 4 {
        // Optimized kernel
        let threadgroup_size = MTLSize::new(threads_per_row as u64, rows_per_block as u64, 1);
        let grid_size = MTLSize::new(num_blocks as u64, 1, 1);
        encoder.dispatch_thread_groups(grid_size, threadgroup_size);
    } else {
        // Simple kernel - one thread per row
        let threadgroup_size = MTLSize::new(256.min(m as u64), 1, 1);
        let num_groups = (m + 255) / 256;
        let grid_size = MTLSize::new(num_groups as u64 * threadgroup_size.width, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
    }

    encoder.end_encoding();

    command_buffer.commit();
    command_buffer.wait_until_completed();

    // Read back results
    let ptr = y_buffer.contents() as *const f32;
    let mut result = vec![0.0f32; m];
    unsafe {
        std::ptr::copy_nonoverlapping(ptr, result.as_mut_ptr(), m);
    }

    Ok(result)
}

/// GEMV operation on Metal GPU with FP16 precision
///
/// Computes y = A * x using half-precision for matrix A and vector x.
/// Achieves approximately 2x throughput compared to FP32.
///
/// # Arguments
/// * `context` - Metal context
/// * `a` - Matrix A (m x n) in FP16
/// * `x` - Input vector (n) in FP16
/// * `m` - Rows of A
/// * `n` - Columns of A
///
/// # Returns
/// Output vector y (m) in FP16
#[cfg(target_os = "macos")]
pub fn gemv_metal_f16(
    context: &MetalContext,
    a: &[half::f16],
    x: &[half::f16],
    m: usize,
    n: usize,
) -> Result<Vec<half::f16>> {
    use metal::MTLResourceOptions;

    if a.len() != m * n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "GEMV matrix size mismatch: A[{}] != {}x{}",
            a.len(),
            m,
            n
        )));
    }
    if x.len() != n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "GEMV vector size mismatch: x[{}] != {}",
            x.len(),
            n
        )));
    }

    let params = GemvParams::new(m, n);

    let device = context.device();
    let queue = context.queue();

    let a_buffer = device.new_buffer_with_data(
        a.as_ptr() as *const _,
        (m * n * std::mem::size_of::<half::f16>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let x_buffer = device.new_buffer_with_data(
        x.as_ptr() as *const _,
        (n * std::mem::size_of::<half::f16>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let y_buffer = device.new_buffer(
        (m * std::mem::size_of::<half::f16>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let params_buffer = device.new_buffer_with_data(
        &params as *const _ as *const _,
        std::mem::size_of::<GemvParams>() as u64,
        MTLResourceOptions::StorageModeShared,
    );

    let shader_source = include_str!("shaders/gemv.metal");
    let library = device
        .new_library_with_source(shader_source, &metal::CompileOptions::new())
        .map_err(|e| RuvLLMError::Backend(format!("Failed to compile GEMV shader: {}", e)))?;

    let function = library
        .get_function("gemv_optimized_f16", None)
        .map_err(|e| RuvLLMError::Backend(format!("Failed to get GEMV F16 function: {}", e)))?;

    let pipeline = device
        .new_compute_pipeline_state_with_function(&function)
        .map_err(|e| RuvLLMError::Backend(format!("Failed to create GEMV F16 pipeline: {}", e)))?;

    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();

    encoder.set_compute_pipeline_state(&pipeline);
    encoder.set_buffer(0, Some(&a_buffer), 0);
    encoder.set_buffer(1, Some(&x_buffer), 0);
    encoder.set_buffer(2, Some(&y_buffer), 0);
    encoder.set_buffer(3, Some(&params_buffer), 0);

    let rows_per_block = 4;
    let threads_per_row = 32;
    let num_blocks = (m + rows_per_block - 1) / rows_per_block;

    let threadgroup_size = MTLSize::new(threads_per_row as u64, rows_per_block as u64, 1);
    let grid_size = MTLSize::new(num_blocks as u64, 1, 1);
    encoder.dispatch_thread_groups(grid_size, threadgroup_size);

    encoder.end_encoding();

    command_buffer.commit();
    command_buffer.wait_until_completed();

    let ptr = y_buffer.contents() as *const half::f16;
    let mut result = vec![half::f16::from_f32(0.0); m];
    unsafe {
        std::ptr::copy_nonoverlapping(ptr, result.as_mut_ptr(), m);
    }

    Ok(result)
}

/// Batched GEMV on Metal GPU
///
/// Computes y[b] = A[b] * x[b] for each batch element
///
/// # Arguments
/// * `context` - Metal context
/// * `a` - Batched matrices (batch_size, m, n)
/// * `x` - Batched input vectors (batch_size, n)
/// * `batch_size` - Number of batches
/// * `m` - Rows per matrix
/// * `n` - Columns per matrix
///
/// # Returns
/// Batched output vectors (batch_size, m)
#[cfg(target_os = "macos")]
pub fn gemv_batched_metal(
    context: &MetalContext,
    a: &[f32],
    x: &[f32],
    batch_size: usize,
    m: usize,
    n: usize,
) -> Result<Vec<f32>> {
    use metal::MTLResourceOptions;

    if a.len() != batch_size * m * n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "Batched GEMV A size mismatch: {} != {}",
            a.len(),
            batch_size * m * n
        )));
    }
    if x.len() != batch_size * n {
        return Err(RuvLLMError::InvalidOperation(format!(
            "Batched GEMV x size mismatch: {} != {}",
            x.len(),
            batch_size * n
        )));
    }

    let device = context.device();
    let queue = context.queue();

    let a_buffer = device.new_buffer_with_data(
        a.as_ptr() as *const _,
        (batch_size * m * n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let x_buffer = device.new_buffer_with_data(
        x.as_ptr() as *const _,
        (batch_size * n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let y_buffer = device.new_buffer(
        (batch_size * m * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );

    // dims: (m, n, batch_size, 0)
    let dims: [u32; 4] = [m as u32, n as u32, batch_size as u32, 0];
    let dims_buffer = device.new_buffer_with_data(
        dims.as_ptr() as *const _,
        (4 * std::mem::size_of::<u32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );

    let shader_source = include_str!("shaders/gemv.metal");
    let library = device
        .new_library_with_source(shader_source, &metal::CompileOptions::new())
        .map_err(|e| RuvLLMError::Backend(format!("Failed to compile GEMV shader: {}", e)))?;

    let function = library
        .get_function("batched_gemv_f32", None)
        .map_err(|e| RuvLLMError::Backend(format!("Failed to get batched GEMV function: {}", e)))?;

    let pipeline = device
        .new_compute_pipeline_state_with_function(&function)
        .map_err(|e| {
            RuvLLMError::Backend(format!("Failed to create batched GEMV pipeline: {}", e))
        })?;

    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();

    encoder.set_compute_pipeline_state(&pipeline);
    encoder.set_buffer(0, Some(&a_buffer), 0);
    encoder.set_buffer(1, Some(&x_buffer), 0);
    encoder.set_buffer(2, Some(&y_buffer), 0);
    encoder.set_buffer(3, Some(&dims_buffer), 0);

    let rows_per_block = 4;
    let threads_per_row = 32;
    let num_row_blocks = (m + rows_per_block - 1) / rows_per_block;

    let threadgroup_size = MTLSize::new(threads_per_row as u64, rows_per_block as u64, 1);
    let grid_size = MTLSize::new(num_row_blocks as u64, batch_size as u64, 1);
    encoder.dispatch_thread_groups(grid_size, threadgroup_size);

    encoder.end_encoding();

    command_buffer.commit();
    command_buffer.wait_until_completed();

    let ptr = y_buffer.contents() as *const f32;
    let mut result = vec![0.0f32; batch_size * m];
    unsafe {
        std::ptr::copy_nonoverlapping(ptr, result.as_mut_ptr(), batch_size * m);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fp16_conversion() {
        let data = vec![1.0f32, 2.0, -3.0, 0.5];
        let fp16 = fp32_to_fp16(&data);
        let back = fp16_to_fp32(&fp16);

        for (orig, converted) in data.iter().zip(back.iter()) {
            assert!((orig - converted).abs() < 0.01);
        }
    }

    #[test]
    fn test_int8_quantization() {
        let data = vec![1.0f32, -1.0, 0.5, -0.5, 0.0];
        let (quantized, scale) = quantize_int8(&data);
        let dequantized = dequantize_int8(&quantized, scale);

        for (orig, converted) in data.iter().zip(dequantized.iter()) {
            assert!((orig - converted).abs() < 0.02);
        }
    }

    #[test]
    fn test_speculative_verification() {
        let vocab_size = 10;
        let num_tokens = 3;

        // Draft: tokens 5, 3, 7
        let mut draft_logits = vec![0.0f32; vocab_size * num_tokens];
        draft_logits[5] = 10.0;
        draft_logits[vocab_size + 3] = 10.0;
        draft_logits[2 * vocab_size + 7] = 10.0;

        // Target: tokens 5, 3, 2 (mismatch at position 2)
        let mut target_logits = vec![0.0f32; vocab_size * num_tokens];
        target_logits[5] = 10.0;
        target_logits[vocab_size + 3] = 10.0;
        target_logits[2 * vocab_size + 2] = 10.0;

        let (num_accepted, tokens) =
            verify_speculative_tokens(&draft_logits, &target_logits, vocab_size, num_tokens);

        assert_eq!(num_accepted, 3); // 2 accepted + 1 target correction
        assert_eq!(tokens, vec![5, 3, 2]);
    }

    #[test]
    fn test_gemv_params() {
        let params = GemvParams::new(4096, 4096);
        assert_eq!(params.m, 4096);
        assert_eq!(params.n, 4096);
        assert_eq!(params.lda, 4096);
        assert_eq!(params.alpha, 1.0);
        assert_eq!(params.beta, 0.0);

        let params_scaled = GemvParams::with_scaling(1024, 2048, 2.0, 0.5);
        assert_eq!(params_scaled.m, 1024);
        assert_eq!(params_scaled.n, 2048);
        assert_eq!(params_scaled.alpha, 2.0);
        assert_eq!(params_scaled.beta, 0.5);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_gemv_metal_basic() {
        use super::super::MetalContext;

        if !super::super::is_metal_available() {
            println!("Metal not available, skipping test");
            return;
        }

        let ctx = MetalContext::new(super::super::MetalConfig::default()).unwrap();

        // Simple 4x4 identity-like test
        // A = [[1,0,0,0], [0,1,0,0], [0,0,1,0], [0,0,0,1]]
        // x = [1, 2, 3, 4]
        // y = [1, 2, 3, 4]
        let m = 4;
        let n = 4;
        let mut a = vec![0.0f32; m * n];
        for i in 0..m {
            a[i * n + i] = 1.0;
        }
        let x = vec![1.0f32, 2.0, 3.0, 4.0];

        let result = gemv_metal(&ctx, &a, &x, m, n);
        assert!(result.is_ok(), "GEMV Metal failed: {:?}", result.err());

        let y = result.unwrap();
        assert_eq!(y.len(), m);

        // For identity matrix, y should equal x
        for i in 0..m {
            assert!(
                (y[i] - x[i]).abs() < 1e-5,
                "Mismatch at {}: {} vs {}",
                i,
                y[i],
                x[i]
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_gemv_metal_larger() {
        use super::super::MetalContext;

        if !super::super::is_metal_available() {
            println!("Metal not available, skipping test");
            return;
        }

        let ctx = MetalContext::new(super::super::MetalConfig::default()).unwrap();

        // Test with a larger matrix for better GPU utilization
        let m = 256;
        let n = 256;

        // A is all 1s, x is all 1s, so y should be all n (256)
        let a = vec![1.0f32; m * n];
        let x = vec![1.0f32; n];

        let result = gemv_metal(&ctx, &a, &x, m, n);
        assert!(result.is_ok(), "GEMV Metal failed: {:?}", result.err());

        let y = result.unwrap();
        assert_eq!(y.len(), m);

        let expected = n as f32;
        for i in 0..m {
            assert!(
                (y[i] - expected).abs() < 1e-3,
                "Mismatch at {}: {} vs {}",
                i,
                y[i],
                expected
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_gemv_metal_correctness() {
        use super::super::MetalContext;

        if !super::super::is_metal_available() {
            println!("Metal not available, skipping test");
            return;
        }

        let ctx = MetalContext::new(super::super::MetalConfig::default()).unwrap();

        // Test with specific values
        // A = [[1, 2, 3],
        //      [4, 5, 6]]
        // x = [1, 2, 3]
        // y = [1*1 + 2*2 + 3*3, 4*1 + 5*2 + 6*3] = [14, 32]
        let m = 2;
        let n = 3;
        let a = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = vec![1.0f32, 2.0, 3.0];

        let result = gemv_metal(&ctx, &a, &x, m, n);
        assert!(result.is_ok());

        let y = result.unwrap();
        assert_eq!(y.len(), 2);
        assert!((y[0] - 14.0).abs() < 1e-4, "y[0] = {}, expected 14", y[0]);
        assert!((y[1] - 32.0).abs() < 1e-4, "y[1] = {}, expected 32", y[1]);
    }
}
