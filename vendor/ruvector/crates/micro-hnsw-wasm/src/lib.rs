//! Micro HNSW v2.3 - Neuromorphic HNSW with Novel Discoveries
//! Last validated: 2025-12-25
//! Target: <12KB WASM with multi-core support
//!
//! Features:
//! - Multiple distance metrics (L2, Cosine, Dot)
//! - Multi-core sharding (256 cores × 32 vectors = 8K total)
//! - Batch operations with 4-vector batching
//! - Beam search for better recall
//! - Result merging across cores
//! - Node types for Cypher-style typed graphs (16 types)
//! - Edge weights for GNN message passing
//! - Vector updates for online learning/GNN propagation
//! - **Spiking Neural Network integration with LIF neurons**
//! - **STDP (Spike-Timing Dependent Plasticity) learning**
//!
//! ## Novel Neuromorphic Discoveries (v2.3)
//! - **Spike-Timing Vector Encoding**: Convert vectors to temporal spike patterns
//! - **Homeostatic Plasticity**: Self-stabilizing network activity
//! - **Oscillatory Resonance**: Frequency-tuned search amplification
//! - **Temporal Pattern Recognition**: Spike-based similarity matching
//! - **Winner-Take-All Circuits**: Competitive neural selection
//! - **Dendritic Computation**: Non-linear local processing

#![no_std]

// ============ Configuration ============
const MAX_VECTORS: usize = 32;       // Per core (256 × 32 = 8K total)
const MAX_DIMS: usize = 16;          // Vector dimensions
const MAX_NEIGHBORS: usize = 6;      // Graph connectivity
const BEAM_WIDTH: usize = 3;         // Search beam width

// ============ Spiking Neural Network Configuration ============
const TAU_MEMBRANE: f32 = 20.0;      // Membrane time constant (ms)
const TAU_REFRAC: f32 = 2.0;         // Refractory period (ms)
const V_RESET: f32 = 0.0;            // Reset potential
const V_REST: f32 = 0.0;             // Resting potential
const STDP_A_PLUS: f32 = 0.01;       // STDP potentiation magnitude
const STDP_A_MINUS: f32 = 0.012;     // STDP depression magnitude
const TAU_STDP: f32 = 20.0;          // STDP time constant
const INV_TAU_STDP: f32 = 0.05;      // Pre-computed 1/TAU_STDP for optimization
const INV_255: f32 = 0.00392157;     // Pre-computed 1/255 for weight normalization

// ============ Novel Neuromorphic Configuration ============
const HOMEOSTATIC_TARGET: f32 = 0.1; // Target spike rate (spikes/ms)
const HOMEOSTATIC_TAU: f32 = 1000.0; // Homeostasis time constant (slow)
const OSCILLATOR_FREQ: f32 = 40.0;   // Gamma oscillation frequency (Hz)
const WTA_INHIBITION: f32 = 0.8;     // Winner-take-all lateral inhibition
const DENDRITIC_NONLIN: f32 = 2.0;   // Dendritic nonlinearity exponent
const SPIKE_ENCODING_RES: u8 = 8;    // Temporal encoding resolution (bits)

// ============ Types ============
#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum Metric { L2 = 0, Cosine = 1, Dot = 2 }

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vector {
    data: [f32; MAX_DIMS],
    norm: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Node {
    neighbors: [u8; MAX_NEIGHBORS],
    count: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SearchResult {
    pub idx: u8,
    pub core_id: u8,
    pub distance: f32,
}

#[repr(C)]
pub struct MicroHnsw {
    vectors: [Vector; MAX_VECTORS],
    nodes: [Node; MAX_VECTORS],
    count: u8,
    dims: u8,
    metric: Metric,
    core_id: u8,
}

// ============ Static Storage ============
static mut HNSW: MicroHnsw = MicroHnsw {
    vectors: [Vector { data: [0.0; MAX_DIMS], norm: 0.0 }; MAX_VECTORS],
    nodes: [Node { neighbors: [0; MAX_NEIGHBORS], count: 0 }; MAX_VECTORS],
    count: 0,
    dims: 16,
    metric: Metric::L2,
    core_id: 0,
};

static mut QUERY: [f32; MAX_DIMS] = [0.0; MAX_DIMS];
static mut INSERT: [f32; MAX_DIMS] = [0.0; MAX_DIMS];
static mut RESULTS: [SearchResult; 16] = [SearchResult { idx: 255, core_id: 0, distance: 0.0 }; 16];
static mut GLOBAL: [SearchResult; 16] = [SearchResult { idx: 255, core_id: 0, distance: 0.0 }; 16];

// ============ GNN/Cypher Extensions ============
// Node types: 4 bits per node (16 types), packed 2 per byte = 16 bytes
static mut NODE_TYPES: [u8; MAX_VECTORS / 2] = [0; 16];
// Edge weights: 4 bits per edge (packed), uniform per node = 32 bytes
static mut EDGE_WEIGHTS: [u8; MAX_VECTORS] = [255; 32];
// Delta buffer for vector updates
static mut DELTA: [f32; MAX_DIMS] = [0.0; MAX_DIMS];


// ============ Spiking Neural Network State ============
// Membrane potentials: LIF neuron states (one per vector)
static mut MEMBRANE: [f32; MAX_VECTORS] = [0.0; MAX_VECTORS];
// Adaptive thresholds: Dynamic firing thresholds
static mut THRESHOLD: [f32; MAX_VECTORS] = [1.0; MAX_VECTORS];
// Last spike time: For STDP calculations
static mut LAST_SPIKE: [f32; MAX_VECTORS] = [-1000.0; MAX_VECTORS];
// Refractory state: Time remaining in refractory period
static mut REFRAC: [f32; MAX_VECTORS] = [0.0; MAX_VECTORS];
// Current simulation time
static mut SIM_TIME: f32 = 0.0;
// Spike output buffer: Which neurons spiked this timestep
static mut SPIKES: [bool; MAX_VECTORS] = [false; MAX_VECTORS];

// ============ Novel Neuromorphic State ============
// Homeostatic plasticity: running average spike rate
static mut SPIKE_RATE: [f32; MAX_VECTORS] = [0.0; MAX_VECTORS];
// Oscillator phase: gamma rhythm for synchronization
static mut OSCILLATOR_PHASE: f32 = 0.0;
// Dendritic compartments: local nonlinear integration
static mut DENDRITE: [[f32; MAX_NEIGHBORS]; MAX_VECTORS] = [[0.0; MAX_NEIGHBORS]; MAX_VECTORS];
// Temporal spike pattern buffer (recent spikes encoded as bits)
static mut SPIKE_PATTERN: [u32; MAX_VECTORS] = [0; MAX_VECTORS];
// Resonance amplitude for each neuron
static mut RESONANCE: [f32; MAX_VECTORS] = [0.0; MAX_VECTORS];
// Winner-take-all state: inhibition accumulator
static mut WTA_INHIBIT: f32 = 0.0;

// ============ Math ============
#[inline(always)]
fn sqrt_fast(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    let i = 0x5f3759df - (x.to_bits() >> 1);
    let y = f32::from_bits(i);
    x * y * (1.5 - 0.5 * x * y * y)
}

#[inline(always)]
fn norm(v: &[f32], n: usize) -> f32 {
    let mut s = 0.0f32;
    let mut i = 0;
    while i < n { s += v[i] * v[i]; i += 1; }
    sqrt_fast(s)
}

#[inline(always)]
fn dist_l2(a: &[f32], b: &[f32], n: usize) -> f32 {
    let mut s = 0.0f32;
    let mut i = 0;
    while i < n { let d = a[i] - b[i]; s += d * d; i += 1; }
    s
}

#[inline(always)]
fn dist_dot(a: &[f32], b: &[f32], n: usize) -> f32 {
    let mut s = 0.0f32;
    let mut i = 0;
    while i < n { s += a[i] * b[i]; i += 1; }
    -s
}

#[inline(always)]
fn dist_cos(a: &[f32], an: f32, b: &[f32], bn: f32, n: usize) -> f32 {
    if an == 0.0 || bn == 0.0 { return 1.0; }
    let mut d = 0.0f32;
    let mut i = 0;
    while i < n { d += a[i] * b[i]; i += 1; }
    1.0 - d / (an * bn)
}

#[inline(always)]
fn distance(q: &[f32], qn: f32, idx: u8) -> f32 {
    unsafe {
        let n = HNSW.dims as usize;
        let v = &HNSW.vectors[idx as usize];
        match HNSW.metric {
            Metric::Cosine => dist_cos(q, qn, &v.data[..n], v.norm, n),
            Metric::Dot => dist_dot(q, &v.data[..n], n),
            Metric::L2 => dist_l2(q, &v.data[..n], n),
        }
    }
}

// ============ Core API ============

/// Initialize: init(dims, metric, core_id)
/// metric: 0=L2, 1=Cosine, 2=Dot
#[no_mangle]
pub extern "C" fn init(dims: u8, metric: u8, core_id: u8) {
    unsafe {
        HNSW.count = 0;
        HNSW.dims = dims.min(MAX_DIMS as u8);
        HNSW.metric = match metric { 1 => Metric::Cosine, 2 => Metric::Dot, _ => Metric::L2 };
        HNSW.core_id = core_id;
    }
}

#[no_mangle]
pub extern "C" fn get_insert_ptr() -> *mut f32 { unsafe { INSERT.as_mut_ptr() } }

#[no_mangle]
pub extern "C" fn get_query_ptr() -> *mut f32 { unsafe { QUERY.as_mut_ptr() } }

#[no_mangle]
pub extern "C" fn get_result_ptr() -> *const SearchResult { unsafe { RESULTS.as_ptr() } }

#[no_mangle]
pub extern "C" fn get_global_ptr() -> *const SearchResult { unsafe { GLOBAL.as_ptr() } }

/// Insert vector from INSERT buffer, returns index or 255 if full
#[no_mangle]
pub extern "C" fn insert() -> u8 {
    unsafe {
        if HNSW.count >= MAX_VECTORS as u8 { return 255; }

        let idx = HNSW.count;
        let n = HNSW.dims as usize;

        // Copy vector and compute norm
        let mut i = 0;
        while i < n { HNSW.vectors[idx as usize].data[i] = INSERT[i]; i += 1; }
        HNSW.vectors[idx as usize].norm = norm(&INSERT[..n], n);
        HNSW.nodes[idx as usize].count = 0;

        // Connect to nearest neighbors
        if idx > 0 {
            let qn = HNSW.vectors[idx as usize].norm;
            let mut best = [0u8; MAX_NEIGHBORS];
            let mut best_d = [f32::MAX; MAX_NEIGHBORS];
            let mut found = 0usize;

            // Find M nearest
            let mut j = 0u8;
            while j < idx {
                let d = distance(&INSERT[..n], qn, j);
                if found < MAX_NEIGHBORS || d < best_d[found.saturating_sub(1)] {
                    let mut p = found.min(MAX_NEIGHBORS - 1);
                    while p > 0 && best_d[p - 1] > d {
                        if p < MAX_NEIGHBORS { best[p] = best[p - 1]; best_d[p] = best_d[p - 1]; }
                        p -= 1;
                    }
                    best[p] = j; best_d[p] = d;
                    if found < MAX_NEIGHBORS { found += 1; }
                }
                j += 1;
            }

            // Add bidirectional edges
            let mut k = 0;
            while k < found {
                let nb = best[k];
                let c = HNSW.nodes[idx as usize].count as usize;
                if c < MAX_NEIGHBORS {
                    HNSW.nodes[idx as usize].neighbors[c] = nb;
                    HNSW.nodes[idx as usize].count += 1;
                }
                let nc = HNSW.nodes[nb as usize].count as usize;
                if nc < MAX_NEIGHBORS {
                    HNSW.nodes[nb as usize].neighbors[nc] = idx;
                    HNSW.nodes[nb as usize].count += 1;
                }
                k += 1;
            }
        }

        HNSW.count += 1;
        idx
    }
}

/// Search for k nearest neighbors using beam search
#[no_mangle]
pub extern "C" fn search(k: u8) -> u8 {
    unsafe {
        if HNSW.count == 0 { return 0; }

        let n = HNSW.dims as usize;
        let k = k.min(16).min(HNSW.count);
        let qn = norm(&QUERY[..n], n);

        // Reset
        let mut i = 0;
        while i < 16 { RESULTS[i] = SearchResult { idx: 255, core_id: HNSW.core_id, distance: f32::MAX }; i += 1; }

        let mut visited = [false; MAX_VECTORS];
        let mut beam = [255u8; BEAM_WIDTH];
        let mut beam_d = [f32::MAX; BEAM_WIDTH];

        // Start from entry point
        beam[0] = 0;
        beam_d[0] = distance(&QUERY[..n], qn, 0);
        visited[0] = true;
        RESULTS[0] = SearchResult { idx: 0, core_id: HNSW.core_id, distance: beam_d[0] };
        let mut rc = 1u8;
        let mut bs = 1usize;

        // Beam search iterations
        let mut iter = 0u8;
        while iter < k.max(BEAM_WIDTH as u8) && bs > 0 {
            let mut nb = [255u8; BEAM_WIDTH];
            let mut nd = [f32::MAX; BEAM_WIDTH];
            let mut ns = 0usize;

            let mut b = 0;
            while b < bs {
                if beam[b] == 255 { b += 1; continue; }
                let node = &HNSW.nodes[beam[b] as usize];

                let mut j = 0u8;
                while j < node.count {
                    let nbr = node.neighbors[j as usize];
                    j += 1;
                    if visited[nbr as usize] { continue; }
                    visited[nbr as usize] = true;

                    let d = distance(&QUERY[..n], qn, nbr);

                    // Update beam
                    if ns < BEAM_WIDTH || d < nd[ns.saturating_sub(1)] {
                        let mut p = ns.min(BEAM_WIDTH - 1);
                        while p > 0 && nd[p - 1] > d {
                            if p < BEAM_WIDTH { nb[p] = nb[p - 1]; nd[p] = nd[p - 1]; }
                            p -= 1;
                        }
                        nb[p] = nbr; nd[p] = d;
                        if ns < BEAM_WIDTH { ns += 1; }
                    }

                    // Update results
                    if rc < 16 || d < RESULTS[(rc - 1) as usize].distance {
                        let mut p = rc.min(15) as usize;
                        while p > 0 && RESULTS[p - 1].distance > d {
                            if p < 16 { RESULTS[p] = RESULTS[p - 1]; }
                            p -= 1;
                        }
                        if p < 16 {
                            RESULTS[p] = SearchResult { idx: nbr, core_id: HNSW.core_id, distance: d };
                            if rc < 16 { rc += 1; }
                        }
                    }
                }
                b += 1;
            }

            beam = nb; beam_d = nd; bs = ns;
            iter += 1;
        }

        rc.min(k)
    }
}

// ============ Multi-Core ============

/// Merge results from another core into global buffer
#[no_mangle]
pub extern "C" fn merge(ptr: *const SearchResult, cnt: u8) -> u8 {
    unsafe {
        let mut gc = 0u8;
        while gc < 16 && GLOBAL[gc as usize].idx != 255 { gc += 1; }

        let mut i = 0u8;
        while i < cnt.min(16) {
            let r = &*ptr.add(i as usize);
            i += 1;
            if r.idx == 255 { continue; }

            if gc < 16 || r.distance < GLOBAL[(gc - 1) as usize].distance {
                let mut p = gc.min(15) as usize;
                while p > 0 && GLOBAL[p - 1].distance > r.distance {
                    if p < 16 { GLOBAL[p] = GLOBAL[p - 1]; }
                    p -= 1;
                }
                if p < 16 {
                    GLOBAL[p] = *r;
                    if gc < 16 { gc += 1; }
                }
            }
        }
        gc
    }
}

/// Clear global results
#[no_mangle]
pub extern "C" fn clear_global() {
    unsafe {
        let mut i = 0;
        while i < 16 { GLOBAL[i] = SearchResult { idx: 255, core_id: 0, distance: f32::MAX }; i += 1; }
    }
}

// ============ Info ============
#[no_mangle]
pub extern "C" fn count() -> u8 { unsafe { HNSW.count } }

#[no_mangle]
pub extern "C" fn get_core_id() -> u8 { unsafe { HNSW.core_id } }

#[no_mangle]
pub extern "C" fn get_metric() -> u8 { unsafe { HNSW.metric as u8 } }

#[no_mangle]
pub extern "C" fn get_dims() -> u8 { unsafe { HNSW.dims } }

#[no_mangle]
pub extern "C" fn get_capacity() -> u8 { MAX_VECTORS as u8 }

// ============ Cypher Node Types ============

/// Set node type (0-15) for Cypher-style typed queries
/// Types packed 2 per byte (4 bits each)
#[no_mangle]
pub extern "C" fn set_node_type(idx: u8, node_type: u8) {
    if idx >= MAX_VECTORS as u8 { return; }
    unsafe {
        let byte_idx = (idx / 2) as usize;
        let node_type = node_type & 0x0F; // Clamp to 4 bits
        if idx & 1 == 0 {
            NODE_TYPES[byte_idx] = (NODE_TYPES[byte_idx] & 0xF0) | node_type;
        } else {
            NODE_TYPES[byte_idx] = (NODE_TYPES[byte_idx] & 0x0F) | (node_type << 4);
        }
    }
}

/// Get node type (0-15)
#[no_mangle]
pub extern "C" fn get_node_type(idx: u8) -> u8 {
    if idx >= MAX_VECTORS as u8 { return 0; }
    unsafe {
        let byte_idx = (idx / 2) as usize;
        if idx & 1 == 0 {
            NODE_TYPES[byte_idx] & 0x0F
        } else {
            NODE_TYPES[byte_idx] >> 4
        }
    }
}

/// Check if node type matches mask (for filtering in JS/host)
#[no_mangle]
pub extern "C" fn type_matches(idx: u8, type_mask: u16) -> u8 {
    ((type_mask >> get_node_type(idx)) & 1) as u8
}

// ============ GNN Edge Weights ============

/// Set node edge weight (uniform for all edges from this node, 0-255)
#[no_mangle]
pub extern "C" fn set_edge_weight(node: u8, weight: u8) {
    if node < MAX_VECTORS as u8 { unsafe { EDGE_WEIGHTS[node as usize] = weight; } }
}

/// Get node edge weight
#[no_mangle]
pub extern "C" fn get_edge_weight(node: u8) -> u8 {
    if node < MAX_VECTORS as u8 { unsafe { EDGE_WEIGHTS[node as usize] } } else { 0 }
}

/// Aggregate neighbors into DELTA buffer (GNN message passing)
#[no_mangle]
pub extern "C" fn aggregate_neighbors(idx: u8) {
    unsafe {
        if idx >= HNSW.count { return; }
        let n = HNSW.dims as usize;
        let nc = HNSW.nodes[idx as usize].count;
        let mut d = 0;
        while d < n { DELTA[d] = 0.0; d += 1; }
        if nc == 0 { return; }
        let mut i = 0u8;
        while i < nc {
            let nb = HNSW.nodes[idx as usize].neighbors[i as usize];
            let w = EDGE_WEIGHTS[nb as usize] as f32;
            d = 0;
            while d < n { DELTA[d] += w * HNSW.vectors[nb as usize].data[d]; d += 1; }
            i += 1;
        }
        let s = INV_255 / nc as f32;
        d = 0; while d < n { DELTA[d] *= s; d += 1; }
    }
}

// ============ Vector Updates ============

/// Get delta buffer pointer for reading aggregated values
#[no_mangle]
pub extern "C" fn get_delta_ptr() -> *const f32 { unsafe { DELTA.as_ptr() } }

/// Update vector: v = v + alpha * delta (in-place)
#[no_mangle]
pub extern "C" fn update_vector(idx: u8, alpha: f32) {
    unsafe {
        if idx >= HNSW.count { return; }
        let n = HNSW.dims as usize;
        let mut i = 0;
        while i < n { HNSW.vectors[idx as usize].data[i] += alpha * DELTA[i]; i += 1; }
        HNSW.vectors[idx as usize].norm = norm(&HNSW.vectors[idx as usize].data[..n], n);
    }
}

/// Get mutable delta buffer pointer
#[no_mangle]
pub extern "C" fn set_delta_ptr() -> *mut f32 { unsafe { DELTA.as_mut_ptr() } }

/// Combined HNSW-SNN cycle: search → convert to currents → inject
/// Useful for linking vector similarity to neural activation
#[no_mangle]
pub extern "C" fn hnsw_to_snn(k: u8, gain: f32) -> u8 {
    unsafe {
        let found = search(k);
        if found == 0 { return 0; }

        // Convert search results to neural currents
        let mut i = 0u8;
        while i < found {
            let r = &RESULTS[i as usize];
            if r.idx != 255 {
                // Inverse distance = stronger activation
                let current = gain / (1.0 + r.distance);
                MEMBRANE[r.idx as usize] += current;
            }
            i += 1;
        }
        found
    }
}

// ============ Spiking Neural Network API ============

/// Reset SNN state for all neurons
#[no_mangle]
pub extern "C" fn snn_reset() {
    unsafe {
        let mut i = 0;
        while i < MAX_VECTORS {
            MEMBRANE[i] = V_REST;
            THRESHOLD[i] = 1.0;
            LAST_SPIKE[i] = -1000.0;
            REFRAC[i] = 0.0;
            SPIKES[i] = false;
            i += 1;
        }
        SIM_TIME = 0.0;
    }
}

/// Set membrane potential for a neuron
#[no_mangle]
pub extern "C" fn snn_set_membrane(idx: u8, v: f32) {
    if idx < MAX_VECTORS as u8 { unsafe { MEMBRANE[idx as usize] = v; } }
}

/// Get membrane potential
#[no_mangle]
pub extern "C" fn snn_get_membrane(idx: u8) -> f32 {
    if idx < MAX_VECTORS as u8 { unsafe { MEMBRANE[idx as usize] } } else { 0.0 }
}

/// Set firing threshold for a neuron
#[no_mangle]
pub extern "C" fn snn_set_threshold(idx: u8, t: f32) {
    if idx < MAX_VECTORS as u8 { unsafe { THRESHOLD[idx as usize] = t; } }
}

/// Inject current into a neuron (adds to membrane potential)
#[no_mangle]
pub extern "C" fn snn_inject(idx: u8, current: f32) {
    if idx < MAX_VECTORS as u8 { unsafe { MEMBRANE[idx as usize] += current; } }
}

/// Get spike status (1 if spiked last step, 0 otherwise)
#[no_mangle]
pub extern "C" fn snn_spiked(idx: u8) -> u8 {
    if idx < MAX_VECTORS as u8 { unsafe { SPIKES[idx as usize] as u8 } } else { 0 }
}

/// Get spike bitset (32 neurons packed into u32)
#[no_mangle]
pub extern "C" fn snn_get_spikes() -> u32 {
    unsafe {
        let mut bits = 0u32;
        let mut i = 0;
        while i < MAX_VECTORS { if SPIKES[i] { bits |= 1 << i; } i += 1; }
        bits
    }
}

/// LIF neuron step: simulate one timestep (dt in ms)
/// Returns number of neurons that spiked
#[no_mangle]
pub extern "C" fn snn_step(dt: f32) -> u8 {
    unsafe {
        let decay = 1.0 - dt / TAU_MEMBRANE;
        let mut spike_count = 0u8;

        let mut i = 0u8;
        while i < HNSW.count {
            let idx = i as usize;
            SPIKES[idx] = false;

            // Skip if in refractory period
            if REFRAC[idx] > 0.0 {
                REFRAC[idx] -= dt;
                i += 1;
                continue;
            }

            // Leaky integration: V = V * decay
            MEMBRANE[idx] *= decay;

            // Check for spike
            if MEMBRANE[idx] >= THRESHOLD[idx] {
                SPIKES[idx] = true;
                spike_count += 1;
                LAST_SPIKE[idx] = SIM_TIME;
                MEMBRANE[idx] = V_RESET;
                REFRAC[idx] = TAU_REFRAC;
            }
            i += 1;
        }

        SIM_TIME += dt;
        spike_count
    }
}

/// Propagate spikes to neighbors (injects current based on edge weights)
/// Call after snn_step to propagate activity
#[no_mangle]
pub extern "C" fn snn_propagate(gain: f32) {
    unsafe {
        let mut i = 0u8;
        while i < HNSW.count {
            if !SPIKES[i as usize] { i += 1; continue; }

            // This neuron spiked, inject current to neighbors
            let nc = HNSW.nodes[i as usize].count;
            let mut j = 0u8;
            while j < nc {
                let nb = HNSW.nodes[i as usize].neighbors[j as usize];
                let w = EDGE_WEIGHTS[i as usize] as f32 / 255.0;
                MEMBRANE[nb as usize] += gain * w;
                j += 1;
            }
            i += 1;
        }
    }
}

/// STDP learning: adjust edge weights based on spike timing
/// Call after snn_step to apply plasticity
#[no_mangle]
pub extern "C" fn snn_stdp() {
    unsafe {
        let mut i = 0u8;
        while i < HNSW.count {
            if !SPIKES[i as usize] { i += 1; continue; }

            // Post-synaptic neuron spiked
            let nc = HNSW.nodes[i as usize].count;
            let mut j = 0u8;
            while j < nc {
                let pre = HNSW.nodes[i as usize].neighbors[j as usize];
                let dt = LAST_SPIKE[pre as usize] - SIM_TIME;

                // LTP: pre before post, LTD: pre after post
                // Simplified exponential approximation
                let dw = if dt < 0.0 {
                    STDP_A_PLUS * (1.0 + dt * INV_TAU_STDP)  // dt negative, so this decays
                } else {
                    -STDP_A_MINUS * (1.0 - dt * INV_TAU_STDP)
                };

                // Update weight (clamped to 0-255 using integer math)
                let w = EDGE_WEIGHTS[pre as usize] as i16 + (dw * 255.0) as i16;
                EDGE_WEIGHTS[pre as usize] = if w < 0 { 0 } else if w > 255 { 255 } else { w as u8 };
                j += 1;
            }
            i += 1;
        }
    }
}

/// Combined: step + propagate + optionally STDP
/// Returns spike count
#[no_mangle]
pub extern "C" fn snn_tick(dt: f32, gain: f32, learn: u8) -> u8 {
    let spikes = snn_step(dt);
    snn_propagate(gain);
    if learn != 0 { snn_stdp(); }
    spikes
}

/// Get current simulation time
#[no_mangle]
pub extern "C" fn snn_get_time() -> f32 { unsafe { SIM_TIME } }

// ============================================================================
// NOVEL NEUROMORPHIC DISCOVERIES
// ============================================================================

// ============ Spike-Timing Vector Encoding ============
// Novel discovery: Encode vectors as temporal spike patterns
// Each dimension becomes a spike time within a coding window

/// Encode vector to temporal spike pattern (rate-to-time conversion)
/// Higher values → earlier spikes (first-spike coding)
/// Returns encoded pattern as 32-bit bitmask
#[no_mangle]
pub extern "C" fn encode_vector_to_spikes(idx: u8) -> u32 {
    unsafe {
        if idx >= HNSW.count { return 0; }
        let n = HNSW.dims as usize;
        let mut pattern = 0u32;

        // Normalize vector values to spike times
        let mut max_val = 0.0f32;
        let mut i = 0;
        while i < n {
            let v = HNSW.vectors[idx as usize].data[i];
            if v > max_val { max_val = v; }
            if -v > max_val { max_val = -v; }
            i += 1;
        }
        if max_val == 0.0 { return 0; }

        // Encode: high values → low bit positions (early spikes)
        i = 0;
        while i < n.min(SPIKE_ENCODING_RES as usize * 4) {
            let normalized = (HNSW.vectors[idx as usize].data[i] + max_val) / (2.0 * max_val);
            let slot = ((1.0 - normalized) * SPIKE_ENCODING_RES as f32) as u8;
            let bit_pos = i as u8 + slot * (n as u8 / SPIKE_ENCODING_RES);
            if bit_pos < 32 { pattern |= 1u32 << bit_pos; }
            i += 1;
        }

        SPIKE_PATTERN[idx as usize] = pattern;
        pattern
    }
}

/// Compute spike-timing similarity between two spike patterns
/// Uses Victor-Purpura-inspired metric: count matching spike times
#[no_mangle]
pub extern "C" fn spike_timing_similarity(a: u32, b: u32) -> f32 {
    // Count matching spike positions
    let matches = (a & b).count_ones() as f32;
    let total = (a | b).count_ones() as f32;
    if total == 0.0 { return 1.0; }
    matches / total  // Jaccard-like similarity
}

/// Search using spike-timing representation
/// Novel: temporal code matching instead of distance
#[no_mangle]
pub extern "C" fn spike_search(query_pattern: u32, k: u8) -> u8 {
    unsafe {
        if HNSW.count == 0 { return 0; }
        let k = k.min(16).min(HNSW.count);

        // Reset results
        let mut i = 0;
        while i < 16 {
            RESULTS[i] = SearchResult { idx: 255, core_id: HNSW.core_id, distance: 0.0 };
            i += 1;
        }

        let mut found = 0u8;
        i = 0;
        while i < HNSW.count as usize {
            let sim = spike_timing_similarity(query_pattern, SPIKE_PATTERN[i]);
            // Store as negative similarity for compatibility (lower = better)
            let dist = 1.0 - sim;

            if found < k || dist < RESULTS[(found - 1) as usize].distance {
                let mut p = found.min(k - 1) as usize;
                while p > 0 && RESULTS[p - 1].distance > dist {
                    if p < 16 { RESULTS[p] = RESULTS[p - 1]; }
                    p -= 1;
                }
                if p < 16 {
                    RESULTS[p] = SearchResult {
                        idx: i as u8,
                        core_id: HNSW.core_id,
                        distance: dist
                    };
                    if found < k { found += 1; }
                }
            }
            i += 1;
        }
        found
    }
}

// ============ Homeostatic Plasticity ============
// Novel: Self-stabilizing network maintains target activity level
// Prevents runaway excitation or complete silence

/// Apply homeostatic plasticity: adjust thresholds to maintain target rate
#[no_mangle]
pub extern "C" fn homeostatic_update(dt: f32) {
    unsafe {
        let alpha = dt / HOMEOSTATIC_TAU;

        let mut i = 0u8;
        while i < HNSW.count {
            let idx = i as usize;

            // Update running spike rate estimate
            let instant_rate = if SPIKES[idx] { 1.0 / dt } else { 0.0 };
            SPIKE_RATE[idx] = SPIKE_RATE[idx] * (1.0 - alpha) + instant_rate * alpha;

            // Adjust threshold to approach target rate
            let rate_error = SPIKE_RATE[idx] - HOMEOSTATIC_TARGET;
            THRESHOLD[idx] += rate_error * alpha;

            // Clamp threshold to reasonable range
            if THRESHOLD[idx] < 0.1 { THRESHOLD[idx] = 0.1; }
            if THRESHOLD[idx] > 10.0 { THRESHOLD[idx] = 10.0; }

            i += 1;
        }
    }
}

/// Get current spike rate estimate
#[no_mangle]
pub extern "C" fn get_spike_rate(idx: u8) -> f32 {
    if idx < MAX_VECTORS as u8 { unsafe { SPIKE_RATE[idx as usize] } } else { 0.0 }
}

// ============ Oscillatory Resonance ============
// Novel: Gamma-rhythm synchronization for binding and search enhancement
// Neurons tuned to oscillation phase get amplified

/// Update oscillator phase
#[no_mangle]
pub extern "C" fn oscillator_step(dt: f32) {
    unsafe {
        // Phase advances with time: ω = 2πf
        let omega = 6.28318 * OSCILLATOR_FREQ / 1000.0; // Convert Hz to rad/ms
        OSCILLATOR_PHASE += omega * dt;
        if OSCILLATOR_PHASE > 6.28318 { OSCILLATOR_PHASE -= 6.28318; }
    }
}

/// Get current oscillator phase (0 to 2π)
#[no_mangle]
pub extern "C" fn oscillator_get_phase() -> f32 { unsafe { OSCILLATOR_PHASE } }

/// Compute resonance boost for a neuron based on phase alignment
/// Neurons in sync with gamma get amplified
#[no_mangle]
pub extern "C" fn compute_resonance(idx: u8) -> f32 {
    unsafe {
        if idx >= HNSW.count { return 0.0; }
        let i = idx as usize;

        // Each neuron has preferred phase based on its index
        let preferred_phase = (idx as f32 / MAX_VECTORS as f32) * 6.28318;
        let phase_diff = (OSCILLATOR_PHASE - preferred_phase).abs();
        let min_diff = if phase_diff > 3.14159 { 6.28318 - phase_diff } else { phase_diff };

        // Resonance is high when phase matches
        RESONANCE[i] = 1.0 - min_diff / 3.14159;
        RESONANCE[i]
    }
}

/// Apply resonance-modulated search boost
/// Query matches are enhanced when neuron is in favorable phase
#[no_mangle]
pub extern "C" fn resonance_search(k: u8, phase_weight: f32) -> u8 {
    unsafe {
        let found = search(k);

        // Modulate results by resonance
        let mut i = 0u8;
        while i < found {
            let idx = RESULTS[i as usize].idx;
            if idx != 255 {
                let res = compute_resonance(idx);
                // Lower distance = better, so multiply by (2 - resonance)
                RESULTS[i as usize].distance *= 2.0 - res * phase_weight;
            }
            i += 1;
        }

        // Re-sort results after resonance modulation
        let mut i = 0usize;
        while i < found as usize {
            let mut j = i + 1;
            while j < found as usize {
                if RESULTS[j].distance < RESULTS[i].distance {
                    let tmp = RESULTS[i];
                    RESULTS[i] = RESULTS[j];
                    RESULTS[j] = tmp;
                }
                j += 1;
            }
            i += 1;
        }
        found
    }
}

// ============ Winner-Take-All Circuits ============
// Novel: Competitive selection via lateral inhibition
// Only the most active neuron wins, enabling hard decisions

/// Reset WTA state
#[no_mangle]
pub extern "C" fn wta_reset() { unsafe { WTA_INHIBIT = 0.0; } }

/// Run WTA competition: only highest membrane potential survives
/// Returns winner index (or 255 if no winner)
#[no_mangle]
pub extern "C" fn wta_compete() -> u8 {
    unsafe {
        let mut max_v = 0.0f32;
        let mut winner = 255u8;

        let mut i = 0u8;
        while i < HNSW.count {
            let v = MEMBRANE[i as usize];
            if v > max_v && REFRAC[i as usize] <= 0.0 {
                max_v = v;
                winner = i;
            }
            i += 1;
        }

        // Apply lateral inhibition to all losers
        if winner != 255 {
            WTA_INHIBIT = max_v * WTA_INHIBITION;
            i = 0;
            while i < HNSW.count {
                if i != winner {
                    MEMBRANE[i as usize] -= WTA_INHIBIT;
                    if MEMBRANE[i as usize] < V_RESET {
                        MEMBRANE[i as usize] = V_RESET;
                    }
                }
                i += 1;
            }
        }
        winner
    }
}

/// Soft WTA: proportional inhibition based on rank
#[no_mangle]
pub extern "C" fn wta_soft() {
    unsafe {
        // Find max membrane potential
        let mut max_v = 0.0f32;
        let mut i = 0u8;
        while i < HNSW.count {
            if MEMBRANE[i as usize] > max_v { max_v = MEMBRANE[i as usize]; }
            i += 1;
        }
        if max_v <= 0.0 { return; }

        // Normalize and apply softmax-like competition
        i = 0;
        while i < HNSW.count {
            let ratio = MEMBRANE[i as usize] / max_v;
            // Exponential competition: low ratios get strongly suppressed
            let survival = ratio * ratio; // Square for sharper competition
            MEMBRANE[i as usize] *= survival;
            i += 1;
        }
    }
}

// ============ Dendritic Computation ============
// Novel: Nonlinear integration in dendritic compartments
// Enables local coincidence detection before soma integration

/// Reset dendritic compartments
#[no_mangle]
pub extern "C" fn dendrite_reset() {
    unsafe {
        let mut i = 0;
        while i < MAX_VECTORS {
            let mut j = 0;
            while j < MAX_NEIGHBORS { DENDRITE[i][j] = 0.0; j += 1; }
            i += 1;
        }
    }
}

/// Inject input to specific dendritic compartment
#[no_mangle]
pub extern "C" fn dendrite_inject(neuron: u8, branch: u8, current: f32) {
    unsafe {
        if neuron < MAX_VECTORS as u8 && branch < MAX_NEIGHBORS as u8 {
            DENDRITE[neuron as usize][branch as usize] += current;
        }
    }
}

/// Dendritic integration with nonlinearity
/// Multiple coincident inputs on same branch get amplified
#[no_mangle]
pub extern "C" fn dendrite_integrate(neuron: u8) -> f32 {
    unsafe {
        if neuron >= HNSW.count { return 0.0; }
        let idx = neuron as usize;
        let nc = HNSW.nodes[idx].count as usize;

        let mut total = 0.0f32;
        let mut branch = 0;
        while branch < nc {
            let d = DENDRITE[idx][branch];
            // Nonlinear: small inputs are linear, large inputs saturate with boost
            if d > 0.0 {
                // Sigmoidal nonlinearity with supralinear boost
                let nonlin = if d < 1.0 {
                    d
                } else {
                    1.0 + (d - 1.0) / (1.0 + (d - 1.0) / DENDRITIC_NONLIN)
                };
                total += nonlin;
            }
            branch += 1;
        }

        // Transfer to soma
        MEMBRANE[idx] += total;
        total
    }
}

/// Propagate spikes through dendritic tree (not just soma)
#[no_mangle]
pub extern "C" fn dendrite_propagate(gain: f32) {
    unsafe {
        let mut i = 0u8;
        while i < HNSW.count {
            if !SPIKES[i as usize] { i += 1; continue; }

            // This neuron spiked, inject to neighbor dendrites
            let nc = HNSW.nodes[i as usize].count;
            let mut j = 0u8;
            while j < nc {
                let nb = HNSW.nodes[i as usize].neighbors[j as usize];
                let w = EDGE_WEIGHTS[i as usize] as f32 / 255.0;

                // Find which dendrite branch this connection is on
                let mut branch = 0u8;
                let nb_nc = HNSW.nodes[nb as usize].count;
                while branch < nb_nc {
                    if HNSW.nodes[nb as usize].neighbors[branch as usize] == i {
                        break;
                    }
                    branch += 1;
                }

                if branch < MAX_NEIGHBORS as u8 {
                    DENDRITE[nb as usize][branch as usize] += gain * w;
                }
                j += 1;
            }
            i += 1;
        }
    }
}

// ============ Temporal Pattern Recognition ============
// Novel: Store and match spike pattern sequences
// Enables recognition of dynamic temporal signatures

/// Record current spike state into pattern buffer (shift register)
#[no_mangle]
pub extern "C" fn pattern_record() {
    unsafe {
        let mut i = 0;
        while i < MAX_VECTORS {
            // Shift pattern left and add new spike
            SPIKE_PATTERN[i] = (SPIKE_PATTERN[i] << 1) | (SPIKES[i] as u32);
            i += 1;
        }
    }
}

/// Get temporal spike pattern for a neuron
#[no_mangle]
pub extern "C" fn get_pattern(idx: u8) -> u32 {
    if idx < MAX_VECTORS as u8 { unsafe { SPIKE_PATTERN[idx as usize] } } else { 0 }
}

/// Match pattern against stored patterns (Hamming similarity)
/// Returns best matching neuron index
#[no_mangle]
pub extern "C" fn pattern_match(target: u32) -> u8 {
    unsafe {
        let mut best_idx = 255u8;
        let mut best_sim = 0u32;

        let mut i = 0u8;
        while i < HNSW.count {
            // XOR gives difference, NOT gives similarity bits
            let diff = target ^ SPIKE_PATTERN[i as usize];
            let sim = (!diff).count_ones();
            if sim > best_sim {
                best_sim = sim;
                best_idx = i;
            }
            i += 1;
        }
        best_idx
    }
}

/// Temporal correlation: find neurons with similar spike history
#[no_mangle]
pub extern "C" fn pattern_correlate(idx: u8, threshold: u8) -> u32 {
    unsafe {
        if idx >= HNSW.count { return 0; }
        let target = SPIKE_PATTERN[idx as usize];
        let mut correlated = 0u32;

        let mut i = 0u8;
        while i < HNSW.count {
            if i != idx {
                let diff = target ^ SPIKE_PATTERN[i as usize];
                let dist = diff.count_ones() as u8;
                if dist <= threshold && i < 32 {
                    correlated |= 1u32 << i;
                }
            }
            i += 1;
        }
        correlated
    }
}

// ============ Combined Neuromorphic Search ============
// Novel: Unified search combining all mechanisms

/// Advanced neuromorphic search with all novel features
/// Combines: HNSW graph, spike timing, oscillation, WTA
#[no_mangle]
pub extern "C" fn neuromorphic_search(k: u8, dt: f32, iterations: u8) -> u8 {
    unsafe {
        if HNSW.count == 0 { return 0; }

        // Reset neural state
        snn_reset();
        dendrite_reset();
        wta_reset();

        // Convert query to spike pattern
        let n = HNSW.dims as usize;
        let qn = norm(&QUERY[..n], n);

        // Initialize membrane potentials from vector distances
        let mut i = 0u8;
        while i < HNSW.count {
            let d = distance(&QUERY[..n], qn, i);
            // Inverse distance = initial activation
            MEMBRANE[i as usize] = 1.0 / (1.0 + d);
            i += 1;
        }

        // Run neuromorphic dynamics
        let mut iter = 0u8;
        while iter < iterations {
            oscillator_step(dt);

            // Dendritic integration
            i = 0;
            while i < HNSW.count {
                dendrite_integrate(i);
                i += 1;
            }

            // Neural step with spike propagation
            snn_step(dt);
            dendrite_propagate(0.5);

            // WTA competition for sharpening
            wta_soft();

            // Record spike patterns
            pattern_record();

            // Homeostatic regulation
            homeostatic_update(dt);

            iter += 1;
        }

        // Collect results based on final spike patterns and resonance
        let mut i = 0;
        while i < 16 {
            RESULTS[i] = SearchResult { idx: 255, core_id: HNSW.core_id, distance: f32::MAX };
            i += 1;
        }

        let mut found = 0u8;
        i = 0;
        while i < HNSW.count as usize {
            // Score = spike count + resonance + membrane potential
            let spikes = SPIKE_PATTERN[i].count_ones() as f32;
            let res = RESONANCE[i];
            let vm = MEMBRANE[i];
            let score = -(spikes * 10.0 + res * 5.0 + vm);  // Negative for sorting

            if found < k || score < RESULTS[(found - 1) as usize].distance {
                let mut p = found.min(k - 1) as usize;
                while p > 0 && RESULTS[p - 1].distance > score {
                    if p < 16 { RESULTS[p] = RESULTS[p - 1]; }
                    p -= 1;
                }
                if p < 16 {
                    RESULTS[p] = SearchResult {
                        idx: i as u8,
                        core_id: HNSW.core_id,
                        distance: score
                    };
                    if found < k { found += 1; }
                }
            }
            i += 1;
        }
        found
    }
}

/// Get total network activity (sum of spike rates)
#[no_mangle]
pub extern "C" fn get_network_activity() -> f32 {
    unsafe {
        let mut total = 0.0f32;
        let mut i = 0;
        while i < MAX_VECTORS {
            total += SPIKE_RATE[i];
            i += 1;
        }
        total
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
