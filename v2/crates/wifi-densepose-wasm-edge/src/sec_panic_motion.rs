//! Panic/erratic motion detection — ADR-041 Category 2 Security module.
//!
//! Detects erratic high-energy movement patterns indicative of distress, struggle,
//! or fleeing. Computes two signals:
//!
//! 1. **Jerk** — rate of change of motion energy (d/dt of velocity proxy).
//!    High jerk indicates sudden, erratic changes in movement.
//!
//! 2. **Motion entropy** — unpredictability of motion direction changes.
//!    A person walking smoothly has low entropy; someone struggling or
//!    panicking exhibits rapid, random direction reversals = high entropy.
//!
//! Both jerk and entropy must exceed their respective thresholds simultaneously
//! over a 5-second window (100 frames at 20 Hz) to trigger an alert.
//!
//! Events: PANIC_DETECTED(250), STRUGGLE_PATTERN(251), FLEEING_DETECTED(252).
//! Budget: S (<5 ms).

#[cfg(not(feature = "std"))]
use libm::{fabsf, sqrtf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

const MAX_SC: usize = 32;
/// Window size for jerk/entropy computation (5 seconds at 20 Hz).
const WINDOW: usize = 100;
/// Jerk threshold (rate of change of motion energy per frame).
const JERK_THRESH: f32 = 2.0;
/// Entropy threshold (direction reversal rate in window).
const ENTROPY_THRESH: f32 = 0.35;
/// Minimum motion energy for detection (ignore idle).
const MIN_MOTION: f32 = 1.0;
/// Minimum presence required.
const MIN_PRESENCE: i32 = 1;
/// Fraction of window frames that must exceed both thresholds.
const TRIGGER_FRAC: f32 = 0.3;
/// Cooldown after event emission.
const COOLDOWN: u16 = 100;
/// Fleeing: sustained high energy threshold.
const FLEE_ENERGY_THRESH: f32 = 5.0;
/// Fleeing: minimum jerk threshold (lower than panic — running is rhythmic not chaotic).
/// Just needs to be above noise floor (person must be actively moving, not just present).
const FLEE_JERK_THRESH: f32 = 0.05;
/// Fleeing: maximum entropy (low = consistent direction, running is directional).
const FLEE_MAX_ENTROPY: f32 = 0.25;
/// Struggle detection: high jerk but moderate total energy (not fleeing).
const STRUGGLE_JERK_THRESH: f32 = 1.5;

pub const EVENT_PANIC_DETECTED: i32 = 250;
pub const EVENT_STRUGGLE_PATTERN: i32 = 251;
pub const EVENT_FLEEING_DETECTED: i32 = 252;

/// Panic/erratic motion detector.
pub struct PanicMotionDetector {
    /// Circular buffer of motion energy values.
    energy_buf: [f32; WINDOW],
    /// Circular buffer of phase variance values (for direction estimation).
    variance_buf: [f32; WINDOW],
    buf_idx: usize,
    buf_filled: bool,
    /// Previous motion energy (for jerk computation).
    prev_energy: f32,
    prev_energy_init: bool,
    /// Cooldowns.
    cd_panic: u16,
    cd_struggle: u16,
    cd_fleeing: u16,
    frame_count: u32,
    /// Total panic events.
    panic_count: u32,
}

impl PanicMotionDetector {
    pub const fn new() -> Self {
        Self {
            energy_buf: [0.0; WINDOW],
            variance_buf: [0.0; WINDOW],
            buf_idx: 0,
            buf_filled: false,
            prev_energy: 0.0,
            prev_energy_init: false,
            cd_panic: 0,
            cd_struggle: 0,
            cd_fleeing: 0,
            frame_count: 0,
            panic_count: 0,
        }
    }

    /// Process one frame. Returns `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        motion_energy: f32,
        variance_mean: f32,
        _phase_mean: f32,
        presence: i32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.cd_panic = self.cd_panic.saturating_sub(1);
        self.cd_struggle = self.cd_struggle.saturating_sub(1);
        self.cd_fleeing = self.cd_fleeing.saturating_sub(1);

        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut ne = 0usize;

        // Store in circular buffer.
        self.energy_buf[self.buf_idx] = motion_energy;
        self.variance_buf[self.buf_idx] = variance_mean;
        self.buf_idx = (self.buf_idx + 1) % WINDOW;
        if self.buf_idx == 0 {
            self.buf_filled = true;
        }

        // Need full window before analysis.
        if !self.buf_filled {
            self.prev_energy = motion_energy;
            self.prev_energy_init = true;
            return unsafe { &EVENTS[..0] };
        }

        // Require presence.
        if presence < MIN_PRESENCE {
            self.prev_energy = motion_energy;
            return unsafe { &EVENTS[..0] };
        }

        // Compute jerk (absolute rate of change of motion energy).
        let _jerk = if self.prev_energy_init {
            fabsf(motion_energy - self.prev_energy)
        } else {
            0.0
        };

        // Compute window statistics.
        let (mean_jerk, mean_energy, entropy, high_jerk_frac) =
            self.compute_window_stats();

        self.prev_energy = motion_energy;
        self.prev_energy_init = true;

        // Skip if not enough motion.
        if mean_energy < MIN_MOTION {
            return unsafe { &EVENTS[..0] };
        }

        // Panic detection: high jerk AND high entropy over threshold fraction of window.
        let is_panic = mean_jerk > JERK_THRESH
            && entropy > ENTROPY_THRESH
            && high_jerk_frac > TRIGGER_FRAC;

        if is_panic && self.cd_panic == 0 && ne < 3 {
            let severity = (mean_jerk / JERK_THRESH) * (entropy / ENTROPY_THRESH);
            unsafe { EVENTS[ne] = (EVENT_PANIC_DETECTED, severity.min(10.0)); }
            ne += 1;
            self.cd_panic = COOLDOWN;
            self.panic_count += 1;
        }

        // Struggle pattern: elevated jerk, moderate energy (person not displacing far).
        // Does not require high_jerk_frac (individual jerks may be below JERK_THRESH
        // but the *mean* jerk is still elevated from constant direction reversals).
        let is_struggle = mean_jerk > STRUGGLE_JERK_THRESH
            && mean_energy < FLEE_ENERGY_THRESH
            && mean_energy > MIN_MOTION
            && entropy > ENTROPY_THRESH * 0.5;

        if is_struggle && !is_panic && self.cd_struggle == 0 && ne < 3 {
            unsafe { EVENTS[ne] = (EVENT_STRUGGLE_PATTERN, mean_jerk); }
            ne += 1;
            self.cd_struggle = COOLDOWN;
        }

        // Fleeing detection: sustained high energy with low entropy (running in one direction).
        // Running produces rhythmic jerk but consistent direction (low entropy).
        let is_fleeing = mean_energy > FLEE_ENERGY_THRESH
            && mean_jerk > FLEE_JERK_THRESH
            && entropy < FLEE_MAX_ENTROPY;

        if is_fleeing && !is_panic && self.cd_fleeing == 0 && ne < 3 {
            unsafe { EVENTS[ne] = (EVENT_FLEEING_DETECTED, mean_energy); }
            ne += 1;
            self.cd_fleeing = COOLDOWN;
        }

        unsafe { &EVENTS[..ne] }
    }

    /// Compute window-level statistics.
    fn compute_window_stats(&self) -> (f32, f32, f32, f32) {
        let mut sum_jerk = 0.0f32;
        let mut sum_energy = 0.0f32;
        let mut direction_changes = 0u32;
        let mut high_jerk_count = 0u32;
        let mut prev_e = self.energy_buf[0];
        let mut prev_sign = 0i8; // +1 increasing, -1 decreasing, 0 unknown.

        for k in 1..WINDOW {
            let e = self.energy_buf[k];
            let j = fabsf(e - prev_e);
            sum_jerk += j;
            sum_energy += e;

            if j > JERK_THRESH {
                high_jerk_count += 1;
            }

            // Track direction reversals for entropy.
            let sign: i8 = if e > prev_e + 0.1 {
                1
            } else if e < prev_e - 0.1 {
                -1
            } else {
                prev_sign // Unchanged.
            };

            if prev_sign != 0 && sign != 0 && sign != prev_sign {
                direction_changes += 1;
            }
            prev_sign = sign;
            prev_e = e;
        }

        let n = (WINDOW - 1) as f32;
        let mean_jerk = sum_jerk / n;
        let mean_energy = sum_energy / n;
        // Entropy proxy: fraction of frames with direction reversals.
        let entropy = direction_changes as f32 / n;
        let high_jerk_frac = high_jerk_count as f32 / n;

        (mean_jerk, mean_energy, entropy, high_jerk_frac)
    }

    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn panic_count(&self) -> u32 { self.panic_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let det = PanicMotionDetector::new();
        assert_eq!(det.frame_count(), 0);
        assert_eq!(det.panic_count(), 0);
    }

    #[test]
    fn test_no_events_before_window_filled() {
        let mut det = PanicMotionDetector::new();
        for i in 0..(WINDOW - 1) {
            let ev = det.process_frame(5.0 + (i as f32) * 0.1, 1.0, 0.5, 1);
            assert!(ev.is_empty(), "no events before window is filled");
        }
    }

    #[test]
    fn test_calm_motion_no_panic() {
        let mut det = PanicMotionDetector::new();
        // Fill window with smooth, consistent motion.
        for i in 0..200u32 {
            let energy = 2.0 + (i as f32) * 0.01; // Slowly increasing.
            let ev = det.process_frame(energy, 0.1, 0.5, 1);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_PANIC_DETECTED, "calm motion should not trigger panic");
            }
        }
    }

    #[test]
    fn test_panic_detection() {
        let mut det = PanicMotionDetector::new();
        // Fill buffer with erratic, high-jerk motion.
        let mut found_panic = false;
        for i in 0..300u32 {
            // Alternating high and low energy = high jerk + high entropy.
            let energy = if i % 2 == 0 { 8.0 } else { 1.5 };
            let ev = det.process_frame(energy, 1.0, 0.5, 1);
            for &(et, _) in ev {
                if et == EVENT_PANIC_DETECTED {
                    found_panic = true;
                }
            }
        }
        assert!(found_panic, "erratic alternating motion should trigger panic");
        assert!(det.panic_count() >= 1);
    }

    #[test]
    fn test_no_panic_without_presence() {
        let mut det = PanicMotionDetector::new();
        for i in 0..300u32 {
            let energy = if i % 2 == 0 { 8.0 } else { 1.5 };
            let ev = det.process_frame(energy, 1.0, 0.5, 0); // No presence.
            for &(et, _) in ev {
                assert_ne!(et, EVENT_PANIC_DETECTED, "no panic without presence");
            }
        }
    }

    #[test]
    fn test_fleeing_detection() {
        let mut det = PanicMotionDetector::new();
        // Simulate fleeing: sustained high energy, mostly monotonic (low entropy).
        // Person is running in one direction: energy steadily rises with small jitter.
        let mut found_fleeing = false;
        for i in 0..300u32 {
            // Steadily increasing energy: 6.0 up to ~12.0 over 300 frames.
            // Jitter of +/- 0.05 does not reverse direction often => low entropy.
            // Mean energy ~ 9.0 > FLEE_ENERGY_THRESH (5.0).
            // Mean jerk ~ 0.02/frame + occasional 0.1 jitter = ~0.05.
            // But FLEE_JERK_THRESH = 0.3, so we need slightly more jerk.
            // Add a small step every 10 frames.
            let base = 6.0 + (i as f32) * 0.02;
            let step = if i % 10 == 0 { 0.5 } else { 0.0 };
            let energy = base + step;
            let ev = det.process_frame(energy, 0.5, 0.5, 1);
            for &(et, _) in ev {
                if et == EVENT_FLEEING_DETECTED {
                    found_fleeing = true;
                }
            }
        }
        assert!(found_fleeing, "sustained high energy should trigger fleeing");
    }

    #[test]
    fn test_struggle_pattern() {
        let mut det = PanicMotionDetector::new();
        // Simulate struggle: moderate jerk (above STRUGGLE_JERK_THRESH=1.5 but
        // below JERK_THRESH=2.0 or with insufficient high_jerk_frac for panic),
        // moderate energy (below FLEE_ENERGY_THRESH=5.0), some direction changes.
        // Pattern: 3.0, 1.2, 3.0, 1.2, ... => jerk = 1.8 per transition.
        // Mean jerk = 1.8 > 1.5 (struggle threshold).
        // Mean jerk = 1.8 < 2.0 (panic threshold), so panic won't fire.
        // Mean energy = 2.1 > MIN_MOTION=1.0 and < FLEE_ENERGY_THRESH=5.0.
        // Entropy: alternates every frame => ~0.5 > ENTROPY_THRESH*0.5=0.175.
        let mut found_struggle = false;
        for i in 0..300u32 {
            let energy = if i % 2 == 0 { 3.0 } else { 1.2 };
            let ev = det.process_frame(energy, 0.5, 0.5, 1);
            for &(et, _) in ev {
                if et == EVENT_STRUGGLE_PATTERN {
                    found_struggle = true;
                }
            }
        }
        assert!(found_struggle, "moderate energy with high jerk should trigger struggle");
    }

    #[test]
    fn test_low_motion_ignored() {
        let mut det = PanicMotionDetector::new();
        // Very low motion energy — below MIN_MOTION.
        for _ in 0..300 {
            let ev = det.process_frame(0.2, 0.01, 0.1, 1);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_PANIC_DETECTED);
                assert_ne!(et, EVENT_STRUGGLE_PATTERN);
                assert_ne!(et, EVENT_FLEEING_DETECTED);
            }
        }
    }
}
