//! Comprehensive benchmarks for strange-loop crate
//!
//! Benchmarks cover:
//! - Pattern extraction performance
//! - Recursive optimization depth
//! - Meta-learning iteration speed
//! - Self-modification safety checks
//! - Rollback mechanism performance
//! - Validation overhead
//!
//! Performance targets:
//! - Pattern extraction: <10ms for 1000 patterns
//! - Recursive depth: >10 levels without stack overflow
//! - Iteration speed: >1000 iterations/second
//! - Safety overhead: <5% performance impact

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstreamer_strange_loop::{
    StrangeLoop, StrangeLoopConfig, MetaLevel, MetaKnowledge,
    SafetyConstraint, ModificationRule,
};

// ============================================================================
// Test Data Generators
// ============================================================================

fn generate_pattern_data(size: usize, complexity: &str) -> Vec<String> {
    match complexity {
        "simple" => {
            // Highly repetitive patterns
            (0..size)
                .map(|i| format!("pattern{}", i % 10))
                .collect()
        }
        "medium" => {
            // Moderate repetition with variations
            (0..size)
                .map(|i| {
                    let base = i % 50;
                    let variant = i % 3;
                    format!("pattern_{}_{}", base, variant)
                })
                .collect()
        }
        "complex" => {
            // High diversity with some patterns
            (0..size)
                .map(|i| {
                    let hash = (i * 7919) % 200;
                    let subpattern = (i * 31) % 5;
                    format!("complex_{}_{}", hash, subpattern)
                })
                .collect()
        }
        "random" => {
            // Mostly unique patterns
            (0..size)
                .map(|i| {
                    let hash1 = (i * 7919) % 10000;
                    let hash2 = (i * 31337) % 10000;
                    format!("random_{}_{}", hash1, hash2)
                })
                .collect()
        }
        _ => vec!["default".to_string(); size],
    }
}

fn generate_hierarchical_data(depth: usize) -> Vec<Vec<String>> {
    let mut levels = Vec::new();
    let mut current_data = generate_pattern_data(100, "simple");

    for level in 0..depth {
        levels.push(current_data.clone());
        // Generate meta-patterns from current level
        current_data = current_data
            .windows(2)
            .map(|w| format!("meta_{}_{}", level, w.join("_")))
            .collect();
    }

    levels
}

fn generate_large_pattern_set(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| {
            let pattern_type = i % 7;
            match pattern_type {
                0 => format!("linear_{}", i),
                1 => format!("cyclic_{}", i % 100),
                2 => format!("branching_{}_{}", i / 10, i % 10),
                3 => format!("converging_{}", i / 20),
                4 => format!("diverging_{}", i),
                5 => format!("stable_{}", i % 50),
                _ => format!("chaotic_{}", (i * 7919) % 1000),
            }
        })
        .collect()
}

// ============================================================================
// Pattern Extraction Benchmarks
// ============================================================================

fn bench_pattern_extraction_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_extraction_simple");

    for size in [100, 500, 1000, 2000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("simple_patterns", size),
            size,
            |b, &n| {
                let data = generate_pattern_data(n, "simple");
                let mut strange_loop = StrangeLoop::default();

                b.iter(|| {
                    black_box(strange_loop.learn_at_level(
                        black_box(MetaLevel::base()),
                        black_box(&data)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_pattern_extraction_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_extraction_complexity");

    let size = 1000;

    for complexity in ["simple", "medium", "complex", "random"].iter() {
        group.bench_with_input(
            BenchmarkId::new("complexity", complexity),
            complexity,
            |b, &comp| {
                let data = generate_pattern_data(size, comp);
                let mut strange_loop = StrangeLoop::default();

                b.iter(|| {
                    black_box(strange_loop.learn_at_level(
                        black_box(MetaLevel::base()),
                        black_box(&data)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_pattern_extraction_edge_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_extraction_edge_cases");

    // All identical patterns
    group.bench_function("all_identical", |b| {
        let data = vec!["identical".to_string(); 1000];
        let mut strange_loop = StrangeLoop::default();

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // All unique patterns
    group.bench_function("all_unique", |b| {
        let data = generate_pattern_data(1000, "random");
        let mut strange_loop = StrangeLoop::default();

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Large pattern strings
    group.bench_function("large_strings", |b| {
        let data: Vec<String> = (0..1000)
            .map(|i| format!("large_pattern_{}_", i).repeat(10))
            .collect();
        let mut strange_loop = StrangeLoop::default();

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Recursive Optimization Depth Benchmarks
// ============================================================================

fn bench_recursive_depth_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursive_depth");

    for depth in [1, 3, 5, 10, 15].iter() {
        group.bench_with_input(
            BenchmarkId::new("depth", depth),
            depth,
            |b, &d| {
                let mut config = StrangeLoopConfig::default();
                config.max_meta_depth = d;
                let mut strange_loop = StrangeLoop::new(config);
                let data = generate_pattern_data(100, "simple");

                b.iter(|| {
                    strange_loop.reset();
                    black_box(strange_loop.learn_at_level(
                        black_box(MetaLevel::base()),
                        black_box(&data)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_recursive_depth_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursive_depth_stress");

    // Deep recursion with simple patterns
    group.bench_function("deep_simple", |b| {
        let mut config = StrangeLoopConfig::default();
        config.max_meta_depth = 10;
        let mut strange_loop = StrangeLoop::new(config);
        let data = generate_pattern_data(50, "simple");

        b.iter(|| {
            strange_loop.reset();
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Deep recursion with complex patterns
    group.bench_function("deep_complex", |b| {
        let mut config = StrangeLoopConfig::default();
        config.max_meta_depth = 10;
        let mut strange_loop = StrangeLoop::new(config);
        let data = generate_pattern_data(50, "complex");

        b.iter(|| {
            strange_loop.reset();
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Maximum safe depth
    group.bench_function("max_safe_depth", |b| {
        let mut config = StrangeLoopConfig::default();
        config.max_meta_depth = 20;
        let mut strange_loop = StrangeLoop::new(config);
        let data = generate_pattern_data(30, "simple");

        b.iter(|| {
            strange_loop.reset();
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Meta-Learning Iteration Speed Benchmarks
// ============================================================================

fn bench_meta_learning_iterations(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_learning_iterations");

    // Single iteration
    group.bench_function("single_iteration", |b| {
        let data = generate_pattern_data(100, "medium");
        let mut strange_loop = StrangeLoop::default();

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Multiple iterations (batch)
    for iterations in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*iterations as u64));

        group.bench_with_input(
            BenchmarkId::new("iterations", iterations),
            iterations,
            |b, &n| {
                let data = generate_pattern_data(100, "medium");

                b.iter(|| {
                    let mut strange_loop = StrangeLoop::default();
                    for _ in 0..n {
                        black_box(strange_loop.learn_at_level(
                            black_box(MetaLevel::base()),
                            black_box(&data)
                        )).ok();
                    }
                });
            }
        );
    }

    group.finish();
}

fn bench_iteration_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("iteration_throughput");

    // Measure iterations per second
    group.bench_function("iterations_per_second", |b| {
        let data = generate_pattern_data(100, "simple");
        let mut strange_loop = StrangeLoop::default();
        let mut count = 0u64;

        b.iter(|| {
            strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ).ok();
            count += 1;
            black_box(count)
        });
    });

    // Parallel meta-learning simulation
    group.bench_function("parallel_simulation", |b| {
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            // Simulate parallel learning at different levels
            let mut strange_loop = StrangeLoop::default();
            for level in 0..3 {
                black_box(strange_loop.learn_at_level(
                    black_box(MetaLevel(level)),
                    black_box(&data)
                )).ok();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Self-Modification Safety Check Benchmarks
// ============================================================================

fn bench_safety_check_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("safety_check_overhead");

    // Learning without safety checks
    group.bench_function("no_safety_checks", |b| {
        let mut config = StrangeLoopConfig::default();
        config.safety_check_enabled = false;
        let mut strange_loop = StrangeLoop::new(config);
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Learning with safety checks
    group.bench_function("with_safety_checks", |b| {
        let mut config = StrangeLoopConfig::default();
        config.safety_check_enabled = true;
        let mut strange_loop = StrangeLoop::new(config);
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Compare overhead percentage
    group.bench_function("safety_overhead_ratio", |b| {
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            // With safety
            let mut config_safe = StrangeLoopConfig::default();
            config_safe.safety_check_enabled = true;
            let mut safe_loop = StrangeLoop::new(config_safe);

            let safe_result = black_box(safe_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ));

            black_box(safe_result)
        });
    });

    group.finish();
}

fn bench_safety_constraint_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("safety_constraint_validation");

    // Minimal constraints
    group.bench_function("minimal_constraints", |b| {
        let mut strange_loop = StrangeLoop::default();
        let data = generate_pattern_data(100, "simple");

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Multiple constraints
    group.bench_function("multiple_constraints", |b| {
        let mut strange_loop = StrangeLoop::default();

        // Add multiple safety constraints
        for i in 0..10 {
            strange_loop.add_safety_constraint(
                SafetyConstraint::new(
                    format!("constraint_{}", i),
                    format!("G(safe_{})", i)
                )
            );
        }

        let data = generate_pattern_data(100, "simple");

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Rollback Mechanism Benchmarks
// ============================================================================

fn bench_rollback_reset(c: &mut Criterion) {
    let mut group = c.benchmark_group("rollback_mechanism");

    // Reset operation
    group.bench_function("reset_empty", |b| {
        let mut strange_loop = StrangeLoop::default();

        b.iter(|| {
            black_box(strange_loop.reset())
        });
    });

    // Reset after learning
    group.bench_function("reset_after_learning", |b| {
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            let mut strange_loop = StrangeLoop::default();
            strange_loop.learn_at_level(MetaLevel::base(), &data).ok();
            black_box(strange_loop.reset())
        });
    });

    // Reset after deep recursion
    group.bench_function("reset_deep_recursion", |b| {
        let data = generate_pattern_data(50, "simple");

        b.iter(|| {
            let mut config = StrangeLoopConfig::default();
            config.max_meta_depth = 10;
            let mut strange_loop = StrangeLoop::new(config);
            strange_loop.learn_at_level(MetaLevel::base(), &data).ok();
            black_box(strange_loop.reset())
        });
    });

    group.finish();
}

fn bench_rollback_state_recovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_recovery");

    // Save and restore simulation
    group.bench_function("save_restore_cycle", |b| {
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            let mut strange_loop = StrangeLoop::default();

            // Learn
            strange_loop.learn_at_level(MetaLevel::base(), &data).ok();

            // Get state (simulated save)
            let summary = black_box(strange_loop.get_summary());

            // Reset (simulated failure)
            strange_loop.reset();

            // Verify reset
            let new_summary = black_box(strange_loop.get_summary());

            black_box((summary, new_summary))
        });
    });

    group.finish();
}

// ============================================================================
// Validation Overhead Benchmarks
// ============================================================================

fn bench_validation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_overhead");

    // Get knowledge validation
    group.bench_function("get_knowledge_at_level", |b| {
        let mut strange_loop = StrangeLoop::default();
        let data = generate_pattern_data(1000, "medium");
        strange_loop.learn_at_level(MetaLevel::base(), &data).ok();

        b.iter(|| {
            black_box(strange_loop.get_knowledge_at_level(
                black_box(MetaLevel::base())
            ))
        });
    });

    // Get all knowledge validation
    group.bench_function("get_all_knowledge", |b| {
        let mut strange_loop = StrangeLoop::default();
        let data = generate_pattern_data(500, "medium");

        // Learn at multiple levels
        for level in 0..3 {
            strange_loop.learn_at_level(MetaLevel(level), &data).ok();
        }

        b.iter(|| {
            black_box(strange_loop.get_all_knowledge())
        });
    });

    // Summary generation
    group.bench_function("get_summary", |b| {
        let mut strange_loop = StrangeLoop::default();
        let data = generate_pattern_data(1000, "complex");
        strange_loop.learn_at_level(MetaLevel::base(), &data).ok();

        b.iter(|| {
            black_box(strange_loop.get_summary())
        });
    });

    group.finish();
}

// ============================================================================
// Complete Pipeline Benchmarks
// ============================================================================

fn bench_complete_meta_learning_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("complete_pipeline");

    group.bench_function("full_meta_learning", |b| {
        let data = generate_pattern_data(200, "medium");

        b.iter(|| {
            let mut config = StrangeLoopConfig::default();
            config.max_meta_depth = 3;
            config.safety_check_enabled = true;
            let mut strange_loop = StrangeLoop::new(config);

            // Learn at base level (triggers recursive learning)
            let result = strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            );

            // Get summary
            let summary = strange_loop.get_summary();

            // Get knowledge
            let knowledge = strange_loop.get_all_knowledge();

            black_box((result, summary, knowledge))
        });
    });

    group.bench_function("multi_level_learning", |b| {
        let data = generate_pattern_data(150, "complex");

        b.iter(|| {
            let mut strange_loop = StrangeLoop::default();

            // Learn at multiple levels explicitly
            for level in 0..3 {
                strange_loop.learn_at_level(
                    black_box(MetaLevel(level)),
                    black_box(&data)
                ).ok();
            }

            black_box(strange_loop.get_summary())
        });
    });

    group.finish();
}

// ============================================================================
// Memory Efficiency Benchmarks
// ============================================================================

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    // Large pattern sets
    group.bench_function("large_pattern_set", |b| {
        let data = generate_large_pattern_set(5000);
        let mut strange_loop = StrangeLoop::default();

        b.iter(|| {
            black_box(strange_loop.learn_at_level(
                black_box(MetaLevel::base()),
                black_box(&data)
            ))
        });
    });

    // Repeated learning cycles
    group.bench_function("repeated_cycles", |b| {
        let data = generate_pattern_data(100, "medium");

        b.iter(|| {
            let mut strange_loop = StrangeLoop::default();

            for _ in 0..10 {
                strange_loop.learn_at_level(
                    black_box(MetaLevel::base()),
                    black_box(&data)
                ).ok();
            }

            black_box(strange_loop.get_summary())
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = pattern_extraction_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets = bench_pattern_extraction_simple,
              bench_pattern_extraction_complexity,
              bench_pattern_extraction_edge_cases
}

criterion_group! {
    name = recursive_depth_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(std::time::Duration::from_secs(12));
    targets = bench_recursive_depth_scaling,
              bench_recursive_depth_stress
}

criterion_group! {
    name = iteration_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_meta_learning_iterations,
              bench_iteration_throughput
}

criterion_group! {
    name = safety_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_safety_check_overhead,
              bench_safety_constraint_validation
}

criterion_group! {
    name = rollback_benches;
    config = Criterion::default()
        .sample_size(150)
        .measurement_time(std::time::Duration::from_secs(8));
    targets = bench_rollback_reset,
              bench_rollback_state_recovery
}

criterion_group! {
    name = validation_benches;
    config = Criterion::default()
        .sample_size(150)
        .measurement_time(std::time::Duration::from_secs(8));
    targets = bench_validation_overhead
}

criterion_group! {
    name = pipeline_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(std::time::Duration::from_secs(15));
    targets = bench_complete_meta_learning_pipeline
}

criterion_group! {
    name = memory_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(std::time::Duration::from_secs(12));
    targets = bench_memory_efficiency
}

criterion_main!(
    pattern_extraction_benches,
    recursive_depth_benches,
    iteration_benches,
    safety_benches,
    rollback_benches,
    validation_benches,
    pipeline_benches,
    memory_benches
);
