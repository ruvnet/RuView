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
pub mod attractor_drift;
pub mod temporal_gesture;

// ADR-029: Core multistatic pipeline
pub mod coherence;
pub mod coherence_gate;
pub mod multiband;
pub mod multistatic;
pub mod phase_align;
pub mod pose_tracker;

// ADR-134: CIR estimation (ISTA + NeumannSolver warm-start)
pub mod cir;

// ADR-137: Fusion-engine quality scoring (evidence + contradiction flags)
pub mod fusion_quality;

// ADR-138: Array coordinator — clock-quality gating + directional evidence
pub mod array_coordinator;

// ADR-142: Evolution tracker + temporal VoxelMap (Bayesian, privacy-gated)
pub mod evolution;

// ADR-143: RF-SLAM persistent reflector discovery + static-anchor learning
pub mod rf_slam;

// ADR-135: Empty-room baseline calibration (Welford online, circular phase)
pub mod calibration;

// Re-export core types for ergonomic access
pub use coherence::CoherenceState;
pub use coherence_gate::{GateDecision, GatePolicy};
pub use array_coordinator::{
    ArrayCoordinator, ArrayCoordinatorConfig, ArrayNodeInput, DirectionalEvidence,
};
pub use evolution::{
    ChangePoint, EvolutionTracker, TemporalVoxel, TemporalVoxelMap, VoxelGate, VoxelPrivacy,
};
pub use rf_slam::{PersistentReflector, ReflectorClass, ReflectorObservation, RfSlam};
pub use fusion_quality::{
    CalibrationId, ContradictionFlag, EvidenceRef, FamilyId, QualityScore,
};
pub use multiband::{MultiBandCsiFrame, PhaseReference};
pub use multistatic::{FusedSensingFrame, NonCoherentReason, PhaseFusion};
pub use phase_align::{PhaseAlignError, PhaseAligner};
pub use pose_tracker::{
    CompressedPoseHistory, KeypointState, PoseTrack, SkeletonConstraints,
    TemporalKeypointAttention, TrackLifecycleState, TrackerConfig,
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
    pub const TORSO_INDICES: &[usize] = &[LEFT_SHOULDER, RIGHT_SHOULDER, LEFT_HIP, RIGHT_HIP];
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

// =============================================================================
// ADR-136 — Streaming-engine contract surface (Stage / Versioned / QualityScored)
// =============================================================================

/// `FrameMeta` is the streaming-engine vocabulary alias for the core
/// `CsiMetadata` (ADR-136 §2.2). It *is* the same struct — re-exported, not
/// copied — so cross-stage hops carry provenance (`calibration_id`, `model_id`,
/// `model_version`) without conversion cost.
pub use wifi_densepose_core::types::CsiMetadata as FrameMeta;

/// Result type returned by a [`Stage`] transform.
pub type StageResult<O> = std::result::Result<O, RuvSenseError>;

/// A pipeline stage that transforms one typed frame into another (ADR-136 §2.4).
///
/// Stages are `Send + Sync`. Determinism rule: given the same input bytes and
/// the same `&self` configuration, [`Stage::process`] MUST produce the same
/// output bytes (ADR-136 §2.5 replay contract). Mutable runtime state (rolling
/// windows, Welford accumulators) lives behind `&self` interior types whose
/// effect on output is captured by the deterministic-replay fixture.
///
/// **Boundary rule:** a stage never mutates its input's `FrameMeta.calibration_id`
/// or `model_id`/`model_version` except the calibration stage (sets
/// `calibration_id`) and the model-binding stage (sets the model fields). This
/// keeps provenance append-only along the chain.
pub trait Stage<I, O>: Send + Sync {
    /// Human/stage identifier, e.g. `"phase_align"`, `"calibration"`.
    fn name(&self) -> &'static str;

    /// Transform one input frame into one output frame.
    ///
    /// # Errors
    /// Returns [`RuvSenseError`] if the stage cannot process the input.
    fn process(&self, input: I) -> StageResult<O>;
}

/// Forward-compatible version stamp (ADR-136 §2.4, mirrors ADR-119 §2.1).
///
/// A `(major, minor)` pair plus a reserved-flags word so future revisions extend
/// without breaking the deterministic byte layout.
pub trait Versioned {
    /// `(major, minor)` version of this stage's output contract.
    fn version(&self) -> (u8, u8);

    /// Reserved forward-compat flags (ADR-119 reserved bits 2..15). Default `0`.
    fn reserved_flags(&self) -> u16 {
        0
    }

    /// True if a consumer at `other` can consume output produced at
    /// [`Self::version`] — equal major and `self.minor >= other.minor`.
    fn is_compatible_with(&self, other: (u8, u8)) -> bool {
        let (maj, min) = self.version();
        maj == other.0 && min >= other.1
    }
}

/// A stage output carrying a scalar quality score and a confidence interval
/// (ADR-136 §2.4). Consumed by ADR-137 (fusion quality) and ADR-145 (ablation).
pub trait QualityScored {
    /// Scalar quality in `[0.0, 1.0]`; higher is better.
    fn quality_score(&self) -> f32;

    /// `(lower, upper)` confidence bounds with `0.0 <= lower <= upper <= 1.0`.
    fn confidence_bounds(&self) -> (f32, f32);
}

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
    #[allow(dead_code)]
    phase_aligner: PhaseAligner,
    coherence_state: CoherenceState,
    #[allow(dead_code)]
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

    // ===== ADR-136 trait-surface acceptance tests =====

    // Tiny stages forming a Stage<u32,u32> -> Stage<u32,String> chain (AC4).
    struct Doubler;
    impl Stage<u32, u32> for Doubler {
        fn name(&self) -> &'static str {
            "doubler"
        }
        fn process(&self, input: u32) -> StageResult<u32> {
            Ok(input * 2)
        }
    }
    struct Stringify;
    impl Stage<u32, String> for Stringify {
        fn name(&self) -> &'static str {
            "stringify"
        }
        fn process(&self, input: u32) -> StageResult<String> {
            Ok(format!("v{input}"))
        }
    }

    /// AC4 — heterogeneous `Stage` chain composes and visits stages in order.
    #[test]
    fn ac4_stage_chain_composition() {
        let s1 = Doubler;
        let s2 = Stringify;
        let mut visited = Vec::new();
        visited.push(s1.name());
        let mid = s1.process(21).unwrap();
        visited.push(s2.name());
        let out = s2.process(mid).unwrap();
        assert_eq!(out, "v42");
        assert_eq!(visited, vec!["doubler", "stringify"]);
    }

    struct V(u8, u8);
    impl Versioned for V {
        fn version(&self) -> (u8, u8) {
            (self.0, self.1)
        }
    }

    /// AC5 — `Versioned` compatibility: equal major, minor >= consumer's.
    #[test]
    fn ac5_versioned_compatibility() {
        let v = V(1, 3);
        assert!(v.is_compatible_with((1, 3)), "equal");
        assert!(v.is_compatible_with((1, 0)), "newer minor accepts older consumer");
        assert!(!v.is_compatible_with((1, 4)), "older producer rejects newer consumer");
        assert!(!v.is_compatible_with((2, 0)), "major mismatch rejected");
        assert_eq!(v.reserved_flags(), 0);
    }

    struct Q(f32, f32, f32);
    impl QualityScored for Q {
        fn quality_score(&self) -> f32 {
            self.0
        }
        fn confidence_bounds(&self) -> (f32, f32) {
            (self.1, self.2)
        }
    }

    /// AC8 — `QualityScored` bounds invariant: 0 <= lower <= upper <= 1.
    #[test]
    fn ac8_quality_scored_bounds() {
        let q = Q(0.9, 0.7, 0.95);
        let s = q.quality_score();
        let (lo, hi) = q.confidence_bounds();
        assert!((0.0..=1.0).contains(&s));
        assert!(0.0 <= lo && lo <= hi && hi <= 1.0);
    }

    /// `FrameMeta` is the same type as core `CsiMetadata` (ADR-136 §2.2).
    #[test]
    fn frame_meta_is_csi_metadata() {
        fn assert_same<T>(_: &T, _: &T) {}
        let a = FrameMeta::new(
            wifi_densepose_core::types::DeviceId::new("n"),
            wifi_densepose_core::types::FrequencyBand::Band2_4GHz,
            1,
        );
        let b = wifi_densepose_core::types::CsiMetadata::new(
            wifi_densepose_core::types::DeviceId::new("n"),
            wifi_densepose_core::types::FrequencyBand::Band2_4GHz,
            1,
        );
        assert_same(&a, &b); // compiles only if FrameMeta == CsiMetadata
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
