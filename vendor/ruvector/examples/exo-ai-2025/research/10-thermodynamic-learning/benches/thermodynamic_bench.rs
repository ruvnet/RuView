use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use thermodynamic_learning::equilibrium_propagation::*;
use thermodynamic_learning::free_energy_agent::*;
use thermodynamic_learning::landauer_learning::*;
use thermodynamic_learning::novel_algorithms::*;
use thermodynamic_learning::reversible_neural::*;
use thermodynamic_learning::*;

#[cfg(feature = "simd")]
use thermodynamic_learning::simd_ops::*;

/// Benchmark Landauer-optimal learning
fn bench_landauer_optimizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("Landauer Optimizer");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut optimizer = LandauerOptimizer::new(0.01, constants::ROOM_TEMP);
            let gradient: Vec<f64> = (0..size).map(|i| (i as f64 * 0.1).sin()).collect();
            let mut params: Vec<f64> = vec![0.5; size];

            b.iter(|| {
                optimizer.step(black_box(&gradient), black_box(&mut params));
            });
        });
    }

    group.finish();
}

/// Benchmark equilibrium propagation
fn bench_equilibrium_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Equilibrium Propagation");

    for hidden in [4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(hidden), hidden, |b, &hidden| {
            let mut network = EnergyBasedNetwork::new(vec![2, hidden, 1], 1.0, 300.0);
            let input = vec![1.0, 0.5];
            let target = vec![1.0];

            b.iter(|| {
                network.equilibrium_propagation_step(
                    black_box(&input),
                    black_box(&target),
                    0.5,
                    0.01,
                );
            });
        });
    }

    group.finish();
}

/// Benchmark free energy agent perception
fn bench_free_energy_perception(c: &mut Criterion) {
    let mut group = c.benchmark_group("Free Energy Perception");

    for dim in [2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(dim), dim, |b, &dim| {
            let mut agent = FreeEnergyAgent::new(dim, dim + 1, 300.0);
            let observation: Vec<f64> = (0..dim + 1).map(|i| (i as f64 * 0.1).sin()).collect();

            b.iter(|| {
                agent.perceive(black_box(&observation));
            });
        });
    }

    group.finish();
}

/// Benchmark reversible network forward pass
fn bench_reversible_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("Reversible Forward");

    for dim in [4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(dim), dim, |b, &dim| {
            let mut network = ReversibleNetwork::new(dim);
            network.add_coupling_layer(dim * 2, dim / 2);
            network.add_orthogonal_layer();

            let input: Vec<f64> = (0..dim).map(|i| (i as f64 * 0.1).sin()).collect();

            b.iter(|| {
                network.forward(black_box(&input));
            });
        });
    }

    group.finish();
}

/// Benchmark reversible network inverse pass
fn bench_reversible_inverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("Reversible Inverse");

    for dim in [4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(dim), dim, |b, &dim| {
            let mut network = ReversibleNetwork::new(dim);
            network.add_coupling_layer(dim * 2, dim / 2);
            network.add_orthogonal_layer();

            let input: Vec<f64> = (0..dim).map(|i| (i as f64 * 0.1).sin()).collect();
            let output = network.forward(&input);

            b.iter(|| {
                network.inverse(black_box(&output));
            });
        });
    }

    group.finish();
}

/// Benchmark novel entropy-regularized learner
fn bench_entropy_regularized(c: &mut Criterion) {
    let mut group = c.benchmark_group("Entropy Regularized");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut learner = EntropyRegularizedLearner::new(300.0, 0.1);
            let gradient: Vec<f64> = (0..size).map(|i| (i as f64 * 0.1).sin()).collect();
            let mut params: Vec<f64> = vec![0.5; size];

            b.iter(|| {
                learner.step(black_box(&mut params), black_box(&gradient), 1e-20);
            });
        });
    }

    group.finish();
}

/// Benchmark fluctuation theorem optimizer
fn bench_fluctuation_theorem(c: &mut Criterion) {
    let mut group = c.benchmark_group("Fluctuation Theorem");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut optimizer = FluctuationTheoremOptimizer::new(300.0);
            let gradient: Vec<f64> = (0..size).map(|i| (i as f64 * 0.1).sin()).collect();
            let mut params: Vec<f64> = vec![0.5; size];

            b.iter(|| {
                optimizer.step(black_box(&mut params), black_box(&gradient));
            });
        });
    }

    group.finish();
}

/// Benchmark heat engine network
fn bench_heat_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("Heat Engine Network");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut engine = HeatEngineNetwork::new(size, 400.0, 300.0);
            let gradient_hot: Vec<f64> = (0..size).map(|i| (i as f64 * 0.1).sin()).collect();
            let gradient_cold: Vec<f64> = (0..size).map(|i| (i as f64 * 0.05).cos()).collect();

            b.iter(|| {
                engine.cycle(black_box(&gradient_hot), black_box(&gradient_cold));
            });
        });
    }

    group.finish();
}

/// Benchmark SIMD operations
#[cfg(feature = "simd")]
fn bench_simd_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD Operations");

    for size in [100, 1000, 10000].iter() {
        // Dot product
        group.bench_with_input(BenchmarkId::new("dot_product", size), size, |b, &size| {
            let a: Vec<f64> = (0..size).map(|i| i as f64 * 0.1).collect();
            let b: Vec<f64> = (0..size).map(|i| (size - i) as f64 * 0.1).collect();

            b.iter(|| {
                simd_dot_product(black_box(&a), black_box(&b));
            });
        });

        // Norm squared
        group.bench_with_input(BenchmarkId::new("norm_squared", size), size, |b, &size| {
            let x: Vec<f64> = (0..size).map(|i| i as f64 * 0.1).collect();

            b.iter(|| {
                simd_norm_squared(black_box(&x));
            });
        });

        // Entropy calculation
        group.bench_with_input(BenchmarkId::new("entropy", size), size, |b, &size| {
            let probs: Vec<f64> = (0..size)
                .map(|i| ((i as f64 + 1.0) / (size as f64 + 1.0)))
                .collect();

            b.iter(|| {
                energy::entropy(black_box(&probs));
            });
        });
    }

    group.finish();
}

/// Comprehensive energy calculation benchmark
fn bench_energy_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Energy Calculations");

    for size in [100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("landauer_limit", size),
            size,
            |b, &size| {
                let state = ThermodynamicState::new(constants::ROOM_TEMP);

                b.iter(|| {
                    black_box(state.landauer_limit());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("energy_network", size),
            size,
            |b, &size| {
                let network =
                    EnergyBasedNetwork::new(vec![size / 10, size / 5, size / 10], 1.0, 300.0);

                b.iter(|| {
                    black_box(network.energy());
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_landauer_optimizer,
    bench_equilibrium_propagation,
    bench_free_energy_perception,
    bench_reversible_forward,
    bench_reversible_inverse,
    bench_entropy_regularized,
    bench_fluctuation_theorem,
    bench_heat_engine,
    bench_energy_calculations,
);

#[cfg(feature = "simd")]
criterion_group!(simd_benches, bench_simd_ops);

#[cfg(feature = "simd")]
criterion_main!(benches, simd_benches);

#[cfg(not(feature = "simd"))]
criterion_main!(benches);
