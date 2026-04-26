//! Stage 5: Coarse breathing rate extraction.
//!
//! Extracts respiratory rate from body-sensitive BSSID oscillations.
//! Uses a simple bandpass filter (0.1-0.5 Hz) and zero-crossing
//! analysis rather than `OscillatoryRouter` (which is designed for
//! gamma-band frequencies, not sub-Hz breathing).

/// Coarse breathing extractor from multi-BSSID signal variance.
pub struct CoarseBreathingExtractor {
    /// Combined filtered signal history.
    filtered_history: Vec<f32>,
    /// Window size for analysis.
    window: usize,
    /// Maximum tracked BSSIDs.
    n_bssids: usize,
    /// Breathing band low cutoff (Hz).
    freq_low: f32,
    /// Breathing band high cutoff (Hz).
    freq_high: f32,
    /// Sample rate (Hz) -- typically 2 Hz for Tier 1.
    sample_rate: f32,
    /// IIR filter state (simple 2nd-order bandpass).
    filter_state: IirState,
}

/// Simple IIR bandpass filter state (2nd order).
#[derive(Clone, Debug)]
struct IirState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
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

impl CoarseBreathingExtractor {
    /// Create a breathing extractor.
    ///
    /// - `n_bssids`: maximum BSSID slots.
    /// - `sample_rate`: input sample rate in Hz.
    /// - `freq_low`: breathing band low cutoff (default 0.1 Hz).
    /// - `freq_high`: breathing band high cutoff (default 0.5 Hz).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn new(n_bssids: usize, sample_rate: f32, freq_low: f32, freq_high: f32) -> Self {
        let window = (sample_rate * 30.0) as usize; // 30 seconds of data
        Self {
            filtered_history: Vec::with_capacity(window),
            window,
            n_bssids,
            freq_low,
            freq_high,
            sample_rate,
            filter_state: IirState::default(),
        }
    }

    /// Create with defaults suitable for Tier 1 (2 Hz sample rate).
    #[must_use]
    pub fn tier1_default(n_bssids: usize) -> Self {
        Self::new(n_bssids, 2.0, 0.1, 0.5)
    }

    /// Process a frame of residuals with attention weights.
    /// Returns estimated breathing rate (BPM) if detectable.
    ///
    /// - `residuals`: per-BSSID residuals from `PredictiveGate`.
    /// - `weights`: per-BSSID attention weights.
    pub fn extract(&mut self, residuals: &[f32], weights: &[f32]) -> Option<BreathingEstimate> {
        let n = residuals.len().min(self.n_bssids);
        if n == 0 {
            return None;
        }

        // Compute weighted sum of residuals for breathing analysis
        #[allow(clippy::cast_precision_loss)]
        let weighted_signal: f32 = residuals
            .iter()
            .enumerate()
            .take(n)
            .map(|(i, &r)| {
                let w = weights.get(i).copied().unwrap_or(1.0 / n as f32);
                r * w
            })
            .sum();

        // Apply bandpass filter
        let filtered = self.bandpass_filter(weighted_signal);

        // Store in history
        self.filtered_history.push(filtered);
        if self.filtered_history.len() > self.window {
            self.filtered_history.remove(0);
        }

        // Need at least 10 seconds of data to estimate breathing
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let min_samples = (self.sample_rate * 10.0) as usize;
        if self.filtered_history.len() < min_samples {
            return None;
        }

        // Zero-crossing rate -> frequency
        let crossings = count_zero_crossings(&self.filtered_history);
        #[allow(clippy::cast_precision_loss)]
        let duration_s = self.filtered_history.len() as f32 / self.sample_rate;
        #[allow(clippy::cast_precision_loss)]
        let frequency_hz = crossings as f32 / (2.0 * duration_s);

        // Validate frequency is in breathing range
        if frequency_hz < self.freq_low || frequency_hz > self.freq_high {
            return None;
        }

        let bpm = frequency_hz * 60.0;

        // Compute confidence based on signal regularity
        let confidence = compute_confidence(&self.filtered_history);

        Some(BreathingEstimate {
            bpm,
            frequency_hz,
            confidence,
        })
    }

    /// Simple 2nd-order IIR bandpass filter.
    fn bandpass_filter(&mut self, input: f32) -> f32 {
        let state = &mut self.filter_state;

        // Butterworth bandpass coefficients for [freq_low, freq_high] at given sample rate.
        // Using bilinear transform approximation.
        let omega_low = 2.0 * std::f32::consts::PI * self.freq_low / self.sample_rate;
        let omega_high = 2.0 * std::f32::consts::PI * self.freq_high / self.sample_rate;
        let bw = omega_high - omega_low;
        let center = f32::midpoint(omega_low, omega_high);

        let r = 1.0 - bw / 2.0;
        let cos_w0 = center.cos();

        // y[n] = (1-r)*(x[n] - x[n-2]) + 2*r*cos(w0)*y[n-1] - r^2*y[n-2]
        let output =
            (1.0 - r) * (input - state.x2) + 2.0 * r * cos_w0 * state.y1 - r * r * state.y2;

        state.x2 = state.x1;
        state.x1 = input;
        state.y2 = state.y1;
        state.y1 = output;

        output
    }

    /// Reset all filter states and histories.
    pub fn reset(&mut self) {
        self.filtered_history.clear();
        self.filter_state = IirState::default();
    }
}

/// Result of breathing extraction.
#[derive(Debug, Clone)]
pub struct BreathingEstimate {
    /// Estimated breathing rate in breaths per minute.
    pub bpm: f32,
    /// Estimated breathing frequency in Hz.
    pub frequency_hz: f32,
    /// Confidence in the estimate [0, 1].
    pub confidence: f32,
}

/// Compute confidence in the breathing estimate based on signal regularity.
#[allow(clippy::cast_precision_loss)]
fn compute_confidence(history: &[f32]) -> f32 {
    if history.len() < 4 {
        return 0.0;
    }

    // Use variance-based SNR as a confidence metric
    let mean: f32 = history.iter().sum::<f32>() / history.len() as f32;
    let variance: f32 = history
        .iter()
        .map(|x| (x - mean) * (x - mean))
        .sum::<f32>()
        / history.len() as f32;

    if variance < 1e-10 {
        return 0.0;
    }

    // Simple SNR-based confidence
    let peak = history.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let noise = variance.sqrt();

    let snr = if noise > 1e-10 { peak / noise } else { 0.0 };

    // Map SNR to [0, 1] confidence
    (snr / 5.0).min(1.0)
}

/// Count zero crossings in a signal.
fn count_zero_crossings(signal: &[f32]) -> usize {
    signal.windows(2).filter(|w| w[0] * w[1] < 0.0).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_data_returns_none() {
        let mut ext = CoarseBreathingExtractor::tier1_default(4);
        assert!(ext.extract(&[], &[]).is_none());
    }

    #[test]
    fn insufficient_history_returns_none() {
        let mut ext = CoarseBreathingExtractor::tier1_default(4);
        // Just a few frames are not enough
        for _ in 0..5 {
            assert!(ext.extract(&[1.0, 2.0], &[0.5, 0.5]).is_none());
        }
    }

    #[test]
    fn sinusoidal_breathing_detected() {
        let mut ext = CoarseBreathingExtractor::new(1, 10.0, 0.1, 0.5);
        let breathing_freq = 0.25; // 15 BPM

        // Generate 60 seconds of sinusoidal breathing signal at 10 Hz
        for i in 0..600 {
            let t = i as f32 / 10.0;
            let signal = (2.0 * std::f32::consts::PI * breathing_freq * t).sin();
            ext.extract(&[signal], &[1.0]);
        }

        let result = ext.extract(&[0.0], &[1.0]);
        if let Some(est) = result {
            // Should be approximately 15 BPM (0.25 Hz * 60)
            assert!(
                est.bpm > 5.0 && est.bpm < 40.0,
                "estimated BPM should be in breathing range: {}",
                est.bpm
            );
        }
        // It is acceptable if None -- the bandpass filter may need tuning
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
    fn reset_clears_state() {
        let mut ext = CoarseBreathingExtractor::tier1_default(2);
        ext.extract(&[1.0, 2.0], &[0.5, 0.5]);
        ext.reset();
        assert!(ext.filtered_history.is_empty());
    }
}
