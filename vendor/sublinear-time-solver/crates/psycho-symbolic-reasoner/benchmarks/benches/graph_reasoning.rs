use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use graph_reasoner::{GraphReasoner, KnowledgeGraph, Query, Rule, InferenceEngine, RuleEngine};
use std::collections::HashMap;
use rand::prelude::*;

fn generate_test_data(num_facts: usize, num_entities: usize) -> (KnowledgeGraph, Vec<Query>, Vec<Rule>) {
    let mut rng = rand::thread_rng();
    let mut graph = KnowledgeGraph::new();

    // Generate entities
    let entities: Vec<String> = (0..num_entities)
        .map(|i| format!("entity_{}", i))
        .collect();

    let predicates = vec!["likes", "knows", "works_at", "lives_in", "is_a", "has", "owns"];

    // Add facts
    for _ in 0..num_facts {
        let subject = entities.choose(&mut rng).unwrap();
        let predicate = predicates.choose(&mut rng).unwrap();
        let object = entities.choose(&mut rng).unwrap();

        if let Err(e) = graph.add_fact(graph_reasoner::Fact::new(subject, predicate, object)) {
            eprintln!("Error adding fact: {}", e);
        }
    }

    // Generate test queries
    let queries = (0..10)
        .map(|i| {
            Query::new(&format!("query_{}", i), &format!(
                "{{\"pattern\": {{\"subject\": \"{}\", \"predicate\": \"likes\", \"object\": \"?x\"}}}}",
                entities.choose(&mut rng).unwrap()
            )).unwrap()
        })
        .collect();

    // Generate test rules
    let rules = vec![
        Rule::new(
            "transitivity_likes",
            "{{\"if\": [{{\"subject\": \"?x\", \"predicate\": \"likes\", \"object\": \"?y\"}}, {{\"subject\": \"?y\", \"predicate\": \"likes\", \"object\": \"?z\"}}], \"then\": {{\"subject\": \"?x\", \"predicate\": \"likes\", \"object\": \"?z\"}}}}".to_string()
        ).unwrap(),
        Rule::new(
            "social_connection",
            "{{\"if\": [{{\"subject\": \"?x\", \"predicate\": \"knows\", \"object\": \"?y\"}}, {{\"subject\": \"?y\", \"predicate\": \"works_at\", \"object\": \"?z\"}}], \"then\": {{\"subject\": \"?x\", \"predicate\": \"knows_workplace\", \"object\": \"?z\"}}}}".to_string()
        ).unwrap(),
    ];

    (graph, queries, rules)
}

fn bench_graph_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_creation");

    for size in [100, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("facts", size), size, |b, &size| {
            b.iter(|| {
                let (graph, _, _) = generate_test_data(size, size / 10);
                black_box(graph);
            });
        });
    }

    group.finish();
}

fn bench_query_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance");

    let graph_sizes = [1000, 10000, 50000];

    for &size in graph_sizes.iter() {
        let (graph, queries, _) = generate_test_data(size, size / 10);

        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::new("simple_query", size), &size, |b, _| {
            b.iter(|| {
                for query in &queries {
                    let result = graph.query(black_box(query));
                    black_box(result);
                }
            });
        });

        // Complex query benchmark
        let complex_query = Query::new(
            "complex",
            r#"{"pattern": {"subject": "?x", "predicate": "?p", "object": "?y"}, "filters": [{"type": "has_property", "property": "likes"}]}"#
        ).unwrap();

        group.bench_with_input(BenchmarkId::new("complex_query", size), &size, |b, _| {
            b.iter(|| {
                let result = graph.query(black_box(&complex_query));
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_inference_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference_performance");

    for &size in [500, 1000, 5000].iter() {
        let (mut graph, _, rules) = generate_test_data(size, size / 10);
        let mut inference_engine = InferenceEngine::new();
        let mut rule_engine = RuleEngine::new();

        for rule in rules {
            rule_engine.add_rule(rule);
        }

        group.bench_with_input(BenchmarkId::new("inference_iterations", size), &size, |b, _| {
            b.iter(|| {
                let results = inference_engine.infer(
                    black_box(&mut graph),
                    black_box(&rule_engine),
                    black_box(5)
                );
                black_box(results);
            });
        });
    }

    group.finish();
}

fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for &size in [1000, 10000, 100000].iter() {
        group.bench_with_input(BenchmarkId::new("memory_overhead", size), &size, |b, &size| {
            b.iter_custom(|iters| {
                let start = std::time::Instant::now();

                for _ in 0..iters {
                    let (graph, queries, rules) = generate_test_data(size, size / 10);

                    // Simulate operations that might cause memory leaks
                    for query in &queries {
                        let _ = graph.query(query);
                    }

                    let mut inference_engine = InferenceEngine::new();
                    let mut rule_engine = RuleEngine::new();

                    for rule in rules {
                        rule_engine.add_rule(rule);
                    }

                    let _ = inference_engine.infer(&mut graph.clone(), &rule_engine, 3);

                    black_box((graph, queries, inference_engine, rule_engine));
                }

                start.elapsed()
            });
        });
    }

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_operations");

    let (graph, queries, _) = generate_test_data(10000, 1000);
    let graph = std::sync::Arc::new(graph);

    group.bench_function("concurrent_queries", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4).map(|_| {
                let graph_clone = graph.clone();
                let queries_clone = queries.clone();

                std::thread::spawn(move || {
                    for query in &queries_clone {
                        let result = graph_clone.query(query);
                        black_box(result);
                    }
                })
            }).collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

fn bench_graph_operations_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_complexity");

    // Test different graph densities
    for density in [0.1, 0.5, 1.0, 2.0].iter() {
        let num_entities = 1000;
        let num_facts = (num_entities as f64 * density) as usize;

        let (graph, queries, _) = generate_test_data(num_facts, num_entities);

        group.bench_with_input(
            BenchmarkId::new("density_impact", format!("{:.1}", density)),
            density,
            |b, _| {
                b.iter(|| {
                    for query in &queries {
                        let result = graph.query(black_box(query));
                        black_box(result);
                    }
                });
            }
        );
    }

    group.finish();
}

fn bench_serialization_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    for &size in [1000, 10000, 50000].iter() {
        let (graph, _, _) = generate_test_data(size, size / 10);
        let stats = graph.get_statistics();

        group.bench_with_input(BenchmarkId::new("serialize_stats", size), &size, |b, _| {
            b.iter(|| {
                let serialized = serde_json::to_string(black_box(&stats));
                black_box(serialized);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_creation,
    bench_query_performance,
    bench_inference_performance,
    bench_memory_usage,
    bench_concurrent_operations,
    bench_graph_operations_complexity,
    bench_serialization_performance
);

criterion_main!(benches);