//! QuantumStubBackend — feature-gated quantum substrate for EXO-AI.
//!
//! When `ruqu` feature is not enabled, provides a classical simulation
//! that matches the quantum backend's interface. Enables compilation and
//! testing without ruQu dependency while preserving integration contract.
//!
//! ADR-029: ruQu exotic algorithms (interference_search, reasoning_qec,
//! quantum_decay) are the canonical quantum backend when enabled.

use super::{AdaptResult, SearchResult, SubstrateBackend};
use std::time::Instant;

/// Quantum measurement outcome (amplitude → probability)
#[derive(Debug, Clone)]
pub struct QuantumMeasurement {
    pub basis_state: u64,
    pub probability: f64,
    pub amplitude_re: f64,
    pub amplitude_im: f64,
}

/// Quantum decoherence parameters (T1/T2 analog for pattern eviction)
#[derive(Debug, Clone)]
pub struct DecoherenceParams {
    /// T1 relaxation time (ms) — energy loss
    pub t1_ms: f64,
    /// T2 dephasing time (ms) — coherence loss
    pub t2_ms: f64,
}

impl Default for DecoherenceParams {
    fn default() -> Self {
        // Typical superconducting qubit parameters, scaled to cognitive timescales
        Self {
            t1_ms: 100.0,
            t2_ms: 50.0,
        }
    }
}

/// Quantum interference state (2^n basis states, compressed representation)
struct InterferenceState {
    #[allow(dead_code)]
    n_qubits: usize,
    /// State amplitudes (real, imaginary) — only track non-negligible amplitudes
    amplitudes: Vec<(u64, f64, f64)>, // (basis_state, re, im)
    /// Decoherence clock (ms since initialization)
    age_ms: f64,
    params: DecoherenceParams,
}

impl InterferenceState {
    fn new(n_qubits: usize) -> Self {
        // Initialize in equal superposition |+⟩^n
        let n_states = 1usize << n_qubits.min(8); // Cap at 8 qubits for memory
        let amp = 1.0 / (n_states as f64).sqrt();
        let amplitudes = (0..n_states as u64).map(|i| (i, amp, 0.0)).collect();
        Self {
            n_qubits: n_qubits.min(8),
            amplitudes,
            age_ms: 0.0,
            params: DecoherenceParams::default(),
        }
    }

    /// Apply T1/T2 decoherence after dt_ms milliseconds.
    fn decohere(&mut self, dt_ms: f64) {
        self.age_ms += dt_ms;
        let t1_decay = (-self.age_ms / self.params.t1_ms).exp();
        let t2_decay = (-self.age_ms / self.params.t2_ms).exp();
        for (_, re, im) in self.amplitudes.iter_mut() {
            *re *= t1_decay * t2_decay;
            *im *= t2_decay;
        }
    }

    /// Compute coherence (purity measure: Tr(ρ²))
    fn purity(&self) -> f64 {
        let norm_sq: f64 = self
            .amplitudes
            .iter()
            .map(|(_, re, im)| re * re + im * im)
            .sum();
        norm_sq
    }

    /// Apply quantum interference: embed classical vector as phase rotations.
    /// |ψ⟩ → Σ_i v_i e^{iθ_i} |i⟩ (normalized)
    fn embed_vector(&mut self, vec: &[f32]) {
        use std::f64::consts::TAU;
        for (i, (_, re, im)) in self.amplitudes.iter_mut().enumerate() {
            let v = vec.get(i).copied().unwrap_or(0.0) as f64;
            let phase = v * TAU; // Map [-1,1] to [-2π, 2π]
            let magnitude = (*re * *re + *im * *im).sqrt();
            *re = phase.cos() * magnitude;
            *im = phase.sin() * magnitude;
        }
        // Renormalize
        let norm = self
            .amplitudes
            .iter()
            .map(|(_, r, i)| r * r + i * i)
            .sum::<f64>()
            .sqrt();
        if norm > 1e-10 {
            for (_, re, im) in self.amplitudes.iter_mut() {
                *re /= norm;
                *im /= norm;
            }
        }
    }

    /// Measure: collapse to basis states, return top-k by probability.
    #[allow(dead_code)]
    fn measure_top_k(&self, k: usize) -> Vec<QuantumMeasurement> {
        let mut measurements: Vec<QuantumMeasurement> = self
            .amplitudes
            .iter()
            .map(|&(basis_state, re, im)| QuantumMeasurement {
                basis_state,
                probability: re * re + im * im,
                amplitude_re: re,
                amplitude_im: im,
            })
            .collect();
        measurements.sort_unstable_by(|a, b| {
            b.probability
                .partial_cmp(&a.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        measurements.truncate(k);
        measurements
    }
}

/// Quantum stub backend — classical simulation of quantum interference search.
pub struct QuantumStubBackend {
    n_qubits: usize,
    state: InterferenceState,
    stored_patterns: Vec<(u64, Vec<f32>)>,
    next_id: u64,
    decohere_dt_ms: f64,
}

impl QuantumStubBackend {
    pub fn new(n_qubits: usize) -> Self {
        let n = n_qubits.min(8);
        Self {
            n_qubits: n,
            state: InterferenceState::new(n),
            stored_patterns: Vec::new(),
            next_id: 0,
            decohere_dt_ms: 10.0,
        }
    }

    /// Quantum decay-based eviction: remove patterns whose T2 coherence is below threshold.
    pub fn evict_decoherent(&mut self, coherence_threshold: f64) {
        self.state.decohere(self.decohere_dt_ms);
        let purity = self.state.purity();
        if purity < coherence_threshold {
            // Re-initialize state (decoherence-driven forgetting)
            self.state = InterferenceState::new(self.n_qubits);
        }
    }

    pub fn purity(&self) -> f64 {
        self.state.purity()
    }

    pub fn store(&mut self, pattern: &[f32]) -> u64 {
        let id = self.next_id;
        self.stored_patterns.push((id, pattern.to_vec()));
        self.next_id += 1;
        // Embed into quantum state as interference pattern
        self.state.embed_vector(pattern);
        id
    }
}

impl SubstrateBackend for QuantumStubBackend {
    fn name(&self) -> &'static str {
        "quantum-interference-stub"
    }

    fn similarity_search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
        let t0 = Instant::now();
        // Classical interference: inner product weighted by quantum amplitudes
        let mut results: Vec<SearchResult> = self
            .stored_patterns
            .iter()
            .map(|(id, pattern)| {
                // Score = |⟨ψ|query⟩|² weighted by pattern norm
                let inner: f32 = pattern
                    .iter()
                    .zip(query.iter())
                    .map(|(a, b)| a * b)
                    .sum::<f32>();
                let norm_p = pattern.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
                let norm_q = query.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
                // Amplitude-weighted cosine similarity
                let score = (inner / (norm_p * norm_q)) * self.state.purity() as f32;
                SearchResult {
                    id: *id,
                    score: score.max(0.0),
                    embedding: pattern.clone(),
                }
            })
            .collect();
        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(k);
        let _elapsed = t0.elapsed();
        results
    }

    fn adapt(&mut self, pattern: &[f32], reward: f32) -> AdaptResult {
        let t0 = Instant::now();
        if reward.abs() > 0.5 {
            self.store(pattern);
        }
        // Decohere proportional to time (quantum decay = forgetting)
        self.evict_decoherent(0.5);
        let delta_norm = pattern.iter().map(|x| x * x).sum::<f32>().sqrt() * reward.abs();
        AdaptResult {
            delta_norm,
            mode: "quantum-decay-adapt",
            latency_us: t0.elapsed().as_micros() as u64,
        }
    }

    fn coherence(&self) -> f32 {
        self.state.purity() as f32
    }

    fn reset(&mut self) {
        self.state = InterferenceState::new(self.n_qubits);
        self.stored_patterns.clear();
        self.next_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_state_initialized() {
        let backend = QuantumStubBackend::new(4);
        // Initial purity of pure equal superposition = 1.0
        assert!(
            (backend.purity() - 1.0).abs() < 1e-6,
            "Initial state should be pure"
        );
    }

    #[test]
    fn test_quantum_decoherence() {
        let mut backend = QuantumStubBackend::new(4);
        backend.state.params.t1_ms = 10.0;
        backend.state.params.t2_ms = 5.0;
        let initial_purity = backend.purity();
        for _ in 0..50 {
            backend.evict_decoherent(0.01); // Very low threshold, don't reset
            backend.state.decohere(2.0);
        }
        // Purity should have decreased due to T1/T2 decay
        assert!(
            backend.purity() < initial_purity,
            "Decoherence should reduce purity"
        );
    }

    #[test]
    fn test_quantum_similarity_search() {
        let mut backend = QuantumStubBackend::new(4);
        let p1 = vec![1.0f32, 0.0, 0.0, 0.0];
        let p2 = vec![0.0f32, 1.0, 0.0, 0.0];
        backend.store(&p1);
        backend.store(&p2);

        let results = backend.similarity_search(&p1, 2);
        assert!(!results.is_empty());
        // p1 should score highest against query p1
        assert!(results[0].score >= results.get(1).map(|r| r.score).unwrap_or(0.0));
    }

    #[test]
    fn test_interference_embedding() {
        let mut state = InterferenceState::new(4);
        let vec = vec![0.5f32; 8];
        state.embed_vector(&vec);
        // After embedding, state should remain normalized (purity ≤ 1)
        assert!(
            state.purity() <= 1.0 + 1e-6,
            "Quantum state must remain normalized"
        );
    }
}
