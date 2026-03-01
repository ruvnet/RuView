//! DNA Solver Benchmarks -- ruvector-solver integration
//!
//! Three benchmark groups targeting real DNA analysis scenarios:
//! A. Localized relevance via Forward Push PPR on k-mer graphs
//! B. Laplacian solve for sequence denoising/consistency
//! C. Cohort-scale label propagation
//!
//! Uses real human gene sequences from NCBI RefSeq (HBB, TP53, BRCA1, CYP2D6, INS).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ruvector_solver::cg::ConjugateGradientSolver;
use ruvector_solver::forward_push::ForwardPushSolver;
use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::traits::SolverEngine;
use ruvector_solver::types::{ComputeBudget, CsrMatrix};
use rvdna::kmer_pagerank::KmerGraphRanker;
use rvdna::real_data;

// ============================================================================
// Helpers
// ============================================================================

/// Real gene sequences from NCBI RefSeq
fn real_gene_sequences() -> Vec<&'static [u8]> {
    vec![
        real_data::HBB_CODING_SEQUENCE.as_bytes(),
        real_data::TP53_EXONS_5_8.as_bytes(),
        real_data::BRCA1_EXON11_FRAGMENT.as_bytes(),
        real_data::CYP2D6_CODING.as_bytes(),
        real_data::INS_CODING.as_bytes(),
    ]
}

/// Generate synthetic DNA sequences with mutations from a template
fn mutated_sequences(template: &[u8], count: usize, mutation_rate: f64, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let bases = [b'A', b'C', b'G', b'T'];
    (0..count)
        .map(|_| {
            template
                .iter()
                .map(|&b| {
                    if rng.gen::<f64>() < mutation_rate {
                        bases[rng.gen_range(0..4)]
                    } else {
                        b
                    }
                })
                .collect()
        })
        .collect()
}

/// Build k-mer fingerprint vector for a sequence using FNV-1a hashing
fn fingerprint(seq: &[u8], k: usize, dims: usize) -> Vec<f64> {
    if seq.len() < k {
        return vec![0.0; dims];
    }
    let mut counts = vec![0u32; dims];
    for window in seq.windows(k) {
        let hash = fnv1a(window);
        counts[hash % dims] += 1;
    }
    let total: u32 = counts.iter().sum();
    if total == 0 {
        return vec![0.0; dims];
    }
    let inv = 1.0 / total as f64;
    counts.iter().map(|&c| c as f64 * inv).collect()
}

fn fnv1a(data: &[u8]) -> usize {
    let mut hash: u64 = 14695981039346656037;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash as usize
}

fn cosine_sim(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na < 1e-15 || nb < 1e-15 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Build a column-stochastic transition matrix from sequence fingerprints.
///
/// Edge weights are cosine similarities above `threshold`, normalized so
/// each column sums to 1. Isolated nodes get a self-loop.
fn build_stochastic_matrix(fps: &[Vec<f64>], threshold: f64) -> CsrMatrix<f64> {
    let n = fps.len();
    let mut col_sums = vec![0.0f64; n];
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let sim = cosine_sim(&fps[i], &fps[j]);
            if sim > threshold {
                entries.push((i, j, sim));
                col_sums[j] += sim;
            }
        }
    }

    let mut normalized: Vec<(usize, usize, f64)> = entries
        .into_iter()
        .map(|(i, j, w)| (i, j, w / col_sums[j].max(1e-15)))
        .collect();

    // Self-loops for dangling nodes
    for j in 0..n {
        if col_sums[j] < 1e-15 {
            normalized.push((j, j, 1.0));
        }
    }

    CsrMatrix::<f64>::from_coo(n, n, normalized)
}

/// Build graph Laplacian from fingerprints: L = D - A (with small regularization).
///
/// The regularization term (0.01 added to each diagonal) ensures the Laplacian
/// is strictly positive definite, which is required for both the Neumann solver
/// (diagonal dominance) and the CG solver (SPD requirement).
fn build_laplacian(fps: &[Vec<f64>], threshold: f64) -> CsrMatrix<f64> {
    let n = fps.len();
    let mut degree = vec![0.0f64; n];
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let sim = cosine_sim(&fps[i], &fps[j]);
            if sim > threshold {
                entries.push((i, j, -sim));
                entries.push((j, i, -sim));
                degree[i] += sim;
                degree[j] += sim;
            }
        }
    }

    // Diagonal: degree + regularization for positive-definiteness
    for (i, &d) in degree.iter().enumerate() {
        entries.push((i, i, d + 0.01));
    }

    CsrMatrix::<f64>::from_coo(n, n, entries)
}

// ============================================================================
// Group A: Localized Relevance on K-mer Graphs (Forward Push PPR)
// ============================================================================

fn localized_relevance_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("solver_ppr");
    group.sample_size(30);

    // Benchmark with real genes using KmerGraphRanker
    {
        let genes = real_gene_sequences();
        let ranker = KmerGraphRanker::new(11, 128);

        group.bench_function("real_genes_5seq", |b| {
            b.iter(|| black_box(ranker.rank_sequences(&genes, 0.15, 1e-4, 0.05)));
        });
    }

    // Scale with mutated cohorts using ForwardPushSolver directly
    for &n in &[50usize, 100, 500] {
        let template = real_data::HBB_CODING_SEQUENCE.as_bytes();
        let mutated = mutated_sequences(template, n, 0.05, 42);
        let fps: Vec<Vec<f64>> = mutated.iter().map(|s| fingerprint(s, 11, 128)).collect();
        let matrix = build_stochastic_matrix(&fps, 0.05);

        let solver = ForwardPushSolver::new(0.15, 1e-4);

        group.bench_with_input(BenchmarkId::new("ppr_single_source", n), &n, |b, _| {
            b.iter(|| black_box(solver.ppr_from_source(&matrix, 0)));
        });
    }

    group.finish();
}

// ============================================================================
// Group B: Laplacian Solve for Denoising / Consistency
// ============================================================================

fn laplacian_solve_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("solver_laplacian");
    group.sample_size(20);

    for &n in &[50usize, 100, 500] {
        let template = real_data::TP53_EXONS_5_8.as_bytes();
        let mutated = mutated_sequences(template, n, 0.03, 42);
        let fps: Vec<Vec<f64>> = mutated.iter().map(|s| fingerprint(s, 11, 128)).collect();
        let laplacian = build_laplacian(&fps, 0.1);

        // RHS: noisy signal (first 10% = 1.0, rest = small noise)
        let mut rhs = vec![0.0f64; n];
        let mut rng = StdRng::seed_from_u64(42);
        for i in 0..n {
            rhs[i] = if i < n / 10 {
                1.0
            } else {
                rng.gen::<f64>() * 0.1
            };
        }

        let budget = ComputeBudget::default();

        // Neumann solver (via SolverEngine trait, f64 -> f32 conversion)
        let neumann = NeumannSolver::new(1e-6, 200);

        group.bench_with_input(BenchmarkId::new("neumann_denoise", n), &n, |b, _| {
            b.iter(|| {
                // Neumann may fail on non-diag-dominant Laplacians;
                // the benchmark measures attempt latency regardless.
                let _ = black_box(SolverEngine::solve(&neumann, &laplacian, &rhs, &budget));
            });
        });

        // CG solver (preconditioned, well-suited for SPD Laplacians)
        let cg = ConjugateGradientSolver::new(1e-6, 500, true);

        group.bench_with_input(BenchmarkId::new("cg_denoise", n), &n, |b, _| {
            b.iter(|| black_box(SolverEngine::solve(&cg, &laplacian, &rhs, &budget)));
        });
    }

    group.finish();
}

// ============================================================================
// Group C: Cohort-Scale Label Propagation
// ============================================================================

fn cohort_propagation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("solver_cohort");
    group.sample_size(10);

    for &n in &[100usize, 500, 1000] {
        // Build mixed cohort: HBB variants + TP53 variants + BRCA1 variants
        let mut all_seqs: Vec<Vec<u8>> = Vec::new();
        let genes: Vec<&[u8]> = vec![
            real_data::HBB_CODING_SEQUENCE.as_bytes(),
            real_data::TP53_EXONS_5_8.as_bytes(),
            real_data::BRCA1_EXON11_FRAGMENT.as_bytes(),
        ];

        let per_gene = n / 3;
        for (gi, gene) in genes.iter().enumerate() {
            let variants = mutated_sequences(gene, per_gene, 0.04, 42 + gi as u64);
            all_seqs.extend(variants);
        }
        // Fill remainder with HBB variants
        while all_seqs.len() < n {
            let extra = mutated_sequences(genes[0], 1, 0.05, 99 + all_seqs.len() as u64);
            all_seqs.extend(extra);
        }
        all_seqs.truncate(n);

        let fps: Vec<Vec<f64>> = all_seqs.iter().map(|s| fingerprint(s, 11, 128)).collect();
        let laplacian = build_laplacian(&fps, 0.05);

        // Label propagation: known labels for first 10% of each gene group
        let mut labels = vec![0.0f64; n];
        let labeled_count = (per_gene / 10).max(1);
        for i in 0..labeled_count.min(n) {
            labels[i] = 1.0; // Gene group 1 (HBB)
        }
        for i in per_gene..(per_gene + labeled_count).min(n) {
            labels[i] = 2.0; // Gene group 2 (TP53)
        }
        let start_3 = 2 * per_gene;
        for i in start_3..(start_3 + labeled_count).min(n) {
            labels[i] = 3.0; // Gene group 3 (BRCA1)
        }

        let cg = ConjugateGradientSolver::new(1e-6, 1000, true);
        let budget = ComputeBudget::default();

        group.bench_with_input(BenchmarkId::new("label_propagation", n), &n, |b, _| {
            b.iter(|| black_box(SolverEngine::solve(&cg, &laplacian, &labels, &budget)));
        });
    }

    group.finish();
}

// ============================================================================
// Configuration
// ============================================================================

criterion_group!(
    benches,
    localized_relevance_benchmarks,
    laplacian_solve_benchmarks,
    cohort_propagation_benchmarks,
);

criterion_main!(benches);
