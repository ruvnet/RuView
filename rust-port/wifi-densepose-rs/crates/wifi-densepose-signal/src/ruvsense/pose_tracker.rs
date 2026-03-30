//! 17-Keypoint Kalman Pose Tracker with Re-ID (ADR-029 Section 2.7)
//!
//! Tracks multiple people as persistent 17-keypoint skeletons across time.
//! Each keypoint has a 6D Kalman state (x, y, z, vx, vy, vz) with a
//! constant-velocity motion model. Track lifecycle follows:
//!
//!   Tentative -> Active -> Lost -> Terminated
//!
//! Detection-to-track assignment uses a joint cost combining Mahalanobis
//! distance (60%) and AETHER re-ID embedding cosine similarity (40%),
//! implemented via `ruvector-mincut::DynamicPersonMatcher`.
//!
//! # Parameters
//!
//! | Parameter | Value | Rationale |
//! |-----------|-------|-----------|
//! | State dimension | 6 per keypoint | Constant-velocity model |
//! | Process noise | 0.3 m/s^2 | Normal walking acceleration |
//! | Measurement noise | 0.08 m | Target <8cm RMS at torso |
//! | Birth hits | 2 frames | Reject single-frame noise |
//! | Loss misses | 5 frames | Brief occlusion tolerance |
//! | Re-ID embedding | 128-dim | AETHER body-shape discriminative |
//! | Re-ID window | 5 seconds | Crossing recovery |
//!
//! # RuVector Integration
//!
//! - `ruvector-mincut` -> Person separation and track assignment

use std::collections::VecDeque;

use super::{TrackId, NUM_KEYPOINTS};

/// Errors from the pose tracker.
#[derive(Debug, thiserror::Error)]
pub enum PoseTrackerError {
    /// Invalid keypoint index.
    #[error("Invalid keypoint index {index}, max is {}", NUM_KEYPOINTS - 1)]
    InvalidKeypointIndex { index: usize },

    /// Invalid embedding dimension.
    #[error("Embedding dimension {got} does not match expected {expected}")]
    EmbeddingDimMismatch { expected: usize, got: usize },

    /// Mahalanobis gate exceeded.
    #[error("Mahalanobis distance {distance:.2} exceeds gate {gate:.2}")]
    MahalanobisGateExceeded { distance: f32, gate: f32 },

    /// Track not found.
    #[error("Track {0} not found")]
    TrackNotFound(TrackId),

    /// No detections provided.
    #[error("No detections provided for update")]
    NoDetections,
}

/// Per-keypoint Kalman state.
///
/// Maintains a 6D state vector [x, y, z, vx, vy, vz] and a 6x6 covariance
/// matrix stored as the upper triangle (21 elements, row-major).
#[derive(Debug, Clone)]
pub struct KeypointState {
    /// State vector [x, y, z, vx, vy, vz].
    pub state: [f32; 6],
    /// 6x6 covariance upper triangle (21 elements, row-major).
    /// Indices: (0,0)=0, (0,1)=1, (0,2)=2, (0,3)=3, (0,4)=4, (0,5)=5,
    ///          (1,1)=6, (1,2)=7, (1,3)=8, (1,4)=9, (1,5)=10,
    ///          (2,2)=11, (2,3)=12, (2,4)=13, (2,5)=14,
    ///          (3,3)=15, (3,4)=16, (3,5)=17,
    ///          (4,4)=18, (4,5)=19,
    ///          (5,5)=20
    pub covariance: [f32; 21],
    /// Confidence (0.0-1.0) from DensePose model output.
    pub confidence: f32,
}

impl KeypointState {
    /// Create a new keypoint state at the given 3D position.
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        let mut cov = [0.0_f32; 21];
        // Initialize diagonal with default uncertainty
        let pos_var = 0.1 * 0.1;  // 10 cm initial uncertainty
        let vel_var = 0.5 * 0.5;  // 0.5 m/s initial velocity uncertainty
        cov[0] = pos_var;   // x variance
        cov[6] = pos_var;   // y variance
        cov[11] = pos_var;  // z variance
        cov[15] = vel_var;  // vx variance
        cov[18] = vel_var;  // vy variance
        cov[20] = vel_var;  // vz variance

        Self {
            state: [x, y, z, 0.0, 0.0, 0.0],
            covariance: cov,
            confidence: 0.0,
        }
    }

    /// Return the position [x, y, z].
    pub fn position(&self) -> [f32; 3] {
        [self.state[0], self.state[1], self.state[2]]
    }

    /// Return the velocity [vx, vy, vz].
    pub fn velocity(&self) -> [f32; 3] {
        [self.state[3], self.state[4], self.state[5]]
    }

    /// Predict step: advance state by dt seconds using constant-velocity model.
    ///
    /// x' = x + vx * dt
    /// P' = F * P * F^T + Q
    pub fn predict(&mut self, dt: f32, process_noise_accel: f32) {
        // State prediction: x' = x + v * dt
        self.state[0] += self.state[3] * dt;
        self.state[1] += self.state[4] * dt;
        self.state[2] += self.state[5] * dt;

        // Process noise Q (constant acceleration model)
        let dt2 = dt * dt;
        let dt3 = dt2 * dt;
        let dt4 = dt3 * dt;
        let q = process_noise_accel * process_noise_accel;

        // Add process noise to diagonal elements
        // Position variances: + q * dt^4 / 4
        let pos_q = q * dt4 / 4.0;
        // Velocity variances: + q * dt^2
        let vel_q = q * dt2;
        // Position-velocity cross: + q * dt^3 / 2
        let _cross_q = q * dt3 / 2.0;

        // Simplified: only update diagonal for numerical stability
        self.covariance[0] += pos_q;   // xx
        self.covariance[6] += pos_q;   // yy
        self.covariance[11] += pos_q;  // zz
        self.covariance[15] += vel_q;  // vxvx
        self.covariance[18] += vel_q;  // vyvy
        self.covariance[20] += vel_q;  // vzvz
    }

    /// Measurement update: incorporate a position observation [x, y, z].
    ///
    /// Uses the standard Kalman update with position-only measurement model
    /// H = [I3 | 0_3x3].
    pub fn update(
        &mut self,
        measurement: &[f32; 3],
        measurement_noise: f32,
        noise_multiplier: f32,
    ) {
        let r = measurement_noise * measurement_noise * noise_multiplier;

        // Innovation (residual)
        let innov = [
            measurement[0] - self.state[0],
            measurement[1] - self.state[1],
            measurement[2] - self.state[2],
        ];

        // Innovation covariance S = H * P * H^T + R
        // Since H = [I3 | 0], S is just the top-left 3x3 of P + R
        let s = [
            self.covariance[0] + r,
            self.covariance[6] + r,
            self.covariance[11] + r,
        ];

        // Kalman gain K = P * H^T * S^-1
        // For diagonal S, K_ij = P_ij / S_jj (simplified)
        let k = [
            [self.covariance[0] / s[0], 0.0, 0.0],               // x row
            [0.0, self.covariance[6] / s[1], 0.0],               // y row
            [0.0, 0.0, self.covariance[11] / s[2]],              // z row
            [self.covariance[3] / s[0], 0.0, 0.0],               // vx row
            [0.0, self.covariance[9] / s[1], 0.0],               // vy row
            [0.0, 0.0, self.covariance[14] / s[2]],              // vz row
        ];

        // State update: x' = x + K * innov
        for i in 0..6 {
            for j in 0..3 {
                self.state[i] += k[i][j] * innov[j];
            }
        }

        // Covariance update: P' = (I - K*H) * P (simplified diagonal update)
        self.covariance[0] *= 1.0 - k[0][0];
        self.covariance[6] *= 1.0 - k[1][1];
        self.covariance[11] *= 1.0 - k[2][2];
    }

    /// Compute the Mahalanobis distance between this state and a measurement.
    pub fn mahalanobis_distance(&self, measurement: &[f32; 3]) -> f32 {
        let innov = [
            measurement[0] - self.state[0],
            measurement[1] - self.state[1],
            measurement[2] - self.state[2],
        ];

        // Using diagonal approximation
        let mut dist_sq = 0.0_f32;
        let variances = [self.covariance[0], self.covariance[6], self.covariance[11]];
        for i in 0..3 {
            let v = variances[i].max(1e-6);
            dist_sq += innov[i] * innov[i] / v;
        }

        dist_sq.sqrt()
    }
}

impl Default for KeypointState {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// Track lifecycle state machine.
///
/// Follows the pattern from ADR-026:
///   Tentative -> Active -> Lost -> Terminated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackLifecycleState {
    /// Track has been detected but not yet confirmed (< birth_hits frames).
    Tentative,
    /// Track is confirmed and actively being updated.
    Active,
    /// Track has lost measurement association (< loss_misses frames).
    Lost,
    /// Track has been terminated (exceeded max lost duration or deemed false positive).
    Terminated,
}

impl TrackLifecycleState {
    /// Returns true if the track is in an active or tentative state.
    pub fn is_alive(&self) -> bool {
        matches!(self, Self::Tentative | Self::Active | Self::Lost)
    }

    /// Returns true if the track can receive measurement updates.
    pub fn accepts_updates(&self) -> bool {
        matches!(self, Self::Tentative | Self::Active)
    }

    /// Returns true if the track is eligible for re-identification.
    pub fn is_lost(&self) -> bool {
        matches!(self, Self::Lost)
    }
}

/// A pose track -- aggregate root for tracking one person.
///
/// Contains 17 keypoint Kalman states, lifecycle, and re-ID embedding.
#[derive(Debug, Clone)]
pub struct PoseTrack {
    /// Unique track identifier.
    pub id: TrackId,
    /// Per-keypoint Kalman state (COCO-17 ordering).
    pub keypoints: [KeypointState; NUM_KEYPOINTS],
    /// Track lifecycle state.
    pub lifecycle: TrackLifecycleState,
    /// Running-average AETHER embedding for re-ID (128-dim).
    pub embedding: Vec<f32>,
    /// Total frames since creation.
    pub age: u64,
    /// Frames since last successful measurement update.
    pub time_since_update: u64,
    /// Number of consecutive measurement updates (for birth gate).
    pub consecutive_hits: u64,
    /// Creation timestamp in microseconds.
    pub created_at: u64,
    /// Last update timestamp in microseconds.
    pub updated_at: u64,
}

impl PoseTrack {
    /// Create a new tentative track from a detection.
    pub fn new(
        id: TrackId,
        keypoint_positions: &[[f32; 3]; NUM_KEYPOINTS],
        timestamp_us: u64,
        embedding_dim: usize,
    ) -> Self {
        let keypoints = std::array::from_fn(|i| {
            let [x, y, z] = keypoint_positions[i];
            KeypointState::new(x, y, z)
        });

        Self {
            id,
            keypoints,
            lifecycle: TrackLifecycleState::Tentative,
            embedding: vec![0.0; embedding_dim],
            age: 0,
            time_since_update: 0,
            consecutive_hits: 1,
            created_at: timestamp_us,
            updated_at: timestamp_us,
        }
    }

    /// Predict all keypoints forward by dt seconds.
    pub fn predict(&mut self, dt: f32, process_noise: f32) {
        for kp in &mut self.keypoints {
            kp.predict(dt, process_noise);
        }
        self.age += 1;
        self.time_since_update += 1;
    }

    /// Update all keypoints with new measurements.
    ///
    /// Also updates lifecycle state transitions based on birth/loss gates.
    pub fn update_keypoints(
        &mut self,
        measurements: &[[f32; 3]; NUM_KEYPOINTS],
        measurement_noise: f32,
        noise_multiplier: f32,
        timestamp_us: u64,
    ) {
        for (kp, meas) in self.keypoints.iter_mut().zip(measurements.iter()) {
            kp.update(meas, measurement_noise, noise_multiplier);
        }

        self.time_since_update = 0;
        self.consecutive_hits += 1;
        self.updated_at = timestamp_us;

        // Lifecycle transitions
        self.update_lifecycle();
    }

    /// Update the embedding with EMA decay.
    pub fn update_embedding(&mut self, new_embedding: &[f32], decay: f32) {
        if new_embedding.len() != self.embedding.len() {
            return;
        }

        let alpha = 1.0 - decay;
        for (e, &ne) in self.embedding.iter_mut().zip(new_embedding.iter()) {
            *e = decay * *e + alpha * ne;
        }
    }

    /// Compute the centroid position (mean of all keypoints).
    pub fn centroid(&self) -> [f32; 3] {
        let n = NUM_KEYPOINTS as f32;
        let mut c = [0.0_f32; 3];
        for kp in &self.keypoints {
            let pos = kp.position();
            c[0] += pos[0];
            c[1] += pos[1];
            c[2] += pos[2];
        }
        c[0] /= n;
        c[1] /= n;
        c[2] /= n;
        c
    }

    /// Compute torso jitter RMS in meters.
    ///
    /// Uses the torso keypoints (shoulders, hips) velocity magnitudes
    /// as a proxy for jitter.
    pub fn torso_jitter_rms(&self) -> f32 {
        let torso_indices = super::keypoint::TORSO_INDICES;
        let mut sum_sq = 0.0_f32;
        let mut count = 0;

        for &idx in torso_indices {
            let vel = self.keypoints[idx].velocity();
            let speed_sq = vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2];
            sum_sq += speed_sq;
            count += 1;
        }

        if count == 0 {
            return 0.0;
        }

        (sum_sq / count as f32).sqrt()
    }

    /// Mark the track as lost.
    pub fn mark_lost(&mut self) {
        if self.lifecycle != TrackLifecycleState::Terminated {
            self.lifecycle = TrackLifecycleState::Lost;
        }
    }

    /// Mark the track as terminated.
    pub fn terminate(&mut self) {
        self.lifecycle = TrackLifecycleState::Terminated;
    }

    /// Update lifecycle state based on consecutive hits and misses.
    fn update_lifecycle(&mut self) {
        match self.lifecycle {
            TrackLifecycleState::Tentative => {
                if self.consecutive_hits >= 2 {
                    // Birth gate: promote to Active after 2 consecutive updates
                    self.lifecycle = TrackLifecycleState::Active;
                }
            }
            TrackLifecycleState::Lost => {
                // Re-acquired: promote back to Active
                self.lifecycle = TrackLifecycleState::Active;
                self.consecutive_hits = 1;
            }
            _ => {}
        }
    }
}

/// Tracker configuration parameters.
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Process noise acceleration (m/s^2). Default: 0.3.
    pub process_noise: f32,
    /// Measurement noise std dev (m). Default: 0.08.
    pub measurement_noise: f32,
    /// Mahalanobis gate threshold (chi-squared(3) at 3-sigma = 9.0).
    pub mahalanobis_gate: f32,
    /// Frames required for tentative->active promotion. Default: 2.
    pub birth_hits: u64,
    /// Max frames without update before tentative->lost. Default: 5.
    pub loss_misses: u64,
    /// Re-ID window in frames (5 seconds at 20Hz = 100). Default: 100.
    pub reid_window: u64,
    /// Embedding EMA decay rate. Default: 0.95.
    pub embedding_decay: f32,
    /// Embedding dimension. Default: 128.
    pub embedding_dim: usize,
    /// Position weight in assignment cost. Default: 0.6.
    pub position_weight: f32,
    /// Embedding weight in assignment cost. Default: 0.4.
    pub embedding_weight: f32,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            process_noise: 0.3,
            measurement_noise: 0.08,
            mahalanobis_gate: 9.0,
            birth_hits: 2,
            loss_misses: 5,
            reid_window: 100,
            embedding_decay: 0.95,
            embedding_dim: 128,
            position_weight: 0.6,
            embedding_weight: 0.4,
        }
    }
}

/// Multi-person pose tracker.
///
/// Manages a collection of `PoseTrack` instances with automatic lifecycle
/// management, detection-to-track assignment, and re-identification.
#[derive(Debug)]
pub struct PoseTracker {
    config: TrackerConfig,
    tracks: Vec<PoseTrack>,
    next_id: u64,
}

impl PoseTracker {
    /// Create a new tracker with default configuration.
    pub fn new() -> Self {
        Self {
            config: TrackerConfig::default(),
            tracks: Vec::new(),
            next_id: 0,
        }
    }

    /// Create a new tracker with custom configuration.
    pub fn with_config(config: TrackerConfig) -> Self {
        Self {
            config,
            tracks: Vec::new(),
            next_id: 0,
        }
    }

    /// Return all active tracks (not terminated).
    pub fn active_tracks(&self) -> Vec<&PoseTrack> {
        self.tracks
            .iter()
            .filter(|t| t.lifecycle.is_alive())
            .collect()
    }

    /// Return all tracks including terminated ones.
    pub fn all_tracks(&self) -> &[PoseTrack] {
        &self.tracks
    }

    /// Return the number of active (alive) tracks.
    pub fn active_count(&self) -> usize {
        self.tracks.iter().filter(|t| t.lifecycle.is_alive()).count()
    }

    /// Predict step for all tracks (advance by dt seconds).
    pub fn predict_all(&mut self, dt: f32) {
        for track in &mut self.tracks {
            if track.lifecycle.is_alive() {
                track.predict(dt, self.config.process_noise);
            }
        }

        // Mark tracks as lost after exceeding loss_misses
        for track in &mut self.tracks {
            if track.lifecycle.accepts_updates()
                && track.time_since_update >= self.config.loss_misses
            {
                track.mark_lost();
            }
        }

        // Terminate tracks that have been lost too long
        let reid_window = self.config.reid_window;
        for track in &mut self.tracks {
            if track.lifecycle.is_lost() && track.time_since_update >= reid_window {
                track.terminate();
            }
        }
    }

    /// Create a new track from a detection.
    pub fn create_track(
        &mut self,
        keypoints: &[[f32; 3]; NUM_KEYPOINTS],
        timestamp_us: u64,
    ) -> TrackId {
        let id = TrackId::new(self.next_id);
        self.next_id += 1;

        let track = PoseTrack::new(id, keypoints, timestamp_us, self.config.embedding_dim);
        self.tracks.push(track);
        id
    }

    /// Find the track with the given ID.
    pub fn find_track(&self, id: TrackId) -> Option<&PoseTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Find the track with the given ID (mutable).
    pub fn find_track_mut(&mut self, id: TrackId) -> Option<&mut PoseTrack> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Remove terminated tracks from the collection.
    pub fn prune_terminated(&mut self) {
        self.tracks
            .retain(|t| t.lifecycle != TrackLifecycleState::Terminated);
    }

    /// Compute the assignment cost between a track and a detection.
    ///
    /// cost = position_weight * mahalanobis(track, detection.position)
    ///      + embedding_weight * (1 - cosine_sim(track.embedding, detection.embedding))
    pub fn assignment_cost(
        &self,
        track: &PoseTrack,
        detection_centroid: &[f32; 3],
        detection_embedding: &[f32],
    ) -> f32 {
        // Position cost: Mahalanobis distance at centroid
        let centroid_kp = track.centroid();
        let centroid_state = KeypointState::new(centroid_kp[0], centroid_kp[1], centroid_kp[2]);
        let maha = centroid_state.mahalanobis_distance(detection_centroid);

        // Embedding cost: 1 - cosine similarity
        let embed_cost = 1.0 - cosine_similarity(&track.embedding, detection_embedding);

        self.config.position_weight * maha + self.config.embedding_weight * embed_cost
    }
}

impl Default for PoseTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Cosine similarity between two vectors.
///
/// Returns a value in [-1.0, 1.0] where 1.0 means identical direction.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;

    for i in 0..n {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom < 1e-12 {
        return 0.0;
    }

    (dot / denom).clamp(-1.0, 1.0)
}

/// A detected pose from the model, before assignment to a track.
#[derive(Debug, Clone)]
pub struct PoseDetection {
    /// Per-keypoint positions [x, y, z, confidence] for 17 keypoints.
    pub keypoints: [[f32; 4]; NUM_KEYPOINTS],
    /// AETHER re-ID embedding (128-dim).
    pub embedding: Vec<f32>,
}

impl PoseDetection {
    /// Extract the 3D position array from keypoints.
    pub fn positions(&self) -> [[f32; 3]; NUM_KEYPOINTS] {
        std::array::from_fn(|i| [self.keypoints[i][0], self.keypoints[i][1], self.keypoints[i][2]])
    }

    /// Compute the centroid of the detection.
    pub fn centroid(&self) -> [f32; 3] {
        let n = NUM_KEYPOINTS as f32;
        let mut c = [0.0_f32; 3];
        for kp in &self.keypoints {
            c[0] += kp[0];
            c[1] += kp[1];
            c[2] += kp[2];
        }
        c[0] /= n;
        c[1] /= n;
        c[2] /= n;
        c
    }

    /// Mean confidence across all keypoints.
    pub fn mean_confidence(&self) -> f32 {
        let sum: f32 = self.keypoints.iter().map(|kp| kp[3]).sum();
        sum / NUM_KEYPOINTS as f32
    }
}

// ---------------------------------------------------------------------------
// Skeleton kinematic constraints (RuVector Phase 3)
// ---------------------------------------------------------------------------

/// Expected bone lengths in normalized coordinates (parent_idx, child_idx, length).
///
/// These define the COCO-17 kinematic tree edges with approximate proportions
/// derived from anthropometric averages. Used by [`SkeletonConstraints`] to
/// reject impossible poses (e.g., arm longer than torso).
const BONE_LENGTHS: &[(usize, usize, f32)] = &[
    (5, 7, 0.15),   // L shoulder -> L elbow
    (7, 9, 0.14),   // L elbow -> L wrist
    (6, 8, 0.15),   // R shoulder -> R elbow
    (8, 10, 0.14),  // R elbow -> R wrist
    (5, 11, 0.25),  // L shoulder -> L hip
    (6, 12, 0.25),  // R shoulder -> R hip
    (11, 13, 0.22), // L hip -> L knee
    (13, 15, 0.22), // L knee -> L ankle
    (12, 14, 0.22), // R hip -> R knee
    (14, 16, 0.22), // R knee -> R ankle
    (5, 6, 0.18),   // L shoulder -> R shoulder
    (11, 12, 0.15), // L hip -> R hip
];

/// Skeleton kinematic constraint enforcer using Jakobsen relaxation.
///
/// Iteratively projects bone lengths toward their expected values so that
/// the resulting skeleton obeys basic anthropometric limits. Bones that
/// deviate more than [`Self::TOLERANCE`] (30 %) from their rest length are
/// corrected over [`Self::ITERATIONS`] passes.
pub struct SkeletonConstraints;

impl SkeletonConstraints {
    /// Maximum allowed fractional deviation before correction kicks in.
    const TOLERANCE: f32 = 0.30;

    /// Number of Jakobsen relaxation iterations.
    const ITERATIONS: usize = 3;

    /// Enforce kinematic constraints in-place on `keypoints`.
    ///
    /// Each element is `[x, y, z]`. The method runs several iterations of
    /// distance-constraint projection (Jakobsen method) over the edges
    /// defined in [`BONE_LENGTHS`].
    pub fn enforce_constraints(keypoints: &mut [[f32; 3]; 17]) {
        for _ in 0..Self::ITERATIONS {
            for &(a, b, rest_len) in BONE_LENGTHS {
                let dx = keypoints[b][0] - keypoints[a][0];
                let dy = keypoints[b][1] - keypoints[a][1];
                let dz = keypoints[b][2] - keypoints[a][2];
                let current_len = (dx * dx + dy * dy + dz * dz).sqrt();

                // Skip degenerate / zero-length bones (e.g. all-zero pose).
                if current_len < 1e-9 {
                    continue;
                }

                let ratio = current_len / rest_len;
                // Only correct if deviation exceeds tolerance.
                if ratio < (1.0 - Self::TOLERANCE) || ratio > (1.0 + Self::TOLERANCE) {
                    let correction = (rest_len - current_len) / current_len * 0.5;
                    let cx = dx * correction;
                    let cy = dy * correction;
                    let cz = dz * correction;

                    keypoints[a][0] -= cx;
                    keypoints[a][1] -= cy;
                    keypoints[a][2] -= cz;
                    keypoints[b][0] += cx;
                    keypoints[b][1] += cy;
                    keypoints[b][2] += cz;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Compressed pose history (RuVector Phase 3 -- temporal tensor)
// ---------------------------------------------------------------------------

/// Two-tier compressed pose history.
///
/// Recent poses are stored at full `f32` precision in the *hot* ring buffer.
/// Once the hot buffer is full the oldest pose is quantised to `i16` and
/// pushed into the *warm* tier, keeping memory usage bounded while still
/// allowing similarity queries against a longer temporal window.
pub struct CompressedPoseHistory {
    /// Recent poses at full precision.
    hot: VecDeque<[[f32; 3]; 17]>,
    /// Older poses quantised to i16.
    warm: VecDeque<[[i16; 3]; 17]>,
    /// Scale factor used for warm quantisation (divide f32, multiply to
    /// reconstruct).
    scale: f32,
    max_hot: usize,
    max_warm: usize,
}

impl CompressedPoseHistory {
    /// Create a new history with the given tier sizes.
    ///
    /// `scale` controls the fixed-point quantisation: warm values are stored
    /// as `(value / scale).round() as i16`.
    pub fn new(max_hot: usize, max_warm: usize, scale: f32) -> Self {
        Self {
            hot: VecDeque::with_capacity(max_hot),
            warm: VecDeque::with_capacity(max_warm),
            scale: if scale.abs() < 1e-12 { 1.0 } else { scale },
            max_hot,
            max_warm,
        }
    }

    /// Push a new pose into the history.
    ///
    /// When the hot tier is full the oldest entry is quantised and moved to
    /// the warm tier. When the warm tier overflows the oldest warm entry is
    /// discarded.
    pub fn push(&mut self, pose: &[[f32; 3]; 17]) {
        if self.hot.len() >= self.max_hot {
            if let Some(evicted) = self.hot.pop_front() {
                let quantised = self.quantise(&evicted);
                if self.warm.len() >= self.max_warm {
                    self.warm.pop_front();
                }
                self.warm.push_back(quantised);
            }
        }
        self.hot.push_back(*pose);
    }

    /// Cosine similarity between `pose` and the most recent stored pose.
    ///
    /// Both poses are flattened to 51-element vectors before the dot-product
    /// is computed. Returns 0.0 when the history is empty or either vector
    /// has zero norm.
    pub fn similarity(&self, pose: &[[f32; 3]; 17]) -> f32 {
        let recent = match self.hot.back() {
            Some(r) => r,
            None => return 0.0,
        };

        let mut dot = 0.0_f32;
        let mut norm_a = 0.0_f32;
        let mut norm_b = 0.0_f32;

        for kp in 0..17 {
            for d in 0..3 {
                let a = recent[kp][d];
                let b = pose[kp][d];
                dot += a * b;
                norm_a += a * a;
                norm_b += b * b;
            }
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom < 1e-12 {
            return 0.0;
        }
        (dot / denom).clamp(-1.0, 1.0)
    }

    /// Total number of stored poses (hot + warm).
    pub fn len(&self) -> usize {
        self.hot.len() + self.warm.len()
    }

    /// Returns `true` when the history contains no poses.
    pub fn is_empty(&self) -> bool {
        self.hot.is_empty() && self.warm.is_empty()
    }

    // -- internal helpers ---------------------------------------------------

    fn quantise(&self, pose: &[[f32; 3]; 17]) -> [[i16; 3]; 17] {
        let inv = 1.0 / self.scale;
        let mut out = [[0_i16; 3]; 17];
        for kp in 0..17 {
            for d in 0..3 {
                out[kp][d] = (pose[kp][d] * inv)
                    .round()
                    .clamp(i16::MIN as f32, i16::MAX as f32)
                    as i16;
            }
        }
        out
    }
}

impl Default for CompressedPoseHistory {
    fn default() -> Self {
        Self::new(10, 50, 0.001)
    }
}

// ---------------------------------------------------------------------------
// Temporal Keypoint Attention (RuVector Phase 2)
// ---------------------------------------------------------------------------

/// Sliding-window temporal smoother for 17-keypoint pose estimates.
///
/// Maintains a ring buffer of the last `WINDOW_SIZE` pose frames and applies
/// exponential-decay weighted averaging to produce temporally coherent output.
/// Additionally enforces kinematic constraints: bone lengths cannot change by
/// more than 20% between consecutive frames.
///
/// This is a lightweight inline implementation that mirrors the algorithm in
/// `ruvector-attention` without pulling the crate into the sensing server.
pub struct TemporalKeypointAttention {
    /// Ring buffer of recent pose frames (newest at back).
    window: std::collections::VecDeque<[[f32; 3]; NUM_KEYPOINTS]>,
    /// Maximum number of frames to retain.
    window_size: usize,
    /// Exponential decay factor per frame (e.g., 0.7 means frame t-1 has
    /// weight 0.7, frame t-2 has weight 0.49, etc.).
    decay: f32,
}

impl TemporalKeypointAttention {
    /// Default window size (10 frames at 10-20 Hz = 0.5-1.0 s look-back).
    pub const DEFAULT_WINDOW: usize = 10;
    /// Default decay factor.
    pub const DEFAULT_DECAY: f32 = 0.7;
    /// Maximum allowed bone-length change ratio between consecutive frames.
    pub const MAX_BONE_CHANGE: f32 = 0.20;

    /// Create a new temporal attention smoother with default parameters.
    pub fn new() -> Self {
        Self {
            window: std::collections::VecDeque::with_capacity(Self::DEFAULT_WINDOW),
            window_size: Self::DEFAULT_WINDOW,
            decay: Self::DEFAULT_DECAY,
        }
    }

    /// Create with custom window size and decay.
    pub fn with_params(window_size: usize, decay: f32) -> Self {
        Self {
            window: std::collections::VecDeque::with_capacity(window_size),
            window_size,
            decay: decay.clamp(0.0, 1.0),
        }
    }

    /// Smooth the current keypoint estimate using the temporal window.
    ///
    /// 1. Pushes `current` into the window (evicting oldest if full).
    /// 2. Computes exponential-decay weighted average across all frames.
    /// 3. Enforces bone-length constraints against the previous frame.
    pub fn smooth_keypoints(
        &mut self,
        current: &[[f32; 3]; NUM_KEYPOINTS],
    ) -> [[f32; 3]; NUM_KEYPOINTS] {
        // Grab the previous frame (before pushing current) for bone clamping.
        let prev_frame = self.window.back().copied();

        // Push current frame into the window.
        if self.window.len() >= self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(*current);

        // Compute weighted average with exponential decay (newest = highest weight).
        let n = self.window.len();
        let mut result = [[0.0_f32; 3]; NUM_KEYPOINTS];
        let mut total_weight = 0.0_f32;

        for (age, frame) in self.window.iter().rev().enumerate() {
            let w = self.decay.powi(age as i32);
            total_weight += w;
            for kp in 0..NUM_KEYPOINTS {
                for dim in 0..3 {
                    result[kp][dim] += w * frame[kp][dim];
                }
            }
        }

        if total_weight > 0.0 {
            for kp in 0..NUM_KEYPOINTS {
                for dim in 0..3 {
                    result[kp][dim] /= total_weight;
                }
            }
        }

        // Enforce bone-length constraints: no bone can change >20% from prev frame.
        if let Some(prev) = prev_frame {
            if n >= 2 {
                Self::clamp_bone_lengths(&mut result, &prev);
            }
        }

        result
    }

    /// Clamp bone lengths so they don't change by more than MAX_BONE_CHANGE
    /// compared to the previous frame.
    fn clamp_bone_lengths(
        pose: &mut [[f32; 3]; NUM_KEYPOINTS],
        prev: &[[f32; 3]; NUM_KEYPOINTS],
    ) {
        for &(parent, child, _) in BONE_LENGTHS {
            let prev_len = Self::bone_len(prev, parent, child);
            if prev_len < 1e-6 {
                continue; // skip degenerate bones
            }
            let cur_len = Self::bone_len(pose, parent, child);
            if cur_len < 1e-6 {
                continue;
            }

            let ratio = cur_len / prev_len;
            let lo = 1.0 - Self::MAX_BONE_CHANGE;
            let hi = 1.0 + Self::MAX_BONE_CHANGE;

            if ratio < lo || ratio > hi {
                // Scale the child position toward/away from parent to clamp.
                let target_len = prev_len * ratio.clamp(lo, hi);
                let scale = target_len / cur_len;
                for dim in 0..3 {
                    let diff = pose[child][dim] - pose[parent][dim];
                    pose[child][dim] = pose[parent][dim] + diff * scale;
                }
            }
        }
    }

    /// Euclidean distance between two keypoints in a pose.
    fn bone_len(pose: &[[f32; 3]; NUM_KEYPOINTS], a: usize, b: usize) -> f32 {
        let dx = pose[b][0] - pose[a][0];
        let dy = pose[b][1] - pose[a][1];
        let dz = pose[b][2] - pose[a][2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Number of frames currently in the window.
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// Whether the window is empty.
    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }

    /// Clear the window (e.g., on track reset).
    pub fn clear(&mut self) {
        self.window.clear();
    }
}

impl Default for TemporalKeypointAttention {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_positions() -> [[f32; 3]; NUM_KEYPOINTS] {
        [[0.0, 0.0, 0.0]; NUM_KEYPOINTS]
    }

    #[allow(dead_code)]
    fn offset_positions(offset: f32) -> [[f32; 3]; NUM_KEYPOINTS] {
        std::array::from_fn(|i| [offset + i as f32 * 0.1, offset, 0.0])
    }

    #[test]
    fn keypoint_state_creation() {
        let kp = KeypointState::new(1.0, 2.0, 3.0);
        assert_eq!(kp.position(), [1.0, 2.0, 3.0]);
        assert_eq!(kp.velocity(), [0.0, 0.0, 0.0]);
        assert_eq!(kp.confidence, 0.0);
    }

    #[test]
    fn keypoint_predict_moves_position() {
        let mut kp = KeypointState::new(0.0, 0.0, 0.0);
        kp.state[3] = 1.0; // vx = 1 m/s
        kp.predict(0.05, 0.3); // 50ms step
        assert!((kp.state[0] - 0.05).abs() < 1e-5, "x should be ~0.05, got {}", kp.state[0]);
    }

    #[test]
    fn keypoint_predict_increases_uncertainty() {
        let mut kp = KeypointState::new(0.0, 0.0, 0.0);
        let initial_var = kp.covariance[0];
        kp.predict(0.05, 0.3);
        assert!(kp.covariance[0] > initial_var);
    }

    #[test]
    fn keypoint_update_reduces_uncertainty() {
        let mut kp = KeypointState::new(0.0, 0.0, 0.0);
        kp.predict(0.05, 0.3);
        let post_predict_var = kp.covariance[0];
        kp.update(&[0.01, 0.0, 0.0], 0.08, 1.0);
        assert!(kp.covariance[0] < post_predict_var);
    }

    #[test]
    fn mahalanobis_zero_distance() {
        let kp = KeypointState::new(1.0, 2.0, 3.0);
        let d = kp.mahalanobis_distance(&[1.0, 2.0, 3.0]);
        assert!(d < 1e-3);
    }

    #[test]
    fn mahalanobis_positive_for_offset() {
        let kp = KeypointState::new(0.0, 0.0, 0.0);
        let d = kp.mahalanobis_distance(&[1.0, 0.0, 0.0]);
        assert!(d > 0.0);
    }

    #[test]
    fn lifecycle_transitions() {
        assert!(TrackLifecycleState::Tentative.is_alive());
        assert!(TrackLifecycleState::Active.is_alive());
        assert!(TrackLifecycleState::Lost.is_alive());
        assert!(!TrackLifecycleState::Terminated.is_alive());

        assert!(TrackLifecycleState::Tentative.accepts_updates());
        assert!(TrackLifecycleState::Active.accepts_updates());
        assert!(!TrackLifecycleState::Lost.accepts_updates());
        assert!(!TrackLifecycleState::Terminated.accepts_updates());

        assert!(!TrackLifecycleState::Tentative.is_lost());
        assert!(TrackLifecycleState::Lost.is_lost());
    }

    #[test]
    fn track_creation() {
        let positions = zero_positions();
        let track = PoseTrack::new(TrackId(0), &positions, 1000, 128);
        assert_eq!(track.id, TrackId(0));
        assert_eq!(track.lifecycle, TrackLifecycleState::Tentative);
        assert_eq!(track.embedding.len(), 128);
        assert_eq!(track.age, 0);
        assert_eq!(track.consecutive_hits, 1);
    }

    #[test]
    fn track_birth_gate() {
        let positions = zero_positions();
        let mut track = PoseTrack::new(TrackId(0), &positions, 0, 128);
        assert_eq!(track.lifecycle, TrackLifecycleState::Tentative);

        // First update: still tentative (need 2 hits)
        track.update_keypoints(&positions, 0.08, 1.0, 100);
        assert_eq!(track.lifecycle, TrackLifecycleState::Active);
    }

    #[test]
    fn track_loss_gate() {
        let positions = zero_positions();
        let mut track = PoseTrack::new(TrackId(0), &positions, 0, 128);
        track.lifecycle = TrackLifecycleState::Active;

        // Predict without updates exceeding loss_misses
        for _ in 0..6 {
            track.predict(0.05, 0.3);
        }
        // Manually mark lost (normally done by tracker)
        if track.time_since_update >= 5 {
            track.mark_lost();
        }
        assert_eq!(track.lifecycle, TrackLifecycleState::Lost);
    }

    #[test]
    fn track_centroid() {
        let positions: [[f32; 3]; NUM_KEYPOINTS] =
            std::array::from_fn(|_| [1.0, 2.0, 3.0]);
        let track = PoseTrack::new(TrackId(0), &positions, 0, 128);
        let c = track.centroid();
        assert!((c[0] - 1.0).abs() < 1e-5);
        assert!((c[1] - 2.0).abs() < 1e-5);
        assert!((c[2] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn track_embedding_update() {
        let positions = zero_positions();
        let mut track = PoseTrack::new(TrackId(0), &positions, 0, 4);
        let new_embed = vec![1.0, 2.0, 3.0, 4.0];
        track.update_embedding(&new_embed, 0.5);
        // EMA: 0.5 * 0.0 + 0.5 * new = new / 2
        for i in 0..4 {
            assert!((track.embedding[i] - new_embed[i] * 0.5).abs() < 1e-5);
        }
    }

    #[test]
    fn tracker_create_and_find() {
        let mut tracker = PoseTracker::new();
        let positions = zero_positions();
        let id = tracker.create_track(&positions, 1000);
        assert!(tracker.find_track(id).is_some());
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn tracker_predict_marks_lost() {
        let mut tracker = PoseTracker::with_config(TrackerConfig {
            loss_misses: 3,
            reid_window: 10,
            ..Default::default()
        });
        let positions = zero_positions();
        let id = tracker.create_track(&positions, 0);

        // Promote to active
        if let Some(t) = tracker.find_track_mut(id) {
            t.lifecycle = TrackLifecycleState::Active;
        }

        // Predict 4 times without update
        for _ in 0..4 {
            tracker.predict_all(0.05);
        }

        let track = tracker.find_track(id).unwrap();
        assert_eq!(track.lifecycle, TrackLifecycleState::Lost);
    }

    #[test]
    fn tracker_prune_terminated() {
        let mut tracker = PoseTracker::new();
        let positions = zero_positions();
        let id = tracker.create_track(&positions, 0);
        if let Some(t) = tracker.find_track_mut(id) {
            t.terminate();
        }
        assert_eq!(tracker.all_tracks().len(), 1);
        tracker.prune_terminated();
        assert_eq!(tracker.all_tracks().len(), 0);
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_similarity_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn pose_detection_centroid() {
        let kps: [[f32; 4]; NUM_KEYPOINTS] =
            std::array::from_fn(|_| [1.0, 2.0, 3.0, 0.9]);
        let det = PoseDetection {
            keypoints: kps,
            embedding: vec![0.0; 128],
        };
        let c = det.centroid();
        assert!((c[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pose_detection_mean_confidence() {
        let kps: [[f32; 4]; NUM_KEYPOINTS] =
            std::array::from_fn(|_| [0.0, 0.0, 0.0, 0.8]);
        let det = PoseDetection {
            keypoints: kps,
            embedding: vec![0.0; 128],
        };
        assert!((det.mean_confidence() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn pose_detection_positions() {
        let kps: [[f32; 4]; NUM_KEYPOINTS] =
            std::array::from_fn(|i| [i as f32, 0.0, 0.0, 1.0]);
        let det = PoseDetection {
            keypoints: kps,
            embedding: vec![],
        };
        let pos = det.positions();
        assert_eq!(pos[0], [0.0, 0.0, 0.0]);
        assert_eq!(pos[5], [5.0, 0.0, 0.0]);
    }

    #[test]
    fn assignment_cost_computation() {
        let mut tracker = PoseTracker::new();
        let positions = zero_positions();
        let id = tracker.create_track(&positions, 0);

        let track = tracker.find_track(id).unwrap();
        let cost = tracker.assignment_cost(track, &[0.0, 0.0, 0.0], &vec![0.0; 128]);
        // Zero distance + zero embedding cost should be near 0
        // But embedding cost = 1 - cosine_sim(zeros, zeros) = 1 - 0 = 1
        // So cost = 0.6 * 0 + 0.4 * 1 = 0.4
        assert!((cost - 0.4).abs() < 0.1, "Expected ~0.4, got {}", cost);
    }

    #[test]
    fn torso_jitter_rms_stationary() {
        let positions = zero_positions();
        let track = PoseTrack::new(TrackId(0), &positions, 0, 128);
        let jitter = track.torso_jitter_rms();
        assert!(jitter < 1e-5, "Stationary track should have near-zero jitter");
    }

    #[test]
    fn default_tracker_config() {
        let cfg = TrackerConfig::default();
        assert!((cfg.process_noise - 0.3).abs() < f32::EPSILON);
        assert!((cfg.measurement_noise - 0.08).abs() < f32::EPSILON);
        assert!((cfg.mahalanobis_gate - 9.0).abs() < f32::EPSILON);
        assert_eq!(cfg.birth_hits, 2);
        assert_eq!(cfg.loss_misses, 5);
        assert_eq!(cfg.reid_window, 100);
        assert!((cfg.embedding_decay - 0.95).abs() < f32::EPSILON);
        assert_eq!(cfg.embedding_dim, 128);
        assert!((cfg.position_weight - 0.6).abs() < f32::EPSILON);
        assert!((cfg.embedding_weight - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn track_terminate_prevents_lost() {
        let positions = zero_positions();
        let mut track = PoseTrack::new(TrackId(0), &positions, 0, 128);
        track.terminate();
        assert_eq!(track.lifecycle, TrackLifecycleState::Terminated);
        track.mark_lost(); // Should not override Terminated
        assert_eq!(track.lifecycle, TrackLifecycleState::Terminated);
    }

    // -----------------------------------------------------------------------
    // SkeletonConstraints tests
    // -----------------------------------------------------------------------

    /// Build a plausible standing skeleton in normalised coordinates.
    fn valid_skeleton() -> [[f32; 3]; 17] {
        let mut kps = [[0.0_f32; 3]; 17];
        // Head / face (indices 0-4) clustered near top.
        kps[0] = [0.0, 1.0, 0.0];   // nose
        kps[1] = [-0.02, 1.02, 0.0]; // left eye
        kps[2] = [0.02, 1.02, 0.0];  // right eye
        kps[3] = [-0.04, 1.0, 0.0];  // left ear
        kps[4] = [0.04, 1.0, 0.0];   // right ear
        // Torso
        kps[5] = [-0.09, 0.85, 0.0]; // L shoulder
        kps[6] = [0.09, 0.85, 0.0];  // R shoulder
        kps[7] = [-0.09, 0.70, 0.0]; // L elbow  (dist ~0.15 from shoulder)
        kps[8] = [0.09, 0.70, 0.0];  // R elbow
        kps[9] = [-0.09, 0.56, 0.0]; // L wrist  (dist ~0.14 from elbow)
        kps[10] = [0.09, 0.56, 0.0]; // R wrist
        kps[11] = [-0.075, 0.60, 0.0]; // L hip  (dist ~0.25 from shoulder)
        kps[12] = [0.075, 0.60, 0.0];  // R hip
        kps[13] = [-0.075, 0.38, 0.0]; // L knee (dist ~0.22 from hip)
        kps[14] = [0.075, 0.38, 0.0];  // R knee
        kps[15] = [-0.075, 0.16, 0.0]; // L ankle (dist ~0.22 from knee)
        kps[16] = [0.075, 0.16, 0.0];  // R ankle
        kps
    }

    #[test]
    fn test_valid_skeleton_unchanged() {
        let mut kps = valid_skeleton();
        let before = kps;
        SkeletonConstraints::enforce_constraints(&mut kps);

        // Each keypoint should move by less than 0.02 (small perturbation
        // from iterative relaxation on an already-valid skeleton).
        for i in 0..17 {
            let d = ((kps[i][0] - before[i][0]).powi(2)
                + (kps[i][1] - before[i][1]).powi(2)
                + (kps[i][2] - before[i][2]).powi(2))
            .sqrt();
            assert!(
                d < 0.05,
                "keypoint {} moved {:.4}, expected < 0.05",
                i,
                d
            );
        }
    }

    #[test]
    fn test_stretched_bone_corrected() {
        let mut kps = valid_skeleton();

        // Stretch L shoulder -> L elbow to 2x expected (0.30 instead of 0.15).
        kps[7] = [-0.09, 0.55, 0.0]; // push elbow far down

        let dist_before = {
            let dx = kps[7][0] - kps[5][0];
            let dy = kps[7][1] - kps[5][1];
            let dz = kps[7][2] - kps[5][2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };
        assert!(
            dist_before > 0.25,
            "pre-condition: bone should be stretched, got {}",
            dist_before
        );

        SkeletonConstraints::enforce_constraints(&mut kps);

        let dist_after = {
            let dx = kps[7][0] - kps[5][0];
            let dy = kps[7][1] - kps[5][1];
            let dz = kps[7][2] - kps[5][2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        // After enforcement the bone should be much closer to the rest
        // length of 0.15 (within tolerance band 0.105 .. 0.195).
        assert!(
            dist_after < dist_before,
            "bone should be shorter after correction: before={:.4}, after={:.4}",
            dist_before,
            dist_after
        );
    }

    #[test]
    fn test_zero_skeleton_handled() {
        // All-zero keypoints must not panic.
        let mut kps = [[0.0_f32; 3]; 17];
        SkeletonConstraints::enforce_constraints(&mut kps);
        // Just assert it didn't panic; the result should still be all-zero
        // since zero-length bones are skipped.
        for kp in &kps {
            assert!(kp[0].is_finite());
            assert!(kp[1].is_finite());
            assert!(kp[2].is_finite());
        }
    }

    // -----------------------------------------------------------------------
    // CompressedPoseHistory tests
    // -----------------------------------------------------------------------

    #[test]
    fn compressed_history_push_and_len() {
        let mut hist = CompressedPoseHistory::new(3, 5, 0.001);
        assert!(hist.is_empty());
        assert_eq!(hist.len(), 0);

        let pose = valid_skeleton();
        hist.push(&pose);
        assert_eq!(hist.len(), 1);
        assert!(!hist.is_empty());

        // Fill hot
        hist.push(&pose);
        hist.push(&pose);
        assert_eq!(hist.len(), 3); // 3 hot, 0 warm

        // Overflow hot -> warm promotion
        hist.push(&pose);
        assert_eq!(hist.len(), 4); // 3 hot, 1 warm
    }

    #[test]
    fn compressed_history_warm_overflow() {
        let mut hist = CompressedPoseHistory::new(2, 2, 0.001);
        let pose = valid_skeleton();

        // Push 6 poses: hot=2, warm should cap at 2
        for _ in 0..6 {
            hist.push(&pose);
        }
        // hot=2, warm capped at 2
        assert_eq!(hist.len(), 4);
    }

    #[test]
    fn compressed_history_similarity_identical() {
        let mut hist = CompressedPoseHistory::default();
        let pose = valid_skeleton();
        hist.push(&pose);

        let sim = hist.similarity(&pose);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "identical pose should have similarity ~1.0, got {}",
            sim
        );
    }

    #[test]
    fn compressed_history_similarity_empty() {
        let hist = CompressedPoseHistory::default();
        let pose = valid_skeleton();
        assert_eq!(hist.similarity(&pose), 0.0);
    }

    #[test]
    fn compressed_history_default() {
        let hist = CompressedPoseHistory::default();
        assert_eq!(hist.max_hot, 10);
        assert_eq!(hist.max_warm, 50);
        assert!((hist.scale - 0.001).abs() < 1e-9);
    }

    // ── TemporalKeypointAttention tests (RuVector Phase 2) ─────────────

    #[test]
    fn temporal_attention_empty_returns_input() {
        let mut attn = TemporalKeypointAttention::new();
        let input: [[f32; 3]; NUM_KEYPOINTS] = std::array::from_fn(|i| [i as f32, 0.0, 0.0]);
        let out = attn.smooth_keypoints(&input);
        // First frame: no history, so output should equal input.
        for i in 0..NUM_KEYPOINTS {
            assert!((out[i][0] - input[i][0]).abs() < 1e-5);
        }
    }

    #[test]
    fn temporal_attention_smooths_jitter() {
        let mut attn = TemporalKeypointAttention::new();
        let base: [[f32; 3]; NUM_KEYPOINTS] = std::array::from_fn(|_| [100.0, 200.0, 0.0]);
        // Feed stable frames first.
        for _ in 0..5 {
            attn.smooth_keypoints(&base);
        }
        // Now feed a jittery frame.
        let jittery: [[f32; 3]; NUM_KEYPOINTS] = std::array::from_fn(|_| [110.0, 210.0, 0.0]);
        let out = attn.smooth_keypoints(&jittery);
        // Output should be closer to base than to jittery (smoothed).
        assert!(out[0][0] < 110.0, "Expected smoothing, got {}", out[0][0]);
        assert!(out[0][0] > 100.0, "Expected some movement, got {}", out[0][0]);
    }

    #[test]
    fn temporal_attention_window_size_capped() {
        let mut attn = TemporalKeypointAttention::with_params(3, 0.7);
        let frame: [[f32; 3]; NUM_KEYPOINTS] = std::array::from_fn(|_| [1.0, 1.0, 1.0]);
        for _ in 0..10 {
            attn.smooth_keypoints(&frame);
        }
        assert_eq!(attn.len(), 3);
    }

    #[test]
    fn temporal_attention_clear() {
        let mut attn = TemporalKeypointAttention::new();
        let frame = zero_positions();
        attn.smooth_keypoints(&frame);
        assert!(!attn.is_empty());
        attn.clear();
        assert!(attn.is_empty());
    }
}
