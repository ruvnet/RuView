use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_core::arena::Arena;
use ruvector_core::cache_optimized::SoAVectorStorage;
use ruvector_core::distance::*;
use ruvector_core::lockfree::{LockFreeCounter, LockFreeStats, ObjectPool};
use ruvector_core::simd_intrinsics::*;
use ruvector_core::types::DistanceMetric;
use std::sync::Arc;
use std::thread;

// Benchmark SIMD intrinsics vs SimSIMD
fn bench_simd_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_comparison");

    for size in [128, 384, 768, 1536].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32 * 0.1).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i + 1) as f32 * 0.1).collect();

        // Euclidean distance
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("euclidean_simsimd", size),
            size,
            |bench, _| {
                bench.iter(|| euclidean_distance(black_box(&a), black_box(&b)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("euclidean_avx2", size),
            size,
            |bench, _| {
                bench.iter(|| euclidean_distance_avx2(black_box(&a), black_box(&b)));
            },
        );

        // Dot product
        group.bench_with_input(
            BenchmarkId::new("dot_product_simsimd", size),
            size,
            |bench, _| {
                bench.iter(|| dot_product_distance(black_box(&a), black_box(&b)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("dot_product_avx2", size),
            size,
            |bench, _| {
                bench.iter(|| dot_product_avx2(black_box(&a), black_box(&b)));
            },
        );

        // Cosine similarity
        group.bench_with_input(
            BenchmarkId::new("cosine_simsimd", size),
            size,
            |bench, _| {
                bench.iter(|| cosine_distance(black_box(&a), black_box(&b)));
            },
        );

        group.bench_with_input(BenchmarkId::new("cosine_avx2", size), size, |bench, _| {
            bench.iter(|| cosine_similarity_avx2(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

// Benchmark Structure-of-Arrays vs Array-of-Structures
fn bench_cache_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_optimization");

    let dimensions = 384;
    let num_vectors = 10000;

    // Prepare data
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| (0..dimensions).map(|j| (i * j) as f32 * 0.001).collect())
        .collect();

    let query: Vec<f32> = (0..dimensions).map(|i| i as f32 * 0.01).collect();

    // Array-of-Structures (traditional Vec<Vec<f32>>)
    group.bench_function("aos_batch_distance", |bench| {
        bench.iter(|| {
            let mut distances: Vec<f32> = Vec::with_capacity(num_vectors);
            for vector in &vectors {
                let dist = euclidean_distance(black_box(&query), black_box(vector));
                distances.push(dist);
            }
            black_box(distances)
        });
    });

    // Structure-of-Arrays
    let mut soa_storage = SoAVectorStorage::new(dimensions, num_vectors);
    for vector in &vectors {
        soa_storage.push(vector);
    }

    group.bench_function("soa_batch_distance", |bench| {
        bench.iter(|| {
            let mut distances = vec![0.0; num_vectors];
            soa_storage.batch_euclidean_distances(black_box(&query), &mut distances);
            black_box(distances)
        });
    });

    group.finish();
}

// Benchmark arena allocation vs standard allocation
fn bench_arena_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_allocation");

    let num_allocations = 1000;
    let vec_size = 100;

    group.bench_function("standard_allocation", |bench| {
        bench.iter(|| {
            let mut vecs = Vec::new();
            for _ in 0..num_allocations {
                let mut v = Vec::with_capacity(vec_size);
                for j in 0..vec_size {
                    v.push(j as f32);
                }
                vecs.push(v);
            }
            black_box(vecs)
        });
    });

    group.bench_function("arena_allocation", |bench| {
        bench.iter(|| {
            let arena = Arena::new(1024 * 1024);
            let mut vecs = Vec::new();
            for _ in 0..num_allocations {
                let mut v = arena.alloc_vec::<f32>(vec_size);
                for j in 0..vec_size {
                    v.push(j as f32);
                }
                vecs.push(v);
            }
            black_box(vecs)
        });
    });

    group.finish();
}

// Benchmark lock-free operations vs locked operations
fn bench_lockfree(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree");

    // Counter benchmark
    group.bench_function("lockfree_counter_single_thread", |bench| {
        let counter = LockFreeCounter::new(0);
        bench.iter(|| {
            for _ in 0..10000 {
                counter.increment();
            }
        });
    });

    group.bench_function("lockfree_counter_multi_thread", |bench| {
        bench.iter(|| {
            let counter = Arc::new(LockFreeCounter::new(0));
            let mut handles = vec![];

            for _ in 0..4 {
                let counter_clone = Arc::clone(&counter);
                handles.push(thread::spawn(move || {
                    for _ in 0..2500 {
                        counter_clone.increment();
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(counter.get())
        });
    });

    // Stats collector benchmark
    group.bench_function("lockfree_stats", |bench| {
        let stats = LockFreeStats::new();
        bench.iter(|| {
            for i in 0..1000 {
                stats.record_query(i);
            }
            black_box(stats.snapshot())
        });
    });

    // Object pool benchmark
    group.bench_function("object_pool_acquire_release", |bench| {
        let pool = ObjectPool::new(10, || Vec::<f32>::with_capacity(1000));
        bench.iter(|| {
            let mut obj = pool.acquire();
            for i in 0..100 {
                obj.push(i as f32);
            }
            black_box(&*obj);
        });
    });

    group.finish();
}

// Benchmark thread scaling
fn bench_thread_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("thread_scaling");

    let dimensions = 384;
    let num_vectors = 10000;
    let query: Vec<f32> = (0..dimensions).map(|i| i as f32 * 0.01).collect();
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| (0..dimensions).map(|j| (i * j) as f32 * 0.001).collect())
        .collect();

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_distance", num_threads),
            num_threads,
            |bench, &threads| {
                bench.iter(|| {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| {
                            let result = batch_distances(
                                black_box(&query),
                                black_box(&vectors),
                                DistanceMetric::Euclidean,
                            )
                            .unwrap();
                            black_box(result)
                        })
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_simd_comparison,
    bench_cache_optimization,
    bench_arena_allocation,
    bench_lockfree,
    bench_thread_scaling
);
criterion_main!(benches);
