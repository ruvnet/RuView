//! Variational Quantum Eigensolver (VQE)
//!
//! Finds the ground-state energy of a Hamiltonian using a classical-quantum
//! hybrid optimization loop:
//!
//! 1. A parameterized **ansatz** circuit prepares a trial state on the quantum
//!    processor (or simulator).
//! 2. The **expectation value** of the Hamiltonian is measured for that state.
//! 3. A **classical optimizer** (gradient descent with parameter-shift rule)
//!    updates the circuit parameters to minimize the energy.
//! 4. Steps 1-3 repeat until convergence or the iteration budget is exhausted.
//!
//! The ansatz used here is "hardware-efficient": each layer applies Ry and Rz
//! rotations on every qubit, followed by a linear CNOT entangling chain.

use ruqu_core::circuit::QuantumCircuit;
use ruqu_core::simulator::{SimConfig, Simulator};
use ruqu_core::types::{Hamiltonian, PauliOp, PauliString};

// ---------------------------------------------------------------------------
// Configuration and result types
// ---------------------------------------------------------------------------

/// Configuration for a VQE run.
pub struct VqeConfig {
    /// The Hamiltonian whose ground-state energy we seek.
    pub hamiltonian: Hamiltonian,
    /// Number of qubits in the ansatz circuit.
    pub num_qubits: u32,
    /// Number of ansatz layers (depth). Each layer contributes
    /// `2 * num_qubits` parameters (Ry + Rz per qubit).
    pub ansatz_depth: u32,
    /// Maximum number of classical optimizer iterations.
    pub max_iterations: u32,
    /// Stop early when the absolute energy change between successive
    /// iterations falls below this threshold.
    pub convergence_threshold: f64,
    /// Step size for gradient descent.
    pub learning_rate: f64,
    /// Optional RNG seed for reproducible simulation.
    pub seed: Option<u64>,
}

/// Result returned by [`run_vqe`].
pub struct VqeResult {
    /// Lowest energy found during the optimization.
    pub optimal_energy: f64,
    /// Parameter vector that produced `optimal_energy`.
    pub optimal_parameters: Vec<f64>,
    /// Energy at each iteration (length = `num_iterations`).
    pub energy_history: Vec<f64>,
    /// Total number of iterations executed.
    pub num_iterations: u32,
    /// Whether the optimizer converged before exhausting `max_iterations`.
    pub converged: bool,
}

// ---------------------------------------------------------------------------
// Ansatz construction
// ---------------------------------------------------------------------------

/// Return the total number of variational parameters for the given ansatz
/// dimensions. Each layer uses `2 * num_qubits` parameters (one Ry and one
/// Rz rotation per qubit).
pub fn num_parameters(num_qubits: u32, depth: u32) -> usize {
    (2 * num_qubits as usize) * (depth as usize)
}

/// Build a hardware-efficient ansatz circuit.
///
/// Each layer consists of:
/// 1. **Rotation sub-layer**: Ry(theta) on every qubit.
/// 2. **Rotation sub-layer**: Rz(theta) on every qubit.
/// 3. **Entangling sub-layer**: Linear CNOT chain (0->1, 1->2, ..., n-2->n-1).
///
/// `params` must have exactly [`num_parameters`]`(num_qubits, depth)` entries.
///
/// # Panics
///
/// Panics if `params.len()` does not equal the expected parameter count.
pub fn build_ansatz(num_qubits: u32, depth: u32, params: &[f64]) -> QuantumCircuit {
    let expected = num_parameters(num_qubits, depth);
    assert_eq!(
        params.len(),
        expected,
        "build_ansatz: expected {} parameters, got {}",
        expected,
        params.len()
    );

    let mut circuit = QuantumCircuit::new(num_qubits);
    let mut idx = 0;

    for _layer in 0..depth {
        // Ry rotations
        for q in 0..num_qubits {
            circuit.ry(q, params[idx]);
            idx += 1;
        }
        // Rz rotations
        for q in 0..num_qubits {
            circuit.rz(q, params[idx]);
            idx += 1;
        }
        // Linear CNOT entangling chain
        for q in 0..num_qubits.saturating_sub(1) {
            circuit.cnot(q, q + 1);
        }
    }

    circuit
}

// ---------------------------------------------------------------------------
// Energy evaluation
// ---------------------------------------------------------------------------

/// Evaluate the expectation value of the Hamiltonian for a given set of
/// ansatz parameters.
///
/// Builds the ansatz, simulates it, and returns `<psi|H|psi>`.
pub fn evaluate_energy(config: &VqeConfig, params: &[f64]) -> ruqu_core::error::Result<f64> {
    let circuit = build_ansatz(config.num_qubits, config.ansatz_depth, params);
    let sim_config = SimConfig {
        seed: config.seed,
        noise: None,
        shots: None,
    };
    let result = Simulator::run_with_config(&circuit, &sim_config)?;
    Ok(result.state.expectation_hamiltonian(&config.hamiltonian))
}

// ---------------------------------------------------------------------------
// VQE optimizer
// ---------------------------------------------------------------------------

/// Run the VQE optimization loop.
///
/// Uses gradient descent with the **parameter-shift rule** to compute
/// analytical gradients. For each parameter theta_i the gradient is:
///
/// ```text
/// dE/d(theta_i) = [ E(theta_i + pi/2) - E(theta_i - pi/2) ] / 2
/// ```
///
/// This requires 2 circuit evaluations per parameter per iteration, so the
/// total cost is `O(max_iterations * 2 * num_parameters)` circuit runs.
pub fn run_vqe(config: &VqeConfig) -> ruqu_core::error::Result<VqeResult> {
    let n_params = num_parameters(config.num_qubits, config.ansatz_depth);

    // Initialize parameters with small values to break symmetry.
    let mut params = vec![0.1_f64; n_params];

    let mut energy_history: Vec<f64> = Vec::with_capacity(config.max_iterations as usize);
    let mut converged = false;

    let mut best_energy = f64::MAX;
    let mut best_params = params.clone();

    for iteration in 0..config.max_iterations {
        // ------------------------------------------------------------------
        // Forward pass: compute current energy
        // ------------------------------------------------------------------
        let energy = evaluate_energy(config, &params)?;
        energy_history.push(energy);

        if energy < best_energy {
            best_energy = energy;
            best_params = params.clone();
        }

        // ------------------------------------------------------------------
        // Convergence check (skip first iteration since we need a delta)
        // ------------------------------------------------------------------
        if iteration > 0 {
            let prev = energy_history[iteration as usize - 1];
            if (prev - energy).abs() < config.convergence_threshold {
                converged = true;
                break;
            }
        }

        // ------------------------------------------------------------------
        // Backward pass: compute gradient via parameter-shift rule
        // ------------------------------------------------------------------
        let shift = std::f64::consts::FRAC_PI_2;
        let mut gradient = vec![0.0_f64; n_params];

        for i in 0..n_params {
            let mut params_plus = params.clone();
            let mut params_minus = params.clone();
            params_plus[i] += shift;
            params_minus[i] -= shift;

            let e_plus = evaluate_energy(config, &params_plus)?;
            let e_minus = evaluate_energy(config, &params_minus)?;
            gradient[i] = (e_plus - e_minus) / 2.0;
        }

        // ------------------------------------------------------------------
        // Parameter update (gradient descent -- minimize energy)
        // ------------------------------------------------------------------
        for i in 0..n_params {
            params[i] -= config.learning_rate * gradient[i];
        }
    }

    let num_iterations = energy_history.len() as u32;
    Ok(VqeResult {
        optimal_energy: best_energy,
        optimal_parameters: best_params,
        energy_history,
        num_iterations,
        converged,
    })
}

// ---------------------------------------------------------------------------
// Hamiltonian helpers
// ---------------------------------------------------------------------------

/// Create an approximate H2 (molecular hydrogen) Hamiltonian in the STO-3G
/// basis mapped to 2 qubits via the Bravyi-Kitaev transformation.
///
/// ```text
/// H = -1.0523 II + 0.3979 IZ + -0.3979 ZI + -0.0112 ZZ + 0.1809 XX
/// ```
///
/// The exact ground-state energy of this Hamiltonian is approximately -1.137
/// Hartree (at equilibrium bond length ~0.735 angstrom).
pub fn h2_hamiltonian() -> Hamiltonian {
    Hamiltonian {
        terms: vec![
            // Identity term (constant offset)
            (-1.0523, PauliString { ops: vec![] }),
            // IZ: Pauli-Z on qubit 1
            (
                0.3979,
                PauliString {
                    ops: vec![(1, PauliOp::Z)],
                },
            ),
            // ZI: Pauli-Z on qubit 0
            (
                -0.3979,
                PauliString {
                    ops: vec![(0, PauliOp::Z)],
                },
            ),
            // ZZ: Pauli-Z on both qubits
            (
                -0.0112,
                PauliString {
                    ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
                },
            ),
            // XX: Pauli-X on both qubits
            (
                0.1809,
                PauliString {
                    ops: vec![(0, PauliOp::X), (1, PauliOp::X)],
                },
            ),
        ],
        num_qubits: 2,
    }
}

/// Create a simple single-qubit Z Hamiltonian: `H = -1.0 Z`.
///
/// The ground state is |0> with energy -1.0. Useful for smoke-testing VQE.
pub fn single_z_hamiltonian() -> Hamiltonian {
    Hamiltonian {
        terms: vec![(
            -1.0,
            PauliString {
                ops: vec![(0, PauliOp::Z)],
            },
        )],
        num_qubits: 1,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_parameters() {
        assert_eq!(num_parameters(2, 1), 4);
        assert_eq!(num_parameters(4, 3), 24);
        assert_eq!(num_parameters(1, 5), 10);
    }

    #[test]
    fn test_build_ansatz_gate_count() {
        let n = 3;
        let depth = 2;
        let params = vec![0.0; num_parameters(n, depth)];
        let circuit = build_ansatz(n, depth, &params);
        assert_eq!(circuit.num_qubits(), n);
        // Each layer: 3 Ry + 3 Rz + 2 CNOT = 8 gates, times 2 layers = 16
        assert_eq!(circuit.gates().len(), 16);
    }

    #[test]
    #[should_panic(expected = "expected 4 parameters")]
    fn test_build_ansatz_wrong_param_count() {
        build_ansatz(2, 1, &[0.0; 3]);
    }

    #[test]
    fn test_h2_hamiltonian_structure() {
        let h = h2_hamiltonian();
        assert_eq!(h.num_qubits, 2);
        assert_eq!(h.terms.len(), 5);
    }

    #[test]
    fn test_single_z_hamiltonian() {
        let h = single_z_hamiltonian();
        assert_eq!(h.num_qubits, 1);
        assert_eq!(h.terms.len(), 1);
    }
}
