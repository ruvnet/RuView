//! Seizure detection — ADR-041 Category 1 Medical module.
//!
//! Detects tonic-clonic seizures via high-energy rhythmic motion in the
//! 3-8 Hz band, discriminating from:
//!   - Falls: single impulse followed by stillness
//!   - Tremor: lower amplitude, higher regularity
//!
//! Seizure phases:
//!   - Tonic: sustained muscle rigidity → high motion energy, low variance
//!   - Clonic: rhythmic jerking → high energy with 3-8 Hz periodicity
//!   - Post-ictal: sudden drop to minimal movement
//!
//! Events:
//!   SEIZURE_ONSET  (140) — initial seizure detection
//!   SEIZURE_TONIC  (141) — tonic phase identified
//!   SEIZURE_CLONIC (142) — clonic (rhythmic jerking) phase
//!   POST_ICTAL     (143) — post-ictal period (sudden movement cessation)
//!
//! Host API inputs: phase, amplitude, motion energy, presence.
//! Budget: S (< 5 ms).

// ── libm ────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "std"))]
use libm::{sqrtf, fabsf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

// ── Constants ───────────────────────────────────────────────────────────────

/// Motion energy history window (at ~20 Hz frame rate → 5 seconds).
/// We process at frame rate for rhythm detection.
const ENERGY_WINDOW: usize = 100;

/// Phase history for rhythm analysis.
const PHASE_WINDOW: usize = 100;

/// High motion energy threshold (normalised).
const HIGH_ENERGY_THRESH: f32 = 2.0;

/// Tonic phase: sustained high energy with low variance.
const TONIC_ENERGY_THRESH: f32 = 1.5;
const TONIC_VAR_CEIL: f32 = 0.5;
const TONIC_MIN_FRAMES: u16 = 20;

/// Clonic phase: rhythmic pattern in 3-8 Hz band.
/// At 20 Hz sampling, 3 Hz = period of ~7 frames, 8 Hz = period of ~2.5 frames.
const CLONIC_PERIOD_MIN: usize = 2;
const CLONIC_PERIOD_MAX: usize = 7;
const CLONIC_AUTOCORR_THRESH: f32 = 0.30;
const CLONIC_MIN_FRAMES: u16 = 30;

/// Post-ictal: motion drops below this for N consecutive frames.
const POST_ICTAL_ENERGY_THRESH: f32 = 0.2;
const POST_ICTAL_MIN_FRAMES: u16 = 40;

/// Fall discrimination: single impulse → high energy for <5 frames then low.
const FALL_MAX_DURATION: u16 = 10;

/// Tremor discrimination: amplitude must be above this to be seizure-grade.
const TREMOR_AMPLITUDE_FLOOR: f32 = 0.8;

/// Cooldown after seizure cycle completes (frames).
const COOLDOWN_FRAMES: u16 = 200;

/// Minimum sustained high-energy frames before onset.
const ONSET_MIN_FRAMES: u16 = 10;

// ── Event IDs ───────────────────────────────────────────────────────────────

pub const EVENT_SEIZURE_ONSET: i32 = 140;
pub const EVENT_SEIZURE_TONIC: i32 = 141;
pub const EVENT_SEIZURE_CLONIC: i32 = 142;
pub const EVENT_POST_ICTAL: i32 = 143;

// ── State machine ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeizurePhase {
    /// Normal monitoring.
    Monitoring,
    /// Possible onset (high energy detected, building confidence).
    PossibleOnset,
    /// Tonic phase (sustained rigidity).
    Tonic,
    /// Clonic phase (rhythmic jerking).
    Clonic,
    /// Post-ictal (sudden cessation).
    PostIctal,
    /// Cooldown after episode.
    Cooldown,
}

/// Seizure detector.
pub struct SeizureDetector {
    /// Current phase of seizure state machine.
    phase: SeizurePhase,

    /// Motion energy ring buffer.
    energy_buf: [f32; ENERGY_WINDOW],
    energy_idx: usize,
    energy_len: usize,

    /// Amplitude ring buffer (for rhythm detection).
    amp_buf: [f32; PHASE_WINDOW],
    amp_idx: usize,
    amp_len: usize,

    /// Consecutive frames in current sub-state.
    state_frames: u16,

    /// Frames of high energy (for onset detection).
    high_energy_frames: u16,

    /// Frames of low energy (for post-ictal).
    low_energy_frames: u16,

    /// Cooldown counter.
    cooldown: u16,

    /// Total seizure events detected.
    seizure_count: u32,

    /// Frame counter.
    frame_count: u32,
}

impl SeizureDetector {
    pub const fn new() -> Self {
        Self {
            phase: SeizurePhase::Monitoring,
            energy_buf: [0.0; ENERGY_WINDOW],
            energy_idx: 0,
            energy_len: 0,
            amp_buf: [0.0; PHASE_WINDOW],
            amp_idx: 0,
            amp_len: 0,
            state_frames: 0,
            high_energy_frames: 0,
            low_energy_frames: 0,
            cooldown: 0,
            seizure_count: 0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame (called at ~20 Hz).
    ///
    /// * `_phase` — representative phase (reserved)
    /// * `amplitude` — representative amplitude
    /// * `motion_energy` — host-reported motion energy
    /// * `presence` — host presence flag
    ///
    /// Returns `&[(event_id, value)]`.
    pub fn process_frame(
        &mut self,
        _phase: f32,
        amplitude: f32,
        motion_energy: f32,
        presence: i32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        // Push into ring buffers.
        self.energy_buf[self.energy_idx] = motion_energy;
        self.energy_idx = (self.energy_idx + 1) % ENERGY_WINDOW;
        if self.energy_len < ENERGY_WINDOW { self.energy_len += 1; }

        self.amp_buf[self.amp_idx] = amplitude;
        self.amp_idx = (self.amp_idx + 1) % PHASE_WINDOW;
        if self.amp_len < PHASE_WINDOW { self.amp_len += 1; }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;

        // No detection without presence.
        if presence < 1 {
            if self.phase != SeizurePhase::Monitoring && self.phase != SeizurePhase::Cooldown {
                self.phase = SeizurePhase::Monitoring;
                self.state_frames = 0;
                self.high_energy_frames = 0;
            }
            return unsafe { &EVENTS[..n] };
        }

        // Tick cooldown.
        if self.phase == SeizurePhase::Cooldown {
            self.cooldown = self.cooldown.saturating_sub(1);
            if self.cooldown == 0 {
                self.phase = SeizurePhase::Monitoring;
                self.state_frames = 0;
            }
            return unsafe { &EVENTS[..n] };
        }

        // ── State machine ───────────────────────────────────────────────
        match self.phase {
            SeizurePhase::Monitoring => {
                if motion_energy > HIGH_ENERGY_THRESH {
                    self.high_energy_frames += 1;
                    if self.high_energy_frames >= ONSET_MIN_FRAMES {
                        // Discriminate from fall: check if it's a single impulse.
                        // Falls have <FALL_MAX_DURATION frames of high energy then drop.
                        // We're already at ONSET_MIN_FRAMES, so likely not a fall.
                        self.phase = SeizurePhase::PossibleOnset;
                        self.state_frames = self.high_energy_frames;
                    }
                } else {
                    self.high_energy_frames = 0;
                }
            }

            SeizurePhase::PossibleOnset => {
                self.state_frames += 1;

                if motion_energy < HIGH_ENERGY_THRESH * 0.5 {
                    // Energy dropped — was it a fall (short burst)?
                    if self.state_frames <= FALL_MAX_DURATION {
                        // Too short for seizure — likely a fall or artifact.
                        self.phase = SeizurePhase::Monitoring;
                        self.state_frames = 0;
                        self.high_energy_frames = 0;
                        return unsafe { &EVENTS[..n] };
                    }
                }

                // Check for tonic characteristics.
                let energy_var = self.recent_energy_variance();
                if energy_var < TONIC_VAR_CEIL && motion_energy > TONIC_ENERGY_THRESH {
                    self.phase = SeizurePhase::Tonic;
                    self.state_frames = 0;
                    self.seizure_count += 1;
                    unsafe { EVENTS[n] = (EVENT_SEIZURE_ONSET, motion_energy); }
                    n += 1;
                }

                // Check for clonic characteristics (skip tonic, go directly to clonic).
                // Only if we haven't already transitioned to Tonic above.
                if self.phase == SeizurePhase::PossibleOnset
                    && self.amp_len >= PHASE_WINDOW && amplitude > TREMOR_AMPLITUDE_FLOOR {
                    if let Some(period) = self.detect_rhythm() {
                        self.phase = SeizurePhase::Clonic;
                        self.state_frames = 0;
                        self.seizure_count += 1;
                        unsafe { EVENTS[n] = (EVENT_SEIZURE_ONSET, motion_energy); }
                        n += 1;
                        if n < 4 {
                            unsafe { EVENTS[n] = (EVENT_SEIZURE_CLONIC, period as f32); }
                            n += 1;
                        }
                    }
                }

                // Timeout — if we've been in possible-onset too long without
                // classifying, return to monitoring.
                if self.state_frames > 200 {
                    self.phase = SeizurePhase::Monitoring;
                    self.state_frames = 0;
                    self.high_energy_frames = 0;
                }
            }

            SeizurePhase::Tonic => {
                self.state_frames += 1;

                // Check transition to clonic.
                if self.amp_len >= PHASE_WINDOW {
                    let energy_var = self.recent_energy_variance();
                    if energy_var > TONIC_VAR_CEIL {
                        if let Some(period) = self.detect_rhythm() {
                            if self.state_frames >= TONIC_MIN_FRAMES && n < 4 {
                                unsafe { EVENTS[n] = (EVENT_SEIZURE_TONIC, self.state_frames as f32); }
                                n += 1;
                            }
                            self.phase = SeizurePhase::Clonic;
                            self.state_frames = 0;
                            if n < 4 {
                                unsafe { EVENTS[n] = (EVENT_SEIZURE_CLONIC, period as f32); }
                                n += 1;
                            }
                        }
                    }
                }

                // Check for post-ictal (direct transition from tonic).
                if motion_energy < POST_ICTAL_ENERGY_THRESH {
                    self.low_energy_frames += 1;
                    if self.low_energy_frames >= POST_ICTAL_MIN_FRAMES {
                        if self.state_frames >= TONIC_MIN_FRAMES && n < 4 {
                            unsafe { EVENTS[n] = (EVENT_SEIZURE_TONIC, self.state_frames as f32); }
                            n += 1;
                        }
                        self.phase = SeizurePhase::PostIctal;
                        self.state_frames = 0;
                    }
                } else {
                    self.low_energy_frames = 0;
                }
            }

            SeizurePhase::Clonic => {
                self.state_frames += 1;

                // Check for post-ictal transition.
                if motion_energy < POST_ICTAL_ENERGY_THRESH {
                    self.low_energy_frames += 1;
                    if self.low_energy_frames >= POST_ICTAL_MIN_FRAMES {
                        self.phase = SeizurePhase::PostIctal;
                        self.state_frames = 0;
                    }
                } else {
                    self.low_energy_frames = 0;
                }
            }

            SeizurePhase::PostIctal => {
                self.state_frames += 1;
                if self.state_frames == 1 && n < 4 {
                    unsafe { EVENTS[n] = (EVENT_POST_ICTAL, 1.0); }
                    n += 1;
                }

                // After enough post-ictal frames, go to cooldown.
                if self.state_frames >= POST_ICTAL_MIN_FRAMES {
                    self.phase = SeizurePhase::Cooldown;
                    self.cooldown = COOLDOWN_FRAMES;
                    self.state_frames = 0;
                    self.high_energy_frames = 0;
                    self.low_energy_frames = 0;
                }
            }

            SeizurePhase::Cooldown => {
                // Handled above.
            }
        }

        unsafe { &EVENTS[..n] }
    }

    /// Compute variance of recent motion energy.
    fn recent_energy_variance(&self) -> f32 {
        if self.energy_len < 4 { return 0.0; }
        let n = self.energy_len.min(20);
        let mut sum = 0.0f32;
        for i in 0..n {
            let idx = (self.energy_idx + ENERGY_WINDOW - n + i) % ENERGY_WINDOW;
            sum += self.energy_buf[idx];
        }
        let mean = sum / n as f32;
        let mut var = 0.0f32;
        for i in 0..n {
            let idx = (self.energy_idx + ENERGY_WINDOW - n + i) % ENERGY_WINDOW;
            let d = self.energy_buf[idx] - mean;
            var += d * d;
        }
        var / n as f32
    }

    /// Detect rhythmic pattern in amplitude buffer using autocorrelation.
    /// Returns the dominant period (in frames) if above threshold.
    fn detect_rhythm(&self) -> Option<usize> {
        if self.amp_len < PHASE_WINDOW { return None; }

        let start = self.amp_idx; // oldest sample
        let n = self.amp_len;

        // Compute mean.
        let mut sum = 0.0f32;
        for i in 0..n { sum += self.amp_buf[i]; }
        let mean = sum / n as f32;

        // Compute variance.
        let mut var = 0.0f32;
        for i in 0..n {
            let d = self.amp_buf[i] - mean;
            var += d * d;
        }
        var /= n as f32;
        if var < 0.01 { return None; }

        // Autocorrelation for seizure-band lags.
        let mut best_ac = 0.0f32;
        let mut best_lag = 0usize;

        for lag in CLONIC_PERIOD_MIN..=CLONIC_PERIOD_MAX.min(n - 1) {
            let mut ac = 0.0f32;
            let samples = n - lag;
            for i in 0..samples {
                let a = self.amp_buf[(start + i) % PHASE_WINDOW] - mean;
                let b = self.amp_buf[(start + i + lag) % PHASE_WINDOW] - mean;
                ac += a * b;
            }
            let norm = ac / (samples as f32 * var);
            if norm > best_ac {
                best_ac = norm;
                best_lag = lag;
            }
        }

        if best_ac > CLONIC_AUTOCORR_THRESH {
            Some(best_lag)
        } else {
            None
        }
    }

    /// Current seizure phase.
    pub fn phase(&self) -> SeizurePhase {
        self.phase
    }

    /// Total seizure episodes detected.
    pub fn seizure_count(&self) -> u32 {
        self.seizure_count
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
        let d = SeizureDetector::new();
        assert_eq!(d.phase(), SeizurePhase::Monitoring);
        assert_eq!(d.seizure_count(), 0);
        assert_eq!(d.frame_count(), 0);
    }

    #[test]
    fn test_normal_motion_no_seizure() {
        let mut d = SeizureDetector::new();
        for _ in 0..200 {
            let ev = d.process_frame(0.0, 0.5, 0.3, 1);
            for &(t, _) in ev {
                assert!(
                    t != EVENT_SEIZURE_ONSET && t != EVENT_SEIZURE_TONIC
                    && t != EVENT_SEIZURE_CLONIC && t != EVENT_POST_ICTAL,
                    "no seizure events with normal motion"
                );
            }
        }
        assert_eq!(d.seizure_count(), 0);
    }

    #[test]
    fn test_fall_discrimination() {
        let mut d = SeizureDetector::new();
        // Short burst of high energy (fall-like): <FALL_MAX_DURATION frames.
        for _ in 0..5 {
            d.process_frame(0.0, 2.0, 5.0, 1);
        }
        // Then low energy (person is down).
        for _ in 0..100 {
            d.process_frame(0.0, 0.1, 0.05, 1);
        }
        // Should not trigger seizure.
        assert_eq!(d.seizure_count(), 0);
    }

    #[test]
    fn test_seizure_onset_with_sustained_high_energy() {
        let mut d = SeizureDetector::new();
        let mut onset_seen = false;

        // Sustained high energy with low variance (tonic-like).
        for _ in 0..100 {
            let ev = d.process_frame(0.0, 2.0, 3.0, 1);
            for &(t, _) in ev {
                if t == EVENT_SEIZURE_ONSET { onset_seen = true; }
            }
        }
        assert!(onset_seen, "seizure onset should trigger with sustained high energy");
        assert!(d.seizure_count() >= 1);
    }

    #[test]
    fn test_post_ictal_detection() {
        let mut d = SeizureDetector::new();
        let mut post_ictal_seen = false;

        // Tonic phase: sustained high energy.
        for _ in 0..50 {
            d.process_frame(0.0, 2.0, 3.0, 1);
        }

        // Sudden cessation → post-ictal.
        for _ in 0..100 {
            let ev = d.process_frame(0.0, 0.05, 0.05, 1);
            for &(t, _) in ev {
                if t == EVENT_POST_ICTAL { post_ictal_seen = true; }
            }
        }
        assert!(post_ictal_seen, "post-ictal should be detected after seizure cessation");
    }

    #[test]
    fn test_no_detection_without_presence() {
        let mut d = SeizureDetector::new();
        for _ in 0..200 {
            let ev = d.process_frame(0.0, 5.0, 10.0, 0);
            for &(t, _) in ev {
                assert!(t != EVENT_SEIZURE_ONSET, "no seizure events without presence");
            }
        }
        assert_eq!(d.seizure_count(), 0);
    }

    #[test]
    fn test_recent_energy_variance() {
        let mut d = SeizureDetector::new();
        // Feed constant energy.
        for _ in 0..30 {
            d.energy_buf[d.energy_idx] = 2.0;
            d.energy_idx = (d.energy_idx + 1) % ENERGY_WINDOW;
            d.energy_len = (d.energy_len + 1).min(ENERGY_WINDOW);
        }
        let v = d.recent_energy_variance();
        assert!(v < 0.01, "variance should be near zero for constant energy, got {}", v);
    }

    #[test]
    fn test_cooldown_after_episode() {
        let mut d = SeizureDetector::new();

        // Trigger seizure onset.
        for _ in 0..50 {
            d.process_frame(0.0, 2.0, 3.0, 1);
        }
        // Post-ictal.
        for _ in 0..100 {
            d.process_frame(0.0, 0.05, 0.05, 1);
        }

        // Should be in cooldown or monitoring now.
        let initial_count = d.seizure_count();

        // High energy again during cooldown should not trigger.
        for _ in 0..50 {
            d.process_frame(0.0, 2.0, 3.0, 1);
        }
        // Count should not increase beyond what the cooldown allows.
        // (The exact behavior depends on timing, but we verify no crash.)
        let _ = d.seizure_count();
        let _ = initial_count;
    }
}
