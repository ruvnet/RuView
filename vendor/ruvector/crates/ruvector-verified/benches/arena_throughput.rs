//! Arena throughput benchmarks.
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_env_alloc_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("env_alloc_sequential");
    for count in [100, 1000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut env = ruvector_verified::ProofEnvironment::new();
                for _ in 0..count {
                    env.alloc_term();
                }
            });
        });
    }
    group.finish();
}

fn bench_env_cache_throughput(c: &mut Criterion) {
    c.bench_function("cache_insert_1000", |b| {
        b.iter(|| {
            let mut env = ruvector_verified::ProofEnvironment::new();
            for i in 0..1000u64 {
                env.cache_insert(i, i as u32);
            }
        });
    });
}

fn bench_env_cache_lookup_hit(c: &mut Criterion) {
    c.bench_function("cache_lookup_1000_hits", |b| {
        let mut env = ruvector_verified::ProofEnvironment::new();
        for i in 0..1000u64 {
            env.cache_insert(i, i as u32);
        }
        b.iter(|| {
            for i in 0..1000u64 {
                env.cache_lookup(i);
            }
        });
    });
}

fn bench_env_reset(c: &mut Criterion) {
    c.bench_function("env_reset", |b| {
        let mut env = ruvector_verified::ProofEnvironment::new();
        for i in 0..1000u64 {
            env.cache_insert(i, i as u32);
        }
        env.alloc_term();
        b.iter(|| {
            env.reset();
        });
    });
}

fn bench_pool_acquire_release(c: &mut Criterion) {
    c.bench_function("pool_acquire_release", |b| {
        b.iter(|| {
            let _res = ruvector_verified::pools::acquire();
            // auto-returns on drop
        });
    });
}

fn bench_attestation_roundtrip(c: &mut Criterion) {
    c.bench_function("attestation_roundtrip", |b| {
        let att = ruvector_verified::ProofAttestation::new([1u8; 32], [2u8; 32], 42, 9500);
        b.iter(|| {
            let bytes = att.to_bytes();
            ruvector_verified::proof_store::ProofAttestation::from_bytes(&bytes).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_env_alloc_sequential,
    bench_env_cache_throughput,
    bench_env_cache_lookup_hit,
    bench_env_reset,
    bench_pool_acquire_release,
    bench_attestation_roundtrip,
);
criterion_main!(benches);
