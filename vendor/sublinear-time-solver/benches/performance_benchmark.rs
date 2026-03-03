//! Performance benchmarks to validate optimizations against Python baselines

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sublinear_time_solver::fast_solver::{FastCSRMatrix, FastConjugateGradient};
use sublinear_time_solver::types::Precision;

fn generate_sparse_matrix(size: usize, sparsity: f64) -> (FastCSRMatrix, Vec<Precision>) {
    let mut triplets = Vec::new();
    let mut rng_state = 1u64;

    // Generate diagonally dominant sparse matrix
    for i in 0..size {
        // Diagonal element (make it dominant)
        let diag_val = 5.0 + (i as f64) * 0.01;
        triplets.push((i, i, diag_val));

        // Off-diagonal elements
        let nnz_per_row = ((size as f64) * sparsity).max(1.0) as usize;
        for _ in 0..nnz_per_row.min(5) {
            // Simple LCG for reproducible random numbers
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng_state as usize) % size;

            if i != j {
                let val = (rng_state as f64 / u64::MAX as f64) * 0.5; // Keep small for diagonal dominance
                triplets.push((i, j, val));
            }
        }
    }

    let matrix = FastCSRMatrix::from_triplets(triplets, size, size);
    let b = vec![1.0; size]; // Simple right-hand side

    (matrix, b)
}

fn bench_matrix_vector_multiply(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_vector_multiply");

    for size in [100, 1000, 5000].iter() {
        let (matrix, _) = generate_sparse_matrix(*size, 0.01);
        let x = vec![1.0; *size];

        group.bench_with_input(
            BenchmarkId::new("fast_csr", size),
            size,
            |b, _| {
                let mut y = vec![0.0; *size];
                b.iter(|| {
                    matrix.multiply_vector_fast(black_box(&x), black_box(&mut y));
                });
            },
        );
    }

    group.finish();
}

fn bench_conjugate_gradient_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("conjugate_gradient_solve");

    for size in [100, 1000].iter() {
        let (matrix, b) = generate_sparse_matrix(*size, 0.01);
        let solver = FastConjugateGradient::new(1000, 1e-8);

        group.bench_with_input(
            BenchmarkId::new("fast_cg", size),
            size,
            |bench, _| {
                bench.iter(|| {
                    let _solution = solver.solve(black_box(&matrix), black_box(&b));
                });
            },
        );
    }

    group.finish();
}

fn bench_sparse_dense_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_vs_dense");

    // Test the critical 1000x1000 case that shows MCP Dense is 190x slower
    let size = 1000;
    let (sparse_matrix, b) = generate_sparse_matrix(size, 0.001); // Very sparse

    let solver = FastConjugateGradient::new(100, 1e-6);

    group.bench_function("optimized_sparse_1000x1000", |bench| {
        bench.iter(|| {
            let _solution = solver.solve(black_box(&sparse_matrix), black_box(&b));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_matrix_vector_multiply,
    bench_conjugate_gradient_solve,
    bench_sparse_dense_comparison
);
criterion_main!(benches);