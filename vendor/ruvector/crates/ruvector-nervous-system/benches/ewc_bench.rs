use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_nervous_system::plasticity::consolidate::{ComplementaryLearning, Experience, EWC};

fn bench_fisher_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fisher_computation");

    for size in [1_000, 10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut ewc = EWC::new(1000.0);
            let params = vec![0.5; size];
            let gradients: Vec<Vec<f32>> = (0..50).map(|_| vec![0.1; size]).collect();

            b.iter(|| {
                ewc.compute_fisher(black_box(&params), black_box(&gradients))
                    .unwrap();
            });
        });
    }

    group.finish();
}

fn bench_ewc_loss(c: &mut Criterion) {
    let mut group = c.benchmark_group("ewc_loss");

    for size in [1_000, 10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut ewc = EWC::new(1000.0);
            let params = vec![0.5; size];
            let gradients: Vec<Vec<f32>> = (0..50).map(|_| vec![0.1; size]).collect();
            ewc.compute_fisher(&params, &gradients).unwrap();

            let new_params = vec![0.6; size];

            b.iter(|| {
                black_box(ewc.ewc_loss(black_box(&new_params)));
            });
        });
    }

    group.finish();
}

fn bench_ewc_gradient(c: &mut Criterion) {
    let mut group = c.benchmark_group("ewc_gradient");

    for size in [1_000, 10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut ewc = EWC::new(1000.0);
            let params = vec![0.5; size];
            let gradients: Vec<Vec<f32>> = (0..50).map(|_| vec![0.1; size]).collect();
            ewc.compute_fisher(&params, &gradients).unwrap();

            let new_params = vec![0.6; size];

            b.iter(|| {
                black_box(ewc.ewc_gradient(black_box(&new_params)));
            });
        });
    }

    group.finish();
}

fn bench_consolidation(c: &mut Criterion) {
    let mut group = c.benchmark_group("consolidation");

    for buffer_size in [100, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(buffer_size),
            &buffer_size,
            |b, &buffer_size| {
                let mut cls = ComplementaryLearning::new(1000, buffer_size, 1000.0);

                // Fill buffer
                for _ in 0..buffer_size {
                    let exp = Experience::new(vec![1.0; 10], vec![0.5; 10], 1.0);
                    cls.store_experience(exp);
                }

                b.iter(|| {
                    cls.consolidate(black_box(10), black_box(0.01)).unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_experience_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("experience_storage");

    let cls = ComplementaryLearning::new(1000, 10_000, 1000.0);
    let exp = Experience::new(vec![1.0; 100], vec![0.5; 100], 1.0);

    group.bench_function("store_experience", |b| {
        b.iter(|| {
            cls.store_experience(black_box(exp.clone()));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_fisher_computation,
    bench_ewc_loss,
    bench_ewc_gradient,
    bench_consolidation,
    bench_experience_storage
);
criterion_main!(benches);
