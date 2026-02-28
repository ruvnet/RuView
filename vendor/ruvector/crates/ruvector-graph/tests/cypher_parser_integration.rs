//! Integration tests for Cypher parser

use ruvector_graph::cypher::{ast::*, parse_cypher};

#[test]
fn test_simple_match_query() {
    let query = "MATCH (n:Person) RETURN n";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse simple MATCH query: {:?}",
        result.err()
    );

    let ast = result.unwrap();
    assert_eq!(ast.statements.len(), 2); // MATCH and RETURN
}

#[test]
fn test_match_with_where() {
    let query = "MATCH (n:Person) WHERE n.age > 30 RETURN n.name";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse MATCH with WHERE: {:?}",
        result.err()
    );
}

#[test]
fn test_relationship_pattern() {
    let query = "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a, r, b";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse relationship pattern: {:?}",
        result.err()
    );
}

#[test]
fn test_create_node() {
    let query = "CREATE (n:Person {name: 'Alice', age: 30})";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse CREATE query: {:?}",
        result.err()
    );
}

#[test]
#[ignore = "Hyperedge syntax not yet implemented in parser"]
fn test_hyperedge_pattern() {
    let query = "MATCH (a)-[r:TRANSACTION]->(b, c, d) RETURN a, r, b, c, d";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse hyperedge: {:?}",
        result.err()
    );

    let ast = result.unwrap();
    assert!(ast.has_hyperedges(), "Query should contain hyperedges");
}

#[test]
fn test_aggregation_functions() {
    let query = "MATCH (n:Person) RETURN COUNT(n), AVG(n.age), MAX(n.salary)";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse aggregation query: {:?}",
        result.err()
    );
}

#[test]
fn test_order_by_limit() {
    let query = "MATCH (n:Person) RETURN n.name ORDER BY n.age DESC LIMIT 10";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse ORDER BY LIMIT: {:?}",
        result.err()
    );
}

#[test]
fn test_complex_query() {
    let query = r#"
        MATCH (a:Person)-[r:KNOWS]->(b:Person)
        WHERE a.age > 30 AND b.name = 'Alice'
        RETURN a.name, b.name, r.since
        ORDER BY r.since DESC
        LIMIT 10
    "#;
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse complex query: {:?}",
        result.err()
    );
}

#[test]
#[ignore = "CREATE relationship with properties not yet fully implemented"]
fn test_create_relationship() {
    let query = r#"
        MATCH (a:Person), (b:Person)
        WHERE a.name = 'Alice' AND b.name = 'Bob'
        CREATE (a)-[:KNOWS {since: 2024}]->(b)
    "#;
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse CREATE relationship: {:?}",
        result.err()
    );
}

#[test]
#[ignore = "MERGE ON CREATE SET not yet implemented"]
fn test_merge_pattern() {
    let query = "MERGE (n:Person {name: 'Alice'}) ON CREATE SET n.created = 2024";
    let result = parse_cypher(query);
    assert!(result.is_ok(), "Failed to parse MERGE: {:?}", result.err());
}

#[test]
fn test_with_clause() {
    let query = r#"
        MATCH (n:Person)
        WITH n, n.age AS age
        WHERE age > 30
        RETURN n.name, age
    "#;
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse WITH clause: {:?}",
        result.err()
    );
}

#[test]
fn test_path_pattern() {
    let query = "MATCH p = (a:Person)-[*1..3]->(b:Person) RETURN p";
    let result = parse_cypher(query);
    assert!(
        result.is_ok(),
        "Failed to parse path pattern: {:?}",
        result.err()
    );
}

#[test]
fn test_is_read_only() {
    let query1 = "MATCH (n:Person) RETURN n";
    let ast1 = parse_cypher(query1).unwrap();
    assert!(ast1.is_read_only());

    let query2 = "CREATE (n:Person {name: 'Alice'})";
    let ast2 = parse_cypher(query2).unwrap();
    assert!(!ast2.is_read_only());
}
