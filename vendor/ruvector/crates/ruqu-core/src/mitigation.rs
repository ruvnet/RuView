//! Error mitigation pipeline for quantum circuits.
//!
//! Implements three established mitigation strategies:
//!
//! * **Zero-Noise Extrapolation (ZNE)** -- amplify noise by circuit folding, then
//!   extrapolate back to the zero-noise limit.
//! * **Measurement Error Mitigation** -- correct readout errors via calibration
//!   matrices built from per-qubit `(p01, p10)` error rates.
//! * **Clifford Data Regression (CDR)** -- learn a linear correction model by
//!   comparing noisy and ideal results on near-Clifford training circuits.

use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use std::collections::HashMap;

// ============================================================================
// 1. Zero-Noise Extrapolation (ZNE)
// ============================================================================

/// Configuration for Zero-Noise Extrapolation.
#[derive(Debug, Clone)]
pub struct ZneConfig {
    /// Noise scaling factors to sample (must include 1.0 as the baseline).
    pub noise_factors: Vec<f64>,
    /// Method used to extrapolate to the zero-noise limit.
    pub extrapolation: ExtrapolationMethod,
}

/// Extrapolation method for ZNE.
#[derive(Debug, Clone)]
pub enum ExtrapolationMethod {
    /// Simple linear fit through all data points.
    Linear,
    /// Polynomial fit of the given degree via least-squares.
    Polynomial(usize),
    /// Richardson extrapolation (exact for polynomials of degree n-1 where n
    /// is the number of data points).
    Richardson,
}

/// Fold a quantum circuit to amplify noise by the given `factor`.
///
/// Gate folding replaces each unitary gate G with the sequence G (G^dag G)^k
/// where k is determined by the noise factor.
///
/// * For integer factors (e.g. 3), every non-measurement gate G becomes
///   G G^dag G (i.e. one extra G^dag G pair).
/// * For fractional factors (e.g. 1.5 on a 4-gate circuit), a prefix of
///   gates are folded so the total gate count matches the target.
///
/// Non-unitary operations (Measure, Reset, Barrier) are never folded.
pub fn fold_circuit(circuit: &QuantumCircuit, factor: f64) -> QuantumCircuit {
    assert!(factor >= 1.0, "noise factor must be >= 1.0");

    let gates = circuit.gates();
    let mut folded = QuantumCircuit::new(circuit.num_qubits());

    // Collect indices of unitary (foldable) gates.
    let unitary_indices: Vec<usize> = gates
        .iter()
        .enumerate()
        .filter(|(_, g)| !g.is_non_unitary())
        .map(|(i, _)| i)
        .collect();

    let n_unitary = unitary_indices.len();

    // Total number of unitary gate slots after folding. Each fold adds 2 gates
    // (G^dag G), so total = n_unitary * factor, rounded to the nearest integer.
    let target_unitary_slots = (n_unitary as f64 * factor).round() as usize;

    // Each folded gate occupies 3 slots (G G^dag G), unfolded occupies 1.
    // If we fold k gates: total = k * 3 + (n_unitary - k) = 2k + n_unitary
    // => k = (target_unitary_slots - n_unitary) / 2
    let num_folds = if target_unitary_slots > n_unitary {
        (target_unitary_slots - n_unitary) / 2
    } else {
        0
    };

    // Determine how many full folding rounds per gate, and how many extra gates
    // get one additional round.
    let full_rounds = num_folds / n_unitary.max(1);
    let extra_folds = num_folds % n_unitary.max(1);

    // Build a set of unitary-gate indices that get the extra fold.
    // We fold the first `extra_folds` unitary gates one additional time.
    let mut unitary_counter: usize = 0;

    for gate in gates.iter() {
        if gate.is_non_unitary() {
            folded.add_gate(gate.clone());
            continue;
        }

        // This is a unitary gate. Determine how many fold rounds it gets.
        let rounds = full_rounds + if unitary_counter < extra_folds { 1 } else { 0 };
        unitary_counter += 1;

        // Original gate.
        folded.add_gate(gate.clone());

        // Append (G^dag G) `rounds` times.
        for _ in 0..rounds {
            let dag = gate_dagger(gate);
            folded.add_gate(dag);
            folded.add_gate(gate.clone());
        }
    }

    folded
}

/// Compute the conjugate transpose (dagger) of a gate.
///
/// For single-qubit gates with known matrix U, we compute U^dag by conjugating
/// and transposing the 2x2 matrix. For two-qubit gates, the dagger is computed
/// from the known structure.
fn gate_dagger(gate: &Gate) -> Gate {
    match gate {
        // Self-inverse gates: H, X, Y, Z, CNOT, CZ, SWAP, Barrier.
        Gate::H(q) => Gate::H(*q),
        Gate::X(q) => Gate::X(*q),
        Gate::Y(q) => Gate::Y(*q),
        Gate::Z(q) => Gate::Z(*q),
        Gate::CNOT(c, t) => Gate::CNOT(*c, *t),
        Gate::CZ(q1, q2) => Gate::CZ(*q1, *q2),
        Gate::SWAP(q1, q2) => Gate::SWAP(*q1, *q2),

        // S^dag = Sdg, Sdg^dag = S.
        Gate::S(q) => Gate::Sdg(*q),
        Gate::Sdg(q) => Gate::S(*q),

        // T^dag = Tdg, Tdg^dag = T.
        Gate::T(q) => Gate::Tdg(*q),
        Gate::Tdg(q) => Gate::T(*q),

        // Rotation gates: dagger negates the angle.
        Gate::Rx(q, theta) => Gate::Rx(*q, -theta),
        Gate::Ry(q, theta) => Gate::Ry(*q, -theta),
        Gate::Rz(q, theta) => Gate::Rz(*q, -theta),
        Gate::Phase(q, theta) => Gate::Phase(*q, -theta),
        Gate::Rzz(q1, q2, theta) => Gate::Rzz(*q1, *q2, -theta),

        // Custom unitary: conjugate transpose of the 2x2 matrix.
        Gate::Unitary1Q(q, m) => {
            let dag = [
                [m[0][0].conj(), m[1][0].conj()],
                [m[0][1].conj(), m[1][1].conj()],
            ];
            Gate::Unitary1Q(*q, dag)
        }

        // Non-unitary ops should not reach here, but handle gracefully.
        Gate::Measure(q) => Gate::Measure(*q),
        Gate::Reset(q) => Gate::Reset(*q),
        Gate::Barrier => Gate::Barrier,
    }
}

/// Richardson extrapolation to the zero-noise limit.
///
/// Given n data points `(noise_factors[i], values[i])`, the Richardson
/// extrapolation computes the unique polynomial of degree n-1 that passes
/// through all points, then evaluates it at x = 0. This is equivalent to
/// the Lagrange interpolation formula evaluated at zero.
pub fn richardson_extrapolate(noise_factors: &[f64], values: &[f64]) -> f64 {
    assert_eq!(
        noise_factors.len(),
        values.len(),
        "noise_factors and values must have the same length"
    );
    let n = noise_factors.len();
    assert!(n > 0, "need at least one data point");

    // Lagrange interpolation at x = 0:
    //   P(0) = sum_i  values[i] * product_{j != i} (0 - x_j) / (x_i - x_j)
    let mut result = 0.0;
    for i in 0..n {
        let mut weight = 1.0;
        for j in 0..n {
            if j != i {
                // (0 - x_j) / (x_i - x_j)
                weight *= -noise_factors[j] / (noise_factors[i] - noise_factors[j]);
            }
        }
        result += values[i] * weight;
    }
    result
}

/// Polynomial extrapolation via least-squares fit.
///
/// Fits a polynomial of the specified `degree` to the data, then evaluates
/// at x = 0 (returning the constant term of the fit).
pub fn polynomial_extrapolate(noise_factors: &[f64], values: &[f64], degree: usize) -> f64 {
    assert_eq!(
        noise_factors.len(),
        values.len(),
        "noise_factors and values must have the same length"
    );
    let n = noise_factors.len();
    let p = degree + 1; // number of coefficients
    assert!(
        n >= p,
        "need at least degree+1 data points for a degree-{degree} polynomial"
    );

    // Build the Vandermonde matrix A (n x p) where A[i][j] = x_i^j.
    // Then solve A^T A c = A^T y via normal equations.
    // Since we only need c[0] (the value at x=0), we solve the full system.

    // A^T A  (p x p)
    let mut ata = vec![vec![0.0_f64; p]; p];
    // A^T y  (p x 1)
    let mut aty = vec![0.0_f64; p];

    for i in 0..n {
        let x = noise_factors[i];
        let y = values[i];

        // Precompute powers of x up to 2 * degree.
        let max_power = 2 * degree;
        let mut x_powers = Vec::with_capacity(max_power + 1);
        x_powers.push(1.0);
        for k in 1..=max_power {
            x_powers.push(x_powers[k - 1] * x);
        }

        for j in 0..p {
            aty[j] += y * x_powers[j];
            for k in 0..p {
                ata[j][k] += x_powers[j + k];
            }
        }
    }

    // Solve p x p linear system via Gaussian elimination with partial pivoting.
    let coeffs = solve_linear_system(&mut ata, &mut aty);

    // The value at x = 0 is simply c[0].
    coeffs[0]
}

/// Linear extrapolation to x = 0.
///
/// Fits y = a*x + b via least-squares and returns b (the y-intercept).
pub fn linear_extrapolate(noise_factors: &[f64], values: &[f64]) -> f64 {
    polynomial_extrapolate(noise_factors, values, 1)
}

/// Solve a dense linear system Ax = b using Gaussian elimination with partial
/// pivoting. Modifies `a` and `b` in place. Returns the solution vector.
fn solve_linear_system(a: &mut Vec<Vec<f64>>, b: &mut Vec<f64>) -> Vec<f64> {
    let n = b.len();
    assert!(n > 0);

    // Forward elimination with partial pivoting.
    for col in 0..n {
        // Find pivot.
        let mut max_row = col;
        let mut max_val = a[col][col].abs();
        for row in (col + 1)..n {
            let v = a[row][col].abs();
            if v > max_val {
                max_val = v;
                max_row = row;
            }
        }

        // Swap rows.
        if max_row != col {
            a.swap(col, max_row);
            b.swap(col, max_row);
        }

        let pivot = a[col][col];
        assert!(
            pivot.abs() > 1e-15,
            "singular or near-singular matrix in least-squares solve"
        );

        // Eliminate below.
        for row in (col + 1)..n {
            let factor = a[row][col] / pivot;
            for k in col..n {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }

    // Back substitution.
    let mut x = vec![0.0; n];
    for col in (0..n).rev() {
        let mut sum = b[col];
        for k in (col + 1)..n {
            sum -= a[col][k] * x[k];
        }
        x[col] = sum / a[col][col];
    }

    x
}

// ============================================================================
// 2. Measurement Error Mitigation
// ============================================================================

/// Corrects readout errors using a full calibration matrix built from
/// per-qubit error probabilities.
#[derive(Debug, Clone)]
pub struct MeasurementCorrector {
    num_qubits: usize,
    /// Row-major 2^n x 2^n calibration matrix. Entry `[i][j]` is the
    /// probability of observing bitstring `i` when the true state is `j`.
    calibration_matrix: Vec<Vec<f64>>,
}

impl MeasurementCorrector {
    /// Build the calibration matrix from per-qubit readout errors.
    ///
    /// `readout_errors[q] = (p01, p10)` where:
    /// * `p01` = probability of reading 1 when the true state is 0
    /// * `p10` = probability of reading 0 when the true state is 1
    ///
    /// The full calibration matrix is the tensor product of the individual
    /// 2x2 matrices:
    ///   M_q = [[1 - p01, p10],
    ///          [p01,     1 - p10]]
    pub fn new(readout_errors: &[(f64, f64)]) -> Self {
        let num_qubits = readout_errors.len();
        let dim = 1usize << num_qubits;

        // Build per-qubit 2x2 matrices.
        let qubit_matrices: Vec<[[f64; 2]; 2]> = readout_errors
            .iter()
            .map(|&(p01, p10)| [[1.0 - p01, p10], [p01, 1.0 - p10]])
            .collect();

        // Tensor product to build the full dim x dim matrix.
        let mut cal = vec![vec![0.0; dim]; dim];
        for row in 0..dim {
            for col in 0..dim {
                let mut val = 1.0;
                for q in 0..num_qubits {
                    let row_bit = (row >> q) & 1;
                    let col_bit = (col >> q) & 1;
                    val *= qubit_matrices[q][row_bit][col_bit];
                }
                cal[row][col] = val;
            }
        }

        Self {
            num_qubits,
            calibration_matrix: cal,
        }
    }

    /// Correct measurement counts by applying the inverse of the calibration
    /// matrix.
    ///
    /// For small qubit counts (<= 12), the full matrix is inverted directly.
    /// For larger systems, the tensor product structure is exploited for
    /// efficient correction.
    ///
    /// Returns corrected counts as floating-point values since the inverse
    /// may produce non-integer results.
    pub fn correct_counts(&self, counts: &HashMap<Vec<bool>, usize>) -> HashMap<Vec<bool>, f64> {
        let dim = 1usize << self.num_qubits;

        // Build the probability vector from counts.
        let total_shots: usize = counts.values().sum();
        let total_f64 = total_shots as f64;

        let mut prob_vec = vec![0.0; dim];
        for (bits, &count) in counts {
            let idx = bits_to_index(bits, self.num_qubits);
            prob_vec[idx] = count as f64 / total_f64;
        }

        // Invert and apply.
        let corrected_probs = if self.num_qubits <= 12 {
            // Direct matrix inversion for small systems.
            let inv = invert_matrix(&self.calibration_matrix);
            mat_vec_mul(&inv, &prob_vec)
        } else {
            // Exploit tensor product structure for large systems.
            // The inverse of A tensor B = A^-1 tensor B^-1.
            // Apply the per-qubit inverse matrices sequentially.
            self.tensor_product_correct(&prob_vec)
        };

        // Convert back to counts (scaled by total shots).
        let mut result = HashMap::new();
        for idx in 0..dim {
            let corrected_count = corrected_probs[idx] * total_f64;
            if corrected_count.abs() > 1e-10 {
                let bits = index_to_bits(idx, self.num_qubits);
                result.insert(bits, corrected_count);
            }
        }

        result
    }

    /// Accessor for the calibration matrix.
    pub fn calibration_matrix(&self) -> &Vec<Vec<f64>> {
        &self.calibration_matrix
    }

    /// Apply per-qubit inverse correction using tensor product structure.
    ///
    /// This avoids building and inverting the full 2^n x 2^n matrix by
    /// applying each qubit's 2x2 inverse separately in sequence.
    fn tensor_product_correct(&self, prob_vec: &[f64]) -> Vec<f64> {
        let dim = 1usize << self.num_qubits;
        let mut result = prob_vec.to_vec();

        // Extract per-qubit 2x2 matrices from the calibration matrix and invert.
        for q in 0..self.num_qubits {
            // Re-derive per-qubit matrix from the calibration matrix structure.
            // For qubit q, the 2x2 submatrix is extracted by looking at how
            // bit q affects the matrix entry.
            let qubit_mat = self.extract_qubit_matrix(q);
            let inv = invert_2x2(&qubit_mat);

            // Apply the 2x2 inverse along the q-th qubit axis.
            let mut new_result = vec![0.0; dim];
            let stride = 1usize << q;
            for block_start in (0..dim).step_by(stride * 2) {
                for offset in 0..stride {
                    let i0 = block_start + offset;
                    let i1 = i0 + stride;
                    new_result[i0] = inv[0][0] * result[i0] + inv[0][1] * result[i1];
                    new_result[i1] = inv[1][0] * result[i0] + inv[1][1] * result[i1];
                }
            }
            result = new_result;
        }

        result
    }

    /// Extract the 2x2 calibration matrix for a single qubit from the full
    /// calibration matrix.
    fn extract_qubit_matrix(&self, qubit: usize) -> [[f64; 2]; 2] {
        // The per-qubit matrix is encoded in the tensor product structure.
        // To extract qubit q's matrix, look at a pair of indices that differ
        // only in bit q. The simplest choice: indices 0 and (1 << q).
        let i0 = 0;
        let i1 = 1usize << qubit;

        [
            [
                self.calibration_matrix[i0][i0],
                self.calibration_matrix[i0][i1],
            ],
            [
                self.calibration_matrix[i1][i0],
                self.calibration_matrix[i1][i1],
            ],
        ]
    }
}

/// Convert a bit vector to an integer index.
fn bits_to_index(bits: &[bool], num_qubits: usize) -> usize {
    let mut idx = 0usize;
    for q in 0..num_qubits {
        if q < bits.len() && bits[q] {
            idx |= 1 << q;
        }
    }
    idx
}

/// Convert an integer index back to a bit vector.
fn index_to_bits(idx: usize, num_qubits: usize) -> Vec<bool> {
    (0..num_qubits).map(|q| (idx >> q) & 1 == 1).collect()
}

/// Invert a 2x2 matrix.
fn invert_2x2(m: &[[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    assert!(det.abs() > 1e-15, "singular 2x2 matrix");
    let inv_det = 1.0 / det;
    [
        [m[1][1] * inv_det, -m[0][1] * inv_det],
        [-m[1][0] * inv_det, m[0][0] * inv_det],
    ]
}

/// Invert a square matrix via Gauss-Jordan elimination with partial pivoting.
fn invert_matrix(mat: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = mat.len();
    // Augmented matrix [A | I].
    let mut aug: Vec<Vec<f64>> = mat
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut aug_row = row.clone();
            aug_row.resize(2 * n, 0.0);
            aug_row[n + i] = 1.0;
            aug_row
        })
        .collect();

    // Forward elimination.
    for col in 0..n {
        // Partial pivoting.
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..n {
            let v = aug[row][col].abs();
            if v > max_val {
                max_val = v;
                max_row = row;
            }
        }
        aug.swap(col, max_row);

        let pivot = aug[col][col];
        assert!(
            pivot.abs() > 1e-15,
            "singular matrix in calibration inversion"
        );

        // Scale pivot row.
        let inv_pivot = 1.0 / pivot;
        for k in 0..(2 * n) {
            aug[col][k] *= inv_pivot;
        }

        // Eliminate all other rows.
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = aug[row][col];
            for k in 0..(2 * n) {
                aug[row][k] -= factor * aug[col][k];
            }
        }
    }

    // Extract the right half as the inverse.
    aug.iter().map(|row| row[n..].to_vec()).collect()
}

/// Multiply a matrix by a vector.
fn mat_vec_mul(mat: &[Vec<f64>], vec: &[f64]) -> Vec<f64> {
    mat.iter()
        .map(|row| row.iter().zip(vec.iter()).map(|(a, b)| a * b).sum())
        .collect()
}

// ============================================================================
// 3. Clifford Data Regression (CDR)
// ============================================================================

/// Configuration for Clifford Data Regression.
#[derive(Debug, Clone)]
pub struct CdrConfig {
    /// Number of near-Clifford training circuits to generate.
    pub num_training_circuits: usize,
    /// Seed for the random replacement of non-Clifford gates.
    pub seed: u64,
}

/// Generate near-Clifford training circuits from the original circuit.
///
/// Each training circuit is a copy of the original where non-Clifford gates
/// (T, Tdg, Rx, Ry, Rz, Phase, Rzz) are replaced with random Clifford
/// gates acting on the same qubits. The resulting circuits are efficiently
/// simulable by a stabilizer backend.
pub fn generate_training_circuits(
    circuit: &QuantumCircuit,
    config: &CdrConfig,
) -> Vec<QuantumCircuit> {
    let mut circuits = Vec::with_capacity(config.num_training_circuits);

    // Simple LCG-based deterministic RNG (no external dependency needed for
    // training circuit generation; keeps this module self-contained).
    let mut rng_state = config.seed;
    let lcg_next = |state: &mut u64| -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    };

    // Clifford single-qubit replacements.
    let clifford_1q = |q: u32, choice: u64| -> Gate {
        match choice % 6 {
            0 => Gate::H(q),
            1 => Gate::X(q),
            2 => Gate::Y(q),
            3 => Gate::Z(q),
            4 => Gate::S(q),
            _ => Gate::Sdg(q),
        }
    };

    // Clifford two-qubit replacements.
    let clifford_2q = |q1: u32, q2: u32, choice: u64| -> Gate {
        match choice % 3 {
            0 => Gate::CNOT(q1, q2),
            1 => Gate::CZ(q1, q2),
            _ => Gate::SWAP(q1, q2),
        }
    };

    for _ in 0..config.num_training_circuits {
        let mut training = QuantumCircuit::new(circuit.num_qubits());

        for gate in circuit.gates() {
            let replacement = match gate {
                // Non-Clifford single-qubit gates: replace with random Clifford.
                Gate::T(q) | Gate::Tdg(q) => {
                    let r = lcg_next(&mut rng_state);
                    clifford_1q(*q, r)
                }
                Gate::Rx(q, _) | Gate::Ry(q, _) | Gate::Rz(q, _) | Gate::Phase(q, _) => {
                    let r = lcg_next(&mut rng_state);
                    clifford_1q(*q, r)
                }
                Gate::Unitary1Q(q, _) => {
                    let r = lcg_next(&mut rng_state);
                    clifford_1q(*q, r)
                }

                // Non-Clifford two-qubit gates: replace with random Clifford.
                Gate::Rzz(q1, q2, _) => {
                    let r = lcg_next(&mut rng_state);
                    clifford_2q(*q1, *q2, r)
                }

                // Clifford and non-unitary gates: keep as-is.
                other => other.clone(),
            };
            training.add_gate(replacement);
        }

        circuits.push(training);
    }

    circuits
}

/// Apply Clifford Data Regression correction to a target noisy expectation value.
///
/// Given pairs `(noisy_values[i], ideal_values[i])` from the training circuits,
/// fits the linear model `ideal = a * noisy + b` via least-squares and applies
/// the same transformation to `target_noisy`.
pub fn cdr_correct(noisy_values: &[f64], ideal_values: &[f64], target_noisy: f64) -> f64 {
    assert_eq!(
        noisy_values.len(),
        ideal_values.len(),
        "noisy_values and ideal_values must have the same length"
    );
    let n = noisy_values.len();
    assert!(n >= 2, "need at least 2 training points for CDR");

    // Least-squares linear regression: ideal = a * noisy + b
    //
    // a = (n * sum(x*y) - sum(x) * sum(y)) / (n * sum(x^2) - (sum(x))^2)
    // b = (sum(y) - a * sum(x)) / n

    let sum_x: f64 = noisy_values.iter().sum();
    let sum_y: f64 = ideal_values.iter().sum();
    let sum_xy: f64 = noisy_values
        .iter()
        .zip(ideal_values.iter())
        .map(|(x, y)| x * y)
        .sum();
    let sum_x2: f64 = noisy_values.iter().map(|x| x * x).sum();

    let n_f64 = n as f64;
    let denom = n_f64 * sum_x2 - sum_x * sum_x;

    if denom.abs() < 1e-15 {
        // All noisy values are the same; return the mean ideal value.
        return sum_y / n_f64;
    }

    let a = (n_f64 * sum_xy - sum_x * sum_y) / denom;
    let b = (sum_y - a * sum_x) / n_f64;

    a * target_noisy + b
}

// ============================================================================
// 4. Helpers
// ============================================================================

/// Compute the Z-basis expectation value `<Z>` for a single qubit from
/// shot counts.
///
/// For each bitstring, if the qubit is in state 0, it contributes +1;
/// if in state 1, it contributes -1. The expectation is the weighted
/// average over all shots.
pub fn expectation_from_counts(counts: &HashMap<Vec<bool>, usize>, qubit: u32) -> f64 {
    let mut total_shots: usize = 0;
    let mut z_sum: f64 = 0.0;

    for (bits, &count) in counts {
        total_shots += count;
        let bit_val = bits.get(qubit as usize).copied().unwrap_or(false);
        // |0> -> +1, |1> -> -1
        let z_eigenvalue = if bit_val { -1.0 } else { 1.0 };
        z_sum += z_eigenvalue * count as f64;
    }

    if total_shots == 0 {
        return 0.0;
    }

    z_sum / total_shots as f64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Complex;

    // ---- Richardson extrapolation ----------------------------------------

    #[test]
    fn test_richardson_recovers_polynomial() {
        // For a quadratic f(x) = 3x^2 - 2x + 5, three data points should
        // recover f(0) = 5 exactly via Richardson (degree-2 interpolation).
        let noise_factors = vec![1.0, 2.0, 3.0];
        let values: Vec<f64> = noise_factors
            .iter()
            .map(|&x| 3.0 * x * x - 2.0 * x + 5.0)
            .collect();

        let result = richardson_extrapolate(&noise_factors, &values);
        assert!(
            (result - 5.0).abs() < 1e-10,
            "Richardson should recover f(0) = 5.0, got {result}"
        );
    }

    #[test]
    fn test_richardson_linear_data() {
        // f(x) = 2x + 7 => f(0) = 7
        let noise_factors = vec![1.0, 2.0];
        let values = vec![9.0, 11.0];
        let result = richardson_extrapolate(&noise_factors, &values);
        assert!(
            (result - 7.0).abs() < 1e-10,
            "Richardson on linear data: expected 7.0, got {result}"
        );
    }

    #[test]
    fn test_richardson_cubic() {
        // f(x) = x^3 - x + 1 => f(0) = 1
        let noise_factors = vec![1.0, 1.5, 2.0, 3.0];
        let values: Vec<f64> = noise_factors.iter().map(|&x| x * x * x - x + 1.0).collect();
        let result = richardson_extrapolate(&noise_factors, &values);
        assert!(
            (result - 1.0).abs() < 1e-9,
            "Richardson on cubic data: expected 1.0, got {result}"
        );
    }

    // ---- Linear extrapolation -------------------------------------------

    #[test]
    fn test_linear_extrapolation_exact() {
        // y = 3x + 2 => y(0) = 2
        let noise_factors = vec![1.0, 2.0, 3.0];
        let values: Vec<f64> = noise_factors.iter().map(|&x| 3.0 * x + 2.0).collect();
        let result = linear_extrapolate(&noise_factors, &values);
        assert!(
            (result - 2.0).abs() < 1e-10,
            "Linear extrapolation: expected 2.0, got {result}"
        );
    }

    #[test]
    fn test_linear_extrapolation_two_points() {
        let noise_factors = vec![1.0, 3.0];
        let values = vec![5.0, 11.0]; // slope = 3, intercept = 2
        let result = linear_extrapolate(&noise_factors, &values);
        assert!(
            (result - 2.0).abs() < 1e-10,
            "Linear extrapolation with 2 points: expected 2.0, got {result}"
        );
    }

    // ---- Polynomial extrapolation ---------------------------------------

    #[test]
    fn test_polynomial_extrapolation_quadratic() {
        // f(x) = x^2 + 1 => f(0) = 1
        let noise_factors = vec![1.0, 2.0, 3.0];
        let values: Vec<f64> = noise_factors.iter().map(|&x| x * x + 1.0).collect();
        let result = polynomial_extrapolate(&noise_factors, &values, 2);
        assert!(
            (result - 1.0).abs() < 1e-10,
            "Polynomial (degree 2): expected 1.0, got {result}"
        );
    }

    // ---- Fold circuit ---------------------------------------------------

    #[test]
    fn test_fold_circuit_factor_1() {
        // factor = 1.0 should return a circuit with the same gates.
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.cnot(0, 1);
        circuit.measure(0);
        circuit.measure(1);

        let folded = fold_circuit(&circuit, 1.0);

        assert_eq!(
            folded.gates().len(),
            circuit.gates().len(),
            "fold factor=1 should produce the same number of gates"
        );
    }

    #[test]
    fn test_fold_circuit_factor_3() {
        // factor = 3 should triple each unitary gate: G G^dag G.
        // Original: H, CNOT (2 unitary gates).
        // Folded:   H H^dag H, CNOT CNOT^dag CNOT (6 unitary gates).
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.cnot(0, 1);

        let folded = fold_circuit(&circuit, 3.0);

        // 2 unitary gates * factor 3 = 6 gate slots.
        let unitary_count = folded
            .gates()
            .iter()
            .filter(|g| !g.is_non_unitary())
            .count();
        assert_eq!(
            unitary_count, 6,
            "fold factor=3 on 2-gate circuit: expected 6 unitary gates, got {unitary_count}"
        );
    }

    #[test]
    fn test_fold_circuit_factor_3_preserves_measurements() {
        // Measurements should pass through unchanged.
        let mut circuit = QuantumCircuit::new(1);
        circuit.h(0);
        circuit.measure(0);

        let folded = fold_circuit(&circuit, 3.0);

        let measure_count = folded
            .gates()
            .iter()
            .filter(|g| matches!(g, Gate::Measure(_)))
            .count();
        assert_eq!(measure_count, 1, "measurements should not be folded");

        let unitary_count = folded
            .gates()
            .iter()
            .filter(|g| !g.is_non_unitary())
            .count();
        assert_eq!(
            unitary_count, 3,
            "1 H gate folded at factor 3 => 3 unitary gates"
        );
    }

    #[test]
    fn test_fold_circuit_fractional_factor() {
        // factor = 1.5 on 4 unitary gates.
        // target slots = round(4 * 1.5) = 6, so num_folds = (6 - 4) / 2 = 1.
        // One gate gets folded (3 slots), three remain (1 slot each) = 6 total.
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.x(1);
        circuit.cnot(0, 1);
        circuit.z(0);

        let folded = fold_circuit(&circuit, 1.5);
        let unitary_count = folded
            .gates()
            .iter()
            .filter(|g| !g.is_non_unitary())
            .count();
        assert_eq!(
            unitary_count, 6,
            "fold factor=1.5 on 4-gate circuit: expected 6 unitary gates, got {unitary_count}"
        );
    }

    // ---- MeasurementCorrector -------------------------------------------

    #[test]
    fn test_measurement_corrector_zero_error_is_identity() {
        // With no readout errors, the calibration matrix should be the identity.
        let corrector = MeasurementCorrector::new(&[(0.0, 0.0), (0.0, 0.0)]);
        let cal = corrector.calibration_matrix();

        let dim = 4; // 2 qubits -> 2^2 = 4
        for i in 0..dim {
            for j in 0..dim {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (cal[i][j] - expected).abs() < 1e-12,
                    "cal[{i}][{j}] = {}, expected {expected}",
                    cal[i][j]
                );
            }
        }
    }

    #[test]
    fn test_measurement_corrector_single_qubit() {
        // Single qubit with p01 = 0.1, p10 = 0.05.
        // M = [[0.9, 0.05], [0.1, 0.95]]
        let corrector = MeasurementCorrector::new(&[(0.1, 0.05)]);
        let cal = corrector.calibration_matrix();

        assert!((cal[0][0] - 0.9).abs() < 1e-12);
        assert!((cal[0][1] - 0.05).abs() < 1e-12);
        assert!((cal[1][0] - 0.1).abs() < 1e-12);
        assert!((cal[1][1] - 0.95).abs() < 1e-12);
    }

    #[test]
    fn test_measurement_corrector_correction_identity() {
        // With zero errors, correction should return the same probabilities.
        let corrector = MeasurementCorrector::new(&[(0.0, 0.0)]);

        let mut counts = HashMap::new();
        counts.insert(vec![false], 600);
        counts.insert(vec![true], 400);

        let corrected = corrector.correct_counts(&counts);

        let c0 = corrected.get(&vec![false]).copied().unwrap_or(0.0);
        let c1 = corrected.get(&vec![true]).copied().unwrap_or(0.0);

        assert!(
            (c0 - 600.0).abs() < 1e-6,
            "expected 600.0 for |0>, got {c0}"
        );
        assert!(
            (c1 - 400.0).abs() < 1e-6,
            "expected 400.0 for |1>, got {c1}"
        );
    }

    #[test]
    fn test_measurement_corrector_nontrivial_correction() {
        // With errors, the corrected counts should differ from raw counts.
        let corrector = MeasurementCorrector::new(&[(0.1, 0.05)]);

        let mut counts = HashMap::new();
        counts.insert(vec![false], 550);
        counts.insert(vec![true], 450);

        let corrected = corrector.correct_counts(&counts);
        let c0 = corrected.get(&vec![false]).copied().unwrap_or(0.0);
        let c1 = corrected.get(&vec![true]).copied().unwrap_or(0.0);

        // The correction should shift counts toward the true distribution.
        // M^{-1} applied to [0.55, 0.45]^T should yield something different.
        assert!(
            (c0 + c1 - 1000.0).abs() < 1.0,
            "total corrected counts should sum to ~1000"
        );
        // Just verify it actually changed.
        assert!(
            (c0 - 550.0).abs() > 1.0 || (c1 - 450.0).abs() > 1.0,
            "correction should change the counts"
        );
    }

    // ---- CDR linear regression ------------------------------------------

    #[test]
    fn test_cdr_correct_known_linear() {
        // If ideal = 2 * noisy - 1, then for target_noisy = 3.0:
        //   corrected = 2 * 3.0 - 1 = 5.0
        let noisy_values = vec![1.0, 2.0, 3.0, 4.0];
        let ideal_values: Vec<f64> = noisy_values.iter().map(|&x| 2.0 * x - 1.0).collect();

        let result = cdr_correct(&noisy_values, &ideal_values, 3.0);
        assert!(
            (result - 5.0).abs() < 1e-10,
            "CDR correction: expected 5.0, got {result}"
        );
    }

    #[test]
    fn test_cdr_correct_identity_model() {
        // If ideal == noisy, correction should return target_noisy unchanged.
        let noisy_values = vec![1.0, 2.0, 3.0];
        let ideal_values = vec![1.0, 2.0, 3.0];

        let result = cdr_correct(&noisy_values, &ideal_values, 5.0);
        assert!(
            (result - 5.0).abs() < 1e-10,
            "CDR identity model: expected 5.0, got {result}"
        );
    }

    #[test]
    fn test_cdr_correct_offset() {
        // ideal = noisy + 0.5
        let noisy_values = vec![0.0, 1.0, 2.0];
        let ideal_values = vec![0.5, 1.5, 2.5];

        let result = cdr_correct(&noisy_values, &ideal_values, 3.0);
        assert!(
            (result - 3.5).abs() < 1e-10,
            "CDR offset model: expected 3.5, got {result}"
        );
    }

    // ---- Generate training circuits -------------------------------------

    #[test]
    fn test_generate_training_circuits_count() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.t(0);
        circuit.cnot(0, 1);
        circuit.rx(1, 0.5);

        let config = CdrConfig {
            num_training_circuits: 10,
            seed: 42,
        };

        let training = generate_training_circuits(&circuit, &config);
        assert_eq!(training.len(), 10);
    }

    #[test]
    fn test_generate_training_circuits_preserves_clifford_gates() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.cnot(0, 1);
        circuit.x(1);

        let config = CdrConfig {
            num_training_circuits: 5,
            seed: 0,
        };

        let training = generate_training_circuits(&circuit, &config);

        // All gates in the original are Clifford, so training circuits should
        // have the same number of gates.
        for tc in &training {
            assert_eq!(
                tc.gates().len(),
                circuit.gates().len(),
                "training circuit should have same gate count"
            );
        }
    }

    #[test]
    fn test_generate_training_circuits_replaces_non_clifford() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.t(0); // non-Clifford

        let config = CdrConfig {
            num_training_circuits: 20,
            seed: 123,
        };

        let training = generate_training_circuits(&circuit, &config);

        // None of the training circuits should contain a T gate.
        for tc in &training {
            for gate in tc.gates() {
                assert!(
                    !matches!(gate, Gate::T(_)),
                    "training circuit should not contain T gate"
                );
            }
        }
    }

    #[test]
    fn test_generate_training_circuits_deterministic() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rx(0, 1.0);
        circuit.t(0);

        let config = CdrConfig {
            num_training_circuits: 5,
            seed: 42,
        };

        let training1 = generate_training_circuits(&circuit, &config);
        let training2 = generate_training_circuits(&circuit, &config);

        // Same seed should produce the same number of circuits with the same
        // gate counts.
        assert_eq!(training1.len(), training2.len());
        for (t1, t2) in training1.iter().zip(training2.iter()) {
            assert_eq!(t1.gates().len(), t2.gates().len());
        }
    }

    // ---- expectation_from_counts ----------------------------------------

    #[test]
    fn test_expectation_all_zero() {
        // All shots yield |0> => <Z> = +1.0
        let mut counts = HashMap::new();
        counts.insert(vec![false], 1000);

        let exp = expectation_from_counts(&counts, 0);
        assert!(
            (exp - 1.0).abs() < 1e-12,
            "all |0>: expected <Z> = 1.0, got {exp}"
        );
    }

    #[test]
    fn test_expectation_all_one() {
        // All shots yield |1> => <Z> = -1.0
        let mut counts = HashMap::new();
        counts.insert(vec![true], 500);

        let exp = expectation_from_counts(&counts, 0);
        assert!(
            (exp - (-1.0)).abs() < 1e-12,
            "all |1>: expected <Z> = -1.0, got {exp}"
        );
    }

    #[test]
    fn test_expectation_equal_split() {
        // 50/50 split => <Z> = 0
        let mut counts = HashMap::new();
        counts.insert(vec![false], 500);
        counts.insert(vec![true], 500);

        let exp = expectation_from_counts(&counts, 0);
        assert!(
            exp.abs() < 1e-12,
            "equal split: expected <Z> = 0.0, got {exp}"
        );
    }

    #[test]
    fn test_expectation_multi_qubit() {
        // 2 qubits: |00> x 300, |01> x 200, |10> x 100, |11> x 400
        // For qubit 0: |0> appears in |00> + |10> = 400, |1> in |01> + |11> = 600
        //   <Z_0> = (400 - 600) / 1000 = -0.2
        // For qubit 1: |0> appears in |00> + |01> = 500, |1> in |10> + |11> = 500
        //   <Z_1> = (500 - 500) / 1000 = 0.0
        let mut counts = HashMap::new();
        counts.insert(vec![false, false], 300);
        counts.insert(vec![true, false], 200);
        counts.insert(vec![false, true], 100);
        counts.insert(vec![true, true], 400);

        let exp0 = expectation_from_counts(&counts, 0);
        let exp1 = expectation_from_counts(&counts, 1);

        assert!(
            (exp0 - (-0.2)).abs() < 1e-12,
            "qubit 0: expected -0.2, got {exp0}"
        );
        assert!(exp1.abs() < 1e-12, "qubit 1: expected 0.0, got {exp1}");
    }

    #[test]
    fn test_expectation_empty_counts() {
        let counts: HashMap<Vec<bool>, usize> = HashMap::new();
        let exp = expectation_from_counts(&counts, 0);
        assert!(exp.abs() < 1e-12, "empty counts should give 0.0, got {exp}");
    }

    // ---- Gate dagger correctness ----------------------------------------

    #[test]
    fn test_gate_dagger_self_inverse() {
        // H, X, Y, Z are their own inverses.
        let gates = vec![Gate::H(0), Gate::X(0), Gate::Y(0), Gate::Z(0)];
        for gate in &gates {
            let dag = gate_dagger(gate);
            // For self-inverse gates, the matrix of the dagger should equal
            // the matrix of the original.
            if let (Some(m_orig), Some(m_dag)) = (gate.matrix_1q(), dag.matrix_1q()) {
                for i in 0..2 {
                    for j in 0..2 {
                        let diff = (m_orig[i][j] - m_dag[i][j]).norm();
                        assert!(
                            diff < 1e-12,
                            "gate_dagger of self-inverse gate should match: diff = {diff}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_gate_dagger_s_sdg() {
        // S^dag = Sdg, so matrix of S^dag should equal matrix of Sdg.
        let s_dag = gate_dagger(&Gate::S(0));
        let sdg = Gate::Sdg(0);

        let m1 = s_dag.matrix_1q().unwrap();
        let m2 = sdg.matrix_1q().unwrap();

        for i in 0..2 {
            for j in 0..2 {
                let diff = (m1[i][j] - m2[i][j]).norm();
                assert!(diff < 1e-12, "S dagger should equal Sdg");
            }
        }
    }

    #[test]
    fn test_gate_dagger_rotation_inverse() {
        // Rx(theta)^dag = Rx(-theta). Product should be identity.
        let theta = 1.23;
        let rx = Gate::Rx(0, theta);
        let rx_dag = gate_dagger(&rx);

        let m = rx.matrix_1q().unwrap();
        let m_dag = rx_dag.matrix_1q().unwrap();

        // Product m * m_dag should be identity.
        let product = mat_mul_2x2(&m, &m_dag);
        for i in 0..2 {
            for j in 0..2 {
                let expected = if i == j { Complex::ONE } else { Complex::ZERO };
                let diff = (product[i][j] - expected).norm();
                assert!(
                    diff < 1e-12,
                    "Rx * Rx^dag should be identity at [{i}][{j}]: diff = {diff}"
                );
            }
        }
    }

    /// Helper: multiply two 2x2 complex matrices.
    fn mat_mul_2x2(a: &[[Complex; 2]; 2], b: &[[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
        let mut result = [[Complex::ZERO; 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..2 {
                    result[i][j] = result[i][j] + a[i][k] * b[k][j];
                }
            }
        }
        result
    }
}
