// LoRA (Low-Rank Adaptation) Forward Pass Shader
//
// Computes: output = input + scaling * (input @ A @ B)
//
// Where:
// - input: (batch_size, in_dim)
// - A: (in_dim, rank) - down projection
// - B: (rank, out_dim) - up projection
// - output: (batch_size, out_dim)
//
// Performance target: <1ms for typical LoRA ranks (2-64)
//
// Optimization strategy:
// 1. Fuse both matmuls into single kernel
// 2. Use shared memory for intermediate (rank is small)
// 3. Each thread computes one output element

const WARP_SIZE: u32 = 32u;
const MAX_RANK: u32 = 64u;  // Maximum supported LoRA rank

struct Uniforms {
    batch_size: f32,
    in_dim: f32,
    rank: f32,
    out_dim: f32,
    scaling: f32,     // alpha / rank
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read> lora_A: array<f32>;  // (in_dim, rank)
@group(0) @binding(2) var<storage, read> lora_B: array<f32>;  // (rank, out_dim)
@group(0) @binding(3) var<storage, read_write> output: array<f32>;
@group(0) @binding(4) var<uniform> uniforms: Uniforms;

// Shared memory for intermediate result (input @ A)
var<workgroup> intermediate: array<f32, 2048>;  // batch * rank (fits typical cases)

// Thread-local registers
var<private> input_cache: array<f32, 32>;  // Cache input values
var<private> a_cache: array<f32, 64>;      // Cache A column

@compute @workgroup_size(256, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let batch_size = u32(uniforms.batch_size);
    let in_dim = u32(uniforms.in_dim);
    let rank = u32(uniforms.rank);
    let out_dim = u32(uniforms.out_dim);
    let scaling = uniforms.scaling;

    let thread_id = local_id.x;
    let global_thread = global_id.x;

    // Compute which output element this thread handles
    let batch_idx = global_thread / out_dim;
    let out_idx = global_thread % out_dim;

    if (batch_idx >= batch_size) {
        return;
    }

    // Phase 1: Compute input @ A for this batch element
    // Store in shared memory for reuse
    // Each thread contributes to computing intermediate[batch_idx, :]

    // For small rank, each thread can compute entire row
    if (rank <= MAX_RANK && thread_id < rank) {
        var sum = 0.0f;

        // Dot product: input[batch_idx, :] @ A[:, thread_id]
        for (var i = 0u; i < in_dim; i++) {
            let input_val = input[batch_idx * in_dim + i];
            let a_val = lora_A[i * rank + thread_id];
            sum += input_val * a_val;
        }

        // Store in shared memory
        let shared_idx = (batch_idx % 32u) * rank + thread_id;  // Wrap for shared memory size
        if (shared_idx < 2048u) {
            intermediate[shared_idx] = sum;
        }
    }

    workgroupBarrier();

    // Phase 2: Compute intermediate @ B for this output position
    var lora_output = 0.0f;

    // Dot product: intermediate[batch_idx, :] @ B[:, out_idx]
    for (var r = 0u; r < rank; r++) {
        let shared_idx = (batch_idx % 32u) * rank + r;
        let inter_val = select(0.0, intermediate[shared_idx], shared_idx < 2048u);
        let b_val = lora_B[r * out_dim + out_idx];
        lora_output += inter_val * b_val;
    }

    // Apply scaling and add to output
    // Note: For true residual connection, we'd add to existing output
    // Here we assume output buffer is pre-filled with base model output
    // or we're computing the delta only
    output[batch_idx * out_dim + out_idx] = lora_output * scaling;
}

// Fused LoRA with base weight: output = (input @ W) + scaling * (input @ A @ B)
// More efficient when we have access to base weights
@compute @workgroup_size(256, 1, 1)
fn main_fused(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // Would include base weight computation
    // Placeholder for full integration
}

// Batched LoRA for multiple adapters (multi-task serving)
// Each batch element can use different LoRA weights
@compute @workgroup_size(256, 1, 1)
fn main_batched_lora(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // Supports different LoRA for different requests in same batch
    // Useful for serving multiple fine-tuned models
    // Placeholder for multi-tenant serving
}

// Quantized LoRA (int4 weights)
// Significant memory savings for large rank or many adapters
@compute @workgroup_size(256, 1, 1)
fn main_quantized(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // A and B stored as int4 with scale factors
    // Dequantize on-the-fly during computation
    // Placeholder for memory-constrained deployment
}

// DoRA (Weight-Decomposed Low-Rank Adaptation)
// Decomposes weight update into magnitude and direction
@compute @workgroup_size(256, 1, 1)
fn main_dora(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    // DoRA: output = m * (W + scaling * A @ B) / ||W + scaling * A @ B||
    // where m is learned magnitude
    // Placeholder for DoRA support
}
