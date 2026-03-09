//! Cross-channel coupling and connectivity metrics.
//!
//! Provides measures of functional connectivity between neural signals:
//! - Phase Locking Value (PLV)
//! - Magnitude-squared coherence
//! - Imaginary coherence (robust to volume conduction)
//! - Amplitude envelope correlation
//! - Full connectivity matrix computation

use num_complex::Complex;
use ruv_neural_core::signal::{FrequencyBand, MultiChannelTimeSeries};
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::f64::consts::PI;

use crate::filter::BandpassFilter;
use crate::hilbert::hilbert_transform;

thread_local! {
    static FFT_PLANNER: RefCell<FftPlanner<f64>> = RefCell::new(FftPlanner::new());
}

/// Type of connectivity metric to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectivityMetric {
    /// Phase Locking Value.
    Plv,
    /// Amplitude envelope correlation.
    Aec,
}

/// Returns `true` if any sample in `data` is NaN or infinite.
pub fn contains_non_finite(data: &[f64]) -> bool {
    data.iter().any(|x| !x.is_finite())
}

/// Validate that signal data contains no NaN or Inf values.
///
/// Returns `Ok(())` if all values are finite, or an error otherwise.
pub fn validate_signal_finite(data: &[f64], label: &str) -> std::result::Result<(), String> {
    if contains_non_finite(data) {
        Err(format!("{label} contains NaN or infinite values"))
    } else {
        Ok(())
    }
}

/// Compute the Phase Locking Value (PLV) between two signals.
///
/// PLV = |mean(exp(j * (phase_a - phase_b)))|
///
/// The signals are first bandpass-filtered to the specified frequency band,
/// then the Hilbert transform extracts instantaneous phase.
///
/// PLV = 1.0 indicates perfect phase synchrony;
/// PLV ~ 0.0 indicates no consistent phase relationship.
///
/// # Arguments
/// * `signal_a` - First channel time series
/// * `signal_b` - Second channel time series
/// * `sample_rate` - Sampling rate in Hz
/// * `band` - Frequency band for phase extraction
pub fn phase_locking_value(
    signal_a: &[f64],
    signal_b: &[f64],
    sample_rate: f64,
    band: FrequencyBand,
) -> f64 {
    let n = signal_a.len().min(signal_b.len());
    if n < 4 {
        return 0.0;
    }

    // Reject NaN/Inf at the pipeline entry point
    if contains_non_finite(&signal_a[..n]) || contains_non_finite(&signal_b[..n]) {
        return 0.0;
    }

    let (low, high) = band.range_hz();
    let bp = BandpassFilter::new(2, low, high, sample_rate);

    let filtered_a = bp.apply(&signal_a[..n]);
    let filtered_b = bp.apply(&signal_b[..n]);

    let analytic_a = hilbert_transform(&filtered_a);
    let analytic_b = hilbert_transform(&filtered_b);

    // Compute mean of exp(j*(phase_a - phase_b))
    let mut sum = Complex::new(0.0, 0.0);
    for i in 0..n {
        let phase_a = analytic_a[i].im.atan2(analytic_a[i].re);
        let phase_b = analytic_b[i].im.atan2(analytic_b[i].re);
        let diff = phase_a - phase_b;
        sum += Complex::new(diff.cos(), diff.sin());
    }

    (sum / n as f64).norm()
}

/// Compute magnitude-squared coherence between two signals.
///
/// Coh(f) = |S_ab(f)|^2 / (S_aa(f) * S_bb(f))
///
/// Uses Welch's method with overlapping segments and Hann window.
///
/// # Returns
/// Vector of (frequency, coherence) pairs.
pub fn coherence(
    signal_a: &[f64],
    signal_b: &[f64],
    sample_rate: f64,
) -> Vec<(f64, f64)> {
    let n = signal_a.len().min(signal_b.len());
    if n == 0 {
        return Vec::new();
    }

    let window_size = 256.min(n);
    let overlap = window_size / 2;
    let hop = window_size - overlap;

    let window = hann_window(window_size);
    let num_freqs = window_size / 2 + 1;

    let fft = FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(window_size));

    let mut saa = vec![0.0; num_freqs];
    let mut sbb = vec![0.0; num_freqs];
    let mut sab = vec![Complex::new(0.0, 0.0); num_freqs];
    let mut num_segments = 0;

    let mut start = 0;
    while start + window_size <= n {
        let mut fa: Vec<Complex<f64>> = (0..window_size)
            .map(|i| Complex::new(signal_a[start + i] * window[i], 0.0))
            .collect();
        let mut fb: Vec<Complex<f64>> = (0..window_size)
            .map(|i| Complex::new(signal_b[start + i] * window[i], 0.0))
            .collect();

        fft.process(&mut fa);
        fft.process(&mut fb);

        for k in 0..num_freqs {
            saa[k] += fa[k].norm_sqr();
            sbb[k] += fb[k].norm_sqr();
            sab[k] += fa[k] * fb[k].conj();
        }
        num_segments += 1;
        start += hop;
    }

    if num_segments == 0 {
        return Vec::new();
    }

    let freq_res = sample_rate / window_size as f64;
    (0..num_freqs)
        .map(|k| {
            let freq = k as f64 * freq_res;
            let denom = saa[k] * sbb[k];
            let coh = if denom > 1e-30 {
                sab[k].norm_sqr() / denom
            } else {
                0.0
            };
            (freq, coh.min(1.0))
        })
        .collect()
}

/// Compute imaginary coherence between two signals.
///
/// ImCoh(f) = Im(S_ab(f)) / sqrt(S_aa(f) * S_bb(f))
///
/// The imaginary part of coherence is robust to volume conduction
/// artifacts, which produce zero-lag (purely real) correlations.
///
/// # Returns
/// Vector of (frequency, imaginary_coherence) pairs.
pub fn imaginary_coherence(
    signal_a: &[f64],
    signal_b: &[f64],
    sample_rate: f64,
) -> Vec<(f64, f64)> {
    let n = signal_a.len().min(signal_b.len());
    if n == 0 {
        return Vec::new();
    }

    let window_size = 256.min(n);
    let overlap = window_size / 2;
    let hop = window_size - overlap;

    let window = hann_window(window_size);
    let num_freqs = window_size / 2 + 1;

    let fft = FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(window_size));

    let mut saa = vec![0.0; num_freqs];
    let mut sbb = vec![0.0; num_freqs];
    let mut sab = vec![Complex::new(0.0, 0.0); num_freqs];
    let mut num_segments = 0;

    let mut start = 0;
    while start + window_size <= n {
        let mut fa: Vec<Complex<f64>> = (0..window_size)
            .map(|i| Complex::new(signal_a[start + i] * window[i], 0.0))
            .collect();
        let mut fb: Vec<Complex<f64>> = (0..window_size)
            .map(|i| Complex::new(signal_b[start + i] * window[i], 0.0))
            .collect();

        fft.process(&mut fa);
        fft.process(&mut fb);

        for k in 0..num_freqs {
            saa[k] += fa[k].norm_sqr();
            sbb[k] += fb[k].norm_sqr();
            sab[k] += fa[k] * fb[k].conj();
        }
        num_segments += 1;
        start += hop;
    }

    if num_segments == 0 {
        return Vec::new();
    }

    let freq_res = sample_rate / window_size as f64;
    (0..num_freqs)
        .map(|k| {
            let freq = k as f64 * freq_res;
            let denom = (saa[k] * sbb[k]).sqrt();
            let im_coh = if denom > 1e-30 {
                sab[k].im / denom
            } else {
                0.0
            };
            (freq, im_coh)
        })
        .collect()
}

/// Compute amplitude envelope correlation between two signals.
///
/// 1. Bandpass filter both signals to the specified frequency band
/// 2. Extract amplitude envelopes via Hilbert transform
/// 3. Compute Pearson correlation of the envelopes
///
/// # Returns
/// Correlation coefficient in [-1, 1].
pub fn amplitude_envelope_correlation(
    signal_a: &[f64],
    signal_b: &[f64],
    sample_rate: f64,
    band: FrequencyBand,
) -> f64 {
    let n = signal_a.len().min(signal_b.len());
    if n < 4 {
        return 0.0;
    }

    // Reject NaN/Inf at the pipeline entry point
    if contains_non_finite(&signal_a[..n]) || contains_non_finite(&signal_b[..n]) {
        return 0.0;
    }

    let (low, high) = band.range_hz();
    let bp = BandpassFilter::new(2, low, high, sample_rate);

    let filtered_a = bp.apply(&signal_a[..n]);
    let filtered_b = bp.apply(&signal_b[..n]);

    let env_a = crate::hilbert::instantaneous_amplitude(&filtered_a);
    let env_b = crate::hilbert::instantaneous_amplitude(&filtered_b);

    pearson_correlation(&env_a, &env_b)
}

/// Compute a full connectivity matrix for all channel pairs.
///
/// Pre-computes filtered analytic signals (or amplitude envelopes) for all
/// channels once, then computes pairwise metrics. This eliminates redundant
/// FFT/Hilbert work: for N channels, each channel is transformed once instead
/// of (N-1) times.
///
/// # Arguments
/// * `data` - Multi-channel time series
/// * `metric` - Which connectivity metric to use
/// * `band` - Frequency band (for PLV and AEC)
///
/// # Returns
/// NxN matrix where entry [i][j] is the connectivity between channels i and j.
pub fn compute_all_pairs(
    data: &MultiChannelTimeSeries,
    metric: ConnectivityMetric,
    band: FrequencyBand,
) -> Vec<Vec<f64>> {
    let nc = data.num_channels;
    let sr = data.sample_rate_hz;
    let mut matrix = vec![vec![0.0; nc]; nc];

    if nc == 0 {
        return matrix;
    }

    let (low, high) = band.range_hz();
    let n = data.data[0].len();

    match metric {
        ConnectivityMetric::Plv => {
            // Pre-compute analytic signals for all channels once.
            let bp = BandpassFilter::new(2, low, high, sr);
            let analytic_signals: Vec<Vec<Complex<f64>>> = data
                .data
                .iter()
                .map(|ch| {
                    let filtered = bp.apply(&ch[..n.min(ch.len())]);
                    hilbert_transform(&filtered)
                })
                .collect();

            for i in 0..nc {
                matrix[i][i] = 1.0;
                for j in (i + 1)..nc {
                    let len = analytic_signals[i].len().min(analytic_signals[j].len());
                    if len < 4 {
                        continue;
                    }
                    let mut sum = Complex::new(0.0, 0.0);
                    for k in 0..len {
                        let phase_a = analytic_signals[i][k].im.atan2(analytic_signals[i][k].re);
                        let phase_b = analytic_signals[j][k].im.atan2(analytic_signals[j][k].re);
                        let diff = phase_a - phase_b;
                        sum += Complex::new(diff.cos(), diff.sin());
                    }
                    let val = (sum / len as f64).norm();
                    matrix[i][j] = val;
                    matrix[j][i] = val;
                }
            }
        }
        ConnectivityMetric::Aec => {
            // Pre-compute amplitude envelopes for all channels once.
            let bp = BandpassFilter::new(2, low, high, sr);
            let envelopes: Vec<Vec<f64>> = data
                .data
                .iter()
                .map(|ch| {
                    let filtered = bp.apply(&ch[..n.min(ch.len())]);
                    crate::hilbert::instantaneous_amplitude(&filtered)
                })
                .collect();

            for i in 0..nc {
                matrix[i][i] = 1.0;
                for j in (i + 1)..nc {
                    let val = pearson_correlation(&envelopes[i], &envelopes[j]);
                    matrix[i][j] = val;
                    matrix[j][i] = val;
                }
            }
        }
    }

    matrix
}

/// Pearson correlation coefficient between two vectors.
fn pearson_correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }

    let mean_a = a[..n].iter().sum::<f64>() / n as f64;
    let mean_b = b[..n].iter().sum::<f64>() / n as f64;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-30 {
        0.0
    } else {
        cov / denom
    }
}

/// Generate a Hann window (local copy for this module).
fn hann_window(length: usize) -> Vec<f64> {
    (0..length)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (length - 1).max(1) as f64).cos()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use std::f64::consts::PI;

    #[test]
    fn plv_of_identical_signals_is_one() {
        let sr = 1000.0;
        let n = 2000;
        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (2.0 * PI * 10.0 * t).sin()
            })
            .collect();

        let plv = phase_locking_value(&signal, &signal, sr, FrequencyBand::Alpha);

        assert!(
            plv > 0.9,
            "PLV of identical signals should be ~1.0, got {plv}"
        );
    }

    #[test]
    fn plv_of_unrelated_signals_is_low() {
        let sr = 1000.0;
        let n = 4000;
        // Two signals at different frequencies
        let signal_a: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (2.0 * PI * 10.0 * t).sin()
            })
            .collect();
        let signal_b: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (2.0 * PI * 11.3 * t).sin() + 0.5 * (2.0 * PI * 9.7 * t).cos()
            })
            .collect();

        let plv = phase_locking_value(&signal_a, &signal_b, sr, FrequencyBand::Alpha);

        assert!(
            plv < 0.7,
            "PLV of unrelated signals should be low, got {plv}"
        );
    }

    #[test]
    fn coherence_of_identical_signals_is_one() {
        let sr = 1000.0;
        let n = 2000;
        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (2.0 * PI * 20.0 * t).sin()
            })
            .collect();

        let coh = coherence(&signal, &signal, sr);

        // At the signal frequency (~20 Hz), coherence should be ~1.0
        let peak_coh = coh
            .iter()
            .filter(|(f, _)| *f > 15.0 && *f < 25.0)
            .map(|(_, c)| *c)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        assert!(
            peak_coh > 0.95,
            "Coherence of identical signals should be ~1.0 at signal freq, got {peak_coh}"
        );
    }

    #[test]
    fn compute_all_pairs_returns_symmetric_matrix() {
        let data = MultiChannelTimeSeries {
            data: vec![
                (0..1000)
                    .map(|i| (2.0 * PI * 10.0 * i as f64 / 1000.0).sin())
                    .collect(),
                (0..1000)
                    .map(|i| (2.0 * PI * 10.0 * i as f64 / 1000.0).cos())
                    .collect(),
                (0..1000)
                    .map(|i| (2.0 * PI * 10.0 * i as f64 / 1000.0 + 0.3).sin())
                    .collect(),
            ],
            sample_rate_hz: 1000.0,
            num_channels: 3,
            num_samples: 1000,
            timestamp_start: 0.0,
        };

        let matrix = compute_all_pairs(&data, ConnectivityMetric::Plv, FrequencyBand::Alpha);

        assert_eq!(matrix.len(), 3);
        assert_eq!(matrix[0].len(), 3);

        // Diagonal should be 1.0
        for i in 0..3 {
            assert_abs_diff_eq!(matrix[i][i], 1.0, epsilon = 1e-10);
        }

        // Should be symmetric
        for i in 0..3 {
            for j in 0..3 {
                assert_abs_diff_eq!(matrix[i][j], matrix[j][i], epsilon = 1e-10);
            }
        }
    }
}
