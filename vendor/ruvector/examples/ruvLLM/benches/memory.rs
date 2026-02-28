//! Memory service benchmarks for RuvLLM
//!
//! Benchmarks HNSW insertion, search, and graph operations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use ruvllm::config::MemoryConfig;
use ruvllm::memory::MemoryService;
use ruvllm::types::{EdgeType, MemoryEdge, MemoryNode, NodeType};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn create_random_node(id: &str, dim: usize, seed: u64) -> MemoryNode {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() - 0.5).collect();
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    vec.iter_mut().for_each(|x| *x /= norm);

    MemoryNode {
        id: id.into(),
        vector: vec,
        text: format!("Node {}", id),
        node_type: NodeType::Document,
        source: "bench".into(),
        metadata: HashMap::new(),
    }
}

fn benchmark_memory_insert(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = MemoryConfig::default();
    let memory = rt.block_on(MemoryService::new(&config)).unwrap();

    let mut counter = 0u64;

    c.bench_function("memory_insert_single", |b| {
        b.iter(|| {
            counter += 1;
            let node = create_random_node(&format!("bench-{}", counter), 768, counter);
            black_box(memory.insert_node(node).unwrap())
        })
    });
}

fn benchmark_memory_insert_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_insert_batch");
    for batch_size in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(batch_size as u64));

        let config = MemoryConfig::default();
        let memory = rt.block_on(MemoryService::new(&config)).unwrap();

        let nodes: Vec<MemoryNode> = (0..batch_size)
            .map(|i| create_random_node(&format!("batch-{}", i), 768, i as u64))
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &nodes,
            |b, nodes| {
                b.iter(|| {
                    for node in nodes.clone() {
                        black_box(memory.insert_node(node).unwrap());
                    }
                })
            },
        );
    }
    group.finish();
}

fn benchmark_memory_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = MemoryConfig::default();
    let memory = rt.block_on(MemoryService::new(&config)).unwrap();

    // Pre-populate with nodes
    for i in 0..1000 {
        let node = create_random_node(&format!("search-{}", i), 768, i as u64);
        memory.insert_node(node).unwrap();
    }

    let query = vec![0.1f32; 768];

    c.bench_function("memory_search_k10_1000", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(memory.search_with_graph(&query, 10, 64, 0).await.unwrap())
        })
    });
}

fn benchmark_memory_search_varying_k(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = MemoryConfig::default();
    let memory = rt.block_on(MemoryService::new(&config)).unwrap();

    // Pre-populate
    for i in 0..1000 {
        let node = create_random_node(&format!("k-{}", i), 768, i as u64);
        memory.insert_node(node).unwrap();
    }

    let query = vec![0.1f32; 768];

    let mut group = c.benchmark_group("memory_search_k");
    for k in [1, 5, 10, 20, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |b, &k| {
            b.to_async(&rt).iter(|| async {
                black_box(memory.search_with_graph(&query, k, 64, 0).await.unwrap())
            })
        });
    }
    group.finish();
}

fn benchmark_memory_search_varying_ef(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = MemoryConfig::default();
    let memory = rt.block_on(MemoryService::new(&config)).unwrap();

    // Pre-populate
    for i in 0..1000 {
        let node = create_random_node(&format!("ef-{}", i), 768, i as u64);
        memory.insert_node(node).unwrap();
    }

    let query = vec![0.1f32; 768];

    let mut group = c.benchmark_group("memory_search_ef");
    for ef in [16, 32, 64, 128, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(ef), &ef, |b, &ef| {
            b.to_async(&rt).iter(|| async {
                black_box(memory.search_with_graph(&query, 10, ef, 0).await.unwrap())
            })
        });
    }
    group.finish();
}

fn benchmark_memory_search_with_graph(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = MemoryConfig::default();
    let memory = rt.block_on(MemoryService::new(&config)).unwrap();

    // Pre-populate with nodes and edges
    for i in 0..500 {
        let node = create_random_node(&format!("graph-{}", i), 768, i as u64);
        memory.insert_node(node).unwrap();
    }

    for i in 0..499 {
        let edge = MemoryEdge {
            id: format!("edge-{}", i),
            src: format!("graph-{}", i),
            dst: format!("graph-{}", i + 1),
            edge_type: EdgeType::Follows,
            weight: 0.8,
            metadata: HashMap::new(),
        };
        memory.insert_edge(edge).unwrap();
    }

    let query = vec![0.1f32; 768];

    let mut group = c.benchmark_group("memory_search_hops");
    for hops in [0, 1, 2, 3] {
        group.bench_with_input(BenchmarkId::from_parameter(hops), &hops, |b, &hops| {
            b.to_async(&rt).iter(|| async {
                black_box(
                    memory
                        .search_with_graph(&query, 10, 64, hops)
                        .await
                        .unwrap(),
                )
            })
        });
    }
    group.finish();
}

fn benchmark_memory_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_scaling");
    for num_nodes in [100, 500, 1000, 5000] {
        let config = MemoryConfig::default();
        let memory = rt.block_on(MemoryService::new(&config)).unwrap();

        // Pre-populate
        for i in 0..num_nodes {
            let node = create_random_node(&format!("scale-{}", i), 768, i as u64);
            memory.insert_node(node).unwrap();
        }

        let query = vec![0.1f32; 768];

        group.bench_with_input(
            BenchmarkId::from_parameter(num_nodes),
            &num_nodes,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    black_box(memory.search_with_graph(&query, 10, 64, 0).await.unwrap())
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_memory_insert,
    benchmark_memory_insert_batch,
    benchmark_memory_search,
    benchmark_memory_search_varying_k,
    benchmark_memory_search_varying_ef,
    benchmark_memory_search_with_graph,
    benchmark_memory_scaling,
);
criterion_main!(benches);
