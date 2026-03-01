// RMSNorm and LayerNorm Shaders for WebGPU WASM
//
// Implements normalization layers used in transformer architectures:
// - RMSNorm: Used in Llama, Mistral (no mean subtraction)
// - LayerNorm: Standard transformer normalization
//
// RMSNorm: y = x / sqrt(mean(x^2) + eps) * weight
// LayerNorm: y = (x - mean) / sqrt(var + eps) * weight + bias

const WARP_SIZE: u32 = 32u;
const MAX_DIM: u32 = 8192u;

struct NormUniforms {
    hidden_dim: u32,
    batch_size: u32,
    eps: f32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read> weight: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;
@group(0) @binding(3) var<uniform> uniforms: NormUniforms;

// Shared memory for parallel reduction
var<workgroup> partial_sums: array<f32, 256>;

// RMSNorm: y = x * rsqrt(mean(x^2) + eps) * weight
@compute @workgroup_size(256, 1, 1)
fn rms_norm(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let hidden_dim = uniforms.hidden_dim;
    let eps = uniforms.eps;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * hidden_dim;

    // Each thread computes partial sum of squares
    var thread_sum = 0.0f;
    let elements_per_thread = (hidden_dim + 255u) / 256u;

    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            let x = input[offset + idx];
            thread_sum += x * x;
        }
    }

    // Store partial sum
    partial_sums[thread_id] = thread_sum;
    workgroupBarrier();

    // Parallel reduction for sum of squares
    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            partial_sums[thread_id] += partial_sums[thread_id + stride];
        }
        workgroupBarrier();
    }

    // Compute RMS scale factor
    let mean_sq = partial_sums[0] / f32(hidden_dim);
    let rms_scale = 1.0f / sqrt(mean_sq + eps);
    workgroupBarrier();

    // Apply normalization and weight
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            let x = input[offset + idx];
            output[offset + idx] = x * rms_scale * weight[idx];
        }
    }
}

// Fused RMSNorm + Residual: y = (x + residual) * rsqrt(mean((x+res)^2) + eps) * weight
@compute @workgroup_size(256, 1, 1)
fn rms_norm_residual(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let hidden_dim = uniforms.hidden_dim;
    let eps = uniforms.eps;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * hidden_dim;

    // Compute partial sum of (x + residual)^2
    var thread_sum = 0.0f;
    let elements_per_thread = (hidden_dim + 255u) / 256u;

    // First pass: compute residual sum and store in shared for reduction
    // Note: residual is passed in output buffer for in-place update
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            let x = input[offset + idx] + output[offset + idx]; // x + residual
            thread_sum += x * x;
        }
    }

    partial_sums[thread_id] = thread_sum;
    workgroupBarrier();

    // Parallel reduction
    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            partial_sums[thread_id] += partial_sums[thread_id + stride];
        }
        workgroupBarrier();
    }

    let mean_sq = partial_sums[0] / f32(hidden_dim);
    let rms_scale = 1.0f / sqrt(mean_sq + eps);
    workgroupBarrier();

    // Apply normalization
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            let x = input[offset + idx] + output[offset + idx];
            output[offset + idx] = x * rms_scale * weight[idx];
        }
    }
}

// Standard LayerNorm with bias
@group(0) @binding(4) var<storage, read> bias: array<f32>;

@compute @workgroup_size(256, 1, 1)
fn layer_norm(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let hidden_dim = uniforms.hidden_dim;
    let eps = uniforms.eps;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * hidden_dim;

    let elements_per_thread = (hidden_dim + 255u) / 256u;

    // First pass: compute mean
    var thread_sum = 0.0f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            thread_sum += input[offset + idx];
        }
    }

    partial_sums[thread_id] = thread_sum;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            partial_sums[thread_id] += partial_sums[thread_id + stride];
        }
        workgroupBarrier();
    }

    let mean = partial_sums[0] / f32(hidden_dim);
    workgroupBarrier();

    // Second pass: compute variance
    var thread_var = 0.0f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            let diff = input[offset + idx] - mean;
            thread_var += diff * diff;
        }
    }

    partial_sums[thread_id] = thread_var;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            partial_sums[thread_id] += partial_sums[thread_id + stride];
        }
        workgroupBarrier();
    }

    let variance = partial_sums[0] / f32(hidden_dim);
    let inv_std = 1.0f / sqrt(variance + eps);
    workgroupBarrier();

    // Third pass: normalize and apply affine transform
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < hidden_dim) {
            let x = input[offset + idx];
            output[offset + idx] = (x - mean) * inv_std * weight[idx] + bias[idx];
        }
    }
}

// Fast RMSNorm for small hidden dimensions (direct reduction)
@compute @workgroup_size(128, 1, 1)
fn rms_norm_small(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let hidden_dim = uniforms.hidden_dim;
    let eps = uniforms.eps;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * hidden_dim;

    // For small hidden_dim (<= 128), direct computation
    if (thread_id < hidden_dim) {
        // Compute sum of squares (all threads contribute)
        var sum_sq = 0.0f;
        for (var i = 0u; i < hidden_dim; i++) {
            let x = input[offset + i];
            sum_sq += x * x;
        }

        let rms = sqrt(sum_sq / f32(hidden_dim) + eps);
        let x = input[offset + thread_id];
        output[offset + thread_id] = x / rms * weight[thread_id];
    }
}
