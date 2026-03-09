//! Artifact detection and rejection for neural recordings.
//!
//! Detects common physiological and environmental artifacts:
//! - Eye blinks: large slow deflections (primarily frontal channels)
//! - Muscle artifacts: high-frequency broadband power bursts
//! - Cardiac artifacts: QRS complex detection
//!
//! Provides functions to mark and remove/interpolate artifact periods.

use ruv_neural_core::signal::MultiChannelTimeSeries;

use crate::filter::{BandpassFilter, HighpassFilter, LowpassFilter};

/// Detect eye blink artifacts in a single channel.
///
/// Eye blinks produce large, slow voltage deflections (1-5 Hz)
/// with amplitudes 5-10x the background signal. Detection uses:
/// 1. Lowpass filter to isolate slow components
/// 2. Amplitude thresholding at `mean + 3*std`
/// 3. Merging of nearby detections
///
/// # Arguments
/// * `signal` - Single-channel time series
/// * `sample_rate` - Sampling rate in Hz
///
/// # Returns
/// Vector of (start_sample, end_sample) ranges for detected blinks.
pub fn detect_eye_blinks(signal: &[f64], sample_rate: f64) -> Vec<(usize, usize)> {
    if signal.len() < (sample_rate * 0.2) as usize {
        return Vec::new();
    }

    // Lowpass filter at 5 Hz to isolate blink waveform
    let lp = LowpassFilter::new(2, 5.0, sample_rate);
    let filtered = lp.apply(signal);

    // Compute absolute values
    let abs_signal: Vec<f64> = filtered.iter().map(|x| x.abs()).collect();

    // Compute mean and std of the absolute filtered signal
    let mean = abs_signal.iter().sum::<f64>() / abs_signal.len() as f64;
    let variance = abs_signal
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / abs_signal.len() as f64;
    let std_dev = variance.sqrt();

    // Threshold at mean + 3*std
    let threshold = mean + 3.0 * std_dev;

    // Find contiguous regions above threshold
    let mut ranges = Vec::new();
    let mut in_artifact = false;
    let mut start = 0;

    for (i, &val) in abs_signal.iter().enumerate() {
        if val > threshold && !in_artifact {
            in_artifact = true;
            start = i;
        } else if val <= threshold && in_artifact {
            in_artifact = false;
            ranges.push((start, i));
        }
    }
    if in_artifact {
        ranges.push((start, abs_signal.len()));
    }

    // Extend ranges by 50ms on each side (blink onset/offset)
    let pad = (sample_rate * 0.05) as usize;
    let merged = merge_ranges_with_padding(&ranges, pad, signal.len());

    merged
}

/// Detect muscle artifact in a single channel.
///
/// Muscle artifacts produce broadband high-frequency power (>30 Hz).
/// Detection uses:
/// 1. Highpass filter at 30 Hz
/// 2. Compute sliding window RMS
/// 3. Threshold at mean + 3*std of RMS
///
/// # Returns
/// Vector of (start_sample, end_sample) ranges for detected artifacts.
pub fn detect_muscle_artifact(signal: &[f64], sample_rate: f64) -> Vec<(usize, usize)> {
    if signal.len() < (sample_rate * 0.1) as usize {
        return Vec::new();
    }

    // Highpass filter at 30 Hz to isolate muscle activity
    let hp = HighpassFilter::new(2, 30.0, sample_rate);
    let filtered = hp.apply(signal);

    // Sliding window RMS (50ms window)
    let window_len = (sample_rate * 0.05) as usize;
    let window_len = window_len.max(1);
    let n = filtered.len();
    let mut rms_signal = vec![0.0; n];

    // Compute running sum of squares
    let mut sum_sq = 0.0;
    for i in 0..n {
        sum_sq += filtered[i] * filtered[i];
        if i >= window_len {
            sum_sq -= filtered[i - window_len] * filtered[i - window_len];
        }
        let count = (i + 1).min(window_len);
        rms_signal[i] = (sum_sq / count as f64).sqrt();
    }

    // Threshold at mean + 3*std of RMS
    let mean = rms_signal.iter().sum::<f64>() / n as f64;
    let variance = rms_signal
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / n as f64;
    let std_dev = variance.sqrt();
    let threshold = mean + 3.0 * std_dev;

    let mut ranges = Vec::new();
    let mut in_artifact = false;
    let mut start = 0;

    for (i, &val) in rms_signal.iter().enumerate() {
        if val > threshold && !in_artifact {
            in_artifact = true;
            start = i;
        } else if val <= threshold && in_artifact {
            in_artifact = false;
            ranges.push((start, i));
        }
    }
    if in_artifact {
        ranges.push((start, n));
    }

    let pad = (sample_rate * 0.025) as usize;
    merge_ranges_with_padding(&ranges, pad, signal.len())
}

/// Detect cardiac (QRS complex) artifact peaks in a single channel.
///
/// Uses a simplified Pan-Tompkins-style approach:
/// 1. Bandpass filter 5-15 Hz
/// 2. Differentiate and square
/// 3. Moving window integration
/// 4. Threshold-based peak detection with refractory period
///
/// # Returns
/// Vector of sample indices where QRS peaks are detected.
pub fn detect_cardiac(signal: &[f64], sample_rate: f64) -> Vec<usize> {
    if signal.len() < (sample_rate * 0.5) as usize {
        return Vec::new();
    }

    // Bandpass 5-15 Hz to isolate QRS complex
    let bp = BandpassFilter::new(2, 5.0, 15.0, sample_rate);
    let filtered = bp.apply(signal);

    // Differentiate
    let n = filtered.len();
    let mut diff = vec![0.0; n];
    for i in 1..n {
        diff[i] = filtered[i] - filtered[i - 1];
    }

    // Square
    let squared: Vec<f64> = diff.iter().map(|x| x * x).collect();

    // Moving window integration (150ms window)
    let win_len = (sample_rate * 0.15) as usize;
    let win_len = win_len.max(1);
    let mut integrated = vec![0.0; n];
    let mut sum = 0.0;

    for i in 0..n {
        sum += squared[i];
        if i >= win_len {
            sum -= squared[i - win_len];
        }
        integrated[i] = sum / win_len.min(i + 1) as f64;
    }

    // Threshold: mean + 0.5*std (tuned for cardiac artifacts which are periodic)
    let mean = integrated.iter().sum::<f64>() / n as f64;
    let variance = integrated
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / n as f64;
    let std_dev = variance.sqrt();
    let threshold = mean + 0.5 * std_dev;

    // Find peaks above threshold with refractory period (200ms)
    let refractory = (sample_rate * 0.2) as usize;
    let mut peaks = Vec::new();
    let mut last_peak: Option<usize> = None;

    for i in 1..(n - 1) {
        if integrated[i] > threshold
            && integrated[i] > integrated[i - 1]
            && integrated[i] >= integrated[i + 1]
        {
            if let Some(lp) = last_peak {
                if i - lp < refractory {
                    continue;
                }
            }
            peaks.push(i);
            last_peak = Some(i);
        }
    }

    peaks
}

/// Remove artifacts from multi-channel data by linear interpolation.
///
/// For each artifact range, replaces the data with a linear interpolation
/// between the sample before the range and the sample after the range.
///
/// # Arguments
/// * `data` - Multi-channel time series
/// * `artifact_ranges` - Sorted, non-overlapping (start, end) sample ranges
///
/// # Returns
/// A new `MultiChannelTimeSeries` with artifacts interpolated out.
pub fn reject_artifacts(
    data: &MultiChannelTimeSeries,
    artifact_ranges: &[(usize, usize)],
) -> MultiChannelTimeSeries {
    let mut clean_data = data.data.clone();

    for channel in &mut clean_data {
        let n = channel.len();
        for &(start, end) in artifact_ranges {
            let start = start.min(n);
            let end = end.min(n);
            if start >= end {
                continue;
            }

            // Get boundary values for interpolation
            let val_before = if start > 0 { channel[start - 1] } else { 0.0 };
            let val_after = if end < n { channel[end] } else { 0.0 };
            let span = (end - start) as f64;

            // Linear interpolation across the artifact
            // frac goes from 1/(span+1) to span/(span+1), excluding boundaries
            let intervals = span + 1.0;
            for i in start..end {
                let frac = (i - start + 1) as f64 / intervals;
                channel[i] = val_before * (1.0 - frac) + val_after * frac;
            }
        }
    }

    MultiChannelTimeSeries {
        data: clean_data,
        sample_rate_hz: data.sample_rate_hz,
        num_channels: data.num_channels,
        num_samples: data.num_samples,
        timestamp_start: data.timestamp_start,
    }
}

/// Merge artifact ranges and add padding on each side.
fn merge_ranges_with_padding(
    ranges: &[(usize, usize)],
    pad: usize,
    max_len: usize,
) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return Vec::new();
    }

    // Pad each range
    let padded: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(s, e)| (s.saturating_sub(pad), (e + pad).min(max_len)))
        .collect();

    // Merge overlapping ranges
    let mut merged = Vec::new();
    let (mut cur_start, mut cur_end) = padded[0];

    for &(s, e) in &padded[1..] {
        if s <= cur_end {
            cur_end = cur_end.max(e);
        } else {
            merged.push((cur_start, cur_end));
            cur_start = s;
            cur_end = e;
        }
    }
    merged.push((cur_start, cur_end));

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::signal::MultiChannelTimeSeries;

    #[test]
    fn detect_eye_blinks_finds_large_deflections() {
        let sr = 1000.0;
        let n = 5000;
        // Create signal with a large slow deflection (simulated blink)
        let mut signal = vec![0.0; n];
        // Normal background: small random-like variation
        for i in 0..n {
            signal[i] = 0.01 * ((i as f64 * 0.1).sin());
        }
        // Insert a blink: large Gaussian-like bump at sample 2500
        for i in 2400..2600 {
            let t = (i as f64 - 2500.0) / 30.0;
            signal[i] += 5.0 * (-t * t / 2.0).exp();
        }

        let blinks = detect_eye_blinks(&signal, sr);
        // Should detect at least one blink near sample 2500
        assert!(
            !blinks.is_empty(),
            "Should detect the simulated eye blink"
        );

        // At least one range should overlap with 2400..2600
        let found = blinks.iter().any(|&(s, e)| s < 2600 && e > 2400);
        assert!(found, "Blink range should overlap with injected artifact");
    }

    #[test]
    fn reject_artifacts_interpolates_correctly() {
        let data = MultiChannelTimeSeries {
            data: vec![vec![1.0, 2.0, 100.0, 100.0, 5.0, 6.0]],
            sample_rate_hz: 1000.0,
            num_channels: 1,
            num_samples: 6,
            timestamp_start: 0.0,
        };

        let cleaned = reject_artifacts(&data, &[(2, 4)]);

        // Samples 2 and 3 should be linearly interpolated between 2.0 and 5.0
        assert!((cleaned.data[0][2] - 3.0).abs() < 0.01);
        assert!((cleaned.data[0][3] - 4.0).abs() < 0.01);

        // Non-artifact samples should be unchanged
        assert!((cleaned.data[0][0] - 1.0).abs() < 1e-10);
        assert!((cleaned.data[0][4] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn detect_cardiac_finds_periodic_peaks() {
        let sr = 1000.0;
        let duration = 3.0;
        let n = (sr * duration) as usize;
        let mut signal = vec![0.0; n];

        // Simulate cardiac artifact: periodic QRS-like spikes at ~1 Hz
        let heart_rate_hz = 1.0;
        let interval = (sr / heart_rate_hz) as usize;

        for beat in 0..3 {
            let center = beat * interval + interval / 2;
            if center >= n {
                break;
            }
            // QRS complex: sharp spike ~10ms wide
            let half_width = (sr * 0.005) as usize;
            for i in center.saturating_sub(half_width)..(center + half_width).min(n) {
                let t = (i as f64 - center as f64) / (half_width as f64);
                signal[i] = 10.0 * (-t * t * 5.0).exp();
            }
        }

        let peaks = detect_cardiac(&signal, sr);

        // Should find roughly 3 peaks
        assert!(
            peaks.len() >= 1,
            "Should detect at least one cardiac peak, found {}",
            peaks.len()
        );
    }
}
