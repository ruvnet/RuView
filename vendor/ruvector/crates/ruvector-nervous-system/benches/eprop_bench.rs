//! E-prop performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_nervous_system::plasticity::eprop::{EpropNetwork, EpropSynapse};

fn bench_synapse_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("synapse_update");

    let mut synapse = EpropSynapse::new(0.5, 20.0);

    group.bench_function("single_update", |b| {
        b.iter(|| {
            synapse.update(
                black_box(true),
                black_box(0.5),
                black_box(0.1),
                black_box(1.0),
                black_box(0.01),
            );
        });
    });

    group.finish();
}

fn bench_network_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_forward");

    for &hidden_size in &[100, 500, 1000] {
        let mut network = EpropNetwork::new(100, hidden_size, 10);
        let input = vec![0.5; 100];

        group.throughput(Throughput::Elements(hidden_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(hidden_size),
            &hidden_size,
            |b, _| {
                b.iter(|| {
                    network.forward(black_box(&input), black_box(1.0));
                });
            },
        );
    }

    group.finish();
}

fn bench_network_backward(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_backward");

    for &hidden_size in &[100, 500, 1000] {
        let mut network = EpropNetwork::new(100, hidden_size, 10);
        let input = vec![0.5; 100];
        let error = vec![0.1; 10];

        // Run forward first to populate spike buffer
        network.forward(&input, 1.0);

        group.throughput(Throughput::Elements(hidden_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(hidden_size),
            &hidden_size,
            |b, _| {
                b.iter(|| {
                    network.backward(black_box(&error), black_box(0.01), black_box(1.0));
                });
            },
        );
    }

    group.finish();
}

fn bench_online_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("online_step");

    let mut network = EpropNetwork::new(100, 1000, 10);
    let input = vec![0.5; 100];
    let target = vec![1.0; 10];

    group.throughput(Throughput::Elements(1000));
    group.bench_function("1000_neurons", |b| {
        b.iter(|| {
            network.online_step(
                black_box(&input),
                black_box(&target),
                black_box(1.0),
                black_box(0.01),
            );
        });
    });

    group.finish();
}

fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    for &size in &[100, 500, 1000, 5000] {
        group.bench_with_input(BenchmarkId::new("create_network", size), &size, |b, &s| {
            b.iter(|| {
                let network = EpropNetwork::new(100, s, 10);
                black_box(network);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_synapse_update,
    bench_network_forward,
    bench_network_backward,
    bench_online_step,
    bench_memory_footprint
);
criterion_main!(benches);
