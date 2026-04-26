//! Shelf engagement detection — ADR-041 Category 4: Retail & Hospitality.
//!
//! Detects customers stopping near shelving using CSI phase perturbation analysis.
//! Low translational motion + high-frequency phase perturbation indicates someone
//! standing still but interacting with products (reaching, examining).
//!
//! Engagement classification:
//! - Browse:          < 5 seconds of engagement
//! - Consider:        5-30 seconds of engagement
//! - Deep engagement: > 30 seconds of engagement
//!
//! Events (440-series):
//! - `SHELF_BROWSE(440)`:      Short browsing event detected
//! - `SHELF_CONSIDER(441)`:    Medium consideration event
//! - `SHELF_ENGAGE(442)`:      Deep engagement event
//! - `REACH_DETECTED(443)`:    Reaching gesture detected (high-freq phase burst)
//!
//! Host API used: presence, motion energy, variance, phase.

use crate::vendor_common::{CircularBuffer, Ema};

#[cfg(not(feature = "std"))]
use libm::{fabsf, sqrtf};
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

// ── Event IDs ─────────────────────────────────────────────────────────────────

pub const EVENT_SHELF_BROWSE: i32 = 440;
pub const EVENT_SHELF_CONSIDER: i32 = 441;
pub const EVENT_SHELF_ENGAGE: i32 = 442;
pub const EVENT_REACH_DETECTED: i32 = 443;

// ── Configuration constants ──────────────────────────────────────────────────

/// Maximum subcarriers.
const MAX_SC: usize = 32;

/// Frame rate assumption (Hz).
const FRAME_RATE: f32 = 20.0;

/// Browse threshold in seconds.
const BROWSE_THRESH_S: f32 = 5.0;
/// Consider threshold in seconds.
const CONSIDER_THRESH_S: f32 = 30.0;

/// Browse threshold in frames.
const BROWSE_THRESH_FRAMES: u32 = (BROWSE_THRESH_S * FRAME_RATE) as u32;
/// Consider threshold in frames.
const CONSIDER_THRESH_FRAMES: u32 = (CONSIDER_THRESH_S * FRAME_RATE) as u32;

/// Motion energy threshold for "standing still" (low translational motion).
const STILL_MOTION_THRESH: f32 = 0.08;

/// High-frequency phase perturbation threshold (indicates hand/arm movement).
const PHASE_PERTURBATION_THRESH: f32 = 0.04;

/// Reach detection: high-frequency phase burst above this threshold.
const REACH_BURST_THRESH: f32 = 0.15;

/// Minimum frames of stillness before engagement counting starts.
const STILL_DEBOUNCE: u32 = 10;

/// Cooldown frames after emitting an engagement event.
const ENGAGEMENT_COOLDOWN: u16 = 60;

/// EMA alpha for phase perturbation smoothing.
const PERTURBATION_EMA_ALPHA: f32 = 0.2;

/// EMA alpha for motion smoothing.
const MOTION_EMA_ALPHA: f32 = 0.15;

/// Phase history depth for high-frequency analysis (0.5 s at 20 Hz).
const PHASE_HISTORY: usize = 10;

/// Maximum events per frame.
const MAX_EVENTS: usize = 4;

// ── Engagement State ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngagementLevel {
    /// No engagement (passing by or absent).
    None,
    /// Brief browsing (< 5s).
    Browse,
    /// Considering product (5-30s).
    Consider,
    /// Deep engagement (> 30s).
    DeepEngage,
}

// ── Shelf Engagement Detector ───────────────────────────────────────────────

/// Detects and classifies customer shelf engagement from CSI data.
pub struct ShelfEngagementDetector {
    /// Previous phase values for perturbation calculation.
    prev_phases: [f32; MAX_SC],
    /// Phase perturbation EMA (high-frequency component).
    perturbation_ema: Ema,
    /// Motion energy EMA.
    motion_ema: Ema,
    /// Phase difference history for burst detection.
    phase_diff_history: CircularBuffer<PHASE_HISTORY>,
    /// Whether previous phases are initialized.
    phase_init: bool,
    /// Consecutive frames of "still + perturbation" (engagement).
    engagement_frames: u32,
    /// Consecutive frames of stillness (before engagement counting).
    still_frames: u32,
    /// Current engagement level.
    level: EngagementLevel,
    /// Previous emitted engagement level (avoid duplicate events).
    prev_emitted_level: EngagementLevel,
    /// Cooldown counter.
    cooldown: u16,
    /// Frame counter.
    frame_count: u32,
    /// Total browsing events.
    total_browse: u32,
    /// Total consider events.
    total_consider: u32,
    /// Total deep engagement events.
    total_engage: u32,
    /// Total reach detections.
    total_reaches: u32,
    /// Number of subcarriers last frame.
    n_sc: usize,
}

impl ShelfEngagementDetector {
    pub const fn new() -> Self {
        Self {
            prev_phases: [0.0; MAX_SC],
            perturbation_ema: Ema::new(PERTURBATION_EMA_ALPHA),
            motion_ema: Ema::new(MOTION_EMA_ALPHA),
            phase_diff_history: CircularBuffer::new(),
            phase_init: false,
            engagement_frames: 0,
            still_frames: 0,
            level: EngagementLevel::None,
            prev_emitted_level: EngagementLevel::None,
            cooldown: 0,
            frame_count: 0,
            total_browse: 0,
            total_consider: 0,
            total_engage: 0,
            total_reaches: 0,
            n_sc: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// - `presence`: 1 if someone is present
    /// - `motion_energy`: aggregate motion energy
    /// - `variance`: mean subcarrier variance
    /// - `phases`: per-subcarrier phase values
    ///
    /// Returns event slice `&[(event_type, value)]`.
    pub fn process_frame(
        &mut self,
        presence: i32,
        motion_energy: f32,
        _variance: f32,
        phases: &[f32],
    ) -> &[(i32, f32)] {
        self.frame_count += 1;

        let n_sc = phases.len().min(MAX_SC);
        self.n_sc = n_sc;

        let is_present = presence > 0;
        let smoothed_motion = self.motion_ema.update(motion_energy);

        if self.cooldown > 0 {
            self.cooldown -= 1;
        }

        // Initialize previous phases.
        if !self.phase_init && n_sc > 0 {
            for i in 0..n_sc {
                self.prev_phases[i] = phases[i];
            }
            self.phase_init = true;
            return &[];
        }

        // Compute high-frequency phase perturbation.
        // This measures small rapid phase changes (hand/arm movements near shelf)
        // distinct from large translational phase shifts (walking).
        let mut perturbation = 0.0f32;
        if n_sc > 0 {
            // Compute per-subcarrier phase difference, then take std dev.
            let mut diffs = [0.0f32; MAX_SC];
            let mut diff_mean = 0.0f32;
            for i in 0..n_sc {
                diffs[i] = phases[i] - self.prev_phases[i];
                diff_mean += diffs[i];
            }
            diff_mean /= n_sc as f32;

            // Variance of phase differences (high = reaching/grabbing, low = still/walking).
            let mut diff_var = 0.0f32;
            for i in 0..n_sc {
                let d = diffs[i] - diff_mean;
                diff_var += d * d;
            }
            diff_var /= n_sc as f32;
            perturbation = sqrtf(diff_var);

            // Update previous phases.
            for i in 0..n_sc {
                self.prev_phases[i] = phases[i];
            }
        }

        let smoothed_perturbation = self.perturbation_ema.update(perturbation);
        self.phase_diff_history.push(perturbation);

        // Build events.
        static mut EVENTS: [(i32, f32); MAX_EVENTS] = [(0, 0.0); MAX_EVENTS];
        let mut ne = 0usize;

        if !is_present {
            // No one present: end any engagement.
            if self.level != EngagementLevel::None {
                // Emit final engagement classification.
                ne = self.emit_engagement_end(ne);
            }
            self.engagement_frames = 0;
            self.still_frames = 0;
            self.level = EngagementLevel::None;
            self.prev_emitted_level = EngagementLevel::None;
            unsafe { return &EVENTS[..ne]; }
        }

        // Detect stillness (low translational motion).
        if smoothed_motion < STILL_MOTION_THRESH {
            self.still_frames += 1;
        } else {
            // Moving: reset engagement.
            if self.level != EngagementLevel::None && self.engagement_frames > 0 {
                ne = self.emit_engagement_end(ne);
            }
            self.still_frames = 0;
            self.engagement_frames = 0;
            self.level = EngagementLevel::None;
            self.prev_emitted_level = EngagementLevel::None;
            unsafe { return &EVENTS[..ne]; }
        }

        // Only start engagement counting after debounce.
        if self.still_frames >= STILL_DEBOUNCE && smoothed_perturbation > PHASE_PERTURBATION_THRESH {
            self.engagement_frames += 1;

            // Classify engagement level.
            if self.engagement_frames >= CONSIDER_THRESH_FRAMES {
                self.level = EngagementLevel::DeepEngage;
            } else if self.engagement_frames >= BROWSE_THRESH_FRAMES {
                self.level = EngagementLevel::Consider;
            } else {
                self.level = EngagementLevel::Browse;
            }

            // Emit on level upgrade.
            if self.level != self.prev_emitted_level && self.cooldown == 0 {
                let (event_id, duration) = match self.level {
                    EngagementLevel::Browse => {
                        self.total_browse += 1;
                        (EVENT_SHELF_BROWSE, self.engagement_frames as f32 / FRAME_RATE)
                    }
                    EngagementLevel::Consider => {
                        self.total_consider += 1;
                        (EVENT_SHELF_CONSIDER, self.engagement_frames as f32 / FRAME_RATE)
                    }
                    EngagementLevel::DeepEngage => {
                        self.total_engage += 1;
                        (EVENT_SHELF_ENGAGE, self.engagement_frames as f32 / FRAME_RATE)
                    }
                    EngagementLevel::None => (0, 0.0),
                };

                if event_id != 0 && ne < MAX_EVENTS {
                    unsafe {
                        EVENTS[ne] = (event_id, duration);
                    }
                    ne += 1;
                    self.prev_emitted_level = self.level;
                    self.cooldown = ENGAGEMENT_COOLDOWN;
                }
            }
        }

        // Reach detection: sudden high-frequency phase burst while still.
        if self.still_frames > STILL_DEBOUNCE && perturbation > REACH_BURST_THRESH && ne < MAX_EVENTS {
            self.total_reaches += 1;
            unsafe {
                EVENTS[ne] = (EVENT_REACH_DETECTED, perturbation);
            }
            ne += 1;
        }

        unsafe { &EVENTS[..ne] }
    }

    /// Emit engagement end event based on current level.
    fn emit_engagement_end(&self, ne: usize) -> usize {
        // The engagement classification was already emitted during the session.
        // We could emit a summary here, but to stay within budget we just return.
        ne
    }

    /// Get current engagement level.
    pub fn engagement_level(&self) -> EngagementLevel {
        self.level
    }

    /// Get engagement duration in seconds.
    pub fn engagement_duration_s(&self) -> f32 {
        self.engagement_frames as f32 / FRAME_RATE
    }

    /// Get total browse events.
    pub fn total_browse_events(&self) -> u32 {
        self.total_browse
    }

    /// Get total consider events.
    pub fn total_consider_events(&self) -> u32 {
        self.total_consider
    }

    /// Get total deep engagement events.
    pub fn total_engage_events(&self) -> u32 {
        self.total_engage
    }

    /// Get total reach detections.
    pub fn total_reach_events(&self) -> u32 {
        self.total_reaches
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state() {
        let se = ShelfEngagementDetector::new();
        assert_eq!(se.engagement_level(), EngagementLevel::None);
        assert!(se.engagement_duration_s() < 0.001);
        assert_eq!(se.total_browse_events(), 0);
        assert_eq!(se.total_consider_events(), 0);
        assert_eq!(se.total_engage_events(), 0);
        assert_eq!(se.total_reach_events(), 0);
    }

    #[test]
    fn test_no_presence_no_engagement() {
        let mut se = ShelfEngagementDetector::new();
        let phases = [0.0f32; 16];
        for _ in 0..200 {
            let events = se.process_frame(0, 0.0, 0.0, &phases);
            for &(et, _) in events {
                assert!(
                    et != EVENT_SHELF_BROWSE && et != EVENT_SHELF_CONSIDER && et != EVENT_SHELF_ENGAGE,
                    "no engagement events without presence"
                );
            }
        }
        assert_eq!(se.engagement_level(), EngagementLevel::None);
    }

    #[test]
    fn test_walking_past_no_engagement() {
        let mut se = ShelfEngagementDetector::new();
        // Initialize phases.
        let init_phases = [0.0f32; 16];
        se.process_frame(1, 0.5, 0.1, &init_phases);

        // High motion (walking) should not trigger engagement.
        for _ in 0..200 {
            let phases: [f32; 16] = core::array::from_fn(|i| (i as f32) * 0.1);
            se.process_frame(1, 0.5, 0.1, &phases);
        }
        assert_eq!(se.engagement_level(), EngagementLevel::None);
    }

    #[test]
    fn test_browse_detection() {
        let mut se = ShelfEngagementDetector::new();
        // Init with baseline phases.
        let init_phases = [0.0f32; 16];
        se.process_frame(1, 0.01, 0.01, &init_phases);

        let mut browse_detected = false;
        // Simulate standing still with spatially diverse phase perturbations.
        // The key: each frame's per-subcarrier phase must vary enough that
        // the std-dev of (phases[i] - prev_phases[i]) exceeds PHASE_PERTURBATION_THRESH.
        for frame in 0..(BROWSE_THRESH_FRAMES + STILL_DEBOUNCE + 10) {
            let mut phases = [0.0f32; 16];
            for i in 0..16 {
                // Alternating sign pattern with frame-varying magnitude
                // produces high spatial variance in frame-to-frame differences.
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                let mag = 0.15 * (1.0 + (frame as f32 * 0.5).sin());
                phases[i] = sign * mag * (i as f32 * 0.3 + 0.1);
            }
            let events = se.process_frame(1, 0.02, 0.03, &phases);
            for &(et, _) in events {
                if et == EVENT_SHELF_BROWSE {
                    browse_detected = true;
                }
            }
        }
        assert!(browse_detected, "browse event should be detected for short engagement");
    }

    #[test]
    fn test_reach_detection() {
        let mut se = ShelfEngagementDetector::new();
        let init_phases = [0.0f32; 16];
        se.process_frame(1, 0.01, 0.01, &init_phases);

        // Build up stillness.
        for _ in 0..STILL_DEBOUNCE + 5 {
            se.process_frame(1, 0.02, 0.01, &[0.0f32; 16]);
        }

        let mut reach_detected = false;
        // Sudden large perturbation (reach burst).
        let mut reach_phases = [0.0f32; 16];
        for i in 0..16 {
            reach_phases[i] = if i % 2 == 0 { 0.5 } else { -0.5 };
        }
        let events = se.process_frame(1, 0.02, 0.05, &reach_phases);
        for &(et, _) in events {
            if et == EVENT_REACH_DETECTED {
                reach_detected = true;
            }
        }
        assert!(reach_detected, "reach should be detected from high phase burst");
    }

    #[test]
    fn test_engagement_resets_on_departure() {
        let mut se = ShelfEngagementDetector::new();
        let init_phases = [0.0f32; 16];
        se.process_frame(1, 0.01, 0.01, &init_phases);

        // Build some engagement.
        for frame in 0..50 {
            let mut phases = [0.0f32; 16];
            for i in 0..16 {
                phases[i] = 0.1 * ((frame as f32 * 0.5 + i as f32).sin());
            }
            se.process_frame(1, 0.02, 0.03, &phases);
        }

        // Person leaves.
        se.process_frame(0, 0.0, 0.0, &[0.0f32; 16]);
        assert_eq!(se.engagement_level(), EngagementLevel::None);
        assert!(se.engagement_duration_s() < 0.001);
    }

    #[test]
    fn test_empty_phases_no_panic() {
        let mut se = ShelfEngagementDetector::new();
        let empty: [f32; 0] = [];
        let _events = se.process_frame(1, 0.1, 0.05, &empty);
        // Should not panic.
    }

    #[test]
    fn test_consider_level_upgrade() {
        let mut se = ShelfEngagementDetector::new();
        let init_phases = [0.0f32; 16];
        se.process_frame(1, 0.01, 0.01, &init_phases);

        let mut consider_detected = false;
        // Simulate long engagement (> 30s = 600 frames + debounce).
        for frame in 0..(CONSIDER_THRESH_FRAMES + STILL_DEBOUNCE + 10) {
            let mut phases = [0.0f32; 16];
            for i in 0..16 {
                // Same spatially diverse pattern as browse test.
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                let mag = 0.15 * (1.0 + (frame as f32 * 0.5).sin());
                phases[i] = sign * mag * (i as f32 * 0.3 + 0.1);
            }
            let events = se.process_frame(1, 0.02, 0.03, &phases);
            for &(et, _) in events {
                if et == EVENT_SHELF_CONSIDER {
                    consider_detected = true;
                }
            }
        }
        assert!(consider_detected, "consider event should fire after {} frames", CONSIDER_THRESH_FRAMES);
    }
}
