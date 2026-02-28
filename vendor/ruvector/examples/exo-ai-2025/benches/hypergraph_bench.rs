use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use exo_core::{EntityId, Relation, RelationType};
use exo_hypergraph::{HypergraphConfig, HypergraphSubstrate};

fn create_test_hypergraph() -> HypergraphSubstrate {
    let config = HypergraphConfig::default();
    HypergraphSubstrate::new(config)
}

fn benchmark_hyperedge_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hypergraph_edge_creation");

    for edge_size in [2, 5, 10, 20].iter() {
        let mut graph = create_test_hypergraph();

        // Pre-create entities
        let mut entities = Vec::new();
        for _ in 0..100 {
            let entity = EntityId::new();
            graph.add_entity(entity, serde_json::json!({}));
            entities.push(entity);
        }

        let relation = Relation {
            relation_type: RelationType::new("test"),
            properties: serde_json::json!({"weight": 0.9}),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(edge_size),
            edge_size,
            |b, &size| {
                b.iter(|| {
                    let entity_set: Vec<EntityId> = entities.iter().take(size).copied().collect();
                    graph.create_hyperedge(black_box(&entity_set), black_box(&relation))
                });
            },
        );
    }

    group.finish();
}

fn benchmark_query_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("hypergraph_query");

    for num_edges in [100, 500, 1000].iter() {
        let mut graph = create_test_hypergraph();

        // Create entities
        let mut entities = Vec::new();
        for _ in 0..200 {
            let entity = EntityId::new();
            graph.add_entity(entity, serde_json::json!({}));
            entities.push(entity);
        }

        // Create hyperedges
        let relation = Relation {
            relation_type: RelationType::new("test"),
            properties: serde_json::json!({}),
        };

        for _ in 0..*num_edges {
            let entity_set: Vec<EntityId> = entities.iter().take(5).copied().collect();
            graph.create_hyperedge(&entity_set, &relation).unwrap();
        }

        let query_entity = entities[0];

        group.bench_with_input(BenchmarkId::from_parameter(num_edges), num_edges, |b, _| {
            b.iter(|| graph.hyperedges_for_entity(black_box(&query_entity)));
        });
    }

    group.finish();
}

fn benchmark_betti_numbers(c: &mut Criterion) {
    let mut graph = create_test_hypergraph();

    // Create a complex structure
    let mut entities = Vec::new();
    for _ in 0..100 {
        let entity = EntityId::new();
        graph.add_entity(entity, serde_json::json!({}));
        entities.push(entity);
    }

    let relation = Relation {
        relation_type: RelationType::new("test"),
        properties: serde_json::json!({}),
    };

    for _ in 0..500 {
        let entity_set: Vec<EntityId> = entities.iter().take(5).copied().collect();
        graph.create_hyperedge(&entity_set, &relation).unwrap();
    }

    c.bench_function("hypergraph_betti_numbers", |b| {
        b.iter(|| graph.betti_numbers(black_box(3)));
    });
}

criterion_group!(
    benches,
    benchmark_hyperedge_creation,
    benchmark_query_performance,
    benchmark_betti_numbers
);
criterion_main!(benches);
