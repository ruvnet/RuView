//! Edge (relationship) operation tests
//!
//! Tests for creating edges, querying relationships, and graph traversals.

use ruvector_graph::{Edge, EdgeBuilder, GraphDB, Label, Node, Properties, PropertyValue};

#[test]
fn test_create_edge_basic() {
    let db = GraphDB::new();

    // Create nodes first
    let node1 = Node::new(
        "person1".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        Properties::new(),
    );
    let node2 = Node::new(
        "person2".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        Properties::new(),
    );

    db.create_node(node1).unwrap();
    db.create_node(node2).unwrap();

    // Create edge
    let edge = Edge::new(
        "edge1".to_string(),
        "person1".to_string(),
        "person2".to_string(),
        "KNOWS".to_string(),
        Properties::new(),
    );

    let edge_id = db.create_edge(edge).unwrap();
    assert_eq!(edge_id, "edge1");
}

#[test]
fn test_get_edge_existing() {
    let db = GraphDB::new();

    // Setup nodes
    let node1 = Node::new("n1".to_string(), vec![], Properties::new());
    let node2 = Node::new("n2".to_string(), vec![], Properties::new());
    db.create_node(node1).unwrap();
    db.create_node(node2).unwrap();

    // Create edge with properties
    let mut properties = Properties::new();
    properties.insert("since".to_string(), PropertyValue::Integer(2020));

    let edge = Edge::new(
        "e1".to_string(),
        "n1".to_string(),
        "n2".to_string(),
        "FRIEND_OF".to_string(),
        properties,
    );

    db.create_edge(edge).unwrap();

    let retrieved = db.get_edge("e1").unwrap();
    assert_eq!(retrieved.id, "e1");
    assert_eq!(retrieved.from, "n1");
    assert_eq!(retrieved.to, "n2");
    assert_eq!(retrieved.edge_type, "FRIEND_OF");
}

#[test]
fn test_edge_with_properties() {
    let db = GraphDB::new();

    // Setup
    db.create_node(Node::new("a".to_string(), vec![], Properties::new()))
        .unwrap();
    db.create_node(Node::new("b".to_string(), vec![], Properties::new()))
        .unwrap();

    let mut properties = Properties::new();
    properties.insert("weight".to_string(), PropertyValue::Float(0.85));
    properties.insert(
        "type".to_string(),
        PropertyValue::String("strong".to_string()),
    );
    properties.insert("verified".to_string(), PropertyValue::Boolean(true));

    let edge = Edge::new(
        "weighted_edge".to_string(),
        "a".to_string(),
        "b".to_string(),
        "CONNECTED_TO".to_string(),
        properties,
    );

    db.create_edge(edge).unwrap();

    let retrieved = db.get_edge("weighted_edge").unwrap();
    assert_eq!(
        retrieved.properties.get("weight"),
        Some(&PropertyValue::Float(0.85))
    );
    assert_eq!(
        retrieved.properties.get("verified"),
        Some(&PropertyValue::Boolean(true))
    );
}

#[test]
fn test_bidirectional_edges() {
    let db = GraphDB::new();

    db.create_node(Node::new("alice".to_string(), vec![], Properties::new()))
        .unwrap();
    db.create_node(Node::new("bob".to_string(), vec![], Properties::new()))
        .unwrap();

    // Alice -> Bob
    let edge1 = Edge::new(
        "e1".to_string(),
        "alice".to_string(),
        "bob".to_string(),
        "FOLLOWS".to_string(),
        Properties::new(),
    );

    // Bob -> Alice
    let edge2 = Edge::new(
        "e2".to_string(),
        "bob".to_string(),
        "alice".to_string(),
        "FOLLOWS".to_string(),
        Properties::new(),
    );

    db.create_edge(edge1).unwrap();
    db.create_edge(edge2).unwrap();

    let e1 = db.get_edge("e1").unwrap();
    let e2 = db.get_edge("e2").unwrap();

    assert_eq!(e1.from, "alice");
    assert_eq!(e1.to, "bob");
    assert_eq!(e2.from, "bob");
    assert_eq!(e2.to, "alice");
}

#[test]
fn test_self_loop_edge() {
    let db = GraphDB::new();

    db.create_node(Node::new("node".to_string(), vec![], Properties::new()))
        .unwrap();

    let edge = Edge::new(
        "self_loop".to_string(),
        "node".to_string(),
        "node".to_string(),
        "REFERENCES".to_string(),
        Properties::new(),
    );

    db.create_edge(edge).unwrap();

    let retrieved = db.get_edge("self_loop").unwrap();
    assert_eq!(retrieved.from, retrieved.to);
}

#[test]
fn test_multiple_edges_same_nodes() {
    let db = GraphDB::new();

    db.create_node(Node::new("x".to_string(), vec![], Properties::new()))
        .unwrap();
    db.create_node(Node::new("y".to_string(), vec![], Properties::new()))
        .unwrap();

    // Multiple relationship types between same nodes
    let edge1 = Edge::new(
        "e1".to_string(),
        "x".to_string(),
        "y".to_string(),
        "WORKS_WITH".to_string(),
        Properties::new(),
    );

    let edge2 = Edge::new(
        "e2".to_string(),
        "x".to_string(),
        "y".to_string(),
        "FRIENDS_WITH".to_string(),
        Properties::new(),
    );

    db.create_edge(edge1).unwrap();
    db.create_edge(edge2).unwrap();

    assert!(db.get_edge("e1").is_some());
    assert!(db.get_edge("e2").is_some());
}

#[test]
fn test_edge_timestamp_property() {
    let db = GraphDB::new();

    db.create_node(Node::new("user1".to_string(), vec![], Properties::new()))
        .unwrap();
    db.create_node(Node::new("post1".to_string(), vec![], Properties::new()))
        .unwrap();

    let mut properties = Properties::new();
    properties.insert("timestamp".to_string(), PropertyValue::Integer(1699564800));
    properties.insert(
        "action".to_string(),
        PropertyValue::String("liked".to_string()),
    );

    let edge = Edge::new(
        "interaction".to_string(),
        "user1".to_string(),
        "post1".to_string(),
        "INTERACTED".to_string(),
        properties,
    );

    db.create_edge(edge).unwrap();

    let retrieved = db.get_edge("interaction").unwrap();
    assert!(retrieved.properties.contains_key("timestamp"));
}

#[test]
fn test_get_nonexistent_edge() {
    let db = GraphDB::new();
    let result = db.get_edge("does_not_exist");
    assert!(result.is_none());
}

#[test]
fn test_create_many_edges() {
    let db = GraphDB::new();

    // Create hub node
    db.create_node(Node::new("hub".to_string(), vec![], Properties::new()))
        .unwrap();

    // Create 100 spoke nodes
    for i in 0..100 {
        let node_id = format!("spoke_{}", i);
        db.create_node(Node::new(node_id.clone(), vec![], Properties::new()))
            .unwrap();

        let edge = Edge::new(
            format!("edge_{}", i),
            "hub".to_string(),
            node_id,
            "CONNECTS".to_string(),
            Properties::new(),
        );

        db.create_edge(edge).unwrap();
    }

    // Verify all edges exist
    for i in 0..100 {
        assert!(db.get_edge(&format!("edge_{}", i)).is_some());
    }
}

#[test]
fn test_edge_builder() {
    let db = GraphDB::new();

    db.create_node(Node::new("a".to_string(), vec![], Properties::new()))
        .unwrap();
    db.create_node(Node::new("b".to_string(), vec![], Properties::new()))
        .unwrap();

    let edge = EdgeBuilder::new("a".to_string(), "b".to_string(), "KNOWS")
        .id("e1")
        .property("since", 2020i64)
        .property("weight", 0.95f64)
        .build();

    db.create_edge(edge).unwrap();

    let retrieved = db.get_edge("e1").unwrap();
    assert_eq!(retrieved.from, "a");
    assert_eq!(retrieved.to, "b");
    assert_eq!(retrieved.edge_type, "KNOWS");
    assert_eq!(
        retrieved.get_property("since"),
        Some(&PropertyValue::Integer(2020))
    );
}

// ============================================================================
// Property-based tests
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn edge_id_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,20}".prop_map(|s| s.to_string())
    }

    fn edge_type_strategy() -> impl Strategy<Value = String> {
        "[A-Z_]{2,15}".prop_map(|s| s.to_string())
    }

    proptest! {
        #[test]
        fn test_edge_roundtrip(
            edge_id in edge_id_strategy(),
            edge_type in edge_type_strategy()
        ) {
            let db = GraphDB::new();

            // Setup nodes
            db.create_node(Node::new("from".to_string(), vec![], Properties::new())).unwrap();
            db.create_node(Node::new("to".to_string(), vec![], Properties::new())).unwrap();

            let edge = Edge::new(
                edge_id.clone(),
                "from".to_string(),
                "to".to_string(),
                edge_type.clone(),
                Properties::new(),
            );

            db.create_edge(edge).unwrap();

            let retrieved = db.get_edge(&edge_id).unwrap();
            assert_eq!(retrieved.id, edge_id);
            assert_eq!(retrieved.edge_type, edge_type);
        }

        #[test]
        fn test_many_edges_unique(
            edge_ids in prop::collection::hash_set(edge_id_strategy(), 10..50)
        ) {
            let db = GraphDB::new();

            // Create source and target nodes
            db.create_node(Node::new("source".to_string(), vec![], Properties::new())).unwrap();
            db.create_node(Node::new("target".to_string(), vec![], Properties::new())).unwrap();

            for edge_id in &edge_ids {
                let edge = Edge::new(
                    edge_id.clone(),
                    "source".to_string(),
                    "target".to_string(),
                    "TEST".to_string(),
                    Properties::new(),
                );
                db.create_edge(edge).unwrap();
            }

            for edge_id in &edge_ids {
                assert!(db.get_edge(edge_id).is_some());
            }
        }
    }
}
