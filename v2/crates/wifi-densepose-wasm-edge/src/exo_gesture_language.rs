//! Sign language letter recognition from CSI signatures — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Classifies hand/arm movements into sign language letter groups using
//! WiFi CSI phase and amplitude patterns.  Since full 26-letter ASL template
//! storage is impractical on a constrained WASM edge device, we use a
//! simplified approach:
//!
//! 1. **Feature extraction** -- Extract a compact signature from each CSI
//!    frame: mean phase, phase spread, mean amplitude, amplitude spread,
//!    motion energy, and variance.  These 6 features are accumulated into
//!    a short time-series (gesture window).
//!
//! 2. **Template matching** -- Up to 26 reference templates (one per letter)
//!    can be loaded.  Each template is a fixed-length feature sequence.
//!    We use DTW (Dynamic Time Warping) with a Sakoe-Chiba band to match
//!    the current gesture window against all loaded templates.
//!
//! 3. **Decision threshold** -- Only accept a match if the DTW distance is
//!    below a configurable threshold.  Reject non-letter movements.
//!
//! 4. **Word boundary detection** -- A pause (low motion energy for N frames)
//!    between gestures signals a word boundary.
//!
//! # Events (620-623: Exotic / Research)
//!
//! - `LETTER_RECOGNIZED` (620): Letter index (0=A, 1=B, ..., 25=Z).
//! - `LETTER_CONFIDENCE` (621): Inverse DTW distance (higher = better match).
//! - `WORD_BOUNDARY` (622): 1.0 when word boundary detected.
//! - `GESTURE_REJECTED` (623): 1.0 when gesture did not match any template.
//!
//! # Budget
//!
//! H (heavy, < 10 ms) -- DTW over short sequences (max 32 frames, 26 templates).

use crate::vendor_common::Ema;
use libm::sqrtf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of letter templates.
const MAX_TEMPLATES: usize = 26;

/// Feature dimension per frame (phase_mean, phase_spread, amp_mean, amp_spread,
/// motion_energy, variance).
const FEAT_DIM: usize = 6;

/// Maximum gesture window length (frames at 20 Hz).
const GESTURE_WIN_LEN: usize = 32;

/// Maximum subcarriers to consider.
const MAX_SC: usize = 32;

/// Minimum gesture window fill before attempting matching.
const MIN_GESTURE_FILL: usize = 8;

/// DTW match acceptance threshold (normalized distance).
const MATCH_THRESHOLD: f32 = 0.5;

/// DTW Sakoe-Chiba band width.
const DTW_BAND: usize = 4;

/// Word boundary: number of consecutive low-motion frames.
const WORD_PAUSE_FRAMES: u32 = 15;

/// Motion threshold for "low motion" (pause detection).
const PAUSE_MOTION_THRESH: f32 = 0.08;

/// EMA smoothing for motion energy.
const MOTION_ALPHA: f32 = 0.2;

/// Minimum frames between recognized letters (debounce).
const DEBOUNCE_FRAMES: u32 = 10;

// ── Event IDs (620-623: Exotic) ──────────────────────────────────────────────

pub const EVENT_LETTER_RECOGNIZED: i32 = 620;
pub const EVENT_LETTER_CONFIDENCE: i32 = 621;
pub const EVENT_WORD_BOUNDARY: i32 = 622;
pub const EVENT_GESTURE_REJECTED: i32 = 623;

// ── Gesture Language Detector ────────────────────────────────────────────────

/// Sign language letter recognition from WiFi CSI signatures.
///
/// Supports up to 26 letter templates loaded via `set_template()`.
/// Uses DTW matching on compact feature sequences.
pub struct GestureLanguageDetector {
    /// Template feature sequences: [template_idx][frame][feature].
    templates: [[[f32; FEAT_DIM]; GESTURE_WIN_LEN]; MAX_TEMPLATES],
    /// Length of each template (0 = not loaded).
    template_lens: [usize; MAX_TEMPLATES],
    /// Number of loaded templates.
    n_templates: usize,
    /// Current gesture window feature buffer.
    gesture_buf: [[f32; FEAT_DIM]; GESTURE_WIN_LEN],
    /// Current fill of gesture buffer.
    gesture_fill: usize,
    /// Whether we are in an active gesture (motion detected).
    gesture_active: bool,
    /// EMA-smoothed motion energy.
    motion_ema: Ema,
    /// Consecutive low-motion frames (for word boundary).
    pause_count: u32,
    /// Whether a word boundary was already emitted for this pause.
    word_boundary_emitted: bool,
    /// Frames since last recognized letter (debounce).
    since_last_letter: u32,
    /// Last recognized letter index (255 = none).
    last_letter: u8,
    /// Last match confidence.
    last_confidence: f32,
    /// Total frames processed.
    frame_count: u32,
}

impl GestureLanguageDetector {
    pub const fn new() -> Self {
        Self {
            templates: [[[0.0; FEAT_DIM]; GESTURE_WIN_LEN]; MAX_TEMPLATES],
            template_lens: [0; MAX_TEMPLATES],
            n_templates: 0,
            gesture_buf: [[0.0; FEAT_DIM]; GESTURE_WIN_LEN],
            gesture_fill: 0,
            gesture_active: false,
            motion_ema: Ema::new(MOTION_ALPHA),
            pause_count: 0,
            word_boundary_emitted: false,
            since_last_letter: DEBOUNCE_FRAMES,
            last_letter: 255,
            last_confidence: 0.0,
            frame_count: 0,
        }
    }

    /// Load a template for letter `index` (0=A, ..., 25=Z).
    ///
    /// `features` is a sequence of frames, each with `FEAT_DIM` values.
    /// Length must be <= `GESTURE_WIN_LEN`.
    pub fn set_template(&mut self, index: usize, features: &[[f32; FEAT_DIM]]) {
        if index >= MAX_TEMPLATES {
            return;
        }
        let len = if features.len() > GESTURE_WIN_LEN {
            GESTURE_WIN_LEN
        } else {
            features.len()
        };

        for i in 0..len {
            self.templates[index][i] = features[i];
        }
        self.template_lens[index] = len;

        // Recount loaded templates.
        self.n_templates = 0;
        for i in 0..MAX_TEMPLATES {
            if self.template_lens[i] > 0 {
                self.n_templates += 1;
            }
        }
    }

    /// Load a simple synthetic template for testing: a ramp pattern for each letter.
    pub fn load_synthetic_templates(&mut self) {
        for letter in 0..MAX_TEMPLATES {
            let base = letter as f32 * 0.1;
            let len = 12; // 12-frame templates.
            for f in 0..len {
                let t = f as f32 / len as f32;
                self.templates[letter][f] = [
                    base + t * 0.5,           // phase mean ramp
                    0.1 + base * 0.05,        // phase spread
                    0.5 + base * 0.1 + t * 0.2, // amp mean
                    0.05,                      // amp spread
                    0.3 * t,                   // motion energy
                    0.1 + t * 0.05,            // variance
                ];
            }
            self.template_lens[letter] = len;
        }
        self.n_templates = MAX_TEMPLATES;
    }

    /// Process one CSI frame.
    ///
    /// # Arguments
    /// - `phases` -- per-subcarrier phase values.
    /// - `amplitudes` -- per-subcarrier amplitude values.
    /// - `variance` -- representative variance.
    /// - `motion_energy` -- motion energy from Tier 2.
    /// - `presence` -- 1 if person present.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: f32,
        motion_energy: f32,
        presence: i32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.frame_count += 1;
        self.since_last_letter += 1;

        let smoothed_motion = self.motion_ema.update(motion_energy);

        // No person -> reset gesture state.
        if presence == 0 {
            self.reset_gesture();
            return &[];
        }

        // ── Word boundary detection ──
        if smoothed_motion < PAUSE_MOTION_THRESH {
            self.pause_count += 1;
            if self.pause_count >= WORD_PAUSE_FRAMES && !self.word_boundary_emitted {
                // End of gesture: attempt matching if we have data.
                if self.gesture_fill >= MIN_GESTURE_FILL && self.gesture_active {
                    let (letter, confidence) = self.match_gesture();
                    if letter < MAX_TEMPLATES as u8 && self.since_last_letter >= DEBOUNCE_FRAMES {
                        unsafe {
                            EVENTS[n_ev] = (EVENT_LETTER_RECOGNIZED, letter as f32);
                        }
                        n_ev += 1;
                        unsafe {
                            EVENTS[n_ev] = (EVENT_LETTER_CONFIDENCE, confidence);
                        }
                        n_ev += 1;
                        self.last_letter = letter;
                        self.last_confidence = confidence;
                        self.since_last_letter = 0;
                    } else {
                        unsafe {
                            EVENTS[n_ev] = (EVENT_GESTURE_REJECTED, 1.0);
                        }
                        n_ev += 1;
                    }
                }

                // Emit word boundary.
                unsafe {
                    EVENTS[n_ev] = (EVENT_WORD_BOUNDARY, 1.0);
                }
                n_ev += 1;
                self.word_boundary_emitted = true;
                self.reset_gesture();
            }
        } else {
            self.pause_count = 0;
            self.word_boundary_emitted = false;
            self.gesture_active = true;

            // ── Feature extraction and buffering ──
            let n_sc = min_usize(phases.len(), min_usize(amplitudes.len(), MAX_SC));
            if n_sc > 0 && self.gesture_fill < GESTURE_WIN_LEN {
                let features = extract_features(phases, amplitudes, n_sc, motion_energy, variance);
                self.gesture_buf[self.gesture_fill] = features;
                self.gesture_fill += 1;
            }
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Match the current gesture buffer against all loaded templates.
    /// Returns (best_letter, confidence). Letter = 255 if no match.
    fn match_gesture(&self) -> (u8, f32) {
        if self.n_templates == 0 || self.gesture_fill < MIN_GESTURE_FILL {
            return (255, 0.0);
        }

        let mut best_dist = f32::MAX;
        let mut best_idx: u8 = 255;

        for t in 0..MAX_TEMPLATES {
            let tlen = self.template_lens[t];
            if tlen < MIN_GESTURE_FILL {
                continue;
            }

            let dist = self.dtw_multivariate(t, tlen);
            if dist < best_dist {
                best_dist = dist;
                best_idx = t as u8;
            }
        }

        if best_dist < MATCH_THRESHOLD && best_idx < MAX_TEMPLATES as u8 {
            // Confidence: inverse distance, clamped to [0, 1].
            let confidence = if best_dist > 0.0 {
                let c = 1.0 - (best_dist / MATCH_THRESHOLD);
                if c < 0.0 { 0.0 } else if c > 1.0 { 1.0 } else { c }
            } else {
                1.0
            };
            (best_idx, confidence)
        } else {
            (255, 0.0)
        }
    }

    /// Multivariate DTW between gesture buffer and template `t_idx`.
    ///
    /// Uses Sakoe-Chiba band and computes Euclidean distance across all
    /// `FEAT_DIM` features per frame.
    fn dtw_multivariate(&self, t_idx: usize, t_len: usize) -> f32 {
        let n = self.gesture_fill;
        let m = t_len;

        if n == 0 || m == 0 || n > GESTURE_WIN_LEN || m > GESTURE_WIN_LEN {
            return f32::MAX;
        }

        // Stack-allocated cost matrix.
        let mut cost = [[f32::MAX; GESTURE_WIN_LEN]; GESTURE_WIN_LEN];

        cost[0][0] = frame_distance(&self.gesture_buf[0], &self.templates[t_idx][0]);

        for i in 0..n {
            for j in 0..m {
                let diff = if i > j { i - j } else { j - i };
                if diff > DTW_BAND {
                    continue;
                }

                let c = frame_distance(&self.gesture_buf[i], &self.templates[t_idx][j]);
                if i == 0 && j == 0 {
                    cost[0][0] = c;
                } else {
                    let mut prev = f32::MAX;
                    if i > 0 && cost[i - 1][j] < prev {
                        prev = cost[i - 1][j];
                    }
                    if j > 0 && cost[i][j - 1] < prev {
                        prev = cost[i][j - 1];
                    }
                    if i > 0 && j > 0 && cost[i - 1][j - 1] < prev {
                        prev = cost[i - 1][j - 1];
                    }
                    cost[i][j] = c + prev;
                }
            }
        }

        // Normalize by path length.
        cost[n - 1][m - 1] / (n + m) as f32
    }

    /// Reset the gesture buffer and active state.
    fn reset_gesture(&mut self) {
        self.gesture_fill = 0;
        self.gesture_active = false;
    }

    /// Get the last recognized letter (255 = none).
    pub fn last_letter(&self) -> u8 {
        self.last_letter
    }

    /// Get the last match confidence [0, 1].
    pub fn last_confidence(&self) -> f32 {
        self.last_confidence
    }

    /// Get number of loaded templates.
    pub fn template_count(&self) -> usize {
        self.n_templates
    }

    /// Total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset to initial state (clears templates too).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Extract compact 6D feature vector from raw CSI arrays.
fn extract_features(
    phases: &[f32],
    amplitudes: &[f32],
    n_sc: usize,
    motion_energy: f32,
    variance: f32,
) -> [f32; FEAT_DIM] {
    let mut phase_sum = 0.0f32;
    let mut amp_sum = 0.0f32;
    let mut phase_sq_sum = 0.0f32;
    let mut amp_sq_sum = 0.0f32;

    for i in 0..n_sc {
        phase_sum += phases[i];
        amp_sum += amplitudes[i];
        phase_sq_sum += phases[i] * phases[i];
        amp_sq_sum += amplitudes[i] * amplitudes[i];
    }

    let n = n_sc as f32;
    let phase_mean = phase_sum / n;
    let amp_mean = amp_sum / n;
    let phase_var = phase_sq_sum / n - phase_mean * phase_mean;
    let amp_var = amp_sq_sum / n - amp_mean * amp_mean;
    let phase_spread = sqrtf(if phase_var > 0.0 { phase_var } else { 0.0 });
    let amp_spread = sqrtf(if amp_var > 0.0 { amp_var } else { 0.0 });

    [phase_mean, phase_spread, amp_mean, amp_spread, motion_energy, variance]
}

/// Euclidean distance between two feature frames.
fn frame_distance(a: &[f32; FEAT_DIM], b: &[f32; FEAT_DIM]) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..FEAT_DIM {
        let d = a[i] - b[i];
        sum += d * d;
    }
    sqrtf(sum)
}

/// Minimum of two usize values.
const fn min_usize(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use libm::fabsf;

    #[test]
    fn test_const_new() {
        let gl = GestureLanguageDetector::new();
        assert_eq!(gl.frame_count(), 0);
        assert_eq!(gl.last_letter(), 255);
        assert_eq!(gl.template_count(), 0);
    }

    #[test]
    fn test_no_templates_no_match() {
        let mut gl = GestureLanguageDetector::new();
        let phases = [0.5f32; 16];
        let amps = [1.0f32; 16];
        // Feed motion frames then pause.
        for _ in 0..20 {
            gl.process_frame(&phases, &amps, 0.1, 0.5, 1);
        }
        // Pause to trigger matching.
        for _ in 0..20 {
            gl.process_frame(&phases, &amps, 0.0, 0.01, 1);
        }
        assert_eq!(gl.last_letter(), 255, "no templates -> no match");
    }

    #[test]
    fn test_load_synthetic_templates() {
        let mut gl = GestureLanguageDetector::new();
        gl.load_synthetic_templates();
        assert_eq!(gl.template_count(), 26, "should have 26 templates loaded");
    }

    #[test]
    fn test_set_template() {
        let mut gl = GestureLanguageDetector::new();
        let features = [[0.1, 0.2, 0.3, 0.4, 0.5, 0.6]; 10];
        gl.set_template(0, &features);
        assert_eq!(gl.template_count(), 1);
    }

    #[test]
    fn test_word_boundary_on_pause() {
        let mut gl = GestureLanguageDetector::new();
        let phases = [0.5f32; 16];
        let amps = [1.0f32; 16];
        // Feed active gesture.
        for _ in 0..20 {
            gl.process_frame(&phases, &amps, 0.1, 0.5, 1);
        }
        // Now pause.
        let mut word_boundary_found = false;
        for _ in 0..30 {
            let events = gl.process_frame(&phases, &amps, 0.0, 0.01, 1);
            for ev in events {
                if ev.0 == EVENT_WORD_BOUNDARY {
                    word_boundary_found = true;
                }
            }
        }
        assert!(word_boundary_found, "should emit word boundary after pause");
    }

    #[test]
    fn test_no_presence_resets_gesture() {
        let mut gl = GestureLanguageDetector::new();
        let phases = [0.5f32; 16];
        let amps = [1.0f32; 16];
        // Feed active gesture.
        for _ in 0..10 {
            gl.process_frame(&phases, &amps, 0.1, 0.5, 1);
        }
        // No presence.
        let events = gl.process_frame(&phases, &amps, 0.0, 0.0, 0);
        assert!(events.is_empty(), "no presence should produce no events");
    }

    #[test]
    fn test_frame_distance_identity() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let d = frame_distance(&a, &a);
        assert!(d < 1e-6, "distance to self should be ~0, got {}", d);
    }

    #[test]
    fn test_frame_distance_positive() {
        let a = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let d = frame_distance(&a, &b);
        assert!(fabsf(d - 1.0) < 1e-6, "expected 1.0, got {}", d);
    }

    #[test]
    fn test_extract_features_basic() {
        let phases = [1.0f32; 8];
        let amps = [2.0f32; 8];
        let feats = extract_features(&phases, &amps, 8, 0.5, 0.1);
        assert!(fabsf(feats[0] - 1.0) < 1e-6, "phase mean should be 1.0");
        assert!(fabsf(feats[2] - 2.0) < 1e-6, "amp mean should be 2.0");
        assert!(fabsf(feats[4] - 0.5) < 1e-6, "motion energy should be 0.5");
    }

    #[test]
    fn test_gesture_rejected_on_mismatch() {
        let mut gl = GestureLanguageDetector::new();
        // Load one template with very specific values.
        let features: [[f32; FEAT_DIM]; 12] = [[10.0, 10.0, 10.0, 10.0, 10.0, 10.0]; 12];
        gl.set_template(0, &features);

        let phases = [0.01f32; 16];
        let amps = [0.01f32; 16];
        // Feed very different gesture.
        for _ in 0..20 {
            gl.process_frame(&phases, &amps, 0.01, 0.5, 1);
        }
        // Pause to trigger matching.
        let mut rejected = false;
        for _ in 0..30 {
            let events = gl.process_frame(&phases, &amps, 0.0, 0.01, 1);
            for ev in events {
                if ev.0 == EVENT_GESTURE_REJECTED {
                    rejected = true;
                }
            }
        }
        assert!(rejected, "mismatched gesture should be rejected");
    }

    #[test]
    fn test_reset() {
        let mut gl = GestureLanguageDetector::new();
        gl.load_synthetic_templates();
        let phases = [0.5f32; 16];
        let amps = [1.0f32; 16];
        for _ in 0..50 {
            gl.process_frame(&phases, &amps, 0.1, 0.5, 1);
        }
        assert!(gl.frame_count() > 0);
        gl.reset();
        assert_eq!(gl.frame_count(), 0);
        assert_eq!(gl.template_count(), 0);
    }
}
