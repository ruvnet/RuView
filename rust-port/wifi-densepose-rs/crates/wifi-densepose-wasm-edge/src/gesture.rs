//! DTW (Dynamic Time Warping) gesture recognition — no_std port.
//!
//! Ported from `ruvsense/gesture.rs` for WASM execution on ESP32-S3.
//! Recognizes predefined gesture templates from CSI phase sequences
//! using constrained DTW with Sakoe-Chiba band.

use libm::fabsf;

/// Maximum gesture template length (samples).
const MAX_TEMPLATE_LEN: usize = 40;

/// Maximum observation window (samples).
const MAX_WINDOW_LEN: usize = 60;

/// Number of predefined gesture templates.
const NUM_TEMPLATES: usize = 4;

/// DTW distance threshold for a match.
const DTW_THRESHOLD: f32 = 2.5;

/// Sakoe-Chiba band width (constrains warping path).
const BAND_WIDTH: usize = 5;

/// Gesture template: a named sequence of phase-delta values.
struct GestureTemplate {
    /// Template values (normalized phase deltas).
    values: [f32; MAX_TEMPLATE_LEN],
    /// Actual length of the template.
    len: usize,
    /// Gesture ID (emitted as event value).
    id: u8,
}

/// DTW gesture detector state.
pub struct GestureDetector {
    /// Sliding window of phase deltas.
    window: [f32; MAX_WINDOW_LEN],
    window_len: usize,
    window_idx: usize,
    /// Previous primary phase (for delta computation).
    prev_phase: f32,
    initialized: bool,
    /// Cooldown counter (frames) to avoid duplicate detections.
    cooldown: u16,
    /// Predefined gesture templates.
    templates: [GestureTemplate; NUM_TEMPLATES],
}

impl GestureDetector {
    pub const fn new() -> Self {
        Self {
            window: [0.0; MAX_WINDOW_LEN],
            window_len: 0,
            window_idx: 0,
            prev_phase: 0.0,
            initialized: false,
            cooldown: 0,
            templates: [
                // Template 1: Wave (oscillating phase)
                GestureTemplate {
                    values: {
                        let mut v = [0.0f32; MAX_TEMPLATE_LEN];
                        // Manually define a wave pattern
                        v[0] = 0.5; v[1] = 0.8; v[2] = 0.3; v[3] = -0.3;
                        v[4] = -0.8; v[5] = -0.5; v[6] = 0.3; v[7] = 0.8;
                        v[8] = 0.5; v[9] = -0.3; v[10] = -0.8; v[11] = -0.5;
                        v
                    },
                    len: 12,
                    id: 1,
                },
                // Template 2: Push (steady positive phase shift)
                GestureTemplate {
                    values: {
                        let mut v = [0.0f32; MAX_TEMPLATE_LEN];
                        v[0] = 0.1; v[1] = 0.3; v[2] = 0.5; v[3] = 0.7;
                        v[4] = 0.6; v[5] = 0.4; v[6] = 0.2; v[7] = 0.0;
                        v
                    },
                    len: 8,
                    id: 2,
                },
                // Template 3: Pull (steady negative phase shift)
                GestureTemplate {
                    values: {
                        let mut v = [0.0f32; MAX_TEMPLATE_LEN];
                        v[0] = -0.1; v[1] = -0.3; v[2] = -0.5; v[3] = -0.7;
                        v[4] = -0.6; v[5] = -0.4; v[6] = -0.2; v[7] = 0.0;
                        v
                    },
                    len: 8,
                    id: 3,
                },
                // Template 4: Swipe (sharp directional change)
                GestureTemplate {
                    values: {
                        let mut v = [0.0f32; MAX_TEMPLATE_LEN];
                        v[0] = 0.0; v[1] = 0.2; v[2] = 0.6; v[3] = 1.0;
                        v[4] = 0.8; v[5] = 0.2; v[6] = -0.2; v[7] = -0.4;
                        v[8] = -0.3; v[9] = -0.1;
                        v
                    },
                    len: 10,
                    id: 4,
                },
            ],
        }
    }

    /// Process one frame's phase data, returning a gesture ID if detected.
    pub fn process_frame(&mut self, phases: &[f32]) -> Option<u8> {
        if phases.is_empty() {
            return None;
        }

        // Decrement cooldown.
        if self.cooldown > 0 {
            self.cooldown -= 1;
            // Still need to update state even during cooldown.
        }

        // Use primary (first) subcarrier phase for gesture detection.
        let primary_phase = phases[0];

        if !self.initialized {
            self.prev_phase = primary_phase;
            self.initialized = true;
            return None;
        }

        // Compute phase delta.
        let delta = primary_phase - self.prev_phase;
        self.prev_phase = primary_phase;

        // Add to sliding window (ring buffer).
        self.window[self.window_idx] = delta;
        self.window_idx = (self.window_idx + 1) % MAX_WINDOW_LEN;
        if self.window_len < MAX_WINDOW_LEN {
            self.window_len += 1;
        }

        // Need minimum window before attempting matching.
        if self.window_len < 8 || self.cooldown > 0 {
            return None;
        }

        // Build contiguous observation from ring buffer.
        let mut obs = [0.0f32; MAX_WINDOW_LEN];
        for i in 0..self.window_len {
            let ri = (self.window_idx + MAX_WINDOW_LEN - self.window_len + i) % MAX_WINDOW_LEN;
            obs[i] = self.window[ri];
        }

        // Match against each template.
        let mut best_id: Option<u8> = None;
        let mut best_dist = DTW_THRESHOLD;

        for tmpl in &self.templates {
            if tmpl.len == 0 || self.window_len < tmpl.len {
                continue;
            }

            // Use only the tail of the observation (matching template length + margin).
            let obs_start = if self.window_len > tmpl.len + 10 {
                self.window_len - tmpl.len - 10
            } else {
                0
            };
            let obs_slice = &obs[obs_start..self.window_len];

            let dist = dtw_distance(obs_slice, &tmpl.values[..tmpl.len]);
            if dist < best_dist {
                best_dist = dist;
                best_id = Some(tmpl.id);
            }
        }

        if best_id.is_some() {
            self.cooldown = 40; // ~2 seconds at 20 Hz.
        }

        best_id
    }
}

/// Compute constrained DTW distance between two sequences.
/// Uses Sakoe-Chiba band to limit warping and reduce computation.
fn dtw_distance(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let m = b.len();

    if n == 0 || m == 0 {
        return f32::MAX;
    }

    // Use a flat array on stack (max 60 × 40 = 2400 entries).
    // For WASM, this uses linear memory which is fine.
    const MAX_N: usize = MAX_WINDOW_LEN;
    const MAX_M: usize = MAX_TEMPLATE_LEN;
    let mut cost = [[f32::MAX; MAX_M]; MAX_N];

    cost[0][0] = fabsf(a[0] - b[0]);

    for i in 0..n {
        for j in 0..m {
            // Sakoe-Chiba band constraint.
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

    // Normalize by path length.
    let path_len = (n + m) as f32;
    cost[n - 1][m - 1] / path_len
}
