use graph_reasoner::*;
use std::collections::HashMap;

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_creation() {
        let graph = KnowledgeGraph::new();
        let stats = graph.get_statistics();
        assert_eq!(stats.entity_count, 0);
        assert_eq!(stats.fact_count, 0);
        assert_eq!(stats.relationship_count, 0);
    }

    #[test]
    fn test_add_fact() {
        let mut graph = KnowledgeGraph::new();
        let fact = Fact::new("Alice", "loves", "Bob");
        let result = graph.add_fact(fact);
        assert!(result.is_ok());

        let stats = graph.get_statistics();
        assert_eq!(stats.fact_count, 1);
        assert_eq!(stats.entity_count, 2); // Alice and Bob
    }

    #[test]
    fn test_add_duplicate_fact() {
        let mut graph = KnowledgeGraph::new();
        let fact1 = Fact::new("Alice", "loves", "Bob");
        let fact2 = Fact::new("Alice", "loves", "Bob");

        let result1 = graph.add_fact(fact1);
        let result2 = graph.add_fact(fact2);

        assert!(result1.is_ok());
        assert!(result2.is_ok()); // Should handle duplicates gracefully

        let stats = graph.get_statistics();
        // Should not create duplicate facts
        assert_eq!(stats.fact_count, 1);
    }

    #[test]
    fn test_query_simple() {
        let mut graph = KnowledgeGraph::new();
        let fact = Fact::new("Alice", "loves", "Bob");
        graph.add_fact(fact).unwrap();

        let query = Query::new("Alice", Some("loves"), None);
        let result = graph.query(&query);

        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].subject, "Alice");
        assert_eq!(result.facts[0].predicate, "loves");
        assert_eq!(result.facts[0].object, "Bob");
    }

    #[test]
    fn test_query_with_wildcard() {
        let mut graph = KnowledgeGraph::new();
        graph.add_fact(Fact::new("Alice", "loves", "Bob")).unwrap();
        graph.add_fact(Fact::new("Alice", "likes", "Charlie")).unwrap();
        graph.add_fact(Fact::new("Bob", "loves", "Alice")).unwrap();

        let query = Query::new("Alice", None, None);
        let result = graph.query(&query);

        assert_eq!(result.facts.len(), 2);
    }

    #[test]
    fn test_complex_relationships() {
        let mut graph = KnowledgeGraph::new();

        // Create a family tree
        graph.add_fact(Fact::new("John", "parent_of", "Alice")).unwrap();
        graph.add_fact(Fact::new("John", "parent_of", "Bob")).unwrap();
        graph.add_fact(Fact::new("Mary", "parent_of", "Alice")).unwrap();
        graph.add_fact(Fact::new("Mary", "parent_of", "Bob")).unwrap();
        graph.add_fact(Fact::new("Alice", "sibling_of", "Bob")).unwrap();

        let stats = graph.get_statistics();
        assert_eq!(stats.entity_count, 4); // John, Mary, Alice, Bob
        assert_eq!(stats.fact_count, 5);
    }

    #[test]
    fn test_empty_query() {
        let graph = KnowledgeGraph::new();
        let query = Query::new("NonExistent", None, None);
        let result = graph.query(&query);

        assert_eq!(result.facts.len(), 0);
        assert!(result.execution_time_ms > 0.0);
    }

    #[test]
    fn test_graph_performance() {
        let mut graph = KnowledgeGraph::new();

        // Add many facts
        for i in 0..1000 {
            let fact = Fact::new(
                &format!("entity_{}", i),
                "relates_to",
                &format!("entity_{}", (i + 1) % 1000)
            );
            graph.add_fact(fact).unwrap();
        }

        let stats = graph.get_statistics();
        assert_eq!(stats.fact_count, 1000);

        // Query should still be fast
        let start = std::time::Instant::now();
        let query = Query::new("entity_0", None, None);
        let result = graph.query(&query);
        let duration = start.elapsed();

        assert!(!result.facts.is_empty());
        assert!(duration.as_millis() < 100); // Should be under 100ms
    }
}

#[cfg(test)]
mod inference_tests {
    use super::*;

    #[test]
    fn test_inference_engine_creation() {
        let engine = InferenceEngine::new();
        assert!(engine.get_inference_count() == 0);
    }

    #[test]
    fn test_basic_inference() {
        let mut graph = KnowledgeGraph::new();
        let mut inference_engine = InferenceEngine::new();
        let mut rule_engine = RuleEngine::new();

        // Add facts
        graph.add_fact(Fact::new("Socrates", "is_a", "human")).unwrap();

        // Add rule: if X is_a human then X is mortal
        let rule = Rule {
            id: "mortality_rule".to_string(),
            conditions: vec!["?x is_a human".to_string()],
            conclusions: vec!["?x is mortal".to_string()],
            confidence: 1.0,
        };
        rule_engine.add_rule(rule);

        // Run inference
        let results = inference_engine.infer(&mut graph, &rule_engine, 5);

        assert!(!results.is_empty());
        // Should infer that Socrates is mortal
        let socrates_mortal = graph.query(&Query::new("Socrates", Some("is"), Some("mortal")));
        assert!(!socrates_mortal.facts.is_empty());
    }

    #[test]
    fn test_transitive_inference() {
        let mut graph = KnowledgeGraph::new();
        let mut inference_engine = InferenceEngine::new();
        let mut rule_engine = RuleEngine::new();

        // Add facts
        graph.add_fact(Fact::new("Alice", "friend_of", "Bob")).unwrap();
        graph.add_fact(Fact::new("Bob", "friend_of", "Charlie")).unwrap();

        // Add transitivity rule for friendship
        let rule = Rule {
            id: "friendship_transitivity".to_string(),
            conditions: vec![
                "?x friend_of ?y".to_string(),
                "?y friend_of ?z".to_string()
            ],
            conclusions: vec!["?x friend_of ?z".to_string()],
            confidence: 0.8,
        };
        rule_engine.add_rule(rule);

        // Run inference
        let results = inference_engine.infer(&mut graph, &rule_engine, 10);

        assert!(!results.is_empty());
        // Should infer that Alice is friend of Charlie
        let alice_charlie = graph.query(&Query::new("Alice", Some("friend_of"), Some("Charlie")));
        assert!(!alice_charlie.facts.is_empty());
    }

    #[test]
    fn test_inference_convergence() {
        let mut graph = KnowledgeGraph::new();
        let mut inference_engine = InferenceEngine::new();
        let rule_engine = RuleEngine::new();

        // Simple fact that won't generate new inferences
        graph.add_fact(Fact::new("Test", "property", "value")).unwrap();

        let results = inference_engine.infer(&mut graph, &rule_engine, 100);

        // Should converge quickly with no applicable rules
        assert!(results.len() <= 1);
    }

    #[test]
    fn test_inference_with_confidence() {
        let mut graph = KnowledgeGraph::new();
        let mut inference_engine = InferenceEngine::new();
        let mut rule_engine = RuleEngine::new();

        graph.add_fact(Fact::new("Bird", "can", "fly")).unwrap();

        // Low confidence rule
        let rule = Rule {
            id: "general_rule".to_string(),
            conditions: vec!["?x can fly".to_string()],
            conclusions: vec!["?x is bird".to_string()],
            confidence: 0.3,
        };
        rule_engine.add_rule(rule);

        let results = inference_engine.infer(&mut graph, &rule_engine, 5);

        // Should still make inferences but with low confidence
        assert!(!results.is_empty());
        for result in &results {
            assert!(result.confidence <= 0.3);
        }
    }
}

#[cfg(test)]
mod rule_engine_tests {
    use super::*;

    #[test]
    fn test_rule_engine_creation() {
        let engine = RuleEngine::new();
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn test_add_rule() {
        let mut engine = RuleEngine::new();
        let rule = Rule {
            id: "test_rule".to_string(),
            conditions: vec!["?x is_a person".to_string()],
            conclusions: vec!["?x has age".to_string()],
            confidence: 1.0,
        };

        engine.add_rule(rule);
        assert_eq!(engine.rule_count(), 1);
    }

    #[test]
    fn test_rule_matching() {
        let mut engine = RuleEngine::new();
        let mut graph = KnowledgeGraph::new();

        graph.add_fact(Fact::new("John", "is_a", "person")).unwrap();

        let rule = Rule {
            id: "person_rule".to_string(),
            conditions: vec!["?x is_a person".to_string()],
            conclusions: vec!["?x has consciousness".to_string()],
            confidence: 1.0,
        };
        engine.add_rule(rule);

        let applicable_rules = engine.find_applicable_rules(&graph);
        assert_eq!(applicable_rules.len(), 1);
    }

    #[test]
    fn test_complex_rule_conditions() {
        let mut engine = RuleEngine::new();
        let mut graph = KnowledgeGraph::new();

        // Add multiple facts
        graph.add_fact(Fact::new("Alice", "is_a", "student")).unwrap();
        graph.add_fact(Fact::new("Alice", "enrolled_in", "CS101")).unwrap();
        graph.add_fact(Fact::new("CS101", "is_a", "computer_science_course")).unwrap();

        // Rule with multiple conditions
        let rule = Rule {
            id: "cs_student_rule".to_string(),
            conditions: vec![
                "?person is_a student".to_string(),
                "?person enrolled_in ?course".to_string(),
                "?course is_a computer_science_course".to_string()
            ],
            conclusions: vec!["?person is_a cs_student".to_string()],
            confidence: 0.95,
        };
        engine.add_rule(rule);

        let applicable_rules = engine.find_applicable_rules(&graph);
        assert_eq!(applicable_rules.len(), 1);
    }

    #[test]
    fn test_rule_priority() {
        let mut engine = RuleEngine::new();

        let high_priority_rule = Rule {
            id: "high_priority".to_string(),
            conditions: vec!["?x is_a VIP".to_string()],
            conclusions: vec!["?x gets priority_service".to_string()],
            confidence: 1.0,
        };

        let low_priority_rule = Rule {
            id: "low_priority".to_string(),
            conditions: vec!["?x is_a customer".to_string()],
            conclusions: vec!["?x gets standard_service".to_string()],
            confidence: 0.8,
        };

        engine.add_rule(high_priority_rule);
        engine.add_rule(low_priority_rule);

        assert_eq!(engine.rule_count(), 2);
    }

    #[test]
    fn test_rule_validation() {
        let rule = Rule {
            id: "test".to_string(),
            conditions: vec!["?x loves ?y".to_string()],
            conclusions: vec!["?y is_loved_by ?x".to_string()],
            confidence: 1.0,
        };

        assert!(rule.is_valid());

        // Test invalid rule (empty conditions)
        let invalid_rule = Rule {
            id: "invalid".to_string(),
            conditions: vec![],
            conclusions: vec!["?x is something".to_string()],
            confidence: 1.0,
        };

        assert!(!invalid_rule.is_valid());
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn test_query_creation() {
        let query = Query::new("Alice", Some("loves"), Some("Bob"));
        assert_eq!(query.subject, "Alice");
        assert_eq!(query.predicate, Some("loves".to_string()));
        assert_eq!(query.object, Some("Bob".to_string()));
    }

    #[test]
    fn test_query_with_wildcards() {
        let query = Query::new("Alice", None, None);
        assert_eq!(query.subject, "Alice");
        assert_eq!(query.predicate, None);
        assert_eq!(query.object, None);
    }

    #[test]
    fn test_query_execution_timing() {
        let mut graph = KnowledgeGraph::new();

        // Add some facts
        for i in 0..100 {
            graph.add_fact(Fact::new(
                &format!("entity_{}", i),
                "property",
                &format!("value_{}", i)
            )).unwrap();
        }

        let query = Query::new("entity_50", Some("property"), None);
        let result = graph.query(&query);

        assert!(result.execution_time_ms > 0.0);
        assert!(!result.facts.is_empty());
    }

    #[test]
    fn test_complex_query_patterns() {
        let mut graph = KnowledgeGraph::new();

        // Create a more complex graph
        graph.add_fact(Fact::new("Alice", "works_at", "TechCorp")).unwrap();
        graph.add_fact(Fact::new("Bob", "works_at", "TechCorp")).unwrap();
        graph.add_fact(Fact::new("Charlie", "works_at", "StartupInc")).unwrap();
        graph.add_fact(Fact::new("TechCorp", "industry", "Technology")).unwrap();
        graph.add_fact(Fact::new("StartupInc", "industry", "Technology")).unwrap();

        // Query for all people who work at TechCorp
        let query = Query::new("*", Some("works_at"), Some("TechCorp"));
        let result = graph.query(&query);

        assert_eq!(result.facts.len(), 2); // Alice and Bob
    }

    #[test]
    fn test_query_result_format() {
        let mut graph = KnowledgeGraph::new();
        graph.add_fact(Fact::new("Test", "has", "property")).unwrap();

        let query = Query::new("Test", None, None);
        let result = graph.query(&query);

        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.query_id.len(), 36); // UUID length
        assert!(result.execution_time_ms >= 0.0);
    }
}

#[cfg(test)]
mod types_tests {
    use super::*;

    #[test]
    fn test_entity_creation() {
        let entity = Entity::new("TestEntity", "person");
        assert_eq!(entity.name, "TestEntity");
        assert_eq!(entity.entity_type, "person");
        assert!(!entity.attributes.is_empty()); // Should have timestamp
    }

    #[test]
    fn test_fact_creation() {
        let fact = Fact::new("Alice", "loves", "Bob");
        assert_eq!(fact.subject, "Alice");
        assert_eq!(fact.predicate, "loves");
        assert_eq!(fact.object, "Bob");
        assert!(fact.confidence >= 0.0 && fact.confidence <= 1.0);
    }

    #[test]
    fn test_fact_equality() {
        let fact1 = Fact::new("Alice", "loves", "Bob");
        let fact2 = Fact::new("Alice", "loves", "Bob");
        let fact3 = Fact::new("Bob", "loves", "Alice");

        assert_eq!(fact1, fact2);
        assert_ne!(fact1, fact3);
    }

    #[test]
    fn test_relationship_creation() {
        let relationship = Relationship::new("friendship", "Alice", "Bob", 0.8);
        assert_eq!(relationship.relationship_type, "friendship");
        assert_eq!(relationship.source, "Alice");
        assert_eq!(relationship.target, "Bob");
        assert_eq!(relationship.strength, 0.8);
    }

    #[test]
    fn test_relationship_bidirectionality() {
        let rel1 = Relationship::new("friendship", "Alice", "Bob", 0.8);
        let rel2 = rel1.reverse();

        assert_eq!(rel2.source, "Bob");
        assert_eq!(rel2.target, "Alice");
        assert_eq!(rel2.strength, 0.8);
    }

    #[test]
    fn test_fact_serialization() {
        let fact = Fact::new("Test", "predicate", "object");
        let serialized = serde_json::to_string(&fact).unwrap();
        let deserialized: Fact = serde_json::from_str(&serialized).unwrap();

        assert_eq!(fact, deserialized);
    }

    #[test]
    fn test_confidence_bounds() {
        let mut fact = Fact::new("Test", "pred", "obj");
        fact.confidence = 2.0; // Invalid confidence

        // Should be clamped to valid range
        fact.normalize_confidence();
        assert!(fact.confidence >= 0.0 && fact.confidence <= 1.0);
    }

    #[test]
    fn test_entity_attributes() {
        let mut entity = Entity::new("TestEntity", "concept");
        entity.add_attribute("color", "blue");
        entity.add_attribute("size", "large");

        assert_eq!(entity.get_attribute("color"), Some(&"blue".to_string()));
        assert_eq!(entity.get_attribute("size"), Some(&"large".to_string()));
        assert_eq!(entity.get_attribute("weight"), None);
    }
}