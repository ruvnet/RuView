//! # Quantum/Algebraic Topology Module
//!
//! This module provides quantum computing primitives and algebraic topology tools
//! for the Prime-Radiant coherence engine. It enables:
//!
//! - **Quantum State Simulation**: Pure states, density matrices, and quantum channels
//! - **Topological Invariants**: Betti numbers, Euler characteristic, homology groups
//! - **Persistent Homology**: Track topological features across filtration scales
//! - **Topological Quantum Encoding**: Structure-preserving quantum encodings
//! - **Coherence Integration**: Quantum and topological measures of structural coherence
//!
//! ## Mathematical Foundation
//!
//! The module bridges quantum mechanics and algebraic topology to provide
//! structure-preserving coherence measures:
//!
//! - **Quantum Fidelity**: F(ρ, σ) = (Tr√(√ρ σ √ρ))² measures state similarity
//! - **Topological Energy**: Uses Betti numbers and persistence to quantify structure
//! - **Sheaf Cohomology**: Connects topological invariants to coherence residuals
//!
//! ## Design Philosophy
//!
//! This is a **classical simulation** of quantum concepts, designed for:
//! 1. Numerical stability (using `num-complex` for complex arithmetic)
//! 2. No external quantum hardware requirements
//! 3. Integration with Prime-Radiant's sheaf-theoretic framework
//! 4. WASM compatibility (pure Rust, no system dependencies)

#![allow(dead_code)]

pub mod complex_matrix;
pub mod quantum_state;
pub mod density_matrix;
pub mod quantum_channel;
pub mod topological_invariant;
pub mod persistent_homology;
pub mod simplicial_complex;
pub mod topological_code;
pub mod coherence_integration;

// Re-exports for convenient access
pub use complex_matrix::{ComplexMatrix, ComplexVector};
pub use quantum_state::{QuantumState, QuantumBasis, Qubit};
pub use density_matrix::{DensityMatrix, MixedState};
pub use quantum_channel::{QuantumChannel, KrausOperator, PauliOperator, PauliType};
pub use topological_invariant::{
    TopologicalInvariant, HomologyGroup, CohomologyGroup, Cocycle,
};
pub use persistent_homology::{
    PersistenceDiagram, BirthDeathPair, PersistentHomologyComputer,
};
pub use simplicial_complex::{
    Simplex, SimplicialComplex, SparseMatrix, BoundaryMatrix,
};
pub use topological_code::{
    TopologicalCode, StabilizerCode, GraphState, StructurePreservingEncoder,
    SolverBackedOperator,
};
pub use coherence_integration::{
    TopologicalEnergy, TopologicalCoherenceAnalyzer, QuantumCoherenceMetric,
};

/// Error type for quantum/topology operations
#[derive(Debug, Clone, PartialEq)]
pub enum QuantumTopologyError {
    /// Dimension mismatch between operands
    DimensionMismatch { expected: usize, got: usize },
    /// Invalid quantum state (not normalized)
    InvalidQuantumState(String),
    /// Invalid density matrix (not positive semidefinite or trace != 1)
    InvalidDensityMatrix(String),
    /// Invalid quantum channel (Kraus operators don't sum to identity)
    InvalidQuantumChannel(String),
    /// Singular matrix encountered
    SingularMatrix,
    /// Invalid simplex specification
    InvalidSimplex(String),
    /// Invalid topological code
    InvalidTopologicalCode(String),
    /// Computation failed to converge
    ConvergenceFailure { iterations: usize, tolerance: f64 },
    /// General numerical error
    NumericalError(String),
}

impl std::fmt::Display for QuantumTopologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, got } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, got)
            }
            Self::InvalidQuantumState(msg) => write!(f, "Invalid quantum state: {}", msg),
            Self::InvalidDensityMatrix(msg) => write!(f, "Invalid density matrix: {}", msg),
            Self::InvalidQuantumChannel(msg) => write!(f, "Invalid quantum channel: {}", msg),
            Self::SingularMatrix => write!(f, "Singular matrix encountered"),
            Self::InvalidSimplex(msg) => write!(f, "Invalid simplex: {}", msg),
            Self::InvalidTopologicalCode(msg) => write!(f, "Invalid topological code: {}", msg),
            Self::ConvergenceFailure { iterations, tolerance } => {
                write!(f, "Failed to converge after {} iterations (tol={})", iterations, tolerance)
            }
            Self::NumericalError(msg) => write!(f, "Numerical error: {}", msg),
        }
    }
}

impl std::error::Error for QuantumTopologyError {}

/// Result type for quantum/topology operations
pub type Result<T> = std::result::Result<T, QuantumTopologyError>;

/// Constants used throughout the module
pub mod constants {
    /// Numerical tolerance for floating point comparisons
    pub const EPSILON: f64 = 1e-10;

    /// Maximum iterations for iterative algorithms
    pub const MAX_ITERATIONS: usize = 1000;

    /// Default convergence tolerance
    pub const DEFAULT_TOLERANCE: f64 = 1e-8;

    /// Pi constant
    pub const PI: f64 = std::f64::consts::PI;

    /// Euler's number
    pub const E: f64 = std::f64::consts::E;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = QuantumTopologyError::DimensionMismatch { expected: 4, got: 8 };
        assert!(err.to_string().contains("expected 4"));
        assert!(err.to_string().contains("got 8"));
    }

    #[test]
    fn test_constants() {
        assert!(constants::EPSILON > 0.0);
        assert!(constants::MAX_ITERATIONS > 0);
        assert!((constants::PI - std::f64::consts::PI).abs() < 1e-15);
    }
}
