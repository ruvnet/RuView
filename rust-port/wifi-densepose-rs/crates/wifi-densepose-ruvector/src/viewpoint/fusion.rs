//! MultistaticArray aggregate root and fusion pipeline orchestrator (ADR-031).
//!
//! [`MultistaticArray`] is the DDD aggregate root for the ViewpointFusion
//! bounded context. It orchestrates the full fusion pipeline:
//!
//! 1. Collect per-viewpoint AETHER embeddings.
//! 2. Compute geometric bias from viewpoint pair geometry.
//! 3. Apply cross-viewpoint attention with geometric bias.
//! 4. Gate the output through coherence check.
//! 5. Emit a fused embedding for the DensePose regression head.
//!
//! Uses `ruvector-attention` for the attention mechanism and
//! `ruvector-attn-mincut` for optional noise gating on embeddings.

use crate::viewpoint::attention::{
    AttentionError, CrossViewpointAttention, GeometricBias, ViewpointGeometry,
};
use crate::viewpoint::coherence::{CoherenceGate, CoherenceState};
use crate::viewpoint::geometry::{GeometricDiversityIndex, NodeId};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Unique identifier for a multistatic array deployment.
pub type ArrayId = u64;

/// Per-viewpoint embedding with geometric metadata.
///
/// Represents a single CSI observation processed through the per-viewpoint
/// signal pipeline and AETHER encoder into a contrastive embedding.
#[derive(Debug, Clone)]
pub struct ViewpointEmbedding {
    /// Source node identifier.
    pub node_id: NodeId,
    /// AETHER embedding vector (typically 128-d).
    pub embedding: Vec<f32>,
    /// Azimuth angle from array centroid (radians).
    pub azimuth: f32,
    /// Elevation angle (radians, 0 for 2-D deployments).
    pub elevation: f32,
    /// Baseline distance from array centroid (metres).
    pub baseline: f32,
    /// Node position in metres (x, y).
    pub position: (f32, f32),
    /// Signal-to-noise ratio at capture time (dB).
    pub snr_db: f32,
}

/// Fused embedding output from the cross-viewpoint attention pipeline.
#[derive(Debug, Clone)]
pub struct FusedEmbedding {
    /// The fused embedding vector.
    pub embedding: Vec<f32>,
    /// Geometric Diversity Index at the time of fusion.
    pub gdi: f32,
    /// Coherence value at the time of fusion.
    pub coherence: f32,
    /// Number of viewpoints that contributed to the fusion.
    pub n_viewpoints: usize,
    /// Effective independent viewpoints (after correlation discount).
    pub n_effective: f32,
}

/// Configuration for the fusion pipeline.
#[derive(Debug, Clone)]
pub struct FusionConfig {
    /// Embedding dimension (must match AETHER output, typically 128).
    pub embed_dim: usize,
    /// Coherence threshold for gating (typically 0.7).
    pub coherence_threshold: f32,
    /// Coherence hysteresis band (typically 0.05).
    pub coherence_hysteresis: f32,
    /// Coherence rolling window size (number of frames).
    pub coherence_window: usize,
    /// Geometric bias angle weight.
    pub w_angle: f32,
    /// Geometric bias distance weight.
    pub w_dist: f32,
    /// Reference distance for geometric bias decay (metres).
    pub d_ref: f32,
    /// Minimum SNR (dB) for a viewpoint to contribute to fusion.
    pub min_snr_db: f32,
}

impl Default for FusionConfig {
    fn default() -> Self {
        FusionConfig {
            embed_dim: 128,
            coherence_threshold: 0.7,
            coherence_hysteresis: 0.05,
            coherence_window: 50,
            w_angle: 1.0,
            w_dist: 1.0,
            d_ref: 5.0,
            min_snr_db: 5.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Fusion errors
// ---------------------------------------------------------------------------

/// Errors produced by the fusion pipeline.
#[derive(Debug, Clone)]
pub enum FusionError {
    /// No viewpoint embeddings available for fusion.
    NoViewpoints,
    /// All viewpoints were filtered out (e.g. by SNR threshold).
    AllFiltered {
        /// Number of viewpoints that were rejected.
        rejected: usize,
    },
    /// Coherence gate is closed (environment too unstable).
    CoherenceGateClosed {
        /// Current coherence value.
        coherence: f32,
        /// Required threshold.
        threshold: f32,
    },
    /// Internal attention computation error.
    AttentionError(AttentionError),
    /// Embedding dimension mismatch.
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
        /// Node that produced the mismatched embedding.
        node_id: NodeId,
    },
}

impl std::fmt::Display for FusionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FusionError::NoViewpoints => write!(f, "no viewpoint embeddings available"),
            FusionError::AllFiltered { rejected } => {
                write!(f, "all {rejected} viewpoints filtered by SNR threshold")
            }
            FusionError::CoherenceGateClosed { coherence, threshold } => {
                write!(
                    f,
                    "coherence gate closed: coherence={coherence:.3} < threshold={threshold:.3}"
                )
            }
            FusionError::AttentionError(e) => write!(f, "attention error: {e}"),
            FusionError::DimensionMismatch { expected, actual, node_id } => {
                write!(
                    f,
                    "node {node_id} embedding dim {actual} != expected {expected}"
                )
            }
        }
    }
}

impl std::error::Error for FusionError {}

impl From<AttentionError> for FusionError {
    fn from(e: AttentionError) -> Self {
        FusionError::AttentionError(e)
    }
}

// ---------------------------------------------------------------------------
// Domain events
// ---------------------------------------------------------------------------

/// Events emitted by the ViewpointFusion aggregate.
#[derive(Debug, Clone)]
pub enum ViewpointFusionEvent {
    /// A viewpoint embedding was received from a node.
    ViewpointCaptured {
        /// Source node.
        node_id: NodeId,
        /// Signal quality.
        snr_db: f32,
    },
    /// A TDM cycle completed with all (or some) viewpoints received.
    TdmCycleCompleted {
        /// Monotonic cycle counter.
        cycle_id: u64,
        /// Number of viewpoints received this cycle.
        viewpoints_received: usize,
    },
    /// Fusion completed successfully.
    FusionCompleted {
        /// GDI at the time of fusion.
        gdi: f32,
        /// Number of viewpoints fused.
        n_viewpoints: usize,
    },
    /// Coherence gate evaluation result.
    CoherenceGateTriggered {
        /// Current coherence value.
        coherence: f32,
        /// Whether the gate accepted the update.
        accepted: bool,
    },
    /// Array geometry was updated.
    GeometryUpdated {
        /// New GDI value.
        new_gdi: f32,
        /// Effective independent viewpoints.
        n_effective: f32,
    },
}

// ---------------------------------------------------------------------------
// MultistaticArray (aggregate root)
// ---------------------------------------------------------------------------

/// Aggregate root for the ViewpointFusion bounded context.
///
/// Manages the lifecycle of a multistatic sensor array: collecting viewpoint
/// embeddings, computing geometric diversity, gating on coherence, and
/// producing fused embeddings for downstream pose estimation.
pub struct MultistaticArray {
    /// Unique deployment identifier.
    id: ArrayId,
    /// Active viewpoint embeddings (latest per node).
    viewpoints: Vec<ViewpointEmbedding>,
    /// Cross-viewpoint attention module.
    attention: CrossViewpointAttention,
    /// Coherence state tracker.
    coherence_state: CoherenceState,
    /// Coherence gate.
    coherence_gate: CoherenceGate,
    /// Pipeline configuration.
    config: FusionConfig,
    /// Monotonic TDM cycle counter.
    cycle_count: u64,
    /// Event log (bounded).
    events: Vec<ViewpointFusionEvent>,
    /// Maximum events to retain.
    max_events: usize,
}

impl MultistaticArray {
    /// Create a new multistatic array with the given configuration.
    pub fn new(id: ArrayId, config: FusionConfig) -> Self {
        let attention = CrossViewpointAttention::new(config.embed_dim);
        let attention = CrossViewpointAttention::with_params(
            attention.weights,
            GeometricBias::new(config.w_angle, config.w_dist, config.d_ref),
        );
        let coherence_state = CoherenceState::new(config.coherence_window);
        let coherence_gate =
            CoherenceGate::new(config.coherence_threshold, config.coherence_hysteresis);

        MultistaticArray {
            id,
            viewpoints: Vec::new(),
            attention,
            coherence_state,
            coherence_gate,
            config,
            cycle_count: 0,
            events: Vec::new(),
            max_events: 1000,
        }
    }

    /// Create with default configuration.
    pub fn with_defaults(id: ArrayId) -> Self {
        Self::new(id, FusionConfig::default())
    }

    /// Array deployment identifier.
    pub fn id(&self) -> ArrayId {
        self.id
    }

    /// Number of viewpoints currently held.
    pub fn n_viewpoints(&self) -> usize {
        self.viewpoints.len()
    }

    /// Current TDM cycle count.
    pub fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    /// Submit a viewpoint embedding from a sensor node.
    ///
    /// Replaces any existing embedding for the same `node_id`.
    pub fn submit_viewpoint(&mut self, vp: ViewpointEmbedding) -> Result<(), FusionError> {
        // Validate embedding dimension.
        if vp.embedding.len() != self.config.embed_dim {
            return Err(FusionError::DimensionMismatch {
                expected: self.config.embed_dim,
                actual: vp.embedding.len(),
                node_id: vp.node_id,
            });
        }

        self.emit_event(ViewpointFusionEvent::ViewpointCaptured {
            node_id: vp.node_id,
            snr_db: vp.snr_db,
        });

        // Upsert: replace existing embedding for this node.
        if let Some(pos) = self.viewpoints.iter().position(|v| v.node_id == vp.node_id) {
            self.viewpoints[pos] = vp;
        } else {
            self.viewpoints.push(vp);
        }

        Ok(())
    }

    /// Push a phase-difference measurement for coherence tracking.
    pub fn push_phase_diff(&mut self, phase_diff: f32) {
        self.coherence_state.push(phase_diff);
    }

    /// Current coherence value.
    pub fn coherence(&self) -> f32 {
        self.coherence_state.coherence()
    }

    /// Compute the Geometric Diversity Index for the current array layout.
    pub fn compute_gdi(&self) -> Option<GeometricDiversityIndex> {
        let azimuths: Vec<f32> = self.viewpoints.iter().map(|v| v.azimuth).collect();
        let ids: Vec<NodeId> = self.viewpoints.iter().map(|v| v.node_id).collect();
        let gdi = GeometricDiversityIndex::compute(&azimuths, &ids);
        if let Some(ref g) = gdi {
            // Emit event (mutable borrow not possible here, caller can do it).
            let _ = g; // used for return
        }
        gdi
    }

    /// Run the full fusion pipeline.
    ///
    /// 1. Filter viewpoints by SNR.
    /// 2. Check coherence gate.
    /// 3. Compute geometric bias.
    /// 4. Apply cross-viewpoint attention.
    /// 5. Mean-pool to single fused embedding.
    ///
    /// # Returns
    ///
    /// `Ok(FusedEmbedding)` on success, or an error if the pipeline cannot
    /// produce a valid fusion (no viewpoints, gate closed, etc.).
    pub fn fuse(&mut self) -> Result<FusedEmbedding, FusionError> {
        self.cycle_count += 1;

        // Extract all needed data from viewpoints upfront to avoid borrow conflicts.
        let min_snr = self.config.min_snr_db;
        let total_viewpoints = self.viewpoints.len();
        let extracted: Vec<(NodeId, Vec<f32>, f32, (f32, f32))> = self
            .viewpoints
            .iter()
            .filter(|v| v.snr_db >= min_snr)
            .map(|v| (v.node_id, v.embedding.clone(), v.azimuth, v.position))
            .collect();

        let n_valid = extracted.len();
        if n_valid == 0 {
            if total_viewpoints == 0 {
                return Err(FusionError::NoViewpoints);
            }
            return Err(FusionError::AllFiltered {
                rejected: total_viewpoints,
            });
        }

        // Check coherence gate.
        let coh = self.coherence_state.coherence();
        let gate_open = self.coherence_gate.evaluate(coh);

        self.emit_event(ViewpointFusionEvent::CoherenceGateTriggered {
            coherence: coh,
            accepted: gate_open,
        });

        if !gate_open {
            return Err(FusionError::CoherenceGateClosed {
                coherence: coh,
                threshold: self.config.coherence_threshold,
            });
        }

        // Prepare embeddings and geometries from extracted data.
        let embeddings: Vec<Vec<f32>> = extracted.iter().map(|(_, e, _, _)| e.clone()).collect();
        let geom: Vec<ViewpointGeometry> = extracted
            .iter()
            .map(|(_, _, az, pos)| ViewpointGeometry {
                azimuth: *az,
                position: *pos,
            })
            .collect();

        // Run cross-viewpoint attention fusion.
        let fused_emb = self.attention.fuse(&embeddings, &geom)?;

        // Compute GDI.
        let azimuths: Vec<f32> = extracted.iter().map(|(_, _, az, _)| *az).collect();
        let ids: Vec<NodeId> = extracted.iter().map(|(id, _, _, _)| *id).collect();
        let gdi_opt = GeometricDiversityIndex::compute(&azimuths, &ids);
        let (gdi_val, n_eff) = match &gdi_opt {
            Some(g) => (g.value, g.n_effective),
            None => (0.0, n_valid as f32),
        };

        self.emit_event(ViewpointFusionEvent::TdmCycleCompleted {
            cycle_id: self.cycle_count,
            viewpoints_received: n_valid,
        });

        self.emit_event(ViewpointFusionEvent::FusionCompleted {
            gdi: gdi_val,
            n_viewpoints: n_valid,
        });

        Ok(FusedEmbedding {
            embedding: fused_emb,
            gdi: gdi_val,
            coherence: coh,
            n_viewpoints: n_valid,
            n_effective: n_eff,
        })
    }

    /// Run fusion without coherence gating (for testing or forced updates).
    pub fn fuse_ungated(&mut self) -> Result<FusedEmbedding, FusionError> {
        let min_snr = self.config.min_snr_db;
        let total_viewpoints = self.viewpoints.len();
        let extracted: Vec<(NodeId, Vec<f32>, f32, (f32, f32))> = self
            .viewpoints
            .iter()
            .filter(|v| v.snr_db >= min_snr)
            .map(|v| (v.node_id, v.embedding.clone(), v.azimuth, v.position))
            .collect();

        let n_valid = extracted.len();
        if n_valid == 0 {
            if total_viewpoints == 0 {
                return Err(FusionError::NoViewpoints);
            }
            return Err(FusionError::AllFiltered {
                rejected: total_viewpoints,
            });
        }

        let embeddings: Vec<Vec<f32>> = extracted.iter().map(|(_, e, _, _)| e.clone()).collect();
        let geom: Vec<ViewpointGeometry> = extracted
            .iter()
            .map(|(_, _, az, pos)| ViewpointGeometry {
                azimuth: *az,
                position: *pos,
            })
            .collect();

        let fused_emb = self.attention.fuse(&embeddings, &geom)?;

        let azimuths: Vec<f32> = extracted.iter().map(|(_, _, az, _)| *az).collect();
        let ids: Vec<NodeId> = extracted.iter().map(|(id, _, _, _)| *id).collect();
        let gdi_opt = GeometricDiversityIndex::compute(&azimuths, &ids);
        let (gdi_val, n_eff) = match &gdi_opt {
            Some(g) => (g.value, g.n_effective),
            None => (0.0, n_valid as f32),
        };

        let coh = self.coherence_state.coherence();

        Ok(FusedEmbedding {
            embedding: fused_emb,
            gdi: gdi_val,
            coherence: coh,
            n_viewpoints: n_valid,
            n_effective: n_eff,
        })
    }

    /// Access the event log.
    pub fn events(&self) -> &[ViewpointFusionEvent] {
        &self.events
    }

    /// Clear the event log.
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Remove a viewpoint by node ID.
    pub fn remove_viewpoint(&mut self, node_id: NodeId) {
        self.viewpoints.retain(|v| v.node_id != node_id);
    }

    /// Clear all viewpoints.
    pub fn clear_viewpoints(&mut self) {
        self.viewpoints.clear();
    }

    fn emit_event(&mut self, event: ViewpointFusionEvent) {
        if self.events.len() >= self.max_events {
            // Drop oldest half to avoid unbounded growth.
            let half = self.max_events / 2;
            self.events.drain(..half);
        }
        self.events.push(event);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_viewpoint(node_id: NodeId, angle_idx: usize, n: usize, dim: usize) -> ViewpointEmbedding {
        let angle = 2.0 * std::f32::consts::PI * angle_idx as f32 / n as f32;
        let r = 3.0;
        ViewpointEmbedding {
            node_id,
            embedding: (0..dim).map(|d| ((node_id as usize * dim + d) as f32 * 0.01).sin()).collect(),
            azimuth: angle,
            elevation: 0.0,
            baseline: r,
            position: (r * angle.cos(), r * angle.sin()),
            snr_db: 15.0,
        }
    }

    fn setup_coherent_array(dim: usize) -> MultistaticArray {
        let config = FusionConfig {
            embed_dim: dim,
            coherence_threshold: 0.5,
            coherence_hysteresis: 0.0,
            min_snr_db: 0.0,
            ..FusionConfig::default()
        };
        let mut array = MultistaticArray::new(1, config);
        // Push coherent phase diffs to open the gate.
        for _ in 0..60 {
            array.push_phase_diff(0.1);
        }
        array
    }

    #[test]
    fn fuse_produces_correct_dimension() {
        let dim = 16;
        let mut array = setup_coherent_array(dim);
        for i in 0..4 {
            array.submit_viewpoint(make_viewpoint(i, i as usize, 4, dim)).unwrap();
        }
        let fused = array.fuse().unwrap();
        assert_eq!(fused.embedding.len(), dim);
        assert_eq!(fused.n_viewpoints, 4);
    }

    #[test]
    fn fuse_no_viewpoints_returns_error() {
        let mut array = setup_coherent_array(16);
        assert!(matches!(array.fuse(), Err(FusionError::NoViewpoints)));
    }

    #[test]
    fn fuse_coherence_gate_closed_returns_error() {
        let dim = 16;
        let config = FusionConfig {
            embed_dim: dim,
            coherence_threshold: 0.9,
            coherence_hysteresis: 0.0,
            min_snr_db: 0.0,
            ..FusionConfig::default()
        };
        let mut array = MultistaticArray::new(1, config);
        // Push incoherent phase diffs.
        for i in 0..100 {
            array.push_phase_diff(i as f32 * 0.5);
        }
        array.submit_viewpoint(make_viewpoint(0, 0, 4, dim)).unwrap();
        array.submit_viewpoint(make_viewpoint(1, 1, 4, dim)).unwrap();
        let result = array.fuse();
        assert!(matches!(result, Err(FusionError::CoherenceGateClosed { .. })));
    }

    #[test]
    fn fuse_ungated_bypasses_coherence() {
        let dim = 16;
        let config = FusionConfig {
            embed_dim: dim,
            coherence_threshold: 0.99,
            coherence_hysteresis: 0.0,
            min_snr_db: 0.0,
            ..FusionConfig::default()
        };
        let mut array = MultistaticArray::new(1, config);
        // Push incoherent diffs -- gate would be closed.
        for i in 0..100 {
            array.push_phase_diff(i as f32 * 0.5);
        }
        array.submit_viewpoint(make_viewpoint(0, 0, 4, dim)).unwrap();
        array.submit_viewpoint(make_viewpoint(1, 1, 4, dim)).unwrap();
        let fused = array.fuse_ungated().unwrap();
        assert_eq!(fused.embedding.len(), dim);
    }

    #[test]
    fn submit_replaces_existing_viewpoint() {
        let dim = 8;
        let mut array = setup_coherent_array(dim);
        let vp1 = make_viewpoint(10, 0, 4, dim);
        let mut vp2 = make_viewpoint(10, 1, 4, dim);
        vp2.snr_db = 25.0;
        array.submit_viewpoint(vp1).unwrap();
        assert_eq!(array.n_viewpoints(), 1);
        array.submit_viewpoint(vp2).unwrap();
        assert_eq!(array.n_viewpoints(), 1, "should replace, not add");
    }

    #[test]
    fn dimension_mismatch_returns_error() {
        let dim = 16;
        let mut array = setup_coherent_array(dim);
        let mut vp = make_viewpoint(0, 0, 4, dim);
        vp.embedding = vec![1.0; 8]; // wrong dim
        assert!(matches!(
            array.submit_viewpoint(vp),
            Err(FusionError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn snr_filter_rejects_low_quality() {
        let dim = 16;
        let config = FusionConfig {
            embed_dim: dim,
            coherence_threshold: 0.0,
            min_snr_db: 10.0,
            ..FusionConfig::default()
        };
        let mut array = MultistaticArray::new(1, config);
        for _ in 0..60 {
            array.push_phase_diff(0.1);
        }
        let mut vp = make_viewpoint(0, 0, 4, dim);
        vp.snr_db = 3.0; // below threshold
        array.submit_viewpoint(vp).unwrap();
        assert!(matches!(array.fuse(), Err(FusionError::AllFiltered { .. })));
    }

    #[test]
    fn events_are_emitted_on_fusion() {
        let dim = 8;
        let mut array = setup_coherent_array(dim);
        array.submit_viewpoint(make_viewpoint(0, 0, 4, dim)).unwrap();
        array.submit_viewpoint(make_viewpoint(1, 1, 4, dim)).unwrap();
        array.clear_events();
        let _ = array.fuse();
        assert!(!array.events().is_empty(), "fusion should emit events");
    }

    #[test]
    fn remove_viewpoint_works() {
        let dim = 8;
        let mut array = setup_coherent_array(dim);
        array.submit_viewpoint(make_viewpoint(10, 0, 4, dim)).unwrap();
        array.submit_viewpoint(make_viewpoint(20, 1, 4, dim)).unwrap();
        assert_eq!(array.n_viewpoints(), 2);
        array.remove_viewpoint(10);
        assert_eq!(array.n_viewpoints(), 1);
    }

    #[test]
    fn fused_embedding_reports_gdi() {
        let dim = 16;
        let mut array = setup_coherent_array(dim);
        for i in 0..4 {
            array.submit_viewpoint(make_viewpoint(i, i as usize, 4, dim)).unwrap();
        }
        let fused = array.fuse().unwrap();
        assert!(fused.gdi > 0.0, "GDI should be positive for spread viewpoints");
        assert!(fused.n_effective > 1.0, "effective viewpoints should be > 1");
    }

    #[test]
    fn compute_gdi_standalone() {
        let dim = 8;
        let mut array = setup_coherent_array(dim);
        for i in 0..6 {
            array.submit_viewpoint(make_viewpoint(i, i as usize, 6, dim)).unwrap();
        }
        let gdi = array.compute_gdi().unwrap();
        assert!(gdi.value > 0.0);
        assert!(gdi.n_effective > 1.0);
    }
}
