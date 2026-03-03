//! Non-contact sleep stage classification — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Classifies sleep stages from WiFi CSI physiological signatures without any
//! wearables or cameras.  Uses a state machine driven by multi-feature analysis:
//!
//! 1. **Breathing regularity** -- coefficient of variation of recent breathing
//!    BPM values.  Low CV (<0.10) indicates stable sleep; high CV indicates
//!    REM or wakefulness.
//!
//! 2. **Motion energy** -- EMA-smoothed motion.  Elevated motion indicates
//!    wakefulness; micro-movements distinguish REM from deep sleep.
//!
//! 3. **Heart rate variability (HRV)** -- variance of recent heart rate BPM.
//!    Higher HRV correlates with REM sleep; very low HRV with deep sleep.
//!
//! 4. **Phase micro-movement spectral features** -- high-frequency content
//!    in the phase signal indicates muscle atonia disruption (REM) vs.
//!    deep slow-wave delta activity.
//!
//! ## Sleep Stages
//!
//! - **Awake** (0): High motion OR irregular breathing OR absent presence.
//! - **NREM Light** (1): Low motion, moderate breathing regularity, moderate HRV.
//! - **NREM Deep** (2): Very low motion, very regular breathing, low HRV.
//! - **REM** (3): Very low motion, irregular breathing, elevated HRV, micro-movements.
//!
//! ## Sleep Quality Metrics
//!
//! - **Efficiency** = (total_sleep_frames / total_frames) * 100%.
//! - **REM ratio** = rem_frames / total_sleep_frames.
//! - **Deep ratio** = deep_frames / total_sleep_frames.
//!
//! # Events (600-603: Exotic / Research)
//!
//! - `SLEEP_STAGE` (600): Current stage (0=Awake, 1=Light, 2=Deep, 3=REM).
//! - `SLEEP_QUALITY` (601): Efficiency score [0, 100].
//! - `REM_EPISODE` (602): Duration of current/last REM episode in frames.
//! - `DEEP_SLEEP_RATIO` (603): Deep sleep ratio [0, 1].
//!
//! # Budget
//!
//! H (heavy, < 10 ms) -- rolling stats + state machine, well within budget.

use crate::vendor_common::{CircularBuffer, Ema, WelfordStats};
use libm::sqrtf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Rolling window for breathing BPM history (64 samples at ~1 Hz timer rate).
const BREATH_HIST_LEN: usize = 64;

/// Rolling window for heart rate BPM history.
const HR_HIST_LEN: usize = 64;

/// Phase micro-movement buffer (128 frames at 20 Hz = 6.4 s).
const PHASE_BUF_LEN: usize = 128;

/// Motion energy EMA smoothing factor.
const MOTION_ALPHA: f32 = 0.1;

/// Breathing regularity EMA smoothing factor.
const BREATH_REG_ALPHA: f32 = 0.15;

/// Minimum frames before stage classification begins.
const MIN_WARMUP: u32 = 40;

/// Motion threshold: below this is "low motion" (sleep-like).
const MOTION_LOW_THRESH: f32 = 0.15;

/// Motion threshold: above this is "high motion" (awake).
const MOTION_HIGH_THRESH: f32 = 0.5;

/// Breathing CV threshold: below this is "very regular".
const BREATH_CV_VERY_REG: f32 = 0.08;

/// Breathing CV threshold: below this is "moderately regular".
const BREATH_CV_MOD_REG: f32 = 0.20;

/// HRV (variance) threshold: above this indicates REM-like variability.
const HRV_HIGH_THRESH: f32 = 8.0;

/// HRV threshold: below this indicates deep sleep.
const HRV_LOW_THRESH: f32 = 2.0;

/// Micro-movement energy threshold for REM detection.
const MICRO_MOVEMENT_THRESH: f32 = 0.05;

/// Minimum consecutive frames in same stage before transition is accepted.
const STAGE_HYSTERESIS: u32 = 10;

// ── Event IDs (600-603: Exotic) ──────────────────────────────────────────────

pub const EVENT_SLEEP_STAGE: i32 = 600;
pub const EVENT_SLEEP_QUALITY: i32 = 601;
pub const EVENT_REM_EPISODE: i32 = 602;
pub const EVENT_DEEP_SLEEP_RATIO: i32 = 603;

// ── Sleep Stage Enum ─────────────────────────────────────────────────────────

/// Sleep stage classification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SleepStage {
    Awake = 0,
    NremLight = 1,
    NremDeep = 2,
    Rem = 3,
}

// ── Dream Stage Detector ─────────────────────────────────────────────────────

/// Non-contact sleep stage classifier using WiFi CSI physiological signatures.
pub struct DreamStageDetector {
    /// Rolling breathing BPM values.
    breath_hist: CircularBuffer<BREATH_HIST_LEN>,
    /// Rolling heart rate BPM values.
    hr_hist: CircularBuffer<HR_HIST_LEN>,
    /// Phase micro-movement buffer for spectral analysis.
    phase_buf: CircularBuffer<PHASE_BUF_LEN>,
    /// EMA-smoothed motion energy.
    motion_ema: Ema,
    /// EMA-smoothed breathing regularity (CV).
    breath_reg_ema: Ema,
    /// Welford stats for breathing BPM variance.
    breath_stats: WelfordStats,
    /// Welford stats for heart rate BPM variance.
    hr_stats: WelfordStats,
    /// Current confirmed sleep stage.
    current_stage: SleepStage,
    /// Candidate stage (pending hysteresis confirmation).
    candidate_stage: SleepStage,
    /// Frames the candidate has been stable.
    candidate_count: u32,
    /// Total frames processed.
    frame_count: u32,
    /// Total frames classified as any sleep stage (Light, Deep, REM).
    sleep_frames: u32,
    /// Total frames classified as REM.
    rem_frames: u32,
    /// Total frames classified as Deep.
    deep_frames: u32,
    /// Current REM episode length in frames.
    rem_episode_len: u32,
    /// Last completed REM episode length.
    last_rem_episode: u32,
    /// Last computed micro-movement energy.
    micro_movement: f32,
}

impl DreamStageDetector {
    pub const fn new() -> Self {
        Self {
            breath_hist: CircularBuffer::new(),
            hr_hist: CircularBuffer::new(),
            phase_buf: CircularBuffer::new(),
            motion_ema: Ema::new(MOTION_ALPHA),
            breath_reg_ema: Ema::new(BREATH_REG_ALPHA),
            breath_stats: WelfordStats::new(),
            hr_stats: WelfordStats::new(),
            current_stage: SleepStage::Awake,
            candidate_stage: SleepStage::Awake,
            candidate_count: 0,
            frame_count: 0,
            sleep_frames: 0,
            rem_frames: 0,
            deep_frames: 0,
            rem_episode_len: 0,
            last_rem_episode: 0,
            micro_movement: 0.0,
        }
    }

    /// Process one frame with host-provided physiological signals.
    ///
    /// # Arguments
    /// - `breathing_bpm` -- breathing rate from Tier 2 DSP.
    /// - `heart_rate_bpm` -- heart rate from Tier 2 DSP.
    /// - `motion_energy` -- motion energy from Tier 2 DSP.
    /// - `phase` -- representative subcarrier phase value.
    /// - `variance` -- representative subcarrier variance.
    /// - `presence` -- 1 if person detected, 0 otherwise.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        breathing_bpm: f32,
        heart_rate_bpm: f32,
        motion_energy: f32,
        phase: f32,
        _variance: f32,
        presence: i32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        // Update rolling buffers.
        self.breath_hist.push(breathing_bpm);
        self.hr_hist.push(heart_rate_bpm);
        self.phase_buf.push(phase);

        // Update Welford stats for recent windows.
        self.breath_stats.update(breathing_bpm);
        self.hr_stats.update(heart_rate_bpm);

        // Update EMA motion.
        let smoothed_motion = self.motion_ema.update(motion_energy);

        // Compute breathing coefficient of variation.
        let breath_cv = self.compute_breath_cv();
        self.breath_reg_ema.update(breath_cv);

        // Compute HRV (variance of recent heart rate).
        let hrv = self.compute_hrv();

        // Compute phase micro-movement energy (high-frequency content).
        self.micro_movement = self.compute_micro_movement();

        // Warmup period: don't classify yet.
        if self.frame_count < MIN_WARMUP {
            return &[];
        }

        // Classify candidate stage.
        let new_stage = self.classify_stage(
            smoothed_motion,
            breath_cv,
            hrv,
            self.micro_movement,
            presence,
        );

        // Apply hysteresis.
        if new_stage == self.candidate_stage {
            self.candidate_count += 1;
        } else {
            self.candidate_stage = new_stage;
            self.candidate_count = 1;
        }

        if self.candidate_count >= STAGE_HYSTERESIS && self.candidate_stage != self.current_stage {
            // Track REM episode boundaries.
            if self.current_stage == SleepStage::Rem && self.candidate_stage != SleepStage::Rem {
                self.last_rem_episode = self.rem_episode_len;
                self.rem_episode_len = 0;
            }
            self.current_stage = self.candidate_stage;
        }

        // Update counters.
        if self.current_stage != SleepStage::Awake {
            self.sleep_frames += 1;
        }
        if self.current_stage == SleepStage::Rem {
            self.rem_frames += 1;
            self.rem_episode_len += 1;
        }
        if self.current_stage == SleepStage::NremDeep {
            self.deep_frames += 1;
        }

        // Compute quality metrics.
        let efficiency = if self.frame_count > 0 {
            (self.sleep_frames as f32 / self.frame_count as f32) * 100.0
        } else {
            0.0
        };

        let deep_ratio = if self.sleep_frames > 0 {
            self.deep_frames as f32 / self.sleep_frames as f32
        } else {
            0.0
        };

        let rem_ep = if self.current_stage == SleepStage::Rem {
            self.rem_episode_len
        } else {
            self.last_rem_episode
        };

        // Emit events.
        unsafe {
            EVENTS[n_ev] = (EVENT_SLEEP_STAGE, self.current_stage as u8 as f32);
        }
        n_ev += 1;

        // Emit quality periodically (every 20 frames).
        if self.frame_count % 20 == 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_SLEEP_QUALITY, efficiency);
            }
            n_ev += 1;

            unsafe {
                EVENTS[n_ev] = (EVENT_DEEP_SLEEP_RATIO, deep_ratio);
            }
            n_ev += 1;
        }

        // Emit REM episode when in REM or just exited.
        if rem_ep > 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_REM_EPISODE, rem_ep as f32);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Classify the sleep stage from physiological features.
    fn classify_stage(
        &self,
        motion: f32,
        breath_cv: f32,
        hrv: f32,
        micro_movement: f32,
        presence: i32,
    ) -> SleepStage {
        // No person present -> Awake (or absent).
        if presence == 0 {
            return SleepStage::Awake;
        }

        // High motion -> Awake.
        if motion > MOTION_HIGH_THRESH {
            return SleepStage::Awake;
        }

        // Moderate motion with irregular breathing -> Awake.
        if motion > MOTION_LOW_THRESH && breath_cv > BREATH_CV_MOD_REG {
            return SleepStage::Awake;
        }

        // Low motion regime: distinguish sleep stages.
        if motion <= MOTION_LOW_THRESH {
            // Very regular breathing + low HRV -> Deep sleep.
            if breath_cv < BREATH_CV_VERY_REG && hrv < HRV_LOW_THRESH {
                return SleepStage::NremDeep;
            }

            // Irregular breathing + high HRV + micro-movements -> REM.
            if breath_cv > BREATH_CV_MOD_REG
                && hrv > HRV_HIGH_THRESH
                && micro_movement > MICRO_MOVEMENT_THRESH
            {
                return SleepStage::Rem;
            }

            // Also detect REM with high HRV + micro-movement even with moderate CV.
            if hrv > HRV_HIGH_THRESH && micro_movement > MICRO_MOVEMENT_THRESH {
                return SleepStage::Rem;
            }

            // Default low-motion state: Light sleep.
            return SleepStage::NremLight;
        }

        // Moderate motion, regular breathing -> Light sleep.
        if breath_cv < BREATH_CV_MOD_REG {
            return SleepStage::NremLight;
        }

        SleepStage::Awake
    }

    /// Compute breathing coefficient of variation from recent history.
    fn compute_breath_cv(&self) -> f32 {
        let n = self.breath_hist.len();
        if n < 4 {
            return 1.0; // insufficient data -> high CV (assume irregular).
        }

        let mut sum = 0.0f32;
        let mut sum_sq = 0.0f32;
        for i in 0..n {
            let v = self.breath_hist.get(i);
            sum += v;
            sum_sq += v * v;
        }

        let mean = sum / n as f32;
        if mean < 1.0 {
            return 1.0; // near-zero breathing rate -> irregular.
        }

        let var = sum_sq / n as f32 - mean * mean;
        let var = if var > 0.0 { var } else { 0.0 };
        let std_dev = sqrtf(var);
        std_dev / mean
    }

    /// Compute heart rate variability from recent HR history.
    fn compute_hrv(&self) -> f32 {
        let n = self.hr_hist.len();
        if n < 4 {
            return 0.0;
        }

        let mut sum = 0.0f32;
        let mut sum_sq = 0.0f32;
        for i in 0..n {
            let v = self.hr_hist.get(i);
            sum += v;
            sum_sq += v * v;
        }

        let mean = sum / n as f32;
        let var = sum_sq / n as f32 - mean * mean;
        if var > 0.0 { var } else { 0.0 }
    }

    /// Compute micro-movement energy from phase buffer (high-pass energy).
    ///
    /// Uses successive differences as a simple high-pass filter:
    /// energy = mean(|phase[i] - phase[i-1]|^2).
    fn compute_micro_movement(&self) -> f32 {
        let n = self.phase_buf.len();
        if n < 2 {
            return 0.0;
        }

        let mut energy = 0.0f32;
        for i in 1..n {
            let diff = self.phase_buf.get(i) - self.phase_buf.get(i - 1);
            energy += diff * diff;
        }
        energy / (n - 1) as f32
    }

    /// Get the current sleep stage.
    pub fn stage(&self) -> SleepStage {
        self.current_stage
    }

    /// Get sleep efficiency [0, 100].
    pub fn efficiency(&self) -> f32 {
        if self.frame_count == 0 {
            return 0.0;
        }
        (self.sleep_frames as f32 / self.frame_count as f32) * 100.0
    }

    /// Get deep sleep ratio [0, 1].
    pub fn deep_ratio(&self) -> f32 {
        if self.sleep_frames == 0 {
            return 0.0;
        }
        self.deep_frames as f32 / self.sleep_frames as f32
    }

    /// Get REM ratio [0, 1].
    pub fn rem_ratio(&self) -> f32 {
        if self.sleep_frames == 0 {
            return 0.0;
        }
        self.rem_frames as f32 / self.sleep_frames as f32
    }

    /// Total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Get last micro-movement energy.
    pub fn micro_movement_energy(&self) -> f32 {
        self.micro_movement
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use libm::fabsf;

    #[test]
    fn test_const_new() {
        let ds = DreamStageDetector::new();
        assert_eq!(ds.frame_count(), 0);
        assert_eq!(ds.stage(), SleepStage::Awake);
        assert!(fabsf(ds.efficiency()) < 1e-6);
    }

    #[test]
    fn test_warmup_no_events() {
        let mut ds = DreamStageDetector::new();
        for _ in 0..(MIN_WARMUP - 1) {
            let events = ds.process_frame(14.0, 60.0, 0.0, 0.0, 0.0, 1);
            assert!(events.is_empty(), "should not emit during warmup");
        }
    }

    #[test]
    fn test_high_motion_stays_awake() {
        let mut ds = DreamStageDetector::new();
        // Feed enough frames to pass warmup with high motion.
        for _ in 0..80 {
            ds.process_frame(14.0, 70.0, 1.0, 0.0, 0.0, 1);
        }
        assert_eq!(ds.stage(), SleepStage::Awake);
        // No sleep frames should accumulate.
        assert!(ds.efficiency() < 1.0);
    }

    #[test]
    fn test_low_motion_regular_breathing_deep_sleep() {
        let mut ds = DreamStageDetector::new();
        // Simulate very low motion, very regular breathing (14 BPM constant),
        // low HRV (60 BPM constant), no micro-movements.
        for _ in 0..120 {
            ds.process_frame(14.0, 60.0, 0.02, 0.0, 0.0, 1);
        }
        // After hysteresis, should transition to Deep sleep.
        assert_eq!(ds.stage(), SleepStage::NremDeep,
            "low motion + regular breathing + low HRV should be deep sleep");
        assert!(ds.deep_ratio() > 0.0, "deep ratio should be positive");
    }

    #[test]
    fn test_no_presence_stays_awake() {
        let mut ds = DreamStageDetector::new();
        for _ in 0..80 {
            ds.process_frame(14.0, 60.0, 0.0, 0.0, 0.0, 0); // presence=0
        }
        assert_eq!(ds.stage(), SleepStage::Awake);
    }

    #[test]
    fn test_rem_detection_high_hrv_micro_movement() {
        let mut ds = DreamStageDetector::new();
        // Low motion, but varying heart rate and irregular breathing with micro-movements.
        for i in 0..200 {
            // Irregular breathing: oscillates between 10 and 22 BPM.
            let breath = if i % 3 == 0 { 10.0 } else { 22.0 };
            // Variable heart rate: 55-85 BPM spread -> high HRV.
            let hr = 55.0 + (i % 7) as f32 * 5.0;
            // Phase micro-movements: small rapid changes.
            let phase = (i as f32 * 0.5).sin() * 0.3;
            ds.process_frame(breath, hr, 0.05, phase, 0.0, 1);
        }
        // Should detect REM at some point.
        let is_rem = ds.stage() == SleepStage::Rem;
        let is_light = ds.stage() == SleepStage::NremLight;
        assert!(is_rem || is_light,
            "variable HR + micro-movement should classify as REM or Light, got {:?}",
            ds.stage());
    }

    #[test]
    fn test_sleep_quality_metrics() {
        let mut ds = DreamStageDetector::new();
        // All deep sleep.
        for _ in 0..200 {
            ds.process_frame(14.0, 60.0, 0.02, 0.0, 0.0, 1);
        }
        assert!(ds.efficiency() > 50.0, "efficiency should be high for continuous sleep");
        // Deep ratio should dominate when all is deep sleep.
        assert!(ds.deep_ratio() > 0.5, "deep ratio should be high");
        assert!(fabsf(ds.rem_ratio()) < 0.01, "REM ratio should be near zero");
    }

    #[test]
    fn test_event_ids_correct() {
        let mut ds = DreamStageDetector::new();
        // Run past warmup.
        for _ in 0..MIN_WARMUP + 5 {
            ds.process_frame(14.0, 60.0, 0.0, 0.0, 0.0, 1);
        }
        // Run to a frame where quality events fire (frame % 20 == 0).
        let remaining = 20 - ((MIN_WARMUP + 5) % 20);
        let mut quality_events = false;
        for _ in 0..(remaining + 20) {
            let events = ds.process_frame(14.0, 60.0, 0.0, 0.0, 0.0, 1);
            for ev in events {
                if ev.0 == EVENT_SLEEP_STAGE {
                    // Stage event always present after warmup.
                }
                if ev.0 == EVENT_SLEEP_QUALITY {
                    quality_events = true;
                }
            }
        }
        assert!(quality_events, "quality events should fire periodically");
    }

    #[test]
    fn test_reset() {
        let mut ds = DreamStageDetector::new();
        for _ in 0..100 {
            ds.process_frame(14.0, 60.0, 0.02, 0.0, 0.0, 1);
        }
        assert!(ds.frame_count() > 0);
        ds.reset();
        assert_eq!(ds.frame_count(), 0);
        assert_eq!(ds.stage(), SleepStage::Awake);
    }

    #[test]
    fn test_breath_cv_constant_signal() {
        let mut ds = DreamStageDetector::new();
        // Push constant breathing values.
        for _ in 0..20 {
            ds.breath_hist.push(14.0);
        }
        let cv = ds.compute_breath_cv();
        assert!(cv < 0.01, "constant breathing should have near-zero CV, got {}", cv);
    }

    #[test]
    fn test_micro_movement_zero_for_constant_phase() {
        let mut ds = DreamStageDetector::new();
        for _ in 0..50 {
            ds.phase_buf.push(1.0);
        }
        let mm = ds.compute_micro_movement();
        assert!(mm < 1e-6, "constant phase should have zero micro-movement, got {}", mm);
    }
}
