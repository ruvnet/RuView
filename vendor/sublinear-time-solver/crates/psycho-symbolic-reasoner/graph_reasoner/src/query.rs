use crate::graph::{FactTriple, PathStep, QueryType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub id: String,
    pub query_type: QueryType,
    pub limit: Option<usize>,
    pub min_confidence: Option<f64>,
}

impl Query {
    pub fn find_facts(subject: Option<&str>, predicate: Option<&str>, object: Option<&str>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query_type: QueryType::FindFacts {
                subject: subject.map(|s| s.to_string()),
                predicate: predicate.map(|p| p.to_string()),
                object: object.map(|o| o.to_string()),
            },
            limit: None,
            min_confidence: None,
        }
    }

    pub fn find_path(from: &str, to: &str, max_depth: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query_type: QueryType::FindPath {
                from: from.to_string(),
                to: to.to_string(),
                max_depth,
            },
            limit: None,
            min_confidence: None,
        }
    }

    pub fn find_connected(entity: &str, relationship_type: Option<&str>, max_depth: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query_type: QueryType::FindConnected {
                entity: entity.to_string(),
                relationship_type: relationship_type.map(|r| r.to_string()),
                max_depth,
            },
            limit: None,
            min_confidence: None,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = Some(min_confidence);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub facts: Vec<FactTriple>,
    pub paths: Vec<Vec<PathStep>>,
    pub entities: Vec<String>,
}

impl QueryResult {
    pub fn new() -> Self {
        Self {
            facts: Vec::new(),
            paths: Vec::new(),
            entities: Vec::new(),
        }
    }

    pub fn filter_by_confidence(&mut self, min_confidence: f64) {
        self.facts.retain(|fact| fact.confidence >= min_confidence);
    }

    pub fn limit_results(&mut self, limit: usize) {
        self.facts.truncate(limit);
        self.paths.truncate(limit);
        self.entities.truncate(limit);
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty() && self.paths.is_empty() && self.entities.is_empty()
    }

    pub fn total_results(&self) -> usize {
        self.facts.len() + self.paths.len() + self.entities.len()
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEngine {
    query_history: Vec<Query>,
    result_cache: std::collections::HashMap<String, QueryResult>,
}

impl QueryEngine {
    pub fn new() -> Self {
        Self {
            query_history: Vec::new(),
            result_cache: std::collections::HashMap::new(),
        }
    }

    pub fn execute_query(&mut self, query: Query) -> QueryResult {
        // Check cache first
        if let Some(cached_result) = self.result_cache.get(&query.id) {
            return cached_result.clone();
        }

        // For this implementation, we'll return an empty result
        // In a real system, this would execute the query against the knowledge graph
        let result = QueryResult::new();

        self.query_history.push(query.clone());
        self.result_cache.insert(query.id.clone(), result.clone());

        result
    }

    pub fn get_query_history(&self) -> &[Query] {
        &self.query_history
    }

    pub fn clear_cache(&mut self) {
        self.result_cache.clear();
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}