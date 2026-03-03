use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
fn wasm_compatible_timestamp() -> u64 {
    // For WASM, use a simple counter or js Date
    use js_sys::Date;
    Date::now() as u64 / 1000
}

#[cfg(not(target_arch = "wasm32"))]
fn wasm_compatible_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub properties: HashMap<String, String>,
}

impl Entity {
    pub fn new(name: &str, entity_type: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            entity_type: entity_type.to_string(),
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub predicate: String,
    pub properties: HashMap<String, String>,
    pub confidence: f64,
}

impl Relationship {
    pub fn new(predicate: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            predicate: predicate.to_string(),
            properties: HashMap::new(),
            confidence: 1.0,
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: Option<String>,
    pub timestamp: u64,
}

impl Fact {
    pub fn new(subject: &str, predicate: &str, object: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            confidence: 1.0,
            source: None,
            timestamp: wasm_compatible_timestamp(),
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphStatistics {
    pub entity_count: usize,
    pub relationship_count: usize,
    pub fact_count: usize,
    pub entity_types: HashMap<String, usize>,
    pub relationship_types: HashMap<String, usize>,
    pub average_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeData {
    Entity(Entity),
    Concept(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeData {
    pub relationship: Relationship,
    pub fact_id: Option<String>,
}