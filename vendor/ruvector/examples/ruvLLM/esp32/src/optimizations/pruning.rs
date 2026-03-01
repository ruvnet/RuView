//! MinCut-Inspired Layer Pruning for ESP32
//!
//! Intelligent pruning strategies adapted from ruvector graph algorithms.
//! Identifies and removes least important weights/neurons while preserving model quality.

use heapless::Vec as HVec;

/// Maximum neurons to track for pruning
pub const MAX_PRUNING_UNITS: usize = 64;

/// Pruning configuration
#[derive(Debug, Clone, Copy)]
pub struct PruningConfig {
    /// Target sparsity (0.0 = no pruning, 1.0 = all pruned)
    pub target_sparsity: f32,
    /// Minimum importance threshold (absolute value)
    pub importance_threshold: i8,
    /// Enable structured pruning (whole neurons vs individual weights)
    pub structured: bool,
    /// Gradual pruning steps (0 = one-shot)
    pub gradual_steps: usize,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            target_sparsity: 0.5,
            importance_threshold: 8,
            structured: true,
            gradual_steps: 0,
        }
    }
}

/// Maximum mask words (supports up to 2048 weights)
pub const MAX_MASK_WORDS: usize = 64;

/// Pruning mask for a weight matrix
#[derive(Debug, Clone)]
pub struct PruningMask<const N: usize> {
    /// Bitmask: 1 = keep, 0 = prune
    pub mask: HVec<u32, MAX_MASK_WORDS>,
    /// Number of elements
    pub size: usize,
    /// Number of pruned elements
    pub pruned_count: usize,
}

impl<const N: usize> PruningMask<N> {
    /// Create mask with all weights kept
    pub fn new(size: usize) -> crate::Result<Self> {
        let num_words = (size + 31) / 32;
        let mut mask = HVec::new();

        for i in 0..num_words {
            let bits = if i == num_words - 1 && size % 32 != 0 {
                (1u32 << (size % 32)) - 1
            } else {
                u32::MAX
            };
            mask.push(bits).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self { mask, size, pruned_count: 0 })
    }

    /// Check if weight at index is kept
    #[inline]
    pub fn is_kept(&self, idx: usize) -> bool {
        let word = idx / 32;
        let bit = idx % 32;
        (self.mask.get(word).copied().unwrap_or(0) >> bit) & 1 == 1
    }

    /// Prune weight at index
    pub fn prune(&mut self, idx: usize) {
        if idx < self.size && self.is_kept(idx) {
            let word = idx / 32;
            let bit = idx % 32;
            if let Some(w) = self.mask.get_mut(word) {
                *w &= !(1 << bit);
                self.pruned_count += 1;
            }
        }
    }

    /// Current sparsity level
    pub fn sparsity(&self) -> f32 {
        self.pruned_count as f32 / self.size as f32
    }
}

/// Layer-level pruner using importance scoring
pub struct LayerPruner {
    /// Configuration
    config: PruningConfig,
    /// Importance scores for neurons/weights
    importance_scores: HVec<i16, MAX_PRUNING_UNITS>,
    /// Current pruning step (for gradual pruning)
    current_step: usize,
}

impl LayerPruner {
    /// Create new pruner with config
    pub fn new(config: PruningConfig) -> Self {
        Self {
            config,
            importance_scores: HVec::new(),
            current_step: 0,
        }
    }

    /// Compute importance scores for weights using magnitude
    pub fn compute_magnitude_importance(&mut self, weights: &[i8]) {
        self.importance_scores.clear();

        for &w in weights.iter().take(MAX_PRUNING_UNITS) {
            let importance = (w as i16).abs();
            let _ = self.importance_scores.push(importance);
        }
    }

    /// Compute importance using gradient information (simplified)
    /// For on-device: use weight * activation as proxy
    pub fn compute_gradient_importance(&mut self, weights: &[i8], activations: &[i8]) {
        self.importance_scores.clear();

        for (&w, &a) in weights.iter().zip(activations.iter()).take(MAX_PRUNING_UNITS) {
            // |weight * activation| as importance proxy
            let importance = ((w as i32 * a as i32).abs() >> 4) as i16;
            let _ = self.importance_scores.push(importance);
        }
    }

    /// Create pruning mask based on importance scores
    pub fn create_mask<const N: usize>(&self, size: usize) -> crate::Result<PruningMask<N>> {
        let mut mask = PruningMask::new(size)?;

        // Count weights below threshold
        let threshold = self.compute_threshold(size);

        for (idx, &score) in self.importance_scores.iter().enumerate() {
            if score < threshold {
                mask.prune(idx);
            }
        }

        Ok(mask)
    }

    /// Compute importance threshold for target sparsity
    fn compute_threshold(&self, size: usize) -> i16 {
        let target_pruned = (size as f32 * self.config.target_sparsity) as usize;

        if target_pruned == 0 || self.importance_scores.is_empty() {
            return 0;
        }

        // Find threshold that achieves target sparsity
        // Simple approach: sort importance and pick threshold
        let mut sorted: HVec<i16, MAX_PRUNING_UNITS> = HVec::new();
        for &s in &self.importance_scores {
            let _ = sorted.push(s);
        }

        // Bubble sort (fine for small arrays)
        for i in 0..sorted.len() {
            for j in 0..sorted.len() - 1 - i {
                if sorted[j] > sorted[j + 1] {
                    sorted.swap(j, j + 1);
                }
            }
        }

        let idx = target_pruned.min(sorted.len().saturating_sub(1));
        sorted.get(idx).copied().unwrap_or(0)
    }

    /// Apply pruning mask to weights in-place
    pub fn apply_mask<const N: usize>(&self, weights: &mut [i8], mask: &PruningMask<N>) {
        for (idx, weight) in weights.iter_mut().enumerate() {
            if !mask.is_kept(idx) {
                *weight = 0;
            }
        }
    }

    /// Structured pruning: remove entire neurons
    pub fn prune_neurons(
        &mut self,
        weights: &mut [i8],
        input_dim: usize,
        output_dim: usize,
    ) -> HVec<bool, MAX_PRUNING_UNITS> {
        // Compute per-neuron importance (L1 norm of weights)
        let mut neuron_importance: HVec<i32, MAX_PRUNING_UNITS> = HVec::new();

        for out_idx in 0..output_dim.min(MAX_PRUNING_UNITS) {
            let mut l1_sum: i32 = 0;
            for in_idx in 0..input_dim {
                let w_idx = out_idx * input_dim + in_idx;
                if w_idx < weights.len() {
                    l1_sum += (weights[w_idx] as i32).abs();
                }
            }
            let _ = neuron_importance.push(l1_sum);
        }

        // Find threshold
        let target_pruned = (output_dim as f32 * self.config.target_sparsity) as usize;
        let mut sorted: HVec<i32, MAX_PRUNING_UNITS> = neuron_importance.clone();

        for i in 0..sorted.len() {
            for j in 0..sorted.len() - 1 - i {
                if sorted[j] > sorted[j + 1] {
                    sorted.swap(j, j + 1);
                }
            }
        }

        let threshold = sorted.get(target_pruned).copied().unwrap_or(0);

        // Mark neurons to prune
        let mut keep_mask: HVec<bool, MAX_PRUNING_UNITS> = HVec::new();

        for &importance in &neuron_importance {
            let _ = keep_mask.push(importance >= threshold);
        }

        // Zero out pruned neurons
        for out_idx in 0..output_dim.min(keep_mask.len()) {
            if !keep_mask[out_idx] {
                for in_idx in 0..input_dim {
                    let w_idx = out_idx * input_dim + in_idx;
                    if w_idx < weights.len() {
                        weights[w_idx] = 0;
                    }
                }
            }
        }

        keep_mask
    }

    /// Get statistics about pruning
    pub fn pruning_stats<const N: usize>(&self, mask: &PruningMask<N>) -> PruningStats {
        PruningStats {
            total_weights: mask.size,
            pruned_weights: mask.pruned_count,
            sparsity: mask.sparsity(),
            memory_saved: mask.pruned_count, // 1 byte per weight
        }
    }
}

/// Statistics about pruning results
#[derive(Debug, Clone)]
pub struct PruningStats {
    /// Total weight count
    pub total_weights: usize,
    /// Number of pruned weights
    pub pruned_weights: usize,
    /// Achieved sparsity
    pub sparsity: f32,
    /// Memory saved in bytes
    pub memory_saved: usize,
}

/// MinCut-inspired importance scoring
/// Treats weight matrix as bipartite graph, finds min-cut to preserve information flow
pub struct MinCutScorer {
    /// Flow values from source to each input neuron
    input_flow: HVec<i32, MAX_PRUNING_UNITS>,
    /// Flow values from each output neuron to sink
    output_flow: HVec<i32, MAX_PRUNING_UNITS>,
}

impl MinCutScorer {
    /// Create scorer
    pub fn new() -> Self {
        Self {
            input_flow: HVec::new(),
            output_flow: HVec::new(),
        }
    }

    /// Compute edge importance using simplified max-flow
    /// Edges in min-cut are most critical for information flow
    pub fn compute_edge_importance(
        &mut self,
        weights: &[i8],
        input_dim: usize,
        output_dim: usize,
    ) -> HVec<i16, MAX_PRUNING_UNITS> {
        // Initialize flow (simplified: use column/row sums)
        self.input_flow.clear();
        self.output_flow.clear();

        // Input flow: sum of absolute weights per input
        for in_idx in 0..input_dim.min(MAX_PRUNING_UNITS) {
            let mut flow: i32 = 0;
            for out_idx in 0..output_dim {
                let w_idx = out_idx * input_dim + in_idx;
                if w_idx < weights.len() {
                    flow += (weights[w_idx] as i32).abs();
                }
            }
            let _ = self.input_flow.push(flow);
        }

        // Output flow: sum of absolute weights per output
        for out_idx in 0..output_dim.min(MAX_PRUNING_UNITS) {
            let mut flow: i32 = 0;
            for in_idx in 0..input_dim {
                let w_idx = out_idx * input_dim + in_idx;
                if w_idx < weights.len() {
                    flow += (weights[w_idx] as i32).abs();
                }
            }
            let _ = self.output_flow.push(flow);
        }

        // Edge importance = min(input_flow, output_flow) * |weight|
        // Edges on min-cut have bottleneck flow
        let mut importance: HVec<i16, MAX_PRUNING_UNITS> = HVec::new();

        for out_idx in 0..output_dim.min(self.output_flow.len()) {
            let out_flow = self.output_flow[out_idx];
            for in_idx in 0..input_dim.min(self.input_flow.len()) {
                let in_flow = self.input_flow[in_idx];
                let w_idx = out_idx * input_dim + in_idx;

                if w_idx < weights.len() {
                    let w = (weights[w_idx] as i32).abs();
                    let bottleneck = in_flow.min(out_flow);
                    let edge_importance = ((w * bottleneck) >> 10) as i16;

                    if importance.len() < MAX_PRUNING_UNITS {
                        let _ = importance.push(edge_importance);
                    }
                }
            }
        }

        importance
    }
}

impl Default for MinCutScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pruning_mask() {
        let mut mask = PruningMask::<64>::new(50).unwrap();

        assert!(mask.is_kept(0));
        assert!(mask.is_kept(49));
        assert_eq!(mask.sparsity(), 0.0);

        mask.prune(10);
        mask.prune(20);

        assert!(!mask.is_kept(10));
        assert!(!mask.is_kept(20));
        assert!(mask.is_kept(15));
        assert_eq!(mask.pruned_count, 2);
    }

    #[test]
    fn test_magnitude_pruning() {
        let config = PruningConfig {
            target_sparsity: 0.5,
            ..Default::default()
        };

        let mut pruner = LayerPruner::new(config);

        // Weights with varying magnitudes
        let weights: [i8; 8] = [1, -2, 50, -60, 3, -4, 70, 5];
        pruner.compute_magnitude_importance(&weights);

        let mask = pruner.create_mask::<8>(8).unwrap();

        // Should prune ~50% (low magnitude weights)
        assert!(mask.sparsity() >= 0.25 && mask.sparsity() <= 0.75);

        // High magnitude weights should be kept
        assert!(mask.is_kept(2)); // 50
        assert!(mask.is_kept(3)); // -60
        assert!(mask.is_kept(6)); // 70
    }

    #[test]
    fn test_structured_pruning() {
        let config = PruningConfig {
            target_sparsity: 0.5,
            structured: true,
            ..Default::default()
        };

        let mut pruner = LayerPruner::new(config);

        // 4x4 weight matrix
        let mut weights: [i8; 16] = [
            10, 10, 10, 10,   // High importance neuron
            1, 1, 1, 1,       // Low importance
            20, 20, 20, 20,   // High importance
            2, 2, 2, 2,       // Low importance
        ];

        let keep_mask = pruner.prune_neurons(&mut weights, 4, 4);

        // Should keep high importance neurons
        assert!(keep_mask[0]); // First neuron kept
        assert!(keep_mask[2]); // Third neuron kept

        // Low importance neurons should be zeroed
        if !keep_mask[1] {
            assert_eq!(weights[4], 0);
            assert_eq!(weights[5], 0);
        }
    }

    #[test]
    fn test_mincut_scorer() {
        let mut scorer = MinCutScorer::new();

        let weights: [i8; 9] = [
            10, 20, 30,
            5, 10, 15,
            1, 2, 3,
        ];

        let importance = scorer.compute_edge_importance(&weights, 3, 3);

        // Should have computed importance for edges
        assert!(!importance.is_empty());
    }
}
