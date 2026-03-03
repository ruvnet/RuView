//! Knowledge graph and theorem store for dynamic knowledge representation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use async_trait::async_trait;

use super::reasoning::Theorem;

/// Knowledge graph for storing entities, relations, and theorems
pub struct KnowledgeGraph {
    /// Entities in the knowledge graph
    entities: HashMap<String, Entity>,

    /// Relations between entities
    relations: Vec<Relation>,

    /// Theorems and verified knowledge
    theorems: Vec<Theorem>,

    /// Temporal knowledge (time-windowed facts)
    temporal_facts: Vec<TemporalFact>,

    /// Entity embeddings for semantic similarity
    embeddings: HashMap<String, Vec<f64>>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            relations: Vec::new(),
            theorems: Vec::new(),
            temporal_facts: Vec::new(),
            embeddings: HashMap::new(),
        }
    }

    /// Extract entities from text
    pub async fn extract_entities(&self, text: &str) -> Result<Vec<Entity>, String> {
        let mut entities = Vec::new();

        // Simple entity extraction (can be enhanced with NER)
        let words: Vec<&str> = text.split_whitespace().collect();

        for (i, word) in words.iter().enumerate() {
            // Capitalize words might be entities
            if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                entities.push(Entity {
                    id: format!("entity_{}", i),
                    name: word.to_string(),
                    entity_type: EntityType::Unknown,
                    attributes: HashMap::new(),
                    confidence: 0.7,
                });
            }
        }

        // Extract numeric values
        for (i, word) in words.iter().enumerate() {
            if word.parse::<f64>().is_ok() {
                entities.push(Entity {
                    id: format!("value_{}", i),
                    name: word.to_string(),
                    entity_type: EntityType::Value,
                    attributes: HashMap::new(),
                    confidence: 0.9,
                });
            }
        }

        Ok(entities)
    }

    /// Update knowledge graph with new entities
    pub async fn update(&mut self, entities: Vec<Entity>) -> Result<(), String> {
        for entity in entities {
            // Check if entity exists
            if let Some(existing) = self.entities.get_mut(&entity.id) {
                // Update existing entity
                existing.confidence = (existing.confidence + entity.confidence) / 2.0;
                for (key, value) in entity.attributes {
                    existing.attributes.insert(key, value);
                }
            } else {
                // Add new entity
                self.entities.insert(entity.id.clone(), entity);
            }
        }

        Ok(())
    }

    /// Add a relation between entities
    pub fn add_relation(&mut self, relation: Relation) {
        self.relations.push(relation);
    }

    /// Add a verified theorem
    pub fn add_theorem(&mut self, theorem: Theorem) {
        self.theorems.push(theorem);
    }

    /// Query entities by type
    pub fn query_entities(&self, entity_type: EntityType) -> Vec<&Entity> {
        self.entities.values()
            .filter(|e| e.entity_type == entity_type)
            .collect()
    }

    /// Find related entities
    pub fn find_related(&self, entity_id: &str, max_depth: usize) -> Vec<String> {
        let mut related = HashSet::new();
        let mut to_explore = vec![(entity_id.to_string(), 0)];

        while let Some((current_id, depth)) = to_explore.pop() {
            if depth >= max_depth {
                continue;
            }

            // Find relations involving this entity
            for relation in &self.relations {
                if relation.subject == current_id {
                    related.insert(relation.object.clone());
                    to_explore.push((relation.object.clone(), depth + 1));
                } else if relation.object == current_id {
                    related.insert(relation.subject.clone());
                    to_explore.push((relation.subject.clone(), depth + 1));
                }
            }
        }

        related.into_iter().collect()
    }

    /// Add temporal fact (fact with time window)
    pub fn add_temporal_fact(&mut self, fact: TemporalFact) {
        self.temporal_facts.push(fact);

        // Clean up old facts
        let now = chrono::Utc::now().timestamp();
        self.temporal_facts.retain(|f| {
            if let Some(end) = f.valid_until {
                end > now
            } else {
                true // Keep facts without expiration
            }
        });
    }

    /// Get facts valid at a specific time
    pub fn get_facts_at_time(&self, timestamp: i64) -> Vec<&TemporalFact> {
        self.temporal_facts.iter()
            .filter(|f| {
                f.valid_from <= timestamp &&
                f.valid_until.map(|t| timestamp <= t).unwrap_or(true)
            })
            .collect()
    }

    /// Compute semantic similarity between entities
    pub fn compute_similarity(&self, entity1: &str, entity2: &str) -> f64 {
        if let (Some(emb1), Some(emb2)) = (
            self.embeddings.get(entity1),
            self.embeddings.get(entity2)
        ) {
            // Cosine similarity
            let dot_product: f64 = emb1.iter()
                .zip(emb2.iter())
                .map(|(a, b)| a * b)
                .sum();

            let norm1: f64 = emb1.iter().map(|x| x * x).sum::<f64>().sqrt();
            let norm2: f64 = emb2.iter().map(|x| x * x).sum::<f64>().sqrt();

            if norm1 > 0.0 && norm2 > 0.0 {
                dot_product / (norm1 * norm2)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Update entity embedding
    pub fn update_embedding(&mut self, entity_id: String, embedding: Vec<f64>) {
        self.embeddings.insert(entity_id, embedding);
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn theorem_count(&self) -> usize {
        self.theorems.len()
    }

    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }
}

/// An entity in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: EntityType,
    pub attributes: HashMap<String, String>,
    pub confidence: f64,
}

/// Types of entities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Place,
    Organization,
    Concept,
    Event,
    Value,
    Unknown,
}

/// A relation between two entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,
}

/// A temporal fact (fact valid within a time window)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalFact {
    pub fact: String,
    pub valid_from: i64,
    pub valid_until: Option<i64>,
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_knowledge_graph() {
        let mut kg = KnowledgeGraph::new();

        let entities = kg.extract_entities("Alice works at Google").await.unwrap();
        kg.update(entities).await.unwrap();

        assert!(kg.entity_count() > 0);
    }

    #[test]
    fn test_temporal_facts() {
        let mut kg = KnowledgeGraph::new();
        let now = chrono::Utc::now().timestamp();

        kg.add_temporal_fact(TemporalFact {
            fact: "Weather is sunny".to_string(),
            valid_from: now,
            valid_until: Some(now + 3600),
            confidence: 0.9,
        });

        let facts = kg.get_facts_at_time(now + 1800);
        assert_eq!(facts.len(), 1);
    }
}
