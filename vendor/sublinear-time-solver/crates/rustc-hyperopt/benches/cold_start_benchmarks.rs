//! Benchmarks for cold start optimization

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustc_hyperopt::ColdStartOptimizer;
use tokio::runtime::Runtime;

fn benchmark_optimization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("cold_start_optimization", |b| {
        b.to_async(&rt).iter(|| async {
            let optimizer = black_box(ColdStartOptimizer::new().await.unwrap());
            let result = optimizer.optimize_compilation().await.unwrap();
            black_box(result)
        })
    });
}

fn benchmark_signature_analysis(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("signature_analysis", |b| {
        b.to_async(&rt).iter(|| async {
            let optimizer = black_box(ColdStartOptimizer::new().await.unwrap());
            // This would normally trigger just signature analysis
            let result = optimizer.optimize_compilation().await.unwrap();
            black_box(result.project_signature)
        })
    });
}

criterion_group!(benches, benchmark_optimization, benchmark_signature_analysis);
criterion_main!(benches);