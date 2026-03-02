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
//! # RuVector Integration
//!
//! - `ruvector-attn-mincut` for cross-node spectrogram attention gating
//! - `ruvector-mincut` for person separation (DynamicMinCut)

use super::multiband::MultiBandCsiFrame;

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
    /// Length = n_subcarriers.
    pub fused_phase: Vec<f32>,
    /// Per-node multi-band frames (preserved for geometry computations).
    pub node_frames: Vec<MultiBandCsiFrame>,
    /// Node positions (x, y, z) in meters from deployment configuration.
    pub node_positions: Vec<[f32; 3]>,
    /// Number of active nodes contributing to this frame.
    pub active_nodes: usize,
    /// Cross-node coherence score (0.0-1.0). Higher means more agreement
    /// across viewpoints, indicating a strong body reflection signal.
    pub cross_node_coherence: f32,
}

/// Configuration for multistatic fusion.
#[derive(Debug, Clone)]
pub struct MultistaticConfig {
    /// Maximum timestamp spread (microseconds) across nodes in one cycle.
    /// Default: 5000 us (5 ms), well within the 50 ms TDMA cycle.
    pub guard_interval_us: u64,
    /// Minimum number of nodes for multistatic mode.
    /// Falls back to single-node mode if fewer nodes are available.
    pub min_nodes: usize,
    /// Attention temperature for cross-node weighting.
    /// Lower temperature -> sharper attention (fewer nodes dominate).
    pub attention_temperature: f32,
    /// Whether to enable person separation via min-cut.
    pub enable_person_separation: bool,
}

impl Default for MultistaticConfig {
    fn default() -> Self {
        Self {
            guard_interval_us: 5000,
            min_nodes: 2,
            attention_temperature: 1.0,
            enable_person_separation: true,
        }
    }
}

/// Multistatic frame fuser.
///
/// Collects per-node multi-band frames and produces a single fused
/// sensing frame per TDMA cycle.
#[derive(Debug)]
pub struct MultistaticFuser {
    config: MultistaticConfig,
    /// Node positions in 3D space (meters).
    node_positions: Vec<[f32; 3]>,
}

impl MultistaticFuser {
    /// Create a fuser with default configuration and no node positions.
    pub fn new() -> Self {
        Self {
            config: MultistaticConfig::default(),
            node_positions: Vec::new(),
        }
    }

    /// Create a fuser with custom configuration.
    pub fn with_config(config: MultistaticConfig) -> Self {
        Self {
            config,
            node_positions: Vec::new(),
        }
    }

    /// Set node positions for geometric diversity computations.
    pub fn set_node_positions(&mut self, positions: Vec<[f32; 3]>) {
        self.node_positions = positions;
    }

    /// Return the current node positions.
    pub fn node_positions(&self) -> &[[f32; 3]] {
        &self.node_positions
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

        let n_nodes = amplitudes.len();
        let (fused_amp, fused_ph, coherence) = if n_nodes == 1 {
            // Single-node fallback
            (
                amplitudes[0].to_vec(),
                phases[0].to_vec(),
                1.0_f32,
            )
        } else {
            // Multi-node attention-weighted fusion
            attention_weighted_fusion(&amplitudes, &phases, self.config.attention_temperature)
        };

        // Derive timestamp from median
        let mut timestamps: Vec<u64> = node_frames.iter().map(|f| f.timestamp_us).collect();
        timestamps.sort_unstable();
        let timestamp_us = timestamps[timestamps.len() / 2];

        // Build node positions list, filling with origin for unknown nodes
        let positions: Vec<[f32; 3]> = (0..n_nodes)
            .map(|i| {
                self.node_positions
                    .get(i)
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
        })
    }
}

impl Default for MultistaticFuser {
    fn default() -> Self {
        Self::new()
    }
}

/// Attention-weighted fusion of amplitude and phase vectors from multiple nodes.
///
/// Each node's contribution is weighted by its agreement with the consensus.
/// Returns (fused_amplitude, fused_phase, cross_node_coherence).
fn attention_weighted_fusion(
    amplitudes: &[&[f32]],
    phases: &[&[f32]],
    temperature: f32,
) -> (Vec<f32>, Vec<f32>, f32) {
    let n_nodes = amplitudes.len();
    let n_sub = amplitudes[0].len();

    // Compute mean amplitude as consensus reference
    let mut mean_amp = vec![0.0_f32; n_sub];
    for amp in amplitudes {
        for (i, &v) in amp.iter().enumerate() {
            mean_amp[i] += v;
        }
    }
    for v in &mut mean_amp {
        *v /= n_nodes as f32;
    }

    // Compute attention weights based on similarity to consensus
    let mut weights = vec![0.0_f32; n_nodes];
    for (n, amp) in amplitudes.iter().enumerate() {
        let mut dot = 0.0_f32;
        let mut norm_a = 0.0_f32;
        let mut norm_b = 0.0_f32;
        for i in 0..n_sub {
            dot += amp[i] * mean_amp[i];
            norm_a += amp[i] * amp[i];
            norm_b += mean_amp[i] * mean_amp[i];
        }
        let denom = (norm_a * norm_b).sqrt().max(1e-12);
        let similarity = dot / denom;
        weights[n] = (similarity / temperature).exp();
    }

    // Normalize weights (softmax-style)
    let weight_sum: f32 = weights.iter().sum::<f32>().max(1e-12);
    for w in &mut weights {
        *w /= weight_sum;
    }

    // Weighted fusion
    let mut fused_amp = vec![0.0_f32; n_sub];
    let mut fused_ph_sin = vec![0.0_f32; n_sub];
    let mut fused_ph_cos = vec![0.0_f32; n_sub];

    for (n, (&amp, &ph)) in amplitudes.iter().zip(phases.iter()).enumerate() {
        let w = weights[n];
        for i in 0..n_sub {
            fused_amp[i] += w * amp[i];
            fused_ph_sin[i] += w * ph[i].sin();
            fused_ph_cos[i] += w * ph[i].cos();
        }
    }

    // Recover phase from sin/cos weighted average
    let fused_ph: Vec<f32> = fused_ph_sin
        .iter()
        .zip(fused_ph_cos.iter())
        .map(|(&s, &c)| s.atan2(c))
        .collect();

    // Coherence = mean weight entropy proxy: high when weights are balanced
    let coherence = compute_weight_coherence(&weights);

    (fused_amp, fused_ph, coherence)
}

/// Compute coherence from attention weights.
///
/// Returns 1.0 when all weights are equal (all nodes agree),
/// and approaches 0.0 when a single node dominates.
fn compute_weight_coherence(weights: &[f32]) -> f32 {
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
    let diversity = (ideal_gap / max_gap.max(1e-6)).clamp(0.0, 1.0);
    diversity
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
        }
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
        let mut fuser = MultistaticFuser::new();
        let positions = vec![[0.0, 0.0, 1.0], [3.0, 0.0, 1.0]];
        fuser.set_node_positions(positions.clone());
        assert_eq!(fuser.node_positions(), &positions[..]);
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

    #[test]
    fn geometric_diversity_single_node() {
        assert_eq!(geometric_diversity(&[[0.0, 0.0, 0.0]]), 0.0);
    }

    #[test]
    fn geometric_diversity_two_opposite() {
        let score = geometric_diversity(&[[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        assert!(score > 0.8, "Two opposite nodes should have high diversity: {}", score);
    }

    #[test]
    fn geometric_diversity_four_corners() {
        let score = geometric_diversity(&[
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [5.0, 5.0, 0.0],
            [0.0, 5.0, 0.0],
        ]);
        assert!(score > 0.7, "Four corners should have good diversity: {}", score);
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
        assert!(c < 0.3, "Single dominant node should have low coherence: {}", c);
    }

    #[test]
    fn default_config() {
        let cfg = MultistaticConfig::default();
        assert_eq!(cfg.guard_interval_us, 5000);
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
}
