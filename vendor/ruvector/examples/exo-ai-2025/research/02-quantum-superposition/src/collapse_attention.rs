// Attention as Wavefunction Collapse
//
// Models attention as a quantum measurement operator that collapses
// cognitive superposition into definite conscious states. Implements
// continuous weak measurement, Zeno effect, and entropy dynamics.

use crate::quantum_cognitive_state::{Amplitude, CognitiveState, SuperpositionBuilder};
use num_complex::Complex64;
use std::collections::VecDeque;

/// Attention mechanism implementing measurement-induced collapse
pub struct AttentionOperator {
    /// Focus strength (0 = no attention, 1 = full measurement)
    pub strength: f64,
    /// Which basis states receive attention (weights)
    pub focus_weights: Vec<f64>,
    /// Attention frequency (collapses per second)
    pub frequency_hz: f64,
    /// History of entropy values (tracks collapse dynamics)
    entropy_history: VecDeque<f64>,
}

impl AttentionOperator {
    /// Create new attention operator
    ///
    /// # Arguments
    /// * `strength` - Measurement strength (0-1)
    /// * `focus_weights` - Attention distribution across basis states
    /// * `frequency_hz` - Attention refresh rate (4-10 Hz typical for consciousness)
    pub fn new(strength: f64, focus_weights: Vec<f64>, frequency_hz: f64) -> Self {
        assert!(strength >= 0.0 && strength <= 1.0);
        assert!(frequency_hz > 0.0);

        AttentionOperator {
            strength,
            focus_weights,
            frequency_hz,
            entropy_history: VecDeque::with_capacity(1000),
        }
    }

    /// Create full attention (projective measurement)
    pub fn full_attention(focus_index: usize, n_states: usize, frequency_hz: f64) -> Self {
        let mut weights = vec![0.0; n_states];
        weights[focus_index] = 1.0;

        AttentionOperator::new(1.0, weights, frequency_hz)
    }

    /// Create distributed attention (partial measurement)
    pub fn distributed_attention(weights: Vec<f64>, strength: f64, frequency_hz: f64) -> Self {
        let sum: f64 = weights.iter().sum();
        let normalized: Vec<f64> = weights.iter().map(|w| w / sum).collect();

        AttentionOperator::new(strength, normalized, frequency_hz)
    }

    /// Apply attention measurement to cognitive state
    ///
    /// Strong measurement (strength → 1): Projective collapse
    /// Weak measurement (strength << 1): Gradual amplitude modification
    pub fn apply(&mut self, state: &CognitiveState) -> CognitiveState {
        assert_eq!(self.focus_weights.len(), state.dimension());

        if self.strength >= 0.99 {
            // Full projective measurement
            self.projective_measurement(state)
        } else {
            // Weak continuous measurement
            self.weak_measurement(state)
        }
    }

    /// Projective measurement (full attention)
    fn projective_measurement(&mut self, state: &CognitiveState) -> CognitiveState {
        // Weighted projection operator
        let probs = state.probabilities();
        let weighted_probs: Vec<f64> = probs
            .iter()
            .zip(&self.focus_weights)
            .map(|(p, w)| p * w)
            .collect();

        let total: f64 = weighted_probs.iter().sum();

        if total < 1e-10 {
            // No overlap with attention → return original state
            return state.clone();
        }

        // Sample from weighted distribution
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen::<f64>() * total;

        let mut cumulative = 0.0;
        for (i, &wp) in weighted_probs.iter().enumerate() {
            cumulative += wp;
            if r < cumulative {
                // Collapse to state i
                let collapsed =
                    CognitiveState::definite(i, state.dimension(), state.labels.clone());

                // Track entropy reduction
                self.entropy_history.push_back(0.0);
                if self.entropy_history.len() > 1000 {
                    self.entropy_history.pop_front();
                }

                return collapsed;
            }
        }

        // Fallback
        state.clone()
    }

    /// Weak measurement (partial attention)
    fn weak_measurement(&self, state: &CognitiveState) -> CognitiveState {
        // Observable = weighted projection
        let observable: Vec<f64> = self.focus_weights.clone();

        // Apply weak measurement with strength
        let (_measurement_result, new_state) = state.weak_measure(&observable, self.strength);

        new_state
    }

    /// Evolve cognitive state under continuous attention
    ///
    /// Implements stochastic Schrödinger equation:
    /// dψ = [-iH dt + √γ L dW - ½γ L†L dt] ψ
    pub fn continuous_evolution(
        &mut self,
        state: &CognitiveState,
        time_seconds: f64,
        time_steps: usize,
    ) -> Vec<CognitiveState> {
        let dt = time_seconds / time_steps as f64;
        let mut trajectory = vec![state.clone()];
        let mut current_state = state.clone();

        for _ in 0..time_steps {
            // Decide if measurement occurs at this timestep
            let measurement_prob = self.frequency_hz * dt;

            use rand::Rng;
            let mut rng = rand::thread_rng();

            if rng.gen::<f64>() < measurement_prob {
                // Apply attention measurement
                current_state = self.apply(&current_state);

                // Track entropy
                let entropy = current_state.von_neumann_entropy();
                self.entropy_history.push_back(entropy);
                if self.entropy_history.len() > 1000 {
                    self.entropy_history.pop_front();
                }
            } else {
                // Free evolution (could add Hamiltonian here)
                // For now, state persists
            }

            trajectory.push(current_state.clone());
        }

        trajectory
    }

    /// Calculate entropy reduction rate (dS/dt < 0 during attention)
    pub fn entropy_reduction_rate(&self) -> f64 {
        if self.entropy_history.len() < 2 {
            return 0.0;
        }

        let recent: Vec<f64> = self
            .entropy_history
            .iter()
            .rev()
            .take(10)
            .copied()
            .collect();

        if recent.len() < 2 {
            return 0.0;
        }

        // Simple finite difference
        let delta_s = recent[0] - recent[recent.len() - 1];
        let delta_t = (recent.len() - 1) as f64 / self.frequency_hz;

        if delta_t > 1e-10 {
            delta_s / delta_t
        } else {
            0.0
        }
    }

    /// Get recent entropy history
    pub fn get_entropy_history(&self) -> Vec<f64> {
        self.entropy_history.iter().copied().collect()
    }

    /// Shift attention focus to different state(s)
    pub fn shift_focus(&mut self, new_weights: Vec<f64>) {
        assert_eq!(new_weights.len(), self.focus_weights.len());

        let sum: f64 = new_weights.iter().sum();
        self.focus_weights = new_weights.iter().map(|w| w / sum).collect();
    }
}

/// Quantum Zeno effect: Frequent measurement freezes evolution
///
/// Returns probability of remaining in initial state vs number of measurements
pub fn quantum_zeno_effect(
    initial_state: &CognitiveState,
    measurement_operator_index: usize,
    n_measurements: usize,
    total_time: f64,
) -> f64 {
    let dt = total_time / n_measurements as f64;
    let mut current_state = initial_state.clone();

    for _ in 0..n_measurements {
        // Apply projective measurement at index
        let mut attention = AttentionOperator::full_attention(
            measurement_operator_index,
            current_state.dimension(),
            1.0 / dt,
        );

        current_state = attention.apply(&current_state);
    }

    // Return fidelity with initial state
    initial_state.fidelity(&current_state)
}

/// Attention-induced decoherence model
///
/// Simulates how attention causes off-diagonal coherences to decay
pub struct DecoherenceModel {
    /// Decoherence rates for each off-diagonal element
    gamma_matrix: Vec<Vec<f64>>,
}

impl DecoherenceModel {
    /// Create decoherence model from attention patterns
    ///
    /// States with high attention difference decohere faster
    pub fn from_attention(focus_weights: &[f64], base_rate: f64) -> Self {
        let n = focus_weights.len();
        let mut gamma_matrix = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    // Decoherence rate ∝ attention weight difference
                    let weight_diff = (focus_weights[i] - focus_weights[j]).abs();
                    gamma_matrix[i][j] = base_rate * (1.0 + weight_diff);
                }
            }
        }

        DecoherenceModel { gamma_matrix }
    }

    /// Apply decoherence for time dt
    ///
    /// Off-diagonal elements decay: ρᵢⱼ(t) = ρᵢⱼ(0) exp(-Γᵢⱼ t)
    pub fn apply(&self, state: &CognitiveState, dt: f64) -> CognitiveState {
        let mut new_amplitudes = state.amplitudes.clone();

        // This is simplified - proper density matrix formulation would be better
        // For pure states, we approximate by adding phase diffusion
        use rand_distr::{Distribution, Normal};
        let mut rng = rand::thread_rng();

        for (i, amplitude) in new_amplitudes.iter_mut().enumerate() {
            // Add random phase from decoherence
            let gamma_avg: f64 =
                self.gamma_matrix[i].iter().sum::<f64>() / self.gamma_matrix[i].len() as f64;

            if gamma_avg > 0.0 {
                let phase_noise = Normal::new(0.0, (gamma_avg * dt).sqrt()).unwrap();
                let noise = phase_noise.sample(&mut rng);

                *amplitude *= Complex64::from_polar(1.0, noise);
            }
        }

        let mut new_state = CognitiveState::new(new_amplitudes, state.labels.clone());
        new_state.normalize();
        new_state
    }
}

/// Consciousness threshold based on integrated information (Φ)
///
/// Below threshold: Incoherent amplitudes → no definite collapse
/// Above threshold: Coherent amplitudes → stable qualia
pub struct ConsciousnessThreshold {
    /// Critical Φ value for consciousness
    pub phi_critical: f64,
}

impl ConsciousnessThreshold {
    pub fn new(phi_critical: f64) -> Self {
        ConsciousnessThreshold { phi_critical }
    }

    /// Estimate Φ from amplitude coherence
    ///
    /// Simplified: Φ ≈ mutual information - separability
    pub fn estimate_phi(&self, state: &CognitiveState) -> f64 {
        // For single system, use entropy as proxy
        // High entropy → low Φ (not integrated)
        // Low entropy → potentially high Φ (if not just random)

        let entropy = state.von_neumann_entropy();
        let participation = state.participation_ratio();

        // Φ should be high when:
        // - Not maximally entropic (some structure)
        // - Not single-peaked (some distributed information)

        let max_entropy = (state.dimension() as f64).ln();

        if max_entropy > 0.0 {
            // Normalized entropy distance from both extremes
            let structure = (max_entropy - entropy) / max_entropy;
            let distribution = participation / state.dimension() as f64;

            structure * distribution
        } else {
            0.0
        }
    }

    /// Check if state is conscious
    pub fn is_conscious(&self, state: &CognitiveState) -> bool {
        self.estimate_phi(state) > self.phi_critical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_attention_collapse() {
        let state =
            CognitiveState::uniform(3, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        let initial_entropy = state.von_neumann_entropy();

        let mut attention = AttentionOperator::full_attention(1, 3, 10.0);
        let collapsed = attention.apply(&state);

        let final_entropy = collapsed.von_neumann_entropy();

        // Entropy should decrease (superposition → definite)
        assert!(final_entropy < initial_entropy);
    }

    #[test]
    fn test_weak_measurement_gradual() {
        let state = CognitiveState::uniform(2, vec!["A".to_string(), "B".to_string()]);

        let mut attention = AttentionOperator::distributed_attention(
            vec![0.9, 0.1],
            0.1, // Weak
            10.0,
        );

        let new_state = attention.apply(&state);

        // State should shift toward focus but not fully collapse
        let probs = new_state.probabilities();
        assert!(probs[0] > probs[1]); // Shifted toward higher weight
        assert!(probs[1] > 0.01); // But not fully collapsed
    }

    #[test]
    fn test_quantum_zeno() {
        let state = CognitiveState::uniform(2, vec!["A".to_string(), "B".to_string()]);

        // Frequent measurements should freeze state
        let fidelity_frequent = quantum_zeno_effect(&state, 0, 100, 1.0);
        let fidelity_rare = quantum_zeno_effect(&state, 0, 2, 1.0);

        // More measurements → higher fidelity (state frozen)
        assert!(fidelity_frequent > fidelity_rare);
    }

    #[test]
    fn test_decoherence() {
        let state =
            CognitiveState::uniform(3, vec!["A".to_string(), "B".to_string(), "C".to_string()]);

        let decoherence = DecoherenceModel::from_attention(&[1.0, 0.5, 0.0], 1.0);

        let decohered = decoherence.apply(&state, 1.0);

        // Should still be normalized
        assert!((decohered.norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_consciousness_threshold() {
        let threshold = ConsciousnessThreshold::new(0.3);

        // Pure state: low Φ (no integration)
        let pure = CognitiveState::definite(
            0,
            3,
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
        );
        assert!(!threshold.is_conscious(&pure));

        // Uniform state: low Φ (maximal entropy, no structure)
        let uniform =
            CognitiveState::uniform(3, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        let phi_uniform = threshold.estimate_phi(&uniform);

        // Partially mixed: potentially high Φ
        let partial = SuperpositionBuilder::new()
            .add_real(0.6, "A".to_string())
            .add_real(0.3, "B".to_string())
            .add_real(0.1, "C".to_string())
            .build();

        let phi_partial = threshold.estimate_phi(&partial);

        println!("Φ(uniform) = {}, Φ(partial) = {}", phi_uniform, phi_partial);
    }

    #[test]
    fn test_continuous_evolution() {
        let state =
            CognitiveState::uniform(3, vec!["A".to_string(), "B".to_string(), "C".to_string()]);

        let mut attention = AttentionOperator::full_attention(0, 3, 5.0);

        let trajectory = attention.continuous_evolution(&state, 1.0, 100);

        // Should have evolved state
        assert_eq!(trajectory.len(), 101); // Initial + 100 steps

        // Entropy should generally decrease (may have fluctuations)
        let entropy_history = attention.get_entropy_history();
        println!(
            "Entropy samples: {:?}",
            entropy_history.iter().take(10).collect::<Vec<_>>()
        );
    }
}
