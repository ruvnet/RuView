//! Pipeline benchmarks for RuvLLM
//!
//! Benchmarks the complete request-to-response pipeline.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvllm::{Config, Request, RuvLLM};
use tokio::runtime::Runtime;

fn benchmark_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = Config::builder()
        .embedding_dim(128)
        .router_hidden_dim(32)
        .learning_enabled(false)
        .build()
        .unwrap();

    let llm = rt.block_on(RuvLLM::new(config)).unwrap();

    c.bench_function("query_simple", |b| {
        b.to_async(&rt)
            .iter(|| async { black_box(llm.query("What is Rust?").await.unwrap()) })
    });
}

fn benchmark_query_lengths(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = Config::builder()
        .embedding_dim(128)
        .router_hidden_dim(32)
        .learning_enabled(false)
        .build()
        .unwrap();

    let llm = rt.block_on(RuvLLM::new(config)).unwrap();

    let queries = vec![
        ("short", "Hi"),
        ("medium", "What is machine learning and how does it work?"),
        ("long", "Please explain in detail how neural networks process information, including concepts like forward propagation, backpropagation, gradient descent, and the role of activation functions in learning complex patterns from data."),
    ];

    let mut group = c.benchmark_group("query_by_length");
    for (name, query) in queries {
        group.bench_with_input(BenchmarkId::from_parameter(name), &query, |b, query| {
            b.to_async(&rt)
                .iter(|| async { black_box(llm.query(*query).await.unwrap()) })
        });
    }
    group.finish();
}

fn benchmark_concurrent_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = Config::builder()
        .embedding_dim(128)
        .router_hidden_dim(32)
        .learning_enabled(false)
        .build()
        .unwrap();

    let llm = std::sync::Arc::new(rt.block_on(RuvLLM::new(config)).unwrap());

    let mut group = c.benchmark_group("concurrent_queries");
    for concurrency in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    for _ in 0..concurrency {
                        let llm_clone = llm.clone();
                        handles.push(tokio::spawn(async move {
                            llm_clone.query("Test query").await.unwrap()
                        }));
                    }
                    for handle in handles {
                        black_box(handle.await.unwrap());
                    }
                })
            },
        );
    }
    group.finish();
}

fn benchmark_session(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = Config::builder()
        .embedding_dim(128)
        .router_hidden_dim(32)
        .learning_enabled(false)
        .build()
        .unwrap();

    let llm = rt.block_on(RuvLLM::new(config)).unwrap();

    c.bench_function("session_multi_turn", |b| {
        b.to_async(&rt).iter(|| async {
            let session = llm.new_session();
            black_box(llm.query_session(&session, "First question").await.unwrap());
            black_box(llm.query_session(&session, "Follow up").await.unwrap());
            black_box(
                llm.query_session(&session, "Another follow up")
                    .await
                    .unwrap(),
            );
        })
    });
}

criterion_group!(
    benches,
    benchmark_query,
    benchmark_query_lengths,
    benchmark_concurrent_queries,
    benchmark_session,
);
criterion_main!(benches);
