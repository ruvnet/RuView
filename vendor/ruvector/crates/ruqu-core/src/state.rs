//! Quantum state-vector simulator
//!
//! The core simulation engine: a dense vector of 2^n complex amplitudes with
//! gate application, measurement, collapse, expectation values, and fidelity.

use crate::error::{QuantumError, Result};
use crate::gate::Gate;
use crate::types::*;

use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;

/// Maximum number of qubits supported on this platform.
pub const MAX_QUBITS: u32 = 32;

/// Quantum state represented as a state vector of 2^n complex amplitudes.
pub struct QuantumState {
    amplitudes: Vec<Complex>,
    num_qubits: u32,
    rng: StdRng,
    measurement_record: Vec<MeasurementOutcome>,
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl QuantumState {
    /// Create the |00...0> state for `num_qubits` qubits.
    pub fn new(num_qubits: u32) -> Result<Self> {
        if num_qubits == 0 {
            return Err(QuantumError::CircuitError(
                "cannot create quantum state with 0 qubits".into(),
            ));
        }
        if num_qubits > MAX_QUBITS {
            return Err(QuantumError::QubitLimitExceeded {
                requested: num_qubits,
                maximum: MAX_QUBITS,
            });
        }
        let n = 1usize << num_qubits;
        let mut amplitudes = vec![Complex::ZERO; n];
        amplitudes[0] = Complex::ONE;
        Ok(Self {
            amplitudes,
            num_qubits,
            rng: StdRng::from_entropy(),
            measurement_record: Vec::new(),
        })
    }

    /// Create the |00...0> state with a deterministic seed for reproducibility.
    pub fn new_with_seed(num_qubits: u32, seed: u64) -> Result<Self> {
        if num_qubits > MAX_QUBITS {
            return Err(QuantumError::QubitLimitExceeded {
                requested: num_qubits,
                maximum: MAX_QUBITS,
            });
        }
        let n = 1usize << num_qubits;
        let mut amplitudes = vec![Complex::ZERO; n];
        amplitudes[0] = Complex::ONE;
        Ok(Self {
            amplitudes,
            num_qubits,
            rng: StdRng::seed_from_u64(seed),
            measurement_record: Vec::new(),
        })
    }

    /// Construct a state from an explicit amplitude vector.
    ///
    /// Validates that `amps.len() == 2^num_qubits`.
    pub fn from_amplitudes(amps: Vec<Complex>, num_qubits: u32) -> Result<Self> {
        if num_qubits > MAX_QUBITS {
            return Err(QuantumError::QubitLimitExceeded {
                requested: num_qubits,
                maximum: MAX_QUBITS,
            });
        }
        let expected = 1usize << num_qubits;
        if amps.len() != expected {
            return Err(QuantumError::InvalidStateVector {
                length: amps.len(),
                num_qubits,
            });
        }
        Ok(Self {
            amplitudes: amps,
            num_qubits,
            rng: StdRng::from_entropy(),
            measurement_record: Vec::new(),
        })
    }

    // -------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------

    pub fn num_qubits(&self) -> u32 {
        self.num_qubits
    }

    pub fn num_amplitudes(&self) -> usize {
        self.amplitudes.len()
    }

    pub fn state_vector(&self) -> &[Complex] {
        &self.amplitudes
    }

    /// Get mutable access to the raw amplitude array.
    ///
    /// # Safety
    /// Caller must maintain normalisation after mutation.
    pub fn amplitudes_mut(&mut self) -> &mut [Complex] {
        &mut self.amplitudes
    }

    /// |amplitude|^2 for each basis state.
    pub fn probabilities(&self) -> Vec<f64> {
        self.amplitudes.iter().map(|a| a.norm_sq()).collect()
    }

    /// Probability that `qubit` is in state |1>.
    pub fn probability_of_qubit(&self, qubit: QubitIndex) -> f64 {
        let qubit_bit = 1usize << qubit;
        let mut p1 = 0.0;
        for (i, amp) in self.amplitudes.iter().enumerate() {
            if i & qubit_bit != 0 {
                p1 += amp.norm_sq();
            }
        }
        p1
    }

    pub fn measurement_record(&self) -> &[MeasurementOutcome] {
        &self.measurement_record
    }

    /// Estimated memory (in bytes) needed for a state of `num_qubits` qubits.
    pub fn estimate_memory(num_qubits: u32) -> usize {
        (1usize << num_qubits) * std::mem::size_of::<Complex>()
    }

    /// Provide mutable access to the internal RNG (used by noise model).
    pub(crate) fn rng_mut(&mut self) -> &mut StdRng {
        &mut self.rng
    }

    // -------------------------------------------------------------------
    // Gate dispatch
    // -------------------------------------------------------------------

    /// Apply a gate to the state, returning any measurement outcomes.
    pub fn apply_gate(&mut self, gate: &Gate) -> Result<Vec<MeasurementOutcome>> {
        // Validate qubit indices
        for &q in gate.qubits().iter() {
            self.validate_qubit(q)?;
        }

        match gate {
            Gate::Barrier => Ok(vec![]),

            Gate::Measure(q) => {
                let outcome = self.measure(*q)?;
                Ok(vec![outcome])
            }

            Gate::Reset(q) => {
                self.reset_qubit(*q)?;
                Ok(vec![])
            }

            // Two-qubit gates
            Gate::CNOT(q1, q2) | Gate::CZ(q1, q2) | Gate::SWAP(q1, q2) | Gate::Rzz(q1, q2, _) => {
                if q1 == q2 {
                    return Err(QuantumError::CircuitError(format!(
                        "two-qubit gate requires distinct qubits, got {} and {}",
                        q1, q2
                    )));
                }
                let matrix = gate.matrix_2q().unwrap();
                self.apply_two_qubit_gate(*q1, *q2, &matrix);
                Ok(vec![])
            }

            // Everything else must be a single-qubit unitary
            other => {
                if let Some(matrix) = other.matrix_1q() {
                    let q = other.qubits()[0];
                    self.apply_single_qubit_gate(q, &matrix);
                    Ok(vec![])
                } else {
                    Err(QuantumError::CircuitError(format!(
                        "unsupported gate: {:?}",
                        other
                    )))
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Single-qubit gate kernel
    // -------------------------------------------------------------------

    /// Apply a 2x2 unitary matrix to the given qubit.
    ///
    /// For each pair of amplitudes where the qubit bit is 0 (index `i`)
    /// versus 1 (index `j = i + step`), we apply the matrix transformation.
    pub fn apply_single_qubit_gate(&mut self, qubit: QubitIndex, matrix: &[[Complex; 2]; 2]) {
        let step = 1usize << qubit;
        let n = self.amplitudes.len();

        let mut block_start = 0;
        while block_start < n {
            for i in block_start..block_start + step {
                let j = i + step;
                let a = self.amplitudes[i]; // qubit = 0
                let b = self.amplitudes[j]; // qubit = 1
                self.amplitudes[i] = matrix[0][0] * a + matrix[0][1] * b;
                self.amplitudes[j] = matrix[1][0] * a + matrix[1][1] * b;
            }
            block_start += step << 1;
        }
    }

    // -------------------------------------------------------------------
    // Two-qubit gate kernel
    // -------------------------------------------------------------------

    /// Apply a 4x4 unitary matrix to qubits `q1` and `q2`.
    ///
    /// Matrix row/column index = q1_bit * 2 + q2_bit.
    pub fn apply_two_qubit_gate(
        &mut self,
        q1: QubitIndex,
        q2: QubitIndex,
        matrix: &[[Complex; 4]; 4],
    ) {
        let q1_bit = 1usize << q1;
        let q2_bit = 1usize << q2;
        let n = self.amplitudes.len();

        for base in 0..n {
            // Process each group of 4 amplitudes exactly once: when both
            // target bits in the index are zero.
            if base & q1_bit != 0 || base & q2_bit != 0 {
                continue;
            }

            let idxs = [
                base,                   // q1=0, q2=0
                base | q2_bit,          // q1=0, q2=1
                base | q1_bit,          // q1=1, q2=0
                base | q1_bit | q2_bit, // q1=1, q2=1
            ];

            let vals = [
                self.amplitudes[idxs[0]],
                self.amplitudes[idxs[1]],
                self.amplitudes[idxs[2]],
                self.amplitudes[idxs[3]],
            ];

            for r in 0..4 {
                self.amplitudes[idxs[r]] = matrix[r][0] * vals[0]
                    + matrix[r][1] * vals[1]
                    + matrix[r][2] * vals[2]
                    + matrix[r][3] * vals[3];
            }
        }
    }

    // -------------------------------------------------------------------
    // Measurement
    // -------------------------------------------------------------------

    /// Measure a single qubit projectively.
    ///
    /// 1. Compute P(qubit = 0).
    /// 2. Sample the outcome from the distribution.
    /// 3. Collapse the state vector (zero out the other branch).
    /// 4. Renormalise.
    pub fn measure(&mut self, qubit: QubitIndex) -> Result<MeasurementOutcome> {
        self.validate_qubit(qubit)?;

        let qubit_bit = 1usize << qubit;
        let n = self.amplitudes.len();

        // Probability of measuring |0>
        let mut p0: f64 = 0.0;
        for i in 0..n {
            if i & qubit_bit == 0 {
                p0 += self.amplitudes[i].norm_sq();
            }
        }

        let random: f64 = self.rng.gen();
        let result = random >= p0; // true  => measured |1>
        let prob = if result { 1.0 - p0 } else { p0 };

        // Guard against division by zero (degenerate state).
        let norm_factor = if prob > 0.0 { 1.0 / prob.sqrt() } else { 0.0 };

        // Collapse + renormalise
        for i in 0..n {
            let bit_is_one = i & qubit_bit != 0;
            if bit_is_one == result {
                self.amplitudes[i] = self.amplitudes[i] * norm_factor;
            } else {
                self.amplitudes[i] = Complex::ZERO;
            }
        }

        let outcome = MeasurementOutcome {
            qubit,
            result,
            probability: prob,
        };
        self.measurement_record.push(outcome.clone());
        Ok(outcome)
    }

    /// Measure all qubits sequentially (qubit 0 first).
    pub fn measure_all(&mut self) -> Result<Vec<MeasurementOutcome>> {
        let mut outcomes = Vec::with_capacity(self.num_qubits as usize);
        for q in 0..self.num_qubits {
            outcomes.push(self.measure(q)?);
        }
        Ok(outcomes)
    }

    // -------------------------------------------------------------------
    // Reset
    // -------------------------------------------------------------------

    /// Reset a qubit to |0>.
    ///
    /// Implemented as "measure, then flip if result was |1>".
    pub fn reset_qubit(&mut self, qubit: QubitIndex) -> Result<()> {
        let outcome = self.measure(qubit)?;
        if outcome.result {
            // Qubit collapsed to |1>; apply X to bring it back to |0>.
            let x_matrix = Gate::X(qubit).matrix_1q().unwrap();
            self.apply_single_qubit_gate(qubit, &x_matrix);
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Expectation values
    // -------------------------------------------------------------------

    /// Compute <psi| P |psi> for a Pauli string P.
    ///
    /// For each basis state |i>, we compute P|i> = phase * |j>, then
    /// accumulate conj(amp[j]) * phase * amp[i].
    pub fn expectation_value(&self, pauli: &PauliString) -> f64 {
        let n = self.amplitudes.len();
        let mut result = Complex::ZERO;

        for i in 0..n {
            let mut j = i;
            let mut phase = Complex::ONE;

            for &(qubit, op) in &pauli.ops {
                let bit = (i >> qubit) & 1;
                match op {
                    PauliOp::I => {}
                    PauliOp::X => {
                        j ^= 1usize << qubit;
                    }
                    PauliOp::Y => {
                        j ^= 1usize << qubit;
                        // Y|0> = i|1>,  Y|1> = -i|0>
                        if bit == 0 {
                            phase = phase * Complex::I;
                        } else {
                            phase = phase * Complex::new(0.0, -1.0);
                        }
                    }
                    PauliOp::Z => {
                        if bit == 1 {
                            phase = -phase;
                        }
                    }
                }
            }

            // <j| (phase |i>) = conj(amp[j]) * phase * amp[i]
            result += self.amplitudes[j].conj() * phase * self.amplitudes[i];
        }

        // For a Hermitian observable the result is real (up to numerical noise).
        result.re
    }

    /// Compute <psi| H |psi> for a Hamiltonian H = sum_k c_k P_k.
    pub fn expectation_hamiltonian(&self, h: &Hamiltonian) -> f64 {
        h.terms
            .iter()
            .map(|(coeff, ps)| coeff * self.expectation_value(ps))
            .sum()
    }

    // -------------------------------------------------------------------
    // Normalisation & fidelity
    // -------------------------------------------------------------------

    /// Renormalise the state vector so that sum |a_i|^2 = 1.
    pub fn normalize(&mut self) {
        let norm_sq: f64 = self.amplitudes.iter().map(|a| a.norm_sq()).sum();
        if norm_sq > 0.0 {
            let inv_norm = 1.0 / norm_sq.sqrt();
            for a in self.amplitudes.iter_mut() {
                *a = *a * inv_norm;
            }
        }
    }

    /// State fidelity: |<self|other>|^2.
    pub fn fidelity(&self, other: &QuantumState) -> f64 {
        if self.num_qubits != other.num_qubits {
            return 0.0;
        }
        let mut inner = Complex::ZERO;
        for (a, b) in self.amplitudes.iter().zip(other.amplitudes.iter()) {
            inner += a.conj() * *b;
        }
        inner.norm_sq()
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    fn validate_qubit(&self, qubit: QubitIndex) -> Result<()> {
        if qubit >= self.num_qubits {
            return Err(QuantumError::InvalidQubitIndex {
                index: qubit,
                num_qubits: self.num_qubits,
            });
        }
        Ok(())
    }
}
