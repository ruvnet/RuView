//
// Quantized Operations - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro
//
// Implements INT4/INT8 quantized operations:
// - INT4 GEMV (dequantize on-the-fly)
// - INT8 GEMM with accumulation
// - Mixed-precision operations (INT4 weights, FP16 activations)
// - Group-wise quantization support
//
// M4 Pro Optimizations:
// - SIMD reduction for fast dot products
// - Vectorized dequantization
// - Coalesced memory access for packed weights
// - 1024 threads per threadgroup
//

#include <metal_stdlib>
using namespace metal;

// ============================================================================
// Constants
// ============================================================================
constant uint SIMD_SIZE = 32;
constant uint INT4_PACK = 2;         // 2 INT4 values per byte
constant uint GROUP_SIZE = 128;      // Default quantization group size

// ============================================================================
// Quantization Parameters
// ============================================================================
struct QuantParams {
    uint n;              // Output dimension
    uint k;              // Input dimension
    uint group_size;     // Quantization group size (typically 32, 64, or 128)
    uint num_groups;     // k / group_size
    uint has_zeros;      // Whether zero-point quantization is used
    uint _padding[3];
};

struct Int4GemvParams {
    uint n;              // Number of output elements
    uint k;              // Number of input elements
    uint group_size;     // Quantization group size
    uint _padding;
};

// ============================================================================
// INT4 DEQUANTIZATION HELPERS
// Unpack 2 INT4 values from 1 byte and dequantize
// ============================================================================

// Unpack byte to two INT4 values (-8 to 7)
inline int2 unpack_int4(uint8_t packed) {
    int low = int(packed & 0x0F);
    int high = int((packed >> 4) & 0x0F);
    // Sign extend (4-bit to 32-bit)
    if (low >= 8) low -= 16;
    if (high >= 8) high -= 16;
    return int2(low, high);
}

// Unpack byte to two UINT4 values (0 to 15) - for asymmetric quantization
inline uint2 unpack_uint4(uint8_t packed) {
    return uint2(packed & 0x0F, (packed >> 4) & 0x0F);
}

// ============================================================================
// INT4 GEMV - Vector-Matrix Multiplication with INT4 Weights
// Computes: output = input @ W^T where W is INT4 quantized
// Dequantizes on-the-fly: w_fp = (w_int4 - zero) * scale
// ============================================================================
kernel void int4_gemv(
    device const uint8_t* weights_packed [[buffer(0)]],  // [n, k/2] packed INT4
    device const float* scales [[buffer(1)]],            // [n, num_groups]
    device const float* zeros [[buffer(2)]],             // [n, num_groups] or nullptr
    device const float* input [[buffer(3)]],             // [k]
    device float* output [[buffer(4)]],                  // [n]
    constant Int4GemvParams& params [[buffer(5)]],
    uint gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint row = gid;  // Output row index

    if (row >= params.n) return;

    const uint k = params.k;
    const uint group_size = params.group_size;
    const uint num_groups = (k + group_size - 1) / group_size;
    const uint k_packed = k / 2;  // k/2 bytes for INT4

    // Weight row start in packed format
    const uint w_row_offset = row * k_packed;
    const uint scale_row_offset = row * num_groups;

    float sum = 0.0f;

    // Process in groups for better cache locality
    for (uint g = 0; g < num_groups; g++) {
        const uint group_start = g * group_size;
        const uint group_end = min(group_start + group_size, k);
        const uint group_len = group_end - group_start;

        // Get scale and zero for this group
        float scale = scales[scale_row_offset + g];
        float zero = zeros ? zeros[scale_row_offset + g] : 0.0f;

        // Process 2 elements at a time (1 packed byte)
        const uint packed_start = group_start / 2;
        const uint packed_end = (group_end + 1) / 2;

        for (uint i = packed_start; i < packed_end; i++) {
            uint8_t packed = weights_packed[w_row_offset + i];
            int2 unpacked = unpack_int4(packed);

            // Element indices
            uint idx0 = i * 2;
            uint idx1 = i * 2 + 1;

            // Dequantize and accumulate
            if (idx0 >= group_start && idx0 < group_end) {
                float w0 = (float(unpacked.x) - zero) * scale;
                sum = fma(w0, input[idx0], sum);
            }
            if (idx1 >= group_start && idx1 < group_end) {
                float w1 = (float(unpacked.y) - zero) * scale;
                sum = fma(w1, input[idx1], sum);
            }
        }
    }

    output[row] = sum;
}

// ============================================================================
// INT4 GEMV VECTORIZED - Optimized with SIMD reductions
// Each threadgroup computes multiple output elements
// ============================================================================
kernel void int4_gemv_simd(
    device const uint8_t* weights_packed [[buffer(0)]],
    device const float* scales [[buffer(1)]],
    device const float* zeros [[buffer(2)]],
    device const float* input [[buffer(3)]],
    device float* output [[buffer(4)]],
    constant Int4GemvParams& params [[buffer(5)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint row = gid.y;
    if (row >= params.n) return;

    const uint k = params.k;
    const uint group_size = params.group_size;
    const uint num_groups = (k + group_size - 1) / group_size;
    const uint k_packed = k / 2;

    const uint w_row_offset = row * k_packed;
    const uint scale_row_offset = row * num_groups;

    // Each thread in the warp processes a subset of k
    float partial_sum = 0.0f;

    for (uint g = 0; g < num_groups; g++) {
        const uint group_start = g * group_size;
        const uint group_end = min(group_start + group_size, k);

        float scale = scales[scale_row_offset + g];
        float zero = zeros ? zeros[scale_row_offset + g] : 0.0f;

        // Distribute packed bytes across SIMD lanes
        const uint packed_start = group_start / 2;
        const uint packed_end = (group_end + 1) / 2;
        const uint packed_len = packed_end - packed_start;

        for (uint i = packed_start + simd_lane; i < packed_end; i += SIMD_SIZE) {
            uint8_t packed = weights_packed[w_row_offset + i];
            int2 unpacked = unpack_int4(packed);

            uint idx0 = i * 2;
            uint idx1 = i * 2 + 1;

            if (idx0 < k) {
                float w0 = (float(unpacked.x) - zero) * scale;
                partial_sum = fma(w0, input[idx0], partial_sum);
            }
            if (idx1 < k) {
                float w1 = (float(unpacked.y) - zero) * scale;
                partial_sum = fma(w1, input[idx1], partial_sum);
            }
        }
    }

    // SIMD reduction
    float sum = simd_sum(partial_sum);

    // First lane writes result
    if (simd_lane == 0) {
        output[row] = sum;
    }
}

// ============================================================================
// INT4 GEMM - Matrix-Matrix Multiplication with INT4 Weights
// Computes: C = A @ W^T where W is INT4 quantized
// A: [m, k] FP32/FP16
// W: [n, k] INT4 packed
// C: [m, n] FP32/FP16
// ============================================================================
kernel void int4_gemm(
    device const float* A [[buffer(0)]],                 // [m, k]
    device const uint8_t* W_packed [[buffer(1)]],        // [n, k/2] INT4
    device const float* scales [[buffer(2)]],            // [n, num_groups]
    device const float* zeros [[buffer(3)]],             // [n, num_groups]
    device float* C [[buffer(4)]],                       // [m, n]
    constant uint4& dims [[buffer(5)]],                  // (m, n, k, group_size)
    uint2 gid [[thread_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint m = dims.x;
    const uint n = dims.y;
    const uint k = dims.z;
    const uint group_size = dims.w;
    const uint num_groups = (k + group_size - 1) / group_size;
    const uint k_packed = k / 2;

    // Tile dimensions
    const uint TILE_M = 16;
    const uint TILE_N = 16;

    const uint tile_m = gid.y;
    const uint tile_n = gid.x;
    const uint local_m = tid.y;
    const uint local_n = tid.x;

    const uint row = tile_m * TILE_M + local_m;
    const uint col = tile_n * TILE_N + local_n;

    if (row >= m || col >= n) return;

    // Compute C[row, col] = sum_k A[row, k] * W[col, k]
    float sum = 0.0f;

    const uint w_row_offset = col * k_packed;
    const uint scale_row_offset = col * num_groups;

    for (uint g = 0; g < num_groups; g++) {
        const uint group_start = g * group_size;
        const uint group_end = min(group_start + group_size, k);

        float scale = scales[scale_row_offset + g];
        float zero = zeros ? zeros[scale_row_offset + g] : 0.0f;

        const uint packed_start = group_start / 2;
        const uint packed_end = (group_end + 1) / 2;

        for (uint i = packed_start; i < packed_end; i++) {
            uint8_t packed = W_packed[w_row_offset + i];
            int2 unpacked = unpack_int4(packed);

            uint idx0 = i * 2;
            uint idx1 = i * 2 + 1;

            if (idx0 < k) {
                float w0 = (float(unpacked.x) - zero) * scale;
                float a0 = A[row * k + idx0];
                sum = fma(w0, a0, sum);
            }
            if (idx1 < k) {
                float w1 = (float(unpacked.y) - zero) * scale;
                float a1 = A[row * k + idx1];
                sum = fma(w1, a1, sum);
            }
        }
    }

    C[row * n + col] = sum;
}

// ============================================================================
// INT8 GEMV - Vector-Matrix Multiplication with INT8 Weights
// Simpler than INT4, no unpacking needed
// ============================================================================
struct Int8GemvParams {
    uint n;              // Output dimension
    uint k;              // Input dimension
    float scale;         // Global scale factor
    float zero;          // Global zero point
};

kernel void int8_gemv(
    device const int8_t* weights [[buffer(0)]],          // [n, k]
    device const float* input [[buffer(1)]],             // [k]
    device float* output [[buffer(2)]],                  // [n]
    constant Int8GemvParams& params [[buffer(3)]],
    uint gid [[thread_position_in_grid]],
    uint simd_lane [[thread_index_in_simdgroup]]
) {
    const uint row = gid;
    if (row >= params.n) return;

    const uint k = params.k;
    const float scale = params.scale;
    const float zero = params.zero;

    float sum = 0.0f;
    const uint w_offset = row * k;

    // Vectorized processing
    for (uint i = simd_lane; i < k; i += SIMD_SIZE) {
        float w = (float(weights[w_offset + i]) - zero) * scale;
        sum = fma(w, input[i], sum);
    }

    // SIMD reduction
    sum = simd_sum(sum);

    if (simd_lane == 0) {
        output[row] = sum;
    }
}

// ============================================================================
// QUANTIZE FP32 -> INT4
// Produces packed INT4 weights with per-group scales and zeros
// ============================================================================
struct QuantizeParams {
    uint n;              // Number of rows
    uint k;              // Number of columns
    uint group_size;     // Quantization group size
};

kernel void quantize_fp32_to_int4(
    device const float* input [[buffer(0)]],             // [n, k]
    device uint8_t* output_packed [[buffer(1)]],         // [n, k/2]
    device float* scales [[buffer(2)]],                  // [n, num_groups]
    device float* zeros [[buffer(3)]],                   // [n, num_groups]
    constant QuantizeParams& params [[buffer(4)]],
    uint2 gid [[thread_position_in_grid]]
) {
    const uint row = gid.y;
    const uint group = gid.x;

    if (row >= params.n) return;

    const uint group_size = params.group_size;
    const uint num_groups = (params.k + group_size - 1) / group_size;
    if (group >= num_groups) return;

    const uint group_start = group * group_size;
    const uint group_end = min(group_start + group_size, params.k);
    const uint input_offset = row * params.k;

    // Find min and max in this group
    float min_val = INFINITY;
    float max_val = -INFINITY;

    for (uint i = group_start; i < group_end; i++) {
        float val = input[input_offset + i];
        min_val = min(min_val, val);
        max_val = max(max_val, val);
    }

    // Compute scale and zero point for symmetric quantization to [-8, 7]
    float abs_max = max(abs(min_val), abs(max_val));
    float scale = abs_max / 7.0f;
    float inv_scale = (scale > 0.0f) ? (1.0f / scale) : 0.0f;
    float zero = 0.0f;  // Symmetric quantization

    // Store scale and zero
    uint scale_offset = row * num_groups + group;
    scales[scale_offset] = scale;
    zeros[scale_offset] = zero;

    // Quantize and pack
    const uint packed_start = group_start / 2;
    const uint packed_end = (group_end + 1) / 2;
    const uint output_offset = row * (params.k / 2);

    for (uint i = packed_start; i < packed_end; i++) {
        uint idx0 = i * 2;
        uint idx1 = i * 2 + 1;

        int q0 = 0, q1 = 0;

        if (idx0 < params.k && idx0 >= group_start && idx0 < group_end) {
            float val = input[input_offset + idx0];
            q0 = int(clamp(round(val * inv_scale), -8.0f, 7.0f));
        }
        if (idx1 < params.k && idx1 >= group_start && idx1 < group_end) {
            float val = input[input_offset + idx1];
            q1 = int(clamp(round(val * inv_scale), -8.0f, 7.0f));
        }

        // Pack two INT4 values into one byte
        // Convert from signed to unsigned representation
        uint8_t packed = uint8_t((q0 & 0x0F) | ((q1 & 0x0F) << 4));
        output_packed[output_offset + i] = packed;
    }
}

// ============================================================================
// DEQUANTIZE INT4 -> FP32
// For verification and debugging
// ============================================================================
kernel void dequantize_int4_to_fp32(
    device const uint8_t* input_packed [[buffer(0)]],    // [n, k/2]
    device const float* scales [[buffer(1)]],            // [n, num_groups]
    device const float* zeros [[buffer(2)]],             // [n, num_groups]
    device float* output [[buffer(3)]],                  // [n, k]
    constant QuantizeParams& params [[buffer(4)]],
    uint2 gid [[thread_position_in_grid]]
) {
    const uint row = gid.y;
    const uint col = gid.x;

    if (row >= params.n || col >= params.k) return;

    const uint group = col / params.group_size;
    const uint num_groups = (params.k + params.group_size - 1) / params.group_size;

    float scale = scales[row * num_groups + group];
    float zero = zeros ? zeros[row * num_groups + group] : 0.0f;

    const uint packed_idx = row * (params.k / 2) + col / 2;
    uint8_t packed = input_packed[packed_idx];

    int q;
    if (col % 2 == 0) {
        q = int(packed & 0x0F);
    } else {
        q = int((packed >> 4) & 0x0F);
    }
    // Sign extend
    if (q >= 8) q -= 16;

    output[row * params.k + col] = (float(q) - zero) * scale;
}

// ============================================================================
// FP16 TO INT4 CONVERSION
// Direct FP16 quantization for faster inference
// ============================================================================
kernel void quantize_fp16_to_int4(
    device const half* input [[buffer(0)]],
    device uint8_t* output_packed [[buffer(1)]],
    device half* scales [[buffer(2)]],
    device half* zeros [[buffer(3)]],
    constant QuantizeParams& params [[buffer(4)]],
    uint2 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]],
    uint threads_per_group [[threads_per_threadgroup]]
) {
    const uint row = gid.y;
    const uint group = gid.x;

    if (row >= params.n) return;

    const uint group_size = params.group_size;
    const uint num_groups = (params.k + group_size - 1) / group_size;
    if (group >= num_groups) return;

    const uint group_start = group * group_size;
    const uint group_end = min(group_start + group_size, params.k);
    const uint input_offset = row * params.k;

    // Find min/max using FP32 for accuracy
    float min_val = INFINITY;
    float max_val = -INFINITY;

    for (uint i = group_start + tid; i < group_end; i += threads_per_group) {
        float val = float(input[input_offset + i]);
        min_val = min(min_val, val);
        max_val = max(max_val, val);
    }

    // Warp reduction
    min_val = simd_min(min_val);
    max_val = simd_max(max_val);

    // First thread computes and stores scale/zero
    if (tid == 0) {
        float abs_max = max(abs(min_val), abs(max_val));
        float scale = abs_max / 7.0f;

        uint scale_offset = row * num_groups + group;
        scales[scale_offset] = half(scale);
        zeros[scale_offset] = half(0.0f);
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // All threads quantize
    float scale = float(scales[row * num_groups + group]);
    float inv_scale = (scale > 0.0f) ? (1.0f / scale) : 0.0f;

    const uint output_offset = row * (params.k / 2);

    for (uint i = group_start / 2 + tid; i < (group_end + 1) / 2; i += threads_per_group) {
        uint idx0 = i * 2;
        uint idx1 = i * 2 + 1;

        int q0 = 0, q1 = 0;

        if (idx0 < params.k && idx0 >= group_start && idx0 < group_end) {
            float val = float(input[input_offset + idx0]);
            q0 = int(clamp(round(val * inv_scale), -8.0f, 7.0f));
        }
        if (idx1 < params.k && idx1 >= group_start && idx1 < group_end) {
            float val = float(input[input_offset + idx1]);
            q1 = int(clamp(round(val * inv_scale), -8.0f, 7.0f));
        }

        uint8_t packed = uint8_t((q0 & 0x0F) | ((q1 & 0x0F) << 4));
        output_packed[output_offset + i] = packed;
    }
}
