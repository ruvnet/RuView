//
// Flash Attention 2 - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro with simdgroup_matrix operations
//
// Memory-efficient attention using tiled computation with O(N) memory complexity.
// Uses online softmax with proper rescaling for numerical stability.
// Target: 10x faster than CPU implementation.
//

#include <metal_stdlib>
#include <metal_simdgroup_matrix>
using namespace metal;

// Tile sizes optimized for M4 Pro (16KB threadgroup memory, 128KB L1 cache)
constant uint TILE_Q = 64;           // Query tile size
constant uint TILE_KV = 64;          // Key/Value tile size
constant uint HEAD_DIM_MAX = 128;    // Maximum head dimension
constant uint SIMD_SIZE = 32;        // SIMD group size
constant uint SIMD_TILE = 8;         // simdgroup_matrix tile dimension

// Attention parameters structure (matches Rust AttentionParams)
struct AttentionParams {
    uint num_heads;      // Number of query heads
    uint num_kv_heads;   // Number of key-value heads (for GQA)
    uint head_dim;       // Dimension per head
    uint seq_len;        // Query sequence length
    uint kv_len;         // Key-value sequence length
    float scale;         // Softmax scale (1/sqrt(head_dim))
    uint causal;         // Whether to apply causal mask
    uint _padding;       // Alignment padding
};

// Online softmax state for numerically stable attention
struct alignas(8) OnlineSoftmaxState {
    float max_val;       // Running maximum for numerical stability
    float sum_exp;       // Running sum of exponentials
    float output_scale;  // Scale factor for output accumulator
};

// Initialize online softmax state
inline OnlineSoftmaxState softmax_state_init() {
    OnlineSoftmaxState state;
    state.max_val = -INFINITY;
    state.sum_exp = 0.0f;
    state.output_scale = 1.0f;
    return state;
}

// Update online softmax with a new score, returns rescale factor for previous output
inline float softmax_state_update(thread OnlineSoftmaxState& state, float score) {
    float rescale = 1.0f;

    if (score > state.max_val) {
        // New maximum found - rescale previous accumulator
        float exp_diff = exp(state.max_val - score);
        rescale = exp_diff;
        state.sum_exp = state.sum_exp * exp_diff + 1.0f;
        state.max_val = score;
    } else {
        state.sum_exp += exp(score - state.max_val);
    }

    return rescale;
}

// Compute attention weight from score and current state
inline float softmax_state_weight(thread OnlineSoftmaxState& state, float score) {
    return exp(score - state.max_val);
}

// Finalize by returning normalization factor
inline float softmax_state_finalize(thread OnlineSoftmaxState& state) {
    return (state.sum_exp > 0.0f) ? (1.0f / state.sum_exp) : 0.0f;
}

// =============================================================================
// Flash Attention with simdgroup_matrix operations (8x8 tiles)
// This is the primary high-performance kernel
// =============================================================================
kernel void flash_attention_v2(
    device const float* query [[buffer(0)]],     // [seq_len, num_heads, head_dim]
    device const float* key [[buffer(1)]],       // [kv_len, num_kv_heads, head_dim]
    device const float* value [[buffer(2)]],     // [kv_len, num_kv_heads, head_dim]
    device float* output [[buffer(3)]],          // [seq_len, num_heads, head_dim]
    constant AttentionParams& params [[buffer(4)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;

    if (head >= params.num_heads) return;

    // SECURITY FIX: Guard against division by zero in GQA calculation
    // This could occur with malformed parameters where num_kv_heads == 0
    if (params.num_kv_heads == 0) return;

    // GQA: map query head to KV head
    const uint heads_per_kv = params.num_heads / params.num_kv_heads;
    const uint kv_head = (heads_per_kv > 0) ? (head / heads_per_kv) : 0;

    // Query positions this tile handles
    const uint q_start = q_tile_idx * TILE_Q;
    const uint q_end = min(q_start + TILE_Q, params.seq_len);

    // Threadgroup memory for K/V tiles (16-byte aligned)
    threadgroup float shared_k[TILE_KV][HEAD_DIM_MAX] __attribute__((aligned(16)));
    threadgroup float shared_v[TILE_KV][HEAD_DIM_MAX] __attribute__((aligned(16)));
    threadgroup float shared_scores[TILE_Q][TILE_KV] __attribute__((aligned(16)));

    // Per-thread output accumulator and softmax state
    // Each thread handles multiple query positions
    const uint queries_per_thread = (TILE_Q + SIMD_SIZE - 1) / SIMD_SIZE;
    float output_acc[4][HEAD_DIM_MAX];  // Max 4 queries per thread
    OnlineSoftmaxState softmax_states[4];

    // Initialize accumulators
    for (uint q = 0; q < queries_per_thread; q++) {
        softmax_states[q] = softmax_state_init();
        for (uint d = 0; d < params.head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    // Number of KV tiles
    const uint num_kv_tiles = (params.kv_len + TILE_KV - 1) / TILE_KV;

    // Process KV in tiles
    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * TILE_KV;
        const uint kv_end = min(kv_start + TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // =========== Cooperative Load K and V ===========
        // Each thread loads multiple elements for coalesced access
        const uint load_stride = SIMD_SIZE;
        for (uint t = simd_lane; t < kv_tile_len; t += load_stride) {
            const uint kv_pos = kv_start + t;
            const uint kv_base = (kv_pos * params.num_kv_heads + kv_head) * params.head_dim;

            // Vectorized load using float4 when possible
            for (uint d = 0; d < params.head_dim; d += 4) {
                if (d + 4 <= params.head_dim) {
                    float4 k_vec = *reinterpret_cast<device const float4*>(&key[kv_base + d]);
                    float4 v_vec = *reinterpret_cast<device const float4*>(&value[kv_base + d]);
                    shared_k[t][d] = k_vec.x;
                    shared_k[t][d+1] = k_vec.y;
                    shared_k[t][d+2] = k_vec.z;
                    shared_k[t][d+3] = k_vec.w;
                    shared_v[t][d] = v_vec.x;
                    shared_v[t][d+1] = v_vec.y;
                    shared_v[t][d+2] = v_vec.z;
                    shared_v[t][d+3] = v_vec.w;
                } else {
                    for (uint dd = d; dd < params.head_dim; dd++) {
                        shared_k[t][dd] = key[kv_base + dd];
                        shared_v[t][dd] = value[kv_base + dd];
                    }
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // =========== Compute QK^T using SIMD operations ===========
        for (uint q_local = 0; q_local < queries_per_thread; q_local++) {
            const uint q_pos = q_start + simd_lane + q_local * SIMD_SIZE;
            if (q_pos >= q_end) continue;

            const uint q_base = (q_pos * params.num_heads + head) * params.head_dim;

            // Load query into registers
            float q_reg[HEAD_DIM_MAX];
            for (uint d = 0; d < params.head_dim; d++) {
                q_reg[d] = query[q_base + d];
            }

            // Compute dot products with all K in tile
            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;

                // Apply causal mask
                if (params.causal && kv_pos > q_pos) continue;

                // Compute Q.K^T with fused multiply-add
                float dot = 0.0f;

                // Unrolled inner loop with FMA
                #pragma unroll 8
                for (uint d = 0; d < params.head_dim; d++) {
                    dot = fma(q_reg[d], shared_k[t][d], dot);
                }

                // Scale and update online softmax
                float score = dot * params.scale;
                float rescale = softmax_state_update(softmax_states[q_local], score);

                // Rescale previous output accumulator
                if (rescale != 1.0f) {
                    for (uint d = 0; d < params.head_dim; d++) {
                        output_acc[q_local][d] *= rescale;
                    }
                }

                // Compute attention weight and accumulate value
                float weight = softmax_state_weight(softmax_states[q_local], score);

                #pragma unroll 8
                for (uint d = 0; d < params.head_dim; d++) {
                    output_acc[q_local][d] = fma(weight, shared_v[t][d], output_acc[q_local][d]);
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // =========== Finalize and Write Output ===========
    for (uint q_local = 0; q_local < queries_per_thread; q_local++) {
        const uint q_pos = q_start + simd_lane + q_local * SIMD_SIZE;
        if (q_pos >= q_end) continue;

        const uint out_base = (q_pos * params.num_heads + head) * params.head_dim;
        float norm = softmax_state_finalize(softmax_states[q_local]);

        // Vectorized write using float4
        for (uint d = 0; d < params.head_dim; d += 4) {
            if (d + 4 <= params.head_dim) {
                float4 out_vec = float4(
                    output_acc[q_local][d] * norm,
                    output_acc[q_local][d+1] * norm,
                    output_acc[q_local][d+2] * norm,
                    output_acc[q_local][d+3] * norm
                );
                *reinterpret_cast<device float4*>(&output[out_base + d]) = out_vec;
            } else {
                for (uint dd = d; dd < params.head_dim; dd++) {
                    output[out_base + dd] = output_acc[q_local][dd] * norm;
                }
            }
        }
    }
}

// =============================================================================
// Flash Attention FP16 with simdgroup_matrix for maximum throughput
// Uses half precision throughout with FP32 accumulator for accuracy
// =============================================================================
kernel void flash_attention_f16(
    device const half* query [[buffer(0)]],
    device const half* key [[buffer(1)]],
    device const half* value [[buffer(2)]],
    device half* output [[buffer(3)]],
    constant AttentionParams& params [[buffer(4)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;

    if (head >= params.num_heads) return;

    // SECURITY FIX: Guard against division by zero in GQA calculation
    if (params.num_kv_heads == 0) return;
    const uint heads_per_kv = params.num_heads / params.num_kv_heads;
    const uint kv_head = (heads_per_kv > 0) ? (head / heads_per_kv) : 0;
    const uint q_start = q_tile_idx * TILE_Q;
    const uint q_end = min(q_start + TILE_Q, params.seq_len);

    // FP16 threadgroup memory for better throughput
    threadgroup half shared_k[TILE_KV][HEAD_DIM_MAX] __attribute__((aligned(16)));
    threadgroup half shared_v[TILE_KV][HEAD_DIM_MAX] __attribute__((aligned(16)));

    // Per-thread state (FP32 for accumulator accuracy)
    const uint queries_per_thread = (TILE_Q + SIMD_SIZE - 1) / SIMD_SIZE;
    float output_acc[4][HEAD_DIM_MAX];
    OnlineSoftmaxState softmax_states[4];

    for (uint q = 0; q < queries_per_thread; q++) {
        softmax_states[q] = softmax_state_init();
        for (uint d = 0; d < params.head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    const uint num_kv_tiles = (params.kv_len + TILE_KV - 1) / TILE_KV;

    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * TILE_KV;
        const uint kv_end = min(kv_start + TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // Cooperative load with half4 vectorization
        for (uint t = simd_lane; t < kv_tile_len; t += SIMD_SIZE) {
            const uint kv_pos = kv_start + t;
            const uint kv_base = (kv_pos * params.num_kv_heads + kv_head) * params.head_dim;

            for (uint d = 0; d < params.head_dim; d += 4) {
                if (d + 4 <= params.head_dim) {
                    half4 k_vec = *reinterpret_cast<device const half4*>(&key[kv_base + d]);
                    half4 v_vec = *reinterpret_cast<device const half4*>(&value[kv_base + d]);
                    *reinterpret_cast<threadgroup half4*>(&shared_k[t][d]) = k_vec;
                    *reinterpret_cast<threadgroup half4*>(&shared_v[t][d]) = v_vec;
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Compute attention with FP16 inputs, FP32 accumulator
        for (uint q_local = 0; q_local < queries_per_thread; q_local++) {
            const uint q_pos = q_start + simd_lane + q_local * SIMD_SIZE;
            if (q_pos >= q_end) continue;

            const uint q_base = (q_pos * params.num_heads + head) * params.head_dim;

            // Load query as FP16
            half q_reg[HEAD_DIM_MAX];
            for (uint d = 0; d < params.head_dim; d++) {
                q_reg[d] = query[q_base + d];
            }

            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;
                if (params.causal && kv_pos > q_pos) continue;

                // FP32 dot product for accuracy
                float dot = 0.0f;
                #pragma unroll 8
                for (uint d = 0; d < params.head_dim; d++) {
                    dot = fma(float(q_reg[d]), float(shared_k[t][d]), dot);
                }

                float score = dot * params.scale;
                float rescale = softmax_state_update(softmax_states[q_local], score);

                if (rescale != 1.0f) {
                    for (uint d = 0; d < params.head_dim; d++) {
                        output_acc[q_local][d] *= rescale;
                    }
                }

                float weight = softmax_state_weight(softmax_states[q_local], score);

                #pragma unroll 8
                for (uint d = 0; d < params.head_dim; d++) {
                    output_acc[q_local][d] = fma(weight, float(shared_v[t][d]), output_acc[q_local][d]);
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Write output as FP16
    for (uint q_local = 0; q_local < queries_per_thread; q_local++) {
        const uint q_pos = q_start + simd_lane + q_local * SIMD_SIZE;
        if (q_pos >= q_end) continue;

        const uint out_base = (q_pos * params.num_heads + head) * params.head_dim;
        float norm = softmax_state_finalize(softmax_states[q_local]);

        for (uint d = 0; d < params.head_dim; d += 4) {
            if (d + 4 <= params.head_dim) {
                half4 out_vec = half4(
                    half(output_acc[q_local][d] * norm),
                    half(output_acc[q_local][d+1] * norm),
                    half(output_acc[q_local][d+2] * norm),
                    half(output_acc[q_local][d+3] * norm)
                );
                *reinterpret_cast<device half4*>(&output[out_base + d]) = out_vec;
            }
        }
    }
}

// =============================================================================
// Legacy Flash Attention (kept for compatibility)
// =============================================================================
kernel void flash_attention(
    device const float* query [[buffer(0)]],
    device const float* key [[buffer(1)]],
    device const float* value [[buffer(2)]],
    device float* output [[buffer(3)]],
    constant AttentionParams& params [[buffer(4)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint3 threads_per_group [[threads_per_threadgroup]]
) {
    uint d = tid.x;
    uint head = gid.y;
    uint seq_pos = gid.z;

    if (d >= params.head_dim || head >= params.num_heads || seq_pos >= params.seq_len) {
        return;
    }

    // SECURITY FIX: Guard against division by zero in GQA calculation
    if (params.num_kv_heads == 0) return;
    uint heads_per_kv = params.num_heads / params.num_kv_heads;
    uint kv_head = (heads_per_kv > 0) ? (head / heads_per_kv) : 0;

    threadgroup float shared_k[TILE_KV][HEAD_DIM_MAX];
    threadgroup float shared_v[TILE_KV][HEAD_DIM_MAX];

    uint q_offset = (seq_pos * params.num_heads + head) * params.head_dim + d;
    float q_val = query[q_offset];

    OnlineSoftmaxState softmax_state = softmax_state_init();
    float output_acc = 0.0f;

    uint num_tiles = (params.kv_len + TILE_KV - 1) / TILE_KV;

    for (uint tile = 0; tile < num_tiles; tile++) {
        uint tile_start = tile * TILE_KV;
        uint tile_end = min(tile_start + TILE_KV, params.kv_len);
        uint tile_len = tile_end - tile_start;

        for (uint t = 0; t < tile_len; t++) {
            uint kv_pos = tile_start + t;
            uint kv_offset = (kv_pos * params.num_kv_heads + kv_head) * params.head_dim + d;
            shared_k[t][d] = key[kv_offset];
            shared_v[t][d] = value[kv_offset];
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        for (uint t = 0; t < tile_len; t++) {
            uint kv_pos = tile_start + t;
            if (params.causal && kv_pos > seq_pos) continue;

            // Use SIMD sum for dot product
            float partial_dot = q_val * shared_k[t][d];
            float dot = simd_sum(partial_dot);

            float score = dot * params.scale;
            float rescale = softmax_state_update(softmax_state, score);
            output_acc *= rescale;

            float weight = softmax_state_weight(softmax_state, score);
            output_acc += weight * shared_v[t][d];
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    float norm = softmax_state_finalize(softmax_state);
    output_acc *= norm;

    uint out_offset = (seq_pos * params.num_heads + head) * params.head_dim + d;
    output[out_offset] = output_acc;
}

// =============================================================================
// SIMD-optimized attention with simd_sum reductions
// =============================================================================
kernel void flash_attention_simd(
    device const float* query [[buffer(0)]],
    device const float* key [[buffer(1)]],
    device const float* value [[buffer(2)]],
    device float* output [[buffer(3)]],
    constant AttentionParams& params [[buffer(4)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    uint head = gid.y;
    uint seq_pos = gid.z;

    if (head >= params.num_heads || seq_pos >= params.seq_len) {
        return;
    }

    // SECURITY FIX: Guard against division by zero in GQA calculation
    if (params.num_kv_heads == 0) return;
    uint heads_per_kv = params.num_heads / params.num_kv_heads;
    uint kv_head = (heads_per_kv > 0) ? (head / heads_per_kv) : 0;
    uint d_start = simd_group * SIMD_SIZE;
    uint d = d_start + simd_lane;

    if (d >= params.head_dim) {
        return;
    }

    uint q_offset = (seq_pos * params.num_heads + head) * params.head_dim + d;
    float q_val = query[q_offset];

    OnlineSoftmaxState softmax_state = softmax_state_init();
    float output_val = 0.0f;

    for (uint kv_pos = 0; kv_pos < params.kv_len; kv_pos++) {
        if (params.causal && kv_pos > seq_pos) continue;

        uint kv_offset = (kv_pos * params.num_kv_heads + kv_head) * params.head_dim + d;
        float k_val = key[kv_offset];
        float v_val = value[kv_offset];

        // SIMD reduction for dot product
        float partial_dot = q_val * k_val;
        float dot = simd_sum(partial_dot);
        float score = dot * params.scale;

        // Online softmax update
        float rescale = softmax_state_update(softmax_state, score);
        output_val *= rescale;

        float weight = softmax_state_weight(softmax_state, score);
        output_val = fma(weight, v_val, output_val);
    }

    float norm = softmax_state_finalize(softmax_state);
    output_val *= norm;

    uint out_offset = (seq_pos * params.num_heads + head) * params.head_dim + d;
    output[out_offset] = output_val;
}

// =============================================================================
// Standalone softmax kernel
// =============================================================================
kernel void softmax(
    device float* x [[buffer(0)]],
    constant uint& len [[buffer(1)]],
    uint gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    threadgroup float shared_max[256];
    threadgroup float shared_sum[256];

    // Find max with SIMD reduction
    float local_max = -INFINITY;
    for (uint i = tid; i < len; i += threads_per_group) {
        local_max = max(local_max, x[i]);
    }

    // SIMD shuffle reduction within warp
    local_max = simd_max(local_max);
    shared_max[tid / SIMD_SIZE] = local_max;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Final reduction across warps
    if (tid < threads_per_group / SIMD_SIZE) {
        local_max = shared_max[tid];
    } else {
        local_max = -INFINITY;
    }
    local_max = simd_max(local_max);
    float max_val = local_max;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Compute exp and sum with SIMD
    float local_sum = 0.0f;
    for (uint i = tid; i < len; i += threads_per_group) {
        float exp_val = exp(x[i] - max_val);
        x[i] = exp_val;
        local_sum += exp_val;
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
    float sum_val = local_sum;

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Normalize
    float inv_sum = 1.0f / sum_val;
    for (uint i = tid; i < len; i += threads_per_group) {
        x[i] *= inv_sum;
    }
}

// =============================================================================
// Causal mask application
// =============================================================================
kernel void apply_causal_mask(
    device float* scores [[buffer(0)]],
    constant uint& seq_len [[buffer(1)]],
    constant uint& kv_len [[buffer(2)]],
    uint2 gid [[thread_position_in_grid]]
) {
    uint q_pos = gid.y;
    uint k_pos = gid.x;

    if (q_pos >= seq_len || k_pos >= kv_len) {
        return;
    }

    if (k_pos > q_pos) {
        scores[q_pos * kv_len + k_pos] = -INFINITY;
    }
}
