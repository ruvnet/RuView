//! RuvSense -- Sensing-First RF Mode for Multistatic WiFi DensePose (ADR-029)
//!
//! This bounded context implements the multistatic sensing pipeline that fuses
//! CSI from multiple ESP32 nodes across multiple WiFi channels into a single
//! coherent sensing frame per 50 ms TDMA cycle (20 Hz output).
//!
//! # Architecture
//!
//! The pipeline flows through six stages:
//!
//! 1. **Multi-Band Fusion** (`multiband`) -- Aggregate per-channel CSI frames
//!    from channel-hopping into a wideband virtual snapshot per node.
//! 2. **Phase Alignment** (`phase_align`) -- Correct LO-induced phase rotation
//!    between channels using `ruvector-solver::NeumannSolver`.
//! 3. **Multistatic Fusion** (`multistatic`) -- Fuse N node observations into
//!    a single `FusedSensingFrame` with attention-based cross-node weighting
//!    via `ruvector-attn-mincut`.
//! 4. **Coherence Scoring** (`coherence`) -- Compute per-subcarrier z-score
//!    coherence against a rolling reference template.
//! 5. **Coherence Gating** (`coherence_gate`) -- Apply threshold-based gate
//!    decision: Accept / PredictOnly / Reject / Recalibrate.
//! 6. **Pose Tracking** (`pose_tracker`) -- 17-keypoint Kalman tracker with
//!    lifecycle state machine and AETHER re-ID embedding support.
//!
//! # RuVector Crate Usage
//!
//! - `ruvector-solver` -- Phase alignment, coherence decomposition
//! - `ruvector-attn-mincut` -- Cross-node spectrogram fusion
//! - `ruvector-mincut` -- Person separation and track assignment
//! - `ruvector-attention` -- Cross-channel feature weighting
//!
//! # References
//!
//! - ADR-029: Project RuvSense
//! - IEEE 802.11bf-2024 WLAN Sensing

// ADR-030: Exotic sensing tiers
pub mod adversarial;
pub mod cross_room;
pub mod field_model;
pub mod gesture;
pub mod intention;
pub mod longitudinal;
pub mod tomography;

// ADR-032a: Midstreamer-enhanced sensing
pub mod temporal_gesture;
pub mod attractor_drift;

// ADR-029: Core multistatic pipeline
pub mod coherence;
pub mod coherence_gate;
pub mod multiband;
pub mod multistatic;
pub mod phase_align;
pub mod pose_tracker;

// Re-export core types for ergonomic access
pub use coherence::CoherenceState;
pub use coherence_gate::{GateDecision, GatePolicy};
pub use multiband::MultiBandCsiFrame;
pub use multistatic::FusedSensingFrame;
pub use phase_align::{PhaseAligner, PhaseAlignError};
pub use pose_tracker::{
    CompressedPoseHistory, KeypointState, PoseTrack, SkeletonConstraints,
    TemporalKeypointAttention, TrackLifecycleState,
};

/// Number of keypoints in a full-body pose skeleton (COCO-17).
pub const NUM_KEYPOINTS: usize = 17;

/// Keypoint indices following the COCO-17 convention.
pub mod keypoint {
    pub const NOSE: usize = 0;
    pub const LEFT_EYE: usize = 1;
    pub const RIGHT_EYE: usize = 2;
    pub const LEFT_EAR: usize = 3;
    pub const RIGHT_EAR: usize = 4;
    pub const LEFT_SHOULDER: usize = 5;
    pub const RIGHT_SHOULDER: usize = 6;
    pub const LEFT_ELBOW: usize = 7;
    pub const RIGHT_ELBOW: usize = 8;
    pub const LEFT_WRIST: usize = 9;
    pub const RIGHT_WRIST: usize = 10;
    pub const LEFT_HIP: usize = 11;
    pub const RIGHT_HIP: usize = 12;
    pub const LEFT_KNEE: usize = 13;
    pub const RIGHT_KNEE: usize = 14;
    pub const LEFT_ANKLE: usize = 15;
    pub const RIGHT_ANKLE: usize = 16;

    /// Torso keypoint indices (shoulders, hips, spine midpoint proxy).
    pub const TORSO_INDICES: &[usize] = &[
        LEFT_SHOULDER,
        RIGHT_SHOULDER,
        LEFT_HIP,
        RIGHT_HIP,
    ];
}

/// Unique identifier for a pose track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackId(pub u64);

impl TrackId {
    /// Create a new track identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Track({})", self.0)
    }
}

/// Error type shared across the RuvSense pipeline.
#[derive(Debug, thiserror::Error)]
pub enum RuvSenseError {
    /// Phase alignment failed.
    #[error("Phase alignment error: {0}")]
    PhaseAlign(#[from] phase_align::PhaseAlignError),

    /// Multi-band fusion error.
    #[error("Multi-band fusion error: {0}")]
    MultiBand(#[from] multiband::MultiBandError),

    /// Multistatic fusion error.
    #[error("Multistatic fusion error: {0}")]
    Multistatic(#[from] multistatic::MultistaticError),

    /// Coherence computation error.
    #[error("Coherence error: {0}")]
    Coherence(#[from] coherence::CoherenceError),

    /// Pose tracker error.
    #[error("Pose tracker error: {0}")]
    PoseTracker(#[from] pose_tracker::PoseTrackerError),
}

/// Common result type for RuvSense operations.
pub type Result<T> = std::result::Result<T, RuvSenseError>;

/// Configuration for the RuvSense pipeline.
#[derive(Debug, Clone)]
pub struct RuvSenseConfig {
    /// Maximum number of nodes in the multistatic mesh.
    pub max_nodes: usize,
    /// Target output rate in Hz.
    pub target_hz: f64,
    /// Number of channels in the hop sequence.
    pub num_channels: usize,
    /// Coherence accept threshold (default 0.85).
    pub coherence_accept: f32,
    /// Coherence drift threshold (default 0.5).
    pub coherence_drift: f32,
    /// Maximum stale frames before recalibration (default 200 = 10s at 20Hz).
    pub max_stale_frames: u64,
    /// Embedding dimension for AETHER re-ID (default 128).
    pub embedding_dim: usize,
}

impl Default for RuvSenseConfig {
    fn default() -> Self {
        Self {
            max_nodes: 4,
            target_hz: 20.0,
            num_channels: 3,
            coherence_accept: 0.85,
            coherence_drift: 0.5,
            max_stale_frames: 200,
            embedding_dim: 128,
        }
    }
}

/// Top-level pipeline orchestrator for RuvSense multistatic sensing.
///
/// Coordinates the flow from raw per-node CSI frames through multi-band
/// fusion, phase alignment, multistatic fusion, coherence gating, and
/// finally into the pose tracker.
pub struct RuvSensePipeline {
    config: RuvSenseConfig,
    phase_aligner: PhaseAligner,
    coherence_state: CoherenceState,
    gate_policy: GatePolicy,
    frame_counter: u64,
}

impl RuvSensePipeline {
    /// Create a new pipeline with default configuration.
    pub fn new() -> Self {
        Self::with_config(RuvSenseConfig::default())
    }

    /// Create a new pipeline with the given configuration.
    pub fn with_config(config: RuvSenseConfig) -> Self {
        let n_sub = 56; // canonical subcarrier count
        Self {
            phase_aligner: PhaseAligner::new(config.num_channels),
            coherence_state: CoherenceState::new(n_sub, config.coherence_accept),
            gate_policy: GatePolicy::new(
                config.coherence_accept,
                config.coherence_drift,
                config.max_stale_frames,
            ),
            config,
            frame_counter: 0,
        }
    }

    /// Return a reference to the current pipeline configuration.
    pub fn config(&self) -> &RuvSenseConfig {
        &self.config
    }

    /// Return the total number of frames processed.
    pub fn frame_count(&self) -> u64 {
        self.frame_counter
    }

    /// Return a reference to the current coherence state.
    pub fn coherence_state(&self) -> &CoherenceState {
        &self.coherence_state
    }

    /// Advance the frame counter (called once per sensing cycle).
    pub fn tick(&mut self) {
        self.frame_counter += 1;
    }
}

impl Default for RuvSensePipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = RuvSenseConfig::default();
        assert_eq!(cfg.max_nodes, 4);
        assert!((cfg.target_hz - 20.0).abs() < f64::EPSILON);
        assert_eq!(cfg.num_channels, 3);
        assert!((cfg.coherence_accept - 0.85).abs() < f32::EPSILON);
        assert!((cfg.coherence_drift - 0.5).abs() < f32::EPSILON);
        assert_eq!(cfg.max_stale_frames, 200);
        assert_eq!(cfg.embedding_dim, 128);
    }

    #[test]
    fn pipeline_creation_defaults() {
        let pipe = RuvSensePipeline::new();
        assert_eq!(pipe.frame_count(), 0);
        assert_eq!(pipe.config().max_nodes, 4);
    }

    #[test]
    fn pipeline_tick_increments() {
        let mut pipe = RuvSensePipeline::new();
        pipe.tick();
        pipe.tick();
        pipe.tick();
        assert_eq!(pipe.frame_count(), 3);
    }

    #[test]
    fn track_id_display() {
        let tid = TrackId::new(42);
        assert_eq!(format!("{}", tid), "Track(42)");
        assert_eq!(tid.0, 42);
    }

    #[test]
    fn track_id_equality() {
        assert_eq!(TrackId(1), TrackId(1));
        assert_ne!(TrackId(1), TrackId(2));
    }

    #[test]
    fn keypoint_constants() {
        assert_eq!(keypoint::NOSE, 0);
        assert_eq!(keypoint::LEFT_ANKLE, 15);
        assert_eq!(keypoint::RIGHT_ANKLE, 16);
        assert_eq!(keypoint::TORSO_INDICES.len(), 4);
    }

    #[test]
    fn num_keypoints_is_17() {
        assert_eq!(NUM_KEYPOINTS, 17);
    }

    #[test]
    fn custom_config_pipeline() {
        let cfg = RuvSenseConfig {
            max_nodes: 6,
            target_hz: 10.0,
            num_channels: 6,
            coherence_accept: 0.9,
            coherence_drift: 0.4,
            max_stale_frames: 100,
            embedding_dim: 64,
        };
        let pipe = RuvSensePipeline::with_config(cfg);
        assert_eq!(pipe.config().max_nodes, 6);
        assert!((pipe.config().target_hz - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn error_display() {
        let err = RuvSenseError::Coherence(coherence::CoherenceError::EmptyInput);
        let msg = format!("{}", err);
        assert!(msg.contains("Coherence"));
    }

    #[test]
    fn pipeline_coherence_state_accessible() {
        let pipe = RuvSensePipeline::new();
        let cs = pipe.coherence_state();
        assert!(cs.score() >= 0.0);
    }
}
