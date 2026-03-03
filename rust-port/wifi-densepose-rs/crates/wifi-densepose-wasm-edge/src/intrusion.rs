//! Intrusion detection — ADR-041 Phase 1 module (Security category).
//!
//! Detects unauthorized entry by monitoring CSI phase disturbance patterns:
//! - Sudden amplitude changes in previously quiet zones
//! - Phase velocity exceeding normal movement bounds
//! - Transition from "empty" to "occupied" state
//! - Anomalous movement patterns (too fast for normal human motion)
//!
//! Security-grade: low false-negative rate at the cost of higher false-positive.

#[cfg(not(feature = "std"))]
use libm::{fabsf, sqrtf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

/// Maximum subcarriers.
const MAX_SC: usize = 32;

/// Phase velocity threshold for intrusion (rad/frame — very fast movement).
const INTRUSION_VELOCITY_THRESH: f32 = 1.5;

/// Amplitude change ratio threshold (vs baseline).
const AMPLITUDE_CHANGE_THRESH: f32 = 3.0;

/// Frames of quiet before arming (5 seconds at 20 Hz).
const ARM_FRAMES: u32 = 100;

/// Minimum consecutive detection frames before alert (debounce).
const DETECT_DEBOUNCE: u8 = 3;

/// Cooldown frames after alert (prevent flooding).
const ALERT_COOLDOWN: u16 = 100;

/// Baseline calibration frames.
const BASELINE_FRAMES: u32 = 200;

/// Event types (200-series: Security).
pub const EVENT_INTRUSION_ALERT: i32 = 200;
pub const EVENT_INTRUSION_ZONE: i32 = 201;
pub const EVENT_INTRUSION_ARMED: i32 = 202;
pub const EVENT_INTRUSION_DISARMED: i32 = 203;

/// Detector state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DetectorState {
    /// Calibrating baseline (learning ambient environment).
    Calibrating,
    /// Monitoring but not armed (waiting for environment to settle).
    Monitoring,
    /// Armed — will trigger on intrusion.
    Armed,
    /// Alert active — intrusion detected.
    Alert,
}

/// Intrusion detector.
pub struct IntrusionDetector {
    /// Per-subcarrier baseline amplitude.
    baseline_amp: [f32; MAX_SC],
    /// Per-subcarrier baseline variance.
    baseline_var: [f32; MAX_SC],
    /// Previous phase values.
    prev_phases: [f32; MAX_SC],
    /// Calibration accumulators.
    calib_amp_sum: [f32; MAX_SC],
    calib_amp_sq_sum: [f32; MAX_SC],
    calib_count: u32,
    /// Current state.
    state: DetectorState,
    /// Consecutive quiet frames (for arming).
    quiet_frames: u32,
    /// Consecutive detection frames (debounce).
    detect_frames: u8,
    /// Alert cooldown counter.
    cooldown: u16,
    /// Phase initialized flag.
    phase_init: bool,
    /// Total alerts fired.
    alert_count: u32,
    /// Frame counter.
    frame_count: u32,
}

impl IntrusionDetector {
    pub const fn new() -> Self {
        Self {
            baseline_amp: [0.0; MAX_SC],
            baseline_var: [0.0; MAX_SC],
            prev_phases: [0.0; MAX_SC],
            calib_amp_sum: [0.0; MAX_SC],
            calib_amp_sq_sum: [0.0; MAX_SC],
            calib_count: 0,
            state: DetectorState::Calibrating,
            quiet_frames: 0,
            detect_frames: 0,
            cooldown: 0,
            phase_init: false,
            alert_count: 0,
            frame_count: 0,
        }
    }

    /// Process one frame. Returns events to emit.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
    ) -> &[(i32, f32)] {
        let n_sc = phases.len().min(amplitudes.len()).min(MAX_SC);
        if n_sc < 2 {
            return &[];
        }

        self.frame_count += 1;

        if self.cooldown > 0 {
            self.cooldown -= 1;
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        match self.state {
            DetectorState::Calibrating => {
                // Accumulate baseline statistics.
                for i in 0..n_sc {
                    self.calib_amp_sum[i] += amplitudes[i];
                    self.calib_amp_sq_sum[i] += amplitudes[i] * amplitudes[i];
                }
                self.calib_count += 1;

                if !self.phase_init {
                    for i in 0..n_sc {
                        self.prev_phases[i] = phases[i];
                    }
                    self.phase_init = true;
                }

                if self.calib_count >= BASELINE_FRAMES {
                    let n = self.calib_count as f32;
                    for i in 0..n_sc {
                        self.baseline_amp[i] = self.calib_amp_sum[i] / n;
                        let mean_sq = self.calib_amp_sq_sum[i] / n;
                        let mean = self.baseline_amp[i];
                        self.baseline_var[i] = mean_sq - mean * mean;
                        if self.baseline_var[i] < 0.001 {
                            self.baseline_var[i] = 0.001;
                        }
                    }
                    self.state = DetectorState::Monitoring;
                }
            }

            DetectorState::Monitoring => {
                // Wait for environment to be quiet before arming.
                let disturbance = self.compute_disturbance(phases, amplitudes, n_sc);
                if disturbance < 0.5 {
                    self.quiet_frames += 1;
                } else {
                    self.quiet_frames = 0;
                }

                if self.quiet_frames >= ARM_FRAMES {
                    self.state = DetectorState::Armed;
                    if n_events < 4 {
                        unsafe {
                            EVENTS[n_events] = (EVENT_INTRUSION_ARMED, 1.0);
                        }
                        n_events += 1;
                    }
                }

                // Update previous phases.
                for i in 0..n_sc {
                    self.prev_phases[i] = phases[i];
                }
            }

            DetectorState::Armed => {
                let disturbance = self.compute_disturbance(phases, amplitudes, n_sc);

                if disturbance >= 0.8 {
                    self.detect_frames = self.detect_frames.saturating_add(1);

                    if self.detect_frames >= DETECT_DEBOUNCE && self.cooldown == 0 {
                        self.state = DetectorState::Alert;
                        self.alert_count += 1;
                        self.cooldown = ALERT_COOLDOWN;

                        if n_events < 4 {
                            unsafe {
                                EVENTS[n_events] = (EVENT_INTRUSION_ALERT, disturbance);
                            }
                            n_events += 1;
                        }

                        // Find the most disturbed zone.
                        let zone = self.find_disturbed_zone(amplitudes, n_sc);
                        if n_events < 4 {
                            unsafe {
                                EVENTS[n_events] = (EVENT_INTRUSION_ZONE, zone as f32);
                            }
                            n_events += 1;
                        }
                    }
                } else {
                    self.detect_frames = 0;
                }

                for i in 0..n_sc {
                    self.prev_phases[i] = phases[i];
                }
            }

            DetectorState::Alert => {
                let disturbance = self.compute_disturbance(phases, amplitudes, n_sc);

                // Return to armed once the disturbance subsides.
                if disturbance < 0.3 {
                    self.quiet_frames += 1;
                    if self.quiet_frames >= ARM_FRAMES / 2 {
                        self.state = DetectorState::Armed;
                        self.detect_frames = 0;
                        self.quiet_frames = 0;
                    }
                } else {
                    self.quiet_frames = 0;
                }

                for i in 0..n_sc {
                    self.prev_phases[i] = phases[i];
                }
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Compute overall disturbance score.
    fn compute_disturbance(&self, phases: &[f32], amplitudes: &[f32], n_sc: usize) -> f32 {
        let mut phase_score = 0.0f32;
        let mut amp_score = 0.0f32;

        for i in 0..n_sc {
            // Phase velocity.
            let phase_vel = fabsf(phases[i] - self.prev_phases[i]);
            if phase_vel > INTRUSION_VELOCITY_THRESH {
                phase_score += 1.0;
            }

            // Amplitude deviation from baseline.
            let amp_dev = fabsf(amplitudes[i] - self.baseline_amp[i]);
            let sigma = sqrtf(self.baseline_var[i]);
            if amp_dev > AMPLITUDE_CHANGE_THRESH * sigma {
                amp_score += 1.0;
            }
        }

        let n = n_sc as f32;
        // Combined score: fraction of subcarriers showing disturbance.
        (phase_score / n) * 0.6 + (amp_score / n) * 0.4
    }

    /// Find the zone with highest amplitude disturbance.
    fn find_disturbed_zone(&self, amplitudes: &[f32], n_sc: usize) -> usize {
        let zone_count = (n_sc / 4).max(1);
        let subs_per_zone = n_sc / zone_count;
        let mut max_dev = 0.0f32;
        let mut max_zone = 0usize;

        for z in 0..zone_count {
            let start = z * subs_per_zone;
            let end = if z == zone_count - 1 { n_sc } else { start + subs_per_zone };
            let mut zone_dev = 0.0f32;

            for i in start..end {
                zone_dev += fabsf(amplitudes[i] - self.baseline_amp[i]);
            }

            if zone_dev > max_dev {
                max_dev = zone_dev;
                max_zone = z;
            }
        }

        max_zone
    }

    /// Get current detector state.
    pub fn state(&self) -> DetectorState {
        self.state
    }

    /// Get total alerts fired.
    pub fn total_alerts(&self) -> u32 {
        self.alert_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intrusion_init() {
        let det = IntrusionDetector::new();
        assert_eq!(det.state(), DetectorState::Calibrating);
        assert_eq!(det.total_alerts(), 0);
    }

    #[test]
    fn test_calibration_phase() {
        let mut det = IntrusionDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }

        assert_eq!(det.state(), DetectorState::Monitoring);
    }

    #[test]
    fn test_arm_after_quiet() {
        let mut det = IntrusionDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }
        assert_eq!(det.state(), DetectorState::Monitoring);

        // Feed quiet frames until armed.
        for _ in 0..ARM_FRAMES + 1 {
            det.process_frame(&phases, &amps);
        }
        assert_eq!(det.state(), DetectorState::Armed);
    }

    #[test]
    fn test_intrusion_detection() {
        let mut det = IntrusionDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // Calibrate + arm.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }
        for _ in 0..ARM_FRAMES + 1 {
            det.process_frame(&phases, &amps);
        }
        assert_eq!(det.state(), DetectorState::Armed);

        // Inject large disturbance with varying phases to maintain velocity.
        let intrusion_amps = [10.0f32; 16];

        let mut alert_detected = false;
        for frame in 0..10 {
            // Vary phase each frame so phase velocity stays high.
            let phase_val = 3.0 + (frame as f32) * 2.0;
            let intrusion_phases = [phase_val; 16];
            let events = det.process_frame(&intrusion_phases, &intrusion_amps);
            for &(et, _) in events {
                if et == EVENT_INTRUSION_ALERT {
                    alert_detected = true;
                }
            }
        }

        assert!(alert_detected, "intrusion should be detected after large disturbance");
        assert!(det.total_alerts() >= 1);
    }
}
