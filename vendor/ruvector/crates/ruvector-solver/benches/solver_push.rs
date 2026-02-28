//! Benchmarks for the forward push algorithm (Andersen-Chung-Lang).
//!
//! Forward push computes approximate Personalized PageRank (PPR) vectors in
//! sublinear time. These benchmarks measure scaling with graph size and the
//! effect of tolerance on the number of push operations.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::VecDeque;
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a random sparse graph as a CSR matrix suitable for PageRank.
///
/// Each entry `A[i][j]` represents the transition probability from node `i`
/// to node `j`. The matrix is row-stochastic: each row sums to 1. The
/// graph is constructed by giving each node `avg_degree` random outgoing
/// edges.
fn random_graph_csr(n: usize, avg_degree: usize, seed: u64) -> CsrMatrix<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut entries: Vec<(usize, usize, f32)> = Vec::new();

    for i in 0..n {
        let degree = (avg_degree as f64 * (0.5 + rng.gen::<f64>())) as usize;
        let degree = degree.max(1).min(n - 1);

        // Select random neighbours (without replacement for small degree).
        let mut neighbours = Vec::with_capacity(degree);
        for _ in 0..degree {
            let mut j = rng.gen_range(0..n);
            while j == i {
                j = rng.gen_range(0..n);
            }
            neighbours.push(j);
        }
        neighbours.sort_unstable();
        neighbours.dedup();

        let weight = 1.0 / neighbours.len() as f32;
        for &j in &neighbours {
            entries.push((i, j, weight));
        }
    }

    CsrMatrix::<f32>::from_coo(n, n, entries)
}

// ---------------------------------------------------------------------------
// Inline forward push for benchmarking
// ---------------------------------------------------------------------------

/// Forward push algorithm for approximate Personalized PageRank.
///
/// Computes an approximate PPR vector `pi` for a source node `source` with
/// teleport probability `alpha`. The algorithm maintains a residual vector
/// and pushes mass from nodes whose residual exceeds `tolerance`.
///
/// Returns `(estimate, residual, num_pushes)`.
#[inline(never)]
fn forward_push(
    matrix: &CsrMatrix<f32>,
    source: usize,
    alpha: f32,
    tolerance: f32,
) -> (Vec<f32>, Vec<f32>, usize) {
    let n = matrix.rows;
    let mut estimate = vec![0.0f32; n];
    let mut residual = vec![0.0f32; n];
    residual[source] = 1.0;

    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(source);
    let mut in_queue = vec![false; n];
    in_queue[source] = true;

    let mut num_pushes = 0usize;

    while let Some(u) = queue.pop_front() {
        in_queue[u] = false;
        let r_u = residual[u];

        if r_u.abs() < tolerance {
            continue;
        }

        num_pushes += 1;

        // Absorb alpha fraction.
        estimate[u] += alpha * r_u;
        let push_mass = (1.0 - alpha) * r_u;
        residual[u] = 0.0;

        // Distribute remaining mass to neighbours.
        let start = matrix.row_ptr[u];
        let end = matrix.row_ptr[u + 1];
        let degree = end - start;

        if degree > 0 {
            for idx in start..end {
                let v = matrix.col_indices[idx];
                let w = matrix.values[idx];
                residual[v] += push_mass * w;

                if !in_queue[v] && residual[v].abs() >= tolerance {
                    queue.push_back(v);
                    in_queue[v] = true;
                }
            }
        } else {
            // Dangling node: teleport back to source.
            residual[source] += push_mass;
            if !in_queue[source] && residual[source].abs() >= tolerance {
                queue.push_back(source);
                in_queue[source] = true;
            }
        }
    }

    (estimate, residual, num_pushes)
}

// ---------------------------------------------------------------------------
// Benchmark: forward push scaling with graph size
// ---------------------------------------------------------------------------

fn forward_push_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("forward_push_scaling");
    group.warm_up_time(Duration::from_secs(3));

    let alpha = 0.15f32;
    let tolerance = 1e-4f32;

    for &n in &[100, 1000, 10_000, 100_000] {
        let avg_degree = 10;
        let graph = random_graph_csr(n, avg_degree, 42);

        let sample_count = if n >= 100_000 {
            10
        } else if n >= 10_000 {
            20
        } else {
            100
        };
        group.sample_size(sample_count);
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("n", n), &n, |b, _| {
            b.iter(|| {
                forward_push(
                    criterion::black_box(&graph),
                    0, // source node
                    alpha,
                    tolerance,
                )
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: forward push tolerance sensitivity
// ---------------------------------------------------------------------------

fn forward_push_tolerance(c: &mut Criterion) {
    let mut group = c.benchmark_group("forward_push_tolerance");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(100);

    let n = 10_000;
    let avg_degree = 10;
    let alpha = 0.15f32;
    let graph = random_graph_csr(n, avg_degree, 42);

    for &tol in &[1e-2f32, 1e-4, 1e-6] {
        let label = format!("eps_{:.0e}", tol);
        group.bench_with_input(BenchmarkId::new(&label, n), &tol, |b, &eps| {
            b.iter(|| forward_push(criterion::black_box(&graph), 0, alpha, eps));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: forward push with varying graph density
// ---------------------------------------------------------------------------

fn forward_push_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("forward_push_density");
    group.warm_up_time(Duration::from_secs(3));
    group.sample_size(50);

    let n = 10_000;
    let alpha = 0.15f32;
    let tolerance = 1e-4f32;

    for &avg_degree in &[5, 10, 20, 50] {
        let graph = random_graph_csr(n, avg_degree, 42);

        let label = format!("deg_{}", avg_degree);
        group.throughput(Throughput::Elements(graph.nnz() as u64));
        group.bench_with_input(BenchmarkId::new(&label, n), &avg_degree, |b, _| {
            b.iter(|| forward_push(criterion::black_box(&graph), 0, alpha, tolerance));
        });
    }
    group.finish();
}

criterion_group!(
    push,
    forward_push_scaling,
    forward_push_tolerance,
    forward_push_density
);
criterion_main!(push);
