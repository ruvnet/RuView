//! Attention engine benchmarks for RuvLLM
//!
//! Benchmarks multi-head graph attention.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::{Rng, SeedableRng};
use ruvllm::attention::GraphAttentionEngine;
use ruvllm::config::EmbeddingConfig;
use ruvllm::memory::SubGraph;
use ruvllm::types::{EdgeType, MemoryEdge, MemoryNode, NodeType};
use std::collections::HashMap;

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

fn create_subgraph(num_nodes: usize, num_edges: usize, dim: usize) -> SubGraph {
    let nodes: Vec<MemoryNode> = (0..num_nodes)
        .map(|i| create_random_node(&format!("n-{}", i), dim, i as u64))
        .collect();

    let edges: Vec<MemoryEdge> = (0..num_edges.min(num_nodes.saturating_sub(1)))
        .map(|i| MemoryEdge {
            id: format!("e-{}", i),
            src: format!("n-{}", i),
            dst: format!("n-{}", (i + 1) % num_nodes),
            edge_type: EdgeType::Follows,
            weight: 0.8,
            metadata: HashMap::new(),
        })
        .collect();

    SubGraph {
        nodes,
        edges,
        center_ids: vec!["n-0".into()],
    }
}

fn benchmark_attention_forward(c: &mut Criterion) {
    let config = EmbeddingConfig::default();
    let engine = GraphAttentionEngine::new(&config).unwrap();

    let query = vec![0.1f32; config.dimension];
    let subgraph = create_subgraph(10, 9, config.dimension);

    c.bench_function("attention_forward_10_nodes", |b| {
        b.iter(|| black_box(engine.attend(&query, &subgraph).unwrap()))
    });
}

fn benchmark_attention_varying_nodes(c: &mut Criterion) {
    let config = EmbeddingConfig::default();
    let engine = GraphAttentionEngine::new(&config).unwrap();

    let query = vec![0.1f32; config.dimension];

    let mut group = c.benchmark_group("attention_nodes");
    for num_nodes in [5, 10, 20, 50, 100] {
        let subgraph = create_subgraph(num_nodes, num_nodes - 1, config.dimension);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_nodes),
            &subgraph,
            |b, subgraph| b.iter(|| black_box(engine.attend(&query, subgraph).unwrap())),
        );
    }
    group.finish();
}

fn benchmark_attention_varying_edges(c: &mut Criterion) {
    let config = EmbeddingConfig::default();
    let engine = GraphAttentionEngine::new(&config).unwrap();

    let query = vec![0.1f32; config.dimension];

    let mut group = c.benchmark_group("attention_edges");
    for num_edges in [0, 10, 25, 50, 100] {
        let subgraph = create_subgraph(50, num_edges, config.dimension);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_edges),
            &subgraph,
            |b, subgraph| b.iter(|| black_box(engine.attend(&query, subgraph).unwrap())),
        );
    }
    group.finish();
}

fn benchmark_attention_varying_dims(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_dimension");
    for dim in [128, 256, 512, 768, 1024] {
        let config = EmbeddingConfig {
            dimension: dim,
            ..EmbeddingConfig::default()
        };
        let engine = GraphAttentionEngine::new(&config).unwrap();

        let query = vec![0.1f32; dim];
        let subgraph = create_subgraph(20, 19, dim);

        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &subgraph,
            |b, subgraph| b.iter(|| black_box(engine.attend(&query, subgraph).unwrap())),
        );
    }
    group.finish();
}

fn benchmark_cross_attention(c: &mut Criterion) {
    let config = EmbeddingConfig::default();
    let engine = GraphAttentionEngine::new(&config).unwrap();

    let query = vec![0.1f32; config.dimension];
    let subgraph = create_subgraph(20, 19, config.dimension);

    c.bench_function("cross_attention_20_nodes", |b| {
        b.iter(|| black_box(engine.cross_attend(&query, &subgraph).unwrap()))
    });
}

fn benchmark_attention_empty_graph(c: &mut Criterion) {
    let config = EmbeddingConfig::default();
    let engine = GraphAttentionEngine::new(&config).unwrap();

    let query = vec![0.1f32; config.dimension];
    let subgraph = SubGraph {
        nodes: vec![],
        edges: vec![],
        center_ids: vec![],
    };

    c.bench_function("attention_empty_graph", |b| {
        b.iter(|| black_box(engine.attend(&query, &subgraph).unwrap()))
    });
}

criterion_group!(
    benches,
    benchmark_attention_forward,
    benchmark_attention_varying_nodes,
    benchmark_attention_varying_edges,
    benchmark_attention_varying_dims,
    benchmark_cross_attention,
    benchmark_attention_empty_graph,
);
criterion_main!(benches);
