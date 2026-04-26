//! Meeting room state tracking — ADR-041 Category 3: Smart Building.
//!
//! State machine for meeting room lifecycle:
//!   Empty -> PreMeeting -> Active -> PostMeeting -> Empty
//!
//! Distinguishes genuine meetings (multi-person, >5 min) from transient
//! occupancy (brief walk-through, single person using the room).
//!
//! Tracks meeting start/end, peak headcount, and utilization rate.
//!
//! Host API used: `csi_get_presence()`, `csi_get_n_persons()`,
//!                `csi_get_motion_energy()`

// No sqrt needed — pure arithmetic and comparisons.

/// Minimum frames for a genuine meeting (5 min at 20 Hz = 6000 frames).
const MEETING_MIN_FRAMES: u32 = 6000;

/// Minimum persons to qualify as a meeting (vs solo use).
const MEETING_MIN_PERSONS: u8 = 2;

/// Pre-meeting timeout: if not enough people join within 3 min (3600 frames),
/// revert to Empty.
const PRE_MEETING_TIMEOUT: u32 = 3600;

/// Post-meeting timeout: room goes Empty after 2 min (2400 frames) of vacancy.
const POST_MEETING_TIMEOUT: u32 = 2400;

/// Presence threshold (from host 0/1 signal).
const PRESENCE_THRESHOLD: i32 = 1;

/// Event emission interval.
const EMIT_INTERVAL: u32 = 20;

// ── Event IDs (340-343: Meeting Room) ───────────────────────────────────────

pub const EVENT_MEETING_START: i32 = 340;
pub const EVENT_MEETING_END: i32 = 341;
pub const EVENT_PEAK_HEADCOUNT: i32 = 342;
pub const EVENT_ROOM_AVAILABLE: i32 = 343;

/// Meeting room state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeetingState {
    /// Room is unoccupied and available.
    Empty,
    /// Someone entered; waiting to see if a meeting materializes.
    PreMeeting,
    /// Genuine meeting in progress (multi-person, sustained).
    Active,
    /// Meeting ended; clearing period before marking room available.
    PostMeeting,
}

/// Meeting room tracker.
pub struct MeetingRoomTracker {
    state: MeetingState,
    /// Frames in current state.
    state_frames: u32,
    /// Current person count from host.
    n_persons: u8,
    /// Peak headcount during current/last meeting.
    peak_headcount: u8,
    /// Frames where person count was >= MEETING_MIN_PERSONS.
    multi_person_frames: u32,
    /// Total meeting count.
    meeting_count: u32,
    /// Total meeting frames (for utilization calculation).
    total_meeting_frames: u32,
    /// Total frames tracked (for utilization calculation).
    total_frames: u32,
    /// Frame counter.
    frame_count: u32,
}

impl MeetingRoomTracker {
    pub const fn new() -> Self {
        Self {
            state: MeetingState::Empty,
            state_frames: 0,
            n_persons: 0,
            peak_headcount: 0,
            multi_person_frames: 0,
            meeting_count: 0,
            total_meeting_frames: 0,
            total_frames: 0,
            frame_count: 0,
        }
    }

    /// Process one frame.
    ///
    /// `presence`: presence indicator from host (0 or 1).
    /// `n_persons`: person count from host.
    /// `motion_energy`: motion energy from host.
    ///
    /// Returns events as `(event_type, value)` pairs.
    pub fn process_frame(
        &mut self,
        presence: i32,
        n_persons: i32,
        _motion_energy: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.total_frames += 1;
        self.state_frames += 1;

        let is_present = presence >= PRESENCE_THRESHOLD;
        self.n_persons = if n_persons > 0 { n_persons as u8 } else { 0 };

        if self.n_persons > self.peak_headcount {
            self.peak_headcount = self.n_persons;
        }

        if self.n_persons >= MEETING_MIN_PERSONS {
            self.multi_person_frames += 1;
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        let _prev_state = self.state;

        match self.state {
            MeetingState::Empty => {
                if is_present {
                    self.state = MeetingState::PreMeeting;
                    self.state_frames = 0;
                    self.peak_headcount = self.n_persons;
                    self.multi_person_frames = 0;
                }
            }

            MeetingState::PreMeeting => {
                if !is_present {
                    // Person left before meeting started.
                    self.state = MeetingState::Empty;
                    self.state_frames = 0;
                    self.peak_headcount = 0;
                } else if self.n_persons >= MEETING_MIN_PERSONS
                    && self.state_frames >= 60 // At least 3 seconds of multi-person.
                {
                    // Enough people gathered, transition to Active.
                    self.state = MeetingState::Active;
                    self.state_frames = 0;
                    self.meeting_count += 1;

                    if n_events < 4 {
                        unsafe {
                            EVENTS[n_events] = (EVENT_MEETING_START, self.n_persons as f32);
                        }
                        n_events += 1;
                    }
                } else if self.state_frames >= PRE_MEETING_TIMEOUT {
                    // Timeout: single person using room, not a meeting.
                    // Stay as-is but don't promote to Active.
                    // If they leave, we go back to Empty.
                    // (Solo room use is not tracked as a "meeting".)
                    if !is_present {
                        self.state = MeetingState::Empty;
                        self.state_frames = 0;
                        self.peak_headcount = 0;
                    }
                }
            }

            MeetingState::Active => {
                self.total_meeting_frames += 1;

                if !is_present || self.n_persons == 0 {
                    // Everyone left.
                    self.state = MeetingState::PostMeeting;
                    self.state_frames = 0;

                    // Emit meeting end with duration.
                    let duration_mins = self.total_meeting_frames as f32 / (20.0 * 60.0);
                    if n_events < 4 {
                        unsafe {
                            EVENTS[n_events] = (EVENT_MEETING_END, duration_mins);
                        }
                        n_events += 1;
                    }

                    // Emit peak headcount.
                    if n_events < 4 {
                        unsafe {
                            EVENTS[n_events] = (EVENT_PEAK_HEADCOUNT, self.peak_headcount as f32);
                        }
                        n_events += 1;
                    }
                }
            }

            MeetingState::PostMeeting => {
                if is_present && self.n_persons >= MEETING_MIN_PERSONS {
                    // People came back, resume meeting.
                    self.state = MeetingState::Active;
                    self.state_frames = 0;
                } else if self.state_frames >= POST_MEETING_TIMEOUT || !is_present {
                    // Room cleared.
                    self.state = MeetingState::Empty;
                    self.state_frames = 0;
                    self.peak_headcount = 0;
                    self.multi_person_frames = 0;

                    if n_events < 4 {
                        unsafe {
                            EVENTS[n_events] = (EVENT_ROOM_AVAILABLE, 1.0);
                        }
                        n_events += 1;
                    }
                }
            }
        }

        // Periodic status emission.
        if self.frame_count % EMIT_INTERVAL == 0 && self.state == MeetingState::Active {
            if n_events < 4 {
                unsafe {
                    EVENTS[n_events] = (EVENT_PEAK_HEADCOUNT, self.peak_headcount as f32);
                }
                n_events += 1;
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Get current meeting room state.
    pub fn state(&self) -> MeetingState {
        self.state
    }

    /// Get peak headcount for current/last meeting.
    pub fn peak_headcount(&self) -> u8 {
        self.peak_headcount
    }

    /// Get total meeting count.
    pub fn meeting_count(&self) -> u32 {
        self.meeting_count
    }

    /// Get utilization rate (fraction of total time spent in meetings).
    pub fn utilization_rate(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.total_meeting_frames as f32 / self.total_frames as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meeting_room_init() {
        let mt = MeetingRoomTracker::new();
        assert_eq!(mt.state(), MeetingState::Empty);
        assert_eq!(mt.peak_headcount(), 0);
        assert_eq!(mt.meeting_count(), 0);
        assert!((mt.utilization_rate() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_empty_to_pre_meeting() {
        let mut mt = MeetingRoomTracker::new();

        // Single person enters.
        mt.process_frame(1, 1, 0.1);
        assert_eq!(mt.state(), MeetingState::PreMeeting);
    }

    #[test]
    fn test_pre_meeting_to_active() {
        let mut mt = MeetingRoomTracker::new();

        // Multiple people enter and stay.
        for _ in 0..100 {
            mt.process_frame(1, 3, 0.2);
        }
        assert_eq!(mt.state(), MeetingState::Active);
        assert!(mt.meeting_count() >= 1);
    }

    #[test]
    fn test_meeting_end_and_room_available() {
        let mut mt = MeetingRoomTracker::new();

        // Start meeting.
        for _ in 0..100 {
            mt.process_frame(1, 4, 0.3);
        }
        assert_eq!(mt.state(), MeetingState::Active);

        // Everyone leaves.
        mt.process_frame(0, 0, 0.0);
        assert_eq!(mt.state(), MeetingState::PostMeeting);

        // Wait for post-meeting timeout.
        let mut found_available = false;
        for _ in 0..POST_MEETING_TIMEOUT + 1 {
            let events = mt.process_frame(0, 0, 0.0);
            for &(et, _) in events {
                if et == EVENT_ROOM_AVAILABLE {
                    found_available = true;
                }
            }
        }
        assert_eq!(mt.state(), MeetingState::Empty);
        assert!(found_available, "should emit ROOM_AVAILABLE after clearing");
    }

    #[test]
    fn test_transient_occupancy_not_meeting() {
        let mut mt = MeetingRoomTracker::new();

        // Single person enters briefly.
        for _ in 0..30 {
            mt.process_frame(1, 1, 0.1);
        }
        // Leaves.
        mt.process_frame(0, 0, 0.0);

        assert_eq!(mt.state(), MeetingState::Empty);
        assert_eq!(mt.meeting_count(), 0, "brief single-person visit is not a meeting");
    }

    #[test]
    fn test_peak_headcount_tracked() {
        let mut mt = MeetingRoomTracker::new();

        // Start meeting with 2 people.
        for _ in 0..100 {
            mt.process_frame(1, 2, 0.2);
        }
        assert_eq!(mt.state(), MeetingState::Active);

        // More people join.
        for _ in 0..50 {
            mt.process_frame(1, 6, 0.3);
        }
        assert_eq!(mt.peak_headcount(), 6);

        // Some leave.
        for _ in 0..50 {
            mt.process_frame(1, 3, 0.2);
        }
        // Peak should remain at 6.
        assert_eq!(mt.peak_headcount(), 6);
    }

    #[test]
    fn test_meeting_events_emitted() {
        let mut mt = MeetingRoomTracker::new();

        let mut found_start = false;
        let mut found_end = false;

        // Start meeting.
        for _ in 0..100 {
            let events = mt.process_frame(1, 3, 0.2);
            for &(et, _) in events {
                if et == EVENT_MEETING_START {
                    found_start = true;
                }
            }
        }
        assert!(found_start, "should emit MEETING_START");

        // End meeting.
        for _ in 0..10 {
            let events = mt.process_frame(0, 0, 0.0);
            for &(et, _) in events {
                if et == EVENT_MEETING_END {
                    found_end = true;
                }
            }
        }
        assert!(found_end, "should emit MEETING_END");
    }

    #[test]
    fn test_utilization_rate() {
        let mut mt = MeetingRoomTracker::new();

        // 100 frames of meeting.
        for _ in 0..100 {
            mt.process_frame(1, 3, 0.2);
        }

        // 100 frames of empty.
        for _ in 0..100 {
            mt.process_frame(0, 0, 0.0);
        }

        let rate = mt.utilization_rate();
        // Meeting was active for some of the 200 frames.
        assert!(rate > 0.0, "utilization rate should be positive after a meeting");
        assert!(rate < 1.0, "utilization rate should be less than 1.0");
    }
}
