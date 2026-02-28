//! Neural decision types.

use serde::{Deserialize, Serialize};

/// Trigger that caused the decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionTrigger {
    /// Energy crossed a threshold.
    EnergyThreshold {
        /// The threshold that was crossed.
        threshold: f32,
        /// Direction of crossing (true = upward).
        upward: bool,
    },
    /// Dendritic coincidence detection fired.
    DendriticCoincidence {
        /// Number of active synapses.
        active_synapses: usize,
        /// Required threshold.
        threshold: usize,
    },
    /// Hysteresis state change.
    HysteresisChange {
        /// Previous state.
        from_state: HysteresisState,
        /// New state.
        to_state: HysteresisState,
    },
    /// Oscillator synchronization detected.
    OscillatorSync {
        /// Phase coherence measure.
        coherence: f32,
    },
    /// Workspace broadcast triggered.
    WorkspaceBroadcast {
        /// Significance score.
        significance: f32,
    },
    /// Manual evaluation request.
    Manual,
}

/// Hysteresis state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HysteresisState {
    /// Low energy state (coherent).
    Low,
    /// Transition state (uncertain).
    Transition,
    /// High energy state (incoherent).
    High,
}

/// Confidence level of a decision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecisionConfidence {
    /// Overall confidence (0.0 to 1.0).
    pub overall: f32,
    /// Confidence from energy analysis.
    pub energy_confidence: f32,
    /// Confidence from dendritic processing.
    pub dendritic_confidence: f32,
    /// Confidence from oscillatory routing.
    pub oscillator_confidence: f32,
    /// Number of evidence sources supporting the decision.
    pub supporting_evidence: usize,
}

impl DecisionConfidence {
    /// Create a new decision confidence.
    pub fn new(
        energy_confidence: f32,
        dendritic_confidence: f32,
        oscillator_confidence: f32,
        supporting_evidence: usize,
    ) -> Self {
        // Combine confidences with weighted average
        let overall =
            (energy_confidence * 0.4 + dendritic_confidence * 0.3 + oscillator_confidence * 0.3)
                .clamp(0.0, 1.0);

        Self {
            overall,
            energy_confidence,
            dendritic_confidence,
            oscillator_confidence,
            supporting_evidence,
        }
    }

    /// Create a low-confidence decision.
    pub fn low() -> Self {
        Self {
            overall: 0.3,
            energy_confidence: 0.3,
            dendritic_confidence: 0.3,
            oscillator_confidence: 0.3,
            supporting_evidence: 0,
        }
    }

    /// Create a high-confidence decision.
    pub fn high() -> Self {
        Self {
            overall: 0.95,
            energy_confidence: 0.95,
            dendritic_confidence: 0.95,
            oscillator_confidence: 0.95,
            supporting_evidence: 5,
        }
    }

    /// Check if the confidence is high enough to trust.
    pub fn is_trustworthy(&self) -> bool {
        self.overall >= 0.7 && self.supporting_evidence >= 2
    }
}

impl Default for DecisionConfidence {
    fn default() -> Self {
        Self::low()
    }
}

/// Decision from the neural coherence gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralDecision {
    /// Whether to allow the action.
    pub allow: bool,
    /// Confidence in the decision.
    pub confidence: DecisionConfidence,
    /// Current hysteresis state.
    pub hysteresis_state: HysteresisState,
    /// What triggered this decision.
    pub trigger: DecisionTrigger,
    /// Current energy level.
    pub energy: f32,
    /// Smoothed energy level.
    pub smoothed_energy: f32,
    /// Timestamp of the decision.
    pub timestamp_ms: u64,
    /// Whether this decision should be broadcast.
    pub should_broadcast: bool,
}

impl NeuralDecision {
    /// Create a new neural decision.
    pub fn new(
        allow: bool,
        energy: f32,
        smoothed_energy: f32,
        hysteresis_state: HysteresisState,
        trigger: DecisionTrigger,
        confidence: DecisionConfidence,
    ) -> Self {
        let should_broadcast = !allow || confidence.overall > 0.9;

        Self {
            allow,
            confidence,
            hysteresis_state,
            trigger,
            energy,
            smoothed_energy,
            timestamp_ms: current_time_ms(),
            should_broadcast,
        }
    }

    /// Create an allowing decision.
    pub fn allow(energy: f32) -> Self {
        Self::new(
            true,
            energy,
            energy,
            HysteresisState::Low,
            DecisionTrigger::Manual,
            DecisionConfidence::high(),
        )
    }

    /// Create a denying decision.
    pub fn deny(energy: f32, reason: DecisionTrigger) -> Self {
        Self::new(
            false,
            energy,
            energy,
            HysteresisState::High,
            reason,
            DecisionConfidence::high(),
        )
    }

    /// Check if this decision is significant enough to log.
    pub fn is_significant(&self) -> bool {
        !self.allow || self.confidence.overall > 0.8 || self.should_broadcast
    }

    /// Get a human-readable description of the decision.
    pub fn description(&self) -> String {
        let action = if self.allow { "ALLOW" } else { "DENY" };
        let state = match self.hysteresis_state {
            HysteresisState::Low => "coherent",
            HysteresisState::Transition => "uncertain",
            HysteresisState::High => "incoherent",
        };
        format!(
            "{} (energy={:.3}, state={}, confidence={:.2})",
            action, self.energy, state, self.confidence.overall
        )
    }
}

/// Get current time in milliseconds.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_confidence() {
        let low = DecisionConfidence::low();
        assert!(!low.is_trustworthy());

        let high = DecisionConfidence::high();
        assert!(high.is_trustworthy());

        let mixed = DecisionConfidence::new(0.8, 0.7, 0.6, 3);
        assert!(mixed.is_trustworthy());
    }

    #[test]
    fn test_neural_decision() {
        let allow = NeuralDecision::allow(0.1);
        assert!(allow.allow);
        assert_eq!(allow.hysteresis_state, HysteresisState::Low);

        let deny = NeuralDecision::deny(0.9, DecisionTrigger::Manual);
        assert!(!deny.allow);
        assert!(deny.is_significant());
    }
}
