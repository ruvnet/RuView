//
// GEMV (General Matrix-Vector Multiplication) - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro with simdgroup operations
//
// Computes y = A * x where A is (m x n), x is (n), y is (m)
// Target: 100+ GFLOPS on M4 Pro GPU (vs ~35 GFLOPS CPU)
//
// Optimizations:
// - Simdgroup reduction for efficient parallel reduction
// - Tiled memory access for optimal bandwidth
// - FP16 compute path for 2x throughput
// - Vectorized loads (float4/half4) for coalesced access
// - Optimal threadgroup memory layout for 16KB L1
//
// M4 Pro Specifications:
// - 16KB L1 data cache per core
// - 192KB L2 per core cluster
// - 32-wide SIMD groups
// - 1024 threads per threadgroup max
// - ~3 TFLOPS FP16 compute
//

#include <metal_stdlib>
using namespace metal;

// ============================================================================
// M4 Pro Tuned Constants for GEMV
// ============================================================================

// Threads per output row - optimal for M4 Pro SIMD width
constant uint GEMV_THREADS_PER_ROW = 32;

// Number of rows processed per threadgroup
constant uint GEMV_ROWS_PER_BLOCK = 4;

// Vector elements processed per thread per iteration
constant uint GEMV_ELEMENTS_PER_THREAD = 8;

// Block size for K dimension tiling (fits in threadgroup memory)
constant uint GEMV_K_BLOCK = 256;

// =============================================================================
// GEMV Parameters Structure (matches Rust GemvParams)
// =============================================================================
struct GemvParams {
    uint m;      // Rows of A (output dimension)
    uint n;      // Columns of A (input dimension)
    uint lda;    // Leading dimension of A
    float alpha; // Scale factor (default 1.0)
    float beta;  // Output scale factor (default 0.0, for y = alpha*A*x + beta*y)
};

// =============================================================================
// High-Performance FP32 GEMV with simdgroup reduction
// Grid: (tiles_m, 1, 1) where tiles_m = ceil(m / GEMV_ROWS_PER_BLOCK)
// Threadgroup: (GEMV_THREADS_PER_ROW, GEMV_ROWS_PER_BLOCK, 1) = (32, 4, 1) = 128 threads
// Target: 100+ GFLOPS on M4 Pro GPU
// =============================================================================
kernel void gemv_optimized_f32(
    device const float* A [[buffer(0)]],      // Matrix (m x n)
    device const float* x [[buffer(1)]],      // Input vector (n)
    device float* y [[buffer(2)]],            // Output vector (m)
    constant GemvParams& params [[buffer(3)]],
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint row_base = gid.x * GEMV_ROWS_PER_BLOCK;
    const uint local_row = tid.y;
    const uint row = row_base + local_row;

    // Early exit if row out of bounds
    if (row >= params.m) return;

    // Each thread in the row processes a portion of the dot product
    const uint lane = tid.x;  // 0-31
    const uint n = params.n;
    const uint lda = params.lda;

    // Accumulator for this thread's partial sum
    float sum = 0.0f;

    // Get row pointer
    device const float* a_row = A + row * lda;

    // Process elements in vectorized chunks of 4
    const uint vec_start = lane * 4;
    const uint vec_stride = GEMV_THREADS_PER_ROW * 4;  // 128 elements per iteration

    uint col = vec_start;
    while (col + 4 <= n) {
        // Vectorized load from A and x
        float4 a_val = *reinterpret_cast<device const float4*>(a_row + col);
        float4 x_val = *reinterpret_cast<device const float4*>(x + col);

        // Fused multiply-add
        sum = fma(a_val.x, x_val.x, sum);
        sum = fma(a_val.y, x_val.y, sum);
        sum = fma(a_val.z, x_val.z, sum);
        sum = fma(a_val.w, x_val.w, sum);

        col += vec_stride;
    }

    // Handle remaining elements (scalar)
    col = (n / vec_stride) * vec_stride + lane;
    while (col < n) {
        sum = fma(a_row[col], x[col], sum);
        col += GEMV_THREADS_PER_ROW;
    }

    // Simdgroup reduction across all 32 lanes
    // M4 Pro has efficient simd_sum for warp-level reduction
    float row_sum = simd_sum(sum);

    // Lane 0 writes the final result
    if (lane == 0) {
        if (params.beta != 0.0f) {
            y[row] = params.alpha * row_sum + params.beta * y[row];
        } else {
            y[row] = params.alpha * row_sum;
        }
    }
}

// =============================================================================
// High-Performance FP16 GEMV with simdgroup reduction
// Achieves 2x throughput vs FP32 on M4 Pro's FP16 units
// Target: 200+ GFLOPS theoretical on M4 Pro GPU
// =============================================================================
kernel void gemv_optimized_f16(
    device const half* A [[buffer(0)]],       // Matrix (m x n) in FP16
    device const half* x [[buffer(1)]],       // Input vector (n) in FP16
    device half* y [[buffer(2)]],             // Output vector (m) in FP16
    constant GemvParams& params [[buffer(3)]],
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint row_base = gid.x * GEMV_ROWS_PER_BLOCK;
    const uint local_row = tid.y;
    const uint row = row_base + local_row;

    if (row >= params.m) return;

    const uint lane = tid.x;
    const uint n = params.n;
    const uint lda = params.lda;

    // Use FP32 accumulator for precision, FP16 for memory bandwidth
    float sum = 0.0f;

    device const half* a_row = A + row * lda;

    // Process elements in vectorized chunks of 8 (half4 * 2)
    const uint vec_start = lane * 8;
    const uint vec_stride = GEMV_THREADS_PER_ROW * 8;  // 256 elements per iteration

    uint col = vec_start;
    while (col + 8 <= n) {
        // Vectorized load from A and x (half4 for optimal bandwidth)
        half4 a_val0 = *reinterpret_cast<device const half4*>(a_row + col);
        half4 a_val1 = *reinterpret_cast<device const half4*>(a_row + col + 4);
        half4 x_val0 = *reinterpret_cast<device const half4*>(x + col);
        half4 x_val1 = *reinterpret_cast<device const half4*>(x + col + 4);

        // Accumulate in FP32 for precision
        sum = fma(float(a_val0.x), float(x_val0.x), sum);
        sum = fma(float(a_val0.y), float(x_val0.y), sum);
        sum = fma(float(a_val0.z), float(x_val0.z), sum);
        sum = fma(float(a_val0.w), float(x_val0.w), sum);
        sum = fma(float(a_val1.x), float(x_val1.x), sum);
        sum = fma(float(a_val1.y), float(x_val1.y), sum);
        sum = fma(float(a_val1.z), float(x_val1.z), sum);
        sum = fma(float(a_val1.w), float(x_val1.w), sum);

        col += vec_stride;
    }

    // Handle remaining chunks of 4
    while (col + 4 <= n) {
        half4 a_val = *reinterpret_cast<device const half4*>(a_row + col);
        half4 x_val = *reinterpret_cast<device const half4*>(x + col);

        sum = fma(float(a_val.x), float(x_val.x), sum);
        sum = fma(float(a_val.y), float(x_val.y), sum);
        sum = fma(float(a_val.z), float(x_val.z), sum);
        sum = fma(float(a_val.w), float(x_val.w), sum);

        col += GEMV_THREADS_PER_ROW * 4;
    }

    // Handle remaining scalar elements
    col = (n / 4) * 4 + lane;
    while (col < n) {
        sum = fma(float(a_row[col]), float(x[col]), sum);
        col += GEMV_THREADS_PER_ROW;
    }

    // Simdgroup reduction
    float row_sum = simd_sum(sum);

    // Lane 0 writes result
    if (lane == 0) {
        half alpha_h = half(params.alpha);
        if (params.beta != 0.0f) {
            half beta_h = half(params.beta);
            y[row] = alpha_h * half(row_sum) + beta_h * y[row];
        } else {
            y[row] = alpha_h * half(row_sum);
        }
    }
}

// =============================================================================
// Batched GEMV for multi-head attention (FP32)
// Each batch is an independent GEMV: y[b] = A[b] * x[b]
// Grid: (tiles_m, batch_size, 1)
// =============================================================================
kernel void batched_gemv_f32(
    device const float* A [[buffer(0)]],      // Batched matrices (batch, m, n)
    device const float* x [[buffer(1)]],      // Batched vectors (batch, n)
    device float* y [[buffer(2)]],            // Output (batch, m)
    constant uint4& dims [[buffer(3)]],       // (m, n, batch_size, 0)
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]]
) {
    const uint m = dims.x;
    const uint n = dims.y;
    const uint batch = gid.y;

    if (batch >= dims.z) return;

    const uint row_base = gid.x * GEMV_ROWS_PER_BLOCK;
    const uint local_row = tid.y;
    const uint row = row_base + local_row;

    if (row >= m) return;

    const uint lane = tid.x;

    // Get batch offsets
    device const float* a_batch = A + batch * m * n;
    device const float* x_batch = x + batch * n;
    device float* y_batch = y + batch * m;

    device const float* a_row = a_batch + row * n;

    float sum = 0.0f;

    // Vectorized processing
    uint col = lane * 4;
    const uint vec_stride = GEMV_THREADS_PER_ROW * 4;

    while (col + 4 <= n) {
        float4 a_val = *reinterpret_cast<device const float4*>(a_row + col);
        float4 x_val = *reinterpret_cast<device const float4*>(x_batch + col);

        sum = fma(a_val.x, x_val.x, sum);
        sum = fma(a_val.y, x_val.y, sum);
        sum = fma(a_val.z, x_val.z, sum);
        sum = fma(a_val.w, x_val.w, sum);

        col += vec_stride;
    }

    // Remaining elements
    for (uint c = (n / 4) * 4 + lane; c < n; c += GEMV_THREADS_PER_ROW) {
        sum = fma(a_row[c], x_batch[c], sum);
    }

    // Simdgroup reduction
    float row_sum = simd_sum(sum);

    if (lane == 0) {
        y_batch[row] = row_sum;
    }
}

// =============================================================================
// Tiled GEMV with shared memory for larger K dimensions
// Uses threadgroup memory to cache x vector for reuse across rows
// Grid: (tiles_m, 1, 1)
// Threadgroup: (256, 1, 1) - 8 rows * 32 threads
// =============================================================================
constant uint GEMV_TILED_ROWS = 8;
constant uint GEMV_TILED_THREADS = 32;
constant uint GEMV_TILED_K_BLOCK = 512;

kernel void gemv_tiled_f32(
    device const float* A [[buffer(0)]],
    device const float* x [[buffer(1)]],
    device float* y [[buffer(2)]],
    constant GemvParams& params [[buffer(3)]],
    uint gid [[threadgroup_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    // Shared memory for x vector tile
    threadgroup float shared_x[GEMV_TILED_K_BLOCK];

    const uint row = gid * GEMV_TILED_ROWS + simd_group;
    if (row >= params.m) return;

    const uint lane = simd_lane;
    const uint n = params.n;
    const uint lda = params.lda;

    device const float* a_row = A + row * lda;

    float sum = 0.0f;

    // Process K in blocks to maximize cache reuse
    for (uint k_block = 0; k_block < n; k_block += GEMV_TILED_K_BLOCK) {
        // Cooperative load of x into shared memory
        const uint load_start = tid;
        const uint block_size = min(GEMV_TILED_K_BLOCK, n - k_block);

        for (uint i = load_start; i < block_size; i += GEMV_TILED_ROWS * GEMV_TILED_THREADS) {
            shared_x[i] = x[k_block + i];
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Process block with vectorization
        uint col = lane * 4;
        while (col + 4 <= block_size) {
            float4 a_val = *reinterpret_cast<device const float4*>(a_row + k_block + col);
            float4 x_val = *reinterpret_cast<threadgroup const float4*>(shared_x + col);

            sum = fma(a_val.x, x_val.x, sum);
            sum = fma(a_val.y, x_val.y, sum);
            sum = fma(a_val.z, x_val.z, sum);
            sum = fma(a_val.w, x_val.w, sum);

            col += GEMV_TILED_THREADS * 4;
        }

        // Remaining elements
        for (uint c = col; c < block_size; c += GEMV_TILED_THREADS) {
            sum = fma(a_row[k_block + c], shared_x[c], sum);
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Simdgroup reduction
    float row_sum = simd_sum(sum);

    if (lane == 0) {
        if (params.beta != 0.0f) {
            y[row] = params.alpha * row_sum + params.beta * y[row];
        } else {
            y[row] = params.alpha * row_sum;
        }
    }
}

// =============================================================================
// Simple GEMV for compatibility (single thread per row)
// Grid: (m, 1, 1)
// =============================================================================
kernel void gemv_simple_f32(
    device const float* A [[buffer(0)]],
    device const float* x [[buffer(1)]],
    device float* y [[buffer(2)]],
    constant GemvParams& params [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= params.m) return;

    device const float* a_row = A + gid * params.lda;

    float sum = 0.0f;

    // Vectorized loop
    const uint n_vec = params.n / 4;
    for (uint i = 0; i < n_vec; i++) {
        float4 a_val = *reinterpret_cast<device const float4*>(a_row + i * 4);
        float4 x_val = *reinterpret_cast<device const float4*>(x + i * 4);
        sum = fma(a_val.x, x_val.x, sum);
        sum = fma(a_val.y, x_val.y, sum);
        sum = fma(a_val.z, x_val.z, sum);
        sum = fma(a_val.w, x_val.w, sum);
    }

    // Remainder
    for (uint i = n_vec * 4; i < params.n; i++) {
        sum = fma(a_row[i], x[i], sum);
    }

    if (params.beta != 0.0f) {
        y[gid] = params.alpha * sum + params.beta * y[gid];
    } else {
        y[gid] = params.alpha * sum;
    }
}

// =============================================================================
// Mixed precision GEMV: FP16 matrix, FP32 vector -> FP32 output
// For inference with quantized weights
// =============================================================================
kernel void gemv_mixed_f16_f32(
    device const half* A [[buffer(0)]],       // Matrix in FP16
    device const float* x [[buffer(1)]],      // Vector in FP32
    device float* y [[buffer(2)]],            // Output in FP32
    constant GemvParams& params [[buffer(3)]],
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]]
) {
    const uint row_base = gid.x * GEMV_ROWS_PER_BLOCK;
    const uint local_row = tid.y;
    const uint row = row_base + local_row;

    if (row >= params.m) return;

    const uint lane = tid.x;
    const uint n = params.n;
    const uint lda = params.lda;

    float sum = 0.0f;

    device const half* a_row = A + row * lda;

    // Process in chunks of 8 (half4 * 2)
    uint col = lane * 8;
    const uint vec_stride = GEMV_THREADS_PER_ROW * 8;

    while (col + 8 <= n) {
        half4 a_val0 = *reinterpret_cast<device const half4*>(a_row + col);
        half4 a_val1 = *reinterpret_cast<device const half4*>(a_row + col + 4);
        float4 x_val0 = *reinterpret_cast<device const float4*>(x + col);
        float4 x_val1 = *reinterpret_cast<device const float4*>(x + col + 4);

        sum = fma(float(a_val0.x), x_val0.x, sum);
        sum = fma(float(a_val0.y), x_val0.y, sum);
        sum = fma(float(a_val0.z), x_val0.z, sum);
        sum = fma(float(a_val0.w), x_val0.w, sum);
        sum = fma(float(a_val1.x), x_val1.x, sum);
        sum = fma(float(a_val1.y), x_val1.y, sum);
        sum = fma(float(a_val1.z), x_val1.z, sum);
        sum = fma(float(a_val1.w), x_val1.w, sum);

        col += vec_stride;
    }

    // Remaining elements
    for (uint c = (n / 8) * 8 + lane; c < n; c += GEMV_THREADS_PER_ROW) {
        sum = fma(float(a_row[c]), x[c], sum);
    }

    // Simdgroup reduction
    float row_sum = simd_sum(sum);

    if (lane == 0) {
        if (params.beta != 0.0f) {
            y[row] = params.alpha * row_sum + params.beta * y[row];
        } else {
            y[row] = params.alpha * row_sum;
        }
    }
}
