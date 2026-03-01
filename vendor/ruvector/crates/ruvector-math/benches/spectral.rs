//! Benchmarks for spectral methods

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_math::spectral::{ChebyshevExpansion, ChebyshevPolynomial};

fn bench_chebyshev_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("chebyshev_eval");

    for degree in [10, 20, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("degree", degree),
            &degree,
            |bench, &deg| {
                let poly = ChebyshevPolynomial::new(deg);
                bench.iter(|| poly.eval(black_box(0.5)));
            },
        );
    }

    group.finish();
}

fn bench_chebyshev_eval_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("chebyshev_eval_all");

    for degree in [10, 20, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("degree", degree),
            &degree,
            |bench, &deg| {
                bench.iter(|| ChebyshevPolynomial::eval_all(black_box(0.5), black_box(deg)));
            },
        );
    }

    group.finish();
}

fn bench_chebyshev_expansion(c: &mut Criterion) {
    let mut group = c.benchmark_group("chebyshev_expansion");

    for degree in [10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("from_function", degree),
            &degree,
            |bench, &deg| {
                bench.iter(|| ChebyshevExpansion::from_function(|x| x.sin(), black_box(deg)));
            },
        );
    }

    group.finish();
}

fn bench_heat_kernel(c: &mut Criterion) {
    let mut group = c.benchmark_group("heat_kernel");

    for degree in [10, 20, 50] {
        for t in [0.1, 1.0, 10.0] {
            group.bench_with_input(
                BenchmarkId::new(format!("deg={}_t={}", degree, t), degree),
                &(degree, t),
                |bench, &(deg, t)| {
                    bench.iter(|| ChebyshevExpansion::heat_kernel(black_box(t), black_box(deg)));
                },
            );
        }
    }

    group.finish();
}

fn bench_clenshaw_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("clenshaw_eval");

    for degree in [10, 20, 50, 100] {
        let expansion = ChebyshevExpansion::from_function(|x| x.sin(), degree);

        group.bench_with_input(
            BenchmarkId::new("degree", degree),
            &expansion,
            |bench, exp| {
                bench.iter(|| exp.eval(black_box(0.5)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_chebyshev_eval,
    bench_chebyshev_eval_all,
    bench_chebyshev_expansion,
    bench_heat_kernel,
    bench_clenshaw_eval,
);
criterion_main!(benches);
