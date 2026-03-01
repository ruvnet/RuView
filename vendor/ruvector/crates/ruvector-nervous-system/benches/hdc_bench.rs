use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_nervous_system::hdc::{bind, bundle, HdcMemory, Hypervector};

fn bench_vector_creation(c: &mut Criterion) {
    c.bench_function("hypervector_random", |b| {
        b.iter(|| {
            black_box(Hypervector::random());
        });
    });

    c.bench_function("hypervector_from_seed", |b| {
        b.iter(|| {
            black_box(Hypervector::from_seed(42));
        });
    });
}

fn bench_binding(c: &mut Criterion) {
    let v1 = Hypervector::random();
    let v2 = Hypervector::random();

    c.bench_function("bind_two_vectors", |b| {
        b.iter(|| {
            black_box(v1.bind(&v2));
        });
    });

    c.bench_function("bind_function", |b| {
        b.iter(|| {
            black_box(bind(&v1, &v2));
        });
    });
}

fn bench_bundling(c: &mut Criterion) {
    let mut group = c.benchmark_group("bundling");

    for size in [3, 5, 10, 20, 50].iter() {
        let vectors: Vec<_> = (0..*size).map(|_| Hypervector::random()).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(bundle(&vectors).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_similarity(c: &mut Criterion) {
    let v1 = Hypervector::random();
    let v2 = Hypervector::random();

    c.bench_function("similarity", |b| {
        b.iter(|| {
            black_box(v1.similarity(&v2));
        });
    });

    c.bench_function("hamming_distance", |b| {
        b.iter(|| {
            black_box(v1.hamming_distance(&v2));
        });
    });
}

fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    // Store operation
    group.bench_function("store_single", |b| {
        let mut memory = HdcMemory::new();
        let vector = Hypervector::random();
        let mut i = 0;

        b.iter(|| {
            memory.store(format!("key_{}", i), vector.clone());
            i += 1;
        });
    });

    // Retrieve with different memory sizes
    for size in [10, 100, 1000, 10_000].iter() {
        let mut memory = HdcMemory::with_capacity(*size);

        for i in 0..*size {
            memory.store(format!("key_{}", i), Hypervector::random());
        }

        let query = Hypervector::random();

        group.bench_with_input(BenchmarkId::new("retrieve", size), size, |b, _| {
            b.iter(|| {
                black_box(memory.retrieve(&query, 0.8));
            });
        });

        group.bench_with_input(BenchmarkId::new("retrieve_top_k", size), size, |b, _| {
            b.iter(|| {
                black_box(memory.retrieve_top_k(&query, 10));
            });
        });
    }

    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    c.bench_function("end_to_end_workflow", |b| {
        b.iter(|| {
            let mut memory = HdcMemory::new();

            // Create and store 100 vectors
            for i in 0..100 {
                let v = Hypervector::random();
                memory.store(format!("item_{}", i), v);
            }

            // Retrieve similar items
            let query = Hypervector::random();
            let results = memory.retrieve_top_k(&query, 10);

            black_box(results);
        });
    });
}

criterion_group!(
    benches,
    bench_vector_creation,
    bench_binding,
    bench_bundling,
    bench_similarity,
    bench_memory_operations,
    bench_end_to_end
);

criterion_main!(benches);
