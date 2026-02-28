//! Node CRUD operation tests
//!
//! Tests for creating, reading, updating, and deleting nodes in the graph database.

use ruvector_graph::{GraphDB, Label, Node, Properties, PropertyValue};

#[test]
fn test_create_node_basic() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert(
        "name".to_string(),
        PropertyValue::String("Alice".to_string()),
    );
    properties.insert("age".to_string(), PropertyValue::Integer(30));

    let node = Node::new(
        "node1".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        properties,
    );

    let node_id = db.create_node(node).unwrap();
    assert_eq!(node_id, "node1");
}

#[test]
fn test_get_node_existing() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert("name".to_string(), PropertyValue::String("Bob".to_string()));

    let node = Node::new(
        "node2".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        properties.clone(),
    );

    db.create_node(node).unwrap();

    let retrieved = db.get_node("node2").unwrap();
    assert_eq!(retrieved.id, "node2");
    assert_eq!(
        retrieved.properties.get("name"),
        Some(&PropertyValue::String("Bob".to_string()))
    );
}

#[test]
fn test_get_node_nonexistent() {
    let db = GraphDB::new();
    let result = db.get_node("nonexistent");
    assert!(result.is_none());
}

#[test]
fn test_node_with_multiple_labels() {
    let db = GraphDB::new();

    let labels = vec![
        Label {
            name: "Person".to_string(),
        },
        Label {
            name: "Employee".to_string(),
        },
        Label {
            name: "Manager".to_string(),
        },
    ];

    let mut properties = Properties::new();
    properties.insert(
        "name".to_string(),
        PropertyValue::String("Charlie".to_string()),
    );

    let node = Node::new("node3".to_string(), labels, properties);
    db.create_node(node).unwrap();

    let retrieved = db.get_node("node3").unwrap();
    assert_eq!(retrieved.labels.len(), 3);
}

#[test]
fn test_node_with_complex_properties() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert(
        "name".to_string(),
        PropertyValue::String("David".to_string()),
    );
    properties.insert("age".to_string(), PropertyValue::Integer(35));
    properties.insert("height".to_string(), PropertyValue::Float(1.82));
    properties.insert("active".to_string(), PropertyValue::Boolean(true));
    properties.insert(
        "tags".to_string(),
        PropertyValue::List(vec![
            PropertyValue::String("developer".to_string()),
            PropertyValue::String("team-lead".to_string()),
        ]),
    );

    let node = Node::new(
        "node4".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        properties,
    );

    db.create_node(node).unwrap();

    let retrieved = db.get_node("node4").unwrap();
    assert_eq!(retrieved.properties.len(), 5);
    assert!(matches!(
        retrieved.properties.get("tags"),
        Some(PropertyValue::List(_))
    ));
}

#[test]
fn test_node_with_empty_properties() {
    let db = GraphDB::new();

    let node = Node::new(
        "node5".to_string(),
        vec![Label {
            name: "EmptyNode".to_string(),
        }],
        Properties::new(),
    );

    db.create_node(node).unwrap();

    let retrieved = db.get_node("node5").unwrap();
    assert!(retrieved.properties.is_empty());
}

#[test]
fn test_node_with_no_labels() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert(
        "data".to_string(),
        PropertyValue::String("test".to_string()),
    );

    let node = Node::new("node6".to_string(), vec![], properties);

    db.create_node(node).unwrap();

    let retrieved = db.get_node("node6").unwrap();
    assert!(retrieved.labels.is_empty());
}

#[test]
fn test_node_property_update() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert("counter".to_string(), PropertyValue::Integer(0));

    let node = Node::new(
        "node7".to_string(),
        vec![Label {
            name: "Counter".to_string(),
        }],
        properties,
    );

    db.create_node(node).unwrap();

    // TODO: Implement update_node method
    // For now, we'll recreate the node with updated properties
    let mut updated_properties = Properties::new();
    updated_properties.insert("counter".to_string(), PropertyValue::Integer(1));

    let updated_node = Node::new(
        "node7".to_string(),
        vec![Label {
            name: "Counter".to_string(),
        }],
        updated_properties,
    );

    db.create_node(updated_node).unwrap();

    let retrieved = db.get_node("node7").unwrap();
    assert_eq!(
        retrieved.properties.get("counter"),
        Some(&PropertyValue::Integer(1))
    );
}

#[test]
fn test_create_1000_nodes() {
    let db = GraphDB::new();

    for i in 0..1000 {
        let mut properties = Properties::new();
        properties.insert("index".to_string(), PropertyValue::Integer(i));

        let node = Node::new(
            format!("node_{}", i),
            vec![Label {
                name: "TestNode".to_string(),
            }],
            properties,
        );

        db.create_node(node).unwrap();
    }

    // Verify all nodes were created
    for i in 0..1000 {
        let retrieved = db.get_node(&format!("node_{}", i));
        assert!(retrieved.is_some());
    }
}

#[test]
fn test_node_property_null_value() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert("nullable".to_string(), PropertyValue::Null);

    let node = Node::new(
        "node8".to_string(),
        vec![Label {
            name: "NullTest".to_string(),
        }],
        properties,
    );

    db.create_node(node).unwrap();

    let retrieved = db.get_node("node8").unwrap();
    assert_eq!(
        retrieved.properties.get("nullable"),
        Some(&PropertyValue::Null)
    );
}

#[test]
fn test_node_nested_list_properties() {
    let db = GraphDB::new();

    let mut properties = Properties::new();
    properties.insert(
        "matrix".to_string(),
        PropertyValue::List(vec![
            PropertyValue::List(vec![PropertyValue::Integer(1), PropertyValue::Integer(2)]),
            PropertyValue::List(vec![PropertyValue::Integer(3), PropertyValue::Integer(4)]),
        ]),
    );

    let node = Node::new(
        "node9".to_string(),
        vec![Label {
            name: "Matrix".to_string(),
        }],
        properties,
    );

    db.create_node(node).unwrap();

    let retrieved = db.get_node("node9").unwrap();
    match retrieved.properties.get("matrix") {
        Some(PropertyValue::List(outer)) => {
            assert_eq!(outer.len(), 2);
            match &outer[0] {
                PropertyValue::List(inner) => assert_eq!(inner.len(), 2),
                _ => panic!("Expected inner list"),
            }
        }
        _ => panic!("Expected outer list"),
    }
}

// ============================================================================
// Property-based tests using proptest
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn node_id_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,20}".prop_map(|s| s.to_string())
    }

    fn label_strategy() -> impl Strategy<Value = Label> {
        "[A-Z][a-zA-Z]{0,10}".prop_map(|name| Label { name })
    }

    fn property_value_strategy() -> impl Strategy<Value = PropertyValue> {
        prop_oneof![
            any::<String>().prop_map(PropertyValue::String),
            any::<i64>().prop_map(PropertyValue::Integer),
            any::<f64>()
                .prop_filter("Must be finite", |x| x.is_finite())
                .prop_map(PropertyValue::Float),
            any::<bool>().prop_map(PropertyValue::Boolean),
            Just(PropertyValue::Null),
        ]
    }

    proptest! {
        #[test]
        fn test_node_roundtrip(
            id in node_id_strategy(),
            labels in prop::collection::vec(label_strategy(), 0..5),
            prop_count in 0..10usize
        ) {
            let db = GraphDB::new();

            let mut properties = Properties::new();
            for i in 0..prop_count {
                properties.insert(
                    format!("prop_{}", i),
                    PropertyValue::String(format!("value_{}", i))
                );
            }

            let node = Node::new(id.clone(), labels.clone(), properties.clone());
            db.create_node(node).unwrap();

            let retrieved = db.get_node(&id).unwrap();
            assert_eq!(retrieved.id, id);
            assert_eq!(retrieved.labels.len(), labels.len());
            assert_eq!(retrieved.properties.len(), properties.len());
        }

        #[test]
        fn test_property_value_consistency(
            value in property_value_strategy()
        ) {
            let db = GraphDB::new();

            let mut properties = Properties::new();
            properties.insert("test_prop".to_string(), value.clone());

            let node = Node::new(
                "test_node".to_string(),
                vec![],
                properties
            );

            db.create_node(node).unwrap();

            let retrieved = db.get_node("test_node").unwrap();
            assert_eq!(retrieved.properties.get("test_prop"), Some(&value));
        }

        #[test]
        fn test_many_nodes_no_collision(
            ids in prop::collection::hash_set(node_id_strategy(), 10..100)
        ) {
            let db = GraphDB::new();

            for id in &ids {
                let node = Node::new(
                    id.clone(),
                    vec![],
                    Properties::new()
                );
                db.create_node(node).unwrap();
            }

            for id in &ids {
                assert!(db.get_node(id).is_some());
            }
        }
    }
}
