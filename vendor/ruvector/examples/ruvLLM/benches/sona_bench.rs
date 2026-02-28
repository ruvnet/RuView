//! SONA (Self-Optimizing Neural Architecture) Performance Benchmarks
//!
//! Comprehensive benchmarks for all SONA components:
//! - MicroLoRA forward pass (target: <100μs)
//! - Trajectory recording (target: <1μs per step)
//! - ReasoningBank pattern extraction
//! - InstantLoop full cycle (target: <1ms)
//! - EWC++ loss computation

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvllm::sona::*;

// ============================================================================
// MicroLoRA Benchmarks
// ============================================================================

fn micro_lora_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_lora");

    // Test different hidden dimensions
    for dim in [128, 256, 512] {
        group.throughput(Throughput::Elements(dim as u64));

        // Rank 1 benchmarks
        group.bench_with_input(BenchmarkId::new("forward_rank1", dim), &dim, |b, &dim| {
            let lora = MicroLoRA::new(dim, 1);
            let input = vec![1.0f32; dim];
            let mut output = vec![0.0f32; dim];

            b.iter(|| {
                lora.forward(black_box(&input), black_box(&mut output));
            });
        });

        // Rank 2 benchmarks
        group.bench_with_input(BenchmarkId::new("forward_rank2", dim), &dim, |b, &dim| {
            let lora = MicroLoRA::new(dim, 2);
            let input = vec![1.0f32; dim];
            let mut output = vec![0.0f32; dim];

            b.iter(|| {
                lora.forward(black_box(&input), black_box(&mut output));
            });
        });

        // Scalar (non-SIMD) forward pass for comparison
        group.bench_with_input(BenchmarkId::new("forward_scalar", dim), &dim, |b, &dim| {
            let lora = MicroLoRA::new(dim, 1);
            let input = vec![1.0f32; dim];
            let mut output = vec![0.0f32; dim];

            b.iter(|| {
                lora.forward_scalar(black_box(&input), black_box(&mut output));
            });
        });

        // Gradient accumulation
        group.bench_with_input(
            BenchmarkId::new("accumulate_gradient", dim),
            &dim,
            |b, &dim| {
                let mut lora = MicroLoRA::new(dim, 1);
                let signal = LearningSignal::with_gradient(vec![0.5; dim], vec![0.1; dim], 0.8);

                b.iter(|| {
                    lora.accumulate_gradient(black_box(&signal));
                });
            },
        );

        // Apply accumulated gradients
        group.bench_with_input(
            BenchmarkId::new("apply_accumulated", dim),
            &dim,
            |b, &dim| {
                let mut lora = MicroLoRA::new(dim, 1);

                // Pre-accumulate some gradients
                let signal = LearningSignal::with_gradient(vec![0.5; dim], vec![0.1; dim], 0.8);
                for _ in 0..10 {
                    lora.accumulate_gradient(&signal);
                }

                b.iter(|| {
                    lora.apply_accumulated(black_box(0.001));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Trajectory Recording Benchmarks
// ============================================================================

fn trajectory_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory");

    // Single step recording
    group.bench_function("record_step", |b| {
        let buffer = TrajectoryBuffer::new(10000);
        let id_gen = TrajectoryIdGen::new();

        b.iter(|| {
            let trajectory = QueryTrajectory::new(id_gen.next(), vec![0.1, 0.2, 0.3, 0.4]);
            buffer.record(black_box(trajectory));
        });
    });

    // Builder - complete trajectory construction
    for steps in [5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("build_trajectory", steps),
            &steps,
            |b, &steps| {
                b.iter(|| {
                    let mut builder = TrajectoryBuilder::new(1, vec![0.1, 0.2, 0.3, 0.4]);

                    for i in 0..steps {
                        builder.add_step(vec![0.5; 128], vec![0.3; 64], 0.7);
                    }

                    black_box(builder.build(0.85));
                });
            },
        );
    }

    // Drain operations
    group.bench_function("drain_all", |b| {
        let buffer = TrajectoryBuffer::new(10000);

        // Pre-fill buffer
        for i in 0..1000 {
            buffer.record(QueryTrajectory::new(i, vec![0.1, 0.2]));
        }

        b.iter(|| {
            let drained = buffer.drain();
            black_box(drained);

            // Refill for next iteration
            for i in 0..1000 {
                buffer.record(QueryTrajectory::new(i, vec![0.1, 0.2]));
            }
        });
    });

    group.bench_function("drain_batch_100", |b| {
        let buffer = TrajectoryBuffer::new(10000);

        // Pre-fill buffer
        for i in 0..1000 {
            buffer.record(QueryTrajectory::new(i, vec![0.1, 0.2]));
        }

        b.iter(|| {
            let drained = buffer.drain_n(100);
            black_box(drained);

            // Refill what we drained
            for i in 0..100 {
                buffer.record(QueryTrajectory::new(i, vec![0.1, 0.2]));
            }
        });
    });

    group.finish();
}

// ============================================================================
// ReasoningBank Benchmarks
// ============================================================================

fn reasoning_bank_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("reasoning_bank");

    // Pattern extraction with K-means++
    for trajectory_count in [100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("extract_patterns", trajectory_count),
            &trajectory_count,
            |b, &count| {
                let config = PatternConfig {
                    k_clusters: 10,
                    embedding_dim: 128,
                    max_iterations: 50,
                    min_cluster_size: 3,
                    quality_threshold: 0.5,
                    ..Default::default()
                };

                let mut bank = ReasoningBank::new(config);

                // Add trajectories
                for i in 0..count {
                    let mut trajectory = QueryTrajectory::new(
                        i,
                        vec![
                            (i as f32 * 0.1) % 1.0,
                            (i as f32 * 0.2) % 1.0,
                            (i as f32 * 0.3) % 1.0,
                        ],
                    );
                    trajectory.finalize(0.7 + (i as f32 * 0.001) % 0.3, 1000);
                    bank.add_trajectory(&trajectory);
                }

                b.iter(|| {
                    let patterns = bank.extract_patterns();
                    black_box(patterns);
                });
            },
        );
    }

    // Query similar patterns
    group.bench_function("query_patterns", |b| {
        let config = PatternConfig {
            k_clusters: 20,
            embedding_dim: 128,
            min_cluster_size: 3,
            quality_threshold: 0.5,
            ..Default::default()
        };

        let mut bank = ReasoningBank::new(config);

        // Build up pattern database
        for i in 0..1000 {
            let mut trajectory = QueryTrajectory::new(i, vec![(i as f32 * 0.1) % 1.0; 128]);
            trajectory.finalize(0.8, 1000);
            bank.add_trajectory(&trajectory);
        }
        bank.extract_patterns();

        let query = vec![0.5; 128];

        b.iter(|| {
            let similar = bank.find_similar(black_box(&query), 5);
            black_box(similar);
        });
    });

    // Pattern consolidation
    group.bench_function("consolidate_patterns", |b| {
        let config = PatternConfig {
            k_clusters: 30,
            embedding_dim: 128,
            min_cluster_size: 2,
            quality_threshold: 0.4,
            ..Default::default()
        };

        let mut bank = ReasoningBank::new(config);

        // Create many similar patterns
        for i in 0..500 {
            let mut trajectory = QueryTrajectory::new(i, vec![1.0 + (i as f32 * 0.001); 128]);
            trajectory.finalize(0.8, 1000);
            bank.add_trajectory(&trajectory);
        }
        bank.extract_patterns();

        b.iter(|| {
            let mut bank_clone = bank.clone();
            bank_clone.consolidate(black_box(0.95));
        });
    });

    group.finish();
}

// ============================================================================
// EWC++ Benchmarks
// ============================================================================

fn ewc_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("ewc_plus_plus");

    // Fisher information update
    for param_count in [256, 512, 1024] {
        group.bench_with_input(
            BenchmarkId::new("update_fisher", param_count),
            &param_count,
            |b, &count| {
                let config = EwcConfig {
                    param_count: count,
                    ..Default::default()
                };
                let mut ewc = EwcPlusPlus::new(config);
                let gradients = vec![0.1; count];

                b.iter(|| {
                    ewc.update_fisher(black_box(&gradients));
                });
            },
        );
    }

    // Task boundary detection
    group.bench_function("detect_boundary", |b| {
        let config = EwcConfig {
            param_count: 512,
            gradient_history_size: 100,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Build up history
        for _ in 0..100 {
            ewc.update_fisher(&vec![0.1; 512]);
        }

        let test_gradients = vec![0.15; 512];

        b.iter(|| {
            let is_boundary = ewc.detect_task_boundary(black_box(&test_gradients));
            black_box(is_boundary);
        });
    });

    // Apply constraints
    for task_count in [1, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("apply_constraints", task_count),
            &task_count,
            |b, &tasks| {
                let config = EwcConfig {
                    param_count: 512,
                    max_tasks: tasks,
                    ..Default::default()
                };
                let mut ewc = EwcPlusPlus::new(config);

                // Create multiple tasks
                for _ in 0..tasks {
                    for _ in 0..50 {
                        ewc.update_fisher(&vec![0.1; 512]);
                    }
                    ewc.start_new_task();
                }

                let gradients = vec![0.5; 512];

                b.iter(|| {
                    let constrained = ewc.apply_constraints(black_box(&gradients));
                    black_box(constrained);
                });
            },
        );
    }

    // Regularization loss computation
    group.bench_function("regularization_loss", |b| {
        let config = EwcConfig {
            param_count: 512,
            max_tasks: 5,
            initial_lambda: 1000.0,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Create tasks
        for _ in 0..5 {
            ewc.set_optimal_weights(&vec![0.0; 512]);
            for _ in 0..50 {
                ewc.update_fisher(&vec![0.1; 512]);
            }
            ewc.start_new_task();
        }

        let current_weights = vec![0.1; 512];

        b.iter(|| {
            let loss = ewc.regularization_loss(black_box(&current_weights));
            black_box(loss);
        });
    });

    // Task consolidation
    group.bench_function("consolidate_tasks", |b| {
        let config = EwcConfig {
            param_count: 512,
            max_tasks: 10,
            ..Default::default()
        };

        b.iter(|| {
            let mut ewc = EwcPlusPlus::new(config.clone());

            // Create 10 tasks
            for _ in 0..10 {
                for _ in 0..20 {
                    ewc.update_fisher(&vec![0.1; 512]);
                }
                ewc.start_new_task();
            }

            ewc.consolidate_all_tasks();
            black_box(ewc.task_count());
        });
    });

    group.finish();
}

// ============================================================================
// Integrated Benchmarks (Complete SONA Cycles)
// ============================================================================

fn integrated_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated");

    // Complete instant learning cycle
    group.bench_function("instant_loop_full_cycle", |b| {
        let dim = 256;
        let mut lora = MicroLoRA::new(dim, 1);
        let buffer = TrajectoryBuffer::new(1000);
        let id_gen = TrajectoryIdGen::new();

        b.iter(|| {
            // 1. Record trajectory (simulate 10 steps)
            let mut builder = TrajectoryBuilder::new(id_gen.next(), vec![0.5; dim]);

            for i in 0..10 {
                builder.add_step(vec![0.3; dim], vec![0.2; 128], 0.7 + (i as f32 * 0.02));
            }

            let trajectory = builder.build(0.85);

            // 2. Convert to learning signal
            let signal = LearningSignal::from_trajectory(&trajectory);

            // 3. Accumulate gradient
            lora.accumulate_gradient(&signal);

            // 4. Apply if batch ready (every 10 iterations in real use)
            if lora.pending_updates() >= 10 {
                lora.apply_accumulated(0.001);
            }

            // 5. Store trajectory
            buffer.record(black_box(trajectory));
        });
    });

    // Pattern-based learning cycle
    group.bench_function("pattern_learning_cycle", |b| {
        let config = PatternConfig {
            k_clusters: 10,
            embedding_dim: 128,
            min_cluster_size: 3,
            quality_threshold: 0.6,
            ..Default::default()
        };
        let mut bank = ReasoningBank::new(config);

        // Pre-populate with some trajectories
        for i in 0..100 {
            let mut trajectory = QueryTrajectory::new(i, vec![0.5; 128]);
            trajectory.finalize(0.8, 1000);
            bank.add_trajectory(&trajectory);
        }

        b.iter(|| {
            // 1. Add new trajectory
            let mut trajectory = QueryTrajectory::new(1000, vec![0.6; 128]);
            trajectory.finalize(0.85, 1000);
            bank.add_trajectory(&trajectory);

            // 2. Extract patterns (would be done periodically)
            if bank.trajectory_count() % 50 == 0 {
                let patterns = bank.extract_patterns();
                black_box(patterns);
            }

            // 3. Query similar patterns
            let query = vec![0.6; 128];
            let similar = bank.find_similar(&query, 3);
            black_box(similar);
        });
    });

    // EWC-protected learning
    group.bench_function("ewc_protected_learning", |b| {
        let param_count = 512;
        let config = EwcConfig {
            param_count,
            max_tasks: 5,
            initial_lambda: 1000.0,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Setup with one completed task
        ewc.set_optimal_weights(&vec![0.0; param_count]);
        for _ in 0..50 {
            ewc.update_fisher(&vec![0.1; param_count]);
        }
        ewc.start_new_task();

        let mut lora = MicroLoRA::new(param_count, 1);

        b.iter(|| {
            // 1. Get raw gradients from learning signal
            let signal =
                LearningSignal::with_gradient(vec![0.5; param_count], vec![0.1; param_count], 0.8);

            // 2. Apply EWC constraints
            let constrained = ewc.apply_constraints(&signal.gradient_estimate);

            // 3. Create constrained signal
            let constrained_signal = LearningSignal::with_gradient(
                signal.query_embedding.clone(),
                constrained,
                signal.quality_score,
            );

            // 4. Apply to LoRA
            lora.accumulate_gradient(&constrained_signal);

            // 5. Update Fisher
            ewc.update_fisher(&signal.gradient_estimate);
        });
    });

    group.finish();
}

// ============================================================================
// Learning Signal Benchmarks
// ============================================================================

fn learning_signal_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("learning_signal");

    // Gradient estimation from trajectory
    for step_count in [5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("from_trajectory", step_count),
            &step_count,
            |b, &steps| {
                let mut trajectory = QueryTrajectory::new(1, vec![0.5; 256]);

                for i in 0..steps {
                    trajectory.add_step(TrajectoryStep::new(
                        vec![0.3; 256],
                        vec![0.2; 128],
                        0.7 + (i as f32 * 0.02),
                        i,
                    ));
                }
                trajectory.finalize(0.85, 1000);

                b.iter(|| {
                    let signal = LearningSignal::from_trajectory(black_box(&trajectory));
                    black_box(signal);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    micro_lora_benchmarks,
    trajectory_benchmarks,
    reasoning_bank_benchmarks,
    ewc_benchmarks,
    integrated_benchmarks,
    learning_signal_benchmarks,
);

criterion_main!(benches);
