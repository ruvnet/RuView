//! Quantum Solver Benchmarks — effective qubit scaling with sparse operators
//!
//! Benchmarks the SolverBackedOperator for 10 to 20+ effective qubits,
//! measuring SpMV performance, eigenvalue convergence, and memory scaling.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use prime_radiant_category::quantum::topological_code::SolverBackedOperator;

fn spmv_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_spmv");
    group.sample_size(20);

    for qubits in [10, 12, 14, 16, 18, 20] {
        let bandwidth = 4.min(1 << qubits);
        let op = SolverBackedOperator::banded(qubits, bandwidth, 42);
        let state = vec![1.0 / ((1u64 << qubits) as f64).sqrt(); 1 << qubits];

        group.bench_with_input(
            BenchmarkId::new("apply", qubits),
            &qubits,
            |b, _| {
                b.iter(|| black_box(op.apply(&state)));
            },
        );
    }

    group.finish();
}

fn eigenvalue_convergence(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_eigenvalue");
    group.sample_size(10);

    for qubits in [10, 12, 14, 16] {
        let op = SolverBackedOperator::banded(qubits, 4, 42);

        group.bench_with_input(
            BenchmarkId::new("power_iteration", qubits),
            &qubits,
            |b, _| {
                b.iter(|| black_box(op.dominant_eigenvalue(50, 1e-8)));
            },
        );
    }

    group.finish();
}

fn memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantum_memory");

    for qubits in [10, 15, 20, 25, 30] {
        // Just measure construction time (memory is logged)
        group.bench_with_input(
            BenchmarkId::new("construct_banded", qubits),
            &qubits,
            |b, &q| {
                b.iter(|| {
                    let op = SolverBackedOperator::banded(q, 4, 42);
                    black_box((op.nnz(), op.memory_bytes(), op.dense_memory_bytes()))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    spmv_scaling,
    eigenvalue_convergence,
    memory_scaling,
);

criterion_main!(benches);
