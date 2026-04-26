//! Cardiac arrhythmia detection — ADR-041 Category 1 Medical module.
//!
//! Monitors heart rate from host CSI pipeline and detects:
//!   - Tachycardia: sustained HR > 100 BPM
//!   - Bradycardia: sustained HR < 50 BPM
//!   - Missed beats: sudden HR dips > 30% below running average
//!   - HRV anomaly: RMSSD outside normal range over 30-second window
//!
//! Events:
//!   TACHYCARDIA  (110) — sustained high heart rate
//!   BRADYCARDIA  (111) — sustained low heart rate
//!   MISSED_BEAT  (112) — abrupt HR drop suggesting missed beat
//!   HRV_ANOMALY  (113) — heart rate variability outside normal bounds
//!
//! Host API inputs: heart rate BPM, phase.
//! Budget: S (< 5 ms).

// ── libm for no_std math ────────────────────────────────────────────────────

#[cfg(not(feature = "std"))]
use libm::sqrtf;
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

#[cfg(not(feature = "std"))]
use libm::fabsf;
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

// ── Constants ───────────────────────────────────────────────────────────────

/// HR threshold for tachycardia (BPM).
const TACHY_THRESH: f32 = 100.0;

/// HR threshold for bradycardia (BPM).
const BRADY_THRESH: f32 = 50.0;

/// Consecutive seconds above/below threshold before alert.
const SUSTAINED_SECS: u8 = 10;

/// Missed-beat detection: fractional drop from running average.
const MISSED_BEAT_DROP: f32 = 0.30;

/// RMSSD window size (seconds at ~1 Hz).
const HRV_WINDOW: usize = 30;

/// Normal RMSSD range (ms).  CSI-derived HR is coarser than ECG so the
/// "normal" band is widened.  Values outside trigger HRV_ANOMALY.
const RMSSD_LOW: f32 = 10.0;
const RMSSD_HIGH: f32 = 120.0;

/// Running-average EMA coefficient.
const EMA_ALPHA: f32 = 0.1;

/// Alert cooldown (seconds) to avoid event flooding.
const COOLDOWN_SECS: u16 = 30;

// ── Event IDs ───────────────────────────────────────────────────────────────

pub const EVENT_TACHYCARDIA: i32 = 110;
pub const EVENT_BRADYCARDIA: i32 = 111;
pub const EVENT_MISSED_BEAT: i32 = 112;
pub const EVENT_HRV_ANOMALY: i32 = 113;

// ── State ───────────────────────────────────────────────────────────────────

/// Cardiac arrhythmia detector.
pub struct CardiacArrhythmiaDetector {
    /// EMA of heart rate.
    hr_ema: f32,
    /// Whether the EMA has been initialised.
    ema_init: bool,
    /// Ring buffer of successive RR differences (BPM deltas, 1 Hz).
    rr_diffs: [f32; HRV_WINDOW],
    rr_idx: usize,
    rr_len: usize,
    /// Previous HR sample for delta computation.
    prev_hr: f32,
    prev_hr_init: bool,
    /// Sustained-rate counters.
    tachy_count: u8,
    brady_count: u8,
    /// Per-event cooldowns.
    cd_tachy: u16,
    cd_brady: u16,
    cd_missed: u16,
    cd_hrv: u16,
    /// Frame counter.
    frame_count: u32,
}

impl CardiacArrhythmiaDetector {
    pub const fn new() -> Self {
        Self {
            hr_ema: 0.0,
            ema_init: false,
            rr_diffs: [0.0; HRV_WINDOW],
            rr_idx: 0,
            rr_len: 0,
            prev_hr: 0.0,
            prev_hr_init: false,
            tachy_count: 0,
            brady_count: 0,
            cd_tachy: 0,
            cd_brady: 0,
            cd_missed: 0,
            cd_hrv: 0,
            frame_count: 0,
        }
    }

    /// Process one frame at ~1 Hz.  `hr_bpm` is the host-reported heart rate,
    /// `_phase` is reserved for future RR-interval extraction from CSI phase.
    ///
    /// Returns `&[(event_id, value)]`.
    pub fn process_frame(&mut self, hr_bpm: f32, _phase: f32) -> &[(i32, f32)] {
        self.frame_count += 1;

        // Tick cooldowns.
        self.cd_tachy = self.cd_tachy.saturating_sub(1);
        self.cd_brady = self.cd_brady.saturating_sub(1);
        self.cd_missed = self.cd_missed.saturating_sub(1);
        self.cd_hrv = self.cd_hrv.saturating_sub(1);

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;

        // Ignore invalid / zero / NaN readings.
        // NaN comparisons return false, so we must check explicitly to prevent
        // NaN from contaminating the EMA and RMSSD calculations.
        if !(hr_bpm >= 1.0) {
            return unsafe { &EVENTS[..n] };
        }

        // ── EMA update ──────────────────────────────────────────────────
        if !self.ema_init {
            self.hr_ema = hr_bpm;
            self.ema_init = true;
        } else {
            self.hr_ema += EMA_ALPHA * (hr_bpm - self.hr_ema);
        }

        // ── RR-diff ring buffer (for RMSSD) ─────────────────────────────
        if self.prev_hr_init {
            let diff = hr_bpm - self.prev_hr;
            self.rr_diffs[self.rr_idx] = diff;
            self.rr_idx = (self.rr_idx + 1) % HRV_WINDOW;
            if self.rr_len < HRV_WINDOW {
                self.rr_len += 1;
            }
        }
        self.prev_hr = hr_bpm;
        self.prev_hr_init = true;

        // ── Tachycardia ─────────────────────────────────────────────────
        if hr_bpm > TACHY_THRESH {
            self.tachy_count = self.tachy_count.saturating_add(1);
            if self.tachy_count >= SUSTAINED_SECS && self.cd_tachy == 0 && n < 4 {
                unsafe { EVENTS[n] = (EVENT_TACHYCARDIA, hr_bpm); }
                n += 1;
                self.cd_tachy = COOLDOWN_SECS;
            }
        } else {
            self.tachy_count = 0;
        }

        // ── Bradycardia ─────────────────────────────────────────────────
        if hr_bpm < BRADY_THRESH {
            self.brady_count = self.brady_count.saturating_add(1);
            if self.brady_count >= SUSTAINED_SECS && self.cd_brady == 0 && n < 4 {
                unsafe { EVENTS[n] = (EVENT_BRADYCARDIA, hr_bpm); }
                n += 1;
                self.cd_brady = COOLDOWN_SECS;
            }
        } else {
            self.brady_count = 0;
        }

        // ── Missed beat ─────────────────────────────────────────────────
        if self.ema_init && self.hr_ema > 1.0 {
            let drop_frac = (self.hr_ema - hr_bpm) / self.hr_ema;
            if drop_frac > MISSED_BEAT_DROP && self.cd_missed == 0 && n < 4 {
                unsafe { EVENTS[n] = (EVENT_MISSED_BEAT, hr_bpm); }
                n += 1;
                self.cd_missed = COOLDOWN_SECS;
            }
        }

        // ── HRV (RMSSD) anomaly ─────────────────────────────────────────
        if self.rr_len >= HRV_WINDOW && n < 4 {
            let rmssd = self.compute_rmssd();
            if (rmssd < RMSSD_LOW || rmssd > RMSSD_HIGH) && self.cd_hrv == 0 {
                unsafe { EVENTS[n] = (EVENT_HRV_ANOMALY, rmssd); }
                n += 1;
                self.cd_hrv = COOLDOWN_SECS;
            }
        }

        unsafe { &EVENTS[..n] }
    }

    /// Compute RMSSD from the RR-diff ring buffer.
    ///
    /// RMSSD = sqrt(mean(diff_i^2)) where diff_i are successive differences.
    /// Since host reports BPM (not ms RR intervals), we scale the result.
    fn compute_rmssd(&self) -> f32 {
        if self.rr_len < 2 {
            return 0.0;
        }
        let mut sum_sq = 0.0f32;
        // We need successive differences of successive differences, but our
        // ring buffer already stores successive HR deltas.  We use successive
        // differences of those (second-order) for a proxy of RR variability.
        // For simplicity, use the stored deltas directly:  RMSSD ≈ sqrt(mean(d^2)).
        for i in 0..self.rr_len {
            let d = self.rr_diffs[i];
            sum_sq += d * d;
        }
        let msd = sum_sq / self.rr_len as f32;
        // Convert from BPM^2 to approximate ms-equivalent:
        // At 60 BPM, 1 BPM change ≈ 16.7 ms RR change.  Scale factor ~17.
        sqrtf(msd) * 17.0
    }

    /// Current EMA heart rate.
    pub fn hr_ema(&self) -> f32 {
        self.hr_ema
    }

    /// Frame count.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let d = CardiacArrhythmiaDetector::new();
        assert_eq!(d.frame_count(), 0);
        assert!((d.hr_ema() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_normal_hr_no_events() {
        let mut d = CardiacArrhythmiaDetector::new();
        for _ in 0..60 {
            let ev = d.process_frame(72.0, 0.0);
            for &(t, _) in ev {
                assert!(
                    t != EVENT_TACHYCARDIA && t != EVENT_BRADYCARDIA && t != EVENT_MISSED_BEAT,
                    "no arrhythmia events with normal HR"
                );
            }
        }
    }

    #[test]
    fn test_tachycardia_detection() {
        let mut d = CardiacArrhythmiaDetector::new();
        let mut found = false;
        for _ in 0..20 {
            let ev = d.process_frame(120.0, 0.0);
            for &(t, _) in ev {
                if t == EVENT_TACHYCARDIA { found = true; }
            }
        }
        assert!(found, "tachycardia should trigger with sustained HR > 100");
    }

    #[test]
    fn test_bradycardia_detection() {
        let mut d = CardiacArrhythmiaDetector::new();
        let mut found = false;
        for _ in 0..20 {
            let ev = d.process_frame(40.0, 0.0);
            for &(t, _) in ev {
                if t == EVENT_BRADYCARDIA { found = true; }
            }
        }
        assert!(found, "bradycardia should trigger with sustained HR < 50");
    }

    #[test]
    fn test_missed_beat_detection() {
        let mut d = CardiacArrhythmiaDetector::new();
        // Build up EMA at normal rate.
        for _ in 0..20 {
            d.process_frame(72.0, 0.0);
        }
        // Sudden drop.
        let mut found = false;
        let ev = d.process_frame(40.0, 0.0);
        for &(t, _) in ev {
            if t == EVENT_MISSED_BEAT { found = true; }
        }
        assert!(found, "missed beat should trigger on sudden HR drop > 30%");
    }

    #[test]
    fn test_hrv_anomaly_low_variability() {
        let mut d = CardiacArrhythmiaDetector::new();
        // Feed perfectly constant HR to produce RMSSD ≈ 0 (below RMSSD_LOW).
        let mut found = false;
        for _ in 0..60 {
            let ev = d.process_frame(72.0, 0.0);
            for &(t, _) in ev {
                if t == EVENT_HRV_ANOMALY { found = true; }
            }
        }
        // Constant HR → zero successive differences → RMSSD ~ 0 → below RMSSD_LOW.
        assert!(found, "HRV anomaly should trigger with near-zero variability");
    }

    #[test]
    fn test_cooldown_prevents_flooding() {
        let mut d = CardiacArrhythmiaDetector::new();
        let mut tachy_count = 0u32;
        for _ in 0..100 {
            let ev = d.process_frame(120.0, 0.0);
            for &(t, _) in ev {
                if t == EVENT_TACHYCARDIA { tachy_count += 1; }
            }
        }
        // With a 30-second cooldown over 100 frames, we should see <=4 events.
        assert!(tachy_count <= 4, "cooldown should prevent event flooding, got {}", tachy_count);
    }

    #[test]
    fn test_ema_tracks_hr() {
        let mut d = CardiacArrhythmiaDetector::new();
        for _ in 0..200 {
            d.process_frame(80.0, 0.0);
        }
        assert!((d.hr_ema() - 80.0).abs() < 1.0, "EMA should converge to steady HR");
    }
}
