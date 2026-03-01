// =============================================================================
// Prime-Radiant GPU Compute Shaders - Energy Computation
// =============================================================================
//
// Parallel reduction to compute total coherence energy:
// E(S) = sum(w_e * |r_e|^2)
//
// Uses a two-phase reduction strategy:
// 1. Local reduction within workgroups using shared memory
// 2. Global reduction across workgroup partial sums

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

struct EnergyParams {
    num_elements: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
    _padding3: u32,
    _padding4: u32,
    _padding5: u32,
    _padding6: u32,
}

const WORKGROUP_SIZE: u32 = 256u;

// =============================================================================
// BUFFER BINDINGS
// =============================================================================
// Layout matches Rust kernel bind group:
// binding 0: params (uniform)
// binding 1: input (storage, read) - edge energies or partial sums
// binding 2: output (storage, read_write) - partial sums or final result

/// Energy computation parameters
@group(0) @binding(0) var<uniform> params: EnergyParams;

/// Input values to reduce
@group(0) @binding(1) var<storage, read> input_values: array<f32>;

/// Output partial sums or final result
@group(0) @binding(2) var<storage, read_write> output_values: array<f32>;

// =============================================================================
// SHARED MEMORY
// =============================================================================

/// Shared memory for parallel reduction
var<workgroup> shared_data: array<f32, 256>;

// =============================================================================
// MAIN REDUCTION KERNEL
// =============================================================================

/// Phase 1: Reduce input values within workgroup
@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>
) {
    let tid = local_id.x;
    let gid = global_id.x;
    let element_count = params.num_elements;

    // Load element (or 0 if out of bounds)
    var val: f32 = 0.0;
    if (gid < element_count) {
        val = input_values[gid];
    }

    // Store in shared memory
    shared_data[tid] = val;
    workgroupBarrier();

    // Tree reduction with sequential addressing
    for (var stride = WORKGROUP_SIZE / 2u; stride > 0u; stride >>= 1u) {
        if (tid < stride) {
            shared_data[tid] += shared_data[tid + stride];
        }
        workgroupBarrier();
    }

    // Thread 0 writes the partial sum
    if (tid == 0u) {
        output_values[workgroup_id.x] = shared_data[0];
    }
}

// =============================================================================
// FINAL REDUCTION PASS
// =============================================================================

/// Phase 2: Reduce partial sums to final total
/// Reads from input_values (the partial sums from phase 1)
/// Writes result to output_values[0]
@compute @workgroup_size(256)
fn final_reduce(
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let tid = local_id.x;
    let element_count = params.num_elements;

    // Load partial sum from input (or 0 if out of bounds)
    var sum: f32 = 0.0;
    if (tid < element_count) {
        sum = input_values[tid];
    }

    // Handle case where we have more partial sums than workgroup size
    var idx = tid + WORKGROUP_SIZE;
    while (idx < element_count) {
        sum += input_values[idx];
        idx += WORKGROUP_SIZE;
    }

    shared_data[tid] = sum;
    workgroupBarrier();

    // Tree reduction
    for (var stride = WORKGROUP_SIZE / 2u; stride > 0u; stride >>= 1u) {
        if (tid < stride) {
            shared_data[tid] += shared_data[tid + stride];
        }
        workgroupBarrier();
    }

    // Write final result to output[0]
    if (tid == 0u) {
        output_values[0] = shared_data[0];
    }
}
