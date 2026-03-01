use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_graph::types::{EdgeId, NodeId, Properties, PropertyValue};
use ruvector_graph::{Edge, GraphDB, Node};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Helper to create test graph
fn create_test_graph() -> GraphDB {
    GraphDB::new()
}

/// Benchmark: Single node insertion
fn bench_node_insertion_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_insertion_single");

    for size in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let graph = create_test_graph();
                for i in 0..size {
                    let mut props = Properties::new();
                    props.insert(
                        "name".to_string(),
                        PropertyValue::String(format!("node_{}", i)),
                    );
                    props.insert("value".to_string(), PropertyValue::Integer(i as i64));

                    let node_id = NodeId(format!("node_{}", i));
                    let node = Node::new(node_id, vec!["Person".to_string()], props);
                    black_box(graph.create_node(node).unwrap());
                }
            });
        });
    }

    group.finish();
}

/// Benchmark: Batch node insertion
fn bench_node_insertion_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_insertion_batch");

    for batch_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let graph = create_test_graph();
                    for i in 0..batch_size {
                        let mut props = Properties::new();
                        props.insert(
                            "name".to_string(),
                            PropertyValue::String(format!("node_{}", i)),
                        );
                        props.insert("value".to_string(), PropertyValue::Integer(i as i64));

                        let node_id = NodeId(format!("batch_node_{}", i));
                        let node = Node::new(node_id, vec!["Person".to_string()], props);
                        black_box(graph.create_node(node).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Bulk node insertion (optimized path)
fn bench_node_insertion_bulk(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_insertion_bulk");
    group.sample_size(10); // Reduce samples for large operations

    for bulk_size in [10000, 100000].iter() {
        group.throughput(Throughput::Elements(*bulk_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(bulk_size),
            bulk_size,
            |b, &bulk_size| {
                b.iter(|| {
                    let graph = create_test_graph();
                    for i in 0..bulk_size {
                        let mut props = Properties::new();
                        props.insert("id".to_string(), PropertyValue::Integer(i as i64));
                        props.insert(
                            "name".to_string(),
                            PropertyValue::String(format!("user_{}", i)),
                        );

                        let node_id = NodeId(format!("bulk_user_{}", i));
                        let node = Node::new(node_id, vec!["User".to_string()], props);
                        black_box(graph.create_node(node).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Edge creation
fn bench_edge_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_creation");

    // Setup: Create nodes once
    let graph = Arc::new(create_test_graph());
    let mut node_ids = Vec::new();
    for i in 0..1000 {
        let mut props = Properties::new();
        props.insert("id".to_string(), PropertyValue::Integer(i as i64));
        let node_id = NodeId(format!("edge_test_node_{}", i));
        let node = Node::new(node_id.clone(), vec!["Person".to_string()], props);
        graph.create_node(node).unwrap();
        node_ids.push(node_id);
    }

    for num_edges in [100, 1000].iter() {
        group.throughput(Throughput::Elements(*num_edges as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_edges),
            num_edges,
            |b, &num_edges| {
                let graph = graph.clone();
                let node_ids = node_ids.clone();
                b.iter(|| {
                    for i in 0..num_edges {
                        let from = &node_ids[i % node_ids.len()];
                        let to = &node_ids[(i + 1) % node_ids.len()];

                        let mut props = Properties::new();
                        props.insert("weight".to_string(), PropertyValue::Float(i as f64));

                        let edge_id = EdgeId(format!("edge_{}", i));
                        let edge = Edge::new(
                            edge_id,
                            from.clone(),
                            to.clone(),
                            "KNOWS".to_string(),
                            props,
                        );
                        black_box(graph.create_edge(edge).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Simple node lookup by ID
fn bench_query_node_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_node_lookup");

    // Setup: Create 10k nodes (reduced for faster benchmark)
    let graph = Arc::new(create_test_graph());
    let mut node_ids = Vec::new();
    for i in 0..10000 {
        let mut props = Properties::new();
        props.insert("id".to_string(), PropertyValue::Integer(i as i64));
        let node_id = NodeId(format!("lookup_node_{}", i));
        let node = Node::new(node_id.clone(), vec!["Person".to_string()], props);
        graph.create_node(node).unwrap();
        node_ids.push(node_id);
    }

    group.bench_function("lookup_by_id", |b| {
        let graph = graph.clone();
        let node_ids = node_ids.clone();
        b.iter(|| {
            let id = &node_ids[black_box(1234 % node_ids.len())];
            black_box(graph.get_node(id).unwrap());
        });
    });

    group.finish();
}

/// Benchmark: Edge lookup
fn bench_query_edge_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_edge_lookup");

    // Setup: Create nodes and edges
    let graph = Arc::new(create_test_graph());
    let mut node_ids = Vec::new();
    let mut edge_ids = Vec::new();

    // Create 100 nodes
    for i in 0..100 {
        let mut props = Properties::new();
        props.insert("id".to_string(), PropertyValue::Integer(i as i64));
        let node_id = NodeId(format!("trav_node_{}", i));
        let node = Node::new(node_id.clone(), vec!["Person".to_string()], props);
        graph.create_node(node).unwrap();
        node_ids.push(node_id);
    }

    // Create edges (each node has ~5 outgoing edges)
    for i in 0..node_ids.len() {
        for j in 0..5 {
            let to_idx = (i + j + 1) % node_ids.len();
            let edge_id = EdgeId(format!("trav_edge_{}_{}", i, j));
            let edge = Edge::new(
                edge_id.clone(),
                node_ids[i].clone(),
                node_ids[to_idx].clone(),
                "KNOWS".to_string(),
                Properties::new(),
            );
            graph.create_edge(edge).unwrap();
            edge_ids.push(edge_id);
        }
    }

    group.bench_function("edge_by_id", |b| {
        let graph = graph.clone();
        let edge_ids = edge_ids.clone();
        b.iter(|| {
            let id = &edge_ids[black_box(10 % edge_ids.len())];
            black_box(graph.get_edge(id).unwrap());
        });
    });

    group.finish();
}

/// Benchmark: Get nodes by label
fn bench_query_get_by_label(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_get_by_label");

    let graph = Arc::new(create_test_graph());

    // Create diverse nodes with different labels
    for i in 0..1000 {
        let mut props = Properties::new();
        props.insert("id".to_string(), PropertyValue::Integer(i as i64));
        let node_id = NodeId(format!("label_node_{}", i));

        let label = if i % 3 == 0 {
            "Person"
        } else if i % 3 == 1 {
            "Organization"
        } else {
            "Location"
        };

        let node = Node::new(node_id, vec![label.to_string()], props);
        graph.create_node(node).unwrap();
    }

    group.bench_function("get_persons", |b| {
        let graph = graph.clone();
        b.iter(|| {
            let nodes = graph.get_nodes_by_label("Person");
            black_box(nodes.len());
        });
    });

    group.finish();
}

/// Benchmark: Memory usage tracking
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    group.sample_size(10);

    for num_nodes in [1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_nodes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_nodes),
            num_nodes,
            |b, &num_nodes| {
                b.iter_custom(|iters| {
                    let mut total_duration = Duration::ZERO;

                    for _ in 0..iters {
                        let graph = create_test_graph();

                        let start = std::time::Instant::now();
                        for i in 0..*num_nodes {
                            let mut props = Properties::new();
                            props.insert("id".to_string(), PropertyValue::Integer(i as i64));
                            props.insert(
                                "name".to_string(),
                                PropertyValue::String(format!("node_{}", i)),
                            );

                            let node_id = NodeId(format!("mem_node_{}", i));
                            let node = Node::new(node_id, vec!["TestNode".to_string()], props);
                            graph.create_node(node).unwrap();
                        }
                        total_duration += start.elapsed();

                        // Force drop to measure cleanup
                        drop(graph);
                    }

                    total_duration
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_node_insertion_single,
    bench_node_insertion_batch,
    bench_node_insertion_bulk,
    bench_edge_creation,
    bench_query_node_lookup,
    bench_query_edge_lookup,
    bench_query_get_by_label,
    bench_memory_usage
);

criterion_main!(benches);
