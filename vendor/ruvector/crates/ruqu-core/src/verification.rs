//! Cross-backend automatic verification for quantum circuit simulation.
//!
//! This module provides tools to verify simulation results by running circuits
//! on multiple backends and comparing their output distributions. For pure
//! Clifford circuits, the stabilizer backend serves as an efficient reference
//! implementation that can be compared bitwise against the state-vector backend.
//!
//! # Verification levels
//!
//! | Level | Method | When used |
//! |-------|--------|-----------|
//! | Exact | Bitwise match of distributions | Clifford circuits, <= 25 qubits |
//! | Statistical | Chi-squared + TVD | General comparison of two distributions |
//! | Trend | Correlation of energy landscape | Future: Hamiltonian-level comparison |
//! | Skipped | N/A | Non-Clifford or no reference available |

use crate::backend::{analyze_circuit, BackendType};
use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use crate::simulator::Simulator;
use crate::stabilizer::StabilizerState;

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// How rigorously the verification was performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationLevel {
    /// Bitwise match (Clifford circuits: stabilizer vs state vector).
    Exact,
    /// Chi-squared test within tolerance.
    Statistical,
    /// Correlation of energy landscape.
    Trend,
    /// Verification not applicable.
    Skipped,
}

/// Outcome of a cross-backend verification run.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// The level of verification that was performed.
    pub level: VerificationLevel,
    /// Whether the verification passed.
    pub passed: bool,
    /// The backend used for the primary simulation.
    pub primary_backend: BackendType,
    /// The backend used for the reference simulation, if any.
    pub reference_backend: Option<BackendType>,
    /// Total variation distance between the two distributions.
    pub total_variation_distance: Option<f64>,
    /// P-value from the chi-squared goodness-of-fit test.
    pub chi_squared_p_value: Option<f64>,
    /// Pearson correlation coefficient between distributions.
    pub correlation: Option<f64>,
    /// Human-readable explanation of the verification outcome.
    pub explanation: String,
    /// Individual bitstring discrepancies, sorted by absolute difference.
    pub discrepancies: Vec<Discrepancy>,
}

/// A single bitstring where the primary and reference distributions disagree.
#[derive(Debug, Clone)]
pub struct Discrepancy {
    /// The bitstring (one bool per qubit, qubit 0 first).
    pub bitstring: Vec<bool>,
    /// Probability of this bitstring in the primary distribution.
    pub primary_probability: f64,
    /// Probability of this bitstring in the reference distribution.
    pub reference_probability: f64,
    /// Absolute difference between the two probabilities.
    pub absolute_difference: f64,
}

// ---------------------------------------------------------------------------
// Main verification entry point
// ---------------------------------------------------------------------------

/// Verify a quantum circuit by running it on multiple backends and comparing
/// the resulting measurement distributions.
///
/// # Algorithm
///
/// 1. Analyze the circuit to determine its Clifford fraction.
/// 2. If the circuit is pure Clifford AND has <= 25 qubits, run on both the
///    state-vector and stabilizer backends, then compare the distributions at
///    the Exact level.
/// 3. If the circuit is NOT pure Clifford AND has <= 25 qubits, run on the
///    state-vector backend only and report verification as Skipped.
/// 4. For circuits exceeding 25 qubits, report as Skipped.
///
/// # Arguments
///
/// * `circuit` - The quantum circuit to verify.
/// * `shots` - Number of measurement shots per backend.
/// * `seed` - Deterministic seed for reproducibility.
pub fn verify_circuit(circuit: &QuantumCircuit, shots: u32, seed: u64) -> VerificationResult {
    let analysis = analyze_circuit(circuit);
    let num_qubits = circuit.num_qubits();
    let is_clifford = is_clifford_circuit(circuit);

    // Case 1: Pure Clifford AND small enough for state vector comparison.
    if is_clifford && num_qubits <= 25 {
        // Run on state-vector backend.
        let sv_result = Simulator::run_shots(circuit, shots, Some(seed));
        let sv_counts = match sv_result {
            Ok(r) => r.counts,
            Err(e) => {
                return VerificationResult {
                    level: VerificationLevel::Skipped,
                    passed: false,
                    primary_backend: BackendType::StateVector,
                    reference_backend: None,
                    total_variation_distance: None,
                    chi_squared_p_value: None,
                    correlation: None,
                    explanation: format!("State-vector simulation failed: {}", e),
                    discrepancies: vec![],
                };
            }
        };

        // Run on stabilizer backend.
        let stab_counts = run_stabilizer_shots(circuit, shots, seed);

        // Compare the two distributions.
        let mut result = verify_against_reference(
            &sv_counts,
            &stab_counts,
            0.0, // Exact match: zero tolerance for Clifford circuits
        );

        result.primary_backend = BackendType::StateVector;
        result.reference_backend = Some(BackendType::Stabilizer);

        // Upgrade to Exact level if the distributions match perfectly.
        if result.passed && result.total_variation_distance.map_or(false, |d| d == 0.0) {
            result.level = VerificationLevel::Exact;
            result.explanation = format!(
                "Exact match: {}-qubit Clifford circuit verified across \
                 state-vector and stabilizer backends ({} shots, \
                 clifford_fraction={:.2})",
                num_qubits, shots, analysis.clifford_fraction
            );
        } else {
            // Even for Clifford circuits, sampling noise may cause small
            // differences. Use statistical comparison with a tight tolerance.
            let tight_tolerance = 0.05;
            let mut stat_result =
                verify_against_reference(&sv_counts, &stab_counts, tight_tolerance);
            stat_result.primary_backend = BackendType::StateVector;
            stat_result.reference_backend = Some(BackendType::Stabilizer);
            stat_result.explanation = format!(
                "Statistical comparison of {}-qubit Clifford circuit across \
                 state-vector and stabilizer backends ({} shots, TVD={:.6})",
                num_qubits,
                shots,
                stat_result.total_variation_distance.unwrap_or(0.0)
            );
            return stat_result;
        }

        return result;
    }

    // Case 2: Not Clifford AND small enough for state vector.
    if !is_clifford && num_qubits <= 25 {
        return VerificationResult {
            level: VerificationLevel::Skipped,
            passed: true,
            primary_backend: BackendType::StateVector,
            reference_backend: None,
            total_variation_distance: None,
            chi_squared_p_value: None,
            correlation: None,
            explanation: format!(
                "Verification skipped: {}-qubit circuit contains non-Clifford \
                 gates (clifford_fraction={:.2}, {} non-Clifford gates). \
                 No reference backend available for cross-validation.",
                num_qubits, analysis.clifford_fraction, analysis.non_clifford_gates
            ),
            discrepancies: vec![],
        };
    }

    // Case 3: Too many qubits for state-vector comparison.
    VerificationResult {
        level: VerificationLevel::Skipped,
        passed: true,
        primary_backend: analysis.recommended_backend,
        reference_backend: None,
        total_variation_distance: None,
        chi_squared_p_value: None,
        correlation: None,
        explanation: format!(
            "Verification skipped: {}-qubit circuit exceeds state-vector \
             capacity for cross-backend comparison.",
            num_qubits
        ),
        discrepancies: vec![],
    }
}

// ---------------------------------------------------------------------------
// Distribution comparison
// ---------------------------------------------------------------------------

/// Compare two measurement distributions and produce a verification result.
///
/// # Arguments
///
/// * `primary` - Counts from the primary backend.
/// * `reference` - Counts from the reference backend.
/// * `tolerance` - Maximum allowed total variation distance for a pass.
///
/// # Returns
///
/// A `VerificationResult` at the `Statistical` level (or `Exact` if TVD is
/// exactly zero and tolerance is zero).
pub fn verify_against_reference(
    primary: &HashMap<Vec<bool>, usize>,
    reference: &HashMap<Vec<bool>, usize>,
    tolerance: f64,
) -> VerificationResult {
    let p_norm = normalize_counts(primary);
    let q_norm = normalize_counts(reference);

    let distance = tvd(&p_norm, &q_norm);

    let total_ref: usize = reference.values().sum();
    let (chi2_stat, dof) = chi_squared_statistic(primary, &q_norm, total_ref);
    let p_value = if dof > 0 {
        chi_squared_p_value(chi2_stat, dof)
    } else {
        1.0
    };

    let corr = pearson_correlation(&p_norm, &q_norm);

    // Build sorted discrepancy list.
    let mut all_keys: Vec<&Vec<bool>> = p_norm.keys().chain(q_norm.keys()).collect();
    all_keys.sort();
    all_keys.dedup();

    let mut discrepancies: Vec<Discrepancy> = all_keys
        .iter()
        .map(|key| {
            let pp = p_norm.get(*key).copied().unwrap_or(0.0);
            let rp = q_norm.get(*key).copied().unwrap_or(0.0);
            Discrepancy {
                bitstring: (*key).clone(),
                primary_probability: pp,
                reference_probability: rp,
                absolute_difference: (pp - rp).abs(),
            }
        })
        .filter(|d| d.absolute_difference > 1e-15)
        .collect();

    // Sort by absolute difference, descending.
    discrepancies.sort_by(|a, b| {
        b.absolute_difference
            .partial_cmp(&a.absolute_difference)
            .unwrap()
    });

    let passed = distance <= tolerance;

    let level = if tolerance == 0.0 && passed {
        VerificationLevel::Exact
    } else {
        VerificationLevel::Statistical
    };

    let explanation = if passed {
        format!(
            "Verification passed: TVD={:.6}, chi2 p-value={:.4}, \
             correlation={:.4}, tolerance={:.6}",
            distance, p_value, corr, tolerance
        )
    } else {
        format!(
            "Verification FAILED: TVD={:.6} exceeds tolerance={:.6}, \
             chi2 p-value={:.4}, correlation={:.4}, \
             {} discrepancies found",
            distance,
            tolerance,
            p_value,
            corr,
            discrepancies.len()
        )
    };

    VerificationResult {
        level,
        passed,
        primary_backend: BackendType::Auto,
        reference_backend: None,
        total_variation_distance: Some(distance),
        chi_squared_p_value: Some(p_value),
        correlation: Some(corr),
        explanation,
        discrepancies,
    }
}

// ---------------------------------------------------------------------------
// Clifford circuit detection
// ---------------------------------------------------------------------------

/// Check if ALL gates in a circuit are Clifford-compatible.
///
/// Clifford-compatible gates are: H, X, Y, Z, S, Sdg, CNOT, CZ, SWAP,
/// Measure, Reset, and Barrier. Any other gate (T, Tdg, rotations, custom
/// unitaries) makes the circuit non-Clifford.
pub fn is_clifford_circuit(circuit: &QuantumCircuit) -> bool {
    circuit.gates().iter().all(|gate| is_clifford_gate(gate))
}

/// Check if a single gate is Clifford-compatible.
fn is_clifford_gate(gate: &Gate) -> bool {
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
            | Gate::Reset(_)
            | Gate::Barrier
    )
}

// ---------------------------------------------------------------------------
// Stabilizer shot execution
// ---------------------------------------------------------------------------

/// Execute a Clifford circuit on the stabilizer backend for multiple shots.
///
/// For each shot, creates a fresh `StabilizerState`, applies all gates in
/// order, and collects measurement outcomes into a histogram. If the circuit
/// contains no explicit `Measure` gates, all qubits are measured at the end.
///
/// `Reset` gates are handled by measuring the qubit and conditionally
/// applying an X gate to force it back to |0>.
///
/// # Panics
///
/// Panics if a non-Clifford gate is encountered (the caller must ensure the
/// circuit is Clifford-only via `is_clifford_circuit`).
pub fn run_stabilizer_shots(
    circuit: &QuantumCircuit,
    shots: u32,
    seed: u64,
) -> HashMap<Vec<bool>, usize> {
    let n = circuit.num_qubits() as usize;
    let mut counts: HashMap<Vec<bool>, usize> = HashMap::new();

    let has_measurements = circuit
        .gates()
        .iter()
        .any(|g| matches!(g, Gate::Measure(_)));

    for shot in 0..shots {
        let shot_seed = seed.wrapping_add(shot as u64);
        let mut state = StabilizerState::new_with_seed(n, shot_seed)
            .expect("failed to create stabilizer state");

        let mut measured_bits: Vec<Option<bool>> = vec![None; n];

        for gate in circuit.gates() {
            match gate {
                Gate::Reset(q) => {
                    // Implement reset: measure, then conditionally flip.
                    let qubit = *q as usize;
                    let outcome = state.measure(qubit).expect("stabilizer measurement failed");
                    if outcome.result {
                        state.x_gate(qubit);
                    }
                    // Clear the measured bit since reset puts qubit back to |0>.
                    measured_bits[qubit] = None;
                }
                Gate::Measure(q) => {
                    let outcomes = state
                        .apply_gate(gate)
                        .expect("stabilizer gate application failed");
                    if let Some(outcome) = outcomes.first() {
                        measured_bits[*q as usize] = Some(outcome.result);
                    }
                }
                _ => {
                    state
                        .apply_gate(gate)
                        .expect("stabilizer gate application failed");
                }
            }
        }

        // If no explicit measurements, measure all qubits.
        if !has_measurements {
            for q in 0..n {
                let outcome = state.measure(q).expect("stabilizer measurement failed");
                measured_bits[q] = Some(outcome.result);
            }
        }

        // Build the bit-vector for this shot.
        let bits: Vec<bool> = measured_bits.iter().map(|mb| mb.unwrap_or(false)).collect();

        *counts.entry(bits).or_insert(0) += 1;
    }

    counts
}

// ---------------------------------------------------------------------------
// Helper functions: distribution normalization and metrics
// ---------------------------------------------------------------------------

/// Convert raw counts to a probability distribution.
///
/// Each count is divided by the total number of shots to produce a
/// probability in [0, 1].
pub fn normalize_counts(counts: &HashMap<Vec<bool>, usize>) -> HashMap<Vec<bool>, f64> {
    let total: usize = counts.values().sum();
    if total == 0 {
        return HashMap::new();
    }
    let total_f = total as f64;
    counts
        .iter()
        .map(|(k, &v)| (k.clone(), v as f64 / total_f))
        .collect()
}

/// Compute the total variation distance between two probability distributions.
///
/// TVD = 0.5 * sum_x |p(x) - q(x)|
///
/// Returns a value in [0, 1] where 0 means identical distributions and 1
/// means completely disjoint support.
pub fn tvd(p: &HashMap<Vec<bool>, f64>, q: &HashMap<Vec<bool>, f64>) -> f64 {
    let mut all_keys: Vec<&Vec<bool>> = p.keys().chain(q.keys()).collect();
    all_keys.sort();
    all_keys.dedup();

    let sum: f64 = all_keys
        .iter()
        .map(|key| {
            let pv = p.get(*key).copied().unwrap_or(0.0);
            let qv = q.get(*key).copied().unwrap_or(0.0);
            (pv - qv).abs()
        })
        .sum();

    0.5 * sum
}

/// Compute the chi-squared statistic for a goodness-of-fit test.
///
/// Tests whether the observed counts (from the primary distribution) are
/// consistent with the expected probabilities (from the reference
/// distribution).
///
/// Returns `(statistic, degrees_of_freedom)`. Bins with an expected count
/// below 5 are merged into an "other" bin to maintain test validity.
///
/// # Arguments
///
/// * `observed` - Raw counts from the primary distribution.
/// * `expected_probs` - Probability distribution from the reference.
/// * `total` - Total number of reference shots (used to scale expected probs
///   to expected counts).
pub fn chi_squared_statistic(
    observed: &HashMap<Vec<bool>, usize>,
    expected_probs: &HashMap<Vec<bool>, f64>,
    _total: usize,
) -> (f64, usize) {
    let obs_total: usize = observed.values().sum();
    if obs_total == 0 {
        return (0.0, 0);
    }
    let obs_total_f = obs_total as f64;

    let mut all_keys: Vec<&Vec<bool>> = observed.keys().chain(expected_probs.keys()).collect();
    all_keys.sort();
    all_keys.dedup();

    let mut chi2 = 0.0;
    let mut bins_used = 0usize;
    let mut other_observed = 0.0;
    let mut other_expected = 0.0;

    for key in &all_keys {
        let obs = observed.get(*key).copied().unwrap_or(0) as f64;
        let exp_prob = expected_probs.get(*key).copied().unwrap_or(0.0);
        let exp = exp_prob * obs_total_f;

        if exp < 5.0 {
            // Merge into the "other" bin.
            other_observed += obs;
            other_expected += exp;
        } else {
            let diff = obs - exp;
            chi2 += (diff * diff) / exp;
            bins_used += 1;
        }
    }

    // Process the merged "other" bin.
    if other_expected >= 5.0 {
        let diff = other_observed - other_expected;
        chi2 += (diff * diff) / other_expected;
        bins_used += 1;
    } else if other_expected > 0.0 && other_observed > 0.0 {
        // Small expected count; include but note reduced reliability.
        let diff = other_observed - other_expected;
        chi2 += (diff * diff) / other_expected.max(1.0);
        bins_used += 1;
    }

    // Degrees of freedom = number of bins - 1 (constraint: totals match).
    let dof = if bins_used > 1 { bins_used - 1 } else { 0 };

    (chi2, dof)
}

/// Approximate the chi-squared p-value using the Wilson-Hilferty
/// normal approximation.
///
/// For a chi-squared random variable X with k degrees of freedom:
///
/// ```text
/// z = ((X/k)^(1/3) - (1 - 2/(9k))) / sqrt(2/(9k))
/// ```
///
/// The p-value is then `1 - Phi(z)` where `Phi` is the standard normal CDF.
///
/// This approximation is accurate for k >= 3 and reasonable for k >= 1.
pub fn chi_squared_p_value(statistic: f64, dof: usize) -> f64 {
    if dof == 0 {
        return 1.0;
    }
    if statistic <= 0.0 {
        return 1.0;
    }

    let k = dof as f64;

    // Wilson-Hilferty approximation.
    let term = 2.0 / (9.0 * k);
    let cube_root = (statistic / k).powf(1.0 / 3.0);
    let z = (cube_root - (1.0 - term)) / term.sqrt();

    // Standard normal survival function: 1 - Phi(z).
    // Use the complementary error function approximation.
    1.0 - standard_normal_cdf(z)
}

// ---------------------------------------------------------------------------
// Pearson correlation
// ---------------------------------------------------------------------------

/// Compute the Pearson correlation coefficient between two distributions.
///
/// Returns a value in [-1, 1]. Returns 0.0 if either distribution has zero
/// variance (constant).
fn pearson_correlation(p: &HashMap<Vec<bool>, f64>, q: &HashMap<Vec<bool>, f64>) -> f64 {
    let mut all_keys: Vec<&Vec<bool>> = p.keys().chain(q.keys()).collect();
    all_keys.sort();
    all_keys.dedup();

    if all_keys.is_empty() {
        return 0.0;
    }

    let n = all_keys.len() as f64;

    let p_vals: Vec<f64> = all_keys
        .iter()
        .map(|k| p.get(*k).copied().unwrap_or(0.0))
        .collect();
    let q_vals: Vec<f64> = all_keys
        .iter()
        .map(|k| q.get(*k).copied().unwrap_or(0.0))
        .collect();

    let p_mean: f64 = p_vals.iter().sum::<f64>() / n;
    let q_mean: f64 = q_vals.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_p = 0.0;
    let mut var_q = 0.0;

    for i in 0..all_keys.len() {
        let dp = p_vals[i] - p_mean;
        let dq = q_vals[i] - q_mean;
        cov += dp * dq;
        var_p += dp * dp;
        var_q += dq * dq;
    }

    if var_p < 1e-30 || var_q < 1e-30 {
        return 0.0;
    }

    cov / (var_p.sqrt() * var_q.sqrt())
}

// ---------------------------------------------------------------------------
// Standard normal CDF approximation
// ---------------------------------------------------------------------------

/// Approximate the standard normal CDF using the Abramowitz and Stegun
/// rational approximation (formula 26.2.17).
///
/// Accurate to approximately 7.5 decimal digits.
fn standard_normal_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }

    // Constants for the approximation.
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x.abs() / std::f64::consts::SQRT_2;

    let t = 1.0 / (1.0 + p * abs_x);
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;

    let erf_approx =
        1.0 - (a1 * t + a2 * t2 + a3 * t3 + a4 * t4 + a5 * t5) * (-abs_x * abs_x).exp();

    0.5 * (1.0 + sign * erf_approx)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;

    // -- Helper to build a count map from a list of (bitstring, count) pairs --

    fn make_counts(entries: &[(&[bool], usize)]) -> HashMap<Vec<bool>, usize> {
        entries
            .iter()
            .map(|(bits, count)| (bits.to_vec(), *count))
            .collect()
    }

    // -----------------------------------------------------------------------
    // is_clifford_circuit
    // -----------------------------------------------------------------------

    #[test]
    fn clifford_circuit_returns_true_for_clifford_only() {
        let mut circ = QuantumCircuit::new(3);
        circ.h(0).cnot(0, 1).s(2).x(0).y(1).z(2);
        circ.cz(0, 2).swap(1, 2);
        circ.measure(0).measure(1).measure(2);
        assert!(is_clifford_circuit(&circ));
    }

    #[test]
    fn clifford_circuit_returns_false_with_t_gate() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).t(0).cnot(0, 1);
        assert!(!is_clifford_circuit(&circ));
    }

    #[test]
    fn clifford_circuit_returns_true_for_sdg_gate() {
        let mut circ = QuantumCircuit::new(1);
        circ.h(0);
        circ.add_gate(Gate::Sdg(0));
        assert!(is_clifford_circuit(&circ));
    }

    #[test]
    fn clifford_circuit_returns_false_for_rx_gate() {
        let mut circ = QuantumCircuit::new(1);
        circ.rx(0, 0.5);
        assert!(!is_clifford_circuit(&circ));
    }

    #[test]
    fn clifford_circuit_returns_true_with_reset_and_barrier() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1).barrier();
        circ.reset(0).measure(1);
        assert!(is_clifford_circuit(&circ));
    }

    // -----------------------------------------------------------------------
    // normalize_counts
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_counts_produces_probabilities() {
        let counts = make_counts(&[(&[false, false], 50), (&[true, true], 50)]);
        let probs = normalize_counts(&counts);
        assert!((probs[&vec![false, false]] - 0.5).abs() < 1e-10);
        assert!((probs[&vec![true, true]] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn normalize_counts_empty_returns_empty() {
        let counts: HashMap<Vec<bool>, usize> = HashMap::new();
        let probs = normalize_counts(&counts);
        assert!(probs.is_empty());
    }

    // -----------------------------------------------------------------------
    // tvd
    // -----------------------------------------------------------------------

    #[test]
    fn identical_distributions_have_zero_tvd() {
        let p: HashMap<Vec<bool>, f64> = [(vec![false, false], 0.5), (vec![true, true], 0.5)]
            .into_iter()
            .collect();

        let distance = tvd(&p, &p);
        assert!(
            distance.abs() < 1e-15,
            "TVD of identical distributions should be 0, got {}",
            distance
        );
    }

    #[test]
    fn completely_different_distributions_have_tvd_near_one() {
        let p: HashMap<Vec<bool>, f64> = [(vec![false], 1.0)].into_iter().collect();
        let q: HashMap<Vec<bool>, f64> = [(vec![true], 1.0)].into_iter().collect();

        let distance = tvd(&p, &q);
        assert!(
            (distance - 1.0).abs() < 1e-15,
            "TVD of disjoint distributions should be 1, got {}",
            distance
        );
    }

    #[test]
    fn tvd_partial_overlap() {
        let p: HashMap<Vec<bool>, f64> = [(vec![false], 0.7), (vec![true], 0.3)]
            .into_iter()
            .collect();

        let q: HashMap<Vec<bool>, f64> = [(vec![false], 0.3), (vec![true], 0.7)]
            .into_iter()
            .collect();

        let distance = tvd(&p, &q);
        // TVD = 0.5 * (|0.7-0.3| + |0.3-0.7|) = 0.5 * (0.4 + 0.4) = 0.4
        assert!(
            (distance - 0.4).abs() < 1e-15,
            "Expected TVD=0.4, got {}",
            distance
        );
    }

    // -----------------------------------------------------------------------
    // chi_squared_statistic and chi_squared_p_value
    // -----------------------------------------------------------------------

    #[test]
    fn chi_squared_perfect_fit_has_low_statistic() {
        let observed = make_counts(&[(&[false], 500), (&[true], 500)]);
        let expected: HashMap<Vec<bool>, f64> = [(vec![false], 0.5), (vec![true], 0.5)]
            .into_iter()
            .collect();

        let (stat, dof) = chi_squared_statistic(&observed, &expected, 1000);
        assert!(
            stat < 1.0,
            "Perfect fit should have near-zero chi2, got {}",
            stat
        );
        assert_eq!(dof, 1);

        let pval = chi_squared_p_value(stat, dof);
        assert!(
            pval > 0.05,
            "Perfect fit p-value should be large, got {}",
            pval
        );
    }

    #[test]
    fn chi_squared_bad_fit_has_high_statistic() {
        // Observed is heavily biased; expected is uniform.
        let observed = make_counts(&[(&[false], 900), (&[true], 100)]);
        let expected: HashMap<Vec<bool>, f64> = [(vec![false], 0.5), (vec![true], 0.5)]
            .into_iter()
            .collect();

        let (stat, dof) = chi_squared_statistic(&observed, &expected, 1000);
        assert!(stat > 10.0, "Bad fit should have large chi2, got {}", stat);
        assert_eq!(dof, 1);

        let pval = chi_squared_p_value(stat, dof);
        assert!(
            pval < 0.01,
            "Bad fit p-value should be very small, got {}",
            pval
        );
    }

    #[test]
    fn chi_squared_p_value_zero_dof() {
        let pval = chi_squared_p_value(5.0, 0);
        assert!((pval - 1.0).abs() < 1e-10);
    }

    #[test]
    fn chi_squared_p_value_zero_statistic() {
        let pval = chi_squared_p_value(0.0, 5);
        assert!((pval - 1.0).abs() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // verify_against_reference
    // -----------------------------------------------------------------------

    #[test]
    fn identical_distributions_pass_verification() {
        let counts = make_counts(&[(&[false, false], 500), (&[true, true], 500)]);
        let result = verify_against_reference(&counts, &counts, 0.01);
        assert!(result.passed);
        assert!(
            result.total_variation_distance.unwrap() < 1e-10,
            "TVD should be 0 for identical counts"
        );
    }

    #[test]
    fn very_different_distributions_fail_verification() {
        let primary = make_counts(&[(&[false], 1000)]);
        let reference = make_counts(&[(&[true], 1000)]);

        let result = verify_against_reference(&primary, &reference, 0.1);
        assert!(!result.passed);
        assert!(
            (result.total_variation_distance.unwrap() - 1.0).abs() < 1e-10,
            "TVD should be 1 for disjoint distributions"
        );
    }

    #[test]
    fn discrepancies_sorted_by_absolute_difference() {
        let primary = make_counts(&[
            (&[false, false], 400),
            (&[false, true], 300),
            (&[true, false], 200),
            (&[true, true], 100),
        ]);
        let reference = make_counts(&[
            (&[false, false], 250),
            (&[false, true], 250),
            (&[true, false], 250),
            (&[true, true], 250),
        ]);

        let result = verify_against_reference(&primary, &reference, 0.5);

        // Verify discrepancies are sorted descending by absolute_difference.
        for i in 1..result.discrepancies.len() {
            assert!(
                result.discrepancies[i - 1].absolute_difference
                    >= result.discrepancies[i].absolute_difference,
                "Discrepancies should be sorted descending by \
                 absolute_difference: {} < {}",
                result.discrepancies[i - 1].absolute_difference,
                result.discrepancies[i].absolute_difference
            );
        }

        // The largest discrepancy should be for [false, false] or [true, true].
        // primary [false,false] = 0.4, reference = 0.25, diff = 0.15
        // primary [true,true] = 0.1, reference = 0.25, diff = 0.15
        // primary [false,true] = 0.3, reference = 0.25, diff = 0.05
        // primary [true,false] = 0.2, reference = 0.25, diff = 0.05
        assert!(
            result.discrepancies[0].absolute_difference >= 0.14,
            "Top discrepancy should have absolute_difference >= 0.14, got {}",
            result.discrepancies[0].absolute_difference
        );
    }

    // -----------------------------------------------------------------------
    // run_stabilizer_shots
    // -----------------------------------------------------------------------

    #[test]
    fn stabilizer_shots_zero_state_gives_all_zeros() {
        // Circuit with no gates, just measure all qubits.
        let mut circ = QuantumCircuit::new(3);
        circ.measure(0).measure(1).measure(2);

        let counts = run_stabilizer_shots(&circ, 100, 42);

        // All outcomes should be [false, false, false].
        assert_eq!(counts.len(), 1, "Should have exactly one outcome");
        assert_eq!(
            counts[&vec![false, false, false]],
            100,
            "All 100 shots should give |000>"
        );
    }

    #[test]
    fn stabilizer_shots_bell_state_gives_correlated_results() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1).measure(0).measure(1);

        let counts = run_stabilizer_shots(&circ, 1000, 42);

        // A Bell state should only produce |00> and |11>.
        for (bits, _count) in &counts {
            assert_eq!(
                bits[0], bits[1],
                "Bell state qubits must be correlated, got {:?}",
                bits
            );
        }

        // Both outcomes should appear (with high probability at 1000 shots).
        assert!(
            counts.contains_key(&vec![false, false]),
            "Should see |00> outcome"
        );
        assert!(
            counts.contains_key(&vec![true, true]),
            "Should see |11> outcome"
        );

        // Check roughly 50/50 split (within a generous margin).
        let count_00 = counts.get(&vec![false, false]).copied().unwrap_or(0);
        let count_11 = counts.get(&vec![true, true]).copied().unwrap_or(0);
        assert_eq!(count_00 + count_11, 1000);
        assert!(
            count_00 > 350 && count_00 < 650,
            "Expected roughly 50/50, got {}/{}",
            count_00,
            count_11
        );
    }

    #[test]
    fn stabilizer_shots_implicit_measurement() {
        // No explicit measure gates: all qubits measured at the end.
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1);

        let counts = run_stabilizer_shots(&circ, 500, 99);

        // Bell state: only |00> and |11> should appear.
        for (bits, _count) in &counts {
            assert_eq!(bits[0], bits[1], "Bell state must be correlated");
        }
    }

    // -----------------------------------------------------------------------
    // verify_circuit (integration tests)
    // -----------------------------------------------------------------------

    #[test]
    fn bell_state_passes_exact_verification() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1).measure(0).measure(1);

        let result = verify_circuit(&circ, 2000, 42);

        assert_eq!(result.primary_backend, BackendType::StateVector);
        assert_eq!(result.reference_backend, Some(BackendType::Stabilizer));
        assert!(
            result.passed,
            "Bell state should pass verification: {}",
            result.explanation
        );
        // Should be Exact or Statistical (both acceptable for Clifford).
        assert!(
            result.level == VerificationLevel::Exact
                || result.level == VerificationLevel::Statistical,
            "Expected Exact or Statistical, got {:?}",
            result.level
        );
    }

    #[test]
    fn non_clifford_circuit_is_skipped() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).t(0).cnot(0, 1).measure(0).measure(1);

        let result = verify_circuit(&circ, 1000, 42);

        assert_eq!(result.level, VerificationLevel::Skipped);
        assert!(result.reference_backend.is_none());
        assert!(
            result.explanation.contains("non-Clifford"),
            "Explanation should mention non-Clifford gates: {}",
            result.explanation
        );
    }

    #[test]
    fn ghz_state_passes_verification() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0);
        circ.cnot(0, 1).cnot(1, 2).cnot(2, 3);
        circ.measure(0).measure(1).measure(2).measure(3);

        let result = verify_circuit(&circ, 2000, 123);

        assert!(
            result.passed,
            "GHZ state should pass verification: {}",
            result.explanation
        );
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn empty_circuit_passes_verification() {
        let mut circ = QuantumCircuit::new(2);
        circ.measure(0).measure(1);

        let result = verify_circuit(&circ, 100, 0);

        assert!(result.passed);
        // Pure Clifford (only measurements), should do cross-backend check.
        assert_eq!(result.reference_backend, Some(BackendType::Stabilizer));
    }

    #[test]
    fn pearson_correlation_identical_distributions() {
        let p: HashMap<Vec<bool>, f64> = [(vec![false], 0.3), (vec![true], 0.7)]
            .into_iter()
            .collect();

        let corr = pearson_correlation(&p, &p);
        assert!(
            (corr - 1.0).abs() < 1e-10,
            "Identical distributions should have correlation 1.0, got {}",
            corr
        );
    }

    #[test]
    fn standard_normal_cdf_known_values() {
        // Phi(0) = 0.5
        assert!(
            (standard_normal_cdf(0.0) - 0.5).abs() < 1e-6,
            "CDF(0) should be 0.5"
        );
        // Phi(-inf) -> 0
        assert!(
            standard_normal_cdf(-10.0) < 1e-10,
            "CDF(-10) should be near 0"
        );
        // Phi(+inf) -> 1
        assert!(
            (standard_normal_cdf(10.0) - 1.0).abs() < 1e-10,
            "CDF(10) should be near 1"
        );
        // Phi(1.96) ~ 0.975
        assert!(
            (standard_normal_cdf(1.96) - 0.975).abs() < 0.01,
            "CDF(1.96) should be near 0.975, got {}",
            standard_normal_cdf(1.96)
        );
    }
}
