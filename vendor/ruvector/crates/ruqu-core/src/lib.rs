//! # ruqu-core -- Quantum Execution Intelligence Engine
//!
//! Pure Rust quantum simulation and execution engine for the ruVector stack.
//! Supports state-vector (up to 32 qubits), stabilizer (millions), Clifford+T
//! (moderate T-count), and tensor network backends with automatic routing,
//! noise modeling, error mitigation, and cryptographic witness logging.
//!
//! ## Quick Start
//!
//! ```
//! use ruqu_core::prelude::*;
//!
//! // Create a Bell state |00> + |11> (unnormalised notation)
//! let mut circuit = QuantumCircuit::new(2);
//! circuit.h(0).cnot(0, 1);
//! let result = Simulator::run(&circuit).unwrap();
//! let probs = result.state.probabilities();
//! // probs ~= [0.5, 0.0, 0.0, 0.5]
//! ```

// -- Core simulation layer --
pub mod backend;
pub mod circuit;
pub mod circuit_analyzer;
pub mod error;
pub mod gate;
pub mod mixed_precision;
pub mod optimizer;
pub mod simd;
pub mod simulator;
pub mod stabilizer;
pub mod state;
pub mod tensor_network;
pub mod types;

// -- Scientific instrument layer (ADR-QE-015) --
pub mod confidence;
pub mod hardware;
pub mod mitigation;
pub mod noise;
pub mod qasm;
pub mod replay;
pub mod transpiler;
pub mod verification;
pub mod witness;

// -- SOTA differentiation layer --
pub mod clifford_t;
pub mod decomposition;
pub mod pipeline;
pub mod planner;

// -- QEC control plane --
pub mod control_theory;
pub mod decoder;
pub mod qec_scheduler;
pub mod subpoly_decoder;

// -- Benchmark & proof suite --
pub mod benchmark;

/// Re-exports of the most commonly used items.
pub mod prelude {
    pub use crate::backend::BackendType;
    pub use crate::circuit::QuantumCircuit;
    pub use crate::error::{QuantumError, Result};
    pub use crate::gate::Gate;
    pub use crate::qasm::to_qasm3;
    pub use crate::simulator::{ShotResult, SimConfig, SimulationResult, Simulator};
    pub use crate::state::QuantumState;
    pub use crate::types::*;
}
