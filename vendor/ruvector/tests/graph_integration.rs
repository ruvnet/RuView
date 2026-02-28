//! Integration tests for RuVector graph database
//!
//! End-to-end tests that verify all components work together correctly.

use ruvector_graph::{GraphDB, Node, Edge, Label, RelationType, Properties, PropertyValue};

// ============================================================================
// Full Workflow Integration Tests
// ============================================================================

#[test]
fn test_complete_graph_workflow() {
    let db = GraphDB::new();

    // 1. Create nodes
    let mut alice_props = Properties::new();
    alice_props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
    alice_props.insert("age".to_string(), PropertyValue::Integer(30));

    let mut bob_props = Properties::new();
    bob_props.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
    bob_props.insert("age".to_string(), PropertyValue::Integer(35));

    db.create_node(Node::new(
        "alice".to_string(),
        vec![Label { name: "Person".to_string() }],
        alice_props,
    )).unwrap();

    db.create_node(Node::new(
        "bob".to_string(),
        vec![Label { name: "Person".to_string() }],
        bob_props,
    )).unwrap();

    // 2. Create relationship
    let mut edge_props = Properties::new();
    edge_props.insert("since".to_string(), PropertyValue::Integer(2020));

    db.create_edge(Edge::new(
        "knows".to_string(),
        "alice".to_string(),
        "bob".to_string(),
        RelationType { name: "KNOWS".to_string() },
        edge_props,
    )).unwrap();

    // 3. Verify everything was created
    let alice = db.get_node("alice").unwrap();
    let bob = db.get_node("bob").unwrap();
    let edge = db.get_edge("knows").unwrap();

    assert_eq!(alice.labels[0].name, "Person");
    assert_eq!(bob.labels[0].name, "Person");
    assert_eq!(edge.rel_type.name, "KNOWS");
    assert_eq!(edge.from_node, "alice");
    assert_eq!(edge.to_node, "bob");
}

#[test]
fn test_social_network_scenario() {
    let db = GraphDB::new();

    // Create users
    for i in 0..10 {
        let mut props = Properties::new();
        props.insert("username".to_string(), PropertyValue::String(format!("user{}", i)));
        props.insert("followers".to_string(), PropertyValue::Integer(i * 100));

        db.create_node(Node::new(
            format!("user{}", i),
            vec![Label { name: "User".to_string() }],
            props,
        )).unwrap();
    }

    // Create follow relationships
    for i in 0..9 {
        let edge = Edge::new(
            format!("follow_{}", i),
            format!("user{}", i),
            format!("user{}", i + 1),
            RelationType { name: "FOLLOWS".to_string() },
            Properties::new(),
        );

        db.create_edge(edge).unwrap();
    }

    // Verify graph structure
    for i in 0..10 {
        assert!(db.get_node(&format!("user{}", i)).is_some());
    }

    for i in 0..9 {
        assert!(db.get_edge(&format!("follow_{}", i)).is_some());
    }
}

#[test]
fn test_movie_database_scenario() {
    let db = GraphDB::new();

    // Create movies
    let mut inception_props = Properties::new();
    inception_props.insert("title".to_string(), PropertyValue::String("Inception".to_string()));
    inception_props.insert("year".to_string(), PropertyValue::Integer(2010));
    inception_props.insert("rating".to_string(), PropertyValue::Float(8.8));

    db.create_node(Node::new(
        "inception".to_string(),
        vec![Label { name: "Movie".to_string() }],
        inception_props,
    )).unwrap();

    // Create actors
    let mut dicaprio_props = Properties::new();
    dicaprio_props.insert("name".to_string(), PropertyValue::String("Leonardo DiCaprio".to_string()));

    db.create_node(Node::new(
        "dicaprio".to_string(),
        vec![Label { name: "Actor".to_string() }],
        dicaprio_props,
    )).unwrap();

    // Create ACTED_IN relationship
    let mut role_props = Properties::new();
    role_props.insert("character".to_string(), PropertyValue::String("Cobb".to_string()));

    db.create_edge(Edge::new(
        "acted1".to_string(),
        "dicaprio".to_string(),
        "inception".to_string(),
        RelationType { name: "ACTED_IN".to_string() },
        role_props,
    )).unwrap();

    // Verify
    let movie = db.get_node("inception").unwrap();
    let actor = db.get_node("dicaprio").unwrap();
    let role = db.get_edge("acted1").unwrap();

    assert!(movie.properties.contains_key("title"));
    assert!(actor.properties.contains_key("name"));
    assert_eq!(role.rel_type.name, "ACTED_IN");
}

#[test]
fn test_knowledge_graph_scenario() {
    let db = GraphDB::new();

    // Create concepts
    let concepts = vec![
        ("ml", "Machine Learning"),
        ("ai", "Artificial Intelligence"),
        ("dl", "Deep Learning"),
        ("nn", "Neural Networks"),
    ];

    for (id, name) in concepts {
        let mut props = Properties::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));

        db.create_node(Node::new(
            id.to_string(),
            vec![Label { name: "Concept".to_string() }],
            props,
        )).unwrap();
    }

    // Create relationships
    db.create_edge(Edge::new(
        "e1".to_string(),
        "dl".to_string(),
        "ml".to_string(),
        RelationType { name: "IS_A".to_string() },
        Properties::new(),
    )).unwrap();

    db.create_edge(Edge::new(
        "e2".to_string(),
        "ml".to_string(),
        "ai".to_string(),
        RelationType { name: "IS_A".to_string() },
        Properties::new(),
    )).unwrap();

    db.create_edge(Edge::new(
        "e3".to_string(),
        "dl".to_string(),
        "nn".to_string(),
        RelationType { name: "USES".to_string() },
        Properties::new(),
    )).unwrap();

    // Verify concept hierarchy
    assert!(db.get_node("ai").is_some());
    assert!(db.get_edge("e1").is_some());
}

// ============================================================================
// Complex Multi-Step Operations
// ============================================================================

#[test]
fn test_batch_import() {
    let db = GraphDB::new();

    // Simulate importing a batch of data
    let nodes_to_import = 100;

    for i in 0..nodes_to_import {
        let mut props = Properties::new();
        props.insert("id".to_string(), PropertyValue::Integer(i));
        props.insert("type".to_string(), PropertyValue::String("imported".to_string()));

        let node = Node::new(
            format!("import_{}", i),
            vec![Label { name: "Imported".to_string() }],
            props,
        );

        db.create_node(node).unwrap();
    }

    // Verify all were imported
    for i in 0..nodes_to_import {
        assert!(db.get_node(&format!("import_{}", i)).is_some());
    }
}

#[test]
fn test_graph_transformation() {
    let db = GraphDB::new();

    // Original graph: Linear chain
    for i in 0..10 {
        db.create_node(Node::new(
            format!("n{}", i),
            vec![],
            Properties::new(),
        )).unwrap();
    }

    for i in 0..9 {
        db.create_edge(Edge::new(
            format!("e{}", i),
            format!("n{}", i),
            format!("n{}", i + 1),
            RelationType { name: "NEXT".to_string() },
            Properties::new(),
        )).unwrap();
    }

    // Transform: Add reverse edges
    for i in 0..9 {
        db.create_edge(Edge::new(
            format!("rev_e{}", i),
            format!("n{}", i + 1),
            format!("n{}", i),
            RelationType { name: "PREV".to_string() },
            Properties::new(),
        )).unwrap();
    }

    // Verify bidirectional graph
    assert!(db.get_edge("e5").is_some());
    assert!(db.get_edge("rev_e5").is_some());
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

#[test]
fn test_handle_missing_nodes_gracefully() {
    let db = GraphDB::new();

    // Try to get non-existent node
    let result = db.get_node("does_not_exist");
    assert!(result.is_none());

    // Try to get non-existent edge
    let result = db.get_edge("missing_edge");
    assert!(result.is_none());
}

#[test]
fn test_duplicate_id_handling() {
    let db = GraphDB::new();

    let node1 = Node::new(
        "duplicate".to_string(),
        vec![],
        Properties::new(),
    );

    db.create_node(node1).unwrap();

    // Try to create node with same ID
    let node2 = Node::new(
        "duplicate".to_string(),
        vec![],
        Properties::new(),
    );

    // This should either update or error, depending on desired semantics
    // For now, just verify it doesn't panic
    let _result = db.create_node(node2);
}

// ============================================================================
// Data Integrity Tests
// ============================================================================

#[test]
fn test_edge_referential_integrity() {
    let db = GraphDB::new();

    // Create nodes
    db.create_node(Node::new("a".to_string(), vec![], Properties::new())).unwrap();
    db.create_node(Node::new("b".to_string(), vec![], Properties::new())).unwrap();

    // Create valid edge
    let edge = Edge::new(
        "e1".to_string(),
        "a".to_string(),
        "b".to_string(),
        RelationType { name: "LINKS".to_string() },
        Properties::new(),
    );

    let result = db.create_edge(edge);
    assert!(result.is_ok());

    // Try to create edge with non-existent source
    // Note: Current implementation doesn't check this, but it should
    let bad_edge = Edge::new(
        "e2".to_string(),
        "nonexistent".to_string(),
        "b".to_string(),
        RelationType { name: "LINKS".to_string() },
        Properties::new(),
    );

    let _result = db.create_edge(bad_edge);
    // TODO: Should fail with error
}

// ============================================================================
// Performance Integration Tests
// ============================================================================

#[test]
fn test_large_graph_operations() {
    let db = GraphDB::new();

    let num_nodes = 1000;
    let num_edges = 5000;

    // Create nodes
    for i in 0..num_nodes {
        db.create_node(Node::new(
            format!("large_{}", i),
            vec![],
            Properties::new(),
        )).unwrap();
    }

    // Create edges
    for i in 0..num_edges {
        let from = i % num_nodes;
        let to = (i + 1) % num_nodes;

        db.create_edge(Edge::new(
            format!("e_{}", i),
            format!("large_{}", from),
            format!("large_{}", to),
            RelationType { name: "EDGE".to_string() },
            Properties::new(),
        )).unwrap();
    }

    // Verify graph size
    // TODO: Add methods to get counts
}
