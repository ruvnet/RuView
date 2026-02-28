//! Solver Benchmark Results in RVF
//!
//! Demonstrates storing, querying, and comparing iterative solver benchmark
//! results using RVF vector stores. Benchmark parameters (algorithm, matrix
//! size, density, tolerance) and timing data (wall time, iterations,
//! convergence rate) are recorded as metadata on per-run embedding vectors.
//!
//! This enables similarity search across benchmark runs: find the closest
//! historical benchmark to a new problem, discover which algorithm performs
//! best for specific problem characteristics, and aggregate statistics
//! across runs.
//!
//! Features:
//!   - Benchmark parameter encoding as vector embeddings
//!   - Metadata-rich timing and convergence records
//!   - Multi-algorithm comparison on the same problem
//!   - Filtered queries by algorithm and problem characteristics
//!   - Aggregation across benchmark runs
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example solver_benchmark

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// Simple LCG-based pseudo-random number generator for deterministic results.
fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state >> 33
}

/// Simple LCG-based pseudo-random f64 in [0, 1).
fn lcg_f64(state: &mut u64) -> f64 {
    lcg_next(state) as f64 / u32::MAX as f64
}

/// Solver algorithms for benchmarking.
const ALGORITHMS: &[&str] = &["cg", "gmres", "bicgstab", "jacobi", "sor"];

/// Problem categories based on matrix structure.
const PROBLEM_TYPES: &[&str] = &[
    "laplacian",
    "elasticity",
    "diffusion",
    "convection",
    "helmholtz",
];

/// A single benchmark run result.
struct BenchmarkRun {
    /// Run identifier.
    run_id: u64,
    /// Solver algorithm name.
    algorithm: &'static str,
    /// Problem type / matrix structure.
    problem_type: &'static str,
    /// Matrix dimension (N x N).
    matrix_size: u64,
    /// Matrix density (0.0 to 1.0).
    density: f64,
    /// Solver tolerance.
    tolerance: f64,
    /// Wall clock time in microseconds.
    wall_time_us: u64,
    /// Number of iterations to converge.
    iterations: u64,
    /// Final residual norm.
    final_residual: f64,
    /// Convergence rate (residual ratio between iterations).
    convergence_rate: f64,
    /// Whether the solver converged within max iterations.
    converged: bool,
}

/// Encode benchmark parameters as a fixed-size embedding vector.
///
/// The embedding captures the problem characteristics so that similar
/// benchmarks (same problem type, similar size/density) are nearby in
/// vector space.
fn benchmark_to_embedding(run: &BenchmarkRun, dim: usize) -> Vec<f32> {
    let mut embedding = vec![0.0f32; dim];

    // Feature 0: log2(matrix_size) normalized to [0, 1]
    embedding[0] = ((run.matrix_size as f64).log2() / 20.0) as f32;

    // Feature 1: density
    embedding[1] = run.density as f32;

    // Feature 2: log10(tolerance) normalized
    embedding[2] = (run.tolerance.log10().abs() / 16.0) as f32;

    // Feature 3: algorithm one-hot hash (spread across features 3-7)
    let algo_idx = ALGORITHMS.iter().position(|&a| a == run.algorithm).unwrap_or(0);
    embedding[3 + algo_idx] = 1.0;

    // Features 8-12: problem type one-hot hash
    let prob_idx = PROBLEM_TYPES
        .iter()
        .position(|&p| p == run.problem_type)
        .unwrap_or(0);
    embedding[8 + prob_idx] = 1.0;

    // Feature 13: log2(wall_time_us) normalized
    embedding[13] = ((run.wall_time_us as f64 + 1.0).log2() / 30.0) as f32;

    // Feature 14: log2(iterations) normalized
    embedding[14] = ((run.iterations as f64 + 1.0).log2() / 16.0) as f32;

    // Feature 15: convergence rate
    embedding[15] = run.convergence_rate as f32;

    // Feature 16: converged flag
    embedding[16] = if run.converged { 1.0 } else { 0.0 };

    // Feature 17: log10(final_residual) normalized
    embedding[17] = (run.final_residual.log10().abs() / 16.0) as f32;

    // Features 18+: interaction terms (size * density, size * tolerance, etc.)
    embedding[18] = (embedding[0] * embedding[1]).min(1.0);
    embedding[19] = (embedding[0] * embedding[2]).min(1.0);
    embedding[20] = (embedding[1] * embedding[15]).min(1.0);

    embedding
}

/// Simulate a benchmark run with deterministic pseudo-random results.
fn simulate_benchmark(
    run_id: u64,
    algorithm: &'static str,
    problem_type: &'static str,
    matrix_size: u64,
    density: f64,
    tolerance: f64,
    seed: u64,
) -> BenchmarkRun {
    let mut state = seed.wrapping_add(run_id * 31 + 7);

    // Base iteration count depends on algorithm and problem size
    let algo_factor = match algorithm {
        "cg" => 0.5,
        "gmres" => 0.7,
        "bicgstab" => 0.6,
        "jacobi" => 2.0,
        "sor" => 1.2,
        _ => 1.0,
    };

    // Problem difficulty factor
    let prob_factor = match problem_type {
        "laplacian" => 0.8,
        "elasticity" => 1.5,
        "diffusion" => 1.0,
        "convection" => 1.8,
        "helmholtz" => 2.0,
        _ => 1.0,
    };

    let base_iters = (matrix_size as f64).sqrt() * algo_factor * prob_factor;
    let noise = 0.8 + lcg_f64(&mut state) * 0.4; // 0.8 to 1.2
    let iterations = (base_iters * noise * (1.0 / tolerance).log10()) as u64;
    let iterations = iterations.max(1).min(10000);

    // Convergence rate: closer to 1.0 means slower convergence
    let base_rate = match algorithm {
        "cg" => 0.85,
        "gmres" => 0.80,
        "bicgstab" => 0.82,
        "jacobi" => 0.95,
        "sor" => 0.90,
        _ => 0.90,
    };
    let convergence_rate = base_rate + lcg_f64(&mut state) * 0.05;

    // Determine if solver converged
    let converged = convergence_rate < 0.99 && iterations < 10000;

    // Final residual
    let final_residual = if converged {
        tolerance * (0.1 + lcg_f64(&mut state) * 0.9)
    } else {
        tolerance * (10.0 + lcg_f64(&mut state) * 100.0)
    };

    // Wall time: proportional to iterations * matrix_size * density
    let time_factor = iterations as f64 * matrix_size as f64 * density;
    let wall_time_us = (time_factor * 0.01 * (0.8 + lcg_f64(&mut state) * 0.4)) as u64;

    BenchmarkRun {
        run_id,
        algorithm,
        problem_type,
        matrix_size,
        density,
        tolerance,
        wall_time_us,
        iterations,
        final_residual,
        convergence_rate,
        converged,
    }
}

fn main() {
    println!("=== Solver Benchmark Results in RVF ===\n");

    let embed_dim = 64;
    let num_runs = 150;

    // ====================================================================
    // 1. Generate benchmark runs
    // ====================================================================
    println!("--- 1. Generate Benchmark Runs ---");

    let matrix_sizes: Vec<u64> = vec![64, 128, 256, 512, 1024];
    let densities: Vec<f64> = vec![0.01, 0.05, 0.10, 0.20];
    let tolerances: Vec<f64> = vec![1e-6, 1e-8, 1e-10, 1e-12];

    let mut runs: Vec<BenchmarkRun> = Vec::with_capacity(num_runs);
    let mut lcg_state: u64 = 42;

    for run_id in 0..num_runs as u64 {
        let algo = ALGORITHMS[lcg_next(&mut lcg_state) as usize % ALGORITHMS.len()];
        let prob = PROBLEM_TYPES[lcg_next(&mut lcg_state) as usize % PROBLEM_TYPES.len()];
        let size = matrix_sizes[lcg_next(&mut lcg_state) as usize % matrix_sizes.len()];
        let dens = densities[lcg_next(&mut lcg_state) as usize % densities.len()];
        let tol = tolerances[lcg_next(&mut lcg_state) as usize % tolerances.len()];

        runs.push(simulate_benchmark(run_id, algo, prob, size, dens, tol, 42));
    }

    let converged_count = runs.iter().filter(|r| r.converged).count();
    println!("  Generated {} benchmark runs", num_runs);
    println!("  Converged: {} / {}", converged_count, num_runs);

    // Algorithm distribution
    println!("\n  Algorithm distribution:");
    for &algo in ALGORITHMS {
        let count = runs.iter().filter(|r| r.algorithm == algo).count();
        println!("    {:>10}: {}", algo, count);
    }

    // Problem type distribution
    println!("\n  Problem type distribution:");
    for &prob in PROBLEM_TYPES {
        let count = runs.iter().filter(|r| r.problem_type == prob).count();
        println!("    {:>12}: {}", prob, count);
    }

    // ====================================================================
    // 2. Create RVF store and ingest benchmark data
    // ====================================================================
    println!("\n--- 2. Store Benchmarks in RVF ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("solver_benchmarks.rvf");

    let options = RvfOptions {
        dimension: embed_dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created: {} dims, L2 metric", embed_dim);

    // Build embeddings and metadata for all runs
    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(num_runs);
    let mut metadata: Vec<MetadataEntry> = Vec::new();

    // Metadata field IDs:
    //   0 = algorithm (string)
    //   1 = problem_type (string)
    //   2 = matrix_size (u64)
    //   3 = wall_time_us (u64)
    //   4 = iterations (u64)
    //   5 = converged (u64: 0 or 1)
    //   6 = convergence_rate_fixed (u64: rate * 1e6)
    //   7 = density_fixed (u64: density * 1e6)
    for run in &runs {
        embeddings.push(benchmark_to_embedding(run, embed_dim));

        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(run.algorithm.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(run.problem_type.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(run.matrix_size),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::U64(run.wall_time_us),
        });
        metadata.push(MetadataEntry {
            field_id: 4,
            value: MetadataValue::U64(run.iterations),
        });
        metadata.push(MetadataEntry {
            field_id: 5,
            value: MetadataValue::U64(if run.converged { 1 } else { 0 }),
        });
        metadata.push(MetadataEntry {
            field_id: 6,
            value: MetadataValue::U64((run.convergence_rate * 1e6) as u64),
        });
        metadata.push(MetadataEntry {
            field_id: 7,
            value: MetadataValue::U64((run.density * 1e6) as u64),
        });
    }

    let vec_refs: Vec<&[f32]> = embeddings.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_runs as u64).collect();

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!(
        "  Ingested {} benchmark records (rejected: {})",
        ingest.accepted, ingest.rejected
    );

    // ====================================================================
    // 3. Compare algorithms on the same problem class
    // ====================================================================
    println!("\n--- 3. Algorithm Comparison: Laplacian Problems ---");

    // Find benchmarks for laplacian problems
    let laplacian_runs: Vec<&BenchmarkRun> = runs
        .iter()
        .filter(|r| r.problem_type == "laplacian" && r.converged)
        .collect();

    println!("  Laplacian runs (converged): {}", laplacian_runs.len());

    // Per-algorithm statistics for laplacian problems
    println!("\n  Per-algorithm performance on laplacian problems:");
    println!(
        "    {:>10}  {:>6}  {:>12}  {:>10}  {:>10}",
        "Algorithm", "Runs", "Avg Time(us)", "Avg Iters", "Avg Rate"
    );
    println!(
        "    {:->10}  {:->6}  {:->12}  {:->10}  {:->10}",
        "", "", "", "", ""
    );

    for &algo in ALGORITHMS {
        let algo_runs: Vec<&&BenchmarkRun> = laplacian_runs
            .iter()
            .filter(|r| r.algorithm == algo)
            .collect();
        if algo_runs.is_empty() {
            continue;
        }
        let count = algo_runs.len();
        let avg_time =
            algo_runs.iter().map(|r| r.wall_time_us).sum::<u64>() as f64 / count as f64;
        let avg_iters =
            algo_runs.iter().map(|r| r.iterations).sum::<u64>() as f64 / count as f64;
        let avg_rate = algo_runs.iter().map(|r| r.convergence_rate).sum::<f64>() / count as f64;

        println!(
            "    {:>10}  {:>6}  {:>12.0}  {:>10.1}  {:>10.4}",
            algo, count, avg_time, avg_iters, avg_rate
        );
    }

    // ====================================================================
    // 4. Query: find similar benchmarks to a new problem
    // ====================================================================
    println!("\n--- 4. Find Similar Benchmarks (Nearest Neighbor) ---");

    // Simulate a new problem and find the most relevant historical benchmarks
    let new_problem = BenchmarkRun {
        run_id: 999,
        algorithm: "cg",
        problem_type: "diffusion",
        matrix_size: 256,
        density: 0.05,
        tolerance: 1e-8,
        wall_time_us: 0,
        iterations: 0,
        final_residual: 0.0,
        convergence_rate: 0.0,
        converged: false,
    };

    let query_embedding = benchmark_to_embedding(&new_problem, embed_dim);
    let k = 10;

    let results = store
        .query(&query_embedding, k, &QueryOptions::default())
        .expect("query failed");

    println!("  New problem: CG on diffusion, 256x256, density=0.05, tol=1e-8");
    println!("  Top-{} most similar historical benchmarks:", k);
    print_benchmark_results(&results, &runs);

    // ====================================================================
    // 5. Filter: best-performing algorithm for large matrices
    // ====================================================================
    println!("\n--- 5. Best Algorithm for Large Matrices (size >= 512) ---");

    // Filter for large matrices that converged
    let filter_large = FilterExpr::And(vec![
        FilterExpr::Ge(2, FilterValue::U64(512)),
        FilterExpr::Eq(5, FilterValue::U64(1)), // converged == true
    ]);
    let opts_large = QueryOptions {
        filter: Some(filter_large),
        ..Default::default()
    };

    // Query with a "fast algorithm" embedding (low iteration count features)
    let mut fast_query = vec![0.0f32; embed_dim];
    fast_query[0] = 0.5; // large matrix
    fast_query[14] = 0.1; // low iterations (we want fast)
    fast_query[16] = 1.0; // converged

    let results_large = store
        .query(&fast_query, k, &opts_large)
        .expect("query failed");

    println!("  Large matrix benchmarks (converged, size >= 512):");
    if results_large.is_empty() {
        println!("    No matching benchmarks found.");
    } else {
        print_benchmark_results(&results_large, &runs);

        // Verify all results have matrix_size >= 512 and converged
        for r in &results_large {
            let run = &runs[r.id as usize];
            assert!(
                run.matrix_size >= 512,
                "Run {} has size {} < 512",
                r.id,
                run.matrix_size
            );
            assert!(run.converged, "Run {} did not converge", r.id);
        }
        println!("  All results verified: size >= 512, converged = true.");
    }

    // ====================================================================
    // 6. Filter by specific algorithm: CG results only
    // ====================================================================
    println!("\n--- 6. CG Algorithm Results ---");

    let filter_cg = FilterExpr::Eq(0, FilterValue::String("cg".to_string()));
    let opts_cg = QueryOptions {
        filter: Some(filter_cg),
        ..Default::default()
    };

    let results_cg = store
        .query(&query_embedding, k, &opts_cg)
        .expect("query failed");

    println!("  CG benchmarks most similar to new problem:");
    print_benchmark_results(&results_cg, &runs);

    for r in &results_cg {
        let run = &runs[r.id as usize];
        assert_eq!(
            run.algorithm, "cg",
            "Run {} has algorithm {} instead of cg",
            r.id, run.algorithm
        );
    }
    println!("  All results verified: algorithm == cg.");

    // ====================================================================
    // 7. Aggregation: overall benchmark statistics
    // ====================================================================
    println!("\n--- 7. Aggregate Benchmark Statistics ---");

    let total_converged = runs.iter().filter(|r| r.converged).count();
    let total_time: u64 = runs.iter().map(|r| r.wall_time_us).sum();
    let avg_time = total_time as f64 / num_runs as f64;
    let avg_iters =
        runs.iter().map(|r| r.iterations).sum::<u64>() as f64 / num_runs as f64;
    let avg_rate =
        runs.iter().map(|r| r.convergence_rate).sum::<f64>() / num_runs as f64;

    let fastest_run = runs
        .iter()
        .filter(|r| r.converged)
        .min_by_key(|r| r.wall_time_us);
    let slowest_run = runs
        .iter()
        .filter(|r| r.converged)
        .max_by_key(|r| r.wall_time_us);

    println!("  Total runs:          {}", num_runs);
    println!("  Converged:           {} ({:.1}%)", total_converged, total_converged as f64 / num_runs as f64 * 100.0);
    println!("  Avg wall time:       {:.0} us", avg_time);
    println!("  Avg iterations:      {:.1}", avg_iters);
    println!("  Avg convergence rate: {:.4}", avg_rate);

    if let Some(fast) = fastest_run {
        println!(
            "  Fastest converged:   run {} ({} on {} {}, {} us, {} iters)",
            fast.run_id, fast.algorithm, fast.problem_type, fast.matrix_size,
            fast.wall_time_us, fast.iterations
        );
    }
    if let Some(slow) = slowest_run {
        println!(
            "  Slowest converged:   run {} ({} on {} {}, {} us, {} iters)",
            slow.run_id, slow.algorithm, slow.problem_type, slow.matrix_size,
            slow.wall_time_us, slow.iterations
        );
    }

    // Per-algorithm summary
    println!("\n  Per-algorithm convergence rates:");
    println!(
        "    {:>10}  {:>6}  {:>10}  {:>10}",
        "Algorithm", "Runs", "Conv%", "AvgRate"
    );
    println!("    {:->10}  {:->6}  {:->10}  {:->10}", "", "", "", "");
    for &algo in ALGORITHMS {
        let algo_runs: Vec<&BenchmarkRun> = runs.iter().filter(|r| r.algorithm == algo).collect();
        let count = algo_runs.len();
        if count == 0 {
            continue;
        }
        let conv = algo_runs.iter().filter(|r| r.converged).count();
        let conv_pct = conv as f64 / count as f64 * 100.0;
        let rate = algo_runs.iter().map(|r| r.convergence_rate).sum::<f64>() / count as f64;
        println!(
            "    {:>10}  {:>6}  {:>9.1}%  {:>10.4}",
            algo, count, conv_pct, rate
        );
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Solver Benchmark Summary ===\n");
    println!("  Benchmark runs:      {}", num_runs);
    println!("  Algorithms tested:   {}", ALGORITHMS.len());
    println!("  Problem types:       {}", PROBLEM_TYPES.len());
    println!("  Embedding dimension: {}", embed_dim);
    println!("  Store vectors:       {}", ingest.accepted);
    println!("  Convergence rate:    {:.1}%", total_converged as f64 / num_runs as f64 * 100.0);

    let status = store.status();
    println!("  Store file size:     {} bytes", status.file_size);
    println!("  Store epoch:         {}", status.current_epoch);

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_benchmark_results(results: &[SearchResult], runs: &[BenchmarkRun]) {
    println!(
        "    {:>4}  {:>10}  {:>10}  {:>12}  {:>6}  {:>8}  {:>10}  {:>8}",
        "ID", "Algorithm", "Problem", "Distance", "Size", "Time(us)", "Iters", "Conv"
    );
    println!(
        "    {:->4}  {:->10}  {:->10}  {:->12}  {:->6}  {:->8}  {:->10}  {:->8}",
        "", "", "", "", "", "", "", ""
    );
    for r in results {
        let run = &runs[r.id as usize];
        println!(
            "    {:>4}  {:>10}  {:>10}  {:>12.6}  {:>6}  {:>8}  {:>10}  {:>8}",
            r.id,
            run.algorithm,
            run.problem_type,
            r.distance,
            run.matrix_size,
            run.wall_time_us,
            run.iterations,
            if run.converged { "yes" } else { "no" }
        );
    }
}
