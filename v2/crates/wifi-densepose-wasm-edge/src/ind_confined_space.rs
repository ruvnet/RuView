//! Confined space monitoring — ADR-041 Category 5 Industrial module.
//!
//! Tracks worker presence and vital signs in confined spaces (tanks,
//! manholes, vessels) to satisfy OSHA confined space monitoring requirements.
//!
//! Features:
//! - Entry/exit detection via presence transitions
//! - Continuous breathing confirmation (proof of life)
//! - Emergency extraction alert if breathing ceases >15 s
//! - Immobile alert if all motion stops >60 s
//!
//! Budget: L (<2 ms per frame).  Event IDs 510-514.

/// Breathing cessation threshold (seconds at ~1 Hz timer or 20 Hz frame rate).
/// 15 seconds = 300 frames at 20 Hz.
const BREATHING_CEASE_FRAMES: u32 = 300;

/// Immobility threshold (seconds). 60 seconds = 1200 frames at 20 Hz.
const IMMOBILE_FRAMES: u32 = 1200;

/// Minimum breathing BPM to be considered "breathing".
const MIN_BREATHING_BPM: f32 = 4.0;

/// Minimum motion energy to be considered "moving".
const MIN_MOTION_ENERGY: f32 = 0.02;

/// Debounce frames for entry/exit detection.
const ENTRY_EXIT_DEBOUNCE: u8 = 10;

/// Breathing confirmation interval (frames, ~5 seconds at 20 Hz).
const BREATHING_REPORT_INTERVAL: u32 = 100;

/// Minimum variance to confirm human (not noise).
const MIN_PRESENCE_VAR: f32 = 0.005;

/// Event IDs (510-series: Industrial/Confined Space).
pub const EVENT_WORKER_ENTRY: i32 = 510;
pub const EVENT_WORKER_EXIT: i32 = 511;
pub const EVENT_BREATHING_OK: i32 = 512;
pub const EVENT_EXTRACTION_ALERT: i32 = 513;
pub const EVENT_IMMOBILE_ALERT: i32 = 514;

/// Worker state within the confined space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorkerState {
    /// No worker detected in the space.
    Empty,
    /// Worker present, vitals normal.
    Present,
    /// Worker present but no breathing detected (danger).
    BreathingCeased,
    /// Worker present but fully immobile (danger).
    Immobile,
}

/// Confined space monitor.
pub struct ConfinedSpaceMonitor {
    /// Current worker state.
    state: WorkerState,
    /// Presence debounce counters.
    present_count: u8,
    absent_count: u8,
    /// Whether a worker is detected (debounced).
    worker_inside: bool,
    /// Frames since last confirmed breathing.
    no_breathing_frames: u32,
    /// Frames since last detected motion.
    no_motion_frames: u32,
    /// Frame counter.
    frame_count: u32,
    /// Last reported breathing BPM.
    last_breathing_bpm: f32,
    /// Extraction alert already fired (prevent flooding).
    extraction_alerted: bool,
    /// Immobile alert already fired.
    immobile_alerted: bool,
}

impl ConfinedSpaceMonitor {
    pub const fn new() -> Self {
        Self {
            state: WorkerState::Empty,
            present_count: 0,
            absent_count: 0,
            worker_inside: false,
            no_breathing_frames: 0,
            no_motion_frames: 0,
            frame_count: 0,
            last_breathing_bpm: 0.0,
            extraction_alerted: false,
            immobile_alerted: false,
        }
    }

    /// Process one frame.
    ///
    /// # Arguments
    /// - `presence`: host-reported presence flag (0 or 1)
    /// - `breathing_bpm`: host-reported breathing rate
    /// - `motion_energy`: host-reported motion energy
    /// - `variance`: mean CSI variance (single value, pre-averaged by caller)
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        presence: i32,
        breathing_bpm: f32,
        motion_energy: f32,
        variance: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        // --- Step 1: Debounced presence detection ---
        let raw_present = presence > 0 && variance > MIN_PRESENCE_VAR;

        if raw_present {
            self.present_count = self.present_count.saturating_add(1);
            self.absent_count = 0;
        } else {
            self.absent_count = self.absent_count.saturating_add(1);
            self.present_count = 0;
        }

        let was_inside = self.worker_inside;

        if self.present_count >= ENTRY_EXIT_DEBOUNCE {
            self.worker_inside = true;
        }
        if self.absent_count >= ENTRY_EXIT_DEBOUNCE {
            self.worker_inside = false;
        }

        // Entry event.
        if self.worker_inside && !was_inside {
            self.state = WorkerState::Present;
            self.no_breathing_frames = 0;
            self.no_motion_frames = 0;
            self.extraction_alerted = false;
            self.immobile_alerted = false;
            if n_events < 4 {
                unsafe { EVENTS[n_events] = (EVENT_WORKER_ENTRY, 1.0); }
                n_events += 1;
            }
        }

        // Exit event.
        if !self.worker_inside && was_inside {
            self.state = WorkerState::Empty;
            if n_events < 4 {
                unsafe { EVENTS[n_events] = (EVENT_WORKER_EXIT, 1.0); }
                n_events += 1;
            }
        }

        // --- Step 2: Monitor vitals while worker is inside ---
        if self.worker_inside {
            // Check breathing.
            if breathing_bpm >= MIN_BREATHING_BPM {
                self.no_breathing_frames = 0;
                self.last_breathing_bpm = breathing_bpm;
                self.extraction_alerted = false;
                // Recover from BreathingCeased state when breathing resumes.
                if self.state == WorkerState::BreathingCeased {
                    self.state = WorkerState::Present;
                }

                // Periodic breathing confirmation.
                if self.frame_count % BREATHING_REPORT_INTERVAL == 0 && n_events < 4 {
                    unsafe { EVENTS[n_events] = (EVENT_BREATHING_OK, breathing_bpm); }
                    n_events += 1;
                }
            } else {
                self.no_breathing_frames += 1;
            }

            // Check motion.
            if motion_energy > MIN_MOTION_ENERGY {
                self.no_motion_frames = 0;
                self.immobile_alerted = false;
                // Recover from Immobile state when motion resumes.
                if self.state == WorkerState::Immobile {
                    self.state = WorkerState::Present;
                }
            } else {
                self.no_motion_frames += 1;
            }

            // --- Step 3: Emergency alerts ---
            // Extraction alert: no breathing for >15 seconds.
            if self.no_breathing_frames >= BREATHING_CEASE_FRAMES
                && !self.extraction_alerted
                && n_events < 4
            {
                self.state = WorkerState::BreathingCeased;
                self.extraction_alerted = true;
                let seconds = self.no_breathing_frames as f32 / 20.0;
                unsafe { EVENTS[n_events] = (EVENT_EXTRACTION_ALERT, seconds); }
                n_events += 1;
            }

            // Immobile alert: no motion for >60 seconds.
            if self.no_motion_frames >= IMMOBILE_FRAMES
                && !self.immobile_alerted
                && n_events < 4
            {
                self.state = WorkerState::Immobile;
                self.immobile_alerted = true;
                let seconds = self.no_motion_frames as f32 / 20.0;
                unsafe { EVENTS[n_events] = (EVENT_IMMOBILE_ALERT, seconds); }
                n_events += 1;
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Current worker state.
    pub fn state(&self) -> WorkerState {
        self.state
    }

    /// Whether a worker is currently inside the confined space.
    pub fn is_worker_inside(&self) -> bool {
        self.worker_inside
    }

    /// Seconds since last confirmed breathing (at 20 Hz frame rate).
    pub fn seconds_since_breathing(&self) -> f32 {
        self.no_breathing_frames as f32 / 20.0
    }

    /// Seconds since last detected motion (at 20 Hz frame rate).
    pub fn seconds_since_motion(&self) -> f32 {
        self.no_motion_frames as f32 / 20.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let mon = ConfinedSpaceMonitor::new();
        assert_eq!(mon.state(), WorkerState::Empty);
        assert!(!mon.is_worker_inside());
        assert_eq!(mon.frame_count, 0);
    }

    #[test]
    fn test_worker_entry() {
        let mut mon = ConfinedSpaceMonitor::new();
        let mut entry_detected = false;

        for _ in 0..20 {
            let events = mon.process_frame(1, 16.0, 0.5, 0.05);
            for &(et, _) in events {
                if et == EVENT_WORKER_ENTRY {
                    entry_detected = true;
                }
            }
        }

        assert!(entry_detected, "worker entry should be detected");
        assert!(mon.is_worker_inside());
        assert_eq!(mon.state(), WorkerState::Present);
    }

    #[test]
    fn test_worker_exit() {
        let mut mon = ConfinedSpaceMonitor::new();

        // First enter.
        for _ in 0..20 {
            mon.process_frame(1, 16.0, 0.5, 0.05);
        }
        assert!(mon.is_worker_inside());

        // Then leave.
        let mut exit_detected = false;
        for _ in 0..20 {
            let events = mon.process_frame(0, 0.0, 0.0, 0.001);
            for &(et, _) in events {
                if et == EVENT_WORKER_EXIT {
                    exit_detected = true;
                }
            }
        }

        assert!(exit_detected, "worker exit should be detected");
        assert!(!mon.is_worker_inside());
        assert_eq!(mon.state(), WorkerState::Empty);
    }

    #[test]
    fn test_breathing_ok_periodic() {
        let mut mon = ConfinedSpaceMonitor::new();
        let mut breathing_ok_count = 0u32;

        // Enter and maintain presence for 200 frames.
        for _ in 0..200 {
            let events = mon.process_frame(1, 16.0, 0.3, 0.05);
            for &(et, _) in events {
                if et == EVENT_BREATHING_OK {
                    breathing_ok_count += 1;
                }
            }
        }

        // At BREATHING_REPORT_INTERVAL=100, expect ~1-2 breathing OK reports.
        assert!(breathing_ok_count >= 1, "should get periodic breathing confirmations, got {}", breathing_ok_count);
    }

    #[test]
    fn test_extraction_alert_no_breathing() {
        let mut mon = ConfinedSpaceMonitor::new();

        // Enter with normal breathing.
        for _ in 0..20 {
            mon.process_frame(1, 16.0, 0.3, 0.05);
        }
        assert!(mon.is_worker_inside());

        // Stop breathing but maintain presence.
        let mut extraction_alert = false;
        for _ in 0..400 {
            let events = mon.process_frame(1, 0.0, 0.1, 0.05);
            for &(et, _) in events {
                if et == EVENT_EXTRACTION_ALERT {
                    extraction_alert = true;
                }
            }
        }

        assert!(extraction_alert, "extraction alert should fire after 15s of no breathing");
        assert_eq!(mon.state(), WorkerState::BreathingCeased);
    }

    #[test]
    fn test_immobile_alert() {
        let mut mon = ConfinedSpaceMonitor::new();

        // Enter with normal activity.
        for _ in 0..20 {
            mon.process_frame(1, 16.0, 0.3, 0.05);
        }

        // Stop all motion (but keep breathing to avoid extraction alert).
        let mut immobile_alert = false;
        for _ in 0..1300 {
            let events = mon.process_frame(1, 14.0, 0.001, 0.05);
            for &(et, _) in events {
                if et == EVENT_IMMOBILE_ALERT {
                    immobile_alert = true;
                }
            }
        }

        assert!(immobile_alert, "immobile alert should fire after 60s of no motion");
        assert_eq!(mon.state(), WorkerState::Immobile);
    }

    #[test]
    fn test_no_alert_when_empty() {
        let mut mon = ConfinedSpaceMonitor::new();

        for _ in 0..500 {
            let events = mon.process_frame(0, 0.0, 0.0, 0.001);
            for &(et, _) in events {
                assert!(
                    et != EVENT_EXTRACTION_ALERT && et != EVENT_IMMOBILE_ALERT,
                    "no emergency alerts when space is empty"
                );
            }
        }
    }
}
