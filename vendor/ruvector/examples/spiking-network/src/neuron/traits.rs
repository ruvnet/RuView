//! Trait definitions for spiking neurons.

use serde::{Deserialize, Serialize};

/// State of a spiking neuron.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NeuronState {
    /// Membrane potential (mV)
    pub membrane_potential: f32,
    /// Time since last spike (ms), None if never spiked
    pub time_since_spike: Option<f32>,
    /// Whether the neuron is currently in refractory period
    pub is_refractory: bool,
    /// Accumulated input current for this timestep
    pub input_current: f32,
}

impl Default for NeuronState {
    fn default() -> Self {
        Self {
            membrane_potential: -65.0, // Resting potential
            time_since_spike: None,
            is_refractory: false,
            input_current: 0.0,
        }
    }
}

/// Parameters that define a spiking neuron's behavior.
pub trait NeuronParams: Clone + Send + Sync {
    /// Get the spike threshold voltage
    fn threshold(&self) -> f32;

    /// Get the reset voltage after spike
    fn reset_potential(&self) -> f32;

    /// Get the resting membrane potential
    fn resting_potential(&self) -> f32;

    /// Get the refractory period in milliseconds
    fn refractory_period(&self) -> f32;

    /// Validate parameters, returning error message if invalid
    fn validate(&self) -> Option<String>;
}

/// Core trait for spiking neuron models.
///
/// Implementing types should be efficient for ASIC deployment:
/// - Avoid floating-point division in `update()`
/// - Use predictable branching
/// - Minimize memory footprint
pub trait SpikingNeuron: Clone + Send + Sync {
    /// Associated parameter type
    type Params: NeuronParams;

    /// Create a new neuron with given parameters
    fn new(params: Self::Params) -> Self;

    /// Get current neuron state
    fn state(&self) -> NeuronState;

    /// Get neuron parameters
    fn params(&self) -> &Self::Params;

    /// Add input current (from incoming spikes or external input)
    ///
    /// This is a sparse operation - only called when input arrives.
    fn receive_input(&mut self, current: f32);

    /// Update neuron state for one timestep.
    ///
    /// Returns `true` if the neuron fires a spike.
    ///
    /// # Arguments
    /// * `dt` - Time step in milliseconds
    ///
    /// # ASIC Optimization
    /// This is the hot path. Implementations should:
    /// - Use only additions and multiplications
    /// - Avoid conditional branches where possible
    /// - Use fixed-point compatible operations
    fn update(&mut self, dt: f32) -> bool;

    /// Reset neuron to initial state
    fn reset(&mut self);

    /// Check if neuron is in refractory period
    fn is_refractory(&self) -> bool;

    /// Get membrane potential
    fn membrane_potential(&self) -> f32;

    /// Get time since last spike (if any)
    fn time_since_spike(&self) -> Option<f32>;
}

/// Energy estimation for ASIC cost analysis.
pub trait EnergyModel {
    /// Estimate energy cost for a single update step (picojoules)
    fn update_energy(&self) -> f32;

    /// Estimate energy cost for spike emission (picojoules)
    fn spike_energy(&self) -> f32;

    /// Estimate silicon area (square micrometers)
    fn silicon_area(&self) -> f32;
}
