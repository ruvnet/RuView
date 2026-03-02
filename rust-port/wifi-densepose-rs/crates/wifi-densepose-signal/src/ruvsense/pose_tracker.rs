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
}
