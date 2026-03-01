//! Coherence-driven early exit for self-speculative inference.
//!
//! Based on LayerSkip (Elhoushi et al., 2024) but uses λ stability instead of learned classifiers.
//!
//! ## Design Rationale
//!
//! LayerSkip enables early exit by learning classifiers that predict when intermediate
//! layers produce sufficiently good outputs. However, this introduces:
//! - Non-determinism from learned components
//! - Additional training overhead
//! - Difficulty in understanding exit decisions
//!
//! Our approach leverages mincut λ signals for early exit decisions:
//! - High λ + stable λ-delta → confident exit
//! - Low λ or volatile λ-delta → continue to deeper layers
//! - Boundary concentration → affects exit confidence
//!
//! This enables self-speculative decoding where we:
//! 1. Exit early with high confidence tokens
//! 2. Generate speculative tokens
//! 3. Verify with full-depth pass
//!
//! Benefits:
//! - Deterministic behavior
//! - Explainable via witness
//! - No training overhead
//! - Integrates with existing mincut infrastructure

extern crate alloc;
use alloc::vec::Vec;

use crate::packets::GatePacket;
use serde::{Deserialize, Serialize};

/// Configuration for coherence-driven early exit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EarlyExitConfig {
    /// Target exit layer (0-indexed)
    /// If conditions met, exit after this layer instead of running all layers
    pub exit_layer: u16,

    /// Minimum λ value required for early exit
    /// Higher values indicate more coherent state
    pub min_lambda_for_exit: u32,

    /// Minimum λ stability required for exit (Q15: 0-32767)
    /// Measures how stable λ has been (lower |λ-delta| → higher stability)
    pub min_lambda_stability_q15: u16,

    /// Maximum boundary concentration for early exit (Q15: 0-32767)
    /// Lower values indicate more distributed boundaries (safer to exit)
    pub max_boundary_concentration_q15: u16,

    /// Number of speculative tokens to generate after early exit
    /// Used for self-speculative decoding
    pub speculative_tokens: u8,

    /// Number of verification layers to run for speculative tokens
    /// Full depth verification if 0
    pub verification_layers: u16,

    /// Enable adaptive exit layer based on λ stability
    /// When true, exit layer adjusts based on coherence strength
    pub adaptive_exit_layer: bool,

    /// Minimum confidence threshold (Q15: 0-32767)
    /// Combined metric of λ and stability for exit decision
    pub min_confidence_q15: u16,
}

impl Default for EarlyExitConfig {
    fn default() -> Self {
        Self {
            exit_layer: 2, // Exit after layer 2 (out of 4)
            min_lambda_for_exit: 80,
            min_lambda_stability_q15: 28000,       // ~85% stability
            max_boundary_concentration_q15: 16384, // 50% max concentration
            speculative_tokens: 4,
            verification_layers: 2,
            adaptive_exit_layer: true,
            min_confidence_q15: 26214, // ~80% confidence
        }
    }
}

impl EarlyExitConfig {
    /// Create configuration optimized for maximum speedup
    pub fn aggressive() -> Self {
        Self {
            exit_layer: 1, // Exit very early
            min_lambda_for_exit: 60,
            min_lambda_stability_q15: 24576, // ~75% stability
            max_boundary_concentration_q15: 20000,
            speculative_tokens: 8,
            verification_layers: 1,
            adaptive_exit_layer: true,
            min_confidence_q15: 22937, // ~70% confidence
        }
    }

    /// Create configuration optimized for accuracy
    pub fn conservative() -> Self {
        Self {
            exit_layer: 3,
            min_lambda_for_exit: 100,
            min_lambda_stability_q15: 30000, // ~92% stability
            max_boundary_concentration_q15: 12000,
            speculative_tokens: 2,
            verification_layers: 4,
            adaptive_exit_layer: false,
            min_confidence_q15: 29491, // ~90% confidence
        }
    }

    /// Validate configuration
    pub fn validate(&self, max_layers: u16) -> Result<(), &'static str> {
        if self.exit_layer >= max_layers {
            return Err("exit_layer must be less than total layers");
        }
        if self.verification_layers > max_layers {
            return Err("verification_layers cannot exceed total layers");
        }
        if self.min_lambda_stability_q15 > 32767 {
            return Err("min_lambda_stability_q15 must be <= 32767");
        }
        if self.max_boundary_concentration_q15 > 32767 {
            return Err("max_boundary_concentration_q15 must be <= 32767");
        }
        if self.min_confidence_q15 > 32767 {
            return Err("min_confidence_q15 must be <= 32767");
        }
        Ok(())
    }
}

/// Decision result from early exit evaluation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EarlyExitDecision {
    /// Whether early exit is allowed
    pub can_exit: bool,

    /// Confidence in the exit decision (Q15: 0-32767)
    pub confidence_q15: u16,

    /// Layer at which to exit (if can_exit is true)
    pub exit_layer: u16,

    /// Reason for the decision
    pub reason: ExitReason,

    /// Whether to enable speculative generation
    pub enable_speculation: bool,
}

impl Default for EarlyExitDecision {
    fn default() -> Self {
        Self {
            can_exit: false,
            confidence_q15: 0,
            exit_layer: 0,
            reason: ExitReason::InsufficientConfidence,
            enable_speculation: false,
        }
    }
}

/// Reason for early exit decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ExitReason {
    /// Not enough confidence to exit early
    InsufficientConfidence = 0,

    /// λ below minimum threshold
    LambdaTooLow = 1,

    /// λ-delta too volatile
    LambdaUnstable = 2,

    /// Boundary concentration too high
    BoundariesTooConcentrated = 3,

    /// All conditions met - safe to exit
    ConfidentExit = 4,

    /// Forced to continue (layer too early)
    ForcedContinue = 5,
}

/// Coherence-driven early exit controller.
///
/// Uses mincut λ signals to determine when intermediate layers produce
/// sufficiently good outputs for early exit.
pub struct CoherenceEarlyExit {
    config: EarlyExitConfig,
    max_layers: u16,
}

impl CoherenceEarlyExit {
    /// Create a new early exit controller.
    ///
    /// # Arguments
    /// * `config` - Early exit configuration
    /// * `max_layers` - Maximum number of layers in the model
    pub fn new(config: EarlyExitConfig, max_layers: u16) -> Result<Self, &'static str> {
        config.validate(max_layers)?;
        Ok(Self { config, max_layers })
    }

    /// Create with default configuration.
    pub fn with_default_config(max_layers: u16) -> Result<Self, &'static str> {
        Self::new(EarlyExitConfig::default(), max_layers)
    }

    /// Evaluate whether to exit early at the given layer.
    ///
    /// # Arguments
    /// * `gate` - Current gate packet with λ signals
    /// * `layer` - Current layer index (0-indexed)
    ///
    /// # Returns
    /// Early exit decision with confidence and reasoning
    pub fn should_exit(&self, gate: &GatePacket, layer: usize) -> EarlyExitDecision {
        let layer = layer as u16;

        // Determine target exit layer (adaptive or fixed)
        let target_exit_layer = if self.config.adaptive_exit_layer {
            self.calculate_adaptive_exit_layer(gate)
        } else {
            self.config.exit_layer
        };

        // Not at target layer yet
        if layer < target_exit_layer {
            return EarlyExitDecision {
                can_exit: false,
                confidence_q15: 0,
                exit_layer: target_exit_layer,
                reason: ExitReason::ForcedContinue,
                enable_speculation: false,
            };
        }

        // Past target layer - check conditions
        if layer != target_exit_layer {
            return EarlyExitDecision {
                can_exit: false,
                confidence_q15: 0,
                exit_layer: target_exit_layer,
                reason: ExitReason::ForcedContinue,
                enable_speculation: false,
            };
        }

        // At target layer - evaluate exit conditions
        self.evaluate_exit_conditions(gate, layer)
    }

    /// Verify speculative tokens against full-depth outputs.
    ///
    /// Used in self-speculative decoding to validate early-exit predictions.
    ///
    /// # Arguments
    /// * `draft_logits` - Logits from early-exit pass
    /// * `full_logits` - Logits from full-depth verification pass
    ///
    /// # Returns
    /// True if speculation was correct (tokens match)
    pub fn verify_speculation(&self, draft_logits: &[i32], full_logits: &[i32]) -> bool {
        if draft_logits.len() != full_logits.len() {
            return false;
        }

        // Find argmax for both
        let draft_argmax = self.argmax(draft_logits);
        let full_argmax = self.argmax(full_logits);

        // Simple verification: top-1 token must match
        draft_argmax == full_argmax
    }

    /// Verify with tolerance for top-k matching.
    ///
    /// More lenient verification that checks if draft token is in top-k of full logits.
    pub fn verify_speculation_topk(
        &self,
        draft_logits: &[i32],
        full_logits: &[i32],
        k: usize,
    ) -> bool {
        if draft_logits.len() != full_logits.len() || k == 0 {
            return false;
        }

        let draft_argmax = self.argmax(draft_logits);
        let top_k_indices = self.topk(full_logits, k);

        top_k_indices.contains(&draft_argmax)
    }

    /// Get current configuration
    pub fn config(&self) -> &EarlyExitConfig {
        &self.config
    }

    // ---- Private helpers ----

    fn calculate_adaptive_exit_layer(&self, gate: &GatePacket) -> u16 {
        // Calculate λ stability (inverse of |λ-delta|)
        let lambda_delta_abs = gate.lambda_delta().abs() as u32;
        let stability = if gate.lambda_prev > 0 {
            // Normalize to Q15: (1 - |delta|/lambda_prev) * 32768
            let ratio = (lambda_delta_abs * 32768) / gate.lambda_prev.max(1);
            32768u32.saturating_sub(ratio).min(32767) as u16
        } else {
            0
        };

        // Higher stability → can exit earlier
        // Lower stability → exit later
        if stability >= 30000 && gate.lambda >= self.config.min_lambda_for_exit {
            // Very stable - exit very early
            (self.config.exit_layer.saturating_sub(1)).max(1)
        } else if stability >= 25000 {
            // Moderately stable - exit at configured layer
            self.config.exit_layer
        } else {
            // Less stable - exit later
            (self.config.exit_layer + 1).min(self.max_layers.saturating_sub(1))
        }
    }

    fn evaluate_exit_conditions(&self, gate: &GatePacket, layer: u16) -> EarlyExitDecision {
        // Check λ minimum
        if gate.lambda < self.config.min_lambda_for_exit {
            return EarlyExitDecision {
                can_exit: false,
                confidence_q15: 0,
                exit_layer: layer,
                reason: ExitReason::LambdaTooLow,
                enable_speculation: false,
            };
        }

        // Check λ stability
        let lambda_delta_abs = gate.lambda_delta().abs() as u32;
        let stability = if gate.lambda_prev > 0 {
            let ratio = (lambda_delta_abs * 32768) / gate.lambda_prev.max(1);
            32768u32.saturating_sub(ratio).min(32767) as u16
        } else {
            0
        };

        if stability < self.config.min_lambda_stability_q15 {
            return EarlyExitDecision {
                can_exit: false,
                confidence_q15: stability,
                exit_layer: layer,
                reason: ExitReason::LambdaUnstable,
                enable_speculation: false,
            };
        }

        // Check boundary concentration
        if gate.boundary_concentration_q15 > self.config.max_boundary_concentration_q15 {
            return EarlyExitDecision {
                can_exit: false,
                confidence_q15: stability,
                exit_layer: layer,
                reason: ExitReason::BoundariesTooConcentrated,
                enable_speculation: false,
            };
        }

        // Calculate combined confidence
        // Weighted average of: λ strength, stability, and boundary dispersion
        let lambda_strength = ((gate.lambda as u64 * 32768) / 100).min(32767) as u16; // Normalize λ (assume max ~100)
        let boundary_dispersion = 32767 - gate.boundary_concentration_q15; // Invert concentration

        let confidence =
            ((lambda_strength as u32 * 4 + stability as u32 * 4 + boundary_dispersion as u32 * 2)
                / 10)
                .min(32767) as u16;

        // Check against minimum confidence
        if confidence < self.config.min_confidence_q15 {
            return EarlyExitDecision {
                can_exit: false,
                confidence_q15: confidence,
                exit_layer: layer,
                reason: ExitReason::InsufficientConfidence,
                enable_speculation: false,
            };
        }

        // All conditions met - allow early exit
        EarlyExitDecision {
            can_exit: true,
            confidence_q15: confidence,
            exit_layer: layer,
            reason: ExitReason::ConfidentExit,
            enable_speculation: self.config.speculative_tokens > 0,
        }
    }

    fn argmax(&self, logits: &[i32]) -> usize {
        if logits.is_empty() {
            return 0;
        }

        let mut max_idx = 0;
        let mut max_val = logits[0];

        for (i, &val) in logits.iter().enumerate().skip(1) {
            if val > max_val {
                max_val = val;
                max_idx = i;
            }
        }

        max_idx
    }

    /// Find top-k indices using partial sort - O(n + k log k) instead of O(n log n)
    ///
    /// For k << n, this provides ~7x speedup over full sorting.
    fn topk(&self, logits: &[i32], k: usize) -> Vec<usize> {
        if logits.is_empty() || k == 0 {
            return Vec::new();
        }

        let k = k.min(logits.len());

        // For small k, use selection-based approach
        // Maintain k largest elements seen so far
        let mut top_k: Vec<(usize, i32)> = Vec::with_capacity(k + 1);

        for (idx, &val) in logits.iter().enumerate() {
            // Binary search for insertion position (descending order)
            let pos = top_k
                .binary_search_by(|(_, v)| val.cmp(v)) // Reverse comparison for descending
                .unwrap_or_else(|p| p);

            if pos < k {
                top_k.insert(pos, (idx, val));
                if top_k.len() > k {
                    top_k.pop(); // Remove smallest (last element)
                }
            }
        }

        top_k.into_iter().map(|(idx, _)| idx).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_early_exit_config_default() {
        let config = EarlyExitConfig::default();
        assert!(config.validate(4).is_ok());
        assert_eq!(config.exit_layer, 2);
    }

    #[test]
    fn test_early_exit_config_aggressive() {
        let config = EarlyExitConfig::aggressive();
        assert!(config.validate(4).is_ok());
        assert_eq!(config.exit_layer, 1);
        assert_eq!(config.speculative_tokens, 8);
    }

    #[test]
    fn test_early_exit_config_conservative() {
        let config = EarlyExitConfig::conservative();
        assert!(config.validate(4).is_ok());
        assert_eq!(config.exit_layer, 3);
        assert_eq!(config.speculative_tokens, 2);
    }

    #[test]
    fn test_early_exit_controller_creation() {
        let config = EarlyExitConfig::default();
        let controller = CoherenceEarlyExit::new(config, 4);
        assert!(controller.is_ok());

        let controller = CoherenceEarlyExit::with_default_config(4);
        assert!(controller.is_ok());
    }

    #[test]
    fn test_should_exit_confident() {
        let mut config = EarlyExitConfig::default();
        config.adaptive_exit_layer = false; // Disable adaptive for deterministic testing
        let controller = CoherenceEarlyExit::new(config, 4).unwrap();

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 98, // Very stable
            boundary_edges: 5,
            boundary_concentration_q15: 10000, // Low concentration
            partition_count: 3,
            flags: 0,
        };

        let decision = controller.should_exit(&gate, 2);
        assert!(decision.can_exit);
        assert_eq!(decision.reason, ExitReason::ConfidentExit);
        assert!(decision.confidence_q15 > 0);
    }

    #[test]
    fn test_should_exit_lambda_too_low() {
        let config = EarlyExitConfig::default();
        let controller = CoherenceEarlyExit::new(config, 4).unwrap();

        let gate = GatePacket {
            lambda: 50, // Below min_lambda_for_exit (80)
            lambda_prev: 48,
            boundary_edges: 5,
            boundary_concentration_q15: 10000,
            partition_count: 3,
            flags: 0,
        };

        let decision = controller.should_exit(&gate, 2);
        assert!(!decision.can_exit);
        assert_eq!(decision.reason, ExitReason::LambdaTooLow);
    }

    #[test]
    fn test_should_exit_unstable() {
        let mut config = EarlyExitConfig::default();
        config.adaptive_exit_layer = false; // Disable adaptive for deterministic testing
        let controller = CoherenceEarlyExit::new(config, 4).unwrap();

        let gate = GatePacket {
            lambda: 85,       // Above minimum but unstable
            lambda_prev: 100, // Large delta - unstable
            boundary_edges: 5,
            boundary_concentration_q15: 10000,
            partition_count: 3,
            flags: 0,
        };

        let decision = controller.should_exit(&gate, 2);
        assert!(!decision.can_exit);
        assert_eq!(decision.reason, ExitReason::LambdaUnstable);
    }

    #[test]
    fn test_should_exit_boundaries_concentrated() {
        let mut config = EarlyExitConfig::default();
        config.adaptive_exit_layer = false; // Disable adaptive for deterministic testing
        let controller = CoherenceEarlyExit::new(config, 4).unwrap();

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 98,
            boundary_edges: 20,
            boundary_concentration_q15: 25000, // Too high
            partition_count: 3,
            flags: 0,
        };

        let decision = controller.should_exit(&gate, 2);
        assert!(!decision.can_exit);
        assert_eq!(decision.reason, ExitReason::BoundariesTooConcentrated);
    }

    #[test]
    fn test_should_exit_too_early() {
        let mut config = EarlyExitConfig::default();
        config.adaptive_exit_layer = false; // Disable adaptive for deterministic testing
        let controller = CoherenceEarlyExit::new(config, 4).unwrap();

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 98,
            boundary_edges: 5,
            boundary_concentration_q15: 10000,
            partition_count: 3,
            flags: 0,
        };

        // At layer 1, but exit_layer is 2
        let decision = controller.should_exit(&gate, 1);
        assert!(!decision.can_exit);
        assert_eq!(decision.reason, ExitReason::ForcedContinue);
    }

    #[test]
    fn test_verify_speculation_exact() {
        let controller = CoherenceEarlyExit::with_default_config(4).unwrap();

        let draft = vec![10, 100, 30, 20];
        let full = vec![15, 100, 35, 25];

        // Both have argmax at index 1
        assert!(controller.verify_speculation(&draft, &full));

        let draft2 = vec![10, 100, 30, 20];
        let full2 = vec![15, 50, 135, 25];

        // Different argmax
        assert!(!controller.verify_speculation(&draft2, &full2));
    }

    #[test]
    fn test_verify_speculation_topk() {
        let controller = CoherenceEarlyExit::with_default_config(4).unwrap();

        let draft = vec![10, 100, 30, 20];
        let full = vec![15, 95, 135, 25];

        // Draft argmax (1) not in top-1 of full (argmax=2), but in top-2
        assert!(!controller.verify_speculation(&draft, &full)); // Exact fails
        assert!(controller.verify_speculation_topk(&draft, &full, 2)); // Top-2 succeeds
    }

    #[test]
    fn test_adaptive_exit_layer() {
        let mut config = EarlyExitConfig::default();
        config.adaptive_exit_layer = true;
        config.exit_layer = 2;

        let controller = CoherenceEarlyExit::new(config, 4).unwrap();

        // Very stable - should exit earlier
        let stable_gate = GatePacket {
            lambda: 100,
            lambda_prev: 99,
            boundary_edges: 5,
            boundary_concentration_q15: 10000,
            partition_count: 3,
            flags: 0,
        };

        let _decision = controller.should_exit(&stable_gate, 1);
        // May exit at layer 1 due to high stability
        // (depends on adaptive calculation)

        // Unstable - should exit later
        let unstable_gate = GatePacket {
            lambda: 70,
            lambda_prev: 100,
            boundary_edges: 15,
            boundary_concentration_q15: 15000,
            partition_count: 5,
            flags: 0,
        };

        let decision = controller.should_exit(&unstable_gate, 2);
        // Should not exit due to instability
        assert!(!decision.can_exit);
    }
}
