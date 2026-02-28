//! Baseline benchmarks for dense and sparse matrix-vector operations.
//!
//! These benchmarks establish performance baselines for the core linear algebra
//! primitives used throughout the solver crate: naive dense matrix-vector
//! multiply and CSR sparse matrix-vector multiply (SpMV).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Helpers: deterministic random data generation
// ---------------------------------------------------------------------------

/// Generate a dense matrix stored as a flat row-major `Vec<f32>`.
///
/// Uses a deterministic seed so benchmark results are reproducible across runs.
fn random_dense_matrix(rows: usize, cols: usize, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..rows * cols).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Generate a random CSR matrix with approximately `density` fraction of
/// non-zero entries.
///
/// The matrix is square (`n x n`). Each entry in the upper triangle is
/// included independently with probability `density`, then mirrored to the
/// lower triangle for symmetry. Diagonal entries are always present and set
/// to a value ensuring strict diagonal dominance.
fn random_csr_matrix(n: usize, density: f64, seed: u64) -> CsrMatrix<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut entries: Vec<(usize, usize, f32)> = Vec::new();

    // Off-diagonal entries (symmetric).
    for i in 0..n {
        for j in (i + 1)..n {
            if rng.gen::<f64>() < density {
                let val: f32 = rng.gen_range(-0.5..0.5);
                entries.push((i, j, val));
                entries.push((j, i, val));
            }
        }
    }

    // Build row-wise absolute sums for diagonal dominance.
    let mut row_abs_sums = vec![0.0f32; n];
    for &(r, _c, v) in &entries {
        row_abs_sums[r] += v.abs();
    }

    // Diagonal entries: ensure diagonal dominance for solver stability.
    for i in 0..n {
        entries.push((i, i, row_abs_sums[i] + 1.0));
    }

    CsrMatrix::<f32>::from_coo(n, n, entries)
}

/// Generate a random vector of length `n` with values in [-1, 1].
fn random_vector(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// ---------------------------------------------------------------------------
// Dense matrix-vector multiply (naive baseline)
// ---------------------------------------------------------------------------

/// Naive dense matrix-vector multiply: `y = A * x`.
///
/// `a` is stored in row-major order with dimensions `rows x cols`.
#[inline(never)]
fn dense_matvec(a: &[f32], x: &[f32], y: &mut [f32], rows: usize, cols: usize) {
    for i in 0..rows {
        let mut sum = 0.0f32;
        let row_start = i * cols;
        for j in 0..cols {
            sum += a[row_start + j] * x[j];
        }
        y[i] = sum;
    }
}

fn dense_matvec_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_matvec");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    for size in [64, 256, 1024, 4096] {
        let a = random_dense_matrix(size, size, 42);
        let x = random_vector(size, 43);
        let mut y = vec![0.0f32; size];

        group.throughput(Throughput::Elements((size * size) as u64));
        group.bench_with_input(BenchmarkId::new("naive", size), &size, |b, &n| {
            b.iter(|| {
                dense_matvec(
                    criterion::black_box(&a),
                    criterion::black_box(&x),
                    criterion::black_box(&mut y),
                    n,
                    n,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Sparse matrix-vector multiply (CSR SpMV)
// ---------------------------------------------------------------------------

fn sparse_spmv_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_spmv");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    for (n, density) in [(1000, 0.01), (1000, 0.05), (10_000, 0.01)] {
        let csr = random_csr_matrix(n, density, 44);
        let x = random_vector(n, 45);
        let mut y = vec![0.0f32; n];

        let label = format!("{}x{}_{:.0}pct", n, n, density * 100.0);
        group.throughput(Throughput::Elements(csr.nnz() as u64));
        group.bench_with_input(BenchmarkId::new(&label, n), &n, |b, _| {
            b.iter(|| {
                csr.spmv(criterion::black_box(&x), criterion::black_box(&mut y));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Dense vs sparse crossover
// ---------------------------------------------------------------------------

/// Benchmark that compares dense and sparse matvec at the same dimension
/// to help identify the crossover point where sparse becomes faster.
fn dense_vs_sparse_crossover(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_vs_sparse_crossover");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    for size in [64, 128, 256, 512, 1024] {
        let density = 0.05;

        // Dense setup.
        let a_dense = random_dense_matrix(size, size, 42);
        let x = random_vector(size, 43);
        let mut y_dense = vec![0.0f32; size];

        group.throughput(Throughput::Elements((size * size) as u64));
        group.bench_with_input(BenchmarkId::new("dense", size), &size, |b, &n| {
            b.iter(|| {
                dense_matvec(
                    criterion::black_box(&a_dense),
                    criterion::black_box(&x),
                    criterion::black_box(&mut y_dense),
                    n,
                    n,
                );
            });
        });

        // Sparse setup.
        let csr = random_csr_matrix(size, density, 44);
        let mut y_sparse = vec![0.0f32; size];

        group.bench_with_input(BenchmarkId::new("sparse_5pct", size), &size, |b, _| {
            b.iter(|| {
                csr.spmv(
                    criterion::black_box(&x),
                    criterion::black_box(&mut y_sparse),
                );
            });
        });
    }
    group.finish();
}

criterion_group!(
    baselines,
    dense_matvec_baseline,
    sparse_spmv_baseline,
    dense_vs_sparse_crossover
);
criterion_main!(baselines);
