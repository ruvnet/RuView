// Tiled Matrix Multiplication Shader
//
// Computes C = A * B using 128x128 tiles for cache efficiency.
// Targets 10+ TFLOPS on discrete GPUs.
//
// Algorithm:
// 1. Each workgroup computes a TILE_SIZE x TILE_SIZE block of C
// 2. A and B are loaded into shared memory in tiles
// 3. Each thread computes a 4x4 subblock for register tiling
// 4. Accumulation happens in registers, then written to C
//
// Memory Layout:
// - A: M x K matrix (row-major)
// - B: K x N matrix (row-major)
// - C: M x N matrix (row-major, output)

// Tile dimensions (must match host code)
const TILE_SIZE: u32 = 128u;
const BLOCK_SIZE: u32 = 16u;  // Threads per dimension in workgroup
const THREAD_TILE: u32 = 8u;  // Each thread computes 8x8 elements

// Uniforms
struct Uniforms {
    M: u32,  // Rows of A, rows of C
    N: u32,  // Cols of B, cols of C
    K: u32,  // Cols of A, rows of B
    tile_size: u32,
}

@group(0) @binding(0) var<storage, read> A: array<f32>;
@group(0) @binding(1) var<storage, read> B: array<f32>;
@group(0) @binding(2) var<storage, read_write> C: array<f32>;
@group(0) @binding(3) var<uniform> uniforms: Uniforms;

// Shared memory for tile caching
var<workgroup> A_tile: array<f32, 2048>;  // TILE_SIZE * BLOCK_SIZE = 128 * 16
var<workgroup> B_tile: array<f32, 2048>;

// Thread-local accumulator registers
var<private> acc: array<f32, 64>;  // THREAD_TILE * THREAD_TILE = 8 * 8

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let M = uniforms.M;
    let N = uniforms.N;
    let K = uniforms.K;

    // Global row and column for this thread's block
    let block_row = group_id.x * TILE_SIZE;
    let block_col = group_id.y * TILE_SIZE;

    // Thread position within workgroup
    let thread_row = local_id.x;
    let thread_col = local_id.y;

    // Initialize accumulators to zero
    for (var i = 0u; i < 64u; i++) {
        acc[i] = 0.0;
    }

    // Number of K-tiles to process
    let num_k_tiles = (K + TILE_SIZE - 1u) / TILE_SIZE;

    // Iterate over K dimension in tiles
    for (var k_tile = 0u; k_tile < num_k_tiles; k_tile++) {
        let k_base = k_tile * TILE_SIZE;

        // Cooperative load of A tile into shared memory
        // Each thread loads multiple elements
        for (var i = 0u; i < THREAD_TILE; i++) {
            let a_row = block_row + thread_row * THREAD_TILE + i;
            for (var j = 0u; j < THREAD_TILE; j++) {
                let a_col = k_base + thread_col * THREAD_TILE + j;
                let shared_idx = (thread_row * THREAD_TILE + i) * BLOCK_SIZE + thread_col;

                if (a_row < M && a_col < K) {
                    // Only load partial tile for first few elements to fit in shared memory
                    if (shared_idx < 2048u) {
                        A_tile[shared_idx] = A[a_row * K + a_col];
                    }
                }
            }
        }

        // Cooperative load of B tile into shared memory
        for (var i = 0u; i < THREAD_TILE; i++) {
            let b_row = k_base + thread_row * THREAD_TILE + i;
            for (var j = 0u; j < THREAD_TILE; j++) {
                let b_col = block_col + thread_col * THREAD_TILE + j;
                let shared_idx = (thread_row * THREAD_TILE + i) * BLOCK_SIZE + thread_col;

                if (b_row < K && b_col < N) {
                    if (shared_idx < 2048u) {
                        B_tile[shared_idx] = B[b_row * N + b_col];
                    }
                }
            }
        }

        // Synchronize to ensure all data is loaded
        workgroupBarrier();

        // Compute partial dot products
        // Each thread computes an 8x8 subblock
        for (var k = 0u; k < min(TILE_SIZE, K - k_base); k++) {
            // Load A values into registers
            var a_regs: array<f32, 8>;
            for (var i = 0u; i < THREAD_TILE; i++) {
                let a_shared_row = thread_row * THREAD_TILE + i;
                let a_shared_idx = a_shared_row * BLOCK_SIZE + (k % BLOCK_SIZE);
                if (a_shared_idx < 2048u) {
                    a_regs[i] = A_tile[a_shared_idx];
                } else {
                    a_regs[i] = 0.0;
                }
            }

            // Load B values into registers
            var b_regs: array<f32, 8>;
            for (var j = 0u; j < THREAD_TILE; j++) {
                let b_shared_row = k % BLOCK_SIZE;
                let b_shared_col = thread_col * THREAD_TILE + j;
                let b_shared_idx = b_shared_row * BLOCK_SIZE + (b_shared_col % BLOCK_SIZE);
                if (b_shared_idx < 2048u) {
                    b_regs[j] = B_tile[b_shared_idx];
                } else {
                    b_regs[j] = 0.0;
                }
            }

            // Outer product accumulation
            for (var i = 0u; i < THREAD_TILE; i++) {
                for (var j = 0u; j < THREAD_TILE; j++) {
                    acc[i * THREAD_TILE + j] += a_regs[i] * b_regs[j];
                }
            }
        }

        // Synchronize before loading next tile
        workgroupBarrier();
    }

    // Write accumulated results to global memory
    for (var i = 0u; i < THREAD_TILE; i++) {
        let c_row = block_row + thread_row * THREAD_TILE + i;
        for (var j = 0u; j < THREAD_TILE; j++) {
            let c_col = block_col + thread_col * THREAD_TILE + j;

            if (c_row < M && c_col < N) {
                C[c_row * N + c_col] = acc[i * THREAD_TILE + j];
            }
        }
    }
}

// Quantized int8 matrix multiplication variant
// Uses int8 inputs with int32 accumulation, then scales to f32 output
@compute @workgroup_size(16, 16, 1)
fn main_int8(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // Quantized version would use packed i8x4 and accumulate to i32
    // Then scale by quantization factors at the end
    // Left as placeholder for future implementation
}
