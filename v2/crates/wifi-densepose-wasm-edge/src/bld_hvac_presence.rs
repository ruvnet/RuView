//! HVAC-optimized presence detection — ADR-041 Category 3: Smart Building.
//!
//! Provides presence information tuned for HVAC energy management:
//! - Long departure timeout (5 min / 6000 frames) to avoid premature shutoff
//! - Fast arrival debounce (10 s / 200 frames) for quick occupancy detection
//! - Activity level classification: sedentary vs active
//!
//! Host API used: `csi_get_presence()`, `csi_get_motion_energy()`

// No libm imports needed — pure arithmetic and comparisons.

/// Arrival debounce: 10 seconds at 20 Hz = 200 frames.
const ARRIVAL_DEBOUNCE: u32 = 200;

/// Departure timeout: 5 minutes at 20 Hz = 6000 frames.
const DEPARTURE_TIMEOUT: u32 = 6000;

/// Motion energy threshold separating sedentary from active.
const ACTIVITY_THRESHOLD: f32 = 0.3;

/// EMA smoothing for motion energy.
const MOTION_ALPHA: f32 = 0.1;

/// Minimum presence score to consider someone present.
const PRESENCE_THRESHOLD: f32 = 0.5;

/// Event emission interval (every N frames to limit bandwidth).
const EMIT_INTERVAL: u32 = 20;

// ── Event IDs (310-312: HVAC Presence) ──────────────────────────────────────

pub const EVENT_HVAC_OCCUPIED: i32 = 310;
pub const EVENT_ACTIVITY_LEVEL: i32 = 311;
pub const EVENT_DEPARTURE_COUNTDOWN: i32 = 312;

/// HVAC presence states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HvacState {
    /// No one present, HVAC can enter energy-saving mode.
    Vacant,
    /// Presence detected but still within arrival debounce window.
    ArrivalPending,
    /// Confirmed occupied.
    Occupied,
    /// Presence lost, counting down before declaring vacant.
    DeparturePending,
}

/// Activity level classification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivityLevel {
    /// Low motion energy (reading, desk work, sleeping).
    Sedentary,
    /// High motion energy (walking, exercising, cleaning).
    Active,
}

/// HVAC-optimized presence detector.
pub struct HvacPresenceDetector {
    state: HvacState,
    /// Smoothed motion energy (EMA).
    motion_ema: f32,
    /// Current activity level.
    activity: ActivityLevel,
    /// Consecutive frames with presence detected (for arrival debounce).
    presence_frames: u32,
    /// Consecutive frames without presence (for departure timeout).
    absence_frames: u32,
    /// Frame counter.
    frame_count: u32,
}

impl HvacPresenceDetector {
    pub const fn new() -> Self {
        Self {
            state: HvacState::Vacant,
            motion_ema: 0.0,
            activity: ActivityLevel::Sedentary,
            presence_frames: 0,
            absence_frames: 0,
            frame_count: 0,
        }
    }

    /// Process one frame of presence and motion data.
    ///
    /// `presence_score`: 0.0-1.0 presence confidence from host.
    /// `motion_energy`: raw motion energy from host.
    ///
    /// Returns events as `(event_type, value)` pairs.
    pub fn process_frame(
        &mut self,
        presence_score: f32,
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        // Smooth motion energy with EMA.
        self.motion_ema = MOTION_ALPHA * motion_energy
            + (1.0 - MOTION_ALPHA) * self.motion_ema;

        // Classify activity level.
        self.activity = if self.motion_ema > ACTIVITY_THRESHOLD {
            ActivityLevel::Active
        } else {
            ActivityLevel::Sedentary
        };

        let is_present = presence_score > PRESENCE_THRESHOLD;

        // State machine transitions.
        match self.state {
            HvacState::Vacant => {
                if is_present {
                    self.presence_frames += 1;
                    self.absence_frames = 0;
                    if self.presence_frames >= ARRIVAL_DEBOUNCE {
                        self.state = HvacState::Occupied;
                    } else {
                        self.state = HvacState::ArrivalPending;
                    }
                } else {
                    self.presence_frames = 0;
                }
            }
            HvacState::ArrivalPending => {
                if is_present {
                    self.presence_frames += 1;
                    if self.presence_frames >= ARRIVAL_DEBOUNCE {
                        self.state = HvacState::Occupied;
                    }
                } else {
                    // Lost presence during debounce, reset.
                    self.presence_frames = 0;
                    self.state = HvacState::Vacant;
                }
            }
            HvacState::Occupied => {
                if is_present {
                    self.absence_frames = 0;
                } else {
                    self.absence_frames += 1;
                    self.state = HvacState::DeparturePending;
                }
            }
            HvacState::DeparturePending => {
                if is_present {
                    // Person returned, cancel departure.
                    self.absence_frames = 0;
                    self.state = HvacState::Occupied;
                } else {
                    self.absence_frames += 1;
                    if self.absence_frames >= DEPARTURE_TIMEOUT {
                        self.state = HvacState::Vacant;
                        self.presence_frames = 0;
                    }
                }
            }
        }

        // Build output events.
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n = 0usize;

        if self.frame_count % EMIT_INTERVAL == 0 {
            // Occupied status: 1.0 = occupied, 0.0 = vacant.
            let occupied_val = match self.state {
                HvacState::Occupied | HvacState::DeparturePending => 1.0,
                _ => 0.0,
            };
            unsafe {
                EVENTS[n] = (EVENT_HVAC_OCCUPIED, occupied_val);
            }
            n += 1;

            // Activity level: 0.0 = sedentary, 1.0 = active, plus raw EMA.
            let activity_val = match self.activity {
                ActivityLevel::Sedentary => 0.0 + self.motion_ema.min(0.99),
                ActivityLevel::Active => 1.0,
            };
            unsafe {
                EVENTS[n] = (EVENT_ACTIVITY_LEVEL, activity_val);
            }
            n += 1;
        }

        // Departure countdown: emit remaining time fraction when pending.
        if self.state == HvacState::DeparturePending
            && self.frame_count % EMIT_INTERVAL == 0
            && n < 3
        {
            let remaining = DEPARTURE_TIMEOUT.saturating_sub(self.absence_frames);
            let fraction = remaining as f32 / DEPARTURE_TIMEOUT as f32;
            unsafe {
                EVENTS[n] = (EVENT_DEPARTURE_COUNTDOWN, fraction);
            }
            n += 1;
        }

        unsafe { &EVENTS[..n] }
    }

    /// Get current HVAC state.
    pub fn state(&self) -> HvacState {
        self.state
    }

    /// Get current activity level.
    pub fn activity(&self) -> ActivityLevel {
        self.activity
    }

    /// Get smoothed motion energy.
    pub fn motion_ema(&self) -> f32 {
        self.motion_ema
    }

    /// Check if the space is considered occupied (for HVAC decisions).
    pub fn is_occupied(&self) -> bool {
        matches!(self.state, HvacState::Occupied | HvacState::DeparturePending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hvac_init() {
        let det = HvacPresenceDetector::new();
        assert_eq!(det.state(), HvacState::Vacant);
        assert!(!det.is_occupied());
        assert_eq!(det.activity(), ActivityLevel::Sedentary);
    }

    #[test]
    fn test_arrival_debounce() {
        let mut det = HvacPresenceDetector::new();

        // Feed presence for less than debounce period.
        for _ in 0..100 {
            det.process_frame(0.8, 0.1);
        }
        // Should still be in ArrivalPending, not yet Occupied.
        assert_eq!(det.state(), HvacState::ArrivalPending);
        assert!(!det.is_occupied());

        // Feed presence until debounce completes.
        for _ in 100..ARRIVAL_DEBOUNCE + 1 {
            det.process_frame(0.8, 0.1);
        }
        assert_eq!(det.state(), HvacState::Occupied);
        assert!(det.is_occupied());
    }

    #[test]
    fn test_departure_timeout() {
        let mut det = HvacPresenceDetector::new();

        // Establish occupancy.
        for _ in 0..ARRIVAL_DEBOUNCE + 10 {
            det.process_frame(0.8, 0.1);
        }
        assert!(det.is_occupied());

        // Remove presence: should go to DeparturePending.
        det.process_frame(0.0, 0.0);
        assert_eq!(det.state(), HvacState::DeparturePending);
        assert!(det.is_occupied()); // Still "occupied" during countdown.

        // Feed absence frames up to timeout.
        for _ in 0..DEPARTURE_TIMEOUT {
            det.process_frame(0.0, 0.0);
        }
        assert_eq!(det.state(), HvacState::Vacant);
        assert!(!det.is_occupied());
    }

    #[test]
    fn test_departure_cancelled_on_return() {
        let mut det = HvacPresenceDetector::new();

        // Establish occupancy.
        for _ in 0..ARRIVAL_DEBOUNCE + 10 {
            det.process_frame(0.8, 0.1);
        }
        assert!(det.is_occupied());

        // Start departure.
        for _ in 0..100 {
            det.process_frame(0.0, 0.0);
        }
        assert_eq!(det.state(), HvacState::DeparturePending);

        // Person returns.
        det.process_frame(0.8, 0.1);
        assert_eq!(det.state(), HvacState::Occupied);
    }

    #[test]
    fn test_activity_level_classification() {
        let mut det = HvacPresenceDetector::new();

        // Feed high motion energy for enough frames to saturate EMA.
        for _ in 0..200 {
            det.process_frame(0.8, 0.8);
        }
        assert_eq!(det.activity(), ActivityLevel::Active);

        // Feed low motion energy.
        for _ in 0..200 {
            det.process_frame(0.8, 0.01);
        }
        assert_eq!(det.activity(), ActivityLevel::Sedentary);
    }

    #[test]
    fn test_events_emitted_periodically() {
        let mut det = HvacPresenceDetector::new();

        // Establish occupancy.
        for _ in 0..ARRIVAL_DEBOUNCE + 10 {
            det.process_frame(0.8, 0.1);
        }

        // Process frames and check for events at EMIT_INTERVAL boundaries.
        let mut found_occupied_event = false;
        let mut found_activity_event = false;
        for _ in 0..EMIT_INTERVAL + 1 {
            let events = det.process_frame(0.8, 0.1);
            for &(et, _) in events {
                if et == EVENT_HVAC_OCCUPIED {
                    found_occupied_event = true;
                }
                if et == EVENT_ACTIVITY_LEVEL {
                    found_activity_event = true;
                }
            }
        }
        assert!(found_occupied_event, "should emit HVAC_OCCUPIED events");
        assert!(found_activity_event, "should emit ACTIVITY_LEVEL events");
    }

    #[test]
    fn test_false_presence_does_not_trigger() {
        let mut det = HvacPresenceDetector::new();

        // Brief presence blip (shorter than debounce).
        for _ in 0..50 {
            det.process_frame(0.8, 0.1);
        }
        // Then absence.
        det.process_frame(0.0, 0.0);
        assert_eq!(det.state(), HvacState::Vacant);
    }
}
