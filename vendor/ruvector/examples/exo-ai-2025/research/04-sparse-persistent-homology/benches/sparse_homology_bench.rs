use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use sparse_persistent_homology::*;

/// Generate random points in d-dimensional space
fn generate_random_points(n: usize, d: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| (0..d).map(|_| rng.gen_range(0.0..1.0)).collect())
        .collect()
}

/// Generate random filtration for testing
fn generate_random_filtration(n_vertices: usize) -> Filtration {
    let mut filt = Filtration::new();
    let mut rng = rand::thread_rng();

    // Add vertices
    for i in 0..n_vertices {
        filt.add_simplex(vec![i], rng.gen_range(0.0..1.0));
    }

    // Add edges
    for i in 0..n_vertices {
        for j in (i + 1)..n_vertices {
            if rng.gen_bool(0.3) {
                // 30% edge probability
                filt.add_simplex(vec![i, j], rng.gen_range(0.0..1.0));
            }
        }
    }

    filt
}

/// Benchmark distance matrix computation (scalar vs SIMD)
fn bench_distance_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("distance_matrix");

    for n in [10, 50, 100, 200].iter() {
        let points = generate_random_points(*n, 50);

        group.throughput(Throughput::Elements(*n as u64 * (*n as u64 - 1) / 2));
        group.bench_with_input(BenchmarkId::new("scalar", n), &points, |b, points| {
            b.iter(|| simd_filtration::euclidean_distance_matrix_scalar(black_box(points)));
        });

        group.bench_with_input(BenchmarkId::new("auto", n), &points, |b, points| {
            b.iter(|| simd_filtration::euclidean_distance_matrix(black_box(points)));
        });
    }

    group.finish();
}

/// Benchmark apparent pairs identification
fn bench_apparent_pairs(c: &mut Criterion) {
    let mut group = c.benchmark_group("apparent_pairs");

    for n in [10, 20, 50, 100].iter() {
        let filt = generate_random_filtration(*n);

        group.throughput(Throughput::Elements(filt.len() as u64));
        group.bench_with_input(BenchmarkId::new("standard", n), &filt, |b, filt| {
            b.iter(|| apparent_pairs::identify_apparent_pairs(black_box(filt)));
        });

        group.bench_with_input(BenchmarkId::new("fast", n), &filt, |b, filt| {
            b.iter(|| apparent_pairs::identify_apparent_pairs_fast(black_box(filt)));
        });
    }

    group.finish();
}

/// Benchmark sparse boundary matrix reduction
fn bench_matrix_reduction(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_reduction");

    for n in [10, 20, 30, 50].iter() {
        // Create a simple chain complex
        let mut boundaries = vec![vec![]; *n]; // n vertices
        let mut dimensions = vec![0; *n];

        // Add edges
        for i in 0..(n - 1) {
            boundaries.push(vec![i, i + 1]);
            dimensions.push(1);
        }

        group.throughput(Throughput::Elements(boundaries.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("standard_reduction", n),
            &(*n, boundaries.clone(), dimensions.clone()),
            |b, (_, boundaries, dimensions)| {
                b.iter(|| {
                    let mut matrix = SparseBoundaryMatrix::from_filtration(
                        boundaries.clone(),
                        dimensions.clone(),
                        vec![],
                    );
                    black_box(matrix.reduce())
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("cohomology_reduction", n),
            &(*n, boundaries.clone(), dimensions.clone()),
            |b, (_, boundaries, dimensions)| {
                b.iter(|| {
                    let mut matrix = SparseBoundaryMatrix::from_filtration(
                        boundaries.clone(),
                        dimensions.clone(),
                        vec![],
                    );
                    black_box(matrix.reduce_cohomology())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark persistence landscape computation
fn bench_persistence_landscape(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistence_landscape");

    for n in [10, 50, 100, 200].iter() {
        let features: Vec<_> = (0..*n)
            .map(|i| streaming_homology::PersistenceFeature {
                birth: i as f64 * 0.01,
                death: (i as f64 + 5.0) * 0.01,
                dimension: 1,
            })
            .collect();

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(
            BenchmarkId::new("landscape", n),
            &features,
            |b, features| {
                b.iter(|| {
                    persistence_vectors::PersistenceLandscape::from_features(black_box(features), 5)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("persistence_image", n),
            &features,
            |b, features| {
                b.iter(|| {
                    persistence_vectors::PersistenceImage::from_features(
                        black_box(features),
                        32,
                        0.1,
                    )
                });
            },
        );
    }

    group.finish();
}

/// Benchmark topological attention mechanism
fn bench_topological_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_attention");

    for n in [10, 50, 100, 200].iter() {
        let features: Vec<_> = (0..*n)
            .map(|i| streaming_homology::PersistenceFeature {
                birth: i as f64 * 0.01,
                death: (i as f64 + 5.0) * 0.01,
                dimension: 1,
            })
            .collect();

        let activations: Vec<f64> = (0..*n).map(|i| i as f64 * 0.1).collect();

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(
            BenchmarkId::new("compute_weights", n),
            &features,
            |b, features| {
                b.iter(|| {
                    topological_attention::TopologicalAttention::from_features(black_box(features))
                });
            },
        );

        let attention = topological_attention::TopologicalAttention::from_features(&features);
        group.bench_with_input(
            BenchmarkId::new("apply_attention", n),
            &activations,
            |b, activations| {
                b.iter(|| attention.apply(black_box(activations)));
            },
        );
    }

    group.finish();
}

/// Benchmark streaming persistence updates
fn bench_streaming_persistence(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_persistence");

    for window_size in [50, 100, 200].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("update", window_size),
            window_size,
            |b, &window_size| {
                let mut streaming = StreamingPersistence::new(window_size);
                let point = vec![0.5_f32; 10];
                let mut t = 0.0;

                b.iter(|| {
                    streaming.update(black_box(point.clone()), black_box(t));
                    t += 0.01;
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Betti number computation
fn bench_betti_numbers(c: &mut Criterion) {
    let mut group = c.benchmark_group("betti_numbers");

    for n in [10, 20, 50, 100].iter() {
        let mut boundaries = vec![vec![]; *n];
        let mut dimensions = vec![0; *n];

        for i in 0..(n - 1) {
            boundaries.push(vec![i, i + 1]);
            dimensions.push(1);
        }

        let matrix = SparseBoundaryMatrix::from_filtration(boundaries, dimensions, vec![]);

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::new("fast", n), &matrix, |b, matrix| {
            b.iter(|| betti::compute_betti_fast(black_box(matrix), 2));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_distance_matrix,
    bench_apparent_pairs,
    bench_matrix_reduction,
    bench_persistence_landscape,
    bench_topological_attention,
    bench_streaming_persistence,
    bench_betti_numbers,
);

criterion_main!(benches);
