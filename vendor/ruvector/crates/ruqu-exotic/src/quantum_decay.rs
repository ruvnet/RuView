//! Quantum Decay -- Embeddings decohere instead of being deleted
//!
//! Treats f64 embedding vectors as quantum state amplitudes.  Applies quantum
//! noise channels (dephasing, amplitude damping) over time instead of TTL
//! deletion.  Cold vectors lose phase fidelity before magnitude, and similarity
//! degrades smoothly rather than disappearing at a hard deadline.
//!
//! # Model
//!
//! Two physical noise channels are applied each time [`QuantumEmbedding::decohere`]
//! is called:
//!
//! 1. **Dephasing (T2)** -- random Rz-like phase kicks on every amplitude.
//!    Magnitudes are preserved but phase coherence is scrambled.
//! 2. **Amplitude damping (T1)** -- amplitudes decay toward the |0> ground
//!    state, modelling energy dissipation.  Probability leaked from excited
//!    states is transferred to the ground state.
//!
//! The `noise_rate` parameter controls how aggressively both channels act per
//! unit of abstract time `dt`.

use ruqu_core::state::QuantumState;
use ruqu_core::types::Complex;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the minimum number of qubits needed to hold `len` amplitudes.
/// Always returns at least 1 (QuantumState requires num_qubits >= 1).
fn required_qubits(len: usize) -> u32 {
    if len <= 2 {
        return 1;
    }
    let mut n = 1u32;
    while (1usize << n) < len {
        n += 1;
    }
    n
}

// ---------------------------------------------------------------------------
// QuantumEmbedding
// ---------------------------------------------------------------------------

/// A vector embedding treated as a quantum state that decoheres over time.
///
/// Classical f64 values are normalised and zero-padded to the next power of two,
/// then stored as complex amplitudes of a [`QuantumState`].  Decoherence is
/// modelled by applying stochastic phase and amplitude noise, causing the
/// fidelity with the original state to decay smoothly.
pub struct QuantumEmbedding {
    /// Embedding encoded as quantum amplitudes.
    state: QuantumState,
    /// Snapshot of amplitudes at creation for fidelity tracking.
    original_state: Vec<Complex>,
    /// Dimensionality of the original embedding before power-of-2 padding.
    original_dim: usize,
    /// Abstract time units elapsed since creation.
    age: f64,
    /// Decoherence rate per time unit.
    noise_rate: f64,
}

impl QuantumEmbedding {
    /// Create from a classical f64 embedding vector.
    ///
    /// The embedding is L2-normalised and encoded as purely-real quantum
    /// amplitudes.  If the length is not a power of two, the vector is
    /// zero-padded.  An empty or all-zero embedding is mapped to the |0>
    /// computational basis state.
    pub fn from_embedding(embedding: &[f64], noise_rate: f64) -> Self {
        let original_dim = embedding.len().max(1);
        let num_qubits = required_qubits(original_dim);
        let padded_len = 1usize << num_qubits;

        // L2 normalisation factor
        let norm_sq: f64 = embedding.iter().map(|x| x * x).sum();
        let inv_norm = if norm_sq > 0.0 {
            1.0 / norm_sq.sqrt()
        } else {
            0.0
        };

        // Build zero-padded amplitude vector
        let mut amps = vec![Complex::ZERO; padded_len];
        for (i, &val) in embedding.iter().enumerate() {
            amps[i] = Complex::new(val * inv_norm, 0.0);
        }

        // Degenerate case: put all probability in |0>
        if inv_norm == 0.0 {
            amps[0] = Complex::ONE;
        }

        let original_state = amps.clone();

        let state = QuantumState::from_amplitudes(amps, num_qubits)
            .expect("padded amplitude vector length must equal 2^num_qubits");

        Self {
            state,
            original_state,
            original_dim,
            age: 0.0,
            noise_rate,
        }
    }

    /// Apply decoherence for `dt` time units.
    ///
    /// Two noise channels act in sequence:
    ///
    /// 1. **Dephasing** -- every amplitude is multiplied by e^{i*theta} where
    ///    theta is drawn uniformly from `[-pi * noise_rate * dt, pi * noise_rate * dt]`.
    ///    This scrambles phase coherence while exactly preserving per-amplitude
    ///    probabilities.
    ///
    /// 2. **Amplitude damping** -- each non-ground-state amplitude is scaled by
    ///    sqrt(1 - gamma) where gamma = 1 - e^{-noise_rate * dt}.  The probability
    ///    leaked from excited states is added to the |0> ground state, then the
    ///    whole vector is renormalised.
    ///
    /// The `seed` controls the pseudo-random number generator for
    /// reproducibility.
    pub fn decohere(&mut self, dt: f64, seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);

        // Damping parameter gamma in [0, 1), approaches 1 for large dt * rate
        let gamma = 1.0 - (-self.noise_rate * dt).exp();
        // Phase noise scale in [0, inf)
        let phase_scale = self.noise_rate * dt;

        let amps = self.state.amplitudes_mut();
        let n = amps.len();

        // ------ Phase noise (dephasing) ------
        for amp in amps.iter_mut().take(n) {
            let angle = (rng.gen::<f64>() - 0.5) * 2.0 * PI * phase_scale;
            let phase_kick = Complex::from_polar(1.0, angle);
            *amp = *amp * phase_kick;
        }

        // ------ Amplitude damping toward |0> ------
        let decay_factor = (1.0 - gamma).sqrt();
        let mut leaked_probability = 0.0;

        for amp in amps.iter_mut().skip(1) {
            let prob_before = amp.norm_sq();
            *amp = *amp * decay_factor;
            leaked_probability += prob_before - amp.norm_sq();
        }

        // Transfer leaked probability into the ground state
        let p0 = amps[0].norm_sq();
        let new_p0 = p0 + leaked_probability;
        if new_p0 > 0.0 && p0 > 0.0 {
            amps[0] = amps[0] * (new_p0 / p0).sqrt();
        } else if new_p0 > 0.0 {
            amps[0] = Complex::new(new_p0.sqrt(), 0.0);
        }

        // Correct any accumulated numerical drift
        self.state.normalize();

        self.age += dt;
    }

    /// Fidelity with the original state: |<original|current>|^2 in [0, 1].
    ///
    /// Returns 1.0 for a freshly created embedding (perfect memory) and
    /// decays toward 0.0 as the state decoheres (completely forgotten).
    pub fn fidelity(&self) -> f64 {
        let current = self.state.state_vector();
        let mut inner = Complex::ZERO;
        for (orig, cur) in self.original_state.iter().zip(current.iter()) {
            inner = inner + orig.conj() * *cur;
        }
        inner.norm_sq()
    }

    /// Current age of this embedding in abstract time units.
    pub fn age(&self) -> f64 {
        self.age
    }

    /// Quantum-aware similarity: |<self|other>|^2 as a complex inner product.
    ///
    /// Unlike cosine similarity, this captures phase relationships.  Two
    /// embeddings that have decohered along different random trajectories will
    /// show reduced similarity even if their probability distributions are
    /// similar, because their phases no longer align.
    pub fn quantum_similarity(&self, other: &QuantumEmbedding) -> f64 {
        let sv1 = self.state.state_vector();
        let sv2 = other.state.state_vector();
        let len = sv1.len().min(sv2.len());
        let mut inner = Complex::ZERO;
        for i in 0..len {
            inner = inner + sv1[i].conj() * sv2[i];
        }
        inner.norm_sq()
    }

    /// Extract back to a classical f64 vector.
    ///
    /// Returns the real part of each amplitude, truncated to the original
    /// embedding dimension.  This is lossy when the state has decohered:
    /// dephasing moves energy into imaginary components that are discarded,
    /// and amplitude damping shifts probability toward |0>.
    pub fn to_embedding(&self) -> Vec<f64> {
        self.state
            .state_vector()
            .iter()
            .take(self.original_dim)
            .map(|c| c.re)
            .collect()
    }

    /// Check if the embedding has decohered below a fidelity threshold.
    ///
    /// Returns `true` when the state still retains at least `threshold`
    /// fidelity with its original value.
    pub fn is_coherent(&self, threshold: f64) -> bool {
        self.fidelity() >= threshold
    }
}

// ---------------------------------------------------------------------------
// Batch operations
// ---------------------------------------------------------------------------

/// Apply decoherence to a batch of embeddings, returning indices of those
/// still coherent.
///
/// Each embedding is decohered by `dt` time units using a unique seed derived
/// from the base `seed` and the embedding's index.  Embeddings whose fidelity
/// drops below `threshold` are considered forgotten; the returned vector
/// contains the indices of embeddings that remain coherent.
pub fn decohere_batch(
    embeddings: &mut [QuantumEmbedding],
    dt: f64,
    threshold: f64,
    seed: u64,
) -> Vec<usize> {
    let mut coherent = Vec::new();
    for (i, emb) in embeddings.iter_mut().enumerate() {
        // Derive a per-embedding seed to avoid correlated noise
        let emb_seed = seed
            .wrapping_add(i as u64)
            .wrapping_mul(6_364_136_223_846_793_005);
        emb.decohere(dt, emb_seed);
        if emb.is_coherent(threshold) {
            coherent.push(i);
        }
    }
    coherent
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple embedding of the given dimension.
    fn sample_embedding(dim: usize) -> Vec<f64> {
        (0..dim).map(|i| (i as f64 + 1.0)).collect()
    }

    #[test]
    fn from_embedding_creates_normalised_state() {
        let emb = QuantumEmbedding::from_embedding(&[3.0, 4.0], 0.1);
        let sv = emb.state.state_vector();
        let norm_sq: f64 = sv.iter().map(|c| c.norm_sq()).sum();
        assert!((norm_sq - 1.0).abs() < 1e-10, "state should be normalised");
    }

    #[test]
    fn from_embedding_pads_to_power_of_two() {
        let emb = QuantumEmbedding::from_embedding(&[1.0, 2.0, 3.0], 0.1);
        // 3 elements -> 4 (2 qubits)
        assert_eq!(emb.state.state_vector().len(), 4);
        assert_eq!(emb.state.num_qubits(), 2);
    }

    #[test]
    fn fresh_embedding_has_unit_fidelity() {
        let emb = QuantumEmbedding::from_embedding(&[1.0, 0.0, 0.0, 0.0], 0.1);
        assert!((emb.fidelity() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn decoherence_reduces_fidelity() {
        let mut emb = QuantumEmbedding::from_embedding(&sample_embedding(4), 0.5);
        let f_before = emb.fidelity();
        emb.decohere(1.0, 42);
        let f_after = emb.fidelity();
        assert!(
            f_after < f_before,
            "fidelity should decrease: before={f_before}, after={f_after}"
        );
    }

    #[test]
    fn decoherence_advances_age() {
        let mut emb = QuantumEmbedding::from_embedding(&[1.0, 2.0], 0.1);
        assert!((emb.age() - 0.0).abs() < 1e-15);
        emb.decohere(0.5, 1);
        assert!((emb.age() - 0.5).abs() < 1e-15);
        emb.decohere(1.5, 2);
        assert!((emb.age() - 2.0).abs() < 1e-15);
    }

    #[test]
    fn heavy_decoherence_destroys_fidelity() {
        let mut emb = QuantumEmbedding::from_embedding(&sample_embedding(8), 2.0);
        for i in 0..20 {
            emb.decohere(1.0, 100 + i);
        }
        assert!(
            emb.fidelity() < 0.3,
            "heavy decoherence should destroy fidelity: {}",
            emb.fidelity()
        );
    }

    #[test]
    fn quantum_similarity_is_symmetric() {
        let a = QuantumEmbedding::from_embedding(&[1.0, 0.0, 0.0, 0.0], 0.1);
        let b = QuantumEmbedding::from_embedding(&[0.0, 1.0, 0.0, 0.0], 0.1);
        let sim_ab = a.quantum_similarity(&b);
        let sim_ba = b.quantum_similarity(&a);
        assert!(
            (sim_ab - sim_ba).abs() < 1e-10,
            "similarity should be symmetric"
        );
    }

    #[test]
    fn identical_embeddings_have_similarity_one() {
        let a = QuantumEmbedding::from_embedding(&[1.0, 2.0, 3.0, 4.0], 0.1);
        let b = QuantumEmbedding::from_embedding(&[1.0, 2.0, 3.0, 4.0], 0.1);
        assert!(
            (a.quantum_similarity(&b) - 1.0).abs() < 1e-10,
            "identical embeddings should have similarity 1.0"
        );
    }

    #[test]
    fn to_embedding_round_trips_without_decoherence() {
        let original = vec![3.0, 4.0];
        let emb = QuantumEmbedding::from_embedding(&original, 0.1);
        let recovered = emb.to_embedding();
        assert_eq!(recovered.len(), original.len());
        // Should be the normalised version of the original
        let norm = (3.0f64 * 3.0 + 4.0 * 4.0).sqrt();
        assert!((recovered[0] - 3.0 / norm).abs() < 1e-10);
        assert!((recovered[1] - 4.0 / norm).abs() < 1e-10);
    }

    #[test]
    fn is_coherent_respects_threshold() {
        let mut emb = QuantumEmbedding::from_embedding(&sample_embedding(4), 1.0);
        assert!(emb.is_coherent(0.9));
        // Decohere heavily
        for i in 0..10 {
            emb.decohere(1.0, 200 + i);
        }
        assert!(!emb.is_coherent(0.99));
    }

    #[test]
    fn decohere_batch_filters_correctly() {
        let mut batch: Vec<QuantumEmbedding> = (0..5)
            .map(|i| {
                QuantumEmbedding::from_embedding(
                    &sample_embedding(4),
                    // Higher noise rate for later embeddings
                    0.1 * (i as f64 + 1.0),
                )
            })
            .collect();

        let coherent = decohere_batch(&mut batch, 1.0, 0.3, 999);
        // Embeddings with lower noise rates should remain coherent longer
        // At least the lowest-noise-rate embedding should survive
        assert!(
            !coherent.is_empty(),
            "at least some embeddings should remain coherent with mild decoherence"
        );
        // The first embedding (lowest noise) should be the most likely to survive
        if !coherent.is_empty() {
            assert_eq!(coherent[0], 0, "lowest-noise embedding should survive");
        }
    }

    #[test]
    fn empty_embedding_handled() {
        let emb = QuantumEmbedding::from_embedding(&[], 0.1);
        assert!((emb.fidelity() - 1.0).abs() < 1e-10);
        let recovered = emb.to_embedding();
        // original_dim is max(0, 1) = 1
        assert_eq!(recovered.len(), 1);
    }

    #[test]
    fn zero_noise_rate_preserves_fidelity() {
        let mut emb = QuantumEmbedding::from_embedding(&sample_embedding(4), 0.0);
        emb.decohere(10.0, 42);
        // With noise_rate=0, gamma=0 and phase_scale=0, so no change
        assert!(
            (emb.fidelity() - 1.0).abs() < 1e-10,
            "zero noise rate should preserve fidelity perfectly"
        );
    }
}
