use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use exo_core::{Metadata, Pattern, PatternId, Query, SubstrateTime};
use exo_temporal::{CausalConeType, TemporalConfig, TemporalMemory};

fn create_test_memory() -> TemporalMemory {
    TemporalMemory::new(TemporalConfig::default())
}

fn create_test_pattern(embedding: Vec<f32>) -> Pattern {
    Pattern::new(embedding, Metadata::new())
}

fn benchmark_causal_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_causal_query");

    for num_events in [100, 500, 1000].iter() {
        let memory = create_test_memory();

        // Pre-populate with events in causal chain
        let mut pattern_ids = Vec::new();
        for i in 0..*num_events {
            let embedding = vec![0.5; 128];
            let pattern = create_test_pattern(embedding);
            let antecedents = if i > 0 && i % 10 == 0 {
                vec![pattern_ids[i - 1]]
            } else {
                vec![]
            };
            let id = memory.store(pattern, &antecedents).unwrap();
            pattern_ids.push(id);
        }

        // Consolidate to long-term
        memory.consolidate();

        let query = Query::from_embedding(vec![0.5; 128]);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_events),
            num_events,
            |b, _| {
                b.iter(|| {
                    memory.causal_query(
                        black_box(&query),
                        black_box(SubstrateTime::now()),
                        black_box(CausalConeType::Past),
                    )
                });
            },
        );
    }

    group.finish();
}

fn benchmark_consolidation(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_consolidation");

    for num_events in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_events),
            num_events,
            |b, &events| {
                b.iter(|| {
                    let memory = create_test_memory();
                    // Fill short-term buffer
                    for _ in 0..events {
                        let embedding = vec![0.5; 128];
                        let pattern = create_test_pattern(embedding);
                        memory.store(pattern, &[]).unwrap();
                    }
                    // Benchmark consolidation
                    memory.consolidate()
                });
            },
        );
    }

    group.finish();
}

fn benchmark_pattern_storage(c: &mut Criterion) {
    let memory = create_test_memory();

    c.bench_function("temporal_pattern_storage", |b| {
        b.iter(|| {
            let embedding = vec![0.5; 128];
            let pattern = create_test_pattern(embedding);
            memory.store(black_box(pattern), black_box(&[]))
        });
    });
}

fn benchmark_pattern_retrieval(c: &mut Criterion) {
    let memory = create_test_memory();

    // Pre-populate
    let mut pattern_ids = Vec::new();
    for _ in 0..1000 {
        let embedding = vec![0.5; 128];
        let pattern = create_test_pattern(embedding);
        let id = memory.store(pattern, &[]).unwrap();
        pattern_ids.push(id);
    }

    c.bench_function("temporal_pattern_retrieval", |b| {
        let query_id = pattern_ids[500];
        b.iter(|| memory.get(black_box(&query_id)));
    });
}

criterion_group!(
    benches,
    benchmark_causal_query,
    benchmark_consolidation,
    benchmark_pattern_storage,
    benchmark_pattern_retrieval
);
criterion_main!(benches);
