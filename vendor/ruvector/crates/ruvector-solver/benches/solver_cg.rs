//! Benchmarks for the Conjugate Gradient (CG) solver.
//!
//! CG is the method of choice for symmetric positive-definite (SPD) systems.
//! These benchmarks measure scaling behaviour, the effect of diagonal
//! preconditioning, and a head-to-head comparison with the Neumann series
//! solver.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a symmetric positive-definite (SPD) CSR matrix.
///
/// Constructs a sparse SPD matrix by generating random off-diagonal entries
/// and ensuring strict diagonal dominance: `a_{ii} = sum_j |a_{ij}| + 1`.
fn spd_csr_matrix(n: usize, density: f64, seed: u64) -> CsrMatrix<f32> {
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
// Inline CG solver for benchmarking
// ---------------------------------------------------------------------------

/// Conjugate gradient solver for SPD systems `Ax = b`.
///
/// This is a textbook CG implementation inlined here so the benchmark does
/// not depend on the (currently stub) cg module.
#[inline(never)]
fn cg_solve(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f32>, usize, f64) {
    let n = matrix.rows;
    let mut x = vec![0.0f32; n];
    let mut r = rhs.to_vec(); // r_0 = b - A*x_0, with x_0 = 0 => r_0 = b
    let mut p = r.clone();
    let mut ap = vec![0.0f32; n];

    let mut rs_old: f64 = r.iter().map(|&v| (v as f64) * (v as f64)).sum();
    let mut iterations = 0;

    for k in 0..max_iter {
        // ap = A * p
        matrix.spmv(&p, &mut ap);

        // alpha = (r^T r) / (p^T A p)
        let p_ap: f64 = p
            .iter()
            .zip(ap.iter())
            .map(|(&pi, &api)| (pi as f64) * (api as f64))
            .sum();

        if p_ap.abs() < 1e-30 {
            iterations = k + 1;
            break;
        }

        let alpha = rs_old / p_ap;

        // x = x + alpha * p
        for i in 0..n {
            x[i] += (alpha as f32) * p[i];
        }

        // r = r - alpha * ap
        for i in 0..n {
            r[i] -= (alpha as f32) * ap[i];
        }

        let rs_new: f64 = r.iter().map(|&v| (v as f64) * (v as f64)).sum();
        iterations = k + 1;

        if rs_new.sqrt() < tolerance {
            break;
        }

        // p = r + (rs_new / rs_old) * p
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + (beta as f32) * p[i];
        }

        rs_old = rs_new;
    }

    let residual_norm = rs_old.sqrt();
    (x, iterations, residual_norm)
}

/// Diagonal-preconditioned CG solver.
///
/// Uses the Jacobi (diagonal) preconditioner: `M = diag(A)`.
/// Solves `M^{-1} A x = M^{-1} b` via the preconditioned CG algorithm.
#[inline(never)]
fn pcg_solve(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f32>, usize, f64) {
    let n = matrix.rows;

    // Extract diagonal for preconditioner.
    let mut diag_inv = vec![1.0f32; n];
    for i in 0..n {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        for idx in start..end {
            if matrix.col_indices[idx] == i {
                let d = matrix.values[idx];
                diag_inv[i] = if d.abs() > 1e-12 { 1.0 / d } else { 1.0 };
                break;
            }
        }
    }

    let mut x = vec![0.0f32; n];
    let mut r = rhs.to_vec();
    let mut z: Vec<f32> = r
        .iter()
        .zip(diag_inv.iter())
        .map(|(&ri, &di)| ri * di)
        .collect();
    let mut p = z.clone();
    let mut ap = vec![0.0f32; n];

    let mut rz_old: f64 = r
        .iter()
        .zip(z.iter())
        .map(|(&ri, &zi)| (ri as f64) * (zi as f64))
        .sum();

    let mut iterations = 0;

    for k in 0..max_iter {
        matrix.spmv(&p, &mut ap);

        let p_ap: f64 = p
            .iter()
            .zip(ap.iter())
            .map(|(&pi, &api)| (pi as f64) * (api as f64))
            .sum();

        if p_ap.abs() < 1e-30 {
            iterations = k + 1;
            break;
        }

        let alpha = rz_old / p_ap;

        for i in 0..n {
            x[i] += (alpha as f32) * p[i];
            r[i] -= (alpha as f32) * ap[i];
        }

        let residual_norm: f64 = r
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            .sqrt();
        iterations = k + 1;

        if residual_norm < tolerance {
            break;
        }

        // z = M^{-1} r
        for i in 0..n {
            z[i] = r[i] * diag_inv[i];
        }

        let rz_new: f64 = r
            .iter()
            .zip(z.iter())
            .map(|(&ri, &zi)| (ri as f64) * (zi as f64))
            .sum();

        let beta = rz_new / rz_old;
        for i in 0..n {
            p[i] = z[i] + (beta as f32) * p[i];
        }

        rz_old = rz_new;
    }

    let residual_norm = r
        .iter()
        .map(|&v| (v as f64) * (v as f64))
        .sum::<f64>()
        .sqrt();
    (x, iterations, residual_norm)
}

/// Neumann series iteration (inlined for comparison benchmark).
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
        matrix.spmv(&x, &mut residual_buf);
        for i in 0..n {
            residual_buf[i] = rhs[i] - residual_buf[i];
        }

        residual_norm = residual_buf
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            .sqrt();

        iterations = k + 1;
        if residual_norm < tolerance {
            break;
        }

        for i in 0..n {
            x[i] += residual_buf[i];
        }
    }

    (x, iterations, residual_norm)
}

// ---------------------------------------------------------------------------
// Benchmark: CG scaling with problem size
// ---------------------------------------------------------------------------

fn cg_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("cg_scaling");
    group.warm_up_time(Duration::from_secs(3));

    for &n in &[100, 1000, 10_000] {
        let density = if n <= 1000 { 0.02 } else { 0.005 };
        let matrix = spd_csr_matrix(n, density, 42);
        let rhs = random_vector(n, 43);

        let sample_count = if n >= 10_000 { 20 } else { 100 };
        group.sample_size(sample_count);
        group.throughput(Throughput::Elements(matrix.nnz() as u64));

        group.bench_with_input(BenchmarkId::new("n", n), &n, |b, _| {
            b.iter(|| {
                cg_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    1e-6,
                    5000,
                )
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: with vs without diagonal preconditioner
// ---------------------------------------------------------------------------

fn cg_preconditioning(c: &mut Criterion) {
    let mut group = c.benchmark_group("cg_preconditioning");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    for &n in &[500, 1000, 2000] {
        let matrix = spd_csr_matrix(n, 0.02, 42);
        let rhs = random_vector(n, 43);

        group.bench_with_input(BenchmarkId::new("cg_plain", n), &n, |b, _| {
            b.iter(|| {
                cg_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    1e-6,
                    5000,
                )
            });
        });

        group.bench_with_input(BenchmarkId::new("cg_diag_precond", n), &n, |b, _| {
            b.iter(|| {
                pcg_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    1e-6,
                    5000,
                )
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: CG vs Neumann for same problem
// ---------------------------------------------------------------------------

fn cg_vs_neumann(c: &mut Criterion) {
    let mut group = c.benchmark_group("cg_vs_neumann");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    for &n in &[100, 500, 1000] {
        let matrix = spd_csr_matrix(n, 0.02, 42);
        let rhs = random_vector(n, 43);

        group.bench_with_input(BenchmarkId::new("cg", n), &n, |b, _| {
            b.iter(|| {
                cg_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    1e-6,
                    5000,
                )
            });
        });

        group.bench_with_input(BenchmarkId::new("neumann", n), &n, |b, _| {
            b.iter(|| {
                neumann_solve(
                    criterion::black_box(&matrix),
                    criterion::black_box(&rhs),
                    1e-6,
                    5000,
                )
            });
        });
    }
    group.finish();
}

criterion_group!(cg, cg_scaling, cg_preconditioning, cg_vs_neumann);
criterion_main!(cg);
