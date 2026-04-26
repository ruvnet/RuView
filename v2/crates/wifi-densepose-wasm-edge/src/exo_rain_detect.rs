//! Rain detection from CSI micro-disturbances — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Raindrops impacting surfaces (roof, windows, walls) produce broadband
//! impulse vibrations that propagate through building structure and
//! modulate CSI phase.  These perturbations are distinguishable from
//! human motion by their:
//!
//! 1. **Broadband nature** — rain affects all subcarriers roughly equally,
//!    unlike human motion which is spatially selective.
//! 2. **Stochastic timing** — Poisson-distributed impulse arrivals, unlike
//!    the quasi-periodic patterns of walking or breathing.
//! 3. **Absence of large-scale motion** — rain perturbations are small
//!    and lack the coherent phase shifts of a moving body.
//!
//! ## Detection pipeline
//!
//! 1. Require `presence == 0` (empty room) to avoid confounding.
//! 2. Compute broadband phase variance across all subcarrier groups.
//!    If the variance is uniformly elevated (all groups above threshold),
//!    this suggests a distributed vibration source (rain).
//! 3. Estimate intensity from aggregate vibration energy:
//!    - Light: energy < 0.3
//!    - Moderate: 0.3 <= energy < 0.7
//!    - Heavy: energy >= 0.7
//! 4. Track onset (transition from quiet to rain) and cessation
//!    (transition from rain to quiet) with hysteresis.
//!
//! # Events (660-series: Exotic / Research)
//!
//! - `RAIN_ONSET` (660): 1.0 when rain begins.
//! - `RAIN_INTENSITY` (661): Intensity level (1=light, 2=moderate, 3=heavy).
//! - `RAIN_CESSATION` (662): 1.0 when rain stops.
//!
//! # Budget
//!
//! L (light, < 2 ms) — per-frame: variance comparison across 8 groups.

use crate::vendor_common::Ema;

// ── Constants ────────────────────────────────────────────────────────────────

/// Number of subcarrier groups to monitor.
const N_GROUPS: usize = 8;

/// Maximum subcarriers from host API.
const MAX_SC: usize = 32;

/// Baseline variance EWMA alpha (very slow, tracks ambient noise).
const BASELINE_ALPHA: f32 = 0.0005;

/// Short-term variance EWMA alpha (fast, tracks current conditions).
const SHORT_ALPHA: f32 = 0.05;

/// Aggregate energy EWMA alpha for intensity smoothing.
const ENERGY_ALPHA: f32 = 0.03;

/// Variance ratio threshold: current / baseline must exceed this to count
/// as "elevated" for a group.
const VARIANCE_RATIO_THRESHOLD: f32 = 2.5;

/// Minimum fraction of groups that must be elevated for broadband detection.
/// Rain should affect most groups; 6/8 = 75%.
const MIN_GROUP_FRACTION: f32 = 0.75;

/// Hysteresis: consecutive frames of rain signal before onset.
const ONSET_FRAMES: u32 = 10;

/// Hysteresis: consecutive quiet frames before cessation.
const CESSATION_FRAMES: u32 = 20;

/// Intensity thresholds (normalized energy).
const INTENSITY_LIGHT_MAX: f32 = 0.3;
const INTENSITY_MODERATE_MAX: f32 = 0.7;

/// Minimum empty-room frames before detection starts.
const MIN_EMPTY_FRAMES: u32 = 40;

// ── Event IDs (660-series: Exotic) ───────────────────────────────────────────

pub const EVENT_RAIN_ONSET: i32 = 660;
pub const EVENT_RAIN_INTENSITY: i32 = 661;
pub const EVENT_RAIN_CESSATION: i32 = 662;

// ── Rain intensity level ─────────────────────────────────────────────────────

/// Rain intensity classification.
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum RainIntensity {
    None = 0,
    Light = 1,
    Moderate = 2,
    Heavy = 3,
}

// ── Rain Detector ────────────────────────────────────────────────────────────

/// Detects rain from broadband CSI phase variance perturbations.
pub struct RainDetector {
    /// Baseline variance per subcarrier group (slow EWMA).
    baseline_var: [Ema; N_GROUPS],
    /// Short-term variance per subcarrier group (fast EWMA).
    short_var: [Ema; N_GROUPS],
    /// Smoothed aggregate vibration energy.
    energy_ema: Ema,
    /// Current rain state.
    raining: bool,
    /// Current intensity classification.
    intensity: RainIntensity,
    /// Consecutive frames of broadband variance elevation.
    rain_frames: u32,
    /// Consecutive frames without broadband variance elevation.
    quiet_frames: u32,
    /// Number of empty-room frames processed.
    empty_frames: u32,
    /// Total frames processed.
    frame_count: u32,
}

impl RainDetector {
    pub const fn new() -> Self {
        Self {
            baseline_var: [
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
                Ema::new(BASELINE_ALPHA), Ema::new(BASELINE_ALPHA),
            ],
            short_var: [
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
                Ema::new(SHORT_ALPHA), Ema::new(SHORT_ALPHA),
            ],
            energy_ema: Ema::new(ENERGY_ALPHA),
            raining: false,
            intensity: RainIntensity::None,
            rain_frames: 0,
            quiet_frames: 0,
            empty_frames: 0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases` — per-subcarrier phase values (up to 32).
    /// `variance` — per-subcarrier variance values (up to 32).
    /// `amplitudes` — per-subcarrier amplitude values (up to 32).
    /// `presence` — 0 = room empty, >0 = humans present.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        variance: &[f32],
        amplitudes: &[f32],
        presence: i32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        // Only detect when room is empty.
        if presence != 0 {
            return &[];
        }

        let n_sc = core::cmp::min(phases.len(), MAX_SC);
        let n_sc = core::cmp::min(n_sc, variance.len());
        let n_sc = core::cmp::min(n_sc, amplitudes.len());
        if n_sc < N_GROUPS {
            return &[];
        }

        self.empty_frames += 1;

        // Compute per-group variance.
        let subs_per = n_sc / N_GROUPS;
        if subs_per == 0 {
            return &[];
        }

        let mut group_var = [0.0f32; N_GROUPS];
        for g in 0..N_GROUPS {
            let start = g * subs_per;
            let end = if g == N_GROUPS - 1 { n_sc } else { start + subs_per };
            let count = (end - start) as f32;
            let mut sv = 0.0f32;
            for i in start..end {
                sv += variance[i];
            }
            group_var[g] = sv / count;
        }

        // Update baselines and short-term estimates.
        let mut elevated_count = 0u32;
        let mut total_energy = 0.0f32;
        for g in 0..N_GROUPS {
            self.baseline_var[g].update(group_var[g]);
            self.short_var[g].update(group_var[g]);

            let baseline = self.baseline_var[g].value;
            let short = self.short_var[g].value;

            // Check if this group has elevated variance.
            if baseline > 1e-10 && short > baseline * VARIANCE_RATIO_THRESHOLD {
                elevated_count += 1;
            }

            // Accumulate energy as excess above baseline.
            if baseline > 1e-10 {
                let excess = if short > baseline {
                    (short - baseline) / baseline
                } else {
                    0.0
                };
                total_energy += excess;
            }
        }

        // Normalize energy to [0, 1] (cap at 1.0).
        let avg_energy = total_energy / N_GROUPS as f32;
        let norm_energy = if avg_energy > 1.0 { 1.0 } else { avg_energy };
        self.energy_ema.update(norm_energy);

        // Need minimum data before detection.
        if self.empty_frames < MIN_EMPTY_FRAMES {
            return &[];
        }

        // Check broadband criterion: most groups must be elevated.
        let fraction = elevated_count as f32 / N_GROUPS as f32;
        let broadband = fraction >= MIN_GROUP_FRACTION;

        // Update state machine with hysteresis.
        if broadband {
            self.rain_frames += 1;
            self.quiet_frames = 0;
        } else {
            self.quiet_frames += 1;
            self.rain_frames = 0;
        }

        let was_raining = self.raining;

        // Onset: was not raining, now have enough consecutive rain frames.
        if !self.raining && self.rain_frames >= ONSET_FRAMES {
            self.raining = true;
            unsafe {
                EVENTS[n_ev] = (EVENT_RAIN_ONSET, 1.0);
            }
            n_ev += 1;
        }

        // Cessation: was raining, now have enough quiet frames.
        if was_raining && self.quiet_frames >= CESSATION_FRAMES {
            self.raining = false;
            self.intensity = RainIntensity::None;
            unsafe {
                EVENTS[n_ev] = (EVENT_RAIN_CESSATION, 1.0);
            }
            n_ev += 1;
        }

        // Classify intensity while raining.
        if self.raining {
            let energy = self.energy_ema.value;
            self.intensity = if energy < INTENSITY_LIGHT_MAX {
                RainIntensity::Light
            } else if energy < INTENSITY_MODERATE_MAX {
                RainIntensity::Moderate
            } else {
                RainIntensity::Heavy
            };

            unsafe {
                EVENTS[n_ev] = (EVENT_RAIN_INTENSITY, self.intensity as u8 as f32);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Whether rain is currently detected.
    pub fn is_raining(&self) -> bool {
        self.raining
    }

    /// Get the current rain intensity.
    pub fn intensity(&self) -> RainIntensity {
        self.intensity
    }

    /// Get the smoothed vibration energy [0, 1].
    pub fn energy(&self) -> f32 {
        self.energy_ema.value
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
        let rd = RainDetector::new();
        assert_eq!(rd.frame_count(), 0);
        assert_eq!(rd.empty_frames(), 0);
        assert!(!rd.is_raining());
        assert_eq!(rd.intensity() as u8, RainIntensity::None as u8);
    }

    #[test]
    fn test_presence_blocks_detection() {
        let mut rd = RainDetector::new();
        let phases = [0.5f32; 32];
        let vars = [1.0f32; 32]; // high variance
        let amps = [1.0f32; 32];
        for _ in 0..100 {
            let events = rd.process_frame(&phases, &vars, &amps, 1); // present
            assert!(events.is_empty());
        }
        assert_eq!(rd.empty_frames(), 0);
    }

    #[test]
    fn test_quiet_room_no_rain() {
        let mut rd = RainDetector::new();
        let phases = [0.5f32; 32];
        let vars = [0.001f32; 32]; // very low variance
        let amps = [1.0f32; 32];
        for _ in 0..MIN_EMPTY_FRAMES + 50 {
            let events = rd.process_frame(&phases, &vars, &amps, 0);
            for ev in events {
                assert_ne!(ev.0, EVENT_RAIN_ONSET,
                    "quiet room should not trigger rain onset");
            }
        }
        assert!(!rd.is_raining());
    }

    #[test]
    fn test_broadband_variance_triggers_rain() {
        let mut rd = RainDetector::new();
        let phases = [0.5f32; 32];
        let amps = [1.0f32; 32];
        let low_vars = [0.001f32; 32];

        // Build baseline with low variance.
        for _ in 0..MIN_EMPTY_FRAMES + 50 {
            rd.process_frame(&phases, &low_vars, &amps, 0);
        }

        // Inject broadband high variance (rain-like).
        let high_vars = [0.5f32; 32];
        let mut onset_seen = false;
        for _ in 0..ONSET_FRAMES + 20 {
            let events = rd.process_frame(&phases, &high_vars, &amps, 0);
            for ev in events {
                if ev.0 == EVENT_RAIN_ONSET {
                    onset_seen = true;
                }
            }
        }
        assert!(onset_seen, "broadband variance elevation should trigger rain onset");
        assert!(rd.is_raining());
    }

    #[test]
    fn test_rain_cessation() {
        let mut rd = RainDetector::new();
        let phases = [0.5f32; 32];
        let amps = [1.0f32; 32];
        let low_vars = [0.001f32; 32];
        let high_vars = [0.5f32; 32];

        // Build baseline then start rain.
        for _ in 0..MIN_EMPTY_FRAMES + 50 {
            rd.process_frame(&phases, &low_vars, &amps, 0);
        }
        for _ in 0..ONSET_FRAMES + 10 {
            rd.process_frame(&phases, &high_vars, &amps, 0);
        }
        assert!(rd.is_raining());

        // Return to quiet — the short-term EWMA needs time to decay
        // below the baseline before the broadband criterion fails.
        // With SHORT_ALPHA=0.05, the EWMA half-life is ~14 frames,
        // so we need ~50+ quiet frames before the short-term drops
        // below 2.5x baseline, then CESSATION_FRAMES more to confirm.
        let mut cessation_seen = false;
        for _ in 0..200 {
            let events = rd.process_frame(&phases, &low_vars, &amps, 0);
            for ev in events {
                if ev.0 == EVENT_RAIN_CESSATION {
                    cessation_seen = true;
                }
            }
        }
        assert!(cessation_seen, "return to quiet should trigger rain cessation");
        assert!(!rd.is_raining());
    }

    #[test]
    fn test_intensity_levels() {
        assert_eq!(RainIntensity::None as u8, 0);
        assert_eq!(RainIntensity::Light as u8, 1);
        assert_eq!(RainIntensity::Moderate as u8, 2);
        assert_eq!(RainIntensity::Heavy as u8, 3);
    }

    #[test]
    fn test_insufficient_subcarriers() {
        let mut rd = RainDetector::new();
        let small = [1.0f32; 4];
        let events = rd.process_frame(&small, &small, &small, 0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_reset() {
        let mut rd = RainDetector::new();
        let phases = [0.5f32; 32];
        let vars = [0.001f32; 32];
        let amps = [1.0f32; 32];
        for _ in 0..50 {
            rd.process_frame(&phases, &vars, &amps, 0);
        }
        assert!(rd.frame_count() > 0);
        rd.reset();
        assert_eq!(rd.frame_count(), 0);
        assert!(!rd.is_raining());
    }
}
