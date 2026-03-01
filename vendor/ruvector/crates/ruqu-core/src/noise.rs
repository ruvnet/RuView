//! Enhanced noise models for realistic quantum simulation.
//!
//! This module provides Kraus-operator-based noise channels (depolarizing,
//! amplitude damping, phase damping, thermal relaxation), device calibration
//! data, readout-error modelling, and measurement-error mitigation via
//! confusion-matrix inversion.

use crate::types::Complex;
use rand::Rng;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Device calibration data
// ---------------------------------------------------------------------------

/// Hardware-specific calibration parameters obtained from a real device.
#[derive(Debug, Clone)]
pub struct DeviceCalibration {
    /// T1 relaxation times in microseconds, indexed by qubit.
    pub qubit_t1: Vec<f64>,
    /// T2 dephasing times in microseconds, indexed by qubit.
    pub qubit_t2: Vec<f64>,
    /// Readout error rates per qubit: (p01, p10) where p01 is the
    /// probability of reading 1 when the state is 0, and p10 is the
    /// probability of reading 0 when the state is 1.
    pub readout_error: Vec<(f64, f64)>,
    /// Gate error rates keyed by gate name (e.g. "cx_0_1", "sx_0").
    pub gate_errors: HashMap<String, f64>,
    /// Gate durations in microseconds keyed by gate name.
    pub gate_times: HashMap<String, f64>,
    /// Connectivity graph: pairs of physically connected qubits.
    pub coupling_map: Vec<(u32, u32)>,
}

// ---------------------------------------------------------------------------
// Thermal relaxation parameters
// ---------------------------------------------------------------------------

/// Parameters for a combined T1/T2 thermal-relaxation channel.
#[derive(Debug, Clone, Copy)]
pub struct ThermalRelaxation {
    /// T1 time (amplitude damping timescale) in microseconds.
    pub t1: f64,
    /// T2 time (dephasing timescale) in microseconds. Must satisfy T2 <= 2*T1.
    pub t2: f64,
    /// Duration of the gate in microseconds.
    pub gate_time: f64,
}

// ---------------------------------------------------------------------------
// Enhanced noise model
// ---------------------------------------------------------------------------

/// A composable noise model supporting multiple physical error channels.
#[derive(Debug, Clone)]
pub struct EnhancedNoiseModel {
    /// Per-gate single-qubit depolarizing error rate.
    pub depolarizing_rate: f64,
    /// Per-gate two-qubit depolarizing error rate.
    pub two_qubit_depolarizing_rate: f64,
    /// Amplitude damping parameter (gamma) derived from T1 decay.
    pub amplitude_damping_gamma: Option<f64>,
    /// Phase damping parameter (lambda) derived from T2 dephasing.
    pub phase_damping_lambda: Option<f64>,
    /// Readout error probabilities (p01, p10).
    pub readout_error: Option<(f64, f64)>,
    /// Thermal relaxation channel parameters.
    pub thermal_relaxation: Option<ThermalRelaxation>,
    /// ZZ crosstalk coupling strength between neighbouring qubits.
    pub crosstalk_zz: Option<f64>,
}

impl Default for EnhancedNoiseModel {
    fn default() -> Self {
        Self {
            depolarizing_rate: 0.0,
            two_qubit_depolarizing_rate: 0.0,
            amplitude_damping_gamma: None,
            phase_damping_lambda: None,
            readout_error: None,
            thermal_relaxation: None,
            crosstalk_zz: None,
        }
    }
}

impl EnhancedNoiseModel {
    /// Construct an `EnhancedNoiseModel` from device calibration data for a
    /// specific gate acting on a specific qubit.
    ///
    /// The gate name is used to look up error rates and durations. The qubit
    /// index selects per-qubit T1, T2, and readout-error values.
    pub fn from_calibration(cal: &DeviceCalibration, gate_name: &str, qubit: u32) -> Self {
        let idx = qubit as usize;

        // Gate error rate becomes the depolarizing rate.
        let depolarizing_rate = cal.gate_errors.get(gate_name).copied().unwrap_or(0.0);

        // Gate duration (needed for thermal relaxation conversion).
        let gate_time = cal.gate_times.get(gate_name).copied().unwrap_or(0.0);

        // T1 and T2 values for this qubit.
        let t1 = cal.qubit_t1.get(idx).copied().unwrap_or(f64::INFINITY);
        let t2 = cal.qubit_t2.get(idx).copied().unwrap_or(f64::INFINITY);

        // Derive amplitude-damping gamma = 1 - exp(-gate_time / T1).
        let amplitude_damping_gamma = if t1.is_finite() && t1 > 0.0 && gate_time > 0.0 {
            Some(1.0 - (-gate_time / t1).exp())
        } else {
            None
        };

        // Derive phase-damping lambda.
        // Pure dephasing rate: 1/T_phi = 1/T2 - 1/(2*T1).
        // lambda = 1 - exp(-gate_time / T_phi) when T_phi > 0.
        let phase_damping_lambda = if t2.is_finite() && t2 > 0.0 && gate_time > 0.0 {
            let inv_t_phi = (1.0 / t2) - (1.0 / (2.0 * t1));
            if inv_t_phi > 0.0 {
                Some(1.0 - (-gate_time * inv_t_phi).exp())
            } else {
                None
            }
        } else {
            None
        };

        // Readout errors for this qubit.
        let readout_error = cal.readout_error.get(idx).copied();

        // Thermal relaxation if we have valid T1, T2, gate_time.
        let thermal_relaxation =
            if t1.is_finite() && t2.is_finite() && t1 > 0.0 && t2 > 0.0 && gate_time > 0.0 {
                Some(ThermalRelaxation { t1, t2, gate_time })
            } else {
                None
            };

        Self {
            depolarizing_rate,
            two_qubit_depolarizing_rate: 0.0,
            amplitude_damping_gamma,
            phase_damping_lambda,
            readout_error,
            thermal_relaxation,
            crosstalk_zz: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Kraus operator sets
// ---------------------------------------------------------------------------

/// Identity matrix as a 2x2 complex array.
const IDENTITY: [[Complex; 2]; 2] = [[Complex::ONE, Complex::ZERO], [Complex::ZERO, Complex::ONE]];

/// Depolarizing channel Kraus operators.
///
/// The channel is E(rho) = (1 - p) rho + (p/3)(X rho X + Y rho Y + Z rho Z).
///
/// Kraus representation:
///   K0 = sqrt(1 - p) I
///   K1 = sqrt(p/3)   X
///   K2 = sqrt(p/3)   Y
///   K3 = sqrt(p/3)   Z
pub fn depolarizing_kraus(p: f64) -> Vec<[[Complex; 2]; 2]> {
    let s0 = (1.0 - p).max(0.0).sqrt();
    let sp = (p / 3.0).max(0.0).sqrt();

    let c = |v: f64| Complex::new(v, 0.0);

    // K0 = sqrt(1-p) * I
    let k0 = [[c(s0), Complex::ZERO], [Complex::ZERO, c(s0)]];

    // K1 = sqrt(p/3) * X
    let k1 = [[Complex::ZERO, c(sp)], [c(sp), Complex::ZERO]];

    // K2 = sqrt(p/3) * Y = sqrt(p/3) * [[0, -i],[i, 0]]
    let k2 = [
        [Complex::ZERO, Complex::new(0.0, -sp)],
        [Complex::new(0.0, sp), Complex::ZERO],
    ];

    // K3 = sqrt(p/3) * Z
    let k3 = [[c(sp), Complex::ZERO], [Complex::ZERO, c(-sp)]];

    vec![k0, k1, k2, k3]
}

/// Amplitude damping channel Kraus operators.
///
/// Models energy relaxation (T1 decay):
///   K0 = [[1, 0], [0, sqrt(1-gamma)]]
///   K1 = [[0, sqrt(gamma)], [0, 0]]
///
/// gamma = 1 - exp(-gate_time / T1).
pub fn amplitude_damping_kraus(gamma: f64) -> Vec<[[Complex; 2]; 2]> {
    let sg = gamma.max(0.0).min(1.0).sqrt();
    let s1g = (1.0 - gamma).max(0.0).sqrt();

    let c = |v: f64| Complex::new(v, 0.0);

    let k0 = [[Complex::ONE, Complex::ZERO], [Complex::ZERO, c(s1g)]];

    let k1 = [[Complex::ZERO, c(sg)], [Complex::ZERO, Complex::ZERO]];

    vec![k0, k1]
}

/// Phase damping channel Kraus operators.
///
/// Models pure dephasing (T2 process beyond T1):
///   K0 = [[1, 0], [0, sqrt(1-lambda)]]
///   K1 = [[0, 0], [0, sqrt(lambda)]]
///
/// lambda = 1 - exp(-gate_time / T_phi) where 1/T_phi = 1/T2 - 1/(2*T1).
pub fn phase_damping_kraus(lambda: f64) -> Vec<[[Complex; 2]; 2]> {
    let sl = lambda.max(0.0).min(1.0).sqrt();
    let s1l = (1.0 - lambda).max(0.0).sqrt();

    let c = |v: f64| Complex::new(v, 0.0);

    let k0 = [[Complex::ONE, Complex::ZERO], [Complex::ZERO, c(s1l)]];

    let k1 = [[Complex::ZERO, Complex::ZERO], [Complex::ZERO, c(sl)]];

    vec![k0, k1]
}

/// Thermal relaxation channel Kraus operators.
///
/// Combines amplitude damping and phase damping from T1 and T2 parameters.
///
/// When T2 <= T1 (the "non-degenerate" regime, which encompasses most
/// physical devices where T2 <= 2*T1), we decompose the channel as:
///   - Amplitude damping with gamma = 1 - exp(-gate_time / T1)
///   - Followed by phase damping with an effective lambda derived from
///     the residual dephasing after accounting for T1.
///
/// The combined Kraus operators are:
///   For each (Ki from AD) x (Kj from PD), emit Ki * Kj.
///
/// When T2 > T1 but T2 <= 2*T1, we still produce a valid channel by
/// clamping the effective dephasing.
pub fn thermal_relaxation_kraus(t1: f64, t2: f64, gate_time: f64) -> Vec<[[Complex; 2]; 2]> {
    // Edge case: zero gate time means no decoherence.
    if gate_time <= 0.0 || t1 <= 0.0 {
        return vec![IDENTITY];
    }

    // Amplitude damping parameter.
    let gamma = 1.0 - (-gate_time / t1).exp();

    // Effective T2 clamped to physical bound: T2 <= 2*T1.
    let t2_eff = t2.min(2.0 * t1);

    // Pure dephasing rate: 1/T_phi = 1/T2 - 1/(2*T1).
    let inv_t_phi = if t2_eff > 0.0 {
        (1.0 / t2_eff) - (1.0 / (2.0 * t1))
    } else {
        0.0
    };

    let lambda = if inv_t_phi > 0.0 {
        1.0 - (-gate_time * inv_t_phi).exp()
    } else {
        0.0
    };

    // Get the individual Kraus sets.
    let ad_ops = amplitude_damping_kraus(gamma);
    let pd_ops = phase_damping_kraus(lambda);

    // Combine: K_combined = K_ad * K_pd (matrix product).
    let mut combined = Vec::with_capacity(ad_ops.len() * pd_ops.len());
    for ad in &ad_ops {
        for pd in &pd_ops {
            combined.push(mat_mul_2x2(ad, pd));
        }
    }

    combined
}

// ---------------------------------------------------------------------------
// Readout error
// ---------------------------------------------------------------------------

/// Apply a classical readout error to a measurement outcome.
///
/// - If the true outcome is `false` (|0>), flip to `true` with probability `p01`.
/// - If the true outcome is `true` (|1>), flip to `false` with probability `p10`.
pub fn apply_readout_error(outcome: bool, p01: f64, p10: f64, rng: &mut impl Rng) -> bool {
    let r: f64 = rng.gen();
    if outcome {
        // True outcome is |1>; flip to |0> with probability p10.
        if r < p10 {
            false
        } else {
            true
        }
    } else {
        // True outcome is |0>; flip to |1> with probability p01.
        if r < p01 {
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Readout error mitigation
// ---------------------------------------------------------------------------

/// Measurement error mitigator that applies inverse-confusion-matrix correction
/// to raw shot counts.
///
/// For up to 12 qubits the full 2^n x 2^n confusion matrix is built and
/// inverted via least-squares (Gaussian elimination). Beyond 12 qubits a
/// tensor-product approximation is used where each qubit's 2x2 confusion
/// matrix is inverted independently and the correction is applied per-qubit.
#[derive(Debug, Clone)]
pub struct ReadoutCorrector {
    /// Per-qubit readout error rates (p01, p10).
    readout_errors: Vec<(f64, f64)>,
    /// Number of qubits.
    num_qubits: usize,
}

impl ReadoutCorrector {
    /// Build a new corrector from per-qubit readout error rates.
    pub fn new(readout_errors: &[(f64, f64)]) -> Self {
        Self {
            readout_errors: readout_errors.to_vec(),
            num_qubits: readout_errors.len(),
        }
    }

    /// Correct raw measurement counts using inverse confusion matrix.
    ///
    /// Returns floating-point corrected counts (may be non-integer due to the
    /// linear algebra involved). Negative corrected values are clamped to zero.
    pub fn correct_counts(&self, counts: &HashMap<Vec<bool>, usize>) -> HashMap<Vec<bool>, f64> {
        if self.num_qubits == 0 {
            return counts.iter().map(|(k, &v)| (k.clone(), v as f64)).collect();
        }

        if self.num_qubits <= 12 {
            self.correct_full_matrix(counts)
        } else {
            self.correct_tensor_product(counts)
        }
    }

    /// Full confusion-matrix inversion for small qubit counts.
    fn correct_full_matrix(&self, counts: &HashMap<Vec<bool>, usize>) -> HashMap<Vec<bool>, f64> {
        let n = self.num_qubits;
        let dim = 1usize << n;

        // Build the confusion matrix A where A[measured][true] = P(measured | true).
        // A = A_0 (x) A_1 (x) ... (x) A_{n-1}   (tensor product of 2x2 matrices).
        let confusion = self.build_confusion_matrix(dim, n);

        // Build the raw count vector (indexed by bitstring as integer).
        let mut raw_vec = vec![0.0f64; dim];
        for (bits, &count) in counts {
            let idx = bits_to_index(bits, n);
            raw_vec[idx] = count as f64;
        }

        // Solve A * corrected = raw via Gaussian elimination (least-squares).
        let corrected_vec = solve_linear_system(&confusion, &raw_vec, dim);

        // Convert back to HashMap, clamping negatives to zero.
        let mut result = HashMap::new();
        for i in 0..dim {
            let val = corrected_vec[i].max(0.0);
            if val > 1e-10 {
                let bits = index_to_bits(i, n);
                result.insert(bits, val);
            }
        }
        result
    }

    /// Tensor-product approximation for large qubit counts.
    ///
    /// Each qubit's 2x2 confusion matrix is inverted independently, then the
    /// correction is applied qubit-by-qubit via iterative rescaling.
    fn correct_tensor_product(
        &self,
        counts: &HashMap<Vec<bool>, usize>,
    ) -> HashMap<Vec<bool>, f64> {
        let n = self.num_qubits;

        // Compute the inverse 2x2 confusion matrix for each qubit.
        let inv_matrices: Vec<[[f64; 2]; 2]> = self
            .readout_errors
            .iter()
            .map(|&(p01, p10)| invert_2x2_confusion(p01, p10))
            .collect();

        // Start with raw counts as floats.
        let mut corrected: HashMap<Vec<bool>, f64> =
            counts.iter().map(|(k, &v)| (k.clone(), v as f64)).collect();

        // Apply each qubit's inverse confusion matrix independently.
        // For each qubit q, we group bitstrings by all bits except q,
        // then apply the 2x2 inverse to the pair (count_with_q=0, count_with_q=1).
        for q in 0..n {
            let inv = &inv_matrices[q];
            let mut new_corrected: HashMap<Vec<bool>, f64> = HashMap::new();

            // Collect all unique bitstrings that appear, paired by qubit q.
            let keys: Vec<Vec<bool>> = corrected.keys().cloned().collect();
            let mut processed: std::collections::HashSet<Vec<bool>> =
                std::collections::HashSet::new();

            for bits in &keys {
                if processed.contains(bits) {
                    continue;
                }

                // Create the partner bitstring (same except bit q is flipped).
                let mut partner = bits.clone();
                partner[q] = !partner[q];

                processed.insert(bits.clone());
                processed.insert(partner.clone());

                let val_this = corrected.get(bits).copied().unwrap_or(0.0);
                let val_partner = corrected.get(&partner).copied().unwrap_or(0.0);

                // Determine which is the q=0 case and which is q=1.
                let (val_0, val_1, bits_0, bits_1) = if !bits[q] {
                    (val_this, val_partner, bits.clone(), partner.clone())
                } else {
                    (val_partner, val_this, partner.clone(), bits.clone())
                };

                // Apply inverse confusion: [c0', c1'] = inv * [c0, c1]
                let new_0 = inv[0][0] * val_0 + inv[0][1] * val_1;
                let new_1 = inv[1][0] * val_0 + inv[1][1] * val_1;

                if new_0.abs() > 1e-10 {
                    new_corrected.insert(bits_0, new_0.max(0.0));
                }
                if new_1.abs() > 1e-10 {
                    new_corrected.insert(bits_1, new_1.max(0.0));
                }
            }

            corrected = new_corrected;
        }

        corrected
    }

    /// Build the full 2^n x 2^n confusion matrix via tensor product of per-qubit
    /// 2x2 confusion matrices.
    fn build_confusion_matrix(&self, dim: usize, n: usize) -> Vec<Vec<f64>> {
        let mut confusion = vec![vec![0.0f64; dim]; dim];

        for true_state in 0..dim {
            for measured_state in 0..dim {
                let mut prob = 1.0;
                for q in 0..n {
                    let true_bit = (true_state >> q) & 1;
                    let meas_bit = (measured_state >> q) & 1;
                    let (p01, p10) = self.readout_errors[q];

                    // P(meas_bit | true_bit)
                    prob *= match (true_bit, meas_bit) {
                        (0, 0) => 1.0 - p01,
                        (0, 1) => p01,
                        (1, 0) => p10,
                        (1, 1) => 1.0 - p10,
                        _ => unreachable!(),
                    };
                }
                confusion[measured_state][true_state] = prob;
            }
        }

        confusion
    }
}

// ---------------------------------------------------------------------------
// Helper: 2x2 matrix multiplication for Complex
// ---------------------------------------------------------------------------

/// Multiply two 2x2 complex matrices.
fn mat_mul_2x2(a: &[[Complex; 2]; 2], b: &[[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
    [
        [
            a[0][0] * b[0][0] + a[0][1] * b[1][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1],
        ],
        [
            a[1][0] * b[0][0] + a[1][1] * b[1][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1],
        ],
    ]
}

/// Compute the conjugate transpose (dagger) of a 2x2 complex matrix.
#[cfg(test)]
fn dagger_2x2(m: &[[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
    [
        [m[0][0].conj(), m[1][0].conj()],
        [m[0][1].conj(), m[1][1].conj()],
    ]
}

// ---------------------------------------------------------------------------
// Helper: bitstring <-> index conversion
// ---------------------------------------------------------------------------

/// Convert a boolean bitstring to an integer index.
/// bits[0] is the least significant bit.
fn bits_to_index(bits: &[bool], n: usize) -> usize {
    let mut idx = 0usize;
    for q in 0..n.min(bits.len()) {
        if bits[q] {
            idx |= 1 << q;
        }
    }
    idx
}

/// Convert an integer index to a boolean bitstring of length n.
fn index_to_bits(idx: usize, n: usize) -> Vec<bool> {
    (0..n).map(|q| (idx >> q) & 1 == 1).collect()
}

// ---------------------------------------------------------------------------
// Helper: invert a 2x2 confusion matrix
// ---------------------------------------------------------------------------

/// Invert the 2x2 confusion matrix for a single qubit:
///   [[1-p01, p10],
///    [p01,   1-p10]]
///
/// Returns the inverse as a 2x2 array of f64.
fn invert_2x2_confusion(p01: f64, p10: f64) -> [[f64; 2]; 2] {
    let a = 1.0 - p01;
    let b = p10;
    let c = p01;
    let d = 1.0 - p10;

    let det = a * d - b * c;
    if det.abs() < 1e-15 {
        // Singular matrix -- return identity as fallback.
        return [[1.0, 0.0], [0.0, 1.0]];
    }

    let inv_det = 1.0 / det;
    [[d * inv_det, -b * inv_det], [-c * inv_det, a * inv_det]]
}

// ---------------------------------------------------------------------------
// Helper: solve linear system via Gaussian elimination with partial pivoting
// ---------------------------------------------------------------------------

/// Solve A * x = b for x using Gaussian elimination with partial pivoting.
///
/// A is a dim x dim matrix, b is a dim-length vector.
/// Returns the solution vector x.
fn solve_linear_system(a: &[Vec<f64>], b: &[f64], dim: usize) -> Vec<f64> {
    // Build augmented matrix [A | b].
    let mut aug: Vec<Vec<f64>> = Vec::with_capacity(dim);
    for i in 0..dim {
        let mut row = Vec::with_capacity(dim + 1);
        row.extend_from_slice(&a[i]);
        row.push(b[i]);
        aug.push(row);
    }

    // Forward elimination with partial pivoting.
    for col in 0..dim {
        // Find pivot.
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..dim {
            let val = aug[row][col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        // Swap rows.
        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        if pivot.abs() < 1e-15 {
            continue; // Skip singular column.
        }

        // Eliminate below.
        for row in (col + 1)..dim {
            let factor = aug[row][col] / pivot;
            for j in col..=dim {
                let val = aug[col][j];
                aug[row][j] -= factor * val;
            }
        }
    }

    // Back substitution.
    let mut x = vec![0.0f64; dim];
    for col in (0..dim).rev() {
        let pivot = aug[col][col];
        if pivot.abs() < 1e-15 {
            x[col] = 0.0;
            continue;
        }
        let mut sum = aug[col][dim];
        for j in (col + 1)..dim {
            sum -= aug[col][j] * x[j];
        }
        x[col] = sum / pivot;
    }

    x
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    /// Helper: check that sum_i Ki^dag Ki = I (trace-preserving condition).
    fn assert_trace_preserving(ops: &[[[Complex; 2]; 2]], tol: f64) {
        let mut sum = [[Complex::ZERO; 2]; 2];
        for k in ops {
            let kdag = dagger_2x2(k);
            let prod = mat_mul_2x2(&kdag, k);
            for r in 0..2 {
                for c in 0..2 {
                    sum[r][c] = sum[r][c] + prod[r][c];
                }
            }
        }
        // sum should be the identity.
        assert!(
            (sum[0][0].re - 1.0).abs() < tol,
            "sum[0][0] = {:?}, expected 1.0",
            sum[0][0]
        );
        assert!(
            sum[0][0].im.abs() < tol,
            "sum[0][0].im = {}, expected 0.0",
            sum[0][0].im
        );
        assert!(
            sum[0][1].re.abs() < tol && sum[0][1].im.abs() < tol,
            "sum[0][1] = {:?}, expected 0.0",
            sum[0][1]
        );
        assert!(
            sum[1][0].re.abs() < tol && sum[1][0].im.abs() < tol,
            "sum[1][0] = {:?}, expected 0.0",
            sum[1][0]
        );
        assert!(
            (sum[1][1].re - 1.0).abs() < tol,
            "sum[1][1] = {:?}, expected 1.0",
            sum[1][1]
        );
        assert!(
            sum[1][1].im.abs() < tol,
            "sum[1][1].im = {}, expected 0.0",
            sum[1][1].im
        );
    }

    // -------------------------------------------------------------------
    // Depolarizing channel tests
    // -------------------------------------------------------------------

    #[test]
    fn depolarizing_kraus_trace_preserving() {
        for &p in &[0.0, 0.01, 0.1, 0.5, 1.0] {
            let ops = depolarizing_kraus(p);
            assert_trace_preserving(&ops, 1e-12);
        }
    }

    #[test]
    fn depolarizing_p0_is_identity() {
        let ops = depolarizing_kraus(0.0);
        assert_eq!(ops.len(), 4);
        // K0 should be identity, K1..K3 should be zero matrices.
        let k0 = &ops[0];
        assert!((k0[0][0].re - 1.0).abs() < 1e-14);
        assert!((k0[1][1].re - 1.0).abs() < 1e-14);
        assert!(k0[0][1].norm_sq() < 1e-28);
        assert!(k0[1][0].norm_sq() < 1e-28);

        for k in &ops[1..] {
            for r in 0..2 {
                for c in 0..2 {
                    assert!(
                        k[r][c].norm_sq() < 1e-28,
                        "Non-zero element in zero Kraus op: {:?}",
                        k[r][c]
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Amplitude damping tests
    // -------------------------------------------------------------------

    #[test]
    fn amplitude_damping_kraus_trace_preserving() {
        for &gamma in &[0.0, 0.01, 0.1, 0.5, 0.99, 1.0] {
            let ops = amplitude_damping_kraus(gamma);
            assert_trace_preserving(&ops, 1e-12);
        }
    }

    #[test]
    fn amplitude_damping_gamma1_decays_one_to_zero() {
        // With gamma = 1, the |1> state should be completely mapped to |0>.
        // K0 = [[1,0],[0,0]], K1 = [[0,1],[0,0]]
        // Acting on rho = |1><1|:
        //   K0 * |1> = 0, K1 * |1> = |0>
        // So the output state is |0><0|.
        let ops = amplitude_damping_kraus(1.0);
        assert_eq!(ops.len(), 2);

        // K0 should be [[1,0],[0,0]]
        assert!((ops[0][0][0].re - 1.0).abs() < 1e-14);
        assert!(ops[0][1][1].norm_sq() < 1e-28);

        // K1 should be [[0,1],[0,0]]
        assert!((ops[1][0][1].re - 1.0).abs() < 1e-14);
        assert!(ops[1][1][0].norm_sq() < 1e-28);
        assert!(ops[1][1][1].norm_sq() < 1e-28);

        // Apply to |1> state vector: [0, 1]
        // K0 * [0,1] = [0*1+0*0, 0*0+0*1] = [0, 0]
        // K1 * [0,1] = [0*0+1*1, 0*0+0*1] = [1, 0]
        // rho_out = |0><0| -- so |1> decays completely to |0>.
        let state_one = [Complex::ZERO, Complex::ONE];
        let k1_on_one = [
            ops[1][0][0] * state_one[0] + ops[1][0][1] * state_one[1],
            ops[1][1][0] * state_one[0] + ops[1][1][1] * state_one[1],
        ];
        assert!(
            (k1_on_one[0].re - 1.0).abs() < 1e-14,
            "Expected |0> component = 1.0"
        );
        assert!(
            k1_on_one[1].norm_sq() < 1e-28,
            "Expected |1> component = 0.0"
        );
    }

    // -------------------------------------------------------------------
    // Phase damping tests
    // -------------------------------------------------------------------

    #[test]
    fn phase_damping_kraus_trace_preserving() {
        for &lambda in &[0.0, 0.01, 0.1, 0.5, 1.0] {
            let ops = phase_damping_kraus(lambda);
            assert_trace_preserving(&ops, 1e-12);
        }
    }

    #[test]
    fn phase_damping_lambda0_is_identity() {
        let ops = phase_damping_kraus(0.0);
        assert_eq!(ops.len(), 2);
        // K0 should be identity.
        assert!((ops[0][0][0].re - 1.0).abs() < 1e-14);
        assert!((ops[0][1][1].re - 1.0).abs() < 1e-14);
        // K1 should be zero.
        for r in 0..2 {
            for c in 0..2 {
                assert!(ops[1][r][c].norm_sq() < 1e-28);
            }
        }
    }

    // -------------------------------------------------------------------
    // Thermal relaxation tests
    // -------------------------------------------------------------------

    #[test]
    fn thermal_relaxation_kraus_trace_preserving() {
        let test_cases = [
            (50.0, 30.0, 0.05),  // typical: T2 < T1
            (50.0, 50.0, 0.05),  // T2 == T1
            (50.0, 100.0, 0.05), // T2 > T1 (clamped to 2*T1)
            (100.0, 80.0, 1.0),  // longer gate time
            (50.0, 30.0, 0.001), // very short gate
        ];
        for &(t1, t2, gt) in &test_cases {
            let ops = thermal_relaxation_kraus(t1, t2, gt);
            assert_trace_preserving(&ops, 1e-10);
        }
    }

    #[test]
    fn thermal_relaxation_zero_gate_time_is_identity() {
        let ops = thermal_relaxation_kraus(50.0, 30.0, 0.0);
        assert_eq!(ops.len(), 1);
        assert!((ops[0][0][0].re - 1.0).abs() < 1e-14);
        assert!((ops[0][1][1].re - 1.0).abs() < 1e-14);
    }

    // -------------------------------------------------------------------
    // Readout error tests
    // -------------------------------------------------------------------

    #[test]
    fn readout_error_no_flip_when_rates_zero() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            assert!(!apply_readout_error(false, 0.0, 0.0, &mut rng));
            assert!(apply_readout_error(true, 0.0, 0.0, &mut rng));
        }
    }

    #[test]
    fn readout_error_always_flips_when_rates_one() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            // p01 = 1.0: false always flips to true
            assert!(apply_readout_error(false, 1.0, 0.0, &mut rng));
            // p10 = 1.0: true always flips to false
            assert!(!apply_readout_error(true, 0.0, 1.0, &mut rng));
        }
    }

    #[test]
    fn readout_error_statistical_rates() {
        let mut rng = StdRng::seed_from_u64(12345);
        let p01 = 0.1;
        let p10 = 0.2;
        let trials = 100_000;

        let mut flips_01 = 0usize;
        let mut flips_10 = 0usize;

        for _ in 0..trials {
            if apply_readout_error(false, p01, p10, &mut rng) {
                flips_01 += 1;
            }
            if !apply_readout_error(true, p01, p10, &mut rng) {
                flips_10 += 1;
            }
        }

        let measured_p01 = flips_01 as f64 / trials as f64;
        let measured_p10 = flips_10 as f64 / trials as f64;

        assert!(
            (measured_p01 - p01).abs() < 0.01,
            "p01: expected ~{}, got {}",
            p01,
            measured_p01
        );
        assert!(
            (measured_p10 - p10).abs() < 0.01,
            "p10: expected ~{}, got {}",
            p10,
            measured_p10
        );
    }

    // -------------------------------------------------------------------
    // ReadoutCorrector tests
    // -------------------------------------------------------------------

    #[test]
    fn readout_corrector_identity_when_no_errors() {
        let corrector = ReadoutCorrector::new(&[(0.0, 0.0), (0.0, 0.0)]);
        let mut counts = HashMap::new();
        counts.insert(vec![false, false], 500);
        counts.insert(vec![true, true], 500);

        let corrected = corrector.correct_counts(&counts);

        assert!(
            (corrected.get(&vec![false, false]).copied().unwrap_or(0.0) - 500.0).abs() < 1e-6,
            "Expected 500.0 for |00>"
        );
        assert!(
            (corrected.get(&vec![true, true]).copied().unwrap_or(0.0) - 500.0).abs() < 1e-6,
            "Expected 500.0 for |11>"
        );
    }

    #[test]
    fn readout_corrector_corrects_known_bias() {
        // Single qubit with 10% p01 and 5% p10 error.
        // True distribution: 700 x |0> and 300 x |1>.
        // Measured distribution:
        //   meas_0 = 700*(1-0.10) + 300*0.05 = 630 + 15 = 645
        //   meas_1 = 700*0.10 + 300*(1-0.05) = 70 + 285 = 355
        let corrector = ReadoutCorrector::new(&[(0.10, 0.05)]);
        let mut counts = HashMap::new();
        counts.insert(vec![false], 645);
        counts.insert(vec![true], 355);

        let corrected = corrector.correct_counts(&counts);

        let c0 = corrected.get(&vec![false]).copied().unwrap_or(0.0);
        let c1 = corrected.get(&vec![true]).copied().unwrap_or(0.0);

        assert!((c0 - 700.0).abs() < 1.0, "Expected ~700, got {}", c0);
        assert!((c1 - 300.0).abs() < 1.0, "Expected ~300, got {}", c1);
    }

    #[test]
    fn readout_corrector_two_qubit_correction() {
        // Two qubits, each with p01=0.05, p10=0.03.
        // True: 1000 x |00>.
        // Measured: P(00|00) = (1-0.05)^2 = 0.9025 -> 902.5
        //           P(01|00) = (1-0.05)*0.05 = 0.0475 -> 47.5
        //           P(10|00) = 0.05*(1-0.05) = 0.0475 -> 47.5
        //           P(11|00) = 0.05*0.05 = 0.0025 -> 2.5
        let corrector = ReadoutCorrector::new(&[(0.05, 0.03), (0.05, 0.03)]);
        let mut counts = HashMap::new();
        counts.insert(vec![false, false], 903);
        counts.insert(vec![true, false], 47);
        counts.insert(vec![false, true], 48);
        counts.insert(vec![true, true], 2);

        let corrected = corrector.correct_counts(&counts);

        let c00 = corrected.get(&vec![false, false]).copied().unwrap_or(0.0);
        // The corrected count for |00> should be close to 1000.
        assert!((c00 - 1000.0).abs() < 10.0, "Expected ~1000, got {}", c00);
    }

    // -------------------------------------------------------------------
    // from_calibration tests
    // -------------------------------------------------------------------

    #[test]
    fn from_calibration_produces_valid_model() {
        let mut gate_errors = HashMap::new();
        gate_errors.insert("sx_0".to_string(), 0.001);
        gate_errors.insert("cx_0_1".to_string(), 0.01);

        let mut gate_times = HashMap::new();
        gate_times.insert("sx_0".to_string(), 0.035); // 35 ns
        gate_times.insert("cx_0_1".to_string(), 0.3);

        let cal = DeviceCalibration {
            qubit_t1: vec![50.0, 60.0],
            qubit_t2: vec![30.0, 40.0],
            readout_error: vec![(0.02, 0.03), (0.01, 0.02)],
            gate_errors,
            gate_times,
            coupling_map: vec![(0, 1)],
        };

        let model = EnhancedNoiseModel::from_calibration(&cal, "sx_0", 0);

        // Depolarizing rate should match gate error.
        assert!((model.depolarizing_rate - 0.001).abs() < 1e-10);

        // Should have amplitude damping (T1 is finite).
        assert!(model.amplitude_damping_gamma.is_some());
        let gamma = model.amplitude_damping_gamma.unwrap();
        let expected_gamma = 1.0 - (-0.035 / 50.0_f64).exp();
        assert!(
            (gamma - expected_gamma).abs() < 1e-10,
            "gamma: expected {}, got {}",
            expected_gamma,
            gamma
        );

        // Should have phase damping.
        assert!(model.phase_damping_lambda.is_some());

        // Should have readout error.
        assert_eq!(model.readout_error, Some((0.02, 0.03)));

        // Should have thermal relaxation.
        assert!(model.thermal_relaxation.is_some());
        let tr = model.thermal_relaxation.unwrap();
        assert!((tr.t1 - 50.0).abs() < 1e-10);
        assert!((tr.t2 - 30.0).abs() < 1e-10);
        assert!((tr.gate_time - 0.035).abs() < 1e-10);
    }

    #[test]
    fn from_calibration_missing_gate_defaults_to_zero() {
        let cal = DeviceCalibration {
            qubit_t1: vec![50.0],
            qubit_t2: vec![30.0],
            readout_error: vec![(0.02, 0.03)],
            gate_errors: HashMap::new(),
            gate_times: HashMap::new(),
            coupling_map: vec![],
        };

        let model = EnhancedNoiseModel::from_calibration(&cal, "nonexistent", 0);

        // No gate error data -> depolarizing = 0.
        assert!((model.depolarizing_rate).abs() < 1e-10);

        // No gate time -> no amplitude/phase damping.
        assert!(model.amplitude_damping_gamma.is_none());
        assert!(model.phase_damping_lambda.is_none());

        // Readout error should still be present from calibration data.
        assert_eq!(model.readout_error, Some((0.02, 0.03)));
    }

    #[test]
    fn from_calibration_qubit_out_of_range() {
        let cal = DeviceCalibration {
            qubit_t1: vec![50.0],
            qubit_t2: vec![30.0],
            readout_error: vec![(0.02, 0.03)],
            gate_errors: HashMap::new(),
            gate_times: HashMap::new(),
            coupling_map: vec![],
        };

        // Qubit 5 is out of range; should gracefully handle with defaults.
        let model = EnhancedNoiseModel::from_calibration(&cal, "sx_5", 5);
        assert!(model.amplitude_damping_gamma.is_none());
        assert!(model.readout_error.is_none());
    }

    // -------------------------------------------------------------------
    // Helper function tests
    // -------------------------------------------------------------------

    #[test]
    fn bits_to_index_roundtrip() {
        for n in 1..=6 {
            for idx in 0..(1usize << n) {
                let bits = index_to_bits(idx, n);
                assert_eq!(bits.len(), n);
                let recovered = bits_to_index(&bits, n);
                assert_eq!(recovered, idx, "Roundtrip failed for n={}, idx={}", n, idx);
            }
        }
    }

    #[test]
    fn mat_mul_identity() {
        let id = IDENTITY;
        let result = mat_mul_2x2(&id, &id);
        for r in 0..2 {
            for c in 0..2 {
                let expected = if r == c { 1.0 } else { 0.0 };
                assert!(
                    (result[r][c].re - expected).abs() < 1e-14,
                    "result[{}][{}] = {:?}",
                    r,
                    c,
                    result[r][c]
                );
                assert!(result[r][c].im.abs() < 1e-14);
            }
        }
    }

    #[test]
    fn invert_2x2_confusion_roundtrip() {
        let p01 = 0.1;
        let p10 = 0.05;
        let inv = invert_2x2_confusion(p01, p10);

        // Original confusion matrix.
        let a = 1.0 - p01;
        let b = p10;
        let c = p01;
        let d = 1.0 - p10;

        // Product should be identity.
        let prod_00 = a * inv[0][0] + b * inv[1][0];
        let prod_01 = a * inv[0][1] + b * inv[1][1];
        let prod_10 = c * inv[0][0] + d * inv[1][0];
        let prod_11 = c * inv[0][1] + d * inv[1][1];

        assert!((prod_00 - 1.0).abs() < 1e-10);
        assert!(prod_01.abs() < 1e-10);
        assert!(prod_10.abs() < 1e-10);
        assert!((prod_11 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn solve_linear_system_simple() {
        // 2x2 system: [[2, 1], [1, 3]] * [x, y] = [5, 10]
        // Solution: x = 5/5 = 1, y = 3  -> 2*1+1*3=5, 1*1+3*3=10
        let a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let b = vec![5.0, 10.0];
        let x = solve_linear_system(&a, &b, 2);
        assert!((x[0] - 1.0).abs() < 1e-10, "x[0] = {}", x[0]);
        assert!((x[1] - 3.0).abs() < 1e-10, "x[1] = {}", x[1]);
    }
}
