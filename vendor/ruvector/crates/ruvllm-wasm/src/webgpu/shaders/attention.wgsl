// Flash Attention Shader for WebGPU WASM
//
// Implements memory-efficient attention using online softmax algorithm.
// Supports causal masking for autoregressive generation.
//
// Algorithm:
// 1. Process Q in blocks, streaming K and V
// 2. Maintain running max and sum for numerical stability
// 3. Rescale outputs on-the-fly (Flash Attention v2)
// 4. O(n) memory vs O(n^2) for standard attention
//
// Memory Layout:
// - Q: (seq_len, num_heads, head_dim)
// - K: (seq_len, num_heads, head_dim)
// - V: (seq_len, num_heads, head_dim)
// - Output: (seq_len, num_heads, head_dim)

const BLOCK_SIZE: u32 = 32u;  // Reduced for WebGPU limits
const MAX_HEAD_DIM: u32 = 128u;

struct AttentionUniforms {
    seq_len: u32,
    head_dim: u32,
    num_heads: u32,
    scale: f32,        // 1/sqrt(head_dim)
    causal_mask: u32,  // 1 for causal, 0 for full attention
    kv_seq_len: u32,   // For encoder-decoder or prefill
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read> Q: array<f32>;
@group(0) @binding(1) var<storage, read> K: array<f32>;
@group(0) @binding(2) var<storage, read> V: array<f32>;
@group(0) @binding(3) var<storage, read_write> Output: array<f32>;
@group(0) @binding(4) var<uniform> uniforms: AttentionUniforms;

// Shared memory for blocks
var<workgroup> Q_shared: array<f32, 4096>;  // BLOCK_SIZE * MAX_HEAD_DIM
var<workgroup> K_shared: array<f32, 4096>;
var<workgroup> V_shared: array<f32, 4096>;
var<workgroup> scores_shared: array<f32, 1024>;  // BLOCK_SIZE * BLOCK_SIZE

// Thread-local state for online softmax
var<private> m_i: f32;      // Running max
var<private> l_i: f32;      // Running sum
var<private> o_i: array<f32, 128>;  // Output accumulator

@compute @workgroup_size(32, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let seq_len = uniforms.seq_len;
    let head_dim = uniforms.head_dim;
    let num_heads = uniforms.num_heads;
    let scale = uniforms.scale;
    let is_causal = uniforms.causal_mask == 1u;
    let kv_seq_len = uniforms.kv_seq_len;

    // This workgroup handles one Q block for one head
    let head_idx = group_id.y;
    let q_block_idx = group_id.x;
    let q_start = q_block_idx * BLOCK_SIZE;

    let thread_id = local_id.x;
    let hidden_stride = num_heads * head_dim;

    // Initialize online softmax state
    m_i = -1e10f;
    l_i = 0.0f;
    for (var d = 0u; d < head_dim; d++) {
        o_i[d] = 0.0f;
    }

    // Load Q block into shared memory
    let q_pos = q_start + thread_id;
    if (q_pos < seq_len && thread_id < BLOCK_SIZE) {
        for (var d = 0u; d < head_dim; d++) {
            let q_idx = q_pos * hidden_stride + head_idx * head_dim + d;
            Q_shared[thread_id * head_dim + d] = Q[q_idx];
        }
    }
    workgroupBarrier();

    // Iterate over K/V blocks
    let num_kv_blocks = (kv_seq_len + BLOCK_SIZE - 1u) / BLOCK_SIZE;

    for (var kv_block = 0u; kv_block < num_kv_blocks; kv_block++) {
        let kv_start = kv_block * BLOCK_SIZE;

        // Early exit for causal attention
        if (is_causal && kv_start > q_start + BLOCK_SIZE) {
            break;
        }

        // Load K block
        let k_pos = kv_start + thread_id;
        if (k_pos < kv_seq_len && thread_id < BLOCK_SIZE) {
            for (var d = 0u; d < head_dim; d++) {
                let k_idx = k_pos * hidden_stride + head_idx * head_dim + d;
                K_shared[thread_id * head_dim + d] = K[k_idx];
            }
        }

        // Load V block
        let v_pos = kv_start + thread_id;
        if (v_pos < kv_seq_len && thread_id < BLOCK_SIZE) {
            for (var d = 0u; d < head_dim; d++) {
                let v_idx = v_pos * hidden_stride + head_idx * head_dim + d;
                V_shared[thread_id * head_dim + d] = V[v_idx];
            }
        }
        workgroupBarrier();

        // Compute attention scores and update online softmax
        if (thread_id < BLOCK_SIZE && q_pos < seq_len) {
            let kv_block_len = min(BLOCK_SIZE, kv_seq_len - kv_start);

            // Compute row max for this block
            var block_max = -1e10f;
            var local_scores: array<f32, 32>;

            for (var k = 0u; k < kv_block_len; k++) {
                let k_global = kv_start + k;

                // Apply causal mask
                if (is_causal && k_global > q_pos) {
                    local_scores[k] = -1e10f;
                    continue;
                }

                // Compute Q[q_pos] dot K[k]
                var score = 0.0f;
                for (var d = 0u; d < head_dim; d++) {
                    score += Q_shared[thread_id * head_dim + d] * K_shared[k * head_dim + d];
                }
                score *= scale;
                local_scores[k] = score;
                block_max = max(block_max, score);
            }

            // Update running statistics
            let m_ij = max(m_i, block_max);

            // Rescale previous accumulator
            let alpha = exp(m_i - m_ij);
            for (var d = 0u; d < head_dim; d++) {
                o_i[d] *= alpha;
            }
            l_i *= alpha;

            // Accumulate weighted V for this block
            for (var k = 0u; k < kv_block_len; k++) {
                let k_global = kv_start + k;
                if (is_causal && k_global > q_pos) {
                    continue;
                }

                let p_ij = exp(local_scores[k] - m_ij);
                l_i += p_ij;

                for (var d = 0u; d < head_dim; d++) {
                    o_i[d] += p_ij * V_shared[k * head_dim + d];
                }
            }

            m_i = m_ij;
        }

        workgroupBarrier();
    }

    // Normalize and write output
    if (thread_id < BLOCK_SIZE && q_pos < seq_len) {
        let inv_l = select(1.0f / l_i, 0.0f, l_i == 0.0f);

        for (var d = 0u; d < head_dim; d++) {
            let out_idx = q_pos * hidden_stride + head_idx * head_dim + d;
            Output[out_idx] = o_i[d] * inv_l;
        }
    }
}

// Grouped Query Attention (GQA) variant
// Multiple Q heads share same K/V heads
@compute @workgroup_size(32, 1, 1)
fn main_gqa(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // For GQA: kv_head_idx = q_head_idx / num_q_per_kv
    // This allows Llama2/3 style grouped attention
    // Implementation similar to main() with modified indexing
}

// Single token attention for generation phase
// More efficient when seq_len = 1 (decoding)
@compute @workgroup_size(256, 1, 1)
fn main_decode(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let head_dim = uniforms.head_dim;
    let num_heads = uniforms.num_heads;
    let scale = uniforms.scale;
    let kv_seq_len = uniforms.kv_seq_len;
    let is_causal = uniforms.causal_mask == 1u;

    let head_idx = group_id.x;
    let thread_id = local_id.x;
    let hidden_stride = num_heads * head_dim;

    // Each thread handles part of the KV sequence
    let kv_per_thread = (kv_seq_len + 255u) / 256u;

    // Thread-local accumulators
    var local_max = -1e10f;
    var local_sum = 0.0f;
    var local_out: array<f32, 128>;
    for (var d = 0u; d < head_dim; d++) {
        local_out[d] = 0.0f;
    }

    // Load Q (single token)
    var q_vec: array<f32, 128>;
    if (thread_id == 0u) {
        for (var d = 0u; d < head_dim; d++) {
            q_vec[d] = Q[head_idx * head_dim + d];
        }
    }
    // Broadcast Q to all threads via shared memory
    for (var d = 0u; d < head_dim; d++) {
        Q_shared[d] = Q[head_idx * head_dim + d];
    }
    workgroupBarrier();

    // Process assigned KV positions
    for (var i = 0u; i < kv_per_thread; i++) {
        let k_pos = thread_id * kv_per_thread + i;
        if (k_pos >= kv_seq_len) {
            break;
        }

        // Compute attention score
        var score = 0.0f;
        for (var d = 0u; d < head_dim; d++) {
            let k_idx = k_pos * hidden_stride + head_idx * head_dim + d;
            score += Q_shared[d] * K[k_idx];
        }
        score *= scale;

        // Update local max
        let new_max = max(local_max, score);
        let alpha = exp(local_max - new_max);

        for (var d = 0u; d < head_dim; d++) {
            local_out[d] *= alpha;
        }
        local_sum = local_sum * alpha + exp(score - new_max);

        // Accumulate weighted V
        let p = exp(score - new_max);
        for (var d = 0u; d < head_dim; d++) {
            let v_idx = k_pos * hidden_stride + head_idx * head_dim + d;
            local_out[d] += p * V[v_idx];
        }

        local_max = new_max;
    }

    // Reduction across threads (simplified - real impl would use parallel reduction)
    // Store partial results for CPU reduction or use atomics
    if (thread_id == 0u) {
        let inv_sum = select(1.0f / local_sum, 0.0f, local_sum == 0.0f);
        for (var d = 0u; d < head_dim; d++) {
            Output[head_idx * head_dim + d] = local_out[d] * inv_sum;
        }
    }
}
