//
// Rotary Position Embeddings (RoPE) - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro
//
// Applies rotary embeddings to query and key tensors for position encoding.
// Used in LLaMA, Mistral, and other modern transformer architectures.
//
// Optimizations:
// - Precomputed sin/cos tables in constant memory
// - Batch processing of multiple positions
// - Vectorized memory access (float2/half2)
// - Fused rotation operations
//

#include <metal_stdlib>
using namespace metal;

// Constants for precomputed tables
constant uint MAX_SEQ_LEN = 8192;    // Maximum sequence length
constant uint MAX_HEAD_DIM = 128;    // Maximum head dimension
constant uint SIMD_SIZE = 32;

// RoPE parameters structure (matches Rust RopeParams)
struct RopeParams {
    uint head_dim;      // Head dimension (must be even)
    uint num_heads;     // Number of heads
    uint position;      // Current position
    float theta_base;   // Base for frequency calculation (default 10000)
};

// Extended RoPE parameters for batch processing
struct RopeBatchParams {
    uint head_dim;
    uint num_heads;
    uint seq_len;
    uint batch_size;
    float theta_base;
    uint _padding[3];
};

// =============================================================================
// Apply RoPE with precomputed sin/cos tables (fastest version)
// Tables should be precomputed once per model load
// =============================================================================
kernel void apply_rope_precomputed(
    device float* x [[buffer(0)]],
    constant float* cos_table [[buffer(1)]],  // [position, head_dim/2] in constant memory
    constant float* sin_table [[buffer(2)]],  // [position, head_dim/2] in constant memory
    constant RopeParams& params [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;           // Position in head dimension (0 to head_dim/2 - 1)
    uint head = gid.y;        // Head index
    uint batch = gid.z;       // Batch index

    uint head_dim = params.head_dim;
    uint num_heads = params.num_heads;

    if (d >= head_dim / 2 || head >= num_heads) {
        return;
    }

    // Offset into the tensor
    uint offset = (batch * num_heads + head) * head_dim;

    // Load pair of values
    float x0 = x[offset + 2 * d];
    float x1 = x[offset + 2 * d + 1];

    // Get precomputed cos/sin from constant memory
    uint table_offset = params.position * (head_dim / 2) + d;
    float cos_val = cos_table[table_offset];
    float sin_val = sin_table[table_offset];

    // Apply rotation with fused multiply-add
    x[offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// Vectorized RoPE using float2 for paired elements
// =============================================================================
kernel void apply_rope_vec2(
    device float2* x [[buffer(0)]],  // Reinterpret as pairs
    constant float* cos_table [[buffer(1)]],
    constant float* sin_table [[buffer(2)]],
    constant RopeParams& params [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;           // Pair index (0 to head_dim/2 - 1)
    uint head = gid.y;
    uint batch = gid.z;

    uint head_dim = params.head_dim;
    uint num_heads = params.num_heads;

    if (d >= head_dim / 2 || head >= num_heads) {
        return;
    }

    // Offset for float2 access
    uint offset = (batch * num_heads + head) * (head_dim / 2) + d;

    float2 val = x[offset];

    uint table_offset = params.position * (head_dim / 2) + d;
    float cos_val = cos_table[table_offset];
    float sin_val = sin_table[table_offset];

    // Apply rotation: [x0*cos - x1*sin, x0*sin + x1*cos]
    float2 rotated;
    rotated.x = fma(val.x, cos_val, -val.y * sin_val);
    rotated.y = fma(val.x, sin_val, val.y * cos_val);

    x[offset] = rotated;
}

// =============================================================================
// Original RoPE with precomputed tables (kept for compatibility)
// =============================================================================
kernel void apply_rope(
    device float* x [[buffer(0)]],
    device const float* cos_table [[buffer(1)]],
    device const float* sin_table [[buffer(2)]],
    constant RopeParams& params [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]]
) {
    uint d = gid.x;
    uint head = gid.y;
    uint batch = gid.z;

    uint head_dim = params.head_dim;
    uint num_heads = params.num_heads;

    if (d >= head_dim / 2) {
        return;
    }

    uint offset = (batch * num_heads + head) * head_dim;

    float x0 = x[offset + 2 * d];
    float x1 = x[offset + 2 * d + 1];

    float cos_val = cos_table[d];
    float sin_val = sin_table[d];

    x[offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// RoPE with inline frequency computation (no precomputed tables)
// Useful when tables aren't available or for dynamic positions
// =============================================================================
kernel void apply_rope_inline(
    device float* x [[buffer(0)]],
    constant RopeParams& params [[buffer(1)]],
    uint3 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]]
) {
    uint d = gid.x;
    uint head = gid.y;
    uint batch = gid.z;

    uint head_dim = params.head_dim;
    uint num_heads = params.num_heads;
    uint position = params.position;
    float theta_base = params.theta_base;

    if (d >= head_dim / 2) {
        return;
    }

    uint offset = (batch * num_heads + head) * head_dim;

    // Compute frequency for this dimension
    // freq = 1 / (theta_base ^ (2d / head_dim))
    float freq = 1.0f / pow(theta_base, float(2 * d) / float(head_dim));
    float angle = float(position) * freq;

    // Use fast sin/cos
    float cos_val = cos(angle);
    float sin_val = sin(angle);

    float x0 = x[offset + 2 * d];
    float x1 = x[offset + 2 * d + 1];

    x[offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// Batched RoPE for multiple positions (efficient for prefill)
// Processes entire sequences in parallel with precomputed tables
// =============================================================================
kernel void apply_rope_batched_v2(
    device float* x [[buffer(0)]],                // [batch, seq_len, num_heads, head_dim]
    constant float* cos_table [[buffer(1)]],      // [max_seq_len, head_dim/2]
    constant float* sin_table [[buffer(2)]],      // [max_seq_len, head_dim/2]
    device const uint* positions [[buffer(3)]],   // [batch, seq_len] position indices
    constant RopeBatchParams& params [[buffer(4)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;                    // Dimension pair index
    uint head = gid.y;                 // Head index
    uint seq_batch = gid.z;            // Combined sequence + batch index

    uint head_dim = params.head_dim;
    uint num_heads = params.num_heads;
    uint seq_len = params.seq_len;

    if (d >= head_dim / 2 || head >= num_heads) {
        return;
    }

    uint batch = seq_batch / seq_len;
    uint seq_pos = seq_batch % seq_len;

    if (batch >= params.batch_size) {
        return;
    }

    // Get position for this token
    uint position = positions[batch * seq_len + seq_pos];

    // Compute offsets
    uint x_offset = ((batch * seq_len + seq_pos) * num_heads + head) * head_dim;
    uint table_offset = position * (head_dim / 2) + d;

    // Load values as pair
    float x0 = x[x_offset + 2 * d];
    float x1 = x[x_offset + 2 * d + 1];

    // Get sin/cos from constant memory
    float cos_val = cos_table[table_offset];
    float sin_val = sin_table[table_offset];

    // Apply rotation
    x[x_offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[x_offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// Original batched RoPE (kept for compatibility)
// =============================================================================
kernel void apply_rope_batched(
    device float* x [[buffer(0)]],
    device const uint* positions [[buffer(1)]],
    constant uint& num_heads [[buffer(2)]],
    constant uint& head_dim [[buffer(3)]],
    constant uint& seq_len [[buffer(4)]],
    constant float& theta_base [[buffer(5)]],
    uint3 gid [[thread_position_in_grid]],
    uint3 group_id [[threadgroup_position_in_grid]]
) {
    uint d = gid.x;
    uint head = gid.y;
    uint seq_batch = gid.z;

    uint batch = seq_batch / seq_len;
    uint seq_pos = seq_batch % seq_len;

    if (d >= head_dim / 2) {
        return;
    }

    uint position = positions[batch * seq_len + seq_pos];
    uint offset = ((batch * seq_len + seq_pos) * num_heads + head) * head_dim;

    float freq = 1.0f / pow(theta_base, float(2 * d) / float(head_dim));
    float angle = float(position) * freq;

    float cos_val = cos(angle);
    float sin_val = sin(angle);

    float x0 = x[offset + 2 * d];
    float x1 = x[offset + 2 * d + 1];

    x[offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// FP16 RoPE with half2 vectorization
// =============================================================================
kernel void apply_rope_f16_v2(
    device half2* x [[buffer(0)]],  // Reinterpret as pairs for vectorized access
    constant half* cos_table [[buffer(1)]],
    constant half* sin_table [[buffer(2)]],
    constant RopeParams& params [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;
    uint head = gid.y;
    uint batch = gid.z;

    if (d >= params.head_dim / 2) {
        return;
    }

    uint offset = (batch * params.num_heads + head) * (params.head_dim / 2) + d;
    uint table_offset = params.position * (params.head_dim / 2) + d;

    half2 val = x[offset];
    half cos_val = cos_table[table_offset];
    half sin_val = sin_table[table_offset];

    half2 rotated;
    rotated.x = fma(val.x, cos_val, -val.y * sin_val);
    rotated.y = fma(val.x, sin_val, val.y * cos_val);

    x[offset] = rotated;
}

// =============================================================================
// Original FP16 RoPE (kept for compatibility)
// =============================================================================
kernel void apply_rope_f16(
    device half* x [[buffer(0)]],
    device const half* cos_table [[buffer(1)]],
    device const half* sin_table [[buffer(2)]],
    constant RopeParams& params [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;
    uint head = gid.y;
    uint batch = gid.z;

    if (d >= params.head_dim / 2) {
        return;
    }

    uint offset = (batch * params.num_heads + head) * params.head_dim;

    half x0 = x[offset + 2 * d];
    half x1 = x[offset + 2 * d + 1];

    half cos_val = cos_table[d];
    half sin_val = sin_table[d];

    x[offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// Precompute RoPE cos/sin tables (run once per model load)
// Output can be stored in constant memory for fast access
// =============================================================================
kernel void precompute_rope_tables_v2(
    device float* cos_table [[buffer(0)]],  // [max_seq_len, head_dim/2]
    device float* sin_table [[buffer(1)]],  // [max_seq_len, head_dim/2]
    constant uint& head_dim [[buffer(2)]],
    constant uint& max_seq_len [[buffer(3)]],
    constant float& theta_base [[buffer(4)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]]
) {
    uint pos = gid.y;
    uint d = gid.x;

    if (pos >= max_seq_len || d >= head_dim / 2) {
        return;
    }

    // Compute frequency using reciprocal to avoid repeated division
    float inv_freq = 1.0f / pow(theta_base, float(2 * d) / float(head_dim));
    float angle = float(pos) * inv_freq;

    // Use sincos for efficiency when available
    float s, c;
    s = sin(angle);
    c = cos(angle);

    uint idx = pos * (head_dim / 2) + d;
    cos_table[idx] = c;
    sin_table[idx] = s;
}

// =============================================================================
// Original table precomputation (kept for compatibility)
// =============================================================================
kernel void precompute_rope_tables(
    device float* cos_table [[buffer(0)]],
    device float* sin_table [[buffer(1)]],
    constant uint& head_dim [[buffer(2)]],
    constant uint& max_seq_len [[buffer(3)]],
    constant float& theta_base [[buffer(4)]],
    uint2 gid [[thread_position_in_grid]]
) {
    uint pos = gid.y;
    uint d = gid.x;

    if (pos >= max_seq_len || d >= head_dim / 2) {
        return;
    }

    float freq = 1.0f / pow(theta_base, float(2 * d) / float(head_dim));
    float angle = float(pos) * freq;

    uint idx = pos * (head_dim / 2) + d;
    cos_table[idx] = cos(angle);
    sin_table[idx] = sin(angle);
}

// =============================================================================
// ALiBi (Attention with Linear Biases) - alternative to RoPE
// =============================================================================
kernel void apply_alibi(
    device float* attn_scores [[buffer(0)]],
    constant uint& seq_len [[buffer(1)]],
    constant uint& kv_len [[buffer(2)]],
    constant uint& num_heads [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint q_pos = gid.y;
    uint k_pos = gid.x;
    uint batch_head = gid.z;

    uint batch = batch_head / num_heads;
    uint head = batch_head % num_heads;

    if (q_pos >= seq_len || k_pos >= kv_len) {
        return;
    }

    // ALiBi slope: 2^(-8*(h+1)/H) where h is head index, H is total heads
    float slope = exp2(-8.0f * float(head + 1) / float(num_heads));
    int pos_diff = int(q_pos) - int(k_pos);
    float bias = slope * float(pos_diff);

    uint idx = ((batch * num_heads + head) * seq_len + q_pos) * kv_len + k_pos;
    attn_scores[idx] += bias;
}

// =============================================================================
// YaRN (Yet another RoPE extension) for extended context
// =============================================================================
struct YaRNParams {
    uint head_dim;
    uint num_heads;
    uint position;
    float theta_base;
    float scale;           // Position scale factor
    float attn_scale;      // Attention scale factor
    float beta_fast;       // High-frequency extrapolation factor
    float beta_slow;       // Low-frequency interpolation factor
    uint original_max_len; // Original training context length
};

kernel void apply_rope_yarn(
    device float* x [[buffer(0)]],
    constant YaRNParams& params [[buffer(1)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;
    uint head = gid.y;
    uint batch = gid.z;

    if (d >= params.head_dim / 2) {
        return;
    }

    uint offset = (batch * params.num_heads + head) * params.head_dim;

    // YaRN frequency scaling
    float freq_base = 1.0f / pow(params.theta_base, float(2 * d) / float(params.head_dim));
    float wavelength = 2.0f * M_PI_F / freq_base;

    // Compute ramp function
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
    float freq = freq_base * (1.0f - ramp + ramp / params.scale);
    float angle = float(params.position) * freq;

    float cos_val = cos(angle);
    float sin_val = sin(angle);

    float x0 = x[offset + 2 * d];
    float x1 = x[offset + 2 * d + 1];

    x[offset + 2 * d] = fma(x0, cos_val, -x1 * sin_val);
    x[offset + 2 * d + 1] = fma(x0, sin_val, x1 * cos_val);
}

// =============================================================================
// Fused RoPE for Q and K together (common pattern in transformers)
// Applies RoPE to both query and key tensors in a single kernel launch
// =============================================================================
kernel void apply_rope_qk_fused(
    device float* query [[buffer(0)]],           // [batch, seq_len, num_heads, head_dim]
    device float* key [[buffer(1)]],             // [batch, seq_len, num_kv_heads, head_dim]
    constant float* cos_table [[buffer(2)]],     // [max_seq_len, head_dim/2]
    constant float* sin_table [[buffer(3)]],     // [max_seq_len, head_dim/2]
    device const uint* positions [[buffer(4)]],  // [batch, seq_len]
    constant uint& num_q_heads [[buffer(5)]],
    constant uint& num_kv_heads [[buffer(6)]],
    constant uint& head_dim [[buffer(7)]],
    constant uint& seq_len [[buffer(8)]],
    uint3 gid [[thread_position_in_grid]]
) {
    uint d = gid.x;                    // Dimension pair index
    uint head = gid.y;                 // Head index (for Q)
    uint seq_batch = gid.z;            // Combined seq + batch

    if (d >= head_dim / 2) {
        return;
    }

    uint batch = seq_batch / seq_len;
    uint seq_pos = seq_batch % seq_len;
    uint position = positions[batch * seq_len + seq_pos];

    uint table_offset = position * (head_dim / 2) + d;
    float cos_val = cos_table[table_offset];
    float sin_val = sin_table[table_offset];

    // Apply to query
    if (head < num_q_heads) {
        uint q_offset = ((batch * seq_len + seq_pos) * num_q_heads + head) * head_dim;
        float q0 = query[q_offset + 2 * d];
        float q1 = query[q_offset + 2 * d + 1];
        query[q_offset + 2 * d] = fma(q0, cos_val, -q1 * sin_val);
        query[q_offset + 2 * d + 1] = fma(q0, sin_val, q1 * cos_val);
    }

    // Apply to key (handle GQA with fewer KV heads)
    if (head < num_kv_heads) {
        uint k_offset = ((batch * seq_len + seq_pos) * num_kv_heads + head) * head_dim;
        float k0 = key[k_offset + 2 * d];
        float k1 = key[k_offset + 2 * d + 1];
        key[k_offset + 2 * d] = fma(k0, cos_val, -k1 * sin_val);
        key[k_offset + 2 * d + 1] = fma(k0, sin_val, k1 * cos_val);
    }
}
