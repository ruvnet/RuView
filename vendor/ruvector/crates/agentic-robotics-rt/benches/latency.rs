use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ros3_rt::{LatencyTracker, ROS3Executor, Priority, Deadline};
use std::time::Duration;

fn benchmark_latency_tracking(c: &mut Criterion) {
    c.bench_function("latency_record", |b| {
        let tracker = LatencyTracker::new("benchmark");
        let duration = Duration::from_micros(100);

        b.iter(|| {
            black_box(tracker.record(duration));
        });
    });
}

fn benchmark_executor_spawn(c: &mut Criterion) {
    let executor = ROS3Executor::new().unwrap();

    c.bench_function("executor_spawn_high", |b| {
        b.iter(|| {
            executor.spawn_high(async {
                black_box(42);
            });
        });
    });
}

criterion_group!(benches, benchmark_latency_tracking, benchmark_executor_spawn);
criterion_main!(benches);
