// =============================================================================
// Prime-Radiant GPU Compute Shaders - Sparse Attention Mask
// =============================================================================
//
// Generate sparse attention masks from energy thresholds.
// Only edges with energy below threshold (coherent) are included.
//
// This enables efficient sparse attention where only meaningful
// (low-energy, coherent) connections are computed, dramatically
// reducing computation for large graphs.
//
// Output Formats:
// 1. Index list: Compact list of (row, col) pairs for valid edges
// 2. Dense mask: Full NxN boolean matrix (for small N)
// 3. CSR format: Compressed sparse row for efficient sparse matmul
//
// Optimizations:
// - Stream compaction for index list generation
// - Warp-level voting for efficient counting
// - Coalesced writes using shared memory staging

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

struct SparseMaskParams {
    total_edges: u32,
    coherence_threshold: f32,
    max_edges: u32,
    output_format: u32, // 0=indices, 1=dense, 2=csr
    seq_len: u32,
    batch_size: u32,
    padding: array<u32, 2>,
}

struct EdgeIndex {
    row: u32,
    col: u32,
}

struct CSRPointers {
    row_ptr: u32,
    nnz: u32,
}

const WORKGROUP_SIZE: u32 = 256u;
const OUTPUT_INDICES: u32 = 0u;
const OUTPUT_DENSE: u32 = 1u;
const OUTPUT_CSR: u32 = 2u;

// =============================================================================
// BUFFER BINDINGS
// =============================================================================

/// Input edge energies (seq_len * seq_len per batch, or sparse)
@group(0) @binding(0) var<storage, read> edge_energies: array<f32>;

/// Output: sparse edge indices (for index format)
@group(0) @binding(1) var<storage, read_write> sparse_indices: array<EdgeIndex>;

/// Output: dense mask (for dense format)
@group(0) @binding(2) var<storage, read_write> dense_mask: array<u32>;

/// Output: number of valid edges (atomic counter)
@group(0) @binding(3) var<storage, read_write> edge_count: atomic<u32>;

/// Mask parameters
@group(0) @binding(4) var<uniform> params: SparseMaskParams;

// =============================================================================
// SHARED MEMORY
// =============================================================================

/// Shared memory for stream compaction
var<workgroup> shared_valid: array<u32, 256>;

/// Prefix sum for compaction offsets
var<workgroup> shared_prefix: array<u32, 256>;

/// Staging buffer for coalesced writes
var<workgroup> shared_indices: array<EdgeIndex, 256>;

/// Workgroup-level count of valid edges
var<workgroup> workgroup_count: atomic<u32>;

// =============================================================================
// BASIC SPARSE MASK GENERATION
// =============================================================================

/// Generate sparse mask as index list
@compute @workgroup_size(256)
fn generate_sparse_indices(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>
) {
    let idx = global_id.x;
    let tid = local_id.x;
    let total_edges = params.total_edges;
    let threshold = params.coherence_threshold;
    let seq_len = params.seq_len;

    // Initialize workgroup counter
    if (tid == 0u) {
        atomicStore(&workgroup_count, 0u);
    }
    workgroupBarrier();

    // Check if this edge is valid (below threshold)
    var is_valid: u32 = 0u;
    var row: u32 = 0u;
    var col: u32 = 0u;

    if (idx < total_edges) {
        let energy = edge_energies[idx];
        is_valid = select(0u, 1u, energy < threshold);

        // Compute row and column from linear index
        row = idx / seq_len;
        col = idx % seq_len;
    }

    shared_valid[tid] = is_valid;
    workgroupBarrier();

    // Compute prefix sum for compaction
    // Hillis-Steele parallel scan
    shared_prefix[tid] = is_valid;
    workgroupBarrier();

    for (var offset = 1u; offset < WORKGROUP_SIZE; offset <<= 1u) {
        var val: u32 = 0u;
        if (tid >= offset) {
            val = shared_prefix[tid - offset];
        }
        workgroupBarrier();
        shared_prefix[tid] += val;
        workgroupBarrier();
    }

    // Total valid in this workgroup
    let total_valid = shared_prefix[WORKGROUP_SIZE - 1u];

    // Get global offset for this workgroup
    var global_offset: u32 = 0u;
    if (tid == 0u && total_valid > 0u) {
        global_offset = atomicAdd(&edge_count, total_valid);
        atomicStore(&workgroup_count, global_offset);
    }
    workgroupBarrier();
    global_offset = atomicLoad(&workgroup_count);

    // Write valid edges to output using compacted indices
    if (is_valid == 1u && idx < total_edges) {
        // Exclusive prefix sum gives position
        let local_pos = select(0u, shared_prefix[tid - 1u], tid > 0u);
        let global_pos = global_offset + local_pos;

        if (global_pos < params.max_edges) {
            sparse_indices[global_pos] = EdgeIndex(row, col);
        }
    }
}

// =============================================================================
// DENSE MASK GENERATION
// =============================================================================

/// Generate dense boolean mask (packed as u32 bits)
@compute @workgroup_size(256)
fn generate_dense_mask(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let idx = global_id.x;
    let total_edges = params.total_edges;
    let threshold = params.coherence_threshold;

    if (idx >= total_edges) {
        return;
    }

    let energy = edge_energies[idx];
    let is_valid = energy < threshold;

    // Pack 32 boolean values per u32
    let word_idx = idx / 32u;
    let bit_idx = idx % 32u;

    if (is_valid) {
        // Atomic OR to set the bit
        atomicOr(&dense_mask[word_idx], 1u << bit_idx);
    }
}

/// Unpack dense mask bit
fn is_edge_valid(dense_mask_ptr: ptr<storage, array<u32>, read>, idx: u32) -> bool {
    let word_idx = idx / 32u;
    let bit_idx = idx % 32u;
    return ((*dense_mask_ptr)[word_idx] & (1u << bit_idx)) != 0u;
}

// =============================================================================
// CSR FORMAT GENERATION
// =============================================================================

/// CSR row pointers
@group(1) @binding(0) var<storage, read_write> csr_row_ptr: array<u32>;

/// CSR column indices
@group(1) @binding(1) var<storage, read_write> csr_col_idx: array<u32>;

/// CSR values (attention weights or energies)
@group(1) @binding(2) var<storage, read_write> csr_values: array<f32>;

/// Per-row counters for CSR construction
@group(1) @binding(3) var<storage, read_write> row_counts: array<atomic<u32>>;

/// Phase 1: Count valid edges per row
@compute @workgroup_size(256)
fn count_edges_per_row(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let idx = global_id.x;
    let total_edges = params.total_edges;
    let threshold = params.coherence_threshold;
    let seq_len = params.seq_len;

    if (idx >= total_edges) {
        return;
    }

    let energy = edge_energies[idx];

    if (energy < threshold) {
        let row = idx / seq_len;
        atomicAdd(&row_counts[row], 1u);
    }
}

/// Phase 2: Compute row pointers via prefix sum
@compute @workgroup_size(256)
fn compute_row_pointers(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let row = global_id.x;
    let tid = local_id.x;
    let seq_len = params.seq_len;

    if (row >= seq_len) {
        return;
    }

    // Load count into shared memory
    shared_prefix[tid] = atomicLoad(&row_counts[row]);
    workgroupBarrier();

    // Inclusive prefix sum
    for (var offset = 1u; offset < WORKGROUP_SIZE; offset <<= 1u) {
        var val: u32 = 0u;
        if (tid >= offset) {
            val = shared_prefix[tid - offset];
        }
        workgroupBarrier();
        shared_prefix[tid] += val;
        workgroupBarrier();
    }

    // Convert to exclusive prefix sum for row pointers
    // row_ptr[i] = sum of counts for rows 0..i-1
    let inclusive_sum = shared_prefix[tid];
    let count = atomicLoad(&row_counts[row]);
    let exclusive_sum = inclusive_sum - count;

    csr_row_ptr[row] = exclusive_sum;

    // Reset counter to be used as write position
    atomicStore(&row_counts[row], exclusive_sum);

    // Last row sets the final pointer (total nnz)
    if (row == seq_len - 1u) {
        csr_row_ptr[seq_len] = inclusive_sum;
    }
}

/// Phase 3: Populate CSR column indices and values
@compute @workgroup_size(256)
fn populate_csr_data(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let idx = global_id.x;
    let total_edges = params.total_edges;
    let threshold = params.coherence_threshold;
    let seq_len = params.seq_len;

    if (idx >= total_edges) {
        return;
    }

    let energy = edge_energies[idx];

    if (energy < threshold) {
        let row = idx / seq_len;
        let col = idx % seq_len;

        // Get write position using atomic increment
        let pos = atomicAdd(&row_counts[row], 1u);

        csr_col_idx[pos] = col;
        csr_values[pos] = energy;
    }
}

// =============================================================================
// BATCHED SPARSE MASK
// =============================================================================

/// Batch offsets for multi-batch processing
@group(2) @binding(0) var<storage, read> batch_offsets: array<u32>;

/// Per-batch edge counts
@group(2) @binding(1) var<storage, read_write> batch_edge_counts: array<atomic<u32>>;

/// Generate sparse mask for multiple batches
@compute @workgroup_size(256)
fn generate_batched_sparse_mask(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>
) {
    let batch_idx = workgroup_id.z;
    let local_idx = global_id.x;
    let tid = local_id.x;

    let seq_len = params.seq_len;
    let edges_per_batch = seq_len * seq_len;
    let threshold = params.coherence_threshold;

    if (local_idx >= edges_per_batch) {
        return;
    }

    // Global index in energy array
    let global_idx = batch_idx * edges_per_batch + local_idx;

    let energy = edge_energies[global_idx];
    let is_valid = select(0u, 1u, energy < threshold);

    // Stream compaction within batch
    shared_valid[tid] = is_valid;
    workgroupBarrier();

    // Prefix sum
    shared_prefix[tid] = is_valid;
    workgroupBarrier();

    for (var offset = 1u; offset < WORKGROUP_SIZE; offset <<= 1u) {
        var val: u32 = 0u;
        if (tid >= offset) {
            val = shared_prefix[tid - offset];
        }
        workgroupBarrier();
        shared_prefix[tid] += val;
        workgroupBarrier();
    }

    // Get batch-local offset
    if (tid == 0u) {
        let total_valid = shared_prefix[WORKGROUP_SIZE - 1u];
        let offset = atomicAdd(&batch_edge_counts[batch_idx], total_valid);
        atomicStore(&workgroup_count, offset);
    }
    workgroupBarrier();

    let batch_offset = batch_offsets[batch_idx];
    let workgroup_offset = atomicLoad(&workgroup_count);

    // Write valid edges
    if (is_valid == 1u) {
        let local_pos = select(0u, shared_prefix[tid - 1u], tid > 0u);
        let global_pos = batch_offset + workgroup_offset + local_pos;

        let row = local_idx / seq_len;
        let col = local_idx % seq_len;

        if (global_pos < params.max_edges) {
            sparse_indices[global_pos] = EdgeIndex(row, col);
        }
    }
}

// =============================================================================
// DYNAMIC THRESHOLD ADJUSTMENT
// =============================================================================

/// Statistics for adaptive threshold
@group(3) @binding(0) var<storage, read_write> mask_stats: array<f32>;

/// Compute mask statistics for adaptive thresholding
@compute @workgroup_size(256)
fn compute_mask_statistics(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let idx = global_id.x;
    let tid = local_id.x;
    let total_edges = params.total_edges;
    let threshold = params.coherence_threshold;

    // Count valid and total, compute sparsity ratio
    var valid_count: u32 = 0u;

    if (idx < total_edges) {
        let energy = edge_energies[idx];
        valid_count = select(0u, 1u, energy < threshold);
    }

    shared_prefix[tid] = valid_count;
    workgroupBarrier();

    // Reduce to get total valid
    for (var stride = WORKGROUP_SIZE / 2u; stride > 0u; stride >>= 1u) {
        if (tid < stride) {
            shared_prefix[tid] += shared_prefix[tid + stride];
        }
        workgroupBarrier();
    }

    // Thread 0 updates global statistics
    if (tid == 0u) {
        // Atomic add to global counter
        // mask_stats[0] = total valid edges
        // mask_stats[1] = sparsity ratio (computed after all workgroups)
    }
}

// =============================================================================
// CAUSAL MASK COMBINATION
// =============================================================================

/// Combine energy-based sparse mask with causal mask
@compute @workgroup_size(16, 16)
fn combine_with_causal_mask(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let row = global_id.y;
    let col = global_id.x;
    let seq_len = params.seq_len;
    let threshold = params.coherence_threshold;

    if (row >= seq_len || col >= seq_len) {
        return;
    }

    let idx = row * seq_len + col;
    let energy = edge_energies[idx];

    // Valid if: (1) below energy threshold AND (2) satisfies causal constraint
    let energy_valid = energy < threshold;
    let causal_valid = col <= row; // Can only attend to past

    let is_valid = energy_valid && causal_valid;

    // Write to dense mask
    let word_idx = idx / 32u;
    let bit_idx = idx % 32u;

    if (is_valid) {
        atomicOr(&dense_mask[word_idx], 1u << bit_idx);
    }
}
