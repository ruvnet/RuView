use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use sublinear_time_solver::core::{SparseMatrix, Vector};
use sublinear_time_solver::solver::hybrid::{HybridSolver, HybridConfig};
use sublinear_time_solver::solver::random_walk::{RandomWalkEngine, RandomWalkConfig, BidirectionalWalk, VarianceReduction};
use sublinear_time_solver::solver::sampling::{AdaptiveSampler, SamplingConfig, SamplingStrategy, MultiLevelSampler};
use sublinear_time_solver::algorithms::Algorithm;

/// Create a test sparse matrix with given size and sparsity
fn create_benchmark_matrix(n: usize, sparsity: f64) -> SparseMatrix {
    let mut matrix = SparseMatrix::new(n, n);
    let num_nonzeros = (n * n) as f64 * sparsity;

    // Create a symmetric positive definite matrix
    for i in 0..n {
        matrix.insert(i, i, 5.0 + i as f64 * 0.01); // Strong diagonal dominance
    }

    let mut rng = 12345u64; // Simple LCG for reproducible results
    let mut added_elements = 0;

    while added_elements < num_nonzeros as usize - n {
        // Simple LCG
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let i = (rng as usize) % n;

        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let j = (rng as usize) % n;

        if i != j && !matrix.get_row(i).contains_key(&j) {
            let value = (rng as f64 / u64::MAX as f64) * 0.5 - 0.25; // [-0.25, 0.25]
            matrix.insert(i, j, value);
            matrix.insert(j, i, value); // Symmetric
            added_elements += 2;
        }
    }

    matrix
}

/// Create a test vector
fn create_benchmark_vector(n: usize) -> Vector {
    (0..n).map(|i| 1.0 + (i as f64) * 0.1).collect()
}

/// Benchmark random walk solver with different configurations
fn benchmark_random_walk_solver(c: &mut Criterion) {
    let mut group = c.benchmark_group("random_walk_solver");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    let sizes = vec![50, 100, 200];
    let variance_methods = vec![
        VarianceReduction::None,
        VarianceReduction::Antithetic,
    ];

    for size in sizes {
        let matrix = create_benchmark_matrix(size, 0.1);
        let b = create_benchmark_vector(size);

        for method in &variance_methods {
            let config = RandomWalkConfig {
                max_steps: 5000,
                variance_reduction: method.clone(),
                convergence_tolerance: 1e-6,
                seed: Some(42),
                ..Default::default()
            };

            let benchmark_id = BenchmarkId::from_parameter(format!("size_{}_method_{:?}", size, method));

            group.bench_with_input(benchmark_id, &size, |b_bench, _| {
                b_bench.iter(|| {
                    let mut engine = RandomWalkEngine::new(config.clone());
                    black_box(engine.solve_linear_system(&matrix, &b).unwrap())
                });
            });
        }
    }

    group.finish();
}

/// Benchmark bidirectional random walk
fn benchmark_bidirectional_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("bidirectional_walk");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    let sizes = vec![30, 60, 120];

    for size in sizes {
        let matrix = create_benchmark_matrix(size, 0.15);
        let b = create_benchmark_vector(size);

        let config = RandomWalkConfig {
            max_steps: 3000,
            variance_reduction: VarianceReduction::Antithetic,
            seed: Some(42),
            ..Default::default()
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b_bench, _| {
            b_bench.iter(|| {
                let mut solver = BidirectionalWalk::new(config.clone());
                black_box(solver.solve_linear_system(&matrix, &b).unwrap())
            });
        });
    }

    group.finish();
}

/// Benchmark adaptive sampling strategies
fn benchmark_adaptive_sampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_sampling");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(15);

    let strategies = vec![
        SamplingStrategy::Uniform,
        SamplingStrategy::ImportanceSampling,
        SamplingStrategy::AdaptiveSampling,
        SamplingStrategy::QuasiMonteCarlo,
    ];

    let domain_size = 1000;
    let target_function = |x: usize| (x as f64 / 100.0).sin().abs() + 0.5;

    for strategy in strategies {
        let config = SamplingConfig {
            strategy: strategy.clone(),
            sample_size: 500,
            seed: Some(42),
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", strategy)),
            &strategy,
            |b_bench, _| {
                b_bench.iter(|| {
                    let mut sampler = AdaptiveSampler::new(config.clone());
                    black_box(sampler.generate_samples(domain_size, &target_function).unwrap())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark multi-level sampling
fn benchmark_multilevel_sampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("multilevel_sampling");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    let num_levels_vec = vec![2, 3, 4];

    for num_levels in num_levels_vec {
        let base_config = SamplingConfig {
            sample_size: 1000,
            seed: Some(42),
            ..Default::default()
        };

        let domain_sizes: Vec<usize> = (0..num_levels).map(|i| 1000 / (2_usize.pow(i as u32))).collect();
        let target_functions: Vec<Box<dyn Fn(usize) -> f64>> = (0..num_levels)
            .map(|level| {
                Box::new(move |x: usize| {
                    (x as f64 / (10.0 * (level + 1) as f64)).sin().abs() + 0.1
                }) as Box<dyn Fn(usize) -> f64>
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_levels),
            &num_levels,
            |b_bench, _| {
                b_bench.iter(|| {
                    let mut ml_sampler = MultiLevelSampler::new(num_levels, base_config.clone());
                    let fn_refs: Vec<&dyn Fn(usize) -> f64> = target_functions.iter().map(|f| f.as_ref()).collect();
                    black_box(ml_sampler.generate_multilevel_samples(&domain_sizes, &fn_refs).unwrap())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark hybrid solver with different configurations
fn benchmark_hybrid_solver(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_solver");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(8);

    let sizes = vec![40, 80, 160];
    let configs = vec![
        ("deterministic_only", HybridConfig {
            use_deterministic: true,
            use_random_walk: false,
            use_bidirectional: false,
            use_multilevel: false,
            max_iterations: 1000,
            convergence_tolerance: 1e-6,
            parallel_execution: false,
            ..Default::default()
        }),
        ("random_walk_only", HybridConfig {
            use_deterministic: false,
            use_random_walk: true,
            use_bidirectional: false,
            use_multilevel: false,
            max_iterations: 500,
            convergence_tolerance: 1e-5,
            parallel_execution: false,
            random_walk_config: RandomWalkConfig {
                max_steps: 3000,
                variance_reduction: VarianceReduction::Antithetic,
                seed: Some(42),
                ..Default::default()
            },
            ..Default::default()
        }),
        ("hybrid_approach", HybridConfig {
            use_deterministic: true,
            use_random_walk: true,
            use_bidirectional: true,
            use_multilevel: false,
            deterministic_weight: 0.6,
            max_iterations: 300,
            convergence_tolerance: 1e-6,
            parallel_execution: false,
            adaptation_interval: 50,
            ..Default::default()
        }),
    ];

    for size in sizes {
        let matrix = create_benchmark_matrix(size, 0.08);
        let b = create_benchmark_vector(size);

        for (config_name, config) in &configs {
            let benchmark_id = BenchmarkId::from_parameter(format!("size_{}_config_{}", size, config_name));

            group.bench_with_input(benchmark_id, &size, |b_bench, _| {
                b_bench.iter(|| {
                    let mut solver = HybridSolver::new(config.clone());
                    black_box(solver.solve_linear_system(&matrix, &b).unwrap())
                });
            });
        }
    }

    group.finish();
}

/// Benchmark parallel vs sequential execution
fn benchmark_parallel_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_vs_sequential");
    group.measurement_time(Duration::from_secs(25));
    group.sample_size(5);

    let size = 100;
    let matrix = create_benchmark_matrix(size, 0.1);
    let b = create_benchmark_vector(size);

    let configs = vec![
        ("sequential", HybridConfig {
            parallel_execution: false,
            max_iterations: 200,
            convergence_tolerance: 1e-5,
            ..Default::default()
        }),
        ("parallel", HybridConfig {
            parallel_execution: true,
            max_iterations: 200,
            convergence_tolerance: 1e-5,
            ..Default::default()
        }),
    ];

    for (mode, config) in configs {
        group.bench_with_input(
            BenchmarkId::from_parameter(mode),
            &mode,
            |b_bench, _| {
                b_bench.iter(|| {
                    let mut solver = HybridSolver::new(config.clone());
                    black_box(solver.solve_linear_system(&matrix, &b).unwrap())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark convergence speed with different tolerances
fn benchmark_convergence_tolerance(c: &mut Criterion) {
    let mut group = c.benchmark_group("convergence_tolerance");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    let size = 60;
    let matrix = create_benchmark_matrix(size, 0.12);
    let b = create_benchmark_vector(size);

    let tolerances = vec![1e-4, 1e-6, 1e-8];

    for tolerance in tolerances {
        let config = HybridConfig {
            convergence_tolerance: tolerance,
            max_iterations: 1000,
            parallel_execution: false,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:.0e}", tolerance)),
            &tolerance,
            |b_bench, _| {
                b_bench.iter(|| {
                    let mut solver = HybridSolver::new(config.clone());
                    black_box(solver.solve_linear_system(&matrix, &b).unwrap())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage and performance correlation
fn benchmark_memory_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_performance");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(8);

    let size = 80;
    let matrix = create_benchmark_matrix(size, 0.1);
    let b = create_benchmark_vector(size);

    let memory_limits = vec![512, 1024, 2048]; // MB

    for memory_limit in memory_limits {
        let config = HybridConfig {
            memory_limit,
            max_iterations: 500,
            convergence_tolerance: 1e-6,
            parallel_execution: false,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}MB", memory_limit)),
            &memory_limit,
            |b_bench, _| {
                b_bench.iter(|| {
                    let mut solver = HybridSolver::new(config.clone());
                    let solution = solver.solve_linear_system(&matrix, &b).unwrap();
                    let metrics = solver.get_metrics();
                    black_box((solution, metrics.memory_usage))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark sparsity effects on performance
fn benchmark_sparsity_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparsity_effects");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    let size = 100;
    let sparsity_levels = vec![0.05, 0.1, 0.2, 0.3];

    for sparsity in sparsity_levels {
        let matrix = create_benchmark_matrix(size, sparsity);
        let b = create_benchmark_vector(size);

        let config = HybridConfig {
            max_iterations: 300,
            convergence_tolerance: 1e-6,
            parallel_execution: false,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("sparsity_{:.2}", sparsity)),
            &sparsity,
            |b_bench, _| {
                b_bench.iter(|| {
                    let mut solver = HybridSolver::new(config.clone());
                    black_box(solver.solve_linear_system(&matrix, &b).unwrap())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    solver_benches,
    benchmark_random_walk_solver,
    benchmark_bidirectional_walk,
    benchmark_adaptive_sampling,
    benchmark_multilevel_sampling,
    benchmark_hybrid_solver,
    benchmark_parallel_vs_sequential,
    benchmark_convergence_tolerance,
    benchmark_memory_performance,
    benchmark_sparsity_effects
);

criterion_main!(solver_benches);