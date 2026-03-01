//! Error types for the ruQu quantum simulation engine

use crate::types::QubitIndex;
use thiserror::Error;

/// Errors that can occur during quantum simulation
#[derive(Error, Debug)]
pub enum QuantumError {
    #[error("qubit limit exceeded: requested {requested}, maximum {maximum}")]
    QubitLimitExceeded { requested: u32, maximum: u32 },

    #[error("invalid qubit index {index} for {num_qubits}-qubit system")]
    InvalidQubitIndex { index: QubitIndex, num_qubits: u32 },

    #[error("memory allocation failed: need {required_bytes} bytes")]
    MemoryAllocationFailed { required_bytes: usize },

    #[error("invalid state vector: length {length} does not match 2^{num_qubits}")]
    InvalidStateVector { length: usize, num_qubits: u32 },

    #[error("circuit error: {0}")]
    CircuitError(String),
}

/// Convenience alias used throughout the crate
pub type Result<T> = std::result::Result<T, QuantumError>;
