//! Elevator occupancy counting — ADR-041 Category 3: Smart Building.
//!
//! Counts occupants in an elevator cabin (1-12 persons) using confined-space
//! multipath analysis:
//! - Amplitude variance scales with body count in a small reflective space
//! - Phase diversity increases with more scatterers
//! - Sudden multipath geometry changes indicate door open/close events
//!
//! Host API used: `csi_get_amplitude()`, `csi_get_variance()`,
//!                `csi_get_phase()`, `csi_get_motion_energy()`,
//!                `csi_get_n_persons()`

use libm::fabsf;
#[cfg(not(feature = "std"))]
use libm::sqrtf;
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// Maximum occupants the elevator model supports.
const MAX_OCCUPANTS: usize = 12;

/// Overload threshold (default).
const DEFAULT_OVERLOAD: u8 = 10;

/// Baseline calibration frames.
const BASELINE_FRAMES: u32 = 200;

/// EMA smoothing for amplitude statistics.
const ALPHA: f32 = 0.15;

/// Variance ratio threshold for door open/close detection.
const DOOR_VARIANCE_RATIO: f32 = 4.0;

/// Debounce frames for door events.
const DOOR_DEBOUNCE: u8 = 3;

/// Cooldown frames after door event.
const DOOR_COOLDOWN: u16 = 40;

/// Event emission interval.
const EMIT_INTERVAL: u32 = 10;

// ── Event IDs (330-333: Elevator) ───────────────────────────────────────────

pub const EVENT_ELEVATOR_COUNT: i32 = 330;
pub const EVENT_DOOR_OPEN: i32 = 331;
pub const EVENT_DOOR_CLOSE: i32 = 332;
pub const EVENT_OVERLOAD_WARNING: i32 = 333;

/// Door state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DoorState {
    Closed,
    Open,
}

/// Elevator occupancy counter.
pub struct ElevatorCounter {
    /// Baseline amplitude per subcarrier (empty cabin).
    baseline_amp: [f32; MAX_SC],
    /// Baseline variance per subcarrier.
    baseline_var: [f32; MAX_SC],
    /// Previous frame amplitude for delta detection.
    prev_amp: [f32; MAX_SC],
    /// Smoothed overall variance.
    smoothed_var: f32,
    /// Smoothed amplitude spread.
    smoothed_spread: f32,
    /// Calibration accumulators.
    calib_amp_sum: [f32; MAX_SC],
    calib_amp_sq_sum: [f32; MAX_SC],
    calib_count: u32,
    calibrated: bool,
    /// Estimated occupant count.
    count: u8,
    /// Overload threshold.
    overload_thresh: u8,
    /// Door state.
    door: DoorState,
    /// Door event debounce counter.
    door_debounce: u8,
    /// Door event pending type (true = open, false = close).
    door_pending_open: bool,
    /// Door cooldown counter.
    door_cooldown: u16,
    /// Frame counter.
    frame_count: u32,
}

impl ElevatorCounter {
    pub const fn new() -> Self {
        Self {
            baseline_amp: [0.0; MAX_SC],
            baseline_var: [0.0; MAX_SC],
            prev_amp: [0.0; MAX_SC],
            smoothed_var: 0.0,
            smoothed_spread: 0.0,
            calib_amp_sum: [0.0; MAX_SC],
            calib_amp_sq_sum: [0.0; MAX_SC],
            calib_count: 0,
            calibrated: false,
            count: 0,
            overload_thresh: DEFAULT_OVERLOAD,
            door: DoorState::Closed,
            door_debounce: 0,
            door_pending_open: false,
            door_cooldown: 0,
            frame_count: 0,
        }
    }

    /// Process one frame.
    ///
    /// `amplitudes`: per-subcarrier amplitude array.
    /// `phases`: per-subcarrier phase array.
    /// `motion_energy`: overall motion energy from host.
    /// `host_n_persons`: person count hint from host (0 if unavailable).
    ///
    /// Returns events as `(event_type, value)` pairs.
    pub fn process_frame(
        &mut self,
        amplitudes: &[f32],
        phases: &[f32],
        motion_energy: f32,
        host_n_persons: i32,
    ) -> &[(i32, f32)] {
        let n_sc = amplitudes.len().min(phases.len()).min(MAX_SC);
        if n_sc < 2 {
            return &[];
        }

        self.frame_count += 1;

        if self.door_cooldown > 0 {
            self.door_cooldown -= 1;
        }

        // ── Calibration phase ───────────────────────────────────────────
        if !self.calibrated {
            for i in 0..n_sc {
                self.calib_amp_sum[i] += amplitudes[i];
                self.calib_amp_sq_sum[i] += amplitudes[i] * amplitudes[i];
            }
            self.calib_count += 1;

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
                    self.prev_amp[i] = amplitudes[i];
                }
                self.calibrated = true;
            }
            return &[];
        }

        // ── Compute multipath statistics ────────────────────────────────

        // 1. Overall amplitude variance deviation from baseline.
        let mut var_sum = 0.0f32;
        let mut spread_sum = 0.0f32;
        let mut delta_sum = 0.0f32;

        for i in 0..n_sc {
            let dev = amplitudes[i] - self.baseline_amp[i];
            var_sum += dev * dev;

            // Amplitude spread: max-min range.
            spread_sum += fabsf(amplitudes[i] - self.baseline_amp[i]);

            // Frame-to-frame delta for door detection.
            delta_sum += fabsf(amplitudes[i] - self.prev_amp[i]);

            self.prev_amp[i] = amplitudes[i];
        }

        let n_f = n_sc as f32;
        let frame_var = var_sum / n_f;
        let frame_spread = spread_sum / n_f;
        let frame_delta = delta_sum / n_f;

        // EMA smooth.
        self.smoothed_var = ALPHA * frame_var + (1.0 - ALPHA) * self.smoothed_var;
        self.smoothed_spread = ALPHA * frame_spread + (1.0 - ALPHA) * self.smoothed_spread;

        // ── Door detection ──────────────────────────────────────────────
        // A door open/close causes a sudden change in multipath geometry.
        let baseline_avg_var = {
            let mut s = 0.0f32;
            for i in 0..n_sc {
                s += self.baseline_var[i];
            }
            s / n_f
        };
        let door_threshold = sqrtf(baseline_avg_var) * DOOR_VARIANCE_RATIO;
        let is_door_event = frame_delta > door_threshold;

        if is_door_event && self.door_cooldown == 0 {
            let pending_open = self.door == DoorState::Closed;
            if self.door_pending_open == pending_open {
                self.door_debounce = self.door_debounce.saturating_add(1);
            } else {
                self.door_pending_open = pending_open;
                self.door_debounce = 1;
            }
        } else {
            self.door_debounce = 0;
        }

        let mut door_event: Option<i32> = None;
        if self.door_debounce >= DOOR_DEBOUNCE && self.door_cooldown == 0 {
            if self.door_pending_open {
                self.door = DoorState::Open;
                door_event = Some(EVENT_DOOR_OPEN);
            } else {
                self.door = DoorState::Closed;
                door_event = Some(EVENT_DOOR_CLOSE);
            }
            self.door_cooldown = DOOR_COOLDOWN;
            self.door_debounce = 0;
        }

        // ── Occupant count estimation ───────────────────────────────────
        // In a confined elevator cabin, multipath variance scales roughly
        // linearly with body count. We use a simple calibrated mapping.
        //
        // Fuse: host hint (if available) + own variance-based estimate.
        let var_ratio = if baseline_avg_var > 0.001 {
            self.smoothed_var / baseline_avg_var
        } else {
            self.smoothed_var * 100.0
        };

        // Empirical mapping: each person adds roughly 1.0 to var_ratio.
        let var_estimate = (var_ratio * 1.2) as u8;

        // Motion-energy based bonus: more people = more ambient motion.
        let motion_bonus = if motion_energy > 0.5 { 1u8 } else { 0u8 };

        let own_estimate = var_estimate.saturating_add(motion_bonus);
        let clamped_estimate = if own_estimate > MAX_OCCUPANTS as u8 {
            MAX_OCCUPANTS as u8
        } else {
            own_estimate
        };

        // Fuse with host hint if available.
        if host_n_persons > 0 {
            let host_val = host_n_persons as u8;
            // Weighted average: 60% host, 40% own.
            let fused = ((host_val as u16 * 6 + clamped_estimate as u16 * 4) / 10) as u8;
            self.count = if fused > MAX_OCCUPANTS as u8 {
                MAX_OCCUPANTS as u8
            } else {
                fused
            };
        } else {
            self.count = clamped_estimate;
        }

        // ── Build events ────────────────────────────────────────────────
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        // Door events (immediate).
        if let Some(evt) = door_event {
            if n_events < 4 {
                unsafe {
                    EVENTS[n_events] = (evt, self.count as f32);
                }
                n_events += 1;
            }
        }

        // Periodic count and overload.
        if self.frame_count % EMIT_INTERVAL == 0 {
            if n_events < 4 {
                unsafe {
                    EVENTS[n_events] = (EVENT_ELEVATOR_COUNT, self.count as f32);
                }
                n_events += 1;
            }

            // Overload warning.
            if self.count >= self.overload_thresh && n_events < 4 {
                unsafe {
                    EVENTS[n_events] = (EVENT_OVERLOAD_WARNING, self.count as f32);
                }
                n_events += 1;
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Get current occupant count estimate.
    pub fn occupant_count(&self) -> u8 {
        self.count
    }

    /// Get current door state.
    pub fn door_state(&self) -> DoorState {
        self.door
    }

    /// Set overload threshold.
    pub fn set_overload_threshold(&mut self, thresh: u8) {
        self.overload_thresh = thresh;
    }

    /// Check if calibration is complete.
    pub fn is_calibrated(&self) -> bool {
        self.calibrated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elevator_init() {
        let ec = ElevatorCounter::new();
        assert!(!ec.is_calibrated());
        assert_eq!(ec.occupant_count(), 0);
        assert_eq!(ec.door_state(), DoorState::Closed);
    }

    #[test]
    fn test_calibration() {
        let mut ec = ElevatorCounter::new();
        let amps = [1.0f32; 16];
        let phases = [0.0f32; 16];

        for _ in 0..BASELINE_FRAMES {
            let events = ec.process_frame(&amps, &phases, 0.0, 0);
            assert!(events.is_empty());
        }
        assert!(ec.is_calibrated());
    }

    #[test]
    fn test_occupancy_increases_with_variance() {
        let mut ec = ElevatorCounter::new();
        let baseline_amps = [1.0f32; 16];
        let phases = [0.0f32; 16];

        // Calibrate with empty cabin.
        for _ in 0..BASELINE_FRAMES {
            ec.process_frame(&baseline_amps, &phases, 0.0, 0);
        }

        // Introduce variance (people in cabin).
        let mut occupied_amps = [1.0f32; 16];
        for i in 0..16 {
            occupied_amps[i] = 1.0 + ((i % 3) as f32) * 2.0;
        }

        for _ in 0..50 {
            ec.process_frame(&occupied_amps, &phases, 0.2, 0);
        }

        assert!(ec.occupant_count() >= 1, "should detect at least 1 occupant");
    }

    #[test]
    fn test_host_hint_fusion() {
        let mut ec = ElevatorCounter::new();
        let amps = [1.0f32; 16];
        let phases = [0.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ec.process_frame(&amps, &phases, 0.0, 0);
        }

        // Feed with host hint of 5 persons.
        for _ in 0..30 {
            ec.process_frame(&amps, &phases, 0.1, 5);
        }

        // Count should be influenced by host hint.
        assert!(ec.occupant_count() >= 2, "host hint should influence count");
    }

    #[test]
    fn test_overload_event() {
        let mut ec = ElevatorCounter::new();
        ec.set_overload_threshold(3);
        let amps = [1.0f32; 16];
        let phases = [0.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ec.process_frame(&amps, &phases, 0.0, 0);
        }

        // Feed high count via host hint.
        let mut found_overload = false;
        for _ in 0..100 {
            let events = ec.process_frame(&amps, &phases, 0.5, 8);
            for &(et, _) in events {
                if et == EVENT_OVERLOAD_WARNING {
                    found_overload = true;
                }
            }
        }
        assert!(found_overload, "should emit OVERLOAD_WARNING when count >= threshold");
    }

    #[test]
    fn test_door_detection() {
        let mut ec = ElevatorCounter::new();
        let steady_amps = [1.0f32; 16];
        let phases = [0.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ec.process_frame(&steady_amps, &phases, 0.0, 0);
        }

        // Feed steady frames to initialize prev_amp.
        for _ in 0..10 {
            ec.process_frame(&steady_amps, &phases, 0.0, 0);
        }

        // Sudden large amplitude changes (simulates door opening).
        // Alternate between two very different amplitude patterns so that
        // frame-to-frame delta stays high across the debounce window.
        let door_amps_a = [8.0f32; 16];
        let door_amps_b = [1.0f32; 16];

        let mut found_door_event = false;
        for frame in 0..20 {
            let amps = if frame % 2 == 0 { &door_amps_a } else { &door_amps_b };
            let events = ec.process_frame(amps, &phases, 0.3, 0);
            for &(et, _) in events {
                if et == EVENT_DOOR_OPEN || et == EVENT_DOOR_CLOSE {
                    found_door_event = true;
                }
            }
        }
        assert!(found_door_event, "should detect door event from sudden amplitude change");
    }

    #[test]
    fn test_short_input() {
        let mut ec = ElevatorCounter::new();
        let events = ec.process_frame(&[1.0], &[0.0], 0.0, 0);
        assert!(events.is_empty());
    }
}
