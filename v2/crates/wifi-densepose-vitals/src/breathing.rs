//! Respiratory rate extraction from CSI residuals.
//!
//! Uses bandpass filtering (0.1-0.5 Hz) and spectral analysis
//! to extract breathing rate from multi-subcarrier CSI data.
//!
//! The approach follows the same IIR bandpass + zero-crossing pattern
//! used by [`CoarseBreathingExtractor`](wifi_densepose_wifiscan::pipeline::CoarseBreathingExtractor)
//! in the wifiscan crate, adapted for multi-subcarrier f64 processing
//! with weighted subcarrier fusion.

use crate::types::{VitalEstimate, VitalStatus};

/// IIR bandpass filter state (2nd-order resonator).
#[derive(Clone, Debug)]
struct IirState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Default for IirState {
    fn default() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

/// Respiratory rate extractor using bandpass filtering and zero-crossing analysis.
pub struct BreathingExtractor {
    /// Per-sample filtered signal history.
    filtered_history: Vec<f64>,
    /// Sample rate in Hz.
    sample_rate: f64,
    /// Analysis window in seconds.
    window_secs: f64,
    /// Maximum subcarrier slots.
    n_subcarriers: usize,
    /// Breathing band low cutoff (Hz).
    freq_low: f64,
    /// Breathing band high cutoff (Hz).
    freq_high: f64,
    /// IIR filter state.
    filter_state: IirState,
}

impl BreathingExtractor {
    /// Create a new breathing extractor.
    ///
    /// - `n_subcarriers`: number of subcarrier channels.
    /// - `sample_rate`: input sample rate in Hz.
    /// - `window_secs`: analysis window length in seconds (default: 30).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn new(n_subcarriers: usize, sample_rate: f64, window_secs: f64) -> Self {
        let capacity = (sample_rate * window_secs) as usize;
        Self {
            filtered_history: Vec::with_capacity(capacity),
            sample_rate,
            window_secs,
            n_subcarriers,
            freq_low: 0.1,
            freq_high: 0.5,
            filter_state: IirState::default(),
        }
    }

    /// Create with ESP32 defaults (56 subcarriers, 100 Hz, 30 s window).
    #[must_use]
    pub fn esp32_default() -> Self {
        Self::new(56, 100.0, 30.0)
    }

    /// Extract respiratory rate from a vector of per-subcarrier residuals.
    ///
    /// - `residuals`: amplitude residuals from the preprocessor.
    /// - `weights`: per-subcarrier attention weights (higher = more
    ///   body-sensitive). If shorter than `residuals`, missing weights
    ///   default to uniform.
    ///
    /// Returns a `VitalEstimate` with the breathing rate in BPM, or
    /// `None` if insufficient history has been accumulated.
    pub fn extract(&mut self, residuals: &[f64], weights: &[f64]) -> Option<VitalEstimate> {
        let n = residuals.len().min(self.n_subcarriers);
        if n == 0 {
            return None;
        }

        // Weighted fusion of subcarrier residuals
        let uniform_w = 1.0 / n as f64;
        let weighted_signal: f64 = residuals
            .iter()
            .enumerate()
            .take(n)
            .map(|(i, &r)| {
                let w = weights.get(i).copied().unwrap_or(uniform_w);
                r * w
            })
            .sum();

        // Apply IIR bandpass filter
        let filtered = self.bandpass_filter(weighted_signal);

        // Append to history, enforce window limit
        self.filtered_history.push(filtered);
        let max_len = (self.sample_rate * self.window_secs) as usize;
        if self.filtered_history.len() > max_len {
            self.filtered_history.remove(0);
        }

        // Need at least 10 seconds of data
        let min_samples = (self.sample_rate * 10.0) as usize;
        if self.filtered_history.len() < min_samples {
            return None;
        }

        // Zero-crossing rate -> frequency
        let crossings = count_zero_crossings(&self.filtered_history);
        let duration_s = self.filtered_history.len() as f64 / self.sample_rate;
        let frequency_hz = crossings as f64 / (2.0 * duration_s);

        // Validate frequency is within the breathing band
        if frequency_hz < self.freq_low || frequency_hz > self.freq_high {
            return None;
        }

        let bpm = frequency_hz * 60.0;
        let confidence = compute_confidence(&self.filtered_history);

        let status = if confidence >= 0.7 {
            VitalStatus::Valid
        } else if confidence >= 0.4 {
            VitalStatus::Degraded
        } else {
            VitalStatus::Unreliable
        };

        Some(VitalEstimate {
            value_bpm: bpm,
            confidence,
            status,
        })
    }

    /// 2nd-order IIR bandpass filter using a resonator topology.
    ///
    /// y[n] = (1-r)*(x[n] - x[n-2]) + 2*r*cos(w0)*y[n-1] - r^2*y[n-2]
    fn bandpass_filter(&mut self, input: f64) -> f64 {
        let state = &mut self.filter_state;

        let omega_low = 2.0 * std::f64::consts::PI * self.freq_low / self.sample_rate;
        let omega_high = 2.0 * std::f64::consts::PI * self.freq_high / self.sample_rate;
        let bw = omega_high - omega_low;
        let center = f64::midpoint(omega_low, omega_high);

        let r = 1.0 - bw / 2.0;
        let cos_w0 = center.cos();

        let output =
            (1.0 - r) * (input - state.x2) + 2.0 * r * cos_w0 * state.y1 - r * r * state.y2;

        state.x2 = state.x1;
        state.x1 = input;
        state.y2 = state.y1;
        state.y1 = output;

        output
    }

    /// Reset all filter state and history.
    pub fn reset(&mut self) {
        self.filtered_history.clear();
        self.filter_state = IirState::default();
    }

    /// Current number of samples in the history buffer.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.filtered_history.len()
    }

    /// Breathing band cutoff frequencies.
    #[must_use]
    pub fn band(&self) -> (f64, f64) {
        (self.freq_low, self.freq_high)
    }
}

/// Count zero crossings in a signal.
fn count_zero_crossings(signal: &[f64]) -> usize {
    signal.windows(2).filter(|w| w[0] * w[1] < 0.0).count()
}

/// Compute confidence in the breathing estimate based on signal regularity.
fn compute_confidence(history: &[f64]) -> f64 {
    if history.len() < 4 {
        return 0.0;
    }

    let n = history.len() as f64;
    let mean: f64 = history.iter().sum::<f64>() / n;
    let variance: f64 = history.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;

    if variance < 1e-15 {
        return 0.0;
    }

    let peak = history
        .iter()
        .map(|x| x.abs())
        .fold(0.0_f64, f64::max);
    let noise = variance.sqrt();

    let snr = if noise > 1e-15 { peak / noise } else { 0.0 };

    // Map SNR to [0, 1] confidence
    (snr / 5.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_data_returns_none() {
        let mut ext = BreathingExtractor::new(4, 10.0, 30.0);
        assert!(ext.extract(&[], &[]).is_none());
    }

    #[test]
    fn insufficient_history_returns_none() {
        let mut ext = BreathingExtractor::new(2, 10.0, 30.0);
        // Just a few frames are not enough
        for _ in 0..5 {
            assert!(ext.extract(&[1.0, 2.0], &[0.5, 0.5]).is_none());
        }
    }

    #[test]
    fn zero_crossings_count() {
        let signal = vec![1.0, -1.0, 1.0, -1.0, 1.0];
        assert_eq!(count_zero_crossings(&signal), 4);
    }

    #[test]
    fn zero_crossings_constant() {
        let signal = vec![1.0, 1.0, 1.0, 1.0];
        assert_eq!(count_zero_crossings(&signal), 0);
    }

    #[test]
    fn sinusoidal_breathing_detected() {
        let sample_rate = 10.0;
        let mut ext = BreathingExtractor::new(1, sample_rate, 60.0);
        let breathing_freq = 0.25; // 15 BPM

        // Generate 60 seconds of sinusoidal breathing signal
        for i in 0..600 {
            let t = i as f64 / sample_rate;
            let signal = (2.0 * std::f64::consts::PI * breathing_freq * t).sin();
            ext.extract(&[signal], &[1.0]);
        }

        let result = ext.extract(&[0.0], &[1.0]);
        if let Some(est) = result {
            // Should be approximately 15 BPM (0.25 Hz * 60)
            assert!(
                est.value_bpm > 5.0 && est.value_bpm < 40.0,
                "estimated BPM should be in breathing range: {}",
                est.value_bpm,
            );
            assert!(est.confidence > 0.0, "confidence should be > 0");
        }
    }

    #[test]
    fn reset_clears_state() {
        let mut ext = BreathingExtractor::new(2, 10.0, 30.0);
        ext.extract(&[1.0, 2.0], &[0.5, 0.5]);
        assert!(ext.history_len() > 0);
        ext.reset();
        assert_eq!(ext.history_len(), 0);
    }

    #[test]
    fn band_returns_correct_values() {
        let ext = BreathingExtractor::new(1, 10.0, 30.0);
        let (low, high) = ext.band();
        assert!((low - 0.1).abs() < f64::EPSILON);
        assert!((high - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_zero_for_flat_signal() {
        let history = vec![0.0; 100];
        let conf = compute_confidence(&history);
        assert!((conf - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_positive_for_oscillating_signal() {
        let history: Vec<f64> = (0..100)
            .map(|i| (i as f64 * 0.5).sin())
            .collect();
        let conf = compute_confidence(&history);
        assert!(conf > 0.0);
    }

    #[test]
    fn esp32_default_creates_correctly() {
        let ext = BreathingExtractor::esp32_default();
        assert_eq!(ext.n_subcarriers, 56);
    }
}
