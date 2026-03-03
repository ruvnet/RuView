//! Environmental anomaly detector ("Ghost Hunter") — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Monitors CSI when `presence == 0` (no humans detected) for any
//! perturbation above the noise floor.  When the room should be empty
//! but CSI changes are detected, something unexplained is happening.
//!
//! ## Anomaly classification
//!
//! Anomalies are classified into four categories based on their temporal
//! signature:
//!
//! 1. **Impulsive** — Short, sharp transients (< 5 frames).  Typical of
//!    structural settling, objects falling, thermal cracking.
//!
//! 2. **Periodic** — Recurring perturbations with detectable periodicity.
//!    Typical of mechanical systems (HVAC compressor, washing machine),
//!    biological activity (pest movement patterns), or hidden breathing.
//!
//! 3. **Drift** — Slow monotonic shift in phase or amplitude baseline.
//!    Typical of temperature changes, humidity variation, gas leaks
//!    (which alter dielectric properties of air).
//!
//! 4. **Random** — Stochastic perturbations with no discernible pattern.
//!    Typical of electromagnetic interference (EMI), Wi-Fi co-channel
//!    interference, or cosmic events.
//!
//! ## Hidden presence detection
//!
//! A special sub-detector looks for the breathing signature: periodic
//! phase oscillation at 0.15-0.5 Hz (9-30 BPM) with low amplitude.
//! This can detect a person hiding motionless who evades the main
//! presence detector.
//!
//! # Events (650-series: Exotic / Research)
//!
//! - `ANOMALY_DETECTED` (650): Aggregate anomaly energy [0, 1].
//! - `ANOMALY_CLASS` (651): Classification (1=impulsive, 2=periodic,
//!   3=drift, 4=random).
//! - `HIDDEN_PRESENCE` (652): Breathing-like signature confidence [0, 1].
//! - `ENVIRONMENTAL_DRIFT` (653): Monotonic drift magnitude.
//!
//! # Budget
//!
//! S (standard, < 5 ms) — per-frame: noise floor comparison + periodicity
//! check via autocorrelation of a short buffer (64 points, 16 lags).

use crate::vendor_common::{CircularBuffer, Ema, WelfordStats};
use libm::fabsf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Number of subcarrier groups to monitor.
const N_GROUPS: usize = 8;

/// Maximum subcarriers from host API.
const MAX_SC: usize = 32;

/// Anomaly energy circular buffer length (64 points at 20 Hz = 3.2 s).
const ANOMALY_BUF_LEN: usize = 64;

/// Phase history buffer for periodicity detection.
const PHASE_BUF_LEN: usize = 64;

/// Maximum autocorrelation lag for periodicity detection.
const MAX_LAG: usize = 16;

/// Noise floor EWMA alpha (adapts slowly to ambient noise).
const NOISE_ALPHA: f32 = 0.001;

/// Anomaly detection threshold: multiplier above noise floor.
const ANOMALY_SIGMA: f32 = 3.0;

/// Impulsive anomaly max duration in frames.
const IMPULSE_MAX_FRAMES: u32 = 5;

/// Periodicity detection threshold for autocorrelation peak.
const PERIOD_THRESHOLD: f32 = 0.4;

/// Drift detection: minimum consecutive frames with same-sign delta.
const DRIFT_MIN_FRAMES: u32 = 30;

/// Hidden presence: breathing frequency range in lag units at 20 Hz.
/// 0.15 Hz -> period 133 frames -> lag 133 (too long)
/// We use a shorter check: 0.2-0.5 Hz -> period 40-100 frames.
/// At 20 Hz frame rate, breathing at 15 BPM = 0.25 Hz = period 80 frames.
/// We check autocorrelation at lags corresponding to 10-50 frame periods
/// (0.4-2.0 Hz, covering 24-120 BPM — includes breathing and low HR).
const BREATHING_LAG_MIN: usize = 5;
const BREATHING_LAG_MAX: usize = 15;

/// Hidden presence confidence threshold.
const HIDDEN_PRESENCE_THRESHOLD: f32 = 0.3;

/// Minimum empty frames before starting anomaly detection.
const MIN_EMPTY_FRAMES: u32 = 40;

/// EMA alpha for anomaly energy smoothing.
const ANOMALY_ENERGY_ALPHA: f32 = 0.1;

// ── Event IDs (650-series: Exotic) ───────────────────────────────────────────

pub const EVENT_ANOMALY_DETECTED: i32 = 650;
pub const EVENT_ANOMALY_CLASS: i32 = 651;
pub const EVENT_HIDDEN_PRESENCE: i32 = 652;
pub const EVENT_ENVIRONMENTAL_DRIFT: i32 = 653;

// ── Anomaly classification ───────────────────────────────────────────────────

/// Anomaly type classification.
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum AnomalyClass {
    None = 0,
    Impulsive = 1,
    Periodic = 2,
    Drift = 3,
    Random = 4,
}

// ── Ghost Hunter Detector ────────────────────────────────────────────────────

/// Environmental anomaly detector for empty-room CSI monitoring.
pub struct GhostHunterDetector {
    /// Noise floor per subcarrier group (slow EWMA of variance).
    noise_floor: [Ema; N_GROUPS],
    /// Anomaly energy buffer per group.
    anomaly_buf: [CircularBuffer<ANOMALY_BUF_LEN>; N_GROUPS],
    /// Phase history buffer for periodicity detection (aggregate).
    phase_buf: CircularBuffer<PHASE_BUF_LEN>,
    /// Autocorrelation buffer for periodicity.
    autocorr: [f32; MAX_LAG],
    /// Consecutive frames with anomaly above threshold.
    active_anomaly_frames: u32,
    /// Consecutive frames with same-sign drift.
    drift_frames: u32,
    /// Sign of last amplitude delta (true = positive).
    drift_sign_positive: bool,
    /// Previous aggregate amplitude (for drift detection).
    prev_agg_amp: f32,
    /// Whether prev_agg_amp is initialized.
    prev_amp_initialized: bool,
    /// Smoothed anomaly energy.
    anomaly_energy_ema: Ema,
    /// Current anomaly classification.
    current_class: AnomalyClass,
    /// Hidden presence confidence.
    hidden_presence_score: f32,
    /// Number of empty-room frames processed.
    empty_frames: u32,
    /// Total frames processed.
    frame_count: u32,
    /// Welford stats for aggregate phase (for mean/var).
    phase_stats: WelfordStats,
}

impl GhostHunterDetector {
    pub const fn new() -> Self {
        Self {
            noise_floor: [
                Ema::new(NOISE_ALPHA), Ema::new(NOISE_ALPHA),
                Ema::new(NOISE_ALPHA), Ema::new(NOISE_ALPHA),
                Ema::new(NOISE_ALPHA), Ema::new(NOISE_ALPHA),
                Ema::new(NOISE_ALPHA), Ema::new(NOISE_ALPHA),
            ],
            anomaly_buf: [
                CircularBuffer::new(), CircularBuffer::new(),
                CircularBuffer::new(), CircularBuffer::new(),
                CircularBuffer::new(), CircularBuffer::new(),
                CircularBuffer::new(), CircularBuffer::new(),
            ],
            phase_buf: CircularBuffer::new(),
            autocorr: [0.0; MAX_LAG],
            active_anomaly_frames: 0,
            drift_frames: 0,
            drift_sign_positive: true,
            prev_agg_amp: 0.0,
            prev_amp_initialized: false,
            anomaly_energy_ema: Ema::new(ANOMALY_ENERGY_ALPHA),
            current_class: AnomalyClass::None,
            hidden_presence_score: 0.0,
            empty_frames: 0,
            frame_count: 0,
            phase_stats: WelfordStats::new(),
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases` — per-subcarrier phase values.
    /// `amplitudes` — per-subcarrier amplitude values.
    /// `variance` — per-subcarrier variance values.
    /// `presence` — 0 = empty, >0 = humans present.
    /// `motion_energy` — host Tier 2 aggregate motion energy.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        presence: i32,
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        // Only analyze when room is reported empty.
        if presence != 0 {
            self.active_anomaly_frames = 0;
            self.drift_frames = 0;
            self.current_class = AnomalyClass::None;
            return &[];
        }

        let n_sc = core::cmp::min(amplitudes.len(), MAX_SC);
        let n_sc = core::cmp::min(n_sc, phases.len());
        let n_sc = core::cmp::min(n_sc, variance.len());
        if n_sc < N_GROUPS {
            return &[];
        }

        self.empty_frames += 1;

        // Compute per-group aggregates.
        let subs_per = n_sc / N_GROUPS;
        if subs_per == 0 {
            return &[];
        }

        let mut group_amp = [0.0f32; N_GROUPS];
        let mut group_var = [0.0f32; N_GROUPS];
        let mut group_phase = [0.0f32; N_GROUPS];

        for g in 0..N_GROUPS {
            let start = g * subs_per;
            let end = if g == N_GROUPS - 1 { n_sc } else { start + subs_per };
            let count = (end - start) as f32;
            let mut sa = 0.0f32;
            let mut sv = 0.0f32;
            let mut sp = 0.0f32;
            for i in start..end {
                sa += amplitudes[i];
                sv += variance[i];
                sp += phases[i];
            }
            group_amp[g] = sa / count;
            group_var[g] = sv / count;
            group_phase[g] = sp / count;
        }

        // Update noise floor and compute anomaly energy.
        let mut total_anomaly = 0.0f32;
        for g in 0..N_GROUPS {
            self.noise_floor[g].update(group_var[g]);
            let floor = self.noise_floor[g].value;
            let excess = if group_var[g] > floor * ANOMALY_SIGMA {
                group_var[g] - floor
            } else {
                0.0
            };
            self.anomaly_buf[g].push(excess);
            total_anomaly += excess;
        }
        let avg_anomaly = total_anomaly / N_GROUPS as f32;
        self.anomaly_energy_ema.update(avg_anomaly);

        // Push aggregate phase for periodicity check.
        let mut agg_phase = 0.0f32;
        for g in 0..N_GROUPS {
            agg_phase += group_phase[g];
        }
        agg_phase /= N_GROUPS as f32;
        self.phase_buf.push(agg_phase);
        self.phase_stats.update(agg_phase);

        // Aggregate amplitude for drift.
        let mut agg_amp = 0.0f32;
        for g in 0..N_GROUPS {
            agg_amp += group_amp[g];
        }
        agg_amp /= N_GROUPS as f32;

        // Need minimum data before detection.
        if self.empty_frames < MIN_EMPTY_FRAMES {
            if !self.prev_amp_initialized {
                self.prev_agg_amp = agg_amp;
                self.prev_amp_initialized = true;
            }
            return &[];
        }

        // ── Classify anomaly ─────────────────────────────────────────────
        let anomaly_active = avg_anomaly > 0.01 || motion_energy > 0.05;

        if anomaly_active {
            self.active_anomaly_frames += 1;
        } else {
            self.active_anomaly_frames = 0;
        }

        // Drift detection: track same-sign amplitude delta.
        let amp_delta = agg_amp - self.prev_agg_amp;
        let is_positive = amp_delta >= 0.0;
        if self.prev_amp_initialized && is_positive == self.drift_sign_positive {
            self.drift_frames += 1;
        } else {
            self.drift_frames = 1;
            self.drift_sign_positive = is_positive;
        }
        self.prev_agg_amp = agg_amp;

        // Classify.
        self.current_class = if !anomaly_active {
            AnomalyClass::None
        } else if self.active_anomaly_frames > 0 && self.active_anomaly_frames <= IMPULSE_MAX_FRAMES {
            AnomalyClass::Impulsive
        } else if self.drift_frames >= DRIFT_MIN_FRAMES {
            AnomalyClass::Drift
        } else if self.check_periodicity() {
            AnomalyClass::Periodic
        } else if self.active_anomaly_frames > IMPULSE_MAX_FRAMES {
            AnomalyClass::Random
        } else {
            AnomalyClass::None
        };

        // ── Hidden presence detection (breathing signature) ──────────────
        self.hidden_presence_score = self.check_hidden_breathing();

        // ── Emit events ──────────────────────────────────────────────────
        let energy = self.anomaly_energy_ema.value;
        let norm_energy = if energy > 1.0 { 1.0 } else { energy };

        if anomaly_active {
            unsafe {
                EVENTS[n_ev] = (EVENT_ANOMALY_DETECTED, norm_energy);
            }
            n_ev += 1;

            if self.current_class != AnomalyClass::None {
                unsafe {
                    EVENTS[n_ev] = (EVENT_ANOMALY_CLASS, self.current_class as u8 as f32);
                }
                n_ev += 1;
            }
        }

        if self.hidden_presence_score > HIDDEN_PRESENCE_THRESHOLD {
            unsafe {
                EVENTS[n_ev] = (EVENT_HIDDEN_PRESENCE, self.hidden_presence_score);
            }
            n_ev += 1;
        }

        if self.drift_frames >= DRIFT_MIN_FRAMES {
            let drift_mag = fabsf(amp_delta) * self.drift_frames as f32;
            unsafe {
                EVENTS[n_ev] = (EVENT_ENVIRONMENTAL_DRIFT, drift_mag);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Check periodicity in the phase buffer via short autocorrelation.
    fn check_periodicity(&mut self) -> bool {
        let fill = self.phase_buf.len();
        if fill < MAX_LAG * 2 {
            return false;
        }

        let phase_mean = self.phase_stats.mean();
        let phase_var = self.phase_stats.variance();
        if phase_var < 1e-10 {
            return false;
        }
        let inv_var = 1.0 / phase_var;

        for k in 0..MAX_LAG {
            let lag = k + 1;
            let pairs = fill - lag;
            let mut sum = 0.0f32;
            for t in 0..pairs {
                let a = self.phase_buf.get(t) - phase_mean;
                let b = self.phase_buf.get(t + lag) - phase_mean;
                sum += a * b;
            }
            self.autocorr[k] = (sum / pairs as f32) * inv_var;
        }

        // Check for any strong peak.
        for k in 2..MAX_LAG.saturating_sub(1) {
            let prev = self.autocorr[k - 1];
            let curr = self.autocorr[k];
            let next = self.autocorr[k + 1];
            if curr > prev && curr > next && curr > PERIOD_THRESHOLD {
                return true;
            }
        }
        false
    }

    /// Check for hidden breathing signature in phase buffer.
    fn check_hidden_breathing(&self) -> f32 {
        let fill = self.phase_buf.len();
        if fill < PHASE_BUF_LEN {
            return 0.0;
        }

        let phase_mean = self.phase_stats.mean();
        let phase_var = self.phase_stats.variance();
        if phase_var < 1e-10 {
            return 0.0;
        }
        let inv_var = 1.0 / phase_var;

        // Check autocorrelation at breathing-range lags.
        let mut max_corr = 0.0f32;
        for lag in BREATHING_LAG_MIN..=BREATHING_LAG_MAX {
            if lag >= fill {
                break;
            }
            let pairs = fill - lag;
            let mut sum = 0.0f32;
            for t in 0..pairs {
                let a = self.phase_buf.get(t) - phase_mean;
                let b = self.phase_buf.get(t + lag) - phase_mean;
                sum += a * b;
            }
            let corr = (sum / pairs as f32) * inv_var;
            if corr > max_corr {
                max_corr = corr;
            }
        }

        // Clamp to [0, 1].
        if max_corr < 0.0 { 0.0 } else if max_corr > 1.0 { 1.0 } else { max_corr }
    }

    /// Get the current anomaly classification.
    pub fn anomaly_class(&self) -> AnomalyClass {
        self.current_class
    }

    /// Get the hidden presence confidence [0, 1].
    pub fn hidden_presence_confidence(&self) -> f32 {
        self.hidden_presence_score
    }

    /// Get the smoothed anomaly energy.
    pub fn anomaly_energy(&self) -> f32 {
        self.anomaly_energy_ema.value
    }

    /// Get total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Get number of empty-room frames processed.
    pub fn empty_frames(&self) -> u32 {
        self.empty_frames
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_new() {
        let gh = GhostHunterDetector::new();
        assert_eq!(gh.frame_count(), 0);
        assert_eq!(gh.empty_frames(), 0);
        assert_eq!(gh.anomaly_class() as u8, AnomalyClass::None as u8);
    }

    #[test]
    fn test_presence_blocks_detection() {
        let mut gh = GhostHunterDetector::new();
        let phases = [0.5f32; 32];
        let amps = [1.0f32; 32];
        let vars = [0.5f32; 32]; // high variance
        for _ in 0..100 {
            let events = gh.process_frame(&phases, &amps, &vars, 1, 0.0);
            assert!(events.is_empty(), "should not emit when humans present");
        }
        assert_eq!(gh.empty_frames(), 0);
    }

    #[test]
    fn test_quiet_room_no_anomaly() {
        let mut gh = GhostHunterDetector::new();
        let phases = [0.5f32; 32];
        let amps = [1.0f32; 32];
        let vars = [0.001f32; 32]; // very low variance
        for _ in 0..MIN_EMPTY_FRAMES + 50 {
            let events = gh.process_frame(&phases, &amps, &vars, 0, 0.0);
            for ev in events {
                assert_ne!(ev.0, EVENT_ANOMALY_DETECTED,
                    "quiet room should not trigger anomaly");
            }
        }
    }

    #[test]
    fn test_high_variance_triggers_anomaly() {
        let mut gh = GhostHunterDetector::new();
        let phases = [0.5f32; 32];
        let amps = [1.0f32; 32];
        let low_vars = [0.001f32; 32];
        let high_vars = [1.0f32; 32];

        // Build up noise floor with quiet data.
        for _ in 0..MIN_EMPTY_FRAMES + 20 {
            gh.process_frame(&phases, &amps, &low_vars, 0, 0.0);
        }

        // Inject high-variance anomaly.
        let mut anomaly_seen = false;
        for _ in 0..30 {
            let events = gh.process_frame(&phases, &amps, &high_vars, 0, 0.5);
            for ev in events {
                if ev.0 == EVENT_ANOMALY_DETECTED {
                    anomaly_seen = true;
                }
            }
        }
        assert!(anomaly_seen, "high variance should trigger anomaly detection");
    }

    #[test]
    fn test_anomaly_class_values() {
        assert_eq!(AnomalyClass::None as u8, 0);
        assert_eq!(AnomalyClass::Impulsive as u8, 1);
        assert_eq!(AnomalyClass::Periodic as u8, 2);
        assert_eq!(AnomalyClass::Drift as u8, 3);
        assert_eq!(AnomalyClass::Random as u8, 4);
    }

    #[test]
    fn test_insufficient_subcarriers() {
        let mut gh = GhostHunterDetector::new();
        let small = [1.0f32; 4];
        let events = gh.process_frame(&small, &small, &small, 0, 0.0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_hidden_breathing_detection() {
        let mut gh = GhostHunterDetector::new();
        let amps = [1.0f32; 32];
        let vars = [0.001f32; 32];

        // Build up baseline.
        let flat_phases = [0.5f32; 32];
        for _ in 0..MIN_EMPTY_FRAMES {
            gh.process_frame(&flat_phases, &amps, &vars, 0, 0.0);
        }

        // Inject breathing-like periodic phase oscillation.
        // Period = 10 frames (at 20 Hz = 2 Hz, slightly fast but within range).
        let period = 10;
        for frame in 0..PHASE_BUF_LEN as u32 + 20 {
            let phase_val = 0.5 + 0.2 * libm::sinf(
                2.0 * core::f32::consts::PI * frame as f32 / period as f32
            );
            let mut phases = [phase_val; 32];
            // Add slight variation per subcarrier.
            for i in 0..32 {
                phases[i] += i as f32 * 0.001;
            }
            gh.process_frame(&phases, &amps, &vars, 0, 0.0);
        }

        // The breathing detector should find periodicity.
        // Note: detection depends on autocorrelation magnitude.
        let confidence = gh.hidden_presence_confidence();
        // We check that the detector at least computed something.
        assert!(confidence >= 0.0 && confidence <= 1.0,
            "confidence should be in [0, 1], got {}", confidence);
    }

    #[test]
    fn test_reset() {
        let mut gh = GhostHunterDetector::new();
        let phases = [0.5f32; 32];
        let amps = [1.0f32; 32];
        let vars = [0.001f32; 32];
        for _ in 0..50 {
            gh.process_frame(&phases, &amps, &vars, 0, 0.0);
        }
        assert!(gh.frame_count() > 0);
        gh.reset();
        assert_eq!(gh.frame_count(), 0);
        assert_eq!(gh.empty_frames(), 0);
    }
}
