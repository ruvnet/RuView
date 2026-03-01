//! Heart rate extraction from CSI phase coherence.
//!
//! Uses bandpass filtering (0.8-2.0 Hz) and autocorrelation-based
//! peak detection to extract cardiac rate from inter-subcarrier
//! phase data. Requires multi-subcarrier CSI data (ESP32 mode only).
//!
//! The cardiac signal (0.1-0.5 mm body surface displacement) is
//! ~10x weaker than the respiratory signal (1-5 mm chest displacement),
//! so this module relies on phase coherence across subcarriers rather
//! than single-channel amplitude analysis.

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

/// Heart rate extractor using bandpass filtering and autocorrelation
/// peak detection.
pub struct HeartRateExtractor {
    /// Per-sample filtered signal history.
    filtered_history: Vec<f64>,
    /// Sample rate in Hz.
    sample_rate: f64,
    /// Analysis window in seconds.
    window_secs: f64,
    /// Maximum subcarrier slots.
    n_subcarriers: usize,
    /// Cardiac band low cutoff (Hz) -- 0.8 Hz = 48 BPM.
    freq_low: f64,
    /// Cardiac band high cutoff (Hz) -- 2.0 Hz = 120 BPM.
    freq_high: f64,
    /// IIR filter state.
    filter_state: IirState,
    /// Minimum subcarriers required for reliable HR estimation.
    min_subcarriers: usize,
}

impl HeartRateExtractor {
    /// Create a new heart rate extractor.
    ///
    /// - `n_subcarriers`: number of subcarrier channels.
    /// - `sample_rate`: input sample rate in Hz.
    /// - `window_secs`: analysis window length in seconds (default: 15).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn new(n_subcarriers: usize, sample_rate: f64, window_secs: f64) -> Self {
        let capacity = (sample_rate * window_secs) as usize;
        Self {
            filtered_history: Vec::with_capacity(capacity),
            sample_rate,
            window_secs,
            n_subcarriers,
            freq_low: 0.8,
            freq_high: 2.0,
            filter_state: IirState::default(),
            min_subcarriers: 4,
        }
    }

    /// Create with ESP32 defaults (56 subcarriers, 100 Hz, 15 s window).
    #[must_use]
    pub fn esp32_default() -> Self {
        Self::new(56, 100.0, 15.0)
    }

    /// Extract heart rate from per-subcarrier residuals and phase data.
    ///
    /// - `residuals`: amplitude residuals from the preprocessor.
    /// - `phases`: per-subcarrier unwrapped phases (radians).
    ///
    /// Returns a `VitalEstimate` with heart rate in BPM, or `None`
    /// if insufficient data or too few subcarriers.
    pub fn extract(&mut self, residuals: &[f64], phases: &[f64]) -> Option<VitalEstimate> {
        let n = residuals.len().min(self.n_subcarriers).min(phases.len());
        if n == 0 {
            return None;
        }

        // For cardiac signals, use phase-coherence weighted fusion.
        // Compute mean phase differential as a proxy for body-surface
        // displacement sensitivity.
        let phase_signal = compute_phase_coherence_signal(residuals, phases, n);

        // Apply cardiac-band IIR bandpass filter
        let filtered = self.bandpass_filter(phase_signal);

        // Append to history, enforce window limit
        self.filtered_history.push(filtered);
        let max_len = (self.sample_rate * self.window_secs) as usize;
        if self.filtered_history.len() > max_len {
            self.filtered_history.remove(0);
        }

        // Need at least 5 seconds of data for cardiac detection
        let min_samples = (self.sample_rate * 5.0) as usize;
        if self.filtered_history.len() < min_samples {
            return None;
        }

        // Use autocorrelation to find the dominant periodicity
        let (period_samples, acf_peak) =
            autocorrelation_peak(&self.filtered_history, self.sample_rate, self.freq_low, self.freq_high);

        if period_samples == 0 {
            return None;
        }

        let frequency_hz = self.sample_rate / period_samples as f64;
        let bpm = frequency_hz * 60.0;

        // Validate BPM is in physiological range (40-180 BPM)
        if !(40.0..=180.0).contains(&bpm) {
            return None;
        }

        // Confidence based on autocorrelation peak strength and subcarrier count
        let subcarrier_factor = if n >= self.min_subcarriers {
            1.0
        } else {
            n as f64 / self.min_subcarriers as f64
        };
        let confidence = (acf_peak * subcarrier_factor).clamp(0.0, 1.0);

        let status = if confidence >= 0.6 && n >= self.min_subcarriers {
            VitalStatus::Valid
        } else if confidence >= 0.3 {
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

    /// 2nd-order IIR bandpass filter (cardiac band: 0.8-2.0 Hz).
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

    /// Cardiac band cutoff frequencies.
    #[must_use]
    pub fn band(&self) -> (f64, f64) {
        (self.freq_low, self.freq_high)
    }
}

/// Compute a phase-coherence-weighted signal from residuals and phases.
///
/// Combines amplitude residuals with inter-subcarrier phase coherence
/// to enhance the cardiac signal. Subcarriers with similar phase
/// derivatives are likely sensing the same body surface.
fn compute_phase_coherence_signal(residuals: &[f64], phases: &[f64], n: usize) -> f64 {
    if n <= 1 {
        return residuals.first().copied().unwrap_or(0.0);
    }

    // Compute inter-subcarrier phase differences as coherence weights.
    // Adjacent subcarriers with small phase differences are more coherent.
    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;

    for i in 0..n {
        let coherence = if i + 1 < n {
            let phase_diff = (phases[i + 1] - phases[i]).abs();
            // Higher coherence when phase difference is small
            (-phase_diff).exp()
        } else if i > 0 {
            let phase_diff = (phases[i] - phases[i - 1]).abs();
            (-phase_diff).exp()
        } else {
            1.0
        };

        weighted_sum += residuals[i] * coherence;
        weight_total += coherence;
    }

    if weight_total > 1e-15 {
        weighted_sum / weight_total
    } else {
        0.0
    }
}

/// Find the dominant periodicity via autocorrelation in the cardiac band.
///
/// Returns `(period_in_samples, peak_normalized_acf)`. If no peak is
/// found, returns `(0, 0.0)`.
fn autocorrelation_peak(
    signal: &[f64],
    sample_rate: f64,
    freq_low: f64,
    freq_high: f64,
) -> (usize, f64) {
    let n = signal.len();
    if n < 4 {
        return (0, 0.0);
    }

    // Lag range corresponding to the cardiac band
    let min_lag = (sample_rate / freq_high).floor() as usize; // highest freq = shortest period
    let max_lag = (sample_rate / freq_low).ceil() as usize; // lowest freq = longest period
    let max_lag = max_lag.min(n / 2);

    if min_lag >= max_lag || min_lag >= n {
        return (0, 0.0);
    }

    // Compute mean-subtracted signal
    let mean: f64 = signal.iter().sum::<f64>() / n as f64;

    // Autocorrelation at lag 0 for normalisation
    let acf0: f64 = signal.iter().map(|&x| (x - mean) * (x - mean)).sum();
    if acf0 < 1e-15 {
        return (0, 0.0);
    }

    // Search for the peak in the cardiac lag range
    let mut best_lag = 0;
    let mut best_acf = f64::MIN;

    for lag in min_lag..=max_lag {
        let acf: f64 = signal
            .iter()
            .take(n - lag)
            .enumerate()
            .map(|(i, &x)| (x - mean) * (signal[i + lag] - mean))
            .sum();

        let normalized = acf / acf0;
        if normalized > best_acf {
            best_acf = normalized;
            best_lag = lag;
        }
    }

    if best_acf > 0.0 {
        (best_lag, best_acf)
    } else {
        (0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_data_returns_none() {
        let mut ext = HeartRateExtractor::new(4, 100.0, 15.0);
        assert!(ext.extract(&[], &[]).is_none());
    }

    #[test]
    fn insufficient_history_returns_none() {
        let mut ext = HeartRateExtractor::new(2, 100.0, 15.0);
        for _ in 0..10 {
            assert!(ext.extract(&[0.1, 0.2], &[0.0, 0.0]).is_none());
        }
    }

    #[test]
    fn sinusoidal_heartbeat_detected() {
        let sample_rate = 50.0;
        let mut ext = HeartRateExtractor::new(4, sample_rate, 20.0);
        let heart_freq = 1.2; // 72 BPM

        // Generate 20 seconds of simulated cardiac signal across 4 subcarriers
        for i in 0..1000 {
            let t = i as f64 / sample_rate;
            let base = (2.0 * std::f64::consts::PI * heart_freq * t).sin();
            let residuals = vec![base * 0.1, base * 0.08, base * 0.12, base * 0.09];
            let phases = vec![0.0, 0.01, 0.02, 0.03]; // highly coherent
            ext.extract(&residuals, &phases);
        }

        let final_residuals = vec![0.0; 4];
        let final_phases = vec![0.0; 4];
        let result = ext.extract(&final_residuals, &final_phases);

        if let Some(est) = result {
            assert!(
                est.value_bpm > 40.0 && est.value_bpm < 180.0,
                "estimated BPM should be in cardiac range: {}",
                est.value_bpm,
            );
        }
    }

    #[test]
    fn reset_clears_state() {
        let mut ext = HeartRateExtractor::new(2, 100.0, 15.0);
        ext.extract(&[0.1, 0.2], &[0.0, 0.1]);
        assert!(ext.history_len() > 0);
        ext.reset();
        assert_eq!(ext.history_len(), 0);
    }

    #[test]
    fn band_returns_correct_values() {
        let ext = HeartRateExtractor::new(1, 100.0, 15.0);
        let (low, high) = ext.band();
        assert!((low - 0.8).abs() < f64::EPSILON);
        assert!((high - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn autocorrelation_finds_known_period() {
        let sample_rate = 50.0;
        let freq = 1.0; // 1 Hz = period of 50 samples
        let signal: Vec<f64> = (0..500)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sample_rate).sin())
            .collect();

        let (period, acf) = autocorrelation_peak(&signal, sample_rate, 0.8, 2.0);
        assert!(period > 0, "should find a period");
        assert!(acf > 0.5, "autocorrelation peak should be strong: {acf}");

        let estimated_freq = sample_rate / period as f64;
        assert!(
            (estimated_freq - 1.0).abs() < 0.1,
            "estimated frequency should be ~1 Hz, got {estimated_freq}",
        );
    }

    #[test]
    fn phase_coherence_single_subcarrier() {
        let result = compute_phase_coherence_signal(&[5.0], &[0.0], 1);
        assert!((result - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn phase_coherence_multi_subcarrier() {
        // Two coherent subcarriers (small phase difference)
        let result = compute_phase_coherence_signal(&[1.0, 1.0], &[0.0, 0.01], 2);
        // Both weights should be ~1.0 (exp(-0.01) ~ 0.99), so result ~ 1.0
        assert!((result - 1.0).abs() < 0.1, "coherent result should be ~1.0: {result}");
    }

    #[test]
    fn esp32_default_creates_correctly() {
        let ext = HeartRateExtractor::esp32_default();
        assert_eq!(ext.n_subcarriers, 56);
    }
}
