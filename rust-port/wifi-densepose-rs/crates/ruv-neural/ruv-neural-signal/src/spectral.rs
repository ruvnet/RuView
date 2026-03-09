//! Spectral analysis for neural time series data.
//!
//! Provides Welch's method for power spectral density estimation,
//! short-time Fourier transform (STFT), band power extraction,
//! spectral entropy, and peak frequency detection.
//!
//! All transforms use a Hann window for spectral leakage reduction.

use num_complex::Complex;
use ruv_neural_core::signal::{FrequencyBand, TimeFrequencyMap};
use rustfft::FftPlanner;
use std::cell::RefCell;
use std::f64::consts::PI;

thread_local! {
    static FFT_PLANNER: RefCell<FftPlanner<f64>> = RefCell::new(FftPlanner::new());
}

/// Generate a Hann window of the given length.
fn hann_window(length: usize) -> Vec<f64> {
    (0..length)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (length - 1).max(1) as f64).cos()))
        .collect()
}

/// Compute the power spectral density using Welch's method.
///
/// Divides the signal into overlapping segments (50% overlap), applies a Hann
/// window, computes the periodogram for each segment, and averages.
///
/// # Arguments
/// * `signal` - Input time series
/// * `sample_rate` - Sampling rate in Hz
/// * `window_size` - Length of each segment in samples
///
/// # Returns
/// (frequencies, power_spectral_density) in Hz and signal_units^2/Hz.
pub fn compute_psd(signal: &[f64], sample_rate: f64, window_size: usize) -> (Vec<f64>, Vec<f64>) {
    let n = signal.len();
    if n == 0 || window_size == 0 {
        return (Vec::new(), Vec::new());
    }

    let win_size = window_size.min(n);
    let overlap = win_size / 2;
    let hop = win_size - overlap;
    let window = hann_window(win_size);

    let window_power: f64 = window.iter().map(|w| w * w).sum();

    let fft = FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(win_size));

    let num_freqs = win_size / 2 + 1;
    let mut psd_accum = vec![0.0; num_freqs];
    let mut num_segments = 0;

    let mut start = 0;
    while start + win_size <= n {
        let mut windowed: Vec<Complex<f64>> = (0..win_size)
            .map(|i| Complex::new(signal[start + i] * window[i], 0.0))
            .collect();

        fft.process(&mut windowed);

        for k in 0..num_freqs {
            let power = windowed[k].norm_sqr();
            let scale = if k == 0 || k == win_size / 2 { 1.0 } else { 2.0 };
            psd_accum[k] += power * scale;
        }
        num_segments += 1;
        start += hop;
    }

    if num_segments == 0 {
        return (Vec::new(), Vec::new());
    }

    let norm = num_segments as f64 * sample_rate * window_power;
    let psd: Vec<f64> = psd_accum.iter().map(|p| p / norm).collect();

    let freq_resolution = sample_rate / win_size as f64;
    let freqs: Vec<f64> = (0..num_freqs).map(|k| k as f64 * freq_resolution).collect();

    (freqs, psd)
}

/// Compute the short-time Fourier transform (STFT).
///
/// # Arguments
/// * `signal` - Input time series
/// * `sample_rate` - Sampling rate in Hz
/// * `window_size` - FFT window length in samples
/// * `hop_size` - Hop size between windows in samples
///
/// # Returns
/// A [`TimeFrequencyMap`] containing the magnitude spectrogram.
pub fn compute_stft(
    signal: &[f64],
    sample_rate: f64,
    window_size: usize,
    hop_size: usize,
) -> TimeFrequencyMap {
    let n = signal.len();
    if n == 0 || window_size == 0 || hop_size == 0 {
        return TimeFrequencyMap {
            data: Vec::new(),
            time_points: Vec::new(),
            frequency_bins: Vec::new(),
        };
    }

    let win_size = window_size.min(n);
    let window = hann_window(win_size);

    let fft = FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(win_size));

    let num_freqs = win_size / 2 + 1;
    let freq_resolution = sample_rate / win_size as f64;
    let frequency_bins: Vec<f64> = (0..num_freqs).map(|k| k as f64 * freq_resolution).collect();

    let mut data = Vec::new();
    let mut time_points = Vec::new();

    let mut start = 0;
    while start + win_size <= n {
        let mut windowed: Vec<Complex<f64>> = (0..win_size)
            .map(|i| Complex::new(signal[start + i] * window[i], 0.0))
            .collect();

        fft.process(&mut windowed);

        let magnitudes: Vec<f64> = windowed[..num_freqs]
            .iter()
            .map(|c| c.norm() / win_size as f64)
            .collect();

        data.push(magnitudes);
        time_points.push((start as f64 + win_size as f64 / 2.0) / sample_rate);
        start += hop_size;
    }

    TimeFrequencyMap {
        data,
        time_points,
        frequency_bins,
    }
}

/// Extract total power within a specific frequency band from a PSD.
///
/// Integrates (trapezoidal) the PSD values for frequencies within the band range.
pub fn band_power(psd: &[f64], freqs: &[f64], band: FrequencyBand) -> f64 {
    let (low, high) = band.range_hz();
    let df = if freqs.len() > 1 {
        freqs[1] - freqs[0]
    } else {
        1.0
    };

    psd.iter()
        .zip(freqs.iter())
        .filter(|(_, f)| **f >= low && **f <= high)
        .map(|(p, _)| p * df)
        .sum()
}

/// Compute the spectral entropy of a power spectral density.
///
/// Normalizes the PSD to a probability distribution and computes
/// Shannon entropy: H = -sum(p * log2(p)).
///
/// Higher entropy = more uniform (noise-like) spectrum.
/// Lower entropy = more peaked (tonal) spectrum.
pub fn spectral_entropy(psd: &[f64]) -> f64 {
    let total: f64 = psd.iter().sum();
    if total <= 0.0 || psd.is_empty() {
        return 0.0;
    }

    let mut entropy = 0.0;
    for &p in psd {
        let prob = p / total;
        if prob > 1e-30 {
            entropy -= prob * prob.log2();
        }
    }

    entropy
}

/// Find the frequency of the maximum power in the PSD.
pub fn peak_frequency(psd: &[f64], freqs: &[f64]) -> f64 {
    if psd.is_empty() || freqs.is_empty() {
        return 0.0;
    }

    let (max_idx, _) = psd
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    freqs[max_idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use std::f64::consts::PI;

    #[test]
    fn psd_of_sinusoid_peaks_at_correct_frequency() {
        let sr = 1000.0;
        let freq = 40.0;
        let n = 4000;
        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (2.0 * PI * freq * t).sin()
            })
            .collect();

        let (freqs, psd) = compute_psd(&signal, sr, 512);

        let peak = peak_frequency(&psd, &freqs);
        let freq_res = sr / 512.0;
        assert!(
            (peak - freq).abs() < freq_res * 1.5,
            "Peak at {peak} Hz, expected {freq} Hz (resolution {freq_res} Hz)"
        );
    }

    #[test]
    fn spectral_entropy_white_noise_gt_pure_tone() {
        let sr = 1000.0;
        let n = 4000;

        let tone: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (2.0 * PI * 50.0 * t).sin()
            })
            .collect();

        let noise: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                let mut val = 0.0;
                for f in (1..200).step_by(3) {
                    val += (2.0 * PI * f as f64 * t + f as f64 * 0.7).sin();
                }
                val
            })
            .collect();

        let (_, psd_tone) = compute_psd(&tone, sr, 512);
        let (_, psd_noise) = compute_psd(&noise, sr, 512);

        let ent_tone = spectral_entropy(&psd_tone);
        let ent_noise = spectral_entropy(&psd_noise);

        assert!(
            ent_noise > ent_tone,
            "Noise entropy ({ent_noise}) should be > tone entropy ({ent_tone})"
        );
    }

    #[test]
    fn stft_produces_correct_dimensions() {
        let sr = 1000.0;
        let n = 2000;
        let signal: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).sin()).collect();

        let stft = compute_stft(&signal, sr, 256, 128);

        assert_eq!(stft.frequency_bins.len(), 129);

        let expected_frames = (n - 256) / 128 + 1;
        assert_eq!(stft.time_points.len(), expected_frames);
        assert_eq!(stft.data.len(), expected_frames);
    }

    #[test]
    fn band_power_extracts_correct_band() {
        let freqs: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let mut psd = vec![0.0; 100];
        psd[10] = 100.0;

        let alpha_power = band_power(&psd, &freqs, FrequencyBand::Alpha);
        let beta_power = band_power(&psd, &freqs, FrequencyBand::Beta);

        assert!(alpha_power > 0.0, "Alpha band should have power");
        assert_abs_diff_eq!(beta_power, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn empty_signal_psd() {
        let (freqs, psd) = compute_psd(&[], 1000.0, 256);
        assert!(freqs.is_empty());
        assert!(psd.is_empty());
    }
}
