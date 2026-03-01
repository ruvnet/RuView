use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ruqu_core::prelude::*;

fn bench_single_qubit_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_qubit_gates");

    for num_qubits in [4, 8, 12, 16, 20] {
        group.bench_with_input(
            BenchmarkId::new("hadamard", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut state = QuantumState::new(n).unwrap();
                    state.apply_gate(&Gate::H(0)).unwrap();
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rx_rotation", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut state = QuantumState::new(n).unwrap();
                    state.apply_gate(&Gate::Rx(0, 1.234)).unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_two_qubit_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("two_qubit_gates");

    for num_qubits in [4, 8, 12, 16, 20] {
        group.bench_with_input(
            BenchmarkId::new("cnot", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut state = QuantumState::new(n).unwrap();
                    state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("rzz", num_qubits), &num_qubits, |b, &n| {
            b.iter(|| {
                let mut state = QuantumState::new(n).unwrap();
                state.apply_gate(&Gate::Rzz(0, 1, 0.5)).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_bell_state_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("bell_state");

    for num_qubits in [2, 4, 8, 12, 16] {
        group.bench_with_input(
            BenchmarkId::new("create", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut circuit = QuantumCircuit::new(n);
                    circuit.h(0).cnot(0, 1);
                    Simulator::run(&circuit).unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_grover_circuit(c: &mut Criterion) {
    let mut group = c.benchmark_group("grover_circuit");

    for num_qubits in [4, 6, 8, 10] {
        group.bench_with_input(
            BenchmarkId::new("full_algorithm", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut state = QuantumState::new_with_seed(n, 42).unwrap();
                    // Apply Hadamard to all qubits
                    for q in 0..n {
                        state.apply_gate(&Gate::H(q)).unwrap();
                    }
                    let target = 0usize;
                    let iterations =
                        (std::f64::consts::FRAC_PI_4 * ((1u64 << n) as f64).sqrt()) as u32;
                    for _ in 0..iterations {
                        // Oracle (simplified)
                        state.apply_gate(&Gate::Z(0)).unwrap();
                        // Diffuser
                        for q in 0..n {
                            state.apply_gate(&Gate::H(q)).unwrap();
                        }
                        for q in 0..n {
                            state.apply_gate(&Gate::X(q)).unwrap();
                        }
                        state.apply_gate(&Gate::Z(n - 1)).unwrap();
                        for q in 0..n {
                            state.apply_gate(&Gate::X(q)).unwrap();
                        }
                        for q in 0..n {
                            state.apply_gate(&Gate::H(q)).unwrap();
                        }
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_qaoa_layer(c: &mut Criterion) {
    let mut group = c.benchmark_group("qaoa_layer");

    for num_qubits in [4, 8, 12, 16] {
        group.bench_with_input(
            BenchmarkId::new("one_layer", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut state = QuantumState::new(n).unwrap();
                    for q in 0..n {
                        state.apply_gate(&Gate::H(q)).unwrap();
                    }
                    // Phase separation: linear chain
                    for q in 0..n.saturating_sub(1) {
                        state.apply_gate(&Gate::Rzz(q, q + 1, 0.5)).unwrap();
                    }
                    // Mixing
                    for q in 0..n {
                        state.apply_gate(&Gate::Rx(q, 0.3)).unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_expectation_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("expectation_value");

    for num_qubits in [4, 8, 12, 16] {
        group.bench_with_input(
            BenchmarkId::new("single_z", num_qubits),
            &num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                state.apply_gate(&Gate::H(0)).unwrap();
                let z = PauliString {
                    ops: vec![(0, PauliOp::Z)],
                };
                b.iter(|| {
                    state.expectation_value(&z);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("zz_pair", num_qubits),
            &num_qubits,
            |b, &n| {
                let mut state = QuantumState::new(n).unwrap();
                state.apply_gate(&Gate::H(0)).unwrap();
                state.apply_gate(&Gate::CNOT(0, 1)).unwrap();
                let zz = PauliString {
                    ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
                };
                b.iter(|| {
                    state.expectation_value(&zz);
                });
            },
        );
    }
    group.finish();
}

fn bench_measurement(c: &mut Criterion) {
    let mut group = c.benchmark_group("measurement");

    for num_qubits in [4, 8, 12, 16] {
        group.bench_with_input(
            BenchmarkId::new("single_qubit_measure", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    let mut state = QuantumState::new_with_seed(n, 42).unwrap();
                    state.apply_gate(&Gate::H(0)).unwrap();
                    state.measure(0).unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_state_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_allocation");

    for num_qubits in [8, 12, 16, 20] {
        group.bench_with_input(
            BenchmarkId::new("allocate_and_init", num_qubits),
            &num_qubits,
            |b, &n| {
                b.iter(|| {
                    QuantumState::new(n).unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_single_qubit_gates,
    bench_two_qubit_gates,
    bench_bell_state_creation,
    bench_grover_circuit,
    bench_qaoa_layer,
    bench_expectation_value,
    bench_measurement,
    bench_state_allocation,
);
criterion_main!(benches);
