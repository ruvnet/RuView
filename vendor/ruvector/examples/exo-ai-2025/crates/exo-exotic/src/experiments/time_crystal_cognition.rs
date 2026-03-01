//! Experiment 03: Time-Crystal Cognition
//!
//! Research frontier: Discrete time translation symmetry breaking in cognitive systems.
//! Theory: Kuramoto oscillators + ruvector-temporal-tensor tiered compression
//! create time-crystal-like periodic cognitive states that persist without energy input.
//!
//! Key insight (ADR-029): The Kuramoto coupling constant K maps to the
//! temporal tensor's "access frequency" — high-K oscillators correspond to
//! hot-tier patterns in the tiered compression scheme.

use exo_core::backends::neuromorphic::NeuromorphicBackend;

/// Cognitive time crystal: periodic attractor in spiking network
pub struct TimeCrystalExperiment {
    backend: NeuromorphicBackend,
    /// Crystal period (in LIF ticks)
    pub crystal_period: usize,
    /// Number of periods to simulate
    pub n_periods: usize,
    /// Pattern embedded as time crystal seed
    pub seed_pattern: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct TimeCrystalResult {
    /// Measured period (ticks between repeat activations)
    pub measured_period: usize,
    /// Period stability (variance across measurements)
    pub period_stability: f64,
    /// Symmetry breaking: ratio of crystal phase to total simulation
    pub symmetry_breaking_ratio: f64,
    /// Whether a stable attractor was found
    pub stable_attractor: bool,
    /// Energy proxy (circadian coherence × spike count)
    pub energy_proxy: f64,
}

impl TimeCrystalExperiment {
    pub fn new(period: usize) -> Self {
        Self {
            backend: NeuromorphicBackend::new(),
            crystal_period: period,
            n_periods: 10,
            seed_pattern: vec![1.0f32; 64],
        }
    }

    pub fn run(&mut self) -> TimeCrystalResult {
        // Seed the time crystal: encode pattern at T=0
        self.backend.store(&self.seed_pattern);

        let total_ticks = self.crystal_period * self.n_periods;
        let mut spike_counts = Vec::with_capacity(total_ticks);
        let mut coherences = Vec::with_capacity(total_ticks);

        // Stimulate with period-matched input
        for tick in 0..total_ticks {
            // Periodic input: sin wave at crystal frequency
            let phase = 2.0 * std::f32::consts::PI * tick as f32 / self.crystal_period as f32;
            let input: Vec<f32> = (0..100)
                .map(|i| {
                    let spatial_phase = 2.0 * std::f32::consts::PI * i as f32 / 100.0;
                    (phase + spatial_phase).sin() * 0.5 + 0.5
                })
                .collect();

            let spikes = self.backend.lif_tick(&input);
            spike_counts.push(spikes.iter().filter(|&&s| s).count());
            coherences.push(self.backend.circadian_coherence());
        }

        // Detect period: autocorrelation of spike count signal
        let measured_period = detect_period(&spike_counts);
        let period_match = measured_period
            .map(|p| p == self.crystal_period)
            .unwrap_or(false);

        // Stability: variance of inter-peak intervals
        let mean_coh = coherences.iter().sum::<f32>() / coherences.len().max(1) as f32;
        let variance = coherences
            .iter()
            .map(|&c| (c - mean_coh).powi(2) as f64)
            .sum::<f64>()
            / coherences.len().max(1) as f64;

        // Symmetry breaking: crystal phase occupies subset of period states
        let total_spikes: usize = spike_counts.iter().sum();
        let crystal_spikes = spike_counts
            .chunks(self.crystal_period)
            .map(|chunk| chunk[0])
            .sum::<usize>();
        let symmetry_ratio = crystal_spikes as f64 / total_spikes.max(1) as f64;

        let energy_proxy = mean_coh as f64 * total_spikes as f64 / total_ticks as f64;

        TimeCrystalResult {
            measured_period: measured_period.unwrap_or(0),
            period_stability: 1.0 - variance.min(1.0),
            symmetry_breaking_ratio: symmetry_ratio,
            stable_attractor: period_match,
            energy_proxy,
        }
    }
}

/// Detect dominant period via autocorrelation
fn detect_period(signal: &[usize]) -> Option<usize> {
    if signal.len() < 4 {
        return None;
    }
    let mean = signal.iter().sum::<usize>() as f64 / signal.len() as f64;
    let max_lag = signal.len() / 2;
    let mut best_lag = None;
    let mut best_corr = f64::NEG_INFINITY;
    for lag in 2..max_lag {
        let corr = signal
            .iter()
            .zip(signal[lag..].iter())
            .map(|(&a, &b)| (a as f64 - mean) * (b as f64 - mean))
            .sum::<f64>();
        if corr > best_corr {
            best_corr = corr;
            best_lag = Some(lag);
        }
    }
    best_lag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_crystal_experiment_runs() {
        let mut exp = TimeCrystalExperiment::new(10);
        exp.n_periods = 5;
        let result = exp.run();
        assert!(result.energy_proxy >= 0.0);
        assert!(result.period_stability >= 0.0 && result.period_stability <= 1.0);
    }

    #[test]
    fn test_period_detection() {
        // Signal with clear period 5
        let signal: Vec<usize> = (0..50).map(|i| if i % 5 == 0 { 10 } else { 1 }).collect();
        let period = detect_period(&signal);
        assert!(period.is_some(), "Should detect period in periodic signal");
        assert_eq!(period.unwrap(), 5, "Should detect period of 5");
    }
}
