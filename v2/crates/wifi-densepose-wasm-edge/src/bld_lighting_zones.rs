//! Per-zone lighting control — ADR-041 Category 3: Smart Building.
//!
//! Maps up to 4 spatial zones to lighting states:
//! - ON: zone occupied and active
//! - DIM: zone occupied but sedentary for >10 min (12000 frames at 20 Hz)
//! - OFF: zone vacant
//!
//! Gradual state transitions via per-zone state machine.
//!
//! Host API used: `csi_get_presence()`, `csi_get_motion_energy()`,
//!                `csi_get_variance()`

use libm::fabsf;

/// Maximum zones to manage.
const MAX_ZONES: usize = 4;

/// Maximum subcarriers per zone group.
const MAX_SC: usize = 32;

/// Variance threshold for zone occupancy detection.
const OCCUPANCY_THRESHOLD: f32 = 0.03;

/// Motion energy threshold for active vs sedentary.
const ACTIVE_THRESHOLD: f32 = 0.25;

/// Frames of sedentary occupancy before dimming (10 min at 20 Hz).
const DIM_TIMEOUT: u32 = 12000;

/// Frames of vacancy before turning off (30 s at 20 Hz).
const OFF_TIMEOUT: u32 = 600;

/// EMA smoothing for zone variance.
const ALPHA: f32 = 0.12;

/// Baseline calibration frames.
const BASELINE_FRAMES: u32 = 200;

/// Event emission interval.
const EMIT_INTERVAL: u32 = 20;

// ── Event IDs (320-322: Lighting Zones) ─────────────────────────────────────

pub const EVENT_LIGHT_ON: i32 = 320;
pub const EVENT_LIGHT_DIM: i32 = 321;
pub const EVENT_LIGHT_OFF: i32 = 322;

/// Lighting state per zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightState {
    Off,
    Dim,
    On,
}

/// Per-zone state tracking.
#[derive(Clone, Copy)]
struct ZoneLight {
    /// Current lighting state.
    state: LightState,
    /// Previous state (for transition detection).
    prev_state: LightState,
    /// Smoothed variance score.
    score: f32,
    /// Baseline variance (calibrated).
    baseline_var: f32,
    /// Whether zone is currently occupied.
    occupied: bool,
    /// Whether zone is currently active (high motion).
    active: bool,
    /// Consecutive frames of sedentary occupancy (for dim timer).
    sedentary_frames: u32,
    /// Consecutive frames of vacancy (for off timer).
    vacant_frames: u32,
}

/// Lighting zone controller.
pub struct LightingZoneController {
    zones: [ZoneLight; MAX_ZONES],
    n_zones: usize,
    /// Calibration accumulators.
    calib_sum: [f32; MAX_ZONES],
    calib_count: u32,
    calibrated: bool,
    /// Frame counter.
    frame_count: u32,
}

impl LightingZoneController {
    pub const fn new() -> Self {
        const ZONE_INIT: ZoneLight = ZoneLight {
            state: LightState::Off,
            prev_state: LightState::Off,
            score: 0.0,
            baseline_var: 0.0,
            occupied: false,
            active: false,
            sedentary_frames: 0,
            vacant_frames: 0,
        };
        Self {
            zones: [ZONE_INIT; MAX_ZONES],
            n_zones: 0,
            calib_sum: [0.0; MAX_ZONES],
            calib_count: 0,
            calibrated: false,
            frame_count: 0,
        }
    }

    /// Process one frame.
    ///
    /// `amplitudes`: per-subcarrier amplitude array.
    /// `motion_energy`: overall motion energy from host.
    ///
    /// Returns events as `(event_type, value)` pairs.
    /// Value encodes zone_id in integer part.
    pub fn process_frame(
        &mut self,
        amplitudes: &[f32],
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        let n_sc = amplitudes.len().min(MAX_SC);
        if n_sc < 4 {
            return &[];
        }

        self.frame_count += 1;

        let zone_count = (n_sc / 4).min(MAX_ZONES).max(1);
        self.n_zones = zone_count;
        let subs_per_zone = n_sc / zone_count;

        // Compute per-zone variance.
        let mut zone_vars = [0.0f32; MAX_ZONES];
        for z in 0..zone_count {
            let start = z * subs_per_zone;
            let end = if z == zone_count - 1 { n_sc } else { start + subs_per_zone };
            let count = (end - start) as f32;
            if count < 1.0 {
                continue;
            }

            let mut mean = 0.0f32;
            for i in start..end {
                mean += amplitudes[i];
            }
            mean /= count;

            let mut var = 0.0f32;
            for i in start..end {
                let d = amplitudes[i] - mean;
                var += d * d;
            }
            zone_vars[z] = var / count;
        }

        // Calibration phase.
        if !self.calibrated {
            for z in 0..zone_count {
                self.calib_sum[z] += zone_vars[z];
            }
            self.calib_count += 1;
            if self.calib_count >= BASELINE_FRAMES {
                let n = self.calib_count as f32;
                for z in 0..zone_count {
                    self.zones[z].baseline_var = self.calib_sum[z] / n;
                }
                self.calibrated = true;
            }
            return &[];
        }

        // Per-zone occupancy + activity update.
        for z in 0..zone_count {
            let deviation = fabsf(zone_vars[z] - self.zones[z].baseline_var);
            let raw_score = if self.zones[z].baseline_var > 0.001 {
                deviation / self.zones[z].baseline_var
            } else {
                deviation * 100.0
            };

            // EMA smooth.
            self.zones[z].score = ALPHA * raw_score + (1.0 - ALPHA) * self.zones[z].score;

            // Occupancy with hysteresis.
            let _was_occupied = self.zones[z].occupied;
            if self.zones[z].occupied {
                self.zones[z].occupied = self.zones[z].score > OCCUPANCY_THRESHOLD * 0.5;
            } else {
                self.zones[z].occupied = self.zones[z].score > OCCUPANCY_THRESHOLD;
            }

            // Per-zone activity: use motion_energy as a proxy, scaled by zone score.
            self.zones[z].active = motion_energy > ACTIVE_THRESHOLD
                && self.zones[z].score > OCCUPANCY_THRESHOLD * 0.7;

            // Update state machine.
            self.zones[z].prev_state = self.zones[z].state;

            if self.zones[z].occupied {
                self.zones[z].vacant_frames = 0;
                if self.zones[z].active {
                    self.zones[z].sedentary_frames = 0;
                    self.zones[z].state = LightState::On;
                } else {
                    self.zones[z].sedentary_frames += 1;
                    if self.zones[z].sedentary_frames >= DIM_TIMEOUT {
                        self.zones[z].state = LightState::Dim;
                    } else {
                        // Stay On during early sedentary period.
                        if self.zones[z].state == LightState::Off {
                            self.zones[z].state = LightState::On;
                        }
                    }
                }
            } else {
                self.zones[z].sedentary_frames = 0;
                self.zones[z].vacant_frames += 1;
                if self.zones[z].vacant_frames >= OFF_TIMEOUT {
                    self.zones[z].state = LightState::Off;
                }
                // During vacancy grace period, keep Dim if was On/Dim.
                if self.zones[z].vacant_frames < OFF_TIMEOUT
                    && self.zones[z].state == LightState::On
                {
                    self.zones[z].state = LightState::Dim;
                }
            }
        }

        // Build output events.
        static mut EVENTS: [(i32, f32); 8] = [(0, 0.0); 8];
        let mut n_events = 0usize;

        // Emit transitions immediately.
        for z in 0..zone_count {
            if self.zones[z].state != self.zones[z].prev_state && n_events < 8 {
                let event_id = match self.zones[z].state {
                    LightState::On => EVENT_LIGHT_ON,
                    LightState::Dim => EVENT_LIGHT_DIM,
                    LightState::Off => EVENT_LIGHT_OFF,
                };
                unsafe {
                    EVENTS[n_events] = (event_id, z as f32);
                }
                n_events += 1;
            }
        }

        // Periodic summary of all zone states.
        if self.frame_count % EMIT_INTERVAL == 0 {
            for z in 0..zone_count {
                if n_events < 8 {
                    let event_id = match self.zones[z].state {
                        LightState::On => EVENT_LIGHT_ON,
                        LightState::Dim => EVENT_LIGHT_DIM,
                        LightState::Off => EVENT_LIGHT_OFF,
                    };
                    // Encode zone_id + confidence in value.
                    let val = z as f32 + self.zones[z].score.min(0.99);
                    unsafe {
                        EVENTS[n_events] = (event_id, val);
                    }
                    n_events += 1;
                }
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Get the lighting state of a specific zone.
    pub fn zone_state(&self, zone_id: usize) -> LightState {
        if zone_id < self.n_zones {
            self.zones[zone_id].state
        } else {
            LightState::Off
        }
    }

    /// Get the number of active zones.
    pub fn n_zones(&self) -> usize {
        self.n_zones
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
    fn test_lighting_init() {
        let ctrl = LightingZoneController::new();
        assert!(!ctrl.is_calibrated());
        assert_eq!(ctrl.zone_state(0), LightState::Off);
    }

    #[test]
    fn test_calibration() {
        let mut ctrl = LightingZoneController::new();
        let amps = [1.0f32; 16];

        for _ in 0..BASELINE_FRAMES {
            let events = ctrl.process_frame(&amps, 0.0);
            assert!(events.is_empty());
        }
        assert!(ctrl.is_calibrated());
    }

    #[test]
    fn test_light_on_with_occupancy() {
        let mut ctrl = LightingZoneController::new();
        let uniform = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ctrl.process_frame(&uniform, 0.0);
        }

        // Inject disturbance in zone 0 with high motion energy.
        let mut disturbed = [1.0f32; 16];
        disturbed[0] = 5.0;
        disturbed[1] = 0.2;
        disturbed[2] = 4.5;
        disturbed[3] = 0.3;

        for _ in 0..100 {
            ctrl.process_frame(&disturbed, 0.5);
        }

        assert_eq!(ctrl.zone_state(0), LightState::On);
    }

    #[test]
    fn test_light_dim_after_sedentary_timeout() {
        let mut ctrl = LightingZoneController::new();
        let uniform = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ctrl.process_frame(&uniform, 0.0);
        }

        // Disturbed zone with high motion (turn on).
        let mut disturbed = [1.0f32; 16];
        disturbed[0] = 5.0;
        disturbed[1] = 0.2;
        disturbed[2] = 4.5;
        disturbed[3] = 0.3;

        for _ in 0..50 {
            ctrl.process_frame(&disturbed, 0.5);
        }
        assert_eq!(ctrl.zone_state(0), LightState::On);

        // Feed with low motion (sedentary) for DIM_TIMEOUT frames.
        for _ in 0..DIM_TIMEOUT + 10 {
            ctrl.process_frame(&disturbed, 0.01);
        }
        assert_eq!(ctrl.zone_state(0), LightState::Dim);
    }

    #[test]
    fn test_light_off_after_vacancy() {
        let mut ctrl = LightingZoneController::new();
        let uniform = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ctrl.process_frame(&uniform, 0.0);
        }

        // Create occupancy then remove it.
        let mut disturbed = [1.0f32; 16];
        disturbed[0] = 5.0;
        disturbed[1] = 0.2;
        disturbed[2] = 4.5;
        disturbed[3] = 0.3;

        for _ in 0..50 {
            ctrl.process_frame(&disturbed, 0.5);
        }

        // Remove disturbance and wait for OFF_TIMEOUT.
        for _ in 0..OFF_TIMEOUT + 100 {
            ctrl.process_frame(&uniform, 0.0);
        }
        assert_eq!(ctrl.zone_state(0), LightState::Off);
    }

    #[test]
    fn test_transition_events_emitted() {
        let mut ctrl = LightingZoneController::new();
        let uniform = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            ctrl.process_frame(&uniform, 0.0);
        }

        // Create disturbance to trigger On transition.
        let mut disturbed = [1.0f32; 16];
        disturbed[0] = 5.0;
        disturbed[1] = 0.2;
        disturbed[2] = 4.5;
        disturbed[3] = 0.3;

        let mut found_on = false;
        for _ in 0..100 {
            let events = ctrl.process_frame(&disturbed, 0.5);
            for &(et, _) in events {
                if et == EVENT_LIGHT_ON {
                    found_on = true;
                }
            }
        }
        assert!(found_on, "should emit LIGHT_ON event on transition");
    }

    #[test]
    fn test_short_input_returns_empty() {
        let mut ctrl = LightingZoneController::new();
        let short = [1.0f32; 2];
        let events = ctrl.process_frame(&short, 0.0);
        assert!(events.is_empty());
    }
}
