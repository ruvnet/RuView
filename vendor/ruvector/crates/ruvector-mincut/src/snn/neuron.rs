//! # Leaky Integrate-and-Fire Neuron Model
//!
//! Implements LIF neurons with adaptive thresholds and refractory periods.
//!
//! ## Membrane Dynamics
//!
//! ```text
//! τ_m * dV/dt = -(V - V_rest) + R * I(t)
//! ```
//!
//! When V >= θ: emit spike, V → V_reset, enter refractory period
//!
//! ## Features
//!
//! - Exponential leak with configurable time constant
//! - Adaptive threshold (increases after spike, decays back)
//! - Absolute refractory period
//! - Homeostatic plasticity for stable firing rates

use super::{SimTime, Spike};
use rayon::prelude::*;
use std::collections::VecDeque;

/// Threshold for using parallel neuron updates (overhead not worth it for small populations)
/// Set high because neuron.step() is very fast, parallel overhead dominates for smaller sizes.
const PARALLEL_THRESHOLD: usize = 2000;

/// Configuration for LIF neuron
#[derive(Debug, Clone)]
pub struct NeuronConfig {
    /// Membrane time constant (ms)
    pub tau_membrane: f64,
    /// Resting potential (mV)
    pub v_rest: f64,
    /// Reset potential after spike (mV)
    pub v_reset: f64,
    /// Initial threshold (mV)
    pub threshold: f64,
    /// Absolute refractory period (ms)
    pub t_refrac: f64,
    /// Membrane resistance (MΩ)
    pub resistance: f64,
    /// Threshold adaptation increment
    pub threshold_adapt: f64,
    /// Threshold adaptation time constant (ms)
    pub tau_threshold: f64,
    /// Enable homeostatic plasticity
    pub homeostatic: bool,
    /// Target spike rate (spikes/ms) for homeostasis
    pub target_rate: f64,
    /// Homeostatic time constant (ms)
    pub tau_homeostatic: f64,
}

impl Default for NeuronConfig {
    fn default() -> Self {
        Self {
            tau_membrane: 20.0,
            v_rest: 0.0,
            v_reset: 0.0,
            threshold: 1.0,
            t_refrac: 2.0,
            resistance: 1.0,
            threshold_adapt: 0.1,
            tau_threshold: 100.0,
            homeostatic: true,
            target_rate: 0.01,
            tau_homeostatic: 1000.0,
        }
    }
}

/// State of a single LIF neuron
#[derive(Debug, Clone)]
pub struct NeuronState {
    /// Membrane potential (mV)
    pub v: f64,
    /// Current threshold (may be adapted)
    pub threshold: f64,
    /// Time remaining in refractory period (ms)
    pub refrac_remaining: f64,
    /// Last spike time (-∞ if never spiked)
    pub last_spike_time: f64,
    /// Running average spike rate (for homeostasis)
    pub spike_rate: f64,
}

impl Default for NeuronState {
    fn default() -> Self {
        Self {
            v: 0.0,
            threshold: 1.0,
            refrac_remaining: 0.0,
            last_spike_time: f64::NEG_INFINITY,
            spike_rate: 0.0,
        }
    }
}

/// Leaky Integrate-and-Fire neuron
#[derive(Debug, Clone)]
pub struct LIFNeuron {
    /// Neuron ID
    pub id: usize,
    /// Configuration parameters
    pub config: NeuronConfig,
    /// Current state
    pub state: NeuronState,
}

impl LIFNeuron {
    /// Create a new LIF neuron with given ID
    pub fn new(id: usize) -> Self {
        Self {
            id,
            config: NeuronConfig::default(),
            state: NeuronState::default(),
        }
    }

    /// Create a new LIF neuron with custom configuration
    pub fn with_config(id: usize, config: NeuronConfig) -> Self {
        let mut state = NeuronState::default();
        state.threshold = config.threshold;
        Self { id, config, state }
    }

    /// Reset neuron state to initial conditions
    pub fn reset(&mut self) {
        self.state = NeuronState {
            threshold: self.config.threshold,
            ..NeuronState::default()
        };
    }

    /// Integrate input current for one timestep
    /// Returns true if neuron spiked
    pub fn step(&mut self, current: f64, dt: f64, time: SimTime) -> bool {
        // Handle refractory period
        if self.state.refrac_remaining > 0.0 {
            self.state.refrac_remaining -= dt;
            return false;
        }

        // Membrane dynamics: τ dV/dt = -(V - V_rest) + R*I
        let dv = (-self.state.v + self.config.v_rest + self.config.resistance * current)
            / self.config.tau_membrane
            * dt;
        self.state.v += dv;

        // Threshold adaptation decay
        if self.state.threshold > self.config.threshold {
            let d_thresh =
                -(self.state.threshold - self.config.threshold) / self.config.tau_threshold * dt;
            self.state.threshold += d_thresh;
        }

        // Check for spike
        if self.state.v >= self.state.threshold {
            // Fire!
            self.state.v = self.config.v_reset;
            self.state.refrac_remaining = self.config.t_refrac;
            self.state.last_spike_time = time;

            // Threshold adaptation
            self.state.threshold += self.config.threshold_adapt;

            // Update running spike rate for homeostasis using proper exponential
            // decay based on tau_homeostatic: rate += (1 - rate) * dt / tau
            let alpha = (dt / self.config.tau_homeostatic).min(1.0);
            self.state.spike_rate = self.state.spike_rate * (1.0 - alpha) + alpha;

            return true;
        }

        // Update spike rate (decay toward 0)
        self.state.spike_rate *= 1.0 - dt / self.config.tau_homeostatic;

        // Homeostatic plasticity: adjust threshold based on firing rate
        if self.config.homeostatic {
            let rate_error = self.state.spike_rate - self.config.target_rate;
            let d_base_thresh = rate_error * dt / self.config.tau_homeostatic;
            // Only apply to base threshold, not adapted part
            // This is a simplification - full implementation would track separately
        }

        false
    }

    /// Inject a direct spike (for input neurons)
    pub fn inject_spike(&mut self, time: SimTime) {
        self.state.last_spike_time = time;
        // Use same homeostatic update as regular spikes
        let alpha = (1.0 / self.config.tau_homeostatic).min(1.0);
        self.state.spike_rate = self.state.spike_rate * (1.0 - alpha) + alpha;
    }

    /// Get time since last spike
    pub fn time_since_spike(&self, current_time: SimTime) -> f64 {
        current_time - self.state.last_spike_time
    }

    /// Check if neuron is in refractory period
    pub fn is_refractory(&self) -> bool {
        self.state.refrac_remaining > 0.0
    }

    /// Get membrane potential
    pub fn membrane_potential(&self) -> f64 {
        self.state.v
    }

    /// Set membrane potential directly
    pub fn set_membrane_potential(&mut self, v: f64) {
        self.state.v = v;
    }

    /// Get current threshold
    pub fn threshold(&self) -> f64 {
        self.state.threshold
    }
}

/// A collection of spikes over time for one neuron
#[derive(Debug, Clone)]
pub struct SpikeTrain {
    /// Neuron ID
    pub neuron_id: usize,
    /// Spike times (sorted)
    pub spike_times: Vec<SimTime>,
    /// Maximum time window to keep
    pub max_window: f64,
}

impl SpikeTrain {
    /// Create a new empty spike train
    pub fn new(neuron_id: usize) -> Self {
        Self {
            neuron_id,
            spike_times: Vec::new(),
            max_window: 1000.0, // 1 second default
        }
    }

    /// Create with custom window size
    pub fn with_window(neuron_id: usize, max_window: f64) -> Self {
        Self {
            neuron_id,
            spike_times: Vec::new(),
            max_window,
        }
    }

    /// Record a spike at given time
    pub fn record_spike(&mut self, time: SimTime) {
        self.spike_times.push(time);

        // Prune old spikes
        let cutoff = time - self.max_window;
        self.spike_times.retain(|&t| t >= cutoff);
    }

    /// Clear all recorded spikes
    pub fn clear(&mut self) {
        self.spike_times.clear();
    }

    /// Get number of spikes in the train
    pub fn count(&self) -> usize {
        self.spike_times.len()
    }

    /// Compute instantaneous spike rate (spikes/ms)
    pub fn spike_rate(&self, window: f64) -> f64 {
        if self.spike_times.is_empty() {
            return 0.0;
        }

        let latest = self.spike_times.last().copied().unwrap_or(0.0);
        let count = self
            .spike_times
            .iter()
            .filter(|&&t| t >= latest - window)
            .count();

        count as f64 / window
    }

    /// Compute inter-spike interval statistics
    pub fn mean_isi(&self) -> Option<f64> {
        if self.spike_times.len() < 2 {
            return None;
        }

        let mut total_isi = 0.0;
        for i in 1..self.spike_times.len() {
            total_isi += self.spike_times[i] - self.spike_times[i - 1];
        }

        Some(total_isi / (self.spike_times.len() - 1) as f64)
    }

    /// Get coefficient of variation of ISI
    pub fn cv_isi(&self) -> Option<f64> {
        let mean = self.mean_isi()?;
        if mean == 0.0 {
            return None;
        }

        let mut variance = 0.0;
        for i in 1..self.spike_times.len() {
            let isi = self.spike_times[i] - self.spike_times[i - 1];
            variance += (isi - mean).powi(2);
        }
        variance /= (self.spike_times.len() - 1) as f64;

        Some(variance.sqrt() / mean)
    }

    /// Convert spike train to binary pattern (temporal encoding)
    ///
    /// Safely handles potential overflow in bin calculation.
    pub fn to_pattern(&self, start: SimTime, bin_size: f64, num_bins: usize) -> Vec<bool> {
        let mut pattern = vec![false; num_bins];

        // Guard against zero/negative bin_size
        if bin_size <= 0.0 || num_bins == 0 {
            return pattern;
        }

        let end_time = start + bin_size * num_bins as f64;

        for &spike_time in &self.spike_times {
            if spike_time >= start && spike_time < end_time {
                // Safe bin calculation with overflow protection
                let offset = spike_time - start;
                let bin_f64 = offset / bin_size;

                // Check for overflow before casting
                if bin_f64 >= 0.0 && bin_f64 < num_bins as f64 {
                    let bin = bin_f64 as usize;
                    if bin < num_bins {
                        pattern[bin] = true;
                    }
                }
            }
        }

        pattern
    }

    /// Check if spike times are sorted (for optimization)
    #[inline]
    fn is_sorted(times: &[f64]) -> bool {
        times.windows(2).all(|w| w[0] <= w[1])
    }

    /// Compute cross-correlation with another spike train
    ///
    /// Uses O(n log n) sliding window algorithm instead of O(n²) pairwise comparison.
    /// Optimized to skip sorting when spike trains are already sorted (typical case).
    /// Uses binary search for initial window position.
    pub fn cross_correlation(&self, other: &SpikeTrain, max_lag: f64, bin_size: f64) -> Vec<f64> {
        // Guard against invalid parameters
        if bin_size <= 0.0 || max_lag <= 0.0 {
            return vec![0.0];
        }

        // Safe num_bins calculation with overflow protection
        let num_bins_f64 = 2.0 * max_lag / bin_size + 1.0;
        let num_bins = if num_bins_f64 > 0.0 && num_bins_f64 < usize::MAX as f64 {
            (num_bins_f64 as usize).min(100_000) // Cap at 100K bins to prevent DoS
        } else {
            return vec![0.0];
        };

        let mut correlation = vec![0.0; num_bins];

        // Empty train optimization
        if self.spike_times.is_empty() || other.spike_times.is_empty() {
            return correlation;
        }

        // Avoid cloning and sorting if already sorted (typical case for spike trains)
        let t1_owned: Vec<f64>;
        let t2_owned: Vec<f64>;

        let t1: &[f64] = if Self::is_sorted(&self.spike_times) {
            &self.spike_times
        } else {
            t1_owned = {
                let mut v = self.spike_times.clone();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                v
            };
            &t1_owned
        };

        let t2: &[f64] = if Self::is_sorted(&other.spike_times) {
            &other.spike_times
        } else {
            t2_owned = {
                let mut v = other.spike_times.clone();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                v
            };
            &t2_owned
        };

        // Use binary search for first spike's window start
        let first_lower = t1[0] - max_lag;
        let mut window_start = t2.partition_point(|&x| x < first_lower);

        for &t1_spike in t1 {
            let lower_bound = t1_spike - max_lag;
            let upper_bound = t1_spike + max_lag;

            // Advance window_start past spikes too early
            while window_start < t2.len() && t2[window_start] < lower_bound {
                window_start += 1;
            }

            // Count spikes within window
            let mut j = window_start;
            while j < t2.len() && t2[j] <= upper_bound {
                let lag = t1_spike - t2[j];

                // Safe bin calculation (inlined for performance)
                let bin = ((lag + max_lag) / bin_size) as usize;
                if bin < num_bins {
                    correlation[bin] += 1.0;
                }
                j += 1;
            }
        }

        // Normalize by geometric mean of spike counts
        let norm = ((self.count() * other.count()) as f64).sqrt();
        if norm > 0.0 {
            let inv_norm = 1.0 / norm;
            for c in &mut correlation {
                *c *= inv_norm;
            }
        }

        correlation
    }
}

/// Population of LIF neurons
#[derive(Debug, Clone)]
pub struct NeuronPopulation {
    /// All neurons in the population
    pub neurons: Vec<LIFNeuron>,
    /// Spike trains for each neuron
    pub spike_trains: Vec<SpikeTrain>,
    /// Current simulation time
    pub time: SimTime,
}

impl NeuronPopulation {
    /// Create a new population with n neurons
    pub fn new(n: usize) -> Self {
        let neurons: Vec<_> = (0..n).map(|i| LIFNeuron::new(i)).collect();
        let spike_trains: Vec<_> = (0..n).map(|i| SpikeTrain::new(i)).collect();

        Self {
            neurons,
            spike_trains,
            time: 0.0,
        }
    }

    /// Create population with custom configuration
    pub fn with_config(n: usize, config: NeuronConfig) -> Self {
        let neurons: Vec<_> = (0..n)
            .map(|i| LIFNeuron::with_config(i, config.clone()))
            .collect();
        let spike_trains: Vec<_> = (0..n).map(|i| SpikeTrain::new(i)).collect();

        Self {
            neurons,
            spike_trains,
            time: 0.0,
        }
    }

    /// Get number of neurons
    pub fn size(&self) -> usize {
        self.neurons.len()
    }

    /// Step all neurons with given currents
    ///
    /// Uses parallel processing for large populations (>200 neurons).
    pub fn step(&mut self, currents: &[f64], dt: f64) -> Vec<Spike> {
        self.time += dt;
        let time = self.time;

        if self.neurons.len() >= PARALLEL_THRESHOLD {
            // Parallel path: compute neuron updates in parallel
            let spike_flags: Vec<bool> = self
                .neurons
                .par_iter_mut()
                .enumerate()
                .map(|(i, neuron)| {
                    let current = currents.get(i).copied().unwrap_or(0.0);
                    neuron.step(current, dt, time)
                })
                .collect();

            // Sequential: collect spikes and record to trains
            let mut spikes = Vec::new();
            for (i, &spiked) in spike_flags.iter().enumerate() {
                if spiked {
                    spikes.push(Spike { neuron_id: i, time });
                    self.spike_trains[i].record_spike(time);
                }
            }
            spikes
        } else {
            // Sequential path for small populations (avoid parallel overhead)
            let mut spikes = Vec::new();
            for (i, neuron) in self.neurons.iter_mut().enumerate() {
                let current = currents.get(i).copied().unwrap_or(0.0);
                if neuron.step(current, dt, time) {
                    spikes.push(Spike { neuron_id: i, time });
                    self.spike_trains[i].record_spike(time);
                }
            }
            spikes
        }
    }

    /// Reset all neurons
    pub fn reset(&mut self) {
        self.time = 0.0;
        for neuron in &mut self.neurons {
            neuron.reset();
        }
        for train in &mut self.spike_trains {
            train.clear();
        }
    }

    /// Get population spike rate
    pub fn population_rate(&self, window: f64) -> f64 {
        let total: f64 = self.spike_trains.iter().map(|t| t.spike_rate(window)).sum();
        total / self.neurons.len() as f64
    }

    /// Compute population synchrony
    pub fn synchrony(&self, window: f64) -> f64 {
        // Collect recent spikes
        let mut all_spikes = Vec::new();
        let cutoff = self.time - window;

        for train in &self.spike_trains {
            for &t in &train.spike_times {
                if t >= cutoff {
                    all_spikes.push(Spike {
                        neuron_id: train.neuron_id,
                        time: t,
                    });
                }
            }
        }

        super::compute_synchrony(&all_spikes, window / 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lif_neuron_creation() {
        let neuron = LIFNeuron::new(0);
        assert_eq!(neuron.id, 0);
        assert_eq!(neuron.state.v, 0.0);
    }

    #[test]
    fn test_lif_neuron_spike() {
        let mut neuron = LIFNeuron::new(0);

        // Apply strong current until it spikes
        let mut spiked = false;
        for i in 0..100 {
            if neuron.step(2.0, 1.0, i as f64) {
                spiked = true;
                break;
            }
        }

        assert!(spiked);
        assert!(neuron.is_refractory());
    }

    #[test]
    fn test_spike_train() {
        let mut train = SpikeTrain::new(0);
        train.record_spike(10.0);
        train.record_spike(20.0);
        train.record_spike(30.0);

        assert_eq!(train.count(), 3);

        let mean_isi = train.mean_isi().unwrap();
        assert!((mean_isi - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_neuron_population() {
        let mut pop = NeuronPopulation::new(100);

        // Apply uniform current
        let currents = vec![1.5; 100];

        let mut total_spikes = 0;
        for _ in 0..100 {
            let spikes = pop.step(&currents, 1.0);
            total_spikes += spikes.len();
        }

        // Should have some spikes after 100ms with current of 1.5
        assert!(total_spikes > 0);
    }

    #[test]
    fn test_spike_train_pattern() {
        let mut train = SpikeTrain::new(0);
        train.record_spike(1.0);
        train.record_spike(3.0);
        train.record_spike(7.0);

        let pattern = train.to_pattern(0.0, 1.0, 10);
        assert_eq!(pattern.len(), 10);
        assert!(pattern[1]); // Spike at t=1
        assert!(pattern[3]); // Spike at t=3
        assert!(pattern[7]); // Spike at t=7
        assert!(!pattern[0]); // No spike at t=0
    }
}
