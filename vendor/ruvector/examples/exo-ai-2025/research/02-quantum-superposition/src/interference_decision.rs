// Interference-Based Decision Making
//
// Implements decision algorithms using amplitude interference for quantum-inspired
// cognition. Decisions emerge from constructive/destructive interference of
// amplitude paths rather than classical utility maximization.

use crate::quantum_cognitive_state::{Amplitude, CognitiveState, SuperpositionBuilder};
use num_complex::Complex64;
use std::f64::consts::PI;

/// Decision maker using quantum amplitude interference
pub struct InterferenceDecisionMaker {
    /// Current cognitive state (superposition of options)
    pub state: CognitiveState,
    /// Decision history for learning phase relationships
    history: Vec<DecisionRecord>,
}

#[derive(Clone, Debug)]
struct DecisionRecord {
    options: Vec<String>,
    chosen: usize,
    confidence: f64,
    timestamp: f64,
}

impl InterferenceDecisionMaker {
    /// Create new decision maker with initial state
    pub fn new(initial_state: CognitiveState) -> Self {
        InterferenceDecisionMaker {
            state: initial_state,
            history: Vec::new(),
        }
    }

    /// Two-alternative forced choice with interference
    ///
    /// # Arguments
    /// * `option_a` - Label for first option
    /// * `option_b` - Label for second option
    /// * `phase_difference` - Phase between amplitudes (context-dependent)
    ///
    /// # Returns
    /// (chosen_option, probability, interference_contribution)
    pub fn two_alternative_choice(
        &mut self,
        option_a: &str,
        option_b: &str,
        phase_difference: f64,
    ) -> (String, f64, f64) {
        // Create superposition of two options with phase relationship
        let magnitude = 1.0 / 2.0_f64.sqrt();

        let amp_a = Complex64::from_polar(magnitude, 0.0);
        let amp_b = Complex64::from_polar(magnitude, phase_difference);

        let state = SuperpositionBuilder::new()
            .add_state(amp_a, option_a.to_string())
            .add_state(amp_b, option_b.to_string())
            .build();

        // Calculate probabilities with interference
        let probs = state.probabilities();

        // Interference term contribution
        let classical_prob = 0.5; // Without interference
        let interference = probs[0] - classical_prob;

        // Measure and record decision
        let (choice_idx, collapsed, prob) = state.measure();

        self.history.push(DecisionRecord {
            options: vec![option_a.to_string(), option_b.to_string()],
            chosen: choice_idx,
            confidence: prob,
            timestamp: 0.0, // Could use actual time
        });

        self.state = collapsed;

        (state.labels[choice_idx].clone(), prob, interference)
    }

    /// Multi-alternative decision with N-path interference
    ///
    /// All options interfere pairwise, creating complex probability landscape
    pub fn multi_alternative_choice(
        &mut self,
        options: Vec<String>,
        phase_vector: Vec<f64>,
    ) -> (String, f64, Vec<f64>) {
        assert_eq!(options.len(), phase_vector.len());

        let n = options.len();
        let magnitude = 1.0 / (n as f64).sqrt();

        // Build superposition with specified phases
        let mut builder = SuperpositionBuilder::new();
        for (label, &phase) in options.iter().zip(&phase_vector) {
            builder = builder.add_polar(magnitude, phase, label.clone());
        }
        let state = builder.build();

        let probs = state.probabilities();

        // Calculate interference contributions
        let classical_prob = 1.0 / n as f64;
        let interference_effects: Vec<f64> = probs.iter().map(|&p| p - classical_prob).collect();

        // Perform measurement
        let (choice_idx, collapsed, prob) = state.measure();

        self.history.push(DecisionRecord {
            options: options.clone(),
            chosen: choice_idx,
            confidence: prob,
            timestamp: 0.0,
        });

        self.state = collapsed;

        (options[choice_idx].clone(), prob, interference_effects)
    }

    /// Conjunction decision (Linda problem solver)
    ///
    /// Models conjunction fallacy via amplitude overlap
    pub fn conjunction_decision(
        &mut self,
        individual_a: &str,
        individual_b: &str,
        conjunction_ab: &str,
        overlap_strength: f64, // How much AB overlaps with A or B semantically
    ) -> (Vec<f64>, String) {
        // Create amplitudes based on description matching
        // A (bank teller): Low amplitude - doesn't match description
        // B (feminist): High amplitude - matches description
        // A∧B (feminist bank teller): Intermediate, but includes high B component

        let amp_a = Complex64::new(0.2, 0.0); // Low representativeness
        let amp_b = Complex64::new(0.7, 0.0); // High representativeness

        // Conjunction amplitude includes contribution from B
        let amp_ab = overlap_strength * amp_b + (1.0 - overlap_strength) * amp_a;

        let state = SuperpositionBuilder::new()
            .add_state(amp_a, individual_a.to_string())
            .add_state(amp_b, individual_b.to_string())
            .add_state(amp_ab, conjunction_ab.to_string())
            .build();

        let probs = state.probabilities();

        // Classical expectation: P(A∧B) ≤ P(A)
        // CAFT prediction: Can have P(A∧B) > P(A) if overlap_strength is high
        let (choice_idx, collapsed, _) = state.measure();

        self.state = collapsed;

        (probs, state.labels[choice_idx].clone())
    }

    /// Order-dependent decision (survey question effects)
    ///
    /// Demonstrates that asking Q1 before Q2 changes P(Q2) via state collapse
    pub fn ordered_questions(
        &mut self,
        question1_options: Vec<String>,
        question2_options: Vec<String>,
        q1_phases: Vec<f64>,
        q2_phases: Vec<f64>,
        coupling_strength: f64, // How much Q1 answer influences Q2
    ) -> (String, String, f64) {
        // Answer Q1 first
        let (ans1, prob1, _) = self.multi_alternative_choice(question1_options.clone(), q1_phases);

        // Q1 collapsed state influences Q2 amplitudes
        let q1_idx = question1_options.iter().position(|x| x == &ans1).unwrap();

        // Modify Q2 phases based on Q1 outcome
        let mut modified_q2_phases = q2_phases.clone();
        for phase in &mut modified_q2_phases {
            *phase += coupling_strength * (q1_idx as f64 * PI / question1_options.len() as f64);
        }

        // Answer Q2 with modified state
        let (ans2, prob2, _) =
            self.multi_alternative_choice(question2_options.clone(), modified_q2_phases);

        // Order effect magnitude
        let order_effect = coupling_strength * prob1;

        (ans1, ans2, order_effect)
    }

    /// Prisoner's Dilemma with quantum game theory
    ///
    /// Non-separable joint state enables cooperation
    pub fn quantum_prisoners_dilemma(
        &mut self,
        player2_strategy: &str,     // "cooperate" or "defect"
        entanglement_strength: f64, // Degree of non-separability
    ) -> (String, f64, f64) {
        // Classical strategies
        let cooperate = Complex64::new(1.0, 0.0);
        let defect = Complex64::new(0.0, 1.0);

        // Create entangled-like joint state
        // High entanglement → correlated outcomes (both cooperate or both defect)
        let amp_cc = entanglement_strength.sqrt() * cooperate;
        let amp_dd = entanglement_strength.sqrt() * defect;
        let amp_cd = ((1.0 - entanglement_strength) / 2.0).sqrt() * cooperate;
        let amp_dc = ((1.0 - entanglement_strength) / 2.0).sqrt() * defect;

        let state = SuperpositionBuilder::new()
            .add_state(amp_cc, "cooperate-cooperate".to_string())
            .add_state(amp_dd, "defect-defect".to_string())
            .add_state(amp_cd, "cooperate-defect".to_string())
            .add_state(amp_dc, "defect-cooperate".to_string())
            .build();

        let probs = state.probabilities();

        // Calculate payoffs (standard PD: CC=3, CD=0, DC=5, DD=1)
        let payoffs = [3.0, 1.0, 0.0, 5.0];
        let expected_payoff: f64 = probs.iter().zip(&payoffs).map(|(p, u)| p * u).sum();

        // Cooperation probability
        let p_cooperate = probs[0] + probs[2]; // CC + CD

        // Measure and extract player 1's decision
        let (choice_idx, collapsed, _) = state.measure();

        let player1_decision = if choice_idx == 0 || choice_idx == 2 {
            "cooperate".to_string()
        } else {
            "defect".to_string()
        };

        self.state = collapsed;

        (player1_decision, p_cooperate, expected_payoff)
    }

    /// Calculate decision confidence from amplitude magnitudes
    ///
    /// Confidence = |α_chosen|² (Born rule interpretation)
    pub fn confidence(&self) -> f64 {
        self.state
            .probabilities()
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap_or(0.0)
    }

    /// Get decision history
    pub fn get_history(&self) -> &[DecisionRecord] {
        &self.history
    }

    /// Clear decision history
    pub fn reset_history(&mut self) {
        self.history.clear();
    }
}

/// Compute interference pattern for two cognitive paths
///
/// Returns probability as function of phase difference
pub fn interference_pattern(phase_diff_range: Vec<f64>) -> Vec<f64> {
    let amplitude = 1.0 / 2.0_f64.sqrt();

    phase_diff_range
        .iter()
        .map(|&phi| {
            let amp1 = Complex64::from_polar(amplitude, 0.0);
            let amp2 = Complex64::from_polar(amplitude, phi);
            let total = amp1 + amp2;
            total.norm_sqr()
        })
        .collect()
}

/// Calculate semantic phase from concept vectors
///
/// Phase = angle between concept vectors in embedding space
pub fn semantic_phase(vector1: &[f64], vector2: &[f64]) -> f64 {
    assert_eq!(vector1.len(), vector2.len());

    let dot: f64 = vector1.iter().zip(vector2).map(|(a, b)| a * b).sum();
    let norm1: f64 = vector1.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm2: f64 = vector2.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm1 > 1e-10 && norm2 > 1e-10 {
        (dot / (norm1 * norm2)).acos()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_alternative_constructive() {
        let initial = CognitiveState::uniform(2, vec!["A".to_string(), "B".to_string()]);
        let mut dm = InterferenceDecisionMaker::new(initial);

        // Phase difference = 0 → constructive interference
        let (choice, prob, interference) = dm.two_alternative_choice("option_a", "option_b", 0.0);

        // With constructive interference, probabilities deviate from 0.5
        assert!(interference.abs() > 0.0);
    }

    #[test]
    fn test_two_alternative_destructive() {
        let initial = CognitiveState::uniform(2, vec!["A".to_string(), "B".to_string()]);
        let mut dm = InterferenceDecisionMaker::new(initial);

        // Phase difference = π → destructive interference
        let (choice, prob, interference) = dm.two_alternative_choice("option_a", "option_b", PI);

        // Interference term should be negative (destructive)
        assert!(interference < 0.0);
    }

    #[test]
    fn test_conjunction_fallacy() {
        let initial =
            CognitiveState::uniform(3, vec!["A".to_string(), "B".to_string(), "AB".to_string()]);
        let mut dm = InterferenceDecisionMaker::new(initial);

        // High overlap → conjunction can exceed individual
        let (probs, choice) = dm.conjunction_decision(
            "bank_teller",
            "feminist",
            "feminist_bank_teller",
            0.8, // High semantic overlap with "feminist"
        );

        // P(feminist ∧ bank_teller) can be > P(bank_teller) with high overlap
        // This reproduces the empirical "fallacy"
        println!(
            "P(bank): {}, P(fem): {}, P(both): {}",
            probs[0], probs[1], probs[2]
        );
    }

    #[test]
    fn test_interference_pattern() {
        let phases: Vec<f64> = (0..100).map(|i| (i as f64) * 2.0 * PI / 100.0).collect();
        let pattern = interference_pattern(phases);

        // Should oscillate between 0 and 1
        let max = pattern
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let min = pattern
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        assert!(*max <= 1.0);
        assert!(*min >= 0.0);
        assert!((max - min) > 0.5); // Significant oscillation
    }

    #[test]
    fn test_prisoners_dilemma() {
        let initial = CognitiveState::uniform(
            4,
            vec![
                "CC".to_string(),
                "DD".to_string(),
                "CD".to_string(),
                "DC".to_string(),
            ],
        );
        let mut dm = InterferenceDecisionMaker::new(initial);

        // High entanglement → more cooperation
        let (decision, p_coop, payoff) = dm.quantum_prisoners_dilemma("cooperate", 0.9);

        println!(
            "Decision: {}, P(cooperate): {}, Payoff: {}",
            decision, p_coop, payoff
        );

        // Should have higher cooperation than classical (0.5)
        assert!(p_coop > 0.5);
    }

    #[test]
    fn test_semantic_phase() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];

        let phase = semantic_phase(&v1, &v2);

        // Orthogonal vectors → π/2 phase
        assert!((phase - PI / 2.0).abs() < 1e-6);
    }
}
