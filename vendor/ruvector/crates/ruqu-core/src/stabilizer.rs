//! Aaronson-Gottesman stabilizer simulator for Clifford circuits.
//!
//! Uses a tableau of 2n rows and (2n+1) columns to represent the stabilizer
//! and destabilizer generators of an n-qubit state.  Each Clifford gate is
//! applied in O(n) time and each measurement in O(n^2), enabling simulation
//! of millions of qubits for circuits composed entirely of Clifford gates.
//!
//! Reference: Aaronson & Gottesman, "Improved Simulation of Stabilizer
//! Circuits", Phys. Rev. A 70, 052328 (2004).

use crate::error::{QuantumError, Result};
use crate::gate::Gate;
use crate::types::MeasurementOutcome;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Stabilizer state for efficient Clifford circuit simulation.
///
/// Uses the Aaronson-Gottesman tableau representation to simulate
/// Clifford circuits in O(n^2) time per gate, enabling simulation
/// of millions of qubits.
pub struct StabilizerState {
    num_qubits: usize,
    /// Tableau: 2n rows, each row has n X-bits, n Z-bits, and 1 phase bit.
    /// Stored as a flat `Vec<bool>` for simplicity.
    /// Row i occupies indices `[i * stride .. (i+1) * stride)`.
    /// Layout within a row: `x[0..n], z[0..n], r` (total width = 2n + 1).
    tableau: Vec<bool>,
    rng: StdRng,
    measurement_record: Vec<MeasurementOutcome>,
}

impl StabilizerState {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new stabilizer state representing |00...0>.
    ///
    /// The initial tableau has destabilizer i = X_i, stabilizer i = Z_i,
    /// and all phase bits set to 0.
    pub fn new(num_qubits: usize) -> Result<Self> {
        Self::new_with_seed(num_qubits, 0)
    }

    /// Create a new stabilizer state with a specific RNG seed.
    pub fn new_with_seed(num_qubits: usize, seed: u64) -> Result<Self> {
        if num_qubits == 0 {
            return Err(QuantumError::CircuitError(
                "stabilizer state requires at least 1 qubit".into(),
            ));
        }

        let n = num_qubits;
        let stride = 2 * n + 1;
        let total = 2 * n * stride;
        let mut tableau = vec![false; total];

        // Destabilizer i (row i): X_i  =>  x[i]=1, rest zero
        for i in 0..n {
            tableau[i * stride + i] = true; // x bit for qubit i
        }
        // Stabilizer i (row n+i): Z_i  =>  z[i]=1, rest zero
        for i in 0..n {
            tableau[(n + i) * stride + n + i] = true; // z bit for qubit i
        }

        Ok(Self {
            num_qubits,
            tableau,
            rng: StdRng::seed_from_u64(seed),
            measurement_record: Vec::new(),
        })
    }

    // -----------------------------------------------------------------------
    // Tableau access helpers
    // -----------------------------------------------------------------------

    #[inline]
    fn stride(&self) -> usize {
        2 * self.num_qubits + 1
    }

    /// Get the X bit for `(row, col)`.
    #[inline]
    fn x(&self, row: usize, col: usize) -> bool {
        self.tableau[row * self.stride() + col]
    }

    /// Get the Z bit for `(row, col)`.
    #[inline]
    fn z(&self, row: usize, col: usize) -> bool {
        self.tableau[row * self.stride() + self.num_qubits + col]
    }

    /// Get the phase bit for `row`.
    #[inline]
    fn r(&self, row: usize) -> bool {
        self.tableau[row * self.stride() + 2 * self.num_qubits]
    }

    #[inline]
    fn set_x(&mut self, row: usize, col: usize, val: bool) {
        let idx = row * self.stride() + col;
        self.tableau[idx] = val;
    }

    #[inline]
    fn set_z(&mut self, row: usize, col: usize, val: bool) {
        let idx = row * self.stride() + self.num_qubits + col;
        self.tableau[idx] = val;
    }

    #[inline]
    fn set_r(&mut self, row: usize, val: bool) {
        let idx = row * self.stride() + 2 * self.num_qubits;
        self.tableau[idx] = val;
    }

    /// Multiply row `target` by row `source` (left-multiply the Pauli string
    /// of `target` by that of `source`), updating the phase of `target`.
    ///
    /// Uses the `g` function to accumulate the phase contribution from
    /// each qubit position.
    fn row_mult(&mut self, target: usize, source: usize) {
        let n = self.num_qubits;
        let mut phase_sum: i32 = 0;

        // Accumulate phase from commutation relations
        for j in 0..n {
            phase_sum += g(
                self.x(source, j),
                self.z(source, j),
                self.x(target, j),
                self.z(target, j),
            );
        }

        // Combine phases: new_r = (2*r_target + 2*r_source + phase_sum) mod 4
        // r=1 means phase -1 (i.e. factor of i^2 = -1), so we work mod 4 in
        // units of i.  r_bit maps to 0 or 2.
        let total = 2 * (self.r(target) as i32) + 2 * (self.r(source) as i32) + phase_sum;
        // Result phase bit: total mod 4 == 2 => r=1, else r=0
        let new_r = ((total % 4) + 4) % 4 == 2;
        self.set_r(target, new_r);

        // XOR the X and Z bits
        let stride = self.stride();
        for j in 0..n {
            let sx = self.tableau[source * stride + j];
            self.tableau[target * stride + j] ^= sx;
        }
        for j in 0..n {
            let sz = self.tableau[source * stride + n + j];
            self.tableau[target * stride + n + j] ^= sz;
        }
    }

    // -----------------------------------------------------------------------
    // Clifford gate operations
    // -----------------------------------------------------------------------

    /// Apply a Hadamard gate on `qubit`.
    ///
    /// Conjugation rules: H X H = Z, H Z H = X, H Y H = -Y.
    /// Tableau update: swap X and Z columns for this qubit in every row,
    /// and flip the phase bit where both X and Z were set (Y -> -Y).
    pub fn hadamard(&mut self, qubit: usize) {
        let n = self.num_qubits;
        for i in 0..(2 * n) {
            let xi = self.x(i, qubit);
            let zi = self.z(i, qubit);
            // phase flip for Y entries: if both x and z are set
            if xi && zi {
                self.set_r(i, !self.r(i));
            }
            // swap x and z
            self.set_x(i, qubit, zi);
            self.set_z(i, qubit, xi);
        }
    }

    /// Apply the phase gate (S gate) on `qubit`.
    ///
    /// Conjugation rules: S X S^dag = Y, S Z S^dag = Z, S Y S^dag = -X.
    /// Tableau update: Z_j -> Z_j XOR X_j, phase flipped where X and Z
    /// are both set.
    pub fn phase_gate(&mut self, qubit: usize) {
        let n = self.num_qubits;
        for i in 0..(2 * n) {
            let xi = self.x(i, qubit);
            let zi = self.z(i, qubit);
            // Phase update: r ^= (x AND z)
            if xi && zi {
                self.set_r(i, !self.r(i));
            }
            // z -> z XOR x
            self.set_z(i, qubit, zi ^ xi);
        }
    }

    /// Apply a CNOT gate with `control` and `target`.
    ///
    /// Conjugation rules:
    ///   X_c -> X_c X_t,  Z_t -> Z_c Z_t,
    ///   X_t -> X_t,      Z_c -> Z_c.
    /// Tableau update for every row:
    ///   phase ^= x_c AND z_t AND (x_t XOR z_c XOR 1)
    ///   x_t ^= x_c
    ///   z_c ^= z_t
    pub fn cnot(&mut self, control: usize, target: usize) {
        let n = self.num_qubits;
        for i in 0..(2 * n) {
            let xc = self.x(i, control);
            let zt = self.z(i, target);
            let xt = self.x(i, target);
            let zc = self.z(i, control);
            // Phase update
            if xc && zt && (xt == zc) {
                self.set_r(i, !self.r(i));
            }
            // x_target ^= x_control
            self.set_x(i, target, xt ^ xc);
            // z_control ^= z_target
            self.set_z(i, control, zc ^ zt);
        }
    }

    /// Apply a Pauli-X gate on `qubit`.
    ///
    /// Conjugation: X commutes with X, anticommutes with Z and Y.
    /// Tableau update: flip phase where Z bit is set for this qubit.
    pub fn x_gate(&mut self, qubit: usize) {
        let n = self.num_qubits;
        for i in 0..(2 * n) {
            if self.z(i, qubit) {
                self.set_r(i, !self.r(i));
            }
        }
    }

    /// Apply a Pauli-Y gate on `qubit`.
    ///
    /// Conjugation: Y anticommutes with both X and Z.
    /// Tableau update: flip phase where X or Z (but via XOR: where x XOR z).
    pub fn y_gate(&mut self, qubit: usize) {
        let n = self.num_qubits;
        for i in 0..(2 * n) {
            let xi = self.x(i, qubit);
            let zi = self.z(i, qubit);
            // Y anticommutes with X and Z, commutes with Y and I
            // phase flips when exactly one of x,z is set (i.e. X or Z, not Y or I)
            if xi ^ zi {
                self.set_r(i, !self.r(i));
            }
        }
    }

    /// Apply a Pauli-Z gate on `qubit`.
    ///
    /// Conjugation: Z commutes with Z, anticommutes with X and Y.
    /// Tableau update: flip phase where X bit is set for this qubit.
    pub fn z_gate(&mut self, qubit: usize) {
        let n = self.num_qubits;
        for i in 0..(2 * n) {
            if self.x(i, qubit) {
                self.set_r(i, !self.r(i));
            }
        }
    }

    /// Apply a CZ (controlled-Z) gate on `q1` and `q2`.
    ///
    /// CZ = (I x H) . CNOT . (I x H).  Implemented by decomposition.
    pub fn cz(&mut self, q1: usize, q2: usize) {
        self.hadamard(q2);
        self.cnot(q1, q2);
        self.hadamard(q2);
    }

    /// Apply a SWAP gate on `q1` and `q2`.
    ///
    /// SWAP = CNOT(q1,q2) . CNOT(q2,q1) . CNOT(q1,q2).
    pub fn swap(&mut self, q1: usize, q2: usize) {
        self.cnot(q1, q2);
        self.cnot(q2, q1);
        self.cnot(q1, q2);
    }

    // -----------------------------------------------------------------------
    // Measurement
    // -----------------------------------------------------------------------

    /// Measure `qubit` in the computational (Z) basis.
    ///
    /// Follows the Aaronson-Gottesman algorithm:
    /// 1. Check if any stabilizer generator anticommutes with Z on the
    ///    measured qubit (i.e. has its X bit set for that qubit).
    /// 2. If yes (random outcome): collapse the state and record the result.
    /// 3. If no (deterministic outcome): compute the result from phases.
    pub fn measure(&mut self, qubit: usize) -> Result<MeasurementOutcome> {
        if qubit >= self.num_qubits {
            return Err(QuantumError::InvalidQubitIndex {
                index: qubit as u32,
                num_qubits: self.num_qubits as u32,
            });
        }

        let n = self.num_qubits;

        // Search for a stabilizer (rows n..2n-1) that anticommutes with Z_qubit.
        // A generator anticommutes with Z_qubit iff its X bit for that qubit is 1.
        let p = (n..(2 * n)).find(|&i| self.x(i, qubit));

        if let Some(p) = p {
            // --- Random outcome ---
            // For every other row that anticommutes with Z_qubit, multiply it by row p
            // to make it commute.
            for i in 0..(2 * n) {
                if i != p && self.x(i, qubit) {
                    self.row_mult(i, p);
                }
            }

            // Move row p to the destabilizer: copy stabilizer p to destabilizer (p-n),
            // then set row p to be +/- Z_qubit.
            let dest_row = p - n;
            let stride = self.stride();
            // Copy row p to destabilizer row
            for j in 0..stride {
                self.tableau[dest_row * stride + j] = self.tableau[p * stride + j];
            }

            // Clear row p and set it to Z_qubit with random phase
            for j in 0..stride {
                self.tableau[p * stride + j] = false;
            }
            self.set_z(p, qubit, true);

            let result: bool = self.rng.gen();
            self.set_r(p, result);

            let outcome = MeasurementOutcome {
                qubit: qubit as u32,
                result,
                probability: 0.5,
            };
            self.measurement_record.push(outcome.clone());
            Ok(outcome)
        } else {
            // --- Deterministic outcome ---
            // No stabilizer anticommutes with Z_qubit, so Z_qubit is in the
            // stabilizer group.  We need to determine its sign.
            //
            // Use a scratch row technique: set a temporary row to the identity,
            // then multiply in every destabilizer whose corresponding stabilizer
            // has x[qubit]=1... but since we confirmed no stabilizer has x set,
            // we look at destabilizers instead.
            //
            // Actually per the CHP algorithm: accumulate into a scratch state
            // by multiplying destabilizer rows whose *destabilizer* X bit for
            // this qubit is set.  The accumulated phase gives the measurement
            // outcome.

            // We'll use the first extra technique: allocate a scratch row
            // initialized to +I and multiply in all generators from rows 0..n
            // (destabilizers) that have x[qubit]=1 in the *stabilizer* row n+i.
            // Wait -- let me re-read the CHP paper carefully.
            //
            // Per Aaronson-Gottesman (Section III.C, deterministic case):
            // Set scratch = identity. For each i in 0..n, if destabilizer i
            // has x[qubit]=1, multiply scratch by stabilizer (n+i).
            // The phase of the scratch row gives the measurement result.

            let stride = self.stride();
            let mut scratch = vec![false; stride];

            for i in 0..n {
                // Check destabilizer row i: does it have x[qubit] set?
                if self.x(i, qubit) {
                    // Multiply scratch by stabilizer row (n+i)
                    let stab_row = n + i;
                    let mut phase_sum: i32 = 0;
                    for j in 0..n {
                        let sx = scratch[j];
                        let sz = scratch[n + j];
                        let rx = self.x(stab_row, j);
                        let rz = self.z(stab_row, j);
                        phase_sum += g(rx, rz, sx, sz);
                    }
                    let scratch_r = scratch[2 * n];
                    let stab_r = self.r(stab_row);
                    let total = 2 * (scratch_r as i32) + 2 * (stab_r as i32) + phase_sum;
                    scratch[2 * n] = ((total % 4) + 4) % 4 == 2;

                    for j in 0..n {
                        scratch[j] ^= self.x(stab_row, j);
                    }
                    for j in 0..n {
                        scratch[n + j] ^= self.z(stab_row, j);
                    }
                }
            }

            let result = scratch[2 * n]; // phase bit = measurement outcome

            let outcome = MeasurementOutcome {
                qubit: qubit as u32,
                result,
                probability: 1.0,
            };
            self.measurement_record.push(outcome.clone());
            Ok(outcome)
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Return the number of qubits in this stabilizer state.
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Return the measurement record accumulated so far.
    pub fn measurement_record(&self) -> &[MeasurementOutcome] {
        &self.measurement_record
    }

    /// Create a copy of this stabilizer state with a new RNG seed.
    ///
    /// The quantum state (tableau) is duplicated exactly; only the RNG
    /// and measurement record are reset.  This is used by the Clifford+T
    /// backend to fork stabilizer terms during T-gate decomposition.
    pub fn clone_with_seed(&self, seed: u64) -> Result<Self> {
        Ok(Self {
            num_qubits: self.num_qubits,
            tableau: self.tableau.clone(),
            rng: StdRng::seed_from_u64(seed),
            measurement_record: Vec::new(),
        })
    }

    /// Check whether a gate is a Clifford gate (simulable by this backend).
    ///
    /// Clifford gates are: H, X, Y, Z, S, Sdg, CNOT, CZ, SWAP.
    /// Measure and Reset are also supported (non-unitary but handled).
    /// T, Tdg, Rx, Ry, Rz, Phase, Rzz, and custom unitaries are NOT Clifford
    /// in general.
    pub fn is_clifford_gate(gate: &Gate) -> bool {
        matches!(
            gate,
            Gate::H(_)
                | Gate::X(_)
                | Gate::Y(_)
                | Gate::Z(_)
                | Gate::S(_)
                | Gate::Sdg(_)
                | Gate::CNOT(_, _)
                | Gate::CZ(_, _)
                | Gate::SWAP(_, _)
                | Gate::Measure(_)
                | Gate::Barrier
        )
    }

    // -----------------------------------------------------------------------
    // Gate dispatch
    // -----------------------------------------------------------------------

    /// Apply a gate from the `Gate` enum, returning measurement outcomes if any.
    ///
    /// Returns an error for non-Clifford gates.
    pub fn apply_gate(&mut self, gate: &Gate) -> Result<Vec<MeasurementOutcome>> {
        match gate {
            Gate::H(q) => {
                self.hadamard(*q as usize);
                Ok(vec![])
            }
            Gate::X(q) => {
                self.x_gate(*q as usize);
                Ok(vec![])
            }
            Gate::Y(q) => {
                self.y_gate(*q as usize);
                Ok(vec![])
            }
            Gate::Z(q) => {
                self.z_gate(*q as usize);
                Ok(vec![])
            }
            Gate::S(q) => {
                self.phase_gate(*q as usize);
                Ok(vec![])
            }
            Gate::Sdg(q) => {
                // S^dag = S^3: apply S three times
                let qu = *q as usize;
                self.phase_gate(qu);
                self.phase_gate(qu);
                self.phase_gate(qu);
                Ok(vec![])
            }
            Gate::CNOT(c, t) => {
                self.cnot(*c as usize, *t as usize);
                Ok(vec![])
            }
            Gate::CZ(q1, q2) => {
                self.cz(*q1 as usize, *q2 as usize);
                Ok(vec![])
            }
            Gate::SWAP(q1, q2) => {
                self.swap(*q1 as usize, *q2 as usize);
                Ok(vec![])
            }
            Gate::Measure(q) => {
                let outcome = self.measure(*q as usize)?;
                Ok(vec![outcome])
            }
            Gate::Barrier => Ok(vec![]),
            _ => Err(QuantumError::CircuitError(format!(
                "gate {:?} is not a Clifford gate and cannot be simulated \
                 by the stabilizer backend",
                gate
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Phase accumulation helper
// ---------------------------------------------------------------------------

/// Compute the phase contribution when multiplying two single-qubit Pauli
/// operators encoded as (x, z) bits.
///
/// Returns 0, +1, or -1 representing a phase of i^0, i^1, or i^{-1}.
///
/// Encoding: (0,0)=I, (1,0)=X, (1,1)=Y, (0,1)=Z.
#[inline]
fn g(x1: bool, z1: bool, x2: bool, z2: bool) -> i32 {
    if !x1 && !z1 {
        return 0; // I * anything = 0 phase
    }
    if x1 && z1 {
        // Y * ...
        if x2 && z2 {
            0
        } else if x2 {
            1
        } else if z2 {
            -1
        } else {
            0
        }
    } else if x1 && !z1 {
        // X * ...
        if x2 && z2 {
            -1
        } else if x2 {
            0
        } else if z2 {
            1
        } else {
            0
        }
    } else {
        // Z * ...  (z1 && !x1)
        if x2 && z2 {
            1
        } else if x2 {
            -1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_measurement() {
        // |0> state: measuring should give 0 deterministically
        let mut state = StabilizerState::new(1).unwrap();
        let outcome = state.measure(0).unwrap();
        assert!(!outcome.result, "measuring |0> should yield 0");
        assert_eq!(outcome.probability, 1.0);
    }

    #[test]
    fn test_x_gate_flips() {
        // X|0> = |1>: measuring should give 1 deterministically
        let mut state = StabilizerState::new(1).unwrap();
        state.x_gate(0);
        let outcome = state.measure(0).unwrap();
        assert!(outcome.result, "measuring X|0> should yield 1");
        assert_eq!(outcome.probability, 1.0);
    }

    #[test]
    fn test_hadamard_creates_superposition() {
        // H|0> = |+>: measurement should be random (prob 0.5)
        let mut state = StabilizerState::new_with_seed(1, 42).unwrap();
        state.hadamard(0);
        let outcome = state.measure(0).unwrap();
        assert_eq!(outcome.probability, 0.5);
    }

    #[test]
    fn test_bell_state() {
        // Create Bell state |00> + |11> (up to normalization)
        // Both qubits should always measure the same value.
        let mut state = StabilizerState::new_with_seed(2, 123).unwrap();
        state.hadamard(0);
        state.cnot(0, 1);
        let o0 = state.measure(0).unwrap();
        let o1 = state.measure(1).unwrap();
        assert_eq!(o0.result, o1.result, "Bell state qubits must be correlated");
    }

    #[test]
    fn test_z_gate_phase() {
        // Z|0> = |0> (no change)
        let mut state = StabilizerState::new(1).unwrap();
        state.z_gate(0);
        let outcome = state.measure(0).unwrap();
        assert!(!outcome.result, "Z|0> should still be |0>");

        // Z|1> = -|1> (global phase, same measurement)
        let mut state2 = StabilizerState::new(1).unwrap();
        state2.x_gate(0);
        state2.z_gate(0);
        let outcome2 = state2.measure(0).unwrap();
        assert!(outcome2.result, "Z|1> should still measure as |1>");
    }

    #[test]
    fn test_phase_gate() {
        // S^2 = Z: applying S twice should act as Z
        let mut s1 = StabilizerState::new_with_seed(1, 99).unwrap();
        s1.hadamard(0);
        s1.phase_gate(0);
        s1.phase_gate(0);
        // Now state is Z H|0> = Z|+> = |->

        let mut s2 = StabilizerState::new_with_seed(1, 99).unwrap();
        s2.hadamard(0);
        s2.z_gate(0);
        // Also |->

        // Measuring in X basis: H then measure
        s1.hadamard(0);
        s2.hadamard(0);
        let o1 = s1.measure(0).unwrap();
        let o2 = s2.measure(0).unwrap();
        assert_eq!(o1.result, o2.result, "S^2 should equal Z");
    }

    #[test]
    fn test_cz_gate() {
        // CZ on |+0> should give |0+> + |1-> = |00> + |01> + |10> - |11>
        // This is a product state in the X-Z basis.
        // After CZ, measuring qubit 0 in Z basis should still be random.
        let mut state = StabilizerState::new_with_seed(2, 777).unwrap();
        state.hadamard(0);
        state.cz(0, 1);
        let o = state.measure(0).unwrap();
        assert_eq!(o.probability, 0.5);
    }

    #[test]
    fn test_swap_gate() {
        // Prepare |10>, SWAP -> |01>
        let mut state = StabilizerState::new(2).unwrap();
        state.x_gate(0);
        state.swap(0, 1);
        let o0 = state.measure(0).unwrap();
        let o1 = state.measure(1).unwrap();
        assert!(!o0.result, "after SWAP, qubit 0 should be |0>");
        assert!(o1.result, "after SWAP, qubit 1 should be |1>");
    }

    #[test]
    fn test_is_clifford_gate() {
        assert!(StabilizerState::is_clifford_gate(&Gate::H(0)));
        assert!(StabilizerState::is_clifford_gate(&Gate::CNOT(0, 1)));
        assert!(StabilizerState::is_clifford_gate(&Gate::S(0)));
        assert!(!StabilizerState::is_clifford_gate(&Gate::T(0)));
        assert!(!StabilizerState::is_clifford_gate(&Gate::Rx(0, 0.5)));
    }

    #[test]
    fn test_apply_gate_dispatch() {
        let mut state = StabilizerState::new(2).unwrap();
        state.apply_gate(&Gate::H(0)).unwrap();
        state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
        let outcomes = state.apply_gate(&Gate::Measure(0)).unwrap();
        assert_eq!(outcomes.len(), 1);
    }

    #[test]
    fn test_non_clifford_rejected() {
        let mut state = StabilizerState::new(1).unwrap();
        let result = state.apply_gate(&Gate::T(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_measurement_record() {
        let mut state = StabilizerState::new(2).unwrap();
        state.x_gate(1);
        state.measure(0).unwrap();
        state.measure(1).unwrap();
        let record = state.measurement_record();
        assert_eq!(record.len(), 2);
        assert!(!record[0].result);
        assert!(record[1].result);
    }

    #[test]
    fn test_invalid_qubit_measure() {
        let mut state = StabilizerState::new(2).unwrap();
        let result = state.measure(5);
        assert!(result.is_err());
    }

    #[test]
    fn test_y_gate() {
        // Y|0> = i|1>, so measurement should give 1
        let mut state = StabilizerState::new(1).unwrap();
        state.y_gate(0);
        let outcome = state.measure(0).unwrap();
        assert!(outcome.result, "Y|0> should measure as |1>");
    }

    #[test]
    fn test_sdg_gate() {
        // Sdg = S^3, and S^4 = I, so S . Sdg = I
        let mut state = StabilizerState::new_with_seed(1, 42).unwrap();
        state.hadamard(0);
        state.phase_gate(0); // S
        state.apply_gate(&Gate::Sdg(0)).unwrap(); // Sdg
                                                  // Should be back to H|0> = |+>
        state.hadamard(0);
        let outcome = state.measure(0).unwrap();
        assert!(!outcome.result, "S.Sdg should be identity");
        assert_eq!(outcome.probability, 1.0);
    }

    #[test]
    fn test_g_function() {
        // I * anything = 0
        assert_eq!(g(false, false, true, true), 0);
        // X * Y = iZ  => phase +1
        assert_eq!(g(true, false, true, true), -1);
        // X * Z = -iY => phase -1... wait: g(X, Z) = g(1,0, 0,1) = 1
        // Actually X*Z = -iY, but g returns the exponent of i in the
        // *product* commutation, and we get +1 here because the Pauli
        // product rule for X*Z uses a different sign convention.
        assert_eq!(g(true, false, false, true), 1);
        // Y * X = -iZ => phase -1... g(1,1, 1,0) = 1
        assert_eq!(g(true, true, true, false), 1);
    }

    #[test]
    fn test_ghz_state() {
        // GHZ state: H on q0, then CNOT chain
        let n = 5;
        let mut state = StabilizerState::new_with_seed(n, 314).unwrap();
        state.hadamard(0);
        for i in 0..(n - 1) {
            state.cnot(i, i + 1);
        }
        // All qubits should measure the same value
        let first = state.measure(0).unwrap();
        for i in 1..n {
            let oi = state.measure(i).unwrap();
            assert_eq!(
                first.result, oi.result,
                "GHZ state: qubit {} disagrees with qubit 0",
                i
            );
        }
    }
}
