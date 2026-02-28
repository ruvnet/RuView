// Quantum Cognitive State: Amplitude-Based Thought Superposition
//
// This module implements the core data structures and operations for
// Cognitive Amplitude Field Theory (CAFT), representing cognitive states
// as complex-valued amplitude vectors in Hilbert space.

use num_complex::Complex64;
use std::ops::{Add, Mul};

/// Complex amplitude representing a cognitive state component
pub type Amplitude = Complex64;

/// Cognitive state vector in N-dimensional Hilbert space
///
/// Represents a superposition of N basis cognitive states (concepts, percepts, decisions)
/// with complex amplitudes. The squared magnitude |α_i|² gives the probability of
/// collapsing to state i upon measurement (Born rule).
#[derive(Clone, Debug)]
pub struct CognitiveState {
    /// Complex amplitudes for each basis state
    pub amplitudes: Vec<Amplitude>,
    /// Optional labels for basis states (concept names)
    pub labels: Vec<String>,
    /// Normalization tracking (should always be ≈ 1.0)
    normalized: bool,
}

impl CognitiveState {
    /// Create new cognitive state from amplitudes
    ///
    /// # Arguments
    /// * `amplitudes` - Complex amplitude coefficients
    /// * `labels` - Optional semantic labels for basis states
    ///
    /// # Example
    /// ```
    /// let psi = CognitiveState::new(
    ///     vec![Complex64::new(0.6, 0.0), Complex64::new(0.0, 0.8)],
    ///     vec!["concept_A".to_string(), "concept_B".to_string()]
    /// );
    /// ```
    pub fn new(amplitudes: Vec<Amplitude>, labels: Vec<String>) -> Self {
        assert_eq!(
            amplitudes.len(),
            labels.len(),
            "Amplitude and label count mismatch"
        );

        let mut state = CognitiveState {
            amplitudes,
            labels,
            normalized: false,
        };
        state.normalize();
        state
    }

    /// Create superposition state with equal amplitudes (maximally uncertain)
    pub fn uniform(n_states: usize, labels: Vec<String>) -> Self {
        let amplitude = Complex64::new(1.0 / (n_states as f64).sqrt(), 0.0);
        CognitiveState::new(vec![amplitude; n_states], labels)
    }

    /// Create definite state (collapsed to single basis state)
    pub fn definite(index: usize, n_states: usize, labels: Vec<String>) -> Self {
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); n_states];
        amplitudes[index] = Complex64::new(1.0, 0.0);
        CognitiveState::new(amplitudes, labels)
    }

    /// Normalize state vector to unit norm: Σ|α_i|² = 1
    pub fn normalize(&mut self) {
        let norm = self.norm();
        if norm > 1e-10 {
            for amplitude in &mut self.amplitudes {
                *amplitude /= norm;
            }
            self.normalized = true;
        }
    }

    /// Calculate norm: √(Σ|α_i|²)
    pub fn norm(&self) -> f64 {
        self.amplitudes
            .iter()
            .map(|a| a.norm_sqr())
            .sum::<f64>()
            .sqrt()
    }

    /// Calculate probabilities for each basis state (Born rule)
    ///
    /// Returns vector where P[i] = |α_i|²
    pub fn probabilities(&self) -> Vec<f64> {
        self.amplitudes.iter().map(|a| a.norm_sqr()).collect()
    }

    /// Inner product ⟨φ|ψ⟩ with another state
    ///
    /// Returns complex amplitude for overlap between states.
    /// Squared magnitude gives transition probability.
    pub fn inner_product(&self, other: &CognitiveState) -> Amplitude {
        assert_eq!(self.amplitudes.len(), other.amplitudes.len());

        self.amplitudes
            .iter()
            .zip(&other.amplitudes)
            .map(|(a, b)| a.conj() * b)
            .sum()
    }

    /// Calculate fidelity F(ψ, φ) = |⟨ψ|φ⟩|²
    ///
    /// Measures "closeness" of two quantum states (0 = orthogonal, 1 = identical)
    pub fn fidelity(&self, other: &CognitiveState) -> f64 {
        self.inner_product(other).norm_sqr()
    }

    /// Perform projective measurement on basis state
    ///
    /// Returns (outcome_index, collapsed_state, measurement_probability)
    /// Implements the projection postulate / wavefunction collapse.
    pub fn measure(&self) -> (usize, CognitiveState, f64) {
        use rand::Rng;

        let probs = self.probabilities();
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen();

        // Sample from Born distribution
        let mut cumulative = 0.0;
        for (i, &p) in probs.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                // Collapse to state i
                let collapsed =
                    CognitiveState::definite(i, self.amplitudes.len(), self.labels.clone());
                return (i, collapsed, p);
            }
        }

        // Fallback (should never reach due to normalization)
        let last = probs.len() - 1;
        (
            last,
            CognitiveState::definite(last, self.amplitudes.len(), self.labels.clone()),
            probs[last],
        )
    }

    /// Weak measurement with strength parameter
    ///
    /// Returns (expectation_value, post_measurement_state)
    /// Performs partial collapse based on measurement strength.
    pub fn weak_measure(&self, observable: &[f64], strength: f64) -> (f64, CognitiveState) {
        use rand_distr::{Distribution, Normal};

        // Calculate expectation value
        let expectation: f64 = self
            .amplitudes
            .iter()
            .zip(observable)
            .map(|(a, &o)| a.norm_sqr() * o)
            .sum();

        // Add measurement noise
        let noise = Normal::new(0.0, 1.0 / strength.sqrt()).unwrap();
        let result = expectation + noise.sample(&mut rand::thread_rng());

        // Apply weak back-action (shift amplitudes toward measurement outcome)
        let mut new_amplitudes = self.amplitudes.clone();
        for (i, amplitude) in new_amplitudes.iter_mut().enumerate() {
            let shift = strength * observable[i] * (*amplitude);
            *amplitude += shift;
        }

        let mut new_state = CognitiveState {
            amplitudes: new_amplitudes,
            labels: self.labels.clone(),
            normalized: false,
        };
        new_state.normalize();

        (result, new_state)
    }

    /// Calculate von Neumann entropy: S = -Σ |α_i|² log|α_i|²
    ///
    /// Measures uncertainty/superposition degree (0 = pure state, log(N) = maximal)
    pub fn von_neumann_entropy(&self) -> f64 {
        self.probabilities()
            .iter()
            .filter(|&&p| p > 1e-10)
            .map(|&p| -p * p.ln())
            .sum()
    }

    /// Calculate participation ratio: PR = 1 / Σ|α_i|⁴
    ///
    /// Measures effective number of states in superposition (1 = pure, N = uniform)
    pub fn participation_ratio(&self) -> f64 {
        let sum_p4: f64 = self.probabilities().iter().map(|&p| p * p).sum();

        if sum_p4 > 1e-10 {
            1.0 / sum_p4
        } else {
            0.0
        }
    }

    /// Get most likely outcome (argmax |α_i|²) and its probability
    pub fn most_likely(&self) -> (usize, f64, &str) {
        let probs = self.probabilities();
        let (idx, &prob) = probs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        (idx, prob, &self.labels[idx])
    }

    /// Number of basis states
    pub fn dimension(&self) -> usize {
        self.amplitudes.len()
    }
}

/// Superposition builder for constructing weighted cognitive states
pub struct SuperpositionBuilder {
    amplitudes: Vec<Amplitude>,
    labels: Vec<String>,
}

impl SuperpositionBuilder {
    pub fn new() -> Self {
        SuperpositionBuilder {
            amplitudes: Vec::new(),
            labels: Vec::new(),
        }
    }

    /// Add a basis state with complex amplitude
    pub fn add_state(mut self, amplitude: Amplitude, label: String) -> Self {
        self.amplitudes.push(amplitude);
        self.labels.push(label);
        self
    }

    /// Add a basis state with real amplitude (zero phase)
    pub fn add_real(mut self, amplitude: f64, label: String) -> Self {
        self.amplitudes.push(Complex64::new(amplitude, 0.0));
        self.labels.push(label);
        self
    }

    /// Add a basis state with magnitude and phase
    pub fn add_polar(mut self, magnitude: f64, phase: f64, label: String) -> Self {
        self.amplitudes
            .push(Complex64::from_polar(magnitude, phase));
        self.labels.push(label);
        self
    }

    /// Build the normalized cognitive state
    pub fn build(self) -> CognitiveState {
        CognitiveState::new(self.amplitudes, self.labels)
    }
}

/// Tensor product of two cognitive states (composite system)
///
/// Creates entangled-like state space for multi-agent or hierarchical cognition
pub fn tensor_product(state1: &CognitiveState, state2: &CognitiveState) -> CognitiveState {
    let n1 = state1.dimension();
    let n2 = state2.dimension();

    let mut amplitudes = Vec::with_capacity(n1 * n2);
    let mut labels = Vec::with_capacity(n1 * n2);

    for i in 0..n1 {
        for j in 0..n2 {
            amplitudes.push(state1.amplitudes[i] * state2.amplitudes[j]);
            labels.push(format!("{}⊗{}", state1.labels[i], state2.labels[j]));
        }
    }

    CognitiveState::new(amplitudes, labels)
}

/// Calculate interference visibility between two paths
///
/// V = (P_max - P_min) / (P_max + P_min) ∈ [0, 1]
pub fn interference_visibility(amplitude1: Amplitude, amplitude2: Amplitude) -> f64 {
    let p1 = amplitude1.norm_sqr();
    let p2 = amplitude2.norm_sqr();

    let p_max = (amplitude1 + amplitude2).norm_sqr();
    let p_min = (amplitude1 - amplitude2).norm_sqr();

    if p_max + p_min > 1e-10 {
        (p_max - p_min) / (p_max + p_min)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_normalization() {
        let psi = CognitiveState::new(
            vec![Complex64::new(3.0, 0.0), Complex64::new(0.0, 4.0)],
            vec!["A".to_string(), "B".to_string()],
        );

        assert!((psi.norm() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_born_rule() {
        let psi = CognitiveState::new(
            vec![Complex64::new(0.6, 0.0), Complex64::new(0.0, 0.8)],
            vec!["A".to_string(), "B".to_string()],
        );

        let probs = psi.probabilities();
        assert!((probs[0] - 0.36).abs() < 1e-10);
        assert!((probs[1] - 0.64).abs() < 1e-10);
    }

    #[test]
    fn test_interference() {
        let a1 = Complex64::new(1.0, 0.0);
        let a2 = Complex64::new(1.0, 0.0);

        let visibility = interference_visibility(a1, a2);
        assert!((visibility - 1.0).abs() < 1e-10); // Perfect constructive

        let a3 = Complex64::new(1.0, 0.0);
        let a4 = Complex64::new(-1.0, 0.0);

        let visibility2 = interference_visibility(a3, a4);
        assert!((visibility2 - 1.0).abs() < 1e-10); // Perfect destructive
    }

    #[test]
    fn test_entropy() {
        // Pure state: S = 0
        let pure = CognitiveState::definite(
            0,
            3,
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
        );
        assert!(pure.von_neumann_entropy() < 1e-10);

        // Maximally mixed: S = log(N)
        let mixed =
            CognitiveState::uniform(3, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert!((mixed.von_neumann_entropy() - (3.0_f64).ln()).abs() < 1e-6);
    }

    #[test]
    fn test_superposition_builder() {
        let psi = SuperpositionBuilder::new()
            .add_real(0.6, "happy".to_string())
            .add_polar(0.8, PI / 2.0, "sad".to_string())
            .build();

        assert_eq!(psi.dimension(), 2);
        assert!((psi.norm() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_product() {
        let state1 = CognitiveState::uniform(2, vec!["A".to_string(), "B".to_string()]);
        let state2 = CognitiveState::uniform(2, vec!["C".to_string(), "D".to_string()]);

        let composite = tensor_product(&state1, &state2);

        assert_eq!(composite.dimension(), 4);
        assert!((composite.norm() - 1.0).abs() < 1e-10);
    }
}
