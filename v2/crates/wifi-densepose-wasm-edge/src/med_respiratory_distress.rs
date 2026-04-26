//! Respiratory distress detection — ADR-041 Category 1 Medical module.
//!
//! Detects pathological breathing patterns from host CSI pipeline:
//!   - Tachypnea: sustained breathing rate > 25 BPM
//!   - Labored breathing: high amplitude variance relative to baseline
//!   - Cheyne-Stokes respiration: crescendo-decrescendo periodicity (30-90 s)
//!     detected via autocorrelation of the breathing amplitude envelope
//!   - Overall respiratory distress level: composite severity score 0-100
//!
//! Events:
//!   TACHYPNEA           (120) — sustained high respiratory rate
//!   LABORED_BREATHING   (121) — high amplitude variance / effort
//!   CHEYNE_STOKES       (122) — periodic waxing-waning pattern detected
//!   RESP_DISTRESS_LEVEL (123) — composite distress score 0-100
//!
//! Host API inputs: breathing BPM, phase, variance.
//! Budget: H (< 10 ms).

// ── libm ────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "std"))]
use libm::{sqrtf, fabsf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

// ── Constants ───────────────────────────────────────────────────────────────

/// Tachypnea threshold (BPM).
const TACHYPNEA_THRESH: f32 = 25.0;

/// Sustained-rate debounce (seconds).
const SUSTAINED_SECS: u8 = 8;

/// Variance ring buffer for labored breathing detection.
const VAR_WINDOW: usize = 60;

/// Labored breathing: variance ratio above baseline to trigger.
const LABORED_VAR_RATIO: f32 = 3.0;

/// Autocorrelation buffer for Cheyne-Stokes detection.
/// Needs at least 90 seconds at 1 Hz to detect 30-90 s periodicity.
const AC_WINDOW: usize = 120;

/// Cheyne-Stokes autocorrelation peak threshold.
const CS_PEAK_THRESH: f32 = 0.35;

/// Lag range for Cheyne-Stokes period (30-90 seconds).
const CS_LAG_MIN: usize = 30;
const CS_LAG_MAX: usize = 90;

/// Distress-level report interval (seconds).
const DISTRESS_REPORT_INTERVAL: u32 = 30;

/// Alert cooldown (seconds).
const COOLDOWN_SECS: u16 = 20;

/// Baseline learning period (seconds).
const BASELINE_SECS: u32 = 60;

// ── Event IDs ───────────────────────────────────────────────────────────────

pub const EVENT_TACHYPNEA: i32 = 120;
pub const EVENT_LABORED_BREATHING: i32 = 121;
pub const EVENT_CHEYNE_STOKES: i32 = 122;
pub const EVENT_RESP_DISTRESS_LEVEL: i32 = 123;

// ── State ───────────────────────────────────────────────────────────────────

/// Respiratory distress detector.
pub struct RespiratoryDistressDetector {
    // ── Ring buffers ────────────────────────────────────────────────
    /// Breathing BPM history for autocorrelation.
    bpm_buf: [f32; AC_WINDOW],
    bpm_idx: usize,
    bpm_len: usize,

    /// Variance history for labored-breathing baseline.
    var_buf: [f32; VAR_WINDOW],
    var_idx: usize,
    var_len: usize,

    // ── Baselines ───────────────────────────────────────────────────
    /// Running mean of variance (Welford).
    var_mean: f32,
    var_count: u32,

    // ── Debounce / cooldown ─────────────────────────────────────────
    tachy_count: u8,
    cd_tachy: u16,
    cd_labored: u16,
    cd_cs: u16,

    // ── Composite distress ──────────────────────────────────────────
    last_distress: f32,

    /// Frame counter.
    frame_count: u32,
}

impl RespiratoryDistressDetector {
    pub const fn new() -> Self {
        Self {
            bpm_buf: [0.0; AC_WINDOW],
            bpm_idx: 0,
            bpm_len: 0,
            var_buf: [0.0; VAR_WINDOW],
            var_idx: 0,
            var_len: 0,
            var_mean: 0.0,
            var_count: 0,
            tachy_count: 0,
            cd_tachy: 0,
            cd_labored: 0,
            cd_cs: 0,
            last_distress: 0.0,
            frame_count: 0,
        }
    }

    /// Process one frame at ~1 Hz.
    ///
    /// * `breathing_bpm` — current breathing rate from host
    /// * `_phase` — reserved for future phase-based analysis
    /// * `variance` — amplitude variance from host (proxy for effort)
    ///
    /// Returns `&[(event_id, value)]`.
    pub fn process_frame(
        &mut self,
        breathing_bpm: f32,
        _phase: f32,
        variance: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        self.cd_tachy = self.cd_tachy.saturating_sub(1);
        self.cd_labored = self.cd_labored.saturating_sub(1);
        self.cd_cs = self.cd_cs.saturating_sub(1);

        // Guard against NaN inputs — skip ring buffer update to avoid
        // contaminating autocorrelation and baseline calculations.
        let bpm_valid = breathing_bpm == breathing_bpm; // NaN != NaN
        let var_valid = variance == variance;

        // Push into ring buffers (only valid values).
        if bpm_valid {
            self.bpm_buf[self.bpm_idx] = breathing_bpm;
            self.bpm_idx = (self.bpm_idx + 1) % AC_WINDOW;
            if self.bpm_len < AC_WINDOW { self.bpm_len += 1; }
        }

        if var_valid {
            self.var_buf[self.var_idx] = variance;
            self.var_idx = (self.var_idx + 1) % VAR_WINDOW;
            if self.var_len < VAR_WINDOW { self.var_len += 1; }
        }

        // Update baseline variance mean (Welford online).
        if var_valid && self.frame_count <= BASELINE_SECS {
            self.var_count += 1;
            let d = variance - self.var_mean;
            self.var_mean += d / self.var_count as f32;
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;

        // ── Tachypnea ───────────────────────────────────────────────────
        if breathing_bpm > TACHYPNEA_THRESH {
            self.tachy_count = self.tachy_count.saturating_add(1);
            if self.tachy_count >= SUSTAINED_SECS && self.cd_tachy == 0 && n < 4 {
                unsafe { EVENTS[n] = (EVENT_TACHYPNEA, breathing_bpm); }
                n += 1;
                self.cd_tachy = COOLDOWN_SECS;
            }
        } else {
            self.tachy_count = 0;
        }

        // ── Labored breathing ───────────────────────────────────────────
        if self.var_count >= BASELINE_SECS && self.var_mean > 0.001 {
            let current_var = self.recent_var_mean();
            let ratio = current_var / self.var_mean;
            if ratio > LABORED_VAR_RATIO && self.cd_labored == 0 && n < 4 {
                unsafe { EVENTS[n] = (EVENT_LABORED_BREATHING, ratio); }
                n += 1;
                self.cd_labored = COOLDOWN_SECS;
            }
        }

        // ── Cheyne-Stokes (autocorrelation) ─────────────────────────────
        if self.bpm_len >= AC_WINDOW && self.cd_cs == 0 && n < 4 {
            if let Some(period) = self.detect_cheyne_stokes() {
                unsafe { EVENTS[n] = (EVENT_CHEYNE_STOKES, period as f32); }
                n += 1;
                self.cd_cs = COOLDOWN_SECS;
            }
        }

        // ── Composite distress level ────────────────────────────────────
        if self.frame_count % DISTRESS_REPORT_INTERVAL == 0 && n < 4 {
            let score = self.compute_distress_score(breathing_bpm, variance);
            self.last_distress = score;
            unsafe { EVENTS[n] = (EVENT_RESP_DISTRESS_LEVEL, score); }
            n += 1;
        }

        unsafe { &EVENTS[..n] }
    }

    /// Mean of recent variance samples.
    fn recent_var_mean(&self) -> f32 {
        if self.var_len == 0 { return 0.0; }
        let mut sum = 0.0f32;
        for i in 0..self.var_len {
            sum += self.var_buf[i];
        }
        sum / self.var_len as f32
    }

    /// Detect Cheyne-Stokes periodicity via normalised autocorrelation.
    ///
    /// Returns the period in seconds if a significant peak is found in the
    /// 30-90 second lag range.
    fn detect_cheyne_stokes(&self) -> Option<usize> {
        if self.bpm_len < AC_WINDOW {
            return None;
        }

        // Compute mean.
        let mut sum = 0.0f32;
        for i in 0..self.bpm_len {
            sum += self.bpm_buf[i];
        }
        let mean = sum / self.bpm_len as f32;

        // Compute variance (for normalisation).
        let mut var_sum = 0.0f32;
        for i in 0..self.bpm_len {
            let d = self.bpm_buf[i] - mean;
            var_sum += d * d;
        }
        let var = var_sum / self.bpm_len as f32;
        if var < 0.01 { return None; } // flat signal, no periodicity

        // Autocorrelation for lags in Cheyne-Stokes range.
        let start = if self.bpm_len < AC_WINDOW { 0 } else { self.bpm_idx };
        let mut best_peak = 0.0f32;
        let mut best_lag = 0usize;

        let lag_max = CS_LAG_MAX.min(self.bpm_len - 1);

        for lag in CS_LAG_MIN..=lag_max {
            let mut ac = 0.0f32;
            let samples = self.bpm_len - lag;
            for i in 0..samples {
                let a = self.bpm_buf[(start + i) % AC_WINDOW] - mean;
                let b = self.bpm_buf[(start + i + lag) % AC_WINDOW] - mean;
                ac += a * b;
            }
            let norm_ac = ac / (samples as f32 * var);
            if norm_ac > best_peak {
                best_peak = norm_ac;
                best_lag = lag;
            }
        }

        if best_peak > CS_PEAK_THRESH {
            Some(best_lag)
        } else {
            None
        }
    }

    /// Compute composite respiratory distress score (0-100).
    fn compute_distress_score(&self, breathing_bpm: f32, variance: f32) -> f32 {
        let mut score = 0.0f32;

        // Rate component: distance from normal (12-20 BPM centre at 16).
        let rate_dev = fabsf(breathing_bpm - 16.0);
        score += (rate_dev / 20.0).min(1.0) * 40.0;

        // Variance component.
        if self.var_mean > 0.001 {
            let ratio = variance / self.var_mean;
            score += ((ratio - 1.0).max(0.0) / 5.0).min(1.0) * 30.0;
        }

        // Tachypnea component.
        if breathing_bpm > TACHYPNEA_THRESH {
            score += 20.0;
        }

        // Cheyne-Stokes detected recently.
        if self.cd_cs > 0 && self.cd_cs < COOLDOWN_SECS {
            score += 10.0;
        }

        if score > 100.0 { 100.0 } else { score }
    }

    /// Last computed distress score.
    pub fn last_distress_score(&self) -> f32 {
        self.last_distress
    }

    /// Frame count.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let d = RespiratoryDistressDetector::new();
        assert_eq!(d.frame_count(), 0);
        assert!((d.last_distress_score() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_normal_breathing_no_alerts() {
        let mut d = RespiratoryDistressDetector::new();
        for _ in 0..120 {
            let ev = d.process_frame(16.0, 0.0, 0.5);
            for &(t, _) in ev {
                assert!(
                    t != EVENT_TACHYPNEA && t != EVENT_LABORED_BREATHING && t != EVENT_CHEYNE_STOKES,
                    "no respiratory distress alerts with normal breathing"
                );
            }
        }
    }

    #[test]
    fn test_tachypnea_detection() {
        let mut d = RespiratoryDistressDetector::new();
        let mut found = false;
        for _ in 0..30 {
            let ev = d.process_frame(30.0, 0.0, 0.5);
            for &(t, _) in ev {
                if t == EVENT_TACHYPNEA { found = true; }
            }
        }
        assert!(found, "tachypnea should trigger with sustained rate > 25");
    }

    #[test]
    fn test_labored_breathing_detection() {
        let mut d = RespiratoryDistressDetector::new();
        // Build baseline with low variance.
        for _ in 0..BASELINE_SECS {
            d.process_frame(16.0, 0.0, 0.1);
        }
        // Inject high variance.
        let mut found = false;
        for _ in 0..120 {
            let ev = d.process_frame(16.0, 0.0, 5.0);
            for &(t, _) in ev {
                if t == EVENT_LABORED_BREATHING { found = true; }
            }
        }
        assert!(found, "labored breathing should trigger with high variance");
    }

    #[test]
    fn test_distress_score_emitted() {
        let mut d = RespiratoryDistressDetector::new();
        let mut found = false;
        for _ in 0..DISTRESS_REPORT_INTERVAL + 1 {
            let ev = d.process_frame(16.0, 0.0, 0.5);
            for &(t, _) in ev {
                if t == EVENT_RESP_DISTRESS_LEVEL { found = true; }
            }
        }
        assert!(found, "distress level should be reported periodically");
    }

    #[test]
    fn test_cheyne_stokes_detection() {
        let mut d = RespiratoryDistressDetector::new();
        // Simulate crescendo-decrescendo with 60-second period:
        // BPM oscillates between 5 and 25 with sinusoidal-like pattern.
        let mut found = false;
        let period = 60.0f32;
        for i in 0..300u32 {
            let phase = (i as f32) / period * 2.0 * core::f32::consts::PI;
            // Use a manual sin approximation for no_std compatibility in tests.
            let sin_val = manual_sin(phase);
            let bpm = 15.0 + 10.0 * sin_val;
            let ev = d.process_frame(bpm, 0.0, 0.5);
            for &(t, v) in ev {
                if t == EVENT_CHEYNE_STOKES {
                    found = true;
                    // Period should be near 60.
                    assert!(v > 25.0 && v < 95.0,
                        "Cheyne-Stokes period should be in 30-90 range, got {}", v);
                }
            }
        }
        assert!(found, "Cheyne-Stokes should be detected with periodic breathing");
    }

    #[test]
    fn test_distress_score_range() {
        let mut d = RespiratoryDistressDetector::new();
        // Build baseline.
        for _ in 0..BASELINE_SECS {
            d.process_frame(16.0, 0.0, 0.5);
        }
        // Feed distressed breathing until report.
        for _ in 0..DISTRESS_REPORT_INTERVAL {
            d.process_frame(35.0, 0.0, 5.0);
        }
        let score = d.last_distress_score();
        assert!(score >= 0.0 && score <= 100.0, "distress score should be 0-100, got {}", score);
        assert!(score > 30.0, "distress score should be elevated with tachypnea + high variance, got {}", score);
    }

    /// Simple sin approximation (Taylor series, 5 terms) for test use.
    fn manual_sin(x: f32) -> f32 {
        // Normalize to [-pi, pi].
        let pi = core::f32::consts::PI;
        let mut x = x % (2.0 * pi);
        if x > pi { x -= 2.0 * pi; }
        if x < -pi { x += 2.0 * pi; }
        let x2 = x * x;
        let x3 = x2 * x;
        let x5 = x3 * x2;
        let x7 = x5 * x2;
        x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0
    }
}
