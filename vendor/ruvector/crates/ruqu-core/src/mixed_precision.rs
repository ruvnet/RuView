//! Mixed-precision (f32) quantum state vector.
//!
//! Provides a float32 complex type and state vector that uses half the memory
//! of the standard f64 state, enabling simulation of approximately one
//! additional qubit at each memory threshold.
//!
//! | Qubits | f64 memory | f32 memory |
//! |--------|-----------|-----------|
//! | 25     | 512 MiB   | 256 MiB   |
//! | 30     | 16 GiB    | 8 GiB     |
//! | 32     | 64 GiB    | 32 GiB    |
//! | 33     | 128 GiB   | 64 GiB    |

use crate::error::{QuantumError, Result};
use crate::gate::Gate;
use crate::types::{Complex, MeasurementOutcome, QubitIndex};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub};

// ---------------------------------------------------------------------------
// Complex32
// ---------------------------------------------------------------------------

/// Complex number using f32 precision (8 bytes vs 16 bytes for f64).
///
/// This is the building block for `QuantumStateF32`. Each amplitude occupies
/// half the memory of the standard `Complex` (f64) type, doubling the number
/// of amplitudes that fit in a given memory budget and thus enabling roughly
/// one additional qubit of simulation capacity.
#[derive(Clone, Copy, PartialEq)]
pub struct Complex32 {
    /// Real component.
    pub re: f32,
    /// Imaginary component.
    pub im: f32,
}

impl Complex32 {
    /// The additive identity, 0 + 0i.
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };

    /// The multiplicative identity, 1 + 0i.
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };

    /// The imaginary unit, 0 + 1i.
    pub const I: Self = Self { re: 0.0, im: 1.0 };

    /// Create a new complex number from real and imaginary parts.
    #[inline]
    pub fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    /// Squared magnitude: |z|^2 = re^2 + im^2.
    #[inline]
    pub fn norm_sq(&self) -> f32 {
        self.re * self.re + self.im * self.im
    }

    /// Magnitude: |z|.
    #[inline]
    pub fn norm(&self) -> f32 {
        self.norm_sq().sqrt()
    }

    /// Complex conjugate: conj(a + bi) = a - bi.
    #[inline]
    pub fn conj(&self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    /// Convert from an f64 `Complex` by narrowing each component to f32.
    #[inline]
    pub fn from_f64(c: &Complex) -> Self {
        Self {
            re: c.re as f32,
            im: c.im as f32,
        }
    }

    /// Convert to an f64 `Complex` by widening each component to f64.
    #[inline]
    pub fn to_f64(&self) -> Complex {
        Complex {
            re: self.re as f64,
            im: self.im as f64,
        }
    }
}

// ---------------------------------------------------------------------------
// Arithmetic trait implementations for Complex32
// ---------------------------------------------------------------------------

impl Add for Complex32 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl Sub for Complex32 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl Mul for Complex32 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl Neg for Complex32 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

impl AddAssign for Complex32 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl Mul<f32> for Complex32 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl fmt::Debug for Complex32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.re, self.im)
    }
}

impl fmt::Display for Complex32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.im >= 0.0 {
            write!(f, "{}+{}i", self.re, self.im)
        } else {
            write!(f, "{}{}i", self.re, self.im)
        }
    }
}

// ---------------------------------------------------------------------------
// QuantumStateF32
// ---------------------------------------------------------------------------

/// Maximum qubits for f32 state vector (1 more than f64 due to halved memory).
pub const MAX_QUBITS_F32: u32 = 33;

/// Quantum state using f32 precision for reduced memory usage.
///
/// Uses 8 bytes per amplitude instead of 16, enabling simulation of
/// approximately one additional qubit at each memory boundary. This is
/// intended for warm/exploratory runs; final verification can upcast to
/// the full `QuantumState` (f64) via [`QuantumStateF32::to_f64`].
pub struct QuantumStateF32 {
    amplitudes: Vec<Complex32>,
    num_qubits: u32,
    rng: StdRng,
    measurement_record: Vec<MeasurementOutcome>,
    /// Running count of gate applications, used for error bound estimation.
    gate_count: u64,
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl QuantumStateF32 {
    /// Create the |00...0> state for `num_qubits` qubits using f32 precision.
    pub fn new(num_qubits: u32) -> Result<Self> {
        if num_qubits == 0 {
            return Err(QuantumError::CircuitError(
                "cannot create quantum state with 0 qubits".into(),
            ));
        }
        if num_qubits > MAX_QUBITS_F32 {
            return Err(QuantumError::QubitLimitExceeded {
                requested: num_qubits,
                maximum: MAX_QUBITS_F32,
            });
        }
        let n = 1usize << num_qubits;
        let mut amplitudes = vec![Complex32::ZERO; n];
        amplitudes[0] = Complex32::ONE;
        Ok(Self {
            amplitudes,
            num_qubits,
            rng: StdRng::from_entropy(),
            measurement_record: Vec::new(),
            gate_count: 0,
        })
    }

    /// Create the |00...0> state with a deterministic seed for reproducibility.
    pub fn new_with_seed(num_qubits: u32, seed: u64) -> Result<Self> {
        if num_qubits == 0 {
            return Err(QuantumError::CircuitError(
                "cannot create quantum state with 0 qubits".into(),
            ));
        }
        if num_qubits > MAX_QUBITS_F32 {
            return Err(QuantumError::QubitLimitExceeded {
                requested: num_qubits,
                maximum: MAX_QUBITS_F32,
            });
        }
        let n = 1usize << num_qubits;
        let mut amplitudes = vec![Complex32::ZERO; n];
        amplitudes[0] = Complex32::ONE;
        Ok(Self {
            amplitudes,
            num_qubits,
            rng: StdRng::seed_from_u64(seed),
            measurement_record: Vec::new(),
            gate_count: 0,
        })
    }

    /// Downcast from an f64 `QuantumState`, narrowing each amplitude to f32.
    ///
    /// The measurement record is cloned from the source state.
    pub fn from_f64(state: &crate::state::QuantumState) -> Self {
        let amplitudes: Vec<Complex32> = state
            .state_vector()
            .iter()
            .map(|c| Complex32::from_f64(c))
            .collect();
        Self {
            num_qubits: state.num_qubits(),
            amplitudes,
            rng: StdRng::from_entropy(),
            measurement_record: state.measurement_record().to_vec(),
            gate_count: 0,
        }
    }

    /// Upcast to an f64 `QuantumState` for high-precision verification.
    ///
    /// Each f32 amplitude is widened to f64. The measurement record is
    /// **not** transferred since the f64 state is typically used for fresh
    /// verification runs.
    pub fn to_f64(&self) -> Result<crate::state::QuantumState> {
        let amps: Vec<Complex> = self.amplitudes.iter().map(|c| c.to_f64()).collect();
        crate::state::QuantumState::from_amplitudes(amps, self.num_qubits)
    }

    // -------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------

    /// Number of qubits in this state.
    pub fn num_qubits(&self) -> u32 {
        self.num_qubits
    }

    /// Number of amplitudes (2^num_qubits).
    pub fn num_amplitudes(&self) -> usize {
        self.amplitudes.len()
    }

    /// Compute |amplitude|^2 for each basis state.
    ///
    /// Probabilities are returned as f64 for downstream accuracy: the f32
    /// norm-squared values are widened before being returned.
    pub fn probabilities(&self) -> Vec<f64> {
        self.amplitudes.iter().map(|a| a.norm_sq() as f64).collect()
    }

    /// Estimated memory in bytes for an f32 state of `num_qubits` qubits.
    ///
    /// Each amplitude is 8 bytes (two f32 values).
    pub fn estimate_memory(num_qubits: u32) -> usize {
        (1usize << num_qubits) * std::mem::size_of::<Complex32>()
    }

    /// Returns the record of measurements performed on this state.
    pub fn measurement_record(&self) -> &[MeasurementOutcome] {
        &self.measurement_record
    }

    /// Rough upper-bound estimate of accumulated floating-point error from
    /// using f32 instead of f64.
    ///
    /// Each gate application introduces approximately `f32::EPSILON` (~1.2e-7)
    /// of relative error per amplitude. Over `g` gates this compounds to
    /// roughly `g * eps`. This is a conservative, heuristic bound.
    pub fn precision_error_bound(&self) -> f64 {
        (self.gate_count as f64) * (f32::EPSILON as f64)
    }

    // -------------------------------------------------------------------
    // Gate dispatch
    // -------------------------------------------------------------------

    /// Apply a gate to the state, returning any measurement outcomes.
    ///
    /// The gate's f64 matrices are converted to f32 before application.
    pub fn apply_gate(&mut self, gate: &Gate) -> Result<Vec<MeasurementOutcome>> {
        // Validate qubit indices.
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
                let matrix_f64 = gate.matrix_2q().unwrap();
                let matrix = convert_matrix_2q(&matrix_f64);
                self.apply_two_qubit_gate(*q1, *q2, &matrix);
                self.gate_count += 1;
                Ok(vec![])
            }

            // Everything else must be a single-qubit unitary.
            other => {
                if let Some(matrix_f64) = other.matrix_1q() {
                    let q = other.qubits()[0];
                    let matrix = convert_matrix_1q(&matrix_f64);
                    self.apply_single_qubit_gate(q, &matrix);
                    self.gate_count += 1;
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
    /// versus 1 (index `j = i + step`), the matrix transformation is applied.
    pub fn apply_single_qubit_gate(&mut self, qubit: QubitIndex, matrix: &[[Complex32; 2]; 2]) {
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
        matrix: &[[Complex32; 4]; 4],
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
    /// 1. Compute P(qubit = 0) using f32 arithmetic.
    /// 2. Sample the outcome.
    /// 3. Collapse the state vector (zero out the other branch).
    /// 4. Renormalise.
    ///
    /// The probability stored in the returned `MeasurementOutcome` is widened
    /// to f64 for compatibility with the rest of the engine.
    pub fn measure(&mut self, qubit: QubitIndex) -> Result<MeasurementOutcome> {
        self.validate_qubit(qubit)?;

        let qubit_bit = 1usize << qubit;
        let n = self.amplitudes.len();

        // Probability of measuring |0> (accumulated in f32).
        let mut p0: f32 = 0.0;
        for i in 0..n {
            if i & qubit_bit == 0 {
                p0 += self.amplitudes[i].norm_sq();
            }
        }

        let random: f64 = self.rng.gen();
        let result = random >= p0 as f64; // true => measured |1>
        let prob_f32 = if result { 1.0_f32 - p0 } else { p0 };

        // Guard against division by zero (degenerate state).
        let norm_factor = if prob_f32 > 0.0 {
            1.0_f32 / prob_f32.sqrt()
        } else {
            0.0_f32
        };

        // Collapse + renormalise.
        for i in 0..n {
            let bit_is_one = i & qubit_bit != 0;
            if bit_is_one == result {
                self.amplitudes[i] = self.amplitudes[i] * norm_factor;
            } else {
                self.amplitudes[i] = Complex32::ZERO;
            }
        }

        let outcome = MeasurementOutcome {
            qubit,
            result,
            probability: prob_f32 as f64,
        };
        self.measurement_record.push(outcome.clone());
        Ok(outcome)
    }

    // -------------------------------------------------------------------
    // Reset
    // -------------------------------------------------------------------

    /// Reset a qubit to |0>.
    ///
    /// Implemented as "measure, then flip if result was |1>".
    fn reset_qubit(&mut self, qubit: QubitIndex) -> Result<()> {
        let outcome = self.measure(qubit)?;
        if outcome.result {
            // Qubit collapsed to |1>; apply X to bring it back to |0>.
            let x_matrix_f64 = Gate::X(qubit).matrix_1q().unwrap();
            let x_matrix = convert_matrix_1q(&x_matrix_f64);
            self.apply_single_qubit_gate(qubit, &x_matrix);
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Validate that a qubit index is within range.
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

// ---------------------------------------------------------------------------
// Matrix conversion helpers (f64 -> f32)
// ---------------------------------------------------------------------------

/// Convert a 2x2 f64 gate matrix to f32.
fn convert_matrix_1q(m: &[[Complex; 2]; 2]) -> [[Complex32; 2]; 2] {
    [
        [Complex32::from_f64(&m[0][0]), Complex32::from_f64(&m[0][1])],
        [Complex32::from_f64(&m[1][0]), Complex32::from_f64(&m[1][1])],
    ]
}

/// Convert a 4x4 f64 gate matrix to f32.
fn convert_matrix_2q(m: &[[Complex; 4]; 4]) -> [[Complex32; 4]; 4] {
    [
        [
            Complex32::from_f64(&m[0][0]),
            Complex32::from_f64(&m[0][1]),
            Complex32::from_f64(&m[0][2]),
            Complex32::from_f64(&m[0][3]),
        ],
        [
            Complex32::from_f64(&m[1][0]),
            Complex32::from_f64(&m[1][1]),
            Complex32::from_f64(&m[1][2]),
            Complex32::from_f64(&m[1][3]),
        ],
        [
            Complex32::from_f64(&m[2][0]),
            Complex32::from_f64(&m[2][1]),
            Complex32::from_f64(&m[2][2]),
            Complex32::from_f64(&m[2][3]),
        ],
        [
            Complex32::from_f64(&m[3][0]),
            Complex32::from_f64(&m[3][1]),
            Complex32::from_f64(&m[3][2]),
            Complex32::from_f64(&m[3][3]),
        ],
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-6;

    fn approx_eq_f32(a: f32, b: f32) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn complex32_arithmetic() {
        let a = Complex32::new(1.0, 2.0);
        let b = Complex32::new(3.0, -1.0);

        let sum = a + b;
        assert!(approx_eq_f32(sum.re, 4.0));
        assert!(approx_eq_f32(sum.im, 1.0));

        let diff = a - b;
        assert!(approx_eq_f32(diff.re, -2.0));
        assert!(approx_eq_f32(diff.im, 3.0));

        // (1+2i)*(3-i) = 3 - i + 6i - 2i^2 = 3 + 5i + 2 = 5 + 5i
        let prod = a * b;
        assert!(approx_eq_f32(prod.re, 5.0));
        assert!(approx_eq_f32(prod.im, 5.0));

        let neg = -a;
        assert!(approx_eq_f32(neg.re, -1.0));
        assert!(approx_eq_f32(neg.im, -2.0));

        assert!(approx_eq_f32(a.norm_sq(), 5.0));
        assert!(approx_eq_f32(a.conj().im, -2.0));
    }

    #[test]
    fn complex32_f64_conversion() {
        let c64 = Complex::new(1.5, -2.5);
        let c32 = Complex32::from_f64(&c64);
        assert!(approx_eq_f32(c32.re, 1.5));
        assert!(approx_eq_f32(c32.im, -2.5));

        let back = c32.to_f64();
        assert!((back.re - 1.5).abs() < 1e-6);
        assert!((back.im - (-2.5)).abs() < 1e-6);
    }

    #[test]
    fn state_f32_creation() {
        let state = QuantumStateF32::new(3).unwrap();
        assert_eq!(state.num_qubits(), 3);
        assert_eq!(state.num_amplitudes(), 8);

        let probs = state.probabilities();
        assert!((probs[0] - 1.0).abs() < 1e-6);
        for &p in &probs[1..] {
            assert!(p.abs() < 1e-6);
        }
    }

    #[test]
    fn state_f32_zero_qubits_error() {
        assert!(QuantumStateF32::new(0).is_err());
    }

    #[test]
    fn state_f32_memory_estimate() {
        // 3 qubits -> 8 amplitudes * 8 bytes = 64 bytes
        assert_eq!(QuantumStateF32::estimate_memory(3), 64);
        // 10 qubits -> 1024 amplitudes * 8 bytes = 8192 bytes
        assert_eq!(QuantumStateF32::estimate_memory(10), 8192);
    }

    #[test]
    fn state_f32_h_gate() {
        let mut state = QuantumStateF32::new_with_seed(1, 42).unwrap();
        state.apply_gate(&Gate::H(0)).unwrap();

        let probs = state.probabilities();
        assert!((probs[0] - 0.5).abs() < 1e-5);
        assert!((probs[1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn state_f32_bell_state() {
        let mut state = QuantumStateF32::new_with_seed(2, 42).unwrap();
        state.apply_gate(&Gate::H(0)).unwrap();
        state.apply_gate(&Gate::CNOT(0, 1)).unwrap();

        let probs = state.probabilities();
        // Bell state: |00> + |11>, each with probability 0.5
        assert!((probs[0] - 0.5).abs() < 1e-5);
        assert!(probs[1].abs() < 1e-5);
        assert!(probs[2].abs() < 1e-5);
        assert!((probs[3] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn state_f32_measurement() {
        let mut state = QuantumStateF32::new_with_seed(1, 42).unwrap();
        state.apply_gate(&Gate::X(0)).unwrap();

        let outcome = state.measure(0).unwrap();
        assert!(outcome.result); // Must be |1> with certainty
        assert!((outcome.probability - 1.0).abs() < 1e-5);
        assert_eq!(state.measurement_record().len(), 1);
    }

    #[test]
    fn state_f32_from_f64_roundtrip() {
        let f64_state = crate::state::QuantumState::new_with_seed(3, 99).unwrap();
        let f32_state = QuantumStateF32::from_f64(&f64_state);
        assert_eq!(f32_state.num_qubits(), 3);
        assert_eq!(f32_state.num_amplitudes(), 8);

        // Upcast back and check probabilities are close.
        let back = f32_state.to_f64().unwrap();
        let p_orig = f64_state.probabilities();
        let p_back = back.probabilities();
        for (a, b) in p_orig.iter().zip(p_back.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn state_f32_precision_error_bound() {
        let mut state = QuantumStateF32::new_with_seed(2, 42).unwrap();
        assert_eq!(state.precision_error_bound(), 0.0);

        state.apply_gate(&Gate::H(0)).unwrap();
        state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
        // 2 gates applied
        let bound = state.precision_error_bound();
        assert!(bound > 0.0);
        assert!(bound < 1e-5); // Should be very small for 2 gates
    }

    #[test]
    fn state_f32_invalid_qubit() {
        let mut state = QuantumStateF32::new(2).unwrap();
        assert!(state.apply_gate(&Gate::H(5)).is_err());
    }

    #[test]
    fn state_f32_distinct_qubits_check() {
        let mut state = QuantumStateF32::new(2).unwrap();
        assert!(state.apply_gate(&Gate::CNOT(0, 0)).is_err());
    }
}
