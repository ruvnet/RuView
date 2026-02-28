//! # SIMD Energy Computation
//!
//! High-performance coherence energy computation using SIMD intrinsics.
//! These operations are critical for the hot path of coherence evaluation.
//!
//! ## Key Operations
//!
//! | Operation | Description | Use Case |
//! |-----------|-------------|----------|
//! | `batch_residuals_simd` | Compute residuals for multiple edges | Bulk energy update |
//! | `batch_residual_norms_simd` | Compute squared norms of residuals | Energy aggregation |
//! | `weighted_energy_sum_simd` | Sum residual energies with weights | Total energy |
//! | `batch_lane_assignment_simd` | Branchless lane routing | Gate evaluation |
//!
//! ## Performance Characteristics
//!
//! The batch operations are designed to process multiple edges in parallel,
//! achieving near-optimal memory bandwidth utilization when vector dimensions
//! align with SIMD register widths.

use wide::{f32x8, CmpGe};

use crate::execution::ComputeLane;

/// Compute residuals for multiple edges in parallel.
///
/// Given flattened source and target state vectors, computes the residual
/// for each edge: `residual[i] = source[i] - target[i]`
///
/// # Arguments
///
/// * `sources` - Flattened source states: `[s0_0, s0_1, ..., s1_0, s1_1, ...]`
/// * `targets` - Flattened target states: `[t0_0, t0_1, ..., t1_0, t1_1, ...]`
/// * `residuals` - Output buffer for residuals (same layout as inputs)
/// * `dim` - Dimension of each state vector
/// * `count` - Number of edges to process
///
/// # Layout
///
/// For `count` edges with `dim`-dimensional states:
/// - Total elements = `count * dim`
/// - Edge `i` starts at index `i * dim`
///
/// # Panics
///
/// Panics in debug mode if buffer sizes don't match `dim * count`.
#[inline]
pub fn batch_residuals_simd(
    sources: &[f32],
    targets: &[f32],
    residuals: &mut [f32],
    dim: usize,
    count: usize,
) {
    let total = dim * count;
    debug_assert_eq!(sources.len(), total);
    debug_assert_eq!(targets.len(), total);
    debug_assert_eq!(residuals.len(), total);

    // For small batches, use scalar
    if total < 32 {
        batch_residuals_scalar(sources, targets, residuals);
        return;
    }

    // SIMD subtraction
    let chunks_s = sources.chunks_exact(8);
    let chunks_t = targets.chunks_exact(8);
    let chunks_r = residuals.chunks_exact_mut(8);

    let remainder_s = chunks_s.remainder();
    let remainder_t = chunks_t.remainder();
    let offset = total - remainder_s.len();

    for ((cs, ct), cr) in chunks_s.zip(chunks_t).zip(chunks_r) {
        let vs = load_f32x8(cs);
        let vt = load_f32x8(ct);
        let result = vs - vt;
        store_f32x8(cr, result);
    }

    // Handle remainder
    for (i, (&vs, &vt)) in remainder_s.iter().zip(remainder_t.iter()).enumerate() {
        residuals[offset + i] = vs - vt;
    }
}

/// Compute squared norms of residuals for multiple edges.
///
/// This operation computes `||residual_i||^2` for each edge without
/// storing the full residual vectors.
///
/// # Arguments
///
/// * `sources` - Flattened source states
/// * `targets` - Flattened target states
/// * `norms` - Output buffer for squared norms (length = `count`)
/// * `dim` - Dimension of each state vector
/// * `count` - Number of edges
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::energy::batch_residual_norms_simd;
///
/// let sources = [1.0, 0.0, 0.0, 0.0]; // 2 edges, dim=2
/// let targets = [0.0, 0.0, 1.0, 0.0];
/// let mut norms = [0.0f32; 2];
///
/// batch_residual_norms_simd(&sources, &targets, &mut norms, 2, 2);
/// // norms[0] = 1.0 (||[1,0] - [0,0]||^2)
/// // norms[1] = 1.0 (||[0,0] - [1,0]||^2)
/// ```
#[inline]
pub fn batch_residual_norms_simd(
    sources: &[f32],
    targets: &[f32],
    norms: &mut [f32],
    dim: usize,
    count: usize,
) {
    debug_assert_eq!(sources.len(), dim * count);
    debug_assert_eq!(targets.len(), dim * count);
    debug_assert_eq!(norms.len(), count);

    // For small dimensions, process edges directly
    if dim < 16 {
        for i in 0..count {
            let offset = i * dim;
            norms[i] = compute_residual_norm_sq_scalar(
                &sources[offset..offset + dim],
                &targets[offset..offset + dim],
            );
        }
        return;
    }

    // For larger dimensions, use SIMD per-edge
    for i in 0..count {
        let offset = i * dim;
        norms[i] = compute_residual_norm_sq_simd(
            &sources[offset..offset + dim],
            &targets[offset..offset + dim],
        );
    }
}

/// Compute residual norm squared for a single edge using SIMD.
///
/// # Arguments
///
/// * `source` - Source state vector
/// * `target` - Target state vector
///
/// # Returns
///
/// `||source - target||^2`
#[inline]
pub fn compute_residual_norm_sq_simd(source: &[f32], target: &[f32]) -> f32 {
    debug_assert_eq!(source.len(), target.len());

    let len = source.len();

    if len < 16 {
        return compute_residual_norm_sq_scalar(source, target);
    }

    let chunks_s = source.chunks_exact(8);
    let chunks_t = target.chunks_exact(8);
    let remainder_s = chunks_s.remainder();
    let remainder_t = chunks_t.remainder();

    let mut acc0 = f32x8::ZERO;
    let mut acc1 = f32x8::ZERO;

    let mut chunks_s_iter = chunks_s;
    let mut chunks_t_iter = chunks_t;

    // Unroll 2x
    while let (Some(cs0), Some(ct0)) = (chunks_s_iter.next(), chunks_t_iter.next()) {
        let vs0 = load_f32x8(cs0);
        let vt0 = load_f32x8(ct0);
        let diff0 = vs0 - vt0;
        acc0 = diff0.mul_add(diff0, acc0);

        if let (Some(cs1), Some(ct1)) = (chunks_s_iter.next(), chunks_t_iter.next()) {
            let vs1 = load_f32x8(cs1);
            let vt1 = load_f32x8(ct1);
            let diff1 = vs1 - vt1;
            acc1 = diff1.mul_add(diff1, acc1);
        }
    }

    let combined = acc0 + acc1;
    let mut sum = combined.reduce_add();

    // Handle remainder
    for (&vs, &vt) in remainder_s.iter().zip(remainder_t.iter()) {
        let diff = vs - vt;
        sum += diff * diff;
    }

    sum
}

/// Compute weighted energy sum using SIMD horizontal reduction.
///
/// # Arguments
///
/// * `residual_norms` - Squared norms of residuals: `||r_e||^2`
/// * `weights` - Edge weights: `w_e`
///
/// # Returns
///
/// Total energy: `E(S) = sum(w_e * ||r_e||^2)`
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::energy::weighted_energy_sum_simd;
///
/// let norms = [1.0, 4.0, 9.0, 16.0];
/// let weights = [1.0, 0.5, 0.25, 0.125];
/// let energy = weighted_energy_sum_simd(&norms, &weights);
/// // energy = 1*1 + 0.5*4 + 0.25*9 + 0.125*16 = 1 + 2 + 2.25 + 2 = 7.25
/// ```
#[inline]
pub fn weighted_energy_sum_simd(residual_norms: &[f32], weights: &[f32]) -> f32 {
    debug_assert_eq!(residual_norms.len(), weights.len());

    let len = residual_norms.len();

    if len < 16 {
        return weighted_energy_sum_scalar(residual_norms, weights);
    }

    let chunks_n = residual_norms.chunks_exact(8);
    let chunks_w = weights.chunks_exact(8);
    let remainder_n = chunks_n.remainder();
    let remainder_w = chunks_w.remainder();

    let mut acc0 = f32x8::ZERO;
    let mut acc1 = f32x8::ZERO;

    let mut chunks_n_iter = chunks_n;
    let mut chunks_w_iter = chunks_w;

    // Unroll 2x
    while let (Some(cn0), Some(cw0)) = (chunks_n_iter.next(), chunks_w_iter.next()) {
        let vn0 = load_f32x8(cn0);
        let vw0 = load_f32x8(cw0);
        acc0 = vn0.mul_add(vw0, acc0);

        if let (Some(cn1), Some(cw1)) = (chunks_n_iter.next(), chunks_w_iter.next()) {
            let vn1 = load_f32x8(cn1);
            let vw1 = load_f32x8(cw1);
            acc1 = vn1.mul_add(vw1, acc1);
        }
    }

    let combined = acc0 + acc1;
    let mut sum = combined.reduce_add();

    // Handle remainder
    for (&n, &w) in remainder_n.iter().zip(remainder_w.iter()) {
        sum += n * w;
    }

    sum
}

/// Batch lane assignment using branchless SIMD comparison.
///
/// Assigns each energy value to a compute lane based on threshold comparison.
/// Uses branchless operations for consistent performance regardless of data.
///
/// # Arguments
///
/// * `energies` - Array of energy values to route
/// * `thresholds` - `[reflex, retrieval, heavy, human]` thresholds
/// * `lanes` - Output buffer for lane assignments (as `u8`)
///
/// # Lane Assignment Logic
///
/// - `energy < reflex` -> Lane 0 (Reflex)
/// - `reflex <= energy < retrieval` -> Lane 1 (Retrieval)
/// - `retrieval <= energy < heavy` -> Lane 2 (Heavy)
/// - `energy >= heavy` -> Lane 3 (Human)
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::energy::batch_lane_assignment_simd;
///
/// let energies = [0.1, 0.25, 0.6, 0.9];
/// let thresholds = [0.2, 0.5, 0.8, 1.0];
/// let mut lanes = [0u8; 4];
///
/// batch_lane_assignment_simd(&energies, thresholds, &mut lanes);
/// // lanes = [0, 1, 2, 3] (Reflex, Retrieval, Heavy, Human)
/// ```
#[inline]
pub fn batch_lane_assignment_simd(energies: &[f32], thresholds: [f32; 4], lanes: &mut [u8]) {
    debug_assert_eq!(energies.len(), lanes.len());

    let len = energies.len();

    // Thresholds for lane boundaries
    let t_reflex = thresholds[0];
    let t_retrieval = thresholds[1];
    let t_heavy = thresholds[2];

    if len < 16 {
        batch_lane_assignment_scalar(energies, thresholds, lanes);
        return;
    }

    // SIMD thresholds
    let vt_reflex = f32x8::splat(t_reflex);
    let vt_retrieval = f32x8::splat(t_retrieval);
    let vt_heavy = f32x8::splat(t_heavy);

    let chunks_e = energies.chunks_exact(8);
    let chunks_l = lanes.chunks_exact_mut(8);

    let remainder_e = chunks_e.remainder();
    let offset = len - remainder_e.len();

    let v_one = f32x8::splat(1.0);
    let v_zero = f32x8::ZERO;

    for (ce, cl) in chunks_e.zip(chunks_l) {
        let ve = load_f32x8(ce);

        // Branchless comparison using SIMD masks
        let mask_reflex = ve.cmp_ge(vt_reflex);
        let mask_retrieval = ve.cmp_ge(vt_retrieval);
        let mask_heavy = ve.cmp_ge(vt_heavy);

        // Convert masks to 1.0/0.0 using blend, then sum
        let add_reflex = mask_reflex.blend(v_one, v_zero);
        let add_retrieval = mask_retrieval.blend(v_one, v_zero);
        let add_heavy = mask_heavy.blend(v_one, v_zero);

        let lane_floats = add_reflex + add_retrieval + add_heavy;
        let lane_arr: [f32; 8] = lane_floats.into();

        // Convert to u8 (branchless)
        for i in 0..8 {
            cl[i] = (lane_arr[i] as u8).min(3);
        }
    }

    // Handle remainder
    for (i, &e) in remainder_e.iter().enumerate() {
        let lane = (e >= t_reflex) as u8 + (e >= t_retrieval) as u8 + (e >= t_heavy) as u8;
        lanes[offset + i] = lane.min(3);
    }
}

/// Convert lane assignments to ComputeLane enum values.
///
/// # Arguments
///
/// * `lane_bytes` - Raw lane assignments (0-3)
///
/// # Returns
///
/// Vector of `ComputeLane` values
pub fn lanes_to_enum(lane_bytes: &[u8]) -> Vec<ComputeLane> {
    lane_bytes
        .iter()
        .map(|&b| ComputeLane::from_u8(b).unwrap_or(ComputeLane::Human))
        .collect()
}

/// Compute total energy for a graph with batched operations.
///
/// This is the main entry point for efficient energy computation.
///
/// # Arguments
///
/// * `sources` - Flattened source states
/// * `targets` - Flattened target states
/// * `weights` - Edge weights
/// * `dim` - State vector dimension
/// * `count` - Number of edges
///
/// # Returns
///
/// Total coherence energy: `E(S) = sum(w_e * ||r_e||^2)`
#[inline]
pub fn compute_total_energy_simd(
    sources: &[f32],
    targets: &[f32],
    weights: &[f32],
    dim: usize,
    count: usize,
) -> f32 {
    debug_assert_eq!(sources.len(), dim * count);
    debug_assert_eq!(targets.len(), dim * count);
    debug_assert_eq!(weights.len(), count);

    // Compute residual norms
    let mut norms = vec![0.0f32; count];
    batch_residual_norms_simd(sources, targets, &mut norms, dim, count);

    // Compute weighted sum
    weighted_energy_sum_simd(&norms, weights)
}

/// Compute per-edge energies for a graph.
///
/// # Arguments
///
/// * `sources` - Flattened source states
/// * `targets` - Flattened target states
/// * `weights` - Edge weights
/// * `energies` - Output buffer for per-edge energies
/// * `dim` - State vector dimension
/// * `count` - Number of edges
#[inline]
pub fn compute_edge_energies_simd(
    sources: &[f32],
    targets: &[f32],
    weights: &[f32],
    energies: &mut [f32],
    dim: usize,
    count: usize,
) {
    debug_assert_eq!(sources.len(), dim * count);
    debug_assert_eq!(targets.len(), dim * count);
    debug_assert_eq!(weights.len(), count);
    debug_assert_eq!(energies.len(), count);

    // Compute residual norms
    batch_residual_norms_simd(sources, targets, energies, dim, count);

    // Multiply by weights in-place
    if count < 16 {
        for i in 0..count {
            energies[i] *= weights[i];
        }
        return;
    }

    let chunks_e = energies.chunks_exact_mut(8);
    let chunks_w = weights.chunks_exact(8);

    let remainder_w = chunks_w.remainder();
    let offset = count - remainder_w.len();

    for (ce, cw) in chunks_e.zip(chunks_w) {
        let ve = load_f32x8(ce);
        let vw = load_f32x8(cw);
        let result = ve * vw;
        store_f32x8(ce, result);
    }

    for (i, &w) in remainder_w.iter().enumerate() {
        energies[offset + i] *= w;
    }
}

// ============================================================================
// Scalar Fallback Implementations
// ============================================================================

#[inline(always)]
fn batch_residuals_scalar(sources: &[f32], targets: &[f32], residuals: &mut [f32]) {
    for ((s, t), r) in sources.iter().zip(targets.iter()).zip(residuals.iter_mut()) {
        *r = s - t;
    }
}

#[inline(always)]
fn compute_residual_norm_sq_scalar(source: &[f32], target: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (&s, &t) in source.iter().zip(target.iter()) {
        let diff = s - t;
        sum += diff * diff;
    }
    sum
}

#[inline(always)]
fn weighted_energy_sum_scalar(norms: &[f32], weights: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (&n, &w) in norms.iter().zip(weights.iter()) {
        sum += n * w;
    }
    sum
}

#[inline(always)]
fn batch_lane_assignment_scalar(energies: &[f32], thresholds: [f32; 4], lanes: &mut [u8]) {
    let t_reflex = thresholds[0];
    let t_retrieval = thresholds[1];
    let t_heavy = thresholds[2];

    for (e, l) in energies.iter().zip(lanes.iter_mut()) {
        let lane = (*e >= t_reflex) as u8 + (*e >= t_retrieval) as u8 + (*e >= t_heavy) as u8;
        *l = lane.min(3);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

#[inline(always)]
fn load_f32x8(slice: &[f32]) -> f32x8 {
    debug_assert!(slice.len() >= 8);
    // Use try_into for direct memory copy instead of element-by-element
    let arr: [f32; 8] = slice[..8].try_into().unwrap();
    f32x8::from(arr)
}

#[inline(always)]
fn store_f32x8(slice: &mut [f32], v: f32x8) {
    debug_assert!(slice.len() >= 8);
    let arr: [f32; 8] = v.into();
    slice[..8].copy_from_slice(&arr);
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        // Use relative error for larger values
        let max_abs = a.abs().max(b.abs());
        if max_abs > 1.0 {
            (a - b).abs() / max_abs < EPSILON
        } else {
            (a - b).abs() < EPSILON
        }
    }

    #[test]
    fn test_batch_residuals_small() {
        let sources = [1.0, 2.0, 3.0, 4.0];
        let targets = [0.5, 1.5, 2.5, 3.5];
        let mut residuals = [0.0f32; 4];

        batch_residuals_simd(&sources, &targets, &mut residuals, 2, 2);

        let expected = [0.5, 0.5, 0.5, 0.5];
        for (i, (&r, &e)) in residuals.iter().zip(expected.iter()).enumerate() {
            assert!(approx_eq(r, e), "at {} got {} expected {}", i, r, e);
        }
    }

    #[test]
    fn test_batch_residuals_large() {
        let n = 1024;
        let sources: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let targets: Vec<f32> = (0..n).map(|i| i as f32 * 0.5).collect();
        let mut residuals_simd = vec![0.0f32; n];
        let mut residuals_scalar = vec![0.0f32; n];

        batch_residuals_simd(&sources, &targets, &mut residuals_simd, 64, 16);
        batch_residuals_scalar(&sources, &targets, &mut residuals_scalar);

        for (i, (&s, &sc)) in residuals_simd
            .iter()
            .zip(residuals_scalar.iter())
            .enumerate()
        {
            assert!(approx_eq(s, sc), "at {} got {} expected {}", i, s, sc);
        }
    }

    #[test]
    fn test_batch_residual_norms() {
        // 2 edges, dim=2
        let sources = [1.0, 0.0, 0.0, 1.0];
        let targets = [0.0, 0.0, 1.0, 0.0];
        let mut norms = [0.0f32; 2];

        batch_residual_norms_simd(&sources, &targets, &mut norms, 2, 2);

        // Edge 0: ||(1,0) - (0,0)||^2 = 1
        // Edge 1: ||(0,1) - (1,0)||^2 = 1 + 1 = 2
        assert!(approx_eq(norms[0], 1.0), "got {}", norms[0]);
        assert!(approx_eq(norms[1], 2.0), "got {}", norms[1]);
    }

    #[test]
    fn test_weighted_energy_sum() {
        let norms = [1.0, 4.0, 9.0, 16.0];
        let weights = [1.0, 0.5, 0.25, 0.125];

        let result = weighted_energy_sum_simd(&norms, &weights);
        // 1*1 + 0.5*4 + 0.25*9 + 0.125*16 = 1 + 2 + 2.25 + 2 = 7.25
        assert!(approx_eq(result, 7.25), "got {}", result);
    }

    #[test]
    fn test_weighted_energy_sum_large() {
        let n = 1024;
        let norms: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let weights: Vec<f32> = (0..n).map(|_| 0.5).collect();

        let result = weighted_energy_sum_simd(&norms, &weights);
        let expected = weighted_energy_sum_scalar(&norms, &weights);
        assert!(
            approx_eq(result, expected),
            "got {} expected {}",
            result,
            expected
        );
    }

    #[test]
    fn test_batch_lane_assignment() {
        let energies = [0.1, 0.25, 0.6, 0.9];
        let thresholds = [0.2, 0.5, 0.8, 1.0];
        let mut lanes = [0u8; 4];

        batch_lane_assignment_simd(&energies, thresholds, &mut lanes);

        // 0.1 < 0.2 -> Lane 0
        // 0.2 <= 0.25 < 0.5 -> Lane 1
        // 0.5 <= 0.6 < 0.8 -> Lane 2
        // 0.8 <= 0.9 < 1.0 -> Lane 3
        assert_eq!(lanes, [0, 1, 2, 3]);
    }

    #[test]
    fn test_batch_lane_assignment_large() {
        let n = 1024;
        let energies: Vec<f32> = (0..n).map(|i| (i as f32) / (n as f32)).collect();
        let thresholds = [0.2, 0.5, 0.8, 1.0];
        let mut lanes_simd = vec![0u8; n];
        let mut lanes_scalar = vec![0u8; n];

        batch_lane_assignment_simd(&energies, thresholds, &mut lanes_simd);
        batch_lane_assignment_scalar(&energies, thresholds, &mut lanes_scalar);

        assert_eq!(lanes_simd, lanes_scalar);
    }

    #[test]
    fn test_compute_total_energy() {
        // 2 edges, dim=2
        let sources = [1.0, 0.0, 0.0, 1.0];
        let targets = [0.0, 0.0, 1.0, 0.0];
        let weights = [1.0, 2.0];

        let energy = compute_total_energy_simd(&sources, &targets, &weights, 2, 2);

        // Edge 0: w=1, ||r||^2 = 1 -> energy = 1
        // Edge 1: w=2, ||r||^2 = 2 -> energy = 4
        // Total = 5
        assert!(approx_eq(energy, 5.0), "got {}", energy);
    }

    #[test]
    fn test_compute_edge_energies() {
        let sources = [1.0, 0.0, 0.0, 1.0];
        let targets = [0.0, 0.0, 1.0, 0.0];
        let weights = [1.0, 2.0];
        let mut energies = [0.0f32; 2];

        compute_edge_energies_simd(&sources, &targets, &weights, &mut energies, 2, 2);

        assert!(approx_eq(energies[0], 1.0), "got {}", energies[0]);
        assert!(approx_eq(energies[1], 4.0), "got {}", energies[1]);
    }

    #[test]
    fn test_lanes_to_enum() {
        let bytes = [0u8, 1, 2, 3, 0];
        let lanes = lanes_to_enum(&bytes);

        assert_eq!(lanes[0], ComputeLane::Reflex);
        assert_eq!(lanes[1], ComputeLane::Retrieval);
        assert_eq!(lanes[2], ComputeLane::Heavy);
        assert_eq!(lanes[3], ComputeLane::Human);
        assert_eq!(lanes[4], ComputeLane::Reflex);
    }

    #[test]
    fn test_residual_norm_consistency() {
        // Verify SIMD and scalar produce same results
        let n = 128;
        let source: Vec<f32> = (0..n).map(|i| (i as f32) * 0.1).collect();
        let target: Vec<f32> = (0..n).map(|i| (i as f32) * 0.2).collect();

        let simd_result = compute_residual_norm_sq_simd(&source, &target);
        let scalar_result = compute_residual_norm_sq_scalar(&source, &target);

        assert!(
            approx_eq(simd_result, scalar_result),
            "simd={} scalar={}",
            simd_result,
            scalar_result
        );
    }
}
