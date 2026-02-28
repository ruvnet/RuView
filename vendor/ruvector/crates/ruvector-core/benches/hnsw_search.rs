use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_core::types::{DbOptions, DistanceMetric, HnswConfig, SearchQuery};
use ruvector_core::{VectorDB, VectorEntry};

fn bench_hnsw_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_search");

    // Create temp database
    let temp_dir = tempfile::tempdir().unwrap();
    let options = DbOptions {
        dimensions: 128,
        distance_metric: DistanceMetric::Cosine,
        storage_path: temp_dir
            .path()
            .join("test.db")
            .to_string_lossy()
            .to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: None,
    };

    let db = VectorDB::new(options).unwrap();

    // Insert test vectors
    let vectors: Vec<VectorEntry> = (0..1000)
        .map(|i| VectorEntry {
            id: Some(format!("v{}", i)),
            vector: (0..128).map(|j| ((i + j) as f32) * 0.1).collect(),
            metadata: None,
        })
        .collect();

    db.insert_batch(vectors).unwrap();

    // Benchmark search
    let query: Vec<f32> = (0..128).map(|i| i as f32).collect();

    for k in [1, 10, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(k), k, |bench, &k| {
            bench.iter(|| {
                db.search(SearchQuery {
                    vector: black_box(query.clone()),
                    k: black_box(k),
                    filter: None,
                    ef_search: None,
                })
                .unwrap()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_hnsw_search);
criterion_main!(benches);
