//
// Fused Attention - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro
//
// Implements Flash Attention 2 algorithm with:
// - Fused Q*K^T -> softmax -> *V in single kernel
// - Online softmax (no intermediate attention matrix storage)
// - O(N) memory complexity instead of O(N^2)
// - Shared memory for K, V tiles
// - Causal masking support
// - GQA (Grouped Query Attention) support
//
// M4 Pro Optimizations:
// - 1024 threads per threadgroup
// - Optimized for 16KB L1, 192KB L2 per core
// - simdgroup operations for fast reductions
// - Vectorized half4 memory access
// - Bank conflict-free shared memory layout
//

#include <metal_stdlib>
#include <metal_simdgroup_matrix>
using namespace metal;

// ============================================================================
// M4 Pro Tuned Constants for Fused Attention
// ============================================================================
constant uint ATTN_TILE_Q = 64;      // Query tile size
constant uint ATTN_TILE_KV = 64;     // KV tile size
constant uint HEAD_DIM_MAX = 128;    // Max head dimension
constant uint SIMD_SIZE = 32;        // SIMD group size
constant uint WARPS_PER_BLOCK = 8;   // 256 threads per tile

// ============================================================================
// Attention Parameters Structure
// ============================================================================
struct FusedAttentionParams {
    uint num_heads;      // Number of query heads
    uint num_kv_heads;   // Number of key-value heads (for GQA)
    uint head_dim;       // Dimension per head
    uint seq_len;        // Query sequence length
    uint kv_len;         // Key-value sequence length
    float scale;         // Softmax scale (1/sqrt(head_dim))
    uint causal;         // Whether to apply causal mask
    uint use_alibi;      // Whether to use ALiBi positional encoding
};

// ============================================================================
// Online Softmax State for Numerically Stable Attention
// Maintains running max and sum for incremental softmax computation
// ============================================================================
struct OnlineSoftmax {
    float max_val;       // Running maximum for numerical stability
    float sum_exp;       // Running sum of exponentials

    // Initialize with -inf max and zero sum
    static OnlineSoftmax init() {
        OnlineSoftmax state;
        state.max_val = -INFINITY;
        state.sum_exp = 0.0f;
        return state;
    }

    // Update state with new value, return rescale factor for previous output
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

    // Get weight for current value
    float weight(float val) const {
        return exp(val - max_val);
    }

    // Get final normalization factor
    float normalize() const {
        return (sum_exp > 0.0f) ? (1.0f / sum_exp) : 0.0f;
    }
};

// ============================================================================
// FUSED ATTENTION KERNEL - Flash Attention 2 Style
// Fuses Q*K^T -> softmax -> *V into single kernel
// Grid: (1, num_heads, seq_len / ATTN_TILE_Q)
// Threadgroup: 256 threads (8 warps)
// ============================================================================
kernel void fused_attention(
    device const half* Q [[buffer(0)]],        // [seq_len, num_heads, head_dim]
    device const half* K [[buffer(1)]],        // [kv_len, num_kv_heads, head_dim]
    device const half* V [[buffer(2)]],        // [kv_len, num_kv_heads, head_dim]
    device half* O [[buffer(3)]],              // [seq_len, num_heads, head_dim]
    constant FusedAttentionParams& params [[buffer(4)]],
    threadgroup half* shared [[threadgroup(0)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;
    const uint head_dim = params.head_dim;

    if (head >= params.num_heads) return;

    // GQA: map query head to KV head
    const uint kv_head = head / (params.num_heads / params.num_kv_heads);

    // Query positions this tile handles
    const uint q_start = q_tile_idx * ATTN_TILE_Q;
    const uint q_end = min(q_start + ATTN_TILE_Q, params.seq_len);

    // Partition shared memory for K and V tiles
    // Layout: K[ATTN_TILE_KV][head_dim+4], V[ATTN_TILE_KV][head_dim+4]
    threadgroup half* shared_k = shared;
    threadgroup half* shared_v = shared + ATTN_TILE_KV * (HEAD_DIM_MAX + 4);

    // Thread-local query register and output accumulator
    const uint thread_id = simd_group * SIMD_SIZE + simd_lane;
    const uint queries_per_threadgroup = (ATTN_TILE_Q + WARPS_PER_BLOCK - 1) / WARPS_PER_BLOCK;

    // Each warp handles a subset of queries
    const uint my_q_offset = simd_group * queries_per_threadgroup;

    // Per-query state: output accumulator and online softmax
    float output_acc[8][HEAD_DIM_MAX];  // Max queries_per_threadgroup = 8
    OnlineSoftmax softmax_state[8];

    // Initialize accumulators
    for (uint q = 0; q < queries_per_threadgroup; q++) {
        softmax_state[q] = OnlineSoftmax::init();
        for (uint d = 0; d < head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    // Load queries for this warp into registers
    half q_reg[8][HEAD_DIM_MAX];
    for (uint q = 0; q < queries_per_threadgroup; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos < q_end) {
            const uint q_base = (q_pos * params.num_heads + head) * head_dim;
            for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                q_reg[q][d] = Q[q_base + d];
            }
        }
    }

    // Number of KV tiles
    const uint num_kv_tiles = (params.kv_len + ATTN_TILE_KV - 1) / ATTN_TILE_KV;

    // Process KV in tiles
    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * ATTN_TILE_KV;
        const uint kv_end = min(kv_start + ATTN_TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // ============ Cooperative Load K and V ============
        // Each thread loads multiple elements for coalesced access
        for (uint t = thread_id; t < kv_tile_len * head_dim; t += WARPS_PER_BLOCK * SIMD_SIZE) {
            const uint kv_local = t / head_dim;
            const uint d = t % head_dim;
            const uint kv_pos = kv_start + kv_local;
            const uint kv_base = (kv_pos * params.num_kv_heads + kv_head) * head_dim;

            if (kv_pos < params.kv_len) {
                shared_k[kv_local * (HEAD_DIM_MAX + 4) + d] = K[kv_base + d];
                shared_v[kv_local * (HEAD_DIM_MAX + 4) + d] = V[kv_base + d];
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // ============ Compute QK^T and Accumulate ============
        for (uint q = 0; q < queries_per_threadgroup; q++) {
            const uint q_pos = q_start + my_q_offset + q;
            if (q_pos >= q_end) continue;

            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;

                // Apply causal mask
                if (params.causal && kv_pos > q_pos) continue;

                // Compute Q.K^T dot product with SIMD reduction
                float dot = 0.0f;
                for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                    float q_val = float(q_reg[q][d]);
                    float k_val = float(shared_k[t * (HEAD_DIM_MAX + 4) + d]);
                    dot += q_val * k_val;
                }
                dot = simd_sum(dot);

                // Scale and update online softmax
                float score = dot * params.scale;

                // ALiBi bias if enabled
                if (params.use_alibi) {
                    float slope = exp2(-8.0f * float(head + 1) / float(params.num_heads));
                    score += slope * float(int(q_pos) - int(kv_pos));
                }

                float rescale = softmax_state[q].update(score);

                // Rescale previous output accumulator
                if (rescale != 1.0f) {
                    for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                        output_acc[q][d] *= rescale;
                    }
                }

                // Compute attention weight and accumulate value
                float weight = softmax_state[q].weight(score);

                for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                    float v_val = float(shared_v[t * (HEAD_DIM_MAX + 4) + d]);
                    output_acc[q][d] += weight * v_val;
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // ============ Finalize and Write Output ============
    for (uint q = 0; q < queries_per_threadgroup; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint out_base = (q_pos * params.num_heads + head) * head_dim;
        float norm = softmax_state[q].normalize();

        // Broadcast norm to all SIMD lanes
        norm = simd_broadcast_first(norm);

        // Vectorized write
        for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
            O[out_base + d] = half(output_acc[q][d] * norm);
        }
    }
}

// ============================================================================
// FUSED ATTENTION FP16 - Higher Throughput Version
// Uses FP16 throughout with FP32 accumulator for accuracy
// ============================================================================
kernel void fused_attention_f16(
    device const half* Q [[buffer(0)]],
    device const half* K [[buffer(1)]],
    device const half* V [[buffer(2)]],
    device half* O [[buffer(3)]],
    constant FusedAttentionParams& params [[buffer(4)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;
    const uint head_dim = params.head_dim;

    if (head >= params.num_heads) return;

    const uint kv_head = head / (params.num_heads / params.num_kv_heads);
    const uint q_start = q_tile_idx * ATTN_TILE_Q;
    const uint q_end = min(q_start + ATTN_TILE_Q, params.seq_len);

    // FP16 shared memory
    threadgroup half shared_k[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));
    threadgroup half shared_v[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));

    // Per-thread state
    const uint thread_id = simd_group * SIMD_SIZE + simd_lane;
    const uint queries_per_warp = (ATTN_TILE_Q + WARPS_PER_BLOCK - 1) / WARPS_PER_BLOCK;
    const uint my_q_offset = simd_group * queries_per_warp;

    // FP32 accumulators for numerical stability
    float output_acc[8][HEAD_DIM_MAX];
    OnlineSoftmax softmax_state[8];

    for (uint q = 0; q < queries_per_warp; q++) {
        softmax_state[q] = OnlineSoftmax::init();
        for (uint d = 0; d < head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    // Load queries as FP16
    half q_reg[8][HEAD_DIM_MAX];
    for (uint q = 0; q < queries_per_warp; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos < q_end) {
            const uint q_base = (q_pos * params.num_heads + head) * head_dim;
            // Vectorized load using half4
            for (uint d = simd_lane * 4; d < head_dim; d += SIMD_SIZE * 4) {
                if (d + 4 <= head_dim) {
                    half4 qv = *reinterpret_cast<device const half4*>(&Q[q_base + d]);
                    q_reg[q][d] = qv.x;
                    q_reg[q][d+1] = qv.y;
                    q_reg[q][d+2] = qv.z;
                    q_reg[q][d+3] = qv.w;
                }
            }
        }
    }

    const uint num_kv_tiles = (params.kv_len + ATTN_TILE_KV - 1) / ATTN_TILE_KV;

    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * ATTN_TILE_KV;
        const uint kv_end = min(kv_start + ATTN_TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // Cooperative load with half4 vectorization
        for (uint t = thread_id; t < kv_tile_len; t += WARPS_PER_BLOCK * SIMD_SIZE / 4) {
            const uint kv_pos = kv_start + t;
            const uint kv_base = (kv_pos * params.num_kv_heads + kv_head) * head_dim;

            if (kv_pos < params.kv_len) {
                for (uint d = 0; d < head_dim; d += 4) {
                    half4 k_vec = *reinterpret_cast<device const half4*>(&K[kv_base + d]);
                    half4 v_vec = *reinterpret_cast<device const half4*>(&V[kv_base + d]);
                    *reinterpret_cast<threadgroup half4*>(&shared_k[t][d]) = k_vec;
                    *reinterpret_cast<threadgroup half4*>(&shared_v[t][d]) = v_vec;
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Compute attention with FP32 accumulator
        for (uint q = 0; q < queries_per_warp; q++) {
            const uint q_pos = q_start + my_q_offset + q;
            if (q_pos >= q_end) continue;

            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;
                if (params.causal && kv_pos > q_pos) continue;

                // FP32 dot product
                float dot = 0.0f;
                #pragma unroll 8
                for (uint d = 0; d < head_dim; d++) {
                    dot = fma(float(q_reg[q][d]), float(shared_k[t][d]), dot);
                }

                float score = dot * params.scale;
                float rescale = softmax_state[q].update(score);

                if (rescale != 1.0f) {
                    for (uint d = 0; d < head_dim; d++) {
                        output_acc[q][d] *= rescale;
                    }
                }

                float weight = softmax_state[q].weight(score);

                #pragma unroll 8
                for (uint d = 0; d < head_dim; d++) {
                    output_acc[q][d] = fma(weight, float(shared_v[t][d]), output_acc[q][d]);
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Write output as FP16
    for (uint q = 0; q < queries_per_warp; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint out_base = (q_pos * params.num_heads + head) * head_dim;
        float norm = softmax_state[q].normalize();

        for (uint d = 0; d < head_dim; d += 4) {
            if (d + 4 <= head_dim) {
                half4 out_vec = half4(
                    half(output_acc[q][d] * norm),
                    half(output_acc[q][d+1] * norm),
                    half(output_acc[q][d+2] * norm),
                    half(output_acc[q][d+3] * norm)
                );
                *reinterpret_cast<device half4*>(&O[out_base + d]) = out_vec;
            }
        }
    }
}

// ============================================================================
// BATCHED FUSED ATTENTION - For batch processing
// Handles multiple sequences in parallel
// ============================================================================
kernel void fused_attention_batched(
    device const half* Q [[buffer(0)]],        // [batch, seq_len, num_heads, head_dim]
    device const half* K [[buffer(1)]],        // [batch, kv_len, num_kv_heads, head_dim]
    device const half* V [[buffer(2)]],        // [batch, kv_len, num_kv_heads, head_dim]
    device half* O [[buffer(3)]],              // [batch, seq_len, num_heads, head_dim]
    constant FusedAttentionParams& params [[buffer(4)]],
    constant uint& batch_size [[buffer(5)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint batch = gid.x;
    const uint head = gid.y;
    const uint q_tile_idx = gid.z;

    if (batch >= batch_size || head >= params.num_heads) return;

    const uint kv_head = head / (params.num_heads / params.num_kv_heads);
    const uint head_dim = params.head_dim;
    const uint q_start = q_tile_idx * ATTN_TILE_Q;
    const uint q_end = min(q_start + ATTN_TILE_Q, params.seq_len);

    // Offset into batch
    const uint q_batch_offset = batch * params.seq_len * params.num_heads * head_dim;
    const uint kv_batch_offset = batch * params.kv_len * params.num_kv_heads * head_dim;
    const uint o_batch_offset = batch * params.seq_len * params.num_heads * head_dim;

    // Shared memory
    threadgroup half shared_k[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));
    threadgroup half shared_v[ATTN_TILE_KV][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));

    const uint thread_id = simd_group * SIMD_SIZE + simd_lane;
    const uint queries_per_warp = (ATTN_TILE_Q + WARPS_PER_BLOCK - 1) / WARPS_PER_BLOCK;
    const uint my_q_offset = simd_group * queries_per_warp;

    float output_acc[8][HEAD_DIM_MAX];
    OnlineSoftmax softmax_state[8];

    for (uint q = 0; q < queries_per_warp; q++) {
        softmax_state[q] = OnlineSoftmax::init();
        for (uint d = 0; d < head_dim; d++) {
            output_acc[q][d] = 0.0f;
        }
    }

    // Load queries
    half q_reg[8][HEAD_DIM_MAX];
    for (uint q = 0; q < queries_per_warp; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos < q_end) {
            const uint q_base = q_batch_offset + (q_pos * params.num_heads + head) * head_dim;
            for (uint d = 0; d < head_dim; d++) {
                q_reg[q][d] = Q[q_base + d];
            }
        }
    }

    const uint num_kv_tiles = (params.kv_len + ATTN_TILE_KV - 1) / ATTN_TILE_KV;

    for (uint kv_tile = 0; kv_tile < num_kv_tiles; kv_tile++) {
        const uint kv_start = kv_tile * ATTN_TILE_KV;
        const uint kv_end = min(kv_start + ATTN_TILE_KV, params.kv_len);
        const uint kv_tile_len = kv_end - kv_start;

        // Load K, V
        for (uint t = thread_id; t < kv_tile_len; t += WARPS_PER_BLOCK * SIMD_SIZE / head_dim) {
            const uint kv_pos = kv_start + t;
            const uint kv_base = kv_batch_offset + (kv_pos * params.num_kv_heads + kv_head) * head_dim;

            if (kv_pos < params.kv_len) {
                for (uint d = 0; d < head_dim; d++) {
                    shared_k[t][d] = K[kv_base + d];
                    shared_v[t][d] = V[kv_base + d];
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Compute
        for (uint q = 0; q < queries_per_warp; q++) {
            const uint q_pos = q_start + my_q_offset + q;
            if (q_pos >= q_end) continue;

            for (uint t = 0; t < kv_tile_len; t++) {
                const uint kv_pos = kv_start + t;
                if (params.causal && kv_pos > q_pos) continue;

                float dot = 0.0f;
                for (uint d = 0; d < head_dim; d++) {
                    dot = fma(float(q_reg[q][d]), float(shared_k[t][d]), dot);
                }

                float score = dot * params.scale;
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
    for (uint q = 0; q < queries_per_warp; q++) {
        const uint q_pos = q_start + my_q_offset + q;
        if (q_pos >= q_end) continue;

        const uint out_base = o_batch_offset + (q_pos * params.num_heads + head) * head_dim;
        float norm = softmax_state[q].normalize();

        for (uint d = 0; d < head_dim; d++) {
            O[out_base + d] = half(output_acc[q][d] * norm);
        }
    }
}

// ============================================================================
// PAGED ATTENTION - For KV cache with variable lengths
// Supports paged KV cache for memory efficiency
// ============================================================================
struct PagedAttentionParams {
    uint num_heads;
    uint num_kv_heads;
    uint head_dim;
    uint block_size;     // KV cache block size
    uint num_blocks;     // Total number of blocks
    float scale;
    uint causal;
    uint _padding;
};

kernel void paged_attention(
    device const half* Q [[buffer(0)]],           // [seq_len, num_heads, head_dim]
    device const half* K_cache [[buffer(1)]],     // Paged [num_blocks, block_size, num_kv_heads, head_dim]
    device const half* V_cache [[buffer(2)]],     // Paged [num_blocks, block_size, num_kv_heads, head_dim]
    device const uint* block_tables [[buffer(3)]], // [seq_len, max_blocks_per_seq]
    device const uint* context_lens [[buffer(4)]], // [seq_len] actual KV lengths
    device half* O [[buffer(5)]],                 // [seq_len, num_heads, head_dim]
    constant PagedAttentionParams& params [[buffer(6)]],
    uint3 tid [[thread_position_in_threadgroup]],
    uint3 gid [[threadgroup_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint q_idx = gid.z;
    const uint head = gid.y;
    const uint head_dim = params.head_dim;

    if (head >= params.num_heads) return;

    const uint kv_head = head / (params.num_heads / params.num_kv_heads);
    const uint context_len = context_lens[q_idx];
    const uint block_size = params.block_size;

    // Shared memory for K, V block
    threadgroup half shared_k[64][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));
    threadgroup half shared_v[64][HEAD_DIM_MAX + 4] __attribute__((aligned(16)));

    // Load query
    const uint q_base = (q_idx * params.num_heads + head) * head_dim;
    half q_reg[HEAD_DIM_MAX];
    for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
        q_reg[d] = Q[q_base + d];
    }

    // Online softmax state
    OnlineSoftmax softmax_state = OnlineSoftmax::init();
    float output_acc[HEAD_DIM_MAX];
    for (uint d = 0; d < head_dim; d++) {
        output_acc[d] = 0.0f;
    }

    // Iterate over blocks
    const uint num_kv_blocks = (context_len + block_size - 1) / block_size;
    const uint thread_id = simd_group * SIMD_SIZE + simd_lane;

    for (uint block_idx = 0; block_idx < num_kv_blocks; block_idx++) {
        const uint physical_block = block_tables[q_idx * params.num_blocks + block_idx];
        const uint block_start = block_idx * block_size;
        const uint block_end = min(block_start + block_size, context_len);
        const uint block_len = block_end - block_start;

        // Load K, V from paged cache
        const uint cache_base = (physical_block * block_size * params.num_kv_heads + kv_head) * head_dim;

        for (uint t = thread_id; t < block_len; t += WARPS_PER_BLOCK * SIMD_SIZE / head_dim) {
            const uint cache_offset = cache_base + t * params.num_kv_heads * head_dim;
            for (uint d = 0; d < head_dim; d++) {
                shared_k[t][d] = K_cache[cache_offset + d];
                shared_v[t][d] = V_cache[cache_offset + d];
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Compute attention for this block
        for (uint t = 0; t < block_len; t++) {
            const uint kv_pos = block_start + t;
            if (params.causal && kv_pos > q_idx) continue;

            float dot = 0.0f;
            for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                dot += float(q_reg[d]) * float(shared_k[t][d]);
            }
            dot = simd_sum(dot);

            float score = dot * params.scale;
            float rescale = softmax_state.update(score);

            if (rescale != 1.0f) {
                for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                    output_acc[d] *= rescale;
                }
            }

            float weight = softmax_state.weight(score);
            for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
                output_acc[d] += weight * float(shared_v[t][d]);
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Write output
    const uint out_base = (q_idx * params.num_heads + head) * head_dim;
    float norm = softmax_state.normalize();

    for (uint d = simd_lane; d < head_dim; d += SIMD_SIZE) {
        O[out_base + d] = half(output_acc[d] * norm);
    }
}
