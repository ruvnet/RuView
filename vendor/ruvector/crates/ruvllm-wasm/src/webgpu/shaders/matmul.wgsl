// Tiled Matrix Multiplication Shader for WebGPU WASM
//
// Computes C = A * B using 16x16 tiles optimized for browser WebGPU.
// Uses workgroup shared memory for cache-efficient tile loading.
//
// Memory Layout (row-major):
// - A: M x K matrix
// - B: K x N matrix
// - C: M x N matrix (output)

// Tile size optimized for WebGPU limits
const TILE_SIZE: u32 = 16u;

struct Uniforms {
    M: u32,  // Rows of A, rows of C
    N: u32,  // Cols of B, cols of C
    K: u32,  // Cols of A, rows of B
    alpha: f32, // Scaling factor (default 1.0)
}

@group(0) @binding(0) var<storage, read> A: array<f32>;
@group(0) @binding(1) var<storage, read> B: array<f32>;
@group(0) @binding(2) var<storage, read_write> C: array<f32>;
@group(0) @binding(3) var<uniform> uniforms: Uniforms;

// Shared memory for tile caching
var<workgroup> A_tile: array<f32, 256>;  // TILE_SIZE * TILE_SIZE
var<workgroup> B_tile: array<f32, 256>;

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let M = uniforms.M;
    let N = uniforms.N;
    let K = uniforms.K;
    let alpha = uniforms.alpha;

    // Global row and column
    let row = global_id.x;
    let col = global_id.y;

    // Thread position within tile
    let local_row = local_id.x;
    let local_col = local_id.y;

    // Accumulator for this thread's output element
    var sum = 0.0f;

    // Number of tiles to process along K dimension
    let num_tiles = (K + TILE_SIZE - 1u) / TILE_SIZE;

    // Iterate over tiles
    for (var t = 0u; t < num_tiles; t++) {
        let tile_k = t * TILE_SIZE;

        // Load A tile element
        let a_row = row;
        let a_col = tile_k + local_col;
        if (a_row < M && a_col < K) {
            A_tile[local_row * TILE_SIZE + local_col] = A[a_row * K + a_col];
        } else {
            A_tile[local_row * TILE_SIZE + local_col] = 0.0;
        }

        // Load B tile element
        let b_row = tile_k + local_row;
        let b_col = col;
        if (b_row < K && b_col < N) {
            B_tile[local_row * TILE_SIZE + local_col] = B[b_row * N + b_col];
        } else {
            B_tile[local_row * TILE_SIZE + local_col] = 0.0;
        }

        // Synchronize to ensure tile is fully loaded
        workgroupBarrier();

        // Compute partial dot product for this tile
        let tile_k_end = min(TILE_SIZE, K - tile_k);
        for (var k = 0u; k < tile_k_end; k++) {
            sum += A_tile[local_row * TILE_SIZE + k] * B_tile[k * TILE_SIZE + local_col];
        }

        // Synchronize before loading next tile
        workgroupBarrier();
    }

    // Write result with optional scaling
    if (row < M && col < N) {
        C[row * N + col] = sum * alpha;
    }
}

// Batched matrix multiply for multi-head attention projections
// C[b] = A[b] * B where A is batch_size x M x K and B is K x N
@compute @workgroup_size(16, 16, 1)
fn main_batched(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let M = uniforms.M;
    let N = uniforms.N;
    let K = uniforms.K;

    let batch_idx = group_id.z;
    let row = global_id.x;
    let col = global_id.y;

    let local_row = local_id.x;
    let local_col = local_id.y;

    var sum = 0.0f;
    let num_tiles = (K + TILE_SIZE - 1u) / TILE_SIZE;

    // Offset into batched A
    let batch_offset_a = batch_idx * M * K;
    let batch_offset_c = batch_idx * M * N;

    for (var t = 0u; t < num_tiles; t++) {
        let tile_k = t * TILE_SIZE;

        // Load A tile (batched)
        let a_row = row;
        let a_col = tile_k + local_col;
        if (a_row < M && a_col < K) {
            A_tile[local_row * TILE_SIZE + local_col] = A[batch_offset_a + a_row * K + a_col];
        } else {
            A_tile[local_row * TILE_SIZE + local_col] = 0.0;
        }

        // Load B tile (shared across batch)
        let b_row = tile_k + local_row;
        let b_col = col;
        if (b_row < K && b_col < N) {
            B_tile[local_row * TILE_SIZE + local_col] = B[b_row * N + b_col];
        } else {
            B_tile[local_row * TILE_SIZE + local_col] = 0.0;
        }

        workgroupBarrier();

        let tile_k_end = min(TILE_SIZE, K - tile_k);
        for (var k = 0u; k < tile_k_end; k++) {
            sum += A_tile[local_row * TILE_SIZE + k] * B_tile[k * TILE_SIZE + local_col];
        }

        workgroupBarrier();
    }

    if (row < M && col < N) {
        C[batch_offset_c + row * N + col] = sum;
    }
}

// Vector-matrix multiply optimized for single token generation
// y = x * W where x is 1 x K and W is K x N
@compute @workgroup_size(256, 1, 1)
fn main_gemv(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let K = uniforms.K;
    let N = uniforms.N;

    let col = global_id.x;

    if (col >= N) {
        return;
    }

    var sum = 0.0f;

    // Simple reduction - each thread computes one output element
    for (var k = 0u; k < K; k++) {
        sum += A[k] * B[k * N + col];
    }

    C[col] = sum * uniforms.alpha;
}
