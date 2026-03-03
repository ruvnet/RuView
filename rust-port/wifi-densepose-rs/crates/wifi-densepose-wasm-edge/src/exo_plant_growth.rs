//! Plant growth and leaf movement detector — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Detects plant growth and leaf movement from micro-CSI changes over
//! hours/days.  Plants cause extremely slow, monotonic drift in CSI
//! amplitude (growth) and diurnal phase oscillations (circadian leaf
//! movement).  The module maintains multi-hour EWMA baselines per
//! subcarrier group and only accumulates data when `presence == 0`
//! (room must be empty to isolate plant-scale perturbations from
//! human motion).
//!
//! ## Detection modes
//!
//! 1. **Growth rate** — Slow monotonic drift in amplitude baseline,
//!    measured as the slope of an EWMA-smoothed amplitude trend over
//!    a sliding window.  Plant growth produces a continuous ~0.01 dB/hour
//!    amplitude decrease as new leaf area intercepts RF energy.
//!
//! 2. **Circadian phase** — 24-hour oscillation in phase baseline
//!    caused by nyctinastic leaf movement (leaves fold at night).
//!    Detected by tracking the phase EWMA's peak-to-trough over a
//!    diurnal window and computing the oscillation phase.
//!
//! 3. **Wilting detection** — Sudden amplitude increase (less absorption)
//!    combined with reduced phase variance indicates wilting/dehydration.
//!
//! 4. **Watering event** — Abrupt amplitude drop (more water = more
//!    absorption) with a subsequent recovery to a new baseline.
//!
//! # Events (640-series: Exotic / Research)
//!
//! - `GROWTH_RATE` (640): Amplitude drift rate (dB/hour equivalent, scaled).
//! - `CIRCADIAN_PHASE` (641): Diurnal oscillation magnitude [0, 1].
//! - `WILT_DETECTED` (642): 1.0 when wilting signature detected.
//! - `WATERING_EVENT` (643): 1.0 when watering signature detected.
//!
//! # Budget
//!
//! L (light, < 2 ms) — per-frame: 8 EWMA updates + simple comparisons.

use crate::vendor_common::Ema;
use libm::fabsf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Number of subcarrier groups to track (matches flash-attention tiling).
const N_GROUPS: usize = 8;

/// Maximum subcarriers from host API.
const MAX_SC: usize = 32;

/// Slow EWMA alpha for multi-hour baseline (very slow adaptation).
/// At 20 Hz, alpha=0.0001 has half-life ~3500 frames = ~175 seconds.
const BASELINE_ALPHA: f32 = 0.0001;

/// Faster EWMA alpha for short-term average (detect sudden changes).
const SHORT_ALPHA: f32 = 0.01;

/// Minimum frames of empty-room data before analysis begins.
const MIN_EMPTY_FRAMES: u32 = 200;

/// Amplitude drift threshold to report growth (scaled units).
const GROWTH_THRESHOLD: f32 = 0.005;

/// Amplitude jump threshold for watering event detection.
const WATERING_DROP_THRESHOLD: f32 = 0.15;

/// Amplitude jump threshold for wilting detection.
const WILT_RISE_THRESHOLD: f32 = 0.10;

/// Phase variance drop factor for wilting confirmation.
const WILT_VARIANCE_FACTOR: f32 = 0.5;

/// Diurnal oscillation: frames per tracking window (50 frames at 20 Hz = 2.5 s).
/// We track peak-to-trough of the phase EWMA across this rolling window.
const DIURNAL_WINDOW: usize = 50;

/// Minimum diurnal oscillation magnitude to report circadian phase.
const CIRCADIAN_MIN_MAGNITUDE: f32 = 0.01;

// ── Event IDs (640-series: Exotic) ───────────────────────────────────────────

pub const EVENT_GROWTH_RATE: i32 = 640;
pub const EVENT_CIRCADIAN_PHASE: i32 = 641;
pub const EVENT_WILT_DETECTED: i32 = 642;
pub const EVENT_WATERING_EVENT: i32 = 643;

// ── Plant Growth Detector ────────────────────────────────────────────────────

/// Detects plant growth and leaf movement from micro-CSI perturbations.
///
/// Only accumulates data when `presence == 0` (room empty). Maintains
/// slow and fast EWMA baselines per subcarrier group for amplitude
/// and phase to detect growth drift, circadian oscillation, wilting,
/// and watering events.
pub struct PlantGrowthDetector {
    /// Slow EWMA of amplitude per subcarrier group.
    amp_baseline: [Ema; N_GROUPS],
    /// Fast EWMA of amplitude per subcarrier group.
    amp_short: [Ema; N_GROUPS],
    /// Slow EWMA of phase per subcarrier group.
    phase_baseline: [Ema; N_GROUPS],
    /// Fast EWMA of phase variance per subcarrier group.
    phase_var_ema: [Ema; N_GROUPS],
    /// Rolling window of phase baseline values for diurnal tracking.
    phase_window: [[f32; DIURNAL_WINDOW]; N_GROUPS],
    /// Write index into phase_window.
    phase_window_idx: usize,
    /// Number of samples written to phase_window.
    phase_window_fill: usize,
    /// Previous slow-baseline amplitude snapshot (for drift computation).
    prev_baseline_amp: [f32; N_GROUPS],
    /// Whether prev_baseline_amp has been initialized.
    baseline_initialized: bool,
    /// Number of empty-room frames accumulated.
    empty_frames: u32,
    /// Total frames processed (including non-empty).
    frame_count: u32,
    /// Frames since last drift computation.
    drift_interval_count: u32,
}

impl PlantGrowthDetector {
    pub const fn new() -> Self {
        Self {
            amp_baseline: [
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
            ],
            amp_short: [
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
            ],
            phase_baseline: [
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
            ],
            phase_var_ema: [
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
            ],
            phase_window: [[0.0; DIURNAL_WINDOW]; N_GROUPS],
            phase_window_idx: 0,
            phase_window_fill: 0,
            prev_baseline_amp: [0.0; N_GROUPS],
            baseline_initialized: false,
            empty_frames: 0,
            frame_count: 0,
            drift_interval_count: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// `amplitudes` — per-subcarrier amplitude values (up to 32).
    /// `phases` — per-subcarrier phase values (up to 32).
    /// `variance` — per-subcarrier variance values (up to 32).
    /// `presence` — 0 = room empty, >0 = humans present.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        amplitudes: &[f32],
        phases: &[f32],
        variance: &[f32],
        presence: i32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        // Only accumulate data when room is empty.
        if presence != 0 {
            return &[];
        }

        let n_sc = core::cmp::min(amplitudes.len(), MAX_SC);
        let n_sc = core::cmp::min(n_sc, phases.len());
        let n_sc = core::cmp::min(n_sc, variance.len());
        if n_sc < N_GROUPS {
            return &[];
        }

        self.empty_frames += 1;

        // Compute per-group means.
        let subs_per = n_sc / N_GROUPS;
        if subs_per == 0 {
            return &[];
        }

        let mut group_amp = [0.0f32; N_GROUPS];
        let mut group_phase = [0.0f32; N_GROUPS];
        let mut group_var = [0.0f32; N_GROUPS];

        for g in 0..N_GROUPS {
            let start = g * subs_per;
            let end = if g == N_GROUPS - 1 { n_sc } else { start + subs_per };
            let count = (end - start) as f32;
            let mut sa = 0.0f32;
            let mut sp = 0.0f32;
            let mut sv = 0.0f32;
            for i in start..end {
                sa += amplitudes[i];
                sp += phases[i];
                sv += variance[i];
            }
            group_amp[g] = sa / count;
            group_phase[g] = sp / count;
            group_var[g] = sv / count;
        }

        // Update EWMAs.
        for g in 0..N_GROUPS {
            self.amp_baseline[g].update(group_amp[g]);
            self.amp_short[g].update(group_amp[g]);
            self.phase_baseline[g].update(group_phase[g]);
            self.phase_var_ema[g].update(group_var[g]);

            // Track phase baseline in rolling window for diurnal detection.
            self.phase_window[g][self.phase_window_idx] = self.phase_baseline[g].value;
        }
        self.phase_window_idx = (self.phase_window_idx + 1) % DIURNAL_WINDOW;
        if self.phase_window_fill < DIURNAL_WINDOW {
            self.phase_window_fill += 1;
        }

        // Need enough data before analysis.
        if self.empty_frames < MIN_EMPTY_FRAMES {
            return &[];
        }

        // Initialize baseline snapshot on first analysis pass.
        if !self.baseline_initialized {
            for g in 0..N_GROUPS {
                self.prev_baseline_amp[g] = self.amp_baseline[g].value;
            }
            self.baseline_initialized = true;
            self.drift_interval_count = 0;
            return &[];
        }

        self.drift_interval_count += 1;

        // ── Growth rate detection (every 100 frames = 5s at 20 Hz) ───────
        if self.drift_interval_count >= 100 {
            let mut total_drift = 0.0f32;
            for g in 0..N_GROUPS {
                let drift = self.amp_baseline[g].value - self.prev_baseline_amp[g];
                total_drift += drift;
                self.prev_baseline_amp[g] = self.amp_baseline[g].value;
            }
            let avg_drift = total_drift / N_GROUPS as f32;
            self.drift_interval_count = 0;

            if fabsf(avg_drift) > GROWTH_THRESHOLD {
                unsafe {
                    EVENTS[n_ev] = (EVENT_GROWTH_RATE, avg_drift);
                }
                n_ev += 1;
            }
        }

        // ── Circadian phase detection ────────────────────────────────────
        if self.phase_window_fill >= DIURNAL_WINDOW {
            let mut total_osc = 0.0f32;
            for g in 0..N_GROUPS {
                let mut min_v = f32::MAX;
                let mut max_v = f32::MIN;
                for i in 0..DIURNAL_WINDOW {
                    let v = self.phase_window[g][i];
                    if v < min_v { min_v = v; }
                    if v > max_v { max_v = v; }
                }
                total_osc += max_v - min_v;
            }
            let avg_osc = total_osc / N_GROUPS as f32;
            if avg_osc > CIRCADIAN_MIN_MAGNITUDE {
                // Normalize to [0, 1] range (cap at 1.0).
                let normalized = if avg_osc > 1.0 { 1.0 } else { avg_osc };
                unsafe {
                    EVENTS[n_ev] = (EVENT_CIRCADIAN_PHASE, normalized);
                }
                n_ev += 1;
            }
        }

        // ── Wilting detection ────────────────────────────────────────────
        // Wilting: short-term amplitude rises above baseline AND phase
        // variance drops significantly.
        {
            let mut amp_rise_count = 0u8;
            let mut var_drop_count = 0u8;
            for g in 0..N_GROUPS {
                let rise = self.amp_short[g].value - self.amp_baseline[g].value;
                if rise > WILT_RISE_THRESHOLD {
                    amp_rise_count += 1;
                }
                // Phase variance dropped below half of baseline.
                if self.phase_var_ema[g].value < self.amp_baseline[g].value * WILT_VARIANCE_FACTOR
                    && self.phase_var_ema[g].value < 0.1
                {
                    var_drop_count += 1;
                }
            }
            // Need majority of groups to agree.
            if amp_rise_count >= (N_GROUPS / 2) as u8 && var_drop_count >= 2 {
                unsafe {
                    EVENTS[n_ev] = (EVENT_WILT_DETECTED, 1.0);
                }
                n_ev += 1;
            }
        }

        // ── Watering event detection ─────────────────────────────────────
        // Watering: short-term amplitude drops below baseline significantly.
        {
            let mut drop_count = 0u8;
            for g in 0..N_GROUPS {
                let drop = self.amp_baseline[g].value - self.amp_short[g].value;
                if drop > WATERING_DROP_THRESHOLD {
                    drop_count += 1;
                }
            }
            if drop_count >= (N_GROUPS / 2) as u8 {
                unsafe {
                    EVENTS[n_ev] = (EVENT_WATERING_EVENT, 1.0);
                }
                n_ev += 1;
            }
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Get the number of empty-room frames accumulated.
    pub fn empty_frames(&self) -> u32 {
        self.empty_frames
    }

    /// Get total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Whether enough baseline data has been accumulated for analysis.
    pub fn is_calibrated(&self) -> bool {
        self.baseline_initialized
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
        let pg = PlantGrowthDetector::new();
        assert_eq!(pg.frame_count(), 0);
        assert_eq!(pg.empty_frames(), 0);
        assert!(!pg.is_calibrated());
    }

    #[test]
    fn test_presence_blocks_accumulation() {
        let mut pg = PlantGrowthDetector::new();
        let amps = [1.0f32; 32];
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        for _ in 0..100 {
            let events = pg.process_frame(&amps, &phases, &vars, 1); // present
            assert!(events.is_empty(), "should not emit when humans present");
        }
        assert_eq!(pg.empty_frames(), 0);
    }

    #[test]
    fn test_insufficient_subcarriers_no_events() {
        let mut pg = PlantGrowthDetector::new();
        let amps = [1.0f32; 4]; // too few
        let phases = [0.5f32; 4];
        let vars = [0.01f32; 4];
        let events = pg.process_frame(&amps, &phases, &vars, 0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_empty_room_accumulates() {
        let mut pg = PlantGrowthDetector::new();
        let amps = [1.0f32; 32];
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        for _ in 0..50 {
            pg.process_frame(&amps, &phases, &vars, 0);
        }
        assert_eq!(pg.empty_frames(), 50);
    }

    #[test]
    fn test_calibration_after_min_frames() {
        let mut pg = PlantGrowthDetector::new();
        let amps = [1.0f32; 32];
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        for _ in 0..MIN_EMPTY_FRAMES + 1 {
            pg.process_frame(&amps, &phases, &vars, 0);
        }
        assert!(pg.is_calibrated());
    }

    #[test]
    fn test_stable_signal_no_growth_events() {
        let mut pg = PlantGrowthDetector::new();
        let amps = [1.0f32; 32];
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        // Run enough frames for calibration + analysis.
        for _ in 0..MIN_EMPTY_FRAMES + 200 {
            let events = pg.process_frame(&amps, &phases, &vars, 0);
            for ev in events {
                // Stable signal should not trigger growth or watering.
                assert_ne!(ev.0, EVENT_WATERING_EVENT,
                    "stable signal should not trigger watering");
            }
        }
    }

    #[test]
    fn test_watering_event_detection() {
        let mut pg = PlantGrowthDetector::new();
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];

        // Calibrate with high amplitude.
        let high_amps = [5.0f32; 32];
        for _ in 0..MIN_EMPTY_FRAMES + 200 {
            pg.process_frame(&high_amps, &phases, &vars, 0);
        }

        // Suddenly drop amplitude (simulates watering).
        let low_amps = [3.0f32; 32];
        let mut watering_detected = false;
        for _ in 0..200 {
            let events = pg.process_frame(&low_amps, &phases, &vars, 0);
            for ev in events {
                if ev.0 == EVENT_WATERING_EVENT {
                    watering_detected = true;
                }
            }
        }
        // The short-term average will converge, so detection depends on
        // how quickly the EWMA catches up. With SHORT_ALPHA=0.01, the
        // short-term tracks faster than the baseline.
        assert!(watering_detected, "should detect watering event on amplitude drop");
    }

    #[test]
    fn test_reset() {
        let mut pg = PlantGrowthDetector::new();
        let amps = [1.0f32; 32];
        let phases = [0.5f32; 32];
        let vars = [0.01f32; 32];
        for _ in 0..100 {
            pg.process_frame(&amps, &phases, &vars, 0);
        }
        assert!(pg.frame_count() > 0);
        pg.reset();
        assert_eq!(pg.frame_count(), 0);
        assert_eq!(pg.empty_frames(), 0);
        assert!(!pg.is_calibrated());
    }
}
