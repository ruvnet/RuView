use crate::graph::KnowledgeGraph;
use crate::types::Fact;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<Condition>,
    pub conclusions: Vec<Conclusion>,
    pub confidence: f64,
    pub priority: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub subject: Variable,
    pub predicate: String,
    pub object: Variable,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conclusion {
    pub subject: Variable,
    pub predicate: String,
    pub object: Variable,
    pub confidence_modifier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Variable {
    Literal { value: String },
    Variable { name: String },
    Wildcard,
}

impl Variable {
    pub fn literal(value: &str) -> Self {
        Self::Literal { value: value.to_string() }
    }

    pub fn variable(name: &str) -> Self {
        Self::Variable { name: name.to_string() }
    }

    pub fn wildcard() -> Self {
        Self::Wildcard
    }
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub variables: HashMap<String, String>,
}

impl Binding {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn bind(&mut self, variable: &str, value: &str) -> bool {
        if let Some(existing) = self.variables.get(variable) {
            existing == value
        } else {
            self.variables.insert(variable.to_string(), value.to_string());
            true
        }
    }

    pub fn get(&self, variable: &str) -> Option<&String> {
        self.variables.get(variable)
    }

    pub fn resolve_variable(&self, var: &Variable) -> Option<String> {
        match var {
            Variable::Literal { value } => Some(value.clone()),
            Variable::Variable { name } => self.variables.get(name).cloned(),
            Variable::Wildcard => None,
        }
    }
}

impl Default for Binding {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            conditions: Vec::new(),
            conclusions: Vec::new(),
            confidence: 1.0,
            priority: 0,
        }
    }

    pub fn add_condition(mut self, subject: Variable, predicate: &str, object: Variable) -> Self {
        self.conditions.push(Condition {
            subject,
            predicate: predicate.to_string(),
            object,
            required: true,
        });
        self
    }

    pub fn add_conclusion(mut self, subject: Variable, predicate: &str, object: Variable) -> Self {
        self.conclusions.push(Conclusion {
            subject,
            predicate: predicate.to_string(),
            object,
            confidence_modifier: 1.0,
        });
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn apply(&self, graph: &KnowledgeGraph) -> Vec<Fact> {
        let mut new_facts = Vec::new();
        let bindings = self.find_bindings(graph);

        for binding in bindings {
            for conclusion in &self.conclusions {
                if let (Some(subject), Some(object)) = (
                    binding.resolve_variable(&conclusion.subject),
                    binding.resolve_variable(&conclusion.object),
                ) {
                    let confidence = self.confidence * conclusion.confidence_modifier;
                    let fact = Fact::new(&subject, &conclusion.predicate, &object)
                        .with_confidence(confidence)
                        .with_source(&format!("rule:{}", self.name));
                    new_facts.push(fact);
                }
            }
        }

        new_facts
    }

    fn find_bindings(&self, graph: &KnowledgeGraph) -> Vec<Binding> {
        let mut bindings = vec![Binding::new()];

        for condition in &self.conditions {
            let mut new_bindings = Vec::new();

            for binding in bindings {
                let condition_bindings = self.match_condition(condition, &binding, graph);
                new_bindings.extend(condition_bindings);
            }

            bindings = new_bindings;
            if bindings.is_empty() {
                break;
            }
        }

        bindings
    }

    fn match_condition(&self, condition: &Condition, binding: &Binding, graph: &KnowledgeGraph) -> Vec<Binding> {
        let mut result_bindings = Vec::new();

        // Get all facts from the graph and try to match them against the condition
        // This is a simplified implementation - a real system would use indexing
        for node_index in graph.graph().node_indices() {
            let edges = graph.graph().edges(node_index);
            for edge_ref in edges {
                let target_index = edge_ref.target();
                let edge_data = edge_ref.weight();
                if edge_data.relationship.predicate == condition.predicate {
                    if let (Some(subject_name), Some(object_name)) = (
                        graph.get_node_name(node_index),
                        graph.get_node_name(target_index),
                    ) {
                            let mut new_binding = binding.clone();

                            // Try to match subject
                            let subject_matches = match &condition.subject {
                                Variable::Literal { value } => value == &subject_name,
                                Variable::Variable { name } => new_binding.bind(name, &subject_name),
                                Variable::Wildcard => true,
                            };

                            // Try to match object
                            let object_matches = if subject_matches {
                                match &condition.object {
                                    Variable::Literal { value } => value == &object_name,
                                    Variable::Variable { name } => new_binding.bind(name, &object_name),
                                    Variable::Wildcard => true,
                                }
                            } else {
                                false
                            };

                            if subject_matches && object_matches {
                                result_bindings.push(new_binding);
                            }
                        }
                }
            }
        }

        result_bindings
    }
}

#[derive(Debug)]
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn apply_rules(&self, graph: &mut KnowledgeGraph, max_iterations: u32) -> Vec<Fact> {
        let mut all_new_facts = Vec::new();
        let mut iteration = 0;

        loop {
            if iteration >= max_iterations {
                break;
            }

            let mut new_facts_this_iteration = Vec::new();

            for rule in &self.rules {
                let facts = rule.apply(graph);
                new_facts_this_iteration.extend(facts);
            }

            if new_facts_this_iteration.is_empty() {
                break; // No new facts generated, we've reached a fixed point
            }

            // Add new facts to the graph
            for fact in &new_facts_this_iteration {
                let _ = graph.add_fact(fact.clone());
            }

            all_new_facts.extend(new_facts_this_iteration);
            iteration += 1;
        }

        all_new_facts
    }

    pub fn get_rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        if let Some(pos) = self.rules.iter().position(|r| r.id == rule_id) {
            self.rules.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    pub fn create_transitivity_rule() -> Rule {
        Rule::new("transitivity", "If A relates to B and B relates to C, then A relates to C")
            .add_condition(
                Variable::variable("A"),
                "relates_to",
                Variable::variable("B"),
            )
            .add_condition(
                Variable::variable("B"),
                "relates_to",
                Variable::variable("C"),
            )
            .add_conclusion(
                Variable::variable("A"),
                "relates_to",
                Variable::variable("C"),
            )
            .with_confidence(0.8)
            .with_priority(10)
    }

    pub fn create_subset_inheritance_rule() -> Rule {
        Rule::new("subset_inheritance", "If X is a subset of Y and Y has property Z, then X has property Z")
            .add_condition(
                Variable::variable("X"),
                "subset_of",
                Variable::variable("Y"),
            )
            .add_condition(
                Variable::variable("Y"),
                "has_property",
                Variable::variable("Z"),
            )
            .add_conclusion(
                Variable::variable("X"),
                "has_property",
                Variable::variable("Z"),
            )
            .with_confidence(0.9)
            .with_priority(15)
    }

    pub fn create_symmetry_rule(predicate: &str) -> Rule {
        Rule::new(
            &format!("{}_symmetry", predicate),
            &format!("If A {} B, then B {} A", predicate, predicate),
        )
        .add_condition(
            Variable::variable("A"),
            predicate,
            Variable::variable("B"),
        )
        .add_conclusion(
            Variable::variable("B"),
            predicate,
            Variable::variable("A"),
        )
        .with_confidence(1.0)
        .with_priority(20)
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}