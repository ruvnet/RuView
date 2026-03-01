//! Benchmarks for new capabilities
//!
//! Run with: cargo bench --package ruvector-graph --bench new_capabilities_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_graph::cypher::parser::parse_cypher;
use ruvector_graph::hybrid::semantic_search::{SemanticSearch, SemanticSearchConfig};
use ruvector_graph::hybrid::vector_index::{EmbeddingConfig, HybridIndex, VectorIndexType};

// ============================================================================
// Parser Benchmarks
// ============================================================================

fn bench_simple_match(c: &mut Criterion) {
    let query = "MATCH (n:Person) RETURN n";

    c.bench_function("parser/simple_match", |b| {
        b.iter(|| parse_cypher(black_box(query)))
    });
}

fn bench_relationship_match(c: &mut Criterion) {
    let query = "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a, r, b";

    c.bench_function("parser/relationship_match", |b| {
        b.iter(|| parse_cypher(black_box(query)))
    });
}

fn bench_chained_relationship(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser/chained_relationships");

    // 2-hop chain
    let query_2hop = "MATCH (a)-[r]->(b)-[s]->(c) RETURN a, c";
    group.bench_function("2_hop", |b| b.iter(|| parse_cypher(black_box(query_2hop))));

    // 3-hop chain
    let query_3hop = "MATCH (a)-[r]->(b)-[s]->(c)-[t]->(d) RETURN a, d";
    group.bench_function("3_hop", |b| b.iter(|| parse_cypher(black_box(query_3hop))));

    // 4-hop chain
    let query_4hop = "MATCH (a)-[r]->(b)-[s]->(c)-[t]->(d)-[u]->(e) RETURN a, e";
    group.bench_function("4_hop", |b| b.iter(|| parse_cypher(black_box(query_4hop))));

    group.finish();
}

fn bench_mixed_direction_chain(c: &mut Criterion) {
    let query = "MATCH (a:Person)-[r:KNOWS]->(b:Person)<-[s:MANAGES]-(c:Manager) RETURN a, b, c";

    c.bench_function("parser/mixed_direction_chain", |b| {
        b.iter(|| parse_cypher(black_box(query)))
    });
}

fn bench_map_literal(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser/map_literal");

    // Empty map
    let query_empty = "MATCH (n) RETURN {}";
    group.bench_function("empty", |b| b.iter(|| parse_cypher(black_box(query_empty))));

    // Small map (2 keys)
    let query_small = "MATCH (n) RETURN {name: n.name, age: n.age}";
    group.bench_function("2_keys", |b| {
        b.iter(|| parse_cypher(black_box(query_small)))
    });

    // Medium map (5 keys)
    let query_medium = "MATCH (n) RETURN {a: n.a, b: n.b, c: n.c, d: n.d, e: n.e}";
    group.bench_function("5_keys", |b| {
        b.iter(|| parse_cypher(black_box(query_medium)))
    });

    // Large map (10 keys)
    let query_large = "MATCH (n) RETURN {a: n.a, b: n.b, c: n.c, d: n.d, e: n.e, f: n.f, g: n.g, h: n.h, i: n.i, j: n.j}";
    group.bench_function("10_keys", |b| {
        b.iter(|| parse_cypher(black_box(query_large)))
    });

    group.finish();
}

fn bench_remove_statement(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser/remove");

    // Remove property
    let query_prop = "MATCH (n:Person) REMOVE n.age RETURN n";
    group.bench_function("property", |b| {
        b.iter(|| parse_cypher(black_box(query_prop)))
    });

    // Remove single label
    let query_label = "MATCH (n:Person:Employee) REMOVE n:Employee RETURN n";
    group.bench_function("single_label", |b| {
        b.iter(|| parse_cypher(black_box(query_label)))
    });

    // Remove multiple labels
    let query_multi = "MATCH (n:A:B:C:D) REMOVE n:B:C:D RETURN n";
    group.bench_function("multi_label", |b| {
        b.iter(|| parse_cypher(black_box(query_multi)))
    });

    group.finish();
}

fn bench_complex_query(c: &mut Criterion) {
    let query = r#"
        MATCH (p:Person)-[r:WORKS_AT]->(c:Company)<-[h:HEADQUARTERED]-(l:Location)
        WHERE p.age > 30 AND c.revenue > 1000000
        RETURN {
            person: p.name,
            company: c.name,
            location: l.city
        }
        ORDER BY p.age DESC
        LIMIT 10
    "#;

    c.bench_function("parser/complex_query", |b| {
        b.iter(|| parse_cypher(black_box(query)))
    });
}

// ============================================================================
// Semantic Search Benchmarks
// ============================================================================

fn setup_semantic_search(num_vectors: usize, dimensions: usize) -> SemanticSearch {
    let config = EmbeddingConfig {
        dimensions,
        ..Default::default()
    };
    let index = HybridIndex::new(config).unwrap();
    index.initialize_index(VectorIndexType::Node).unwrap();

    // Add test embeddings
    for i in 0..num_vectors {
        let mut embedding = vec![0.0f32; dimensions];
        // Create varied embeddings
        embedding[i % dimensions] = 1.0;
        embedding[(i + 1) % dimensions] = 0.5;

        index
            .add_node_embedding(format!("node_{}", i), embedding)
            .unwrap();
    }

    SemanticSearch::new(index, SemanticSearchConfig::default())
}

fn bench_semantic_search_small(c: &mut Criterion) {
    let search = setup_semantic_search(100, 128);
    let query: Vec<f32> = (0..128).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();

    c.bench_function("semantic_search/100_vectors_128d", |b| {
        b.iter(|| search.find_similar_nodes(black_box(&query), 10))
    });
}

fn bench_semantic_search_medium(c: &mut Criterion) {
    let search = setup_semantic_search(1000, 128);
    let query: Vec<f32> = (0..128).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();

    c.bench_function("semantic_search/1000_vectors_128d", |b| {
        b.iter(|| search.find_similar_nodes(black_box(&query), 10))
    });
}

fn bench_semantic_search_dimensions(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_search/dimensions");

    for dim in [64, 128, 256, 384, 512].iter() {
        let search = setup_semantic_search(500, *dim);
        let query: Vec<f32> = (0..*dim).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();

        group.bench_with_input(BenchmarkId::from_parameter(dim), dim, |b, _| {
            b.iter(|| search.find_similar_nodes(black_box(&query), 10))
        });
    }

    group.finish();
}

fn bench_semantic_search_top_k(c: &mut Criterion) {
    let search = setup_semantic_search(1000, 128);
    let query: Vec<f32> = (0..128).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();

    let mut group = c.benchmark_group("semantic_search/top_k");

    for k in [1, 5, 10, 25, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(k), k, |b, &k| {
            b.iter(|| search.find_similar_nodes(black_box(&query), k))
        });
    }

    group.finish();
}

// ============================================================================
// Distance Conversion Benchmark (the fix we made)
// ============================================================================

fn bench_distance_conversion(c: &mut Criterion) {
    let distances: Vec<f32> = (0..10000).map(|i| (i as f32) / 10000.0).collect();

    c.bench_function("semantic_search/distance_conversion_10k", |b| {
        b.iter(|| {
            let _: Vec<f32> = distances.iter().map(|d| 1.0 - d).collect();
        })
    });
}

fn bench_similarity_filtering(c: &mut Criterion) {
    let distances: Vec<f32> = (0..10000).map(|i| (i as f32) / 10000.0).collect();
    let min_similarity = 0.7f32;

    c.bench_function("semantic_search/similarity_filter_10k", |b| {
        b.iter(|| {
            let _: Vec<f32> = distances
                .iter()
                .map(|d| 1.0 - d)
                .filter(|s| *s >= min_similarity)
                .collect();
        })
    });
}

criterion_group!(
    parser_benches,
    bench_simple_match,
    bench_relationship_match,
    bench_chained_relationship,
    bench_mixed_direction_chain,
    bench_map_literal,
    bench_remove_statement,
    bench_complex_query,
);

criterion_group!(
    semantic_search_benches,
    bench_semantic_search_small,
    bench_semantic_search_medium,
    bench_semantic_search_dimensions,
    bench_semantic_search_top_k,
    bench_distance_conversion,
    bench_similarity_filtering,
);

criterion_main!(parser_benches, semantic_search_benches);
