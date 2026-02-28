//! Performance and regression tests
//!
//! Benchmark tests to ensure performance doesn't degrade over time.

use ruvector_graph::{Edge, GraphDB, Label, Node, Properties, PropertyValue};
use std::time::Instant;

// ============================================================================
// Baseline Performance Tests
// ============================================================================

#[test]
fn test_node_creation_performance() {
    let db = GraphDB::new();
    let num_nodes = 10_000;

    let start = Instant::now();

    for i in 0..num_nodes {
        let mut props = Properties::new();
        props.insert("id".to_string(), PropertyValue::Integer(i));

        let node = Node::new(
            format!("node_{}", i),
            vec![Label {
                name: "Benchmark".to_string(),
            }],
            props,
        );

        db.create_node(node).unwrap();
    }

    let duration = start.elapsed();

    println!("Created {} nodes in {:?}", num_nodes, duration);
    println!(
        "Rate: {:.2} nodes/sec",
        num_nodes as f64 / duration.as_secs_f64()
    );

    // Baseline: Should create at least 10k nodes/sec
    assert!(
        duration.as_secs() < 5,
        "Node creation too slow: {:?}",
        duration
    );
}

#[test]
fn test_node_retrieval_performance() {
    let db = GraphDB::new();
    let num_nodes = 10_000;

    // Setup
    for i in 0..num_nodes {
        db.create_node(Node::new(format!("node_{}", i), vec![], Properties::new()))
            .unwrap();
    }

    // Measure retrieval
    let start = Instant::now();

    for i in 0..num_nodes {
        let node = db.get_node(&format!("node_{}", i));
        assert!(node.is_some());
    }

    let duration = start.elapsed();

    println!("Retrieved {} nodes in {:?}", num_nodes, duration);
    println!(
        "Rate: {:.2} reads/sec",
        num_nodes as f64 / duration.as_secs_f64()
    );

    // Should be very fast for in-memory lookups
    assert!(
        duration.as_secs() < 1,
        "Node retrieval too slow: {:?}",
        duration
    );
}

#[test]
fn test_edge_creation_performance() {
    let db = GraphDB::new();
    let num_nodes = 1000;
    let edges_per_node = 10;

    // Create nodes
    for i in 0..num_nodes {
        db.create_node(Node::new(format!("n{}", i), vec![], Properties::new()))
            .unwrap();
    }

    // Create edges
    let start = Instant::now();

    for i in 0..num_nodes {
        for j in 0..edges_per_node {
            let to = (i + j + 1) % num_nodes;
            let edge = Edge::new(
                format!("e_{}_{}", i, j),
                format!("n{}", i),
                format!("n{}", to),
                "CONNECTS".to_string(),
                Properties::new(),
            );

            db.create_edge(edge).unwrap();
        }
    }

    let duration = start.elapsed();
    let total_edges = num_nodes * edges_per_node;

    println!("Created {} edges in {:?}", total_edges, duration);
    println!(
        "Rate: {:.2} edges/sec",
        total_edges as f64 / duration.as_secs_f64()
    );
}

// TODO: Implement graph traversal methods
// #[test]
// fn test_traversal_performance() {
//     let db = GraphDB::new();
//     let num_nodes = 1000;
//
//     // Create chain
//     for i in 0..num_nodes {
//         db.create_node(Node::new(format!("n{}", i), vec![], Properties::new())).unwrap();
//     }
//
//     for i in 0..num_nodes - 1 {
//         db.create_edge(Edge::new(
//             format!("e{}", i),
//             format!("n{}", i),
//             format!("n{}", i + 1),
//             RelationType { name: "NEXT".to_string() },
//             Properties::new(),
//         )).unwrap();
//     }
//
//     // Measure traversal
//     let start = Instant::now();
//     let path = db.traverse("n0", "NEXT", 100).unwrap();
//     let duration = start.elapsed();
//
//     assert_eq!(path.len(), 100);
//     println!("Traversed 100 hops in {:?}", duration);
// }

// ============================================================================
// Scalability Tests
// ============================================================================

#[test]
fn test_large_graph_creation() {
    let db = GraphDB::new();
    let num_nodes = 100_000;

    let start = Instant::now();

    for i in 0..num_nodes {
        if i % 10_000 == 0 {
            println!("Created {} nodes...", i);
        }

        let node = Node::new(format!("large_{}", i), vec![], Properties::new());

        db.create_node(node).unwrap();
    }

    let duration = start.elapsed();

    println!("Created {} nodes in {:?}", num_nodes, duration);
    println!(
        "Rate: {:.2} nodes/sec",
        num_nodes as f64 / duration.as_secs_f64()
    );
}

#[test]
#[ignore] // Long-running test
fn test_million_node_graph() {
    let db = GraphDB::new();
    let num_nodes = 1_000_000;

    let start = Instant::now();

    for i in 0..num_nodes {
        if i % 100_000 == 0 {
            println!("Created {} nodes...", i);
        }

        let node = Node::new(format!("mega_{}", i), vec![], Properties::new());

        db.create_node(node).unwrap();
    }

    let duration = start.elapsed();

    println!("Created {} nodes in {:?}", num_nodes, duration);
    println!(
        "Rate: {:.2} nodes/sec",
        num_nodes as f64 / duration.as_secs_f64()
    );
}

// ============================================================================
// Memory Usage Tests
// ============================================================================

#[test]
fn test_memory_efficiency() {
    let db = GraphDB::new();
    let num_nodes = 10_000;

    for i in 0..num_nodes {
        let mut props = Properties::new();
        props.insert("data".to_string(), PropertyValue::String("x".repeat(100)));

        let node = Node::new(format!("mem_{}", i), vec![], props);

        db.create_node(node).unwrap();
    }

    // TODO: Measure actual memory usage
    // This would require platform-specific APIs
}

// ============================================================================
// Property-based Performance Tests
// ============================================================================

#[test]
fn test_property_heavy_nodes() {
    let db = GraphDB::new();
    let num_nodes = 1_000;
    let props_per_node = 50;

    let start = Instant::now();

    for i in 0..num_nodes {
        let mut props = Properties::new();

        for j in 0..props_per_node {
            props.insert(format!("prop_{}", j), PropertyValue::Integer(j as i64));
        }

        let node = Node::new(format!("heavy_{}", i), vec![], props);

        db.create_node(node).unwrap();
    }

    let duration = start.elapsed();

    println!(
        "Created {} property-heavy nodes in {:?}",
        num_nodes, duration
    );
}

// ============================================================================
// Query Performance Tests (TODO)
// ============================================================================

// #[test]
// fn test_simple_query_performance() {
//     let db = setup_benchmark_graph(10_000);
//
//     let start = Instant::now();
//     let results = db.execute("MATCH (n:Person) RETURN n LIMIT 100").unwrap();
//     let duration = start.elapsed();
//
//     assert_eq!(results.len(), 100);
//     println!("Simple query took: {:?}", duration);
// }

// #[test]
// fn test_aggregation_performance() {
//     let db = setup_benchmark_graph(100_000);
//
//     let start = Instant::now();
//     let results = db.execute("MATCH (n:Person) RETURN COUNT(n)").unwrap();
//     let duration = start.elapsed();
//
//     println!("Aggregation over 100k nodes took: {:?}", duration);
// }

// #[test]
// fn test_join_performance() {
//     let db = setup_benchmark_graph(10_000);
//
//     let start = Instant::now();
//     let results = db.execute("
//         MATCH (a:Person)-[:KNOWS]->(b:Person)
//         WHERE a.age > 30
//         RETURN a, b
//     ").unwrap();
//     let duration = start.elapsed();
//
//     println!("Join query took: {:?}", duration);
// }

// ============================================================================
// Index Performance Tests (TODO)
// ============================================================================

// #[test]
// fn test_indexed_lookup_performance() {
//     let db = GraphDB::new();
//
//     // Create index
//     db.create_index("Person", "email").unwrap();
//
//     // Insert data
//     for i in 0..100_000 {
//         db.execute(&format!(
//             "CREATE (:Person {{email: 'user{}@example.com'}})",
//             i
//         )).unwrap();
//     }
//
//     // Measure lookup
//     let start = Instant::now();
//     let results = db.execute("MATCH (n:Person {email: 'user50000@example.com'}) RETURN n").unwrap();
//     let duration = start.elapsed();
//
//     assert_eq!(results.len(), 1);
//     println!("Indexed lookup took: {:?}", duration);
//     assert!(duration.as_millis() < 10); // Should be very fast
// }

// ============================================================================
// Regression Tests
// ============================================================================

#[test]
fn test_regression_node_creation() {
    let db = GraphDB::new();

    let start = Instant::now();

    for i in 0..1000 {
        db.create_node(Node::new(format!("regr_{}", i), vec![], Properties::new()))
            .unwrap();
    }

    let duration = start.elapsed();

    // Baseline threshold - should not regress beyond this
    // Adjust based on baseline measurements
    assert!(
        duration.as_millis() < 500,
        "Regression detected: {:?}",
        duration
    );
}

#[test]
fn test_regression_node_retrieval() {
    let db = GraphDB::new();

    // Setup
    for i in 0..1000 {
        db.create_node(Node::new(format!("regr_{}", i), vec![], Properties::new()))
            .unwrap();
    }

    let start = Instant::now();

    for i in 0..1000 {
        let _ = db.get_node(&format!("regr_{}", i));
    }

    let duration = start.elapsed();

    // Should be very fast
    assert!(
        duration.as_millis() < 100,
        "Regression detected: {:?}",
        duration
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

#[allow(dead_code)]
fn setup_benchmark_graph(num_nodes: usize) -> GraphDB {
    let db = GraphDB::new();

    for i in 0..num_nodes {
        let mut props = Properties::new();
        props.insert(
            "name".to_string(),
            PropertyValue::String(format!("Person{}", i)),
        );
        props.insert(
            "age".to_string(),
            PropertyValue::Integer((20 + (i % 60)) as i64),
        );

        db.create_node(Node::new(
            format!("person_{}", i),
            vec![Label {
                name: "Person".to_string(),
            }],
            props,
        ))
        .unwrap();
    }

    // Create some edges
    for i in 0..num_nodes / 10 {
        let from = i;
        let to = (i + 1) % num_nodes;

        db.create_edge(Edge::new(
            format!("knows_{}", i),
            format!("person_{}", from),
            format!("person_{}", to),
            "KNOWS".to_string(),
            Properties::new(),
        ))
        .unwrap();
    }

    db
}
