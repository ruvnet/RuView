//! Gate controller for coherence-based intervention.
//!
//! Implements coherence-gated control inspired by:
//! - **Mixture-of-Depths** (Raposo et al., 2024) - Dynamic compute tiers based on complexity
//! - **Energy-Based Transformers** (Gladstone et al., 2025) - Lambda as energy metric
//! - **Spectral Graph Theory** (Kreuzer et al., 2021) - Mincut signals for coherence
//!
//! The gate controller evaluates mincut signals and determines:
//! - Whether to intervene
//! - What type of intervention (reduce scope, flush KV, freeze writes, quarantine)
//! - What compute tier to use (0=normal, 1=reduced, 2=safe, 3=skip)
//! - What effective parameters to apply (layers, sequence length, attention window)
//!
//! Supports both rule-based and energy-based policies.
//!
//! ## References
//!
//! - Raposo, D., et al. (2024). Mixture-of-Depths. arXiv:2404.02258.
//! - Gladstone, A., et al. (2025). Energy-Based Transformers. arXiv:2507.02092.
//! - Kreuzer, D., et al. (2021). Spectral Attention. NeurIPS 2021.

use crate::config::GatePolicy;
use crate::packets::{GateDecision, GatePacket, GateReason, SpikePacket};

#[cfg(feature = "energy_gate")]
use crate::energy_gate::{EnergyGate, EnergyGateConfig};

/// Tier decision from gate evaluation.
#[derive(Clone, Copy, Debug)]
pub struct TierDecision {
    /// Gate decision
    pub decision: GateDecision,

    /// Reason for the decision
    pub reason: GateReason,

    /// Compute tier (0 = normal, 1 = reduced, 2 = safe, 3 = skip)
    pub tier: u8,

    /// Number of layers to run
    pub layers_to_run: u16,

    /// Effective sequence length
    pub effective_seq_len: u16,

    /// Effective attention window
    pub effective_window: u16,

    /// Whether to skip inference entirely
    pub skip: bool,
}

impl Default for TierDecision {
    fn default() -> Self {
        Self {
            decision: GateDecision::Allow,
            reason: GateReason::None,
            tier: 0,
            layers_to_run: 4,
            effective_seq_len: 64,
            effective_window: 16,
            skip: false,
        }
    }
}

/// Gate controller for evaluating coherence and selecting compute tiers.
pub struct GateController {
    /// Gate policy
    policy: GatePolicy,

    /// Optional energy-based gate
    #[cfg(feature = "energy_gate")]
    energy_gate: Option<EnergyGate>,

    /// Default layers for tier 0
    layers_normal: u16,

    /// Layers for degraded tier
    layers_degraded: u16,

    /// Default sequence length
    seq_len_normal: u16,

    /// Degraded sequence length
    seq_len_degraded: u16,

    /// Safe sequence length
    seq_len_safe: u16,

    /// Normal window
    window_normal: u16,

    /// Degraded window
    window_degraded: u16,
}

impl GateController {
    /// Create a new gate controller with the given policy.
    pub fn new(policy: GatePolicy) -> Self {
        // Use baseline config defaults - these get overridden by actual config
        Self {
            policy,
            #[cfg(feature = "energy_gate")]
            energy_gate: None,
            layers_normal: 4,
            layers_degraded: 2,
            seq_len_normal: 64,
            seq_len_degraded: 32,
            seq_len_safe: 8,
            window_normal: 16,
            window_degraded: 8,
        }
    }

    /// Create with energy-based gate policy (requires `energy_gate` feature).
    #[cfg(feature = "energy_gate")]
    pub fn with_energy_gate(policy: GatePolicy, energy_config: EnergyGateConfig) -> Self {
        let energy_gate = EnergyGate::new(energy_config, policy.clone());
        Self {
            policy,
            energy_gate: Some(energy_gate),
            layers_normal: 4,
            layers_degraded: 2,
            seq_len_normal: 64,
            seq_len_degraded: 32,
            seq_len_safe: 8,
            window_normal: 16,
            window_degraded: 8,
        }
    }

    /// Create with explicit configuration parameters
    pub fn with_config(
        policy: GatePolicy,
        layers_normal: u16,
        layers_degraded: u16,
        seq_len_normal: u16,
        seq_len_degraded: u16,
        seq_len_safe: u16,
        window_normal: u16,
        window_degraded: u16,
    ) -> Self {
        Self {
            policy,
            #[cfg(feature = "energy_gate")]
            energy_gate: None,
            layers_normal,
            layers_degraded,
            seq_len_normal,
            seq_len_degraded,
            seq_len_safe,
            window_normal,
            window_degraded,
        }
    }

    /// Create with explicit configuration and energy gate (requires `energy_gate` feature).
    #[cfg(feature = "energy_gate")]
    pub fn with_config_and_energy(
        policy: GatePolicy,
        energy_config: EnergyGateConfig,
        layers_normal: u16,
        layers_degraded: u16,
        seq_len_normal: u16,
        seq_len_degraded: u16,
        seq_len_safe: u16,
        window_normal: u16,
        window_degraded: u16,
    ) -> Self {
        let energy_gate = EnergyGate::new(energy_config, policy.clone());
        Self {
            policy,
            energy_gate: Some(energy_gate),
            layers_normal,
            layers_degraded,
            seq_len_normal,
            seq_len_degraded,
            seq_len_safe,
            window_normal,
            window_degraded,
        }
    }

    /// Evaluate gate conditions and return tier decision.
    ///
    /// Gate checks occur at multiple points:
    /// 1. Pre-infer: decide tier, effective seq_len, effective window
    /// 2. Pre-attention: may further reduce window
    /// 3. Pre-KV write: may disable KV writes, flush KV, or quarantine
    /// 4. Post-layer: may early exit remaining layers
    /// 5. Pre-external write: may freeze external writes
    ///
    /// If energy gate is enabled, it will be used first with fallback to rule-based policy.
    pub fn evaluate(&self, gate: &GatePacket, spikes: Option<&SpikePacket>) -> TierDecision {
        // Try energy-based evaluation first if enabled
        #[cfg(feature = "energy_gate")]
        if let Some(ref energy_gate) = self.energy_gate {
            let (decision, confidence) = energy_gate.decide(gate);

            // If high confidence, use energy gate decision
            if confidence >= 0.7 {
                return self.tier_from_decision(decision, GateReason::None);
            }
            // Otherwise fall through to rule-based policy
        }

        // Rule-based evaluation (original logic)
        // Check for forced flags first
        if gate.skip_requested() {
            return TierDecision {
                decision: GateDecision::Allow,
                reason: GateReason::ForcedByFlag,
                tier: 3,
                skip: true,
                layers_to_run: 0,
                effective_seq_len: 0,
                effective_window: 0,
            };
        }

        if gate.force_safe() {
            return TierDecision {
                decision: GateDecision::FreezeWrites,
                reason: GateReason::ForcedByFlag,
                tier: 2,
                skip: false,
                layers_to_run: 1,
                effective_seq_len: self.seq_len_safe,
                effective_window: 4,
            };
        }

        // Check spike conditions (if spikes provided)
        if let Some(sp) = spikes {
            if !sp.is_active() {
                // Spike not fired - consider skip or cheap path
                return TierDecision {
                    decision: GateDecision::Allow,
                    reason: GateReason::None,
                    tier: 3,
                    skip: true,
                    layers_to_run: 0,
                    effective_seq_len: 0,
                    effective_window: 0,
                };
            }

            // Check spike storm condition
            if sp.rate_q15 > self.policy.spike_rate_q15_max {
                return self.tier_safe(GateReason::SpikeStorm);
            }
        }

        // Check lambda conditions
        if gate.lambda < self.policy.lambda_min {
            return self.tier_with_intervention(
                GateDecision::QuarantineUpdates,
                GateReason::LambdaBelowMin,
            );
        }

        // Check lambda drop
        let drop_ratio = gate.drop_ratio_q15();
        if drop_ratio > self.policy.drop_ratio_q15_max {
            return self
                .tier_with_intervention(GateDecision::FlushKv, GateReason::LambdaDroppedFast);
        }

        // Check boundary conditions
        if gate.boundary_edges > self.policy.boundary_edges_max {
            return self.tier_reduced(GateReason::BoundarySpike);
        }

        if gate.boundary_concentration_q15 > self.policy.boundary_concentration_q15_max {
            return self.tier_reduced(GateReason::BoundaryConcentrationSpike);
        }

        // Check partition drift
        if gate.partition_count > self.policy.partitions_max {
            return self.tier_reduced(GateReason::PartitionDrift);
        }

        // All checks passed - allow normal operation
        TierDecision {
            decision: GateDecision::Allow,
            reason: GateReason::None,
            tier: 0,
            skip: false,
            layers_to_run: self.layers_normal,
            effective_seq_len: self.seq_len_normal,
            effective_window: self.window_normal,
        }
    }

    /// Check if KV writes should be allowed based on current conditions
    pub fn should_allow_kv_writes(&self, gate: &GatePacket) -> bool {
        if gate.lambda < self.policy.lambda_min {
            return self.policy.allow_kv_write_when_unstable;
        }

        let drop_ratio = gate.drop_ratio_q15();
        if drop_ratio > self.policy.drop_ratio_q15_max {
            return self.policy.allow_kv_write_when_unstable;
        }

        true
    }

    /// Check if external writes should be allowed
    pub fn should_allow_external_writes(&self, gate: &GatePacket) -> bool {
        if gate.lambda < self.policy.lambda_min {
            return self.policy.allow_external_write_when_unstable;
        }

        let drop_ratio = gate.drop_ratio_q15();
        if drop_ratio > self.policy.drop_ratio_q15_max {
            return self.policy.allow_external_write_when_unstable;
        }

        true
    }

    // ---- Private helpers ----

    #[cfg(feature = "energy_gate")]
    fn tier_from_decision(&self, decision: GateDecision, reason: GateReason) -> TierDecision {
        match decision {
            GateDecision::Allow => TierDecision {
                decision,
                reason,
                tier: 0,
                skip: false,
                layers_to_run: self.layers_normal,
                effective_seq_len: self.seq_len_normal,
                effective_window: self.window_normal,
            },
            GateDecision::ReduceScope => self.tier_reduced(reason),
            GateDecision::FlushKv => self.tier_with_intervention(decision, reason),
            GateDecision::FreezeWrites => self.tier_safe(reason),
            GateDecision::QuarantineUpdates => self.tier_with_intervention(decision, reason),
        }
    }

    fn tier_reduced(&self, reason: GateReason) -> TierDecision {
        TierDecision {
            decision: GateDecision::ReduceScope,
            reason,
            tier: 1,
            skip: false,
            layers_to_run: self.layers_degraded,
            effective_seq_len: self.seq_len_degraded,
            effective_window: self.window_degraded,
        }
    }

    fn tier_safe(&self, reason: GateReason) -> TierDecision {
        TierDecision {
            decision: GateDecision::FreezeWrites,
            reason,
            tier: 2,
            skip: false,
            layers_to_run: 1,
            effective_seq_len: self.seq_len_safe,
            effective_window: 4,
        }
    }

    fn tier_with_intervention(&self, decision: GateDecision, reason: GateReason) -> TierDecision {
        let (tier, layers, seq_len, window) = match decision {
            GateDecision::ReduceScope => (
                1,
                self.layers_degraded,
                self.seq_len_degraded,
                self.window_degraded,
            ),
            GateDecision::FlushKv => (
                1,
                self.layers_degraded,
                self.seq_len_degraded,
                self.window_degraded,
            ),
            GateDecision::FreezeWrites => (2, 1, self.seq_len_safe, 4),
            GateDecision::QuarantineUpdates => (2, 1, self.seq_len_safe, 4),
            GateDecision::Allow => (
                0,
                self.layers_normal,
                self.seq_len_normal,
                self.window_normal,
            ),
        };

        TierDecision {
            decision,
            reason,
            tier,
            skip: false,
            layers_to_run: layers,
            effective_seq_len: seq_len,
            effective_window: window,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_allow() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 5,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let decision = gate_ctrl.evaluate(&gate, None);
        assert_eq!(decision.decision, GateDecision::Allow);
        assert_eq!(decision.tier, 0);
        assert!(!decision.skip);
    }

    #[test]
    fn test_gate_lambda_below_min() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 10, // Below default min of 30
            lambda_prev: 100,
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, None);
        assert_eq!(decision.decision, GateDecision::QuarantineUpdates);
        assert_eq!(decision.reason, GateReason::LambdaBelowMin);
    }

    #[test]
    fn test_gate_lambda_drop() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 40,
            lambda_prev: 100, // 60% drop
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, None);
        assert_eq!(decision.decision, GateDecision::FlushKv);
        assert_eq!(decision.reason, GateReason::LambdaDroppedFast);
    }

    #[test]
    fn test_gate_boundary_spike() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 50, // Above default max of 20
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, None);
        assert_eq!(decision.decision, GateDecision::ReduceScope);
        assert_eq!(decision.reason, GateReason::BoundarySpike);
        assert_eq!(decision.tier, 1);
    }

    #[test]
    fn test_gate_force_safe() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 100,
            flags: GatePacket::FLAG_FORCE_SAFE,
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, None);
        assert_eq!(decision.decision, GateDecision::FreezeWrites);
        assert_eq!(decision.reason, GateReason::ForcedByFlag);
        assert_eq!(decision.tier, 2);
    }

    #[test]
    fn test_gate_skip() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            flags: GatePacket::FLAG_SKIP,
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, None);
        assert!(decision.skip);
        assert_eq!(decision.tier, 3);
    }

    #[test]
    fn test_gate_spike_inactive() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 100,
            ..Default::default()
        };

        let spike = SpikePacket {
            fired: 0, // Not fired
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, Some(&spike));
        assert!(decision.skip);
        assert_eq!(decision.tier, 3);
    }

    #[test]
    fn test_gate_spike_storm() {
        let policy = GatePolicy::default();
        let gate_ctrl = GateController::new(policy);

        let gate = GatePacket {
            lambda: 100,
            ..Default::default()
        };

        let spike = SpikePacket {
            fired: 1,
            rate_q15: 30000, // Very high rate
            ..Default::default()
        };

        let decision = gate_ctrl.evaluate(&gate, Some(&spike));
        assert_eq!(decision.decision, GateDecision::FreezeWrites);
        assert_eq!(decision.reason, GateReason::SpikeStorm);
        assert_eq!(decision.tier, 2);
    }
}
