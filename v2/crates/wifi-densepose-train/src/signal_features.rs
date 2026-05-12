//! Hand-off layer between raw windowed CSI and the SOTA signal-processing
//! crate ([`wifi_densepose_signal`]).
//!
//! Historically `wifi-densepose-signal` was listed as a dependency of this
//! crate but never imported — the training pipeline only ever consumed the
//! raw amplitude/phase tensors. This module wires the two together: it takes
//! a windowed CSI observation and runs it through
//! [`wifi_densepose_signal::features::FeatureExtractor`] to derive a compact,
//! fixed-length feature vector (amplitude statistics, phase coherence, and a
//! power-spectral-density summary).
//!
//! These derived features are the building block for a future vitals /
//! multi-task supervision head (breathing-band and heart-rate-band power can
//! be read off the PSD summary); for now they are produced on demand via
//! [`extract_signal_features`] / [`crate::dataset::CsiSample::signal_features`]
//! and are not yet fed back into the loss. Wiring them as a training target
//! is tracked as a follow-up to the 2026-05-11 training-pipeline audit.

use ndarray::{s, Array1, Array4};
use wifi_densepose_signal::csi_processor::CsiData;
use wifi_densepose_signal::features::FeatureExtractor;

/// Length of the vector returned by [`extract_signal_features`].
///
/// The layout is:
/// 1. amplitude peak
/// 2. amplitude RMS
/// 3. amplitude dynamic range (max − min)
/// 4. mean of the per-subcarrier amplitude means
/// 5. mean of the per-subcarrier amplitude variances
/// 6. phase coherence
/// 7. mean of the per-subcarrier phase variances
/// 8. PSD total power
/// 9. PSD peak power
/// 10. PSD peak frequency (Hz)
/// 11. PSD spectral centroid
/// 12. PSD spectral bandwidth
pub const FEATURE_LEN: usize = 12;

/// Default centre frequency assumed when the CSI window carries no metadata.
const DEFAULT_CENTRE_FREQ_HZ: f64 = 2.4e9;

/// Default channel bandwidth (HT40) assumed when the CSI window carries no
/// metadata.
const DEFAULT_BANDWIDTH_HZ: f64 = 40.0e6;

/// Derive a compact, fixed-length ([`FEATURE_LEN`]) signal-processing feature
/// vector from a windowed CSI observation by running its centre frame through
/// [`wifi_densepose_signal::features::FeatureExtractor`].
///
/// `amplitude` and `phase` are `[window_frames, n_tx, n_rx, n_subcarriers]`
/// tensors (the [`crate::dataset::CsiSample`] layout). The centre frame is
/// flattened to `[n_tx · n_rx, n_subcarriers]` (the antenna-major shape the
/// signal crate expects) and converted to `f64`.
///
/// The returned values are always finite for finite input: the underlying
/// extractors clamp degenerate cases, and any non-finite result is mapped to
/// `0.0` so callers can rely on the vector being usable as a model feature.
pub fn extract_signal_features(amplitude: &Array4<f32>, phase: &Array4<f32>) -> Array1<f32> {
    let (n_t, n_tx, n_rx, n_sc) = amplitude.dim();
    debug_assert_eq!(amplitude.dim(), phase.dim(), "amplitude/phase shape mismatch");
    if n_t == 0 || n_tx == 0 || n_rx == 0 || n_sc == 0 {
        return Array1::zeros(FEATURE_LEN);
    }
    let n_ant = n_tx * n_rx;
    let t = n_t / 2;

    let to_2d = |src: &Array4<f32>| -> Vec<f64> {
        src.slice(s![t, .., .., ..]).iter().map(|&v| f64::from(v)).collect()
    };
    let amp2d = match ndarray::Array2::from_shape_vec((n_ant, n_sc), to_2d(amplitude)) {
        Ok(a) => a,
        Err(_) => return Array1::zeros(FEATURE_LEN),
    };
    let phase2d = match ndarray::Array2::from_shape_vec((n_ant, n_sc), to_2d(phase)) {
        Ok(p) => p,
        Err(_) => return Array1::zeros(FEATURE_LEN),
    };

    let csi = match CsiData::builder()
        .amplitude(amp2d)
        .phase(phase2d)
        .frequency(DEFAULT_CENTRE_FREQ_HZ)
        .bandwidth(DEFAULT_BANDWIDTH_HZ)
        .build()
    {
        Ok(c) => c,
        Err(_) => return Array1::zeros(FEATURE_LEN),
    };

    let feats = FeatureExtractor::default_config().extract(&csi);

    let amp_mean_overall = mean_or_zero(feats.amplitude.mean.iter().copied());
    let amp_var_overall = mean_or_zero(feats.amplitude.variance.iter().copied());
    let phase_var_overall = mean_or_zero(feats.phase.variance.iter().copied());

    let raw = [
        feats.amplitude.peak,
        feats.amplitude.rms,
        feats.amplitude.dynamic_range,
        amp_mean_overall,
        amp_var_overall,
        feats.phase.coherence,
        phase_var_overall,
        feats.psd.total_power,
        feats.psd.peak_power,
        feats.psd.peak_frequency,
        feats.psd.centroid,
        feats.psd.bandwidth,
    ];
    debug_assert_eq!(raw.len(), FEATURE_LEN);
    Array1::from_iter(raw.iter().map(|&v| sanitise(v)))
}

/// Mean of an iterator of `f64`, or `0.0` if it is empty or non-finite.
fn mean_or_zero<I: Iterator<Item = f64>>(it: I) -> f64 {
    let (sum, n) = it.fold((0.0_f64, 0_usize), |(s, k), v| (s + v, k + 1));
    if n == 0 {
        0.0
    } else {
        sum / n as f64
    }
}

/// Map non-finite values to `0.0` and downcast to `f32`.
fn sanitise(v: f64) -> f32 {
    if v.is_finite() {
        v as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array4;

    #[test]
    fn zero_sized_input_yields_zero_vector() {
        let empty = Array4::<f32>::zeros((0, 0, 0, 0));
        let f = extract_signal_features(&empty, &empty);
        assert_eq!(f.len(), FEATURE_LEN);
        assert!(f.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn constant_input_is_finite_and_correct_length() {
        let amp = Array4::<f32>::from_elem((4, 3, 3, 56), 1.5);
        let phase = Array4::<f32>::from_elem((4, 3, 3, 56), 0.25);
        let f = extract_signal_features(&amp, &phase);
        assert_eq!(f.len(), FEATURE_LEN);
        assert!(f.iter().all(|v| v.is_finite()), "features must be finite: {f:?}");
    }
}
