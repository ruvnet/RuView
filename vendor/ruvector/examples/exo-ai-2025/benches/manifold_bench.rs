use burn::backend::NdArray;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use exo_core::{ManifoldConfig, Metadata, Pattern, PatternId, SubstrateTime};
use exo_manifold::ManifoldEngine;

type TestBackend = NdArray;

fn create_test_engine() -> ManifoldEngine<TestBackend> {
    let config = ManifoldConfig {
        dimension: 512,
        hidden_dim: 256,
        hidden_layers: 4,
        omega_0: 30.0,
        learning_rate: 0.01,
        max_descent_steps: 50,
        ..Default::default()
    };
    let device = Default::default();
    ManifoldEngine::<TestBackend>::new(config, device)
}

fn create_test_pattern(dim: usize, salience: f32) -> Pattern {
    Pattern {
        id: PatternId::new(),
        embedding: vec![0.5; dim],
        metadata: Metadata::default(),
        timestamp: SubstrateTime::now(),
        antecedents: vec![],
        salience,
    }
}

fn benchmark_retrieval(c: &mut Criterion) {
    let mut group = c.benchmark_group("manifold_retrieval");

    for num_patterns in [100, 500, 1000].iter() {
        let mut engine = create_test_engine();

        // Pre-populate with patterns
        for _ in 0..*num_patterns {
            let pattern = create_test_pattern(512, 0.7);
            engine.deform(pattern, 0.7).unwrap();
        }

        let query = vec![0.5; 512];

        group.bench_with_input(
            BenchmarkId::from_parameter(num_patterns),
            num_patterns,
            |b, _| {
                b.iter(|| engine.retrieve(black_box(&query), black_box(10)));
            },
        );
    }

    group.finish();
}

fn benchmark_deformation(c: &mut Criterion) {
    let mut group = c.benchmark_group("manifold_deformation");

    for batch_size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut engine = create_test_engine();
                    for _ in 0..size {
                        let pattern = create_test_pattern(512, 0.8);
                        engine.deform(black_box(pattern), black_box(0.8)).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_forgetting(c: &mut Criterion) {
    let mut engine = create_test_engine();

    // Pre-populate
    for i in 0..500 {
        let salience = if i < 100 { 0.9 } else { 0.3 };
        let pattern = create_test_pattern(512, salience);
        engine.deform(pattern, salience).unwrap();
    }

    c.bench_function("manifold_forgetting", |b| {
        b.iter(|| engine.forget(black_box(0.5), black_box(0.1)));
    });
}

criterion_group!(
    benches,
    benchmark_retrieval,
    benchmark_deformation,
    benchmark_forgetting
);
criterion_main!(benches);
