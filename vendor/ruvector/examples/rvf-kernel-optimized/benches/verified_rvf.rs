//! Benchmarks for the verified RVF pipeline.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_proof_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("proof_generation");
    for dim in [128u32, 384, 768, 1536] {
        group.bench_with_input(BenchmarkId::new("prove_dim_eq", dim), &dim, |b, &d| {
            b.iter(|| {
                let mut env = ruvector_verified::ProofEnvironment::new();
                ruvector_verified::prove_dim_eq(&mut env, d, d).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_arena_intern(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_intern");
    group.bench_function("cold_100_misses", |b| {
        b.iter(|| {
            let arena = ruvector_verified::fast_arena::FastTermArena::new();
            for i in 0..100u64 {
                arena.intern(i);
            }
        });
    });
    group.bench_function("hot_100_hits", |b| {
        let arena = ruvector_verified::fast_arena::FastTermArena::new();
        arena.intern(42);
        b.iter(|| {
            for _ in 0..100 {
                arena.intern(42);
            }
        });
    });
    group.finish();
}

fn bench_gated_routing(c: &mut Criterion) {
    use ruvector_verified::gated::{self, ProofKind};

    let mut group = c.benchmark_group("gated_routing");
    let env = ruvector_verified::ProofEnvironment::new();
    group.bench_function("reflexivity", |b| {
        b.iter(|| gated::route_proof(ProofKind::Reflexivity, &env));
    });
    group.bench_function("dimension_equality", |b| {
        b.iter(|| {
            gated::route_proof(
                ProofKind::DimensionEquality {
                    expected: 384,
                    actual: 384,
                },
                &env,
            )
        });
    });
    group.bench_function("pipeline_composition", |b| {
        b.iter(|| gated::route_proof(ProofKind::PipelineComposition { stages: 5 }, &env));
    });
    group.finish();
}

fn bench_conversion_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("conversion_cache");
    group.bench_function("insert_1000", |b| {
        b.iter(|| {
            let mut cache = ruvector_verified::cache::ConversionCache::with_capacity(2048);
            for i in 0..1000u32 {
                cache.insert(i, 384, i + 1000);
            }
        });
    });
    group.bench_function("lookup_hit_1000", |b| {
        let mut cache = ruvector_verified::cache::ConversionCache::with_capacity(2048);
        for i in 0..1000u32 {
            cache.insert(i, 384, i + 1000);
        }
        b.iter(|| {
            for i in 0..1000u32 {
                cache.get(i, 384);
            }
        });
    });
    group.finish();
}

fn bench_attestation(c: &mut Criterion) {
    let mut group = c.benchmark_group("attestation");
    group.bench_function("create_and_serialize", |b| {
        let env = ruvector_verified::ProofEnvironment::new();
        b.iter(|| {
            let att = ruvector_verified::proof_store::create_attestation(&env, 0);
            att.to_bytes()
        });
    });
    group.bench_function("roundtrip", |b| {
        let env = ruvector_verified::ProofEnvironment::new();
        let att = ruvector_verified::proof_store::create_attestation(&env, 0);
        let bytes = att.to_bytes();
        b.iter(|| ruvector_verified::ProofAttestation::from_bytes(&bytes).unwrap());
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_proof_generation,
    bench_arena_intern,
    bench_gated_routing,
    bench_conversion_cache,
    bench_attestation,
);
criterion_main!(benches);
