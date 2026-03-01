//
// GEMM (General Matrix Multiplication) - Metal Compute Shader
// Optimized for Apple Silicon M4 Pro with simdgroup_matrix_multiply_accumulate
//
// Computes C = alpha * A @ B + beta * C
// Target: 2+ TFLOPS on M4 Pro GPU
//
// Optimizations:
// - simdgroup_matrix_multiply_accumulate for 8x8 tiles
// - 128x128 output tiles with triple-buffered loading (M4 Pro tuned)
// - Bank conflict-free shared memory with padding
// - Software pipelining for latency hiding
// - Vectorized memory access (float4/half4)
// - Optimal threadgroup memory layout for 16KB L1, 192KB L2
//
// M4 Pro Specifications:
// - 16KB L1 data cache per core
// - 192KB L2 per core cluster
// - 32-wide SIMD groups
// - 1024 threads per threadgroup max
//

#include <metal_stdlib>
#include <metal_simdgroup_matrix>
using namespace metal;

// ============================================================================
// M4 Pro Tuned Constants (BM=64, BN=64, BK=32)
// ============================================================================
// SECURITY FIX: Reduced tile sizes to stay within 32KB threadgroup memory limit
// Previous BM=128,BN=128 with NUM_BUFFERS=3 used ~57KB (exceeds 32KB limit)
// New: BM=64,BN=64 with NUM_BUFFERS=2:
//   shared_a: 2 * 64 * 40 * 2 = 10,240 bytes
//   shared_b: 2 * 32 * 72 * 2 = 9,216 bytes
//   Total: ~19KB < 32KB limit
constant uint BM = 64;              // Output tile rows (reduced for memory safety)
constant uint BN = 64;              // Output tile columns (reduced for memory safety)
constant uint BK = 32;              // Reduction tile size
constant uint SIMD_TILE = 8;        // simdgroup_matrix dimension
constant uint SIMD_SIZE = 32;       // SIMD group size
constant uint WARPS_PER_BLOCK = 4;  // 256 threads (for 64x64 tiles)
constant uint NUM_BUFFERS = 2;      // Double buffering (reduced from 3 for memory safety)

// Legacy tile sizes for compatibility
constant uint TILE_M = 32;
constant uint TILE_N = 32;
constant uint TILE_K = 32;

// GEMM parameters structure (matches Rust GemmParams)
struct GemmParams {
    uint m;      // Rows of A and C
    uint n;      // Columns of B and C
    uint k;      // Columns of A, rows of B
    uint lda;    // Leading dimension of A
    uint ldb;    // Leading dimension of B
    uint ldc;    // Leading dimension of C
    float alpha; // Scale factor for A @ B
    float beta;  // Scale factor for C
};

// =============================================================================
// M4 PRO OPTIMIZED: High-Performance FP16 GEMM (BM=64, BN=64, BK=32)
// Grid: (tiles_n, tiles_m, 1) where tiles_x = ceil(x / BM or BN)
// Threadgroup: 256 threads (16x16 configuration) - reduced for memory safety
// Target: 1.5+ TFLOPS (reduced from 2+ due to smaller tiles for security)
// SECURITY: Uses only 19KB of 32KB threadgroup memory limit
// =============================================================================
kernel void gemm_optimized(
    device const half* A [[buffer(0)]],
    device const half* B [[buffer(1)]],
    device half* C [[buffer(2)]],
    constant GemmParams& params [[buffer(3)]],
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    // 128x128 tile coordinates
    const uint tile_m = gid.y;
    const uint tile_n = gid.x;
    const uint m_start = tile_m * BM;
    const uint n_start = tile_n * BN;

    if (m_start >= params.m || n_start >= params.n) return;

    // Bank conflict-free shared memory with padding (+8 for 128-bit alignment)
    // Memory usage: 2 * 64 * 40 * 2 + 2 * 32 * 72 * 2 = 10,240 + 9,216 = 19,456 bytes < 32KB
    threadgroup half shared_a[NUM_BUFFERS][BM][BK + 8] __attribute__((aligned(16)));
    threadgroup half shared_b[NUM_BUFFERS][BK][BN + 8] __attribute__((aligned(16)));

    // Each warp computes a 16x16 subblock using 2x2 grid of 8x8 simdgroup_matrix ops
    // 4 warps cover 2x2 = 64x64 tile (reduced from 128x128)
    const uint warp_id = simd_group;
    const uint warp_m = (warp_id / 2) * 32;   // 0, 32 (for 64x64 tile)
    const uint warp_n = (warp_id % 2) * 32;   // 0, 32 (for 64x64 tile)

    // 4x4 accumulator grid per warp (32x32 output per warp using 8x8 tiles)
    simdgroup_half8x8 c_frag[4][4];
    #pragma unroll
    for (uint i = 0; i < 4; i++) {
        #pragma unroll
        for (uint j = 0; j < 4; j++) {
            c_frag[i][j] = simdgroup_half8x8(0.0h);
        }
    }

    const uint num_k_tiles = (params.k + BK - 1) / BK;
    uint buffer_idx = 0;

    // Cooperative load helpers
    const uint thread_id = tid.y * 32 + tid.x;
    const uint total_threads = 1024;

    // Preload first tile (software pipelining stage 0)
    {
        // Load A tile [BM x BK] = 128x32 = 4096 elements
        // 1024 threads: each loads 4 elements
        #pragma unroll 4
        for (uint i = thread_id; i < BM * BK; i += total_threads) {
            const uint r = i / BK;
            const uint c = i % BK;
            const uint a_row = m_start + r;
            const uint a_col = c;
            shared_a[0][r][c] = (a_row < params.m && a_col < params.k)
                ? A[a_row * params.lda + a_col] : half(0.0h);
        }

        // Load B tile [BK x BN] = 32x128 = 4096 elements
        #pragma unroll 4
        for (uint i = thread_id; i < BK * BN; i += total_threads) {
            const uint r = i / BN;
            const uint c = i % BN;
            const uint b_row = r;
            const uint b_col = n_start + c;
            shared_b[0][r][c] = (b_row < params.k && b_col < params.n)
                ? B[b_row * params.ldb + b_col] : half(0.0h);
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Main loop with triple-buffered software pipelining
    for (uint k_tile = 0; k_tile < num_k_tiles; k_tile++) {
        const uint next_buffer = (buffer_idx + 1) % NUM_BUFFERS;
        const uint k_start_next = (k_tile + 1) * BK;

        // Prefetch next tile while computing current (async-like pattern)
        if (k_tile + 1 < num_k_tiles) {
            #pragma unroll 4
            for (uint i = thread_id; i < BM * BK; i += total_threads) {
                const uint r = i / BK;
                const uint c = i % BK;
                const uint a_row = m_start + r;
                const uint a_col = k_start_next + c;
                shared_a[next_buffer][r][c] = (a_row < params.m && a_col < params.k)
                    ? A[a_row * params.lda + a_col] : half(0.0h);
            }

            #pragma unroll 4
            for (uint i = thread_id; i < BK * BN; i += total_threads) {
                const uint r = i / BN;
                const uint c = i % BN;
                const uint b_row = k_start_next + r;
                const uint b_col = n_start + c;
                shared_b[next_buffer][r][c] = (b_row < params.k && b_col < params.n)
                    ? B[b_row * params.ldb + b_col] : half(0.0h);
            }
        }

        // Compute 32x32 per warp using 4x4 simdgroup_matrix ops
        #pragma unroll 4
        for (uint k = 0; k < BK; k += SIMD_TILE) {
            // Load 4 A fragments (8x8 each) for this warp's rows
            simdgroup_half8x8 a_frag[4];
            #pragma unroll
            for (uint i = 0; i < 4; i++) {
                simdgroup_load(a_frag[i], &shared_a[buffer_idx][warp_m + i * 8][k], BK + 8);
            }

            // Load 4 B fragments (8x8 each) for this warp's columns
            simdgroup_half8x8 b_frag[4];
            #pragma unroll
            for (uint j = 0; j < 4; j++) {
                simdgroup_load(b_frag[j], &shared_b[buffer_idx][k][warp_n + j * 8], BN + 8);
            }

            // 4x4 multiply-accumulate
            #pragma unroll
            for (uint i = 0; i < 4; i++) {
                #pragma unroll
                for (uint j = 0; j < 4; j++) {
                    simdgroup_multiply_accumulate(c_frag[i][j], a_frag[i], b_frag[j], c_frag[i][j]);
                }
            }
        }

        buffer_idx = next_buffer;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Store results with alpha/beta scaling
    const half alpha_h = half(params.alpha);
    const half beta_h = half(params.beta);

    // Write 32x32 result per warp (4x4 grid of 8x8)
    #pragma unroll
    for (uint i = 0; i < 4; i++) {
        #pragma unroll
        for (uint j = 0; j < 4; j++) {
            const uint out_row_base = m_start + warp_m + i * 8;
            const uint out_col_base = n_start + warp_n + j * 8;

            // Store 8x8 tile
            #pragma unroll
            for (uint r = 0; r < 8; r++) {
                #pragma unroll
                for (uint c = 0; c < 8; c++) {
                    const uint out_row = out_row_base + r;
                    const uint out_col = out_col_base + c;
                    if (out_row < params.m && out_col < params.n) {
                        const uint idx = out_row * params.ldc + out_col;
                        if (beta_h == half(0.0h)) {
                            C[idx] = alpha_h * c_frag[i][j][r][c];
                        } else {
                            C[idx] = alpha_h * c_frag[i][j][r][c] + beta_h * C[idx];
                        }
                    }
                }
            }
        }
    }
}

// =============================================================================
// High-Performance FP16 GEMM with simdgroup_matrix_multiply_accumulate
// Grid: (tiles_n, tiles_m, 1) where tiles_x = ceil(x / TILE_x)
// Threadgroup: (SIMD_SIZE, 4, 1) - 4 simd groups per tile
// =============================================================================
kernel void gemm_f16_v2(
    device const half* A [[buffer(0)]],
    device const half* B [[buffer(1)]],
    device half* C [[buffer(2)]],
    constant GemmParams& params [[buffer(3)]],
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    // Tile coordinates in output matrix
    const uint tile_m = gid.y;
    const uint tile_n = gid.x;

    // Check bounds at tile level
    const uint m_start = tile_m * TILE_M;
    const uint n_start = tile_n * TILE_N;
    if (m_start >= params.m || n_start >= params.n) return;

    // Double-buffered shared memory (16-byte aligned)
    threadgroup half shared_a[NUM_BUFFERS][TILE_M][TILE_K + 4] __attribute__((aligned(16)));
    threadgroup half shared_b[NUM_BUFFERS][TILE_K][TILE_N + 4] __attribute__((aligned(16)));

    // Each simd group computes an 8x8 portion of the 32x32 tile
    // With 4 simd groups: simd0=(0,0), simd1=(0,1), simd2=(1,0), simd3=(1,1)
    const uint simd_m = (simd_group / 2) * 16;  // 0 or 16
    const uint simd_n = (simd_group % 2) * 16;  // 0 or 16

    // Accumulator matrices (2x2 grid of 8x8 tiles per simd group = 16x16)
    simdgroup_half8x8 c_frag[2][2];
    for (uint i = 0; i < 2; i++) {
        for (uint j = 0; j < 2; j++) {
            c_frag[i][j] = simdgroup_half8x8(0.0h);
        }
    }

    const uint num_k_tiles = (params.k + TILE_K - 1) / TILE_K;
    uint buffer_idx = 0;

    // Preload first tile into buffer 0
    {
        const uint k_start = 0;
        const uint load_row = tid.y;
        const uint load_col = simd_lane;

        // Load A tile [TILE_M x TILE_K]
        for (uint r = load_row; r < TILE_M; r += 4) {
            const uint a_row = m_start + r;
            for (uint c = load_col; c < TILE_K; c += SIMD_SIZE) {
                const uint a_col = k_start + c;
                half val = (a_row < params.m && a_col < params.k)
                    ? A[a_row * params.lda + a_col] : half(0.0h);
                shared_a[0][r][c] = val;
            }
        }

        // Load B tile [TILE_K x TILE_N]
        for (uint r = load_row; r < TILE_K; r += 4) {
            const uint b_row = k_start + r;
            for (uint c = load_col; c < TILE_N; c += SIMD_SIZE) {
                const uint b_col = n_start + c;
                half val = (b_row < params.k && b_col < params.n)
                    ? B[b_row * params.ldb + b_col] : half(0.0h);
                shared_b[0][r][c] = val;
            }
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Main loop with double-buffering
    for (uint k_tile = 0; k_tile < num_k_tiles; k_tile++) {
        const uint next_buffer = 1 - buffer_idx;
        const uint k_start_next = (k_tile + 1) * TILE_K;

        // Prefetch next tile while computing current
        if (k_tile + 1 < num_k_tiles) {
            const uint load_row = tid.y;
            const uint load_col = simd_lane;

            for (uint r = load_row; r < TILE_M; r += 4) {
                const uint a_row = m_start + r;
                for (uint c = load_col; c < TILE_K; c += SIMD_SIZE) {
                    const uint a_col = k_start_next + c;
                    half val = (a_row < params.m && a_col < params.k)
                        ? A[a_row * params.lda + a_col] : half(0.0h);
                    shared_a[next_buffer][r][c] = val;
                }
            }

            for (uint r = load_row; r < TILE_K; r += 4) {
                const uint b_row = k_start_next + r;
                for (uint c = load_col; c < TILE_N; c += SIMD_SIZE) {
                    const uint b_col = n_start + c;
                    half val = (b_row < params.k && b_col < params.n)
                        ? B[b_row * params.ldb + b_col] : half(0.0h);
                    shared_b[next_buffer][r][c] = val;
                }
            }
        }

        // Compute using current buffer with simdgroup_matrix
        #pragma unroll 4
        for (uint k = 0; k < TILE_K; k += SIMD_TILE) {
            // Load 2x2 grid of 8x8 A fragments
            simdgroup_half8x8 a_frag[2];
            simdgroup_load(a_frag[0], &shared_a[buffer_idx][simd_m][k], TILE_K + 4);
            simdgroup_load(a_frag[1], &shared_a[buffer_idx][simd_m + 8][k], TILE_K + 4);

            // Load 2 B fragments (8x8 each)
            simdgroup_half8x8 b_frag[2];
            simdgroup_load(b_frag[0], &shared_b[buffer_idx][k][simd_n], TILE_N + 4);
            simdgroup_load(b_frag[1], &shared_b[buffer_idx][k][simd_n + 8], TILE_N + 4);

            // 2x2 matrix multiply-accumulate
            simdgroup_multiply_accumulate(c_frag[0][0], a_frag[0], b_frag[0], c_frag[0][0]);
            simdgroup_multiply_accumulate(c_frag[0][1], a_frag[0], b_frag[1], c_frag[0][1]);
            simdgroup_multiply_accumulate(c_frag[1][0], a_frag[1], b_frag[0], c_frag[1][0]);
            simdgroup_multiply_accumulate(c_frag[1][1], a_frag[1], b_frag[1], c_frag[1][1]);
        }

        buffer_idx = next_buffer;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Store results with alpha/beta scaling
    const half alpha_h = half(params.alpha);
    const half beta_h = half(params.beta);

    // Write 16x16 result per simd group
    for (uint i = 0; i < 2; i++) {
        for (uint j = 0; j < 2; j++) {
            const uint out_row_base = m_start + simd_m + i * 8;
            const uint out_col_base = n_start + simd_n + j * 8;

            // Store with scaling
            if (beta_h == half(0.0h)) {
                // Simple alpha scaling, store directly
                simdgroup_half8x8 scaled;
                for (uint r = 0; r < 8; r++) {
                    for (uint c = 0; c < 8; c++) {
                        const uint out_row = out_row_base + r;
                        const uint out_col = out_col_base + c;
                        if (out_row < params.m && out_col < params.n) {
                            C[out_row * params.ldc + out_col] = alpha_h * c_frag[i][j][r][c];
                        }
                    }
                }
            } else {
                // Alpha + beta scaling
                for (uint r = 0; r < 8; r++) {
                    for (uint c = 0; c < 8; c++) {
                        const uint out_row = out_row_base + r;
                        const uint out_col = out_col_base + c;
                        if (out_row < params.m && out_col < params.n) {
                            const uint idx = out_row * params.ldc + out_col;
                            C[idx] = alpha_h * c_frag[i][j][r][c] + beta_h * C[idx];
                        }
                    }
                }
            }
        }
    }
}

// =============================================================================
// Original FP16 GEMM (kept for compatibility)
// =============================================================================
kernel void gemm_f16(
    device const half* A [[buffer(0)]],
    device const half* B [[buffer(1)]],
    device half* C [[buffer(2)]],
    constant GemmParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint TILE_SIZE = 64;
    const uint TILE_K_OLD = 32;

    uint tile_m = gid.y;
    uint tile_n = gid.x;
    uint row = tile_m * TILE_SIZE + tid.y;
    uint col = tile_n * TILE_SIZE + tid.x * 8 + simd_lane % 8;

    if (row >= params.m || col >= params.n) return;

    threadgroup half shared_a[TILE_SIZE][TILE_K_OLD];
    threadgroup half shared_b[TILE_K_OLD][TILE_SIZE];

    simdgroup_half8x8 c_frag;
    c_frag = simdgroup_half8x8(0.0h);

    uint num_k_tiles = (params.k + TILE_K_OLD - 1) / TILE_K_OLD;

    for (uint k_tile = 0; k_tile < num_k_tiles; k_tile++) {
        uint k_start = k_tile * TILE_K_OLD;

        for (uint i = tid.y; i < TILE_SIZE; i += TILE_SIZE / 8) {
            for (uint j = tid.x; j < TILE_K_OLD; j += TILE_SIZE / 8) {
                uint a_row = tile_m * TILE_SIZE + i;
                uint a_col = k_start + j;
                if (a_row < params.m && a_col < params.k) {
                    shared_a[i][j] = A[a_row * params.lda + a_col];
                } else {
                    shared_a[i][j] = 0.0h;
                }
            }
        }

        for (uint i = tid.y; i < TILE_K_OLD; i += TILE_SIZE / 8) {
            for (uint j = tid.x; j < TILE_SIZE; j += TILE_SIZE / 8) {
                uint b_row = k_start + i;
                uint b_col = tile_n * TILE_SIZE + j;
                if (b_row < params.k && b_col < params.n) {
                    shared_b[i][j] = B[b_row * params.ldb + b_col];
                } else {
                    shared_b[i][j] = 0.0h;
                }
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        for (uint k = 0; k < TILE_K_OLD; k += 8) {
            simdgroup_half8x8 a_frag;
            simdgroup_half8x8 b_frag;
            simdgroup_load(a_frag, &shared_a[tid.y * 8][k], TILE_K_OLD);
            simdgroup_load(b_frag, &shared_b[k][tid.x * 8], TILE_SIZE);
            simdgroup_multiply_accumulate(c_frag, a_frag, b_frag, c_frag);
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    half alpha_h = half(params.alpha);
    half beta_h = half(params.beta);

    for (uint i = 0; i < 8; i++) {
        for (uint j = 0; j < 8; j++) {
            uint out_row = tile_m * TILE_SIZE + tid.y * 8 + i;
            uint out_col = tile_n * TILE_SIZE + tid.x * 8 + j;
            if (out_row < params.m && out_col < params.n) {
                uint out_idx = out_row * params.ldc + out_col;
                half old_val = beta_h != 0.0h ? C[out_idx] : 0.0h;
                C[out_idx] = alpha_h * c_frag[i][j] + beta_h * old_val;
            }
        }
    }
}

// =============================================================================
// High-Performance FP32 GEMM with SIMD optimizations
// =============================================================================
kernel void gemm_f32_v2(
    device const float* A [[buffer(0)]],
    device const float* B [[buffer(1)]],
    device float* C [[buffer(2)]],
    constant GemmParams& params [[buffer(3)]],
    uint2 gid [[threadgroup_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    const uint tile_m = gid.y;
    const uint tile_n = gid.x;

    const uint m_start = tile_m * TILE_M;
    const uint n_start = tile_n * TILE_N;
    if (m_start >= params.m || n_start >= params.n) return;

    // Double-buffered shared memory
    threadgroup float shared_a[NUM_BUFFERS][TILE_M][TILE_K + 2] __attribute__((aligned(16)));
    threadgroup float shared_b[NUM_BUFFERS][TILE_K][TILE_N + 2] __attribute__((aligned(16)));

    // Each thread computes a 4x4 block
    const uint thread_row = (simd_group * SIMD_SIZE + simd_lane) / (TILE_N / 4);
    const uint thread_col = (simd_group * SIMD_SIZE + simd_lane) % (TILE_N / 4);

    // Accumulator registers (4x4 per thread)
    float acc[4][4] = {{0.0f}};

    const uint num_k_tiles = (params.k + TILE_K - 1) / TILE_K;
    uint buffer_idx = 0;

    // Preload first tile
    {
        const uint load_idx = tid.y * SIMD_SIZE + simd_lane;
        const uint loads_per_tile_a = (TILE_M * TILE_K) / (4 * SIMD_SIZE);
        const uint loads_per_tile_b = (TILE_K * TILE_N) / (4 * SIMD_SIZE);

        for (uint i = load_idx; i < TILE_M * TILE_K; i += 4 * SIMD_SIZE) {
            const uint r = i / TILE_K;
            const uint c = i % TILE_K;
            const uint a_row = m_start + r;
            const uint a_col = c;
            shared_a[0][r][c] = (a_row < params.m && a_col < params.k)
                ? A[a_row * params.lda + a_col] : 0.0f;
        }

        for (uint i = load_idx; i < TILE_K * TILE_N; i += 4 * SIMD_SIZE) {
            const uint r = i / TILE_N;
            const uint c = i % TILE_N;
            const uint b_row = r;
            const uint b_col = n_start + c;
            shared_b[0][r][c] = (b_row < params.k && b_col < params.n)
                ? B[b_row * params.ldb + b_col] : 0.0f;
        }
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint k_tile = 0; k_tile < num_k_tiles; k_tile++) {
        const uint next_buffer = 1 - buffer_idx;
        const uint k_start_next = (k_tile + 1) * TILE_K;

        // Prefetch next tile
        if (k_tile + 1 < num_k_tiles) {
            const uint load_idx = tid.y * SIMD_SIZE + simd_lane;

            for (uint i = load_idx; i < TILE_M * TILE_K; i += 4 * SIMD_SIZE) {
                const uint r = i / TILE_K;
                const uint c = i % TILE_K;
                const uint a_row = m_start + r;
                const uint a_col = k_start_next + c;
                shared_a[next_buffer][r][c] = (a_row < params.m && a_col < params.k)
                    ? A[a_row * params.lda + a_col] : 0.0f;
            }

            for (uint i = load_idx; i < TILE_K * TILE_N; i += 4 * SIMD_SIZE) {
                const uint r = i / TILE_N;
                const uint c = i % TILE_N;
                const uint b_row = k_start_next + r;
                const uint b_col = n_start + c;
                shared_b[next_buffer][r][c] = (b_row < params.k && b_col < params.n)
                    ? B[b_row * params.ldb + b_col] : 0.0f;
            }
        }

        // Compute 4x4 block per thread
        #pragma unroll 4
        for (uint k = 0; k < TILE_K; k++) {
            float a_reg[4];
            float b_reg[4];

            #pragma unroll 4
            for (uint i = 0; i < 4; i++) {
                a_reg[i] = shared_a[buffer_idx][thread_row * 4 + i][k];
                b_reg[i] = shared_b[buffer_idx][k][thread_col * 4 + i];
            }

            #pragma unroll 4
            for (uint i = 0; i < 4; i++) {
                #pragma unroll 4
                for (uint j = 0; j < 4; j++) {
                    acc[i][j] = fma(a_reg[i], b_reg[j], acc[i][j]);
                }
            }
        }

        buffer_idx = next_buffer;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    // Store with alpha/beta scaling
    const float alpha = params.alpha;
    const float beta = params.beta;

    #pragma unroll 4
    for (uint i = 0; i < 4; i++) {
        #pragma unroll 4
        for (uint j = 0; j < 4; j++) {
            const uint out_row = m_start + thread_row * 4 + i;
            const uint out_col = n_start + thread_col * 4 + j;
            if (out_row < params.m && out_col < params.n) {
                const uint idx = out_row * params.ldc + out_col;
                if (beta != 0.0f) {
                    C[idx] = fma(alpha, acc[i][j], beta * C[idx]);
                } else {
                    C[idx] = alpha * acc[i][j];
                }
            }
        }
    }
}

// =============================================================================
// Original FP32 GEMM (kept for compatibility)
// =============================================================================
kernel void gemm_f32(
    device const float* A [[buffer(0)]],
    device const float* B [[buffer(1)]],
    device float* C [[buffer(2)]],
    constant GemmParams& params [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]],
    uint2 tid [[thread_position_in_threadgroup]]
) {
    uint tile_m = gid.y / 16;
    uint tile_n = gid.x / 16;
    uint local_row = tid.y;
    uint local_col = tid.x;
    uint row = tile_m * 16 + local_row;
    uint col = tile_n * 16 + local_col;

    if (row >= params.m || col >= params.n) return;

    threadgroup float shared_a[16][32];
    threadgroup float shared_b[32][16];

    float sum = 0.0f;
    uint num_k_tiles = (params.k + 31) / 32;

    for (uint k_tile = 0; k_tile < num_k_tiles; k_tile++) {
        uint k_start = k_tile * 32;

        for (uint j = local_col; j < 32; j += 16) {
            uint a_col = k_start + j;
            if (a_col < params.k) {
                shared_a[local_row][j] = A[row * params.lda + a_col];
            } else {
                shared_a[local_row][j] = 0.0f;
            }
        }

        for (uint i = local_row; i < 32; i += 16) {
            uint b_row = k_start + i;
            if (b_row < params.k) {
                shared_b[i][local_col] = B[b_row * params.ldb + col];
            } else {
                shared_b[i][local_col] = 0.0f;
            }
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);

        #pragma unroll
        for (uint k = 0; k < 32; k++) {
            sum = fma(shared_a[local_row][k], shared_b[k][local_col], sum);
        }

        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    uint out_idx = row * params.ldc + col;
    float old_val = params.beta != 0.0f ? C[out_idx] : 0.0f;
    C[out_idx] = fma(params.alpha, sum, params.beta * old_val);
}

// =============================================================================
// Batched GEMM for attention score computation
// =============================================================================
kernel void batched_gemm_f32(
    device const float* A [[buffer(0)]],
    device const float* B [[buffer(1)]],
    device float* C [[buffer(2)]],
    constant uint4& dims [[buffer(3)]],
    uint3 gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]]
) {
    uint batch = gid.z;
    uint row = gid.y;
    uint col = gid.x;

    uint m = dims.x;
    uint n = dims.y;
    uint k = dims.z;
    uint num_batches = dims.w;

    if (batch >= num_batches || row >= m || col >= n) return;

    uint a_offset = batch * m * k;
    uint b_offset = batch * k * n;
    uint c_offset = batch * m * n;

    // Compute dot product with SIMD when possible
    float sum = 0.0f;

    #pragma unroll 4
    for (uint i = 0; i < k; i++) {
        sum = fma(A[a_offset + row * k + i], B[b_offset + i * n + col], sum);
    }

    C[c_offset + row * n + col] = sum;
}

// =============================================================================
// Vector-matrix multiplication (optimized for single-token generation)
// =============================================================================
kernel void gemv_f32(
    device const float* x [[buffer(0)]],
    device const float* W [[buffer(1)]],
    device float* y [[buffer(2)]],
    constant uint2& dims [[buffer(3)]],
    uint gid [[thread_position_in_grid]],
    uint tid [[thread_position_in_threadgroup]],
    uint simd_lane [[thread_index_in_simdgroup]],
    uint simd_group [[simdgroup_index_in_threadgroup]]
) {
    uint n = dims.x;
    uint k = dims.y;

    if (gid >= n) return;

    // Each thread computes one output using SIMD reduction
    float sum = 0.0f;

    // Use float4 for vectorized loads
    const uint k_vec = k / 4;
    const device float4* x_vec = reinterpret_cast<const device float4*>(x);
    const device float4* w_vec = reinterpret_cast<const device float4*>(&W[gid * k]);

    #pragma unroll 4
    for (uint i = 0; i < k_vec; i++) {
        float4 x_val = x_vec[i];
        float4 w_val = w_vec[i];
        sum = fma(x_val.x, w_val.x, sum);
        sum = fma(x_val.y, w_val.y, sum);
        sum = fma(x_val.z, w_val.z, sum);
        sum = fma(x_val.w, w_val.w, sum);
    }

    // Handle remainder
    for (uint i = k_vec * 4; i < k; i++) {
        sum = fma(x[i], W[gid * k + i], sum);
    }

    y[gid] = sum;
}

// =============================================================================
// Element-wise operations with vectorization
// =============================================================================
kernel void elementwise_add(
    device const float* a [[buffer(0)]],
    device const float* b [[buffer(1)]],
    device float* c [[buffer(2)]],
    constant uint& len [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    const uint vec_len = len / 4;
    if (gid < vec_len) {
        const device float4* a_vec = reinterpret_cast<const device float4*>(a);
        const device float4* b_vec = reinterpret_cast<const device float4*>(b);
        device float4* c_vec = reinterpret_cast<device float4*>(c);
        c_vec[gid] = a_vec[gid] + b_vec[gid];
    } else {
        uint idx = vec_len * 4 + (gid - vec_len);
        if (idx < len) {
            c[idx] = a[idx] + b[idx];
        }
    }
}

kernel void elementwise_mul(
    device const float* a [[buffer(0)]],
    device const float* b [[buffer(1)]],
    device float* c [[buffer(2)]],
    constant uint& len [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    const uint vec_len = len / 4;
    if (gid < vec_len) {
        const device float4* a_vec = reinterpret_cast<const device float4*>(a);
        const device float4* b_vec = reinterpret_cast<const device float4*>(b);
        device float4* c_vec = reinterpret_cast<device float4*>(c);
        c_vec[gid] = a_vec[gid] * b_vec[gid];
    } else {
        uint idx = vec_len * 4 + (gid - vec_len);
        if (idx < len) {
            c[idx] = a[idx] * b[idx];
        }
    }
}

// =============================================================================
// SiLU activation: x * sigmoid(x) - vectorized
// =============================================================================
kernel void silu(
    device float* x [[buffer(0)]],
    constant uint& len [[buffer(1)]],
    uint gid [[thread_position_in_grid]]
) {
    const uint vec_len = len / 4;
    if (gid < vec_len) {
        device float4* x_vec = reinterpret_cast<device float4*>(x);
        float4 val = x_vec[gid];
        float4 sigmoid = 1.0f / (1.0f + exp(-val));
        x_vec[gid] = val * sigmoid;
    } else {
        uint idx = vec_len * 4 + (gid - vec_len);
        if (idx < len) {
            float val = x[idx];
            x[idx] = val / (1.0f + exp(-val));
        }
    }
}

// =============================================================================
// Fused SiLU + multiply (for MLP gate) - vectorized
// =============================================================================
kernel void silu_mul(
    device const float* gate [[buffer(0)]],
    device const float* up [[buffer(1)]],
    device float* out [[buffer(2)]],
    constant uint& len [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    const uint vec_len = len / 4;
    if (gid < vec_len) {
        const device float4* gate_vec = reinterpret_cast<const device float4*>(gate);
        const device float4* up_vec = reinterpret_cast<const device float4*>(up);
        device float4* out_vec = reinterpret_cast<device float4*>(out);

        float4 g = gate_vec[gid];
        float4 sigmoid = 1.0f / (1.0f + exp(-g));
        float4 silu_g = g * sigmoid;
        out_vec[gid] = silu_g * up_vec[gid];
    } else {
        uint idx = vec_len * 4 + (gid - vec_len);
        if (idx < len) {
            float g = gate[idx];
            float silu_g = g / (1.0f + exp(-g));
            out[idx] = silu_g * up[idx];
        }
    }
}
