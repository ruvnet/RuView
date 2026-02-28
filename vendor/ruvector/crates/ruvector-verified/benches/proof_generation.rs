//! Proof generation benchmarks.
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_prove_dim_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove_dim_eq");
    for dim in [32, 128, 384, 512, 1024, 4096] {
        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, &dim| {
            b.iter(|| {
                let mut env = ruvector_verified::ProofEnvironment::new();
                ruvector_verified::prove_dim_eq(&mut env, dim, dim).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_prove_dim_eq_cached(c: &mut Criterion) {
    c.bench_function("prove_dim_eq_cached_100x", |b| {
        b.iter(|| {
            let mut env = ruvector_verified::ProofEnvironment::new();
            for _ in 0..100 {
                ruvector_verified::prove_dim_eq(&mut env, 128, 128).unwrap();
            }
        });
    });
}

fn bench_mk_vector_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("mk_vector_type");
    for dim in [128, 384, 768, 1536] {
        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, &dim| {
            b.iter(|| {
                let mut env = ruvector_verified::ProofEnvironment::new();
                ruvector_verified::mk_vector_type(&mut env, dim).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_proof_env_creation(c: &mut Criterion) {
    c.bench_function("ProofEnvironment::new", |b| {
        b.iter(|| ruvector_verified::ProofEnvironment::new());
    });
}

fn bench_batch_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_verify");
    for count in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let vecs: Vec<Vec<f32>> = (0..count).map(|_| vec![0.0f32; 128]).collect();
            let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
            b.iter(|| {
                let mut env = ruvector_verified::ProofEnvironment::new();
                ruvector_verified::vector_types::verify_batch_dimensions(&mut env, 128, &refs)
                    .unwrap();
            });
        });
    }
    group.finish();
}

fn bench_pipeline_compose(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_compose");
    for stages in [2, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(stages),
            &stages,
            |b, &stages| {
                let chain: Vec<(String, u32, u32)> = (0..stages)
                    .map(|i| (format!("stage_{i}"), i as u32, (i + 1) as u32))
                    .collect();
                b.iter(|| {
                    let mut env = ruvector_verified::ProofEnvironment::new();
                    ruvector_verified::pipeline::compose_chain(&chain, &mut env).unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_prove_dim_eq,
    bench_prove_dim_eq_cached,
    bench_mk_vector_type,
    bench_proof_env_creation,
    bench_batch_verify,
    bench_pipeline_compose,
);
criterion_main!(benches);
