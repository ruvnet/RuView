//! Affect computing from physiological CSI signatures — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Infers continuous arousal level and discrete stress/calm/agitation states
//! from WiFi CSI without cameras or microphones.  Uses physiological proxies:
//!
//! 1. **Breathing pattern analysis** -- Rate and regularity.  Stress correlates
//!    with elevated (>20 BPM) and shallow breathing; calm with slow deep
//!    breathing (6-10 BPM) and low variability.
//!
//! 2. **Motion fidgeting detector** -- High-frequency motion energy (successive
//!    differences) captures fidgeting and restless movements associated with
//!    anxiety and agitation.
//!
//! 3. **Heart rate proxy** -- Elevated resting heart rate correlates with
//!    sympathetic nervous system activation (stress/anxiety).
//!
//! 4. **Phase variance** -- Rapid phase fluctuations indicate sharp body
//!    movements typical of agitation.
//!
//! ## Output Model
//!
//! The primary output is a continuous **arousal level** [0, 1]:
//! - 0.0 = deep calm / relaxation.
//! - 0.5 = neutral baseline.
//! - 1.0 = high arousal / stress / agitation.
//!
//! Secondary outputs are threshold-based detections of discrete states.
//!
//! # Events (610-613: Exotic / Research)
//!
//! - `AROUSAL_LEVEL` (610): Continuous arousal [0, 1].
//! - `STRESS_INDEX` (611): Stress index [0, 1] (elevated breathing + HR + fidget).
//! - `CALM_DETECTED` (612): 1.0 when calm state detected, 0.0 otherwise.
//! - `AGITATION_DETECTED` (613): 1.0 when agitation detected, 0.0 otherwise.
//!
//! # Budget
//!
//! H (heavy, < 10 ms) -- rolling statistics + weighted scoring.

use crate::vendor_common::{CircularBuffer, Ema, WelfordStats};
use libm::sqrtf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Rolling window for breathing BPM history.
const BREATH_HIST_LEN: usize = 32;

/// Rolling window for heart rate history.
const HR_HIST_LEN: usize = 32;

/// Motion energy history for fidget detection.
const MOTION_HIST_LEN: usize = 64;

/// Phase variance history buffer.
const PHASE_VAR_HIST_LEN: usize = 32;

/// EMA smoothing for arousal output.
const AROUSAL_ALPHA: f32 = 0.12;

/// EMA smoothing for stress index.
const STRESS_ALPHA: f32 = 0.10;

/// EMA smoothing for motion fidget energy.
const FIDGET_ALPHA: f32 = 0.15;

/// Minimum frames before classification.
const MIN_WARMUP: u32 = 20;

/// Calm breathing range: 6-10 BPM.
const CALM_BREATH_LOW: f32 = 6.0;
const CALM_BREATH_HIGH: f32 = 10.0;

/// Stress breathing threshold: above 20 BPM.
const STRESS_BREATH_THRESH: f32 = 20.0;

/// Calm motion threshold: very low motion.
const CALM_MOTION_THRESH: f32 = 0.08;

/// Agitation motion threshold: sharp movements.
const AGITATION_MOTION_THRESH: f32 = 0.6;

/// Agitation fidget energy threshold.
const AGITATION_FIDGET_THRESH: f32 = 0.15;

/// Baseline resting heart rate (approximate).
const BASELINE_HR: f32 = 70.0;

/// Heart rate stress contribution scaling (per BPM above baseline).
const HR_STRESS_SCALE: f32 = 0.01;

/// Breathing regularity CV threshold for calm.
const CALM_BREATH_CV_THRESH: f32 = 0.08;

/// Breathing regularity CV threshold for stress/agitation.
const STRESS_BREATH_CV_THRESH: f32 = 0.25;

/// Arousal threshold for calm detection.
const CALM_AROUSAL_THRESH: f32 = 0.25;

/// Arousal threshold for agitation detection.
const AGITATION_AROUSAL_THRESH: f32 = 0.75;

/// Weight: breathing rate contribution to arousal.
const W_BREATH: f32 = 0.30;

/// Weight: heart rate contribution to arousal.
const W_HR: f32 = 0.20;

/// Weight: fidget energy contribution to arousal.
const W_FIDGET: f32 = 0.30;

/// Weight: phase variance contribution to arousal.
const W_PHASE_VAR: f32 = 0.20;

// ── Event IDs (610-613: Exotic) ──────────────────────────────────────────────

pub const EVENT_AROUSAL_LEVEL: i32 = 610;
pub const EVENT_STRESS_INDEX: i32 = 611;
pub const EVENT_CALM_DETECTED: i32 = 612;
pub const EVENT_AGITATION_DETECTED: i32 = 613;

// ── Emotion Detector ─────────────────────────────────────────────────────────

/// Affect computing module using WiFi CSI physiological signatures.
///
/// Outputs continuous arousal level and discrete stress/calm/agitation states.
pub struct EmotionDetector {
    /// Rolling breathing BPM values.
    breath_hist: CircularBuffer<BREATH_HIST_LEN>,
    /// Rolling heart rate BPM values.
    hr_hist: CircularBuffer<HR_HIST_LEN>,
    /// Rolling motion energy for fidget detection.
    motion_hist: CircularBuffer<MOTION_HIST_LEN>,
    /// Rolling phase variance values.
    phase_var_hist: CircularBuffer<PHASE_VAR_HIST_LEN>,
    /// EMA-smoothed arousal level [0, 1].
    arousal_ema: Ema,
    /// EMA-smoothed stress index [0, 1].
    stress_ema: Ema,
    /// EMA-smoothed fidget energy.
    fidget_ema: Ema,
    /// Welford stats for breathing variability.
    breath_stats: WelfordStats,
    /// Current arousal level.
    arousal: f32,
    /// Current stress index.
    stress_index: f32,
    /// Whether calm is detected.
    calm_detected: bool,
    /// Whether agitation is detected.
    agitation_detected: bool,
    /// Total frames processed.
    frame_count: u32,
}

impl EmotionDetector {
    pub const fn new() -> Self {
        Self {
            breath_hist: CircularBuffer::new(),
            hr_hist: CircularBuffer::new(),
            motion_hist: CircularBuffer::new(),
            phase_var_hist: CircularBuffer::new(),
            arousal_ema: Ema::new(AROUSAL_ALPHA),
            stress_ema: Ema::new(STRESS_ALPHA),
            fidget_ema: Ema::new(FIDGET_ALPHA),
            breath_stats: WelfordStats::new(),
            arousal: 0.5,
            stress_index: 0.0,
            calm_detected: false,
            agitation_detected: false,
            frame_count: 0,
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
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        breathing_bpm: f32,
        heart_rate_bpm: f32,
        motion_energy: f32,
        _phase: f32,
        variance: f32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        // Update rolling buffers.
        self.breath_hist.push(breathing_bpm);
        self.hr_hist.push(heart_rate_bpm);
        self.motion_hist.push(motion_energy);
        self.phase_var_hist.push(variance);
        self.breath_stats.update(breathing_bpm);

        // Warmup period.
        if self.frame_count < MIN_WARMUP {
            return &[];
        }

        // ── Feature extraction ──

        // 1. Breathing rate score [0, 1]: higher = more stressed.
        let breath_score = self.compute_breath_score(breathing_bpm);

        // 2. Heart rate score [0, 1]: higher = more stressed.
        let hr_score = self.compute_hr_score(heart_rate_bpm);

        // 3. Fidget energy [0, 1]: computed from motion successive differences.
        let fidget_energy = self.compute_fidget_energy();
        let fidget_score = clamp01(self.fidget_ema.update(fidget_energy));

        // 4. Phase variance score [0, 1]: high variance = agitation.
        let phase_var_score = self.compute_phase_var_score();

        // ── Arousal computation (weighted sum) ──
        let raw_arousal = W_BREATH * breath_score
            + W_HR * hr_score
            + W_FIDGET * fidget_score
            + W_PHASE_VAR * phase_var_score;

        self.arousal = clamp01(self.arousal_ema.update(raw_arousal));

        // ── Stress index (breathing + HR emphasis) ──
        let raw_stress = 0.4 * breath_score + 0.3 * hr_score + 0.2 * fidget_score + 0.1 * phase_var_score;
        self.stress_index = clamp01(self.stress_ema.update(raw_stress));

        // ── Discrete state detection ──
        let breath_cv = self.compute_breath_cv();

        self.calm_detected = self.arousal < CALM_AROUSAL_THRESH
            && motion_energy < CALM_MOTION_THRESH
            && breathing_bpm >= CALM_BREATH_LOW
            && breathing_bpm <= CALM_BREATH_HIGH
            && breath_cv < CALM_BREATH_CV_THRESH;

        self.agitation_detected = self.arousal > AGITATION_AROUSAL_THRESH
            && (motion_energy > AGITATION_MOTION_THRESH
                || fidget_score > AGITATION_FIDGET_THRESH
                || breath_cv > STRESS_BREATH_CV_THRESH);

        // ── Emit events ──
        unsafe {
            EVENTS[n_ev] = (EVENT_AROUSAL_LEVEL, self.arousal);
        }
        n_ev += 1;

        unsafe {
            EVENTS[n_ev] = (EVENT_STRESS_INDEX, self.stress_index);
        }
        n_ev += 1;

        if self.calm_detected {
            unsafe {
                EVENTS[n_ev] = (EVENT_CALM_DETECTED, 1.0);
            }
            n_ev += 1;
        }

        if self.agitation_detected {
            unsafe {
                EVENTS[n_ev] = (EVENT_AGITATION_DETECTED, 1.0);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Compute breathing rate score [0, 1].
    /// Calm range (6-10 BPM) -> ~0.0, stress range (>20 BPM) -> ~1.0.
    fn compute_breath_score(&self, bpm: f32) -> f32 {
        if bpm < CALM_BREATH_LOW {
            // Very low breathing rate is abnormal (apnea-like).
            return 0.3;
        }
        if bpm <= CALM_BREATH_HIGH {
            return 0.0;
        }
        // Linear ramp from calm to stress.
        let score = (bpm - CALM_BREATH_HIGH) / (STRESS_BREATH_THRESH - CALM_BREATH_HIGH);
        clamp01(score)
    }

    /// Compute heart rate score [0, 1].
    fn compute_hr_score(&self, bpm: f32) -> f32 {
        if bpm <= BASELINE_HR {
            return 0.0;
        }
        let score = (bpm - BASELINE_HR) * HR_STRESS_SCALE;
        clamp01(score)
    }

    /// Compute fidget energy from successive motion differences.
    fn compute_fidget_energy(&self) -> f32 {
        let n = self.motion_hist.len();
        if n < 2 {
            return 0.0;
        }

        let mut energy = 0.0f32;
        for i in 1..n {
            let diff = self.motion_hist.get(i) - self.motion_hist.get(i - 1);
            energy += diff * diff;
        }
        energy / (n - 1) as f32
    }

    /// Compute phase variance score [0, 1] from recent phase variance history.
    fn compute_phase_var_score(&self) -> f32 {
        let n = self.phase_var_hist.len();
        if n == 0 {
            return 0.0;
        }

        let mut sum = 0.0f32;
        for i in 0..n {
            sum += self.phase_var_hist.get(i);
        }
        let mean_var = sum / n as f32;

        // Normalize: typical phase variance range is [0, 2].
        clamp01(mean_var / 2.0)
    }

    /// Compute breathing coefficient of variation.
    fn compute_breath_cv(&self) -> f32 {
        let n = self.breath_hist.len();
        if n < 4 {
            return 0.5;
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
            return 1.0;
        }

        let var = sum_sq / n as f32 - mean * mean;
        let var = if var > 0.0 { var } else { 0.0 };
        sqrtf(var) / mean
    }

    /// Get current arousal level [0, 1].
    pub fn arousal(&self) -> f32 {
        self.arousal
    }

    /// Get current stress index [0, 1].
    pub fn stress_index(&self) -> f32 {
        self.stress_index
    }

    /// Whether calm is currently detected.
    pub fn is_calm(&self) -> bool {
        self.calm_detected
    }

    /// Whether agitation is currently detected.
    pub fn is_agitated(&self) -> bool {
        self.agitation_detected
    }

    /// Total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Clamp a value to [0, 1].
fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use libm::fabsf;

    #[test]
    fn test_const_new() {
        let ed = EmotionDetector::new();
        assert_eq!(ed.frame_count(), 0);
        assert!(fabsf(ed.arousal() - 0.5) < 1e-6);
        assert!(!ed.is_calm());
        assert!(!ed.is_agitated());
    }

    #[test]
    fn test_warmup_no_events() {
        let mut ed = EmotionDetector::new();
        for _ in 0..(MIN_WARMUP - 1) {
            let events = ed.process_frame(14.0, 70.0, 0.1, 0.0, 0.1);
            assert!(events.is_empty(), "should not emit during warmup");
        }
    }

    #[test]
    fn test_calm_detection_slow_breathing_low_motion() {
        let mut ed = EmotionDetector::new();
        // Simulate calm: slow breathing (8 BPM), normal HR, very low motion, low variance.
        for _ in 0..200 {
            ed.process_frame(8.0, 65.0, 0.02, 0.0, 0.01);
        }
        // Arousal should be low.
        assert!(ed.arousal() < 0.35,
            "calm conditions should yield low arousal, got {}", ed.arousal());
        assert!(ed.is_calm(),
            "should detect calm with slow breathing and low motion");
    }

    #[test]
    fn test_stress_high_breathing_high_hr() {
        let mut ed = EmotionDetector::new();
        // Simulate stress: fast breathing (25 BPM), elevated HR (100 BPM),
        // fidgety motion (varying), and high phase variance.
        for i in 0..200 {
            let motion = 0.3 + 0.4 * ((i % 5) as f32 / 5.0); // varying = fidget
            ed.process_frame(25.0, 100.0, motion, 0.0, 1.5);
        }
        assert!(ed.arousal() > 0.35,
            "stressed conditions should yield elevated arousal, got {}", ed.arousal());
        assert!(ed.stress_index() > 0.3,
            "stress index should be elevated, got {}", ed.stress_index());
    }

    #[test]
    fn test_agitation_high_motion_irregular_breathing() {
        let mut ed = EmotionDetector::new();
        // Simulate agitation: irregular breathing, high motion (varying = fidgeting),
        // elevated HR, high phase variance.
        for i in 0..200 {
            let breath = if i % 2 == 0 { 28.0 } else { 12.0 }; // very irregular
            let motion = 0.5 + 0.5 * ((i % 3) as f32 / 3.0); // jittery motion
            ed.process_frame(breath, 95.0, motion, 0.0, 2.0);
        }
        assert!(ed.arousal() > 0.3,
            "agitated conditions should yield elevated arousal, got {}", ed.arousal());
    }

    #[test]
    fn test_arousal_always_in_range() {
        let mut ed = EmotionDetector::new();
        // Feed extreme values.
        for _ in 0..100 {
            ed.process_frame(40.0, 150.0, 5.0, 3.14, 10.0);
        }
        assert!(ed.arousal() >= 0.0 && ed.arousal() <= 1.0,
            "arousal must be in [0,1], got {}", ed.arousal());
        assert!(ed.stress_index() >= 0.0 && ed.stress_index() <= 1.0,
            "stress must be in [0,1], got {}", ed.stress_index());
    }

    #[test]
    fn test_event_ids_emitted() {
        let mut ed = EmotionDetector::new();
        // Past warmup.
        for _ in 0..MIN_WARMUP + 5 {
            ed.process_frame(14.0, 70.0, 0.1, 0.0, 0.1);
        }
        let events = ed.process_frame(14.0, 70.0, 0.1, 0.0, 0.1);
        // Should always emit at least arousal and stress.
        assert!(events.len() >= 2, "should emit at least 2 events, got {}", events.len());
        assert_eq!(events[0].0, EVENT_AROUSAL_LEVEL);
        assert_eq!(events[1].0, EVENT_STRESS_INDEX);
    }

    #[test]
    fn test_clamp01() {
        assert!(fabsf(clamp01(-1.0)) < 1e-6);
        assert!(fabsf(clamp01(0.5) - 0.5) < 1e-6);
        assert!(fabsf(clamp01(2.0) - 1.0) < 1e-6);
    }

    #[test]
    fn test_breath_score_calm_range() {
        let ed = EmotionDetector::new();
        // 8 BPM is in calm range [6, 10].
        let score = ed.compute_breath_score(8.0);
        assert!(score < 0.01, "calm breathing should have near-zero score, got {}", score);
    }

    #[test]
    fn test_breath_score_stress_range() {
        let ed = EmotionDetector::new();
        // 25 BPM is above stress threshold.
        let score = ed.compute_breath_score(25.0);
        assert!(score > 0.5, "stressed breathing should have high score, got {}", score);
    }

    #[test]
    fn test_reset() {
        let mut ed = EmotionDetector::new();
        for _ in 0..100 {
            ed.process_frame(14.0, 70.0, 0.1, 0.0, 0.1);
        }
        assert!(ed.frame_count() > 0);
        ed.reset();
        assert_eq!(ed.frame_count(), 0);
        assert!(fabsf(ed.arousal() - 0.5) < 1e-6);
    }
}
