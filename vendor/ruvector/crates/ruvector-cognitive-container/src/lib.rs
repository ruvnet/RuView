//! Verifiable WASM cognitive container with canonical witness chains.
//!
//! This crate composes cognitive primitives (graph ingest, min-cut, spectral
//! analysis, evidence accumulation) into a sealed container that produces a
//! tamper-evident witness chain linking every epoch to its predecessor.

pub mod container;
pub mod epoch;
pub mod error;
pub mod memory;
pub mod witness;

pub use container::{
    CognitiveContainer, ComponentMask, ContainerConfig, ContainerSnapshot, Delta, TickResult,
};
pub use epoch::{ContainerEpochBudget, EpochController, Phase};
pub use error::{ContainerError, Result};
pub use memory::{Arena, MemoryConfig, MemorySlab};
pub use witness::{CoherenceDecision, ContainerWitnessReceipt, VerificationResult, WitnessChain};
