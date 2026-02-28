//! Izhikevich neuron model.
//!
//! The Izhikevich model captures rich spiking dynamics with just two variables:
//! - Membrane potential (fast)
//! - Recovery variable (slow)
//!
//! This allows simulation of 20+ different firing patterns observed in cortical neurons,
//! while remaining computationally efficient.
//!
//! ## Firing Patterns
//!
//! - Regular spiking (RS) - most common excitatory
//! - Intrinsically bursting (IB) - burst then regular
//! - Chattering (CH) - fast rhythmic bursting
//! - Fast spiking (FS) - inhibitory interneurons
//! - Low-threshold spiking (LTS) - inhibitory
//!
//! ## ASIC Considerations
//!
//! - 2 multiply-accumulates per timestep
//! - 1 multiplication for recovery
//! - ~150-200 gates in digital implementation

use super::{NeuronParams, NeuronState, SpikingNeuron, EnergyModel};
use serde::{Deserialize, Serialize};

/// Pre-defined Izhikevich neuron types with biological parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IzhikevichType {
    /// Regular spiking - most common excitatory cortical neuron
    RegularSpiking,
    /// Intrinsically bursting - initial burst then regular spikes
    IntrinsicallyBursting,
    /// Chattering - fast rhythmic bursting
    Chattering,
    /// Fast spiking - typical inhibitory interneuron
    FastSpiking,
    /// Low-threshold spiking - inhibitory with rebound
    LowThresholdSpiking,
    /// Thalamo-cortical - two firing modes
    ThalamoCortical,
    /// Resonator - subthreshold oscillations
    Resonator,
}

impl IzhikevichType {
    /// Get parameters for this neuron type.
    pub fn params(self) -> IzhikevichParams {
        match self {
            Self::RegularSpiking => IzhikevichParams {
                a: 0.02,
                b: 0.2,
                c: -65.0,
                d: 8.0,
                threshold: 30.0,
                refractory: 0.0, // Implicit in dynamics
            },
            Self::IntrinsicallyBursting => IzhikevichParams {
                a: 0.02,
                b: 0.2,
                c: -55.0,
                d: 4.0,
                threshold: 30.0,
                refractory: 0.0,
            },
            Self::Chattering => IzhikevichParams {
                a: 0.02,
                b: 0.2,
                c: -50.0,
                d: 2.0,
                threshold: 30.0,
                refractory: 0.0,
            },
            Self::FastSpiking => IzhikevichParams {
                a: 0.1,
                b: 0.2,
                c: -65.0,
                d: 2.0,
                threshold: 30.0,
                refractory: 0.0,
            },
            Self::LowThresholdSpiking => IzhikevichParams {
                a: 0.02,
                b: 0.25,
                c: -65.0,
                d: 2.0,
                threshold: 30.0,
                refractory: 0.0,
            },
            Self::ThalamoCortical => IzhikevichParams {
                a: 0.02,
                b: 0.25,
                c: -65.0,
                d: 0.05,
                threshold: 30.0,
                refractory: 0.0,
            },
            Self::Resonator => IzhikevichParams {
                a: 0.1,
                b: 0.26,
                c: -65.0,
                d: 2.0,
                threshold: 30.0,
                refractory: 0.0,
            },
        }
    }
}

/// Parameters for Izhikevich neuron model.
///
/// The model equations are:
/// ```text
/// dv/dt = 0.04*v² + 5*v + 140 - u + I
/// du/dt = a*(b*v - u)
/// if v >= 30 mV: v = c, u = u + d
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IzhikevichParams {
    /// Time scale of recovery variable (smaller = slower recovery)
    pub a: f32,
    /// Sensitivity of recovery to subthreshold membrane potential
    pub b: f32,
    /// After-spike reset value of membrane potential (mV)
    pub c: f32,
    /// After-spike reset increment of recovery variable
    pub d: f32,
    /// Spike threshold (mV) - typically 30
    pub threshold: f32,
    /// Explicit refractory period (ms) - usually 0 for Izhikevich
    pub refractory: f32,
}

impl Default for IzhikevichParams {
    fn default() -> Self {
        IzhikevichType::RegularSpiking.params()
    }
}

impl NeuronParams for IzhikevichParams {
    fn threshold(&self) -> f32 {
        self.threshold
    }

    fn reset_potential(&self) -> f32 {
        self.c
    }

    fn resting_potential(&self) -> f32 {
        // Resting potential is approximately -65 to -70 mV
        -65.0
    }

    fn refractory_period(&self) -> f32 {
        self.refractory
    }

    fn validate(&self) -> Option<String> {
        if self.a <= 0.0 || self.a > 1.0 {
            return Some("a should be in (0, 1]".into());
        }
        if self.threshold < 0.0 {
            return Some("threshold should be positive".into());
        }
        None
    }
}

/// Izhikevich neuron model.
///
/// Provides rich spiking dynamics while remaining computationally efficient.
/// The two-variable model captures most qualitative behaviors of biological neurons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IzhikevichNeuron {
    /// Model parameters
    params: IzhikevichParams,
    /// Membrane potential (mV)
    v: f32,
    /// Recovery variable (dimensionless)
    u: f32,
    /// Accumulated input current
    input_current: f32,
    /// Time since last spike
    time_since_spike: Option<f32>,
    /// Refractory countdown (if explicit refractory used)
    refractory_remaining: f32,
}

impl IzhikevichNeuron {
    /// Create neuron from predefined type.
    pub fn from_type(neuron_type: IzhikevichType) -> Self {
        Self::new(neuron_type.params())
    }

    /// Create regular spiking neuron (most common).
    pub fn regular_spiking() -> Self {
        Self::from_type(IzhikevichType::RegularSpiking)
    }

    /// Create fast spiking neuron (inhibitory).
    pub fn fast_spiking() -> Self {
        Self::from_type(IzhikevichType::FastSpiking)
    }

    /// Get recovery variable.
    pub fn recovery(&self) -> f32 {
        self.u
    }
}

impl SpikingNeuron for IzhikevichNeuron {
    type Params = IzhikevichParams;

    fn new(params: IzhikevichParams) -> Self {
        // Initialize at resting state
        let v = params.c;
        let u = params.b * v;
        Self {
            params,
            v,
            u,
            input_current: 0.0,
            time_since_spike: None,
            refractory_remaining: 0.0,
        }
    }

    fn state(&self) -> NeuronState {
        NeuronState {
            membrane_potential: self.v,
            time_since_spike: self.time_since_spike,
            is_refractory: self.refractory_remaining > 0.0,
            input_current: self.input_current,
        }
    }

    fn params(&self) -> &Self::Params {
        &self.params
    }

    fn receive_input(&mut self, current: f32) {
        self.input_current += current;
    }

    fn update(&mut self, dt: f32) -> bool {
        // Update time since spike
        if let Some(ref mut t) = self.time_since_spike {
            *t += dt;
        }

        // Handle explicit refractory if set
        if self.refractory_remaining > 0.0 {
            self.refractory_remaining -= dt;
            self.input_current = 0.0;
            return false;
        }

        // Izhikevich dynamics with Euler integration
        // For numerical stability, use two half-steps for v
        let i = self.input_current;

        // Half step 1
        let dv1 = 0.04 * self.v * self.v + 5.0 * self.v + 140.0 - self.u + i;
        self.v += dv1 * dt * 0.5;

        // Half step 2
        let dv2 = 0.04 * self.v * self.v + 5.0 * self.v + 140.0 - self.u + i;
        self.v += dv2 * dt * 0.5;

        // Recovery variable
        let du = self.params.a * (self.params.b * self.v - self.u);
        self.u += du * dt;

        // Clear input
        self.input_current = 0.0;

        // Spike check
        if self.v >= self.params.threshold {
            // Spike!
            self.v = self.params.c;
            self.u += self.params.d;
            self.time_since_spike = Some(0.0);
            self.refractory_remaining = self.params.refractory;
            true
        } else {
            false
        }
    }

    fn reset(&mut self) {
        self.v = self.params.c;
        self.u = self.params.b * self.v;
        self.input_current = 0.0;
        self.time_since_spike = None;
        self.refractory_remaining = 0.0;
    }

    fn is_refractory(&self) -> bool {
        self.refractory_remaining > 0.0
    }

    fn membrane_potential(&self) -> f32 {
        self.v
    }

    fn time_since_spike(&self) -> Option<f32> {
        self.time_since_spike
    }
}

impl EnergyModel for IzhikevichNeuron {
    fn update_energy(&self) -> f32 {
        // Estimate: 3 multiplies, 6 adds, 1 comparison
        // More complex than LIF
        5.0 // picojoules
    }

    fn spike_energy(&self) -> f32 {
        10.0 // picojoules
    }

    fn silicon_area(&self) -> f32 {
        // ~150-200 gates at 28nm
        17.5 // square micrometers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_izhikevich_types() {
        // Test all predefined types can be created
        let types = [
            IzhikevichType::RegularSpiking,
            IzhikevichType::IntrinsicallyBursting,
            IzhikevichType::Chattering,
            IzhikevichType::FastSpiking,
            IzhikevichType::LowThresholdSpiking,
            IzhikevichType::ThalamoCortical,
            IzhikevichType::Resonator,
        ];

        for neuron_type in types {
            let neuron = IzhikevichNeuron::from_type(neuron_type);
            assert!(neuron.params().validate().is_none());
        }
    }

    #[test]
    fn test_regular_spiking_behavior() {
        let mut neuron = IzhikevichNeuron::regular_spiking();
        let mut spike_count = 0;

        // Inject constant current and count spikes
        for _ in 0..1000 {
            neuron.receive_input(10.0);
            if neuron.update(1.0) {
                spike_count += 1;
            }
        }

        // Should spike regularly
        assert!(spike_count > 10, "Regular spiking neuron should fire regularly");
        assert!(spike_count < 200, "Should not fire too fast");
    }

    #[test]
    fn test_fast_spiking_behavior() {
        let mut fs = IzhikevichNeuron::fast_spiking();
        let mut rs = IzhikevichNeuron::regular_spiking();

        let mut fs_spikes = 0;
        let mut rs_spikes = 0;

        // Same input to both
        for _ in 0..1000 {
            fs.receive_input(14.0);
            rs.receive_input(14.0);

            if fs.update(1.0) { fs_spikes += 1; }
            if rs.update(1.0) { rs_spikes += 1; }
        }

        // Fast spiking should fire more often
        assert!(fs_spikes > rs_spikes, "Fast spiking should fire more than regular");
    }

    #[test]
    fn test_recovery_dynamics() {
        let mut neuron = IzhikevichNeuron::regular_spiking();
        let initial_u = neuron.recovery();

        // After spike, recovery should increase
        neuron.v = 35.0; // Above threshold
        neuron.update(1.0);

        assert!(neuron.recovery() > initial_u, "Recovery should increase after spike");
    }

    #[test]
    fn test_subthreshold_dynamics() {
        let mut neuron = IzhikevichNeuron::regular_spiking();

        // Weak input should not cause immediate spike
        neuron.receive_input(2.0);
        assert!(!neuron.update(1.0));

        // Voltage should rise but not spike
        assert!(neuron.membrane_potential() > neuron.params.c);
    }
}
