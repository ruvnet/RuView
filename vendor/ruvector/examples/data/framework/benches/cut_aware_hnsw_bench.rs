//! Benchmarks for Cut-Aware HNSW
//!
//! Compares performance of gated vs ungated search, and demonstrates
//! the trade-offs between coherence-aware navigation and raw search speed.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruvector_data_framework::cut_aware_hnsw::{CutAwareHNSW, CutAwareConfig};

fn create_test_index(n: usize, dimension: usize) -> CutAwareHNSW {
    let config = CutAwareConfig {
        m: 16,
        ef_construction: 100,
        ef_search: 50,
        coherence_gate_threshold: 0.3,
        max_cross_cut_hops: 2,
        enable_cut_pruning: false,
        cut_recompute_interval: 50,
        min_zone_size: 5,
    };

    let mut index = CutAwareHNSW::new(config);

    // Insert vectors in two clusters
    for i in 0..n {
        let mut vec = vec![0.0; dimension];

        if i < n / 2 {
            // First cluster - high values
            for j in 0..dimension {
                vec[j] = 1.0 + (i as f32 * 0.01) + (j as f32 * 0.001);
            }
        } else {
            // Second cluster - low values
            for j in 0..dimension {
                vec[j] = -1.0 + (i as f32 * 0.01) + (j as f32 * 0.001);
            }
        }

        index.insert(i as u32, &vec).unwrap();
    }

    index
}

fn bench_gated_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("gated_search");

    for size in [100, 500, 1000].iter() {
        let index = create_test_index(*size, 128);
        let query = vec![1.0; 128];

        group.bench_with_input(
            BenchmarkId::new("gated", size),
            size,
            |b, _| {
                b.iter(|| {
                    let results = index.search_gated(black_box(&query), black_box(10));
                    black_box(results);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("ungated", size),
            size,
            |b, _| {
                b.iter(|| {
                    let results = index.search_ungated(black_box(&query), black_box(10));
                    black_box(results);
                });
            },
        );
    }

    group.finish();
}

fn bench_coherent_neighborhood(c: &mut Criterion) {
    let mut group = c.benchmark_group("coherent_neighborhood");

    for size in [100, 500].iter() {
        let index = create_test_index(*size, 128);

        group.bench_with_input(
            BenchmarkId::new("radius_2", size),
            size,
            |b, _| {
                b.iter(|| {
                    let neighbors = index.coherent_neighborhood(black_box(0), black_box(2));
                    black_box(neighbors);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("radius_5", size),
            size,
            |b, _| {
                b.iter(|| {
                    let neighbors = index.coherent_neighborhood(black_box(0), black_box(5));
                    black_box(neighbors);
                });
            },
        );
    }

    group.finish();
}

fn bench_zone_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("zone_computation");

    for size in [100, 500, 1000].iter() {
        let mut index = create_test_index(*size, 128);

        group.bench_with_input(
            BenchmarkId::new("compute_zones", size),
            size,
            |b, _| {
                b.iter(|| {
                    let zones = index.compute_zones();
                    black_box(zones);
                });
            },
        );
    }

    group.finish();
}

fn bench_batch_updates(c: &mut Criterion) {
    use ruvector_data_framework::cut_aware_hnsw::{EdgeUpdate, UpdateKind};

    let mut group = c.benchmark_group("batch_updates");

    for batch_size in [10, 50, 100].iter() {
        let mut index = create_test_index(100, 128);

        let updates: Vec<EdgeUpdate> = (0..*batch_size)
            .map(|i| EdgeUpdate {
                kind: UpdateKind::Insert,
                u: i as u32,
                v: (i + 1) as u32,
                weight: Some(0.8),
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("batch_update", batch_size),
            batch_size,
            |b, _| {
                b.iter(|| {
                    let stats = index.batch_update(black_box(updates.clone()));
                    black_box(stats);
                });
            },
        );
    }

    group.finish();
}

fn bench_cross_zone_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_zone_search");

    let mut index = create_test_index(500, 128);
    index.compute_zones(); // Ensure zones are computed

    let query = vec![0.0; 128]; // Neutral query

    group.bench_function("single_zone", |b| {
        b.iter(|| {
            let results = index.cross_zone_search(black_box(&query), black_box(10), &[0]);
            black_box(results);
        });
    });

    group.bench_function("two_zones", |b| {
        b.iter(|| {
            let results = index.cross_zone_search(black_box(&query), black_box(10), &[0, 1]);
            black_box(results);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_gated_search,
    bench_coherent_neighborhood,
    bench_zone_computation,
    bench_batch_updates,
    bench_cross_zone_search,
);
criterion_main!(benches);
