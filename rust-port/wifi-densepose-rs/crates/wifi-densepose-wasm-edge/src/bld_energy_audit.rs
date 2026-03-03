//! Energy audit — ADR-041 Category 3: Smart Building.
//!
//! Builds hourly occupancy histograms (24 bins/day, 7 days) for energy
//! optimization scheduling:
//! - Identifies consistently unoccupied hours for HVAC/lighting shutoff
//! - Detects after-hours occupancy anomalies
//! - Emits periodic schedule summaries
//!
//! Designed for the `on_timer`-style periodic emission pattern (every N frames).
//!
//! Host API used: `csi_get_presence()`, `csi_get_n_persons()`

/// Hours in a day.
const HOURS_PER_DAY: usize = 24;

/// Days in a week.
const DAYS_PER_WEEK: usize = 7;

/// Frames per hour at 20 Hz.
const FRAMES_PER_HOUR: u32 = 72000;

/// Summary emission interval (every 1200 frames = 1 minute at 20 Hz).
const SUMMARY_INTERVAL: u32 = 1200;

/// After-hours definition: hours 22-06 (10 PM to 6 AM).
const AFTER_HOURS_START: u8 = 22;
const AFTER_HOURS_END: u8 = 6;

/// Minimum occupancy fraction to consider an hour "used" in scheduling.
const USED_THRESHOLD: f32 = 0.1;

/// Frames of presence during after-hours before alert.
const AFTER_HOURS_ALERT_FRAMES: u32 = 600; // 30 seconds.

// ── Event IDs (350-352: Energy Audit) ───────────────────────────────────────

pub const EVENT_SCHEDULE_SUMMARY: i32 = 350;
pub const EVENT_AFTER_HOURS_ALERT: i32 = 351;
pub const EVENT_UTILIZATION_RATE: i32 = 352;

/// Per-hour occupancy accumulator.
#[derive(Clone, Copy)]
struct HourBin {
    /// Total frames observed in this hour slot.
    total_frames: u32,
    /// Frames with presence detected.
    occupied_frames: u32,
    /// Sum of person counts (for average headcount).
    person_sum: u32,
}

impl HourBin {
    const fn new() -> Self {
        Self {
            total_frames: 0,
            occupied_frames: 0,
            person_sum: 0,
        }
    }

    /// Occupancy rate for this hour (0.0-1.0).
    fn occupancy_rate(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.occupied_frames as f32 / self.total_frames as f32
    }

    /// Average headcount during occupied frames.
    fn avg_headcount(&self) -> f32 {
        if self.occupied_frames == 0 {
            return 0.0;
        }
        self.person_sum as f32 / self.occupied_frames as f32
    }
}

/// Energy audit analyzer.
pub struct EnergyAuditor {
    /// Weekly histogram: [day][hour].
    histogram: [[HourBin; HOURS_PER_DAY]; DAYS_PER_WEEK],
    /// Current simulated hour (0-23). In production, derived from host timestamp.
    current_hour: u8,
    /// Current simulated day (0-6).
    current_day: u8,
    /// Frames within the current hour.
    hour_frames: u32,
    /// Consecutive after-hours presence frames.
    after_hours_presence: u32,
    /// Total frames processed.
    frame_count: u32,
    /// Total occupied frames (for overall utilization).
    total_occupied_frames: u32,
}

impl EnergyAuditor {
    pub const fn new() -> Self {
        const BIN_INIT: HourBin = HourBin::new();
        const DAY_INIT: [HourBin; HOURS_PER_DAY] = [BIN_INIT; HOURS_PER_DAY];
        Self {
            histogram: [DAY_INIT; DAYS_PER_WEEK],
            current_hour: 8, // Default start: 8 AM.
            current_day: 0,  // Monday.
            hour_frames: 0,
            after_hours_presence: 0,
            frame_count: 0,
            total_occupied_frames: 0,
        }
    }

    /// Set the current time (called from host or on_init).
    pub fn set_time(&mut self, day: u8, hour: u8) {
        self.current_day = day % DAYS_PER_WEEK as u8;
        self.current_hour = hour % HOURS_PER_DAY as u8;
        self.hour_frames = 0;
    }

    /// Process one frame.
    ///
    /// `presence`: 1 if occupied, 0 if vacant.
    /// `n_persons`: person count from host.
    ///
    /// Returns events as `(event_type, value)` pairs.
    pub fn process_frame(
        &mut self,
        presence: i32,
        n_persons: i32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.hour_frames += 1;

        let is_present = presence > 0;
        let persons = if n_persons > 0 { n_persons as u32 } else { 0 };

        // Update histogram bin.
        let d = self.current_day as usize;
        let h = self.current_hour as usize;
        self.histogram[d][h].total_frames += 1;
        if is_present {
            self.histogram[d][h].occupied_frames += 1;
            self.histogram[d][h].person_sum += persons;
            self.total_occupied_frames += 1;
        }

        // Hour rollover.
        if self.hour_frames >= FRAMES_PER_HOUR {
            self.hour_frames = 0;
            self.current_hour += 1;
            if self.current_hour >= HOURS_PER_DAY as u8 {
                self.current_hour = 0;
                self.current_day = (self.current_day + 1) % DAYS_PER_WEEK as u8;
            }
        }

        // After-hours detection.
        let is_after_hours = self.is_after_hours(self.current_hour);
        if is_present && is_after_hours {
            self.after_hours_presence += 1;
        } else {
            self.after_hours_presence = 0;
        }

        // Build events.
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n_events = 0usize;

        // After-hours alert.
        if self.after_hours_presence >= AFTER_HOURS_ALERT_FRAMES && n_events < 3 {
            unsafe {
                EVENTS[n_events] = (EVENT_AFTER_HOURS_ALERT, self.current_hour as f32);
            }
            n_events += 1;
        }

        // Periodic summary.
        if self.frame_count % SUMMARY_INTERVAL == 0 {
            // Emit current hour's occupancy rate.
            let rate = self.histogram[d][h].occupancy_rate();
            if n_events < 3 {
                unsafe {
                    EVENTS[n_events] = (EVENT_SCHEDULE_SUMMARY, rate);
                }
                n_events += 1;
            }

            // Emit overall utilization rate.
            if n_events < 3 {
                let util = self.utilization_rate();
                unsafe {
                    EVENTS[n_events] = (EVENT_UTILIZATION_RATE, util);
                }
                n_events += 1;
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Check if a given hour is after-hours.
    fn is_after_hours(&self, hour: u8) -> bool {
        if AFTER_HOURS_START > AFTER_HOURS_END {
            // Wraps midnight (e.g., 22-06).
            hour >= AFTER_HOURS_START || hour < AFTER_HOURS_END
        } else {
            hour >= AFTER_HOURS_START && hour < AFTER_HOURS_END
        }
    }

    /// Get overall utilization rate.
    pub fn utilization_rate(&self) -> f32 {
        if self.frame_count == 0 {
            return 0.0;
        }
        self.total_occupied_frames as f32 / self.frame_count as f32
    }

    /// Get occupancy rate for a specific day and hour.
    pub fn hourly_rate(&self, day: usize, hour: usize) -> f32 {
        if day < DAYS_PER_WEEK && hour < HOURS_PER_DAY {
            self.histogram[day][hour].occupancy_rate()
        } else {
            0.0
        }
    }

    /// Get average headcount for a specific day and hour.
    pub fn hourly_headcount(&self, day: usize, hour: usize) -> f32 {
        if day < DAYS_PER_WEEK && hour < HOURS_PER_DAY {
            self.histogram[day][hour].avg_headcount()
        } else {
            0.0
        }
    }

    /// Find the number of consistently unoccupied hours per day.
    /// An hour is "unoccupied" if its occupancy rate is below USED_THRESHOLD.
    pub fn unoccupied_hours(&self, day: usize) -> u8 {
        if day >= DAYS_PER_WEEK {
            return 0;
        }
        let mut count = 0u8;
        for h in 0..HOURS_PER_DAY {
            if self.histogram[day][h].occupancy_rate() < USED_THRESHOLD {
                count += 1;
            }
        }
        count
    }

    /// Get current simulated time.
    pub fn current_time(&self) -> (u8, u8) {
        (self.current_day, self.current_hour)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_audit_init() {
        let ea = EnergyAuditor::new();
        assert!((ea.utilization_rate() - 0.0).abs() < 0.001);
        assert_eq!(ea.current_time(), (0, 8));
    }

    #[test]
    fn test_occupancy_recording() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(0, 9); // Monday 9 AM.

        // Feed 100 frames with presence.
        for _ in 0..100 {
            ea.process_frame(1, 3);
        }

        let rate = ea.hourly_rate(0, 9);
        assert!((rate - 1.0).abs() < 0.01, "fully occupied hour should be ~1.0");

        let headcount = ea.hourly_headcount(0, 9);
        assert!((headcount - 3.0).abs() < 0.01, "average headcount should be ~3.0");
    }

    #[test]
    fn test_partial_occupancy() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(1, 14); // Tuesday 2 PM.

        // 50 frames occupied, 50 vacant.
        for _ in 0..50 {
            ea.process_frame(1, 2);
        }
        for _ in 0..50 {
            ea.process_frame(0, 0);
        }

        let rate = ea.hourly_rate(1, 14);
        assert!((rate - 0.5).abs() < 0.01, "half-occupied hour should be ~0.5");
    }

    #[test]
    fn test_after_hours_alert() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(2, 23); // Wednesday 11 PM (after hours).

        let mut found_alert = false;
        for _ in 0..(AFTER_HOURS_ALERT_FRAMES + 10) {
            let events = ea.process_frame(1, 1);
            for &(et, _) in events {
                if et == EVENT_AFTER_HOURS_ALERT {
                    found_alert = true;
                }
            }
        }
        assert!(found_alert, "should emit AFTER_HOURS_ALERT for sustained after-hours presence");
    }

    #[test]
    fn test_no_after_hours_alert_during_business() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(0, 10); // Monday 10 AM (business hours).

        let mut found_alert = false;
        for _ in 0..2000 {
            let events = ea.process_frame(1, 5);
            for &(et, _) in events {
                if et == EVENT_AFTER_HOURS_ALERT {
                    found_alert = true;
                }
            }
        }
        assert!(!found_alert, "should NOT emit AFTER_HOURS_ALERT during business hours");
    }

    #[test]
    fn test_unoccupied_hours() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(3, 0); // Thursday midnight.

        // Only hour 0 gets data; hours 1-23 have no data and should count as unoccupied.
        for _ in 0..10 {
            ea.process_frame(0, 0);
        }

        // Hour 0 has data but 0% occupancy => all 24 hours unoccupied.
        let unoccupied = ea.unoccupied_hours(3);
        assert_eq!(unoccupied, 24, "all hours with no/low occupancy should be unoccupied");
    }

    #[test]
    fn test_periodic_summary_emission() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(0, 9);

        let mut found_summary = false;
        let mut found_utilization = false;

        for _ in 0..(SUMMARY_INTERVAL + 1) {
            let events = ea.process_frame(1, 2);
            for &(et, _) in events {
                if et == EVENT_SCHEDULE_SUMMARY {
                    found_summary = true;
                }
                if et == EVENT_UTILIZATION_RATE {
                    found_utilization = true;
                }
            }
        }
        assert!(found_summary, "should emit SCHEDULE_SUMMARY periodically");
        assert!(found_utilization, "should emit UTILIZATION_RATE periodically");
    }

    #[test]
    fn test_utilization_rate() {
        let mut ea = EnergyAuditor::new();
        ea.set_time(0, 9);

        // 100 frames occupied.
        for _ in 0..100 {
            ea.process_frame(1, 2);
        }
        // 100 frames vacant.
        for _ in 0..100 {
            ea.process_frame(0, 0);
        }

        let rate = ea.utilization_rate();
        assert!((rate - 0.5).abs() < 0.01, "50/50 occupancy should give ~0.5 utilization");
    }
}
