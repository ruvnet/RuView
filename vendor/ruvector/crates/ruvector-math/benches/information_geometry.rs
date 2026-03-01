//! Benchmarks for information geometry operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_distr::StandardNormal;
use ruvector_math::information_geometry::{FisherInformation, KFACApproximation, NaturalGradient};

fn generate_gradients(n: usize, dim: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dim).map(|_| rng.sample(StandardNormal)).collect())
        .collect()
}

fn bench_fisher_information(c: &mut Criterion) {
    let mut group = c.benchmark_group("fisher_information");

    for dim in [32, 64, 128, 256] {
        let samples = 100;
        let gradients = generate_gradients(samples, dim, 42);

        group.throughput(Throughput::Elements((samples * dim) as u64));

        // Diagonal FIM (fast)
        let fisher = FisherInformation::new();
        group.bench_with_input(
            BenchmarkId::new("diagonal_fim", dim),
            &gradients,
            |b, grads| {
                b.iter(|| fisher.diagonal_fim(black_box(grads)));
            },
        );

        // Full FIM (slower but more accurate)
        if dim <= 128 {
            group.bench_with_input(
                BenchmarkId::new("empirical_fim", dim),
                &gradients,
                |b, grads| {
                    b.iter(|| fisher.empirical_fim(black_box(grads)));
                },
            );
        }
    }

    group.finish();
}

fn bench_natural_gradient(c: &mut Criterion) {
    let mut group = c.benchmark_group("natural_gradient");

    for dim in [32, 64, 128] {
        let samples = 50;
        let gradients = generate_gradients(samples, dim, 42);
        let gradient = gradients[0].clone();

        group.throughput(Throughput::Elements(dim as u64));

        // Diagonal natural gradient (fast)
        let mut ng = NaturalGradient::new(0.01).with_diagonal(true);
        group.bench_with_input(
            BenchmarkId::new("diagonal_step", dim),
            &(&gradient, &gradients),
            |b, (g, gs)| {
                b.iter(|| {
                    ng.reset();
                    ng.step(black_box(g), Some(black_box(gs)))
                });
            },
        );
    }

    group.finish();
}

fn bench_kfac(c: &mut Criterion) {
    let mut group = c.benchmark_group("kfac");

    for (input_dim, output_dim) in [(32, 16), (64, 32), (128, 64)] {
        let batch_size = 32;
        let mut rng = StdRng::seed_from_u64(42);

        let activations: Vec<Vec<f64>> = (0..batch_size)
            .map(|_| (0..input_dim).map(|_| rng.sample(StandardNormal)).collect())
            .collect();

        let gradients: Vec<Vec<f64>> = (0..batch_size)
            .map(|_| {
                (0..output_dim)
                    .map(|_| rng.sample(StandardNormal))
                    .collect()
            })
            .collect();

        let weight_grad: Vec<Vec<f64>> = (0..output_dim)
            .map(|_| (0..input_dim).map(|_| rng.sample(StandardNormal)).collect())
            .collect();

        group.throughput(Throughput::Elements((input_dim * output_dim) as u64));

        // K-FAC update
        let mut kfac =
            ruvector_math::information_geometry::KFACApproximation::new(&[(input_dim, output_dim)]);

        group.bench_function(
            BenchmarkId::new("kfac_update", format!("{}x{}", input_dim, output_dim)),
            |b| {
                b.iter(|| kfac.update_layer(0, black_box(&activations), black_box(&gradients)));
            },
        );

        // K-FAC natural gradient
        kfac.update_layer(0, &activations, &gradients).unwrap();

        group.bench_function(
            BenchmarkId::new("kfac_nat_grad", format!("{}x{}", input_dim, output_dim)),
            |b| {
                b.iter(|| kfac.natural_gradient_layer(0, black_box(&weight_grad)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_fisher_information,
    bench_natural_gradient,
    bench_kfac,
);
criterion_main!(benches);
