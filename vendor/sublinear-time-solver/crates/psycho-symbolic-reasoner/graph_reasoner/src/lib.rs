use wasm_bindgen::prelude::*;

pub mod graph;
pub mod inference;
pub mod rules;
pub mod query;
pub mod types;
pub mod time_utils;

pub use graph::KnowledgeGraph;
pub use inference::{InferenceEngine, InferenceResult};
pub use rules::{Rule, RuleEngine};
pub use query::{Query, QueryResult};
pub use types::{Entity, Fact, Relationship};

// WASM bindings
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct GraphReasoner {
    graph: KnowledgeGraph,
    inference_engine: InferenceEngine,
    rule_engine: RuleEngine,
}

#[wasm_bindgen]
impl GraphReasoner {
    #[wasm_bindgen(constructor)]
    pub fn new() -> GraphReasoner {
        GraphReasoner {
            graph: KnowledgeGraph::new(),
            inference_engine: InferenceEngine::new(),
            rule_engine: RuleEngine::new(),
        }
    }

    #[wasm_bindgen]
    pub fn add_fact(&mut self, subject: &str, predicate: &str, object: &str) -> String {
        let fact = Fact::new(subject, predicate, object);
        match self.graph.add_fact(fact) {
            Ok(id) => id.to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[wasm_bindgen]
    pub fn add_rule(&mut self, rule_json: &str) -> bool {
        match serde_json::from_str::<Rule>(rule_json) {
            Ok(rule) => {
                self.rule_engine.add_rule(rule);
                true
            }
            Err(_) => false,
        }
    }

    #[wasm_bindgen]
    pub fn query(&self, query_json: &str) -> String {
        match serde_json::from_str::<Query>(query_json) {
            Ok(query) => {
                let result = self.graph.query(&query);
                serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e),
        }
    }

    #[wasm_bindgen]
    pub fn infer(&mut self, max_iterations: Option<u32>) -> String {
        let iterations = max_iterations.unwrap_or(10);
        let results = self.inference_engine.infer(&mut self.graph, &self.rule_engine, iterations);
        serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn get_graph_stats(&self) -> String {
        let stats = self.graph.get_statistics();
        serde_json::to_string(&stats).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for GraphReasoner {
    fn default() -> Self {
        Self::new()
    }
}