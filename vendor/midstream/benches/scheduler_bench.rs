//! Comprehensive benchmarks for nanosecond-scheduler crate
//!
//! Benchmarks cover:
//! - Schedule overhead (target: <100ns)
//! - Task execution latency
//! - Priority queue operations
//! - Statistics calculation overhead
//! - Multi-threaded scheduling
//! - Batch operations
//!
//! Performance targets:
//! - Schedule overhead: <100ns
//! - Task execution: <1μs
//! - Stats calculation: <10μs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstreamer_scheduler::{
    NanoScheduler, Task, TaskPriority, ScheduleResult,
    stats::SchedulerStats,
};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// Test Task Generators
// ============================================================================

fn create_simple_task(id: u64) -> Task {
    Task::new(
        format!("task_{}", id),
        Box::new(move || {
            // Minimal work
            black_box(id * 2);
        }),
        TaskPriority::Normal,
    )
}

fn create_compute_task(id: u64, iterations: u64) -> Task {
    Task::new(
        format!("compute_{}", id),
        Box::new(move || {
            let mut sum = 0u64;
            for i in 0..iterations {
                sum = sum.wrapping_add(i);
            }
            black_box(sum);
        }),
        TaskPriority::Normal,
    )
}

fn create_io_task(id: u64, sleep_micros: u64) -> Task {
    Task::new(
        format!("io_{}", id),
        Box::new(move || {
            thread::sleep(Duration::from_micros(sleep_micros));
            black_box(id);
        }),
        TaskPriority::Normal,
    )
}

fn create_priority_task(id: u64, priority: TaskPriority) -> Task {
    Task::new(
        format!("priority_{}", id),
        Box::new(move || {
            black_box(id);
        }),
        priority,
    )
}

// ============================================================================
// Schedule Overhead Benchmarks
// ============================================================================

fn bench_schedule_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_overhead");
    group.throughput(Throughput::Elements(1));

    // Single task scheduling
    group.bench_function("single_task", |b| {
        let mut scheduler = NanoScheduler::new(4);
        let mut task_id = 0u64;

        b.iter(|| {
            task_id += 1;
            let task = create_simple_task(task_id);
            black_box(scheduler.schedule(black_box(task)))
        });
    });

    // Batch scheduling
    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            batch_size,
            |b, &size| {
                let mut scheduler = NanoScheduler::new(4);
                let mut task_id = 0u64;

                b.iter(|| {
                    let tasks: Vec<_> = (0..size)
                        .map(|_| {
                            task_id += 1;
                            create_simple_task(task_id)
                        })
                        .collect();

                    for task in tasks {
                        black_box(scheduler.schedule(black_box(task)));
                    }
                });
            }
        );
    }

    group.finish();
}

fn bench_schedule_with_priorities(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_priorities");

    let priorities = [
        ("critical", TaskPriority::Critical),
        ("high", TaskPriority::High),
        ("normal", TaskPriority::Normal),
        ("low", TaskPriority::Low),
    ];

    for (name, priority) in priorities.iter() {
        group.bench_function(*name, |b| {
            let mut scheduler = NanoScheduler::new(4);
            let mut task_id = 0u64;

            b.iter(|| {
                task_id += 1;
                let task = create_priority_task(task_id, *priority);
                black_box(scheduler.schedule(black_box(task)))
            });
        });
    }

    group.finish();
}

// ============================================================================
// Task Execution Latency Benchmarks
// ============================================================================

fn bench_execution_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_latency");

    // Minimal work
    group.bench_function("minimal_work", |b| {
        let mut scheduler = NanoScheduler::new(4);

        b.iter(|| {
            let task = create_simple_task(1);
            scheduler.schedule(task);
            scheduler.run_once();
        });
    });

    // Light compute
    group.bench_function("light_compute", |b| {
        let mut scheduler = NanoScheduler::new(4);

        b.iter(|| {
            let task = create_compute_task(1, 100);
            scheduler.schedule(task);
            scheduler.run_once();
        });
    });

    // Medium compute
    group.bench_function("medium_compute", |b| {
        let mut scheduler = NanoScheduler::new(4);

        b.iter(|| {
            let task = create_compute_task(1, 1000);
            scheduler.schedule(task);
            scheduler.run_once();
        });
    });

    // Heavy compute
    group.bench_function("heavy_compute", |b| {
        let mut scheduler = NanoScheduler::new(4);

        b.iter(|| {
            let task = create_compute_task(1, 10000);
            scheduler.schedule(task);
            scheduler.run_once();
        });
    });

    group.finish();
}

fn bench_execution_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_throughput");

    for num_tasks in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64));
        group.bench_with_input(
            BenchmarkId::new("tasks", num_tasks),
            num_tasks,
            |b, &size| {
                b.iter(|| {
                    let mut scheduler = NanoScheduler::new(4);

                    for i in 0..size {
                        let task = create_simple_task(i as u64);
                        scheduler.schedule(task);
                    }

                    while scheduler.has_pending_tasks() {
                        scheduler.run_once();
                    }
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Priority Queue Benchmarks
// ============================================================================

fn bench_priority_queue_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_queue");

    // Push operations
    group.bench_function("push_operations", |b| {
        let mut scheduler = NanoScheduler::new(4);
        let mut task_id = 0u64;

        b.iter(|| {
            task_id += 1;
            let priority = match task_id % 4 {
                0 => TaskPriority::Critical,
                1 => TaskPriority::High,
                2 => TaskPriority::Normal,
                _ => TaskPriority::Low,
            };
            let task = create_priority_task(task_id, priority);
            black_box(scheduler.schedule(black_box(task)))
        });
    });

    // Pop operations
    group.bench_function("pop_operations", |b| {
        b.iter(|| {
            let mut scheduler = NanoScheduler::new(4);

            // Fill queue
            for i in 0..100 {
                let priority = match i % 4 {
                    0 => TaskPriority::Critical,
                    1 => TaskPriority::High,
                    2 => TaskPriority::Normal,
                    _ => TaskPriority::Low,
                };
                let task = create_priority_task(i, priority);
                scheduler.schedule(task);
            }

            // Pop all
            while scheduler.has_pending_tasks() {
                black_box(scheduler.run_once());
            }
        });
    });

    // Mixed operations
    group.bench_function("mixed_operations", |b| {
        let mut scheduler = NanoScheduler::new(4);
        let mut task_id = 0u64;

        b.iter(|| {
            task_id += 1;

            // Interleave push and pop
            if task_id % 3 == 0 {
                scheduler.run_once();
            } else {
                let priority = match task_id % 4 {
                    0 => TaskPriority::Critical,
                    1 => TaskPriority::High,
                    2 => TaskPriority::Normal,
                    _ => TaskPriority::Low,
                };
                let task = create_priority_task(task_id, priority);
                black_box(scheduler.schedule(task));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Statistics Calculation Benchmarks
// ============================================================================

fn bench_statistics_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistics");

    // Stats collection overhead
    group.bench_function("stats_collection", |b| {
        let mut scheduler = NanoScheduler::new(4);

        // Generate some history
        for i in 0..100 {
            let task = create_compute_task(i, 100);
            scheduler.schedule(task);
        }

        while scheduler.has_pending_tasks() {
            scheduler.run_once();
        }

        b.iter(|| {
            black_box(scheduler.get_stats())
        });
    });

    // Stats calculation with varying history sizes
    for history_size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("history", history_size),
            history_size,
            |b, &size| {
                let mut scheduler = NanoScheduler::new(4);

                for i in 0..size {
                    let task = create_compute_task(i as u64, 100);
                    scheduler.schedule(task);
                }

                while scheduler.has_pending_tasks() {
                    scheduler.run_once();
                }

                b.iter(|| {
                    black_box(scheduler.get_stats())
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Multi-threaded Scheduling Benchmarks
// ============================================================================

fn bench_multithreaded_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("multithreaded");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("threads", num_threads),
            num_threads,
            |b, &threads| {
                b.iter(|| {
                    let scheduler = Arc::new(Mutex::new(NanoScheduler::new(threads)));
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let scheduler = Arc::clone(&scheduler);
                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let task = create_compute_task(
                                    thread_id as u64 * 100 + i,
                                    100
                                );
                                scheduler.lock().unwrap().schedule(task);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    let mut scheduler = scheduler.lock().unwrap();
                    while scheduler.has_pending_tasks() {
                        scheduler.run_once();
                    }
                });
            }
        );
    }

    group.finish();
}

fn bench_contention_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention");

    // High contention
    group.bench_function("high_contention", |b| {
        b.iter(|| {
            let scheduler = Arc::new(Mutex::new(NanoScheduler::new(4)));
            let mut handles = vec![];

            for _ in 0..8 {
                let scheduler = Arc::clone(&scheduler);
                let handle = thread::spawn(move || {
                    for i in 0..50 {
                        let task = create_simple_task(i);
                        scheduler.lock().unwrap().schedule(task);
                        thread::yield_now();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    // Low contention
    group.bench_function("low_contention", |b| {
        b.iter(|| {
            let scheduler = Arc::new(Mutex::new(NanoScheduler::new(4)));
            let mut handles = vec![];

            for _ in 0..2 {
                let scheduler = Arc::clone(&scheduler);
                let handle = thread::spawn(move || {
                    for i in 0..200 {
                        let task = create_simple_task(i);
                        scheduler.lock().unwrap().schedule(task);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = overhead_benches;
    config = Criterion::default()
        .sample_size(1000)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = bench_schedule_overhead, bench_schedule_with_priorities
}

criterion_group! {
    name = latency_benches;
    config = Criterion::default()
        .sample_size(200)
        .measurement_time(Duration::from_secs(10));
    targets = bench_execution_latency, bench_execution_throughput
}

criterion_group! {
    name = queue_benches;
    config = Criterion::default()
        .sample_size(500)
        .measurement_time(Duration::from_secs(8));
    targets = bench_priority_queue_operations
}

criterion_group! {
    name = stats_benches;
    config = Criterion::default()
        .sample_size(500);
    targets = bench_statistics_overhead
}

criterion_group! {
    name = threading_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(15));
    targets = bench_multithreaded_scheduling, bench_contention_scenarios
}

criterion_main!(
    overhead_benches,
    latency_benches,
    queue_benches,
    stats_benches,
    threading_benches
);
