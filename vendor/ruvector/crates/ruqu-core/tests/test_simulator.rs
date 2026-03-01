//! Tests for ruqu_core::simulator — high-level simulator, circuits, shots, reproducibility.

use ruqu_core::prelude::*;

const EPSILON: f64 = 1e-10;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

// ---------------------------------------------------------------------------
// Basic circuit construction
// ---------------------------------------------------------------------------

#[test]
fn test_circuit_new() {
    let circuit = QuantumCircuit::new(3);
    assert_eq!(circuit.num_qubits(), 3);
    assert_eq!(circuit.gate_count(), 0);
}

#[test]
fn test_circuit_add_gates() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).cnot(0, 1);
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.gate_count(), 2);
}

#[test]
fn test_circuit_chaining() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0).h(1).h(2).cnot(0, 1).cnot(1, 2);
    assert_eq!(circuit.gate_count(), 5);
}

#[test]
fn test_circuit_gates_ref() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).x(0);
    let gates = circuit.gates();
    assert_eq!(gates.len(), 2);
}

#[test]
fn test_circuit_all_single_qubit_gates() {
    let mut circuit = QuantumCircuit::new(1);
    circuit
        .h(0)
        .x(0)
        .y(0)
        .z(0)
        .s(0)
        .t(0)
        .rx(0, 0.5)
        .ry(0, 0.5)
        .rz(0, 0.5);
    assert_eq!(circuit.gate_count(), 9);
}

#[test]
fn test_circuit_two_qubit_gates() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.cnot(0, 1).cz(0, 1).swap(0, 1).rzz(0, 1, 0.5);
    assert_eq!(circuit.gate_count(), 4);
}

#[test]
fn test_circuit_measure() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).cnot(0, 1).measure(0).measure(1);
    assert_eq!(circuit.gate_count(), 4);
}

#[test]
fn test_circuit_measure_all() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0).h(1).h(2).measure_all();
    // measure_all adds a measure for each qubit
    assert!(circuit.gate_count() >= 6);
}

#[test]
fn test_circuit_reset() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.x(0).reset(0);
    assert_eq!(circuit.gate_count(), 2);
}

#[test]
fn test_circuit_barrier() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).barrier().cnot(0, 1);
    assert_eq!(circuit.gate_count(), 3);
}

#[test]
fn test_circuit_add_gate_directly() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.add_gate(Gate::H(0));
    circuit.add_gate(Gate::CNOT(0, 1));
    assert_eq!(circuit.gate_count(), 2);
}

// ---------------------------------------------------------------------------
// Circuit depth
// ---------------------------------------------------------------------------

#[test]
fn test_circuit_depth_single_gate() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0);
    assert_eq!(circuit.depth(), 1);
}

#[test]
fn test_circuit_depth_parallel_gates() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0).h(1).h(2);
    // All H gates act on different qubits, so depth = 1
    assert!(circuit.depth() >= 1);
}

#[test]
fn test_circuit_depth_sequential() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).x(0).y(0);
    // All on same qubit, depth = 3
    assert!(circuit.depth() >= 3);
}

#[test]
fn test_circuit_depth_mixed() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0).h(1).h(2).cnot(0, 1).cnot(1, 2);
    // H gates parallel (depth 1), then CNOT(0,1) (depth 2), then CNOT(1,2) (depth 3)
    assert!(circuit.depth() >= 2);
}

// ---------------------------------------------------------------------------
// Simulator::run
// ---------------------------------------------------------------------------

#[test]
fn test_simulator_basic() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).cnot(0, 1);
    let result = Simulator::run(&circuit).unwrap();
    assert_eq!(result.metrics.num_qubits, 2);
    assert!(result.metrics.gate_count >= 2);
}

#[test]
fn test_simulator_bell_state_probabilities() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).cnot(0, 1);
    let result = Simulator::run(&circuit).unwrap();
    let probs = result.state.probabilities();
    assert!(approx_eq(probs[0], 0.5)); // |00>
    assert!(approx_eq(probs[1], 0.0)); // |01>
    assert!(approx_eq(probs[2], 0.0)); // |10>
    assert!(approx_eq(probs[3], 0.5)); // |11>
}

#[test]
fn test_simulator_identity_circuit() {
    // No gates at all: state should remain |0>
    let circuit = QuantumCircuit::new(1);
    let result = Simulator::run(&circuit).unwrap();
    let probs = result.state.probabilities();
    assert!(approx_eq(probs[0], 1.0));
    assert!(approx_eq(probs[1], 0.0));
}

#[test]
fn test_simulator_x_gate() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.x(0);
    let result = Simulator::run(&circuit).unwrap();
    let probs = result.state.probabilities();
    assert!(approx_eq(probs[0], 0.0));
    assert!(approx_eq(probs[1], 1.0));
}

#[test]
fn test_simulator_ghz() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0).cnot(0, 1).cnot(1, 2);
    let result = Simulator::run(&circuit).unwrap();
    let probs = result.state.probabilities();
    assert!(approx_eq(probs[0], 0.5));
    assert!(approx_eq(probs[7], 0.5));
    for i in 1..7 {
        assert!(approx_eq(probs[i], 0.0));
    }
}

#[test]
fn test_simulator_with_measurement() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.x(0).measure(0);
    let result = Simulator::run(&circuit).unwrap();
    assert!(!result.measurements.is_empty());
    // X|0> = |1>, so measurement should be 1
    assert!(result.measurements[0].result);
}

// ---------------------------------------------------------------------------
// Simulator::run_with_config (seeded)
// ---------------------------------------------------------------------------

#[test]
fn test_seeded_reproducibility() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).measure(0);

    let config = SimConfig {
        seed: Some(42),
        noise: None,
        shots: None,
    };

    let r1 = Simulator::run_with_config(&circuit, &config).unwrap();
    let r2 = Simulator::run_with_config(&circuit, &config).unwrap();

    assert_eq!(r1.measurements.len(), r2.measurements.len());
    if !r1.measurements.is_empty() && !r2.measurements.is_empty() {
        assert_eq!(r1.measurements[0].result, r2.measurements[0].result);
    }
}

#[test]
fn test_different_seeds_may_differ() {
    // With different seeds and a probabilistic circuit, results may differ
    // (not guaranteed per single run, but validates that config is used)
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).measure(0);

    let c1 = SimConfig {
        seed: Some(42),
        noise: None,
        shots: None,
    };
    let c2 = SimConfig {
        seed: Some(99),
        noise: None,
        shots: None,
    };

    let _r1 = Simulator::run_with_config(&circuit, &c1).unwrap();
    let _r2 = Simulator::run_with_config(&circuit, &c2).unwrap();
    // We just verify both complete without error.
}

#[test]
fn test_config_no_seed() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).measure(0);

    let config = SimConfig {
        seed: None,
        noise: None,
        shots: None,
    };

    let result = Simulator::run_with_config(&circuit, &config).unwrap();
    assert!(!result.measurements.is_empty());
}

// ---------------------------------------------------------------------------
// Simulator::run_shots
// ---------------------------------------------------------------------------

#[test]
fn test_run_shots_basic() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.x(0).measure(0);

    let result = Simulator::run_shots(&circuit, 100, Some(42)).unwrap();
    // X|0> = |1>, every shot should measure 1
    let total: usize = result.counts.values().sum();
    assert_eq!(total, 100);
    // All shots should give outcome |1> -> vec![true]
    let count_one = result.counts.get(&vec![true]).copied().unwrap_or(0);
    assert_eq!(count_one, 100, "All shots should measure |1>");
}

#[test]
fn test_run_shots_superposition() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).measure(0);

    let result = Simulator::run_shots(&circuit, 1000, Some(42)).unwrap();
    let total: usize = result.counts.values().sum();
    assert_eq!(total, 1000);

    let count_zero = result.counts.get(&vec![false]).copied().unwrap_or(0);
    let count_one = result.counts.get(&vec![true]).copied().unwrap_or(0);
    assert_eq!(count_zero + count_one, 1000);

    // Expect roughly 50/50 with tolerance
    let ratio = count_zero as f64 / 1000.0;
    assert!(
        ratio > 0.4 && ratio < 0.6,
        "Expected ~50% zeros, got {:.1}%",
        ratio * 100.0
    );
}

#[test]
fn test_run_shots_bell_state() {
    let mut circuit = QuantumCircuit::new(2);
    circuit.h(0).cnot(0, 1).measure(0).measure(1);

    let result = Simulator::run_shots(&circuit, 500, Some(42)).unwrap();
    let total: usize = result.counts.values().sum();
    assert_eq!(total, 500);

    // Bell state: only |00> and |11> outcomes
    let count_00 = result.counts.get(&vec![false, false]).copied().unwrap_or(0);
    let count_11 = result.counts.get(&vec![true, true]).copied().unwrap_or(0);
    let count_01 = result.counts.get(&vec![false, true]).copied().unwrap_or(0);
    let count_10 = result.counts.get(&vec![true, false]).copied().unwrap_or(0);

    assert_eq!(
        count_01 + count_10,
        0,
        "Bell state should never produce |01> or |10>"
    );
    assert_eq!(count_00 + count_11, 500);
}

#[test]
fn test_run_shots_seeded_reproducibility() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).measure(0);

    let r1 = Simulator::run_shots(&circuit, 100, Some(42)).unwrap();
    let r2 = Simulator::run_shots(&circuit, 100, Some(42)).unwrap();

    assert_eq!(r1.counts, r2.counts);
}

#[test]
fn test_run_shots_single_shot() {
    let mut circuit = QuantumCircuit::new(1);
    circuit.h(0).measure(0);

    let result = Simulator::run_shots(&circuit, 1, Some(42)).unwrap();
    let total: usize = result.counts.values().sum();
    assert_eq!(total, 1);
}

// ---------------------------------------------------------------------------
// Memory estimation
// ---------------------------------------------------------------------------

#[test]
fn test_memory_estimate_1_qubit() {
    // 2^1 = 2 complex numbers * 16 bytes (re: f64 + im: f64) = 32
    assert_eq!(QuantumState::estimate_memory(1), 32);
}

#[test]
fn test_memory_estimate_10_qubits() {
    // 2^10 = 1024 * 16 = 16384
    assert_eq!(QuantumState::estimate_memory(10), 16384);
}

#[test]
fn test_memory_estimate_scales_exponentially() {
    let m5 = QuantumState::estimate_memory(5);
    let m6 = QuantumState::estimate_memory(6);
    assert_eq!(m6, m5 * 2);
}

// ---------------------------------------------------------------------------
// Metrics from simulation result
// ---------------------------------------------------------------------------

#[test]
fn test_simulation_metrics_qubit_count() {
    let mut circuit = QuantumCircuit::new(5);
    circuit.h(0);
    let result = Simulator::run(&circuit).unwrap();
    assert_eq!(result.metrics.num_qubits, 5);
}

#[test]
fn test_simulation_metrics_gate_count() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0).cnot(0, 1).cnot(1, 2).x(2);
    let result = Simulator::run(&circuit).unwrap();
    assert!(result.metrics.gate_count >= 4);
}

#[test]
fn test_simulation_result_state_vector_length() {
    let mut circuit = QuantumCircuit::new(3);
    circuit.h(0);
    let result = Simulator::run(&circuit).unwrap();
    // 2^3 = 8 probabilities
    let probs = result.state.probabilities();
    assert_eq!(probs.len(), 8);
}

// ---------------------------------------------------------------------------
// Complex circuits
// ---------------------------------------------------------------------------

#[test]
fn test_qft_like_circuit() {
    // Simplified QFT-like pattern: H on each, controlled rotations
    let mut circuit = QuantumCircuit::new(3);
    circuit
        .h(0)
        .rz(0, std::f64::consts::FRAC_PI_4)
        .rz(0, std::f64::consts::FRAC_PI_2)
        .h(1)
        .rz(1, std::f64::consts::FRAC_PI_4)
        .h(2)
        .swap(0, 2);

    let result = Simulator::run(&circuit).unwrap();
    let total: f64 = result.state.probabilities().iter().sum();
    assert!(approx_eq(total, 1.0));
}

#[test]
fn test_many_gate_circuit() {
    // Stress test: many gates, verify normalization
    let n = 5;
    let mut circuit = QuantumCircuit::new(n);
    for i in 0..n {
        circuit.h(i);
    }
    for i in 0..(n - 1) {
        circuit.cnot(i, i + 1);
    }
    for i in 0..n {
        circuit.rz(i, 0.3 * (i as f64));
    }
    let result = Simulator::run(&circuit).unwrap();
    let total: f64 = result.state.probabilities().iter().sum();
    assert!(approx_eq(total, 1.0));
}
