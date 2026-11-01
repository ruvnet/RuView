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
use std::collections::VecDeque;

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

/// Lowest physiologically plausible heart rate, in BPM. Estimates below this
/// (e.g. a lock onto a breathing harmonic, which the firmware #987 fix also
/// guards against) are rejected rather than emitted as a confident vital — a
/// false low HR is a safety problem. Value-identical to the prior literal.
const HR_PLAUSIBLE_MIN_BPM: f64 = 40.0;
/// Highest physiologically plausible heart rate, in BPM. Estimates above this
/// are rejected. Value-identical to the prior literal.
const HR_PLAUSIBLE_MAX_BPM: f64 = 180.0;

/// Heart rate extractor using bandpass filtering and autocorrelation
/// peak detection.
pub struct HeartRateExtractor {
    /// Per-sample filtered signal history (sliding window; O(1) push/pop).
    filtered_history: VecDeque<f64>,
    /// First-stage evidence caps confidence; extra filtering can color noise.
    single_stage_history: VecDeque<f64>,
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
    /// Cascaded cardiac resonators suppress respiratory leakage before ACF.
    filter_state: [IirState; 2],
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
            filtered_history: VecDeque::with_capacity(capacity),
            single_stage_history: VecDeque::with_capacity(capacity),
            sample_rate,
            window_secs,
            n_subcarriers,
            freq_low: 0.8,
            freq_high: 2.0,
            filter_state: [IirState::default(), IirState::default()],
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
        // `n` is driven by `residuals` (and the subcarrier cap) only, NOT by
        // `phases.len()`. Before this fix, a missing/short `phases` slice
        // (e.g. `phases=[]`, documented as meaning "equal weighting", mirroring
        // `BreathingExtractor`'s `weights=[]`) truncated `n` down to
        // `phases.len()`, so `phases=[]` forced `n == 0` and `extract()`
        // silently returned `None` for every frame (issue #1423). Missing
        // phase entries are now treated by `compute_phase_coherence_signal`
        // as "no coherence information available" and fused with equal
        // weight, exactly like `breathing::fuse_weighted_residuals`'s
        // uniform-weight fallback for a missing/partial `weights` slice.
        let n = residuals.len().min(self.n_subcarriers);
        if n == 0 {
            return None;
        }

        // For cardiac signals, use phase-coherence weighted fusion.
        // Compute mean phase differential as a proxy for body-surface
        // displacement sensitivity.
        let phase_signal = compute_phase_coherence_signal(residuals, phases, n);

        // Apply cardiac-band IIR bandpass filter
        let (single_stage, filtered) = self.bandpass_filter(phase_signal);

        // Defense-in-depth: a non-finite filter output (e.g. a diverged
        // resonator pole at a pathological sample rate) must never enter the
        // history buffer, or `acf0` would become NaN and the extractor would
        // stall permanently. Mirrors the NaN-bypass guard in ADR-154 §3.
        if !filtered.is_finite() {
            return None;
        }

        // Append to history, enforce window limit. `VecDeque` gives O(1)
        // push_back + pop_front for the sliding window (was a `Vec` with an
        // O(n) `remove(0)` per sample — ADR-157 §A1).
        self.filtered_history.push_back(filtered);
        self.single_stage_history.push_back(single_stage);
        let max_len = (self.sample_rate * self.window_secs) as usize;
        if self.filtered_history.len() > max_len {
            self.filtered_history.pop_front();
            self.single_stage_history.pop_front();
        }

        // Need at least 5 seconds of data for cardiac detection
        let min_samples = (self.sample_rate * 5.0) as usize;
        if self.filtered_history.len() < min_samples {
            return None;
        }

        // Use autocorrelation to find the dominant periodicity. The
        // autocorrelation/peak loop needs a contiguous slice; `make_contiguous`
        // rotates the ring buffer in place once per `extract()` so the slice is
        // free for the rest of this call.
        let history = self.filtered_history.make_contiguous();
        let (period_samples, acf_peak) =
            autocorrelation_peak(history, self.sample_rate, self.freq_low, self.freq_high);

        if period_samples == 0 {
            return None;
        }

        // A second filter stage sharpens periodic noise as well as real pulses.
        // Use it to locate the period, but never increase confidence above the
        // original stage's evidence at that SAME period. This prevents filtering
        // alone from turning previously degraded noise into a Valid estimate.
        let corroboration =
            autocorrelation_at_lag(self.single_stage_history.make_contiguous(), period_samples);
        let acf_peak = acf_peak.min(corroboration.max(0.0));

        let frequency_hz = self.sample_rate / period_samples as f64;
        let bpm = frequency_hz * 60.0;

        // Validate BPM is in the physiological plausibility band. An estimate
        // outside [HR_PLAUSIBLE_MIN_BPM, HR_PLAUSIBLE_MAX_BPM] is rejected
        // rather than emitted, so an out-of-band autocorrelation lock can never
        // surface as a confident heart rate.
        if !(HR_PLAUSIBLE_MIN_BPM..=HR_PLAUSIBLE_MAX_BPM).contains(&bpm) {
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

    /// Two cascaded 2nd-order resonators (cardiac band: 0.8-2.0 Hz).
    /// The second stage attenuates strong respiratory components that otherwise
    /// shift the cardiac ACF peak even when a heartbeat is present (#2057).
    fn bandpass_filter(&mut self, input: f64) -> (f64, f64) {
        let omega_low = 2.0 * std::f64::consts::PI * self.freq_low / self.sample_rate;
        let omega_high = 2.0 * std::f64::consts::PI * self.freq_high / self.sample_rate;
        let bw = omega_high - omega_low;
        let center = f64::midpoint(omega_low, omega_high);

        // Resonator pole radius. The pole magnitude is `|r|`; stability needs
        // `|r| < 1`. When the normalized bandwidth `bw = 2*pi*(f_high-f_low)/fs`
        // exceeds 4 (i.e. a very low `fs` relative to the band width),
        // `1 - bw/2` falls below -1, pushing the pole *outside* the unit circle
        // and diverging the filter exponentially to ±inf. A merely-negative `r`
        // (|r| < 1) is still stable, so the clamp's job is the `|r| >= 1` case.
        // Clamp to a stable range so the pole stays inside the unit circle for
        // any `sample_rate` / band-edge configuration (ADR-157 §A3).
        let r = (1.0 - bw / 2.0).clamp(0.0, 0.9999);
        let cos_w0 = center.cos();

        let mut output = input;
        let mut single_stage = 0.0;
        for (stage, state) in self.filter_state.iter_mut().enumerate() {
            let filtered =
                (1.0 - r) * (output - state.x2) + 2.0 * r * cos_w0 * state.y1 - r * r * state.y2;

            // Reset both stages after corrupt input or overflow so later clean
            // frames can recover (ADR-158). Retain the zero-sample fallback.
            if !filtered.is_finite() {
                self.filter_state = [IirState::default(), IirState::default()];
                return (0.0, 0.0);
            }
            state.x2 = state.x1;
            state.x1 = output;
            state.y2 = state.y1;
            state.y1 = filtered;
            output = filtered;
            if stage == 0 {
                single_stage = filtered;
            }
        }
        (single_stage, output)
    }

    /// Reset all filter state and history.
    pub fn reset(&mut self) {
        self.filtered_history.clear();
        self.single_stage_history.clear();
        self.filter_state = [IirState::default(), IirState::default()];
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

/// Lag-zero-normalized evidence from the original single-stage filter.
fn autocorrelation_at_lag(signal: &[f64], lag: usize) -> f64 {
    if signal.is_empty() || lag >= signal.len() {
        return 0.0;
    }
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let energy = signal.iter().map(|x| (x - mean).powi(2)).sum::<f64>();
    if !energy.is_finite() || energy < 1e-15 {
        return 0.0;
    }
    signal
        .iter()
        .zip(&signal[lag..])
        .map(|(x, y)| (x - mean) * (y - mean))
        .sum::<f64>()
        / energy
}

/// Compute a phase-coherence-weighted signal from residuals and phases.
///
/// Combines amplitude residuals with inter-subcarrier phase coherence
/// to enhance the cardiac signal. Subcarriers with similar phase
/// derivatives are likely sensing the same body surface.
///
/// `phases` may be shorter than `n` (including empty). Missing entries mean
/// the caller has no per-subcarrier phase measurement to weight by, so that
/// subcarrier is fused with full coherence (weight 1.0) instead of being
/// excluded -- i.e. `phases=[]` degrades gracefully to equal weighting
/// across all `n` residuals, exactly like `breathing::fuse_weighted_residuals`
/// falling back to a uniform weight for a missing/partial `weights` slice
/// (issue #1423; `phases=[]` is documented as meaning equal weights, the
/// same as `BreathingExtractor`'s `weights=[]`).
fn compute_phase_coherence_signal(residuals: &[f64], phases: &[f64], n: usize) -> f64 {
    if n <= 1 {
        return residuals.first().copied().unwrap_or(0.0);
    }

    let phase_at = |i: usize| phases.get(i).copied();

    // Compute inter-subcarrier phase differences as coherence weights.
    // Adjacent subcarriers with small phase differences are more coherent.
    // If either phase value needed for a pair is missing, treat the pair as
    // fully coherent (weight 1.0) rather than panicking or excluding it.
    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;

    for (i, &r) in residuals.iter().enumerate().take(n) {
        let neighbor = if i + 1 < n {
            Some(i + 1)
        } else if i > 0 {
            Some(i - 1)
        } else {
            None
        };

        let coherence = match neighbor.and_then(|j| phase_at(i).zip(phase_at(j))) {
            Some((a, b)) => (-(b - a).abs()).exp(),
            None => 1.0,
        };

        weighted_sum += r * coherence;
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
    let min_lag = (sample_rate / freq_high).floor() as usize;
    let max_lag = (sample_rate / freq_low).ceil() as usize; // lowest freq = longest period
    let max_lag = max_lag.min(n / 2);

    if min_lag == 0 || min_lag >= max_lag || min_lag >= n {
        return (0, 0.0);
    }

    // Compute mean-subtracted signal
    let mean: f64 = signal.iter().sum::<f64>() / n as f64;

    // Autocorrelation at lag 0 for normalisation
    let acf0: f64 = signal.iter().map(|&x| (x - mean) * (x - mean)).sum();
    if !acf0.is_finite() || acf0 < 1e-15 {
        return (0, 0.0);
    }

    // Require a local maximum, including neighbors outside the admitted band.
    // A slowly varying respiratory signal has a positive but falling ACF here;
    // choosing its largest value would falsely pin HR to the upper band edge.
    // Keep lag-zero normalization: overlap correction can favor a second or
    // third cardiac period and halve an otherwise correct pulse estimate.
    let mut best_lag = 0;
    let mut best_acf = 0.0;

    let normalized_acf = |lag: usize| {
        let acf: f64 = signal
            .iter()
            .take(n - lag)
            .enumerate()
            .map(|(i, &x)| (x - mean) * (signal[i + lag] - mean))
            .sum();

        acf / acf0
    };
    let mut previous = normalized_acf(min_lag - 1);
    let mut current = normalized_acf(min_lag);
    for lag in min_lag..=max_lag {
        let next = normalized_acf(lag + 1);
        if current > previous && current >= next && current > best_acf {
            best_acf = current;
            best_lag = lag;
        }
        previous = current;
        current = next;
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
    fn issue_2057_extra_filtering_does_not_promote_noise() {
        let mut ext = HeartRateExtractor::new(56, 50.0, 15.0);
        let mut seed = 38_u64;
        let mut last = None;
        for _ in 0..250 {
            let residuals: Vec<_> = (0..56)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    0.01 * (2.0 * (seed >> 33) as f64 / (1_u64 << 31) as f64 - 1.0)
                })
                .collect();
            last = ext.extract(&residuals, &[]);
        }
        let estimate = last.expect("fixture has a noise peak to evaluate");
        assert!(
            estimate.confidence < 0.6,
            "filter-colored noise: {estimate:?}"
        );
        assert_ne!(estimate.status, VitalStatus::Valid);
    }

    fn synthetic_recording(
        sample_rate: f64,
        heart_hz: f64,
        heart_amplitude: f64,
        breath_amplitude: f64,
    ) -> Option<VitalEstimate> {
        let mut ext = HeartRateExtractor::new(56, sample_rate, 15.0);
        let mut seed = 0x1234_5678_u64;
        let mut last = None;
        for i in 0..(sample_rate * 20.0) as usize {
            let t = i as f64 / sample_rate;
            let base = heart_amplitude * (std::f64::consts::TAU * heart_hz * t).sin()
                + breath_amplitude * (std::f64::consts::TAU * 0.25 * t).sin();
            let residuals: Vec<f64> = (0..56)
                .map(|_| {
                    seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                    let uniform = (seed >> 32) as f64 / u32::MAX as f64;
                    // Deterministic zero-mean noise with standard deviation 0.01.
                    base + (uniform - 0.5) * 0.01 * 12.0_f64.sqrt()
                })
                .collect();
            last = ext.extract(&residuals, &[]);
        }
        last
    }

    #[test]
    fn issue_2057_breathing_without_heartbeat_is_not_confident_hr() {
        for sample_rate in [50.0, 100.0] {
            if let Some(estimate) = synthetic_recording(sample_rate, 1.2, 0.0, 0.1) {
                assert!(
                    estimate.confidence < 0.3,
                    "breathing alone at {sample_rate} Hz produced {estimate:?}"
                );
            }
        }
    }

    #[test]
    fn issue_2057_heartbeat_survives_stronger_breathing() {
        for breath_amplitude in [0.02, 0.05, 0.10] {
            let estimate = synthetic_recording(100.0, 1.2, 0.01, breath_amplitude)
                .expect("synthetic 72 BPM pulse must remain detectable");
            assert!(
                (estimate.value_bpm - 72.0).abs() < 3.0,
                "breath amplitude {breath_amplitude}: {estimate:?}"
            );
        }
    }

    #[test]
    fn issue_2057_monotonic_acf_is_not_a_cardiac_peak() {
        let signal: Vec<_> = (0..1500)
            .map(|i| (std::f64::consts::TAU * 0.25 * i as f64 / 100.0).sin())
            .collect();
        assert_eq!(autocorrelation_peak(&signal, 100.0, 0.8, 2.0), (0, 0.0));
    }

    #[test]
    fn issue_2057_pulse_detection_preserves_band_edges() {
        for sample_rate in [50.0, 100.0] {
            for heart_hz in [0.8, 1.0, 1.2, 1.5, 1.8, 2.0] {
                let estimate = synthetic_recording(sample_rate, heart_hz, 0.01, 0.0)
                    .expect("in-band synthetic pulse must be detected");
                assert_eq!(
                    estimate.status,
                    VitalStatus::Valid,
                    "clean pulse must retain confidence: {estimate:?}"
                );
                assert!(
                    (estimate.value_bpm - heart_hz * 60.0).abs() < 3.0,
                    "{sample_rate} Hz sampling, {heart_hz} Hz pulse: {estimate:?}"
                );
            }
        }
    }

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
        assert!(ext.single_stage_history.is_empty());
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
        assert!(
            (result - 1.0).abs() < 0.1,
            "coherent result should be ~1.0: {result}"
        );
    }

    #[test]
    fn esp32_default_creates_correctly() {
        let ext = HeartRateExtractor::esp32_default();
        assert_eq!(ext.n_subcarriers, 56);
    }

    /// Pin the physiological plausibility band to its documented values. If a
    /// future edit widens these, an implausible HR could be emitted as a
    /// confident vital — this characterization test forces that to be a
    /// deliberate, reviewed change.
    #[test]
    fn plausibility_band_constants_pinned() {
        assert!((HR_PLAUSIBLE_MIN_BPM - 40.0).abs() < f64::EPSILON);
        assert!((HR_PLAUSIBLE_MAX_BPM - 180.0).abs() < f64::EPSILON);
    }

    /// ADR-158 §A1 bug-catching test: a single non-finite residual must NOT
    /// permanently poison the IIR filter state.
    ///
    /// The cardiac resonator latches `y[n]` into `state.y1`/`y2`. Before the
    /// fix, one NaN/inf residual produced a NaN `output` that was stored into
    /// the state; the `extract()` finite-guard dropped that frame from history,
    /// but every subsequent output stayed NaN, so the history buffer never
    /// refilled and HR extraction was dead until `reset()`. After a leading NaN
    /// frame, the OLD code returned `None` with `history_len() == 0` forever.
    /// This asserts recovery (FAILS on the old code).
    #[test]
    fn nan_frame_does_not_permanently_poison_filter() {
        let sr = 50.0;
        let feed_clean = |ext: &mut HeartRateExtractor| {
            let mut last = None;
            for i in 0..1200 {
                let t = i as f64 / sr;
                let base = (2.0 * std::f64::consts::PI * 1.2 * t).sin();
                let r = vec![base * 0.1, base * 0.08, base * 0.12, base * 0.09];
                last = ext.extract(&r, &[0.0, 0.01, 0.02, 0.03]);
            }
            last
        };

        let mut control = HeartRateExtractor::new(4, sr, 20.0);
        let expected = feed_clean(&mut control).expect("clean pulse should be detected");
        assert!(
            control.history_len() > 0,
            "control clean run must accumulate history"
        );

        let mut ext = HeartRateExtractor::new(4, sr, 20.0);
        ext.extract(&[f64::NAN, 0.1, 0.1, 0.1], &[0.0, 0.01, 0.02, 0.03]);
        let recovered = feed_clean(&mut ext).expect("clean pulse must recover after NaN");
        assert!((recovered.value_bpm - expected.value_bpm).abs() < 1e-12);
        assert!((recovered.confidence - expected.confidence).abs() < 1e-12);
        assert!(
            ext.history_len() > 0,
            "HR extractor must recover and refill history after a NaN frame (got {})",
            ext.history_len()
        );
    }

    /// Safety negative: pure broadband noise (no cardiac component) must NOT be
    /// reported as a clinically `Valid` heart rate. A false "HR = 72 bpm" on
    /// noise is a safety problem (false reassurance / false alert). The
    /// extractor may still emit a low-confidence guess, but its status must be
    /// `Degraded`/`Unreliable`, never `Valid`. Mirrors the honest-negative
    /// requirement in the review brief.
    #[test]
    fn pure_noise_is_never_reported_valid() {
        let mut seed: u64 = 0x1234_5678;
        let mut rng = || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) as f64 / (1u64 << 31) as f64) - 1.0
        };
        let mut ext = HeartRateExtractor::new(8, 50.0, 20.0);
        let mut last = None;
        for _ in 0..1500 {
            let r: Vec<f64> = (0..8).map(|_| rng()).collect();
            let p: Vec<f64> = (0..8).map(|_| rng()).collect();
            last = ext.extract(&r, &p);
        }
        if let Some(est) = last {
            assert_ne!(
                est.status,
                VitalStatus::Valid,
                "pure noise must not yield a clinically Valid HR (bpm={}, conf={})",
                est.value_bpm,
                est.confidence
            );
            assert!(
                est.confidence < 0.6,
                "noise HR confidence must stay below the Valid cutoff: {}",
                est.confidence
            );
        }
    }

    /// ADR-157 §A3 bug-catching test.
    ///
    /// Divergence needs the pole *magnitude* `|r| >= 1`, i.e. `bw >= 4`. With
    /// the cardiac band widened to 0.1-0.9 Hz at `fs = 0.5` Hz,
    /// `bw = 2*pi*(0.9-0.1)/0.5 = 10.05`, so the OLD pole radius
    /// `r = 1 - bw/2 = -4.03` has `|r| = 4.03 > 1` — the filter diverges
    /// exponentially. After ~600 unit-step frames the OLD output overflows f64
    /// to ±inf/NaN; once that lands in `filtered_history`, `acf0` becomes NaN
    /// and the extractor stalls permanently. The clamp (`r.clamp(0.0, 0.9999)`)
    /// plus the finite-guard before the push keep every accumulated sample
    /// finite. This test FAILS on the old code (verified by reverting).
    #[test]
    fn low_sample_rate_filter_stays_finite() {
        let mut ext = HeartRateExtractor::new(4, 0.5, 3600.0);
        ext.freq_low = 0.1;
        ext.freq_high = 0.9;
        // Feed a unit step across 4 coherent subcarriers for 600 frames — enough
        // for the un-clamped resonator to overflow to inf.
        for _ in 0..600 {
            ext.extract(&[1.0, 1.0, 1.0, 1.0], &[0.0, 0.01, 0.02, 0.03]);
        }
        assert!(
            ext.history_len() > 0,
            "history should have accumulated samples"
        );
        for (i, &v) in ext.filtered_history.iter().enumerate() {
            assert!(
                v.is_finite(),
                "filtered_history[{i}] must be finite, got {v}"
            );
        }
    }

    /// Issue #1423 bug-catching test.
    ///
    /// Reproduces the exact GH-issue repro: 56 subcarriers @ 100 Hz (ESP32
    /// defaults), a noiseless 1.2 Hz (72 BPM) sine identical across every
    /// subcarrier, fed frame-by-frame with an **empty** `phases` slice.
    ///
    /// Before the fix, `extract()`'s `n` was
    /// `residuals.len().min(n_subcarriers).min(phases.len())`, so
    /// `phases = []` forced `n == 0` and every single frame returned `None`
    /// (0/4000 estimates) even though the identical call with
    /// `phases = [1.0; 56]` produced thousands of valid estimates. `phases=[]`
    /// must mean "no coherence weighting available", i.e. equal weighting --
    /// the same documented fallback `BreathingExtractor` already honors for
    /// `weights=[]` -- not "invalid input, refuse everything".
    #[test]
    fn issue_1423_empty_phases_yields_equal_weighting_not_silent_none() {
        let n = 56usize;
        let sample_rate = 100.0;
        let heart_freq = 1.2; // 72 BPM

        let mut ext = HeartRateExtractor::esp32_default();
        let mut estimates_with_empty_phases = 0usize;
        for i in 0..4000usize {
            let t = i as f64 / sample_rate;
            let base = (2.0 * std::f64::consts::PI * heart_freq * t).sin();
            let residuals = vec![base; n];
            if ext.extract(&residuals, &[]).is_some() {
                estimates_with_empty_phases += 1;
            }
        }

        assert!(
            estimates_with_empty_phases > 0,
            "HeartRateExtractor::extract() with phases=[] must not silently return None for \
             every frame of a clean 72 BPM signal (issue #1423); got 0/4000 estimates",
        );

        // Sanity check against the issue's own comparison point: an
        // equivalent uniform, non-empty `phases` slice (all subcarriers at
        // the same phase, i.e. zero pairwise difference => full coherence,
        // exactly what the equal-weighting fallback now produces for the
        // empty case too) must yield a comparable number of estimates --
        // the two should behave the same, not `0` vs `thousands`.
        let mut ext2 = HeartRateExtractor::esp32_default();
        let uniform_phases = vec![1.0_f64; n];
        let mut estimates_with_uniform_phases = 0usize;
        for i in 0..4000usize {
            let t = i as f64 / sample_rate;
            let base = (2.0 * std::f64::consts::PI * heart_freq * t).sin();
            let residuals = vec![base; n];
            if ext2.extract(&residuals, &uniform_phases).is_some() {
                estimates_with_uniform_phases += 1;
            }
        }

        assert_eq!(
            estimates_with_empty_phases, estimates_with_uniform_phases,
            "phases=[] (equal-weight fallback) must behave identically to a uniform, \
             non-empty phases slice of the same length -- both represent 'no differential \
             coherence information', got {estimates_with_empty_phases} vs \
             {estimates_with_uniform_phases}",
        );
    }

    /// Issue #1423 companion: a `phases` slice *shorter* than `residuals`
    /// (partial coverage) must fall back to equal weighting for the missing
    /// tail rather than truncating the whole fusion down to the phases that
    /// happen to be present -- mirrors
    /// `breathing::partial_weights_are_renormalized_not_scale_mixed`.
    #[test]
    fn partial_phases_do_not_truncate_subcarrier_count() {
        // 8 residuals, only 2 phase values supplied.
        let residuals = [1.0_f64; 8];
        let phases = [0.0_f64, 0.0];
        let fused = compute_phase_coherence_signal(&residuals, &phases, 8);

        // All residuals are identical (1.0), so regardless of how coherence
        // weights are distributed the fused value must still be 1.0 -- but
        // this only holds if all 8 residuals are actually used. Before the
        // fix, `n` would have been truncated to `phases.len() == 2`, so the
        // caller-facing `extract()` never even reached this function with
        // `n == 8`; this test pins `compute_phase_coherence_signal` itself
        // to handle a short `phases` slice safely and correctly when called
        // with the full `n`.
        assert!(
            (fused - 1.0).abs() < 1e-12,
            "fusion with a partial phases slice must still average all {} residuals, got {fused}",
            residuals.len(),
        );
    }
}
