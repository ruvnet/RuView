use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use meta_sim_consciousness::*;

fn bench_closed_form_phi(c: &mut Criterion) {
    let mut group = c.benchmark_group("closed_form_phi");

    for n in [4, 6, 8, 10, 12].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            // Create cycle network
            let mut adj = vec![vec![0.0; n]; n];
            for i in 0..n {
                adj[i][(i + 1) % n] = 1.0;
            }
            let nodes: Vec<u64> = (0..n as u64).collect();

            let calculator = ClosedFormPhi::default();

            b.iter(|| black_box(calculator.compute_phi_ergodic(&adj, &nodes)));
        });
    }

    group.finish();
}

fn bench_cei_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cei_computation");

    for n in [4, 6, 8, 10].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            let mut adj = vec![vec![0.0; n]; n];
            for i in 0..n {
                adj[i][(i + 1) % n] = 1.0;
            }

            let calculator = ClosedFormPhi::default();

            b.iter(|| black_box(calculator.compute_cei(&adj, 1.0)));
        });
    }

    group.finish();
}

fn bench_ergodicity_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("ergodicity_test");

    for n in [4, 6, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            let mut transition = vec![vec![0.0; n]; n];
            for i in 0..n {
                transition[i][(i + 1) % n] = 1.0;
            }

            let analyzer = ErgodicityAnalyzer::default();
            let observable = |state: &[f64]| state[0];

            b.iter(|| black_box(analyzer.test_ergodicity(&transition, observable)));
        });
    }

    group.finish();
}

fn bench_hierarchical_phi(c: &mut Criterion) {
    let mut group = c.benchmark_group("hierarchical_phi");

    group.bench_function("batch_64_depth_3", |b| {
        let param_space = ConsciousnessParameterSpace::new(4);
        let networks: Vec<_> = param_space
            .generate_networks()
            .into_iter()
            .take(64)
            .collect();

        b.iter(|| {
            let mut batcher = HierarchicalPhiBatcher::new(64, 3, 4);
            black_box(batcher.process_hierarchical_batch(&networks))
        });
    });

    group.finish();
}

fn bench_meta_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_simulation");

    group.bench_function("small_config", |b| {
        let config = MetaSimConfig {
            network_size: 4,
            hierarchy_depth: 2,
            batch_size: 8,
            num_cores: 1,
            simd_width: 1,
            bit_width: 1,
        };

        b.iter(|| {
            let mut simulator = MetaConsciousnessSimulator::new(config.clone());
            black_box(simulator.run_meta_simulation())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_closed_form_phi,
    bench_cei_computation,
    bench_ergodicity_test,
    bench_hierarchical_phi,
    bench_meta_simulation
);
criterion_main!(benches);
