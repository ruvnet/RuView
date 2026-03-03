use crate::graph::KnowledgeGraph;
use crate::rules::RuleEngine;
use crate::types::Fact;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    pub new_facts: Vec<Fact>,
    pub confidence: f64,
    pub derivation_path: Vec<String>,
    pub iteration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceMode {
    Forward,
    Backward,
    Bidirectional,
}

#[derive(Debug)]
pub struct InferenceEngine {
    mode: InferenceMode,
    max_depth: u32,
    min_confidence: f64,
    inference_history: Vec<InferenceResult>,
}

impl InferenceEngine {
    pub fn new() -> Self {
        Self {
            mode: InferenceMode::Forward,
            max_depth: 10,
            min_confidence: 0.1,
            inference_history: Vec::new(),
        }
    }

    pub fn with_mode(mut self, mode: InferenceMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth;
        self
    }

    pub fn with_min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = min_confidence;
        self
    }

    pub fn infer(&mut self, graph: &mut KnowledgeGraph, rule_engine: &RuleEngine, max_iterations: u32) -> Vec<InferenceResult> {
        match self.mode {
            InferenceMode::Forward => self.forward_chain(graph, rule_engine, max_iterations),
            InferenceMode::Backward => self.backward_chain(graph, rule_engine, max_iterations),
            InferenceMode::Bidirectional => self.bidirectional_inference(graph, rule_engine, max_iterations),
        }
    }

    fn forward_chain(&mut self, graph: &mut KnowledgeGraph, rule_engine: &RuleEngine, max_iterations: u32) -> Vec<InferenceResult> {
        let mut results = Vec::new();
        let mut iteration = 0;

        while iteration < max_iterations {
            let mut new_facts_found = false;

            for rule in rule_engine.get_rules() {
                let new_facts = rule.apply(graph);

                if !new_facts.is_empty() {
                    // Filter facts by confidence threshold
                    let valid_facts: Vec<Fact> = new_facts
                        .into_iter()
                        .filter(|fact| fact.confidence >= self.min_confidence)
                        .collect();

                    if !valid_facts.is_empty() {
                        // Add facts to graph
                        for fact in &valid_facts {
                            let _ = graph.add_fact(fact.clone());
                        }

                        let result = InferenceResult {
                            new_facts: valid_facts,
                            confidence: rule.confidence,
                            derivation_path: vec![rule.name.clone()],
                            iteration,
                        };

                        results.push(result);
                        new_facts_found = true;
                    }
                }
            }

            if !new_facts_found {
                break; // No new facts, reached fixed point
            }

            iteration += 1;
        }

        self.inference_history.extend(results.clone());
        results
    }

    fn backward_chain(&mut self, graph: &mut KnowledgeGraph, rule_engine: &RuleEngine, max_iterations: u32) -> Vec<InferenceResult> {
        // Simplified backward chaining implementation
        // In a full implementation, this would work backward from goals
        let mut results = Vec::new();
        let mut goals = VecDeque::new();
        let mut proven_facts = HashSet::new();

        // For this simplified version, we'll use existing facts as initial goals
        let stats = graph.get_statistics();

        // Create some hypothetical goals based on existing predicates
        for (predicate, _) in stats.relationship_types.iter() {
            // Try to find if there are patterns we can work backward from
            let facts = graph.get_facts_by_predicate(predicate);
            for fact in facts.iter().take(5) { // Limit to avoid infinite loops
                goals.push_back(fact.clone());
            }
        }

        let mut iteration = 0;
        while iteration < max_iterations && !goals.is_empty() {
            if let Some(goal) = goals.pop_front() {
                let goal_key = format!("{}:{}:{}", goal.subject, goal.predicate, goal.object);

                if proven_facts.contains(&goal_key) {
                    continue;
                }

                // Try to prove the goal using rules
                for rule in rule_engine.get_rules() {
                    // Check if this rule can conclude our goal
                    for conclusion in &rule.conclusions {
                        // Simplified matching - in reality, this would be more sophisticated
                        if conclusion.predicate == goal.predicate {
                            let result = InferenceResult {
                                new_facts: vec![Fact::new(&goal.subject, &goal.predicate, &goal.object)],
                                confidence: rule.confidence * conclusion.confidence_modifier,
                                derivation_path: vec![format!("backward:{}", rule.name)],
                                iteration,
                            };

                            results.push(result);
                            proven_facts.insert(goal_key.clone());
                            break;
                        }
                    }
                }
            }
            iteration += 1;
        }

        self.inference_history.extend(results.clone());
        results
    }

    fn bidirectional_inference(&mut self, graph: &mut KnowledgeGraph, rule_engine: &RuleEngine, max_iterations: u32) -> Vec<InferenceResult> {
        let mut results = Vec::new();
        let half_iterations = max_iterations / 2;

        // Forward chaining for first half
        let forward_results = self.forward_chain(graph, rule_engine, half_iterations);
        results.extend(forward_results);

        // Backward chaining for second half
        let backward_results = self.backward_chain(graph, rule_engine, half_iterations);
        results.extend(backward_results);

        results
    }

    pub fn explain_inference(&self, fact: &Fact) -> Option<Vec<String>> {
        // Find the derivation path for a given fact
        for result in &self.inference_history {
            for result_fact in &result.new_facts {
                if result_fact.id == fact.id {
                    return Some(result.derivation_path.clone());
                }
            }
        }
        None
    }

    pub fn get_inference_statistics(&self) -> InferenceStatistics {
        let total_inferences = self.inference_history.len();
        let total_facts = self.inference_history
            .iter()
            .map(|r| r.new_facts.len())
            .sum();

        let average_confidence = if total_inferences > 0 {
            self.inference_history
                .iter()
                .map(|r| r.confidence)
                .sum::<f64>() / total_inferences as f64
        } else {
            0.0
        };

        let iterations_used = self.inference_history
            .iter()
            .map(|r| r.iteration)
            .max()
            .unwrap_or(0) + 1;

        InferenceStatistics {
            total_inferences,
            total_facts_derived: total_facts,
            average_confidence,
            iterations_used,
            rules_fired: self.count_unique_rules(),
        }
    }

    fn count_unique_rules(&self) -> usize {
        let mut unique_rules = HashSet::new();
        for result in &self.inference_history {
            for path_step in &result.derivation_path {
                unique_rules.insert(path_step.clone());
            }
        }
        unique_rules.len()
    }

    pub fn clear_history(&mut self) {
        self.inference_history.clear();
    }

    pub fn detect_contradictions(&self, graph: &KnowledgeGraph) -> Vec<Contradiction> {
        let mut contradictions = Vec::new();

        // Simple contradiction detection - look for facts that contradict each other
        // This is a simplified implementation
        let all_facts = self.get_all_facts_from_graph(graph);

        for (i, fact1) in all_facts.iter().enumerate() {
            for fact2 in all_facts.iter().skip(i + 1) {
                if self.are_contradictory(fact1, fact2) {
                    contradictions.push(Contradiction {
                        fact1: fact1.clone(),
                        fact2: fact2.clone(),
                        reason: "Direct contradiction".to_string(),
                    });
                }
            }
        }

        contradictions
    }

    fn get_all_facts_from_graph(&self, _graph: &KnowledgeGraph) -> Vec<Fact> {
        // This is a simplified way to get all facts
        // In a real implementation, you'd have better access to the fact store
        let mut facts = Vec::new();

        // Add facts from inference history as a proxy
        for result in &self.inference_history {
            facts.extend(result.new_facts.clone());
        }

        facts
    }

    fn are_contradictory(&self, fact1: &Fact, fact2: &Fact) -> bool {
        // Simple contradiction detection
        if fact1.subject == fact2.subject && fact1.object == fact2.object {
            // Check for opposite predicates (this would be expanded in a real system)
            let contradictory_pairs = [
                ("is", "is_not"),
                ("likes", "dislikes"),
                ("true", "false"),
                ("exists", "not_exists"),
            ];

            for (pos, neg) in contradictory_pairs.iter() {
                if (fact1.predicate == *pos && fact2.predicate == *neg) ||
                   (fact1.predicate == *neg && fact2.predicate == *pos) {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for InferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceStatistics {
    pub total_inferences: usize,
    pub total_facts_derived: usize,
    pub average_confidence: f64,
    pub iterations_used: u32,
    pub rules_fired: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub fact1: Fact,
    pub fact2: Fact,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct Goal {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub required_confidence: f64,
}

impl Goal {
    pub fn new(subject: &str, predicate: &str, object: &str) -> Self {
        Self {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            required_confidence: 0.5,
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.required_confidence = confidence;
        self
    }
}