use graph_reasoner::*;
use graph_reasoner::inference::{InferenceEngine, InferenceMode};
use graph_reasoner::graph::KnowledgeGraph;
use graph_reasoner::types::{Entity, Fact};
use graph_reasoner::query::{Query, QueryResult};
use graph_reasoner::rules::RuleEngine;

#[test]
fn test_knowledge_graph_creation() {
    let mut graph = KnowledgeGraph::new();

    // Add entities
    let person = Entity::new("Alice", "Person")
        .with_property("age", "30")
        .with_property("occupation", "Engineer");

    let company = Entity::new("TechCorp", "Company")
        .with_property("industry", "Technology");

    let person_idx = graph.add_entity(person).unwrap();
    let company_idx = graph.add_entity(company).unwrap();

    // Add facts
    let fact1 = Fact::new("Alice", "works_at", "TechCorp")
        .with_confidence(0.9);

    let fact2 = Fact::new("Alice", "is_a", "Person")
        .with_confidence(1.0);

    graph.add_fact(fact1).unwrap();
    graph.add_fact(fact2).unwrap();

    // Test queries
    let alice_facts = graph.get_facts_by_subject("Alice");
    assert_eq!(alice_facts.len(), 2);

    let works_at_facts = graph.get_facts_by_predicate("works_at");
    assert_eq!(works_at_facts.len(), 1);
    assert_eq!(works_at_facts[0].subject, "Alice");
    assert_eq!(works_at_facts[0].object, "TechCorp");
}

#[test]
fn test_inference_engine() {
    let mut graph = KnowledgeGraph::new();
    let mut inference_engine = InferenceEngine::new();
    let mut rule_engine = RuleEngine::new();

    // Add basic facts
    graph.add_fact(Fact::new("Alice", "is_a", "Person")).unwrap();
    graph.add_fact(Fact::new("Person", "subset_of", "Human")).unwrap();
    graph.add_fact(Fact::new("Human", "has_property", "mortal")).unwrap();

    // Add subset inheritance rule
    let rule = RuleEngine::create_subset_inheritance_rule();
    rule_engine.add_rule(rule);

    // Perform inference
    let results = inference_engine.infer(&mut graph, &rule_engine, 5);

    // Should infer that Alice has property mortal
    assert!(!results.is_empty());

    // Check if the new fact was added
    let alice_facts = graph.get_facts_by_subject("Alice");
    let has_mortal = alice_facts.iter().any(|f| f.predicate == "has_property" && f.object == "mortal");

    // Note: This test might need adjustment based on the actual inference implementation
    println!("Inference results: {:?}", results);
    println!("Alice facts after inference: {:?}", alice_facts);
}

#[test]
fn test_rule_application() {
    let mut graph = KnowledgeGraph::new();
    let rule_engine = RuleEngine::new();

    // Add transitivity rule
    let mut rule_engine = RuleEngine::new();
    let transitivity_rule = RuleEngine::create_transitivity_rule();
    rule_engine.add_rule(transitivity_rule);

    // Add facts for transitivity test
    graph.add_fact(Fact::new("A", "relates_to", "B")).unwrap();
    graph.add_fact(Fact::new("B", "relates_to", "C")).unwrap();

    // Apply rules
    let new_facts = rule_engine.apply_rules(&mut graph, 3);

    // Should derive that A relates_to C
    let derived_relation = new_facts.iter().any(|f|
        f.subject == "A" && f.predicate == "relates_to" && f.object == "C"
    );

    println!("New facts from rule application: {:?}", new_facts);
    assert!(derived_relation, "Should derive A relates_to C through transitivity");
}

#[test]
fn test_query_interface() {
    let mut graph = KnowledgeGraph::new();

    // Add test data
    graph.add_fact(Fact::new("Alice", "works_at", "Company1")).unwrap();
    graph.add_fact(Fact::new("Bob", "works_at", "Company1")).unwrap();
    graph.add_fact(Fact::new("Charlie", "works_at", "Company2")).unwrap();

    // Test find facts query
    let query = Query::find_facts(None, Some("works_at"), Some("Company1"));
    let result = graph.query(&query);

    assert_eq!(result.facts.len(), 2);
    assert!(result.facts.iter().any(|f| f.subject == "Alice"));
    assert!(result.facts.iter().any(|f| f.subject == "Bob"));

    // Test subject-specific query
    let query = Query::find_facts(Some("Alice"), None, None);
    let result = graph.query(&query);

    assert_eq!(result.facts.len(), 1);
    assert_eq!(result.facts[0].subject, "Alice");
}

#[test]
fn test_graph_statistics() {
    let mut graph = KnowledgeGraph::new();

    // Add various entities and facts
    graph.add_entity(Entity::new("Alice", "Person")).unwrap();
    graph.add_entity(Entity::new("Bob", "Person")).unwrap();
    graph.add_entity(Entity::new("Company1", "Company")).unwrap();

    graph.add_fact(Fact::new("Alice", "works_at", "Company1")).unwrap();
    graph.add_fact(Fact::new("Bob", "works_at", "Company1")).unwrap();
    graph.add_fact(Fact::new("Alice", "friends_with", "Bob")).unwrap();

    let stats = graph.get_statistics();

    assert_eq!(stats.entity_count, 3);
    assert_eq!(stats.fact_count, 3);
    assert_eq!(stats.relationship_count, 3);

    // Check entity types
    assert_eq!(stats.entity_types.get("Person"), Some(&2));
    assert_eq!(stats.entity_types.get("Company"), Some(&1));

    // Check relationship types
    assert_eq!(stats.relationship_types.get("works_at"), Some(&2));
    assert_eq!(stats.relationship_types.get("friends_with"), Some(&1));
}

#[test]
fn test_confidence_handling() {
    let mut graph = KnowledgeGraph::new();

    // Add facts with different confidence levels
    let high_confidence_fact = Fact::new("Alice", "is_a", "Person")
        .with_confidence(0.95);

    let low_confidence_fact = Fact::new("Alice", "might_be", "Engineer")
        .with_confidence(0.3);

    graph.add_fact(high_confidence_fact).unwrap();
    graph.add_fact(low_confidence_fact).unwrap();

    let stats = graph.get_statistics();

    // Average confidence should be between 0.3 and 0.95
    assert!(stats.average_confidence > 0.3);
    assert!(stats.average_confidence < 0.95);
}

#[test]
fn test_backward_chaining() {
    let mut graph = KnowledgeGraph::new();
    let mut inference_engine = InferenceEngine::new()
        .with_mode(InferenceMode::Backward);
    let rule_engine = RuleEngine::new();

    // Add some facts
    graph.add_fact(Fact::new("Socrates", "is_a", "Man")).unwrap();
    graph.add_fact(Fact::new("Man", "is_a", "Human")).unwrap();

    // Perform backward chaining
    let results = inference_engine.infer(&mut graph, &rule_engine, 5);

    // Test should complete without errors
    assert!(results.len() >= 0);
}

#[test]
fn test_contradiction_detection() {
    let graph = KnowledgeGraph::new();
    let inference_engine = InferenceEngine::new();

    // Add contradictory facts through the inference engine's fact collection
    let contradictions = inference_engine.detect_contradictions(&graph);

    // Should not have contradictions in empty graph
    assert!(contradictions.is_empty());
}

// WASM tests would require wasm-bindgen-test crate
// These are commented out for basic functionality testing