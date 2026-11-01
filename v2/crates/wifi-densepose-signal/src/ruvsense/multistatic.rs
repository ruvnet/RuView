//! Multistatic Viewpoint Fusion (ADR-029 Section 2.4)
//!
//! With N ESP32 nodes in a TDMA mesh, each sensing cycle produces N
//! `MultiBandCsiFrame`s. This module fuses them into a single
//! `FusedSensingFrame` using attention-based cross-node weighting.
//!
//! # Algorithm
//!
//! 1. Collect N `MultiBandCsiFrame`s from the current sensing cycle.
//! 2. Use `ruvector-attn-mincut` for cross-node attention: cells showing
//!    correlated motion energy across nodes (body reflection) are amplified;
//!    cells with single-node energy (multipath artifact) are suppressed.
//! 3. Multi-person separation via `ruvector-mincut::DynamicMinCut` builds
//!    a cross-link correlation graph and partitions into K person clusters.
//!
//! # CIR Gate (ADR-134)
//!
//! When `MultistaticConfig::use_cir_gate` is true and a shared `CirEstimator`
//! is attached, the fused coherence score is augmented with the dominant-tap
//! ratio from the CIR of the first active link.  This isolates body-motion
//! signatures to specific delay bins rather than across all subcarriers.
//! Set `use_cir_gate = false` for the legacy CSI-domain-only path (A/B test).
//!
//! # RuVector Integration
//!
//! - `ruvector-attn-mincut` for cross-node spectrogram attention gating
//! - `ruvector-mincut` for person separation (DynamicMinCut)

use std::collections::HashMap;
use std::sync::Arc;

use super::cir::{CirConfig, CirEstimator};
use super::multiband::{MultiBandCsiFrame, PhaseReference};

/// Errors from multistatic fusion.
#[derive(Debug, thiserror::Error)]
pub enum MultistaticError {
    /// No node frames provided.
    #[error("No node frames provided for multistatic fusion")]
    NoFrames,

    /// Insufficient nodes for multistatic mode (need at least 2).
    #[error("Need at least 2 nodes for multistatic fusion, got {0}")]
    InsufficientNodes(usize),

    /// Timestamp mismatch beyond guard interval.
    #[error("Timestamp spread {spread_us} us exceeds guard interval {guard_us} us")]
    TimestampMismatch { spread_us: u64, guard_us: u64 },

    /// Dimension mismatch in fusion inputs.
    #[error("Dimension mismatch: node {node_idx} has {got} subcarriers, expected {expected}")]
    DimensionMismatch {
        node_idx: usize,
        expected: usize,
        got: usize,
    },
}

/// A fused sensing frame from all nodes at one sensing cycle.
///
/// This is the primary output of the multistatic fusion stage and serves
/// as input to model inference and the pose tracker.
#[derive(Debug, Clone)]
pub struct FusedSensingFrame {
    /// Timestamp of this sensing cycle in microseconds.
    pub timestamp_us: u64,
    /// Fused amplitude vector across all nodes (attention-weighted mean).
    /// Length = n_subcarriers.
    pub fused_amplitude: Vec<f32>,
    /// Fused phase vector across all nodes.
    ///
    /// Length = n_subcarriers when [`Self::phase_fusion`] carries phase
    /// ([`PhaseFusion::Coherent`] or [`PhaseFusion::SingleNode`]). **Empty**
    /// for [`PhaseFusion::AmplitudeOnly`]: phases with no shared reference are
    /// not combined, because a weighted mean of unrelated phases is not a
    /// measurement (issue #1752).
    pub fused_phase: Vec<f32>,
    /// Per-node multi-band frames (preserved for geometry computations).
    pub node_frames: Vec<MultiBandCsiFrame>,
    /// Node positions (x, y, z) in meters from deployment configuration.
    ///
    /// Parallel to [`Self::node_frames`]: `node_positions[i]` is the
    /// configured position of `node_frames[i].node_id`, resolved by identity
    /// rather than by rank (issue #1866). Nodes with no configured position
    /// appear at the origin. Index `i` is this cycle's cohort rank, which is
    /// *not* a node id -- read the id from the paired frame.
    pub node_positions: Vec<[f32; 3]>,
    /// Number of active nodes contributing to this frame.
    pub active_nodes: usize,
    /// Cross-node coherence score (0.0-1.0). Higher means more agreement
    /// across viewpoints, indicating a strong body reflection signal.
    ///
    /// Derived from amplitude attention weights. It is **not** RF phase
    /// coherence and never implies the nodes share a phase reference; see
    /// [`Self::phase_fusion`] for that.
    pub cross_node_coherence: f32,
    /// How (and whether) phase was combined this cycle, with the reason when
    /// it was not (issue #1752).
    pub phase_fusion: PhaseFusion,
}

/// Why phase was not combined across nodes (issue #1752).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonCoherentReason {
    /// At least one frame carries no measured phase (placeholder vector).
    PhaseUnavailable,
    /// Phases were measured, but against independent per-node oscillators.
    IndependentClocks,
    /// Frames declare shared references, but not the same one.
    MixedReferences,
}

impl NonCoherentReason {
    /// Stable machine-readable label for status/API output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PhaseUnavailable => "phase_unavailable",
            Self::IndependentClocks => "independent_clocks",
            Self::MixedReferences => "mixed_references",
        }
    }
}

/// Phase-combining mode for one fusion cycle (issue #1752).
///
/// Amplitude fusion is identical in every mode; only the phase output differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseFusion {
    /// Every frame declared the same shared phase reference, so phase was
    /// combined across nodes.
    Coherent {
        /// The shared reference all frames declared.
        reference_id: u32,
    },
    /// One node with measured phase: its phase passes through uncombined.
    SingleNode,
    /// Phase was not combined; `fused_phase` is empty.
    AmplitudeOnly(NonCoherentReason),
}

impl PhaseFusion {
    /// Decide the phase-combining mode for a cohort. Coherent combining needs
    /// every frame to declare the same [`PhaseReference::Shared`] id; anything
    /// less falls back to amplitude-only with the reason.
    #[must_use]
    pub fn for_frames(node_frames: &[MultiBandCsiFrame]) -> Self {
        if node_frames.is_empty()
            || node_frames
                .iter()
                .any(|f| f.phase_reference == PhaseReference::Unavailable)
        {
            return Self::AmplitudeOnly(NonCoherentReason::PhaseUnavailable);
        }
        if node_frames.len() == 1 {
            return Self::SingleNode;
        }
        let mut shared = node_frames.iter().map(|f| match f.phase_reference {
            PhaseReference::Shared { id } => Some(id),
            _ => None,
        });
        let Some(Some(first)) = shared.next() else {
            return Self::AmplitudeOnly(NonCoherentReason::IndependentClocks);
        };
        for id in shared {
            match id {
                None => return Self::AmplitudeOnly(NonCoherentReason::IndependentClocks),
                Some(id) if id != first => {
                    return Self::AmplitudeOnly(NonCoherentReason::MixedReferences)
                }
                Some(_) => {}
            }
        }
        Self::Coherent { reference_id: first }
    }

    /// Whether this cycle produced a phase vector.
    #[must_use]
    pub fn carries_phase(self) -> bool {
        !matches!(self, Self::AmplitudeOnly(_))
    }

    /// Stable machine-readable mode label for status/API output.
    #[must_use]
    pub fn mode_str(self) -> &'static str {
        match self {
            Self::Coherent { .. } => "coherent",
            Self::SingleNode => "single_node",
            Self::AmplitudeOnly(_) => "amplitude_only",
        }
    }

    /// Reason phase was not combined, if it was not.
    #[must_use]
    pub fn reason(self) -> Option<NonCoherentReason> {
        match self {
            Self::AmplitudeOnly(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Configuration for multistatic fusion.
#[derive(Debug, Clone)]
pub struct MultistaticConfig {
    /// Maximum timestamp spread (microseconds) across nodes in one cycle.
    ///
    /// # Derivation from the TDM schedule (issue #1031)
    ///
    /// In an N-slot TDMA mesh, node `k` transmits in slot `k`, so two nodes
    /// are *deliberately* separated by `(cycle_us × slot_fraction)`. On a real
    /// 2-node mesh (slots 0 and 1 of a ~36 ms cycle) we measured an
    /// **18,194 µs** spread between paired frames — i.e. the spread is the slot
    /// offset, NOT clock jitter. The previous 5,000 µs default therefore
    /// rejected every real frame set and fusion silently fell back to per-node
    /// sum/dedup, so multistatic fusion never actually ran on hardware.
    ///
    /// The default is now **60,000 µs (60 ms)**: a full 50 ms TDMA cycle (the
    /// worst-case spread for the last slot of a maximally-loaded schedule) plus
    /// ~20% headroom for inter-cycle scheduling jitter. This accepts a real
    /// N-node cycle as coherent while still rejecting a spread that exceeds one
    /// whole cycle (which would mean frames from *different* sensing cycles were
    /// mixed). Tune per deployment with [`MultistaticConfig::for_tdm_schedule`].
    pub guard_interval_us: u64,
    /// ADR-137 soft guard (microseconds): a spread above this but within
    /// `guard_interval_us` is fused but recorded as a `TimestampMismatch`
    /// contradiction (loose alignment ⇒ privacy demotion).
    ///
    /// Set to **20,000 µs (20 ms)**: just above the observed 18,194 µs 2-slot
    /// spread, so a normal 2-node cycle fuses *cleanly* (no demotion), but a
    /// spread approaching a full cycle is flagged as loose alignment. Kept below
    /// `guard_interval_us` so the soft band is meaningful.
    pub soft_guard_us: u64,
    /// Minimum number of nodes for multistatic mode.
    /// Falls back to single-node mode if fewer nodes are available.
    pub min_nodes: usize,
    /// Attention temperature for cross-node weighting.
    /// Lower temperature -> sharper attention (fewer nodes dominate).
    pub attention_temperature: f32,
    /// Whether to enable person separation via min-cut.
    pub enable_person_separation: bool,
    /// Enable the CIR-domain coherence gate (ADR-134).
    /// Set `false` to fall back to the legacy CSI-domain-only path (A/B test).
    pub use_cir_gate: bool,
}

impl Default for MultistaticConfig {
    fn default() -> Self {
        Self {
            // 60 ms hard / 20 ms soft — see field docs for the TDM derivation
            // (issue #1031). The old 5 ms hard guard rejected every real frame
            // set (observed 2-slot spread ≈ 18.2 ms), silently disabling fusion.
            guard_interval_us: 60_000,
            soft_guard_us: 20_000,
            min_nodes: 2,
            attention_temperature: 1.0,
            enable_person_separation: true,
            use_cir_gate: true,
        }
    }
}

impl MultistaticConfig {
    /// Derive a guard interval from an explicit TDM schedule (issue #1031).
    ///
    /// In an N-slot schedule with per-slot duration `slot_duration_us`, the
    /// maximum legitimate spread between two paired node frames in one cycle is
    /// the full cycle length `tdm_total_slots × slot_duration_us` (last slot vs
    /// first slot). The hard guard is set to that cycle length plus 20% jitter
    /// headroom; the soft guard to the schedule's own **designed stagger**,
    /// `(slots - 1) * slot_duration_us` -- the offset between the first and
    /// last slot, which is the spread a complete, healthy cycle produces.
    ///
    /// # Why not ~⅓ of the cycle (issue #1866-adjacent, corrected)
    ///
    /// The soft guard was previously `cycle_us / 3`, reasoned about as "a
    /// normal *adjacent-slot pair* fuses cleanly". But [`MultistaticFuser::fuse_scored`]
    /// does not compare pairs -- it takes `max(timestamp) - min(timestamp)`
    /// across the **whole cohort**, so the spread it measures is always the
    /// first-to-last offset, `(N - 1)` slots. Against that:
    ///
    /// ```text
    ///   soft < stagger  <=>  N*slot/3 < (N-1)*slot  <=>  N/3 < N-1  <=>  N > 1.5
    /// ```
    ///
    /// which holds for **every** `N >= 2`. The derived soft guard therefore
    /// never covered the schedule it was derived from, at any fleet size, and
    /// a complete healthy cycle was flagged as a `TimestampMismatch`
    /// contradiction -- which demotes the BFLD privacy class one step
    /// (ADR-137 §2.7 -> ADR-141) and suppresses raw outputs.
    ///
    /// The doc example below is the smallest demonstration: the 2-slot mesh
    /// whose **measured** 18,194 µs spread motivated this constructor derived
    /// a 12,000 µs soft guard, below its own motivating measurement.
    ///
    /// `tdm_total_slots` is clamped to ≥ 1. All other fields take their
    /// [`Default`] values.
    ///
    /// # Example
    /// ```
    /// use wifi_densepose_signal::ruvsense::multistatic::MultistaticConfig;
    /// // 2 slots × 18 ms = 36 ms cycle → ~43 ms hard guard accepts the
    /// // reported 18,194 µs 2-slot spread.
    /// let cfg = MultistaticConfig::for_tdm_schedule(2, 18_000);
    /// assert!(cfg.guard_interval_us >= 18_194);
    /// // ...and the soft guard now covers it too, so a healthy 2-slot cycle
    /// // is not recorded as a contradiction.
    /// assert!(cfg.soft_guard_us >= 18_194);
    /// ```
    #[must_use]
    pub fn for_tdm_schedule(tdm_total_slots: usize, slot_duration_us: u64) -> Self {
        let slots = tdm_total_slots.max(1) as u64;
        let cycle_us = slots.saturating_mul(slot_duration_us);
        // +20% jitter headroom on the full cycle.
        let guard_interval_us = cycle_us.saturating_add(cycle_us / 5).max(1);
        // Soft band at the schedule's designed stagger -- the first-to-last
        // slot offset, which is what `fuse_scored` measures. Kept strictly
        // below the hard guard, so a spread approaching a full cycle is still
        // flagged as loose alignment.
        // +20% jitter headroom, mirroring the hard guard: the *nominal* stagger
        // is not the observed one. The 2-slot mesh below has an 18,000 us
        // nominal stagger but a MEASURED 18,194 us spread, so a bare nominal
        // bound would flag the very cycle it was derived from. Always strictly
        // below the hard guard, since stagger < cycle.
        let stagger_us = slots.saturating_sub(1).saturating_mul(slot_duration_us);
        let soft_guard_us = stagger_us
            .saturating_add(stagger_us / 5)
            .clamp(1, guard_interval_us.saturating_sub(1).max(1));
        Self {
            guard_interval_us,
            soft_guard_us,
            ..Default::default()
        }
    }
}

/// Multistatic frame fuser.
///
/// Collects per-node multi-band frames and produces a single fused
/// sensing frame per TDMA cycle.
///
/// # CIR gate (ADR-134)
///
/// A single `Arc<CirEstimator>` is shared across all links.  When
/// `config.use_cir_gate` is true and a `CirEstimator` is attached, the fused
/// `cross_node_coherence` is blended with the dominant-tap ratio from the
/// first available CsiFrame's CIR estimate.  Set `use_cir_gate = false` to
/// disable the CIR path and keep the legacy frequency-domain coherence only.
pub struct MultistaticFuser {
    config: MultistaticConfig,
    /// Node positions in 3D space (meters), keyed by node id (issue #1866).
    ///
    /// Keyed rather than indexed because [`Self::fuse`] receives a *compacted*
    /// cohort: `node_frames_from_states_with_guard` drops nodes that are
    /// stale, outside the timestamp guard, or have an empty history. Under
    /// index addressing, one node missing a single beacon re-ranks every node
    /// above it and silently hands them their neighbour's coordinates. Each
    /// `MultiBandCsiFrame` carries its own `node_id`, so the position is
    /// looked up by identity and a silent node moves nobody.
    node_positions: HashMap<u8, [f32; 3]>,
    /// Optional shared CIR estimator (ADR-134).  `None` = legacy path only.
    cir_estimator: Option<Arc<CirEstimator>>,
}

impl std::fmt::Debug for MultistaticFuser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultistaticFuser")
            .field("config", &self.config)
            .field("node_positions", &self.node_positions)
            .field("cir_estimator", &self.cir_estimator.is_some())
            .finish()
    }
}

impl MultistaticFuser {
    /// Create a fuser with default configuration and no node positions.
    pub fn new() -> Self {
        Self {
            config: MultistaticConfig::default(),
            node_positions: HashMap::new(),
            cir_estimator: None,
        }
    }

    /// Create a fuser with custom configuration.
    pub fn with_config(config: MultistaticConfig) -> Self {
        Self {
            config,
            node_positions: HashMap::new(),
            cir_estimator: None,
        }
    }

    /// Attach a shared `CirEstimator` for CIR-domain coherence gating (ADR-134).
    ///
    /// One estimator is shared across all links.  Build it via
    /// `CirEstimator::new(CirConfig::ht20())` for ESP32-S3 HT20 deployments.
    /// Pass `None` to detach and fall back to the legacy path.
    pub fn set_cir_estimator(&mut self, estimator: Option<Arc<CirEstimator>>) {
        self.cir_estimator = estimator;
    }

    /// Create a fuser with a pre-built `CirEstimator` for **canonical-56**
    /// frames (ADR-154 — the correct default for the RuvSense pipeline).
    ///
    /// The fuser operates on `CanonicalCsiFrame`s, which `hardware_norm.rs`
    /// resamples onto a uniform 56-tone grid. `CirConfig::canonical56()` builds
    /// Φ over those 56 tones so `estimate()` actually runs; `CirConfig::ht20()`
    /// (52 active) would reject every canonical frame with `SubcarrierMismatch`
    /// and silently fall back to the frequency-domain coherence — the dead-gate
    /// bug ADR-154 fixes. Prefer this constructor for canonical-56 deployments.
    pub fn with_cir_canonical56() -> Self {
        let mut fuser = Self::new();
        fuser.cir_estimator = Some(Arc::new(CirEstimator::new(CirConfig::canonical56())));
        fuser
    }

    /// Create a fuser with a pre-built `CirEstimator` for **raw HT20** frames
    /// (64 FFT bins / 52 active tones).
    ///
    /// # Warning (ADR-154)
    ///
    /// This config only runs on frames whose subcarrier count is 64 or 52. The
    /// RuvSense multistatic path feeds *canonical-56* frames, so this estimator
    /// rejects them with `SubcarrierMismatch` and the CIR gate silently
    /// degrades to frequency-domain coherence. Use [`Self::with_cir_canonical56`]
    /// for the canonical pipeline; keep this only for paths that genuinely feed
    /// raw 64/52-bin HT20 frames.
    pub fn with_cir_ht20() -> Self {
        let mut fuser = Self::new();
        fuser.cir_estimator = Some(Arc::new(CirEstimator::new(CirConfig::ht20())));
        fuser
    }

    /// Set node positions from a positional list (**legacy compatibility**).
    ///
    /// The list index is the node id: `positions[i]` belongs to node `i`. That
    /// is the meaning the `--node-positions` flag has always carried when its
    /// entries are given without an explicit `node_id:` prefix, so this
    /// mapping is preserved exactly rather than reinterpreted.
    ///
    /// This is only correct for fleets whose ids are dense and zero-based.
    /// Prefer [`Self::set_node_positions_by_id`], which states the identity
    /// instead of inferring it from ordering.
    pub fn set_node_positions(&mut self, positions: Vec<[f32; 3]>) {
        self.node_positions = positions
            .into_iter()
            .enumerate()
            .map(|(idx, position)| (idx as u8, position))
            .collect();
    }

    /// Set node positions keyed by node id (issue #1866, preferred).
    ///
    /// The position travels with the board it was measured for. Nodes absent
    /// from the map fuse at the origin, exactly as before; nodes present but
    /// not currently reporting cost nothing and shift nobody.
    pub fn set_node_positions_by_id(&mut self, positions: HashMap<u8, [f32; 3]>) {
        self.node_positions = positions;
    }

    /// Return the configured position for one node id, if any.
    #[must_use]
    pub fn node_position(&self, node_id: u8) -> Option<[f32; 3]> {
        self.node_positions.get(&node_id).copied()
    }

    /// Return the configured hard timestamp guard in microseconds.
    ///
    /// Callers that assemble frames before invoking [`Self::fuse`] use this
    /// to select one temporally coherent sensing cohort. Keeping selection and
    /// validation on the same guard prevents stale low-rate nodes from making
    /// every otherwise-live fusion cycle fail.
    #[must_use]
    pub fn guard_interval_us(&self) -> u64 {
        self.config.guard_interval_us
    }

    /// Return the configured positions keyed by node id.
    #[must_use]
    pub fn node_positions_by_id(&self) -> &HashMap<u8, [f32; 3]> {
        &self.node_positions
    }

    /// Return the configured positions as a dense id-indexed list
    /// (**legacy compatibility view**).
    ///
    /// Entry `i` is node `i`'s position, with unconfigured ids filled at the
    /// origin, so this round-trips [`Self::set_node_positions`]. It is a view
    /// of the configuration, *not* of what any particular fusion cycle used —
    /// a cycle's cohort is compacted, so read
    /// [`FusedSensingFrame::node_positions`] alongside its `node_frames` for
    /// that.
    #[must_use]
    pub fn node_positions(&self) -> Vec<[f32; 3]> {
        let Some(&max_id) = self.node_positions.keys().max() else {
            return Vec::new();
        };
        let mut dense = vec![[0.0_f32, 0.0, 0.0]; max_id as usize + 1];
        for (&id, &position) in &self.node_positions {
            dense[id as usize] = position;
        }
        dense
    }

    /// Fuse multiple node frames into a single `FusedSensingFrame`.
    ///
    /// When only one node is provided, falls back to single-node mode
    /// (no cross-node attention). When two or more nodes are available,
    /// applies attention-weighted fusion.
    pub fn fuse(
        &self,
        node_frames: &[MultiBandCsiFrame],
    ) -> std::result::Result<FusedSensingFrame, MultistaticError> {
        if node_frames.is_empty() {
            return Err(MultistaticError::NoFrames);
        }

        // Validate timestamp spread
        if node_frames.len() > 1 {
            let min_ts = node_frames.iter().map(|f| f.timestamp_us).min().unwrap();
            let max_ts = node_frames.iter().map(|f| f.timestamp_us).max().unwrap();
            let spread = max_ts - min_ts;
            if spread > self.config.guard_interval_us {
                return Err(MultistaticError::TimestampMismatch {
                    spread_us: spread,
                    guard_us: self.config.guard_interval_us,
                });
            }
        }

        // Extract per-node amplitude vectors from first channel of each node
        let amplitudes: Vec<&[f32]> = node_frames
            .iter()
            .filter_map(|f| f.channel_frames.first().map(|cf| cf.amplitude.as_slice()))
            .collect();

        let phases: Vec<&[f32]> = node_frames
            .iter()
            .filter_map(|f| f.channel_frames.first().map(|cf| cf.phase.as_slice()))
            .collect();

        if amplitudes.is_empty() {
            return Err(MultistaticError::NoFrames);
        }

        // Validate dimension consistency
        let n_sub = amplitudes[0].len();
        for (i, amp) in amplitudes.iter().enumerate().skip(1) {
            if amp.len() != n_sub {
                return Err(MultistaticError::DimensionMismatch {
                    node_idx: i,
                    expected: n_sub,
                    got: amp.len(),
                });
            }
        }

        // Issue #1752: only combine phase when the frames share a declared
        // phase reference. Otherwise fuse amplitude alone and say why.
        let phase_fusion = PhaseFusion::for_frames(node_frames);
        let phases = phase_fusion.carries_phase().then_some(phases.as_slice());

        let n_nodes = amplitudes.len();
        let (fused_amp, fused_ph, freq_coherence) = if n_nodes == 1 {
            // Single-node fallback
            let ph = phases.map_or_else(Vec::new, |p| p[0].to_vec());
            (amplitudes[0].to_vec(), ph, 1.0_f32)
        } else {
            // Multi-node attention-weighted fusion
            attention_weighted_fusion(&amplitudes, phases, self.config.attention_temperature)
        };

        // ADR-134 CIR gate: blend freq-domain coherence with CIR dominant-tap
        // ratio from the first available frame.  When use_cir_gate = false,
        // the legacy freq-domain coherence is used unchanged (A/B switch).
        let coherence = self.cir_gate_coherence(freq_coherence, node_frames);

        // Derive timestamp from median
        let mut timestamps: Vec<u64> = node_frames.iter().map(|f| f.timestamp_us).collect();
        timestamps.sort_unstable();
        let timestamp_us = timestamps[timestamps.len() / 2];

        // Build the node positions list parallel to `node_frames`, resolving
        // each entry by that frame's own `node_id` (issue #1866). The cohort
        // handed to `fuse` is compacted -- stale, out-of-guard and
        // empty-history nodes are already dropped -- so a frame's index here
        // is its rank in this cycle, not its identity. Addressing by index
        // gave every node above a gap its neighbour's coordinates whenever a
        // single board missed a beacon. Unknown nodes still fill with the
        // origin, unchanged.
        let positions: Vec<[f32; 3]> = node_frames
            .iter()
            .map(|frame| {
                self.node_positions
                    .get(&frame.node_id)
                    .copied()
                    .unwrap_or([0.0, 0.0, 0.0])
            })
            .collect();

        Ok(FusedSensingFrame {
            timestamp_us,
            fused_amplitude: fused_amp,
            fused_phase: fused_ph,
            node_frames: node_frames.to_vec(),
            node_positions: positions,
            active_nodes: n_nodes,
            cross_node_coherence: coherence,
            phase_fusion,
        })
    }

    /// Fuse and produce an auditable [`QualityScore`] alongside the frame
    /// (ADR-137). Additive over [`Self::fuse`]: the frame is identical; the
    /// score records the per-node attention weights actually used, the positive
    /// [`EvidenceRef`]s, and any tolerated [`ContradictionFlag`]s (e.g. a loose
    /// but in-guard timestamp spread). A non-empty contradiction set must demote
    /// the downstream BFLD privacy class (see [`QualityScore::forces_privacy_demotion`]).
    ///
    /// `coherence_accept` is the gate threshold (mirrors `RuvSenseConfig`);
    /// meeting it records a [`EvidenceRef::CoherenceGateThreshold`].
    ///
    /// # Errors
    /// Same hard-error preconditions as [`Self::fuse`].
    pub fn fuse_scored(
        &self,
        node_frames: &[MultiBandCsiFrame],
        coherence_accept: f32,
    ) -> std::result::Result<(FusedSensingFrame, super::fusion_quality::QualityScore), MultistaticError>
    {
        use super::fusion_quality::{ContradictionFlag, EvidenceRef, FamilyId, QualityScore};

        let fused = self.fuse(node_frames)?;

        // Recompute the per-node amplitude views (same selection as `fuse`).
        let amplitudes: Vec<&[f32]> = node_frames
            .iter()
            .filter_map(|f| f.channel_frames.first().map(|cf| cf.amplitude.as_slice()))
            .collect();
        let n_nodes = amplitudes.len();
        let per_node_weights = if n_nodes <= 1 {
            vec![1.0_f32; n_nodes]
        } else {
            node_attention_weights(&amplitudes, self.config.attention_temperature)
        };

        // --- Positive evidence ---
        let mut evidence_refs = Vec::new();
        if n_nodes > 1 {
            evidence_refs.push(EvidenceRef::WeightEntropy {
                normalized_entropy: compute_weight_coherence(&per_node_weights),
                n_nodes,
            });
        }
        if fused.cross_node_coherence >= coherence_accept {
            evidence_refs.push(EvidenceRef::CoherenceGateThreshold {
                coherence: fused.cross_node_coherence,
                threshold: coherence_accept,
            });
        }

        // --- Tolerated contradictions ---
        let mut contradiction_flags = Vec::new();
        if n_nodes > 1 {
            let min_ts = node_frames.iter().map(|f| f.timestamp_us).min().unwrap_or(0);
            let max_ts = node_frames.iter().map(|f| f.timestamp_us).max().unwrap_or(0);
            let spread_ns = (max_ts - min_ts).saturating_mul(1000);
            let soft_guard_ns = self.config.soft_guard_us.saturating_mul(1000);
            if spread_ns > soft_guard_ns {
                contradiction_flags.push(ContradictionFlag::TimestampMismatch {
                    spread_ns,
                    soft_guard_ns,
                });
            }
        }

        let capture_ns = fused.timestamp_us.saturating_mul(1000);
        let base_coherence = fused.cross_node_coherence;
        Ok((
            fused,
            QualityScore {
                family_id: FamilyId::MultistaticCsi,
                capture_ns,
                // Frames at this layer do not yet carry a calibration epoch
                // (ADR-135 id propagation lands with the calibration Stage);
                // recorded as None until then.
                calibration_id: None,
                base_coherence,
                per_node_weights,
                evidence_refs,
                contradiction_flags,
                timestamp_computed_ns: capture_ns,
            },
        ))
    }

    /// Like [`Self::fuse_scored`], but threads a per-node calibration epoch
    /// (ADR-137 §2.3). `calibrations[i]` is the [`CalibrationId`] applied to
    /// `node_frames[i]` (ADR-135 `BaselineCalibration::calibration_id`).
    ///
    /// - If every contributing node carries the **same** calibration id, the
    ///   score's `calibration_id` is set to it and a
    ///   [`EvidenceRef::CalibrationApplied`] is recorded.
    /// - If the calibrations **disagree** (or some are missing), the score's
    ///   `calibration_id` is left `None` and a
    ///   [`ContradictionFlag::CalibrationIdMismatch`] is raised — which forces a
    ///   downstream privacy demotion (ADR-141).
    ///
    /// # Errors
    /// Same hard-error preconditions as [`Self::fuse`].
    pub fn fuse_scored_calibrated(
        &self,
        node_frames: &[MultiBandCsiFrame],
        calibrations: &[Option<super::fusion_quality::CalibrationId>],
        coherence_accept: f32,
    ) -> std::result::Result<(FusedSensingFrame, super::fusion_quality::QualityScore), MultistaticError>
    {
        use super::fusion_quality::{ContradictionFlag, EvidenceRef};
        let (fused, mut score) = self.fuse_scored(node_frames, coherence_accept)?;

        let present: Vec<_> = calibrations.iter().flatten().copied().collect();
        if present.is_empty() {
            return Ok((fused, score)); // uncalibrated path — leave None.
        }
        // Modal (most frequent) calibration id; ties resolve to the first seen.
        let mut modal = present[0];
        let mut best = 0usize;
        for &cand in &present {
            let c = present.iter().filter(|&&x| x == cand).count();
            if c > best {
                best = c;
                modal = cand;
            }
        }
        // Disagreement = any node whose calibration differs from the modal,
        // including nodes that carried no calibration at all.
        let agreeing = present.iter().filter(|&&x| x == modal).count();
        let disagreeing = calibrations.len() - agreeing;

        if disagreeing == 0 {
            score.calibration_id = Some(modal);
            score.evidence_refs.push(EvidenceRef::CalibrationApplied {
                calibration_id: modal,
                n_frames: agreeing,
            });
        } else {
            // Mismatch: unsafe to claim a single calibration epoch (§2.3).
            score.calibration_id = None;
            score
                .contradiction_flags
                .push(ContradictionFlag::CalibrationIdMismatch { expected: modal, disagreeing });
        }
        Ok((fused, score))
    }

    /// Apply the CIR-domain coherence gate (ADR-134).
    ///
    /// When `use_cir_gate` is enabled and a `CirEstimator` is present, runs
    /// the estimator on the first node's first channel frame and blends the
    /// dominant-tap ratio into the frequency-domain coherence score.
    ///
    /// On `CirError::UnsanitizedPhase` the CIR result is dropped and the
    /// frequency-domain coherence is returned unchanged (graceful fallback).
    fn cir_gate_coherence(
        &self,
        freq_coherence: f32,
        node_frames: &[MultiBandCsiFrame],
    ) -> f32 {
        if !self.config.use_cir_gate {
            return freq_coherence;
        }
        let Some(ref estimator) = self.cir_estimator else {
            return freq_coherence;
        };

        // Build a minimal CsiFrame from the first node's first channel frame.
        // We use the amplitude+phase vectors to reconstruct complex values.
        let Some(first_frame) = node_frames.first() else {
            return freq_coherence;
        };
        let Some(cf) = first_frame.channel_frames.first() else {
            return freq_coherence;
        };
        // Issue #1752: a placeholder phase vector would make the CIR describe
        // a zero-phase spectrum, not the channel. Skip the blend when ANY frame
        // in the cohort has a placeholder phase, not just the first.
        if node_frames
            .iter()
            .any(|f| f.phase_reference == PhaseReference::Unavailable)
        {
            return freq_coherence;
        }

        // Reconstruct Complex64 data from amplitude+phase for the CIR estimator.
        let csi_frame = build_csi_frame_from_channel(cf);
        match estimator.estimate(&csi_frame) {
            Ok(cir) => {
                // Blend: coherence = 0.7 · freq + 0.3 · dominant_tap_ratio.
                // High dominant-tap ratio ≡ strong LOS → supports coherent gate.
                0.7 * freq_coherence + 0.3 * cir.dominant_tap_ratio
            }
            Err(super::cir::CirError::UnsanitizedPhase { .. }) => {
                // Frame not sanitized — fall back to freq-domain coherence.
                freq_coherence
            }
            Err(super::cir::CirError::SubcarrierMismatch { expected, got }) => {
                // ADR-154: a mismatch here means the estimator was built for the
                // WRONG tier (e.g. ht20's 52-active Φ vs a canonical-56 frame).
                // That is a *config* error, not a runtime data condition, so make
                // it LOUD in debug builds instead of silently degrading — a silent
                // degrade is exactly how the dead-gate bug hid in production.
                debug_assert!(
                    false,
                    "CIR gate DEAD: estimator expects {expected} subcarriers but got {got}; \
                     build it with CirConfig::canonical56() (see MultistaticFuser::with_cir_canonical56). \
                     Falling back to frequency-domain coherence."
                );
                freq_coherence
            }
            Err(_) => freq_coherence,
        }
    }

    /// Test/diagnostic hook (ADR-154): run the CIR estimator on the first frame
    /// of `node_frames` and return the raw `estimate()` result. Returns `None`
    /// when the gate is disabled or no estimator/frame is available.
    ///
    /// This exposes the Ok/Err verdict that `cir_gate_coherence` consumes, so a
    /// regression test can prove the gate actually runs (counts Ok vs Err on a
    /// canonical-56 stream) rather than silently degrading.
    pub fn cir_estimate_first(
        &self,
        node_frames: &[MultiBandCsiFrame],
    ) -> Option<Result<super::cir::Cir, super::cir::CirError>> {
        if !self.config.use_cir_gate {
            return None;
        }
        let estimator = self.cir_estimator.as_ref()?;
        let cf = node_frames.first()?.channel_frames.first()?;
        let csi_frame = build_csi_frame_from_channel(cf);
        Some(estimator.estimate(&csi_frame))
    }
}

impl Default for MultistaticFuser {
    fn default() -> Self {
        Self::new()
    }
}

/// Reconstruct a minimal `CsiFrame` from a `CanonicalCsiFrame` for CIR estimation.
///
/// Amplitude and phase are re-combined into `Complex64` values so that
/// `CirEstimator::estimate()` can extract the active-subcarrier vector.
fn build_csi_frame_from_channel(
    cf: &crate::hardware_norm::CanonicalCsiFrame,
) -> wifi_densepose_core::types::CsiFrame {
    use ndarray::Array2;
    use num_complex::Complex64;
    use wifi_densepose_core::types::{CsiFrame, CsiMetadata, DeviceId, FrequencyBand};

    let n = cf.amplitude.len();
    let mut data = Array2::<Complex64>::zeros((1, n));
    for (ki, (&amp, &ph)) in cf.amplitude.iter().zip(cf.phase.iter()).enumerate() {
        data[[0, ki]] = Complex64::from_polar(amp as f64, ph as f64);
    }
    let meta = CsiMetadata::new(
        DeviceId::new("multistatic-cir"),
        FrequencyBand::Band2_4GHz,
        6,
    );
    CsiFrame::new(meta, data)
}

/// Attention-weighted fusion of amplitude and phase vectors from multiple nodes.
///
/// Each node's contribution is weighted by its agreement with the consensus.
/// Phase is combined only when `phases` is `Some` (a shared phase reference,
/// issue #1752); otherwise the returned phase vector is empty.
/// Returns (fused_amplitude, fused_phase, cross_node_coherence).
fn attention_weighted_fusion(
    amplitudes: &[&[f32]],
    phases: Option<&[&[f32]]>,
    temperature: f32,
) -> (Vec<f32>, Vec<f32>, f32) {
    let n_sub = amplitudes[0].len();

    // Attention weights (cosine similarity to consensus, softmax).
    let weights = node_attention_weights(amplitudes, temperature);

    // Weighted fusion
    let mut fused_amp = vec![0.0_f32; n_sub];
    let mut fused_ph_sin = vec![0.0_f32; n_sub];
    let mut fused_ph_cos = vec![0.0_f32; n_sub];

    for (n, &amp) in amplitudes.iter().enumerate() {
        let w = weights[n];
        for i in 0..n_sub {
            fused_amp[i] += w * amp[i];
        }
        if let Some(ph) = phases.and_then(|p| p.get(n)) {
            for i in 0..n_sub {
                fused_ph_sin[i] += w * ph[i].sin();
                fused_ph_cos[i] += w * ph[i].cos();
            }
        }
    }

    // Recover phase from sin/cos weighted average
    let fused_ph: Vec<f32> = if phases.is_some() {
        fused_ph_sin
            .iter()
            .zip(fused_ph_cos.iter())
            .map(|(&s, &c)| s.atan2(c))
            .collect()
    } else {
        Vec::new()
    };

    // Coherence = mean weight entropy proxy: high when weights are balanced
    let coherence = compute_weight_coherence(&weights);

    (fused_amp, fused_ph, coherence)
}

/// Compute the per-node attention weights (cosine similarity to the amplitude
/// consensus, softmaxed at `temperature`). Returned weights sum to ~1.0 and are
/// node-index aligned. Exposed so the ADR-137 fusion-quality scorer records the
/// exact weights used for fusion rather than re-deriving an approximation.
#[must_use]
pub fn node_attention_weights(amplitudes: &[&[f32]], temperature: f32) -> Vec<f32> {
    let n_nodes = amplitudes.len();
    if n_nodes == 0 {
        return Vec::new();
    }
    let n_sub = amplitudes[0].len();

    // Mean amplitude as consensus reference.
    let mut mean_amp = vec![0.0_f32; n_sub];
    for amp in amplitudes {
        for (i, &v) in amp.iter().enumerate() {
            mean_amp[i] += v;
        }
    }
    for v in &mut mean_amp {
        *v /= n_nodes as f32;
    }

    // Cosine-similarity logits.
    let mut logits = vec![0.0_f32; n_nodes];
    for (n, amp) in amplitudes.iter().enumerate() {
        let mut dot = 0.0_f32;
        let mut norm_a = 0.0_f32;
        let mut norm_b = 0.0_f32;
        for i in 0..n_sub.min(amp.len()) {
            dot += amp[i] * mean_amp[i];
            norm_a += amp[i] * amp[i];
            norm_b += mean_amp[i] * mean_amp[i];
        }
        let denom = (norm_a * norm_b).sqrt().max(1e-12);
        logits[n] = (dot / denom) / temperature;
    }

    // Numerically stable softmax.
    let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut weights = vec![0.0_f32; n_nodes];
    for (n, &logit) in logits.iter().enumerate() {
        weights[n] = (logit - max_logit).exp();
    }
    let weight_sum: f32 = weights.iter().sum::<f32>().max(1e-12);
    for w in &mut weights {
        *w /= weight_sum;
    }
    weights
}

/// Compute coherence from attention weights.
///
/// Returns 1.0 when all weights are equal (all nodes agree),
/// and approaches 0.0 when a single node dominates.
pub(crate) fn compute_weight_coherence(weights: &[f32]) -> f32 {
    let n = weights.len() as f32;
    if n <= 1.0 {
        return 1.0;
    }

    // Normalized entropy: H / log(n)
    let max_entropy = n.ln();
    if max_entropy < 1e-12 {
        return 1.0;
    }

    let entropy: f32 = weights
        .iter()
        .filter(|&&w| w > 1e-12)
        .map(|&w| -w * w.ln())
        .sum();

    (entropy / max_entropy).clamp(0.0, 1.0)
}

/// Compute the geometric diversity score for a set of node positions.
///
/// Returns a value in [0.0, 1.0] where 1.0 indicates maximum angular
/// coverage. Based on the angular span of node positions relative to the
/// room centroid.
pub fn geometric_diversity(positions: &[[f32; 3]]) -> f32 {
    if positions.len() < 2 {
        return 0.0;
    }

    // Compute centroid
    let n = positions.len() as f32;
    let centroid = [
        positions.iter().map(|p| p[0]).sum::<f32>() / n,
        positions.iter().map(|p| p[1]).sum::<f32>() / n,
        positions.iter().map(|p| p[2]).sum::<f32>() / n,
    ];

    // Compute angles from centroid to each node (in 2D, ignoring z)
    let mut angles: Vec<f32> = positions
        .iter()
        .map(|p| {
            let dx = p[0] - centroid[0];
            let dy = p[1] - centroid[1];
            dy.atan2(dx)
        })
        .collect();

    angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Angular coverage: sum of gaps, diversity is high when gaps are even
    let mut max_gap = 0.0_f32;
    for i in 0..angles.len() {
        let next = (i + 1) % angles.len();
        let mut gap = angles[next] - angles[i];
        if gap < 0.0 {
            gap += 2.0 * std::f32::consts::PI;
        }
        max_gap = max_gap.max(gap);
    }

    // Perfect coverage (N equidistant nodes): max_gap = 2*pi/N
    // Worst case (all co-located): max_gap = 2*pi
    let ideal_gap = 2.0 * std::f32::consts::PI / positions.len() as f32;
    (ideal_gap / max_gap.max(1e-6)).clamp(0.0, 1.0)
}

/// Represents a cluster of TX-RX links attributed to one person.
#[derive(Debug, Clone)]
pub struct PersonCluster {
    /// Cluster identifier.
    pub id: usize,
    /// Indices into the link array belonging to this cluster.
    pub link_indices: Vec<usize>,
    /// Mean correlation strength within the cluster.
    pub intra_correlation: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware_norm::{CanonicalCsiFrame, HardwareType};

    fn make_node_frame(
        node_id: u8,
        timestamp_us: u64,
        n_sub: usize,
        scale: f32,
    ) -> MultiBandCsiFrame {
        let amp: Vec<f32> = (0..n_sub).map(|i| scale * (1.0 + 0.1 * i as f32)).collect();
        let phase: Vec<f32> = (0..n_sub).map(|i| i as f32 * 0.05).collect();
        MultiBandCsiFrame {
            node_id,
            timestamp_us,
            channel_frames: vec![CanonicalCsiFrame {
                amplitude: amp,
                phase,
                hardware_type: HardwareType::Esp32S3,
            }],
            frequencies_mhz: vec![2412],
            coherence: 0.9,
            phase_reference: PhaseReference::NodeLocal,
        }
    }

    fn with_reference(mut f: MultiBandCsiFrame, r: PhaseReference) -> MultiBandCsiFrame {
        f.phase_reference = r;
        f
    }

    #[test]
    fn fuse_single_node_fallback() {
        let fuser = MultistaticFuser::new();
        let frames = vec![make_node_frame(0, 1000, 56, 1.0)];
        let fused = fuser.fuse(&frames).unwrap();
        assert_eq!(fused.active_nodes, 1);
        assert_eq!(fused.fused_amplitude.len(), 56);
        assert!((fused.cross_node_coherence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fuse_two_identical_nodes() {
        let fuser = MultistaticFuser::new();
        let f0 = make_node_frame(0, 1000, 56, 1.0);
        let f1 = make_node_frame(1, 1001, 56, 1.0);
        let fused = fuser.fuse(&[f0, f1]).unwrap();
        assert_eq!(fused.active_nodes, 2);
        assert_eq!(fused.fused_amplitude.len(), 56);
        // Identical nodes -> high coherence
        assert!(fused.cross_node_coherence > 0.5);
    }

    #[test]
    fn fuse_four_nodes() {
        let fuser = MultistaticFuser::new();
        let frames: Vec<MultiBandCsiFrame> = (0..4)
            .map(|i| make_node_frame(i, 1000 + i as u64, 56, 1.0 + 0.1 * i as f32))
            .collect();
        let fused = fuser.fuse(&frames).unwrap();
        assert_eq!(fused.active_nodes, 4);
    }

    // ===== ADR-137 fusion-quality scoring =====

    #[test]
    fn ac_fuse_scored_tight_alignment_no_contradiction() {
        use super::super::fusion_quality::{EvidenceRef, FamilyId};
        let fuser = MultistaticFuser::new();
        // Two identical nodes, 1 us apart (< soft_guard 1000 us): no contradiction.
        let f0 = make_node_frame(0, 1000, 56, 1.0);
        let f1 = make_node_frame(1, 1001, 56, 1.0);
        let (fused, score) = fuser.fuse_scored(&[f0, f1], 0.85).unwrap();

        assert_eq!(score.family_id, FamilyId::MultistaticCsi);
        assert_eq!(score.per_node_weights.len(), 2);
        assert!((score.per_node_weights.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        assert_eq!(score.capture_ns, fused.timestamp_us * 1000);
        // Identical nodes → high coherence → gate evidence present.
        assert!(score
            .evidence_refs
            .iter()
            .any(|e| matches!(e, EvidenceRef::CoherenceGateThreshold { .. })));
        assert!(score
            .evidence_refs
            .iter()
            .any(|e| matches!(e, EvidenceRef::WeightEntropy { n_nodes: 2, .. })));
        assert!(!score.forces_privacy_demotion(), "tight alignment ⇒ no demotion");
    }

    #[test]
    fn ac_fuse_scored_loose_alignment_flags_soft_contradiction() {
        use super::super::fusion_quality::ContradictionFlag;
        // Default soft_guard is now 20_000 us (#1031). A spread above soft but
        // within the 60_000 us hard guard is fused yet flagged as loose. Use a
        // 25_000 us spread: > soft (20 ms), < hard (60 ms).
        let fuser = MultistaticFuser::new();
        let f0 = make_node_frame(0, 1_000, 56, 1.0);
        let f1 = make_node_frame(1, 26_000, 56, 1.0);
        let (_fused, score) = fuser.fuse_scored(&[f0, f1], 0.85).unwrap();

        assert!(score.forces_privacy_demotion(), "loose alignment ⇒ demotion");
        assert!(matches!(
            score.contradiction_flags[0],
            ContradictionFlag::TimestampMismatch { spread_ns: 25_000_000, soft_guard_ns: 20_000_000 }
        ));
        // Penalized coherence is strictly below base when a contradiction fires.
        assert!(score.penalized_coherence() < score.base_coherence);
    }

    /// REGRESSION (issue #1031): a real 2-node TDM frame set with an 18,194 µs
    /// spread (the reported value) must FUSE under the default config — the old
    /// 5,000 µs guard rejected it with `TimestampMismatch`, silently disabling
    /// multistatic fusion on every real deployment.
    #[test]
    fn fuse_real_tdm_spread_18194us_fuses_with_default_guard() {
        let fuser = MultistaticFuser::new(); // default config
        let f0 = make_node_frame(0, 1_000, 56, 1.0);
        let f1 = make_node_frame(1, 1_000 + 18_194, 56, 1.0);
        let fused = fuser
            .fuse(&[f0, f1])
            .expect("18,194 us 2-slot spread must fuse under the #1031 default guard");
        assert_eq!(fused.active_nodes, 2, "both nodes contribute (real fusion)");
        // The 18.2 ms spread is below the soft guard (20 ms), so fuse_scored
        // records it as a CLEAN fuse (no privacy demotion) — the common case.
        let f0b = make_node_frame(0, 1_000, 56, 1.0);
        let f1b = make_node_frame(1, 1_000 + 18_194, 56, 1.0);
        let (_f, score) = fuser.fuse_scored(&[f0b, f1b], 0.85).unwrap();
        assert!(
            !score.forces_privacy_demotion(),
            "a normal 2-slot spread (18.2 ms < 20 ms soft) must NOT demote privacy"
        );
    }

    /// The guard still does its job: a spread larger than a whole TDM cycle
    /// (frames from different cycles) is rejected. Uses a tight per-deployment
    /// config derived from the schedule via `for_tdm_schedule`.
    #[test]
    fn configurable_guard_rejects_too_large_spread() {
        // 2 slots × 18 ms = 36 ms cycle → ~43 ms hard guard.
        let cfg = MultistaticConfig::for_tdm_schedule(2, 18_000);
        assert!(
            cfg.guard_interval_us >= 18_194,
            "derived guard must accept the reported 2-slot spread: {}",
            cfg.guard_interval_us
        );
        let fuser = MultistaticFuser::with_config(cfg.clone());
        // A spread well beyond a full cycle (e.g. 2× the hard guard) is rejected.
        let too_large = cfg.guard_interval_us * 2;
        let f0 = make_node_frame(0, 0, 56, 1.0);
        let f1 = make_node_frame(1, too_large, 56, 1.0);
        assert!(
            matches!(
                fuser.fuse(&[f0, f1]),
                Err(MultistaticError::TimestampMismatch { .. })
            ),
            "a spread beyond a full TDM cycle must still be rejected"
        );
    }

    /// A derived schedule must not flag its own healthy cycle as a
    /// contradiction.
    ///
    /// Every prior assertion on `soft_guard_us` was *relational* (`soft <
    /// hard`, `soft >= 1`), so nothing compared it to a spread the schedule
    /// actually produces -- which is how `cycle/3` survived. `fuse_scored`
    /// measures `max - min` across the whole cohort, i.e. `(N-1)` slots, so
    /// that is what the soft guard must cover.
    #[test]
    fn derived_soft_guard_covers_the_schedules_own_stagger() {
        for (slots, slot_us) in [
            (2_usize, 18_000_u64), // the measured 2-slot mesh in the doc comment
            (3, 27_000),
            (4, 12_500),
            (6, 33_300),
            (9, 35_600),  // a nine-node fleet: 320 ms cycle
            (13, 36_900),
        ] {
            let cfg = MultistaticConfig::for_tdm_schedule(slots, slot_us);
            // Nominal stagger; the derived guard must clear it WITH headroom,
            // because observed spread runs slightly over nominal (18,194 us
            // measured against an 18,000 us nominal 2-slot stagger).
            let stagger = (slots as u64 - 1) * slot_us;
            assert!(
                cfg.soft_guard_us > stagger,
                "{slots} slots x {slot_us} us: soft guard {} us is below the \
                 schedule's own first-to-last stagger of {stagger} us, so a \
                 complete healthy cycle is recorded as a TimestampMismatch \
                 contradiction and demotes the privacy class",
                cfg.soft_guard_us
            );
            assert!(cfg.soft_guard_us < cfg.guard_interval_us);
        }
    }

    /// End to end: a full, evenly-staggered cycle fuses with **no**
    /// contradiction under its own derived config.
    #[test]
    fn a_healthy_full_cycle_raises_no_contradiction() {
        let (slots, slot_us) = (9_usize, 35_600_u64);
        let cfg = MultistaticConfig::for_tdm_schedule(slots, slot_us);
        let fuser = MultistaticFuser::with_config(cfg.clone());

        // Node k transmits in slot k -- the schedule working exactly as designed.
        let frames: Vec<MultiBandCsiFrame> = (0..slots)
            .map(|k| make_node_frame(k as u8, k as u64 * slot_us, 56, 1.0))
            .collect();

        let (_fused, score) = fuser
            .fuse_scored(&frames, 0.0)
            .expect("a healthy cycle must fuse");
        assert!(
            !score.forces_privacy_demotion(),
            "a complete {slots}-slot cycle is the schedule operating normally \
             and must not demote the privacy class; flags: {:?}",
            score.contradiction_flags
        );
    }

    /// The soft band still does its job: a spread beyond the designed stagger
    /// is flagged.
    #[test]
    fn a_spread_beyond_the_stagger_is_still_flagged() {
        let (slots, slot_us) = (9_usize, 35_600_u64);
        let cfg = MultistaticConfig::for_tdm_schedule(slots, slot_us);
        let hard = cfg.guard_interval_us;
        let fuser = MultistaticFuser::with_config(cfg);

        // Two nodes, inside the hard guard but wider than a full cycle's stagger.
        let late = hard - 1;
        let frames = vec![
            make_node_frame(0, 0, 56, 1.0),
            make_node_frame(1, late, 56, 1.0),
        ];
        let (_f, score) = fuser.fuse_scored(&frames, 0.0).expect("within hard guard");
        assert!(
            score.forces_privacy_demotion(),
            "a spread of {late} us exceeds the designed stagger and must still \
             be recorded as loose alignment"
        );
    }

    /// The derived soft guard stays strictly below the hard guard, and a
    /// degenerate (0-slot) schedule clamps to a usable config.
    #[test]
    fn for_tdm_schedule_invariants() {
        let cfg = MultistaticConfig::for_tdm_schedule(4, 12_500); // 50 ms cycle
        assert!(cfg.soft_guard_us < cfg.guard_interval_us);
        assert!(cfg.guard_interval_us >= 50_000);
        // Degenerate input clamps instead of producing a zero/overflow guard.
        let degenerate = MultistaticConfig::for_tdm_schedule(0, 0);
        assert!(degenerate.guard_interval_us >= 1);
        assert!(degenerate.soft_guard_us >= 1);
        assert!(degenerate.soft_guard_us < degenerate.guard_interval_us.max(2));
    }

    #[test]
    fn ac_fuse_scored_calibrated_agreement_sets_id() {
        use super::super::fusion_quality::{CalibrationId, EvidenceRef};
        let fuser = MultistaticFuser::new();
        let f0 = make_node_frame(0, 1000, 56, 1.0);
        let f1 = make_node_frame(1, 1001, 56, 1.0);
        let cal = CalibrationId(0xCAFE);
        let (_f, score) = fuser
            .fuse_scored_calibrated(&[f0, f1], &[Some(cal), Some(cal)], 0.85)
            .unwrap();
        assert_eq!(score.calibration_id, Some(cal), "agreed calibration recorded");
        assert!(score
            .evidence_refs
            .iter()
            .any(|e| matches!(e, EvidenceRef::CalibrationApplied { calibration_id, .. } if *calibration_id == cal)));
        assert!(!score.forces_privacy_demotion());
    }

    #[test]
    fn ac_fuse_scored_calibration_mismatch_flags_and_nulls_id() {
        use super::super::fusion_quality::{CalibrationId, ContradictionFlag};
        let fuser = MultistaticFuser::new();
        let f0 = make_node_frame(0, 1000, 56, 1.0);
        let f1 = make_node_frame(1, 1001, 56, 1.0);
        // Two nodes, DIFFERENT calibration epochs → mismatch.
        let (_f, score) = fuser
            .fuse_scored_calibrated(&[f0, f1], &[Some(CalibrationId(1)), Some(CalibrationId(2))], 0.85)
            .unwrap();
        assert_eq!(score.calibration_id, None, "mismatch ⇒ no single calibration id");
        assert!(score
            .contradiction_flags
            .iter()
            .any(|c| matches!(c, ContradictionFlag::CalibrationIdMismatch { disagreeing: 1, .. })));
        assert!(score.forces_privacy_demotion(), "mismatch forces demotion");
    }

    #[test]
    fn ac_fuse_scored_hard_guard_still_errors() {
        // Beyond the hard guard interval, fuse_scored errors like fuse.
        let config = MultistaticConfig {
            guard_interval_us: 100,
            ..Default::default()
        };
        let fuser = MultistaticFuser::with_config(config);
        let f0 = make_node_frame(0, 0, 56, 1.0);
        let f1 = make_node_frame(1, 200, 56, 1.0);
        assert!(matches!(
            fuser.fuse_scored(&[f0, f1], 0.85),
            Err(MultistaticError::TimestampMismatch { .. })
        ));
    }

    #[test]
    fn empty_frames_error() {
        let fuser = MultistaticFuser::new();
        assert!(matches!(fuser.fuse(&[]), Err(MultistaticError::NoFrames)));
    }

    #[test]
    fn timestamp_mismatch_error() {
        let config = MultistaticConfig {
            guard_interval_us: 100,
            ..Default::default()
        };
        let fuser = MultistaticFuser::with_config(config);
        let f0 = make_node_frame(0, 0, 56, 1.0);
        let f1 = make_node_frame(1, 200, 56, 1.0);
        assert!(matches!(
            fuser.fuse(&[f0, f1]),
            Err(MultistaticError::TimestampMismatch { .. })
        ));
    }

    #[test]
    fn dimension_mismatch_error() {
        let fuser = MultistaticFuser::new();
        let f0 = make_node_frame(0, 1000, 56, 1.0);
        let f1 = make_node_frame(1, 1001, 30, 1.0);
        assert!(matches!(
            fuser.fuse(&[f0, f1]),
            Err(MultistaticError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn node_positions_set_and_retrieved() {
        // The legacy positional setter round-trips through the dense view:
        // index i in, index i out. Storage is keyed by id underneath, and
        // `positions[i]` is documented to mean node i.
        let mut fuser = MultistaticFuser::new();
        let positions = vec![[0.0, 0.0, 1.0], [3.0, 0.0, 1.0]];
        fuser.set_node_positions(positions.clone());
        assert_eq!(fuser.node_positions(), positions);
        assert_eq!(fuser.node_position(0), Some([0.0, 0.0, 1.0]));
        assert_eq!(fuser.node_position(1), Some([3.0, 0.0, 1.0]));
        assert_eq!(fuser.node_position(2), None);
    }

    #[test]
    fn fused_positions_filled() {
        let mut fuser = MultistaticFuser::new();
        fuser.set_node_positions(vec![[1.0, 2.0, 3.0]]);
        let frames = vec![
            make_node_frame(0, 100, 56, 1.0),
            make_node_frame(1, 101, 56, 1.0),
        ];
        let fused = fuser.fuse(&frames).unwrap();
        assert_eq!(fused.node_positions[0], [1.0, 2.0, 3.0]);
        assert_eq!(fused.node_positions[1], [0.0, 0.0, 0.0]); // default
    }

    /// Issue #1866 regressions: a fused contribution must carry the position
    /// belonging to its own `node_id`, under every cohort shape the live
    /// collector can produce.
    ///
    /// `node_frames_from_states_with_guard` hands `fuse` a *compacted* cohort:
    /// stale, out-of-guard and empty-history nodes are already gone. So the
    /// index of a frame is its rank in this cycle, never its identity.
    mod positions_keyed_by_node_id {
        use super::*;

        fn positions(entries: &[(u8, [f32; 3])]) -> HashMap<u8, [f32; 3]> {
            entries.iter().copied().collect()
        }

        /// Every frame gets its own node's coordinates -- the core assertion.
        fn assert_each_frame_owns_its_position(
            configured: &HashMap<u8, [f32; 3]>,
            frames: &[MultiBandCsiFrame],
        ) {
            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions_by_id(configured.clone());
            let fused = fuser.fuse(frames).expect("cohort fuses");

            assert_eq!(fused.node_positions.len(), frames.len());
            for (frame, position) in fused.node_frames.iter().zip(&fused.node_positions) {
                let expected = configured
                    .get(&frame.node_id)
                    .copied()
                    .unwrap_or([0.0, 0.0, 0.0]);
                assert_eq!(
                    *position, expected,
                    "node {} received the wrong position",
                    frame.node_id
                );
            }
        }

        #[test]
        fn sparse_node_ids_each_get_their_own_position() {
            // Real fleets use logical ids like 11/12/13. Under rank addressing
            // every one of these resolved to the origin, because indices
            // 11..13 do not exist in a 3-entry list.
            let configured = positions(&[
                (11, [1.0, 0.0, 0.0]),
                (12, [2.0, 0.0, 0.0]),
                (13, [3.0, 0.0, 0.0]),
            ]);
            let frames = vec![
                make_node_frame(11, 100, 56, 1.0),
                make_node_frame(12, 101, 56, 1.0),
                make_node_frame(13, 102, 56, 1.0),
            ];
            assert_each_frame_owns_its_position(&configured, &frames);

            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions_by_id(configured);
            let fused = fuser.fuse(&frames).expect("cohort fuses");
            assert_eq!(fused.node_positions[0], [1.0, 0.0, 0.0]);
            assert_eq!(fused.node_positions[2], [3.0, 0.0, 0.0]);
        }

        #[test]
        fn configuration_order_does_not_change_any_position() {
            // Insertion order must be irrelevant: identity is stated, not
            // inferred from where an entry sat in the config.
            let ascending = positions(&[
                (3, [3.0, 0.0, 0.0]),
                (5, [5.0, 0.0, 0.0]),
                (9, [9.0, 0.0, 0.0]),
            ]);
            let reversed = positions(&[
                (9, [9.0, 0.0, 0.0]),
                (5, [5.0, 0.0, 0.0]),
                (3, [3.0, 0.0, 0.0]),
            ]);
            let frames = vec![
                make_node_frame(3, 100, 56, 1.0),
                make_node_frame(5, 101, 56, 1.0),
                make_node_frame(9, 102, 56, 1.0),
            ];

            let mut a = MultistaticFuser::new();
            a.set_node_positions_by_id(ascending);
            let mut b = MultistaticFuser::new();
            b.set_node_positions_by_id(reversed);

            assert_eq!(
                a.fuse(&frames).expect("fuses").node_positions,
                b.fuse(&frames).expect("fuses").node_positions
            );
        }

        #[test]
        fn a_silent_node_does_not_move_the_others() {
            // THE REASON THIS IS KEYED BY ID. Node 2 misses a beacon and drops
            // out of the cohort. Under rank addressing nodes 3 and 4 shift
            // down one slot and silently inherit their neighbours' coordinates
            // -- a fleet-wide position error caused by one dropped frame.
            let configured = positions(&[
                (1, [1.0, 0.0, 0.0]),
                (2, [2.0, 0.0, 0.0]),
                (3, [3.0, 0.0, 0.0]),
                (4, [4.0, 0.0, 0.0]),
            ]);
            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions_by_id(configured);

            let all_up = vec![
                make_node_frame(1, 100, 56, 1.0),
                make_node_frame(2, 101, 56, 1.0),
                make_node_frame(3, 102, 56, 1.0),
                make_node_frame(4, 103, 56, 1.0),
            ];
            let two_silent = vec![
                make_node_frame(1, 100, 56, 1.0),
                make_node_frame(3, 102, 56, 1.0),
                make_node_frame(4, 103, 56, 1.0),
            ];

            let full = fuser.fuse(&all_up).expect("fuses");
            let gapped = fuser.fuse(&two_silent).expect("fuses");

            assert_eq!(full.node_positions[2], [3.0, 0.0, 0.0]);
            assert_eq!(full.node_positions[3], [4.0, 0.0, 0.0]);
            // Node 3 is now at rank 1 and node 4 at rank 2, but both keep
            // their own coordinates.
            assert_eq!(gapped.node_positions[0], [1.0, 0.0, 0.0]);
            assert_eq!(gapped.node_positions[1], [3.0, 0.0, 0.0]);
            assert_eq!(gapped.node_positions[2], [4.0, 0.0, 0.0]);
        }

        #[test]
        fn join_and_leave_churn_never_reassigns_a_position() {
            // Walk a node through leave -> rejoin. Every cohort shape must
            // still pair each frame with its own configured coordinates.
            let configured = positions(&[
                (0, [0.0, 0.0, 0.0]),
                (1, [1.0, 1.0, 0.0]),
                (2, [2.0, 2.0, 0.0]),
                (3, [3.0, 3.0, 0.0]),
            ]);
            let cohorts: Vec<Vec<u8>> = vec![
                vec![0, 1, 2, 3],
                vec![0, 2, 3],
                vec![2, 3],
                vec![3],
                vec![0, 3],
                vec![0, 1, 2, 3],
            ];
            for ids in cohorts {
                let frames: Vec<MultiBandCsiFrame> = ids
                    .iter()
                    .enumerate()
                    .map(|(i, &id)| make_node_frame(id, 100 + i as u64, 56, 1.0))
                    .collect();
                assert_each_frame_owns_its_position(&configured, &frames);
            }
        }

        #[test]
        fn a_configured_but_absent_node_costs_nothing() {
            // Positions may be configured for boards that are not reporting.
            // They must not consume a slot or displace anyone.
            let configured = positions(&[
                (1, [1.0, 0.0, 0.0]),
                (7, [7.0, 0.0, 0.0]),
                (42, [42.0, 0.0, 0.0]),
            ]);
            let frames = vec![
                make_node_frame(1, 100, 56, 1.0),
                make_node_frame(7, 101, 56, 1.0),
            ];
            assert_each_frame_owns_its_position(&configured, &frames);

            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions_by_id(configured);
            let fused = fuser.fuse(&frames).expect("fuses");
            assert_eq!(fused.node_positions.len(), 2);
            assert_eq!(fused.node_positions[1], [7.0, 0.0, 0.0]);
        }

        #[test]
        fn an_unconfigured_node_still_fuses_at_the_origin() {
            // Unchanged fallback behaviour: unknown node -> origin, and it
            // must not disturb its neighbours.
            let configured = positions(&[(1, [1.0, 0.0, 0.0]), (3, [3.0, 0.0, 0.0])]);
            let frames = vec![
                make_node_frame(1, 100, 56, 1.0),
                make_node_frame(2, 101, 56, 1.0),
                make_node_frame(3, 102, 56, 1.0),
            ];
            assert_each_frame_owns_its_position(&configured, &frames);

            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions_by_id(configured);
            let fused = fuser.fuse(&frames).expect("fuses");
            assert_eq!(fused.node_positions[1], [0.0, 0.0, 0.0]);
        }

        #[test]
        fn legacy_positional_setter_still_means_index_equals_node_id() {
            // Documented compatibility mapping: a bare `--node-positions`
            // list has always meant "the i-th entry is node i". Preserved
            // exactly, so existing deployments are unaffected.
            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions(vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
            ]);
            let frames = vec![
                make_node_frame(0, 100, 56, 1.0),
                make_node_frame(1, 101, 56, 1.0),
                make_node_frame(2, 102, 56, 1.0),
            ];
            let fused = fuser.fuse(&frames).expect("fuses");
            assert_eq!(fused.node_positions[0], [0.0, 0.0, 0.0]);
            assert_eq!(fused.node_positions[1], [1.0, 0.0, 0.0]);
            assert_eq!(fused.node_positions[2], [2.0, 0.0, 0.0]);
        }

        /// Negative control: the pre-fix behaviour must actually be gone.
        ///
        /// Rank addressing over a dense zero-based config only diverges from
        /// id addressing once the cohort is compacted, so this pins the exact
        /// case the old code got wrong rather than restating the happy path.
        #[test]
        fn rank_addressing_would_have_failed_this_cohort() {
            let configured = positions(&[
                (0, [0.0, 0.0, 0.0]),
                (1, [1.0, 0.0, 0.0]),
                (2, [2.0, 0.0, 0.0]),
            ]);
            let mut fuser = MultistaticFuser::new();
            fuser.set_node_positions_by_id(configured);

            // Node 1 is silent; the cohort is [0, 2].
            let frames = vec![
                make_node_frame(0, 100, 56, 1.0),
                make_node_frame(2, 101, 56, 1.0),
            ];
            let fused = fuser.fuse(&frames).expect("fuses");

            // Rank addressing would hand rank 1 the position of node 1.
            assert_ne!(fused.node_positions[1], [1.0, 0.0, 0.0]);
            assert_eq!(fused.node_positions[1], [2.0, 0.0, 0.0]);
        }
    }

    #[test]
    fn geometric_diversity_single_node() {
        assert_eq!(geometric_diversity(&[[0.0, 0.0, 0.0]]), 0.0);
    }

    #[test]
    fn geometric_diversity_two_opposite() {
        let score = geometric_diversity(&[[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        assert!(
            score > 0.8,
            "Two opposite nodes should have high diversity: {}",
            score
        );
    }

    #[test]
    fn geometric_diversity_four_corners() {
        let score = geometric_diversity(&[
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [5.0, 5.0, 0.0],
            [0.0, 5.0, 0.0],
        ]);
        assert!(
            score > 0.7,
            "Four corners should have good diversity: {}",
            score
        );
    }

    #[test]
    fn weight_coherence_uniform() {
        let weights = vec![0.25, 0.25, 0.25, 0.25];
        let c = compute_weight_coherence(&weights);
        assert!((c - 1.0).abs() < 0.01);
    }

    #[test]
    fn weight_coherence_single_dominant() {
        let weights = vec![0.97, 0.01, 0.01, 0.01];
        let c = compute_weight_coherence(&weights);
        assert!(
            c < 0.3,
            "Single dominant node should have low coherence: {}",
            c
        );
    }

    #[test]
    fn default_config() {
        let cfg = MultistaticConfig::default();
        // #1031: hard guard raised to 60 ms (was 5 ms) to accommodate the real
        // TDM slot offset; soft guard 20 ms, both strictly ordered.
        assert_eq!(cfg.guard_interval_us, 60_000);
        assert_eq!(cfg.soft_guard_us, 20_000);
        assert!(cfg.soft_guard_us < cfg.guard_interval_us);
        assert_eq!(cfg.min_nodes, 2);
        assert!((cfg.attention_temperature - 1.0).abs() < f32::EPSILON);
        assert!(cfg.enable_person_separation);
    }

    #[test]
    fn person_cluster_creation() {
        let cluster = PersonCluster {
            id: 0,
            link_indices: vec![0, 1, 3],
            intra_correlation: 0.85,
        };
        assert_eq!(cluster.link_indices.len(), 3);
    }

    // -----------------------------------------------------------------------
    // ADR-154: CIR coherence gate regression tests (headline anti-slop fix).
    //
    // Before the fix, `with_cir_ht20()` built a 52-active Φ, so every
    // canonical-56 frame returned `SubcarrierMismatch` and the gate silently
    // degraded to frequency-domain coherence (100% Err, blend never applied).
    // After the fix, `with_cir_canonical56()` runs on canonical-56 frames.
    // -----------------------------------------------------------------------

    /// Build a deterministic canonical-56 stream with sanitized (small) phase
    /// so the CIR estimator's ghost-tap guard does not trip.
    fn canonical56_stream(n: usize) -> Vec<MultiBandCsiFrame> {
        (0..n)
            .map(|i| make_node_frame(i as u8, 1000 + i as u64, 56, 1.0 + 0.05 * i as f32))
            .collect()
    }

    /// PROOF (ADR-154): the old ht20 estimator is DEAD on canonical-56 frames —
    /// 100% of `estimate()` calls return `SubcarrierMismatch`.
    #[test]
    fn cir_gate_ht20_is_dead_on_canonical56() {
        let fuser = MultistaticFuser::with_cir_ht20();
        let frames = canonical56_stream(8);
        let mut ok = 0;
        let mut err_mismatch = 0;
        for f in &frames {
            match fuser.cir_estimate_first(std::slice::from_ref(f)) {
                Some(Ok(_)) => ok += 1,
                Some(Err(super::super::cir::CirError::SubcarrierMismatch { .. })) => {
                    err_mismatch += 1
                }
                other => panic!("unexpected estimate result: {other:?}"),
            }
        }
        assert_eq!(ok, 0, "ht20 estimator must NOT decode canonical-56 frames");
        assert_eq!(
            err_mismatch, 8,
            "every canonical-56 frame must hit SubcarrierMismatch under ht20 (dead gate)"
        );
    }

    /// PROOF (ADR-154): after the fix, the canonical-56 estimator decodes every
    /// frame (0% Err) — the gate is alive.
    #[test]
    fn cir_gate_canonical56_is_alive() {
        let fuser = MultistaticFuser::with_cir_canonical56();
        let frames = canonical56_stream(8);
        let mut ok = 0;
        let mut err = 0;
        for f in &frames {
            match fuser.cir_estimate_first(std::slice::from_ref(f)) {
                Some(Ok(_)) => ok += 1,
                Some(Err(_)) => err += 1,
                None => panic!("gate disabled unexpectedly"),
            }
        }
        assert_eq!(err, 0, "canonical-56 estimator must decode every frame");
        assert_eq!(ok, 8, "all 8 canonical-56 frames must produce a CIR");
    }

    /// PROOF (ADR-154): with the live gate, the blended coherence differs from
    /// the gate-off (frequency-domain only) coherence — the CIR term is applied.
    #[test]
    fn cir_gate_on_changes_coherence_vs_off() {
        let frames = canonical56_stream(4);

        // Gate ON, canonical-56 estimator (alive).
        let on = MultistaticFuser::with_cir_canonical56();
        let coh_on = on.fuse(&frames).unwrap().cross_node_coherence;

        // Gate OFF: same frames, CIR path disabled → pure freq-domain coherence.
        let off = MultistaticFuser::with_config(MultistaticConfig {
            use_cir_gate: false,
            ..Default::default()
        });
        let coh_off = off.fuse(&frames).unwrap().cross_node_coherence;

        assert!(
            (coh_on - coh_off).abs() > 1e-6,
            "live CIR gate must change coherence: on={coh_on} off={coh_off}"
        );
    }

    /// PROOF (ADR-154): the dead ht20 gate is indistinguishable from gate-off —
    /// confirming the silent degradation the fix eliminates. (debug_assert is
    /// disabled here via release-style check: we call the coherence path which
    /// only debug-asserts; this test asserts the *numeric* degeneracy and is
    /// gated to release to avoid the intentional debug panic.)
    #[test]
    #[cfg(not(debug_assertions))]
    fn cir_gate_dead_ht20_equals_gate_off() {
        let frames = canonical56_stream(4);
        let dead = MultistaticFuser::with_cir_ht20();
        let coh_dead = dead.fuse(&frames).unwrap().cross_node_coherence;
        let off = MultistaticFuser::with_config(MultistaticConfig {
            use_cir_gate: false,
            ..Default::default()
        });
        let coh_off = off.fuse(&frames).unwrap().cross_node_coherence;
        assert!(
            (coh_dead - coh_off).abs() < 1e-9,
            "dead ht20 gate silently equals gate-off: dead={coh_dead} off={coh_off}"
        );
    }

    // ===== Issue #1752: non-coherent phase gate =====

    #[test]
    fn placeholder_phase_is_never_combined() {
        let fuser = MultistaticFuser::new();
        let frames = [
            with_reference(make_node_frame(0, 1000, 56, 1.0), PhaseReference::Unavailable),
            with_reference(make_node_frame(1, 1001, 56, 1.1), PhaseReference::Unavailable),
        ];
        let fused = fuser.fuse(&frames).unwrap();
        assert_eq!(
            fused.phase_fusion,
            PhaseFusion::AmplitudeOnly(NonCoherentReason::PhaseUnavailable)
        );
        assert!(fused.fused_phase.is_empty(), "placeholder phase must not be fused");
        assert_eq!(fused.fused_amplitude.len(), 56);
    }

    #[test]
    fn independent_node_clocks_fall_back_to_amplitude_only() {
        let fuser = MultistaticFuser::new();
        let frames = [make_node_frame(0, 1000, 56, 1.0), make_node_frame(1, 1001, 56, 1.1)];
        let fused = fuser.fuse(&frames).unwrap();
        assert_eq!(
            fused.phase_fusion,
            PhaseFusion::AmplitudeOnly(NonCoherentReason::IndependentClocks)
        );
        assert_eq!(fused.phase_fusion.reason().map(NonCoherentReason::as_str), Some("independent_clocks"));
        assert!(fused.fused_phase.is_empty());
    }

    #[test]
    fn one_placeholder_node_demotes_the_whole_cohort() {
        let shared = PhaseReference::Shared { id: 7 };
        let frames = [
            with_reference(make_node_frame(0, 1000, 56, 1.0), shared),
            with_reference(make_node_frame(1, 1001, 56, 1.0), PhaseReference::Unavailable),
        ];
        assert_eq!(
            PhaseFusion::for_frames(&frames),
            PhaseFusion::AmplitudeOnly(NonCoherentReason::PhaseUnavailable)
        );
    }

    #[test]
    fn mixed_shared_references_are_not_coherent() {
        let frames = [
            with_reference(make_node_frame(0, 1000, 56, 1.0), PhaseReference::Shared { id: 1 }),
            with_reference(make_node_frame(1, 1001, 56, 1.0), PhaseReference::Shared { id: 2 }),
        ];
        assert_eq!(
            PhaseFusion::for_frames(&frames),
            PhaseFusion::AmplitudeOnly(NonCoherentReason::MixedReferences)
        );
    }

    /// Opt-in path for a future shared-clock source (for example a coherent
    /// multi-radio array): frames that declare one shared reference are
    /// combined coherently and produce a phase vector.
    #[test]
    fn shared_reference_source_opts_into_coherent_fusion() {
        let fuser = MultistaticFuser::new();
        let shared = PhaseReference::Shared { id: 42 };
        let frames: Vec<MultiBandCsiFrame> = (0..3)
            .map(|i| with_reference(make_node_frame(i, 1000 + u64::from(i), 56, 1.0), shared))
            .collect();
        let fused = fuser.fuse(&frames).unwrap();
        assert_eq!(fused.phase_fusion, PhaseFusion::Coherent { reference_id: 42 });
        assert_eq!(fused.fused_phase.len(), 56);
        // Identical phases under one reference fuse back to themselves.
        let expected = &frames[0].channel_frames[0].phase;
        for (got, want) in fused.fused_phase.iter().zip(expected) {
            assert!((got - want).abs() < 1e-5, "{got} vs {want}");
        }
    }

    #[test]
    fn single_node_passes_measured_phase_through() {
        let fuser = MultistaticFuser::new();
        let f = make_node_frame(0, 1000, 56, 1.0);
        let fused = fuser.fuse(std::slice::from_ref(&f)).unwrap();
        assert_eq!(fused.phase_fusion, PhaseFusion::SingleNode);
        assert_eq!(fused.fused_phase, f.channel_frames[0].phase);
    }

    #[test]
    fn amplitude_fusion_is_unchanged_by_the_phase_gate() {
        let fuser = MultistaticFuser::new();
        let local = [make_node_frame(0, 1000, 56, 1.0), make_node_frame(1, 1001, 56, 1.3)];
        let shared: Vec<MultiBandCsiFrame> = local
            .iter()
            .cloned()
            .map(|f| with_reference(f, PhaseReference::Shared { id: 1 }))
            .collect();
        let a = fuser.fuse(&local).unwrap();
        let b = fuser.fuse(&shared).unwrap();
        assert_eq!(a.fused_amplitude, b.fused_amplitude);
        assert_eq!(a.cross_node_coherence, b.cross_node_coherence);
    }

    #[test]
    fn cir_gate_ignores_placeholder_phase() {
        let on = MultistaticFuser::with_cir_canonical56();
        let off = MultistaticFuser::new();
        let frames = [
            with_reference(make_node_frame(0, 1000, 56, 1.0), PhaseReference::Unavailable),
            with_reference(make_node_frame(1, 1001, 56, 1.2), PhaseReference::Unavailable),
        ];
        let coh_on = on.fuse(&frames).unwrap().cross_node_coherence;
        let coh_off = off.fuse(&frames).unwrap().cross_node_coherence;
        assert_eq!(coh_on, coh_off, "no CIR blend from a placeholder phase vector");
    }

    #[test]
    fn cir_gate_skips_when_later_frame_has_placeholder_phase() {
        let on = MultistaticFuser::with_cir_canonical56();
        let off = MultistaticFuser::new();
        let frames = [
            with_reference(make_node_frame(0, 1000, 56, 1.0), PhaseReference::NodeLocal),
            with_reference(make_node_frame(1, 1001, 56, 1.2), PhaseReference::Unavailable),
        ];
        let coh_on = on.fuse(&frames).unwrap().cross_node_coherence;
        let coh_off = off.fuse(&frames).unwrap().cross_node_coherence;
        assert_eq!(coh_on, coh_off, "no CIR blend when any cohort frame lacks phase");
    }
}
