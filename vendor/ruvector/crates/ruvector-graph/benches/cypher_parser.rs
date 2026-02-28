use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ruvector_graph::cypher::parser::parse_cypher;

fn bench_simple_match(c: &mut Criterion) {
    c.bench_function("parse simple MATCH", |b| {
        b.iter(|| parse_cypher(black_box("MATCH (n:Person) RETURN n")))
    });
}

fn bench_complex_match(c: &mut Criterion) {
    c.bench_function("parse complex MATCH with WHERE", |b| {
        b.iter(|| {
            parse_cypher(black_box(
                "MATCH (a:Person)-[r:KNOWS]->(b:Person) WHERE a.age > 30 AND b.name = 'Alice' RETURN a.name, b.name, r.since ORDER BY r.since DESC LIMIT 10"
            ))
        })
    });
}

fn bench_create_query(c: &mut Criterion) {
    c.bench_function("parse CREATE query", |b| {
        b.iter(|| {
            parse_cypher(black_box(
                "CREATE (n:Person {name: 'Bob', age: 30, email: 'bob@example.com'})",
            ))
        })
    });
}

fn bench_hyperedge_query(c: &mut Criterion) {
    c.bench_function("parse hyperedge query", |b| {
        b.iter(|| {
            parse_cypher(black_box(
                "MATCH (person)-[r:TRANSACTION]->(acc1:Account, acc2:Account, merchant:Merchant) WHERE r.amount > 1000 RETURN person, r, acc1, acc2, merchant"
            ))
        })
    });
}

fn bench_aggregation_query(c: &mut Criterion) {
    c.bench_function("parse aggregation query", |b| {
        b.iter(|| {
            parse_cypher(black_box(
                "MATCH (n:Person) RETURN COUNT(n), AVG(n.age), MAX(n.salary), COLLECT(n.name)",
            ))
        })
    });
}

criterion_group!(
    benches,
    bench_simple_match,
    bench_complex_match,
    bench_create_query,
    bench_hyperedge_query,
    bench_aggregation_query
);
criterion_main!(benches);
