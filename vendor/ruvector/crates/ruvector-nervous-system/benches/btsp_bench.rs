use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_nervous_system::plasticity::btsp::{BTSPAssociativeMemory, BTSPLayer, BTSPSynapse};

fn bench_synapse_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("synapse_update");

    let mut synapse = BTSPSynapse::new(0.5, 2000.0).unwrap();

    group.bench_function("update_no_plateau", |b| {
        b.iter(|| {
            synapse.update(black_box(true), black_box(false), black_box(1.0));
        });
    });

    group.bench_function("update_with_plateau", |b| {
        b.iter(|| {
            synapse.update(black_box(true), black_box(true), black_box(1.0));
        });
    });

    group.finish();
}

fn bench_layer_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer_forward");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let layer = BTSPLayer::new(*size, 2000.0);
        let input = vec![0.5; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| layer.forward(black_box(&input)));
        });
    }

    group.finish();
}

fn bench_one_shot_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("one_shot_learning");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let mut layer = BTSPLayer::new(*size, 2000.0);
        let pattern = vec![0.5; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                layer.one_shot_associate(black_box(&pattern), black_box(0.8));
            });
        });
    }

    group.finish();
}

fn bench_associative_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("associative_memory");

    let mut memory = BTSPAssociativeMemory::new(128, 64);
    let key = vec![0.5; 128];
    let value = vec![0.1; 64];

    group.bench_function("store_one_shot", |b| {
        b.iter(|| {
            memory
                .store_one_shot(black_box(&key), black_box(&value))
                .unwrap();
        });
    });

    group.bench_function("retrieve", |b| {
        memory.store_one_shot(&key, &value).unwrap();
        b.iter(|| {
            memory.retrieve(black_box(&key)).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_synapse_update,
    bench_layer_forward,
    bench_one_shot_learning,
    bench_associative_memory
);
criterion_main!(benches);
