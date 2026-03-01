// Prefetch Prediction Benchmark - Accuracy and performance metrics
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use demand_paged_cognition::*;

fn bench_prefetch_accuracy(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_accuracy");

    // Sequential pattern
    group.bench_function("sequential_pattern", |b| {
        b.iter_with_setup(
            || PrefetchCoordinator::new(),
            |coordinator| {
                let context = vec![0.1, 0.2, 0.3];

                // Build sequential pattern
                for i in 0..100 {
                    coordinator.record_access(i, &context);
                }

                // Predict next
                let predictions = coordinator.predict_and_queue(100, &context, 10);
                black_box(predictions)
            },
        );
    });

    // Random pattern
    group.bench_function("random_pattern", |b| {
        b.iter_with_setup(
            || {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let coordinator = PrefetchCoordinator::new();
                let context = vec![0.1, 0.2, 0.3];

                // Build pseudo-random pattern
                for i in 0..100 {
                    let mut hasher = DefaultHasher::new();
                    i.hash(&mut hasher);
                    let page = (hasher.finish() % 1000) as u64;
                    coordinator.record_access(page, &context);
                }

                coordinator
            },
            |coordinator| {
                let context = vec![0.1, 0.2, 0.3];
                let predictions = coordinator.predict_and_queue(500, &context, 10);
                black_box(predictions)
            },
        );
    });

    // Cyclic pattern
    group.bench_function("cyclic_pattern", |b| {
        b.iter_with_setup(
            || {
                let coordinator = PrefetchCoordinator::new();
                let context = vec![0.1, 0.2, 0.3];

                // Build cyclic pattern: 1->2->3->4->1
                for _ in 0..25 {
                    coordinator.record_access(1, &context);
                    coordinator.record_access(2, &context);
                    coordinator.record_access(3, &context);
                    coordinator.record_access(4, &context);
                }

                coordinator
            },
            |coordinator| {
                let context = vec![0.1, 0.2, 0.3];
                let predictions = coordinator.predict_and_queue(4, &context, 5);
                black_box(predictions)
            },
        );
    });

    group.finish();
}

fn bench_streaming_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_learning");

    // Hoeffding Tree update
    group.bench_function("hoeffding_update", |b| {
        let predictor = HoeffdingTreePredictor::new();
        let features = AccessFeatures::new(42);

        b.iter(|| {
            predictor.update(black_box(42), black_box(&features))
        });
    });

    // Markov update
    group.bench_function("markov_update", |b| {
        let predictor = MarkovPredictor::new();

        b.iter(|| {
            predictor.update(black_box(1), black_box(2))
        });
    });

    group.finish();
}

fn bench_feature_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_extraction");

    for history_len in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(history_len),
            history_len,
            |b, &history_len| {
                let history: Vec<u64> = (0..history_len).collect();
                let context = vec![0.1, 0.2, 0.3, 0.4, 0.5];

                b.iter(|| {
                    let features = AccessFeatures::from_history(
                        black_box(&history),
                        black_box(&context),
                    );
                    black_box(features.to_vector())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_prefetch_accuracy,
    bench_streaming_learning,
    bench_feature_extraction
);
criterion_main!(benches);
