// =============================================================================
// Prime-Radiant GPU Compute Shaders - Sheaf Attention
// =============================================================================
//
// Energy-based sheaf attention: A_ij = softmax(-beta * E_ij)
//
// Attention weights are computed from coherence energy:
// - Low energy (coherent) edges get high attention
// - High energy (incoherent) edges get low attention

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

struct AttentionParams {
    num_edges: u32,
    num_nodes: u32,
    beta: f32,
    energy_threshold: f32,
    use_sparse: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

struct EdgeDescriptor {
    source_idx: u32,
    target_idx: u32,
    weight: f32,
    _padding: u32,
}

const WORKGROUP_SIZE: u32 = 256u;
const NEG_INF: f32 = -3.402823e+38;
const EPSILON: f32 = 1e-8;

// =============================================================================
// BUFFER BINDINGS
// =============================================================================
// Layout matches Rust kernel bind group:
// binding 0: params (uniform)
// binding 1: edges (storage, read)
// binding 2: edge_energies (storage, read)
// binding 3: attention_weights (storage, read_write)
// binding 4: node_exp_sums (storage, read_write)

/// Attention parameters
@group(0) @binding(0) var<uniform> params: AttentionParams;

/// Edge descriptors
@group(0) @binding(1) var<storage, read> edges: array<EdgeDescriptor>;

/// Edge energies from residual computation
@group(0) @binding(2) var<storage, read> edge_energies: array<f32>;

/// Output attention weights (one per edge)
@group(0) @binding(3) var<storage, read_write> attention_weights: array<f32>;

/// Per-node exponential sums for normalization
@group(0) @binding(4) var<storage, read_write> node_exp_sums: array<f32>;

// =============================================================================
// SHARED MEMORY
// =============================================================================

/// Shared memory for parallel reduction
var<workgroup> shared_data: array<f32, 256>;

// =============================================================================
// SINGLE-PASS ATTENTION COMPUTATION
// =============================================================================

/// Compute attention weights from edge energies
/// A_e = exp(-beta * E_e) (unnormalized)
/// Each workgroup processes multiple edges
@compute @workgroup_size(256)
fn compute_attention_single_pass(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let edge_idx = global_id.x;
    let num_edges = params.num_edges;
    let beta = params.beta;

    if (edge_idx >= num_edges) {
        return;
    }

    // Get edge energy
    let energy = edge_energies[edge_idx];

    // Compute unnormalized attention weight
    // For energy-based attention: A = exp(-beta * E)
    // High energy (incoherent) -> low attention
    // Low energy (coherent) -> high attention
    var score = -beta * energy;

    // Apply energy threshold masking for sparse attention
    if (params.use_sparse == 1u && energy > params.energy_threshold) {
        score = NEG_INF;
    }

    // Compute exp(score) - clamp to avoid overflow
    let clamped_score = clamp(score, -80.0, 80.0);
    let exp_score = exp(clamped_score);

    // Store unnormalized attention weight
    attention_weights[edge_idx] = exp_score;

    // Accumulate exp sum for source node (for later normalization)
    // Note: This requires atomic operations for correctness in parallel
    // For now, we store unnormalized weights; normalization done in separate pass
    let edge = edges[edge_idx];
    // atomicAdd(&node_exp_sums[edge.source_idx], exp_score);
    // Note: WGSL doesn't have atomicAdd for f32, so we store for CPU normalization
}

// =============================================================================
// NORMALIZATION PASS
// =============================================================================

/// Normalize attention weights by node (outgoing edges sum to 1)
/// Second pass after exp sums are computed
@compute @workgroup_size(256)
fn normalize_attention(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let edge_idx = global_id.x;
    let num_edges = params.num_edges;

    if (edge_idx >= num_edges) {
        return;
    }

    let edge = edges[edge_idx];
    let source_idx = edge.source_idx;

    // Get the sum of exp scores for this source node
    let exp_sum = node_exp_sums[source_idx];

    // Normalize
    let normalized = attention_weights[edge_idx] / max(exp_sum, EPSILON);
    attention_weights[edge_idx] = normalized;
}
