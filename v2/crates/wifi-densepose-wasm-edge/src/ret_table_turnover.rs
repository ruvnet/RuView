//! Table turnover tracking — ADR-041 Category 4: Retail & Hospitality.
//!
//! Restaurant table state machine: empty -> seated -> eating -> departing -> empty.
//! Tracks seating duration and emits turnover events.
//! Designed for single-table sensing zone per ESP32 node.
//!
//! Events (430-series):
//! - `TABLE_SEATED(430)`:     Someone sat down at the table
//! - `TABLE_VACATED(431)`:    Table has been vacated
//! - `TABLE_AVAILABLE(432)`:  Table is clean/ready (post-vacate cooldown)
//! - `TURNOVER_RATE(433)`:    Turnovers per hour (rolling)
//!
//! Host API used: presence, motion energy, n_persons.

use crate::vendor_common::Ema;

// ── Event IDs ─────────────────────────────────────────────────────────────────

pub const EVENT_TABLE_SEATED: i32 = 430;
pub const EVENT_TABLE_VACATED: i32 = 431;
pub const EVENT_TABLE_AVAILABLE: i32 = 432;
pub const EVENT_TURNOVER_RATE: i32 = 433;

// ── Configuration constants ──────────────────────────────────────────────────

/// Frame rate assumption (Hz).
const FRAME_RATE: f32 = 20.0;

/// Frames to confirm seating (debounce: ~2 seconds).
const SEATED_DEBOUNCE_FRAMES: u32 = 40;

/// Frames to confirm vacancy (debounce: ~5 seconds, avoids brief absences).
const VACATED_DEBOUNCE_FRAMES: u32 = 100;

/// Frames for table to be marked available after vacating (~30 seconds for cleanup).
const AVAILABLE_COOLDOWN_FRAMES: u32 = 600;

/// Frames per hour (at 20 Hz).
const FRAMES_PER_HOUR: u32 = 72000;

/// Motion energy threshold below which someone is "settled" (eating/sitting).
const EATING_MOTION_THRESH: f32 = 0.1;

/// Motion energy threshold above which someone is "active" (arriving/departing).
const ACTIVE_MOTION_THRESH: f32 = 0.3;

/// Reporting interval for turnover rate (~5 minutes).
const TURNOVER_REPORT_INTERVAL: u32 = 6000;

/// EMA alpha for motion smoothing.
const MOTION_EMA_ALPHA: f32 = 0.15;

/// Rolling window for turnover rate (1 hour in frames).
const TURNOVER_WINDOW_FRAMES: u32 = 72000;

/// Maximum turnovers tracked in rolling window.
const MAX_TURNOVERS: usize = 50;

/// Maximum events per frame.
const MAX_EVENTS: usize = 4;

// ── Table State ──────────────────────────────────────────────────────────────

/// State machine states for a restaurant table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableState {
    /// Table is empty, ready for guests.
    Empty,
    /// Guests are being seated (presence detected, confirming).
    Seating,
    /// Guests are seated and eating (low motion, sustained presence).
    Eating,
    /// Guests are departing (high motion, presence dropping).
    Departing,
    /// Table vacated, in cleanup cooldown.
    Cooldown,
}

// ── Table Turnover Tracker ──────────────────────────────────────────────────

/// Tracks table occupancy state transitions and turnover metrics.
pub struct TableTurnoverTracker {
    /// Current table state.
    state: TableState,
    /// Smoothed motion energy.
    motion_ema: Ema,
    /// Consecutive frames with presence (for seating confirmation).
    presence_frames: u32,
    /// Consecutive frames without presence (for vacancy confirmation).
    absence_frames: u32,
    /// Frames spent in current seating session.
    session_frames: u32,
    /// Cooldown counter (frames remaining).
    cooldown_counter: u32,
    /// Frame counter.
    frame_count: u32,
    /// Total turnovers since reset.
    total_turnovers: u32,
    /// Recent turnover timestamps (frame numbers) for rate calculation.
    turnover_timestamps: [u32; MAX_TURNOVERS],
    /// Number of recorded turnover timestamps.
    turnover_count: usize,
    /// Index for circular overwrite in turnover_timestamps.
    turnover_idx: usize,
    /// Number of persons at the table (peak during session).
    peak_persons: i32,
}

impl TableTurnoverTracker {
    pub const fn new() -> Self {
        Self {
            state: TableState::Empty,
            motion_ema: Ema::new(MOTION_EMA_ALPHA),
            presence_frames: 0,
            absence_frames: 0,
            session_frames: 0,
            cooldown_counter: 0,
            frame_count: 0,
            total_turnovers: 0,
            turnover_timestamps: [0; MAX_TURNOVERS],
            turnover_count: 0,
            turnover_idx: 0,
            peak_persons: 0,
        }
    }

    /// Process one CSI frame with host-provided signals.
    ///
    /// - `presence`: 1 if someone is present, 0 otherwise
    /// - `motion_energy`: aggregate motion energy
    /// - `n_persons`: estimated person count
    ///
    /// Returns event slice `&[(event_type, value)]`.
    pub fn process_frame(
        &mut self,
        presence: i32,
        motion_energy: f32,
        n_persons: i32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        let is_present = presence > 0 || n_persons > 0;
        let smoothed_motion = self.motion_ema.update(motion_energy);
        let n = if n_persons < 0 { 0 } else { n_persons };

        static mut EVENTS: [(i32, f32); MAX_EVENTS] = [(0, 0.0); MAX_EVENTS];
        let mut ne = 0usize;

        match self.state {
            TableState::Empty => {
                if is_present {
                    self.presence_frames += 1;
                    if self.presence_frames >= SEATED_DEBOUNCE_FRAMES {
                        // Transition: Empty -> Seating confirmed -> Eating.
                        self.state = TableState::Eating;
                        self.session_frames = 0;
                        self.peak_persons = n;
                        self.absence_frames = 0;

                        if ne < MAX_EVENTS {
                            unsafe {
                                EVENTS[ne] = (EVENT_TABLE_SEATED, n as f32);
                            }
                            ne += 1;
                        }
                    }
                } else {
                    self.presence_frames = 0;
                }
            }

            TableState::Seating => {
                // This state is implicit (handled in Empty -> Eating transition).
                // Keeping for completeness; actual logic uses Empty with debounce.
                self.state = TableState::Eating;
            }

            TableState::Eating => {
                self.session_frames += 1;

                // Track peak persons.
                if n > self.peak_persons {
                    self.peak_persons = n;
                }

                if !is_present {
                    self.absence_frames += 1;
                    if self.absence_frames >= VACATED_DEBOUNCE_FRAMES {
                        // Transition: Eating -> Departing -> Cooldown.
                        self.state = TableState::Cooldown;
                        self.cooldown_counter = AVAILABLE_COOLDOWN_FRAMES;
                        self.total_turnovers += 1;

                        // Record turnover timestamp.
                        self.turnover_timestamps[self.turnover_idx] = self.frame_count;
                        self.turnover_idx = (self.turnover_idx + 1) % MAX_TURNOVERS;
                        if self.turnover_count < MAX_TURNOVERS {
                            self.turnover_count += 1;
                        }

                        // Duration in seconds.
                        let duration_s = self.session_frames as f32 / FRAME_RATE;

                        if ne < MAX_EVENTS {
                            unsafe {
                                EVENTS[ne] = (EVENT_TABLE_VACATED, duration_s);
                            }
                            ne += 1;
                        }

                        self.session_frames = 0;
                        self.absence_frames = 0;
                    }
                } else {
                    self.absence_frames = 0;

                    // Detect departing behavior: high motion while presence drops.
                    if smoothed_motion > ACTIVE_MOTION_THRESH && n < self.peak_persons {
                        // Guests may be leaving, but wait for actual absence.
                        self.state = TableState::Departing;
                    }
                }
            }

            TableState::Departing => {
                self.session_frames += 1;

                if !is_present {
                    self.absence_frames += 1;
                    if self.absence_frames >= VACATED_DEBOUNCE_FRAMES {
                        self.state = TableState::Cooldown;
                        self.cooldown_counter = AVAILABLE_COOLDOWN_FRAMES;
                        self.total_turnovers += 1;

                        let turnover_frame = self.frame_count;
                        self.turnover_timestamps[self.turnover_idx] = turnover_frame;
                        self.turnover_idx = (self.turnover_idx + 1) % MAX_TURNOVERS;
                        if self.turnover_count < MAX_TURNOVERS {
                            self.turnover_count += 1;
                        }

                        let duration_s = self.session_frames as f32 / FRAME_RATE;
                        if ne < MAX_EVENTS {
                            unsafe {
                                EVENTS[ne] = (EVENT_TABLE_VACATED, duration_s);
                            }
                            ne += 1;
                        }

                        self.session_frames = 0;
                        self.absence_frames = 0;
                    }
                } else {
                    self.absence_frames = 0;
                    // If motion settles, return to Eating.
                    if smoothed_motion < EATING_MOTION_THRESH {
                        self.state = TableState::Eating;
                    }
                }
            }

            TableState::Cooldown => {
                if self.cooldown_counter > 0 {
                    self.cooldown_counter -= 1;
                }

                if self.cooldown_counter == 0 {
                    self.state = TableState::Empty;
                    self.presence_frames = 0;
                    self.peak_persons = 0;

                    if ne < MAX_EVENTS {
                        unsafe {
                            EVENTS[ne] = (EVENT_TABLE_AVAILABLE, 1.0);
                        }
                        ne += 1;
                    }
                } else if is_present {
                    // Someone sat down during cleanup — fast transition back.
                    self.presence_frames += 1;
                    if self.presence_frames >= SEATED_DEBOUNCE_FRAMES / 2 {
                        self.state = TableState::Eating;
                        self.session_frames = 0;
                        self.peak_persons = n;
                        self.presence_frames = 0;

                        if ne < MAX_EVENTS {
                            unsafe {
                                EVENTS[ne] = (EVENT_TABLE_SEATED, n as f32);
                            }
                            ne += 1;
                        }
                    }
                } else {
                    self.presence_frames = 0;
                }
            }
        }

        // Periodic turnover rate report.
        if self.frame_count % TURNOVER_REPORT_INTERVAL == 0 && self.frame_count > 0 {
            let rate = self.turnover_rate();
            if ne < MAX_EVENTS {
                unsafe {
                    EVENTS[ne] = (EVENT_TURNOVER_RATE, rate);
                }
                ne += 1;
            }
        }

        unsafe { &EVENTS[..ne] }
    }

    /// Compute turnovers per hour (rolling window).
    pub fn turnover_rate(&self) -> f32 {
        if self.turnover_count == 0 || self.frame_count < 100 {
            return 0.0;
        }

        // Count turnovers within the last hour.
        let window_start = if self.frame_count > TURNOVER_WINDOW_FRAMES {
            self.frame_count - TURNOVER_WINDOW_FRAMES
        } else {
            0
        };

        let mut count = 0u32;
        for i in 0..self.turnover_count {
            if self.turnover_timestamps[i] >= window_start {
                count += 1;
            }
        }

        // Scale to per-hour rate.
        let elapsed_hours = self.frame_count as f32 / FRAMES_PER_HOUR as f32;
        let window_hours = if elapsed_hours < 1.0 { elapsed_hours } else { 1.0 };

        if window_hours > 0.001 {
            count as f32 / window_hours
        } else {
            0.0
        }
    }

    /// Get current table state.
    pub fn state(&self) -> TableState {
        self.state
    }

    /// Get total turnovers.
    pub fn total_turnovers(&self) -> u32 {
        self.total_turnovers
    }

    /// Get session duration in seconds (0 if not in a session).
    pub fn session_duration_s(&self) -> f32 {
        match self.state {
            TableState::Eating | TableState::Departing => {
                self.session_frames as f32 / FRAME_RATE
            }
            _ => 0.0,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let tt = TableTurnoverTracker::new();
        assert_eq!(tt.state(), TableState::Empty);
        assert_eq!(tt.total_turnovers(), 0);
        assert!(tt.session_duration_s() < 0.001);
    }

    #[test]
    fn test_seated_after_debounce() {
        let mut tt = TableTurnoverTracker::new();
        let mut seated_event = false;

        for _ in 0..SEATED_DEBOUNCE_FRAMES + 1 {
            let events = tt.process_frame(1, 0.2, 2);
            for &(et, _) in events {
                if et == EVENT_TABLE_SEATED {
                    seated_event = true;
                }
            }
        }

        assert!(seated_event, "TABLE_SEATED should fire after debounce period");
        assert_eq!(tt.state(), TableState::Eating);
    }

    #[test]
    fn test_vacated_after_absence() {
        let mut tt = TableTurnoverTracker::new();

        // Seat guests.
        for _ in 0..SEATED_DEBOUNCE_FRAMES + 1 {
            tt.process_frame(1, 0.05, 2);
        }
        assert_eq!(tt.state(), TableState::Eating);

        // Guests leave.
        let mut vacated_event = false;
        for _ in 0..VACATED_DEBOUNCE_FRAMES + 1 {
            let events = tt.process_frame(0, 0.0, 0);
            for &(et, _) in events {
                if et == EVENT_TABLE_VACATED {
                    vacated_event = true;
                }
            }
        }

        assert!(vacated_event, "TABLE_VACATED should fire after absence debounce");
        assert_eq!(tt.state(), TableState::Cooldown);
        assert_eq!(tt.total_turnovers(), 1);
    }

    #[test]
    fn test_available_after_cooldown() {
        let mut tt = TableTurnoverTracker::new();

        // Seat + vacate.
        for _ in 0..SEATED_DEBOUNCE_FRAMES + 1 {
            tt.process_frame(1, 0.05, 2);
        }
        for _ in 0..VACATED_DEBOUNCE_FRAMES + 1 {
            tt.process_frame(0, 0.0, 0);
        }
        assert_eq!(tt.state(), TableState::Cooldown);

        // Wait for cooldown.
        let mut available_event = false;
        for _ in 0..AVAILABLE_COOLDOWN_FRAMES + 1 {
            let events = tt.process_frame(0, 0.0, 0);
            for &(et, _) in events {
                if et == EVENT_TABLE_AVAILABLE {
                    available_event = true;
                }
            }
        }

        assert!(available_event, "TABLE_AVAILABLE should fire after cooldown");
        assert_eq!(tt.state(), TableState::Empty);
    }

    #[test]
    fn test_brief_absence_doesnt_vacate() {
        let mut tt = TableTurnoverTracker::new();

        // Seat guests.
        for _ in 0..SEATED_DEBOUNCE_FRAMES + 1 {
            tt.process_frame(1, 0.05, 2);
        }
        assert_eq!(tt.state(), TableState::Eating);

        // Brief absence (shorter than debounce).
        for _ in 0..VACATED_DEBOUNCE_FRAMES / 2 {
            tt.process_frame(0, 0.0, 0);
        }

        // Presence returns.
        tt.process_frame(1, 0.05, 2);

        // Should still be in Eating, not vacated.
        assert!(
            tt.state() == TableState::Eating || tt.state() == TableState::Departing,
            "brief absence should not trigger vacate, got {:?}", tt.state()
        );
        assert_eq!(tt.total_turnovers(), 0);
    }

    #[test]
    fn test_turnover_rate_computation() {
        let mut tt = TableTurnoverTracker::new();

        // Simulate two full turnover cycles.
        for _ in 0..2 {
            // Seat.
            for _ in 0..SEATED_DEBOUNCE_FRAMES + 1 {
                tt.process_frame(1, 0.05, 2);
            }
            // Eat for a while.
            for _ in 0..200 {
                tt.process_frame(1, 0.03, 2);
            }
            // Vacate.
            for _ in 0..VACATED_DEBOUNCE_FRAMES + 1 {
                tt.process_frame(0, 0.0, 0);
            }
            // Cooldown.
            for _ in 0..AVAILABLE_COOLDOWN_FRAMES + 1 {
                tt.process_frame(0, 0.0, 0);
            }
        }

        assert_eq!(tt.total_turnovers(), 2);
        let rate = tt.turnover_rate();
        assert!(rate > 0.0, "turnover rate should be positive, got {}", rate);
    }

    #[test]
    fn test_session_duration() {
        let mut tt = TableTurnoverTracker::new();

        // Seat guests.
        for _ in 0..SEATED_DEBOUNCE_FRAMES + 1 {
            tt.process_frame(1, 0.05, 2);
        }

        // Stay for 200 frames (10 seconds at 20 Hz).
        for _ in 0..200 {
            tt.process_frame(1, 0.03, 2);
        }

        let duration = tt.session_duration_s();
        assert!(duration > 9.0 && duration < 12.0,
            "session duration should be ~10s, got {}", duration);
    }

    #[test]
    fn test_negative_inputs() {
        let mut tt = TableTurnoverTracker::new();
        // Should not panic with negative inputs.
        let _events = tt.process_frame(-1, -0.5, -3);
        assert_eq!(tt.state(), TableState::Empty);
    }
}
