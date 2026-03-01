//! Spike scheduler for event-driven inference.
//!
//! Implements spike-driven compute scheduling inspired by:
//! - **Spike-driven Transformer** (Yao et al., 2023) - Event-driven inference with 87× energy reduction
//! - **Spike-driven Transformer V2** (Yao et al., 2024) - Meta spiking architecture with novelty detection
//! - **Dynamic Sparse Attention** (Jiang et al., 2024) - Top-k position selection for 90% FLOPs reduction
//!
//! The spike scheduler determines whether to run inference at all,
//! and if so, at what compute tier based on event signals:
//! - **Firing status:** spike.fired == 1 means run, == 0 means skip
//! - **Rate-based tiers:** Higher rates trigger higher compute budgets
//! - **Novelty gating:** Low novelty reduces tier even when firing
//! - **Sparse routing:** Top-k positions guide attention sparsity
//!
//! ## References
//!
//! - Yao, M., et al. (2023). Spike-driven Transformer. NeurIPS 2023.
//! - Yao, M., et al. (2024). Spike-driven Transformer V2. ICLR 2024.
//! - Jiang, H., et al. (2024). MInference 1.0. NeurIPS 2024.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::packets::SpikePacket;

/// Spike-based scheduling decision.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpikeScheduleDecision {
    /// Whether to run inference
    pub should_run: bool,

    /// Suggested compute tier (0-3)
    pub suggested_tier: u8,

    /// Whether to use sparse attention mask
    pub use_sparse_mask: bool,

    /// Number of sparse positions to attend to
    pub sparse_positions: u8,
}

/// Spike scheduler configuration.
#[derive(Clone, Debug)]
pub struct SpikeSchedulerConfig {
    /// Rate threshold below which we skip (Q15)
    pub rate_skip_threshold_q15: u16,

    /// Rate threshold for tier 1 (Q15)
    pub rate_tier1_threshold_q15: u16,

    /// Rate threshold for tier 2 (Q15)
    pub rate_tier2_threshold_q15: u16,

    /// Novelty threshold below which we reduce tier (Q15)
    pub novelty_low_threshold_q15: u16,

    /// Novelty threshold for full attention (Q15)
    pub novelty_high_threshold_q15: u16,

    /// Minimum top-k entries for sparse attention
    pub sparse_min_positions: u8,
}

impl Default for SpikeSchedulerConfig {
    fn default() -> Self {
        Self {
            rate_skip_threshold_q15: 1024,     // ~3%
            rate_tier1_threshold_q15: 8192,    // ~25%
            rate_tier2_threshold_q15: 16384,   // ~50%
            novelty_low_threshold_q15: 4096,   // ~12.5%
            novelty_high_threshold_q15: 16384, // ~50%
            sparse_min_positions: 4,
        }
    }
}

/// Spike scheduler for event-driven compute decisions.
pub struct SpikeScheduler {
    /// Configuration
    config: SpikeSchedulerConfig,
}

impl SpikeScheduler {
    /// Create a new spike scheduler with default configuration.
    pub fn new() -> Self {
        Self {
            config: SpikeSchedulerConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: SpikeSchedulerConfig) -> Self {
        Self { config }
    }

    /// Evaluate spike packet and return scheduling decision.
    pub fn evaluate(&self, spike: &SpikePacket) -> SpikeScheduleDecision {
        // Check if spike fired
        if !spike.is_active() {
            return SpikeScheduleDecision {
                should_run: false,
                suggested_tier: 3,
                use_sparse_mask: false,
                sparse_positions: 0,
            };
        }

        // Determine tier based on rate and novelty
        let tier = self.compute_tier(spike);

        // Determine sparse attention
        let (use_sparse, positions) = self.compute_sparse(spike);

        SpikeScheduleDecision {
            should_run: true,
            suggested_tier: tier,
            use_sparse_mask: use_sparse,
            sparse_positions: positions,
        }
    }

    /// Compute suggested tier based on spike metrics.
    fn compute_tier(&self, spike: &SpikePacket) -> u8 {
        let rate = spike.rate_q15;
        let novelty = spike.novelty_q15;

        // Very low rate - skip or cheap
        if rate < self.config.rate_skip_threshold_q15 {
            return 3;
        }

        // Low novelty always degrades tier
        let novelty_penalty = if novelty < self.config.novelty_low_threshold_q15 {
            1
        } else {
            0
        };

        // Rate-based tier selection
        let rate_tier = if rate >= self.config.rate_tier2_threshold_q15 {
            0 // High rate - full compute
        } else if rate >= self.config.rate_tier1_threshold_q15 {
            1 // Medium rate - reduced
        } else {
            2 // Low rate - safe
        };

        // Apply novelty penalty (but cap at tier 2)
        (rate_tier + novelty_penalty).min(2)
    }

    /// Determine sparse attention parameters.
    fn compute_sparse(&self, spike: &SpikePacket) -> (bool, u8) {
        // Check if sparse mask is enabled in flags
        if !spike.use_sparse_mask() {
            return (false, 0);
        }

        // Check if we have enough top-k entries
        if spike.top_len < self.config.sparse_min_positions {
            return (false, 0);
        }

        (true, spike.top_len)
    }

    /// Build a sparse attention mask from spike top-k indices.
    ///
    /// Returns a bitmask where bit `i` is set if position `i` should be attended to.
    pub fn build_sparse_mask(&self, spike: &SpikePacket, max_positions: usize) -> Vec<bool> {
        let mut mask = vec![false; max_positions];

        if spike.use_sparse_mask() {
            for &idx in spike.top_indices() {
                let idx = idx as usize;
                if idx < max_positions {
                    mask[idx] = true;
                }
            }
        }

        mask
    }

    /// Get weighted sparse mask with attention weights.
    ///
    /// Returns (index, weight) pairs sorted by weight descending.
    pub fn get_weighted_positions(&self, spike: &SpikePacket) -> Vec<(u16, f32)> {
        let indices = spike.top_indices();
        let weights = spike.top_weights();

        let mut positions: Vec<(u16, f32)> = indices
            .iter()
            .zip(weights.iter())
            .map(|(&idx, &w)| (idx, (w as f32) / 32768.0)) // Convert from Q15
            .collect();

        // Sort by weight descending
        positions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));

        positions
    }
}

impl Default for SpikeScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a simple hash for input signature.
///
/// Used when caller doesn't provide an explicit signature.
pub fn compute_input_signature(tokens: &[u32]) -> u64 {
    // Simple FNV-1a hash
    let mut hash: u64 = 0xcbf29ce484222325;
    for &token in tokens {
        hash ^= token as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Compute signature from quantized embedding.
pub fn compute_embedding_signature(embedding: &[i8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &val in embedding {
        hash ^= val as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_skip_inactive() {
        let scheduler = SpikeScheduler::new();
        let spike = SpikePacket {
            fired: 0,
            ..Default::default()
        };

        let decision = scheduler.evaluate(&spike);
        assert!(!decision.should_run);
        assert_eq!(decision.suggested_tier, 3);
    }

    #[test]
    fn test_scheduler_high_rate() {
        let scheduler = SpikeScheduler::new();
        let spike = SpikePacket {
            fired: 1,
            rate_q15: 20000,    // High rate
            novelty_q15: 20000, // High novelty
            ..Default::default()
        };

        let decision = scheduler.evaluate(&spike);
        assert!(decision.should_run);
        assert_eq!(decision.suggested_tier, 0);
    }

    #[test]
    fn test_scheduler_medium_rate() {
        let scheduler = SpikeScheduler::new();
        let spike = SpikePacket {
            fired: 1,
            rate_q15: 10000, // Medium rate
            novelty_q15: 20000,
            ..Default::default()
        };

        let decision = scheduler.evaluate(&spike);
        assert!(decision.should_run);
        assert_eq!(decision.suggested_tier, 1);
    }

    #[test]
    fn test_scheduler_low_novelty_penalty() {
        let scheduler = SpikeScheduler::new();
        let spike = SpikePacket {
            fired: 1,
            rate_q15: 20000,   // Would be tier 0
            novelty_q15: 2000, // Low novelty
            ..Default::default()
        };

        let decision = scheduler.evaluate(&spike);
        assert!(decision.should_run);
        assert_eq!(decision.suggested_tier, 1); // Penalized from 0 to 1
    }

    #[test]
    fn test_sparse_mask() {
        let scheduler = SpikeScheduler::new();
        let spike = SpikePacket {
            fired: 1,
            top_len: 3,
            top_idx: {
                let mut arr = [0u16; 16];
                arr[0] = 5;
                arr[1] = 10;
                arr[2] = 15;
                arr
            },
            flags: SpikePacket::FLAG_SPARSE_MASK,
            ..Default::default()
        };

        let mask = scheduler.build_sparse_mask(&spike, 20);
        assert!(mask[5]);
        assert!(mask[10]);
        assert!(mask[15]);
        assert!(!mask[0]);
        assert!(!mask[1]);
    }

    #[test]
    fn test_input_signature() {
        let tokens1 = [1, 2, 3, 4];
        let tokens2 = [1, 2, 3, 4];
        let tokens3 = [1, 2, 3, 5];

        assert_eq!(
            compute_input_signature(&tokens1),
            compute_input_signature(&tokens2)
        );
        assert_ne!(
            compute_input_signature(&tokens1),
            compute_input_signature(&tokens3)
        );
    }

    #[test]
    fn test_weighted_positions() {
        let scheduler = SpikeScheduler::new();
        let spike = SpikePacket {
            fired: 1,
            top_len: 3,
            top_idx: {
                let mut arr = [0u16; 16];
                arr[0] = 5;
                arr[1] = 10;
                arr[2] = 15;
                arr
            },
            top_w_q15: {
                let mut arr = [0u16; 16];
                arr[0] = 8192; // 0.25
                arr[1] = 16384; // 0.5
                arr[2] = 4096; // 0.125
                arr
            },
            ..Default::default()
        };

        let positions = scheduler.get_weighted_positions(&spike);
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0].0, 10); // Highest weight first
    }
}
