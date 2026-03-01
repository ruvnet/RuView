use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_sona::{SonaConfig, SonaEngine};

fn trajectory_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory");

    for dim in [64, 128, 256, 512].iter() {
        let engine = SonaEngine::with_config(SonaConfig {
            hidden_dim: *dim,
            embedding_dim: *dim,
            ..Default::default()
        });

        group.bench_with_input(BenchmarkId::new("single", dim), dim, |b, &dim| {
            b.iter(|| {
                let mut builder = engine.begin_trajectory(vec![0.1; dim]);
                builder.add_step(vec![0.5; dim], vec![], 0.8);
                builder.add_step(vec![0.6; dim], vec![], 0.9);
                engine.end_trajectory(builder, black_box(0.85));
            });
        });
    }

    group.finish();
}

fn lora_application_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora");

    for dim in [64, 128, 256, 512].iter() {
        let engine = SonaEngine::with_config(SonaConfig {
            hidden_dim: *dim,
            embedding_dim: *dim,
            ..Default::default()
        });

        // Warmup with some trajectories
        for _ in 0..10 {
            let mut builder = engine.begin_trajectory(vec![0.1; *dim]);
            builder.add_step(vec![0.5; *dim], vec![], 0.8);
            engine.end_trajectory(builder, 0.85);
        }
        engine.flush();

        group.bench_with_input(BenchmarkId::new("micro", dim), dim, |b, &dim| {
            let input = vec![1.0; dim];
            let mut output = vec![0.0; dim];
            b.iter(|| {
                engine.apply_micro_lora(black_box(&input), black_box(&mut output));
            });
        });

        group.bench_with_input(BenchmarkId::new("base", dim), dim, |b, &dim| {
            let input = vec![1.0; dim];
            let mut output = vec![0.0; dim];
            b.iter(|| {
                engine.apply_base_lora(0, black_box(&input), black_box(&mut output));
            });
        });
    }

    group.finish();
}

fn background_learning_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("learning");
    group.sample_size(10); // Fewer samples for expensive operation

    let engine = SonaEngine::with_config(SonaConfig {
        hidden_dim: 256,
        embedding_dim: 256,
        ..Default::default()
    });

    // Prepare 100 trajectories
    for _ in 0..100 {
        let mut builder = engine.begin_trajectory(vec![0.1; 256]);
        builder.add_step(vec![0.5; 256], vec![], 0.8);
        builder.add_step(vec![0.6; 256], vec![], 0.9);
        engine.end_trajectory(builder, 0.85);
    }

    group.bench_function("force_learn", |b| {
        b.iter(|| {
            black_box(engine.force_learn());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    trajectory_benchmark,
    lora_application_benchmark,
    background_learning_benchmark
);
criterion_main!(benches);
