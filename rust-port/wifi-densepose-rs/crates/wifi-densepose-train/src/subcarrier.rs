//! Subcarrier interpolation and selection utilities.
//!
//! This module provides functions to resample CSI subcarrier arrays between
//! different subcarrier counts using linear interpolation, and to select
//! the most informative subcarriers based on signal variance.
//!
//! # Example
//!
//! ```rust
//! use wifi_densepose_train::subcarrier::interpolate_subcarriers;
//! use ndarray::Array4;
//!
//! // Resample from 114 → 56 subcarriers
//! let arr = Array4::<f32>::zeros((100, 3, 3, 114));
//! let resampled = interpolate_subcarriers(&arr, 56);
//! assert_eq!(resampled.shape(), &[100, 3, 3, 56]);
//! ```

use ndarray::{Array4, s};

// ---------------------------------------------------------------------------
// interpolate_subcarriers
// ---------------------------------------------------------------------------

/// Resample a 4-D CSI array along the subcarrier axis (last dimension) to
/// `target_sc` subcarriers using linear interpolation.
///
/// # Arguments
///
/// - `arr`: Input array with shape `[T, n_tx, n_rx, n_sc]`.
/// - `target_sc`: Number of output subcarriers.
///
/// # Returns
///
/// A new array with shape `[T, n_tx, n_rx, target_sc]`.
///
/// # Panics
///
/// Panics if `target_sc == 0` or the input has no subcarrier dimension.
pub fn interpolate_subcarriers(arr: &Array4<f32>, target_sc: usize) -> Array4<f32> {
    assert!(target_sc > 0, "target_sc must be > 0");

    let shape = arr.shape();
    let (n_t, n_tx, n_rx, n_sc) = (shape[0], shape[1], shape[2], shape[3]);

    if n_sc == target_sc {
        return arr.clone();
    }

    let mut out = Array4::<f32>::zeros((n_t, n_tx, n_rx, target_sc));

    // Precompute interpolation weights once.
    let weights = compute_interp_weights(n_sc, target_sc);

    for t in 0..n_t {
        for tx in 0..n_tx {
            for rx in 0..n_rx {
                let src = arr.slice(s![t, tx, rx, ..]);
                let src_slice = src.as_slice().unwrap_or_else(|| {
                    // Fallback: copy to a contiguous slice
                    // (this path is hit when the array has a non-contiguous layout)
                    // In practice ndarray arrays sliced along last dim are contiguous.
                    panic!("Subcarrier slice is not contiguous");
                });

                for (k, &(i0, i1, w)) in weights.iter().enumerate() {
                    let v = src_slice[i0] * (1.0 - w) + src_slice[i1] * w;
                    out[[t, tx, rx, k]] = v;
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// compute_interp_weights
// ---------------------------------------------------------------------------

/// Compute linear interpolation indices and fractional weights for resampling
/// from `src_sc` to `target_sc` subcarriers.
///
/// Returns a `Vec` of `(i0, i1, frac)` tuples where each output subcarrier `k`
/// is computed as `src[i0] * (1 - frac) + src[i1] * frac`.
///
/// # Arguments
///
/// - `src_sc`: Number of subcarriers in the source array.
/// - `target_sc`: Number of subcarriers in the output array.
///
/// # Panics
///
/// Panics if `src_sc == 0` or `target_sc == 0`.
pub fn compute_interp_weights(src_sc: usize, target_sc: usize) -> Vec<(usize, usize, f32)> {
    assert!(src_sc > 0, "src_sc must be > 0");
    assert!(target_sc > 0, "target_sc must be > 0");

    let mut weights = Vec::with_capacity(target_sc);

    for k in 0..target_sc {
        // Map output index k to a continuous position in the source array.
        // Scale so that index 0 maps to 0 and index (target_sc-1) maps to
        // (src_sc-1) — i.e., endpoints are preserved.
        let pos = if target_sc == 1 {
            0.0f32
        } else {
            k as f32 * (src_sc - 1) as f32 / (target_sc - 1) as f32
        };

        let i0 = (pos.floor() as usize).min(src_sc - 1);
        let i1 = (pos.ceil() as usize).min(src_sc - 1);
        let frac = pos - pos.floor();

        weights.push((i0, i1, frac));
    }

    weights
}

// ---------------------------------------------------------------------------
// select_subcarriers_by_variance
// ---------------------------------------------------------------------------

/// Select the `k` most informative subcarrier indices based on temporal variance.
///
/// Computes the variance of each subcarrier across the time and antenna
/// dimensions, then returns the indices of the `k` subcarriers with the
/// highest variance, sorted in ascending order.
///
/// # Arguments
///
/// - `arr`: Input array with shape `[T, n_tx, n_rx, n_sc]`.
/// - `k`: Number of subcarriers to select.
///
/// # Returns
///
/// A `Vec<usize>` of length `k` with the selected subcarrier indices (ascending).
///
/// # Panics
///
/// Panics if `k == 0` or `k > n_sc`.
pub fn select_subcarriers_by_variance(arr: &Array4<f32>, k: usize) -> Vec<usize> {
    let shape = arr.shape();
    let n_sc = shape[3];

    assert!(k > 0, "k must be > 0");
    assert!(k <= n_sc, "k ({k}) must be <= n_sc ({n_sc})");

    let total_elems = shape[0] * shape[1] * shape[2];

    // Compute mean per subcarrier.
    let mut means = vec![0.0f64; n_sc];
    for sc in 0..n_sc {
        let col = arr.slice(s![.., .., .., sc]);
        let sum: f64 = col.iter().map(|&v| v as f64).sum();
        means[sc] = sum / total_elems as f64;
    }

    // Compute variance per subcarrier.
    let mut variances = vec![0.0f64; n_sc];
    for sc in 0..n_sc {
        let col = arr.slice(s![.., .., .., sc]);
        let mean = means[sc];
        let var: f64 = col.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>()
            / total_elems as f64;
        variances[sc] = var;
    }

    // Rank subcarriers by descending variance.
    let mut ranked: Vec<usize> = (0..n_sc).collect();
    ranked.sort_by(|&a, &b| variances[b].partial_cmp(&variances[a]).unwrap_or(std::cmp::Ordering::Equal));

    // Take top-k and sort ascending for a canonical representation.
    let mut selected: Vec<usize> = ranked[..k].to_vec();
    selected.sort_unstable();
    selected
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn identity_resample() {
        let arr = Array4::<f32>::from_shape_fn((4, 3, 3, 56), |(t, tx, rx, k)| {
            (t + tx + rx + k) as f32
        });
        let out = interpolate_subcarriers(&arr, 56);
        assert_eq!(out.shape(), arr.shape());
        // Identity resample must preserve all values exactly.
        for v in arr.iter().zip(out.iter()) {
            assert_abs_diff_eq!(v.0, v.1, epsilon = 1e-6);
        }
    }

    #[test]
    fn upsample_endpoints_preserved() {
        // When resampling from 4 → 8 the first and last values are exact.
        let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 4), |(_, _, _, k)| k as f32);
        let out = interpolate_subcarriers(&arr, 8);
        assert_eq!(out.shape(), &[1, 1, 1, 8]);
        assert_abs_diff_eq!(out[[0, 0, 0, 0]], 0.0_f32, epsilon = 1e-6);
        assert_abs_diff_eq!(out[[0, 0, 0, 7]], 3.0_f32, epsilon = 1e-6);
    }

    #[test]
    fn downsample_endpoints_preserved() {
        // Downsample from 8 → 4.
        let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 8), |(_, _, _, k)| k as f32 * 2.0);
        let out = interpolate_subcarriers(&arr, 4);
        assert_eq!(out.shape(), &[1, 1, 1, 4]);
        // First value: 0.0, last value: 14.0
        assert_abs_diff_eq!(out[[0, 0, 0, 0]], 0.0_f32, epsilon = 1e-5);
        assert_abs_diff_eq!(out[[0, 0, 0, 3]], 14.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn compute_interp_weights_identity() {
        let w = compute_interp_weights(5, 5);
        assert_eq!(w.len(), 5);
        for (k, &(i0, i1, frac)) in w.iter().enumerate() {
            assert_eq!(i0, k);
            assert_eq!(i1, k);
            assert_abs_diff_eq!(frac, 0.0_f32, epsilon = 1e-6);
        }
    }

    #[test]
    fn select_subcarriers_returns_correct_count() {
        let arr = Array4::<f32>::from_shape_fn((10, 3, 3, 56), |(t, _, _, k)| {
            (t * k) as f32
        });
        let selected = select_subcarriers_by_variance(&arr, 8);
        assert_eq!(selected.len(), 8);
    }

    #[test]
    fn select_subcarriers_sorted_ascending() {
        let arr = Array4::<f32>::from_shape_fn((10, 3, 3, 56), |(t, _, _, k)| {
            (t * k) as f32
        });
        let selected = select_subcarriers_by_variance(&arr, 10);
        for w in selected.windows(2) {
            assert!(w[0] < w[1], "Indices must be sorted ascending");
        }
    }

    #[test]
    fn select_subcarriers_all_same_returns_all() {
        // When all subcarriers have zero variance, the function should still
        // return k valid indices.
        let arr = Array4::<f32>::ones((5, 2, 2, 20));
        let selected = select_subcarriers_by_variance(&arr, 5);
        assert_eq!(selected.len(), 5);
        // All selected indices must be in [0, 19]
        for &idx in &selected {
            assert!(idx < 20);
        }
    }
}
