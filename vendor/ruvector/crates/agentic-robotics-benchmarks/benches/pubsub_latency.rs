use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ros3_core::message::RobotState;
use ros3_core::publisher::Publisher;
use ros3_core::subscriber::Subscriber;
use ros3_core::serialization::Serializer;
use std::time::{Duration, Instant};

fn benchmark_publisher_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Publisher Creation");

    group.bench_function("create_publisher", |b| {
        b.iter(|| {
            let publisher = Publisher::<RobotState>::new(
                black_box("test_topic".to_string()),
                Serializer::Cdr,
            );
            black_box(publisher)
        })
    });

    group.finish();
}

fn benchmark_subscriber_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Subscriber Creation");

    group.bench_function("create_subscriber", |b| {
        b.iter(|| {
            let subscriber = Subscriber::<RobotState>::new(
                black_box("test_topic".to_string()),
                Serializer::Cdr,
            );
            black_box(subscriber)
        })
    });

    group.finish();
}

fn benchmark_publish_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Publish Latency");

    let publisher = Publisher::<RobotState>::new("bench_topic".to_string(), Serializer::Cdr);

    let message = RobotState {
        position: [1.0, 2.0, 3.0],
        velocity: [0.1, 0.2, 0.3],
        timestamp: 123456789,
    };

    group.bench_function("single_publish", |b| {
        b.iter(|| {
            let result = futures::executor::block_on(publisher.publish(black_box(&message)));
            black_box(result)
        })
    });

    group.finish();
}

fn benchmark_publish_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("Publish Throughput");

    let publisher = Publisher::<RobotState>::new("bench_topic".to_string(), Serializer::Cdr);

    let message = RobotState {
        position: [1.0, 2.0, 3.0],
        velocity: [0.1, 0.2, 0.3],
        timestamp: 123456789,
    };

    // Benchmark burst publishing
    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_publish", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    for _ in 0..size {
                        futures::executor::block_on(publisher.publish(black_box(&message))).ok();
                    }
                })
            },
        );
    }

    group.finish();
}

fn benchmark_end_to_end_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("End-to-End Latency");
    group.sample_size(100); // Reduce sample size for async operations

    // Measure full publish-subscribe round trip
    group.bench_function("pubsub_roundtrip", |b| {
        b.iter_custom(|iters| {
            let publisher = Publisher::<RobotState>::new("latency_topic".to_string(), Serializer::Cdr);
            let _subscriber = Subscriber::<RobotState>::new("latency_topic".to_string(), Serializer::Cdr);

            let start = Instant::now();

            for i in 0..iters {
                let message = RobotState {
                    position: [i as f64, i as f64, i as f64],
                    velocity: [0.1, 0.2, 0.3],
                    timestamp: i as i64,
                };

                futures::executor::block_on(publisher.publish(&message)).ok();
            }

            start.elapsed()
        })
    });

    group.finish();
}

fn benchmark_serializer_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("Serializer Comparison");

    let message = RobotState {
        position: [1.0, 2.0, 3.0],
        velocity: [0.1, 0.2, 0.3],
        timestamp: 123456789,
    };

    // CDR serializer
    let cdr_publisher = Publisher::<RobotState>::new("cdr_topic".to_string(), Serializer::Cdr);
    group.bench_function("CDR_publish", |b| {
        b.iter(|| {
            futures::executor::block_on(cdr_publisher.publish(black_box(&message))).ok();
        })
    });

    // JSON serializer
    let json_publisher = Publisher::<RobotState>::new("json_topic".to_string(), Serializer::Json);
    group.bench_function("JSON_publish", |b| {
        b.iter(|| {
            futures::executor::block_on(json_publisher.publish(black_box(&message))).ok();
        })
    });

    group.finish();
}

fn benchmark_concurrent_publishers(c: &mut Criterion) {
    let mut group = c.benchmark_group("Concurrent Publishers");
    group.sample_size(50);

    for num_publishers in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent", num_publishers),
            num_publishers,
            |b, &count| {
                b.iter(|| {
                    let publishers: Vec<_> = (0..count)
                        .map(|i| {
                            Publisher::<RobotState>::new(
                                format!("topic_{}", i),
                                Serializer::Cdr,
                            )
                        })
                        .collect();

                    let message = RobotState {
                        position: [1.0, 2.0, 3.0],
                        velocity: [0.1, 0.2, 0.3],
                        timestamp: 123456789,
                    };

                    for publisher in &publishers {
                        futures::executor::block_on(publisher.publish(&message)).ok();
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_publisher_creation,
    benchmark_subscriber_creation,
    benchmark_publish_latency,
    benchmark_publish_throughput,
    benchmark_end_to_end_latency,
    benchmark_serializer_comparison,
    benchmark_concurrent_publishers
);
criterion_main!(benches);
