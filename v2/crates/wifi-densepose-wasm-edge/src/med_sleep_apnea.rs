//! Sleep apnea detection — ADR-041 Category 1 Medical module.
//!
//! Detects obstructive and central sleep apnea by monitoring breathing BPM
//! from the host CSI pipeline.  When breathing drops below 4 BPM for more
//! than 10 seconds the detector flags an apnea event.  It also tracks the
//! Apnea-Hypopnea Index (AHI) — the number of apnea events per hour of
//! monitored sleep time.
//!
//! Events:
//!   APNEA_START (100) — breathing ceased or fell below threshold
//!   APNEA_END   (101) — breathing resumed after an apnea episode
//!   AHI_UPDATE  (102) — periodic AHI score (events/hour)
//!
//! Host API inputs: breathing BPM, presence, variance.
//! Budget: L (< 2 ms).

// ── libm for no_std math ────────────────────────────────────────────────────

#[cfg(not(feature = "std"))]
use libm::fabsf;
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

// ── Constants ───────────────────────────────────────────────────────────────

/// Breathing BPM threshold below which an apnea epoch is counted.
const APNEA_BPM_THRESH: f32 = 4.0;

/// Seconds of sub-threshold breathing required to declare apnea onset.
const APNEA_ONSET_SECS: u32 = 10;

/// AHI report interval in seconds (every 5 minutes).
const AHI_REPORT_INTERVAL: u32 = 300;

/// Maximum apnea episodes tracked per session (fixed buffer).
const MAX_EPISODES: usize = 256;

/// Presence must be non-zero for monitoring to be active.
const PRESENCE_ACTIVE: i32 = 1;

// ── Event IDs ───────────────────────────────────────────────────────────────

pub const EVENT_APNEA_START: i32 = 100;
pub const EVENT_APNEA_END: i32 = 101;
pub const EVENT_AHI_UPDATE: i32 = 102;

// ── State ───────────────────────────────────────────────────────────────────

/// Episode record: start second and duration.
#[derive(Clone, Copy)]
struct ApneaEpisode {
    start_sec: u32,
    duration_sec: u32,
}

impl ApneaEpisode {
    const fn zero() -> Self {
        Self { start_sec: 0, duration_sec: 0 }
    }
}

/// Sleep apnea detector.
pub struct SleepApneaDetector {
    /// Consecutive seconds of sub-threshold breathing.
    low_breath_secs: u32,
    /// Whether we are currently inside an apnea episode.
    in_apnea: bool,
    /// Start timestamp (in timer ticks) of the current apnea episode.
    current_start: u32,
    /// Ring buffer of recorded episodes.
    episodes: [ApneaEpisode; MAX_EPISODES],
    /// Number of recorded episodes (saturates at MAX_EPISODES).
    episode_count: usize,
    /// Total monitoring seconds (presence active).
    monitoring_secs: u32,
    /// Total timer ticks.
    timer_count: u32,
    /// Most recently computed AHI.
    last_ahi: f32,
}

impl SleepApneaDetector {
    pub const fn new() -> Self {
        Self {
            low_breath_secs: 0,
            in_apnea: false,
            current_start: 0,
            episodes: [ApneaEpisode::zero(); MAX_EPISODES],
            episode_count: 0,
            monitoring_secs: 0,
            timer_count: 0,
            last_ahi: 0.0,
        }
    }

    /// Called at ~1 Hz with current breathing BPM, presence flag, and variance.
    ///
    /// Returns `&[(event_id, value)]` slice of emitted events.
    pub fn process_frame(
        &mut self,
        breathing_bpm: f32,
        presence: i32,
        _variance: f32,
    ) -> &[(i32, f32)] {
        self.timer_count += 1;

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;

        // Only monitor when subject is present.
        if presence < PRESENCE_ACTIVE {
            // If subject leaves during apnea, end the episode.
            if self.in_apnea {
                let dur = self.timer_count.saturating_sub(self.current_start);
                self.record_episode(self.current_start, dur);
                self.in_apnea = false;
                self.low_breath_secs = 0;
                unsafe { EVENTS[n] = (EVENT_APNEA_END, dur as f32); }
                n += 1;
            }
            self.low_breath_secs = 0;
            return unsafe { &EVENTS[..n] };
        }

        self.monitoring_secs += 1;

        // Guard against NaN: NaN comparisons return false, which would
        // incorrectly take the "breathing resumed" branch every tick.
        // Treat NaN as invalid — skip detection for this frame.
        if breathing_bpm != breathing_bpm {
            // NaN: f32::NAN != f32::NAN is true.
            return unsafe { &EVENTS[..n] };
        }

        // ── Apnea detection ─────────────────────────────────────────────
        if breathing_bpm < APNEA_BPM_THRESH {
            self.low_breath_secs += 1;

            if !self.in_apnea && self.low_breath_secs >= APNEA_ONSET_SECS {
                // Apnea onset — backdate start to when breathing first dropped.
                self.in_apnea = true;
                self.current_start = self.timer_count.saturating_sub(self.low_breath_secs);
                unsafe { EVENTS[n] = (EVENT_APNEA_START, breathing_bpm); }
                n += 1;
            }
        } else {
            // Breathing resumed.
            if self.in_apnea {
                let dur = self.timer_count.saturating_sub(self.current_start);
                self.record_episode(self.current_start, dur);
                self.in_apnea = false;
                unsafe { EVENTS[n] = (EVENT_APNEA_END, dur as f32); }
                n += 1;
            }
            self.low_breath_secs = 0;
        }

        // ── Periodic AHI update ─────────────────────────────────────────
        if self.timer_count % AHI_REPORT_INTERVAL == 0 && self.monitoring_secs > 0 && n < 4 {
            let hours = self.monitoring_secs as f32 / 3600.0;
            self.last_ahi = if hours > 0.001 {
                self.episode_count as f32 / hours
            } else {
                0.0
            };
            unsafe { EVENTS[n] = (EVENT_AHI_UPDATE, self.last_ahi); }
            n += 1;
        }

        unsafe { &EVENTS[..n] }
    }

    fn record_episode(&mut self, start: u32, duration: u32) {
        if self.episode_count < MAX_EPISODES {
            self.episodes[self.episode_count] = ApneaEpisode {
                start_sec: start,
                duration_sec: duration,
            };
            self.episode_count += 1;
        }
    }

    /// Current AHI value.
    pub fn ahi(&self) -> f32 {
        self.last_ahi
    }

    /// Number of recorded apnea episodes.
    pub fn episode_count(&self) -> usize {
        self.episode_count
    }

    /// Total monitoring seconds.
    pub fn monitoring_seconds(&self) -> u32 {
        self.monitoring_secs
    }

    /// Whether currently in an apnea episode.
    pub fn in_apnea(&self) -> bool {
        self.in_apnea
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let d = SleepApneaDetector::new();
        assert_eq!(d.episode_count(), 0);
        assert!(!d.in_apnea());
        assert!((d.ahi() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_normal_breathing_no_apnea() {
        let mut d = SleepApneaDetector::new();
        for _ in 0..120 {
            let ev = d.process_frame(14.0, 1, 0.1);
            for &(t, _) in ev {
                assert_ne!(t, EVENT_APNEA_START, "no apnea with normal breathing");
            }
        }
        assert_eq!(d.episode_count(), 0);
    }

    #[test]
    fn test_apnea_onset_and_end() {
        let mut d = SleepApneaDetector::new();
        let mut start_seen = false;
        let mut end_seen = false;

        // Feed sub-threshold breathing for >10 seconds.
        for _ in 0..15 {
            let ev = d.process_frame(2.0, 1, 0.1);
            for &(t, _) in ev {
                if t == EVENT_APNEA_START { start_seen = true; }
            }
        }
        assert!(start_seen, "apnea start should fire after 10s of low breathing");
        assert!(d.in_apnea());

        // Resume normal breathing.
        let ev = d.process_frame(14.0, 1, 0.1);
        for &(t, _) in ev {
            if t == EVENT_APNEA_END { end_seen = true; }
        }
        assert!(end_seen, "apnea end should fire when breathing resumes");
        assert!(!d.in_apnea());
        assert_eq!(d.episode_count(), 1);
    }

    #[test]
    fn test_no_monitoring_without_presence() {
        let mut d = SleepApneaDetector::new();
        // No presence — should not trigger apnea even with zero breathing.
        for _ in 0..30 {
            let ev = d.process_frame(0.0, 0, 0.0);
            for &(t, _) in ev {
                assert_ne!(t, EVENT_APNEA_START);
            }
        }
        assert_eq!(d.monitoring_seconds(), 0);
    }

    #[test]
    fn test_ahi_update_emitted() {
        let mut d = SleepApneaDetector::new();
        // First trigger one apnea episode.
        for _ in 0..15 {
            d.process_frame(1.0, 1, 0.1);
        }
        d.process_frame(14.0, 1, 0.1); // end apnea
        assert_eq!(d.episode_count(), 1);

        // Run until AHI report interval.
        let mut ahi_seen = false;
        for _ in d.timer_count..AHI_REPORT_INTERVAL + 1 {
            let ev = d.process_frame(14.0, 1, 0.1);
            for &(t, v) in ev {
                if t == EVENT_AHI_UPDATE {
                    ahi_seen = true;
                    assert!(v > 0.0, "AHI should be positive with 1 episode");
                }
            }
        }
        assert!(ahi_seen, "AHI_UPDATE event should be emitted periodically");
    }

    #[test]
    fn test_multiple_episodes() {
        let mut d = SleepApneaDetector::new();

        for _episode in 0..3 {
            // Apnea period.
            for _ in 0..15 {
                d.process_frame(1.0, 1, 0.1);
            }
            // Recovery.
            for _ in 0..30 {
                d.process_frame(14.0, 1, 0.1);
            }
        }

        assert_eq!(d.episode_count(), 3);
    }

    #[test]
    fn test_apnea_ends_on_presence_lost() {
        let mut d = SleepApneaDetector::new();
        // Enter apnea.
        for _ in 0..15 {
            d.process_frame(1.0, 1, 0.1);
        }
        assert!(d.in_apnea());

        // Lose presence.
        let mut end_seen = false;
        let ev = d.process_frame(1.0, 0, 0.0);
        for &(t, _) in ev {
            if t == EVENT_APNEA_END { end_seen = true; }
        }
        assert!(end_seen, "apnea should end when presence lost");
        assert!(!d.in_apnea());
        assert_eq!(d.episode_count(), 1);
    }
}
