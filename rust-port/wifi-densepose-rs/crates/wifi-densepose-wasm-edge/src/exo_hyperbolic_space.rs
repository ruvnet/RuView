//! Poincare ball embedding for hierarchical location classification — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Embeds CSI fingerprints into a 2D Poincare disk (curvature c=1) to exploit
//! the natural hierarchy of indoor spaces: rooms contain zones.  Hyperbolic
//! geometry gives exponentially more "area" near the boundary, making it ideal
//! for tree-structured location taxonomies.
//!
//! ## Embedding Pipeline
//!
//! 1. Extract an 8D CSI feature vector from the current frame (mean amplitude
//!    across 8 subcarrier groups, matching the flash-attention tiling).
//! 2. Project to 2D via a learned linear map: `p = W * features` where
//!    `W` is a 2x8 matrix set during calibration.
//! 3. Normalize to the Poincare disk: if `||p|| >= 1`, scale to 0.95.
//! 4. Find the nearest reference point by Poincare distance:
//!    `d(x,y) = acosh(1 + 2*||x-y||^2 / ((1-||x||^2)*(1-||y||^2)))`.
//! 5. Determine hierarchy level from the embedding radius:
//!    `||p|| < 0.5` -> room-level, `||p|| >= 0.5` -> zone-level.
//! 6. EMA-smooth the position to avoid jitter.
//!
//! ## Reference Layout (16 points)
//!
//! - 4 room-level refs at radius 0.3, evenly spaced at angles 0, pi/2, pi, 3pi/2.
//!   Labels 0-3 (bathroom, kitchen, living room, bedroom).
//! - 12 zone-level refs at radius 0.7, 3 per room, clustered around each
//!   room's angular position.  Labels 4-15.
//!
//! # Events (685-series: Exotic / Research)
//!
//! - `HIERARCHY_LEVEL` (685): 0 = room level, 1 = zone level.
//! - `HYPERBOLIC_RADIUS` (686): Poincare disk radius [0, 1) of embedding.
//! - `LOCATION_LABEL` (687): Nearest reference label (0-15).
//!
//! # Budget
//!
//! S (standard, < 5 ms) -- 16 Poincare distance computations + projection.

use crate::vendor_common::Ema;
use libm::{acoshf, sqrtf};

// ── Constants ────────────────────────────────────────────────────────────────

/// Poincare disk dimension.
const DIM: usize = 2;

/// Feature vector dimension from CSI (8 subcarrier groups).
const FEAT_DIM: usize = 8;

/// Number of reference embeddings.
const N_REFS: usize = 16;

/// Maximum subcarriers from host API.
const MAX_SC: usize = 32;

/// Maximum allowed norm in the Poincare disk (must be < 1).
const MAX_NORM: f32 = 0.95;

/// Radius threshold separating room-level from zone-level.
const LEVEL_RADIUS_THRESHOLD: f32 = 0.5;

/// EMA smoothing factor for position.
const POS_ALPHA: f32 = 0.3;

/// Minimum Poincare distance improvement to change label (hysteresis).
const LABEL_HYSTERESIS: f32 = 0.2;

/// Room-level reference radius.
const ROOM_RADIUS: f32 = 0.3;

/// Zone-level reference radius.
const ZONE_RADIUS: f32 = 0.7;

/// Small epsilon to avoid division by zero in Poincare distance.
const EPSILON: f32 = 1e-7;

// ── Event IDs (685-series: Exotic) ───────────────────────────────────────────

pub const EVENT_HIERARCHY_LEVEL: i32 = 685;
pub const EVENT_HYPERBOLIC_RADIUS: i32 = 686;
pub const EVENT_LOCATION_LABEL: i32 = 687;

// ── Poincare Ball Embedder ───────────────────────────────────────────────────

/// Hierarchical location classifier using Poincare ball embeddings.
///
/// Pre-configured with 16 reference points (4 rooms, 12 zones) and a
/// linear projection from 8D CSI features to 2D Poincare disk.
pub struct HyperbolicEmbedder {
    /// Reference embeddings on the Poincare disk [N_REFS][DIM].
    references: [[f32; DIM]; N_REFS],
    /// Linear projection matrix W: [DIM][FEAT_DIM] (2x8).
    projection_w: [[f32; FEAT_DIM]; DIM],
    /// Previous best label (for hysteresis).
    prev_label: u8,
    /// Previous best distance (for hysteresis).
    prev_dist: f32,
    /// EMA-smoothed embedding coordinates.
    smooth_pos: [f32; DIM],
    /// Position EMA.
    pos_ema_x: Ema,
    /// Position EMA.
    pos_ema_y: Ema,
    /// Whether the system has been initialized.
    initialized: bool,
    /// Frame counter.
    frame_count: u32,
}

impl HyperbolicEmbedder {
    pub const fn new() -> Self {
        Self {
            references: Self::default_references(),
            projection_w: Self::default_projection(),
            prev_label: 0,
            prev_dist: f32::MAX,
            smooth_pos: [0.0; DIM],
            pos_ema_x: Ema::new(POS_ALPHA),
            pos_ema_y: Ema::new(POS_ALPHA),
            initialized: false,
            frame_count: 0,
        }
    }

    /// Default reference layout: 4 rooms at radius 0.3, 12 zones at radius 0.7.
    const fn default_references() -> [[f32; DIM]; N_REFS] {
        let r = ROOM_RADIUS;
        let z = ZONE_RADIUS;
        [
            // Rooms (indices 0-3, radius 0.3)
            [r * 1.0,     r * 0.0],      // Room 0: bathroom
            [r * 0.0,     r * 1.0],      // Room 1: kitchen
            [r * -1.0,    r * 0.0],      // Room 2: living room
            [r * 0.0,     r * -1.0],     // Room 3: bedroom
            // Room 0 zones (indices 4-6, radius 0.7)
            [z * 0.9553,  z * -0.2955],  // Zone 0a
            [z * 1.0,     z * 0.0],      // Zone 0b
            [z * 0.9553,  z * 0.2955],   // Zone 0c
            // Room 1 zones (indices 7-9)
            [z * 0.2955,  z * 0.9553],   // Zone 1a
            [z * 0.0,     z * 1.0],      // Zone 1b
            [z * -0.2955, z * 0.9553],   // Zone 1c
            // Room 2 zones (indices 10-12)
            [z * -0.9553, z * 0.2955],   // Zone 2a
            [z * -1.0,    z * 0.0],      // Zone 2b
            [z * -0.9553, z * -0.2955],  // Zone 2c
            // Room 3 zones (indices 13-15)
            [z * -0.2955, z * -0.9553],  // Zone 3a
            [z * 0.0,     z * -1.0],     // Zone 3b
            [z * 0.2955,  z * -0.9553],  // Zone 3c
        ]
    }

    /// Default projection matrix mapping 8D features to 2D Poincare disk.
    const fn default_projection() -> [[f32; FEAT_DIM]; DIM] {
        [
            [0.04, 0.03, 0.02, 0.01, -0.01, -0.02, -0.03, -0.04],
            [-0.02, -0.01, 0.01, 0.02, 0.04, 0.03, 0.01, -0.01],
        ]
    }

    /// Process one CSI frame.
    ///
    /// `amplitudes` -- per-subcarrier amplitude values (up to 32).
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(&mut self, amplitudes: &[f32]) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n_ev = 0usize;

        if amplitudes.len() < FEAT_DIM {
            return &[];
        }

        self.frame_count += 1;

        // Step 1: Extract 8D feature vector (mean amplitude per group).
        let mut features = [0.0f32; FEAT_DIM];
        let n_sc = if amplitudes.len() > MAX_SC { MAX_SC } else { amplitudes.len() };
        let subs_per = n_sc / FEAT_DIM;
        if subs_per == 0 {
            return &[];
        }

        for g in 0..FEAT_DIM {
            let start = g * subs_per;
            let end = if g == FEAT_DIM - 1 { n_sc } else { start + subs_per };
            let mut sum = 0.0f32;
            for i in start..end {
                sum += amplitudes[i];
            }
            features[g] = sum / (end - start) as f32;
        }

        // Step 2: Project to 2D Poincare disk.
        let mut point = [0.0f32; DIM];
        for d in 0..DIM {
            let mut val = 0.0f32;
            for f in 0..FEAT_DIM {
                val += self.projection_w[d][f] * features[f];
            }
            point[d] = val;
        }

        // Step 3: Normalize to Poincare disk (||p|| < 1).
        let norm = sqrtf(point[0] * point[0] + point[1] * point[1]);
        if norm >= 1.0 {
            let scale = MAX_NORM / norm;
            point[0] *= scale;
            point[1] *= scale;
        }

        // EMA smooth the position.
        self.smooth_pos[0] = self.pos_ema_x.update(point[0]);
        self.smooth_pos[1] = self.pos_ema_y.update(point[1]);

        // Step 4: Find nearest reference by Poincare distance.
        let mut best_label: u8 = self.prev_label;
        let mut best_dist = f32::MAX;

        for r in 0..N_REFS {
            let d = poincare_distance(&self.smooth_pos, &self.references[r]);
            if d < best_dist {
                best_dist = d;
                best_label = r as u8;
            }
        }

        // Apply hysteresis: only switch if the new label is significantly closer.
        if best_label != self.prev_label {
            let prev_d = poincare_distance(
                &self.smooth_pos,
                &self.references[self.prev_label as usize],
            );
            if prev_d - best_dist < LABEL_HYSTERESIS {
                best_label = self.prev_label;
                best_dist = prev_d;
            }
        }

        self.prev_label = best_label;
        self.prev_dist = best_dist;

        // Step 5: Determine hierarchy level from embedding radius.
        let radius = sqrtf(
            self.smooth_pos[0] * self.smooth_pos[0]
                + self.smooth_pos[1] * self.smooth_pos[1],
        );
        let level: u8 = if radius < LEVEL_RADIUS_THRESHOLD { 0 } else { 1 };

        // Emit events.
        unsafe {
            EVENTS[n_ev] = (EVENT_HIERARCHY_LEVEL, level as f32);
        }
        n_ev += 1;

        unsafe {
            EVENTS[n_ev] = (EVENT_HYPERBOLIC_RADIUS, radius);
        }
        n_ev += 1;

        unsafe {
            EVENTS[n_ev] = (EVENT_LOCATION_LABEL, best_label as f32);
        }
        n_ev += 1;

        unsafe { &EVENTS[..n_ev] }
    }

    /// Set a reference embedding.  `index` must be < N_REFS.
    pub fn set_reference(&mut self, index: usize, coords: [f32; DIM]) {
        if index < N_REFS {
            self.references[index] = coords;
        }
    }

    /// Set the projection matrix row.  `dim` must be 0 or 1.
    pub fn set_projection_row(&mut self, dim: usize, weights: [f32; FEAT_DIM]) {
        if dim < DIM {
            self.projection_w[dim] = weights;
        }
    }

    /// Get the current smoothed position on the Poincare disk.
    pub fn position(&self) -> &[f32; DIM] {
        &self.smooth_pos
    }

    /// Get the current best label (0-15).
    pub fn label(&self) -> u8 {
        self.prev_label
    }

    /// Get total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Compute Poincare disk distance between two 2D points.
///
/// d(x, y) = acosh(1 + 2 * ||x - y||^2 / ((1 - ||x||^2) * (1 - ||y||^2)))
fn poincare_distance(x: &[f32; DIM], y: &[f32; DIM]) -> f32 {
    let mut diff_sq = 0.0f32;
    let mut x_sq = 0.0f32;
    let mut y_sq = 0.0f32;

    for d in 0..DIM {
        let dx = x[d] - y[d];
        diff_sq += dx * dx;
        x_sq += x[d] * x[d];
        y_sq += y[d] * y[d];
    }

    let denom = (1.0 - x_sq) * (1.0 - y_sq);
    if denom < EPSILON {
        return f32::MAX;
    }

    let arg = 1.0 + 2.0 * diff_sq / denom;
    if arg < 1.0 {
        return 0.0;
    }
    acoshf(arg)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use libm::fabsf;

    #[test]
    fn test_const_new() {
        let he = HyperbolicEmbedder::new();
        assert_eq!(he.frame_count(), 0);
        assert_eq!(he.label(), 0);
    }

    #[test]
    fn test_poincare_distance_identity() {
        let a = [0.1, 0.2];
        let d = poincare_distance(&a, &a);
        assert!(d < 1e-5, "distance to self should be ~0, got {}", d);
    }

    #[test]
    fn test_poincare_distance_symmetry() {
        let a = [0.1, 0.2];
        let b = [0.3, -0.1];
        let d_ab = poincare_distance(&a, &b);
        let d_ba = poincare_distance(&b, &a);
        assert!(fabsf(d_ab - d_ba) < 1e-5,
            "Poincare distance should be symmetric: {} vs {}", d_ab, d_ba);
    }

    #[test]
    fn test_poincare_distance_increases_with_separation() {
        let origin = [0.0, 0.0];
        let near = [0.1, 0.0];
        let far = [0.5, 0.0];
        let d_near = poincare_distance(&origin, &near);
        let d_far = poincare_distance(&origin, &far);
        assert!(d_far > d_near,
            "farther point should have larger distance: {} vs {}", d_far, d_near);
    }

    #[test]
    fn test_poincare_distance_boundary_diverges() {
        let origin = [0.0, 0.0];
        let near_boundary = [0.99, 0.0];
        let d = poincare_distance(&origin, &near_boundary);
        assert!(d > 3.0, "boundary distance should be large, got {}", d);
    }

    #[test]
    fn test_insufficient_amplitudes_no_events() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [1.0f32; 4]; // Only 4, need at least FEAT_DIM=8.
        let events = he.process_frame(&amps);
        assert!(events.is_empty());
    }

    #[test]
    fn test_process_frame_emits_three_events() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [10.0f32; 32];
        let events = he.process_frame(&amps);
        assert_eq!(events.len(), 3, "should emit hierarchy, radius, label events");
    }

    #[test]
    fn test_event_ids_correct() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [10.0f32; 32];
        let events = he.process_frame(&amps);
        assert_eq!(events[0].0, EVENT_HIERARCHY_LEVEL);
        assert_eq!(events[1].0, EVENT_HYPERBOLIC_RADIUS);
        assert_eq!(events[2].0, EVENT_LOCATION_LABEL);
    }

    #[test]
    fn test_label_in_range() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [10.0f32; 32];
        for _ in 0..20 {
            let events = he.process_frame(&amps);
            if events.len() == 3 {
                let label = events[2].1 as u8;
                assert!(label < N_REFS as u8,
                    "label {} should be < {}", label, N_REFS);
            }
        }
    }

    #[test]
    fn test_radius_in_poincare_disk() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [10.0f32; 32];
        for _ in 0..20 {
            let events = he.process_frame(&amps);
            if events.len() == 3 {
                let radius = events[1].1;
                assert!(radius >= 0.0 && radius < 1.0,
                    "radius {} should be in [0, 1)", radius);
            }
        }
    }

    #[test]
    fn test_default_references_inside_disk() {
        let refs = HyperbolicEmbedder::default_references();
        for (i, r) in refs.iter().enumerate() {
            let norm = sqrtf(r[0] * r[0] + r[1] * r[1]);
            assert!(norm < 1.0,
                "reference {} at norm {} should be inside unit disk", i, norm);
        }
    }

    #[test]
    fn test_normalization_clamps_to_disk() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [1000.0f32; 32];
        let events = he.process_frame(&amps);
        if events.len() == 3 {
            let radius = events[1].1;
            assert!(radius < 1.0, "radius {} should be < 1.0 after normalization", radius);
        }
    }

    #[test]
    fn test_reset() {
        let mut he = HyperbolicEmbedder::new();
        let amps = [10.0f32; 32];
        he.process_frame(&amps);
        he.process_frame(&amps);
        assert!(he.frame_count() > 0);
        he.reset();
        assert_eq!(he.frame_count(), 0);
    }
}
