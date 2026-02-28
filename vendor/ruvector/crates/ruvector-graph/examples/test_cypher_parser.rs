//! Standalone example demonstrating the Cypher parser
//! Run with: cargo run --example test_cypher_parser

use ruvector_graph::cypher::{ast::*, parse_cypher};

fn main() {
    println!("=== Cypher Parser Test Suite ===\n");

    // Test 1: Simple MATCH
    println!("Test 1: Simple MATCH query");
    let query1 = "MATCH (n:Person) RETURN n";
    match parse_cypher(query1) {
        Ok(ast) => {
            println!("✓ Parsed successfully");
            println!("  Statements: {}", ast.statements.len());
            println!("  Read-only: {}", ast.is_read_only());
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 2: MATCH with WHERE
    println!("Test 2: MATCH with WHERE clause");
    let query2 = "MATCH (n:Person) WHERE n.age > 30 RETURN n.name";
    match parse_cypher(query2) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 3: Relationship pattern
    println!("Test 3: Relationship pattern");
    let query3 = "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a, r, b";
    match parse_cypher(query3) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 4: CREATE node
    println!("Test 4: CREATE node");
    let query4 = "CREATE (n:Person {name: 'Alice', age: 30})";
    match parse_cypher(query4) {
        Ok(ast) => {
            println!("✓ Parsed successfully");
            println!("  Read-only: {}", ast.is_read_only());
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 5: Hyperedge (N-ary relationship)
    println!("Test 5: Hyperedge pattern");
    let query5 = "MATCH (a)-[r:TRANSACTION]->(b, c, d) RETURN a, r, b, c, d";
    match parse_cypher(query5) {
        Ok(ast) => {
            println!("✓ Parsed successfully");
            println!("  Has hyperedges: {}", ast.has_hyperedges());
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 6: Aggregation functions
    println!("Test 6: Aggregation functions");
    let query6 = "MATCH (n:Person) RETURN COUNT(n), AVG(n.age), MAX(n.salary)";
    match parse_cypher(query6) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 7: Complex query with ORDER BY and LIMIT
    println!("Test 7: Complex query with ORDER BY and LIMIT");
    let query7 = r#"
        MATCH (a:Person)-[r:KNOWS]->(b:Person)
        WHERE a.age > 30 AND b.name = 'Alice'
        RETURN a.name, b.name, r.since
        ORDER BY r.since DESC
        LIMIT 10
    "#;
    match parse_cypher(query7) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 8: MERGE with ON CREATE
    println!("Test 8: MERGE with ON CREATE");
    let query8 = "MERGE (n:Person {name: 'Bob'}) ON CREATE SET n.created = 2024";
    match parse_cypher(query8) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 9: WITH clause (query chaining)
    println!("Test 9: WITH clause");
    let query9 = r#"
        MATCH (n:Person)
        WITH n, n.age AS age
        WHERE age > 30
        RETURN n.name, age
    "#;
    match parse_cypher(query9) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    // Test 10: Variable-length path
    println!("Test 10: Variable-length path");
    let query10 = "MATCH p = (a:Person)-[*1..3]->(b:Person) RETURN p";
    match parse_cypher(query10) {
        Ok(_) => println!("✓ Parsed successfully"),
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();

    println!("=== All tests completed ===");
}
