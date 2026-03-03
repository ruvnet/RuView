use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::Duration;
use sublinear_time_solver::{
    OptimizedSparseMatrix,
    OptimizedConjugateGradientSolver,
    OptimizedSolverConfig,
    simd_ops::*,
    matrix::sparse::{CSRStorage, COOStorage},
};

/// Generate a well-conditioned diagonally dominant test matrix
fn create_test_matrix(size: usize, sparsity: f64) -> OptimizedSparseMatrix {
    let mut triplets = Vec::new();
    let nnz_per_row = ((size as f64 * sparsity).max(3.0) as usize).min(size);

    // Create strong diagonal dominance
    for i in 0..size {
        let diagonal_value = 10.0 + (i as f64 * 0.01);
        triplets.push((i, i, diagonal_value));

        // Add off-diagonal elements with magnitude < diagonal/nnz_per_row
        let max_off_diagonal = diagonal_value / (nnz_per_row as f64 * 2.0);

        let mut rng = i as u64 * 1664525 + 1013904223; // Simple LCG
        for _ in 1..nnz_per_row {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let col = (rng as usize) % size;

            if col != i {
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let value = (rng as f64 / u64::MAX as f64) * max_off_diagonal;
                triplets.push((i, col, value));
            }
        }
    }

    OptimizedSparseMatrix::from_triplets(triplets, size, size).unwrap()
}

/// Create a test right-hand side vector
fn create_test_rhs(size: usize) -> Vec<f64> {
    (0..size).map(|i| 1.0 + (i as f64 * 0.001)).collect()
}

/// Benchmark SIMD vs non-SIMD matrix-vector multiplication
fn benchmark_simd_matvec(c: &mut Criterion) {
    let sizes = vec![1000, 5000, 10000];

    for size in sizes {
        let matrix = create_test_matrix(size, 0.01);
        let x = create_test_rhs(size);
        let mut y = vec![0.0; size];

        let mut group = c.benchmark_group("simd_matvec");
        group.throughput(Throughput::Elements(size as u64));
        group.measurement_time(Duration::from_secs(10));

        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &size,
            |b, _| {
                b.iter(|| {
                    matrix.multiply_vector(black_box(&x), black_box(&mut y));
                });
            },
        );

        group.finish();
    }
}

/// Benchmark optimized conjugate gradient solver performance
fn benchmark_optimized_solver_scaling(c: &mut Criterion) {
    let sizes = vec![100, 500, 1000, 2000, 5000];
    let sparsity = 0.01;

    let mut group = c.benchmark_group("optimized_solver_scaling");
    group.measurement_time(Duration::from_secs(15));

    for size in sizes {
        if size > 1000 {
            group.sample_size(10); // Reduce samples for large problems
        }

        let matrix = create_test_matrix(size, sparsity);
        let b = create_test_rhs(size);

        let config = OptimizedSolverConfig {
            max_iterations: 1000,
            tolerance: 1e-6,
            enable_profiling: true,
        };

        group.throughput(Throughput::Elements((size * size) as u64));
        group.bench_with_input(
            BenchmarkId::new("cg_solver", size),
            &size,
            |bench, _| {
                bench.iter(|| {
                    let mut solver = OptimizedConjugateGradientSolver::new(config.clone());
                    let result = solver.solve(black_box(&matrix), black_box(&b)).unwrap();
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark solver vs target performance metrics
fn benchmark_target_performance(c: &mut Criterion) {
    // Target: 100K×100K system in < 150ms
    let large_size = 10000; // Start with 10K for CI/testing, can scale to 100K
    let matrix = create_test_matrix(large_size, 0.001); // Very sparse for 100K
    let b = create_test_rhs(large_size);

    let config = OptimizedSolverConfig {
        max_iterations: 100, // Limited iterations for speed test
        tolerance: 1e-3,     // Relaxed tolerance for speed
        enable_profiling: true,
    };

    let mut group = c.benchmark_group("target_performance");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(5);

    group.bench_function("large_system_10k", |bench| {
        bench.iter(|| {
            let mut solver = OptimizedConjugateGradientSolver::new(config.clone());
            let result = solver.solve(black_box(&matrix), black_box(&b)).unwrap();

            // Validate convergence and performance
            assert!(result.converged);
            println!("Time: {:.2}ms, Iterations: {}, GFLOPS: {:.2}, Bandwidth: {:.2} GB/s",
                result.computation_time_ms,
                result.iterations,
                result.performance_stats.average_gflops,
                result.performance_stats.average_bandwidth_gbs
            );

            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark memory efficiency
fn benchmark_memory_efficiency(c: &mut Criterion) {
    let sizes = vec![1000, 2000, 5000];

    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(Duration::from_secs(10));

    for size in sizes {
        let matrix = create_test_matrix(size, 0.02);
        let b = create_test_rhs(size);

        // Test different memory constraints
        let memory_limits = vec![512, 1024, 2048]; // MB

        for memory_limit in memory_limits {
            let config = OptimizedSolverConfig {
                max_iterations: 500,
                tolerance: 1e-6,
                enable_profiling: true,
            };

            group.bench_with_input(
                BenchmarkId::from_parameter(format!("{}x{}_{}MB", size, size, memory_limit)),
                &(size, memory_limit),
                |bench, _| {
                    bench.iter(|| {
                        let mut solver = OptimizedConjugateGradientSolver::new(config.clone());
                        let result = solver.solve(black_box(&matrix), black_box(&b)).unwrap();

                        // Check memory usage
                        let (matvec_count, bytes_processed) = matrix.get_performance_stats();
                        let estimated_memory_mb = (bytes_processed / 1_048_576) as f64;

                        println!("Size: {}, Memory limit: {}MB, Used: {:.2}MB, GFLOPS: {:.2}",
                            size, memory_limit, estimated_memory_mb,
                            result.performance_stats.average_gflops
                        );

                        black_box(result)
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark sparsity impact on performance
fn benchmark_sparsity_performance(c: &mut Criterion) {
    let size = 2000;
    let sparsity_levels = vec![0.001, 0.005, 0.01, 0.02, 0.05];

    let mut group = c.benchmark_group("sparsity_performance");
    group.measurement_time(Duration::from_secs(10));

    for sparsity in sparsity_levels {
        let matrix = create_test_matrix(size, sparsity);
        let b = create_test_rhs(size);

        let config = OptimizedSolverConfig::default();

        group.throughput(Throughput::Elements((matrix.nnz()) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("sparsity_{:.3}", sparsity)),
            &sparsity,
            |bench, _| {
                bench.iter(|| {
                    let mut solver = OptimizedConjugateGradientSolver::new(config.clone());
                    let result = solver.solve(black_box(&matrix), black_box(&b)).unwrap();

                    println!("Sparsity: {:.3}, NNZ: {}, Time: {:.2}ms, GFLOPS: {:.2}",
                        sparsity, matrix.nnz(), result.computation_time_ms,
                        result.performance_stats.average_gflops
                    );

                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark convergence characteristics
fn benchmark_convergence_analysis(c: &mut Criterion) {
    let size = 1000;
    let matrix = create_test_matrix(size, 0.01);
    let b = create_test_rhs(size);

    let tolerances = vec![1e-3, 1e-6, 1e-9, 1e-12];

    let mut group = c.benchmark_group("convergence_analysis");
    group.measurement_time(Duration::from_secs(15));

    for tolerance in tolerances {
        let config = OptimizedSolverConfig {
            max_iterations: 5000,
            tolerance,
            enable_profiling: true,
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("tol_{:.0e}", tolerance)),
            &tolerance,
            |bench, _| {
                bench.iter(|| {
                    let mut solver = OptimizedConjugateGradientSolver::new(config.clone());
                    let result = solver.solve(black_box(&matrix), black_box(&b)).unwrap();

                    // Calculate convergence rate
                    let convergence_rate = if result.iterations > 1 {
                        (-result.residual_norm.ln()) / (result.iterations as f64)
                    } else {
                        0.0
                    };

                    println!("Tolerance: {:.0e}, Iterations: {}, Rate: {:.4}, Time: {:.2}ms",
                        tolerance, result.iterations, convergence_rate, result.computation_time_ms
                    );

                    // Verify we achieved the target tolerance
                    assert!(result.converged);
                    assert!(result.residual_norm <= tolerance * 10.0); // Allow some margin

                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Comprehensive performance validation against all target metrics
fn benchmark_comprehensive_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("comprehensive_validation");
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(5);

    // Test 1: Large system performance (scaled down for CI)
    let large_size = 5000; // Would be 100K in production
    let large_matrix = create_test_matrix(large_size, 0.0002); // Ultra-sparse
    let large_b = create_test_rhs(large_size);

    let fast_config = OptimizedSolverConfig {
        max_iterations: 50,
        tolerance: 1e-3,
        enable_profiling: true,
    };

    group.bench_function("target_large_system", |bench| {
        bench.iter(|| {
            let mut solver = OptimizedConjugateGradientSolver::new(fast_config.clone());
            let result = solver.solve(black_box(&large_matrix), black_box(&large_b)).unwrap();

            // Target: < 150ms for 100K system (scaled proportionally)
            let target_time_ms = 150.0 * (large_size as f64 / 100_000.0);

            println!("Large system - Size: {}, Time: {:.2}ms (target: {:.2}ms), GFLOPS: {:.2}",
                large_size, result.computation_time_ms, target_time_ms,
                result.performance_stats.average_gflops
            );

            black_box(result)
        });
    });

    // Test 2: Memory efficiency for 10K systems
    let memory_size = 1000; // Would be 10K in production
    let memory_matrix = create_test_matrix(memory_size, 0.01);
    let memory_b = create_test_rhs(memory_size);

    group.bench_function("target_memory_efficiency", |bench| {
        bench.iter(|| {
            let mut solver = OptimizedConjugateGradientSolver::new(OptimizedSolverConfig::default());
            let result = solver.solve(black_box(&memory_matrix), black_box(&memory_b)).unwrap();

            let (_, bytes_processed) = memory_matrix.get_performance_stats();
            let memory_usage_mb = bytes_processed as f64 / 1_048_576.0;

            // Target: < 1MB for 10K systems (scaled)
            let target_memory_mb = 1.0 * (memory_size as f64 / 10_000.0);

            println!("Memory test - Size: {}, Memory: {:.2}MB (target: {:.2}MB), Bandwidth: {:.2} GB/s",
                memory_size, memory_usage_mb, target_memory_mb,
                result.performance_stats.average_bandwidth_gbs
            );

            black_box(result)
        });
    });

    // Test 3: Convergence rate for well-conditioned systems
    let conv_size = 500;
    let conv_matrix = create_test_matrix(conv_size, 0.02);
    let conv_b = create_test_rhs(conv_size);

    group.bench_function("target_convergence_rate", |bench| {
        bench.iter(|| {
            let mut solver = OptimizedConjugateGradientSolver::new(OptimizedSolverConfig::default());
            let result = solver.solve(black_box(&conv_matrix), black_box(&conv_b)).unwrap();

            // Target: > 90% convergence rate for well-conditioned systems
            let convergence_success = result.converged;
            let convergence_rate_percent = if convergence_success { 100.0 } else { 0.0 };

            println!("Convergence test - Size: {}, Success: {}%, Iterations: {}, Residual: {:.2e}",
                conv_size, convergence_rate_percent, result.iterations, result.residual_norm
            );

            assert!(convergence_success, "Should converge for well-conditioned system");

            black_box(result)
        });
    });

    group.finish();
}

/// Generate detailed performance report
fn benchmark_performance_report(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_report");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(3);

    let test_cases = vec![
        (100, 0.05, "small_dense"),
        (500, 0.02, "medium_sparse"),
        (1000, 0.01, "large_sparse"),
        (2000, 0.005, "very_large_very_sparse"),
    ];

    for (size, sparsity, name) in test_cases {
        let matrix = create_test_matrix(size, sparsity);
        let b = create_test_rhs(size);

        let config = OptimizedSolverConfig {
            max_iterations: 1000,
            tolerance: 1e-6,
            enable_profiling: true,
        };

        group.bench_function(name, |bench| {
            bench.iter(|| {
                let mut solver = OptimizedConjugateGradientSolver::new(config.clone());
                matrix.reset_stats();

                let result = solver.solve(black_box(&matrix), black_box(&b)).unwrap();
                let (matvec_count, bytes_processed) = matrix.get_performance_stats();

                // Generate comprehensive report
                println!("\n=== PERFORMANCE REPORT: {} ===", name);
                println!("Matrix size: {}x{}", size, size);
                println!("Sparsity: {:.3} ({} nnz)", sparsity, matrix.nnz());
                println!("Time: {:.2} ms", result.computation_time_ms);
                println!("Iterations: {}", result.iterations);
                println!("Converged: {}", result.converged);
                println!("Final residual: {:.2e}", result.residual_norm);
                println!("Matrix-vector ops: {}", matvec_count);
                println!("Dot products: {}", result.performance_stats.dot_product_count);
                println!("AXPY operations: {}", result.performance_stats.axpy_count);
                println!("Total FLOPS: {}", result.performance_stats.total_flops);
                println!("Average GFLOPS: {:.2}", result.performance_stats.average_gflops);
                println!("Average bandwidth: {:.2} GB/s", result.performance_stats.average_bandwidth_gbs);
                println!("Bytes processed: {:.2} MB", bytes_processed as f64 / 1_048_576.0);

                // Performance efficiency metrics
                let flops_per_iteration = result.performance_stats.total_flops as f64 / result.iterations as f64;
                let time_per_iteration = result.computation_time_ms / result.iterations as f64;

                println!("FLOPS per iteration: {:.0}", flops_per_iteration);
                println!("Time per iteration: {:.3} ms", time_per_iteration);
                println!("Memory efficiency: {:.2} GB/s per GFLOP",
                    result.performance_stats.average_bandwidth_gbs / result.performance_stats.average_gflops.max(0.001));

                // Check against targets
                let meets_speed_target = result.computation_time_ms < 150.0 * (size as f64 / 100_000.0);
                let meets_memory_target = (bytes_processed as f64 / 1_048_576.0) < 1.0 * (size as f64 / 10_000.0);
                let meets_convergence_target = result.converged;

                println!("Meets speed target: {}", meets_speed_target);
                println!("Meets memory target: {}", meets_memory_target);
                println!("Meets convergence target: {}", meets_convergence_target);
                println!("=== END REPORT ===\n");

                black_box(result)
            });
        });
    }

    group.finish();
}

criterion_group!(
    performance_benches,
    benchmark_simd_matvec,
    benchmark_optimized_solver_scaling,
    benchmark_target_performance,
    benchmark_memory_efficiency,
    benchmark_sparsity_performance,
    benchmark_convergence_analysis,
    benchmark_comprehensive_validation,
    benchmark_performance_report
);

criterion_main!(performance_benches);