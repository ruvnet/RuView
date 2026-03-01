//! Topology-Gated Attention
//!
//! Uses topological coherence as a permission signal for attention behavior.

use super::config::AttentionMode;
use super::{AttentionCoherenceConfig, AttentionError, Result};

/// Score from attention computation
#[derive(Debug, Clone)]
pub struct AttentionScore {
    /// Node index
    pub node_idx: usize,
    /// Attention score value
    pub score: f32,
    /// Contribution to coherence
    pub coherence_contribution: f32,
}

/// Result of topology gate evaluation
#[derive(Debug, Clone)]
pub struct TopologyGateResult {
    /// Current coherence score
    pub coherence: f32,
    /// Current mode
    pub mode: AttentionMode,
    /// Effective attention width
    pub width: usize,
    /// Whether updates are allowed
    pub allows_updates: bool,
    /// Ticks since last coherence update
    pub ticks_since_update: usize,
}

impl TopologyGateResult {
    /// Create a default result (stable mode)
    pub fn stable(config: &AttentionCoherenceConfig) -> Self {
        Self {
            coherence: 1.0,
            mode: AttentionMode::Stable,
            width: config.base_width,
            allows_updates: true,
            ticks_since_update: 0,
        }
    }
}

/// Topology-gated attention controller
///
/// Uses structural coherence to control attention behavior:
/// - Stable mode: full attention, normal updates
/// - Cautious mode: reduced width, increased sparsity
/// - Freeze mode: retrieval only, no updates
#[derive(Debug)]
pub struct TopologyGate {
    /// Configuration
    config: AttentionCoherenceConfig,
    /// Current coherence score
    coherence: f32,
    /// Current mode
    mode: AttentionMode,
    /// Ticks since last coherence update
    ticks_since_update: usize,
    /// Cached coherence metrics
    cached_metrics: Option<CoherenceMetrics>,
}

impl TopologyGate {
    /// Create a new topology gate
    pub fn new(config: AttentionCoherenceConfig) -> Self {
        Self {
            coherence: 1.0, // Start optimistic
            mode: AttentionMode::Stable,
            ticks_since_update: 0,
            cached_metrics: None,
            config,
        }
    }

    /// Update coherence from key states
    pub fn update_coherence(&mut self, keys: &[&[f32]]) {
        if keys.is_empty() {
            return;
        }

        let metrics = self.compute_coherence_metrics(keys);
        self.coherence = metrics.coherence_score;
        self.mode = AttentionMode::from_coherence(self.coherence, &self.config);
        self.ticks_since_update = 0;
        self.cached_metrics = Some(metrics);
    }

    /// Tick the coherence counter
    pub fn tick(&mut self) {
        self.ticks_since_update += 1;
    }

    /// Check if coherence update is needed
    pub fn needs_update(&self) -> bool {
        self.ticks_since_update >= self.config.coherence_update_period
            || self.cached_metrics.is_none()
    }

    /// Get current mode
    pub fn current_mode(&self) -> AttentionMode {
        self.mode
    }

    /// Get current coherence score
    pub fn current_coherence(&self) -> f32 {
        self.coherence
    }

    /// Check if updates are allowed
    pub fn allows_updates(&self) -> bool {
        self.mode.allows_updates()
    }

    /// Get effective attention width
    pub fn attention_width(&self) -> usize {
        self.config.width_for_coherence(self.coherence)
    }

    /// Get current gate result
    pub fn current_result(&self) -> TopologyGateResult {
        TopologyGateResult {
            coherence: self.coherence,
            mode: self.mode,
            width: self.attention_width(),
            allows_updates: self.allows_updates(),
            ticks_since_update: self.ticks_since_update,
        }
    }

    /// Compute coherence metrics from keys
    fn compute_coherence_metrics(&self, keys: &[&[f32]]) -> CoherenceMetrics {
        if keys.is_empty() {
            return CoherenceMetrics::empty();
        }

        let n = keys.len();
        let k = self.config.k_neighbors.min(n - 1);

        if k == 0 {
            return CoherenceMetrics::with_score(1.0);
        }

        // Compute pairwise similarities
        let mut similarities: Vec<Vec<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut row = Vec::with_capacity(n);
            for j in 0..n {
                if i == j {
                    row.push(1.0);
                } else {
                    row.push(self.cosine_similarity(keys[i], keys[j]));
                }
            }
            similarities.push(row);
        }

        // Compute boundary mass (proportion of edges to k nearest neighbors)
        let mut total_boundary_mass = 0.0f32;
        let mut total_edges = 0;

        for i in 0..n {
            // Get k nearest neighbors
            let mut neighbor_sims: Vec<(usize, f32)> = similarities[i]
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(j, &s)| (j, s))
                .collect();

            neighbor_sims
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let neighbors: Vec<usize> = neighbor_sims.iter().take(k).map(|(j, _)| *j).collect();

            // Boundary mass: edges to non-neighbors
            for j in 0..n {
                if j != i && !neighbors.contains(&j) {
                    total_boundary_mass += similarities[i][j].max(0.0);
                    total_edges += 1;
                }
            }
        }

        // Compute similarity variance
        let all_sims: Vec<f32> = similarities
            .iter()
            .enumerate()
            .flat_map(|(i, row)| {
                row.iter()
                    .enumerate()
                    .filter(move |(j, _)| *j > i)
                    .map(|(_, &s)| s)
            })
            .collect();

        let mean_sim: f32 = all_sims.iter().sum::<f32>() / all_sims.len().max(1) as f32;
        let variance: f32 = all_sims.iter().map(|s| (s - mean_sim).powi(2)).sum::<f32>()
            / all_sims.len().max(1) as f32;

        // Coherence score: high similarity, low variance, low boundary mass
        let boundary_ratio = if total_edges > 0 {
            total_boundary_mass / total_edges as f32
        } else {
            0.0
        };

        // Combine metrics
        // High mean similarity and low variance = high coherence
        // High boundary mass = low coherence
        let coherence_score =
            (mean_sim * 0.5 + (1.0 - variance.sqrt()) * 0.3 + (1.0 - boundary_ratio) * 0.2)
                .clamp(0.0, 1.0);

        CoherenceMetrics {
            coherence_score,
            mean_similarity: mean_sim,
            similarity_variance: variance,
            boundary_mass: total_boundary_mass,
            num_nodes: n,
        }
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
    }
}

/// Coherence metrics computed from key states
#[derive(Debug, Clone)]
struct CoherenceMetrics {
    /// Overall coherence score
    coherence_score: f32,
    /// Mean pairwise similarity
    mean_similarity: f32,
    /// Variance of pairwise similarities
    similarity_variance: f32,
    /// Total boundary mass (edges to non-neighbors)
    boundary_mass: f32,
    /// Number of nodes
    num_nodes: usize,
}

impl CoherenceMetrics {
    fn empty() -> Self {
        Self {
            coherence_score: 1.0,
            mean_similarity: 1.0,
            similarity_variance: 0.0,
            boundary_mass: 0.0,
            num_nodes: 0,
        }
    }

    fn with_score(score: f32) -> Self {
        Self {
            coherence_score: score,
            mean_similarity: score,
            similarity_variance: 0.0,
            boundary_mass: 0.0,
            num_nodes: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_gate_creation() {
        let config = AttentionCoherenceConfig::default();
        let gate = TopologyGate::new(config);

        assert_eq!(gate.current_mode(), AttentionMode::Stable);
        assert!(gate.allows_updates());
    }

    #[test]
    fn test_update_coherence_similar_keys() {
        let config = AttentionCoherenceConfig::default();
        let mut gate = TopologyGate::new(config);

        // All similar keys = high coherence
        let keys: Vec<Vec<f32>> = (0..10).map(|_| vec![1.0, 0.0, 0.0, 0.0]).collect();
        let key_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        gate.update_coherence(&key_refs);

        assert!(gate.current_coherence() > 0.5);
        assert_eq!(gate.current_mode(), AttentionMode::Stable);
    }

    #[test]
    fn test_update_coherence_diverse_keys() {
        let config = AttentionCoherenceConfig {
            stable_threshold: 0.9,
            freeze_threshold: 0.5,
            ..Default::default()
        };
        let mut gate = TopologyGate::new(config);

        // Diverse keys = lower coherence
        let keys: Vec<Vec<f32>> = (0..10)
            .map(|i| {
                let mut v = vec![0.0f32; 16];
                v[i % 16] = 1.0;
                v
            })
            .collect();
        let key_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        gate.update_coherence(&key_refs);

        // Should trigger cautious or freeze mode due to diversity
        assert!(
            gate.current_mode() == AttentionMode::Cautious
                || gate.current_mode() == AttentionMode::Freeze
        );
    }

    #[test]
    fn test_tick_and_update_period() {
        let config = AttentionCoherenceConfig {
            coherence_update_period: 4,
            ..Default::default()
        };
        let mut gate = TopologyGate::new(config);

        // Initially needs update (no cache)
        assert!(gate.needs_update());

        let keys: Vec<Vec<f32>> = vec![vec![1.0; 8]; 5];
        let key_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        gate.update_coherence(&key_refs);
        assert!(!gate.needs_update());

        // Tick 4 times
        for _ in 0..4 {
            gate.tick();
        }
        assert!(gate.needs_update());
    }

    #[test]
    fn test_attention_width() {
        let config = AttentionCoherenceConfig {
            base_width: 64,
            stable_threshold: 0.7,
            freeze_threshold: 0.3,
            ..Default::default()
        };
        let mut gate = TopologyGate::new(config);

        // High coherence = full width
        gate.coherence = 0.8;
        gate.mode = AttentionMode::from_coherence(0.8, &gate.config);
        assert_eq!(gate.attention_width(), 64);

        // Medium coherence = reduced width
        gate.coherence = 0.5;
        gate.mode = AttentionMode::from_coherence(0.5, &gate.config);
        assert_eq!(gate.attention_width(), 32);

        // Low coherence = minimal width
        gate.coherence = 0.2;
        gate.mode = AttentionMode::from_coherence(0.2, &gate.config);
        assert_eq!(gate.attention_width(), 1);
    }
}
