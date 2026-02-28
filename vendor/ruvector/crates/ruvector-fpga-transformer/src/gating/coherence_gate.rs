//! Coherence-based gating for inference control

use crate::types::{ComputeClass, GateDecision, GateHint, SkipReason};
use crate::witness::WitnessLog;

/// Trait for coherence-based gating
pub trait CoherenceGate: Send + Sync {
    /// Preflight check before inference
    ///
    /// Returns a gate decision based on coherence signals.
    fn preflight(&self, hint: &GateHint) -> GateDecision;

    /// Layer checkpoint for early exit decisions
    ///
    /// Called after each layer to determine if early exit is appropriate.
    /// Returns Some(decision) to exit early, None to continue.
    fn checkpoint(&self, layer: u8, signal_q: i16) -> Option<GateDecision>;

    /// Check if write is allowed based on witness
    ///
    /// Used to gate state changes in memory systems.
    fn allow_write(&self, witness: &WitnessLog) -> bool;
}

/// Configuration for coherence gate
#[derive(Debug, Clone)]
pub struct CoherenceConfig {
    /// Minimum coherence score to run (Q8.8)
    pub min_coherence: i16,
    /// Coherence threshold for early exit
    pub early_exit_threshold: i16,
    /// Enable early exit
    pub early_exit_enabled: bool,
    /// Minimum layers before early exit
    pub min_layers: u8,
    /// Require stable coherence for writes
    pub require_stable_for_write: bool,
    /// Minimum coherence for writes (Q8.8)
    pub min_write_coherence: i16,
}

impl Default for CoherenceConfig {
    fn default() -> Self {
        Self {
            min_coherence: -256,       // -1.0 in Q8.8, very permissive
            early_exit_threshold: 512, // 2.0 in Q8.8
            early_exit_enabled: true,
            min_layers: 2,
            require_stable_for_write: true,
            min_write_coherence: 0, // Require non-negative coherence
        }
    }
}

impl CoherenceConfig {
    /// Create a strict configuration
    pub fn strict() -> Self {
        Self {
            min_coherence: 0,
            early_exit_threshold: 256,
            early_exit_enabled: true,
            min_layers: 4,
            require_stable_for_write: true,
            min_write_coherence: 128,
        }
    }

    /// Create a permissive configuration (always allows)
    pub fn permissive() -> Self {
        Self {
            min_coherence: i16::MIN,
            early_exit_threshold: i16::MAX,
            early_exit_enabled: false,
            min_layers: 0,
            require_stable_for_write: false,
            min_write_coherence: i16::MIN,
        }
    }
}

/// Default coherence gate implementation
pub struct DefaultCoherenceGate {
    config: CoherenceConfig,
}

impl DefaultCoherenceGate {
    /// Create with default config
    pub fn new() -> Self {
        Self::with_config(CoherenceConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: CoherenceConfig) -> Self {
        Self { config }
    }

    /// Check if compute class allows operation
    fn check_compute_class(&self, hint: &GateHint) -> bool {
        // Reflex class can always run (fast path)
        // Higher classes require sufficient coherence
        match hint.max_compute_class {
            ComputeClass::Reflex => true,
            ComputeClass::Associative => hint.coherence_score_q >= self.config.min_coherence / 2,
            ComputeClass::Deliberative => hint.coherence_score_q >= self.config.min_coherence,
        }
    }
}

impl Default for DefaultCoherenceGate {
    fn default() -> Self {
        Self::new()
    }
}

impl CoherenceGate for DefaultCoherenceGate {
    fn preflight(&self, hint: &GateHint) -> GateDecision {
        // Check minimum coherence
        if hint.coherence_score_q < self.config.min_coherence {
            return GateDecision::Skipped {
                reason: SkipReason::LowCoherence,
            };
        }

        // Check compute class restrictions
        if !self.check_compute_class(hint) {
            return GateDecision::Skipped {
                reason: SkipReason::BudgetExceeded,
            };
        }

        // Allow full inference
        GateDecision::RanFull
    }

    fn checkpoint(&self, layer: u8, signal_q: i16) -> Option<GateDecision> {
        if !self.config.early_exit_enabled {
            return None;
        }

        if layer < self.config.min_layers {
            return None;
        }

        // Check if coherence signal is high enough to exit early
        if signal_q >= self.config.early_exit_threshold {
            return Some(GateDecision::EarlyExit { layer });
        }

        None
    }

    fn allow_write(&self, witness: &WitnessLog) -> bool {
        // Skip writes if inference was skipped
        if !witness.gate_decision.did_run() {
            return false;
        }

        // If we require stable coherence, only allow writes after full run
        if self.config.require_stable_for_write {
            matches!(witness.gate_decision, GateDecision::RanFull)
        } else {
            true
        }
    }
}

/// Mincut-aware coherence gate
///
/// Uses mincut signals to make more informed gating decisions.
pub struct MincutCoherenceGate {
    base: DefaultCoherenceGate,
    /// Minimum lambda (mincut value) for inference
    pub min_lambda: i16,
    /// Lambda threshold for early exit
    pub lambda_exit_threshold: i16,
}

impl MincutCoherenceGate {
    /// Create a new mincut-aware gate
    pub fn new(config: CoherenceConfig, min_lambda: i16, lambda_exit_threshold: i16) -> Self {
        Self {
            base: DefaultCoherenceGate::with_config(config),
            min_lambda,
            lambda_exit_threshold,
        }
    }
}

impl CoherenceGate for MincutCoherenceGate {
    fn preflight(&self, hint: &GateHint) -> GateDecision {
        // Use base coherence check
        let base_decision = self.base.preflight(hint);
        if !base_decision.did_run() {
            return base_decision;
        }

        // Additional mincut check
        // If boundary was crossed and coherence is low, skip
        if hint.boundary_crossed && hint.coherence_score_q < 0 {
            return GateDecision::Skipped {
                reason: SkipReason::LowCoherence,
            };
        }

        GateDecision::RanFull
    }

    fn checkpoint(&self, layer: u8, signal_q: i16) -> Option<GateDecision> {
        // Use base checkpoint with mincut-adjusted threshold
        let adjusted_threshold = if signal_q > self.lambda_exit_threshold {
            // High lambda suggests stable state, lower exit threshold
            self.base.config.early_exit_threshold / 2
        } else {
            self.base.config.early_exit_threshold
        };

        if layer >= self.base.config.min_layers && signal_q >= adjusted_threshold {
            return Some(GateDecision::EarlyExit { layer });
        }

        None
    }

    fn allow_write(&self, witness: &WitnessLog) -> bool {
        // Use base write check
        self.base.allow_write(witness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_gate_preflight() {
        let gate = DefaultCoherenceGate::new();

        // High coherence should pass
        let hint = GateHint::new(256, false, ComputeClass::Deliberative);
        assert!(matches!(gate.preflight(&hint), GateDecision::RanFull));

        // Low coherence should fail
        let hint = GateHint::new(-512, false, ComputeClass::Deliberative);
        assert!(matches!(
            gate.preflight(&hint),
            GateDecision::Skipped { .. }
        ));
    }

    #[test]
    fn test_early_exit_checkpoint() {
        let gate = DefaultCoherenceGate::new();

        // Layer 0 - too early
        assert!(gate.checkpoint(0, 1000).is_none());

        // Layer 4 with high signal - should exit
        let decision = gate.checkpoint(4, 1000);
        assert!(matches!(
            decision,
            Some(GateDecision::EarlyExit { layer: 4 })
        ));
    }

    #[test]
    fn test_write_gating() {
        let gate = DefaultCoherenceGate::new();

        // Full run should allow writes
        let witness = crate::witness::WitnessLog::empty();
        assert!(gate.allow_write(&witness));

        // Skipped should not allow writes
        let mut skipped_witness = crate::witness::WitnessLog::empty();
        skipped_witness.gate_decision = GateDecision::Skipped {
            reason: SkipReason::LowCoherence,
        };
        assert!(!gate.allow_write(&skipped_witness));
    }

    #[test]
    fn test_strict_config() {
        let gate = DefaultCoherenceGate::with_config(CoherenceConfig::strict());

        // Strict should require positive coherence
        let hint = GateHint::new(-1, false, ComputeClass::Deliberative);
        assert!(matches!(
            gate.preflight(&hint),
            GateDecision::Skipped { .. }
        ));
    }

    #[test]
    fn test_permissive_config() {
        let gate = DefaultCoherenceGate::with_config(CoherenceConfig::permissive());

        // Permissive should allow anything
        let hint = GateHint::new(i16::MIN, true, ComputeClass::Reflex);
        assert!(matches!(gate.preflight(&hint), GateDecision::RanFull));
    }
}
