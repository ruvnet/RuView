// Flash Attention Shader
//
// Implements memory-efficient attention using the Flash Attention algorithm.
// Target: 2ms for 4K context length.
//
// Algorithm (Flash Attention v2):
// 1. Process Q in blocks, streaming K and V
// 2. Maintain running max and sum for numerical stability
// 3. Rescale outputs on-the-fly
// 4. Avoid materializing full attention matrix (O(n) memory vs O(n^2))
//
// Memory Layout:
// - Q: (seq_len, num_heads * head_dim) - queries
// - K: (seq_len, num_heads * head_dim) - keys
// - V: (seq_len, num_heads * head_dim) - values
// - Output: (seq_len, num_heads * head_dim)

// Block size for flash attention (balance between parallelism and memory)
const BLOCK_SIZE: u32 = 64u;
const WARP_SIZE: u32 = 32u;

struct Uniforms {
    seq_len: f32,
    head_dim: f32,
    num_heads: f32,
    scale: f32,       // 1/sqrt(head_dim)
    causal_mask: f32, // 1.0 for causal, 0.0 for full
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<storage, read> Q: array<f32>;
@group(0) @binding(1) var<storage, read> K: array<f32>;
@group(0) @binding(2) var<storage, read> V: array<f32>;
@group(0) @binding(3) var<storage, read_write> Output: array<f32>;
@group(0) @binding(4) var<uniform> uniforms: Uniforms;

// Shared memory for Q, K, V blocks
var<workgroup> Q_block: array<f32, 4096>;  // BLOCK_SIZE * 64 (max head_dim)
var<workgroup> K_block: array<f32, 4096>;
var<workgroup> V_block: array<f32, 4096>;
var<workgroup> scores: array<f32, 4096>;   // BLOCK_SIZE * BLOCK_SIZE

// Thread-local accumulators
var<private> m_prev: f32;     // Previous max score
var<private> l_prev: f32;     // Previous sum of exp(scores - max)
var<private> acc: array<f32, 64>;  // Output accumulator (head_dim)

// Compute softmax denominator using online algorithm
fn online_softmax_update(
    new_max: f32,
    old_max: f32,
    old_sum: f32,
    new_scores: ptr<function, array<f32, 64>>,
    block_len: u32,
) -> f32 {
    // Rescale old sum
    var new_sum = old_sum * exp(old_max - new_max);

    // Add new contributions
    for (var i = 0u; i < block_len; i++) {
        new_sum += exp((*new_scores)[i] - new_max);
    }

    return new_sum;
}

@compute @workgroup_size(64, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let seq_len = u32(uniforms.seq_len);
    let head_dim = u32(uniforms.head_dim);
    let num_heads = u32(uniforms.num_heads);
    let scale = uniforms.scale;
    let is_causal = uniforms.causal_mask > 0.5;

    // This workgroup processes one block of Q for one head
    let head_idx = group_id.y;
    let q_block_idx = group_id.x;
    let q_start = q_block_idx * BLOCK_SIZE;

    let thread_id = local_id.x;
    let hidden_dim = num_heads * head_dim;

    // Initialize accumulators
    m_prev = -1e10;  // Very negative (will be updated)
    l_prev = 0.0;
    for (var i = 0u; i < 64u; i++) {
        acc[i] = 0.0;
    }

    // Load Q block into shared memory
    // Each thread loads one position's head_dim values
    let q_pos = q_start + thread_id;
    if (q_pos < seq_len && thread_id < BLOCK_SIZE) {
        for (var d = 0u; d < head_dim; d++) {
            let q_idx = q_pos * hidden_dim + head_idx * head_dim + d;
            Q_block[thread_id * head_dim + d] = Q[q_idx];
        }
    }
    workgroupBarrier();

    // Iterate over K/V blocks
    let num_kv_blocks = (seq_len + BLOCK_SIZE - 1u) / BLOCK_SIZE;
    let max_kv_block = select(num_kv_blocks, q_block_idx + 1u, is_causal);

    for (var kv_block_idx = 0u; kv_block_idx < max_kv_block; kv_block_idx++) {
        let kv_start = kv_block_idx * BLOCK_SIZE;

        // Load K block into shared memory
        let k_pos = kv_start + thread_id;
        if (k_pos < seq_len && thread_id < BLOCK_SIZE) {
            for (var d = 0u; d < head_dim; d++) {
                let k_idx = k_pos * hidden_dim + head_idx * head_dim + d;
                K_block[thread_id * head_dim + d] = K[k_idx];
            }
        }

        // Load V block into shared memory
        let v_pos = kv_start + thread_id;
        if (v_pos < seq_len && thread_id < BLOCK_SIZE) {
            for (var d = 0u; d < head_dim; d++) {
                let v_idx = v_pos * hidden_dim + head_idx * head_dim + d;
                V_block[thread_id * head_dim + d] = V[v_idx];
            }
        }
        workgroupBarrier();

        // Compute attention scores for this Q position against all K in block
        // Each thread handles one Q position
        if (thread_id < BLOCK_SIZE && q_pos < seq_len) {
            let kv_block_len = min(BLOCK_SIZE, seq_len - kv_start);

            // Compute Q @ K^T for this thread's Q position
            var local_scores: array<f32, 64>;
            var block_max = -1e10f;

            for (var k = 0u; k < kv_block_len; k++) {
                let k_global = kv_start + k;

                // Causal mask: skip future positions
                if (is_causal && k_global > q_pos) {
                    local_scores[k] = -1e10;
                    continue;
                }

                // Dot product Q[thread] @ K[k]
                var score = 0.0f;
                for (var d = 0u; d < head_dim; d++) {
                    score += Q_block[thread_id * head_dim + d] * K_block[k * head_dim + d];
                }
                score *= scale;

                local_scores[k] = score;
                block_max = max(block_max, score);
            }

            // Update running max
            let new_max = max(m_prev, block_max);

            // Compute rescaling factors
            let scale_old = exp(m_prev - new_max);
            let scale_new = exp(block_max - new_max);

            // Rescale previous accumulator
            for (var d = 0u; d < head_dim; d++) {
                acc[d] *= scale_old;
            }
            l_prev *= scale_old;

            // Compute exp(scores - new_max) and accumulate
            var block_sum = 0.0f;
            for (var k = 0u; k < kv_block_len; k++) {
                let k_global = kv_start + k;
                if (is_causal && k_global > q_pos) {
                    continue;
                }

                let p = exp(local_scores[k] - new_max);
                block_sum += p;

                // Accumulate weighted V
                for (var d = 0u; d < head_dim; d++) {
                    acc[d] += p * V_block[k * head_dim + d];
                }
            }

            // Update running sum
            l_prev += block_sum;
            m_prev = new_max;
        }

        workgroupBarrier();
    }

    // Normalize and write output
    if (thread_id < BLOCK_SIZE && q_pos < seq_len) {
        let inv_sum = select(1.0 / l_prev, 0.0, l_prev == 0.0);

        for (var d = 0u; d < head_dim; d++) {
            let out_idx = q_pos * hidden_dim + head_idx * head_dim + d;
            Output[out_idx] = acc[d] * inv_sum;
        }
    }
}

// Multi-head attention with grouped-query attention (GQA) support
@compute @workgroup_size(64, 1, 1)
fn main_gqa(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // GQA: Multiple Q heads share same K/V heads
    // kv_head = q_head / num_q_per_kv
    // Left as placeholder for models like Llama 2/3
}

// Sliding window attention variant
@compute @workgroup_size(64, 1, 1)
fn main_sliding_window(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // Only attend to positions within window_size
    // Useful for very long sequences (Mistral-style)
    // Left as placeholder
}
