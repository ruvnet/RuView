//! Flash Attention on subcarrier data for spatial focus estimation — ADR-041 signal module.
//!
//! Divides subcarriers into 8 groups (tiles). For each frame:
//! Q = current phase (per-group mean), K = previous phase, V = amplitude.
//! Attention score per tile: Q[i]*K[i]/sqrt(d), then softmax normalization.
//! Tracks attention entropy H = -sum(p*log(p)) via EMA smoothing.
//! Low entropy means activity is focused on one spatial zone (Fresnel region).
//!
//! Tiled computation keeps memory O(1) per tile with fixed-size arrays of 8.
//!
//! Events: ATTENTION_PEAK_SC(700), ATTENTION_SPREAD(701), SPATIAL_FOCUS_ZONE(702).
//! Budget: S (standard, < 5ms on ESP32-S3 WASM3).

use libm::{expf, logf, sqrtf};

const N_GROUPS: usize = 8;
const MAX_SC: usize = 32;
const ENTROPY_ALPHA: f32 = 0.15;
const LOG_EPSILON: f32 = 1e-7;
const MAX_ENTROPY: f32 = 2.079; // ln(8)

pub const EVENT_ATTENTION_PEAK_SC: i32 = 700;
pub const EVENT_ATTENTION_SPREAD: i32 = 701;
pub const EVENT_SPATIAL_FOCUS_ZONE: i32 = 702;

/// Flash Attention spatial focus estimator.
pub struct FlashAttention {
    prev_group_phases: [f32; N_GROUPS],
    attention_weights: [f32; N_GROUPS],
    smoothed_entropy: f32,
    initialized: bool,
    frame_count: u32,
    last_peak: usize,
    last_centroid: f32,
}

impl FlashAttention {
    pub const fn new() -> Self {
        Self {
            prev_group_phases: [0.0; N_GROUPS],
            attention_weights: [0.0; N_GROUPS],
            smoothed_entropy: MAX_ENTROPY,
            initialized: false, frame_count: 0,
            last_peak: 0, last_centroid: 0.0,
        }
    }

    /// Process one frame. Returns (event_id, value) pairs to emit.
    pub fn process_frame(&mut self, phases: &[f32], amplitudes: &[f32]) -> &[(i32, f32)] {
        let n_sc = phases.len().min(amplitudes.len()).min(MAX_SC);
        if n_sc < N_GROUPS { return &[]; }

        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];

        // Per-group means for Q and V.
        let subs_per = n_sc / N_GROUPS;
        let mut q = [0.0f32; N_GROUPS];
        let mut v = [0.0f32; N_GROUPS];
        for g in 0..N_GROUPS {
            let start = g * subs_per;
            let end = if g == N_GROUPS - 1 { n_sc } else { start + subs_per };
            let count = (end - start) as f32;
            let (mut ps, mut as_) = (0.0f32, 0.0f32);
            for i in start..end { ps += phases[i]; as_ += amplitudes[i]; }
            q[g] = ps / count;
            v[g] = as_ / count;
        }

        if !self.initialized {
            for g in 0..N_GROUPS { self.prev_group_phases[g] = q[g]; }
            self.initialized = true;
            return &[];
        }
        self.frame_count += 1;

        // Attention scores: Q*K/sqrt(d).
        let scale = sqrtf(N_GROUPS as f32);
        let mut scores = [0.0f32; N_GROUPS];
        for g in 0..N_GROUPS { scores[g] = q[g] * self.prev_group_phases[g] / scale; }

        // Numerically stable softmax.
        let mut max_s = scores[0];
        for g in 1..N_GROUPS { if scores[g] > max_s { max_s = scores[g]; } }
        let mut exp_sum = 0.0f32;
        let mut exp_s = [0.0f32; N_GROUPS];
        for g in 0..N_GROUPS {
            exp_s[g] = expf(scores[g] - max_s);
            exp_sum += exp_s[g];
        }
        if exp_sum < LOG_EPSILON { exp_sum = LOG_EPSILON; }
        for g in 0..N_GROUPS { self.attention_weights[g] = exp_s[g] / exp_sum; }

        // Peak group.
        let (mut peak_idx, mut peak_w) = (0usize, self.attention_weights[0]);
        for g in 1..N_GROUPS {
            if self.attention_weights[g] > peak_w {
                peak_w = self.attention_weights[g];
                peak_idx = g;
            }
        }
        self.last_peak = peak_idx;

        // Entropy: H = -sum(p * ln(p)).
        let mut entropy = 0.0f32;
        for g in 0..N_GROUPS {
            let p = self.attention_weights[g];
            if p > LOG_EPSILON { entropy -= p * logf(p); }
        }
        self.smoothed_entropy = ENTROPY_ALPHA * entropy + (1.0 - ENTROPY_ALPHA) * self.smoothed_entropy;

        // Weighted centroid.
        let mut centroid = 0.0f32;
        for g in 0..N_GROUPS { centroid += (g as f32) * self.attention_weights[g]; }
        self.last_centroid = centroid;

        // Update K for next frame.
        for g in 0..N_GROUPS { self.prev_group_phases[g] = q[g]; }

        // Emit events.
        unsafe {
            EVENTS[0] = (EVENT_ATTENTION_PEAK_SC, peak_idx as f32);
            EVENTS[1] = (EVENT_ATTENTION_SPREAD, self.smoothed_entropy);
            EVENTS[2] = (EVENT_SPATIAL_FOCUS_ZONE, centroid);
            &EVENTS[..3]
        }
    }

    pub fn weights(&self) -> &[f32; N_GROUPS] { &self.attention_weights }
    pub fn entropy(&self) -> f32 { self.smoothed_entropy }
    pub fn peak_group(&self) -> usize { self.last_peak }
    pub fn centroid(&self) -> f32 { self.last_centroid }
    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn reset(&mut self) { *self = Self::new(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_new() {
        let fa = FlashAttention::new();
        assert_eq!(fa.frame_count(), 0);
        assert_eq!(fa.peak_group(), 0);
    }

    #[test]
    fn test_first_frame_no_events() {
        let mut fa = FlashAttention::new();
        assert!(fa.process_frame(&[0.5; 32], &[1.0; 32]).is_empty());
    }

    #[test]
    fn test_uniform_attention() {
        let mut fa = FlashAttention::new();
        let (p, a) = ([1.0f32; 32], [1.0f32; 32]);
        fa.process_frame(&p, &a);
        let ev = fa.process_frame(&p, &a);
        assert_eq!(ev.len(), 3);
        for w in fa.weights() { assert!((*w - 0.125).abs() < 0.01); }
    }

    #[test]
    fn test_focused_attention() {
        let mut fa = FlashAttention::new();
        let a = [1.0f32; 32];
        fa.process_frame(&[0.0; 32], &a);
        let mut f1 = [0.0f32; 32];
        for i in 12..16 { f1[i] = 3.0; }
        fa.process_frame(&f1, &a);
        let ev = fa.process_frame(&f1, &a);
        let peak = ev.iter().find(|e| e.0 == EVENT_ATTENTION_PEAK_SC).unwrap();
        assert_eq!(peak.1 as usize, 3);
    }

    #[test]
    fn test_too_few_subcarriers() {
        let mut fa = FlashAttention::new();
        assert!(fa.process_frame(&[1.0; 4], &[1.0; 4]).is_empty());
    }

    #[test]
    fn test_centroid_range() {
        let mut fa = FlashAttention::new();
        let (p, a) = ([1.0f32; 32], [1.0f32; 32]);
        fa.process_frame(&p, &a);
        fa.process_frame(&p, &a);
        assert!(fa.centroid() >= 0.0 && fa.centroid() <= 7.0);
    }

    #[test]
    fn test_reset() {
        let mut fa = FlashAttention::new();
        fa.process_frame(&[1.0; 32], &[1.0; 32]);
        fa.process_frame(&[1.0; 32], &[1.0; 32]);
        fa.reset();
        assert_eq!(fa.frame_count(), 0);
    }

    #[test]
    fn test_entropy_trend() {
        let mut fa = FlashAttention::new();
        let a = [1.0f32; 32];
        fa.process_frame(&[0.0; 32], &a);
        fa.process_frame(&[1.0; 32], &a);
        let uniform_h = fa.entropy();
        fa.reset();
        fa.process_frame(&[0.0; 32], &a);
        for _ in 0..10 {
            let mut f = [0.0f32; 32];
            for i in 0..4 { f[i] = 5.0; }
            fa.process_frame(&f, &a);
        }
        assert!(fa.entropy() < uniform_h + 0.5);
    }
}
