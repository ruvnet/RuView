//! Occupancy zone detection — ADR-041 Phase 1 module.
//!
//! Divides the sensing area into spatial zones and detects which zones
//! are occupied based on per-subcarrier amplitude/variance patterns.
//!
//! Each subcarrier group maps to a spatial zone (Fresnel zone geometry).
//! Occupied zones emit events with zone ID and confidence score.

use libm::fabsf;

/// Maximum number of zones (limited by subcarrier count).
const MAX_ZONES: usize = 8;

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// Minimum variance change to consider a zone occupied.
const ZONE_THRESHOLD: f32 = 0.02;

/// EMA smoothing factor for zone scores.
const ALPHA: f32 = 0.15;

/// Number of frames for baseline calibration.
const BASELINE_FRAMES: u32 = 200;

/// Event type for occupancy zone detection (300-series: Smart Building).
pub const EVENT_ZONE_OCCUPIED: i32 = 300;
pub const EVENT_ZONE_COUNT: i32 = 301;
pub const EVENT_ZONE_TRANSITION: i32 = 302;

/// Per-zone state.
struct ZoneState {
    /// Baseline mean variance (calibrated from ambient).
    baseline_var: f32,
    /// Current EMA-smoothed zone score.
    score: f32,
    /// Whether this zone is currently occupied.
    occupied: bool,
    /// Previous occupied state (for transition detection).
    prev_occupied: bool,
}

/// Occupancy zone detector.
pub struct OccupancyDetector {
    zones: [ZoneState; MAX_ZONES],
    n_zones: usize,
    /// Calibration accumulators.
    calib_sum: [f32; MAX_ZONES],
    calib_count: u32,
    calibrated: bool,
    /// Frame counter.
    frame_count: u32,
}

impl OccupancyDetector {
    pub const fn new() -> Self {
        const ZONE_INIT: ZoneState = ZoneState {
            baseline_var: 0.0,
            score: 0.0,
            occupied: false,
            prev_occupied: false,
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

    /// Process one frame of phase and amplitude data.
    ///
    /// Returns a list of (event_type, value) pairs to emit.
    /// Zone events encode zone_id in the integer part and confidence in the fraction.
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

        // Determine zone count: divide subcarriers into groups of 4.
        let zone_count = (n_sc / 4).min(MAX_ZONES).max(1);
        self.n_zones = zone_count;
        let subs_per_zone = n_sc / zone_count;

        // Compute per-zone variance of amplitudes.
        let mut zone_vars = [0.0f32; MAX_ZONES];
        for z in 0..zone_count {
            let start = z * subs_per_zone;
            let end = if z == zone_count - 1 { n_sc } else { start + subs_per_zone };
            let count = (end - start) as f32;

            // H-02 fix: guard against zero-count zones to prevent division by zero.
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

        // Score each zone: deviation from baseline.
        let mut total_occupied = 0u8;
        for z in 0..zone_count {
            let deviation = fabsf(zone_vars[z] - self.zones[z].baseline_var);
            let raw_score = if self.zones[z].baseline_var > 0.001 {
                deviation / self.zones[z].baseline_var
            } else {
                deviation * 100.0
            };

            // EMA smooth.
            self.zones[z].score = ALPHA * raw_score + (1.0 - ALPHA) * self.zones[z].score;

            // Threshold with hysteresis.
            self.zones[z].prev_occupied = self.zones[z].occupied;
            if self.zones[z].occupied {
                // Higher threshold to leave occupied state.
                self.zones[z].occupied = self.zones[z].score > ZONE_THRESHOLD * 0.5;
            } else {
                self.zones[z].occupied = self.zones[z].score > ZONE_THRESHOLD;
            }

            if self.zones[z].occupied {
                total_occupied += 1;
            }
        }

        // Build output events in a static buffer.
        // We re-use a static to avoid allocation in no_std.
        static mut EVENTS: [(i32, f32); 12] = [(0, 0.0); 12];
        let mut n_events = 0usize;

        // Emit per-zone occupancy (every 10 frames to limit bandwidth).
        if self.frame_count % 10 == 0 {
            for z in 0..zone_count {
                if self.zones[z].occupied && n_events < 10 {
                    // Encode zone_id in integer part, confidence in fractional.
                    let val = z as f32 + self.zones[z].score.min(0.99);
                    unsafe {
                        EVENTS[n_events] = (EVENT_ZONE_OCCUPIED, val);
                    }
                    n_events += 1;
                }
            }

            // Emit total occupied zone count.
            if n_events < 11 {
                unsafe {
                    EVENTS[n_events] = (EVENT_ZONE_COUNT, total_occupied as f32);
                }
                n_events += 1;
            }
        }

        // Emit transitions immediately.
        for z in 0..zone_count {
            if self.zones[z].occupied != self.zones[z].prev_occupied && n_events < 12 {
                let val = z as f32 + if self.zones[z].occupied { 0.5 } else { 0.0 };
                unsafe {
                    EVENTS[n_events] = (EVENT_ZONE_TRANSITION, val);
                }
                n_events += 1;
            }
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Get the number of currently occupied zones.
    pub fn occupied_count(&self) -> u8 {
        let mut count = 0u8;
        for z in 0..self.n_zones {
            if self.zones[z].occupied {
                count += 1;
            }
        }
        count
    }

    /// Check if a specific zone is occupied.
    pub fn is_zone_occupied(&self, zone_id: usize) -> bool {
        zone_id < self.n_zones && self.zones[zone_id].occupied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_occupancy_detector_init() {
        let det = OccupancyDetector::new();
        assert_eq!(det.frame_count, 0);
        assert!(!det.calibrated);
        assert_eq!(det.occupied_count(), 0);
    }

    #[test]
    fn test_occupancy_calibration() {
        let mut det = OccupancyDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // Feed baseline frames.
        for _ in 0..BASELINE_FRAMES {
            let events = det.process_frame(&phases, &amps);
            assert!(events.is_empty());
        }

        assert!(det.calibrated);
    }

    #[test]
    fn test_occupancy_detection() {
        let mut det = OccupancyDetector::new();
        let phases = [0.0f32; 16];
        let uniform_amps = [1.0f32; 16];

        // Calibrate with uniform amplitudes.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &uniform_amps);
        }

        // Now inject a disturbance in zone 0 (first 4 subcarriers).
        let mut disturbed = [1.0f32; 16];
        disturbed[0] = 5.0;
        disturbed[1] = 0.2;
        disturbed[2] = 4.5;
        disturbed[3] = 0.3;

        // Process several frames with disturbance.
        for _ in 0..50 {
            det.process_frame(&phases, &disturbed);
        }

        // Zone 0 should be occupied.
        assert!(det.is_zone_occupied(0));
        assert!(det.occupied_count() >= 1);
    }
}
