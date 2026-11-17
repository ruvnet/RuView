//! Server-side fall detection from raw CSI (magic 0xC5110001) — the baseline
//! z-score spike design with a SPLIT stillness threshold.
//!
//! Mirrors the ESP32 edge pipeline's state machine (debounce / cooldown /
//! latch, issue #263) but on the amplitude-shape observable, with two lessons
//! from hardware captures baked in:
//!
//! 1. AGC immunity (measured): the ESP32's RX AGC rescales ALL subcarriers by
//!    a common gain; per-frame shape normalization (`s_i = amp[i]/mean(amp)`)
//!    makes gain clicks structurally invisible.
//! 2. Split thresholds (measured): the baseline may only learn TRUE stillness
//!    (gate 4.5% — opening it to 8% let motion noise inflate σ and blinded
//!    the spike score), while the stillness CONFIRMATION must accept the
//!    lying-pose floor (5.5-7% vs the seated 3.5-4%) — so the confirmation
//!    threshold is 8%.
//!
//! Pipeline:
//!   s_i = amp[i]/mean(amp)                    (AGC-immune shape)
//!   still_score = mean_i |Δs_i|               (frame-to-frame change)
//!   spike_score  = RMS_i((s_i − μ_i)/σ_i)     (deviation from the slow,
//!                                              gated, primed baseline)
//!   Stage 1: spike_score ≥ 3.5 for 3 consecutive frames → pending
//!   Stage 2: still_avg (0.5 s mean of still_score) < 8% for 2 s within the
//!            pending window → latched FALL (10 s hold, 5 s cooldown,
//!            re-arm requires the spike score to return to ambient)
//!
//! Known boundary (single-link CSI, rule-based): a fast sit-down and a
//! walk-then-stop share the "spike + stillness" signature — the false
//! direction is over-reporting, the safe one for caregiving.

use serde::Serialize;
use std::collections::VecDeque;

/// Consecutive spike frames required (firmware `EDGE_FALL_CONSEC_MIN` was 3;
/// raised to 5 from hardware captures: ordinary desk activity produces
/// isolated single-frame z-spikes up to ≈7 that never sustain, while a real
/// fall's shape sweep holds z ≥ threshold for 5-9 consecutive frames —
/// measured: fall bursts at 4.3-8.5 sustained; ~20 activity spikes in a 60 s
/// window never exceeded consec 2).
pub const SPIKE_CONSEC_MIN: u32 = 5;
/// Minimum ms between latched alerts — mirrors `EDGE_FALL_COOLDOWN_MS`.
pub const FALL_COOLDOWN_MS: i64 = 5_000;
/// Default spike threshold (RMS z over subcarriers). Hardware captures:
/// desk-activity pops ≈ 3.9-4.4, falls 4.3-5.8.
pub const DEFAULT_SPIKE_THRESH: f64 = 3.5;
/// Default baseline-learning gate: the baseline μ/σ only updates on frames
/// whose still_score is below this — TRUE stillness only. Opening the gate
/// wider let motion inflate σ and blinded the spike score (measured).
pub const DEFAULT_BASELINE_GATE_THRESH: f64 = 0.045;
/// Default stillness CONFIRMATION threshold, applied to the 4 s average of
/// the frame-to-frame shape change. Calibrated from a real fall capture: the
/// raw (0.5 s) Δs while lying oscillates 5.5-10.8% with a 2-4 s period —
/// that is BREATHING (the fallen pose couples the chest into the link more
/// strongly), not movement. Averaging over 4 s (> one breathing period)
/// flattens it to ≈7.5% while lying and ≈10% while walking, so 9% separates
/// the two. Replay: the real fall latches (the 4-s average stays under the
/// threshold for the full 12-s confirmation); the desk-sitting false
/// positive never arms (its z-spikes sustain ≤3 frames < 5).
pub const DEFAULT_STILL_THRESH: f64 = 0.09;
/// Default: stillness must hold this long to confirm a fall. Calibrated from
/// hardware captures: the person's post-transient still pauses last 2-5 s
/// (typing/shifting resume), while a real fall ends with minutes of no
/// get-up — the stillness DURATION is the discriminator the spike magnitude
/// cannot provide (sitting z-spikes reach 10.5, above real falls' 4.3-5.8).
pub const DEFAULT_STILL_WINDOW_MS: i64 = 12_000;
/// Default: stillness must be seen within this long after the spike, or the
/// event is reclassified as ordinary movement (walking). Must cover the
/// stillness window (12 s) plus the settle time between the fall and going
/// still (measured ≈1-3 s; up to ~12 s tolerated).
pub const DEFAULT_PENDING_WINDOW_MS: i64 = 24_000;
/// Default: how long a confirmed fall stays latched.
pub const DEFAULT_LATCH_HOLD_MS: i64 = 10_000;
/// Baseline EMA time constant (seconds of still-frame time).
const BASELINE_TAU_S: f64 = 10.0;
/// Gated baseline frames before the score is trusted.
const BASELINE_WARMUP_FRAMES: u32 = 60;
/// Floor for the per-subcarrier shape sigma (guards division blowup).
const SHAPE_SIGMA_FLOOR: f64 = 0.02;
/// Frames with an ambient score needed to re-arm after a latch.
const REARM_QUIET_FRAMES: u32 = 15;
/// Stillness averaging window in SECONDS. Must exceed one breathing period
/// (2-5 s) so the breathing-driven shape modulation averages out of the
/// stillness criterion; 4 s is the shortest that flattens the measured
/// lying-pose oscillation.
const STILL_AVG_WINDOW_S: f64 = 4.0;
/// Minimum window in frames (short-rate fallback).
const STILL_AVG_WINDOW_MIN: usize = 15;
/// Fallback CSI rate (Hz) before the node's measured EMA has warmed up.
const FALLBACK_FPS: f64 = 20.0;
/// Server-side capture capacity (frames). At ~30 Hz this is ≈2 minutes —
/// plenty for a fall test (spike + 12-s stillness + margin). When full, the
/// capture auto-stops (analysis wants a bounded, start-aligned record, not a
/// rolling window).
pub const CAPTURE_MAX_FRAMES: usize = 3600;

/// One full-rate capture record (one CSI frame, ~30 Hz).
#[derive(Debug, Clone, Serialize)]
pub struct CaptureFrame {
    pub t_unix_ms: i64,
    /// Spike score (RMS z) for this frame.
    pub spike: f64,
    /// Instant stillness score (mean per-subcarrier |Δshape|).
    pub still: f64,
    /// Stillness moving average (the confirmation criterion's input).
    pub still_avg: f64,
    pub pending: bool,
    pub detected: bool,
}
/// Score-history samples kept for the Debug tab chart.
const SCORE_HISTORY_CAP: usize = 300;

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(default)
}

/// Point-in-time snapshot of the fall detector state.
#[derive(Debug, Clone, Serialize)]
pub struct FallStatus {
    /// Latched fall alert.
    pub detected: bool,
    /// Stage-1 spike score (RMS z of the shape vs the gated baseline).
    pub spike_score: f64,
    /// Stage-1 threshold in force.
    pub spike_threshold: f64,
    /// Consecutive frames currently above the spike threshold.
    pub spike_consec: u32,
    /// Stage-2 instant stillness score (mean per-subcarrier |Δshape|).
    pub still_score: f64,
    /// Stage-2 criterion: moving average (≈0.5 s) of `still_score`.
    pub still_avg: f64,
    /// Stage-2 confirmation threshold in force.
    pub still_threshold: f64,
    /// A spike is armed and awaiting stillness confirmation.
    pub pending: bool,
    /// Unix ms of the last latched alert, if any.
    pub last_alert_unix_ms: Option<i64>,
    /// Frames processed since the last (re)start — warmup indicator.
    pub frames: u64,
}

/// Per-node fall detector. One instance per ESP32 node; state resets
/// automatically when the CSI grid (subcarrier count) changes.
#[derive(Debug)]
pub struct FallDetector {
    spike_thresh: f64,
    still_thresh: f64,
    baseline_gate_thresh: f64,
    still_window_ms: i64,
    pending_window_ms: i64,
    latch_hold_ms: i64,
    cooldown_ms: i64,

    baseline_mean: Vec<f64>,
    baseline_var: Vec<f64>,
    baseline_ready: bool,
    baseline_samples: u32,

    prev_shape: Vec<f64>,
    has_prev_shape: bool,

    frame_count: u64,
    spike_consec: u32,
    still_avg_window: VecDeque<f64>,
    still_consec: u32,
    pending: bool,
    pending_since_ms: Option<i64>,
    fall_detected: bool,
    latch_until_ms: Option<i64>,
    last_alert_unix_ms: Option<i64>,
    rearm_quiet_consec: u32,
    require_rearm: bool,
    last_spike_score: f64,
    last_still_score: f64,
    last_still_avg: f64,
    /// Recent spike scores for the Debug tab trend chart.
    score_history: VecDeque<f64>,
    /// Full-rate capture (user-armed via the API) for waveform export.
    capturing: bool,
    capture: Vec<CaptureFrame>,
    capture_full: bool,
}

impl FallDetector {
    /// Create a detector; thresholds read their `RUVIEW_FALL_*` env overrides
    /// (see the `DEFAULT_*` consts for names and defaults).
    pub fn new() -> Self {
        Self {
            spike_thresh: env_f64("RUVIEW_FALL_SPIKE_THRESH", DEFAULT_SPIKE_THRESH),
            still_thresh: env_f64("RUVIEW_FALL_STILL_THRESH", DEFAULT_STILL_THRESH),
            baseline_gate_thresh: env_f64(
                "RUVIEW_FALL_BASELINE_GATE_THRESH",
                DEFAULT_BASELINE_GATE_THRESH,
            ),
            still_window_ms: env_f64(
                "RUVIEW_FALL_STILL_WINDOW_MS",
                DEFAULT_STILL_WINDOW_MS as f64,
            ) as i64,
            pending_window_ms: env_f64(
                "RUVIEW_FALL_PENDING_WINDOW_MS",
                DEFAULT_PENDING_WINDOW_MS as f64,
            ) as i64,
            latch_hold_ms: env_f64("RUVIEW_FALL_LATCH_HOLD_MS", DEFAULT_LATCH_HOLD_MS as f64)
                as i64,
            cooldown_ms: FALL_COOLDOWN_MS,
            baseline_mean: Vec::new(),
            baseline_var: Vec::new(),
            baseline_ready: false,
            baseline_samples: 0,
            prev_shape: Vec::new(),
            has_prev_shape: false,
            frame_count: 0,
            spike_consec: 0,
            still_avg_window: VecDeque::with_capacity(STILL_AVG_WINDOW_MIN + 1),
            still_consec: 0,
            pending: false,
            pending_since_ms: None,
            fall_detected: false,
            latch_until_ms: None,
            last_alert_unix_ms: None,
            rearm_quiet_consec: 0,
            require_rearm: false,
            last_spike_score: 0.0,
            last_still_score: 0.0,
            last_still_avg: 0.0,
            score_history: VecDeque::with_capacity(SCORE_HISTORY_CAP),
            capturing: false,
            capture: Vec::new(),
            capture_full: false,
        }
    }

    /// Wipe all per-grid state (subcarrier count changed).
    pub fn reset(&mut self) {
        self.baseline_mean.clear();
        self.baseline_var.clear();
        self.baseline_ready = false;
        self.baseline_samples = 0;
        self.prev_shape.clear();
        self.has_prev_shape = false;
        self.frame_count = 0;
        self.spike_consec = 0;
        self.still_consec = 0;
        self.pending = false;
        self.pending_since_ms = None;
        self.fall_detected = false;
        self.latch_until_ms = None;
        self.rearm_quiet_consec = 0;
        self.require_rearm = false;
        self.last_spike_score = 0.0;
        self.last_still_score = 0.0;
        self.last_still_avg = 0.0;
        self.score_history.clear();
        self.capturing = false;
        self.capture.clear();
        self.capture_full = false;
    }

    /// Feed one frame's per-subcarrier amplitudes. `sample_rate_hz` is the
    /// node's measured CSI rate (scales the baseline/stillness time
    /// constants); `now_unix_ms` stamps pending windows, latches, cooldowns.
    pub fn process(
        &mut self,
        amplitudes: &[f64],
        sample_rate_hz: f64,
        now_unix_ms: i64,
    ) -> FallStatus {
        // Grid change (subcarrier count, e.g. HT20 ↔ HE20): start over rather
        // than compare shapes across incomparable tone layouts.
        if self.prev_shape.len() != amplitudes.len() {
            self.reset();
            self.prev_shape = vec![0.0; amplitudes.len()];
        }
        if amplitudes.is_empty() {
            return self.status();
        }
        let fps = if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
            sample_rate_hz
        } else {
            FALLBACK_FPS
        };
        self.frame_count += 1;

        // Step 1: per-frame shape normalization — the AGC-immune view.
        let mean_amp = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
        if !mean_amp.is_finite() || mean_amp <= f64::EPSILON {
            return self.status();
        }
        let shape: Vec<f64> = amplitudes.iter().map(|a| a / mean_amp).collect();

        // Step 2: instant stillness score — frame-to-frame shape change.
        let still_score = if self.has_prev_shape && self.prev_shape.len() == shape.len() {
            shape
                .iter()
                .zip(self.prev_shape.iter())
                .map(|(a, b)| (a - b).abs())
                .sum::<f64>()
                / shape.len() as f64
        } else {
            0.0
        };
        self.prev_shape = shape.clone();
        self.has_prev_shape = true;

        // Step 3: spike score — shape deviation from the slow baseline, in
        // units of each subcarrier's own ambient noise.
        let spike_score = if self.baseline_ready {
            let sum_sq_z: f64 = shape
                .iter()
                .zip(self.baseline_mean.iter())
                .zip(self.baseline_var.iter())
                .map(|((s, mu), var)| {
                    let sigma = var.sqrt().max(SHAPE_SIGMA_FLOOR);
                    let d = (s - mu) / sigma;
                    d * d
                })
                .sum();
            (sum_sq_z / shape.len() as f64).sqrt()
        } else {
            0.0
        };
        self.last_spike_score = spike_score;
        self.last_still_score = still_score;
        self.push_score(spike_score);

        // Gated baseline update: only learn frames where the node is truly
        // still (the TIGHT gate — see DEFAULT_BASELINE_GATE_THRESH), so
        // motion cannot inflate σ and blind the spike score.
        if still_score < self.baseline_gate_thresh {
            if self.baseline_samples == 0 {
                // Prime: the first still frame seeds the baseline directly.
                // EMA-from-zero would spend the whole warmup crawling from 0
                // toward the shape (~1.0) and inflate σ with the initial
                // offset instead of learning the ambient noise.
                self.baseline_mean = shape.clone();
                self.baseline_var = vec![SHAPE_SIGMA_FLOOR * SHAPE_SIGMA_FLOOR; shape.len()];
                self.baseline_samples = 1;
            } else {
                let alpha = (1.0 / (BASELINE_TAU_S * fps)).clamp(0.0, 1.0);
                for i in 0..shape.len() {
                    self.baseline_mean[i] += alpha * (shape[i] - self.baseline_mean[i]);
                    let dev = (shape[i] - self.baseline_mean[i]).powi(2);
                    self.baseline_var[i] += alpha * (dev - self.baseline_var[i]);
                }
                self.baseline_samples += 1;
                if !self.baseline_ready && self.baseline_samples >= BASELINE_WARMUP_FRAMES {
                    self.baseline_ready = true;
                }
            }
        }

        // Latch expiry: a confirmed fall holds for the latch period, then
        // clears (the operator sees the event, not a permanent state).
        if let Some(until) = self.latch_until_ms {
            if now_unix_ms >= until {
                self.fall_detected = false;
                self.latch_until_ms = None;
            }
        }

        // After a latch, suppress re-arming until the spike score returns to
        // ambient — otherwise the (slowly adapting) post-fall baseline keeps
        // the score elevated and the latch flaps every cooldown.
        if self.require_rearm {
            if spike_score < self.spike_thresh {
                self.rearm_quiet_consec += 1;
                if self.rearm_quiet_consec >= REARM_QUIET_FRAMES {
                    self.require_rearm = false;
                    self.rearm_quiet_consec = 0;
                }
            } else {
                self.rearm_quiet_consec = 0;
            }
        }

        // Stage 1: spike debounce (firmware `EDGE_FALL_CONSEC_MIN` parity).
        if spike_score > self.spike_thresh {
            self.spike_consec += 1;
        } else {
            self.spike_consec = 0;
        }
        if !self.pending
            && !self.require_rearm
            && self.baseline_ready
            && self.spike_consec >= SPIKE_CONSEC_MIN
        {
            self.pending = true;
            self.pending_since_ms = Some(now_unix_ms);
            self.still_consec = 0;
        }

        // Stage 2: stillness confirmation while the spike is pending. Runs on
        // the averaged stillness score (a single motion pop must not reset a
        // nearly-complete confirmation), against the LOOSE confirmation
        // threshold (the lying-pose floor).
        let still_win_len = (STILL_AVG_WINDOW_S * fps).round()
            .max(STILL_AVG_WINDOW_MIN as f64) as usize;
        self.still_avg_window.push_back(still_score);
        while self.still_avg_window.len() > still_win_len {
            self.still_avg_window.pop_front();
        }
        let still_avg =
            self.still_avg_window.iter().sum::<f64>() / self.still_avg_window.len() as f64;
        self.last_still_avg = still_avg;

        if self.pending {
            if still_avg < self.still_thresh {
                self.still_consec += 1;
            } else {
                self.still_consec = 0;
            }
            let still_held_ms =
                (self.still_consec as f64 * 1000.0 / fps) as i64 >= self.still_window_ms;
            let since_pending = self
                .pending_since_ms
                .map(|since| now_unix_ms - since)
                .unwrap_or(0);
            if still_held_ms {
                let cooled_down = self
                    .last_alert_unix_ms
                    .is_none_or(|last| now_unix_ms - last >= self.cooldown_ms);
                if cooled_down {
                    self.fall_detected = true;
                    self.last_alert_unix_ms = Some(now_unix_ms);
                    self.latch_until_ms = Some(now_unix_ms + self.latch_hold_ms);
                    self.require_rearm = true;
                    self.rearm_quiet_consec = 0;
                }
                self.pending = false;
                self.pending_since_ms = None;
            } else if since_pending > self.pending_window_ms {
                // Movement continued — ordinary walking, not a fall.
                self.pending = false;
                self.pending_since_ms = None;
            }
        }

        // Full-rate capture: one record per admitted frame.
        if self.capturing {
            if self.capture.len() < CAPTURE_MAX_FRAMES {
                self.capture.push(CaptureFrame {
                    t_unix_ms: now_unix_ms,
                    spike: spike_score,
                    still: still_score,
                    still_avg: self.last_still_avg,
                    pending: self.pending,
                    detected: self.fall_detected,
                });
                if self.capture.len() >= CAPTURE_MAX_FRAMES {
                    self.capture_full = true;
                    self.capturing = false;
                }
            }
        }

        self.status()
    }

    // ── Full-rate capture (user-armed) ────────────────────────────────────

    /// Start a full-rate capture (clears any previous record).
    pub fn capture_start(&mut self) {
        self.capture.clear();
        self.capture_full = false;
        self.capturing = true;
    }

    /// Stop capturing; the record stays available for export.
    pub fn capture_stop(&mut self) {
        self.capturing = false;
    }

    /// Stop capturing and drop the record.
    pub fn capture_clear(&mut self) {
        self.capturing = false;
        self.capture.clear();
        self.capture_full = false;
    }

    pub fn capturing(&self) -> bool {
        self.capturing
    }

    pub fn capture_full(&self) -> bool {
        self.capture_full
    }

    pub fn capture_data(&self) -> &[CaptureFrame] {
        &self.capture
    }

    /// Current state snapshot (also valid before the first frame).
    pub fn status(&self) -> FallStatus {
        FallStatus {
            detected: self.fall_detected,
            spike_score: self.last_spike_score,
            spike_threshold: self.spike_thresh,
            spike_consec: self.spike_consec,
            still_score: self.last_still_score,
            still_avg: self.last_still_avg,
            still_threshold: self.still_thresh,
            pending: self.pending,
            last_alert_unix_ms: self.last_alert_unix_ms,
            frames: self.frame_count,
        }
    }

    /// Recent spike scores, oldest first, for the Debug tab trend chart.
    pub fn score_history(&self) -> Vec<f64> {
        self.score_history.iter().copied().collect()
    }

    fn push_score(&mut self, score: f64) {
        self.score_history.push_back(score);
        while self.score_history.len() > SCORE_HISTORY_CAP {
            self.score_history.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic tiny noise so every run sees identical frames.
    struct Rng(u64);
    impl Rng {
        fn next_f64(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn jitter(&mut self, scale: f64) -> f64 {
            (self.next_f64() - 0.5) * 2.0 * scale
        }
    }

    const N: usize = 64;
    const FPS: f64 = 30.0;

    /// Ambient frame: stable shape + tiny per-frame jitter (Δs ≈ 0.5-1%).
    fn ambient_frame(rng: &mut Rng, jitter: f64) -> Vec<f64> {
        (0..N)
            .map(|i| 10.0 * (1.0 + 0.3 * (i as f64 * 0.3).sin()) + rng.jitter(jitter))
            .collect()
    }

    /// Apply a per-subcarrier differential pattern (even bins up, odd down) —
    /// a body crossing Fresnel zones. `strength` scales the shape change.
    fn shape_change(amp: &mut [f64], strength: f64) {
        for (i, v) in amp.iter_mut().enumerate() {
            *v *= if i % 2 == 0 { 1.0 + strength } else { 1.0 - strength * 0.5 };
        }
    }

    #[test]
    fn steady_amplitude_never_trips() {
        let mut det = FallDetector::new();
        let mut rng = Rng(0x9E3779B97F4A7C15);
        for frame in 0..900u64 {
            let amp = ambient_frame(&mut rng, 0.05);
            let status = det.process(&amp, FPS, frame as i64 * 33);
            assert!(
                !status.detected,
                "frame {frame}: ambient noise latched a fall (score {})",
                status.spike_score
            );
            assert!(!status.pending);
            assert!(
                status.spike_score < status.spike_threshold,
                "ambient z-score should stay under the spike threshold"
            );
        }
    }

    /// The core AGC guarantee: multiplying EVERY subcarrier by a common gain
    /// must leave the shape — and therefore the score — unchanged. The edge
    /// hardware rescales amplitudes like this whenever its AGC re-steps.
    #[test]
    fn agc_gain_step_is_invisible() {
        let mut det = FallDetector::new();
        let mut rng = Rng(0x1234567890ABCDEF);
        for frame in 0..100u64 {
            let amp = ambient_frame(&mut rng, 0.05);
            det.process(&amp, FPS, frame as i64 * 33);
        }
        let before = det.status().spike_score;

        // AGC step: the whole frame rescales by 1.5 (≈ +3.5 dB gain click).
        let mut stepped = ambient_frame(&mut rng, 0.05);
        for v in stepped.iter_mut() {
            *v *= 1.5;
        }
        let status = det.process(&stepped, FPS, 100 * 33);
        assert!(
            (status.spike_score - before).abs() < 0.5,
            "a pure gain step moved the score {} → {} — AGC leaked into the detector",
            before,
            status.spike_score
        );
        assert!(!status.detected && !status.pending);

        // And a +100% step (extreme) still cannot trip it.
        let mut doubled = ambient_frame(&mut rng, 0.05);
        for v in doubled.iter_mut() {
            *v *= 2.0;
        }
        let status = det.process(&doubled, FPS, 101 * 33);
        assert!(!status.detected && !status.pending, "a 2× common gain must not latch");
    }

    /// A fall: violent shape transient (a progressive sweep — every frame of
    /// the fall moves the shape), then the person is on the floor and still →
    /// pending confirms within the stillness window and the latch holds. A
    /// single mid-confirmation twitch must not reset the confirmation; a
    /// second burst while latched must not mint a new alert; the latch
    /// auto-clears after the hold.
    #[test]
    fn fall_transient_then_stillness_latches() {
        let mut det = FallDetector::new();
        let mut rng = Rng(0xDEADBEEF11111111);
        let mut t_ms: i64 = 0;
        for frame in 0..100u64 {
            let amp = ambient_frame(&mut rng, 0.05);
            det.process(&amp, FPS, t_ms);
            t_ms += 33;
        }

        // The fall: ~0.2 s of violent motion — the shape sweeps progressively.
        for step in 0..6u64 {
            let mut amp = ambient_frame(&mut rng, 0.05);
            shape_change(&mut amp, 0.3 * (step as f64 + 1.0));
            det.process(&amp, FPS, t_ms);
            t_ms += 33;
        }
        assert!(det.status().pending, "a violent shape transient must arm the pending state");

        // On the floor: still for the (now 12 s) confirmation window. Single
        // motion pops must not reset the confirmation (the averaged criterion
        // absorbs them).
        let mut latched_at = None;
        for frame in 0..700u64 {
            let mut amp = ambient_frame(&mut rng, 0.05);
            if frame == 30 {
                shape_change(&mut amp, 0.2); // one isolated twitch
            }
            let status = det.process(&amp, FPS, t_ms);
            t_ms += 33;
            if status.detected {
                latched_at = Some(t_ms);
                break;
            }
        }
        assert!(latched_at.is_some(), "spike + stillness (tolerating single pops) must latch a fall");

        // Storm prevention: while latched, a second fall-class burst must not
        // mint a new alert (cooldown/re-arm window).
        let first_alert = det.status().last_alert_unix_ms;
        for _ in 0..5 {
            let mut amp = ambient_frame(&mut rng, 0.05);
            shape_change(&mut amp, 0.4);
            det.process(&amp, FPS, t_ms);
            t_ms += 33;
        }
        for _ in 0..60 {
            let amp = ambient_frame(&mut rng, 0.05);
            det.process(&amp, FPS, t_ms);
            t_ms += 33;
        }
        assert_eq!(
            det.status().last_alert_unix_ms,
            first_alert,
            "no re-latch may occur inside the hold/re-arm window"
        );

        // The latch holds for the hold period, then auto-clears.
        let mut saw_clear = false;
        for frame in 0..600u64 {
            let amp = ambient_frame(&mut rng, 0.05);
            let status = det.process(&amp, FPS, t_ms);
            t_ms += 33;
            if !status.detected {
                saw_clear = true;
                break;
            }
        }
        assert!(saw_clear, "the latch must auto-clear after the hold period");
    }

    /// Walking: sustained moderate shape motion never arms the spike (its
    /// z-score stays under threshold), and a genuine spike followed by
    /// continued movement expires the pending window without latching.
    #[test]
    fn continuous_movement_does_not_latch() {
        let mut det = FallDetector::new();
        let mut rng = Rng(0x5EED5EED5EED5EED);
        let mut t_ms: i64 = 0;
        for frame in 0..100u64 {
            let amp = ambient_frame(&mut rng, 0.05);
            det.process(&amp, FPS, t_ms);
            t_ms += 33;
        }

        // Phase 1 — "walking": the shape alternates every frame (a person
        // moving has no dwell frames). Moderate z, never spikes.
        for frame in 0..450u64 {
            let mut amp = ambient_frame(&mut rng, 0.05);
            let phase = frame % 2;
            for (i, v) in amp.iter_mut().enumerate() {
                *v *= if (i as u64 + phase) % 2 == 0 { 1.06 } else { 0.96 };
            }
            let status = det.process(&amp, FPS, t_ms);
            t_ms += 33;
            assert!(
                !status.detected,
                "frame {frame}: continuous movement latched a fall"
            );
        }

        // Phase 2 — a genuine spike burst, but the person keeps moving (no
        // stillness): pending arms, the pending window expires, no latch.
        for step in 0..6u64 {
            let mut amp = ambient_frame(&mut rng, 0.05);
            shape_change(&mut amp, 0.3 * (step as f64 + 1.0));
            let status = det.process(&amp, FPS, t_ms);
            t_ms += 33;
            assert!(!status.detected);
        }
        assert!(det.status().pending, "the spike burst must arm pending");
        for frame in 0..1500u64 {
            let mut amp = ambient_frame(&mut rng, 0.05);
            let phase = frame % 2;
            for (i, v) in amp.iter_mut().enumerate() {
                *v *= if (i as u64 + phase) % 2 == 0 { 1.06 } else { 0.96 };
            }
            let status = det.process(&amp, FPS, t_ms);
            t_ms += 33;
            assert!(
                !status.detected,
                "frame {frame}: movement after the spike latched a fall"
            );
        }
        assert!(
            !det.status().pending,
            "pending must expire once the movement continues past the window"
        );
    }

    /// A subcarrier-count change (grid switch) must wipe the old baseline.
    #[test]
    fn grid_change_resets_state() {
        let mut det = FallDetector::new();
        let mut rng = Rng(0x00C0FFEE00000001);
        for frame in 0..300u64 {
            let amp = ambient_frame(&mut rng, 0.05);
            det.process(&amp, FPS, frame as i64 * 33);
        }
        assert!(det.status().spike_score >= 0.0);

        let small = vec![10.0f64; 32];
        let status = det.process(&small, FPS, 300 * 33);
        assert_eq!(status.frames, 1, "grid change must reset the frame count");
        assert!(!status.detected && !status.pending);
    }
}
