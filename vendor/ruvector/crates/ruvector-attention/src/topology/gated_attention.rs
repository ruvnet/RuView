//! Topology Gated Attention
//!
//! Main attention mechanism that uses topological coherence as a permission signal.

use super::coherence::{CoherenceMetric, WindowCoherence};
use super::policy::{AttentionMode, AttentionPolicy, PolicyConfig};
use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use serde::{Deserialize, Serialize};

/// Configuration for topology-gated attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyGatedConfig {
    /// Model dimension
    pub dim: usize,
    /// Number of neighbors for coherence graph
    pub k_neighbors: usize,
    /// Coherence metrics to use
    pub metrics: Vec<CoherenceMetric>,
    /// Policy configuration
    pub policy: PolicyConfig,
    /// Temperature for softmax
    pub temperature: f32,
    /// Base attention width
    pub base_width: usize,
}

impl Default for TopologyGatedConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            k_neighbors: 8,
            metrics: vec![
                CoherenceMetric::BoundaryMass,
                CoherenceMetric::SimilarityVariance,
            ],
            policy: PolicyConfig::default(),
            temperature: 1.0,
            base_width: 64,
        }
    }
}

/// Topology Gated Attention
///
/// Uses structural coherence to control attention behavior:
/// - Stable mode: full attention, normal updates
/// - Cautious mode: reduced width, increased sparsity
/// - Freeze mode: retrieval only, no updates
#[derive(Debug, Clone)]
pub struct TopologyGatedAttention {
    config: TopologyGatedConfig,
    policy: AttentionPolicy,
    cached_coherence: Option<WindowCoherence>,
}

impl TopologyGatedAttention {
    /// Create new topology-gated attention
    pub fn new(config: TopologyGatedConfig) -> Self {
        let policy = AttentionPolicy::new(config.policy.clone());

        Self {
            config,
            policy,
            cached_coherence: None,
        }
    }

    /// Create with dimension
    pub fn with_dim(dim: usize) -> Self {
        Self::new(TopologyGatedConfig {
            dim,
            ..Default::default()
        })
    }

    /// Update coherence from keys (call periodically, not every token)
    pub fn update_coherence(&mut self, keys: &[&[f32]]) {
        let coherence =
            WindowCoherence::compute(keys, self.config.k_neighbors, &self.config.metrics);
        self.policy.determine_mode(coherence.score);
        self.cached_coherence = Some(coherence);
    }

    /// Get current mode
    pub fn current_mode(&self) -> AttentionMode {
        self.policy.current_mode()
    }

    /// Check if coherence update is needed
    pub fn needs_coherence_update(&self) -> bool {
        match &self.cached_coherence {
            Some(c) => c.needs_update(self.config.policy.update_period),
            None => true,
        }
    }

    /// Tick coherence counter
    pub fn tick_coherence(&mut self) {
        if let Some(ref mut c) = self.cached_coherence {
            c.tick();
        }
    }

    /// Get effective attention width
    pub fn get_attention_width(&self) -> usize {
        self.policy.get_attention_width(self.config.base_width)
    }

    /// Check if updates are allowed
    pub fn allows_updates(&self) -> bool {
        self.policy.allows_updates()
    }

    /// Compute gated attention
    pub fn compute_gated(
        &mut self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        // Update coherence if needed
        if self.needs_coherence_update() {
            self.update_coherence(keys);
        } else {
            self.tick_coherence();
        }

        match self.current_mode() {
            AttentionMode::Stable => {
                // Full attention
                self.full_attention(query, keys, values)
            }
            AttentionMode::Cautious => {
                // Sparse attention with reduced width
                let width = self.get_attention_width();
                self.sparse_attention(query, keys, values, width)
            }
            AttentionMode::Freeze => {
                // Retrieval only: just return query projection
                // (no attention, just pass-through with light weighting)
                self.retrieval_only(query, keys, values)
            }
        }
    }

    /// Full attention (stable mode)
    fn full_attention(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("No keys".into()));
        }

        // Standard scaled dot-product attention
        let scale = 1.0 / (self.config.dim as f32).sqrt();

        let logits: Vec<f32> = keys
            .iter()
            .map(|k| Self::dot_product_simd(query, k) * scale / self.config.temperature)
            .collect();

        let weights = Self::stable_softmax(&logits);

        self.weighted_sum(&weights, values)
    }

    /// Sparse attention with limited width (cautious mode)
    fn sparse_attention(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        width: usize,
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("No keys".into()));
        }

        let width = width.min(keys.len());

        // Get top-k keys by dot product
        let scale = 1.0 / (self.config.dim as f32).sqrt();
        let mut scores: Vec<(usize, f32)> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (i, Self::dot_product_simd(query, k) * scale))
            .collect();

        scores.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k
        let top_k: Vec<(usize, f32)> = scores.into_iter().take(width).collect();

        // Compute attention over selected keys
        let logits: Vec<f32> = top_k
            .iter()
            .map(|(_, s)| s / self.config.temperature)
            .collect();

        let weights = Self::stable_softmax(&logits);

        // Weighted sum of selected values
        let selected_values: Vec<&[f32]> = top_k.iter().map(|(i, _)| values[*i]).collect();

        self.weighted_sum(&weights, &selected_values)
    }

    /// Retrieval-only mode (freeze mode)
    fn retrieval_only(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("No keys".into()));
        }

        // Find single best match and return its value
        // This is ultra-sparse: only 1 key contributes
        let scale = 1.0 / (self.config.dim as f32).sqrt();

        let best_idx = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (i, Self::dot_product_simd(query, k) * scale))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        Ok(values[best_idx].to_vec())
    }

    /// SIMD-friendly dot product
    #[inline(always)]
    fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += a[base] * b[base];
            sum1 += a[base + 1] * b[base + 1];
            sum2 += a[base + 2] * b[base + 2];
            sum3 += a[base + 3] * b[base + 3];
        }

        let base = chunks * 4;
        for i in 0..remainder {
            sum0 += a[base + i] * b[base + i];
        }

        sum0 + sum1 + sum2 + sum3
    }

    /// Stable softmax
    fn stable_softmax(logits: &[f32]) -> Vec<f32> {
        if logits.is_empty() {
            return vec![];
        }

        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum: f32 = exp_logits.iter().sum();

        exp_logits.iter().map(|&e| e / sum).collect()
    }

    /// Weighted sum
    fn weighted_sum(&self, weights: &[f32], values: &[&[f32]]) -> AttentionResult<Vec<f32>> {
        if weights.is_empty() || values.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty inputs".into()));
        }

        let dim = values[0].len();
        let mut output = vec![0.0f32; dim];

        for (weight, value) in weights.iter().zip(values.iter()) {
            for (o, &v) in output.iter_mut().zip(value.iter()) {
                *o += weight * v;
            }
        }

        Ok(output)
    }
}

impl Attention for TopologyGatedAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        // For trait, use clone to allow mutation
        let mut att = self.clone();
        att.compute_gated(query, keys, values)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if let Some(m) = mask {
            let filtered: Vec<(&[f32], &[f32])> = keys
                .iter()
                .zip(values.iter())
                .enumerate()
                .filter(|(i, _)| m.get(*i).copied().unwrap_or(true))
                .map(|(_, (k, v))| (*k, *v))
                .collect();

            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(k, _)| *k).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(_, v)| *v).collect();

            self.compute(query, &filtered_keys, &filtered_values)
        } else {
            self.compute(query, keys, values)
        }
    }

    fn dim(&self) -> usize {
        self.config.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_gated_attention() {
        let mut attention = TopologyGatedAttention::with_dim(32);

        let query = vec![0.5f32; 32];
        let keys: Vec<Vec<f32>> = (0..20).map(|i| vec![0.1 + i as f32 * 0.02; 32]).collect();
        let values: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; 32]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention
            .compute_gated(&query, &keys_refs, &values_refs)
            .unwrap();
        assert_eq!(output.len(), 32);
    }

    #[test]
    fn test_mode_affects_output() {
        let config = TopologyGatedConfig {
            dim: 16,
            base_width: 32,
            policy: PolicyConfig {
                stable_threshold: 0.9, // Very high threshold
                freeze_threshold: 0.8,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut attention = TopologyGatedAttention::new(config);

        // Create diverse keys (low coherence)
        let keys: Vec<Vec<f32>> = (0..10)
            .map(|i| {
                let mut v = vec![0.0f32; 16];
                v[i % 16] = 1.0;
                v
            })
            .collect();
        let values: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; 16]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        attention.update_coherence(&keys_refs);

        // With diverse keys, should trigger freeze mode
        let query = vec![0.5f32; 16];
        let _output = attention
            .compute_gated(&query, &keys_refs, &values_refs)
            .unwrap();

        // Mode should be freeze or cautious due to low coherence
        let mode = attention.current_mode();
        assert!(mode == AttentionMode::Freeze || mode == AttentionMode::Cautious);
    }

    #[test]
    fn test_coherence_update_period() {
        let config = TopologyGatedConfig {
            dim: 16,
            policy: PolicyConfig {
                update_period: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut attention = TopologyGatedAttention::new(config);

        // No coherence yet
        assert!(attention.needs_coherence_update());

        let keys: Vec<Vec<f32>> = vec![vec![1.0; 16]; 5];
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        attention.update_coherence(&keys_refs);
        assert!(!attention.needs_coherence_update());

        // Tick 4 times
        for _ in 0..4 {
            attention.tick_coherence();
        }

        assert!(attention.needs_coherence_update());
    }
}
