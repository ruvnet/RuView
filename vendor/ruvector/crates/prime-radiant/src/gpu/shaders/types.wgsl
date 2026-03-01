// =============================================================================
// Prime-Radiant GPU Compute Shaders - Shared Types
// =============================================================================
//
// This file contains shared struct definitions and constants used across
// all compute shaders in the Prime-Radiant coherence engine.
//
// Memory Layout:
// - All structs are aligned to 16 bytes for optimal GPU memory access
// - vec4<f32> is used where possible for coalesced memory operations
// - Padding fields ensure proper alignment

// =============================================================================
// COMPUTE PARAMETERS
// =============================================================================

/// Parameters for residual computation
struct ComputeParams {
    /// Total number of edges to process
    edge_count: u32,
    /// Dimension of state vectors
    state_dim: u32,
    /// Restriction map type: 0=identity, 1=diagonal, 2=dense, 3=projection, 4=sparse
    restriction_type: u32,
    /// Padding for 16-byte alignment
    padding: u32,
}

/// Parameters for parallel reduction operations
struct ReductionParams {
    /// Number of elements to reduce
    element_count: u32,
    /// Stride between elements (for strided access patterns)
    stride: u32,
    /// Whether this is the final reduction pass
    is_final_pass: u32,
    /// Output offset for multi-pass reductions
    output_offset: u32,
}

/// Parameters for attention computation
struct AttentionParams {
    /// Batch size (number of independent attention operations)
    batch_size: u32,
    /// Sequence length (number of tokens/nodes)
    seq_len: u32,
    /// Dimension per attention head
    head_dim: u32,
    /// Inverse temperature parameter: A_ij = softmax(-beta * E_ij)
    beta: f32,
    /// Number of attention heads (for multi-head attention)
    num_heads: u32,
    /// Whether to use causal masking
    use_causal_mask: u32,
    /// Energy threshold for sparse attention (skip if E > threshold)
    energy_threshold: f32,
    /// Padding for 16-byte alignment
    padding: u32,
}

/// Parameters for token routing
struct RoutingParams {
    /// Number of tokens to route
    token_count: u32,
    /// Number of lanes/experts
    num_lanes: u32,
    /// Whether to use load balancing
    use_load_balance: u32,
    /// Top-k selection for MoE
    top_k: u32,
}

/// Parameters for sparse mask generation
struct SparseMaskParams {
    /// Total number of potential edges
    total_edges: u32,
    /// Energy threshold for coherence (keep edges below this)
    coherence_threshold: f32,
    /// Maximum edges to keep (for memory bounds)
    max_edges: u32,
    /// Output format: 0=indices, 1=dense mask
    output_format: u32,
}

// =============================================================================
// EDGE AND NODE DATA STRUCTURES
// =============================================================================

/// Edge descriptor for graph connectivity (16-byte aligned)
struct EdgeDescriptor {
    /// Index of source node
    source_idx: u32,
    /// Index of target node
    target_idx: u32,
    /// Offset into restriction data for this edge
    restriction_offset: u32,
    /// Weight for this edge
    weight: f32,
}

/// Node state with metadata (16-byte aligned)
struct NodeState {
    /// Offset into state buffer where this node's state begins
    state_offset: u32,
    /// Dimension of this node's state
    state_dim: u32,
    /// Scope ID for hierarchical energy aggregation
    scope_id: u32,
    /// Flags (bit 0: is_boundary, bit 1: is_fixed, etc.)
    flags: u32,
}

/// Per-edge energy result (16-byte aligned)
struct EdgeEnergy {
    /// Weighted energy: w_e * |r_e|^2
    energy: f32,
    /// Raw residual norm squared: |r_e|^2
    residual_norm_sq: f32,
    /// Edge weight that was applied
    weight: f32,
    /// Padding for alignment
    padding: f32,
}

// =============================================================================
// ATTENTION STRUCTURES
// =============================================================================

/// Attention score for a single edge (16-byte aligned)
struct AttentionScore {
    /// Source node index
    source: u32,
    /// Target node index
    target: u32,
    /// Attention weight (after softmax)
    weight: f32,
    /// Raw score (before softmax)
    raw_score: f32,
}

/// Lane assignment result for token routing (16-byte aligned)
struct LaneAssignment {
    /// Token index
    token_idx: u32,
    /// Assigned lane (0-3 typically)
    lane: u32,
    /// Confidence score for this assignment
    confidence: f32,
    /// Energy value that determined routing
    energy: f32,
}

// =============================================================================
// CONSTANTS
// =============================================================================

/// Workgroup size for 1D dispatches
const WORKGROUP_SIZE_1D: u32 = 256u;

/// Workgroup dimensions for 2D dispatches (attention)
const WORKGROUP_SIZE_2D_X: u32 = 16u;
const WORKGROUP_SIZE_2D_Y: u32 = 16u;

/// Maximum supported state dimension (for stack allocation)
const MAX_STATE_DIM: u32 = 512u;

/// Epsilon for numerical stability
const EPSILON: f32 = 1e-8;

/// Negative infinity for softmax initialization
const NEG_INF: f32 = -3.402823e+38;

/// Restriction map type constants
const RESTRICTION_IDENTITY: u32 = 0u;
const RESTRICTION_DIAGONAL: u32 = 1u;
const RESTRICTION_DENSE: u32 = 2u;
const RESTRICTION_PROJECTION: u32 = 3u;
const RESTRICTION_SPARSE: u32 = 4u;

/// Lane thresholds for token routing (default values)
/// Lane 0: energy < 0.1 (coherent, fast path)
/// Lane 1: 0.1 <= energy < 0.5 (semi-coherent, normal path)
/// Lane 2: 0.5 <= energy < 1.0 (incoherent, slow path)
/// Lane 3: energy >= 1.0 (critical, special handling)
const DEFAULT_LANE_THRESHOLDS: vec4<f32> = vec4<f32>(0.1, 0.5, 1.0, 10.0);

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Compute squared L2 norm of a vec4
fn norm_sq_vec4(v: vec4<f32>) -> f32 {
    return dot(v, v);
}

/// Safe division with epsilon
fn safe_div(a: f32, b: f32) -> f32 {
    return a / max(b, EPSILON);
}

/// Branchless step function
fn step_branchless(threshold: f32, value: f32) -> f32 {
    return select(0.0, 1.0, value >= threshold);
}

/// Compute lane index from energy using branchless comparison
fn compute_lane(energy: f32, thresholds: vec4<f32>) -> u32 {
    return u32(step_branchless(thresholds.x, energy))
         + u32(step_branchless(thresholds.y, energy))
         + u32(step_branchless(thresholds.z, energy));
}

/// Online softmax helper - update max and sum
fn online_softmax_update(
    old_max: f32,
    old_sum: f32,
    new_val: f32
) -> vec2<f32> {
    let new_max = max(old_max, new_val);
    let correction = exp(old_max - new_max);
    let new_sum = old_sum * correction + exp(new_val - new_max);
    return vec2<f32>(new_max, new_sum);
}

/// Fast approximate exp for softmax (when precision is less critical)
fn fast_exp(x: f32) -> f32 {
    // Use native exp for now; can be replaced with polynomial approximation
    return exp(x);
}

/// Clamp value to valid range
fn clamp_f32(val: f32, min_val: f32, max_val: f32) -> f32 {
    return max(min_val, min(max_val, val));
}
