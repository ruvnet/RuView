//! Benchmarks for the Neumann series solver.
//!
//! The Neumann series approximates `(I - M)^{-1} b = sum_{k=0}^{K} M^k b`
//! and converges when the spectral radius of `M` is less than 1. These
//! benchmarks measure convergence rate vs tolerance, scaling behaviour, and
//! crossover against dense direct solves.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a diagonally dominant CSR matrix suitable for Neumann iteration.
///
/// The iteration matrix `M = I - D^{-1} A` has spectral radius < 1 when `A`
/// is strictly diagonally dominant. We construct `A` so that each diagonal
/// entry equals the sum of absolute off-diagonal values in its row plus 1.0.
fn diag_dominant_csr(n: usize, density: f64, seed: u64) -> CsrMatrix<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut entries: Vec<(usize, usize, f32)> = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            if rng.gen::<f64>() < density {
                let val: f32 = rng.gen_range(-0.3..0.3);
                entries.push((i, j, val));
                entries.push((j, i, val));
            }
        }
    }

    let mut row_abs_sums = vec![0.0f32; n];
    for &(r, _c, v) in &entries {
        row_abs_sums[r] += v.abs();
    }
    for i in 0..n {
        entries.push((i, i, row_abs_sums[i] + 1.0));
    }

    CsrMatrix::<f32>::from_coo(n, n, entries)
}

/// Random vector with deterministic seed.
fn random_vector(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// ---------------------------------------------------------------------------
// Inline Neumann series solver for benchmarking
// ---------------------------------------------------------------------------

/// Neumann series iteration: x_{k+1} = x_k + (b - A * x_k).
///
/// This is equivalent to the Richardson iteration with omega = 1 for a
/// diagonally-dominant system. We inline it here so the benchmark does
/// not depend on the (currently stub) neumann module.
#[inline(never)]
fn neumann_solve(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f32>, usize, f64) {
    let n = matrix.rows;
    let mut x = vec![0.0f32; n];
    let mut residual_buf = vec![0.0f32; n];
    let mut iterations = 0;
    let mut residual_norm = f64::MAX;

    for k in 0..max_iter {
        // Compute residual: r = b - A*x.
        matrix.spmv(&x, &mut residual_buf);
        for i in 0..n {
            residual_buf[i] = rhs[i] - residual_buf[i];
        }

        // Residual L2 norm.
        residual_norm = residual_buf
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            .sqrt();

        iterations = k + 1;
        if residual_norm < tolerance {
            break;
        }

        // Update: x = x + r (Richardson step).
        for i in 0..n {
            x[i] += residual_buf[i];
        }
    }

    (x, iterations, residual_norm)
}

// ---------------------------------------------------------------------------
// Benchmark: convergence vs tolerance
// ---------------------------------------------------------------------------

fn neumann_convergence(c: &mut Criterion) {
    let mut group = c.benchmark_group("neumann_convergence");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    let n = 500;
    let matrix = diag_dominant_csr(n, 0.02, 42);
    let rhs = random_vector(n, 43);

    for &tol in &[1e-2, 1e-4, 1e-6] {
        let label = format!("eps_{:.0e}", tol);
        group.bench_with_input(BenchmarkId::new(&label, n), &tol, |b, &eps| {
            b.iter(|| {
                neumann_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    eps,
                    5000,
                )
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: scaling with problem size
// ---------------------------------------------------------------------------

fn neumann_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("neumann_scaling");
    group.warm_up_time(Duration::from_secs(3));

    for &n in &[100, 1000, 10_000] {
        // Use sparser matrices for larger sizes to keep runtime reasonable.
        let density = if n <= 1000 { 0.02 } else { 0.005 };
        let matrix = diag_dominant_csr(n, density, 42);
        let rhs = random_vector(n, 43);

        let sample_count = if n >= 10_000 { 20 } else { 100 };
        group.sample_size(sample_count);
        group.throughput(Throughput::Elements(matrix.nnz() as u64));

        group.bench_with_input(BenchmarkId::new("n", n), &n, |b, _| {
            b.iter(|| {
                neumann_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    1e-4,
                    5000,
                )
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Neumann vs dense direct solve crossover
// ---------------------------------------------------------------------------

/// Naive dense direct solve via Gaussian elimination with partial pivoting.
///
/// This is intentionally unoptimized to represent a "no-library" baseline.
#[inline(never)]
fn dense_direct_solve(a: &[f32], b: &[f32], n: usize) -> Vec<f32> {
    // Build augmented matrix [A | b] in row-major order.
    let mut aug = vec![0.0f64; n * (n + 1)];
    for i in 0..n {
        for j in 0..n {
            aug[i * (n + 1) + j] = a[i * n + j] as f64;
        }
        aug[i * (n + 1) + n] = b[i] as f64;
    }

    // Forward elimination with partial pivoting.
    for col in 0..n {
        // Find pivot.
        let mut max_row = col;
        let mut max_val = aug[col * (n + 1) + col].abs();
        for row in (col + 1)..n {
            let val = aug[row * (n + 1) + col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        // Swap rows.
        if max_row != col {
            for j in 0..=n {
                let idx_a = col * (n + 1) + j;
                let idx_b = max_row * (n + 1) + j;
                aug.swap(idx_a, idx_b);
            }
        }

        let pivot = aug[col * (n + 1) + col];
        if pivot.abs() < 1e-15 {
            continue;
        }

        // Eliminate below.
        for row in (col + 1)..n {
            let factor = aug[row * (n + 1) + col] / pivot;
            for j in col..=n {
                let val = aug[col * (n + 1) + j];
                aug[row * (n + 1) + j] -= factor * val;
            }
        }
    }

    // Back substitution.
    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        let mut sum = aug[i * (n + 1) + n];
        for j in (i + 1)..n {
            sum -= aug[i * (n + 1) + j] * x[j];
        }
        let diag = aug[i * (n + 1) + i];
        x[i] = if diag.abs() > 1e-15 { sum / diag } else { 0.0 };
    }

    x.iter().map(|&v| v as f32).collect()
}

/// Generate the dense representation of a diag-dominant matrix.
fn diag_dominant_dense(n: usize, density: f64, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut a = vec![0.0f32; n * n];

    // Off-diagonal.
    for i in 0..n {
        for j in (i + 1)..n {
            if rng.gen::<f64>() < density {
                let val: f32 = rng.gen_range(-0.3..0.3);
                a[i * n + j] = val;
                a[j * n + i] = val;
            }
        }
    }

    // Diagonal dominance.
    for i in 0..n {
        let mut row_sum = 0.0f32;
        for j in 0..n {
            if j != i {
                row_sum += a[i * n + j].abs();
            }
        }
        a[i * n + i] = row_sum + 1.0;
    }

    a
}

fn neumann_vs_dense(c: &mut Criterion) {
    let mut group = c.benchmark_group("neumann_vs_dense");
    group.warm_up_time(Duration::from_secs(3));

    // Crossover analysis: compare iterative Neumann vs dense direct solve.
    // For small n, dense wins; for large sparse n, Neumann should win.
    for &n in &[50, 100, 200, 500] {
        let density = 0.05;
        let rhs = random_vector(n, 43);

        let sample_count = if n >= 500 { 20 } else { 100 };
        group.sample_size(sample_count);

        // Neumann (sparse).
        let csr = diag_dominant_csr(n, density, 42);
        group.bench_with_input(BenchmarkId::new("neumann_sparse", n), &n, |b, _| {
            b.iter(|| {
                neumann_solve(
                    criterion::black_box(&csr),
                    criterion::black_box(&rhs),
                    1e-4,
                    5000,
                )
            });
        });

        // Dense direct solve.
        let a_dense = diag_dominant_dense(n, density, 42);
        group.bench_with_input(BenchmarkId::new("dense_direct", n), &n, |b, _| {
            b.iter(|| {
                dense_direct_solve(
                    criterion::black_box(&a_dense),
                    criterion::black_box(&rhs),
                    n,
                )
            });
        });
    }
    group.finish();
}

criterion_group!(
    neumann,
    neumann_convergence,
    neumann_scaling,
    neumann_vs_dense
);
criterion_main!(neumann);
