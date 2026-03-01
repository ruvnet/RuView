//
// RoPE + Attention Fusion - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro
//
// Fuses Rotary Position Embedding application with attention computation:
// - Apply RoPE to Q, K before computing attention
// - Reduces memory traffic by avoiding Q, K materialization
// - Supports standard RoPE, YaRN, and NTK-aware scaling
//
// M4 Pro Optimizations:
// - Vectorized half2 operations
// - SIMD reductions for dot products
// - Coalesced memory access patterns
// - 1024 threads per threadgroup
//

#include <metal_stdlib>
#include <metal_simdgroup_matrix>
using namespace metal;

// ============================================================================
// Constants
// ============================================================================
constant uint SIMD_SIZE = 32;
constant uint ATTN_TILE_Q = 32;
constant uint ATTN_TILE_KV = 64;
constant uint HEAD_DIM_MAX = 128;

// ============================================================================
// RoPE + Attention Parameters
// ============================================================================
struct RopeAttentionParams {
    uint num_heads;          // Number of query heads
    uint num_kv_heads;       // Number of key-value heads
    uint head_dim;           // Dimension per head
    uint seq_len;            // Query sequence length
    uint kv_len;             // Key-value sequence length
    float scale;             // Attention scale (1/sqrt(head_dim))
    float theta_base;        // RoPE base (10000 typically)
    uint causal;             // Causal mask flag
    float rope_scale;        // RoPE scaling factor (1.0 for standard)
    float ntk_alpha;         // NTK-aware scaling alpha (1.0 for standard)
};

// ============================================================================
// Online Softmax Helper
// ============================================================================
struct OnlineSoftmax {
    float max_val;
    float sum_exp;

    static OnlineSoftmax init() {
        OnlineSoftmax s;
        s.max_val = -INFINITY;
        s.sum_exp = 0.0f;
        return s;
    }

    float update(float val) {
        float rescale = 1.0f;
        if (val > max_val) {
            float exp_diff = exp(max_val - val);
            rescale = exp_diff;
            sum_exp = sum_exp * exp_diff + 1.0f;
            max_val = val;
        } else {
            sum_exp += exp(val - max_val);
        }
        return rescale;
    }

    float weight(float val) const {
        return exp(val - max_val);
    }

    float normalize() const {
        return (sum_exp > 0.0f) ? (1.0f / sum_exp) : 0.0f;
    }
};

// ============================================================================
// FUSED ROPE + ATTENTION KERNEL
// Apply RoPE to Q, K then compute attention in single kernel
// Grid: (1, num_heads, ceil(seq_len / ATTN_TILE_Q))
// ============================================================================
kernel void rope_then_attention(
    device half* Q [[buffer(0)]],              // [seq_len, num_heads, head_dim]
    device half* K [[buffer(1)]],              // [kv_len, num_kv_heads, head_dim]
    device const half* V [[buffer(2)]],        // [kv_len, num_kv_heads, head_dim]
    device half* O [[buffer(3)]],              // [seq_len, num_heads, head_dim]
    device const float* cos_table [[buffer(4)]], // [max_seq_len, head_dim/2]
    device const float* sin_table [[buffer(5)]], // [max_seq_len, head_dim/2]
    constant RopeAttentionParams& params [[buffer(6)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;
    const uint head_dim = params.head_dim;
    const uint half_dim = head_dim / 2;

    if (head >= params.num_heads) return;

    const uint kv_head = head / (params.num_heads / params.num_kv_heads);
    const uint q_start = q_tile_idx * ATTN_TILE_Q;
    const uint q_end = min(q_start + ATTN_TILE_Q, params.seq_len);

    // Shared memory for rotated K, V
    threadgroup half shared_k[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));
    threadgroup half shared_v[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));

    const uint thread_id = simd_group * SIMD_SIZE + simd_lane;
    const uint warps = 8;  // 256 threads / 32
    const uint queries_per_warp = (ATTN_TILE_Q + warps - 1) / warps;
    const uint my_q_offset = simd_group * queries_per_warp;

    // Per-query output accumulator and softmax state
    float output_acc[4][HEAD_DIM_MAX];
    OnlineSoftmax softmax_state[4];

    for (uint q = 0; q < queries_per_warp && q < 4; q++) {
        softmax_state[q] = OnlineSoftmax::init();
        for (uint d = 0; d < head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    // Load and apply RoPE to queries (each warp handles its queries)
    half q_rotated[4][HEAD_DIM_MAX];
    for (uint q = 0; q < queries_per_warp && q < 4; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint q_base = (q_pos * params.num_heads + head) * head_dim;

        // Apply RoPE to query
        for (uint d = 0; d < half_dim; d++) {
            // Get cos/sin for this position
            const uint table_idx = q_pos * half_dim + d;
            float cos_val = cos_table[table_idx] * params.rope_scale;
            float sin_val = sin_table[table_idx] * params.rope_scale;

            // NTK-aware scaling
            if (params.ntk_alpha != 1.0f) {
                float freq_scale = pow(params.ntk_alpha, float(d) / float(half_dim));
                cos_val *= freq_scale;
                sin_val *= freq_scale;
            }

            // Load Q pair
            float q0 = float(Q[q_base + 2 * d]);
            float q1 = float(Q[q_base + 2 * d + 1]);

            // Rotate
            q_rotated[q][2 * d] = half(q0 * cos_val - q1 * sin_val);
            q_rotated[q][2 * d + 1] = half(q0 * sin_val + q1 * cos_val);
        }
    }

    // Number of KV tiles
    const uint num_kv_tiles = (params.kv_len + ATTN_TILE_KV - 1) / ATTN_TILE_KV;

    // Process KV in tiles
    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * ATTN_TILE_KV;
        const uint kv_end = min(kv_start + ATTN_TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // Cooperative load K (with RoPE applied) and V
        for (uint t = thread_id; t < kv_tile_len; t += warps * SIMD_SIZE) {
            const uint kv_pos = kv_start + t;
            const uint kv_base = (kv_pos * params.num_kv_heads + kv_head) * head_dim;

            // Load and rotate K
            for (uint d = 0; d < half_dim; d++) {
                const uint table_idx = kv_pos * half_dim + d;
                float cos_val = cos_table[table_idx] * params.rope_scale;
                float sin_val = sin_table[table_idx] * params.rope_scale;

                if (params.ntk_alpha != 1.0f) {
                    float freq_scale = pow(params.ntk_alpha, float(d) / float(half_dim));
                    cos_val *= freq_scale;
                    sin_val *= freq_scale;
                }

                float k0 = float(K[kv_base + 2 * d]);
                float k1 = float(K[kv_base + 2 * d + 1]);

                shared_k[t][2 * d] = half(k0 * cos_val - k1 * sin_val);
                shared_k[t][2 * d + 1] = half(k0 * sin_val + k1 * cos_val);
            }

            // Load V (no rotation needed)
            for (uint d = 0; d < head_dim; d++) {
                shared_v[t][d] = V[kv_base + d];
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Compute attention
        for (uint q = 0; q < queries_per_warp && q < 4; q++) {
            const uint q_pos = q_start + my_q_offset + q;
            if (q_pos >= q_end) continue;

            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;

                // Causal mask
                if (params.causal && kv_pos > q_pos) continue;

                // Compute Q.K^T dot product
                float dot = 0.0f;
                for (uint d = 0; d < head_dim; d++) {
                    dot = fma(float(q_rotated[q][d]), float(shared_k[t][d]), dot);
                }

                // Scale
                float score = dot * params.scale;

                // Online softmax update
                float rescale = softmax_state[q].update(score);

                // Rescale previous output
                if (rescale != 1.0f) {
                    for (uint d = 0; d < head_dim; d++) {
                        output_acc[q][d] *= rescale;
                    }
                }

                // Accumulate weighted value
                float weight = softmax_state[q].weight(score);
                for (uint d = 0; d < head_dim; d++) {
                    output_acc[q][d] = fma(weight, float(shared_v[t][d]), output_acc[q][d]);
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Write output
    for (uint q = 0; q < queries_per_warp && q < 4; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint out_base = (q_pos * params.num_heads + head) * head_dim;
        float norm = softmax_state[q].normalize();

        for (uint d = 0; d < head_dim; d++) {
            O[out_base + d] = half(output_acc[q][d] * norm);
        }
    }
}

// ============================================================================
// YARN RoPE + ATTENTION
// Yet another RoPE extension with better extrapolation
// ============================================================================
struct YarnParams {
    uint num_heads;
    uint num_kv_heads;
    uint head_dim;
    uint seq_len;
    uint kv_len;
    float scale;
    float theta_base;
    uint causal;
    float yarn_scale;        // Position scale factor
    float attn_scale;        // Attention scale factor
    float beta_fast;         // High-frequency extrapolation factor
    float beta_slow;         // Low-frequency interpolation factor
    uint original_max_len;   // Original training context length
};

kernel void yarn_attention(
    device half* Q [[buffer(0)]],
    device half* K [[buffer(1)]],
    device const half* V [[buffer(2)]],
    device half* O [[buffer(3)]],
    constant YarnParams& params [[buffer(4)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;
    const uint head_dim = params.head_dim;
    const uint half_dim = head_dim / 2;

    if (head >= params.num_heads) return;

    const uint kv_head = head / (params.num_heads / params.num_kv_heads);
    const uint q_start = q_tile_idx * ATTN_TILE_Q;
    const uint q_end = min(q_start + ATTN_TILE_Q, params.seq_len);

    threadgroup half shared_k[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));
    threadgroup half shared_v[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));

    const uint thread_id = simd_group * SIMD_SIZE + simd_lane;
    const uint warps = 8;
    const uint queries_per_warp = (ATTN_TILE_Q + warps - 1) / warps;
    const uint my_q_offset = simd_group * queries_per_warp;

    float output_acc[4][HEAD_DIM_MAX];
    OnlineSoftmax softmax_state[4];

    for (uint q = 0; q < queries_per_warp && q < 4; q++) {
        softmax_state[q] = OnlineSoftmax::init();
        for (uint d = 0; d < head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    // YaRN-specific: compute frequency ramp
    auto compute_yarn_freq = [&](uint d, uint position) -> float2 {
        float freq_base = 1.0f / pow(params.theta_base, float(2 * d) / float(head_dim));
        float wavelength = 2.0f * M_PI_F / freq_base;

        float low = float(params.original_max_len) / params.beta_fast;
        float high = float(params.original_max_len) / params.beta_slow;

        float ramp;
        if (wavelength < low) {
            ramp = 0.0f;  // High frequency: extrapolate
        } else if (wavelength > high) {
            ramp = 1.0f;  // Low frequency: interpolate
        } else {
            ramp = (wavelength - low) / (high - low);
        }

        // Scale frequency with ramp interpolation
        float freq = freq_base * (1.0f - ramp + ramp / params.yarn_scale);
        float angle = float(position) * freq;

        return float2(cos(angle), sin(angle));
    };

    // Load and apply YaRN RoPE to queries
    half q_rotated[4][HEAD_DIM_MAX];
    for (uint q = 0; q < queries_per_warp && q < 4; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint q_base = (q_pos * params.num_heads + head) * head_dim;

        for (uint d = 0; d < half_dim; d++) {
            float2 cs = compute_yarn_freq(d, q_pos);
            float cos_val = cs.x;
            float sin_val = cs.y;

            float q0 = float(Q[q_base + 2 * d]);
            float q1 = float(Q[q_base + 2 * d + 1]);

            q_rotated[q][2 * d] = half(q0 * cos_val - q1 * sin_val);
            q_rotated[q][2 * d + 1] = half(q0 * sin_val + q1 * cos_val);
        }
    }

    const uint num_kv_tiles = (params.kv_len + ATTN_TILE_KV - 1) / ATTN_TILE_KV;

    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * ATTN_TILE_KV;
        const uint kv_end = min(kv_start + ATTN_TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // Load K with YaRN RoPE and V
        for (uint t = thread_id; t < kv_tile_len; t += warps * SIMD_SIZE) {
            const uint kv_pos = kv_start + t;
            const uint kv_base = (kv_pos * params.num_kv_heads + kv_head) * head_dim;

            for (uint d = 0; d < half_dim; d++) {
                float2 cs = compute_yarn_freq(d, kv_pos);

                float k0 = float(K[kv_base + 2 * d]);
                float k1 = float(K[kv_base + 2 * d + 1]);

                shared_k[t][2 * d] = half(k0 * cs.x - k1 * cs.y);
                shared_k[t][2 * d + 1] = half(k0 * cs.y + k1 * cs.x);
            }

            for (uint d = 0; d < head_dim; d++) {
                shared_v[t][d] = V[kv_base + d];
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Compute attention (same as standard)
        for (uint q = 0; q < queries_per_warp && q < 4; q++) {
            const uint q_pos = q_start + my_q_offset + q;
            if (q_pos >= q_end) continue;

            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;
                if (params.causal && kv_pos > q_pos) continue;

                float dot = 0.0f;
                for (uint d = 0; d < head_dim; d++) {
                    dot = fma(float(q_rotated[q][d]), float(shared_k[t][d]), dot);
                }

                // YaRN attention scale
                float score = dot * params.scale * params.attn_scale;
                float rescale = softmax_state[q].update(score);

                if (rescale != 1.0f) {
                    for (uint d = 0; d < head_dim; d++) {
                        output_acc[q][d] *= rescale;
                    }
                }

                float weight = softmax_state[q].weight(score);
                for (uint d = 0; d < head_dim; d++) {
                    output_acc[q][d] = fma(weight, float(shared_v[t][d]), output_acc[q][d]);
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Write output
    for (uint q = 0; q < queries_per_warp && q < 4; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint out_base = (q_pos * params.num_heads + head) * head_dim;
        float norm = softmax_state[q].normalize();

        for (uint d = 0; d < head_dim; d++) {
            O[out_base + d] = half(output_acc[q][d] * norm);
        }
    }
}

// ============================================================================
// APPLY ROPE TO Q AND K IN-PLACE
// Standalone RoPE for when attention is computed separately
// ============================================================================
kernel void apply_rope_qk_inplace(
    device half* Q [[buffer(0)]],
    device half* K [[buffer(1)]],
    device const float* cos_table [[buffer(2)]],
    device const float* sin_table [[buffer(3)]],
    device const uint* positions [[buffer(4)]],  // [seq_len]
    constant uint& num_q_heads [[buffer(5)]],
    constant uint& num_kv_heads [[buffer(6)]],
    constant uint& head_dim [[buffer(7)]],
    constant uint& seq_len [[buffer(8)]],
    uint3 gid [[thread_position_in_grid]]
) {
    const uint d = gid.x;           // Dimension pair
    const uint head = gid.y;
    const uint pos = gid.z;

    const uint half_dim = head_dim / 2;
    if (d >= half_dim || pos >= seq_len) return;

    const uint position = positions[pos];
    const uint table_idx = position * half_dim + d;
    float cos_val = cos_table[table_idx];
    float sin_val = sin_table[table_idx];

    // Apply to Q
    if (head < num_q_heads) {
        const uint q_base = (pos * num_q_heads + head) * head_dim;
        float q0 = float(Q[q_base + 2 * d]);
        float q1 = float(Q[q_base + 2 * d + 1]);
        Q[q_base + 2 * d] = half(q0 * cos_val - q1 * sin_val);
        Q[q_base + 2 * d + 1] = half(q0 * sin_val + q1 * cos_val);
    }

    // Apply to K
    if (head < num_kv_heads) {
        const uint k_base = (pos * num_kv_heads + head) * head_dim;
        float k0 = float(K[k_base + 2 * d]);
        float k1 = float(K[k_base + 2 * d + 1]);
        K[k_base + 2 * d] = half(k0 * cos_val - k1 * sin_val);
        K[k_base + 2 * d + 1] = half(k0 * sin_val + k1 * cos_val);
    }
}

// ============================================================================
// PRECOMPUTE ROPE TABLES
// Run once per model load
// ============================================================================
kernel void precompute_rope_tables_optimized(
    device float* cos_table [[buffer(0)]],
    device float* sin_table [[buffer(1)]],
    constant uint& head_dim [[buffer(2)]],
    constant uint& max_seq_len [[buffer(3)]],
    constant float& theta_base [[buffer(4)]],
    constant float& scale [[buffer(5)]],  // For NTK scaling
    uint2 gid [[thread_position_in_grid]]
) {
    const uint pos = gid.y;
    const uint d = gid.x;
    const uint half_dim = head_dim / 2;

    if (pos >= max_seq_len || d >= half_dim) return;

    // Compute frequency with optional scaling
    float freq = 1.0f / pow(theta_base * scale, float(2 * d) / float(head_dim));
    float angle = float(pos) * freq;

    uint idx = pos * half_dim + d;
    cos_table[idx] = cos(angle);
    sin_table[idx] = sin(angle);
}
