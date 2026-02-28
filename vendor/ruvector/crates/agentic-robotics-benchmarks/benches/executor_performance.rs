use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ros3_rt::executor::{ROS3Executor, Priority, Deadline};
use ros3_rt::scheduler::PriorityScheduler;
use std::time::Duration;

fn benchmark_executor_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Executor Creation");

    group.bench_function("create_executor", |b| {
        b.iter(|| {
            let executor = ROS3Executor::new().unwrap();
            black_box(executor)
        })
    });

    group.finish();
}

fn benchmark_task_spawning(c: &mut Criterion) {
    let mut group = c.benchmark_group("Task Spawning");

    let executor = ROS3Executor::new().unwrap();

    group.bench_function("spawn_high_priority", |b| {
        b.iter(|| {
            executor.spawn_rt(
                Priority::High,
                Deadline(Duration::from_micros(100)),
                async {
                    // Minimal async task
                    black_box(42);
                },
            );
        })
    });

    group.bench_function("spawn_low_priority", |b| {
        b.iter(|| {
            executor.spawn_rt(
                Priority::Low,
                Deadline(Duration::from_millis(100)),
                async {
                    // Minimal async task
                    black_box(42);
                },
            );
        })
    });

    group.finish();
}

fn benchmark_scheduler_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("Scheduler Overhead");

    let scheduler = PriorityScheduler::new();

    group.bench_function("priority_low", |b| {
        b.iter(|| {
            scheduler.should_use_high_priority(
                black_box(Priority::Low),
                black_box(Deadline(Duration::from_millis(100))),
            );
        })
    });

    group.bench_function("priority_high", |b| {
        b.iter(|| {
            scheduler.should_use_high_priority(
                black_box(Priority::High),
                black_box(Deadline(Duration::from_micros(100))),
            );
        })
    });

    group.bench_function("deadline_check_fast", |b| {
        b.iter(|| {
            scheduler.should_use_high_priority(
                black_box(Priority::Medium),
                black_box(Deadline(Duration::from_micros(500))),
            );
        })
    });

    group.bench_function("deadline_check_slow", |b| {
        b.iter(|| {
            scheduler.should_use_high_priority(
                black_box(Priority::Medium),
                black_box(Deadline(Duration::from_secs(1))),
            );
        })
    });

    group.finish();
}

fn benchmark_task_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Task Distribution");

    for num_tasks in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("spawn_tasks", num_tasks),
            num_tasks,
            |b, &count| {
                b.iter(|| {
                    let executor = ROS3Executor::new().unwrap();

                    for i in 0..count {
                        let priority = if i % 3 == 0 {
                            Priority::High
                        } else if i % 3 == 1 {
                            Priority::Medium
                        } else {
                            Priority::Low
                        };

                        let deadline = if priority == Priority::High {
                            Deadline(Duration::from_micros(100))
                        } else {
                            Deadline(Duration::from_millis(10))
                        };

                        executor.spawn_rt(priority, deadline, async move {
                            black_box(i);
                        });
                    }

                    black_box(executor)
                })
            },
        );
    }

    group.finish();
}

fn benchmark_async_task_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Async Task Execution");
    group.sample_size(50);

    let executor = ROS3Executor::new().unwrap();

    group.bench_function("execute_sync_task", |b| {
        b.iter(|| {
            executor.spawn_rt(
                Priority::High,
                Deadline(Duration::from_micros(100)),
                async {
                    // Synchronous computation
                    let mut sum = 0;
                    for i in 0..100 {
                        sum += i;
                    }
                    black_box(sum)
                },
            );
        })
    });

    group.bench_function("execute_with_yield", |b| {
        b.iter(|| {
            executor.spawn_rt(
                Priority::Medium,
                Deadline(Duration::from_millis(1)),
                async {
                    // Yield to executor
                    tokio::task::yield_now().await;
                    black_box(42)
                },
            );
        })
    });

    group.finish();
}

fn benchmark_priority_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Priority Handling");

    let executor = ROS3Executor::new().unwrap();

    // Mix of priorities
    group.bench_function("mixed_priorities", |b| {
        b.iter(|| {
            // High priority task
            executor.spawn_rt(
                Priority::High,
                Deadline(Duration::from_micros(50)),
                async { black_box(1) },
            );

            // Medium priority task
            executor.spawn_rt(
                Priority::Medium,
                Deadline(Duration::from_millis(1)),
                async { black_box(2) },
            );

            // Low priority task
            executor.spawn_rt(
                Priority::Low,
                Deadline(Duration::from_millis(100)),
                async { black_box(3) },
            );
        })
    });

    group.finish();
}

fn benchmark_deadline_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Deadline Distribution");

    let executor = ROS3Executor::new().unwrap();

    // Tight deadlines (should use high priority runtime)
    group.bench_function("tight_deadlines", |b| {
        b.iter(|| {
            for _ in 0..10 {
                executor.spawn_rt(
                    Priority::High,
                    Deadline(Duration::from_micros(100)),
                    async { black_box(42) },
                );
            }
        })
    });

    // Loose deadlines (should use low priority runtime)
    group.bench_function("loose_deadlines", |b| {
        b.iter(|| {
            for _ in 0..10 {
                executor.spawn_rt(
                    Priority::Low,
                    Deadline(Duration::from_millis(100)),
                    async { black_box(42) },
                );
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_executor_creation,
    benchmark_task_spawning,
    benchmark_scheduler_overhead,
    benchmark_task_distribution,
    benchmark_async_task_execution,
    benchmark_priority_handling,
    benchmark_deadline_distribution
);
criterion_main!(benches);
