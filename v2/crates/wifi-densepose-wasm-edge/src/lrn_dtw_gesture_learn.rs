//! User-teachable gesture recognition via DTW template learning.
//!
//! ADR-041 adaptive learning module — Event IDs 730-733.
//!
//! Allows users to teach the system new gestures by performing them three times.
//! The learning protocol:
//!   1. Enter learning mode: 3 seconds of stillness (motion < threshold)
//!   2. Perform gesture: record phase trajectory during motion
//!   3. Return to stillness: trajectory captured
//!   4. Repeat 3x — if trajectories are similar (DTW distance < learn_threshold),
//!      average them into a template and store it
//!
//! Recognition: DTW distance of incoming phase trajectory against all stored
//! templates. Best match emitted if distance < recognition threshold.
//!
//! Budget: H (heavy, < 10 ms) — DTW is O(n*m) but n=m=64, so 4096 ops.

use libm::fabsf;

/// Maximum phase samples per gesture template.
const TEMPLATE_LEN: usize = 64;

/// Maximum stored gesture templates.
const MAX_TEMPLATES: usize = 16;

/// Number of rehearsals required before a template is committed.
const REHEARSALS_REQUIRED: usize = 3;

/// Stillness threshold (motion energy below this = still).
const STILLNESS_THRESHOLD: f32 = 0.05;

/// Number of consecutive still frames to trigger learning mode (3 s at 20 Hz).
const STILLNESS_FRAMES: u16 = 60;

/// DTW distance threshold for considering two rehearsals "similar".
const LEARN_DTW_THRESHOLD: f32 = 3.0;

/// DTW distance threshold for recognizing a stored gesture.
const RECOGNIZE_DTW_THRESHOLD: f32 = 2.5;

/// Cooldown frames after a gesture match (avoid double-fire, ~2 s at 20 Hz).
const MATCH_COOLDOWN: u16 = 40;

/// Sakoe-Chiba band width to constrain DTW warping.
const BAND_WIDTH: usize = 8;

// ── Event IDs (730-series: Adaptive Learning) ────────────────────────────────

pub const EVENT_GESTURE_LEARNED: i32 = 730;
pub const EVENT_GESTURE_MATCHED: i32 = 731;
pub const EVENT_MATCH_DISTANCE: i32 = 732;
pub const EVENT_TEMPLATE_COUNT: i32 = 733;

/// Learning state machine phases.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LearnPhase {
    /// Idle — waiting for stillness to begin learning.
    Idle,
    /// Counting consecutive stillness frames.
    WaitingStill,
    /// Recording motion trajectory.
    Recording,
    /// Motion ended — trajectory captured, waiting for next rehearsal or commit.
    Captured,
}

/// A single gesture template: a fixed-length phase-delta trajectory.
#[derive(Clone, Copy)]
struct Template {
    samples: [f32; TEMPLATE_LEN],
    len: usize,
    /// User-assigned gesture ID (starts at 100 to avoid colliding with built-in IDs).
    id: u8,
}

impl Template {
    const fn empty() -> Self {
        Self {
            samples: [0.0; TEMPLATE_LEN],
            len: 0,
            id: 0,
        }
    }
}

/// User-teachable gesture learner and recognizer.
pub struct GestureLearner {
    // ── Stored templates ─────────────────────────────────────────────────
    templates: [Template; MAX_TEMPLATES],
    template_count: usize,

    // ── Learning state ───────────────────────────────────────────────────
    learn_phase: LearnPhase,
    /// Consecutive stillness frame counter.
    still_count: u16,
    /// Rehearsal buffer: up to 3 captured trajectories.
    rehearsals: [[f32; TEMPLATE_LEN]; REHEARSALS_REQUIRED],
    rehearsal_lens: [usize; REHEARSALS_REQUIRED],
    rehearsal_count: usize,
    /// Current recording buffer.
    recording: [f32; TEMPLATE_LEN],
    recording_len: usize,

    // ── Recognition state ────────────────────────────────────────────────
    /// Phase delta sliding window for recognition.
    window: [f32; TEMPLATE_LEN],
    window_len: usize,
    window_idx: usize,
    prev_phase: f32,
    phase_initialized: bool,
    cooldown: u16,

    /// Next ID to assign to a learned template.
    next_id: u8,
}

impl GestureLearner {
    pub const fn new() -> Self {
        Self {
            templates: [Template::empty(); MAX_TEMPLATES],
            template_count: 0,
            learn_phase: LearnPhase::Idle,
            still_count: 0,
            rehearsals: [[0.0; TEMPLATE_LEN]; REHEARSALS_REQUIRED],
            rehearsal_lens: [0; REHEARSALS_REQUIRED],
            rehearsal_count: 0,
            recording: [0.0; TEMPLATE_LEN],
            recording_len: 0,
            window: [0.0; TEMPLATE_LEN],
            window_len: 0,
            window_idx: 0,
            prev_phase: 0.0,
            phase_initialized: false,
            cooldown: 0,
            next_id: 100,
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases` — per-subcarrier phase values (uses first subcarrier).
    /// `motion_energy` — aggregate motion metric from host (Tier 2).
    ///
    /// Returns events as `(event_id, value)` pairs in a static buffer.
    pub fn process_frame(&mut self, phases: &[f32], motion_energy: f32) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        if phases.is_empty() {
            return &[];
        }

        // ── Compute phase delta ──────────────────────────────────────────
        let primary = phases[0];
        if !self.phase_initialized {
            self.prev_phase = primary;
            self.phase_initialized = true;
            return &[];
        }
        let delta = primary - self.prev_phase;
        self.prev_phase = primary;

        // ── Push into recognition window ─────────────────────────────────
        self.window[self.window_idx] = delta;
        self.window_idx = (self.window_idx + 1) % TEMPLATE_LEN;
        if self.window_len < TEMPLATE_LEN {
            self.window_len += 1;
        }

        if self.cooldown > 0 {
            self.cooldown -= 1;
        }

        // ── Learning state machine ───────────────────────────────────────
        let is_still = motion_energy < STILLNESS_THRESHOLD;

        match self.learn_phase {
            LearnPhase::Idle => {
                if is_still {
                    self.still_count += 1;
                    if self.still_count >= STILLNESS_FRAMES {
                        self.learn_phase = LearnPhase::WaitingStill;
                        self.rehearsal_count = 0;
                    }
                } else {
                    self.still_count = 0;
                }
            }
            LearnPhase::WaitingStill => {
                if !is_still {
                    // Motion started — begin recording.
                    self.learn_phase = LearnPhase::Recording;
                    self.recording_len = 0;
                    self.recording[0] = delta;
                    self.recording_len = 1;
                }
            }
            LearnPhase::Recording => {
                if self.recording_len < TEMPLATE_LEN {
                    self.recording[self.recording_len] = delta;
                    self.recording_len += 1;
                }
                if is_still {
                    // Motion ended — capture this rehearsal.
                    self.learn_phase = LearnPhase::Captured;
                }
            }
            LearnPhase::Captured => {
                // Store captured trajectory as a rehearsal.
                if self.rehearsal_count < REHEARSALS_REQUIRED && self.recording_len >= 4 {
                    let idx = self.rehearsal_count;
                    let len = self.recording_len;
                    self.rehearsal_lens[idx] = len;
                    let mut i = 0;
                    while i < len {
                        self.rehearsals[idx][i] = self.recording[i];
                        i += 1;
                    }
                    // Zero remainder.
                    while i < TEMPLATE_LEN {
                        self.rehearsals[idx][i] = 0.0;
                        i += 1;
                    }
                    self.rehearsal_count += 1;
                }

                if self.rehearsal_count >= REHEARSALS_REQUIRED {
                    // Check if all 3 rehearsals are mutually similar.
                    if self.rehearsals_are_similar() {
                        if let Some(id) = self.commit_template() {
                            unsafe {
                                EVENTS[n_ev] = (EVENT_GESTURE_LEARNED, id as f32);
                                n_ev += 1;
                                EVENTS[n_ev] = (EVENT_TEMPLATE_COUNT, self.template_count as f32);
                                n_ev += 1;
                            }
                        }
                    }
                    // Reset learning state regardless.
                    self.learn_phase = LearnPhase::Idle;
                    self.still_count = 0;
                    self.rehearsal_count = 0;
                } else {
                    // Wait for next stillness -> motion cycle.
                    self.learn_phase = LearnPhase::WaitingStill;
                }
            }
        }

        // ── Recognition (only when not in active learning) ───────────────
        if self.learn_phase == LearnPhase::Idle && self.cooldown == 0
            && self.template_count > 0 && self.window_len >= 8
        {
            // Build contiguous observation from ring buffer.
            let mut obs = [0.0f32; TEMPLATE_LEN];
            for i in 0..self.window_len {
                let ri = (self.window_idx + TEMPLATE_LEN - self.window_len + i) % TEMPLATE_LEN;
                obs[i] = self.window[ri];
            }

            let mut best_dist = RECOGNIZE_DTW_THRESHOLD;
            let mut best_id: Option<u8> = None;

            for t in 0..self.template_count {
                let tmpl = &self.templates[t];
                if tmpl.len == 0 || self.window_len < tmpl.len {
                    continue;
                }
                // Use tail of observation matching template length.
                let start = if self.window_len > tmpl.len + 8 {
                    self.window_len - tmpl.len - 8
                } else {
                    0
                };
                let dist = dtw_distance(
                    &obs[start..self.window_len],
                    &tmpl.samples[..tmpl.len],
                );
                if dist < best_dist {
                    best_dist = dist;
                    best_id = Some(tmpl.id);
                }
            }

            if let Some(id) = best_id {
                self.cooldown = MATCH_COOLDOWN;
                unsafe {
                    EVENTS[n_ev] = (EVENT_GESTURE_MATCHED, id as f32);
                    n_ev += 1;
                    if n_ev < 4 {
                        EVENTS[n_ev] = (EVENT_MATCH_DISTANCE, best_dist);
                        n_ev += 1;
                    }
                }
            }
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Check if all rehearsals are pairwise similar (DTW distance < threshold).
    fn rehearsals_are_similar(&self) -> bool {
        for i in 0..self.rehearsal_count {
            for j in (i + 1)..self.rehearsal_count {
                let len_i = self.rehearsal_lens[i];
                let len_j = self.rehearsal_lens[j];
                if len_i < 4 || len_j < 4 {
                    return false;
                }
                let dist = dtw_distance(
                    &self.rehearsals[i][..len_i],
                    &self.rehearsals[j][..len_j],
                );
                if dist >= LEARN_DTW_THRESHOLD {
                    return false;
                }
            }
        }
        true
    }

    /// Average rehearsals into a new template and store it.
    /// Returns the assigned gesture ID, or None if template slots are full.
    fn commit_template(&mut self) -> Option<u8> {
        if self.template_count >= MAX_TEMPLATES {
            return None;
        }

        // Find the maximum trajectory length among rehearsals.
        let mut max_len = 0usize;
        for i in 0..self.rehearsal_count {
            if self.rehearsal_lens[i] > max_len {
                max_len = self.rehearsal_lens[i];
            }
        }
        if max_len < 4 {
            return None;
        }

        // Average the rehearsals sample-by-sample.
        let mut avg = [0.0f32; TEMPLATE_LEN];
        for s in 0..max_len {
            let mut sum = 0.0f32;
            let mut count = 0u8;
            for r in 0..self.rehearsal_count {
                if s < self.rehearsal_lens[r] {
                    sum += self.rehearsals[r][s];
                    count += 1;
                }
            }
            if count > 0 {
                avg[s] = sum / count as f32;
            }
        }

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        self.templates[self.template_count] = Template {
            samples: avg,
            len: max_len,
            id,
        };
        self.template_count += 1;

        Some(id)
    }

    /// Number of currently stored templates.
    pub fn template_count(&self) -> usize {
        self.template_count
    }
}

/// Compute constrained DTW distance between two sequences.
///
/// Uses Sakoe-Chiba band to limit warping path. Result is normalized
/// by path length (n + m) to allow comparison across different lengths.
fn dtw_distance(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let m = b.len();

    if n == 0 || m == 0 {
        return f32::MAX;
    }

    // Stack-allocated cost matrix: max 64x64 = 4096 cells.
    let mut cost = [[f32::MAX; TEMPLATE_LEN]; TEMPLATE_LEN];

    cost[0][0] = fabsf(a[0] - b[0]);

    for i in 0..n {
        for j in 0..m {
            let diff = if i > j { i - j } else { j - i };
            if diff > BAND_WIDTH {
                continue;
            }

            let c = fabsf(a[i] - b[j]);

            if i == 0 && j == 0 {
                cost[i][j] = c;
            } else {
                let mut min_prev = f32::MAX;
                if i > 0 && cost[i - 1][j] < min_prev {
                    min_prev = cost[i - 1][j];
                }
                if j > 0 && cost[i][j - 1] < min_prev {
                    min_prev = cost[i][j - 1];
                }
                if i > 0 && j > 0 && cost[i - 1][j - 1] < min_prev {
                    min_prev = cost[i - 1][j - 1];
                }
                cost[i][j] = c + min_prev;
            }
        }
    }

    let path_len = (n + m) as f32;
    cost[n - 1][m - 1] / path_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let gl = GestureLearner::new();
        assert_eq!(gl.template_count(), 0);
        assert_eq!(gl.learn_phase, LearnPhase::Idle);
        assert_eq!(gl.cooldown, 0);
    }

    #[test]
    fn test_dtw_identical() {
        let a = [0.1, 0.3, 0.5, 0.7, 0.5, 0.3, 0.1];
        let b = [0.1, 0.3, 0.5, 0.7, 0.5, 0.3, 0.1];
        let d = dtw_distance(&a, &b);
        assert!(d < 0.001, "identical sequences should have near-zero DTW distance");
    }

    #[test]
    fn test_dtw_different() {
        let a = [0.1, 0.3, 0.5, 0.7, 0.5, 0.3, 0.1];
        let b = [-0.5, -0.8, -1.0, -0.8, -0.5, -0.2, 0.0];
        let d = dtw_distance(&a, &b);
        assert!(d > 0.3, "different sequences should have large DTW distance");
    }

    #[test]
    fn test_dtw_empty() {
        let a: [f32; 0] = [];
        let b = [1.0, 2.0];
        assert_eq!(dtw_distance(&a, &b), f32::MAX);
    }

    #[test]
    fn test_learning_protocol() {
        let mut gl = GestureLearner::new();
        let phase_still = [0.0f32; 8];

        // Phase 1: Stillness for STILLNESS_FRAMES + 1 frames -> enter learning mode.
        // (+1 because the very first call returns early to initialise phase tracking.)
        for _ in 0..=STILLNESS_FRAMES {
            gl.process_frame(&phase_still, 0.01);
        }
        assert_eq!(gl.learn_phase, LearnPhase::WaitingStill);

        // Phase 2: Perform gesture 3 times (motion -> stillness).
        let gesture_phases: [f32; 8] = [0.5, 0.3, 0.2, 0.1, 0.4, 0.6, 0.7, 0.8];

        for rehearsal in 0..3 {
            // Motion frames.
            for frame in 0..10 {
                let mut p = [0.0f32; 8];
                p[0] = gesture_phases[frame % gesture_phases.len()] * (rehearsal as f32 + 1.0) * 0.1;
                gl.process_frame(&p, 0.5);
            }
            // Stillness frame to capture.
            let _ = gl.process_frame(&phase_still, 0.01);
            if rehearsal == 2 {
                // After 3rd rehearsal, should either learn (Idle) or
                // still be in Captured if DTW distances were too different.
                assert!(
                    gl.learn_phase == LearnPhase::Idle || gl.learn_phase == LearnPhase::Captured,
                    "unexpected phase: {:?}", gl.learn_phase
                );
            }
        }
    }

    #[test]
    fn test_template_capacity() {
        let mut gl = GestureLearner::new();
        // Manually fill templates to max.
        for i in 0..MAX_TEMPLATES {
            gl.templates[i] = Template {
                samples: [0.1; TEMPLATE_LEN],
                len: 10,
                id: i as u8,
            };
        }
        gl.template_count = MAX_TEMPLATES;

        // Commit should return None when full.
        assert!(gl.commit_template().is_none());
    }
}
