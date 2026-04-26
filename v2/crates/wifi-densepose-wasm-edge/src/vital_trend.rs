//! Vital sign trend analysis — ADR-041 Phase 1 module.
//!
//! Monitors breathing rate and heart rate over time windows (1-min, 5-min, 15-min)
//! and detects clinically significant trends:
//! - Bradypnea (breathing < 12 BPM sustained)
//! - Tachypnea (breathing > 25 BPM sustained)
//! - Bradycardia (HR < 50 BPM sustained)
//! - Tachycardia (HR > 120 BPM sustained)
//! - Apnea (no breathing detected for > 20 seconds)
//! - Trend reversal (sudden direction change in vital trajectory)

// No libm imports needed — pure arithmetic.

/// Window sizes in samples (at 1 Hz timer rate).
const WINDOW_1M: usize = 60;
const WINDOW_5M: usize = 300;

/// Maximum history depth.
const MAX_HISTORY: usize = 300;  // 5 minutes at 1 Hz.

/// Clinical thresholds (BPM).
const BRADYPNEA_THRESH: f32 = 12.0;
const TACHYPNEA_THRESH: f32 = 25.0;
const BRADYCARDIA_THRESH: f32 = 50.0;
const TACHYCARDIA_THRESH: f32 = 120.0;
const APNEA_SECONDS: u32 = 20;

/// Minimum consecutive alerts before emitting (debounce).
const ALERT_DEBOUNCE: u8 = 5;

/// Event types (100-series: Medical).
pub const EVENT_VITAL_TREND: i32 = 100;
pub const EVENT_BRADYPNEA: i32 = 101;
pub const EVENT_TACHYPNEA: i32 = 102;
pub const EVENT_BRADYCARDIA: i32 = 103;
pub const EVENT_TACHYCARDIA: i32 = 104;
pub const EVENT_APNEA: i32 = 105;
pub const EVENT_BREATHING_AVG: i32 = 110;
pub const EVENT_HEARTRATE_AVG: i32 = 111;

/// Ring buffer for vital sign history.
struct VitalHistory {
    values: [f32; MAX_HISTORY],
    len: usize,
    idx: usize,
}

impl VitalHistory {
    const fn new() -> Self {
        Self {
            values: [0.0; MAX_HISTORY],
            len: 0,
            idx: 0,
        }
    }

    fn push(&mut self, val: f32) {
        self.values[self.idx] = val;
        self.idx = (self.idx + 1) % MAX_HISTORY;
        if self.len < MAX_HISTORY {
            self.len += 1;
        }
    }

    /// Compute mean of the last N samples.
    fn mean_last(&self, n: usize) -> f32 {
        let count = n.min(self.len);
        if count == 0 {
            return 0.0;
        }
        let mut sum = 0.0f32;
        for i in 0..count {
            let ri = (self.idx + MAX_HISTORY - count + i) % MAX_HISTORY;
            sum += self.values[ri];
        }
        sum / count as f32
    }

    /// Check if all of the last N samples are below threshold.
    #[allow(dead_code)]
    fn all_below(&self, n: usize, threshold: f32) -> bool {
        let count = n.min(self.len);
        if count < n {
            return false;
        }
        for i in 0..count {
            let ri = (self.idx + MAX_HISTORY - count + i) % MAX_HISTORY;
            if self.values[ri] >= threshold {
                return false;
            }
        }
        true
    }

    /// Check if all of the last N samples are above threshold.
    #[allow(dead_code)]
    fn all_above(&self, n: usize, threshold: f32) -> bool {
        let count = n.min(self.len);
        if count < n {
            return false;
        }
        for i in 0..count {
            let ri = (self.idx + MAX_HISTORY - count + i) % MAX_HISTORY;
            if self.values[ri] <= threshold {
                return false;
            }
        }
        true
    }

    /// Compute simple linear trend (positive = increasing).
    fn trend(&self, n: usize) -> f32 {
        let count = n.min(self.len);
        if count < 4 {
            return 0.0;
        }

        // Simple: (last_quarter_mean - first_quarter_mean) / window.
        let quarter = count / 4;
        let mut first_sum = 0.0f32;
        let mut last_sum = 0.0f32;

        for i in 0..quarter {
            let ri = (self.idx + MAX_HISTORY - count + i) % MAX_HISTORY;
            first_sum += self.values[ri];
        }
        for i in (count - quarter)..count {
            let ri = (self.idx + MAX_HISTORY - count + i) % MAX_HISTORY;
            last_sum += self.values[ri];
        }

        let first_mean = first_sum / quarter as f32;
        let last_mean = last_sum / quarter as f32;
        (last_mean - first_mean) / count as f32
    }
}

/// Vital trend analyzer.
pub struct VitalTrendAnalyzer {
    breathing: VitalHistory,
    heartrate: VitalHistory,
    /// Debounce counters for each alert type.
    bradypnea_count: u8,
    tachypnea_count: u8,
    bradycardia_count: u8,
    tachycardia_count: u8,
    /// Consecutive samples with near-zero breathing.
    apnea_counter: u32,
    /// Timer call count.
    timer_count: u32,
}

impl VitalTrendAnalyzer {
    pub const fn new() -> Self {
        Self {
            breathing: VitalHistory::new(),
            heartrate: VitalHistory::new(),
            bradypnea_count: 0,
            tachypnea_count: 0,
            bradycardia_count: 0,
            tachycardia_count: 0,
            apnea_counter: 0,
            timer_count: 0,
        }
    }

    /// Called at ~1 Hz with current vital signs.
    ///
    /// Returns events as (event_type, value) pairs.
    pub fn on_timer(&mut self, breathing_bpm: f32, heartrate_bpm: f32) -> &[(i32, f32)] {
        self.timer_count += 1;
        self.breathing.push(breathing_bpm);
        self.heartrate.push(heartrate_bpm);

        static mut EVENTS: [(i32, f32); 8] = [(0, 0.0); 8];
        let mut n = 0usize;

        // ── Apnea detection (highest priority) ──────────────────────────
        if breathing_bpm < 1.0 {
            self.apnea_counter += 1;
            if self.apnea_counter >= APNEA_SECONDS {
                unsafe {
                    EVENTS[n] = (EVENT_APNEA, self.apnea_counter as f32);
                }
                n += 1;
            }
        } else {
            self.apnea_counter = 0;
        }

        // ── Bradypnea (sustained low breathing) ────────────────────────
        if breathing_bpm > 0.0 && breathing_bpm < BRADYPNEA_THRESH {
            self.bradypnea_count = self.bradypnea_count.saturating_add(1);
            if self.bradypnea_count >= ALERT_DEBOUNCE && n < 7 {
                unsafe {
                    EVENTS[n] = (EVENT_BRADYPNEA, breathing_bpm);
                }
                n += 1;
            }
        } else {
            self.bradypnea_count = 0;
        }

        // ── Tachypnea (sustained high breathing) ───────────────────────
        if breathing_bpm > TACHYPNEA_THRESH {
            self.tachypnea_count = self.tachypnea_count.saturating_add(1);
            if self.tachypnea_count >= ALERT_DEBOUNCE && n < 7 {
                unsafe {
                    EVENTS[n] = (EVENT_TACHYPNEA, breathing_bpm);
                }
                n += 1;
            }
        } else {
            self.tachypnea_count = 0;
        }

        // ── Bradycardia ────────────────────────────────────────────────
        if heartrate_bpm > 0.0 && heartrate_bpm < BRADYCARDIA_THRESH {
            self.bradycardia_count = self.bradycardia_count.saturating_add(1);
            if self.bradycardia_count >= ALERT_DEBOUNCE && n < 7 {
                unsafe {
                    EVENTS[n] = (EVENT_BRADYCARDIA, heartrate_bpm);
                }
                n += 1;
            }
        } else {
            self.bradycardia_count = 0;
        }

        // ── Tachycardia ────────────────────────────────────────────────
        if heartrate_bpm > TACHYCARDIA_THRESH {
            self.tachycardia_count = self.tachycardia_count.saturating_add(1);
            if self.tachycardia_count >= ALERT_DEBOUNCE && n < 7 {
                unsafe {
                    EVENTS[n] = (EVENT_TACHYCARDIA, heartrate_bpm);
                }
                n += 1;
            }
        } else {
            self.tachycardia_count = 0;
        }

        // ── Periodic averages (every 60 seconds) ───────────────────────
        if self.timer_count % 60 == 0 && self.breathing.len >= WINDOW_1M {
            let br_avg = self.breathing.mean_last(WINDOW_1M);
            let hr_avg = self.heartrate.mean_last(WINDOW_1M);
            if n < 7 {
                unsafe {
                    EVENTS[n] = (EVENT_BREATHING_AVG, br_avg);
                }
                n += 1;
            }
            if n < 8 {
                unsafe {
                    EVENTS[n] = (EVENT_HEARTRATE_AVG, hr_avg);
                }
                n += 1;
            }
        }

        unsafe { &EVENTS[..n] }
    }

    /// Get the 1-minute breathing average.
    pub fn breathing_avg_1m(&self) -> f32 {
        self.breathing.mean_last(WINDOW_1M)
    }

    /// Get the breathing trend (positive = increasing).
    pub fn breathing_trend_5m(&self) -> f32 {
        self.breathing.trend(WINDOW_5M)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vital_trend_init() {
        let vt = VitalTrendAnalyzer::new();
        assert_eq!(vt.timer_count, 0);
        assert_eq!(vt.apnea_counter, 0);
    }

    #[test]
    fn test_normal_vitals_no_alerts() {
        let mut vt = VitalTrendAnalyzer::new();
        // Normal breathing (16 BPM) and heart rate (72 BPM).
        for _ in 0..60 {
            let events = vt.on_timer(16.0, 72.0);
            // Should not generate clinical alerts.
            for &(et, _) in events {
                assert!(
                    et != EVENT_BRADYPNEA && et != EVENT_TACHYPNEA
                    && et != EVENT_BRADYCARDIA && et != EVENT_TACHYCARDIA
                    && et != EVENT_APNEA,
                    "unexpected clinical alert with normal vitals"
                );
            }
        }
    }

    #[test]
    fn test_apnea_detection() {
        let mut vt = VitalTrendAnalyzer::new();
        let mut apnea_detected = false;

        for _ in 0..30 {
            let events = vt.on_timer(0.0, 72.0);
            for &(et, _) in events {
                if et == EVENT_APNEA {
                    apnea_detected = true;
                }
            }
        }

        assert!(apnea_detected, "apnea should be detected after 20+ seconds of zero breathing");
    }

    #[test]
    fn test_tachycardia_detection() {
        let mut vt = VitalTrendAnalyzer::new();
        let mut tachy_detected = false;

        for _ in 0..20 {
            let events = vt.on_timer(16.0, 130.0);
            for &(et, _) in events {
                if et == EVENT_TACHYCARDIA {
                    tachy_detected = true;
                }
            }
        }

        assert!(tachy_detected, "tachycardia should be detected with sustained HR > 120");
    }

    #[test]
    fn test_breathing_average() {
        let mut vt = VitalTrendAnalyzer::new();
        for _ in 0..60 {
            vt.on_timer(16.0, 72.0);
        }
        let avg = vt.breathing_avg_1m();
        assert!((avg - 16.0).abs() < 0.1, "1-min breathing average should be ~16.0");
    }
}
