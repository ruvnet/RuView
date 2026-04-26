//! Integration tests for [`wifi_densepose_train::subcarrier`].
//!
//! All test data is constructed from fixed, deterministic arrays — no `rand`
//! crate or OS entropy is used.  The same input always produces the same
//! output regardless of the platform or execution order.

use ndarray::Array4;
use wifi_densepose_train::subcarrier::{
    compute_interp_weights, interpolate_subcarriers, select_subcarriers_by_variance,
};

// ---------------------------------------------------------------------------
// Output shape tests
// ---------------------------------------------------------------------------

/// Resampling 114 → 56 subcarriers must produce shape [T, n_tx, n_rx, 56].
#[test]
fn resample_114_to_56_output_shape() {
    let t = 10_usize;
    let n_tx = 3_usize;
    let n_rx = 3_usize;
    let src_sc = 114_usize;
    let tgt_sc = 56_usize;

    // Deterministic data: value = t_idx + tx + rx + k (no randomness).
    let arr = Array4::<f32>::from_shape_fn((t, n_tx, n_rx, src_sc), |(ti, tx, rx, k)| {
        (ti + tx + rx + k) as f32
    });

    let out = interpolate_subcarriers(&arr, tgt_sc);

    assert_eq!(
        out.shape(),
        &[t, n_tx, n_rx, tgt_sc],
        "resampled shape must be [{t}, {n_tx}, {n_rx}, {tgt_sc}], got {:?}",
        out.shape()
    );
}

/// Resampling 56 → 114 (upsampling) must produce shape [T, n_tx, n_rx, 114].
#[test]
fn resample_56_to_114_output_shape() {
    let arr = Array4::<f32>::from_shape_fn((8, 2, 2, 56), |(ti, tx, rx, k)| {
        (ti + tx + rx + k) as f32 * 0.1
    });

    let out = interpolate_subcarriers(&arr, 114);

    assert_eq!(
        out.shape(),
        &[8, 2, 2, 114],
        "upsampled shape must be [8, 2, 2, 114], got {:?}",
        out.shape()
    );
}

// ---------------------------------------------------------------------------
// Identity case: 56 → 56
// ---------------------------------------------------------------------------

/// Resampling from 56 → 56 subcarriers must return a tensor identical to the
/// input (element-wise equality within floating-point precision).
#[test]
fn identity_resample_56_to_56_preserves_values() {
    let arr = Array4::<f32>::from_shape_fn((5, 3, 3, 56), |(ti, tx, rx, k)| {
        // Deterministic: use a simple arithmetic formula.
        (ti as f32 * 1000.0 + tx as f32 * 100.0 + rx as f32 * 10.0 + k as f32).sin()
    });

    let out = interpolate_subcarriers(&arr, 56);

    assert_eq!(
        out.shape(),
        arr.shape(),
        "identity resample must preserve shape"
    );

    for ((ti, tx, rx, k), orig) in arr.indexed_iter() {
        let resampled = out[[ti, tx, rx, k]];
        assert!(
            (resampled - orig).abs() < 1e-5,
            "identity resample mismatch at [{ti},{tx},{rx},{k}]: \
             orig={orig}, resampled={resampled}"
        );
    }
}

// ---------------------------------------------------------------------------
// Monotone (linearly-increasing) input interpolates correctly
// ---------------------------------------------------------------------------

/// For a linearly-increasing input across the subcarrier axis, the resampled
/// output must also be linearly increasing (all values lie on the same line).
#[test]
fn monotone_input_interpolates_linearly() {
    // src[k] = k as f32 for k in 0..8 — a straight line through the origin.
    let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 8), |(_, _, _, k)| k as f32);

    let out = interpolate_subcarriers(&arr, 16);

    // The output must be a linearly-spaced sequence from 0.0 to 7.0.
    // out[i] = i * 7.0 / 15.0   (endpoints preserved by the mapping).
    for i in 0..16_usize {
        let expected = i as f32 * 7.0 / 15.0;
        let actual = out[[0, 0, 0, i]];
        assert!(
            (actual - expected).abs() < 1e-5,
            "linear interpolation wrong at index {i}: expected {expected}, got {actual}"
        );
    }
}

/// Downsampling a linearly-increasing input must also produce a linear output.
#[test]
fn monotone_downsample_interpolates_linearly() {
    // src[k] = k * 2.0 for k in 0..16 (values 0, 2, 4, …, 30).
    let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 16), |(_, _, _, k)| k as f32 * 2.0);

    let out = interpolate_subcarriers(&arr, 8);

    // out[i] = i * 30.0 / 7.0  (endpoints at 0.0 and 30.0).
    for i in 0..8_usize {
        let expected = i as f32 * 30.0 / 7.0;
        let actual = out[[0, 0, 0, i]];
        assert!(
            (actual - expected).abs() < 1e-4,
            "linear downsampling wrong at index {i}: expected {expected}, got {actual}"
        );
    }
}

// ---------------------------------------------------------------------------
// Boundary value preservation
// ---------------------------------------------------------------------------

/// The first output subcarrier must equal the first input subcarrier exactly.
#[test]
fn boundary_first_subcarrier_preserved_on_downsample() {
    // Fixed non-trivial values so we can verify the exact first element.
    let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 114), |(_, _, _, k)| {
        (k as f32 * 0.1 + 1.0).ln()   // deterministic, non-trivial
    });
    let first_value = arr[[0, 0, 0, 0]];

    let out = interpolate_subcarriers(&arr, 56);

    let first_out = out[[0, 0, 0, 0]];
    assert!(
        (first_out - first_value).abs() < 1e-5,
        "first output subcarrier must equal first input subcarrier: \
         expected {first_value}, got {first_out}"
    );
}

/// The last output subcarrier must equal the last input subcarrier exactly.
#[test]
fn boundary_last_subcarrier_preserved_on_downsample() {
    let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 114), |(_, _, _, k)| {
        (k as f32 * 0.1 + 1.0).ln()
    });
    let last_input = arr[[0, 0, 0, 113]];

    let out = interpolate_subcarriers(&arr, 56);

    let last_output = out[[0, 0, 0, 55]];
    assert!(
        (last_output - last_input).abs() < 1e-5,
        "last output subcarrier must equal last input subcarrier: \
         expected {last_input}, got {last_output}"
    );
}

/// The same boundary preservation holds when upsampling.
#[test]
fn boundary_endpoints_preserved_on_upsample() {
    let arr = Array4::<f32>::from_shape_fn((1, 1, 1, 56), |(_, _, _, k)| {
        (k as f32 * 0.05 + 0.5).powi(2)
    });
    let first_input = arr[[0, 0, 0, 0]];
    let last_input = arr[[0, 0, 0, 55]];

    let out = interpolate_subcarriers(&arr, 114);

    let first_output = out[[0, 0, 0, 0]];
    let last_output = out[[0, 0, 0, 113]];

    assert!(
        (first_output - first_input).abs() < 1e-5,
        "first output must equal first input on upsample: \
         expected {first_input}, got {first_output}"
    );
    assert!(
        (last_output - last_input).abs() < 1e-5,
        "last output must equal last input on upsample: \
         expected {last_input}, got {last_output}"
    );
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

/// Calling `interpolate_subcarriers` twice with the same input must yield
/// bit-identical results — no non-deterministic behavior allowed.
#[test]
fn resample_is_deterministic() {
    // Use a fixed deterministic array (seed=42 LCG-style arithmetic).
    let arr = Array4::<f32>::from_shape_fn((10, 3, 3, 114), |(ti, tx, rx, k)| {
        // Simple deterministic formula mimicking SyntheticDataset's LCG pattern.
        let idx = ti * 3 * 3 * 114 + tx * 3 * 114 + rx * 114 + k;
        // LCG: state = (a * state + c) mod m  with seed = 42
        let state_u64 = (6364136223846793005_u64)
            .wrapping_mul(idx as u64 + 42)
            .wrapping_add(1442695040888963407);
        ((state_u64 >> 33) as f32) / (u32::MAX as f32)  // in [0, 1)
    });

    let out1 = interpolate_subcarriers(&arr, 56);
    let out2 = interpolate_subcarriers(&arr, 56);

    for ((ti, tx, rx, k), v1) in out1.indexed_iter() {
        let v2 = out2[[ti, tx, rx, k]];
        assert_eq!(
            v1.to_bits(),
            v2.to_bits(),
            "bit-identical result required at [{ti},{tx},{rx},{k}]: \
             first={v1}, second={v2}"
        );
    }
}

/// Same input parameters → same `compute_interp_weights` output every time.
#[test]
fn compute_interp_weights_is_deterministic() {
    let w1 = compute_interp_weights(114, 56);
    let w2 = compute_interp_weights(114, 56);

    assert_eq!(w1.len(), w2.len(), "weight vector lengths must match");
    for (i, (a, b)) in w1.iter().zip(w2.iter()).enumerate() {
        assert_eq!(
            a, b,
            "weight at index {i} must be bit-identical across calls"
        );
    }
}

// ---------------------------------------------------------------------------
// compute_interp_weights properties
// ---------------------------------------------------------------------------

/// `compute_interp_weights(n, n)` must produce identity weights (i0==i1==k,
/// frac==0).
#[test]
fn compute_interp_weights_identity_case() {
    let n = 56_usize;
    let weights = compute_interp_weights(n, n);

    assert_eq!(weights.len(), n, "identity weights length must equal n");

    for (k, &(i0, i1, frac)) in weights.iter().enumerate() {
        assert_eq!(i0, k, "i0 must equal k for identity weights at {k}");
        assert_eq!(i1, k, "i1 must equal k for identity weights at {k}");
        assert!(
            frac.abs() < 1e-6,
            "frac must be 0 for identity weights at {k}, got {frac}"
        );
    }
}

/// `compute_interp_weights` must produce exactly `target_sc` entries.
#[test]
fn compute_interp_weights_correct_length() {
    let weights = compute_interp_weights(114, 56);
    assert_eq!(
        weights.len(),
        56,
        "114→56 weights must have 56 entries, got {}",
        weights.len()
    );
}

/// All weights must have fractions in [0, 1].
#[test]
fn compute_interp_weights_frac_in_unit_interval() {
    let weights = compute_interp_weights(114, 56);
    for (i, &(_, _, frac)) in weights.iter().enumerate() {
        assert!(
            frac >= 0.0 && frac <= 1.0 + 1e-6,
            "fractional weight at index {i} must be in [0, 1], got {frac}"
        );
    }
}

/// All i0 and i1 indices must be within bounds of the source array.
#[test]
fn compute_interp_weights_indices_in_bounds() {
    let src_sc = 114_usize;
    let weights = compute_interp_weights(src_sc, 56);
    for (k, &(i0, i1, _)) in weights.iter().enumerate() {
        assert!(
            i0 < src_sc,
            "i0={i0} at output {k} is out of bounds for src_sc={src_sc}"
        );
        assert!(
            i1 < src_sc,
            "i1={i1} at output {k} is out of bounds for src_sc={src_sc}"
        );
    }
}

// ---------------------------------------------------------------------------
// select_subcarriers_by_variance
// ---------------------------------------------------------------------------

/// `select_subcarriers_by_variance` must return exactly k indices.
#[test]
fn select_subcarriers_returns_k_indices() {
    let arr = Array4::<f32>::from_shape_fn((20, 3, 3, 56), |(ti, _, _, k)| {
        (ti * k) as f32
    });
    let selected = select_subcarriers_by_variance(&arr, 8);
    assert_eq!(
        selected.len(),
        8,
        "must select exactly 8 subcarriers, got {}",
        selected.len()
    );
}

/// The returned indices must be sorted in ascending order.
#[test]
fn select_subcarriers_indices_are_sorted_ascending() {
    let arr = Array4::<f32>::from_shape_fn((10, 2, 2, 56), |(ti, tx, rx, k)| {
        (ti + tx * 3 + rx * 7 + k * 11) as f32
    });
    let selected = select_subcarriers_by_variance(&arr, 10);
    for window in selected.windows(2) {
        assert!(
            window[0] < window[1],
            "selected indices must be strictly ascending: {:?}",
            selected
        );
    }
}

/// All returned indices must be within [0, n_sc).
#[test]
fn select_subcarriers_indices_are_valid() {
    let n_sc = 56_usize;
    let arr = Array4::<f32>::from_shape_fn((8, 3, 3, n_sc), |(ti, _, _, k)| {
        (ti as f32 * 0.7 + k as f32 * 1.3).cos()
    });
    let selected = select_subcarriers_by_variance(&arr, 5);
    for &idx in &selected {
        assert!(
            idx < n_sc,
            "selected index {idx} is out of bounds for n_sc={n_sc}"
        );
    }
}

/// High-variance subcarriers should be preferred over low-variance ones.
/// Create an array where subcarriers 0..4 have zero variance and
/// subcarriers 4..8 have high variance — the top-4 selection must exclude 0..4.
#[test]
fn select_subcarriers_prefers_high_variance() {
    // Subcarriers 0..4: constant value 0.5 (zero variance).
    // Subcarriers 4..8: vary wildly across time (high variance).
    let arr = Array4::<f32>::from_shape_fn((20, 1, 1, 8), |(ti, _, _, k)| {
        if k < 4 {
            0.5_f32 // constant across time → zero variance
        } else {
            // High variance: alternating +100 / -100 depending on time.
            if ti % 2 == 0 { 100.0 } else { -100.0 }
        }
    });

    let selected = select_subcarriers_by_variance(&arr, 4);

    // All selected indices should be in {4, 5, 6, 7}.
    for &idx in &selected {
        assert!(
            idx >= 4,
            "expected only high-variance subcarriers (4..8) to be selected, \
             but got index {idx}: selected = {:?}",
            selected
        );
    }
}
