//! Criterion microbenchmarks for thermorust motifs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::SeedableRng;
use thermorust::{
    dynamics::{anneal_continuous, anneal_discrete, step_discrete, Params},
    energy::{Couplings, EnergyModel, Ising},
    motifs::{IsingMotif, SoftSpinMotif},
    State,
};

fn bench_discrete_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("step_discrete");
    for n in [8, 16, 32] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let model = Ising::new(Couplings::ferromagnetic_ring(n, 0.2));
            let p = Params::default_n(n);
            let mut s = State::ones(n);
            let mut rng = rand::rngs::SmallRng::seed_from_u64(1);
            b.iter(|| {
                step_discrete(
                    black_box(&model),
                    black_box(&mut s),
                    black_box(&p),
                    &mut rng,
                );
            });
        });
    }
    group.finish();
}

fn bench_10k_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("10k_steps");
    for n in [16, 32] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut motif = IsingMotif::ring(n, 0.2);
                let p = Params::default_n(n);
                let mut rng = rand::rngs::SmallRng::seed_from_u64(123);
                let trace = anneal_discrete(
                    black_box(&motif.model),
                    black_box(&mut motif.state),
                    black_box(&p),
                    black_box(10_000),
                    0,
                    &mut rng,
                );
                black_box(motif.state.dissipated_j)
            });
        });
    }
    group.finish();
}

fn bench_langevin_10k(c: &mut Criterion) {
    let mut group = c.benchmark_group("langevin_10k");
    for n in [8, 16] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut motif = SoftSpinMotif::random(n, 1.0, 0.5, 42);
                let p = Params::default_n(n);
                let mut rng = rand::rngs::SmallRng::seed_from_u64(77);
                anneal_continuous(
                    black_box(&motif.model),
                    black_box(&mut motif.state),
                    black_box(&p),
                    black_box(10_000),
                    0,
                    &mut rng,
                );
                black_box(motif.state.dissipated_j)
            });
        });
    }
    group.finish();
}

fn bench_energy_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("energy_eval");
    for n in [8, 16, 32] {
        let model = Ising::new(Couplings::ferromagnetic_ring(n, 0.2));
        let s = State::ones(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(model.energy(black_box(&s))));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_discrete_step,
    bench_10k_steps,
    bench_langevin_10k,
    bench_energy_evaluation,
);
criterion_main!(benches);
