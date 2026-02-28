//! # Browser-Native Quantum Reality Checks
//!
//! Verification circuits that let users test quantum claims locally.
//! If an AI says behavior is quantum-inspired, the user can verify it
//! against actual quantum mechanics in the browser.
//!
//! Collapses the gap between explanation and verification.

use ruqu_core::error::QuantumError;
use ruqu_core::gate::Gate;
use ruqu_core::state::QuantumState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// What property we expect to verify.
#[derive(Debug, Clone)]
pub enum ExpectedProperty {
    /// P(qubit = 0) ≈ expected ± tolerance
    ProbabilityZero {
        qubit: u32,
        expected: f64,
        tolerance: f64,
    },
    /// P(qubit = 1) ≈ expected ± tolerance
    ProbabilityOne {
        qubit: u32,
        expected: f64,
        tolerance: f64,
    },
    /// Two qubits are entangled: P(same outcome) > min_correlation
    Entangled {
        qubit_a: u32,
        qubit_b: u32,
        min_correlation: f64,
    },
    /// Qubit is in equal superposition: P(1) ≈ 0.5 ± tolerance
    EqualSuperposition { qubit: u32, tolerance: f64 },
    /// Full probability distribution matches ± tolerance
    InterferencePattern {
        probabilities: Vec<f64>,
        tolerance: f64,
    },
}

/// A quantum reality check: a named verification experiment.
pub struct RealityCheck {
    pub name: String,
    pub description: String,
    pub num_qubits: u32,
    pub expected: ExpectedProperty,
}

/// Result of running a reality check.
#[derive(Debug)]
pub struct CheckResult {
    pub check_name: String,
    pub passed: bool,
    pub measured_value: f64,
    pub expected_value: f64,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Verification engine
// ---------------------------------------------------------------------------

/// Run a verification circuit and check the expected property.
pub fn run_check<F>(check: &RealityCheck, circuit_fn: F) -> Result<CheckResult, QuantumError>
where
    F: FnOnce(&mut QuantumState) -> Result<(), QuantumError>,
{
    let mut state = QuantumState::new(check.num_qubits)?;
    circuit_fn(&mut state)?;

    let probs = state.probabilities();

    match &check.expected {
        ExpectedProperty::ProbabilityZero {
            qubit,
            expected,
            tolerance,
        } => {
            let p0 = 1.0 - state.probability_of_qubit(*qubit);
            let pass = (p0 - expected).abs() <= *tolerance;
            Ok(CheckResult {
                check_name: check.name.clone(),
                passed: pass,
                measured_value: p0,
                expected_value: *expected,
                detail: format!(
                    "P(q{}=0) = {:.6}, expected {:.6} +/- {:.6}",
                    qubit, p0, expected, tolerance
                ),
            })
        }
        ExpectedProperty::ProbabilityOne {
            qubit,
            expected,
            tolerance,
        } => {
            let p1 = state.probability_of_qubit(*qubit);
            let pass = (p1 - expected).abs() <= *tolerance;
            Ok(CheckResult {
                check_name: check.name.clone(),
                passed: pass,
                measured_value: p1,
                expected_value: *expected,
                detail: format!(
                    "P(q{}=1) = {:.6}, expected {:.6} +/- {:.6}",
                    qubit, p1, expected, tolerance
                ),
            })
        }
        ExpectedProperty::Entangled {
            qubit_a,
            qubit_b,
            min_correlation,
        } => {
            // Correlation = P(same outcome) = P(00) + P(11)
            let bit_a = 1usize << qubit_a;
            let bit_b = 1usize << qubit_b;
            let mut p_same = 0.0;
            for (i, &p) in probs.iter().enumerate() {
                let a = (i & bit_a) != 0;
                let b = (i & bit_b) != 0;
                if a == b {
                    p_same += p;
                }
            }
            let pass = p_same >= *min_correlation;
            Ok(CheckResult {
                check_name: check.name.clone(),
                passed: pass,
                measured_value: p_same,
                expected_value: *min_correlation,
                detail: format!(
                    "P(q{}==q{}) = {:.6}, min {:.6}",
                    qubit_a, qubit_b, p_same, min_correlation
                ),
            })
        }
        ExpectedProperty::EqualSuperposition { qubit, tolerance } => {
            let p1 = state.probability_of_qubit(*qubit);
            let pass = (p1 - 0.5).abs() <= *tolerance;
            Ok(CheckResult {
                check_name: check.name.clone(),
                passed: pass,
                measured_value: p1,
                expected_value: 0.5,
                detail: format!(
                    "P(q{}=1) = {:.6}, expected 0.5 +/- {:.6}",
                    qubit, p1, tolerance
                ),
            })
        }
        ExpectedProperty::InterferencePattern {
            probabilities: expected_probs,
            tolerance,
        } => {
            let max_diff: f64 = probs
                .iter()
                .zip(expected_probs.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);
            let pass = max_diff <= *tolerance;
            Ok(CheckResult {
                check_name: check.name.clone(),
                passed: pass,
                measured_value: max_diff,
                expected_value: 0.0,
                detail: format!(
                    "max |p_measured - p_expected| = {:.6}, tolerance {:.6}",
                    max_diff, tolerance
                ),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in verification circuits
// ---------------------------------------------------------------------------

/// Verify superposition: H|0⟩ should give 50/50.
pub fn check_superposition() -> CheckResult {
    let check = RealityCheck {
        name: "Superposition".into(),
        description: "H|0> produces equal superposition".into(),
        num_qubits: 1,
        expected: ExpectedProperty::EqualSuperposition {
            qubit: 0,
            tolerance: 1e-10,
        },
    };
    run_check(&check, |state| {
        state.apply_gate(&Gate::H(0))?;
        Ok(())
    })
    .unwrap()
}

/// Verify entanglement: Bell state |00⟩ + |11⟩ has perfect correlation.
pub fn check_entanglement() -> CheckResult {
    let check = RealityCheck {
        name: "Entanglement".into(),
        description: "Bell state has perfectly correlated measurements".into(),
        num_qubits: 2,
        expected: ExpectedProperty::Entangled {
            qubit_a: 0,
            qubit_b: 1,
            min_correlation: 0.99,
        },
    };
    run_check(&check, |state| {
        state.apply_gate(&Gate::H(0))?;
        state.apply_gate(&Gate::CNOT(0, 1))?;
        Ok(())
    })
    .unwrap()
}

/// Verify interference: H-Z-H = X, so |0⟩ → |1⟩.
/// Destructive interference on |0⟩, constructive on |1⟩.
pub fn check_interference() -> CheckResult {
    let check = RealityCheck {
        name: "Interference".into(),
        description: "H-Z-H = X: destructive interference eliminates |0>".into(),
        num_qubits: 1,
        expected: ExpectedProperty::ProbabilityOne {
            qubit: 0,
            expected: 1.0,
            tolerance: 1e-10,
        },
    };
    run_check(&check, |state| {
        state.apply_gate(&Gate::H(0))?;
        state.apply_gate(&Gate::Z(0))?;
        state.apply_gate(&Gate::H(0))?;
        Ok(())
    })
    .unwrap()
}

/// Verify phase kickback: Deutsch's algorithm for balanced f(x)=x.
/// Query qubit should measure |1⟩ with certainty.
pub fn check_phase_kickback() -> CheckResult {
    let check = RealityCheck {
        name: "Phase Kickback".into(),
        description: "Deutsch oracle for f(x)=x: phase kickback produces |1> on query qubit".into(),
        num_qubits: 2,
        expected: ExpectedProperty::ProbabilityOne {
            qubit: 0,
            expected: 1.0,
            tolerance: 1e-10,
        },
    };
    run_check(&check, |state| {
        // Prepare |01⟩
        state.apply_gate(&Gate::X(1))?;
        // Hadamard both
        state.apply_gate(&Gate::H(0))?;
        state.apply_gate(&Gate::H(1))?;
        // Oracle: f(x) = x → CNOT
        state.apply_gate(&Gate::CNOT(0, 1))?;
        // Final Hadamard on query
        state.apply_gate(&Gate::H(0))?;
        Ok(())
    })
    .unwrap()
}

/// Verify no-cloning: CNOT cannot copy a superposition.
/// If |ψ⟩ = H|0⟩ = |+⟩, then CNOT(0,1)|+,0⟩ = (|00⟩+|11⟩)/√2 (Bell state),
/// NOT |+,+⟩ = (|00⟩+|01⟩+|10⟩+|11⟩)/2.
///
/// We detect this by checking that qubit 1 is NOT in an equal superposition
/// independently — it is entangled with qubit 0, not an independent copy.
pub fn check_no_cloning() -> CheckResult {
    let check = RealityCheck {
        name: "No-Cloning".into(),
        description:
            "CNOT cannot independently copy a superposition (produces entanglement instead)".into(),
        num_qubits: 2,
        expected: ExpectedProperty::InterferencePattern {
            // Bell state: P(00) = 0.5, P(01) = 0, P(10) = 0, P(11) = 0.5
            // If cloning worked: P(00) = 0.25, P(01) = 0.25, P(10) = 0.25, P(11) = 0.25
            probabilities: vec![0.5, 0.0, 0.0, 0.5],
            tolerance: 1e-10,
        },
    };
    run_check(&check, |state| {
        state.apply_gate(&Gate::H(0))?;
        state.apply_gate(&Gate::CNOT(0, 1))?;
        Ok(())
    })
    .unwrap()
}

/// Run all built-in checks and return results.
pub fn run_all_checks() -> Vec<CheckResult> {
    vec![
        check_superposition(),
        check_entanglement(),
        check_interference(),
        check_phase_kickback(),
        check_no_cloning(),
    ]
}
