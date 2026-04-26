//! Clean room monitoring — ADR-041 Category 5 Industrial module.
//!
//! Personnel count and movement tracking for cleanroom contamination control
//! per ISO 14644 standards.
//!
//! Features:
//! - Real-time occupancy count tracking
//! - Configurable maximum occupancy enforcement (default 4)
//! - Turbulent motion detection (rapid movement that disturbs laminar airflow)
//! - Periodic compliance reports
//!
//! Budget: L (<2 ms per frame).  Event IDs 520-523.

/// Default maximum allowed occupancy.
const DEFAULT_MAX_OCCUPANCY: u8 = 4;

/// Motion energy threshold for turbulent movement.
/// Normal cleanroom movement is slow and deliberate.
const TURBULENT_MOTION_THRESH: f32 = 0.6;

/// Debounce frames for occupancy violation.
const VIOLATION_DEBOUNCE: u8 = 10;

/// Debounce frames for turbulent motion.
const TURBULENT_DEBOUNCE: u8 = 3;

/// Compliance report interval (frames, ~30 seconds at 20 Hz).
const COMPLIANCE_REPORT_INTERVAL: u32 = 600;

/// Cooldown after occupancy violation alert (frames).
const VIOLATION_COOLDOWN: u16 = 200;

/// Cooldown after turbulent motion alert (frames).
const TURBULENT_COOLDOWN: u16 = 100;

/// Event IDs (520-series: Industrial/Clean Room).
pub const EVENT_OCCUPANCY_COUNT: i32 = 520;
pub const EVENT_OCCUPANCY_VIOLATION: i32 = 521;
pub const EVENT_TURBULENT_MOTION: i32 = 522;
pub const EVENT_COMPLIANCE_REPORT: i32 = 523;

/// Clean room monitor.
pub struct CleanRoomMonitor {
    /// Maximum allowed occupancy.
    max_occupancy: u8,
    /// Current smoothed person count.
    current_count: u8,
    /// Previous reported count (for change detection).
    prev_count: u8,
    /// Occupancy violation debounce counter.
    violation_debounce: u8,
    /// Turbulent motion debounce counter.
    turbulent_debounce: u8,
    /// Violation cooldown.
    violation_cooldown: u16,
    /// Turbulent cooldown.
    turbulent_cooldown: u16,
    /// Frame counter.
    frame_count: u32,
    /// Frames in compliance (occupancy <= max).
    compliant_frames: u32,
    /// Total frames while room is occupied.
    occupied_frames: u32,
    /// Total violation events.
    total_violations: u32,
    /// Total turbulent events.
    total_turbulent: u32,
}

impl CleanRoomMonitor {
    pub const fn new() -> Self {
        Self {
            max_occupancy: DEFAULT_MAX_OCCUPANCY,
            current_count: 0,
            prev_count: 0,
            violation_debounce: 0,
            turbulent_debounce: 0,
            violation_cooldown: 0,
            turbulent_cooldown: 0,
            frame_count: 0,
            compliant_frames: 0,
            occupied_frames: 0,
            total_violations: 0,
            total_turbulent: 0,
        }
    }

    /// Create with custom maximum occupancy.
    pub const fn with_max_occupancy(max: u8) -> Self {
        Self {
            max_occupancy: max,
            current_count: 0,
            prev_count: 0,
            violation_debounce: 0,
            turbulent_debounce: 0,
            violation_cooldown: 0,
            turbulent_cooldown: 0,
            frame_count: 0,
            compliant_frames: 0,
            occupied_frames: 0,
            total_violations: 0,
            total_turbulent: 0,
        }
    }

    /// Process one frame.
    ///
    /// # Arguments
    /// - `n_persons`: host-reported person count
    /// - `presence`: host-reported presence flag (0/1)
    /// - `motion_energy`: host-reported motion energy
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        n_persons: i32,
        presence: i32,
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        if self.violation_cooldown > 0 {
            self.violation_cooldown -= 1;
        }
        if self.turbulent_cooldown > 0 {
            self.turbulent_cooldown -= 1;
        }

        // Clamp person count to reasonable range.
        let count = if n_persons < 0 {
            0u8
        } else if n_persons > 255 {
            255u8
        } else {
            n_persons as u8
        };

        self.prev_count = self.current_count;
        self.current_count = count;

        // Track compliance.
        if count > 0 {
            self.occupied_frames += 1;
            if count <= self.max_occupancy {
                self.compliant_frames += 1;
            }
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        // --- Step 1: Emit count changes ---
        if count != self.prev_count && n_events < 4 {
            unsafe { EVENTS[n_events] = (EVENT_OCCUPANCY_COUNT, count as f32); }
            n_events += 1;
        }

        // --- Step 2: Occupancy violation ---
        if count > self.max_occupancy {
            self.violation_debounce = self.violation_debounce.saturating_add(1);
            if self.violation_debounce >= VIOLATION_DEBOUNCE
                && self.violation_cooldown == 0
                && n_events < 4
            {
                self.total_violations += 1;
                self.violation_cooldown = VIOLATION_COOLDOWN;
                // Value encodes: count * 10 + max_allowed.
                let val = count as f32;
                unsafe { EVENTS[n_events] = (EVENT_OCCUPANCY_VIOLATION, val); }
                n_events += 1;
            }
        } else {
            self.violation_debounce = 0;
        }

        // --- Step 3: Turbulent motion detection ---
        if motion_energy > TURBULENT_MOTION_THRESH && presence > 0 {
            self.turbulent_debounce = self.turbulent_debounce.saturating_add(1);
            if self.turbulent_debounce >= TURBULENT_DEBOUNCE
                && self.turbulent_cooldown == 0
                && n_events < 4
            {
                self.total_turbulent += 1;
                self.turbulent_cooldown = TURBULENT_COOLDOWN;
                unsafe { EVENTS[n_events] = (EVENT_TURBULENT_MOTION, motion_energy); }
                n_events += 1;
            }
        } else {
            self.turbulent_debounce = 0;
        }

        // --- Step 4: Periodic compliance report ---
        if self.frame_count % COMPLIANCE_REPORT_INTERVAL == 0 && n_events < 4 {
            let compliance_pct = if self.occupied_frames > 0 {
                (self.compliant_frames as f32 / self.occupied_frames as f32) * 100.0
            } else {
                100.0
            };
            unsafe { EVENTS[n_events] = (EVENT_COMPLIANCE_REPORT, compliance_pct); }
            n_events += 1;
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Current occupancy count.
    pub fn current_count(&self) -> u8 {
        self.current_count
    }

    /// Maximum allowed occupancy.
    pub fn max_occupancy(&self) -> u8 {
        self.max_occupancy
    }

    /// Whether currently in violation.
    pub fn is_in_violation(&self) -> bool {
        self.current_count > self.max_occupancy
    }

    /// Compliance percentage (0-100).
    pub fn compliance_percent(&self) -> f32 {
        if self.occupied_frames == 0 {
            return 100.0;
        }
        (self.compliant_frames as f32 / self.occupied_frames as f32) * 100.0
    }

    /// Total number of violation events.
    pub fn total_violations(&self) -> u32 {
        self.total_violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let mon = CleanRoomMonitor::new();
        assert_eq!(mon.current_count(), 0);
        assert_eq!(mon.max_occupancy(), DEFAULT_MAX_OCCUPANCY);
        assert!(!mon.is_in_violation());
        assert!((mon.compliance_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_custom_max_occupancy() {
        let mon = CleanRoomMonitor::with_max_occupancy(2);
        assert_eq!(mon.max_occupancy(), 2);
    }

    #[test]
    fn test_occupancy_count_change() {
        let mut mon = CleanRoomMonitor::new();

        // First frame with 2 persons.
        let events = mon.process_frame(2, 1, 0.1);
        let mut count_event = false;
        for &(et, val) in events {
            if et == EVENT_OCCUPANCY_COUNT {
                count_event = true;
                assert!((val - 2.0).abs() < 0.01);
            }
        }
        assert!(count_event, "should emit count change event");
        assert_eq!(mon.current_count(), 2);
    }

    #[test]
    fn test_occupancy_violation() {
        let mut mon = CleanRoomMonitor::with_max_occupancy(3);
        let mut violation_detected = false;

        // Feed frames with 5 persons (over limit of 3).
        for _ in 0..20 {
            let events = mon.process_frame(5, 1, 0.1);
            for &(et, _) in events {
                if et == EVENT_OCCUPANCY_VIOLATION {
                    violation_detected = true;
                }
            }
        }

        assert!(violation_detected, "violation should be detected when over max");
        assert!(mon.is_in_violation());
        assert!(mon.total_violations() >= 1);
    }

    #[test]
    fn test_no_violation_under_limit() {
        let mut mon = CleanRoomMonitor::with_max_occupancy(4);

        for _ in 0..50 {
            let events = mon.process_frame(3, 1, 0.1);
            for &(et, _) in events {
                assert!(et != EVENT_OCCUPANCY_VIOLATION, "no violation when under limit");
            }
        }
        assert!(!mon.is_in_violation());
    }

    #[test]
    fn test_turbulent_motion() {
        let mut mon = CleanRoomMonitor::new();
        let mut turbulent_detected = false;

        // Feed frames with high motion energy.
        for _ in 0..10 {
            let events = mon.process_frame(2, 1, 0.8);
            for &(et, val) in events {
                if et == EVENT_TURBULENT_MOTION {
                    turbulent_detected = true;
                    assert!(val > TURBULENT_MOTION_THRESH);
                }
            }
        }

        assert!(turbulent_detected, "turbulent motion should be detected");
    }

    #[test]
    fn test_compliance_report() {
        let mut mon = CleanRoomMonitor::with_max_occupancy(4);
        let mut compliance_reported = false;

        // Run for COMPLIANCE_REPORT_INTERVAL frames.
        for _ in 0..COMPLIANCE_REPORT_INTERVAL + 1 {
            let events = mon.process_frame(3, 1, 0.1);
            for &(et, val) in events {
                if et == EVENT_COMPLIANCE_REPORT {
                    compliance_reported = true;
                    assert!((val - 100.0).abs() < 0.01, "should be 100% compliant");
                }
            }
        }

        assert!(compliance_reported, "compliance report should be emitted periodically");
    }

    #[test]
    fn test_compliance_degrades_with_violations() {
        let mut mon = CleanRoomMonitor::with_max_occupancy(2);

        // 50 frames compliant.
        for _ in 0..50 {
            mon.process_frame(1, 1, 0.1);
        }
        // 50 frames in violation.
        for _ in 0..50 {
            mon.process_frame(5, 1, 0.1);
        }

        let pct = mon.compliance_percent();
        assert!(pct < 100.0 && pct > 0.0, "compliance should be partial, got {}%", pct);
        assert!((pct - 50.0).abs() < 1.0, "expect ~50% compliance, got {}%", pct);
    }
}
