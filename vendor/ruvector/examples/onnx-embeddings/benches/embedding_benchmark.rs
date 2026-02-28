//! Benchmarks for ONNX embedding generation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::cell::RefCell;

fn embedding_benchmarks(c: &mut Criterion) {
    // Note: These benchmarks require the tokio runtime
    // Run with: cargo bench --features benchmark

    let rt = tokio::runtime::Runtime::new().unwrap();

    // Initialize embedder once (wrapped in RefCell for interior mutability)
    let embedder = RefCell::new(rt.block_on(async {
        ruvector_onnx_embeddings::Embedder::default_model()
            .await
            .expect("Failed to load model")
    }));

    let mut group = c.benchmark_group("embedding_generation");

    // Single text embedding
    group.bench_function("single_text", |b| {
        b.iter(|| {
            let _ = embedder.borrow_mut().embed_one(black_box("This is a test sentence for benchmarking."));
        });
    });

    // Batch embedding at different sizes
    for size in [1, 8, 16, 32, 64].iter() {
        let texts: Vec<String> = (0..*size)
            .map(|i| format!("Benchmark sentence number {} for testing.", i))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("batch", size),
            &texts,
            |b, texts| {
                b.iter(|| {
                    let _ = embedder.borrow_mut().embed(black_box(texts));
                });
            },
        );
    }

    // Large batch embedding
    let large_batch: Vec<String> = (0..100)
        .map(|i| format!("Large batch sentence {} for parallel benchmark.", i))
        .collect();

    group.bench_function("batch_100", |b| {
        b.iter(|| {
            let _ = embedder.borrow_mut().embed(black_box(&large_batch));
        });
    });

    group.finish();
}

fn pooling_benchmarks(c: &mut Criterion) {
    use ruvector_onnx_embeddings::{Pooler, PoolingStrategy};

    let mut group = c.benchmark_group("pooling");

    // Create test data
    let hidden_size = 384;
    let seq_length = 128;
    let batch_size = 32;

    let token_embeddings: Vec<Vec<f32>> = (0..batch_size)
        .map(|_| {
            (0..seq_length * hidden_size)
                .map(|i| (i as f32) * 0.001)
                .collect()
        })
        .collect();

    let attention_masks: Vec<Vec<i64>> = (0..batch_size)
        .map(|_| vec![1i64; seq_length])
        .collect();

    for strategy in [
        PoolingStrategy::Mean,
        PoolingStrategy::Cls,
        PoolingStrategy::Max,
        PoolingStrategy::MeanSqrtLen,
    ] {
        let pooler = Pooler::new(strategy, true);

        group.bench_with_input(
            BenchmarkId::new("strategy", format!("{:?}", strategy)),
            &(&token_embeddings, &attention_masks),
            |b, (tokens, masks)| {
                b.iter(|| {
                    pooler.pool(black_box(tokens), black_box(masks), seq_length, hidden_size)
                });
            },
        );
    }

    group.finish();
}

fn similarity_benchmarks(c: &mut Criterion) {
    use ruvector_onnx_embeddings::Pooler;

    let mut group = c.benchmark_group("similarity");

    // Create test vectors
    let dim = 384;
    let vec_a: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
    let vec_b: Vec<f32> = (0..dim).map(|i| ((dim - i) as f32) * 0.01).collect();

    group.bench_function("cosine_similarity_384d", |b| {
        b.iter(|| {
            Pooler::cosine_similarity(black_box(&vec_a), black_box(&vec_b))
        });
    });

    group.bench_function("dot_product_384d", |b| {
        b.iter(|| {
            Pooler::dot_product(black_box(&vec_a), black_box(&vec_b))
        });
    });

    group.bench_function("euclidean_distance_384d", |b| {
        b.iter(|| {
            Pooler::euclidean_distance(black_box(&vec_a), black_box(&vec_b))
        });
    });

    // Batch similarity
    let candidates: Vec<Vec<f32>> = (0..1000)
        .map(|i| (0..dim).map(|j| ((i + j) as f32) * 0.001).collect())
        .collect();

    group.bench_function("batch_cosine_1000", |b| {
        b.iter(|| {
            ruvector_onnx_embeddings::pooling::batch_cosine_similarity(
                black_box(&vec_a),
                black_box(&candidates),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    embedding_benchmarks,
    pooling_benchmarks,
    similarity_benchmarks
);

criterion_main!(benches);
