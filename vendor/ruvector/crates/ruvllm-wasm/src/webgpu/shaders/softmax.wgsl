// Softmax Shader for WebGPU WASM
//
// Numerically stable softmax: y = exp(x - max(x)) / sum(exp(x - max(x)))
// Uses parallel reduction for finding max and computing sum.
//
// Variants:
// - Full softmax for attention scores
// - Temperature-scaled softmax for sampling
// - Top-k softmax for efficient sampling

const MAX_SEQ_LEN: u32 = 8192u;

struct SoftmaxUniforms {
    dim: u32,         // Dimension to reduce over
    batch_size: u32,  // Number of rows
    temperature: f32, // Scaling factor (1.0 for standard)
    top_k: u32,       // 0 for full softmax, >0 for top-k
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> uniforms: SoftmaxUniforms;

// Shared memory for reductions
var<workgroup> reduction_buf: array<f32, 256>;

// Standard row-wise softmax
@compute @workgroup_size(256, 1, 1)
fn softmax(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let dim = uniforms.dim;
    let temperature = uniforms.temperature;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * dim;

    let elements_per_thread = (dim + 255u) / 256u;

    // Phase 1: Find max value
    var thread_max = -1e10f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            thread_max = max(thread_max, input[offset + idx] / temperature);
        }
    }

    reduction_buf[thread_id] = thread_max;
    workgroupBarrier();

    // Parallel max reduction
    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            reduction_buf[thread_id] = max(reduction_buf[thread_id], reduction_buf[thread_id + stride]);
        }
        workgroupBarrier();
    }

    let max_val = reduction_buf[0];
    workgroupBarrier();

    // Phase 2: Compute sum of exp(x - max)
    var thread_sum = 0.0f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            let x = input[offset + idx] / temperature - max_val;
            thread_sum += exp(x);
        }
    }

    reduction_buf[thread_id] = thread_sum;
    workgroupBarrier();

    // Parallel sum reduction
    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            reduction_buf[thread_id] += reduction_buf[thread_id + stride];
        }
        workgroupBarrier();
    }

    let sum_val = reduction_buf[0];
    let inv_sum = 1.0f / sum_val;
    workgroupBarrier();

    // Phase 3: Compute normalized softmax
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            let x = input[offset + idx] / temperature - max_val;
            output[offset + idx] = exp(x) * inv_sum;
        }
    }
}

// In-place softmax (input and output point to same buffer)
@compute @workgroup_size(256, 1, 1)
fn softmax_inplace(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let dim = uniforms.dim;
    let temperature = uniforms.temperature;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * dim;

    let elements_per_thread = (dim + 255u) / 256u;

    // Find max
    var thread_max = -1e10f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            thread_max = max(thread_max, output[offset + idx] / temperature);
        }
    }

    reduction_buf[thread_id] = thread_max;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            reduction_buf[thread_id] = max(reduction_buf[thread_id], reduction_buf[thread_id + stride]);
        }
        workgroupBarrier();
    }

    let max_val = reduction_buf[0];
    workgroupBarrier();

    // Compute exp and sum
    var thread_sum = 0.0f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            let x = exp(output[offset + idx] / temperature - max_val);
            output[offset + idx] = x;  // Store intermediate exp value
            thread_sum += x;
        }
    }

    reduction_buf[thread_id] = thread_sum;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            reduction_buf[thread_id] += reduction_buf[thread_id + stride];
        }
        workgroupBarrier();
    }

    let inv_sum = 1.0f / reduction_buf[0];
    workgroupBarrier();

    // Normalize in place
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            output[offset + idx] *= inv_sum;
        }
    }
}

// Small dimension softmax (dim <= 256)
@compute @workgroup_size(256, 1, 1)
fn softmax_small(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let dim = uniforms.dim;
    let temperature = uniforms.temperature;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * dim;

    // Load value for this thread
    var x = -1e10f;
    if (thread_id < dim) {
        x = input[offset + thread_id] / temperature;
    }

    reduction_buf[thread_id] = x;
    workgroupBarrier();

    // Find max using warp-level operations
    var max_val = x;
    for (var i = 0u; i < dim; i++) {
        max_val = max(max_val, reduction_buf[i]);
    }
    workgroupBarrier();

    // Compute exp and sum
    var exp_val = 0.0f;
    if (thread_id < dim) {
        exp_val = exp(x - max_val);
    }
    reduction_buf[thread_id] = exp_val;
    workgroupBarrier();

    var sum_val = 0.0f;
    for (var i = 0u; i < dim; i++) {
        sum_val += reduction_buf[i];
    }

    // Write normalized output
    if (thread_id < dim) {
        output[offset + thread_id] = exp_val / sum_val;
    }
}

// Log softmax for numerical stability in loss computation
@compute @workgroup_size(256, 1, 1)
fn log_softmax(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let dim = uniforms.dim;
    let temperature = uniforms.temperature;

    let batch_idx = group_id.x;
    let thread_id = local_id.x;
    let offset = batch_idx * dim;

    let elements_per_thread = (dim + 255u) / 256u;

    // Find max
    var thread_max = -1e10f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            thread_max = max(thread_max, input[offset + idx] / temperature);
        }
    }

    reduction_buf[thread_id] = thread_max;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            reduction_buf[thread_id] = max(reduction_buf[thread_id], reduction_buf[thread_id + stride]);
        }
        workgroupBarrier();
    }

    let max_val = reduction_buf[0];
    workgroupBarrier();

    // Compute log-sum-exp
    var thread_sum = 0.0f;
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            thread_sum += exp(input[offset + idx] / temperature - max_val);
        }
    }

    reduction_buf[thread_id] = thread_sum;
    workgroupBarrier();

    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (thread_id < stride) {
            reduction_buf[thread_id] += reduction_buf[thread_id + stride];
        }
        workgroupBarrier();
    }

    let log_sum = log(reduction_buf[0]) + max_val;
    workgroupBarrier();

    // Compute log softmax: log(softmax(x)) = x - log_sum_exp(x)
    for (var i = 0u; i < elements_per_thread; i++) {
        let idx = thread_id + i * 256u;
        if (idx < dim) {
            output[offset + idx] = input[offset + idx] / temperature - log_sum;
        }
    }
}
