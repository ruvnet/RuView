// =============================================================================
// Prime-Radiant GPU Compute Shaders - Token Routing
// =============================================================================
//
// Parallel lane assignment for tokens based on coherence energy thresholds.
// Routes tokens to different processing lanes (experts) based on their
// local coherence energy, enabling adaptive computation.
//
// Lane Semantics:
// - Lane 0: Coherent (energy < tau_0) - Fast path, minimal processing
// - Lane 1: Semi-coherent (tau_0 <= energy < tau_1) - Normal processing
// - Lane 2: Incoherent (tau_1 <= energy < tau_2) - Enhanced processing
// - Lane 3: Critical (energy >= tau_2) - Special handling required

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

struct RoutingParams {
    num_tokens: u32,
    num_nodes: u32,
    threshold_0: f32,
    threshold_1: f32,
    threshold_2: f32,
    high_energy_threshold: f32,
    _padding0: u32,
    _padding1: u32,
}

struct Token {
    token_id: u32,
    node_idx: u32,
    action_type: u32,
    priority: f32,
}

struct RoutingDecision {
    token_id: u32,
    assigned_lane: u32,
    local_energy: f32,
    confidence: f32,
    escalation_reason: u32,
    num_high_energy_edges: u32,
    max_edge_energy: f32,
    _padding: u32,
}

struct LaneStats {
    lane_counts: vec4<u32>,
    total_energy_per_lane: vec4<f32>,
    _padding: array<u32, 8>,
}

const WORKGROUP_SIZE: u32 = 256u;
const NUM_LANES: u32 = 4u;

// =============================================================================
// BUFFER BINDINGS
// =============================================================================
// Layout matches Rust kernel bind group:
// binding 0: params (uniform)
// binding 1: tokens (storage, read)
// binding 2: local_energies (storage, read)
// binding 3: edge_energies (storage, read)
// binding 4: node_edge_counts (storage, read)
// binding 5: node_edge_offsets (storage, read)
// binding 6: node_edges (storage, read)
// binding 7: routing_decisions (storage, read_write)
// binding 8: lane_stats (storage, read_write)

/// Routing parameters
@group(0) @binding(0) var<uniform> params: RoutingParams;

/// Input tokens
@group(0) @binding(1) var<storage, read> tokens: array<Token>;

/// Pre-computed local energies per node
@group(0) @binding(2) var<storage, read> local_energies: array<f32>;

/// All edge energies
@group(0) @binding(3) var<storage, read> edge_energies: array<f32>;

/// Number of edges per node (CSR format)
@group(0) @binding(4) var<storage, read> node_edge_counts: array<u32>;

/// Edge start offsets per node (CSR format)
@group(0) @binding(5) var<storage, read> node_edge_offsets: array<u32>;

/// Edge indices per node (CSR format)
@group(0) @binding(6) var<storage, read> node_edges: array<u32>;

/// Output routing decisions
@group(0) @binding(7) var<storage, read_write> routing_decisions: array<RoutingDecision>;

/// Output lane statistics
@group(0) @binding(8) var<storage, read_write> lane_stats: LaneStats;

// =============================================================================
// SHARED MEMORY
// =============================================================================

/// Lane counts for workgroup-level reduction
var<workgroup> shared_lane_counts: array<atomic<u32>, 4>;

/// Lane energy sums for workgroup-level reduction
var<workgroup> shared_lane_energies: array<f32, 4>;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Branchless lane computation using step functions
fn compute_lane_branchless(energy: f32, t0: f32, t1: f32, t2: f32) -> u32 {
    let s0 = select(0u, 1u, energy >= t0);
    let s1 = select(0u, 1u, energy >= t1);
    let s2 = select(0u, 1u, energy >= t2);
    return s0 + s1 + s2;
}

/// Compute routing confidence based on how close energy is to threshold boundaries
fn compute_confidence(energy: f32, lane: u32, t0: f32, t1: f32, t2: f32) -> f32 {
    // Confidence is based on distance from nearest threshold
    var dist_to_threshold: f32;

    switch(lane) {
        case 0u: {
            dist_to_threshold = t0 - energy;
        }
        case 1u: {
            dist_to_threshold = min(energy - t0, t1 - energy);
        }
        case 2u: {
            dist_to_threshold = min(energy - t1, t2 - energy);
        }
        case 3u, default: {
            dist_to_threshold = energy - t2;
        }
    }

    // Normalize to [0, 1] - higher means further from boundary
    return clamp(dist_to_threshold * 10.0, 0.0, 1.0);
}

// =============================================================================
// MAIN ROUTING KERNEL
// =============================================================================

/// Route tokens to processing lanes based on local coherence energy
@compute @workgroup_size(256)
fn route_tokens(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>
) {
    let token_idx = global_id.x;
    let local_idx = local_id.x;
    let num_tokens = params.num_tokens;

    // Initialize shared counters (first thread only)
    if (local_idx == 0u) {
        atomicStore(&shared_lane_counts[0], 0u);
        atomicStore(&shared_lane_counts[1], 0u);
        atomicStore(&shared_lane_counts[2], 0u);
        atomicStore(&shared_lane_counts[3], 0u);
        shared_lane_energies[0] = 0.0;
        shared_lane_energies[1] = 0.0;
        shared_lane_energies[2] = 0.0;
        shared_lane_energies[3] = 0.0;
    }
    workgroupBarrier();

    if (token_idx >= num_tokens) {
        return;
    }

    let token = tokens[token_idx];
    let node_idx = token.node_idx;

    // Get local energy for this node
    let local_energy = local_energies[node_idx];

    // Compute lane assignment
    let lane = compute_lane_branchless(
        local_energy,
        params.threshold_0,
        params.threshold_1,
        params.threshold_2
    );

    // Compute confidence
    let confidence = compute_confidence(
        local_energy,
        lane,
        params.threshold_0,
        params.threshold_1,
        params.threshold_2
    );

    // Analyze edges for this node
    let edge_count = node_edge_counts[node_idx];
    let edge_offset = node_edge_offsets[node_idx];

    var num_high_energy_edges: u32 = 0u;
    var max_edge_energy: f32 = 0.0;
    var escalation_reason: u32 = 0u;

    for (var i = 0u; i < edge_count; i++) {
        let edge_idx = node_edges[edge_offset + i];
        let edge_energy = edge_energies[edge_idx];

        if (edge_energy > params.high_energy_threshold) {
            num_high_energy_edges += 1u;
        }
        max_edge_energy = max(max_edge_energy, edge_energy);
    }

    // Determine if escalation is needed
    if (num_high_energy_edges > 2u) {
        escalation_reason = 1u; // Multiple high-energy edges
    } else if (max_edge_energy > params.threshold_2) {
        escalation_reason = 2u; // Single very high energy edge
    }

    // Write routing decision
    var decision: RoutingDecision;
    decision.token_id = token.token_id;
    decision.assigned_lane = lane;
    decision.local_energy = local_energy;
    decision.confidence = confidence;
    decision.escalation_reason = escalation_reason;
    decision.num_high_energy_edges = num_high_energy_edges;
    decision.max_edge_energy = max_edge_energy;
    decision._padding = 0u;

    routing_decisions[token_idx] = decision;

    // Update lane statistics
    atomicAdd(&shared_lane_counts[lane], 1u);
    // Note: No atomic f32 add in WGSL, would need separate reduction pass

    workgroupBarrier();

    // First thread writes workgroup stats to global buffer
    // (In production, would do proper atomic accumulation)
    if (local_idx == 0u && workgroup_id.x == 0u) {
        lane_stats.lane_counts = vec4<u32>(
            atomicLoad(&shared_lane_counts[0]),
            atomicLoad(&shared_lane_counts[1]),
            atomicLoad(&shared_lane_counts[2]),
            atomicLoad(&shared_lane_counts[3])
        );
    }
}
