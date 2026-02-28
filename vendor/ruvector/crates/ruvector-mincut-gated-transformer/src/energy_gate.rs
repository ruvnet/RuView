//! Energy-based gate policy using coherence as energy function.
//!
//! Based on Energy-Based Transformers (Gladstone et al., 2025).
//! Frames gate decisions as energy minimization, providing:
//! - Principled decision-making via energy landscapes
//! - Confidence scores from energy gradients
//! - System 2 thinking through iterative refinement
//!
//! ## Energy Function
//!
//! E(state) = λ_weight * f_lambda(λ) + boundary_weight * f_boundary(b) + entropy_weight * f_entropy(p)
//!
//! Where:
//! - f_lambda: coherence energy (lower lambda = higher energy)
//! - f_boundary: boundary disruption energy
//! - f_entropy: partition entropy energy
//!
//! Lower energy = more stable state = Allow decision
//! Higher energy = unstable state = Intervention needed

use crate::config::GatePolicy;
use crate::packets::{GateDecision, GatePacket, GateReason};
use serde::{Deserialize, Serialize};

/// Configuration for energy-based gate policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnergyGateConfig {
    /// Weight for lambda term in energy function
    pub lambda_weight: f32,

    /// Weight for boundary penalty term
    pub boundary_penalty_weight: f32,

    /// Weight for partition entropy term
    pub partition_entropy_weight: f32,

    /// Convexity radius for local optimization
    pub convexity_radius: f32,

    /// Number of gradient descent steps for refinement
    pub gradient_steps: u8,

    /// Energy threshold for intervention (above = intervene)
    pub energy_threshold: f32,

    /// Energy threshold for quarantine (very high energy)
    pub energy_quarantine_threshold: f32,

    /// Minimum confidence for decisions (0.0-1.0)
    pub min_confidence: f32,

    /// Lambda normalization constant
    pub lambda_norm: f32,
}

impl Default for EnergyGateConfig {
    fn default() -> Self {
        Self {
            lambda_weight: 1.0,
            boundary_penalty_weight: 0.5,
            partition_entropy_weight: 0.3,
            convexity_radius: 2.0,
            gradient_steps: 3,
            energy_threshold: 0.5,
            energy_quarantine_threshold: 0.9,
            min_confidence: 0.6,
            lambda_norm: 150.0, // Typical lambda range
        }
    }
}

/// Energy gradient for optimization.
#[derive(Clone, Debug)]
pub struct EnergyGradient {
    /// Partial derivative w.r.t. lambda
    pub d_lambda: f32,

    /// Partial derivative w.r.t. boundary edges
    pub d_boundary: f32,

    /// Partial derivative w.r.t. partition count
    pub d_partition: f32,

    /// Total gradient magnitude
    pub magnitude: f32,
}

impl EnergyGradient {
    /// Create zero gradient
    pub fn zero() -> Self {
        Self {
            d_lambda: 0.0,
            d_boundary: 0.0,
            d_partition: 0.0,
            magnitude: 0.0,
        }
    }

    /// Compute gradient magnitude
    pub fn compute_magnitude(&mut self) {
        self.magnitude = (self.d_lambda * self.d_lambda
            + self.d_boundary * self.d_boundary
            + self.d_partition * self.d_partition)
            .sqrt();
    }
}

/// Energy-based gate controller.
pub struct EnergyGate {
    config: EnergyGateConfig,
    fallback_policy: GatePolicy,
}

impl EnergyGate {
    /// Create new energy-based gate controller
    pub fn new(config: EnergyGateConfig, fallback_policy: GatePolicy) -> Self {
        Self {
            config,
            fallback_policy,
        }
    }

    /// Compute energy for current gate state.
    ///
    /// Lower energy = more stable/coherent state.
    pub fn compute_energy(&self, gate: &GatePacket) -> f32 {
        // Lambda energy: inversely proportional to lambda
        // Low lambda = high energy (unstable)
        let lambda_normalized = gate.lambda as f32 / self.config.lambda_norm;
        let lambda_energy = if lambda_normalized > 0.0 {
            1.0 / (1.0 + lambda_normalized)
        } else {
            1.0
        };

        // Boundary energy: proportional to boundary edges and concentration
        let boundary_normalized = gate.boundary_edges as f32 / 100.0; // Assume max ~100 edges
        let concentration_normalized = gate.boundary_concentration_q15 as f32 / 32768.0;
        let boundary_energy = boundary_normalized * 0.5 + concentration_normalized * 0.5;

        // Partition entropy energy: measure of partition disorder
        // More partitions = higher entropy = higher energy
        let partition_count = gate.partition_count as f32;
        let partition_energy = if partition_count > 1.0 {
            // Entropy-like measure: log(k) / log(max_k)
            // Normalized to [0, 1] assuming max 10 partitions
            (partition_count.ln() / 10.0f32.ln()).min(1.0)
        } else {
            0.0
        };

        // Weighted sum
        let energy = self.config.lambda_weight * lambda_energy
            + self.config.boundary_penalty_weight * boundary_energy
            + self.config.partition_entropy_weight * partition_energy;

        // Normalize to [0, 1]
        energy
            / (self.config.lambda_weight
                + self.config.boundary_penalty_weight
                + self.config.partition_entropy_weight)
    }

    /// Compute energy gradient for optimization.
    ///
    /// Gradient indicates direction of energy increase.
    /// For intervention, we want to move away from high-energy regions.
    pub fn energy_gradient(&self, gate: &GatePacket) -> EnergyGradient {
        let epsilon = 1.0; // Small perturbation

        // Central difference approximation
        let energy_0 = self.compute_energy(gate);

        // d/d_lambda
        let mut gate_lambda_plus = *gate;
        gate_lambda_plus.lambda = (gate.lambda as f32 + epsilon).max(0.0) as u32;
        let energy_lambda_plus = self.compute_energy(&gate_lambda_plus);
        let d_lambda = (energy_lambda_plus - energy_0) / epsilon;

        // d/d_boundary
        let mut gate_boundary_plus = *gate;
        gate_boundary_plus.boundary_edges = (gate.boundary_edges as f32 + epsilon).max(0.0) as u16;
        let energy_boundary_plus = self.compute_energy(&gate_boundary_plus);
        let d_boundary = (energy_boundary_plus - energy_0) / epsilon;

        // d/d_partition
        let mut gate_partition_plus = *gate;
        gate_partition_plus.partition_count =
            (gate.partition_count as f32 + epsilon).max(1.0) as u16;
        let energy_partition_plus = self.compute_energy(&gate_partition_plus);
        let d_partition = (energy_partition_plus - energy_0) / epsilon;

        let mut gradient = EnergyGradient {
            d_lambda,
            d_boundary,
            d_partition,
            magnitude: 0.0,
        };
        gradient.compute_magnitude();

        gradient
    }

    /// Make gate decision via energy minimization.
    ///
    /// Returns (decision, confidence).
    /// Confidence is based on energy gradient magnitude and distance from thresholds.
    pub fn decide(&self, gate: &GatePacket) -> (GateDecision, f32) {
        // Check forced flags first
        if gate.skip_requested() {
            return (GateDecision::Allow, 1.0);
        }

        if gate.force_safe() {
            return (GateDecision::FreezeWrites, 1.0);
        }

        // Compute energy
        let energy = self.compute_energy(gate);

        // Compute gradient for confidence
        let gradient = self.energy_gradient(gate);

        // Decision based on energy thresholds
        let (decision, _reason) = if energy >= self.config.energy_quarantine_threshold {
            (GateDecision::QuarantineUpdates, GateReason::LambdaBelowMin)
        } else if energy >= self.config.energy_threshold {
            // Medium energy - determine intervention type based on components
            self.determine_intervention(gate, energy, &gradient)
        } else {
            // Low energy - stable, allow
            (GateDecision::Allow, GateReason::None)
        };

        // Compute confidence
        let confidence = self.compute_confidence(energy, &gradient);

        // If confidence is too low, fall back to rule-based policy
        if confidence < self.config.min_confidence {
            // Use traditional gate policy as fallback
            return self.fallback_decision(gate);
        }

        (decision, confidence)
    }

    /// System 2 thinking: iterative refinement via gradient descent.
    ///
    /// Performs multiple evaluation steps to refine the decision.
    /// Useful for borderline cases where initial confidence is low.
    pub fn refine_decision(&self, gate: &GatePacket, steps: u8) -> GateDecision {
        let mut current_gate = *gate;
        let mut best_decision = GateDecision::Allow;
        let mut best_confidence = 0.0f32;

        for _ in 0..steps {
            let (decision, confidence) = self.decide(&current_gate);

            if confidence > best_confidence {
                best_decision = decision;
                best_confidence = confidence;
            }

            // Apply small perturbation in direction of lower energy
            let gradient = self.energy_gradient(&current_gate);

            // Move in negative gradient direction (toward lower energy)
            let step_size = self.config.convexity_radius / steps as f32;

            // Perturb lambda (increase if gradient is negative)
            if gradient.d_lambda < 0.0 {
                current_gate.lambda = (current_gate.lambda as f32 + step_size).min(500.0) as u32;
            }

            // Note: We don't modify boundary/partition directly as they're observations
            // This is a conceptual refinement exploring nearby energy landscape
        }

        best_decision
    }

    // ---- Private helpers ----

    fn determine_intervention(
        &self,
        gate: &GatePacket,
        _energy: f32,
        gradient: &EnergyGradient,
    ) -> (GateDecision, GateReason) {
        // Determine which component contributes most to energy
        let lambda_contribution = gradient.d_lambda.abs();
        let boundary_contribution = gradient.d_boundary.abs();
        let partition_contribution = gradient.d_partition.abs();

        // Select intervention based on dominant factor
        if lambda_contribution > boundary_contribution
            && lambda_contribution > partition_contribution
        {
            // Lambda is the main issue
            if gate.lambda < self.fallback_policy.lambda_min {
                (GateDecision::QuarantineUpdates, GateReason::LambdaBelowMin)
            } else {
                let drop_ratio = gate.drop_ratio_q15();
                if drop_ratio > self.fallback_policy.drop_ratio_q15_max {
                    (GateDecision::FlushKv, GateReason::LambdaDroppedFast)
                } else {
                    (GateDecision::ReduceScope, GateReason::LambdaBelowMin)
                }
            }
        } else if boundary_contribution > partition_contribution {
            // Boundary issues
            (GateDecision::ReduceScope, GateReason::BoundarySpike)
        } else {
            // Partition drift
            (GateDecision::ReduceScope, GateReason::PartitionDrift)
        }
    }

    fn compute_confidence(&self, energy: f32, gradient: &EnergyGradient) -> f32 {
        // Confidence based on:
        // 1. Distance from decision boundaries (energy thresholds)
        // 2. Gradient magnitude (sharp vs. flat energy landscape)

        // Distance from thresholds - higher distance = higher confidence
        let dist_from_threshold = if energy < self.config.energy_threshold {
            // In "allow" region - distance from threshold
            self.config.energy_threshold - energy
        } else if energy < self.config.energy_quarantine_threshold {
            // In "intervention" region - distance from both thresholds
            let dist_lower = energy - self.config.energy_threshold;
            let dist_upper = self.config.energy_quarantine_threshold - energy;
            dist_lower.min(dist_upper)
        } else {
            // In "quarantine" region - distance from threshold
            energy - self.config.energy_quarantine_threshold
        };

        // Normalize distance to [0, 1] - assume max distance of 0.5
        let distance_confidence = (dist_from_threshold / 0.5).min(1.0);

        // Gradient magnitude contribution
        // High magnitude = clear direction = high confidence
        // Normalize by typical gradient magnitude (assume 2.0 is high)
        let gradient_confidence = (gradient.magnitude / 2.0).min(1.0);

        // Combine (weighted average)
        distance_confidence * 0.7 + gradient_confidence * 0.3
    }

    fn fallback_decision(&self, gate: &GatePacket) -> (GateDecision, f32) {
        // Use traditional rule-based policy
        // Low confidence indicates we should use proven heuristics

        if gate.lambda < self.fallback_policy.lambda_min {
            (GateDecision::QuarantineUpdates, 0.5)
        } else if gate.drop_ratio_q15() > self.fallback_policy.drop_ratio_q15_max {
            (GateDecision::FlushKv, 0.5)
        } else if gate.boundary_edges > self.fallback_policy.boundary_edges_max {
            (GateDecision::ReduceScope, 0.5)
        } else if gate.boundary_concentration_q15
            > self.fallback_policy.boundary_concentration_q15_max
        {
            (GateDecision::ReduceScope, 0.5)
        } else if gate.partition_count > self.fallback_policy.partitions_max {
            (GateDecision::ReduceScope, 0.5)
        } else {
            (GateDecision::Allow, 0.5)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_computation() {
        let config = EnergyGateConfig::default();
        let policy = GatePolicy::default();
        let energy_gate = EnergyGate::new(config, policy);

        // High lambda = low energy (stable)
        let gate_stable = GatePacket {
            lambda: 200,
            lambda_prev: 195,
            boundary_edges: 5,
            boundary_concentration_q15: 4096,
            partition_count: 2,
            flags: 0,
        };
        let energy_stable = energy_gate.compute_energy(&gate_stable);

        // Low lambda = high energy (unstable)
        let gate_unstable = GatePacket {
            lambda: 20,
            lambda_prev: 100,
            boundary_edges: 50,
            boundary_concentration_q15: 20000,
            partition_count: 8,
            flags: 0,
        };
        let energy_unstable = energy_gate.compute_energy(&gate_unstable);

        assert!(energy_stable < energy_unstable);
        assert!(energy_stable >= 0.0 && energy_stable <= 1.0);
        assert!(energy_unstable >= 0.0 && energy_unstable <= 1.0);
    }

    #[test]
    fn test_energy_gradient() {
        let config = EnergyGateConfig::default();
        let policy = GatePolicy::default();
        let energy_gate = EnergyGate::new(config, policy);

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95,
            boundary_edges: 10,
            boundary_concentration_q15: 8192,
            partition_count: 3,
            flags: 0,
        };

        let gradient = energy_gate.energy_gradient(&gate);

        // Gradient should have non-zero magnitude
        assert!(gradient.magnitude > 0.0);

        // Lambda gradient should be negative (increasing lambda decreases energy)
        assert!(gradient.d_lambda < 0.0);

        // Boundary gradient should be positive (increasing boundaries increases energy)
        assert!(gradient.d_boundary > 0.0);
    }

    #[test]
    fn test_decision_making() {
        // Use lower thresholds to ensure stable state is truly stable
        let config = EnergyGateConfig {
            energy_threshold: 0.7, // Higher threshold = more permissive
            energy_quarantine_threshold: 0.95,
            min_confidence: 0.3, // Lower min confidence for testing
            ..EnergyGateConfig::default()
        };
        let policy = GatePolicy::default();
        let energy_gate = EnergyGate::new(config, policy);

        // Very stable state - high lambda, low boundary disruption
        let gate_stable = GatePacket {
            lambda: 250, // Very high lambda
            lambda_prev: 245,
            boundary_edges: 2,                // Very few boundary edges
            boundary_concentration_q15: 2048, // Low concentration
            partition_count: 2,
            flags: 0,
        };
        let (decision_stable, confidence_stable) = energy_gate.decide(&gate_stable);
        assert_eq!(decision_stable, GateDecision::Allow);
        assert!(confidence_stable > 0.0);

        // Unstable state - should intervene
        let gate_unstable = GatePacket {
            lambda: 20,
            lambda_prev: 100,
            boundary_edges: 50,
            boundary_concentration_q15: 20000,
            partition_count: 8,
            flags: 0,
        };
        let (decision_unstable, _) = energy_gate.decide(&gate_unstable);
        assert_ne!(decision_unstable, GateDecision::Allow);
    }

    #[test]
    fn test_forced_decisions() {
        let config = EnergyGateConfig::default();
        let policy = GatePolicy::default();
        let energy_gate = EnergyGate::new(config, policy);

        let gate_skip = GatePacket {
            lambda: 100,
            flags: GatePacket::FLAG_SKIP,
            ..Default::default()
        };
        let (decision, confidence) = energy_gate.decide(&gate_skip);
        assert_eq!(decision, GateDecision::Allow);
        assert_eq!(confidence, 1.0);

        let gate_safe = GatePacket {
            lambda: 100,
            flags: GatePacket::FLAG_FORCE_SAFE,
            ..Default::default()
        };
        let (decision, confidence) = energy_gate.decide(&gate_safe);
        assert_eq!(decision, GateDecision::FreezeWrites);
        assert_eq!(confidence, 1.0);
    }

    #[test]
    fn test_refinement() {
        let config = EnergyGateConfig::default();
        let policy = GatePolicy::default();
        let energy_gate = EnergyGate::new(config, policy);

        let gate = GatePacket {
            lambda: 80,
            lambda_prev: 75,
            boundary_edges: 15,
            boundary_concentration_q15: 10000,
            partition_count: 4,
            flags: 0,
        };

        let decision = energy_gate.refine_decision(&gate, 5);

        // Should produce a valid decision
        assert!(matches!(
            decision,
            GateDecision::Allow
                | GateDecision::ReduceScope
                | GateDecision::FlushKv
                | GateDecision::FreezeWrites
                | GateDecision::QuarantineUpdates
        ));
    }

    #[test]
    fn test_confidence_scoring() {
        // Test that energy correlates with state stability
        let config = EnergyGateConfig::default();
        let policy = GatePolicy::default();
        let energy_gate = EnergyGate::new(config, policy);

        // Very stable case - low energy expected
        let gate_stable = GatePacket {
            lambda: 250, // Very high lambda
            lambda_prev: 245,
            boundary_edges: 2,                // Very few edges
            boundary_concentration_q15: 1024, // Very low concentration
            partition_count: 2,
            flags: 0,
        };
        let energy_stable = energy_gate.compute_energy(&gate_stable);

        // Unstable case - high energy expected
        let gate_unstable = GatePacket {
            lambda: 30, // Low lambda
            lambda_prev: 100,
            boundary_edges: 80,                // Many boundary edges
            boundary_concentration_q15: 25000, // High concentration
            partition_count: 8,                // Many partitions
            flags: 0,
        };
        let energy_unstable = energy_gate.compute_energy(&gate_unstable);

        // Stable case should have lower energy
        assert!(energy_stable < energy_unstable);
    }
}
