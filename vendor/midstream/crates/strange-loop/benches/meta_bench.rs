use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use midstreamer_strange_loop::*;

/// Benchmark pattern extraction performance with varying data sizes
fn pattern_extraction_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_extraction");

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            // Create realistic data with some repeated patterns
            let data: Vec<String> = (0..size)
                .map(|i| format!("pattern_{}", i % 20)) // Create 20 unique patterns with repetition
                .collect();

            b.iter(|| {
                let mut strange_loop = StrangeLoop::default();
                let result = strange_loop.learn_at_level(black_box(MetaLevel::base()), black_box(&data));
                result.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark recursive optimization with varying depths
fn recursive_optimization_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursive_optimization");

    // Test different recursion depths (1, 5, 10, 20)
    for depth in [1, 2, 3].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            let config = StrangeLoopConfig {
                max_meta_depth: depth,
                enable_self_modification: false,
                max_modifications_per_cycle: 5,
                safety_check_enabled: true,
            };

            // Generate sample data
            let data: Vec<String> = (0..100)
                .map(|i| format!("level_0_pattern_{}", i % 10))
                .collect();

            b.iter(|| {
                let mut strange_loop = StrangeLoop::new(black_box(config.clone()));

                // Learn at base level, which will trigger recursive meta-learning
                let result = strange_loop.learn_at_level(black_box(MetaLevel::base()), black_box(&data));
                result.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark self-modification overhead
fn self_modification_overhead_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("self_modification_overhead");

    for num_modifications in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(num_modifications), num_modifications, |b, &num_modifications| {
            let config = StrangeLoopConfig {
                max_meta_depth: 3,
                enable_self_modification: true,
                max_modifications_per_cycle: 100,
                safety_check_enabled: true,
            };

            b.iter(|| {
                let mut strange_loop = StrangeLoop::new(black_box(config.clone()));

                for i in 0..num_modifications {
                    let rule = ModificationRule::new(
                        format!("rule_{}", i),
                        format!("trigger_{}", i),
                        format!("action_{}", i),
                    );

                    let _ = strange_loop.apply_modification(black_box(rule));
                }

                strange_loop.get_summary()
            });
        });
    }

    group.finish();
}

/// Benchmark meta-learning convergence time with varying complexity
fn meta_learning_convergence_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_learning_convergence");

    // Test convergence with different numbers of learning iterations
    for iterations in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(iterations), iterations, |b, &iterations| {
            let config = StrangeLoopConfig {
                max_meta_depth: 2,
                enable_self_modification: false,
                max_modifications_per_cycle: 5,
                safety_check_enabled: true,
            };

            b.iter(|| {
                let mut strange_loop = StrangeLoop::new(black_box(config.clone()));

                for i in 0..iterations {
                    let data: Vec<String> = (0..50)
                        .map(|j| format!("iteration_{}_pattern_{}", i, j % 10))
                        .collect();

                    let _ = strange_loop.learn_at_level(black_box(MetaLevel::base()), black_box(&data));
                }

                strange_loop.get_summary()
            });
        });
    }

    group.finish();
}

/// Benchmark memory usage during recursion
fn memory_usage_recursion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage_recursion");

    // Test memory usage with different meta-depths and data sizes
    for (depth, data_size) in [(1, 100), (2, 100), (3, 100), (2, 500), (2, 1000)].iter() {
        let label = format!("depth_{}_size_{}", depth, data_size);
        group.bench_with_input(BenchmarkId::new("recursive_learning", &label), &(depth, data_size), |b, &(depth, data_size)| {
            let config = StrangeLoopConfig {
                max_meta_depth: *depth,
                enable_self_modification: false,
                max_modifications_per_cycle: 5,
                safety_check_enabled: true,
            };

            let data: Vec<String> = (0..*data_size)
                .map(|i| format!("pattern_{}", i % 20))
                .collect();

            b.iter(|| {
                let mut strange_loop = StrangeLoop::new(black_box(config.clone()));
                let _ = strange_loop.learn_at_level(black_box(MetaLevel::base()), black_box(&data));

                // Get all knowledge to measure memory usage
                let all_knowledge = strange_loop.get_all_knowledge();
                black_box(all_knowledge)
            });
        });
    }

    group.finish();
}

/// Benchmark strategy adaptation speed
fn strategy_adaptation_speed_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("strategy_adaptation_speed");

    // Test how quickly the system adapts to new patterns
    for pattern_change_frequency in [5, 10, 20, 50].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(pattern_change_frequency), pattern_change_frequency, |b, &pattern_change_frequency| {
            let config = StrangeLoopConfig {
                max_meta_depth: 2,
                enable_self_modification: false,
                max_modifications_per_cycle: 5,
                safety_check_enabled: true,
            };

            b.iter(|| {
                let mut strange_loop = StrangeLoop::new(black_box(config.clone()));

                // Simulate changing patterns
                for batch in 0..10 {
                    let pattern_base = batch / pattern_change_frequency;
                    let data: Vec<String> = (0..100)
                        .map(|i| format!("strategy_{}_pattern_{}", pattern_base, i % 10))
                        .collect();

                    let _ = strange_loop.learn_at_level(black_box(MetaLevel::base()), black_box(&data));
                }

                strange_loop.get_summary()
            });
        });
    }

    group.finish();
}

/// Benchmark safety constraint checking
fn safety_constraint_checking_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("safety_constraint_checking");

    for num_constraints in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(num_constraints), num_constraints, |b, &num_constraints| {
            let mut config = StrangeLoopConfig {
                max_meta_depth: 2,
                enable_self_modification: true,
                max_modifications_per_cycle: 100,
                safety_check_enabled: true,
            };

            b.iter(|| {
                let mut strange_loop = StrangeLoop::new(black_box(config.clone()));

                // Add multiple safety constraints
                for i in 0..num_constraints {
                    let constraint = SafetyConstraint::new(
                        format!("constraint_{}", i),
                        format!("G(safe_{})", i),
                    );
                    strange_loop.add_safety_constraint(black_box(constraint));
                }

                // Try to apply a modification (which triggers safety checks)
                let rule = ModificationRule::new("test_rule", "test_trigger", "test_action");
                let _ = strange_loop.apply_modification(black_box(rule));

                strange_loop.get_summary()
            });
        });
    }

    group.finish();
}

/// Benchmark knowledge retrieval performance
fn knowledge_retrieval_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("knowledge_retrieval");

    for num_patterns in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(num_patterns), num_patterns, |b, &num_patterns| {
            // Setup: create strange loop with learned knowledge
            let mut strange_loop = StrangeLoop::default();
            let data: Vec<String> = (0..num_patterns)
                .map(|i| format!("pattern_{}", i % 20))
                .collect();
            let _ = strange_loop.learn_at_level(MetaLevel::base(), &data);

            b.iter(|| {
                // Benchmark retrieval
                let knowledge = strange_loop.get_knowledge_at_level(black_box(MetaLevel::base()));
                black_box(knowledge)
            });
        });
    }

    group.finish();
}

/// Benchmark attractor analysis performance
fn attractor_analysis_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("attractor_analysis");

    for trajectory_length in [10, 50, 100, 200].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(trajectory_length), trajectory_length, |b, &trajectory_length| {
            let trajectory_data: Vec<Vec<f64>> = (0..trajectory_length)
                .map(|i| {
                    let t = i as f64 * 0.1;
                    vec![t.sin(), t.cos(), (t * 2.0).sin()] // 3D trajectory
                })
                .collect();

            b.iter(|| {
                let mut strange_loop = StrangeLoop::default();
                let result = strange_loop.analyze_behavior(black_box(trajectory_data.clone()));
                black_box(result)
            });
        });
    }

    group.finish();
}

/// Benchmark reset performance
fn reset_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("reset_performance");

    for knowledge_size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(knowledge_size), knowledge_size, |b, &knowledge_size| {
            b.iter_batched(
                || {
                    // Setup: create strange loop with lots of knowledge
                    let mut strange_loop = StrangeLoop::default();
                    let data: Vec<String> = (0..knowledge_size)
                        .map(|i| format!("pattern_{}", i % 20))
                        .collect();
                    let _ = strange_loop.learn_at_level(MetaLevel::base(), &data);
                    strange_loop
                },
                |mut strange_loop| {
                    // Benchmark reset
                    strange_loop.reset();
                    black_box(strange_loop)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    pattern_extraction_benchmark,
    recursive_optimization_benchmark,
    self_modification_overhead_benchmark,
    meta_learning_convergence_benchmark,
    memory_usage_recursion_benchmark,
    strategy_adaptation_speed_benchmark,
    safety_constraint_checking_benchmark,
    knowledge_retrieval_benchmark,
    attractor_analysis_benchmark,
    reset_benchmark,
);

criterion_main!(benches);
