//! # ruqu-algorithms -- Quantum Algorithm Implementations
//!
//! High-level quantum algorithms built on the `ruqu-core` simulation engine:
//!
//! - **VQE** (Variational Quantum Eigensolver): Find ground-state energies of
//!   molecular Hamiltonians using a classical-quantum hybrid loop with
//!   hardware-efficient ansatz and parameter-shift gradient descent.
//!
//! - **Grover's Search**: Quadratic speedup for unstructured search over N items,
//!   using amplitude amplification with direct state-vector oracle access.
//!
//! - **QAOA** (Quantum Approximate Optimization Algorithm): Approximate solutions
//!   to combinatorial optimization problems (MaxCut) via parameterized
//!   phase-separation and mixing layers.
//!
//! - **Surface Code**: Distance-3 surface code error correction simulation with
//!   stabilizer measurement cycles, noise injection, and syndrome decoding.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use ruqu_algorithms::{VqeConfig, run_vqe, vqe::h2_hamiltonian};
//!
//! let config = VqeConfig {
//!     hamiltonian: h2_hamiltonian(),
//!     num_qubits: 2,
//!     ansatz_depth: 2,
//!     max_iterations: 100,
//!     convergence_threshold: 1e-6,
//!     learning_rate: 0.1,
//!     seed: Some(42),
//! };
//! let result = run_vqe(&config).expect("VQE failed");
//! println!("Ground state energy: {:.6}", result.optimal_energy);
//! ```

pub mod grover;
pub mod qaoa;
pub mod surface_code;
pub mod vqe;

pub use grover::{run_grover, GroverConfig, GroverResult};
pub use qaoa::{run_qaoa, Graph, QaoaConfig, QaoaResult};
pub use surface_code::{run_surface_code, SurfaceCodeConfig, SurfaceCodeResult};
pub use vqe::{run_vqe, VqeConfig, VqeResult};
