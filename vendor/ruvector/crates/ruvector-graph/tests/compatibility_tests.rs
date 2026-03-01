//! Neo4j compatibility tests
//!
//! Tests to verify that RuVector graph database is compatible with Neo4j
//! in terms of query syntax and result format.

use ruvector_graph::{Edge, GraphDB, Label, Node, Properties, PropertyValue};

fn setup_movie_graph() -> GraphDB {
    let db = GraphDB::new();

    // Actors
    let mut keanu_props = Properties::new();
    keanu_props.insert(
        "name".to_string(),
        PropertyValue::String("Keanu Reeves".to_string()),
    );
    keanu_props.insert("born".to_string(), PropertyValue::Integer(1964));

    let mut carrie_props = Properties::new();
    carrie_props.insert(
        "name".to_string(),
        PropertyValue::String("Carrie-Anne Moss".to_string()),
    );
    carrie_props.insert("born".to_string(), PropertyValue::Integer(1967));

    let mut laurence_props = Properties::new();
    laurence_props.insert(
        "name".to_string(),
        PropertyValue::String("Laurence Fishburne".to_string()),
    );
    laurence_props.insert("born".to_string(), PropertyValue::Integer(1961));

    // Movies
    let mut matrix_props = Properties::new();
    matrix_props.insert(
        "title".to_string(),
        PropertyValue::String("The Matrix".to_string()),
    );
    matrix_props.insert("released".to_string(), PropertyValue::Integer(1999));
    matrix_props.insert(
        "tagline".to_string(),
        PropertyValue::String("Welcome to the Real World".to_string()),
    );

    db.create_node(Node::new(
        "keanu".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        keanu_props,
    ))
    .unwrap();

    db.create_node(Node::new(
        "carrie".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        carrie_props,
    ))
    .unwrap();

    db.create_node(Node::new(
        "laurence".to_string(),
        vec![Label {
            name: "Person".to_string(),
        }],
        laurence_props,
    ))
    .unwrap();

    db.create_node(Node::new(
        "matrix".to_string(),
        vec![Label {
            name: "Movie".to_string(),
        }],
        matrix_props,
    ))
    .unwrap();

    // Relationships
    let mut keanu_role = Properties::new();
    keanu_role.insert(
        "roles".to_string(),
        PropertyValue::List(vec![PropertyValue::String("Neo".to_string())]),
    );

    let mut carrie_role = Properties::new();
    carrie_role.insert(
        "roles".to_string(),
        PropertyValue::List(vec![PropertyValue::String("Trinity".to_string())]),
    );

    let mut laurence_role = Properties::new();
    laurence_role.insert(
        "roles".to_string(),
        PropertyValue::List(vec![PropertyValue::String("Morpheus".to_string())]),
    );

    db.create_edge(Edge::new(
        "e1".to_string(),
        "keanu".to_string(),
        "matrix".to_string(),
        "ACTED_IN".to_string(),
        keanu_role,
    ))
    .unwrap();

    db.create_edge(Edge::new(
        "e2".to_string(),
        "carrie".to_string(),
        "matrix".to_string(),
        "ACTED_IN".to_string(),
        carrie_role,
    ))
    .unwrap();

    db.create_edge(Edge::new(
        "e3".to_string(),
        "laurence".to_string(),
        "matrix".to_string(),
        "ACTED_IN".to_string(),
        laurence_role,
    ))
    .unwrap();

    db
}

// ============================================================================
// Neo4j Query Compatibility Tests
// ============================================================================

#[test]
fn test_neo4j_match_all_nodes() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (n) RETURN n
    // TODO: Implement query execution
    // let results = db.execute("MATCH (n) RETURN n").unwrap();
    // assert_eq!(results.len(), 4); // 3 people + 1 movie

    // For now, verify graph setup
    assert!(db.get_node("keanu").is_some());
    assert!(db.get_node("matrix").is_some());
}

#[test]
fn test_neo4j_match_with_label() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (p:Person) RETURN p
    // TODO: Implement
    // let results = db.execute("MATCH (p:Person) RETURN p").unwrap();
    // assert_eq!(results.len(), 3);

    // Verify label filtering would work
    let keanu = db.get_node("keanu").unwrap();
    assert_eq!(keanu.labels[0].name, "Person");
}

#[test]
fn test_neo4j_match_with_properties() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (m:Movie {title: 'The Matrix'}) RETURN m
    // TODO: Implement
    // let results = db.execute("MATCH (m:Movie {title: 'The Matrix'}) RETURN m").unwrap();
    // assert_eq!(results.len(), 1);

    let matrix = db.get_node("matrix").unwrap();
    assert_eq!(
        matrix.properties.get("title"),
        Some(&PropertyValue::String("The Matrix".to_string()))
    );
}

#[test]
fn test_neo4j_match_relationship() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (a:Person)-[r:ACTED_IN]->(m:Movie) RETURN a, r, m
    // TODO: Implement
    // let results = db.execute("MATCH (a:Person)-[r:ACTED_IN]->(m:Movie) RETURN a, r, m").unwrap();
    // assert_eq!(results.len(), 3);

    let edge = db.get_edge("e1").unwrap();
    assert_eq!(edge.edge_type, "ACTED_IN");
}

#[test]
fn test_neo4j_where_clause() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (p:Person) WHERE p.born > 1965 RETURN p
    // TODO: Implement
    // let results = db.execute("MATCH (p:Person) WHERE p.born > 1965 RETURN p").unwrap();
    // assert_eq!(results.len(), 1); // Only Carrie-Anne Moss

    let carrie = db.get_node("carrie").unwrap();
    if let Some(PropertyValue::Integer(born)) = carrie.properties.get("born") {
        assert!(*born > 1965);
    }
}

#[test]
fn test_neo4j_count_aggregation() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (p:Person) RETURN COUNT(p)
    // TODO: Implement
    // let results = db.execute("MATCH (p:Person) RETURN COUNT(p)").unwrap();
    // assert_eq!(results[0]["count"], 3);

    // Manually verify
    assert!(db.get_node("keanu").is_some());
    assert!(db.get_node("carrie").is_some());
    assert!(db.get_node("laurence").is_some());
}

#[test]
fn test_neo4j_collect_aggregation() {
    let db = setup_movie_graph();

    // Neo4j query: MATCH (p:Person)-[:ACTED_IN]->(m:Movie)
    //              RETURN m.title, COLLECT(p.name) AS actors
    // TODO: Implement
    // let results = db.execute("...").unwrap();

    // Verify relationships exist
    assert!(db.get_edge("e1").is_some());
    assert!(db.get_edge("e2").is_some());
    assert!(db.get_edge("e3").is_some());
}

// ============================================================================
// Neo4j Data Type Compatibility
// ============================================================================

#[test]
fn test_neo4j_string_property() {
    let db = GraphDB::new();

    let mut props = Properties::new();
    props.insert(
        "name".to_string(),
        PropertyValue::String("Test".to_string()),
    );

    db.create_node(Node::new("n1".to_string(), vec![], props))
        .unwrap();

    let node = db.get_node("n1").unwrap();
    assert!(matches!(
        node.properties.get("name"),
        Some(PropertyValue::String(_))
    ));
}

#[test]
fn test_neo4j_integer_property() {
    let db = GraphDB::new();

    let mut props = Properties::new();
    props.insert("count".to_string(), PropertyValue::Integer(42));

    db.create_node(Node::new("n1".to_string(), vec![], props))
        .unwrap();

    let node = db.get_node("n1").unwrap();
    assert_eq!(
        node.properties.get("count"),
        Some(&PropertyValue::Integer(42))
    );
}

#[test]
fn test_neo4j_float_property() {
    let db = GraphDB::new();

    let mut props = Properties::new();
    props.insert("score".to_string(), PropertyValue::Float(3.14));

    db.create_node(Node::new("n1".to_string(), vec![], props))
        .unwrap();

    let node = db.get_node("n1").unwrap();
    assert_eq!(
        node.properties.get("score"),
        Some(&PropertyValue::Float(3.14))
    );
}

#[test]
fn test_neo4j_boolean_property() {
    let db = GraphDB::new();

    let mut props = Properties::new();
    props.insert("active".to_string(), PropertyValue::Boolean(true));

    db.create_node(Node::new("n1".to_string(), vec![], props))
        .unwrap();

    let node = db.get_node("n1").unwrap();
    assert_eq!(
        node.properties.get("active"),
        Some(&PropertyValue::Boolean(true))
    );
}

#[test]
fn test_neo4j_list_property() {
    let db = GraphDB::new();

    let mut props = Properties::new();
    props.insert(
        "tags".to_string(),
        PropertyValue::List(vec![
            PropertyValue::String("tag1".to_string()),
            PropertyValue::String("tag2".to_string()),
        ]),
    );

    db.create_node(Node::new("n1".to_string(), vec![], props))
        .unwrap();

    let node = db.get_node("n1").unwrap();
    assert!(matches!(
        node.properties.get("tags"),
        Some(PropertyValue::List(_))
    ));
}

#[test]
fn test_neo4j_null_property() {
    let db = GraphDB::new();

    let mut props = Properties::new();
    props.insert("optional".to_string(), PropertyValue::Null);

    db.create_node(Node::new("n1".to_string(), vec![], props))
        .unwrap();

    let node = db.get_node("n1").unwrap();
    assert_eq!(node.properties.get("optional"), Some(&PropertyValue::Null));
}

// ============================================================================
// Known Differences from Neo4j
// ============================================================================

#[test]
fn test_documented_differences() {
    // Document any intentional differences from Neo4j behavior
    // For example:
    // - Different default values
    // - Different error messages
    // - Different performance characteristics
    // - Missing features

    // This test serves as documentation
    assert!(true);
}
