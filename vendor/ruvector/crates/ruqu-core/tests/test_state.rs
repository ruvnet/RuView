//! Tests for ruqu_core::state — quantum state evolution, measurement, expectation values.

use ruqu_core::prelude::*;

const EPSILON: f64 = 1e-10;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

#[test]
fn test_initial_state_single_qubit() {
    // |0>: amplitude of |0> = 1, amplitude of |1> = 0
    let state = QuantumState::new(1).unwrap();
    let sv = state.state_vector();
    assert_eq!(sv.len(), 2);
    assert!(approx_eq(sv[0].norm_sq(), 1.0));
    assert!(approx_eq(sv[1].norm_sq(), 0.0));
}

#[test]
fn test_initial_state_two_qubits() {
    // |00>: amplitude of |00> = 1, rest = 0
    let state = QuantumState::new(2).unwrap();
    let sv = state.state_vector();
    assert_eq!(sv.len(), 4);
    assert!(approx_eq(sv[0].norm_sq(), 1.0));
    for i in 1..4 {
        assert!(approx_eq(sv[i].norm_sq(), 0.0), "sv[{}] should be 0", i);
    }
}

#[test]
fn test_initial_state_three_qubits() {
    let state = QuantumState::new(3).unwrap();
    let sv = state.state_vector();
    assert_eq!(sv.len(), 8);
    assert!(approx_eq(sv[0].norm_sq(), 1.0));
    for i in 1..8 {
        assert!(approx_eq(sv[i].norm_sq(), 0.0));
    }
}

#[test]
fn test_num_qubits() {
    let state = QuantumState::new(5).unwrap();
    assert_eq!(state.num_qubits(), 5);
}

#[test]
fn test_initial_probabilities() {
    let state = QuantumState::new(2).unwrap();
    let probs = state.probabilities();
    assert_eq!(probs.len(), 4);
    assert!(approx_eq(probs[0], 1.0));
    assert!(approx_eq(probs[1], 0.0));
    assert!(approx_eq(probs[2], 0.0));
    assert!(approx_eq(probs[3], 0.0));
}

// ---------------------------------------------------------------------------
// Hadamard gate
// ---------------------------------------------------------------------------

#[test]
fn test_hadamard_creates_superposition() {
    // H|0> = (|0> + |1>) / sqrt(2)
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5));
    assert!(approx_eq(probs[1], 0.5));
}

#[test]
fn test_hadamard_amplitudes() {
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let sv = state.state_vector();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    assert!(approx_eq(sv[0].re, s));
    assert!(approx_eq(sv[0].im, 0.0));
    assert!(approx_eq(sv[1].re, s));
    assert!(approx_eq(sv[1].im, 0.0));
}

#[test]
fn test_double_hadamard_returns_to_zero() {
    // H*H|0> = |0>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0));
    assert!(approx_eq(probs[1], 0.0));
}

#[test]
fn test_hadamard_on_qubit_1_of_2() {
    // |00> -> H on qubit 1 -> superposition on qubit 1
    // Little-endian: qubit 1 = bit 1, so indices 0 (q1=0) and 2 (q1=1) get 0.5
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(1)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5)); // q0=0,q1=0
    assert!(approx_eq(probs[1], 0.0)); // q0=1,q1=0
    assert!(approx_eq(probs[2], 0.5)); // q0=0,q1=1
    assert!(approx_eq(probs[3], 0.0)); // q0=1,q1=1
}

// ---------------------------------------------------------------------------
// Pauli-X gate
// ---------------------------------------------------------------------------

#[test]
fn test_x_gate_flips() {
    // X|0> = |1>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.0));
    assert!(approx_eq(probs[1], 1.0));
}

#[test]
fn test_double_x_returns() {
    // X*X|0> = |0>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0));
}

#[test]
fn test_x_on_second_qubit() {
    // |00> -> X(1) -> qubit 1 flipped
    // Little-endian: qubit 1 = bit 1, so result is index 2
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::X(1)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.0));
    assert!(approx_eq(probs[2], 1.0)); // bit 1 set = index 2
}

// ---------------------------------------------------------------------------
// Pauli-Y gate
// ---------------------------------------------------------------------------

#[test]
fn test_y_gate_on_zero() {
    // Y|0> = i|1>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::Y(0)).unwrap();
    let sv = state.state_vector();
    assert!(approx_eq(sv[0].norm_sq(), 0.0));
    assert!(approx_eq(sv[1].norm_sq(), 1.0));
    // Phase should be i: re=0, im=1
    assert!(approx_eq(sv[1].re, 0.0));
    assert!(approx_eq(sv[1].im, 1.0));
}

// ---------------------------------------------------------------------------
// Pauli-Z gate
// ---------------------------------------------------------------------------

#[test]
fn test_z_gate_on_zero() {
    // Z|0> = |0>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::Z(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0));
}

#[test]
fn test_z_gate_phase() {
    // Z|+> = |->
    // H|0> = |+> = (|0>+|1>)/sqrt(2)
    // Z|+> = (|0>-|1>)/sqrt(2)
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::Z(0)).unwrap();
    let sv = state.state_vector();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    assert!(approx_eq(sv[0].re, s));
    assert!(approx_eq(sv[0].im, 0.0));
    assert!(approx_eq(sv[1].re, -s));
    assert!(approx_eq(sv[1].im, 0.0));
}

#[test]
fn test_z_on_one() {
    // Z|1> = -|1>  (global phase, probabilities unchanged)
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap(); // |1>
    state.apply_gate(&Gate::Z(0)).unwrap(); // -|1>
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.0));
    assert!(approx_eq(probs[1], 1.0));
    let sv = state.state_vector();
    assert!(approx_eq(sv[1].re, -1.0));
}

// ---------------------------------------------------------------------------
// Bell state
// ---------------------------------------------------------------------------

#[test]
fn test_bell_state() {
    // H on qubit 0, CNOT(0,1) -> (|00> + |11>)/sqrt(2)
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5)); // |00>
    assert!(approx_eq(probs[1], 0.0)); // |01>
    assert!(approx_eq(probs[2], 0.0)); // |10>
    assert!(approx_eq(probs[3], 0.5)); // |11>
}

#[test]
fn test_bell_state_amplitudes() {
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    let sv = state.state_vector();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    assert!(approx_eq(sv[0].re, s));
    assert!(approx_eq(sv[0].im, 0.0));
    assert!(approx_eq(sv[3].re, s));
    assert!(approx_eq(sv[3].im, 0.0));
}

#[test]
fn test_bell_state_phi_minus() {
    // |Phi-> = (|00> - |11>)/sqrt(2)
    // Prepare: H(0), CNOT(0,1), Z(0)
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::Z(0)).unwrap();
    let sv = state.state_vector();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    assert!(approx_eq(sv[0].re, s));
    assert!(approx_eq(sv[3].re, -s));
}

#[test]
fn test_bell_state_psi_plus() {
    // |Psi+> = (|01> + |10>)/sqrt(2)
    // Prepare: X(0), H(0), CNOT(0,1)  or  H(0), CNOT(0,1), X(0)
    // Actually: X(1), H(0), CNOT(0,1) -> need to be careful
    // Simpler: H(0), CNOT(0,1), X(0) -> (X_0 (|00>+|11>))/sqrt(2) = (|10>+|01>)/sqrt(2)
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.0)); // |00>
    assert!(approx_eq(probs[1], 0.5)); // |01>
    assert!(approx_eq(probs[2], 0.5)); // |10>
    assert!(approx_eq(probs[3], 0.0)); // |11>
}

// ---------------------------------------------------------------------------
// GHZ state
// ---------------------------------------------------------------------------

#[test]
fn test_ghz_state() {
    // GHZ: H(0), CNOT(0,1), CNOT(1,2) -> (|000> + |111>)/sqrt(2)
    let mut state = QuantumState::new(3).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::CNOT(1, 2)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5)); // |000>
    assert!(approx_eq(probs[7], 0.5)); // |111>
    for i in 1..7 {
        assert!(
            approx_eq(probs[i], 0.0),
            "probs[{}] = {} should be 0",
            i,
            probs[i]
        );
    }
}

#[test]
fn test_ghz_4_qubits() {
    let mut state = QuantumState::new(4).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::CNOT(1, 2)).unwrap();
    state.apply_gate(&Gate::CNOT(2, 3)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5)); // |0000>
    assert!(approx_eq(probs[15], 0.5)); // |1111>
    for i in 1..15 {
        assert!(approx_eq(probs[i], 0.0));
    }
}

// ---------------------------------------------------------------------------
// SWAP gate
// ---------------------------------------------------------------------------

#[test]
fn test_swap_gate() {
    // X(1) puts qubit 1 in |1> -> index 2 (bit 1 set)
    // SWAP(0,1) exchanges qubit 0 and 1 -> qubit 0=1, qubit 1=0 -> index 1 (bit 0 set)
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::X(1)).unwrap();
    state.apply_gate(&Gate::SWAP(0, 1)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.0));
    assert!(approx_eq(probs[1], 1.0)); // qubit 0=1 -> index 1
    assert!(approx_eq(probs[2], 0.0));
    assert!(approx_eq(probs[3], 0.0));
}

#[test]
fn test_swap_both_same() {
    // SWAP|00> = |00>
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::SWAP(0, 1)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0));
}

#[test]
fn test_double_swap_identity() {
    // SWAP * SWAP = I -> back to original state
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::X(1)).unwrap(); // index 2 (qubit 1=1)
    state.apply_gate(&Gate::SWAP(0, 1)).unwrap(); // index 1
    state.apply_gate(&Gate::SWAP(0, 1)).unwrap(); // back to index 2
    let probs = state.probabilities();
    assert!(approx_eq(probs[2], 1.0)); // back to original
}

// ---------------------------------------------------------------------------
// Rotation gates
// ---------------------------------------------------------------------------

#[test]
fn test_rotation_identity() {
    // Rx(0)|0> = |0>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::Rx(0, 0.0)).unwrap();
    assert!(approx_eq(state.probabilities()[0], 1.0));
}

#[test]
fn test_rx_pi_is_x() {
    // Rx(pi)|0> = -i|1> (probability of |1> should be 1)
    let mut state = QuantumState::new(1).unwrap();
    state
        .apply_gate(&Gate::Rx(0, std::f64::consts::PI))
        .unwrap();
    assert!(approx_eq(state.probabilities()[0], 0.0));
    assert!(approx_eq(state.probabilities()[1], 1.0));
}

#[test]
fn test_ry_pi_flips() {
    // Ry(pi)|0> = |1>
    let mut state = QuantumState::new(1).unwrap();
    state
        .apply_gate(&Gate::Ry(0, std::f64::consts::PI))
        .unwrap();
    assert!(approx_eq(state.probabilities()[1], 1.0));
}

#[test]
fn test_rz_preserves_probability() {
    // Rz only changes phase, not measurement probabilities of |0>
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::Rz(0, 1.234)).unwrap();
    assert!(approx_eq(state.probabilities()[0], 1.0));
}

#[test]
fn test_rx_half_pi_creates_superposition() {
    // Rx(pi/2)|0> should give 50-50 superposition
    let mut state = QuantumState::new(1).unwrap();
    state
        .apply_gate(&Gate::Rx(0, std::f64::consts::FRAC_PI_2))
        .unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5));
    assert!(approx_eq(probs[1], 0.5));
}

#[test]
fn test_ry_half_pi_creates_superposition() {
    let mut state = QuantumState::new(1).unwrap();
    state
        .apply_gate(&Gate::Ry(0, std::f64::consts::FRAC_PI_2))
        .unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 0.5));
    assert!(approx_eq(probs[1], 0.5));
}

// ---------------------------------------------------------------------------
// CZ gate
// ---------------------------------------------------------------------------

#[test]
fn test_cz_on_11() {
    // CZ|11> = -|11>
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    state.apply_gate(&Gate::X(1)).unwrap(); // |11>
    state.apply_gate(&Gate::CZ(0, 1)).unwrap();
    let sv = state.state_vector();
    assert!(approx_eq(sv[3].re, -1.0)); // -|11>
                                        // Probability unchanged
    assert!(approx_eq(state.probabilities()[3], 1.0));
}

#[test]
fn test_cz_on_01() {
    // X(1) -> index 2 (q0=0,q1=1). CZ only phases |11>, so this is unchanged.
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::X(1)).unwrap();
    state.apply_gate(&Gate::CZ(0, 1)).unwrap();
    let sv = state.state_vector();
    assert!(approx_eq(sv[2].re, 1.0)); // index 2 unchanged
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

#[test]
fn test_measurement_collapses() {
    // Measure |+> state; after measurement, state should be collapsed
    let mut state = QuantumState::new_with_seed(1, 42).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let outcome = state.measure(0).unwrap();
    let probs = state.probabilities();
    if outcome.result {
        assert!(approx_eq(probs[1], 1.0));
    } else {
        assert!(approx_eq(probs[0], 1.0));
    }
}

#[test]
fn test_measurement_deterministic_zero() {
    // Measuring |0> always gives 0
    let mut state = QuantumState::new(1).unwrap();
    let outcome = state.measure(0).unwrap();
    assert!(!outcome.result, "|0> should always measure 0");
}

#[test]
fn test_measurement_deterministic_one() {
    // Measuring |1> always gives 1
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    let outcome = state.measure(0).unwrap();
    assert!(outcome.result, "|1> should always measure 1");
}

#[test]
fn test_measure_all() {
    let mut state = QuantumState::new_with_seed(2, 42).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    let outcomes = state.measure_all().unwrap();
    assert_eq!(outcomes.len(), 2);
    // Bell state: both qubits should have the same measurement result
    assert_eq!(outcomes[0].result, outcomes[1].result);
}

#[test]
fn test_measurement_statistics() {
    // Run many measurements on |+> to verify ~50/50 distribution
    let mut count_zero = 0;
    let mut count_one = 0;
    for seed in 0..200 {
        let mut state = QuantumState::new_with_seed(1, seed).unwrap();
        state.apply_gate(&Gate::H(0)).unwrap();
        let outcome = state.measure(0).unwrap();
        if outcome.result {
            count_one += 1;
        } else {
            count_zero += 1;
        }
    }
    // Expect roughly 50/50 with some tolerance
    let ratio = count_zero as f64 / 200.0;
    assert!(
        ratio > 0.3 && ratio < 0.7,
        "Expected ~50% zeros, got {:.1}%",
        ratio * 100.0
    );
}

#[test]
fn test_seeded_measurement_reproducibility() {
    // Same seed should give same measurement outcome
    let mut state1 = QuantumState::new_with_seed(1, 12345).unwrap();
    state1.apply_gate(&Gate::H(0)).unwrap();
    let outcome1 = state1.measure(0).unwrap();

    let mut state2 = QuantumState::new_with_seed(1, 12345).unwrap();
    state2.apply_gate(&Gate::H(0)).unwrap();
    let outcome2 = state2.measure(0).unwrap();

    assert_eq!(outcome1.result, outcome2.result);
}

// ---------------------------------------------------------------------------
// Probability of individual qubits
// ---------------------------------------------------------------------------

#[test]
fn test_probability_of_qubit_zero() {
    let state = QuantumState::new(1).unwrap();
    // P(qubit 0 = 1) = 0
    assert!(approx_eq(state.probability_of_qubit(0), 0.0));
}

#[test]
fn test_probability_of_qubit_superposition() {
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    // P(qubit 0 = 1) = 0.5
    assert!(approx_eq(state.probability_of_qubit(0), 0.5));
}

#[test]
fn test_probability_of_qubit_bell() {
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    // P(qubit 0 = 1) = 0.5, P(qubit 1 = 1) = 0.5
    assert!(approx_eq(state.probability_of_qubit(0), 0.5));
    assert!(approx_eq(state.probability_of_qubit(1), 0.5));
}

// ---------------------------------------------------------------------------
// Expectation values
// ---------------------------------------------------------------------------

#[test]
fn test_expectation_z_on_zero() {
    // <0|Z|0> = 1
    let state = QuantumState::new(1).unwrap();
    let z = PauliString {
        ops: vec![(0, PauliOp::Z)],
    };
    assert!(approx_eq(state.expectation_value(&z), 1.0));
}

#[test]
fn test_expectation_z_on_one() {
    // <1|Z|1> = -1
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    let z = PauliString {
        ops: vec![(0, PauliOp::Z)],
    };
    assert!(approx_eq(state.expectation_value(&z), -1.0));
}

#[test]
fn test_expectation_z_on_plus() {
    // <+|Z|+> = 0
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let z = PauliString {
        ops: vec![(0, PauliOp::Z)],
    };
    assert!(approx_eq(state.expectation_value(&z), 0.0));
}

#[test]
fn test_expectation_x_on_plus() {
    // <+|X|+> = 1
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let x = PauliString {
        ops: vec![(0, PauliOp::X)],
    };
    assert!(approx_eq(state.expectation_value(&x), 1.0));
}

#[test]
fn test_expectation_x_on_minus() {
    // <-|X|-> = -1
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap(); // |->
    let x = PauliString {
        ops: vec![(0, PauliOp::X)],
    };
    assert!(approx_eq(state.expectation_value(&x), -1.0));
}

#[test]
fn test_expectation_zz_bell() {
    // Bell state (|00>+|11>)/sqrt(2): <ZZ> = 1 (both qubits always same)
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    let zz = PauliString {
        ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
    };
    assert!(approx_eq(state.expectation_value(&zz), 1.0));
}

#[test]
fn test_expectation_xx_bell() {
    // Bell state (|00>+|11>)/sqrt(2): <XX> = 1
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    let xx = PauliString {
        ops: vec![(0, PauliOp::X), (1, PauliOp::X)],
    };
    assert!(approx_eq(state.expectation_value(&xx), 1.0));
}

#[test]
fn test_expectation_yy_bell() {
    // Bell state (|00>+|11>)/sqrt(2): <YY> = -1
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    let yy = PauliString {
        ops: vec![(0, PauliOp::Y), (1, PauliOp::Y)],
    };
    assert!(approx_eq(state.expectation_value(&yy), -1.0));
}

#[test]
fn test_expectation_identity() {
    // <psi|I|psi> = 1 for any normalized state
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::Rx(1, 1.5)).unwrap();
    let identity = PauliString { ops: vec![] };
    assert!(approx_eq(state.expectation_value(&identity), 1.0));
}

// ---------------------------------------------------------------------------
// Hamiltonian expectation values
// ---------------------------------------------------------------------------

#[test]
fn test_expectation_hamiltonian_simple() {
    // H = Z0, <0|Z0|0> = 1
    let state = QuantumState::new(1).unwrap();
    let h = Hamiltonian {
        terms: vec![(
            1.0,
            PauliString {
                ops: vec![(0, PauliOp::Z)],
            },
        )],
        num_qubits: 1,
    };
    assert!(approx_eq(state.expectation_hamiltonian(&h), 1.0));
}

#[test]
fn test_expectation_hamiltonian_two_terms() {
    // H = 0.5*Z0 + 0.5*Z1, state = |00>
    // <00|H|00> = 0.5*1 + 0.5*1 = 1.0
    let state = QuantumState::new(2).unwrap();
    let h = Hamiltonian {
        terms: vec![
            (
                0.5,
                PauliString {
                    ops: vec![(0, PauliOp::Z)],
                },
            ),
            (
                0.5,
                PauliString {
                    ops: vec![(1, PauliOp::Z)],
                },
            ),
        ],
        num_qubits: 2,
    };
    assert!(approx_eq(state.expectation_hamiltonian(&h), 1.0));
}

#[test]
fn test_expectation_hamiltonian_after_flip() {
    // H = Z0 + Z1, state = |10> (X on qubit 0)
    // <10|Z0|10> = -1, <10|Z1|10> = 1 => total = 0
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    let h = Hamiltonian {
        terms: vec![
            (
                1.0,
                PauliString {
                    ops: vec![(0, PauliOp::Z)],
                },
            ),
            (
                1.0,
                PauliString {
                    ops: vec![(1, PauliOp::Z)],
                },
            ),
        ],
        num_qubits: 2,
    };
    assert!(approx_eq(state.expectation_hamiltonian(&h), 0.0));
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

#[test]
fn test_normalization_preserved_after_gates() {
    let mut state = QuantumState::new(3).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::Rx(2, 1.23)).unwrap();
    state.apply_gate(&Gate::Rz(0, 0.456)).unwrap();
    state.apply_gate(&Gate::Ry(1, 2.1)).unwrap();
    state.apply_gate(&Gate::CZ(0, 2)).unwrap();
    let total_prob: f64 = state.probabilities().iter().sum();
    assert!(approx_eq(total_prob, 1.0));
}

#[test]
fn test_normalization_many_gates() {
    let mut state = QuantumState::new(4).unwrap();
    for i in 0..4 {
        state.apply_gate(&Gate::H(i)).unwrap();
    }
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::CNOT(2, 3)).unwrap();
    state.apply_gate(&Gate::SWAP(1, 2)).unwrap();
    state.apply_gate(&Gate::Rx(0, 0.7)).unwrap();
    state.apply_gate(&Gate::Ry(3, 1.2)).unwrap();
    let total_prob: f64 = state.probabilities().iter().sum();
    assert!(approx_eq(total_prob, 1.0));
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn test_reset_qubit() {
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap(); // |1>
    state.reset_qubit(0).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0)); // back to |0>
}

#[test]
fn test_reset_qubit_from_superposition() {
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap(); // |+>
    state.reset_qubit(0).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0)); // back to |0>
}

#[test]
fn test_reset_one_qubit_of_entangled() {
    // Bell state, reset qubit 0
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.reset_qubit(0).unwrap();
    // After reset, qubit 0 should be |0>, but qubit 1 may still be mixed
    let p0 = state.probability_of_qubit(0);
    assert!(approx_eq(p0, 0.0), "After reset, qubit 0 should be in |0>");
}

// ---------------------------------------------------------------------------
// Fidelity
// ---------------------------------------------------------------------------

#[test]
fn test_fidelity_same_state() {
    let state = QuantumState::new(2).unwrap();
    assert!(approx_eq(state.fidelity(&state), 1.0));
}

#[test]
fn test_fidelity_orthogonal() {
    let state0 = QuantumState::new(1).unwrap(); // |0>
    let mut state1 = QuantumState::new(1).unwrap();
    state1.apply_gate(&Gate::X(0)).unwrap(); // |1>
    assert!(approx_eq(state0.fidelity(&state1), 0.0));
}

#[test]
fn test_fidelity_partial_overlap() {
    let state0 = QuantumState::new(1).unwrap(); // |0>
    let mut state_plus = QuantumState::new(1).unwrap();
    state_plus.apply_gate(&Gate::H(0)).unwrap(); // |+>
                                                 // |<0|+>|^2 = (1/sqrt(2))^2 = 0.5
    assert!(approx_eq(state0.fidelity(&state_plus), 0.5));
}

#[test]
fn test_fidelity_symmetric() {
    let mut state_a = QuantumState::new(2).unwrap();
    state_a.apply_gate(&Gate::H(0)).unwrap();
    let mut state_b = QuantumState::new(2).unwrap();
    state_b.apply_gate(&Gate::H(1)).unwrap();
    assert!(approx_eq(
        state_a.fidelity(&state_b),
        state_b.fidelity(&state_a)
    ));
}

// ---------------------------------------------------------------------------
// Memory estimation
// ---------------------------------------------------------------------------

#[test]
fn test_memory_estimate_1_qubit() {
    // 2^1 = 2 complex numbers * 16 bytes = 32
    assert_eq!(QuantumState::estimate_memory(1), 32);
}

#[test]
fn test_memory_estimate_10_qubits() {
    // 2^10 = 1024 complex numbers * 16 bytes = 16384
    assert_eq!(QuantumState::estimate_memory(10), 16384);
}

#[test]
fn test_memory_estimate_20_qubits() {
    // 2^20 = 1048576 complex numbers * 16 bytes = 16777216 (~16MB)
    assert_eq!(QuantumState::estimate_memory(20), 16_777_216);
}

// ---------------------------------------------------------------------------
// Qubit limit / error handling
// ---------------------------------------------------------------------------

#[test]
fn test_qubit_limit_too_many() {
    // Should fail for too many qubits (MAX_QUBITS = 32)
    assert!(QuantumState::new(35).is_err());
}

#[test]
fn test_zero_qubits() {
    // Zero qubits should likely fail
    assert!(QuantumState::new(0).is_err());
}

#[test]
fn test_single_qubit_valid() {
    assert!(QuantumState::new(1).is_ok());
}

// ---------------------------------------------------------------------------
// S and T gates on state
// ---------------------------------------------------------------------------

#[test]
fn test_s_gate_on_plus() {
    // S|+> = (|0> + i|1>)/sqrt(2)
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::S(0)).unwrap();
    let sv = state.state_vector();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    assert!(approx_eq(sv[0].re, s));
    assert!(approx_eq(sv[0].im, 0.0));
    assert!(approx_eq(sv[1].re, 0.0));
    assert!(approx_eq(sv[1].im, s));
}

#[test]
fn test_t_gate_phase() {
    // T|+> = (|0> + e^{i*pi/4}|1>)/sqrt(2)
    let mut state = QuantumState::new(1).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::T(0)).unwrap();
    let sv = state.state_vector();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let phase = std::f64::consts::FRAC_PI_4;
    assert!(approx_eq(sv[0].re, s));
    assert!(approx_eq(sv[1].re, s * phase.cos()));
    assert!(approx_eq(sv[1].im, s * phase.sin()));
}

// ---------------------------------------------------------------------------
// Rzz gate
// ---------------------------------------------------------------------------

#[test]
fn test_rzz_zero_is_identity() {
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::H(1)).unwrap();
    let probs_before = state.probabilities().clone();
    state.apply_gate(&Gate::Rzz(0, 1, 0.0)).unwrap();
    let probs_after = state.probabilities();
    for i in 0..4 {
        assert!(approx_eq(probs_before[i], probs_after[i]));
    }
}

// ---------------------------------------------------------------------------
// Quantum teleportation protocol
// ---------------------------------------------------------------------------

#[test]
fn test_teleportation_protocol() {
    // Teleport qubit 0 state via Bell pair on qubits 1,2
    // Prepare arbitrary state on qubit 0: Ry(1.23)|0>
    let mut state = QuantumState::new_with_seed(3, 99).unwrap();
    state.apply_gate(&Gate::Ry(0, 1.23)).unwrap();

    // Record the target amplitudes
    let target_prob_1 = {
        let mut target = QuantumState::new(1).unwrap();
        target.apply_gate(&Gate::Ry(0, 1.23)).unwrap();
        target.probabilities()[1]
    };

    // Create Bell pair on qubits 1, 2
    state.apply_gate(&Gate::H(1)).unwrap();
    state.apply_gate(&Gate::CNOT(1, 2)).unwrap();

    // Bell measurement on qubits 0, 1
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();

    let m0 = state.measure(0).unwrap();
    let m1 = state.measure(1).unwrap();

    // Apply corrections to qubit 2
    if m1.result {
        state.apply_gate(&Gate::X(2)).unwrap();
    }
    if m0.result {
        state.apply_gate(&Gate::Z(2)).unwrap();
    }

    // After teleportation, qubit 2 should have the original state
    let p2 = state.probability_of_qubit(2);
    assert!(
        (p2 - target_prob_1).abs() < 0.01,
        "Teleportation failed: qubit 2 prob = {}, expected {}",
        p2,
        target_prob_1
    );
}

// ---------------------------------------------------------------------------
// Superdense coding
// ---------------------------------------------------------------------------

#[test]
fn test_superdense_coding_00() {
    // Encode classical bits 00: apply I to qubit 0 of Bell pair
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    // Encoding 00: no operation
    // Decode: CNOT, H, measure
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[0], 1.0)); // |00>
}

#[test]
fn test_superdense_coding_01() {
    // Encode classical bits 01: apply X to qubit 0
    // In little-endian bit ordering, decoded result lands at index 2
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap(); // encode 01
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[2], 1.0)); // q0=0,q1=1 = index 2
}

#[test]
fn test_superdense_coding_10() {
    // Encode classical bits 10: apply Z to qubit 0
    // In little-endian bit ordering, decoded result lands at index 1
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::Z(0)).unwrap(); // encode 10
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[1], 1.0)); // q0=1,q1=0 = index 1
}

#[test]
fn test_superdense_coding_11() {
    // Encode classical bits 11: apply ZX (= iY) to qubit 0
    let mut state = QuantumState::new(2).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::X(0)).unwrap();
    state.apply_gate(&Gate::Z(0)).unwrap(); // encode 11
    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
    state.apply_gate(&Gate::H(0)).unwrap();
    let probs = state.probabilities();
    assert!(approx_eq(probs[3], 1.0)); // |11>
}
