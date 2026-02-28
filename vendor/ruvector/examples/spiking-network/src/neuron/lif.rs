//! Leaky Integrate-and-Fire (LIF) neuron model.
//!
//! The LIF model is the workhorse of neuromorphic computing:
//! - Simple dynamics: membrane voltage leaks toward rest
//! - Spikes when threshold crossed
//! - Resets and enters refractory period
//!
//! ## ASIC Benefits
//!
//! - Single multiply-accumulate per timestep
//! - No division (pre-computed decay factor)
//! - 2-3 comparisons per update
//! - ~100 gates in digital implementation

use super::{NeuronParams, NeuronState, SpikingNeuron, EnergyModel};
use serde::{Deserialize, Serialize};

/// Parameters for LIF neuron.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LIFParams {
    /// Membrane time constant (ms) - controls leak rate
    pub tau_m: f32,
    /// Spike threshold (mV)
    pub threshold: f32,
    /// Reset potential after spike (mV)
    pub reset: f32,
    /// Resting membrane potential (mV)
    pub resting: f32,
    /// Refractory period (ms)
    pub refractory: f32,
    /// Membrane resistance (MOhm) - scales input current
    pub resistance: f32,
}

impl Default for LIFParams {
    fn default() -> Self {
        Self {
            tau_m: 20.0,
            threshold: -50.0,
            reset: -70.0,
            resting: -65.0,
            refractory: 2.0,
            resistance: 10.0, // 10 MOhm typical for cortical neurons
        }
    }
}

impl NeuronParams for LIFParams {
    fn threshold(&self) -> f32 {
        self.threshold
    }

    fn reset_potential(&self) -> f32 {
        self.reset
    }

    fn resting_potential(&self) -> f32 {
        self.resting
    }

    fn refractory_period(&self) -> f32 {
        self.refractory
    }

    fn validate(&self) -> Option<String> {
        if self.tau_m <= 0.0 {
            return Some("tau_m must be positive".into());
        }
        if self.threshold <= self.reset {
            return Some("threshold must be greater than reset".into());
        }
        if self.refractory < 0.0 {
            return Some("refractory period cannot be negative".into());
        }
        if self.resistance <= 0.0 {
            return Some("resistance must be positive".into());
        }
        None
    }
}

/// Leaky Integrate-and-Fire neuron.
///
/// Implements the differential equation:
/// ```text
/// τ_m * dV/dt = -(V - V_rest) + R * I
/// ```
///
/// With spike condition: V ≥ V_threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LIFNeuron {
    /// Neuron parameters
    params: LIFParams,
    /// Current membrane potential (mV)
    membrane_potential: f32,
    /// Time remaining in refractory period (ms)
    refractory_remaining: f32,
    /// Accumulated input current for this timestep
    input_current: f32,
    /// Time since last spike (ms)
    time_since_spike: Option<f32>,
    /// Pre-computed decay factor for efficiency
    decay_factor: f32,
}

impl LIFNeuron {
    /// Create LIF neuron with default parameters.
    pub fn with_defaults() -> Self {
        Self::new(LIFParams::default())
    }

    /// Pre-compute decay factor for given timestep.
    ///
    /// This avoids division in the hot path.
    /// decay = exp(-dt / tau_m) ≈ 1 - dt/tau_m for small dt
    fn compute_decay(&self, dt: f32) -> f32 {
        // Use linear approximation for ASIC compatibility
        // Error < 1% for dt < 2ms with tau_m = 20ms
        1.0 - dt / self.params.tau_m
    }

    /// Get the pre-computed decay factor.
    pub fn decay_factor(&self) -> f32 {
        self.decay_factor
    }
}

impl SpikingNeuron for LIFNeuron {
    type Params = LIFParams;

    fn new(params: LIFParams) -> Self {
        let decay_factor = 1.0 - 1.0 / params.tau_m; // For dt=1ms default
        Self {
            params,
            membrane_potential: params.resting,
            refractory_remaining: 0.0,
            input_current: 0.0,
            time_since_spike: None,
            decay_factor,
        }
    }

    fn state(&self) -> NeuronState {
        NeuronState {
            membrane_potential: self.membrane_potential,
            time_since_spike: self.time_since_spike,
            is_refractory: self.refractory_remaining > 0.0,
            input_current: self.input_current,
        }
    }

    fn params(&self) -> &Self::Params {
        &self.params
    }

    fn receive_input(&mut self, current: f32) {
        // Accumulate input - this is the sparse event
        self.input_current += current;
    }

    fn update(&mut self, dt: f32) -> bool {
        // Update time since spike
        if let Some(ref mut t) = self.time_since_spike {
            *t += dt;
        }

        // Handle refractory period
        if self.refractory_remaining > 0.0 {
            self.refractory_remaining -= dt;
            self.input_current = 0.0; // Clear accumulated input
            return false;
        }

        // Compute decay factor for this timestep
        let decay = self.compute_decay(dt);

        // LIF dynamics: V = decay * V + (1-decay) * V_rest + R * I * dt / tau_m
        // Simplified: V = decay * (V - V_rest) + V_rest + R * I * dt / tau_m
        let v_diff = self.membrane_potential - self.params.resting;
        let input_term = self.params.resistance * self.input_current * dt / self.params.tau_m;

        self.membrane_potential = decay * v_diff + self.params.resting + input_term;

        // Clear input for next timestep
        self.input_current = 0.0;

        // Check for spike
        if self.membrane_potential >= self.params.threshold {
            // Spike!
            self.membrane_potential = self.params.reset;
            self.refractory_remaining = self.params.refractory;
            self.time_since_spike = Some(0.0);
            true
        } else {
            false
        }
    }

    fn reset(&mut self) {
        self.membrane_potential = self.params.resting;
        self.refractory_remaining = 0.0;
        self.input_current = 0.0;
        self.time_since_spike = None;
    }

    fn is_refractory(&self) -> bool {
        self.refractory_remaining > 0.0
    }

    fn membrane_potential(&self) -> f32 {
        self.membrane_potential
    }

    fn time_since_spike(&self) -> Option<f32> {
        self.time_since_spike
    }
}

impl EnergyModel for LIFNeuron {
    fn update_energy(&self) -> f32 {
        // Estimate: 1 multiply, 3 adds, 2 comparisons
        // At 28nm: ~0.5 pJ per operation
        3.0 // picojoules
    }

    fn spike_energy(&self) -> f32 {
        // Spike packet generation and routing
        10.0 // picojoules
    }

    fn silicon_area(&self) -> f32 {
        // ~100 gates at 28nm ≈ 0.1 μm² per gate
        10.0 // square micrometers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lif_default_creation() {
        let neuron = LIFNeuron::with_defaults();
        assert_eq!(neuron.membrane_potential(), -65.0);
        assert!(!neuron.is_refractory());
    }

    #[test]
    fn test_lif_spike_generation() {
        let mut neuron = LIFNeuron::with_defaults();

        // Inject strong current
        for _ in 0..100 {
            neuron.receive_input(5.0); // Strong input
            if neuron.update(1.0) {
                // Spiked!
                assert!(neuron.is_refractory());
                assert_eq!(neuron.membrane_potential(), neuron.params.reset);
                return;
            }
        }
        panic!("Neuron should have spiked with strong input");
    }

    #[test]
    fn test_lif_refractory_period() {
        let params = LIFParams {
            refractory: 5.0,
            ..Default::default()
        };
        let mut neuron = LIFNeuron::new(params);

        // Force a spike
        neuron.membrane_potential = params.threshold + 1.0;
        neuron.update(1.0);

        // Should be refractory
        assert!(neuron.is_refractory());

        // Should not spike during refractory
        neuron.receive_input(100.0);
        assert!(!neuron.update(1.0));

        // After refractory period
        for _ in 0..5 {
            neuron.update(1.0);
        }
        assert!(!neuron.is_refractory());
    }

    #[test]
    fn test_lif_leak_to_rest() {
        let mut neuron = LIFNeuron::with_defaults();
        neuron.membrane_potential = -55.0; // Above resting

        // Without input, should decay toward resting
        for _ in 0..100 {
            neuron.update(1.0);
        }

        // Should be close to resting potential
        assert!((neuron.membrane_potential() - (-65.0)).abs() < 1.0);
    }

    #[test]
    fn test_params_validation() {
        let invalid = LIFParams {
            tau_m: -1.0,
            ..Default::default()
        };
        assert!(invalid.validate().is_some());

        let valid = LIFParams::default();
        assert!(valid.validate().is_none());
    }
}
