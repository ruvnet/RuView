//! Happiness score from WiFi CSI physiological proxies -- ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Combines six physiological proxies extracted from CSI into a composite
//! happiness score [0, 1]:
//!
//! 1. **Gait speed** -- Doppler proxy from phase rate-of-change.  Happy people
//!    walk approximately 12% faster than neutral baseline.
//!
//! 2. **Stride regularity** -- Variance of step intervals from successive phase
//!    differences.  Regular strides correlate with confidence and positive affect.
//!
//! 3. **Movement fluidity** -- Smoothness of phase trajectory (second derivative).
//!    Jerky motion indicates anxiety; smooth motion indicates relaxation.
//!
//! 4. **Breathing calm** -- Inverse of breathing rate, extracted from 0.15-0.5 Hz
//!    phase oscillation.  Slow, deep breathing correlates with positive mood.
//!
//! 5. **Posture score** -- Amplitude spread across subcarrier groups.  Upright
//!    posture scatters signal across more subcarriers than slouched.
//!
//! 6. **Dwell time** -- Fraction of recent frames with presence in the sensing
//!    zone.  Longer dwell in social spaces correlates with engagement.
//!
//! The composite happiness score is a weighted sum of these six features,
//! EMA-smoothed for temporal stability.
//!
//! An 8-dimensional "happiness vector" is also produced for ingestion into a
//! Cognitum Seed vector store (dim=8).
//!
//! # Events (690-694: Exotic / Research)
//!
//! - `HAPPINESS_SCORE` (690): Composite happiness [0.0 = sad, 0.5 = neutral, 1.0 = happy].
//! - `GAIT_ENERGY` (691): Normalized gait speed/stride score [0, 1].
//! - `AFFECT_VALENCE` (692): Emotional valence from breathing + motion [0, 1].
//! - `SOCIAL_ENERGY` (693): Group animation/interaction level [0, 1].
//! - `TRANSIT_DIRECTION` (694): 1.0 = entering, 0.0 = exiting (from motion trend).
//!
//! # Budget
//!
//! H (heavy, < 10 ms) -- rolling statistics + weighted scoring.

use crate::vendor_common::{CircularBuffer, Ema, WelfordStats};
use libm::fabsf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Rolling window for phase rate-of-change (gait speed proxy).
/// ESP32: 16 frames at 20 Hz = 0.8s — sufficient for step detection.
const PHASE_ROC_LEN: usize = 16;

/// Rolling window for step interval detection.
const STEP_INTERVAL_LEN: usize = 16;

/// Rolling window for movement fluidity (second derivative of phase).
/// ESP32: 16 frames captures 2-3 stride cycles at walking cadence.
const FLUIDITY_BUF_LEN: usize = 16;

/// Rolling window for breathing rate history.
/// ESP32: 16 samples at 1 Hz timer rate = 16 seconds of breathing data.
const BREATH_HIST_LEN: usize = 16;

/// Rolling window for amplitude spread (posture).
/// ESP32: 8 samples is enough for posture averaging.
const AMP_SPREAD_LEN: usize = 8;

/// Rolling window for presence/dwell tracking.
/// ESP32: 32 frames at 20 Hz = 1.6s dwell window (was 3.2s).
const DWELL_BUF_LEN: usize = 32;

/// Rolling window for motion energy trend (transit direction).
/// ESP32: 16 frames gives clear entering/exiting gradient.
const MOTION_TREND_LEN: usize = 16;

/// EMA smoothing for happiness output.
const HAPPINESS_ALPHA: f32 = 0.10;

/// EMA smoothing for gait speed.
const GAIT_ALPHA: f32 = 0.12;

/// EMA smoothing for fluidity.
const FLUIDITY_ALPHA: f32 = 0.12;

/// EMA smoothing for social energy.
const SOCIAL_ALPHA: f32 = 0.10;

/// Minimum frames before emitting events.
const MIN_WARMUP: u32 = 20;

/// Maximum subcarriers from host API.
/// ESP32 CSI provides up to 52 subcarriers; host caps at 32.
const MAX_SC: usize = 32;

/// Event emission decimation: emit full event set every Nth frame.
/// At 20 Hz, N=4 means events at 5 Hz — reduces UDP packet rate by 75%.
const EVENT_DECIMATION: u32 = 4;

/// Baseline gait speed (phase rate-of-change, arbitrary units).
/// Happy gait is ~12% above this.
const BASELINE_GAIT_SPEED: f32 = 0.5;

/// Maximum expected gait speed for normalization.
const MAX_GAIT_SPEED: f32 = 2.0;

/// Calm breathing range: 6-14 BPM (slow = calm = happier).
const CALM_BREATH_LOW: f32 = 6.0;
const CALM_BREATH_HIGH: f32 = 14.0;

/// Stressed breathing threshold.
const STRESS_BREATH_THRESH: f32 = 22.0;

// ── Weights for composite happiness score ────────────────────────────────────

const W_GAIT_SPEED: f32 = 0.25;
const W_STRIDE_REG: f32 = 0.15;
const W_FLUIDITY: f32 = 0.20;
const W_BREATH_CALM: f32 = 0.20;
const W_POSTURE: f32 = 0.10;
const W_DWELL: f32 = 0.10;

// ── Event IDs (690-694: Exotic) ──────────────────────────────────────────────

pub const EVENT_HAPPINESS_SCORE: i32 = 690;
pub const EVENT_GAIT_ENERGY: i32 = 691;
pub const EVENT_AFFECT_VALENCE: i32 = 692;
pub const EVENT_SOCIAL_ENERGY: i32 = 693;
pub const EVENT_TRANSIT_DIRECTION: i32 = 694;

/// Dimension of the happiness vector for Cognitum Seed ingestion.
pub const HAPPINESS_VECTOR_DIM: usize = 8;

// ── Happiness Score Detector ─────────────────────────────────────────────────

/// Computes a composite happiness score from WiFi CSI physiological proxies.
///
/// Outputs a scalar happiness score [0, 1] and an 8-dim happiness vector
/// suitable for ingestion into a Cognitum Seed vector store.
pub struct HappinessScoreDetector {
    /// Phase rate-of-change history (gait speed proxy).
    phase_roc: CircularBuffer<PHASE_ROC_LEN>,
    /// Step interval variance tracking.
    step_stats: WelfordStats,
    /// Movement fluidity buffer (phase second derivative).
    fluidity_buf: CircularBuffer<FLUIDITY_BUF_LEN>,
    /// Breathing rate history.
    breath_hist: CircularBuffer<BREATH_HIST_LEN>,
    /// Amplitude spread history (posture proxy).
    amp_spread_hist: CircularBuffer<AMP_SPREAD_LEN>,
    /// Dwell buffer: 1.0 if presence, 0.0 if not.
    dwell_buf: CircularBuffer<DWELL_BUF_LEN>,
    /// Motion energy trend buffer (for transit direction).
    motion_trend: CircularBuffer<MOTION_TREND_LEN>,

    /// EMA-smoothed happiness score.
    happiness_ema: Ema,
    /// EMA-smoothed gait energy.
    gait_ema: Ema,
    /// EMA-smoothed fluidity.
    fluidity_ema: Ema,
    /// EMA-smoothed social energy.
    social_ema: Ema,

    /// Previous frame mean phase (for rate-of-change).
    prev_mean_phase: f32,
    /// Previous phase rate-of-change (for second derivative).
    prev_phase_roc: f32,

    /// Current happiness score [0, 1].
    happiness: f32,

    /// 8-dim happiness vector for Cognitum Seed ingestion.
    ///
    /// Layout:
    ///   [0] = happiness_score
    ///   [1] = gait_speed_norm
    ///   [2] = stride_regularity
    ///   [3] = movement_fluidity
    ///   [4] = breathing_calm
    ///   [5] = posture_score
    ///   [6] = dwell_factor
    ///   [7] = social_energy
    pub happiness_vector: [f32; HAPPINESS_VECTOR_DIM],

    /// Total frames processed.
    frame_count: u32,
}

impl HappinessScoreDetector {
    pub const fn new() -> Self {
        Self {
            phase_roc: CircularBuffer::new(),
            step_stats: WelfordStats::new(),
            fluidity_buf: CircularBuffer::new(),
            breath_hist: CircularBuffer::new(),
            amp_spread_hist: CircularBuffer::new(),
            dwell_buf: CircularBuffer::new(),
            motion_trend: CircularBuffer::new(),

            happiness_ema: Ema::new(HAPPINESS_ALPHA),
            gait_ema: Ema::new(GAIT_ALPHA),
            fluidity_ema: Ema::new(FLUIDITY_ALPHA),
            social_ema: Ema::new(SOCIAL_ALPHA),

            prev_mean_phase: 0.0,
            prev_phase_roc: 0.0,

            happiness: 0.5,
            happiness_vector: [0.0; HAPPINESS_VECTOR_DIM],

            frame_count: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// # Arguments
    /// - `phases` -- subcarrier phase values.
    /// - `amplitudes` -- subcarrier amplitude values.
    /// - `variance` -- subcarrier phase variance values.
    /// - `presence` -- 1 if person present, 0 if not.
    /// - `motion_energy` -- host-reported motion energy.
    /// - `breathing_bpm` -- breathing rate from Tier 2 DSP.
    /// - `heart_rate_bpm` -- heart rate from Tier 2 DSP.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        presence: i32,
        motion_energy: f32,
        breathing_bpm: f32,
        heart_rate_bpm: f32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 5] = [(0, 0.0); 5];
        let mut n_ev = 0usize;

        self.frame_count += 1;

        let present = presence > 0;

        // ── Update dwell buffer ──
        self.dwell_buf.push(if present { 1.0 } else { 0.0 });

        // ── Update motion trend ──
        self.motion_trend.push(motion_energy);

        // If nobody is present, emit nothing.
        if !present {
            return &[];
        }

        // ── 1. Gait speed: phase rate-of-change ──
        let mean_phase = mean_slice(phases);
        let phase_roc = fabsf(mean_phase - self.prev_mean_phase);
        self.phase_roc.push(phase_roc);
        self.prev_mean_phase = mean_phase;

        // ── 2. Stride regularity: step interval variance from successive diffs ──
        // Use variance across subcarriers as a step-impact proxy.
        let var_mean = mean_slice(variance);
        self.step_stats.update(var_mean);

        // ── 3. Movement fluidity: second derivative of phase ──
        let phase_accel = fabsf(phase_roc - self.prev_phase_roc);
        self.fluidity_buf.push(phase_accel);
        self.prev_phase_roc = phase_roc;

        // ── 4. Breathing calm ──
        self.breath_hist.push(breathing_bpm);

        // ── 5. Posture: amplitude spread across subcarrier groups ──
        let amp_spread = compute_amplitude_spread(amplitudes);
        self.amp_spread_hist.push(amp_spread);

        // ── Warmup period ──
        if self.frame_count < MIN_WARMUP {
            return &[];
        }

        // ── Feature extraction ──

        // Feature 1: Gait speed score [0, 1].
        let gait_speed = self.compute_gait_speed();
        let gait_speed_norm = clamp01(gait_speed / MAX_GAIT_SPEED);
        let gait_score = clamp01(self.gait_ema.update(gait_speed_norm));

        // Feature 2: Stride regularity [0, 1] (low CV = regular = higher score).
        let stride_regularity = self.compute_stride_regularity();

        // Feature 3: Movement fluidity [0, 1] (low jerk = fluid = higher score).
        let fluidity_raw = self.compute_fluidity();
        let fluidity = clamp01(self.fluidity_ema.update(fluidity_raw));

        // Feature 4: Breathing calm [0, 1] (slow breathing = calm = higher score).
        let breath_calm = self.compute_breath_calm(breathing_bpm);

        // Feature 5: Posture score [0, 1] (wide spread = upright = higher score).
        let posture_score = self.compute_posture_score();

        // Feature 6: Dwell factor [0, 1] (fraction of recent frames with presence).
        let dwell_factor = self.compute_dwell_factor();

        // ── Composite happiness score ──
        let raw_happiness = W_GAIT_SPEED * gait_score
            + W_STRIDE_REG * stride_regularity
            + W_FLUIDITY * fluidity
            + W_BREATH_CALM * breath_calm
            + W_POSTURE * posture_score
            + W_DWELL * dwell_factor;

        self.happiness = clamp01(self.happiness_ema.update(raw_happiness));

        // ── Derived outputs ──

        // Gait energy: combination of gait speed + stride regularity.
        let gait_energy = clamp01(0.6 * gait_score + 0.4 * stride_regularity);

        // Affect valence: breathing calm + fluidity (emotional valence).
        let affect_valence = clamp01(0.5 * breath_calm + 0.3 * fluidity + 0.2 * posture_score);

        // Social energy: motion energy + dwell + heart rate proxy.
        let hr_factor = clamp01((heart_rate_bpm - 60.0) / 60.0);
        let raw_social = 0.4 * clamp01(motion_energy) + 0.3 * dwell_factor + 0.3 * hr_factor;
        let social_energy = clamp01(self.social_ema.update(raw_social));

        // Transit direction: motion energy trend (increasing = entering, decreasing = exiting).
        let transit = self.compute_transit_direction();

        // ── Update happiness vector ──
        self.happiness_vector[0] = self.happiness;
        self.happiness_vector[1] = gait_score;
        self.happiness_vector[2] = stride_regularity;
        self.happiness_vector[3] = fluidity;
        self.happiness_vector[4] = breath_calm;
        self.happiness_vector[5] = posture_score;
        self.happiness_vector[6] = dwell_factor;
        self.happiness_vector[7] = social_energy;

        // ── Emit events (decimated for ESP32 bandwidth) ──
        // Always emit happiness score; other events only every Nth frame.
        unsafe {
            EVENTS[n_ev] = (EVENT_HAPPINESS_SCORE, self.happiness);
        }
        n_ev += 1;

        if self.frame_count % EVENT_DECIMATION == 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_GAIT_ENERGY, gait_energy);
            }
            n_ev += 1;

            unsafe {
                EVENTS[n_ev] = (EVENT_AFFECT_VALENCE, affect_valence);
            }
            n_ev += 1;

            unsafe {
                EVENTS[n_ev] = (EVENT_SOCIAL_ENERGY, social_energy);
            }
            n_ev += 1;

            unsafe {
                EVENTS[n_ev] = (EVENT_TRANSIT_DIRECTION, transit);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Average phase rate-of-change over the rolling window.
    fn compute_gait_speed(&self) -> f32 {
        let n = self.phase_roc.len();
        if n == 0 {
            return 0.0;
        }
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += self.phase_roc.get(i);
        }
        sum / n as f32
    }

    /// Stride regularity: inverse of step interval CV, mapped to [0, 1].
    /// Low CV (regular) -> high score.
    fn compute_stride_regularity(&self) -> f32 {
        if self.step_stats.count() < 4 {
            return 0.5;
        }
        let mean = self.step_stats.mean();
        if mean < 1e-6 {
            return 0.5;
        }
        let cv = self.step_stats.std_dev() / mean;
        // CV of 0 -> score 1.0, CV of 1.0 -> score 0.0.
        clamp01(1.0 - cv)
    }

    /// Movement fluidity: inverse of mean phase acceleration, mapped to [0, 1].
    /// Low jerk -> high fluidity.
    fn compute_fluidity(&self) -> f32 {
        let n = self.fluidity_buf.len();
        if n == 0 {
            return 0.5;
        }
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += self.fluidity_buf.get(i);
        }
        let mean_accel = sum / n as f32;
        // Mean acceleration of 0 -> fluidity 1.0, > 1.0 -> fluidity 0.0.
        clamp01(1.0 - mean_accel)
    }

    /// Breathing calm score [0, 1].
    /// Slow breathing (6-14 BPM) -> high calm, fast breathing (>22) -> low calm.
    fn compute_breath_calm(&self, bpm: f32) -> f32 {
        if bpm >= CALM_BREATH_LOW && bpm <= CALM_BREATH_HIGH {
            return 1.0;
        }
        if bpm < CALM_BREATH_LOW {
            // Very slow -- still fairly calm.
            return 0.7;
        }
        // Linear ramp from calm to stressed.
        let score = 1.0 - (bpm - CALM_BREATH_HIGH) / (STRESS_BREATH_THRESH - CALM_BREATH_HIGH);
        clamp01(score)
    }

    /// Posture score [0, 1] from amplitude spread across subcarriers.
    /// Wide spread = upright posture.
    fn compute_posture_score(&self) -> f32 {
        let n = self.amp_spread_hist.len();
        if n == 0 {
            return 0.5;
        }
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += self.amp_spread_hist.get(i);
        }
        let mean_spread = sum / n as f32;
        // Normalize: typical spread range is [0, 1].
        clamp01(mean_spread)
    }

    /// Dwell factor [0, 1]: fraction of recent frames with presence.
    fn compute_dwell_factor(&self) -> f32 {
        let n = self.dwell_buf.len();
        if n == 0 {
            return 0.0;
        }
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += self.dwell_buf.get(i);
        }
        sum / n as f32
    }

    /// Transit direction from motion energy trend.
    /// Returns 1.0 for entering (increasing trend), 0.0 for exiting (decreasing).
    fn compute_transit_direction(&self) -> f32 {
        let n = self.motion_trend.len();
        if n < 4 {
            return 0.5;
        }
        // Compare recent half to older half.
        let half = n / 2;
        let mut old_sum = 0.0f32;
        let mut new_sum = 0.0f32;
        for i in 0..half {
            old_sum += self.motion_trend.get(i);
        }
        for i in half..n {
            new_sum += self.motion_trend.get(i);
        }
        let old_avg = old_sum / half as f32;
        let new_avg = new_sum / (n - half) as f32;
        // Increasing -> entering (1.0), decreasing -> exiting (0.0).
        if new_avg > old_avg + 0.01 {
            1.0
        } else if new_avg < old_avg - 0.01 {
            0.0
        } else {
            0.5
        }
    }

    /// Get current happiness score [0, 1].
    pub fn happiness(&self) -> f32 {
        self.happiness
    }

    /// Get the 8-dim happiness vector.
    pub fn happiness_vector(&self) -> &[f32; HAPPINESS_VECTOR_DIM] {
        &self.happiness_vector
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

/// Compute mean of a slice.  Returns 0.0 if empty.
/// ESP32-optimized: caps at MAX_SC to avoid processing more subcarriers
/// than the host provides, and uses `#[inline]` for WASM3 interpreter.
#[inline]
fn mean_slice(s: &[f32]) -> f32 {
    let n = s.len();
    if n == 0 {
        return 0.0;
    }
    let n_use = if n > MAX_SC { MAX_SC } else { n };
    let mut sum = 0.0f32;
    for i in 0..n_use {
        sum += s[i];
    }
    sum / n_use as f32
}

/// Compute amplitude spread: normalized variance across subcarriers.
/// Higher spread means signal is distributed across more subcarriers (upright posture).
/// ESP32-optimized: uses variance/mean^2 (CV^2) to avoid sqrtf.
#[inline]
fn compute_amplitude_spread(amplitudes: &[f32]) -> f32 {
    let n = amplitudes.len();
    if n < 2 {
        return 0.0;
    }
    let n_use = if n > MAX_SC { MAX_SC } else { n };

    // Single-pass mean + variance (Welford online, unrolled for speed).
    let mut sum = 0.0f32;
    for i in 0..n_use {
        sum += amplitudes[i];
    }
    let mean = sum / n_use as f32;
    if mean < 1e-6 {
        return 0.0;
    }

    let mut var_sum = 0.0f32;
    for i in 0..n_use {
        let d = amplitudes[i] - mean;
        var_sum += d * d;
    }
    // CV^2 = variance / mean^2 — avoids sqrtf on ESP32.
    // Typical CV range [0, 2] -> CV^2 range [0, 4].
    // Map CV^2 to [0, 1] with saturating scale at 1.0.
    let cv_sq = var_sum / (n_use as f32 * mean * mean);
    clamp01(cv_sq)
}

/// Clamp a value to [0, 1].
#[inline(always)]
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

    /// Helper: feed N frames with presence and reasonable CSI data.
    fn feed_frames(
        det: &mut HappinessScoreDetector,
        n: u32,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        presence: i32,
        motion_energy: f32,
        breathing_bpm: f32,
        heart_rate_bpm: f32,
    ) {
        for _ in 0..n {
            det.process_frame(
                phases,
                amplitudes,
                variance,
                presence,
                motion_energy,
                breathing_bpm,
                heart_rate_bpm,
            );
        }
    }

    #[test]
    fn test_const_new() {
        let det = HappinessScoreDetector::new();
        assert_eq!(det.frame_count(), 0);
        assert!(fabsf(det.happiness() - 0.5) < 1e-6);
        assert_eq!(det.happiness_vector().len(), HAPPINESS_VECTOR_DIM);
    }

    #[test]
    fn test_no_presence_no_score() {
        let mut det = HappinessScoreDetector::new();
        let phases = [0.1, 0.2, 0.3, 0.4];
        let amps = [1.0, 1.0, 1.0, 1.0];
        let var = [0.1, 0.1, 0.1, 0.1];

        // Feed 100 frames with no presence.
        for _ in 0..100 {
            let events = det.process_frame(&phases, &amps, &var, 0, 0.5, 14.0, 70.0);
            assert!(events.is_empty(), "should not emit events without presence");
        }
    }

    #[test]
    fn test_happy_gait() {
        let mut det = HappinessScoreDetector::new();

        // Simulate happy gait: fast phase changes (high gait speed), regular variance,
        // smooth trajectory, calm breathing, good posture.
        let amps = [1.0, 0.8, 1.2, 0.9, 1.1, 0.7, 1.3, 0.85];
        let var = [0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3];

        for i in 0..200u32 {
            // Steadily increasing phase = fast gait (0.8 rad/frame is brisk walking).
            let phase_val = (i as f32) * 0.8;
            let phases = [phase_val; 8];
            det.process_frame(&phases, &amps, &var, 1, 0.6, 10.0, 72.0);
        }

        // Gait energy should be moderate-to-high due to consistent phase changes.
        let vec = det.happiness_vector();
        let gait_score = vec[1];
        assert!(
            gait_score > 0.2,
            "fast regular gait should yield moderate+ gait score, got {}",
            gait_score
        );
    }

    #[test]
    fn test_calm_breathing() {
        let mut det = HappinessScoreDetector::new();

        let phases = [0.1, 0.2, 0.15, 0.18];
        let amps = [1.0, 1.0, 1.0, 1.0];
        let var = [0.2, 0.2, 0.2, 0.2];

        // Feed with calm breathing (10 BPM, in calm range).
        feed_frames(&mut det, 200, &phases, &amps, &var, 1, 0.3, 10.0, 68.0);

        let vec = det.happiness_vector();
        let breath_calm = vec[4];
        assert!(
            breath_calm > 0.7,
            "slow calm breathing should yield high calm score, got {}",
            breath_calm
        );
    }

    #[test]
    fn test_score_bounds() {
        let mut det = HappinessScoreDetector::new();

        // Feed extreme values.
        let phases = [10.0, -10.0, 5.0, -5.0];
        let amps = [100.0, 0.0, 50.0, 200.0];
        let var = [5.0, 5.0, 5.0, 5.0];

        feed_frames(&mut det, 100, &phases, &amps, &var, 1, 5.0, 40.0, 150.0);

        assert!(
            det.happiness() >= 0.0 && det.happiness() <= 1.0,
            "happiness must be in [0,1], got {}",
            det.happiness()
        );

        let vec = det.happiness_vector();
        for (i, &v) in vec.iter().enumerate() {
            assert!(
                v >= 0.0 && v <= 1.0,
                "happiness_vector[{}] must be in [0,1], got {}",
                i,
                v
            );
        }
    }

    #[test]
    fn test_happiness_vector_dim() {
        let det = HappinessScoreDetector::new();
        assert_eq!(
            det.happiness_vector().len(),
            8,
            "happiness vector must be exactly 8 dimensions"
        );
        assert_eq!(HAPPINESS_VECTOR_DIM, 8);
    }

    #[test]
    fn test_event_ids_emitted() {
        let mut det = HappinessScoreDetector::new();
        let phases = [0.1, 0.2, 0.3, 0.4];
        let amps = [1.0, 1.0, 1.0, 1.0];
        let var = [0.1, 0.1, 0.1, 0.1];

        // Past warmup — feed enough frames so next one lands on decimation boundary.
        // EVENT_DECIMATION=4, MIN_WARMUP=20, so frame 24 is first full-emit after warmup.
        // We need frame_count % EVENT_DECIMATION == 0 for full event set.
        let warmup_frames = MIN_WARMUP + (EVENT_DECIMATION - (MIN_WARMUP % EVENT_DECIMATION)) % EVENT_DECIMATION;
        for _ in 0..warmup_frames {
            det.process_frame(&phases, &amps, &var, 1, 0.3, 14.0, 70.0);
        }
        // Next frame should land on decimation boundary and emit all 5 events.
        // Feed (EVENT_DECIMATION - 1) more frames that emit only happiness score.
        for _ in 0..EVENT_DECIMATION - 1 {
            det.process_frame(&phases, &amps, &var, 1, 0.3, 14.0, 70.0);
        }
        let events = det.process_frame(&phases, &amps, &var, 1, 0.3, 14.0, 70.0);
        // On non-decimation frames: 1 event (happiness only).
        // On decimation frames: 5 events (all).
        // Check that we get either 1 or 5; full event set when on boundary.
        assert!(events.len() == 1 || events.len() == 5,
            "should emit 1 or 5 events, got {}", events.len());
        assert_eq!(events[0].0, EVENT_HAPPINESS_SCORE);
        // Verify all 5 on a decimation frame.
        if events.len() == 5 {
            assert_eq!(events[1].0, EVENT_GAIT_ENERGY);
            assert_eq!(events[2].0, EVENT_AFFECT_VALENCE);
            assert_eq!(events[3].0, EVENT_SOCIAL_ENERGY);
            assert_eq!(events[4].0, EVENT_TRANSIT_DIRECTION);
        }
    }

    #[test]
    fn test_clamp01() {
        assert!(fabsf(clamp01(-1.0)) < 1e-6);
        assert!(fabsf(clamp01(0.5) - 0.5) < 1e-6);
        assert!(fabsf(clamp01(2.0) - 1.0) < 1e-6);
    }

    #[test]
    fn test_transit_direction() {
        let mut det = HappinessScoreDetector::new();
        let phases = [0.1, 0.2, 0.3, 0.4];
        let amps = [1.0, 1.0, 1.0, 1.0];
        let var = [0.1, 0.1, 0.1, 0.1];

        // Feed increasing motion energy -> entering.
        // Use enough frames so we land on a decimation boundary with transit event.
        for i in 0..64u32 {
            let energy = (i as f32) * 0.02;
            det.process_frame(&phases, &amps, &var, 1, energy, 14.0, 70.0);
        }
        // Collect events across EVENT_DECIMATION frames to catch the transit event.
        let mut found_transit = false;
        let mut transit_val = 0.0f32;
        for _ in 0..EVENT_DECIMATION {
            let events = det.process_frame(&phases, &amps, &var, 1, 1.5, 14.0, 70.0);
            if let Some(ev) = events.iter().find(|e| e.0 == EVENT_TRANSIT_DIRECTION) {
                found_transit = true;
                transit_val = ev.1;
            }
        }
        assert!(found_transit, "should emit transit direction within decimation window");
        assert!(
            transit_val >= 0.5,
            "increasing motion should indicate entering, got {}",
            transit_val
        );
    }

    #[test]
    fn test_reset() {
        let mut det = HappinessScoreDetector::new();
        let phases = [0.1, 0.2, 0.3, 0.4];
        let amps = [1.0, 1.0, 1.0, 1.0];
        let var = [0.1, 0.1, 0.1, 0.1];

        feed_frames(&mut det, 100, &phases, &amps, &var, 1, 0.3, 14.0, 70.0);
        assert!(det.frame_count() > 0);
        det.reset();
        assert_eq!(det.frame_count(), 0);
        assert!(fabsf(det.happiness() - 0.5) < 1e-6);
    }

    #[test]
    fn test_amplitude_spread() {
        // Uniform amplitudes -> low spread.
        let uniform = [1.0, 1.0, 1.0, 1.0];
        let s1 = compute_amplitude_spread(&uniform);
        assert!(s1 < 0.01, "uniform amps should have near-zero spread, got {}", s1);

        // Varied amplitudes -> higher spread.
        let varied = [0.1, 2.0, 0.5, 3.0, 0.2, 1.5];
        let s2 = compute_amplitude_spread(&varied);
        assert!(s2 > 0.3, "varied amps should have significant spread, got {}", s2);
    }
}
