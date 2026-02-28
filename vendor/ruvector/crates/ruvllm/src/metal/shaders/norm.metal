//
// Normalization Kernels - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro with SIMD reductions
//
// Includes:
// - RMSNorm (Root Mean Square Layer Normalization)
// - LayerNorm (Layer Normalization)
// - Fused normalization + residual operations
//
// Optimizations:
// - SIMD reduction (simd_sum) for parallel sum computation
// - Vectorized memory access (float4)
// - Fused operations to reduce memory bandwidth
// - 16-byte aligned threadgroup memory
//

#include <metal_stdlib>
using namespace metal;

// Constants
constant uint SIMD_SIZE = 32;
constant uint MAX_THREADS = 1024;

// Normalization parameters structure (matches Rust NormParams)
struct NormParams {
    uint hidden_size;       // Hidden dimension
    float eps;              // Epsilon for numerical stability
    uint elements_per_thread;  // Elements per thread for distribution
    uint _padding;          // Alignment padding
};

// =============================================================================
// High-Performance RMSNorm with SIMD reduction
// RMSNorm: x * weight / sqrt(mean(x^2) + eps)
// Used in LLaMA, Mistral, and other modern LLMs
// =============================================================================
kernel void rms_norm_v2(
    device float* x [[buffer(0)]],
    device const float* weight [[buffer(1)]],
    constant NormParams& params [[buffer(2)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;

    uint offset = batch_idx * hidden_size;

    // Shared memory for warp-level reduction results
    threadgroup float warp_sums[32] __attribute__((aligned(16)));

    // Step 1: Compute sum of squares with vectorized loads and SIMD reduction
    float local_sum = 0.0f;

    // Process 4 elements at a time using float4
    const uint vec_size = hidden_size / 4;
    const device float4* x_vec = reinterpret_cast<const device float4*>(x + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = x_vec[i];
        local_sum += val.x * val.x + val.y * val.y + val.z * val.z + val.w * val.w;
    }

    // Handle remainder
    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float val = x[offset + i];
        local_sum += val * val;
    }

    // SIMD reduction within warp
    local_sum = simd_sum(local_sum);

    // Store warp results
    if (simd_lane == 0) {
        warp_sums[simd_group] = local_sum;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Final reduction across warps (first warp only)
    float total_sum = 0.0f;
    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            total_sum = warp_sums[simd_lane];
        }
        total_sum = simd_sum(total_sum);

        if (simd_lane == 0) {
            warp_sums[0] = total_sum;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Compute inverse RMS
    float inv_rms = rsqrt(warp_sums[0] / float(hidden_size) + eps);

    // Step 2: Normalize and apply weight with vectorized stores
    device float4* out_vec = reinterpret_cast<device float4*>(x + offset);
    const device float4* w_vec = reinterpret_cast<const device float4*>(weight);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = x_vec[i];
        float4 w = w_vec[i];
        out_vec[i] = val * inv_rms * w;
    }

    // Handle remainder
    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        x[offset + i] = x[offset + i] * inv_rms * weight[i];
    }
}

// =============================================================================
// Original RMSNorm (kept for compatibility)
// =============================================================================
kernel void rms_norm(
    device float* x [[buffer(0)]],
    device const float* weight [[buffer(1)]],
    constant NormParams& params [[buffer(2)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;
    uint offset = batch_idx * hidden_size;

    threadgroup float shared_sum[MAX_THREADS];

    float local_sum = 0.0f;
    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float val = x[offset + i];
        local_sum = fma(val, val, local_sum);
    }

    // SIMD reduction first
    local_sum = simd_sum(local_sum);
    shared_sum[tid / SIMD_SIZE] = local_sum;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Reduce across warps
    if (tid < threads_per_group / SIMD_SIZE) {
        local_sum = shared_sum[tid];
    } else {
        local_sum = 0.0f;
    }
    local_sum = simd_sum(local_sum);

    float inv_rms = rsqrt(local_sum / float(hidden_size) + eps);

    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        x[offset + i] = x[offset + i] * inv_rms * weight[i];
    }
}

// =============================================================================
// LayerNorm with SIMD reduction
// LayerNorm: (x - mean) / sqrt(var + eps) * weight + bias
// =============================================================================
kernel void layer_norm_v2(
    device float* x [[buffer(0)]],
    device const float* weight [[buffer(1)]],
    device const float* bias [[buffer(2)]],
    constant NormParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;
    uint offset = batch_idx * hidden_size;

    threadgroup float warp_sum[32] __attribute__((aligned(16)));
    threadgroup float warp_sum_sq[32] __attribute__((aligned(16)));

    // Compute sum and sum of squares with vectorization
    float local_sum = 0.0f;
    float local_sum_sq = 0.0f;

    const uint vec_size = hidden_size / 4;
    const device float4* x_vec = reinterpret_cast<const device float4*>(x + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = x_vec[i];
        local_sum += val.x + val.y + val.z + val.w;
        local_sum_sq += val.x * val.x + val.y * val.y + val.z * val.z + val.w * val.w;
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float val = x[offset + i];
        local_sum += val;
        local_sum_sq = fma(val, val, local_sum_sq);
    }

    // SIMD reduction
    local_sum = simd_sum(local_sum);
    local_sum_sq = simd_sum(local_sum_sq);

    if (simd_lane == 0) {
        warp_sum[simd_group] = local_sum;
        warp_sum_sq[simd_group] = local_sum_sq;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Final reduction
    float total_sum = 0.0f;
    float total_sum_sq = 0.0f;
    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            total_sum = warp_sum[simd_lane];
            total_sum_sq = warp_sum_sq[simd_lane];
        }
        total_sum = simd_sum(total_sum);
        total_sum_sq = simd_sum(total_sum_sq);

        if (simd_lane == 0) {
            warp_sum[0] = total_sum;
            warp_sum_sq[0] = total_sum_sq;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    float mean = warp_sum[0] / float(hidden_size);
    float var = warp_sum_sq[0] / float(hidden_size) - mean * mean;
    float inv_std = rsqrt(var + eps);

    // Normalize with vectorization
    device float4* out_vec = reinterpret_cast<device float4*>(x + offset);
    const device float4* w_vec = reinterpret_cast<const device float4*>(weight);
    const device float4* b_vec = bias ? reinterpret_cast<const device float4*>(bias) : nullptr;

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = x_vec[i];
        float4 normalized = (val - mean) * inv_std;
        float4 w = w_vec[i];
        float4 b = b_vec ? b_vec[i] : float4(0.0f);
        out_vec[i] = fma(normalized, w, b);
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float normalized = (x[offset + i] - mean) * inv_std;
        float bias_val = bias ? bias[i] : 0.0f;
        x[offset + i] = fma(normalized, weight[i], bias_val);
    }
}

// =============================================================================
// Original LayerNorm (kept for compatibility)
// =============================================================================
kernel void layer_norm(
    device float* x [[buffer(0)]],
    device const float* weight [[buffer(1)]],
    device const float* bias [[buffer(2)]],
    constant NormParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;
    uint offset = batch_idx * hidden_size;

    threadgroup float shared_sum[MAX_THREADS];
    threadgroup float shared_sum_sq[MAX_THREADS];

    float local_sum = 0.0f;
    float local_sum_sq = 0.0f;

    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float val = x[offset + i];
        local_sum += val;
        local_sum_sq = fma(val, val, local_sum_sq);
    }

    // SIMD reduction
    local_sum = simd_sum(local_sum);
    local_sum_sq = simd_sum(local_sum_sq);

    shared_sum[tid / SIMD_SIZE] = local_sum;
    shared_sum_sq[tid / SIMD_SIZE] = local_sum_sq;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (tid < threads_per_group / SIMD_SIZE) {
        local_sum = shared_sum[tid];
        local_sum_sq = shared_sum_sq[tid];
    } else {
        local_sum = 0.0f;
        local_sum_sq = 0.0f;
    }
    local_sum = simd_sum(local_sum);
    local_sum_sq = simd_sum(local_sum_sq);

    float mean = local_sum / float(hidden_size);
    float var = local_sum_sq / float(hidden_size) - mean * mean;
    float inv_std = rsqrt(var + eps);

    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float normalized = (x[offset + i] - mean) * inv_std;
        float bias_val = bias ? bias[i] : 0.0f;
        x[offset + i] = fma(normalized, weight[i], bias_val);
    }
}

// =============================================================================
// Fused RMSNorm + Residual Addition
// Computes: residual = x + residual; output = RMSNorm(residual) * weight
// Single pass through memory for better bandwidth utilization
// =============================================================================
kernel void rms_norm_residual_v2(
    device float* x [[buffer(0)]],
    device float* residual [[buffer(1)]],
    device const float* weight [[buffer(2)]],
    constant NormParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;
    uint offset = batch_idx * hidden_size;

    threadgroup float warp_sums[32] __attribute__((aligned(16)));
    threadgroup float temp_data[MAX_THREADS] __attribute__((aligned(16)));

    // Step 1: Add residual and compute sum of squares in one pass
    float local_sum = 0.0f;

    const uint vec_size = hidden_size / 4;
    device float4* x_vec = reinterpret_cast<device float4*>(x + offset);
    device float4* res_vec = reinterpret_cast<device float4*>(residual + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 x_val = x_vec[i];
        float4 r_val = res_vec[i];
        float4 sum_val = x_val + r_val;

        // Store sum back to residual
        res_vec[i] = sum_val;

        // Accumulate sum of squares
        local_sum += sum_val.x * sum_val.x + sum_val.y * sum_val.y +
                     sum_val.z * sum_val.z + sum_val.w * sum_val.w;
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float sum_val = x[offset + i] + residual[offset + i];
        residual[offset + i] = sum_val;
        local_sum = fma(sum_val, sum_val, local_sum);
    }

    // SIMD reduction
    local_sum = simd_sum(local_sum);

    if (simd_lane == 0) {
        warp_sums[simd_group] = local_sum;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Final reduction
    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            local_sum = warp_sums[simd_lane];
        } else {
            local_sum = 0.0f;
        }
        local_sum = simd_sum(local_sum);

        if (simd_lane == 0) {
            warp_sums[0] = local_sum;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    float inv_rms = rsqrt(warp_sums[0] / float(hidden_size) + eps);

    // Step 2: Normalize from residual and write to x
    const device float4* w_vec = reinterpret_cast<const device float4*>(weight);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = res_vec[i];
        float4 w = w_vec[i];
        x_vec[i] = val * inv_rms * w;
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        x[offset + i] = residual[offset + i] * inv_rms * weight[i];
    }
}

// =============================================================================
// Original Fused RMSNorm + Residual (kept for compatibility)
// =============================================================================
kernel void rms_norm_residual(
    device float* x [[buffer(0)]],
    device float* residual [[buffer(1)]],
    device const float* weight [[buffer(2)]],
    constant NormParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;
    uint offset = batch_idx * hidden_size;

    threadgroup float shared_sum[MAX_THREADS];

    float local_sum = 0.0f;
    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float val = x[offset + i] + residual[offset + i];
        residual[offset + i] = val;
        local_sum = fma(val, val, local_sum);
    }

    local_sum = simd_sum(local_sum);
    shared_sum[tid / SIMD_SIZE] = local_sum;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (tid < threads_per_group / SIMD_SIZE) {
        local_sum = shared_sum[tid];
    } else {
        local_sum = 0.0f;
    }
    local_sum = simd_sum(local_sum);

    float inv_rms = rsqrt(local_sum / float(hidden_size) + eps);

    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        x[offset + i] = residual[offset + i] * inv_rms * weight[i];
    }
}

// =============================================================================
// FP16 RMSNorm with SIMD reduction
// =============================================================================
kernel void rms_norm_f16_v2(
    device half* x [[buffer(0)]],
    device const half* weight [[buffer(1)]],
    constant NormParams& params [[buffer(2)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    float eps = params.eps;
    uint offset = batch_idx * hidden_size;

    threadgroup float warp_sums[32] __attribute__((aligned(16)));

    // Compute sum of squares (use FP32 for accuracy)
    float local_sum = 0.0f;

    const uint vec_size = hidden_size / 4;
    const device half4* x_vec = reinterpret_cast<const device half4*>(x + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = float4(x_vec[i]);
        local_sum += val.x * val.x + val.y * val.y + val.z * val.z + val.w * val.w;
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float val = float(x[offset + i]);
        local_sum = fma(val, val, local_sum);
    }

    // SIMD reduction
    local_sum = simd_sum(local_sum);

    if (simd_lane == 0) {
        warp_sums[simd_group] = local_sum;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            local_sum = warp_sums[simd_lane];
        } else {
            local_sum = 0.0f;
        }
        local_sum = simd_sum(local_sum);

        if (simd_lane == 0) {
            warp_sums[0] = local_sum;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    half inv_rms = half(rsqrt(warp_sums[0] / float(hidden_size) + eps));

    // Normalize with vectorization
    device half4* out_vec = reinterpret_cast<device half4*>(x + offset);
    const device half4* w_vec = reinterpret_cast<const device half4*>(weight);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        half4 val = x_vec[i];
        half4 w = w_vec[i];
        out_vec[i] = val * inv_rms * w;
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        x[offset + i] = x[offset + i] * inv_rms * weight[i];
    }
}

// =============================================================================
// Original FP16 RMSNorm (kept for compatibility)
// =============================================================================
kernel void rms_norm_f16(
    device half* x [[buffer(0)]],
    device const half* weight [[buffer(1)]],
    constant NormParams& params [[buffer(2)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint hidden_size = params.hidden_size;
    half eps = half(params.eps);
    uint offset = batch_idx * hidden_size;

    threadgroup float shared_sum[MAX_THREADS];

    float local_sum = 0.0f;
    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float val = float(x[offset + i]);
        local_sum = fma(val, val, local_sum);
    }

    local_sum = simd_sum(local_sum);
    shared_sum[tid / SIMD_SIZE] = local_sum;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (tid < threads_per_group / SIMD_SIZE) {
        local_sum = shared_sum[tid];
    } else {
        local_sum = 0.0f;
    }
    local_sum = simd_sum(local_sum);

    half inv_rms = half(rsqrt(local_sum / float(hidden_size) + float(eps)));

    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        x[offset + i] = x[offset + i] * inv_rms * weight[i];
    }
}

// =============================================================================
// Group RMSNorm with SIMD reduction
// =============================================================================
kernel void group_rms_norm(
    device float* x [[buffer(0)]],
    device const float* weight [[buffer(1)]],
    constant uint& num_groups [[buffer(2)]],
    constant uint& channels_per_group [[buffer(3)]],
    constant float& eps [[buffer(4)]],
    uint3 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.z;
    uint group_idx = gid.y;
    uint channels = num_groups * channels_per_group;
    uint group_offset = group_idx * channels_per_group;

    threadgroup float warp_sums[32] __attribute__((aligned(16)));

    float local_sum = 0.0f;
    for (uint c = tid; c < channels_per_group; c += threads_per_group) {
        uint idx = batch_idx * channels + group_offset + c;
        float val = x[idx];
        local_sum = fma(val, val, local_sum);
    }

    local_sum = simd_sum(local_sum);

    if (simd_lane == 0) {
        warp_sums[simd_group] = local_sum;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            local_sum = warp_sums[simd_lane];
        } else {
            local_sum = 0.0f;
        }
        local_sum = simd_sum(local_sum);

        if (simd_lane == 0) {
            warp_sums[0] = local_sum;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    float inv_rms = rsqrt(warp_sums[0] / float(channels_per_group) + eps);

    for (uint c = tid; c < channels_per_group; c += threads_per_group) {
        uint idx = batch_idx * channels + group_offset + c;
        x[idx] = x[idx] * inv_rms * weight[group_offset + c];
    }
}

// =============================================================================
// Fused LayerNorm + Linear projection (common in transformers)
// output = Linear(LayerNorm(x)) = W @ LayerNorm(x) + b
// =============================================================================
// Maximum supported hidden_size for layer_norm_linear_fused kernel
// Metal threadgroup memory is limited and we use static arrays for performance
constant uint MAX_HIDDEN_SIZE_FUSED = 1024;

kernel void layer_norm_linear_fused(
    device const float* x [[buffer(0)]],
    device const float* ln_weight [[buffer(1)]],
    device const float* ln_bias [[buffer(2)]],
    device const float* linear_weight [[buffer(3)]],  // [out_features, hidden_size]
    device const float* linear_bias [[buffer(4)]],    // [out_features]
    device float* output [[buffer(5)]],
    constant uint& hidden_size [[buffer(6)]],
    constant uint& out_features [[buffer(7)]],
    constant float& eps [[buffer(8)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    uint batch_idx = gid.y;
    uint out_idx = gid.x;

    if (out_idx >= out_features) return;

    // SECURITY FIX: Guard against buffer overflow in threadgroup memory
    // The normalized array is statically sized to MAX_HIDDEN_SIZE_FUSED (1024)
    // Models with larger hidden dimensions should use the non-fused kernel instead
    if (hidden_size > MAX_HIDDEN_SIZE_FUSED) return;

    uint x_offset = batch_idx * hidden_size;

    threadgroup float warp_sum[32];
    threadgroup float warp_sum_sq[32];
    threadgroup float normalized[MAX_HIDDEN_SIZE_FUSED];  // SECURITY: Using constant for clarity

    // Step 1: Compute mean and variance with SIMD reduction
    float local_sum = 0.0f;
    float local_sum_sq = 0.0f;

    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float val = x[x_offset + i];
        local_sum += val;
        local_sum_sq = fma(val, val, local_sum_sq);
    }

    local_sum = simd_sum(local_sum);
    local_sum_sq = simd_sum(local_sum_sq);

    if (simd_lane == 0) {
        warp_sum[simd_group] = local_sum;
        warp_sum_sq[simd_group] = local_sum_sq;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            local_sum = warp_sum[simd_lane];
            local_sum_sq = warp_sum_sq[simd_lane];
        } else {
            local_sum = 0.0f;
            local_sum_sq = 0.0f;
        }
        local_sum = simd_sum(local_sum);
        local_sum_sq = simd_sum(local_sum_sq);

        if (simd_lane == 0) {
            warp_sum[0] = local_sum;
            warp_sum_sq[0] = local_sum_sq;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    float mean = warp_sum[0] / float(hidden_size);
    float var = warp_sum_sq[0] / float(hidden_size) - mean * mean;
    float inv_std = rsqrt(var + eps);

    // Step 2: Normalize and store in shared memory
    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        float val = (x[x_offset + i] - mean) * inv_std;
        float bias_val = ln_bias ? ln_bias[i] : 0.0f;
        normalized[i] = fma(val, ln_weight[i], bias_val);
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Step 3: Linear projection (dot product with weight row)
    float dot = 0.0f;
    for (uint i = tid; i < hidden_size; i += threads_per_group) {
        dot = fma(normalized[i], linear_weight[out_idx * hidden_size + i], dot);
    }

    dot = simd_sum(dot);

    if (simd_lane == 0) {
        warp_sum[simd_group] = dot;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (simd_group == 0 && simd_lane == 0) {
        float total = 0.0f;
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        for (uint w = 0; w < num_warps; w++) {
            total += warp_sum[w];
        }
        float bias = linear_bias ? linear_bias[out_idx] : 0.0f;
        output[batch_idx * out_features + out_idx] = total + bias;
    }
}
