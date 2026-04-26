//! Dwell-time heatmap — ADR-041 Category 4: Retail & Hospitality.
//!
//! Tracks dwell time per spatial zone using a 3x3 grid (9 zones).
//! Each zone maps to a group of subcarriers (Fresnel zone geometry).
//! Accumulates dwell-seconds per zone and emits per-zone updates
//! every 30 seconds (600 frames at 20 Hz).
//!
//! Events (410-series):
//! - `DWELL_ZONE_UPDATE(410)`:  Per-zone dwell seconds (zone_id encoded in value)
//! - `HOT_ZONE(411)`:           Zone with highest dwell time
//! - `COLD_ZONE(412)`:          Zone with lowest dwell time (of occupied zones)
//! - `SESSION_SUMMARY(413)`:    Emitted when space empties after occupancy
//!
//! Host API used: presence, variance, motion energy, n_persons.

use crate::vendor_common::Ema;

#[cfg(not(feature = "std"))]
use libm::fabsf;
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

// ── Event IDs ─────────────────────────────────────────────────────────────────

pub const EVENT_DWELL_ZONE_UPDATE: i32 = 410;
pub const EVENT_HOT_ZONE: i32 = 411;
pub const EVENT_COLD_ZONE: i32 = 412;
pub const EVENT_SESSION_SUMMARY: i32 = 413;

// ── Configuration constants ──────────────────────────────────────────────────

/// Number of spatial zones (3x3 grid).
const NUM_ZONES: usize = 9;

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// Frame rate assumption (Hz).
const FRAME_RATE: f32 = 20.0;

/// Seconds per frame.
const SECONDS_PER_FRAME: f32 = 1.0 / FRAME_RATE;

/// Reporting interval in frames (~30 seconds at 20 Hz).
const REPORT_INTERVAL: u32 = 600;

/// Variance threshold to consider a zone occupied.
const ZONE_OCCUPIED_THRESH: f32 = 0.015;

/// EMA alpha for zone variance smoothing.
const ZONE_EMA_ALPHA: f32 = 0.12;

/// Minimum frames of zero presence before session summary.
const EMPTY_FRAMES_FOR_SUMMARY: u32 = 100;

/// Maximum event output slots.
const MAX_EVENTS: usize = 12;

// ── Per-zone state ───────────────────────────────────────────────────────────

struct ZoneState {
    /// EMA-smoothed variance for this zone.
    variance_ema: Ema,
    /// Whether this zone is currently occupied.
    occupied: bool,
    /// Accumulated dwell time (seconds) in current session.
    dwell_seconds: f32,
    /// Total dwell time (seconds) across all sessions.
    total_dwell_seconds: f32,
}

const ZONE_INIT: ZoneState = ZoneState {
    variance_ema: Ema::new(ZONE_EMA_ALPHA),
    occupied: false,
    dwell_seconds: 0.0,
    total_dwell_seconds: 0.0,
};

// ── Dwell Heatmap Tracker ────────────────────────────────────────────────────

/// Tracks dwell time across a 3x3 spatial zone grid.
pub struct DwellHeatmapTracker {
    zones: [ZoneState; NUM_ZONES],
    /// Frame counter.
    frame_count: u32,
    /// Whether anyone is currently present (global).
    any_present: bool,
    /// Consecutive frames with no presence.
    empty_frames: u32,
    /// Whether a session is active (someone was present recently).
    session_active: bool,
    /// Session start frame.
    session_start_frame: u32,
}

impl DwellHeatmapTracker {
    pub const fn new() -> Self {
        Self {
            zones: [ZONE_INIT; NUM_ZONES],
            frame_count: 0,
            any_present: false,
            empty_frames: 0,
            session_active: false,
            session_start_frame: 0,
        }
    }

    /// Process one CSI frame with per-subcarrier variance data.
    ///
    /// - `presence`: 1 if someone is present, 0 otherwise
    /// - `variances`: per-subcarrier variance array
    /// - `motion_energy`: aggregate motion energy
    /// - `n_persons`: estimated person count
    ///
    /// Returns event slice `&[(event_type, value)]`.
    pub fn process_frame(
        &mut self,
        presence: i32,
        variances: &[f32],
        _motion_energy: f32,
        n_persons: i32,
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        let n_sc = variances.len().min(MAX_SC);
        let is_present = presence > 0 || n_persons > 0;

        // Map subcarriers to zones (divide evenly into NUM_ZONES groups).
        let subs_per_zone = if n_sc >= NUM_ZONES { n_sc / NUM_ZONES } else { 1 };
        let active_zones = if n_sc >= NUM_ZONES { NUM_ZONES } else { n_sc.max(1) };

        // Compute per-zone variance and update EMA.
        let mut any_zone_occupied = false;
        for z in 0..active_zones {
            let start = z * subs_per_zone;
            let end = if z == active_zones - 1 { n_sc } else { start + subs_per_zone };
            let count = end - start;
            if count == 0 {
                continue;
            }

            let mut zone_var = 0.0f32;
            for i in start..end {
                zone_var += variances[i];
            }
            zone_var /= count as f32;

            self.zones[z].variance_ema.update(zone_var);

            // Determine zone occupancy.
            let _was_occupied = self.zones[z].occupied;
            self.zones[z].occupied = is_present && self.zones[z].variance_ema.value > ZONE_OCCUPIED_THRESH;

            if self.zones[z].occupied {
                any_zone_occupied = true;
                self.zones[z].dwell_seconds += SECONDS_PER_FRAME;
                self.zones[z].total_dwell_seconds += SECONDS_PER_FRAME;
            }
        }

        // Session management.
        if is_present || any_zone_occupied {
            self.empty_frames = 0;
            if !self.session_active {
                self.session_active = true;
                self.session_start_frame = self.frame_count;
                // Reset session dwell accumulators.
                for z in 0..NUM_ZONES {
                    self.zones[z].dwell_seconds = 0.0;
                }
            }
        } else {
            self.empty_frames += 1;
        }

        self.any_present = is_present || any_zone_occupied;

        // Build events.
        static mut EVENTS: [(i32, f32); MAX_EVENTS] = [(0, 0.0); MAX_EVENTS];
        let mut ne = 0usize;

        // Periodic zone updates.
        if self.frame_count % REPORT_INTERVAL == 0 && self.session_active {
            // Emit dwell time per occupied zone.
            for z in 0..active_zones {
                if self.zones[z].dwell_seconds > 0.0 && ne < MAX_EVENTS - 3 {
                    // Encode zone_id in integer part, dwell seconds in value.
                    let val = z as f32 * 1000.0 + self.zones[z].dwell_seconds;
                    unsafe {
                        EVENTS[ne] = (EVENT_DWELL_ZONE_UPDATE, val);
                    }
                    ne += 1;
                }
            }

            // Find hot zone (highest dwell) and cold zone (lowest non-zero dwell).
            let mut hot_zone = 0usize;
            let mut hot_dwell = 0.0f32;
            let mut cold_zone = 0usize;
            let mut cold_dwell = f32::MAX;

            for z in 0..active_zones {
                if self.zones[z].dwell_seconds > hot_dwell {
                    hot_dwell = self.zones[z].dwell_seconds;
                    hot_zone = z;
                }
                if self.zones[z].dwell_seconds > 0.0 && self.zones[z].dwell_seconds < cold_dwell {
                    cold_dwell = self.zones[z].dwell_seconds;
                    cold_zone = z;
                }
            }

            if hot_dwell > 0.0 && ne < MAX_EVENTS {
                unsafe {
                    EVENTS[ne] = (EVENT_HOT_ZONE, hot_zone as f32 + hot_dwell / 1000.0);
                }
                ne += 1;
            }

            if cold_dwell < f32::MAX && ne < MAX_EVENTS {
                unsafe {
                    EVENTS[ne] = (EVENT_COLD_ZONE, cold_zone as f32 + cold_dwell / 1000.0);
                }
                ne += 1;
            }
        }

        // Session summary when space empties.
        if self.session_active && self.empty_frames >= EMPTY_FRAMES_FOR_SUMMARY {
            self.session_active = false;
            let session_duration = (self.frame_count - self.session_start_frame) as f32 / FRAME_RATE;
            if ne < MAX_EVENTS {
                unsafe {
                    EVENTS[ne] = (EVENT_SESSION_SUMMARY, session_duration);
                }
                ne += 1;
            }
        }

        unsafe { &EVENTS[..ne] }
    }

    /// Get dwell time (seconds) for a specific zone in the current session.
    pub fn zone_dwell(&self, zone_id: usize) -> f32 {
        if zone_id < NUM_ZONES {
            self.zones[zone_id].dwell_seconds
        } else {
            0.0
        }
    }

    /// Get total accumulated dwell time across all sessions for a zone.
    pub fn zone_total_dwell(&self, zone_id: usize) -> f32 {
        if zone_id < NUM_ZONES {
            self.zones[zone_id].total_dwell_seconds
        } else {
            0.0
        }
    }

    /// Check if a specific zone is currently occupied.
    pub fn is_zone_occupied(&self, zone_id: usize) -> bool {
        zone_id < NUM_ZONES && self.zones[zone_id].occupied
    }

    /// Check if a session is currently active.
    pub fn is_session_active(&self) -> bool {
        self.session_active
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let t = DwellHeatmapTracker::new();
        assert_eq!(t.frame_count, 0);
        assert!(!t.session_active);
        assert!(!t.any_present);
        for z in 0..NUM_ZONES {
            assert!(!t.is_zone_occupied(z));
            assert!(t.zone_dwell(z) < 0.001);
        }
    }

    #[test]
    fn test_no_presence_no_dwell() {
        let mut t = DwellHeatmapTracker::new();
        let vars = [0.0f32; 18];
        for _ in 0..100 {
            t.process_frame(0, &vars, 0.0, 0);
        }
        for z in 0..NUM_ZONES {
            assert!(t.zone_dwell(z) < 0.001, "zone {} should have no dwell", z);
        }
        assert!(!t.is_session_active());
    }

    #[test]
    fn test_dwell_accumulates_with_presence() {
        let mut t = DwellHeatmapTracker::new();
        // 18 subcarriers, 2 per zone for 9 zones.
        // Make zone 0 (subcarriers 0-1) have high variance.
        let mut vars = [0.001f32; 18];
        vars[0] = 0.1;
        vars[1] = 0.12;

        // Feed 100 frames with presence (~5 seconds).
        for _ in 0..100 {
            t.process_frame(1, &vars, 0.5, 1);
        }

        // Zone 0 should have accumulated dwell time.
        let dwell_z0 = t.zone_dwell(0);
        assert!(dwell_z0 > 2.0, "zone 0 dwell should be > 2s, got {}", dwell_z0);
        assert!(t.is_session_active());
    }

    #[test]
    fn test_session_summary_on_empty() {
        let mut t = DwellHeatmapTracker::new();
        let vars_active = [0.05f32; 18];
        let vars_empty = [0.0f32; 18];

        // Active phase.
        for _ in 0..200 {
            t.process_frame(1, &vars_active, 0.5, 1);
        }
        assert!(t.is_session_active());

        // Empty phase: wait for session summary.
        let mut summary_emitted = false;
        for _ in 0..EMPTY_FRAMES_FOR_SUMMARY + 10 {
            let events = t.process_frame(0, &vars_empty, 0.0, 0);
            for &(et, _) in events {
                if et == EVENT_SESSION_SUMMARY {
                    summary_emitted = true;
                }
            }
        }
        assert!(summary_emitted, "session summary should be emitted when space empties");
        assert!(!t.is_session_active());
    }

    #[test]
    fn test_periodic_zone_updates() {
        let mut t = DwellHeatmapTracker::new();
        let vars = [0.05f32; 18];
        let mut dwell_update_count = 0;

        for _ in 0..REPORT_INTERVAL + 1 {
            let events = t.process_frame(1, &vars, 0.5, 1);
            for &(et, _) in events {
                if et == EVENT_DWELL_ZONE_UPDATE {
                    dwell_update_count += 1;
                }
            }
        }
        assert!(dwell_update_count > 0, "should emit zone dwell updates at report interval");
    }

    #[test]
    fn test_hot_cold_zone_identification() {
        let mut t = DwellHeatmapTracker::new();
        // Zone 0 has high variance, zone 1 has moderate, rest low.
        let mut vars = [0.001f32; 18];
        vars[0] = 0.2;
        vars[1] = 0.2;
        vars[2] = 0.04;
        vars[3] = 0.04;

        let mut hot_emitted = false;
        let mut _cold_emitted = false;

        for _ in 0..REPORT_INTERVAL + 1 {
            let events = t.process_frame(1, &vars, 0.5, 2);
            for &(et, _) in events {
                if et == EVENT_HOT_ZONE {
                    hot_emitted = true;
                }
                if et == EVENT_COLD_ZONE {
                    _cold_emitted = true;
                }
            }
        }
        assert!(hot_emitted, "hot zone event should be emitted");
    }

    #[test]
    fn test_zone_oob_access() {
        let t = DwellHeatmapTracker::new();
        assert!(t.zone_dwell(100) < 0.001);
        assert!(t.zone_total_dwell(100) < 0.001);
        assert!(!t.is_zone_occupied(100));
    }

    #[test]
    fn test_empty_variance_slice() {
        let mut t = DwellHeatmapTracker::new();
        let vars: [f32; 0] = [];
        // Should not panic.
        let _events = t.process_frame(0, &vars, 0.0, 0);
        // No crash is success.
    }
}
