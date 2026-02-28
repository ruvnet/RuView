use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use hyperbolic_attention::prelude::*;
use hyperbolic_attention::HyperbolicTransformerBlock;

// =============================================================================
// POINCARÉ BALL BENCHMARKS
// =============================================================================

fn bench_poincare_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("poincare_distance");

    for dim in [8, 16, 32, 64, 128, 256, 512] {
        group.throughput(Throughput::Elements(1));

        let x: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let y: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01 + 0.1).collect();
        let k = 1.0;

        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, _| {
            b.iter(|| {
                black_box(poincare_distance(
                    black_box(&x),
                    black_box(&y),
                    black_box(k),
                ))
            });
        });
    }

    group.finish();
}

fn bench_mobius_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("mobius_add");

    for dim in [8, 16, 32, 64, 128, 256] {
        group.throughput(Throughput::Elements(1));

        let x: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let y: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01 + 0.05).collect();
        let k = 1.0;

        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, _| {
            b.iter(|| black_box(mobius_add(black_box(&x), black_box(&y), black_box(k))));
        });
    }

    group.finish();
}

fn bench_exponential_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("exponential_map");

    for dim in [8, 16, 32, 64, 128] {
        group.throughput(Throughput::Elements(1));

        let x: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let v: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.001).collect();
        let k = 1.0;

        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, _| {
            b.iter(|| black_box(exponential_map(black_box(&x), black_box(&v), black_box(k))));
        });
    }

    group.finish();
}

fn bench_batch_distances(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_poincare_distances");

    for (dim, db_size) in [(16, 100), (16, 1000), (64, 100), (128, 100)] {
        group.throughput(Throughput::Elements(db_size as u64));

        let query: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let database: Vec<Vec<f32>> = (0..db_size)
            .map(|j| (0..dim).map(|i| (i as f32 + j as f32) * 0.001).collect())
            .collect();
        let k = 1.0;

        let label = format!("dim{}_db{}", dim, db_size);
        group.bench_with_input(BenchmarkId::from_parameter(&label), &label, |b, _| {
            b.iter(|| {
                black_box(batch_poincare_distances(
                    black_box(&query),
                    black_box(&database),
                    black_box(k),
                ))
            });
        });
    }

    group.finish();
}

// =============================================================================
// LORENTZ MODEL BENCHMARKS
// =============================================================================

fn bench_lorentz_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("lorentz_distance");

    for dim in [8, 16, 32, 64, 128, 256] {
        group.throughput(Throughput::Elements(1));

        let spatial_x: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let spatial_y: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01 + 0.1).collect();
        let k = 1.0;

        let x = poincare_to_lorentz(&spatial_x, k);
        let y = poincare_to_lorentz(&spatial_y, k);

        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, _| {
            b.iter(|| black_box(lorentz_distance(black_box(&x), black_box(&y), black_box(k))));
        });
    }

    group.finish();
}

fn bench_lorentz_exp(c: &mut Criterion) {
    let mut group = c.benchmark_group("lorentz_exp");

    for dim in [8, 16, 32, 64, 128] {
        group.throughput(Throughput::Elements(1));

        let spatial: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let k = 1.0;

        let x = poincare_to_lorentz(&spatial, k);
        let v: Vec<f32> = std::iter::once(0.0)
            .chain((0..dim).map(|i| (i as f32) * 0.001))
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(dim), &dim, |b, _| {
            b.iter(|| black_box(lorentz_exp(black_box(&x), black_box(&v), black_box(k))));
        });
    }

    group.finish();
}

// =============================================================================
// ATTENTION BENCHMARKS
// =============================================================================

fn bench_hyperbolic_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_attention");

    for (dim, seq_len, num_heads) in [(64, 8, 2), (64, 16, 2), (128, 16, 4), (256, 16, 8)] {
        group.throughput(Throughput::Elements(seq_len as u64));

        let config = HyperbolicAttentionConfig::new(dim, num_heads, 1.0);
        let attention = HyperbolicAttention::new(config);

        let inputs: Vec<Vec<f32>> = (0..seq_len)
            .map(|j| (0..dim).map(|i| ((i + j) as f32) * 0.001).collect())
            .collect();

        let label = format!("d{}_s{}_h{}", dim, seq_len, num_heads);
        group.bench_with_input(BenchmarkId::from_parameter(&label), &label, |b, _| {
            b.iter(|| {
                black_box(attention.forward(
                    black_box(&inputs),
                    black_box(&inputs),
                    black_box(&inputs),
                ))
            });
        });
    }

    group.finish();
}

fn bench_multi_head_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_head_hyperbolic_attention");

    for (dim, seq_len, num_heads) in [(128, 8, 4), (128, 16, 4), (256, 16, 8)] {
        group.throughput(Throughput::Elements(seq_len as u64));

        let config = HyperbolicAttentionConfig::new(dim, num_heads, 1.0);
        let attention = MultiHeadHyperbolicAttention::new(config);

        let inputs: Vec<Vec<f32>> = (0..seq_len)
            .map(|j| (0..dim).map(|i| ((i + j) as f32) * 0.001).collect())
            .collect();

        let label = format!("d{}_s{}_h{}", dim, seq_len, num_heads);
        group.bench_with_input(BenchmarkId::from_parameter(&label), &label, |b, _| {
            b.iter(|| {
                black_box(attention.forward(
                    black_box(&inputs),
                    black_box(&inputs),
                    black_box(&inputs),
                ))
            });
        });
    }

    group.finish();
}

fn bench_transformer_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_transformer_block");

    for (dim, seq_len, num_heads) in [(64, 8, 2), (128, 16, 4), (256, 16, 8)] {
        group.throughput(Throughput::Elements(seq_len as u64));

        let block = HyperbolicTransformerBlock::new(dim, num_heads, 1.0);

        let inputs: Vec<Vec<f32>> = (0..seq_len)
            .map(|j| (0..dim).map(|i| ((i + j) as f32) * 0.001).collect())
            .collect();

        let label = format!("d{}_s{}_h{}", dim, seq_len, num_heads);
        group.bench_with_input(BenchmarkId::from_parameter(&label), &label, |b, _| {
            b.iter(|| black_box(block.forward(black_box(&inputs))));
        });
    }

    group.finish();
}

// =============================================================================
// CURVATURE ADAPTATION BENCHMARKS
// =============================================================================

fn bench_learnable_curvature(c: &mut Criterion) {
    let mut group = c.benchmark_group("learnable_curvature");

    group.bench_function("update", |b| {
        let mut curvature = LearnableCurvature::new(1.0);
        b.iter(|| {
            curvature.update(black_box(0.01));
            black_box(curvature.value());
        });
    });

    group.finish();
}

fn bench_multi_curvature(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_curvature");

    for num_components in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_components),
            &num_components,
            |b, &n| {
                let mut multi = MultiCurvature::new(n, 1.0);
                let grads: Vec<f32> = (0..n).map(|i| (i as f32) * 0.01).collect();

                b.iter(|| {
                    multi.update(black_box(&grads));
                    black_box(multi.values());
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// SIMD OPTIMIZATION BENCHMARKS
// =============================================================================

fn bench_simd_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_operations");

    for dim in [8, 16, 32, 64, 128, 256, 512, 1024] {
        group.throughput(Throughput::Elements(dim as u64));

        let a: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.02).collect();

        use hyperbolic_attention::poincare_embedding::dot_product_simd;

        group.bench_with_input(BenchmarkId::new("dot_product", dim), &dim, |bench, _| {
            bench.iter(|| black_box(dot_product_simd(black_box(&a), black_box(&b))));
        });
    }

    group.finish();
}

// =============================================================================
// CRITERION GROUPS
// =============================================================================

criterion_group!(
    poincare_benches,
    bench_poincare_distance,
    bench_mobius_add,
    bench_exponential_map,
    bench_batch_distances,
);

criterion_group!(lorentz_benches, bench_lorentz_distance, bench_lorentz_exp,);

criterion_group!(
    attention_benches,
    bench_hyperbolic_attention,
    bench_multi_head_attention,
    bench_transformer_block,
);

criterion_group!(
    curvature_benches,
    bench_learnable_curvature,
    bench_multi_curvature,
);

criterion_group!(simd_benches, bench_simd_dot_product,);

criterion_main!(
    poincare_benches,
    lorentz_benches,
    attention_benches,
    curvature_benches,
    simd_benches,
);
