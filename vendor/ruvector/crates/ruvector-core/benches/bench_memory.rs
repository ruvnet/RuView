//! Memory Allocation and Pool Benchmarks
//!
//! This module benchmarks arena allocation, cache-optimized storage,
//! and memory access patterns.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_core::arena::Arena;
use ruvector_core::cache_optimized::SoAVectorStorage;

// ============================================================================
// Arena Allocation Benchmarks
// ============================================================================

fn bench_arena_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_allocation");

    for count in [10, 100, 1000, 10000] {
        group.throughput(Throughput::Elements(count));

        // Benchmark arena allocation
        group.bench_with_input(BenchmarkId::new("arena", count), &count, |bench, &count| {
            bench.iter(|| {
                let arena = Arena::new(1024 * 1024);
                for _ in 0..count {
                    let _vec = arena.alloc_vec::<f32>(black_box(64));
                }
            });
        });

        // Compare with standard Vec allocation
        group.bench_with_input(
            BenchmarkId::new("std_vec", count),
            &count,
            |bench, &count| {
                bench.iter(|| {
                    let mut vecs = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        vecs.push(Vec::<f32>::with_capacity(black_box(64)));
                    }
                    vecs
                });
            },
        );
    }

    group.finish();
}

fn bench_arena_allocation_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_allocation_sizes");

    for size in [8, 32, 64, 128, 256, 512, 1024, 4096] {
        group.throughput(Throughput::Bytes(size as u64 * 4)); // f32 = 4 bytes

        group.bench_with_input(BenchmarkId::new("alloc", size), &size, |bench, &size| {
            bench.iter(|| {
                let arena = Arena::new(1024 * 1024);
                for _ in 0..1000 {
                    let _vec = arena.alloc_vec::<f32>(black_box(size));
                }
            });
        });
    }

    group.finish();
}

fn bench_arena_reset_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_reset_reuse");

    for iterations in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("with_reset", iterations),
            &iterations,
            |bench, &iterations| {
                bench.iter(|| {
                    let arena = Arena::new(1024 * 1024);
                    for _ in 0..iterations {
                        // Allocate
                        for _ in 0..100 {
                            let _vec = arena.alloc_vec::<f32>(64);
                        }
                        // Reset for reuse
                        arena.reset();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("without_reset", iterations),
            &iterations,
            |bench, &iterations| {
                bench.iter(|| {
                    for _ in 0..iterations {
                        let arena = Arena::new(1024 * 1024);
                        for _ in 0..100 {
                            let _vec = arena.alloc_vec::<f32>(64);
                        }
                        // No reset, create new arena each time
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_arena_push_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_push");

    for count in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(count));

        // Arena push
        group.bench_with_input(BenchmarkId::new("arena", count), &count, |bench, &count| {
            bench.iter(|| {
                let arena = Arena::new(1024 * 1024);
                let mut vec = arena.alloc_vec::<f32>(count as usize);
                for i in 0..count {
                    vec.push(black_box(i as f32));
                }
                vec
            });
        });

        // Standard Vec push
        group.bench_with_input(
            BenchmarkId::new("std_vec", count),
            &count,
            |bench, &count| {
                bench.iter(|| {
                    let mut vec = Vec::with_capacity(count as usize);
                    for i in 0..count {
                        vec.push(black_box(i as f32));
                    }
                    vec
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// SoA Vector Storage Benchmarks
// ============================================================================

fn bench_soa_storage_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_storage_push");

    for dim in [64, 128, 256, 384, 512, 768] {
        let vector: Vec<f32> = (0..dim).map(|i| i as f32 * 0.01).collect();

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("soa", dim), &dim, |bench, _| {
            bench.iter(|| {
                let mut storage = SoAVectorStorage::new(dim, 128);
                for _ in 0..1000 {
                    storage.push(black_box(&vector));
                }
                storage
            });
        });

        // Compare with Vec<Vec<f32>>
        group.bench_with_input(BenchmarkId::new("vec_of_vec", dim), &dim, |bench, _| {
            bench.iter(|| {
                let mut storage: Vec<Vec<f32>> = Vec::with_capacity(1000);
                for _ in 0..1000 {
                    storage.push(black_box(vector.clone()));
                }
                storage
            });
        });
    }

    group.finish();
}

fn bench_soa_storage_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_storage_get");

    for dim in [128, 384, 768] {
        let mut storage = SoAVectorStorage::new(dim, 128);

        for i in 0..10000 {
            let vector: Vec<f32> = (0..dim).map(|j| (i * dim + j) as f32 * 0.001).collect();
            storage.push(&vector);
        }

        let mut output = vec![0.0_f32; dim];

        group.bench_with_input(BenchmarkId::new("sequential", dim), &dim, |bench, _| {
            bench.iter(|| {
                for i in 0..10000 {
                    storage.get(black_box(i), &mut output);
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("random", dim), &dim, |bench, _| {
            let indices: Vec<usize> = (0..10000).map(|i| (i * 37 + 13) % 10000).collect();
            bench.iter(|| {
                for &idx in &indices {
                    storage.get(black_box(idx), &mut output);
                }
            });
        });
    }

    group.finish();
}

fn bench_soa_dimension_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_dimension_slice");

    for dim in [64, 128, 256, 512] {
        let mut storage = SoAVectorStorage::new(dim, 128);

        for i in 0..10000 {
            let vector: Vec<f32> = (0..dim).map(|j| (i * dim + j) as f32 * 0.001).collect();
            storage.push(&vector);
        }

        group.bench_with_input(
            BenchmarkId::new("access_all_dims", dim),
            &dim,
            |bench, &dim| {
                bench.iter(|| {
                    let mut sum = 0.0_f32;
                    for d in 0..dim {
                        let slice = storage.dimension_slice(black_box(d));
                        sum += slice.iter().sum::<f32>();
                    }
                    sum
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("access_single_dim", dim),
            &dim,
            |bench, _| {
                bench.iter(|| {
                    let slice = storage.dimension_slice(black_box(0));
                    slice.iter().sum::<f32>()
                });
            },
        );
    }

    group.finish();
}

fn bench_soa_batch_distances(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_batch_distances");

    for (dim, count) in [
        (128, 1000),
        (384, 1000),
        (768, 1000),
        (128, 10000),
        (384, 5000),
    ] {
        let mut storage = SoAVectorStorage::new(dim, 128);

        for i in 0..count {
            let vector: Vec<f32> = (0..dim)
                .map(|j| ((i * dim + j) % 1000) as f32 * 0.001)
                .collect();
            storage.push(&vector);
        }

        let query: Vec<f32> = (0..dim).map(|j| j as f32 * 0.002).collect();
        let mut distances = vec![0.0_f32; count];

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new(format!("{}d_x{}", dim, count), dim),
            &dim,
            |bench, _| {
                bench.iter(|| {
                    storage.batch_euclidean_distances(black_box(&query), &mut distances);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Memory Access Pattern Benchmarks
// ============================================================================

fn bench_memory_layout_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_layout");

    let dim = 384;
    let count = 10000;

    // SoA layout
    let mut soa_storage = SoAVectorStorage::new(dim, 128);
    for i in 0..count {
        let vector: Vec<f32> = (0..dim)
            .map(|j| ((i * dim + j) % 1000) as f32 * 0.001)
            .collect();
        soa_storage.push(&vector);
    }

    // AoS layout (Vec<Vec<f32>>)
    let aos_storage: Vec<Vec<f32>> = (0..count)
        .map(|i| {
            (0..dim)
                .map(|j| ((i * dim + j) % 1000) as f32 * 0.001)
                .collect()
        })
        .collect();

    let query: Vec<f32> = (0..dim).map(|j| j as f32 * 0.002).collect();
    let mut soa_distances = vec![0.0_f32; count];

    group.bench_function("soa_batch_euclidean", |bench| {
        bench.iter(|| {
            soa_storage.batch_euclidean_distances(black_box(&query), &mut soa_distances);
        });
    });

    group.bench_function("aos_naive_euclidean", |bench| {
        bench.iter(|| {
            let distances: Vec<f32> = aos_storage
                .iter()
                .map(|v| {
                    query
                        .iter()
                        .zip(v.iter())
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum::<f32>()
                        .sqrt()
                })
                .collect();
            distances
        });
    });

    group.finish();
}

fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    let dim = 512;

    // Test with different vector counts to observe cache effects
    for count in [100, 1000, 10000, 50000] {
        let mut storage = SoAVectorStorage::new(dim, 128);

        for i in 0..count {
            let vector: Vec<f32> = (0..dim)
                .map(|j| ((i * dim + j) % 1000) as f32 * 0.001)
                .collect();
            storage.push(&vector);
        }

        let query: Vec<f32> = (0..dim).map(|j| j as f32 * 0.001).collect();
        let mut distances = vec![0.0_f32; count];

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_distance", count),
            &count,
            |bench, _| {
                bench.iter(|| {
                    storage.batch_euclidean_distances(black_box(&query), &mut distances);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Growth and Reallocation Benchmarks
// ============================================================================

fn bench_soa_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_growth");

    // Test growth from small initial capacity
    group.bench_function("grow_from_small", |bench| {
        bench.iter(|| {
            let mut storage = SoAVectorStorage::new(128, 4); // Very small initial
            for i in 0..10000 {
                let vector: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32 * 0.001).collect();
                storage.push(black_box(&vector));
            }
            storage
        });
    });

    // Test with pre-allocated capacity
    group.bench_function("preallocated", |bench| {
        bench.iter(|| {
            let mut storage = SoAVectorStorage::new(128, 16384); // Pre-allocate
            for i in 0..10000 {
                let vector: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32 * 0.001).collect();
                storage.push(black_box(&vector));
            }
            storage
        });
    });

    group.finish();
}

// ============================================================================
// Mixed Type Allocation Benchmarks
// ============================================================================

fn bench_arena_mixed_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_mixed_types");

    group.bench_function("mixed_allocations", |bench| {
        bench.iter(|| {
            let arena = Arena::new(1024 * 1024);
            for _ in 0..100 {
                let _f32_vec = arena.alloc_vec::<f32>(black_box(64));
                let _f64_vec = arena.alloc_vec::<f64>(black_box(32));
                let _u32_vec = arena.alloc_vec::<u32>(black_box(128));
                let _u8_vec = arena.alloc_vec::<u8>(black_box(256));
            }
        });
    });

    group.bench_function("uniform_allocations", |bench| {
        bench.iter(|| {
            let arena = Arena::new(1024 * 1024);
            for _ in 0..400 {
                let _f32_vec = arena.alloc_vec::<f32>(black_box(64));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    benches,
    bench_arena_allocation,
    bench_arena_allocation_sizes,
    bench_arena_reset_reuse,
    bench_arena_push_operations,
    bench_soa_storage_push,
    bench_soa_storage_get,
    bench_soa_dimension_slice,
    bench_soa_batch_distances,
    bench_memory_layout_comparison,
    bench_cache_efficiency,
    bench_soa_growth,
    bench_arena_mixed_types,
);

criterion_main!(benches);
