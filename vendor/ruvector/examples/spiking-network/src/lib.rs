//! # Spiking Neural Network Library
//!
//! Event-driven spiking neural network implementation optimized for ASIC deployment.
//!
//! ## Philosophy
//!
//! Spiking neural networks do not compute in the traditional sense. They fire only when
//! something meaningful happens. Everything is event-driven. This single shift changes
//! the entire energy and timing model of your ASIC.
//!
//! A conventional network evaluates every neuron every cycle. It burns power on
//! multiplications even when nothing is changing. A spiking model skips all of that.
//! Neurons stay silent until a threshold is crossed. You only compute on change.
//!
//! ## Architecture Benefits
//!
//! - **Sparse computation**: Only active neurons consume resources
//! - **Event-driven**: No wasted cycles on unchanged state
//! - **Local connectivity**: Minimizes routing complexity
//! - **Tiny events**: Each spike is just a few bits
//! - **Microsecond latency**: Local lookups instead of matrix multiplies
//!
//! ## Usage
//!
//! ```rust,ignore
//! use spiking_network::{
//!     neuron::{LIFNeuron, NeuronParams},
//!     network::SpikingNetwork,
//!     encoding::SpikeEncoder,
//! };
//!
//! // Create a network with 1000 neurons
//! let mut network = SpikingNetwork::new(1000);
//!
//! // Encode input as sparse spikes
//! let spikes = SpikeEncoder::rate_encode(&input_data, 0.1);
//!
//! // Process - only fires on meaningful events
//! let output = network.process(&spikes);
//! ```

#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod encoding;
pub mod error;
pub mod learning;
pub mod network;
pub mod neuron;
pub mod router;

// Re-exports for convenience
pub use encoding::{SpikeEncoder, SpikeEvent, SpikeTrain};
pub use error::{Result, SpikingError};
pub use learning::{STDPConfig, STDPLearning};
pub use network::{NetworkConfig, NetworkStats, SpikingNetwork};
pub use neuron::{IzhikevichNeuron, LIFNeuron, NeuronParams, SpikingNeuron};
pub use router::{AsicRouter, RouterConfig, SpikePacket};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
