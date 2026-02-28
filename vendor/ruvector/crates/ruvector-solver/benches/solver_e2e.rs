//! End-to-end benchmarks for the solver orchestration layer.
//!
//! These benchmarks measure the overhead of algorithm selection (routing) and
//! the full end-to-end solve path including routing, validation, solver
//! dispatch, and result construction.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use ruvector_solver::types::{Algorithm, CsrMatrix};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a diagonally dominant CSR matrix.
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
// Inline algorithm router for benchmarking
// ---------------------------------------------------------------------------

/// Properties extracted from the matrix for routing decisions.
#[allow(dead_code)]
struct MatrixProperties {
    n: usize,
    nnz: usize,
    density: f64,
    is_symmetric: bool,
    max_row_degree: usize,
    diag_dominance_ratio: f64,
}

/// Analyze a CSR matrix to extract routing-relevant properties.
#[inline(never)]
fn analyze_matrix(matrix: &CsrMatrix<f32>) -> MatrixProperties {
    let n = matrix.rows;
    let nnz = matrix.nnz();
    let density = nnz as f64 / (n as f64 * n as f64);

    // Check symmetry (sample-based for large matrices).
    let sample_size = n.min(100);
    let mut is_symmetric = true;
    'outer: for i in 0..sample_size {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        for idx in start..end {
            let j = matrix.col_indices[idx];
            if j == i {
                continue;
            }
            // Check if (j, i) exists with the same value.
            let j_start = matrix.row_ptr[j];
            let j_end = matrix.row_ptr[j + 1];
            let mut found = false;
            for jidx in j_start..j_end {
                if matrix.col_indices[jidx] == i {
                    if (matrix.values[jidx] - matrix.values[idx]).abs() > 1e-6 {
                        is_symmetric = false;
                        break 'outer;
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                is_symmetric = false;
                break 'outer;
            }
        }
    }

    // Max row degree.
    let mut max_row_degree = 0;
    for i in 0..n {
        let deg = matrix.row_ptr[i + 1] - matrix.row_ptr[i];
        max_row_degree = max_row_degree.max(deg);
    }

    // Diagonal dominance ratio (sampled).
    let mut diag_dominance_ratio = 0.0;
    let check_rows = n.min(100);
    for i in 0..check_rows {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let mut diag = 0.0f32;
        let mut off_diag_sum = 0.0f32;
        for idx in start..end {
            if matrix.col_indices[idx] == i {
                diag = matrix.values[idx].abs();
            } else {
                off_diag_sum += matrix.values[idx].abs();
            }
        }
        if off_diag_sum > 0.0 {
            diag_dominance_ratio += (diag / off_diag_sum) as f64;
        } else {
            diag_dominance_ratio += 10.0; // Perfect dominance.
        }
    }
    diag_dominance_ratio /= check_rows as f64;

    MatrixProperties {
        n,
        nnz,
        density,
        is_symmetric,
        max_row_degree,
        diag_dominance_ratio,
    }
}

/// Select the best algorithm based on matrix properties.
#[inline(never)]
fn select_algorithm(props: &MatrixProperties, tolerance: f64) -> Algorithm {
    // High diagonal dominance => Neumann series converges fast.
    if props.diag_dominance_ratio > 2.0 && tolerance > 1e-8 {
        return Algorithm::Neumann;
    }

    // SPD matrix => CG is optimal.
    if props.is_symmetric && props.diag_dominance_ratio > 1.0 {
        return Algorithm::CG;
    }

    // Very sparse, large graph => forward push for PPR-like problems.
    if props.density < 0.01 && props.n > 1000 {
        return Algorithm::ForwardPush;
    }

    // Default fallback.
    if props.is_symmetric {
        Algorithm::CG
    } else {
        Algorithm::Neumann
    }
}

// ---------------------------------------------------------------------------
// Inline solvers for e2e benchmarking
// ---------------------------------------------------------------------------

/// Neumann series (Richardson iteration).
#[inline(never)]
fn neumann_solve(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f32>, usize, f64) {
    let n = matrix.rows;
    let mut x = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut iterations = 0;
    let mut residual_norm = f64::MAX;

    for k in 0..max_iter {
        matrix.spmv(&x, &mut r);
        for i in 0..n {
            r[i] = rhs[i] - r[i];
        }
        residual_norm = r
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            .sqrt();
        iterations = k + 1;
        if residual_norm < tolerance {
            break;
        }
        for i in 0..n {
            x[i] += r[i];
        }
    }
    (x, iterations, residual_norm)
}

/// Conjugate gradient.
#[inline(never)]
fn cg_solve(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f32>, usize, f64) {
    let n = matrix.rows;
    let mut x = vec![0.0f32; n];
    let mut r = rhs.to_vec();
    let mut p = r.clone();
    let mut ap = vec![0.0f32; n];

    let mut rs_old: f64 = r.iter().map(|&v| (v as f64) * (v as f64)).sum();
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
        let alpha = rs_old / p_ap;

        for i in 0..n {
            x[i] += (alpha as f32) * p[i];
            r[i] -= (alpha as f32) * ap[i];
        }

        let rs_new: f64 = r.iter().map(|&v| (v as f64) * (v as f64)).sum();
        iterations = k + 1;
        if rs_new.sqrt() < tolerance {
            break;
        }

        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + (beta as f32) * p[i];
        }
        rs_old = rs_new;
    }

    let residual_norm = rs_old.sqrt();
    (x, iterations, residual_norm)
}

/// Full orchestrated solve: analyze -> route -> solve.
#[inline(never)]
fn orchestrator_solve_impl(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
    max_iter: usize,
) -> (Vec<f32>, usize, f64, Algorithm) {
    let props = analyze_matrix(matrix);
    let algorithm = select_algorithm(&props, tolerance);

    let (solution, iterations, residual) = match algorithm {
        Algorithm::Neumann => neumann_solve(matrix, rhs, tolerance, max_iter),
        Algorithm::CG => cg_solve(matrix, rhs, tolerance, max_iter),
        // Fall back to CG for unimplemented algorithms.
        _ => cg_solve(matrix, rhs, tolerance, max_iter),
    };

    (solution, iterations, residual, algorithm)
}

// ---------------------------------------------------------------------------
// Benchmark: router overhead (analyze + select, no solve)
// ---------------------------------------------------------------------------

fn router_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("router_overhead");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    for &n in &[100, 1000, 10_000] {
        let density = if n <= 1000 { 0.02 } else { 0.005 };
        let matrix = diag_dominant_csr(n, density, 42);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("analyze_and_route", n), &n, |b, _| {
            b.iter(|| {
                let props = analyze_matrix(criterion::black_box(&matrix));
                select_algorithm(criterion::black_box(&props), 1e-6)
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: full orchestrated solve (end-to-end)
// ---------------------------------------------------------------------------

fn orchestrator_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("orchestrator_solve");
    group.warm_up_time(Duration::from_secs(3));

    for &n in &[100, 500, 1000, 5000] {
        let density = if n <= 1000 { 0.02 } else { 0.005 };
        let matrix = diag_dominant_csr(n, density, 42);
        let rhs = random_vector(n, 43);

        let sample_count = if n >= 5000 { 20 } else { 100 };
        group.sample_size(sample_count);
        group.throughput(Throughput::Elements(matrix.nnz() as u64));

        group.bench_with_input(BenchmarkId::new("e2e", n), &n, |b, _| {
            b.iter(|| {
                orchestrator_solve_impl(
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
// Benchmark: routing overhead as fraction of total solve time
// ---------------------------------------------------------------------------

fn routing_fraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_fraction");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    let n = 1000;
    let density = 0.02;
    let matrix = diag_dominant_csr(n, density, 42);
    let rhs = random_vector(n, 43);

    // Route only.
    group.bench_function("route_only", |b| {
        b.iter(|| {
            let props = analyze_matrix(criterion::black_box(&matrix));
            select_algorithm(criterion::black_box(&props), 1e-6)
        });
    });

    // Solve only (skip routing).
    group.bench_function("solve_only_cg", |b| {
        b.iter(|| {
            cg_solve(
                criterion::black_box(&matrix),
                criterion::black_box(&rhs),
                1e-6,
                5000,
            )
        });
    });

    // Full e2e (route + solve).
    group.bench_function("e2e_routed", |b| {
        b.iter(|| {
            orchestrator_solve_impl(
                criterion::black_box(&matrix),
                criterion::black_box(&rhs),
                1e-6,
                5000,
            )
        });
    });

    group.finish();
}

criterion_group!(e2e, router_overhead, orchestrator_solve, routing_fraction);
criterion_main!(e2e);
