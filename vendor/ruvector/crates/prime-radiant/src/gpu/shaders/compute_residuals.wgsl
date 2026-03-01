// =============================================================================
// Prime-Radiant GPU Compute Shaders - Residual Computation
// =============================================================================
//
// Computes sheaf Laplacian residuals: r_e = rho_source(x_source) - rho_target(x_target)
// and per-edge energy: E_e = w_e * ||r_e||^2
//
// Each thread processes one edge, computing the residual and squared norm.

// =============================================================================
// TYPE DEFINITIONS (must match Rust structs exactly)
// =============================================================================

struct GpuParams {
    num_edges: u32,
    num_nodes: u32,
    state_dim: u32,
    beta: f32,
    threshold_lane0: f32,
    threshold_lane1: f32,
    threshold_lane2: f32,
    store_residuals: u32,  // 0 = skip storage (energy only), 1 = store residuals
}

struct GpuEdge {
    source_idx: u32,
    target_idx: u32,
    weight: f32,
    rho_source_idx: u32,
    rho_target_idx: u32,
    comparison_dim: u32,
    _padding0: u32,
    _padding1: u32,
}

struct GpuRestrictionMap {
    map_type: u32,       // 0=identity, 1=diagonal, 2=projection, 3=dense
    input_dim: u32,
    output_dim: u32,
    data_offset: u32,
    data_len: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

const WORKGROUP_SIZE: u32 = 256u;
const MAP_IDENTITY: u32 = 0u;
const MAP_DIAGONAL: u32 = 1u;
const MAP_PROJECTION: u32 = 2u;
const MAP_DENSE: u32 = 3u;

// =============================================================================
// BUFFER BINDINGS (matches Rust kernel bind group layout)
// =============================================================================
// binding 0: params (uniform)
// binding 1: node_states (storage, read)
// binding 2: edges (storage, read)
// binding 3: restriction_maps (storage, read)
// binding 4: restriction_data (storage, read)
// binding 5: residuals (storage, read_write)
// binding 6: energies (storage, read_write)

@group(0) @binding(0) var<uniform> params: GpuParams;
@group(0) @binding(1) var<storage, read> node_states: array<f32>;
@group(0) @binding(2) var<storage, read> edges: array<GpuEdge>;
@group(0) @binding(3) var<storage, read> restriction_maps: array<GpuRestrictionMap>;
@group(0) @binding(4) var<storage, read> restriction_data: array<f32>;
@group(0) @binding(5) var<storage, read_write> residuals: array<f32>;
@group(0) @binding(6) var<storage, read_write> energies: array<f32>;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Apply restriction map to a state vector at the given offset
/// Returns the projected value at output dimension d
fn apply_restriction(
    rho: GpuRestrictionMap,
    state_base: u32,
    output_dim: u32
) -> f32 {
    switch(rho.map_type) {
        case MAP_IDENTITY: {
            // Identity: just return the corresponding element
            if (output_dim < rho.output_dim && output_dim < params.state_dim) {
                return node_states[state_base + output_dim];
            }
            return 0.0;
        }
        case MAP_DIAGONAL: {
            // Diagonal: scale by diagonal element
            if (output_dim < rho.data_len) {
                let scale = restriction_data[rho.data_offset + output_dim];
                return node_states[state_base + output_dim] * scale;
            }
            return 0.0;
        }
        case MAP_PROJECTION: {
            // Projection: select specific indices
            if (output_dim < rho.data_len) {
                let idx = u32(restriction_data[rho.data_offset + output_dim]);
                if (idx < params.state_dim) {
                    return node_states[state_base + idx];
                }
            }
            return 0.0;
        }
        case MAP_DENSE, default: {
            // Dense: matrix-vector multiply for row output_dim
            var result: f32 = 0.0;
            let row_offset = rho.data_offset + output_dim * rho.input_dim;
            for (var i = 0u; i < rho.input_dim && i < params.state_dim; i++) {
                result += restriction_data[row_offset + i] * node_states[state_base + i];
            }
            return result;
        }
    }
    return 0.0;
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let edge_idx = global_id.x;

    // Bounds check
    if (edge_idx >= params.num_edges) {
        return;
    }

    // Get edge data
    let edge = edges[edge_idx];

    // Compute base offsets for source and target node states
    let source_base = edge.source_idx * params.state_dim;
    let target_base = edge.target_idx * params.state_dim;

    // Get restriction maps
    let rho_source = restriction_maps[edge.rho_source_idx];
    let rho_target = restriction_maps[edge.rho_target_idx];

    // Compute residual: r = rho_source(x_source) - rho_target(x_target)
    // and accumulate squared norm
    //
    // OPTIMIZATION: Process 4 dimensions at a time using vec4 operations.
    // This leverages GPU SIMD capabilities for ~4x throughput on high-dimensional
    // state vectors. The dot(v, v) operation is particularly efficient on GPU.
    var norm_sq: f32 = 0.0;
    let comparison_dim = edge.comparison_dim;
    let residual_base = edge_idx * comparison_dim;

    // Calculate how many full vec4 iterations and remainder
    let vec4_count = comparison_dim / 4u;
    let remainder = comparison_dim % 4u;

    // Process 4 dimensions at a time
    var d = 0u;
    for (var i = 0u; i < vec4_count; i++) {
        // Load 4 source values via restriction maps
        let source_vec = vec4<f32>(
            apply_restriction(rho_source, source_base, d),
            apply_restriction(rho_source, source_base, d + 1u),
            apply_restriction(rho_source, source_base, d + 2u),
            apply_restriction(rho_source, source_base, d + 3u)
        );

        // Load 4 target values via restriction maps
        let target_vec = vec4<f32>(
            apply_restriction(rho_target, target_base, d),
            apply_restriction(rho_target, target_base, d + 1u),
            apply_restriction(rho_target, target_base, d + 2u),
            apply_restriction(rho_target, target_base, d + 3u)
        );

        // Compute residual vector (4 components at once)
        let r_vec = source_vec - target_vec;

        // Accumulate norm using dot product (very efficient on GPU - single instruction)
        norm_sq += dot(r_vec, r_vec);

        // Store residuals if requested (optional for energy-only computation)
        if (params.store_residuals != 0u) {
            let base_offset = residual_base + d;
            if (base_offset + 3u < arrayLength(&residuals)) {
                residuals[base_offset] = r_vec.x;
                residuals[base_offset + 1u] = r_vec.y;
                residuals[base_offset + 2u] = r_vec.z;
                residuals[base_offset + 3u] = r_vec.w;
            }
        }

        d += 4u;
    }

    // Handle remainder dimensions (0-3 elements)
    for (var j = 0u; j < remainder; j++) {
        let dim_idx = d + j;
        let projected_source = apply_restriction(rho_source, source_base, dim_idx);
        let projected_target = apply_restriction(rho_target, target_base, dim_idx);
        let r = projected_source - projected_target;

        norm_sq += r * r;

        if (params.store_residuals != 0u) {
            let offset = residual_base + dim_idx;
            if (offset < arrayLength(&residuals)) {
                residuals[offset] = r;
            }
        }
    }

    // Compute weighted energy: E_e = w_e * ||r_e||^2
    let energy = edge.weight * norm_sq;

    // Store per-edge energy
    energies[edge_idx] = energy;
}
