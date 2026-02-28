//
// Fused Operations - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro
//
// Contains fused operations for reduced memory bandwidth:
// - Fused LayerNorm + Residual: output = LayerNorm(x + residual) * scale + bias
// - Fused RMSNorm + Residual
// - Fused SwiGLU (gate * swish(up))
// - Fused bias + activation
//
// M4 Pro Optimizations:
// - Single-pass mean and variance computation
// - SIMD reductions for parallel statistics
// - Vectorized memory access (float4/half4)
// - 1024 threads per threadgroup
//

#include <metal_stdlib>
using namespace metal;

// ============================================================================
// Constants
// ============================================================================
constant uint SIMD_SIZE = 32;
constant uint MAX_THREADS = 1024;
constant uint MAX_HIDDEN = 8192;

// ============================================================================
// Fused Normalization Parameters
// ============================================================================
struct FusedNormParams {
    uint hidden_size;    // Hidden dimension
    uint batch_size;     // Batch size
    float eps;           // Epsilon for numerical stability
    uint has_bias;       // Whether bias is present
};

// ============================================================================
// FUSED LAYERNORM + RESIDUAL
// Computes: output = LayerNorm(x + residual) * gamma + beta
// Single pass through memory for better bandwidth utilization
// ============================================================================
kernel void fused_layernorm_residual(
    device const float* x [[buffer(0)]],
    device const float* residual [[buffer(1)]],
    device const float* gamma [[buffer(2)]],
    device const float* beta [[buffer(3)]],
    device float* output [[buffer(4)]],
    constant FusedNormParams& params [[buffer(5)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint batch_idx = gid.y;
    const uint hidden_size = params.hidden_size;
    const float eps = params.eps;

    if (batch_idx >= params.batch_size) return;

    const uint offset = batch_idx * hidden_size;

    // Shared memory for warp reduction results
    threadgroup float warp_sum[32] __attribute__((aligned(16)));
    threadgroup float warp_sum_sq[32] __attribute__((aligned(16)));

    // PASS 1: Compute sum and sum of squares with fused residual add
    // Use vectorized loads for coalesced access
    float local_sum = 0.0f;
    float local_sum_sq = 0.0f;

    const uint vec_size = hidden_size / 4;
    const device float4* x_vec = reinterpret_cast<const device float4*>(x + offset);
    const device float4* res_vec = reinterpret_cast<const device float4*>(residual + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 x_val = x_vec[i];
        float4 r_val = res_vec[i];
        float4 sum_val = x_val + r_val;

        local_sum += sum_val.x + sum_val.y + sum_val.z + sum_val.w;
        local_sum_sq += sum_val.x * sum_val.x + sum_val.y * sum_val.y +
                        sum_val.z * sum_val.z + sum_val.w * sum_val.w;
    }

    // Handle remainder
    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float sum_val = x[offset + i] + residual[offset + i];
        local_sum += sum_val;
        local_sum_sq += sum_val * sum_val;
    }

    // SIMD reduction within warp
    local_sum = simd_sum(local_sum);
    local_sum_sq = simd_sum(local_sum_sq);

    // Store warp results
    if (simd_lane == 0) {
        warp_sum[simd_group] = local_sum;
        warp_sum_sq[simd_group] = local_sum_sq;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Final reduction across warps (first warp only)
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

    // Compute mean and variance
    float mean = warp_sum[0] / float(hidden_size);
    float var = warp_sum_sq[0] / float(hidden_size) - mean * mean;
    float inv_std = rsqrt(var + eps);

    // PASS 2: Normalize and apply gamma/beta
    device float4* out_vec = reinterpret_cast<device float4*>(output + offset);
    const device float4* g_vec = reinterpret_cast<const device float4*>(gamma);
    const device float4* b_vec = params.has_bias ? reinterpret_cast<const device float4*>(beta) : nullptr;

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 sum_val = x_vec[i] + res_vec[i];
        float4 normalized = (sum_val - mean) * inv_std;
        float4 g = g_vec[i];
        float4 b = b_vec ? b_vec[i] : float4(0.0f);
        out_vec[i] = fma(normalized, g, b);
    }

    // Handle remainder
    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float sum_val = x[offset + i] + residual[offset + i];
        float normalized = (sum_val - mean) * inv_std;
        float bias_val = params.has_bias ? beta[i] : 0.0f;
        output[offset + i] = fma(normalized, gamma[i], bias_val);
    }
}

// ============================================================================
// FUSED LAYERNORM + RESIDUAL FP16
// FP16 version with FP32 accumulator for numerical stability
// ============================================================================
kernel void fused_layernorm_residual_f16(
    device const half* x [[buffer(0)]],
    device const half* residual [[buffer(1)]],
    device const half* gamma [[buffer(2)]],
    device const half* beta [[buffer(3)]],
    device half* output [[buffer(4)]],
    constant FusedNormParams& params [[buffer(5)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint batch_idx = gid.y;
    const uint hidden_size = params.hidden_size;
    const float eps = params.eps;

    if (batch_idx >= params.batch_size) return;

    const uint offset = batch_idx * hidden_size;

    threadgroup float warp_sum[32] __attribute__((aligned(16)));
    threadgroup float warp_sum_sq[32] __attribute__((aligned(16)));

    // FP32 accumulation for numerical stability
    float local_sum = 0.0f;
    float local_sum_sq = 0.0f;

    const uint vec_size = hidden_size / 4;
    const device half4* x_vec = reinterpret_cast<const device half4*>(x + offset);
    const device half4* res_vec = reinterpret_cast<const device half4*>(residual + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 x_val = float4(x_vec[i]);
        float4 r_val = float4(res_vec[i]);
        float4 sum_val = x_val + r_val;

        local_sum += sum_val.x + sum_val.y + sum_val.z + sum_val.w;
        local_sum_sq += sum_val.x * sum_val.x + sum_val.y * sum_val.y +
                        sum_val.z * sum_val.z + sum_val.w * sum_val.w;
    }

    // SIMD reduction
    local_sum = simd_sum(local_sum);
    local_sum_sq = simd_sum(local_sum_sq);

    if (simd_lane == 0) {
        warp_sum[simd_group] = local_sum;
        warp_sum_sq[simd_group] = local_sum_sq;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

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

    // Normalize and output in FP16
    device half4* out_vec = reinterpret_cast<device half4*>(output + offset);
    const device half4* g_vec = reinterpret_cast<const device half4*>(gamma);
    const device half4* b_vec = params.has_bias ? reinterpret_cast<const device half4*>(beta) : nullptr;

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 sum_val = float4(x_vec[i]) + float4(res_vec[i]);
        float4 normalized = (sum_val - mean) * inv_std;
        float4 g = float4(g_vec[i]);
        float4 b = b_vec ? float4(b_vec[i]) : float4(0.0f);
        out_vec[i] = half4(fma(normalized, g, b));
    }
}

// ============================================================================
// FUSED RMSNORM + RESIDUAL
// Computes: output = RMSNorm(x + residual) * weight
// ============================================================================
kernel void fused_rmsnorm_residual(
    device const float* x [[buffer(0)]],
    device float* residual [[buffer(1)]],  // Also output of residual update
    device const float* weight [[buffer(2)]],
    device float* output [[buffer(3)]],
    constant FusedNormParams& params [[buffer(4)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint batch_idx = gid.y;
    const uint hidden_size = params.hidden_size;
    const float eps = params.eps;

    if (batch_idx >= params.batch_size) return;

    const uint offset = batch_idx * hidden_size;

    threadgroup float warp_sums[32] __attribute__((aligned(16)));

    // PASS 1: Add residual and compute sum of squares
    float local_sum_sq = 0.0f;

    const uint vec_size = hidden_size / 4;
    device float4* x_vec = reinterpret_cast<device float4*>(const_cast<float*>(x) + offset);
    device float4* res_vec = reinterpret_cast<device float4*>(residual + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 x_val = *reinterpret_cast<const device float4*>(&x[offset + i * 4]);
        float4 r_val = res_vec[i];
        float4 sum_val = x_val + r_val;

        // Update residual in-place
        res_vec[i] = sum_val;

        // Accumulate sum of squares
        local_sum_sq += sum_val.x * sum_val.x + sum_val.y * sum_val.y +
                        sum_val.z * sum_val.z + sum_val.w * sum_val.w;
    }

    // Handle remainder
    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float sum_val = x[offset + i] + residual[offset + i];
        residual[offset + i] = sum_val;
        local_sum_sq += sum_val * sum_val;
    }

    // SIMD reduction
    local_sum_sq = simd_sum(local_sum_sq);

    if (simd_lane == 0) {
        warp_sums[simd_group] = local_sum_sq;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Final reduction
    float total_sum_sq = 0.0f;
    if (simd_group == 0) {
        uint num_warps = (threads_per_group + SIMD_SIZE - 1) / SIMD_SIZE;
        if (simd_lane < num_warps) {
            total_sum_sq = warp_sums[simd_lane];
        }
        total_sum_sq = simd_sum(total_sum_sq);

        if (simd_lane == 0) {
            warp_sums[0] = total_sum_sq;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Compute inverse RMS
    float inv_rms = rsqrt(warp_sums[0] / float(hidden_size) + eps);

    // PASS 2: Normalize from residual and write to output
    device float4* out_vec = reinterpret_cast<device float4*>(output + offset);
    const device float4* w_vec = reinterpret_cast<const device float4*>(weight);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 val = res_vec[i];
        float4 w = w_vec[i];
        out_vec[i] = val * inv_rms * w;
    }

    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        output[offset + i] = residual[offset + i] * inv_rms * weight[i];
    }
}

// ============================================================================
// FUSED SWIGLU
// Computes: output = silu(gate) * up = gate * sigmoid(gate) * up
// Common in LLaMA-style MLP
// ============================================================================
struct SwiGLUParams {
    uint hidden_size;    // Size of gate/up vectors
    uint batch_size;     // Batch size
};

kernel void fused_swiglu(
    device const float* gate [[buffer(0)]],
    device const float* up [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant SwiGLUParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint batch_idx = gid.y;
    const uint hidden_size = params.hidden_size;

    if (batch_idx >= params.batch_size) return;

    const uint offset = batch_idx * hidden_size;

    // Vectorized computation
    const uint vec_size = hidden_size / 4;
    const device float4* gate_vec = reinterpret_cast<const device float4*>(gate + offset);
    const device float4* up_vec = reinterpret_cast<const device float4*>(up + offset);
    device float4* out_vec = reinterpret_cast<device float4*>(output + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        float4 g = gate_vec[i];
        float4 u = up_vec[i];

        // SiLU: x * sigmoid(x)
        float4 sigmoid_g = 1.0f / (1.0f + exp(-g));
        float4 silu_g = g * sigmoid_g;

        out_vec[i] = silu_g * u;
    }

    // Handle remainder
    for (uint i = vec_size * 4 + tid; i < hidden_size; i += threads_per_group) {
        float g = gate[offset + i];
        float u = up[offset + i];
        float sigmoid_g = 1.0f / (1.0f + exp(-g));
        output[offset + i] = g * sigmoid_g * u;
    }
}

// ============================================================================
// FUSED SWIGLU FP16
// ============================================================================
kernel void fused_swiglu_f16(
    device const half* gate [[buffer(0)]],
    device const half* up [[buffer(1)]],
    device half* output [[buffer(2)]],
    constant SwiGLUParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint batch_idx = gid.y;
    const uint hidden_size = params.hidden_size;

    if (batch_idx >= params.batch_size) return;

    const uint offset = batch_idx * hidden_size;

    const uint vec_size = hidden_size / 4;
    const device half4* gate_vec = reinterpret_cast<const device half4*>(gate + offset);
    const device half4* up_vec = reinterpret_cast<const device half4*>(up + offset);
    device half4* out_vec = reinterpret_cast<device half4*>(output + offset);

    for (uint i = tid; i < vec_size; i += threads_per_group) {
        // Compute in FP32 for accuracy
        float4 g = float4(gate_vec[i]);
        float4 u = float4(up_vec[i]);

        float4 sigmoid_g = 1.0f / (1.0f + exp(-g));
        float4 silu_g = g * sigmoid_g;

        out_vec[i] = half4(silu_g * u);
    }
}

// ============================================================================
// FUSED BIAS + GELU
// Computes: output = GELU(input + bias)
// GELU approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
// ============================================================================
struct BiasActivationParams {
    uint size;           // Total number of elements
    uint hidden_size;    // Hidden dimension for bias broadcast
};

kernel void fused_bias_gelu(
    device float* x [[buffer(0)]],  // In-place modification
    device const float* bias [[buffer(1)]],
    constant BiasActivationParams& params [[buffer(2)]],
    uint gid [[thread_position_in_grid]],
    uint threads_per_grid [[threads_per_grid]]
) {
    if (gid >= params.size) return;

    const uint bias_idx = gid % params.hidden_size;
    float val = x[gid] + bias[bias_idx];

    // GELU approximation
    const float sqrt_2_pi = 0.7978845608028654f;  // sqrt(2/pi)
    const float coeff = 0.044715f;
    float x3 = val * val * val;
    float inner = sqrt_2_pi * (val + coeff * x3);
    float gelu = 0.5f * val * (1.0f + tanh(inner));

    x[gid] = gelu;
}

// ============================================================================
// FUSED BIAS + RELU
// ============================================================================
kernel void fused_bias_relu(
    device float* x [[buffer(0)]],
    device const float* bias [[buffer(1)]],
    constant BiasActivationParams& params [[buffer(2)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= params.size) return;

    const uint bias_idx = gid % params.hidden_size;
    float val = x[gid] + bias[bias_idx];
    x[gid] = max(val, 0.0f);
}

// ============================================================================
// FUSED ADD + MULTIPLY (for residual paths)
// output = (a + b) * c
// ============================================================================
kernel void fused_add_mul(
    device const float* a [[buffer(0)]],
    device const float* b [[buffer(1)]],
    device const float* c [[buffer(2)]],
    device float* output [[buffer(3)]],
    constant uint& len [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    const uint vec_len = len / 4;
    if (gid < vec_len) {
        const device float4* a_vec = reinterpret_cast<const device float4*>(a);
        const device float4* b_vec = reinterpret_cast<const device float4*>(b);
        const device float4* c_vec = reinterpret_cast<const device float4*>(c);
        device float4* out_vec = reinterpret_cast<device float4*>(output);

        out_vec[gid] = (a_vec[gid] + b_vec[gid]) * c_vec[gid];
    } else {
        uint idx = vec_len * 4 + (gid - vec_len);
        if (idx < len) {
            output[idx] = (a[idx] + b[idx]) * c[idx];
        }
    }
}

// ============================================================================
// FUSED ROTARY + ATTENTION BIAS
// Apply RoPE to Q, K then add attention bias
// ============================================================================
struct RotaryBiasParams {
    uint head_dim;
    uint num_heads;
    uint seq_len;
    uint kv_len;
    float theta_base;
    uint use_alibi;
};

kernel void fused_rotary_bias(
    device float* Q [[buffer(0)]],
    device float* K [[buffer(1)]],
    device float* attn_bias [[buffer(2)]],  // [num_heads, seq_len, kv_len]
    device const float* cos_table [[buffer(3)]],
    device const float* sin_table [[buffer(4)]],
    constant RotaryBiasParams& params [[buffer(5)]],
    uint3 gid [[thread_position_in_grid]]
) {
    const uint d = gid.x;           // Dimension pair
    const uint head = gid.y;
    const uint seq_pos = gid.z;

    if (d >= params.head_dim / 2 || head >= params.num_heads) return;

    // Apply RoPE to Q
    const uint q_offset = (seq_pos * params.num_heads + head) * params.head_dim;
    float q0 = Q[q_offset + 2 * d];
    float q1 = Q[q_offset + 2 * d + 1];

    float cos_val = cos_table[seq_pos * (params.head_dim / 2) + d];
    float sin_val = sin_table[seq_pos * (params.head_dim / 2) + d];

    Q[q_offset + 2 * d] = fma(q0, cos_val, -q1 * sin_val);
    Q[q_offset + 2 * d + 1] = fma(q0, sin_val, q1 * cos_val);

    // Apply RoPE to K (same seq_pos)
    const uint k_offset = (seq_pos * params.num_heads + head) * params.head_dim;
    float k0 = K[k_offset + 2 * d];
    float k1 = K[k_offset + 2 * d + 1];

    K[k_offset + 2 * d] = fma(k0, cos_val, -k1 * sin_val);
    K[k_offset + 2 * d + 1] = fma(k0, sin_val, k1 * cos_val);

    // Apply ALiBi bias if enabled (only first dimension thread)
    if (params.use_alibi && d == 0) {
        float slope = exp2(-8.0f * float(head + 1) / float(params.num_heads));
        for (uint kv_pos = 0; kv_pos < params.kv_len; kv_pos++) {
            uint bias_idx = (head * params.seq_len + seq_pos) * params.kv_len + kv_pos;
            attn_bias[bias_idx] = slope * float(int(seq_pos) - int(kv_pos));
        }
    }
}
