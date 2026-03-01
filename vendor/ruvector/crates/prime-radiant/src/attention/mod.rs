//! Attention-Weighted Residuals Module
//!
//! Computes attention-weighted coherence using multiple mechanisms:
//! - Topology-gated attention (structural coherence as permission signal)
//! - Mixture of Experts (specialized residual processing)
//! - PDE diffusion (smooth energy propagation)
//!
//! Leverages `ruvector-attention` for the underlying attention implementations.
//!
//! # Features
//!
//! - Three attention modes: Stable, Cautious, Freeze
//! - MoE routing for specialized residual experts
//! - Diffusion-based energy smoothing
//! - Attention score computation for residual weighting

mod adapter;
mod config;
mod diffusion;
mod moe;
mod topology;

pub use adapter::AttentionAdapter;
pub use config::AttentionCoherenceConfig;
pub use diffusion::{DiffusionSmoothing, SmoothedEnergy};
pub use moe::{ExpertRouting, MoEResidualProcessor};
pub use topology::{AttentionScore, TopologyGate, TopologyGateResult};

use std::collections::HashMap;

/// Node identifier type
pub type NodeId = u64;

/// Edge identifier type
pub type EdgeId = (NodeId, NodeId);

/// Result type for attention operations
pub type Result<T> = std::result::Result<T, AttentionError>;

/// Errors in attention-weighted coherence computation
#[derive(Debug, Clone, thiserror::Error)]
pub enum AttentionError {
    /// Invalid dimension
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Empty input
    #[error("Empty input: {0}")]
    EmptyInput(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Computation failed
    #[error("Computation failed: {0}")]
    ComputationFailed(String),

    /// Mode not supported
    #[error("Mode not supported in current state: {0}")]
    ModeNotSupported(String),
}

/// Main attention-weighted coherence engine
///
/// Combines topology-gated attention, MoE routing, and PDE diffusion
/// to compute attention-weighted residuals for coherence analysis.
#[derive(Debug)]
pub struct AttentionCoherence {
    /// Configuration
    config: AttentionCoherenceConfig,
    /// Adapter to attention implementations
    adapter: AttentionAdapter,
    /// Topology gate
    topo_gate: TopologyGate,
    /// MoE residual processor
    moe: MoEResidualProcessor,
    /// Diffusion smoother
    diffusion: DiffusionSmoothing,
}

impl AttentionCoherence {
    /// Create a new attention coherence engine
    pub fn new(config: AttentionCoherenceConfig) -> Self {
        let adapter = AttentionAdapter::new(config.clone());
        let topo_gate = TopologyGate::new(config.clone());
        let moe = MoEResidualProcessor::new(config.clone());
        let diffusion = DiffusionSmoothing::new(config.clone());

        Self {
            config,
            adapter,
            topo_gate,
            moe,
            diffusion,
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(AttentionCoherenceConfig::default())
    }

    /// Compute attention scores for nodes
    ///
    /// Returns attention scores indicating structural importance.
    pub fn compute_attention_scores(
        &mut self,
        node_states: &[&[f32]],
    ) -> Result<HashMap<usize, f32>> {
        if node_states.is_empty() {
            return Err(AttentionError::EmptyInput("node_states".to_string()));
        }

        // Update topology gate coherence
        self.topo_gate.update_coherence(node_states);

        // Compute scores using adapter
        let scores = self.adapter.compute_scores(node_states)?;

        // Convert to hashmap
        Ok(scores
            .into_iter()
            .enumerate()
            .map(|(i, s)| (i, s))
            .collect())
    }

    /// Compute attention-weighted residuals
    ///
    /// Weights each edge residual by the attention scores of its endpoints.
    pub fn weighted_residuals(
        &mut self,
        node_states: &[&[f32]],
        edge_residuals: &[(usize, usize, Vec<f32>)], // (source_idx, target_idx, residual)
    ) -> Result<Vec<WeightedEdgeResidual>> {
        if node_states.is_empty() {
            return Err(AttentionError::EmptyInput("node_states".to_string()));
        }

        // Compute attention scores
        let scores = self.compute_attention_scores(node_states)?;

        // Weight residuals
        let mut weighted = Vec::with_capacity(edge_residuals.len());

        for (source, target, residual) in edge_residuals {
            let source_score = scores.get(source).copied().unwrap_or(1.0);
            let target_score = scores.get(target).copied().unwrap_or(1.0);

            // Average attention weight
            let attention_weight = (source_score + target_score) / 2.0;

            // Residual norm squared
            let residual_norm_sq: f32 = residual.iter().map(|x| x * x).sum();

            // Weighted energy
            let weighted_energy = residual_norm_sq * attention_weight;

            weighted.push(WeightedEdgeResidual {
                source_idx: *source,
                target_idx: *target,
                source_attention: source_score,
                target_attention: target_score,
                attention_weight,
                residual_norm_sq,
                weighted_energy,
            });
        }

        Ok(weighted)
    }

    /// Route residual through MoE experts
    ///
    /// Uses specialized experts for different residual characteristics.
    pub fn moe_process_residual(
        &self,
        residual: &[f32],
        context: &[f32],
    ) -> Result<MoEProcessedResidual> {
        self.moe.process(residual, context)
    }

    /// Apply diffusion smoothing to energy values
    ///
    /// Smooths energy across the graph using PDE diffusion.
    pub fn smooth_energy(
        &self,
        edge_energies: &[(usize, usize, f32)], // (source, target, energy)
        node_states: &[&[f32]],
        steps: usize,
    ) -> Result<SmoothedEnergy> {
        self.diffusion.smooth(edge_energies, node_states, steps)
    }

    /// Get current topology gate result
    pub fn gate_result(&self) -> TopologyGateResult {
        self.topo_gate.current_result()
    }

    /// Check if updates are allowed (not in freeze mode)
    pub fn allows_updates(&self) -> bool {
        self.topo_gate.allows_updates()
    }

    /// Get effective attention width based on current mode
    pub fn attention_width(&self) -> usize {
        self.topo_gate.attention_width()
    }

    /// Get configuration
    pub fn config(&self) -> &AttentionCoherenceConfig {
        &self.config
    }

    /// Compute full attention-weighted energy analysis
    pub fn full_analysis(
        &mut self,
        node_states: &[&[f32]],
        edge_residuals: &[(usize, usize, Vec<f32>)],
    ) -> Result<AttentionEnergyAnalysis> {
        // Get gate result
        let gate_result = self.topo_gate.current_result();

        // Compute weighted residuals
        let weighted = self.weighted_residuals(node_states, edge_residuals)?;

        // Compute energies
        let edge_energies: Vec<(usize, usize, f32)> = weighted
            .iter()
            .map(|w| (w.source_idx, w.target_idx, w.weighted_energy))
            .collect();

        // Apply diffusion if enabled
        let smoothed = if self.config.enable_diffusion {
            Some(self.smooth_energy(&edge_energies, node_states, self.config.diffusion_steps)?)
        } else {
            None
        };

        // Aggregate
        let total_energy: f32 = weighted.iter().map(|w| w.weighted_energy).sum();
        let avg_attention: f32 =
            weighted.iter().map(|w| w.attention_weight).sum::<f32>() / weighted.len().max(1) as f32;

        Ok(AttentionEnergyAnalysis {
            weighted_residuals: weighted,
            smoothed_energy: smoothed,
            total_energy,
            avg_attention_weight: avg_attention,
            gate_result,
            num_edges: edge_residuals.len(),
        })
    }
}

/// Result of weighting an edge residual by attention
#[derive(Debug, Clone)]
pub struct WeightedEdgeResidual {
    /// Source node index
    pub source_idx: usize,
    /// Target node index
    pub target_idx: usize,
    /// Attention score of source node
    pub source_attention: f32,
    /// Attention score of target node
    pub target_attention: f32,
    /// Combined attention weight
    pub attention_weight: f32,
    /// Squared norm of residual
    pub residual_norm_sq: f32,
    /// Final weighted energy
    pub weighted_energy: f32,
}

/// Result of processing a residual through MoE
#[derive(Debug, Clone)]
pub struct MoEProcessedResidual {
    /// Output from expert combination
    pub output: Vec<f32>,
    /// Expert indices that were used
    pub expert_indices: Vec<usize>,
    /// Weights for each expert
    pub expert_weights: Vec<f32>,
    /// Load balance loss (for training)
    pub load_balance_loss: f32,
}

/// Complete attention energy analysis
#[derive(Debug, Clone)]
pub struct AttentionEnergyAnalysis {
    /// All weighted residuals
    pub weighted_residuals: Vec<WeightedEdgeResidual>,
    /// Smoothed energy (if diffusion enabled)
    pub smoothed_energy: Option<SmoothedEnergy>,
    /// Total weighted energy
    pub total_energy: f32,
    /// Average attention weight
    pub avg_attention_weight: f32,
    /// Current gate result
    pub gate_result: TopologyGateResult,
    /// Number of edges analyzed
    pub num_edges: usize,
}

impl AttentionEnergyAnalysis {
    /// Check if coherent (energy below threshold)
    pub fn is_coherent(&self, threshold: f32) -> bool {
        self.total_energy < threshold
    }

    /// Get highest energy edge
    pub fn highest_energy_edge(&self) -> Option<&WeightedEdgeResidual> {
        self.weighted_residuals
            .iter()
            .max_by(|a, b| a.weighted_energy.partial_cmp(&b.weighted_energy).unwrap())
    }

    /// Get edges above threshold
    pub fn edges_above_threshold(&self, threshold: f32) -> Vec<&WeightedEdgeResidual> {
        self.weighted_residuals
            .iter()
            .filter(|r| r.weighted_energy > threshold)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_states(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n).map(|i| vec![0.1 * (i + 1) as f32; dim]).collect()
    }

    #[test]
    fn test_basic_coherence() {
        let config = AttentionCoherenceConfig {
            dimension: 16,
            ..Default::default()
        };
        let mut coherence = AttentionCoherence::new(config);

        let states = make_states(5, 16);
        let state_refs: Vec<&[f32]> = states.iter().map(|s| s.as_slice()).collect();

        let scores = coherence.compute_attention_scores(&state_refs).unwrap();

        assert_eq!(scores.len(), 5);
        for (_, &score) in &scores {
            assert!(score >= 0.0 && score <= 1.0);
        }
    }

    #[test]
    fn test_weighted_residuals() {
        let config = AttentionCoherenceConfig {
            dimension: 8,
            ..Default::default()
        };
        let mut coherence = AttentionCoherence::new(config);

        let states = make_states(4, 8);
        let state_refs: Vec<&[f32]> = states.iter().map(|s| s.as_slice()).collect();

        let residuals = vec![
            (0, 1, vec![0.1f32; 8]),
            (1, 2, vec![0.2f32; 8]),
            (2, 3, vec![0.3f32; 8]),
        ];

        let weighted = coherence
            .weighted_residuals(&state_refs, &residuals)
            .unwrap();

        assert_eq!(weighted.len(), 3);
        for w in &weighted {
            assert!(w.weighted_energy >= 0.0);
            assert!(w.attention_weight > 0.0);
        }
    }

    #[test]
    fn test_full_analysis() {
        let config = AttentionCoherenceConfig {
            dimension: 8,
            enable_diffusion: false,
            ..Default::default()
        };
        let mut coherence = AttentionCoherence::new(config);

        let states = make_states(3, 8);
        let state_refs: Vec<&[f32]> = states.iter().map(|s| s.as_slice()).collect();

        let residuals = vec![(0, 1, vec![0.1f32; 8]), (1, 2, vec![0.2f32; 8])];

        let analysis = coherence.full_analysis(&state_refs, &residuals).unwrap();

        assert_eq!(analysis.num_edges, 2);
        assert!(analysis.total_energy >= 0.0);
        assert!(analysis.avg_attention_weight > 0.0);
    }
}
