//! Spiking neuron models.
//!
//! This module provides biologically-inspired neuron models optimized for
//! event-driven computation. Neurons stay silent until a threshold is crossed,
//! eliminating wasted cycles on unchanged state.
//!
//! ## Available Models
//!
//! - **LIF (Leaky Integrate-and-Fire)**: Simple, efficient, ASIC-friendly
//! - **Izhikevich**: Rich dynamics, biologically plausible spiking patterns
//!
//! ## ASIC Considerations
//!
//! These models are designed for minimal silicon cost:
//! - Fixed-point compatible arithmetic
//! - No division operations in hot paths
//! - Predictable memory access patterns
//! - Branch-friendly state machines

mod lif;
mod izhikevich;
mod traits;

pub use lif::{LIFNeuron, LIFParams};
pub use izhikevich::{IzhikevichNeuron, IzhikevichParams, IzhikevichType};
pub use traits::{NeuronParams, SpikingNeuron, NeuronState};

/// Default membrane time constant (ms)
pub const DEFAULT_TAU_M: f32 = 20.0;

/// Default spike threshold (mV)
pub const DEFAULT_THRESHOLD: f32 = -50.0;

/// Default resting potential (mV)
pub const DEFAULT_RESTING: f32 = -65.0;

/// Default reset potential (mV)
pub const DEFAULT_RESET: f32 = -70.0;

/// Default refractory period (ms)
pub const DEFAULT_REFRACTORY: f32 = 2.0;
