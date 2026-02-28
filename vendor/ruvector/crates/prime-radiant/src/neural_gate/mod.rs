//! Neural Gate Integration - ruvector-nervous-system Adapter
//!
//! This module provides biologically-inspired gating using the `ruvector-nervous-system`
//! crate. It implements neural coherence gating with features from neuroscience:
//!
//! - **Dendritic coincidence detection**: Multiple evidence sources must align
//! - **Hysteresis**: Prevents rapid oscillation between states
//! - **Global workspace**: Broadcast mechanism for significant decisions
//! - **HDC encoding**: Hyperdimensional computing for witness similarity
//!
//! # Architecture
//!
//! The neural gate uses oscillatory routing (Kuramoto model) and workspace theory
//! to implement a coherence-gated decision system that:
//!
//! 1. Filters noise through dendritic coincidence detection
//! 2. Maintains stable decisions via hysteresis
//! 3. Broadcasts significant decisions to all modules
//! 4. Encodes witnesses as hypervectors for similarity search
//!
//! # Key Types
//!
//! - [`NeuralCoherenceGate`]: Main neural gating system
//! - [`NeuralDecision`]: Decision from the neural gate
//! - [`WitnessEncoding`]: HDC encoding of witness records
//! - [`NeuralGateConfig`]: Configuration for the neural gate
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::neural_gate::{NeuralCoherenceGate, NeuralGateConfig};
//!
//! // Create neural gate
//! let mut gate = NeuralCoherenceGate::new(NeuralGateConfig::default());
//!
//! // Evaluate with biologically-inspired gating
//! let decision = gate.evaluate(energy, &context);
//!
//! // Encode witness as hypervector
//! let encoding = gate.encode_witness(&witness_record);
//!
//! // Find similar past witnesses
//! let similar = gate.find_similar_witnesses(&encoding.hypervector, 0.8);
//! ```

mod config;
mod decision;
mod encoding;
mod error;
mod gate;

pub use config::{HysteresisConfig, NeuralGateConfig, OscillatorConfig, WorkspaceConfig};
pub use decision::{DecisionConfidence, DecisionTrigger, NeuralDecision};
pub use encoding::{HypervectorOps, WitnessEncoding};
pub use error::{NeuralGateError, NeuralGateResult};
pub use gate::{GateState, NeuralCoherenceGate};
