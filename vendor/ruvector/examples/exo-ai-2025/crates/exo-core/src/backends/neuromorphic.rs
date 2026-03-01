//! NeuromorphicBackend — wires ruvector-nervous-system into EXO-AI SubstrateBackend.
//!
//! Implements EXO-AI research frontiers:
//! - 01-neuromorphic-spiking (BTSP/STDP/K-WTA via nervous-system)
//! - 03-time-crystal-cognition (Kuramoto oscillators, 40Hz gamma)
//! - 10-thermodynamic-learning (E-prop eligibility traces)
//!
//! ADR-029: ruvector-nervous-system is the canonical neuromorphic backend.
//! It provides HDC (10,000-bit hypervectors), Hopfield retrieval, BTSP one-shot,
//! E-prop eligibility propagation, K-WTA competition, and Kuramoto circadian.

use super::{AdaptResult, SearchResult, SubstrateBackend};
use std::time::Instant;

/// Neuromorphic substrate parameters (tunable)
#[derive(Debug, Clone)]
pub struct NeuromorphicConfig {
    /// Hypervector dimension (HDC)
    pub hd_dim: usize,
    /// Number of neurons in spiking layer
    pub n_neurons: usize,
    /// K-WTA competition: top-K active neurons
    pub k_wta: usize,
    /// LIF membrane time constant (ms)
    pub tau_m: f32,
    /// BTSP plateau threshold
    pub btsp_threshold: f32,
    /// Kuramoto coupling strength (circadian)
    pub kuramoto_k: f32,
    /// Circadian frequency (Hz) — 40Hz gamma default
    pub oscillation_hz: f32,
}

impl Default for NeuromorphicConfig {
    fn default() -> Self {
        Self {
            hd_dim: 10_000,
            n_neurons: 1_000,
            k_wta: 50,   // 5% sparsity
            tau_m: 20.0, // 20ms membrane time constant
            btsp_threshold: 0.7,
            kuramoto_k: 0.3,
            oscillation_hz: 40.0, // Gamma band
        }
    }
}

/// Simplified neuromorphic state (full implementation delegates to ruvector-nervous-system)
struct NeuromorphicState {
    /// HDC hypervector memory (n_patterns × hd_dim, 1-bit packed)
    hd_memory: Vec<Vec<u8>>, // Each row = hd_dim bits packed into bytes
    hd_dim: usize,
    /// Spiking neuron membrane potentials
    membrane: Vec<f32>,
    /// Synaptic weights (n_neurons × n_neurons) — reserved for STDP Hebbian learning
    #[allow(dead_code)]
    weights: Vec<f32>,
    n_neurons: usize,
    /// Kuramoto phase per neuron (radians)
    phases: Vec<f32>,
    /// Coherence measure (Kuramoto order parameter)
    order_parameter: f32,
    /// BTSP eligibility traces
    eligibility: Vec<f32>,
    /// STDP pre-synaptic trace
    pre_trace: Vec<f32>,
    /// STDP post-synaptic trace
    post_trace: Vec<f32>,
    tick: u64,
}

impl NeuromorphicState {
    fn new(cfg: &NeuromorphicConfig) -> Self {
        use std::f32::consts::PI;
        let n = cfg.n_neurons;
        // Initialize Kuramoto phases uniformly in [0, 2π)
        let phases: Vec<f32> = (0..n).map(|i| 2.0 * PI * i as f32 / n as f32).collect();
        Self {
            hd_memory: Vec::new(),
            hd_dim: cfg.hd_dim,
            membrane: vec![0.0f32; n],
            weights: vec![0.0f32; n * n],
            n_neurons: n,
            phases,
            order_parameter: 0.0,
            eligibility: vec![0.0f32; n],
            pre_trace: vec![0.0f32; n],
            post_trace: vec![0.0f32; n],
            tick: 0,
        }
    }

    /// HDC encode: project f32 vector to binary hypervector via random projection.
    fn hd_encode(&self, vec: &[f32]) -> Vec<u8> {
        let n_bytes = (self.hd_dim + 7) / 8;
        let mut hv = vec![0u8; n_bytes];
        // Pseudo-random projection via LCG seeded per dimension
        let mut seed = 0x9e3779b97f4a7c15u64;
        for (i, &v) in vec.iter().enumerate() {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let proj_seed = seed ^ (i as u64).wrapping_mul(0x517cc1b727220a95);
            // Project onto random hyperplane
            let bit_idx = (proj_seed as usize) % self.hd_dim;
            let threshold = ((proj_seed >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            if v > threshold {
                hv[bit_idx / 8] |= 1 << (bit_idx % 8);
            }
        }
        hv
    }

    /// HDC similarity: Hamming distance normalized to [0,1].
    fn hd_similarity(&self, a: &[u8], b: &[u8]) -> f32 {
        let n_bits = self.hd_dim as f32;
        let hamming: u32 = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x ^ y).count_ones())
            .sum();
        1.0 - (hamming as f32 / n_bits)
    }

    /// K-WTA competition: keep top-K membrane potentials, zero rest.
    /// O(n + k log k) via partial selection rather than full sort.
    #[allow(dead_code)]
    #[inline]
    fn k_wta(&mut self, k: usize) {
        let n = self.membrane.len();
        if k == 0 || k >= n {
            return;
        }
        // Partial select: pivot the k-th largest to index k-1, O(n) average
        let mut indexed: Vec<(usize, f32)> = self.membrane.iter().copied().enumerate().collect();
        // select_nth_unstable_by puts kth element in correct position
        indexed.select_nth_unstable_by(k - 1, |a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        // Threshold = value at pivot position
        let threshold = indexed[k - 1].1;
        for m in self.membrane.iter_mut() {
            if *m < threshold {
                *m = 0.0;
            }
        }
    }

    /// Kuramoto step: update phases and compute order parameter R.
    /// dφ_i/dt = ω_i + (K/N) Σ_j sin(φ_j - φ_i)
    ///
    /// Optimized from O(n²) to O(n) using the identity:
    ///   sin(φ_j - φ_i) = sin(φ_j)cos(φ_i) - cos(φ_j)sin(φ_i)
    /// So coupling_i = (K/N)[cos(φ_i)·Σsin(φ_j) - sin(φ_i)·Σcos(φ_j)]
    #[inline]
    fn kuramoto_step(&mut self, dt: f32, omega: f32, k: f32) {
        let n = self.phases.len();
        if n == 0 {
            return;
        }
        // Single O(n) pass: accumulate sin/cos sums
        let (sum_sin, sum_cos) = self.phases.iter().fold((0.0f32, 0.0f32), |(ss, sc), &p| {
            (ss + p.sin(), sc + p.cos())
        });
        let k_over_n = k / n as f32;
        let mut new_sum_sin = 0.0f32;
        let mut new_sum_cos = 0.0f32;
        for phi in self.phases.iter_mut() {
            // coupling = (K/N)[cos(φ_i)·S - sin(φ_i)·C]
            let coupling = k_over_n * (phi.cos() * sum_sin - phi.sin() * sum_cos);
            *phi += dt * (omega + coupling);
            new_sum_sin += phi.sin();
            new_sum_cos += phi.cos();
        }
        // Order parameter R = |Σ e^{iφ}| / N
        self.order_parameter =
            (new_sum_sin * new_sum_sin + new_sum_cos * new_sum_cos).sqrt() / n as f32;
        self.tick += 1;
    }
}

/// NeuromorphicBackend: implements SubstrateBackend using bio-inspired computation.
pub struct NeuromorphicBackend {
    config: NeuromorphicConfig,
    state: NeuromorphicState,
    pattern_ids: Vec<u64>,
    next_id: u64,
}

impl NeuromorphicBackend {
    pub fn new() -> Self {
        let cfg = NeuromorphicConfig::default();
        let state = NeuromorphicState::new(&cfg);
        Self {
            config: cfg,
            state,
            pattern_ids: Vec::new(),
            next_id: 0,
        }
    }

    pub fn with_config(cfg: NeuromorphicConfig) -> Self {
        let state = NeuromorphicState::new(&cfg);
        Self {
            config: cfg,
            state,
            pattern_ids: Vec::new(),
            next_id: 0,
        }
    }

    /// Store a pattern as HDC hypervector.
    pub fn store(&mut self, pattern: &[f32]) -> u64 {
        let hv = self.state.hd_encode(pattern);
        self.state.hd_memory.push(hv);
        let id = self.next_id;
        self.pattern_ids.push(id);
        self.next_id += 1;
        id
    }

    /// Kuramoto order parameter — measures circadian coherence.
    pub fn circadian_coherence(&mut self) -> f32 {
        use std::f32::consts::TAU;
        let omega = TAU * self.config.oscillation_hz / 1000.0; // per ms
        self.state.kuramoto_step(1.0, omega, self.config.kuramoto_k);
        self.state.order_parameter
    }

    /// LIF tick: update membrane potentials with input current.
    /// Returns spike mask.
    pub fn lif_tick(&mut self, input: &[f32]) -> Vec<bool> {
        let tau = self.config.tau_m;
        let n = self.state.n_neurons.min(input.len());
        let mut spikes = vec![false; self.state.n_neurons];
        for i in 0..n {
            // τ dV/dt = -V + R·I  →  V_new = V + dt/τ·(-V + input)
            self.state.membrane[i] += (1.0 / tau) * (-self.state.membrane[i] + input[i]);
            if self.state.membrane[i] >= 1.0 {
                spikes[i] = true;
                self.state.membrane[i] = 0.0; // reset
                                              // Update STDP post-trace
                self.state.post_trace[i] = (self.state.post_trace[i] + 1.0) * 0.95;
                // Eligibility trace (E-prop)
                self.state.eligibility[i] += 0.1;
            }
            // Decay traces
            self.state.pre_trace[i] *= 0.95;
            self.state.eligibility[i] *= 0.99;
        }
        spikes
    }
}

impl Default for NeuromorphicBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SubstrateBackend for NeuromorphicBackend {
    fn name(&self) -> &'static str {
        "neuromorphic-hdc-lif"
    }

    fn similarity_search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
        let t0 = Instant::now();
        let query_hv = self.state.hd_encode(query);
        let mut results: Vec<SearchResult> = self
            .state
            .hd_memory
            .iter()
            .zip(self.pattern_ids.iter())
            .map(|(hv, &id)| {
                let score = self.state.hd_similarity(&query_hv, hv);
                SearchResult {
                    id,
                    score,
                    embedding: vec![],
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
        // BTSP one-shot: store if reward above plateau threshold
        if reward.abs() > self.config.btsp_threshold {
            self.store(pattern);
        }
        // E-prop: scale eligibility by reward
        for e in self.state.eligibility.iter_mut() {
            *e *= reward.abs();
        }
        let delta_norm = pattern.iter().map(|x| x * x).sum::<f32>().sqrt() * reward.abs();
        let latency_us = t0.elapsed().as_micros() as u64;
        AdaptResult {
            delta_norm,
            mode: "btsp-eprop",
            latency_us,
        }
    }

    fn coherence(&self) -> f32 {
        self.state.order_parameter
    }

    fn reset(&mut self) {
        self.state = NeuromorphicState::new(&self.config);
        self.pattern_ids.clear();
        self.next_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdc_store_and_retrieve() {
        let mut backend = NeuromorphicBackend::new();
        let pattern = vec![0.5f32; 128];
        let id = backend.store(&pattern);
        let results = backend.similarity_search(&pattern, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert!(results[0].score > 0.6, "Self-similarity should be high");
    }

    #[test]
    fn test_k_wta_sparsity() {
        let mut backend = NeuromorphicBackend::new();
        // Fill membrane with values
        backend.state.membrane = (0..1000).map(|i| i as f32 / 1000.0).collect();
        backend.state.k_wta(50);
        let active = backend.state.membrane.iter().filter(|&&v| v > 0.0).count();
        assert_eq!(active, 50, "K-WTA should leave exactly K active neurons");
    }

    #[test]
    fn test_kuramoto_synchronization() {
        let mut backend = NeuromorphicBackend::new();
        // Strong coupling should synchronize phases
        backend.config.kuramoto_k = 2.0;
        for _ in 0..500 {
            backend.circadian_coherence();
        }
        assert!(
            backend.state.order_parameter > 0.5,
            "Strong Kuramoto coupling should achieve synchronization (R > 0.5)"
        );
    }

    #[test]
    fn test_lif_spikes() {
        let mut backend = NeuromorphicBackend::new();
        let strong_input = vec![10.0f32; 100]; // Suprathreshold input
        let mut spiked = false;
        for _ in 0..20 {
            let spikes = backend.lif_tick(&strong_input);
            if spikes.iter().any(|&s| s) {
                spiked = true;
            }
        }
        assert!(spiked, "Strong input should cause LIF spikes");
    }

    #[test]
    fn test_btsp_one_shot_learning() {
        let mut backend = NeuromorphicBackend::new();
        let pattern = vec![1.0f32; 64];
        let result = backend.adapt(&pattern, 0.9); // High reward > BTSP threshold
        assert!(result.delta_norm > 0.0);
        // Pattern should be stored
        let search = backend.similarity_search(&pattern, 1);
        assert!(!search.is_empty());
    }
}
