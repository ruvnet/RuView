//! Experiment 01: Neuromorphic Spiking Neural Network Cognition
//!
//! Research frontier: EXO-AI + ruvector-nervous-system integration
//! Theory: Spike-timing-dependent plasticity (STDP) with behavioral timescale
//! learning (BTSP) enables one-shot pattern acquisition in cognitive substrate.
//!
//! ADR-029: ruvector-nervous-system provides BTSP/STDP/K-WTA/HDC/Hopfield.
//! This experiment demonstrates the integration and documents emergent properties.

use exo_core::backends::neuromorphic::{NeuromorphicBackend, NeuromorphicConfig};
use exo_core::backends::SubstrateBackend as _;

/// Experiment configuration
pub struct NeuromorphicExperiment {
    backend: NeuromorphicBackend,
    /// Number of stimulation cycles
    pub n_cycles: usize,
    /// STDP window (ms)
    pub stdp_window_ms: f32,
    /// Patterns to memorize
    pub patterns: Vec<Vec<f32>>,
}

/// Emergent property discovered during experiment
#[derive(Debug, Clone)]
pub struct EmergentProperty {
    pub name: &'static str,
    pub description: &'static str,
    pub measured_value: f64,
    pub theoretical_prediction: f64,
}

/// Result of running the neuromorphic experiment
pub struct NeuromorphicResult {
    pub retrieved_patterns: usize,
    pub total_patterns: usize,
    pub retrieval_accuracy: f64,
    pub circadian_coherence: f32,
    pub spike_sparsity: f64,
    pub emergent_properties: Vec<EmergentProperty>,
    pub latency_us: u64,
}

impl NeuromorphicExperiment {
    pub fn new() -> Self {
        let config = NeuromorphicConfig {
            hd_dim: 10_000,
            n_neurons: 500,
            k_wta: 25, // 5% sparsity
            tau_m: 20.0,
            btsp_threshold: 0.6,
            kuramoto_k: 0.5,
            oscillation_hz: 40.0,
        };
        Self {
            backend: NeuromorphicBackend::with_config(config),
            n_cycles: 20,
            stdp_window_ms: 20.0,
            patterns: Vec::new(),
        }
    }

    /// Load patterns to be memorized (one-shot via BTSP)
    pub fn load_patterns(&mut self, patterns: Vec<Vec<f32>>) {
        self.patterns = patterns;
    }

    /// Run the experiment: store patterns, stimulate, test recall
    pub fn run(&mut self) -> NeuromorphicResult {
        use std::time::Instant;
        let t0 = Instant::now();

        // Phase 1: One-shot encoding via BTSP
        for pattern in &self.patterns {
            self.backend.store(pattern);
        }

        // Phase 2: Simulate circadian rhythm to allow consolidation
        let mut final_coherence = 0.0f32;
        for _ in 0..self.n_cycles {
            final_coherence = self.backend.circadian_coherence();
        }

        // Phase 3: Test recall with noisy queries
        let mut retrieved = 0usize;
        for pattern in &self.patterns {
            // Add 10% noise to query
            let noisy_query: Vec<f32> = pattern
                .iter()
                .map(|&v| v + (v * 0.1 * (rand_f32() - 0.5)))
                .collect();
            let results = self.backend.similarity_search(&noisy_query, 1);
            if let Some(r) = results.first() {
                if r.score > 0.5 {
                    retrieved += 1;
                }
            }
        }

        // Phase 4: LIF spike test for sparsity measurement
        let test_input: Vec<f32> = (0..100).map(|i| (i as f32 / 50.0 - 1.0).abs()).collect();
        let mut total_spikes = 0usize;
        for _ in 0..10 {
            let spikes = self.backend.lif_tick(&test_input);
            total_spikes += spikes.iter().filter(|&&s| s).count();
        }
        let spike_sparsity = 1.0 - (total_spikes as f64 / (100 * 10) as f64);

        let n = self.patterns.len().max(1);
        let accuracy = retrieved as f64 / n as f64;

        let emergent = vec![
            EmergentProperty {
                name: "Gamma Synchronization",
                description: "40Hz Kuramoto oscillators synchronize during memory consolidation",
                measured_value: final_coherence as f64,
                theoretical_prediction: 0.6, // Kuramoto theory: R → 1 for K > K_c
            },
            EmergentProperty {
                name: "Sparse Population Code",
                description: "K-WTA enforces 5% sparsity — matches cortical observations",
                measured_value: spike_sparsity,
                theoretical_prediction: 0.95, // 5% active = 95% sparse
            },
            EmergentProperty {
                name: "One-Shot Retrieval",
                description: "BTSP enables retrieval with 10% noise after single presentation",
                measured_value: accuracy,
                theoretical_prediction: 0.7,
            },
        ];

        NeuromorphicResult {
            retrieved_patterns: retrieved,
            total_patterns: n,
            retrieval_accuracy: accuracy,
            circadian_coherence: final_coherence,
            spike_sparsity,
            emergent_properties: emergent,
            latency_us: t0.elapsed().as_micros() as u64,
        }
    }
}

impl Default for NeuromorphicExperiment {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple deterministic pseudo-random f32 in [0,1) for reproducibility
fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0x517cc1b727220a95);
    let s = SEED.fetch_add(0x6c62272e07bb0142, Ordering::Relaxed);
    let s2 = s.wrapping_mul(0x9e3779b97f4a7c15);
    (s2 >> 33) as f32 / (1u64 << 31) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neuromorphic_experiment_runs() {
        let mut exp = NeuromorphicExperiment::new();
        let patterns: Vec<Vec<f32>> = (0..5)
            .map(|i| (0..64).map(|j| (i * j) as f32 / 64.0).collect())
            .collect();
        exp.load_patterns(patterns);
        let result = exp.run();
        assert_eq!(result.total_patterns, 5);
        assert!(result.spike_sparsity > 0.5, "Should maintain >50% sparsity");
        assert!(!result.emergent_properties.is_empty());
    }

    #[test]
    fn test_emergent_gamma_synchronization() {
        let mut exp = NeuromorphicExperiment::new();
        exp.n_cycles = 200; // More cycles → better synchronization
        exp.load_patterns(vec![vec![0.5f32; 32]]);
        let result = exp.run();
        let gamma = result
            .emergent_properties
            .iter()
            .find(|e| e.name == "Gamma Synchronization")
            .expect("Gamma synchronization should be measured");
        assert!(
            gamma.measured_value > 0.0,
            "Kuramoto order parameter should be nonzero"
        );
    }
}
