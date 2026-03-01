//! Clifford+T backend via low-rank stabilizer decomposition.
//!
//! Bridges the gap between the pure Clifford stabilizer backend (millions of
//! qubits, Clifford-only) and the full state-vector simulator (any gate, <=32
//! qubits).  Circuits with moderate T-count are simulated exactly using a
//! stabilizer rank decomposition:
//!
//!   |psi> = sum_k  alpha_k |stabilizer_k>
//!
//! Each T gate doubles the number of terms (2^t terms for t T-gates).
//! Clifford gates are applied term-by-term in O(n) time each, preserving
//! the stabilizer structure.
//!
//! Reference: Bravyi & Gosset, "Improved Classical Simulation of Quantum
//! Circuits Dominated by Clifford Gates", Phys. Rev. Lett. 116, 250501 (2016).

use crate::circuit::QuantumCircuit;
use crate::error::{QuantumError, Result};
use crate::gate::Gate;
use crate::stabilizer::StabilizerState;
use crate::types::{Complex, MeasurementOutcome};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of stabilizer terms (2^16).
const DEFAULT_MAX_TERMS: usize = 65536;

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// Result of running a circuit through the Clifford+T backend.
#[derive(Debug, Clone)]
pub struct CliffordTResult {
    /// All measurement outcomes collected during the circuit.
    pub measurements: Vec<MeasurementOutcome>,
    /// Total number of T and Tdg gates encountered.
    pub t_count: usize,
    /// Number of stabilizer terms at the end of the circuit.
    pub num_terms: usize,
    /// Peak number of stabilizer terms during the circuit.
    pub peak_terms: usize,
}

// ---------------------------------------------------------------------------
// CliffordTState
// ---------------------------------------------------------------------------

/// Clifford+T simulator state using stabilizer rank decomposition.
///
/// Represents a quantum state as a weighted sum of stabilizer states:
///
///   |psi> = sum_k  alpha_k |stabilizer_k>
///
/// Clifford gates are applied to each term individually.  Each T gate
/// doubles the number of terms via the decomposition:
///
///   T = (1 + e^(i*pi/4))/2 * I  +  (1 - e^(i*pi/4))/2 * Z
pub struct CliffordTState {
    num_qubits: usize,
    /// Stabilizer rank decomposition: each term is (coefficient, stabilizer_state).
    terms: Vec<(Complex, StabilizerState)>,
    t_count: usize,
    max_terms: usize,
    seed: u64,
    /// Monotonic counter for generating unique fork seeds.
    fork_counter: u64,
    /// RNG used for measurement outcome sampling.
    rng: StdRng,
}

impl CliffordTState {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new Clifford+T state for `num_qubits` qubits.
    ///
    /// * `max_t_gates` -- maximum T/Tdg gates allowed.  The number of terms
    ///   grows as 2^t, capped at `min(2^max_t_gates, 65536)`.
    /// * `seed` -- RNG seed for reproducible measurement outcomes.
    ///
    /// The initial state is |00...0> with a single stabilizer term of
    /// coefficient 1.
    pub fn new(num_qubits: usize, max_t_gates: usize, seed: u64) -> Result<Self> {
        if num_qubits == 0 {
            return Err(QuantumError::CircuitError(
                "Clifford+T state requires at least 1 qubit".into(),
            ));
        }

        let max_terms = if max_t_gates >= 20 {
            DEFAULT_MAX_TERMS
        } else {
            (1usize << max_t_gates).min(DEFAULT_MAX_TERMS)
        };

        let initial = StabilizerState::new_with_seed(num_qubits, seed)?;

        Ok(Self {
            num_qubits,
            terms: vec![(Complex::ONE, initial)],
            t_count: 0,
            max_terms,
            seed,
            fork_counter: 1,
            rng: StdRng::seed_from_u64(seed.wrapping_add(0xDEAD_BEEF)),
        })
    }

    // -------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------

    /// Return the current number of stabilizer terms in the decomposition.
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    /// Return the total T-gate count (T + Tdg) applied so far.
    pub fn t_count(&self) -> usize {
        self.t_count
    }

    /// Return the number of qubits.
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Generate a unique RNG seed for a forked stabilizer state.
    fn next_seed(&mut self) -> u64 {
        let s = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.fork_counter);
        self.fork_counter += 1;
        s
    }

    /// Validate that a qubit index is in range.
    fn check_qubit(&self, qubit: usize) -> Result<()> {
        if qubit >= self.num_qubits {
            Err(QuantumError::InvalidQubitIndex {
                index: qubit as u32,
                num_qubits: self.num_qubits as u32,
            })
        } else {
            Ok(())
        }
    }

    // -------------------------------------------------------------------
    // Clifford gate application
    // -------------------------------------------------------------------

    /// Apply a Clifford gate to all terms in the decomposition.
    ///
    /// Supported: H, X, Y, Z, S, Sdg, CNOT, CZ, SWAP, Barrier.
    /// For Measure, use `apply_gate` or `measure` instead.
    pub fn apply_clifford(&mut self, gate: &Gate) -> Result<()> {
        if matches!(gate, Gate::Barrier) {
            return Ok(());
        }

        if !StabilizerState::is_clifford_gate(gate) || matches!(gate, Gate::Measure(_)) {
            return Err(QuantumError::CircuitError(format!(
                "gate {:?} is not a (non-measurement) Clifford gate",
                gate
            )));
        }

        for &q in gate.qubits().iter() {
            self.check_qubit(q as usize)?;
        }

        for (_coeff, state) in &mut self.terms {
            state.apply_gate(gate)?;
        }

        Ok(())
    }

    // -------------------------------------------------------------------
    // T / Tdg decomposition
    // -------------------------------------------------------------------

    /// Common implementation for T and Tdg gate decomposition.
    ///
    /// The gate is decomposed as:  gate = c_plus * I + c_minus * Z
    ///
    /// For each existing term (alpha, |psi>), this produces two new terms:
    ///   (alpha * c_plus,  |psi>)
    ///   (alpha * c_minus, Z_qubit |psi>)
    ///
    /// The Z branch is obtained by cloning the stabilizer state via
    /// `clone_with_seed` and applying Z on the target qubit.
    fn apply_t_impl(&mut self, qubit: usize, c_plus: Complex, c_minus: Complex) -> Result<()> {
        self.check_qubit(qubit)?;

        let new_count = self.terms.len() * 2;
        if new_count > self.max_terms {
            return Err(QuantumError::CircuitError(format!(
                "T/Tdg gate would create {} terms, exceeding max of {}",
                new_count, self.max_terms
            )));
        }

        let old_terms = std::mem::take(&mut self.terms);
        let mut new_terms = Vec::with_capacity(new_count);

        for (alpha, state) in old_terms {
            // Branch 2 first: clone the state, then apply Z for the c_minus branch.
            let fork_seed = self.next_seed();
            let mut forked = state.clone_with_seed(fork_seed)?;
            forked.z_gate(qubit);

            // Branch 1: alpha * c_plus * |psi>  (original state, unchanged).
            new_terms.push((alpha * c_plus, state));
            // Branch 2: alpha * c_minus * Z_qubit |psi>.
            new_terms.push((alpha * c_minus, forked));
        }

        self.terms = new_terms;
        self.t_count += 1;

        Ok(())
    }

    /// Apply a T gate on `qubit` via stabilizer rank decomposition.
    ///
    /// T = |0><0| + e^(i*pi/4)|1><1|
    ///   = (1 + e^(i*pi/4))/2 * I  +  (1 - e^(i*pi/4))/2 * Z
    ///
    /// Each existing term splits into two, doubling the total.
    pub fn apply_t(&mut self, qubit: usize) -> Result<()> {
        let omega = Complex::new(
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2,
        );
        let c_plus = (Complex::ONE + omega) * 0.5;
        let c_minus = (Complex::ONE - omega) * 0.5;
        self.apply_t_impl(qubit, c_plus, c_minus)
    }

    /// Apply a Tdg (T-dagger) gate on `qubit`.
    ///
    /// Tdg = |0><0| + e^(-i*pi/4)|1><1|
    ///     = (1 + e^(-i*pi/4))/2 * I  +  (1 - e^(-i*pi/4))/2 * Z
    pub fn apply_tdg(&mut self, qubit: usize) -> Result<()> {
        let omega_conj = Complex::new(
            std::f64::consts::FRAC_1_SQRT_2,
            -std::f64::consts::FRAC_1_SQRT_2,
        );
        let c_plus = (Complex::ONE + omega_conj) * 0.5;
        let c_minus = (Complex::ONE - omega_conj) * 0.5;
        self.apply_t_impl(qubit, c_plus, c_minus)
    }

    // -------------------------------------------------------------------
    // Gate dispatch
    // -------------------------------------------------------------------

    /// Apply a gate, routing to the appropriate handler.
    ///
    /// * Clifford gates: applied to all terms via `apply_clifford`.
    /// * T / Tdg: stabilizer rank decomposition.
    /// * Measure: weighted measurement across all terms.
    /// * Barrier: no-op.
    /// * Others (Rx, Ry, Rz, Phase, Rzz, Reset, Unitary1Q): error.
    pub fn apply_gate(&mut self, gate: &Gate) -> Result<Vec<MeasurementOutcome>> {
        match gate {
            Gate::T(q) => {
                self.apply_t(*q as usize)?;
                Ok(vec![])
            }
            Gate::Tdg(q) => {
                self.apply_tdg(*q as usize)?;
                Ok(vec![])
            }
            Gate::Measure(q) => {
                let outcome = self.measure(*q as usize)?;
                Ok(vec![outcome])
            }
            Gate::Barrier => Ok(vec![]),
            _ if StabilizerState::is_clifford_gate(gate) => {
                self.apply_clifford(gate)?;
                Ok(vec![])
            }
            _ => Err(QuantumError::CircuitError(format!(
                "gate {:?} is not supported by the Clifford+T backend; \
                 only Clifford gates and T/Tdg are allowed",
                gate
            ))),
        }
    }

    // -------------------------------------------------------------------
    // Measurement
    // -------------------------------------------------------------------

    /// Measure `qubit` in the computational (Z) basis.
    ///
    /// Algorithm:
    /// 1. For each term, probe the measurement probability by cloning the
    ///    stabilizer state, measuring the clone, and reading whether the
    ///    outcome was deterministic (prob 1.0) or random (prob 0.5).
    /// 2. Compute the weighted probability of |0>:
    ///    p0 = sum_k |alpha_k|^2 * p0_k  /  sum_k |alpha_k|^2
    /// 3. Sample an outcome using the RNG.
    /// 4. Collapse each term to match: measure the live state and fix up
    ///    any wrong-outcome random measurements via X gate.
    /// 5. Remove incompatible terms and renormalise.
    pub fn measure(&mut self, qubit: usize) -> Result<MeasurementOutcome> {
        self.check_qubit(qubit)?;

        if self.terms.is_empty() {
            return Err(QuantumError::CircuitError(
                "no stabilizer terms remain".into(),
            ));
        }

        // Step 1: probe each term's measurement probability via cloning.
        // Use index-based iteration to avoid borrow conflict with next_seed().
        let n = self.terms.len();
        let mut term_p0: Vec<f64> = Vec::with_capacity(n);
        let mut total_weight = 0.0f64;
        let mut p0_weighted = 0.0f64;

        for i in 0..n {
            let w = self.terms[i].0.norm_sq();
            if w < 1e-30 {
                term_p0.push(0.5);
                continue;
            }
            total_weight += w;

            let probe_seed = self.next_seed();
            let mut probe = self.terms[i].1.clone_with_seed(probe_seed)?;
            let probe_meas = probe.measure(qubit)?;

            let p0_k = if (probe_meas.probability - 1.0).abs() < 1e-10 {
                if !probe_meas.result {
                    1.0
                } else {
                    0.0
                }
            } else {
                0.5
            };

            term_p0.push(p0_k);
            p0_weighted += w * p0_k;
        }

        // Step 2: normalised probability of |0>.
        let p0 = if total_weight > 1e-30 {
            (p0_weighted / total_weight).clamp(0.0, 1.0)
        } else {
            0.5
        };

        // Step 3: sample outcome.
        let r: f64 = self.rng.gen();
        let outcome = r >= p0; // true => |1>
        let prob = if outcome { 1.0 - p0 } else { p0 };

        // Step 4 & 5: collapse and filter.
        //
        // For each term we need the post-measurement stabilizer state
        // conditioned on the chosen outcome.  The stabilizer measurement
        // is destructive (it collapses the full multi-qubit state), so
        // we must not "fix up" a wrong outcome with X -- that would
        // break entanglement correlations on other qubits.
        //
        // Strategy: clone the state before measuring.  Measure the clone.
        // If it gives the desired outcome, use the measured clone.  If
        // not, try again with a different seed.  For deterministic
        // outcomes that disagree, the term is incompatible and is dropped.
        let old_terms = std::mem::take(&mut self.terms);
        let mut new_terms: Vec<(Complex, StabilizerState)> = Vec::with_capacity(old_terms.len());

        for (i, (alpha, state)) in old_terms.into_iter().enumerate() {
            let w = alpha.norm_sq();
            if w < 1e-30 {
                continue;
            }

            let p0_k = term_p0[i];
            let term_prob = if !outcome { p0_k } else { 1.0 - p0_k };

            if term_prob < 1e-15 {
                // Deterministic measurement gives the wrong outcome.
                continue;
            }

            // For deterministic measurements (p0_k is 0 or 1), only the
            // correct outcome passes the filter above, so any clone will
            // produce the right result.  For random measurements (p0_k=0.5),
            // we retry until we get the desired outcome.
            for _ in 0..50 {
                let clone_seed = self.next_seed();
                let mut cloned = state.clone_with_seed(clone_seed)?;
                let meas = cloned.measure(qubit)?;
                if meas.result == outcome {
                    let scale = term_prob.sqrt();
                    new_terms.push((alpha * scale, cloned));
                    break;
                }
                // Wrong outcome on a random measurement -- retry.
            }
            // After 50 attempts (probability 2^{-50} of all failing for
            // a 50/50 measurement), silently drop.  This is astronomically
            // unlikely and introduces negligible error.
        }

        self.terms = new_terms;
        self.renormalize();

        Ok(MeasurementOutcome {
            qubit: qubit as u32,
            result: outcome,
            probability: prob,
        })
    }

    // -------------------------------------------------------------------
    // Expectation value
    // -------------------------------------------------------------------

    /// Compute the expectation value <Z> for the given qubit.
    ///
    /// <Z> = sum_k |alpha_k|^2 * z_k  /  sum_k |alpha_k|^2
    ///
    /// where z_k is +1 (deterministic |0>), -1 (deterministic |1>), or
    /// 0 (random 50/50) for stabilizer term k.
    pub fn expectation_value(&self, qubit: usize) -> f64 {
        if qubit >= self.num_qubits {
            return 0.0;
        }

        let mut weighted_z = 0.0f64;
        let mut total_weight = 0.0f64;
        let mut probe_seed = self
            .seed
            .wrapping_add(self.fork_counter)
            .wrapping_add(0xCAFE_BABE);

        for (alpha, state) in &self.terms {
            let w = alpha.norm_sq();
            if w < 1e-30 {
                continue;
            }
            total_weight += w;

            probe_seed = probe_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            if let Ok(mut probe) = state.clone_with_seed(probe_seed) {
                if let Ok(meas) = probe.measure(qubit) {
                    let z_k = if (meas.probability - 1.0).abs() < 1e-10 {
                        if !meas.result {
                            1.0
                        } else {
                            -1.0
                        }
                    } else {
                        0.0
                    };
                    weighted_z += w * z_k;
                }
            }
        }

        if total_weight > 1e-30 {
            weighted_z / total_weight
        } else {
            0.0
        }
    }

    // -------------------------------------------------------------------
    // Term management
    // -------------------------------------------------------------------

    /// Remove terms whose amplitude is below `threshold` and renormalise.
    pub fn prune_small_terms(&mut self, threshold: f64) {
        let threshold_sq = threshold * threshold;

        let old_terms = std::mem::take(&mut self.terms);
        let mut new_terms = Vec::with_capacity(old_terms.len());

        for (alpha, state) in old_terms {
            if alpha.norm_sq() >= threshold_sq {
                new_terms.push((alpha, state));
            }
        }

        self.terms = new_terms;
        self.renormalize();
    }

    /// Renormalise coefficients so that sum_k |alpha_k|^2 = 1.
    fn renormalize(&mut self) {
        let total: f64 = self.terms.iter().map(|(a, _)| a.norm_sq()).sum();
        if total < 1e-30 || (total - 1.0).abs() < 1e-14 {
            return;
        }
        let inv_sqrt = 1.0 / total.sqrt();
        for (a, _) in &mut self.terms {
            *a = *a * inv_sqrt;
        }
    }

    // -------------------------------------------------------------------
    // High-level circuit runner
    // -------------------------------------------------------------------

    /// Run a complete quantum circuit through the Clifford+T backend.
    ///
    /// Returns measurement outcomes and simulation statistics.
    pub fn run_circuit(
        circuit: &QuantumCircuit,
        max_t: usize,
        seed: u64,
    ) -> Result<CliffordTResult> {
        let mut state = CliffordTState::new(circuit.num_qubits() as usize, max_t, seed)?;
        let mut measurements = Vec::new();
        let mut peak_terms: usize = 1;

        for gate in circuit.gates() {
            let outcomes = state.apply_gate(gate)?;
            measurements.extend(outcomes);
            if state.num_terms() > peak_terms {
                peak_terms = state.num_terms();
            }
        }

        Ok(CliffordTResult {
            measurements,
            t_count: state.t_count(),
            num_terms: state.num_terms(),
            peak_terms,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;
    use crate::gate::Gate;

    // ---- Pure Clifford: matches StabilizerState ----

    #[test]
    fn test_pure_clifford_x_gate() {
        let mut ct = CliffordTState::new(1, 0, 42).unwrap();
        ct.apply_gate(&Gate::X(0)).unwrap();
        let m = ct.measure(0).unwrap();
        assert!(m.result, "X|0> should measure |1>");
        assert_eq!(ct.num_terms(), 1, "pure Clifford keeps 1 term");
    }

    #[test]
    fn test_pure_clifford_bell_state() {
        for seed in 0..20u64 {
            let mut ct = CliffordTState::new(2, 0, seed).unwrap();
            ct.apply_gate(&Gate::H(0)).unwrap();
            ct.apply_gate(&Gate::CNOT(0, 1)).unwrap();
            let m0 = ct.measure(0).unwrap();
            let m1 = ct.measure(1).unwrap();
            assert_eq!(
                m0.result, m1.result,
                "Bell state qubits must agree (seed={})",
                seed
            );
        }
    }

    // ---- Single T gate creates 2 terms ----

    #[test]
    fn test_single_t_creates_two_terms() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        assert_eq!(st.num_terms(), 1);
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        assert_eq!(st.t_count(), 1);
    }

    // ---- Two T gates create 4 terms ----

    #[test]
    fn test_two_t_gates_create_four_terms() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 4);
        assert_eq!(st.t_count(), 2);
    }

    // ---- T then Tdg: terms can be pruned back ----

    #[test]
    fn test_t_then_tdg_prunable() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        st.apply_gate(&Gate::Tdg(0)).unwrap();
        assert_eq!(st.num_terms(), 4);

        // T * Tdg = I on |0>, so after pruning measurement should give |0>.
        st.prune_small_terms(0.1);
        let m = st.measure(0).unwrap();
        assert!(!m.result, "T.Tdg|0> should measure |0>");
    }

    // ---- Bell state + T: measurement correlation ----

    #[test]
    fn test_bell_plus_t_correlation() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.cnot(0, 1);
        circuit.t(0);
        circuit.measure(0);
        circuit.measure(1);

        let shots = 100;
        let mut correlated = 0;
        for s in 0..shots {
            let res = CliffordTState::run_circuit(&circuit, 4, s as u64 * 7919 + 13).unwrap();
            assert_eq!(res.measurements.len(), 2);
            assert_eq!(res.t_count, 1);
            assert_eq!(res.peak_terms, 2);
            if res.measurements[0].result == res.measurements[1].result {
                correlated += 1;
            }
        }
        assert!(
            correlated > 90,
            "Bell+T: qubits should be correlated ({}/{})",
            correlated,
            shots
        );
    }

    // ---- Max terms exceeded returns error ----

    #[test]
    fn test_max_terms_exceeded() {
        let mut st = CliffordTState::new(1, 2, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap(); // 2 terms
        st.apply_gate(&Gate::T(0)).unwrap(); // 4 terms
        let err = st.apply_gate(&Gate::T(0)); // would be 8 > 4
        assert!(err.is_err());
    }

    // ---- Measure collapses terms ----

    #[test]
    fn test_measure_collapses_terms() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        let _m = st.measure(0).unwrap();
        assert!(st.num_terms() >= 1 && st.num_terms() <= 2);
    }

    // ---- GHZ + T ----

    #[test]
    fn test_ghz_plus_t() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.h(0);
        circuit.cnot(0, 1);
        circuit.cnot(1, 2);
        circuit.t(0);
        circuit.measure(0);
        circuit.measure(1);
        circuit.measure(2);

        let shots = 100;
        let mut all_same = 0;
        for s in 0..shots {
            let res = CliffordTState::run_circuit(&circuit, 4, s as u64 * 999983 + 7).unwrap();
            assert_eq!(res.measurements.len(), 3);
            assert_eq!(res.t_count, 1);
            let (r0, r1, r2) = (
                res.measurements[0].result,
                res.measurements[1].result,
                res.measurements[2].result,
            );
            if r0 == r1 && r1 == r2 {
                all_same += 1;
            }
        }
        assert!(
            all_same > 90,
            "GHZ+T: all qubits should agree ({}/{})",
            all_same,
            shots
        );
    }

    // ---- Non-Clifford non-T gates are rejected ----

    #[test]
    fn test_unsupported_gates_rejected() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        assert!(st.apply_gate(&Gate::Rx(0, 0.5)).is_err());
        assert!(st.apply_gate(&Gate::Ry(0, 0.3)).is_err());
        assert!(st.apply_gate(&Gate::Rz(0, 0.1)).is_err());
        assert!(st.apply_gate(&Gate::Phase(0, 1.0)).is_err());
    }

    // ---- Zero qubits rejected ----

    #[test]
    fn test_zero_qubits() {
        assert!(CliffordTState::new(0, 4, 42).is_err());
    }

    // ---- Expectation values ----

    #[test]
    fn test_expectation_z_ground() {
        let st = CliffordTState::new(1, 4, 42).unwrap();
        let z = st.expectation_value(0);
        assert!(
            (z - 1.0).abs() < 0.01,
            "<Z> for |0> should be +1, got {}",
            z
        );
    }

    #[test]
    fn test_expectation_z_excited() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::X(0)).unwrap();
        let z = st.expectation_value(0);
        assert!(
            (z + 1.0).abs() < 0.01,
            "<Z> for |1> should be -1, got {}",
            z
        );
    }

    #[test]
    fn test_expectation_z_superposition() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        let z = st.expectation_value(0);
        assert!(z.abs() < 0.01, "<Z> for |+> should be 0, got {}", z);
    }

    // ---- Tdg creates 2 terms ----

    #[test]
    fn test_tdg_creates_two_terms() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::Tdg(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        assert_eq!(st.t_count(), 1);
    }

    // ---- run_circuit statistics ----

    #[test]
    fn test_run_circuit_statistics() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.h(0);
        circuit.t(0);
        circuit.measure(0);

        let res = CliffordTState::run_circuit(&circuit, 4, 42).unwrap();
        assert_eq!(res.measurements.len(), 1);
        assert_eq!(res.t_count, 1);
        assert_eq!(res.peak_terms, 2);
    }

    // ---- Prune extremes ----

    #[test]
    fn test_prune_low_threshold_keeps_all() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        st.prune_small_terms(1e-15);
        assert_eq!(st.num_terms(), 2);
    }

    #[test]
    fn test_prune_high_threshold_removes_all() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        st.prune_small_terms(100.0);
        assert_eq!(st.num_terms(), 0);
    }

    // ---- Barrier is a no-op ----

    #[test]
    fn test_barrier() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::Barrier).unwrap();
        assert_eq!(st.num_terms(), 1);
    }

    // ---- Invalid qubit indices ----

    #[test]
    fn test_invalid_qubit_t() {
        let mut st = CliffordTState::new(2, 4, 42).unwrap();
        assert!(st.apply_t(5).is_err());
    }

    #[test]
    fn test_invalid_qubit_tdg() {
        let mut st = CliffordTState::new(2, 4, 42).unwrap();
        assert!(st.apply_tdg(5).is_err());
    }

    #[test]
    fn test_invalid_qubit_measure() {
        let mut st = CliffordTState::new(2, 4, 42).unwrap();
        assert!(st.measure(5).is_err());
    }

    // ---- T on different qubits ----

    #[test]
    fn test_t_on_different_qubits() {
        let mut st = CliffordTState::new(2, 4, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        st.apply_gate(&Gate::T(1)).unwrap();
        assert_eq!(st.num_terms(), 4);
        assert_eq!(st.t_count(), 2);
    }

    // ---- Clifford after T preserves term count ----

    #[test]
    fn test_clifford_after_t() {
        let mut st = CliffordTState::new(2, 4, 42).unwrap();
        st.apply_gate(&Gate::T(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        st.apply_gate(&Gate::H(0)).unwrap();
        assert_eq!(st.num_terms(), 2);
        st.apply_gate(&Gate::CNOT(0, 1)).unwrap();
        assert_eq!(st.num_terms(), 2);
    }

    // ---- Deterministic measurement after X ----

    #[test]
    fn test_deterministic_measure_x() {
        let mut st = CliffordTState::new(1, 4, 42).unwrap();
        st.apply_gate(&Gate::X(0)).unwrap();
        let m = st.measure(0).unwrap();
        assert!(m.result, "X|0> should measure |1>");
    }

    // ---- Multiple measurements in circuit ----

    #[test]
    fn test_multi_measure_circuit() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.x(1);
        circuit.measure(0);
        circuit.measure(1);
        circuit.measure(2);

        let res = CliffordTState::run_circuit(&circuit, 0, 42).unwrap();
        assert_eq!(res.measurements.len(), 3);
        assert!(!res.measurements[0].result);
        assert!(res.measurements[1].result);
        assert!(!res.measurements[2].result);
    }

    // ---- S gate (Clifford) via Clifford+T backend ----

    #[test]
    fn test_s_gate_clifford_t() {
        // S^2 = Z, so H S S H = H Z H = X, thus H S S H |0> = |1>.
        let mut st = CliffordTState::new(1, 0, 42).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        st.apply_gate(&Gate::S(0)).unwrap();
        st.apply_gate(&Gate::S(0)).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        let m = st.measure(0).unwrap();
        assert!(m.result, "H.S.S.H|0> = X|0> = |1>");
    }

    // ---- Sdg gate ----

    #[test]
    fn test_sdg_gate() {
        // S . Sdg = I, so H S Sdg H |0> = |0>.
        let mut st = CliffordTState::new(1, 0, 42).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        st.apply_gate(&Gate::S(0)).unwrap();
        st.apply_gate(&Gate::Sdg(0)).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        let m = st.measure(0).unwrap();
        assert!(!m.result, "H.S.Sdg.H|0> = |0>");
    }

    // ---- CZ, SWAP gates ----

    #[test]
    fn test_cz_gate_clifford_t() {
        let mut st = CliffordTState::new(2, 0, 42).unwrap();
        st.apply_gate(&Gate::H(0)).unwrap();
        st.apply_gate(&Gate::CZ(0, 1)).unwrap();
        let m0 = st.measure(0).unwrap();
        assert_eq!(m0.probability, 0.5, "CZ on |+0> leaves q0 random");
    }

    #[test]
    fn test_swap_gate_clifford_t() {
        let mut st = CliffordTState::new(2, 0, 42).unwrap();
        st.apply_gate(&Gate::X(0)).unwrap();
        st.apply_gate(&Gate::SWAP(0, 1)).unwrap();
        let m0 = st.measure(0).unwrap();
        let m1 = st.measure(1).unwrap();
        assert!(!m0.result, "after SWAP |10>, q0 = |0>");
        assert!(m1.result, "after SWAP |10>, q1 = |1>");
    }

    // ---- Expectation value out-of-range qubit returns 0 ----

    #[test]
    fn test_expectation_value_oob() {
        let st = CliffordTState::new(1, 4, 42).unwrap();
        assert_eq!(st.expectation_value(99), 0.0);
    }

    // ---- T gate on |0> is deterministic ----

    #[test]
    fn test_t_on_zero_measure() {
        // T|0> = |0> (T only phases |1>), so measurement should always give 0.
        for seed in 0..20u64 {
            let mut st = CliffordTState::new(1, 4, seed).unwrap();
            st.apply_gate(&Gate::T(0)).unwrap();
            let m = st.measure(0).unwrap();
            assert!(!m.result, "T|0> should measure |0> (seed={})", seed);
        }
    }

    // ---- T gate on |1> is deterministic ----

    #[test]
    fn test_t_on_one_measure() {
        // X|0> = |1>, T|1> = e^(i*pi/4)|1>; measurement should give 1.
        for seed in 0..20u64 {
            let mut st = CliffordTState::new(1, 4, seed).unwrap();
            st.apply_gate(&Gate::X(0)).unwrap();
            st.apply_gate(&Gate::T(0)).unwrap();
            let m = st.measure(0).unwrap();
            assert!(m.result, "T|1> should measure |1> (seed={})", seed);
        }
    }

    // ---- num_qubits accessor ----

    #[test]
    fn test_num_qubits_accessor() {
        let st = CliffordTState::new(5, 4, 42).unwrap();
        assert_eq!(st.num_qubits(), 5);
    }

    // ---- Y and Z gates through Clifford+T ----

    #[test]
    fn test_y_gate() {
        let mut st = CliffordTState::new(1, 0, 42).unwrap();
        st.apply_gate(&Gate::Y(0)).unwrap();
        let m = st.measure(0).unwrap();
        assert!(m.result, "Y|0> should measure |1>");
    }

    #[test]
    fn test_z_gate_on_zero() {
        let mut st = CliffordTState::new(1, 0, 42).unwrap();
        st.apply_gate(&Gate::Z(0)).unwrap();
        let m = st.measure(0).unwrap();
        assert!(!m.result, "Z|0> = |0>");
    }
}
