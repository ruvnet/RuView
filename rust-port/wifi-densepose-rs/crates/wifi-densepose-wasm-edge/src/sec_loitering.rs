//! Loitering detection — ADR-041 Category 2 Security module.
//!
//! Detects prolonged stationary presence beyond a configurable dwell threshold.
//! Uses a four-state machine: Absent -> Entering -> Present -> Loitering.
//! Includes a cooldown on the Loitering -> Absent transition to prevent
//! flapping from brief occlusions.
//!
//! Default thresholds (at 20 Hz frame rate):
//! - Dwell threshold: 5 minutes = 6000 frames
//! - Entering confirmation: 3 seconds = 60 frames
//! - Cooldown on exit: 30 seconds = 600 frames
//! - Motion energy below which presence is "stationary": 0.5
//!
//! Events: LOITERING_START(240), LOITERING_ONGOING(241), LOITERING_END(242).
//! Budget: L (<2 ms).

/// Frames of continuous presence before entering -> present (3 seconds at 20 Hz).
const ENTER_CONFIRM_FRAMES: u32 = 60;
/// Frames of presence before loitering alert (5 minutes at 20 Hz).
const DWELL_THRESHOLD: u32 = 6000;
/// Cooldown frames before loitering -> absent (30 seconds at 20 Hz).
const EXIT_COOLDOWN: u32 = 600;
/// Motion energy threshold: below this the person is considered stationary.
const STATIONARY_MOTION_THRESH: f32 = 0.5;
/// Frames between ongoing loitering reports (every 30 seconds).
const ONGOING_REPORT_INTERVAL: u32 = 600;
/// Cooldown after loitering_end before re-detecting.
const POST_END_COOLDOWN: u32 = 200;

pub const EVENT_LOITERING_START: i32 = 240;
pub const EVENT_LOITERING_ONGOING: i32 = 241;
pub const EVENT_LOITERING_END: i32 = 242;

/// Loitering state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoiterState {
    /// No one present.
    Absent,
    /// Someone detected, confirming presence.
    Entering,
    /// Person present, counting dwell time.
    Present,
    /// Dwell threshold exceeded — loitering.
    Loitering,
}

/// Loitering detector.
pub struct LoiteringDetector {
    state: LoiterState,
    /// Consecutive frames with presence detected.
    presence_frames: u32,
    /// Total dwell frames since entering Present state.
    dwell_frames: u32,
    /// Consecutive frames without presence (for exit cooldown).
    absent_frames: u32,
    /// Frame counter for ongoing report interval.
    ongoing_timer: u32,
    /// Post-end cooldown counter.
    post_end_cd: u32,
    frame_count: u32,
    /// Total loitering events.
    loiter_count: u32,
}

impl LoiteringDetector {
    pub const fn new() -> Self {
        Self {
            state: LoiterState::Absent,
            presence_frames: 0,
            dwell_frames: 0,
            absent_frames: 0,
            ongoing_timer: 0,
            post_end_cd: 0,
            frame_count: 0,
            loiter_count: 0,
        }
    }

    /// Process one frame. Returns `(event_id, value)` pairs.
    ///
    /// `presence`: host presence flag (0 = empty, 1+ = present).
    /// `motion_energy`: host motion energy value.
    pub fn process_frame(
        &mut self,
        presence: i32,
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.post_end_cd = self.post_end_cd.saturating_sub(1);

        static mut EVENTS: [(i32, f32); 2] = [(0, 0.0); 2];
        let mut ne = 0usize;

        // Determine if someone is present and roughly stationary.
        let is_present = presence > 0;
        let is_stationary = motion_energy < STATIONARY_MOTION_THRESH;

        match self.state {
            LoiterState::Absent => {
                if is_present && self.post_end_cd == 0 {
                    self.state = LoiterState::Entering;
                    self.presence_frames = 1;
                    self.absent_frames = 0;
                }
            }

            LoiterState::Entering => {
                if is_present {
                    self.presence_frames += 1;
                    if self.presence_frames >= ENTER_CONFIRM_FRAMES {
                        self.state = LoiterState::Present;
                        self.dwell_frames = 0;
                    }
                } else {
                    // Person left before confirmation.
                    self.state = LoiterState::Absent;
                    self.presence_frames = 0;
                }
            }

            LoiterState::Present => {
                if is_present {
                    self.absent_frames = 0;
                    // Only count stationary frames toward dwell.
                    if is_stationary {
                        self.dwell_frames += 1;
                    }

                    if self.dwell_frames >= DWELL_THRESHOLD {
                        self.state = LoiterState::Loitering;
                        self.loiter_count += 1;
                        self.ongoing_timer = 0;

                        if ne < 2 {
                            let dwell_seconds = self.dwell_frames as f32 / 20.0;
                            unsafe {
                                EVENTS[ne] = (EVENT_LOITERING_START, dwell_seconds);
                            }
                            ne += 1;
                        }
                    }
                } else {
                    self.absent_frames += 1;
                    // If person leaves during present phase, go to absent.
                    if self.absent_frames >= EXIT_COOLDOWN / 2 {
                        self.state = LoiterState::Absent;
                        self.dwell_frames = 0;
                        self.absent_frames = 0;
                    }
                }
            }

            LoiterState::Loitering => {
                if is_present {
                    self.absent_frames = 0;
                    self.dwell_frames += 1;
                    self.ongoing_timer += 1;

                    // Periodic ongoing report.
                    if self.ongoing_timer >= ONGOING_REPORT_INTERVAL {
                        self.ongoing_timer = 0;
                        if ne < 2 {
                            let total_seconds = self.dwell_frames as f32 / 20.0;
                            unsafe {
                                EVENTS[ne] = (EVENT_LOITERING_ONGOING, total_seconds);
                            }
                            ne += 1;
                        }
                    }
                } else {
                    self.absent_frames += 1;

                    // Exit cooldown: require sustained absence before ending loitering.
                    if self.absent_frames >= EXIT_COOLDOWN {
                        self.state = LoiterState::Absent;
                        self.post_end_cd = POST_END_COOLDOWN;

                        if ne < 2 {
                            let total_seconds = self.dwell_frames as f32 / 20.0;
                            unsafe {
                                EVENTS[ne] = (EVENT_LOITERING_END, total_seconds);
                            }
                            ne += 1;
                        }

                        self.dwell_frames = 0;
                        self.absent_frames = 0;
                        self.ongoing_timer = 0;
                    }
                }
            }
        }

        unsafe { &EVENTS[..ne] }
    }

    pub fn state(&self) -> LoiterState { self.state }
    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn loiter_count(&self) -> u32 { self.loiter_count }
    pub fn dwell_frames(&self) -> u32 { self.dwell_frames }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let det = LoiteringDetector::new();
        assert_eq!(det.state(), LoiterState::Absent);
        assert_eq!(det.frame_count(), 0);
        assert_eq!(det.loiter_count(), 0);
    }

    #[test]
    fn test_entering_confirmation() {
        let mut det = LoiteringDetector::new();

        // Feed presence for less than confirmation threshold.
        for _ in 0..(ENTER_CONFIRM_FRAMES - 1) {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Entering);

        // One more frame should confirm.
        det.process_frame(1, 0.2);
        assert_eq!(det.state(), LoiterState::Present);
    }

    #[test]
    fn test_entering_cancelled_on_absence() {
        let mut det = LoiteringDetector::new();

        // Start entering.
        for _ in 0..30 {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Entering);

        // Person leaves.
        det.process_frame(0, 0.0);
        assert_eq!(det.state(), LoiterState::Absent);
    }

    #[test]
    fn test_loitering_start_event() {
        let mut det = LoiteringDetector::new();

        // Confirm presence.
        for _ in 0..ENTER_CONFIRM_FRAMES {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Present);

        // Dwell until threshold.
        let mut found_start = false;
        for _ in 0..(DWELL_THRESHOLD + 1) {
            let ev = det.process_frame(1, 0.2);
            for &(et, _) in ev {
                if et == EVENT_LOITERING_START {
                    found_start = true;
                }
            }
        }
        assert!(found_start, "loitering start should fire after dwell threshold");
        assert_eq!(det.state(), LoiterState::Loitering);
        assert_eq!(det.loiter_count(), 1);
    }

    #[test]
    fn test_loitering_ongoing_report() {
        let mut det = LoiteringDetector::new();

        // Enter + confirm + dwell.
        for _ in 0..ENTER_CONFIRM_FRAMES {
            det.process_frame(1, 0.2);
        }
        for _ in 0..(DWELL_THRESHOLD + 1) {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Loitering);

        // Continue loitering for a reporting interval.
        let mut found_ongoing = false;
        for _ in 0..(ONGOING_REPORT_INTERVAL + 1) {
            let ev = det.process_frame(1, 0.2);
            for &(et, _) in ev {
                if et == EVENT_LOITERING_ONGOING {
                    found_ongoing = true;
                }
            }
        }
        assert!(found_ongoing, "loitering ongoing should fire periodically");
    }

    #[test]
    fn test_loitering_end_with_cooldown() {
        let mut det = LoiteringDetector::new();

        // Enter + confirm + dwell into loitering.
        for _ in 0..ENTER_CONFIRM_FRAMES {
            det.process_frame(1, 0.2);
        }
        for _ in 0..(DWELL_THRESHOLD + 1) {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Loitering);

        // Person leaves — needs EXIT_COOLDOWN frames of absence to end.
        let mut found_end = false;
        for _ in 0..(EXIT_COOLDOWN + 1) {
            let ev = det.process_frame(0, 0.0);
            for &(et, v) in ev {
                if et == EVENT_LOITERING_END {
                    found_end = true;
                    assert!(v > 0.0, "end event should report dwell time");
                }
            }
        }
        assert!(found_end, "loitering end should fire after exit cooldown");
        assert_eq!(det.state(), LoiterState::Absent);
    }

    #[test]
    fn test_brief_absence_does_not_end_loitering() {
        let mut det = LoiteringDetector::new();

        // Enter + confirm + dwell into loitering.
        for _ in 0..ENTER_CONFIRM_FRAMES {
            det.process_frame(1, 0.2);
        }
        for _ in 0..(DWELL_THRESHOLD + 1) {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Loitering);

        // Brief absence (less than cooldown).
        for _ in 0..50 {
            det.process_frame(0, 0.0);
        }
        // Person returns.
        det.process_frame(1, 0.2);
        assert_eq!(det.state(), LoiterState::Loitering, "brief absence should not end loitering");
    }

    #[test]
    fn test_moving_person_does_not_accumulate_dwell() {
        let mut det = LoiteringDetector::new();

        // Confirm presence.
        for _ in 0..ENTER_CONFIRM_FRAMES {
            det.process_frame(1, 0.2);
        }
        assert_eq!(det.state(), LoiterState::Present);

        // Person is present but moving (high motion energy).
        for _ in 0..1000 {
            det.process_frame(1, 5.0); // Above STATIONARY_MOTION_THRESH.
        }
        // Should still be in Present, not Loitering, because motion is high.
        assert_eq!(det.state(), LoiterState::Present);
        assert!(det.dwell_frames() < DWELL_THRESHOLD,
            "moving person should not accumulate dwell frames quickly");
    }
}
