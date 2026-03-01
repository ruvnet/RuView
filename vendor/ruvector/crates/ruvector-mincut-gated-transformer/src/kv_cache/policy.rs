//! Tier transition and rematerialization policies.
//!
//! Determines when to:
//! - Quantize tokens (move from hot to warm, warm to archive)
//! - Rematerialize (trade compute for memory under extreme pressure)
//! - Adapt tier boundaries based on quality metrics

#[cfg(feature = "no_std_gateway")]
use alloc::vec::Vec;

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

use super::tier::TierBoundary;

/// Decision for token eviction/quantization
#[derive(Clone, Debug, PartialEq)]
pub enum EvictionDecision {
    /// Keep in current tier (no action needed)
    Keep,
    /// Evict and optionally recompute on access
    Evict { recompute_on_access: bool },
    /// Quantize to a target bit width
    Quantize { target_bits: u8 },
    /// Move to next tier (hot->warm, warm->archive)
    Graduate,
}

/// Memory usage tracker
#[derive(Debug, Clone)]
pub struct MemoryTracker {
    /// Current memory usage in bytes
    current_bytes: usize,
    /// Peak memory usage in bytes
    peak_bytes: usize,
    /// Available memory in bytes
    available_bytes: usize,
    /// History of memory usage (for trend analysis)
    history: Vec<usize>,
    /// Maximum history entries to keep
    max_history: usize,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new(available_bytes: usize) -> Self {
        Self {
            current_bytes: 0,
            peak_bytes: 0,
            available_bytes,
            history: Vec::new(),
            max_history: 100,
        }
    }

    /// Update current memory usage
    pub fn update(&mut self, bytes: usize) {
        self.current_bytes = bytes;
        self.peak_bytes = self.peak_bytes.max(bytes);

        self.history.push(bytes);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Get current memory pressure (0.0 - 1.0)
    pub fn pressure(&self) -> f32 {
        if self.available_bytes == 0 {
            1.0
        } else {
            self.current_bytes as f32 / self.available_bytes as f32
        }
    }

    /// Check if memory is under pressure
    pub fn is_under_pressure(&self, threshold: f32) -> bool {
        self.pressure() >= threshold
    }

    /// Get memory trend (positive = increasing, negative = decreasing)
    pub fn trend(&self) -> f32 {
        if self.history.len() < 2 {
            return 0.0;
        }

        let recent = self.history.len().saturating_sub(10);
        let recent_avg = self.history[recent..].iter().sum::<usize>() as f32
            / (self.history.len() - recent) as f32;

        let earlier = recent.saturating_sub(10);
        let earlier_avg = self.history[earlier..recent].iter().sum::<usize>() as f32
            / (recent - earlier).max(1) as f32;

        (recent_avg - earlier_avg) / earlier_avg.max(1.0)
    }

    /// Get current usage
    pub fn current_usage(&self) -> usize {
        self.current_bytes
    }

    /// Get peak usage
    pub fn peak_usage(&self) -> usize {
        self.peak_bytes
    }

    /// Get available memory
    pub fn available(&self) -> usize {
        self.available_bytes
    }
}

/// Policy for tier transitions
pub struct TierPolicy {
    /// Current tier boundaries
    boundary: TierBoundary,
    /// Quality target (1.0 - expected PPL degradation)
    quality_target: f32,
    /// Minimum hot buffer size
    min_hot_size: usize,
    /// Maximum hot buffer size
    max_hot_size: usize,
    /// Whether to use adaptive boundaries
    adaptive: bool,
}

impl TierPolicy {
    /// Create a new tier policy
    pub fn new(boundary: TierBoundary, quality_target: f32) -> Self {
        Self {
            boundary,
            quality_target,
            min_hot_size: 32,
            max_hot_size: 512,
            adaptive: true,
        }
    }

    /// Create a fixed (non-adaptive) policy
    pub fn fixed(boundary: TierBoundary) -> Self {
        Self {
            boundary,
            quality_target: 0.95,
            min_hot_size: boundary.hot_threshold,
            max_hot_size: boundary.hot_threshold,
            adaptive: false,
        }
    }

    /// Get current tier boundaries
    pub fn boundary(&self) -> &TierBoundary {
        &self.boundary
    }

    /// Determine if a token should transition to next tier
    pub fn should_graduate(&self, age: usize, quality_score: f32) -> EvictionDecision {
        // If quality is good, can be more aggressive
        let adjusted_hot = if quality_score > self.quality_target * 1.05 {
            (self.boundary.hot_threshold as f32 * 0.8) as usize
        } else if quality_score < self.quality_target {
            (self.boundary.hot_threshold as f32 * 1.2) as usize
        } else {
            self.boundary.hot_threshold
        };

        if age < adjusted_hot.clamp(self.min_hot_size, self.max_hot_size) {
            EvictionDecision::Keep
        } else if age < self.boundary.warm_threshold {
            EvictionDecision::Quantize { target_bits: 4 }
        } else {
            EvictionDecision::Quantize { target_bits: 2 }
        }
    }

    /// Expand hot boundary (when quality is degrading)
    pub fn expand_hot_boundary(&mut self, factor: f32) {
        if !self.adaptive {
            return;
        }

        let new_hot = (self.boundary.hot_threshold as f32 * factor) as usize;
        self.boundary.hot_threshold = new_hot.clamp(self.min_hot_size, self.max_hot_size);
    }

    /// Shrink hot boundary (when quality is good, can be more aggressive)
    pub fn shrink_hot_boundary(&mut self, factor: f32) {
        if !self.adaptive {
            return;
        }

        let new_hot = (self.boundary.hot_threshold as f32 * factor) as usize;
        self.boundary.hot_threshold = new_hot.clamp(self.min_hot_size, self.max_hot_size);
    }

    /// Set adaptive mode
    pub fn set_adaptive(&mut self, adaptive: bool) {
        self.adaptive = adaptive;
    }
}

/// Cost model for rematerialization
#[derive(Debug, Clone)]
pub struct RematerializationCostModel {
    /// Cost to recompute one layer's KV for one token (in FLOPs)
    pub flops_per_token_per_layer: usize,
    /// Memory saved by evicting one token's KV (in bytes)
    pub bytes_per_token: usize,
    /// Current available compute budget
    pub compute_budget: usize,
}

impl Default for RematerializationCostModel {
    fn default() -> Self {
        Self {
            // Approximate for a 7B model
            flops_per_token_per_layer: 2 * 4096 * 4096, // 2 * hidden^2
            bytes_per_token: 4096 * 2 * 2,              // hidden * 2 (kv) * 2 (fp16)
            compute_budget: 1_000_000_000,              // 1 GFLOP budget
        }
    }
}

/// Policy for rematerialization (trading compute for memory)
pub struct RematerializationPolicy {
    /// Memory pressure threshold to trigger rematerialization
    memory_threshold: f32,
    /// Minimum tokens to keep materialized
    min_materialized: usize,
    /// Cost model
    cost_model: RematerializationCostModel,
    /// Memory tracker
    memory_tracker: MemoryTracker,
}

impl RematerializationPolicy {
    /// Create a new rematerialization policy
    pub fn new(memory_threshold: f32, min_materialized: usize) -> Self {
        Self {
            memory_threshold,
            min_materialized,
            cost_model: RematerializationCostModel::default(),
            memory_tracker: MemoryTracker::new(16 * 1024 * 1024 * 1024), // 16GB default
        }
    }

    /// Create with custom cost model
    pub fn with_cost_model(mut self, cost_model: RematerializationCostModel) -> Self {
        self.cost_model = cost_model;
        self
    }

    /// Set available memory
    pub fn set_available_memory(&mut self, bytes: usize) {
        self.memory_tracker = MemoryTracker::new(bytes);
    }

    /// Update current memory usage
    pub fn update_memory(&mut self, bytes: usize) {
        self.memory_tracker.update(bytes);
    }

    /// Evaluate if eviction/rematerialization should occur
    pub fn evaluate(&self, current_bytes: usize) -> Option<EvictionDecision> {
        let pressure = current_bytes as f32 / self.memory_tracker.available() as f32;

        if pressure < self.memory_threshold {
            return None;
        }

        // Calculate cost-benefit of rematerialization
        let recompute_cost = self.cost_model.flops_per_token_per_layer;
        let _memory_benefit = self.cost_model.bytes_per_token;

        // Favor quantization over eviction if compute budget is low
        if recompute_cost > self.cost_model.compute_budget {
            Some(EvictionDecision::Quantize { target_bits: 2 })
        } else {
            Some(EvictionDecision::Evict {
                recompute_on_access: true,
            })
        }
    }

    /// Decide whether to evict or keep a specific token
    pub fn should_evict(
        &self,
        token_position: usize,
        layer: usize,
        total_tokens: usize,
    ) -> EvictionDecision {
        let pressure = self.memory_tracker.pressure();

        if pressure < self.memory_threshold {
            return EvictionDecision::Keep;
        }

        // Older tokens are better eviction candidates
        let age = total_tokens.saturating_sub(token_position);
        let relative_age = age as f32 / total_tokens.max(1) as f32;

        // Calculate adjusted cost
        let recompute_cost = self.cost_model.flops_per_token_per_layer * (layer + 1);
        let age_factor = 1.0 / (1.0 + relative_age);
        let adjusted_cost = recompute_cost as f32 * age_factor;

        if total_tokens <= self.min_materialized {
            EvictionDecision::Keep
        } else if adjusted_cost < self.cost_model.compute_budget as f32 {
            EvictionDecision::Evict {
                recompute_on_access: true,
            }
        } else {
            EvictionDecision::Quantize { target_bits: 2 }
        }
    }

    /// Get current memory pressure
    pub fn memory_pressure(&self) -> f32 {
        self.memory_tracker.pressure()
    }

    /// Get memory tracker
    pub fn memory_tracker(&self) -> &MemoryTracker {
        &self.memory_tracker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker() {
        let mut tracker = MemoryTracker::new(1000);

        tracker.update(100);
        assert_eq!(tracker.current_usage(), 100);
        assert_eq!(tracker.pressure(), 0.1);
        assert!(!tracker.is_under_pressure(0.5));

        tracker.update(900);
        assert_eq!(tracker.pressure(), 0.9);
        assert!(tracker.is_under_pressure(0.5));
    }

    #[test]
    fn test_memory_tracker_peak() {
        let mut tracker = MemoryTracker::new(1000);

        tracker.update(500);
        tracker.update(300);
        assert_eq!(tracker.peak_usage(), 500);
        assert_eq!(tracker.current_usage(), 300);
    }

    #[test]
    fn test_tier_policy_should_graduate() {
        let boundary = TierBoundary::new(64, 512);
        let policy = TierPolicy::new(boundary, 0.95);

        // Young token: keep
        assert_eq!(policy.should_graduate(10, 0.97), EvictionDecision::Keep);

        // Medium age: quantize to 4-bit (warm tier)
        assert_eq!(
            policy.should_graduate(100, 0.97),
            EvictionDecision::Quantize { target_bits: 4 }
        );

        // Old token: quantize to 2-bit (archive tier)
        assert_eq!(
            policy.should_graduate(600, 0.97),
            EvictionDecision::Quantize { target_bits: 2 }
        );
    }

    #[test]
    fn test_tier_policy_adaptive() {
        let boundary = TierBoundary::new(64, 512);
        let mut policy = TierPolicy::new(boundary, 0.95);

        assert_eq!(policy.boundary().hot_threshold, 64);

        policy.expand_hot_boundary(1.5);
        assert!(policy.boundary().hot_threshold > 64);

        policy.shrink_hot_boundary(0.5);
        assert!(policy.boundary().hot_threshold < 96);
    }

    #[test]
    fn test_tier_policy_fixed() {
        let boundary = TierBoundary::new(64, 512);
        let mut policy = TierPolicy::fixed(boundary);

        let original = policy.boundary().hot_threshold;
        policy.expand_hot_boundary(2.0);
        assert_eq!(policy.boundary().hot_threshold, original);
    }

    #[test]
    fn test_rematerialization_policy() {
        let mut policy = RematerializationPolicy::new(0.9, 512);
        policy.set_available_memory(1000);

        // Low pressure: no action
        let decision = policy.evaluate(500);
        assert!(decision.is_none());

        // High pressure: should recommend action
        let decision = policy.evaluate(950);
        assert!(decision.is_some());
    }

    #[test]
    fn test_rematerialization_should_evict() {
        let mut policy = RematerializationPolicy::new(0.8, 100);
        policy.set_available_memory(1000);
        policy.update_memory(900);

        // Old token under pressure: might evict
        let decision = policy.should_evict(0, 0, 1000);
        assert_ne!(decision, EvictionDecision::Keep);

        // Reset to low pressure
        policy.update_memory(100);
        let decision = policy.should_evict(0, 0, 1000);
        assert_eq!(decision, EvictionDecision::Keep);
    }

    #[test]
    fn test_cost_model_default() {
        let model = RematerializationCostModel::default();
        assert!(model.flops_per_token_per_layer > 0);
        assert!(model.bytes_per_token > 0);
        assert!(model.compute_budget > 0);
    }
}
