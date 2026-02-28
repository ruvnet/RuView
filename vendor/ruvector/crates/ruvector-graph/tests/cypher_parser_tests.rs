//! Cypher query parser tests
//!
//! Tests for parsing valid and invalid Cypher queries to ensure syntax correctness.

use ruvector_graph::cypher::parse_cypher;

// ============================================================================
// Valid Cypher Queries
// ============================================================================

#[test]
fn test_parse_simple_match() {
    let result = parse_cypher("MATCH (n) RETURN n");
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn test_parse_match_with_label() {
    let result = parse_cypher("MATCH (n:Person) RETURN n");
    assert!(result.is_ok());
}

#[test]
fn test_parse_match_with_properties() {
    let result = parse_cypher("MATCH (n:Person {name: 'Alice'}) RETURN n");
    assert!(result.is_ok());
}

#[test]
fn test_parse_match_relationship() {
    let result = parse_cypher("MATCH (a)-[r:KNOWS]->(b) RETURN a, r, b");
    assert!(result.is_ok());
}

#[test]
fn test_parse_match_undirected_relationship() {
    let result = parse_cypher("MATCH (a)-[r:FRIEND]-(b) RETURN a, b");
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn test_parse_match_path() {
    let result = parse_cypher("MATCH p = (a)-[:KNOWS*1..3]->(b) RETURN p");
    assert!(result.is_ok());
}

#[test]
fn test_parse_create_node() {
    let result = parse_cypher("CREATE (n:Person {name: 'Bob', age: 30})");
    assert!(result.is_ok());
}

#[test]
fn test_parse_create_relationship() {
    let result =
        parse_cypher("CREATE (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'})");
    assert!(result.is_ok());
}

#[test]
fn test_parse_merge() {
    let result = parse_cypher("MERGE (n:Person {name: 'Charlie'})");
    assert!(result.is_ok());
}

#[test]
fn test_parse_delete() {
    let result = parse_cypher("MATCH (n:Person {name: 'Alice'}) DELETE n");
    assert!(result.is_ok());
}

#[test]
fn test_parse_set_property() {
    let result = parse_cypher("MATCH (n:Person {name: 'Alice'}) SET n.age = 31");
    assert!(result.is_ok());
}

#[test]
fn test_parse_remove_property() {
    let result = parse_cypher("MATCH (n:Person {name: 'Alice'}) REMOVE n.age");
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn test_parse_where_clause() {
    let result = parse_cypher("MATCH (n:Person) WHERE n.age > 25 RETURN n");
    assert!(result.is_ok());
}

#[test]
fn test_parse_order_by() {
    let result = parse_cypher("MATCH (n:Person) RETURN n ORDER BY n.age DESC");
    assert!(result.is_ok());
}

#[test]
fn test_parse_limit() {
    let result = parse_cypher("MATCH (n:Person) RETURN n LIMIT 10");
    assert!(result.is_ok());
}

#[test]
fn test_parse_skip() {
    let result = parse_cypher("MATCH (n:Person) RETURN n SKIP 5 LIMIT 10");
    assert!(result.is_ok());
}

#[test]
fn test_parse_aggregate_count() {
    let result = parse_cypher("MATCH (n:Person) RETURN COUNT(n)");
    assert!(result.is_ok());
}

#[test]
fn test_parse_aggregate_sum() {
    let result = parse_cypher("MATCH (n:Person) RETURN SUM(n.age)");
    assert!(result.is_ok());
}

#[test]
fn test_parse_aggregate_avg() {
    let result = parse_cypher("MATCH (n:Person) RETURN AVG(n.age)");
    assert!(result.is_ok());
}

#[test]
fn test_parse_with_clause() {
    let result = parse_cypher("MATCH (n:Person) WITH n.age AS age WHERE age > 25 RETURN age");
    assert!(result.is_ok());
}

#[test]
fn test_parse_optional_match() {
    let result = parse_cypher("OPTIONAL MATCH (n:Person)-[r:KNOWS]->(m) RETURN n, m");
    assert!(result.is_ok());
}

// ============================================================================
// Complex Query Tests
// ============================================================================

#[test]
#[ignore = "Complex multi-direction patterns with <- not yet fully implemented"]
fn test_parse_complex_graph_pattern() {
    let result = parse_cypher(
        "
        MATCH (user:User)-[:PURCHASED]->(product:Product)<-[:PURCHASED]-(other:User)
        WHERE other.id <> 123
        WITH other, COUNT(*) AS commonProducts
        WHERE commonProducts > 3
        RETURN other.name
        ORDER BY commonProducts DESC
        LIMIT 10
    ",
    );
    assert!(result.is_ok());
}

#[test]
fn test_parse_variable_length_path() {
    let result =
        parse_cypher("MATCH (a:Person)-[:KNOWS*1..5]->(b:Person) WHERE a.name = 'Alice' RETURN b");
    assert!(result.is_ok());
}

#[test]
fn test_parse_multiple_patterns() {
    let result = parse_cypher(
        "
        MATCH (a:Person)-[:KNOWS]->(b:Person)
        MATCH (b)-[:WORKS_AT]->(c:Company)
        RETURN a.name, b.name, c.name
    ",
    );
    assert!(result.is_ok());
}

#[test]
fn test_parse_collect_aggregation() {
    let result = parse_cypher(
        "MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p.name, COLLECT(f.name) AS friends",
    );
    assert!(result.is_ok());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
#[ignore = "Empty query validation not yet implemented"]
fn test_parse_empty_query() {
    let result = parse_cypher("");
    // Empty query should fail
    assert!(result.is_err());
}

#[test]
#[ignore = "Whitespace-only query validation not yet implemented"]
fn test_parse_whitespace_only() {
    let result = parse_cypher("   \n\t  ");
    // Whitespace only should fail
    assert!(result.is_err());
}

#[test]
fn test_parse_parameters() {
    let result = parse_cypher("MATCH (n:Person {name: $name, age: $age}) RETURN n");
    assert!(result.is_ok());
}

#[test]
fn test_parse_list_literal() {
    let result = parse_cypher("RETURN [1, 2, 3, 4, 5] AS numbers");
    assert!(result.is_ok());
}

#[test]
#[ignore = "Map literal in RETURN not yet implemented"]
fn test_parse_map_literal() {
    let result = parse_cypher("RETURN {name: 'Alice', age: 30} AS person");
    assert!(result.is_ok());
}
