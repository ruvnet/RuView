// Benchmarks for Time Crystal Cognition

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ndarray::Array1;
use time_crystal_cognition::*;

fn bench_discrete_time_crystal(c: &mut Criterion) {
    let mut group = c.benchmark_group("discrete_time_crystal");

    for n_oscillators in [50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("dtc_simulation", n_oscillators),
            n_oscillators,
            |b, &n| {
                b.iter(|| {
                    let mut config = DTCConfig::default();
                    config.n_oscillators = n;
                    let mut dtc = DiscreteTimeCrystal::new(config);
                    let trajectory = dtc.run(black_box(1.0)); // 1 second
                    black_box(trajectory)
                });
            },
        );
    }

    group.finish();
}

fn bench_period_doubling_detection(c: &mut Criterion) {
    let mut config = DTCConfig::default();
    config.n_oscillators = 100;
    let mut dtc = DiscreteTimeCrystal::new(config);
    let trajectory = dtc.run(2.0);

    c.bench_function("period_doubling_detection", |b| {
        b.iter(|| {
            let result = dtc.detect_period_doubling(black_box(&trajectory));
            black_box(result)
        });
    });
}

fn bench_floquet_system(c: &mut Criterion) {
    let mut group = c.benchmark_group("floquet_cognition");

    for n_neurons in [50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("floquet_simulation", n_neurons),
            n_neurons,
            |b, &n| {
                b.iter(|| {
                    let mut config = FloquetConfig::default();
                    config.n_neurons = n;
                    let weights = FloquetCognitiveSystem::generate_asymmetric_weights(n, 0.2, 1.0);
                    let mut system = FloquetCognitiveSystem::new(config, weights);
                    let trajectory = system.run(black_box(10)); // 10 periods
                    black_box(trajectory)
                });
            },
        );
    }

    group.finish();
}

fn bench_temporal_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_memory");

    // Benchmark encoding
    group.bench_function("encode_item", |b| {
        b.iter(|| {
            let mut memory = TemporalMemory::new(TemporalMemoryConfig::default());
            let item = Array1::from_vec(vec![1.0; 64]);
            memory.encode(black_box(item)).unwrap();
            black_box(memory)
        });
    });

    // Benchmark maintenance
    group.bench_function("maintain_1000_steps", |b| {
        let mut memory = TemporalMemory::new(TemporalMemoryConfig::default());
        let item = Array1::from_vec(vec![1.0; 64]);
        memory.encode(item).unwrap();

        b.iter(|| {
            for _ in 0..1000 {
                memory.step();
            }
            black_box(&memory)
        });
    });

    // Benchmark retrieval
    group.bench_function("retrieve_item", |b| {
        let mut memory = TemporalMemory::new(TemporalMemoryConfig::default());
        let item = Array1::from_vec(vec![1.0; 64]);
        memory.encode(item.clone()).unwrap();

        for _ in 0..1000 {
            memory.step();
        }

        b.iter(|| {
            let result = memory.retrieve(black_box(&item));
            black_box(result)
        });
    });

    group.finish();
}

fn bench_working_memory_task(c: &mut Criterion) {
    c.bench_function("working_memory_task", |b| {
        b.iter(|| {
            let config = TemporalMemoryConfig::default();
            let mut task = WorkingMemoryTask::new(config, 4, 64);
            task.run_delayed_match_to_sample(0.5, 2.0);
            black_box(task)
        });
    });
}

criterion_group!(
    benches,
    bench_discrete_time_crystal,
    bench_period_doubling_detection,
    bench_floquet_system,
    bench_temporal_memory,
    bench_working_memory_task
);

criterion_main!(benches);
