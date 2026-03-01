//! Benchmarks for GNN operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ndarray::Array2;

// Note: These benchmarks require the crate to compile successfully.
// They test the core GNN operations for performance regression.

fn create_test_features(n: usize, dim: usize) -> Array2<f32> {
    Array2::from_elem((n, dim), 0.5)
}

fn create_test_adjacency(n: usize) -> Array2<f32> {
    let mut adj = Array2::<f32>::eye(n);
    // Add some random edges
    for i in 0..n.saturating_sub(1) {
        adj[[i, i + 1]] = 0.5;
        adj[[i + 1, i]] = 0.5;
    }
    adj
}

fn benchmark_gcn_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("gcn_forward");

    for n in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            let features = create_test_features(n, 64);
            let adj = create_test_adjacency(n);

            b.iter(|| {
                // Simple matrix multiplication to simulate GCN forward
                let aggregated = black_box(&adj).dot(black_box(&features));
                black_box(aggregated)
            });
        });
    }

    group.finish();
}

fn benchmark_attention_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention");

    for n in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            let query = create_test_features(n, 64);
            let key = create_test_features(n, 64);

            b.iter(|| {
                // Compute attention scores
                let scores = black_box(&query).dot(&black_box(&key).t());

                // Softmax (simplified)
                let mut result = scores.clone();
                for mut row in result.rows_mut() {
                    let max = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let sum: f32 = row.iter().map(|x| (x - max).exp()).sum();
                    for x in row.iter_mut() {
                        *x = (*x - max).exp() / sum;
                    }
                }
                black_box(result)
            });
        });
    }

    group.finish();
}

fn benchmark_cosine_similarity(c: &mut Criterion) {
    c.bench_function("cosine_similarity_256d", |bencher| {
        let vec_a: Vec<f32> = (0..256).map(|i| (i as f32).sin()).collect();
        let vec_b: Vec<f32> = (0..256).map(|i| (i as f32).cos()).collect();

        bencher.iter(|| {
            let dot: f32 = black_box(&vec_a).iter().zip(black_box(&vec_b)).map(|(x, y)| x * y).sum();
            let norm_a: f32 = vec_a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm_b: f32 = vec_b.iter().map(|x| x * x).sum::<f32>().sqrt();
            black_box(dot / (norm_a * norm_b))
        });
    });
}

fn benchmark_info_nce_loss(c: &mut Criterion) {
    c.bench_function("info_nce_10_negatives", |b| {
        let anchor: Vec<f32> = (0..128).map(|i| (i as f32 * 0.01).sin()).collect();
        let positive: Vec<f32> = (0..128).map(|i| (i as f32 * 0.01).sin() + 0.1).collect();
        let negatives: Vec<Vec<f32>> = (0..10)
            .map(|j| (0..128).map(|i| ((i + j * 10) as f32 * 0.01).cos()).collect())
            .collect();
        let neg_refs: Vec<&[f32]> = negatives.iter().map(|v| v.as_slice()).collect();

        b.iter(|| {
            let temp = 0.07;

            // Compute cosine similarities
            let pos_sim = {
                let dot: f32 = black_box(&anchor).iter().zip(black_box(&positive)).map(|(x, y)| x * y).sum();
                let norm_a: f32 = anchor.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b: f32 = positive.iter().map(|x| x * x).sum::<f32>().sqrt();
                dot / (norm_a * norm_b) / temp
            };

            let neg_sims: Vec<f32> = neg_refs.iter().map(|neg| {
                let dot: f32 = anchor.iter().zip(*neg).map(|(x, y)| x * y).sum();
                let norm_a: f32 = anchor.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b: f32 = neg.iter().map(|x| x * x).sum::<f32>().sqrt();
                dot / (norm_a * norm_b) / temp
            }).collect();

            // Log-sum-exp
            let max_sim = neg_sims.iter().chain(std::iter::once(&pos_sim))
                .cloned().fold(f32::NEG_INFINITY, f32::max);
            let sum_exp: f32 = std::iter::once(pos_sim).chain(neg_sims)
                .map(|s| (s - max_sim).exp()).sum();

            black_box(-pos_sim + max_sim + sum_exp.ln())
        });
    });
}

criterion_group!(
    benches,
    benchmark_gcn_forward,
    benchmark_attention_computation,
    benchmark_cosine_similarity,
    benchmark_info_nce_loss,
);

criterion_main!(benches);
