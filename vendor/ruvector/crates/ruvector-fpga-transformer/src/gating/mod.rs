//! Gating subsystem for coherence-based inference control
//!
//! Provides preflight and postflight gates that integrate mincut signals
//! and write policies for memory safety.

pub mod coherence_gate;
pub mod policy_gate;

pub use coherence_gate::{CoherenceConfig, CoherenceGate, DefaultCoherenceGate};
pub use policy_gate::{DefaultPolicyGate, PolicyGate, WritePolicy};

use crate::types::{GateDecision, GateHint, SkipReason};
use crate::witness::WitnessLog;

/// Combined gate that checks both coherence and policy
pub struct CombinedGate {
    coherence: Box<dyn CoherenceGate>,
    policy: Box<dyn PolicyGate>,
}

impl CombinedGate {
    /// Create a new combined gate
    pub fn new(coherence: Box<dyn CoherenceGate>, policy: Box<dyn PolicyGate>) -> Self {
        Self { coherence, policy }
    }

    /// Create with default implementations
    pub fn default_gates() -> Self {
        Self::new(
            Box::new(DefaultCoherenceGate::new()),
            Box::new(DefaultPolicyGate::new()),
        )
    }

    /// Preflight check before inference
    pub fn preflight(&self, hint: &GateHint) -> GateDecision {
        // First check policy
        if !self.policy.allow_inference(hint) {
            return GateDecision::Skipped {
                reason: SkipReason::PolicyDenied,
            };
        }

        // Then check coherence
        self.coherence.preflight(hint)
    }

    /// Checkpoint during inference
    pub fn checkpoint(&self, layer: u8, signal_q: i16) -> Option<GateDecision> {
        self.coherence.checkpoint(layer, signal_q)
    }

    /// Check if write is allowed after inference
    pub fn allow_write(&self, witness: &WitnessLog) -> bool {
        self.coherence.allow_write(witness) && self.policy.allow_write(witness)
    }
}

impl CoherenceGate for CombinedGate {
    fn preflight(&self, hint: &GateHint) -> GateDecision {
        CombinedGate::preflight(self, hint)
    }

    fn checkpoint(&self, layer: u8, signal_q: i16) -> Option<GateDecision> {
        CombinedGate::checkpoint(self, layer, signal_q)
    }

    fn allow_write(&self, witness: &WitnessLog) -> bool {
        CombinedGate::allow_write(self, witness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combined_gate_preflight() {
        let gate = CombinedGate::default_gates();

        // Allow all hint should pass
        let decision = gate.preflight(&GateHint::allow_all());
        assert!(matches!(decision, GateDecision::RanFull));
    }

    #[test]
    fn test_combined_gate_low_coherence() {
        let gate = CombinedGate::default_gates();

        // Very low coherence should skip
        let hint = GateHint::new(-1000, false, crate::types::ComputeClass::Reflex);
        let decision = gate.preflight(&hint);
        assert!(matches!(decision, GateDecision::Skipped { .. }));
    }
}
