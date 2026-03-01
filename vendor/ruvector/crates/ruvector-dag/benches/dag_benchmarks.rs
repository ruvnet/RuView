use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_dag::attention::*;
use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};
use ruvector_dag::sona::*;

fn create_dag(size: usize) -> QueryDag {
    let mut dag = QueryDag::new();

    for i in 0..size {
        dag.add_node(OperatorNode::new(
            i,
            OperatorType::SeqScan {
                table: format!("t{}", i),
            },
        ));
    }

    for i in 0..size.saturating_sub(1) {
        let _ = dag.add_edge(i, i + 1);
    }

    dag
}

fn create_complex_dag(size: usize) -> QueryDag {
    let mut dag = QueryDag::new();

    // Create nodes
    for i in 0..size {
        let op_type = match i % 4 {
            0 => OperatorType::SeqScan {
                table: format!("t{}", i),
            },
            1 => OperatorType::HnswScan {
                index: format!("idx{}", i),
                dim: 128,
            },
            2 => OperatorType::HashJoin {
                key: format!("key{}", i),
            },
            _ => OperatorType::Filter {
                condition: format!("col{} > {}", i, i * 10),
            },
        };
        dag.add_node(OperatorNode::new(i, op_type));
    }

    // Create tree-like structure
    for i in 0..size.saturating_sub(1) {
        let _ = dag.add_edge(i, i + 1);
        if i % 3 == 0 && i + 2 < size {
            let _ = dag.add_edge(i, i + 2);
        }
    }

    dag
}

fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_sort");

    for size in [10, 100, 500, 1000] {
        let dag = create_dag(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &dag, |b, dag| {
            b.iter(|| dag.topological_sort())
        });
    }

    group.finish();
}

fn bench_dag_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_construction");

    for size in [10, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| create_dag(size))
        });
    }

    group.finish();
}

fn bench_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention");

    let dag = create_dag(100);
    let attention = TopologicalAttention::new(TopologicalConfig::default());

    group.bench_function("topological_100", |b| b.iter(|| attention.forward(&dag)));

    let complex_dag = create_complex_dag(100);
    group.bench_function("topological_complex_100", |b| {
        b.iter(|| attention.forward(&complex_dag))
    });

    group.finish();
}

fn bench_attention_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_cache");

    let mut cache = AttentionCache::new(100);
    let dag = create_dag(50);

    // Pre-populate cache
    let mut scores = std::collections::HashMap::new();
    for i in 0..50 {
        scores.insert(i, i as f32 / 50.0);
    }
    cache.insert(&dag, "test", scores.clone());

    group.bench_function("cache_hit", |b| b.iter(|| cache.get(&dag, "test")));

    group.bench_function("cache_miss", |b| {
        let other_dag = create_dag(60);
        b.iter(|| cache.get(&other_dag, "test"))
    });

    group.finish();
}

fn bench_micro_lora(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_lora");

    let lora = MicroLoRA::new(MicroLoRAConfig::default(), 256);
    let input = ndarray::Array1::from_vec(vec![0.1f32; 256]);

    group.bench_function("forward_256", |b| b.iter(|| lora.forward(&input)));

    let lora_512 = MicroLoRA::new(MicroLoRAConfig::default(), 512);
    let input_512 = ndarray::Array1::from_vec(vec![0.1f32; 512]);

    group.bench_function("forward_512", |b| b.iter(|| lora_512.forward(&input_512)));

    group.finish();
}

fn bench_lora_adaptation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lora_adaptation");

    let mut lora = MicroLoRA::new(MicroLoRAConfig::default(), 256);
    let gradient = ndarray::Array1::from_vec(vec![0.01f32; 256]);

    group.bench_function("adapt_256", |b| b.iter(|| lora.adapt(&gradient, 0.1)));

    group.finish();
}

fn bench_trajectory_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory_buffer");

    let buffer = DagTrajectoryBuffer::new(1000);

    group.bench_function("push", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            buffer.push(DagTrajectory::new(
                counter,
                vec![0.1; 256],
                "test".to_string(),
                100.0,
                150.0,
            ));
            counter += 1;
        })
    });

    // Pre-fill buffer
    for i in 0..1000 {
        buffer.push(DagTrajectory::new(
            i,
            vec![0.1; 256],
            "test".to_string(),
            100.0,
            150.0,
        ));
    }

    group.bench_function("drain", |b| b.iter(|| buffer.drain()));

    group.finish();
}

fn bench_reasoning_bank(c: &mut Criterion) {
    let mut group = c.benchmark_group("reasoning_bank");

    let mut bank = DagReasoningBank::new(ReasoningBankConfig {
        num_clusters: 10,
        pattern_dim: 256,
        max_patterns: 1000,
        similarity_threshold: 0.5,
    });

    // Pre-populate
    for i in 0..100 {
        let pattern: Vec<f32> = (0..256)
            .map(|j| ((i * 256 + j) as f32 / 1000.0).sin())
            .collect();
        bank.store_pattern(pattern, 0.8);
    }

    let query: Vec<f32> = (0..256).map(|j| (j as f32 / 1000.0).sin()).collect();

    group.bench_function("query_similar_10", |b| {
        b.iter(|| bank.query_similar(&query, 10))
    });

    group.bench_function("store_pattern", |b| {
        let mut counter = 0;
        b.iter(|| {
            let pattern: Vec<f32> = (0..256)
                .map(|j| ((counter * 256 + j) as f32 / 1000.0).cos())
                .collect();
            bank.store_pattern(pattern, 0.7);
            counter += 1;
        })
    });

    group.finish();
}

fn bench_ewc(c: &mut Criterion) {
    let mut group = c.benchmark_group("ewc");

    let mut ewc = EwcPlusPlus::new(EwcConfig::default());

    let params = ndarray::Array1::from_vec(vec![1.0; 256]);
    let fisher = ndarray::Array1::from_vec(vec![0.5; 256]);

    group.bench_function("consolidate", |b| {
        b.iter(|| ewc.consolidate(&params, &fisher))
    });

    ewc.consolidate(&params, &fisher);

    group.bench_function("penalty", |b| {
        let test_params = ndarray::Array1::from_vec(vec![1.1; 256]);
        b.iter(|| ewc.penalty(&test_params))
    });

    group.finish();
}

fn bench_dag_depths(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_depths");

    for size in [10, 100, 500] {
        let dag = create_complex_dag(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &dag, |b, dag| {
            b.iter(|| dag.compute_depths())
        });
    }

    group.finish();
}

fn bench_dag_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_serialization");

    let dag = create_complex_dag(100);

    group.bench_function("to_json", |b| b.iter(|| dag.to_json()));

    let json = dag.to_json().unwrap();

    group.bench_function("from_json", |b| b.iter(|| QueryDag::from_json(&json)));

    group.finish();
}

criterion_group!(
    dag_benches,
    bench_dag_construction,
    bench_topological_sort,
    bench_dag_depths,
    bench_dag_serialization,
);

criterion_group!(attention_benches, bench_attention, bench_attention_cache,);

criterion_group!(
    sona_benches,
    bench_micro_lora,
    bench_lora_adaptation,
    bench_trajectory_buffer,
    bench_reasoning_bank,
    bench_ewc,
);

criterion_main!(dag_benches, attention_benches, sona_benches);
