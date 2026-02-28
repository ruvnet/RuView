//! # Time-Reversible Quantum Memory
//!
//! Because the simulator has full state access and all quantum gates are
//! unitary (and therefore invertible), we can **rewind** evolution.
//!
//! This enables counterfactual debugging: "What would this system have
//! believed if one observation was missing?"
//!
//! Most ML systems are forward-only. This is backward-capable.

use ruqu_core::error::QuantumError;
use ruqu_core::gate::Gate;
use ruqu_core::state::QuantumState;
use ruqu_core::types::Complex;

// ---------------------------------------------------------------------------
// Gate inversion
// ---------------------------------------------------------------------------

/// Compute the inverse of a unitary gate.
///
/// Self-inverse gates (X, Y, Z, H, CNOT, CZ, SWAP) return themselves.
/// Rotation gates negate their angle. S↔S†, T↔T†.
/// Non-unitary operations (Measure, Reset, Barrier) cannot be inverted.
pub fn inverse_gate(gate: &Gate) -> Result<Gate, QuantumError> {
    match gate {
        // Self-inverse
        Gate::X(q) => Ok(Gate::X(*q)),
        Gate::Y(q) => Ok(Gate::Y(*q)),
        Gate::Z(q) => Ok(Gate::Z(*q)),
        Gate::H(q) => Ok(Gate::H(*q)),
        Gate::CNOT(a, b) => Ok(Gate::CNOT(*a, *b)),
        Gate::CZ(a, b) => Ok(Gate::CZ(*a, *b)),
        Gate::SWAP(a, b) => Ok(Gate::SWAP(*a, *b)),

        // Rotation inverses: negate angle
        Gate::Rx(q, t) => Ok(Gate::Rx(*q, -*t)),
        Gate::Ry(q, t) => Ok(Gate::Ry(*q, -*t)),
        Gate::Rz(q, t) => Ok(Gate::Rz(*q, -*t)),
        Gate::Phase(q, t) => Ok(Gate::Phase(*q, -*t)),
        Gate::Rzz(a, b, t) => Ok(Gate::Rzz(*a, *b, -*t)),

        // Adjoint pairs
        Gate::S(q) => Ok(Gate::Sdg(*q)),
        Gate::Sdg(q) => Ok(Gate::S(*q)),
        Gate::T(q) => Ok(Gate::Tdg(*q)),
        Gate::Tdg(q) => Ok(Gate::T(*q)),

        // Custom unitary: conjugate transpose
        Gate::Unitary1Q(q, m) => {
            let inv = [
                [m[0][0].conj(), m[1][0].conj()],
                [m[0][1].conj(), m[1][1].conj()],
            ];
            Ok(Gate::Unitary1Q(*q, inv))
        }

        // Non-unitary: cannot invert
        Gate::Measure(_) | Gate::Reset(_) | Gate::Barrier => Err(QuantumError::CircuitError(
            "cannot invert non-unitary gate (Measure/Reset/Barrier)".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Reversible memory
// ---------------------------------------------------------------------------

/// A recorded gate with its precomputed inverse.
#[derive(Clone)]
struct GateRecord {
    gate: Gate,
    inverse: Gate,
}

/// Quantum memory that records all operations and can rewind them.
///
/// Every [`apply`] stores the gate and its inverse. [`rewind`] pops the
/// last n gates and applies their inverses, restoring an earlier state.
/// [`counterfactual`] replays history with one step omitted.
pub struct ReversibleMemory {
    state: QuantumState,
    history: Vec<GateRecord>,
    initial_amps: Vec<Complex>,
    num_qubits: u32,
}

/// Result of a counterfactual analysis.
#[derive(Debug)]
pub struct CounterfactualResult {
    /// Probabilities without the removed step.
    pub counterfactual_probs: Vec<f64>,
    /// Probabilities with the step included (original).
    pub original_probs: Vec<f64>,
    /// L2 divergence between the two distributions.
    pub divergence: f64,
    /// Which step was removed.
    pub removed_step: usize,
}

/// Sensitivity of each step to perturbation.
#[derive(Debug)]
pub struct SensitivityResult {
    /// For each step: 1 − fidelity(perturbed, original).
    pub sensitivities: Vec<f64>,
    /// Index of the most sensitive step.
    pub most_sensitive: usize,
    /// Index of the least sensitive step.
    pub least_sensitive: usize,
}

impl ReversibleMemory {
    /// Create a new reversible memory with `num_qubits` qubits in |0…0⟩.
    pub fn new(num_qubits: u32) -> Result<Self, QuantumError> {
        let state = QuantumState::new(num_qubits)?;
        let initial_amps = state.state_vector().to_vec();
        Ok(Self {
            state,
            history: Vec::new(),
            initial_amps,
            num_qubits,
        })
    }

    /// Create with a deterministic seed.
    pub fn new_with_seed(num_qubits: u32, seed: u64) -> Result<Self, QuantumError> {
        let state = QuantumState::new_with_seed(num_qubits, seed)?;
        let initial_amps = state.state_vector().to_vec();
        Ok(Self {
            state,
            history: Vec::new(),
            initial_amps,
            num_qubits,
        })
    }

    /// Apply a gate and record it. Non-unitary gates are rejected.
    pub fn apply(&mut self, gate: Gate) -> Result<(), QuantumError> {
        let inv = inverse_gate(&gate)?;
        self.state.apply_gate(&gate)?;
        self.history.push(GateRecord { gate, inverse: inv });
        Ok(())
    }

    /// Rewind the last `steps` operations by applying their inverses.
    /// Returns how many were actually rewound.
    pub fn rewind(&mut self, steps: usize) -> Result<usize, QuantumError> {
        let actual = steps.min(self.history.len());
        for _ in 0..actual {
            let record = self.history.pop().unwrap();
            self.state.apply_gate(&record.inverse)?;
        }
        Ok(actual)
    }

    /// Counterfactual: what would the final state be if step `remove_index`
    /// never happened?
    ///
    /// Replays the full history from the initial state, skipping the
    /// specified step, then compares with the original outcome.
    pub fn counterfactual(
        &self,
        remove_index: usize,
    ) -> Result<CounterfactualResult, QuantumError> {
        if remove_index >= self.history.len() {
            return Err(QuantumError::CircuitError(format!(
                "step {} out of range (history has {} steps)",
                remove_index,
                self.history.len()
            )));
        }

        // Replay without the removed step
        let mut cf_state =
            QuantumState::from_amplitudes(self.initial_amps.clone(), self.num_qubits)?;
        for (i, record) in self.history.iter().enumerate() {
            if i != remove_index {
                cf_state.apply_gate(&record.gate)?;
            }
        }

        let cf_probs = cf_state.probabilities();
        let orig_probs = self.state.probabilities();

        // L2 divergence
        let divergence: f64 = orig_probs
            .iter()
            .zip(cf_probs.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();

        Ok(CounterfactualResult {
            counterfactual_probs: cf_probs,
            original_probs: orig_probs,
            divergence,
            removed_step: remove_index,
        })
    }

    /// Sensitivity analysis: for each step, insert a small Rz perturbation
    /// after it and measure how much the final state diverges.
    ///
    /// Sensitivity = 1 − fidelity(perturbed_final, original_final).
    pub fn sensitivity_analysis(
        &self,
        perturbation_angle: f64,
    ) -> Result<SensitivityResult, QuantumError> {
        if self.history.is_empty() {
            return Ok(SensitivityResult {
                sensitivities: vec![],
                most_sensitive: 0,
                least_sensitive: 0,
            });
        }

        let mut sensitivities = Vec::with_capacity(self.history.len());

        for perturb_idx in 0..self.history.len() {
            let mut perturbed =
                QuantumState::from_amplitudes(self.initial_amps.clone(), self.num_qubits)?;

            for (i, record) in self.history.iter().enumerate() {
                perturbed.apply_gate(&record.gate)?;
                if i == perturb_idx {
                    let q = record.gate.qubits().first().copied().unwrap_or(0);
                    perturbed.apply_gate(&Gate::Rz(q, perturbation_angle))?;
                }
            }

            let fid = self.state.fidelity(&perturbed);
            sensitivities.push(1.0 - fid);
        }

        let most_sensitive = sensitivities
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let least_sensitive = sensitivities
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        Ok(SensitivityResult {
            sensitivities,
            most_sensitive,
            least_sensitive,
        })
    }

    /// Current state vector.
    pub fn state_vector(&self) -> &[Complex] {
        self.state.state_vector()
    }

    /// Current measurement probabilities.
    pub fn probabilities(&self) -> Vec<f64> {
        self.state.probabilities()
    }

    /// Number of recorded operations.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Number of qubits.
    pub fn num_qubits(&self) -> u32 {
        self.num_qubits
    }
}
