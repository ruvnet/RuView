use crate::query::{Query, QueryResult};
use crate::types::{EdgeData, Entity, Fact, GraphStatistics, NodeData, Relationship};
use indexmap::IndexMap;
use petgraph::{Graph, Undirected};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
// use uuid::Uuid; // Not directly used in this module

pub type NodeIndex = petgraph::graph::NodeIndex;
pub type EdgeIndex = petgraph::graph::EdgeIndex;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Edge not found: {0}")]
    EdgeNotFound(String),
    #[error("Invalid fact: {0}")]
    InvalidFact(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug)]
pub struct KnowledgeGraph {
    graph: Graph<NodeData, EdgeData, Undirected>,
    entities: IndexMap<String, NodeIndex>,
    facts: IndexMap<String, EdgeIndex>,
    entity_names: IndexMap<String, NodeIndex>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new_undirected(),
            entities: IndexMap::new(),
            facts: IndexMap::new(),
            entity_names: IndexMap::new(),
        }
    }

    pub fn graph(&self) -> &Graph<NodeData, EdgeData, Undirected> {
        &self.graph
    }

    pub fn add_entity(&mut self, entity: Entity) -> Result<NodeIndex, GraphError> {
        if self.entity_names.contains_key(&entity.name) {
            return Ok(self.entity_names[&entity.name]);
        }

        let node_index = self.graph.add_node(NodeData::Entity(entity.clone()));
        self.entities.insert(entity.id.clone(), node_index);
        self.entity_names.insert(entity.name.clone(), node_index);
        Ok(node_index)
    }

    pub fn add_concept(&mut self, concept: &str) -> NodeIndex {
        if let Some(&index) = self.entity_names.get(concept) {
            return index;
        }

        let node_index = self.graph.add_node(NodeData::Concept(concept.to_string()));
        self.entity_names.insert(concept.to_string(), node_index);
        node_index
    }

    pub fn add_fact(&mut self, fact: Fact) -> Result<String, GraphError> {
        // Get or create subject node
        let subject_index = if let Some(&index) = self.entity_names.get(&fact.subject) {
            index
        } else {
            self.add_concept(&fact.subject)
        };

        // Get or create object node
        let object_index = if let Some(&index) = self.entity_names.get(&fact.object) {
            index
        } else {
            self.add_concept(&fact.object)
        };

        // Create relationship
        let relationship = Relationship::new(&fact.predicate)
            .with_confidence(fact.confidence);

        // Add edge
        let edge_data = EdgeData {
            relationship,
            fact_id: Some(fact.id.clone()),
        };

        let edge_index = self.graph.add_edge(subject_index, object_index, edge_data);
        self.facts.insert(fact.id.clone(), edge_index);

        Ok(fact.id)
    }

    pub fn get_entity(&self, name: &str) -> Option<&Entity> {
        if let Some(&node_index) = self.entity_names.get(name) {
            if let Some(NodeData::Entity(entity)) = self.graph.node_weight(node_index) {
                return Some(entity);
            }
        }
        None
    }

    pub fn get_facts_by_subject(&self, subject: &str) -> Vec<FactTriple> {
        let mut facts = Vec::new();

        if let Some(&subject_index) = self.entity_names.get(subject) {
            let edges = self.graph.edges(subject_index);
            for edge_ref in edges {
                let target_index = edge_ref.target();
                if let Some(target_name) = self.get_node_name(target_index) {
                    facts.push(FactTriple {
                        subject: subject.to_string(),
                        predicate: edge_ref.weight().relationship.predicate.clone(),
                        object: target_name,
                        confidence: edge_ref.weight().relationship.confidence,
                    });
                }
            }
        }

        facts
    }

    pub fn get_facts_by_predicate(&self, predicate: &str) -> Vec<FactTriple> {
        let mut facts = Vec::new();

        for edge_index in self.graph.edge_indices() {
            if let Some(edge_data) = self.graph.edge_weight(edge_index) {
                if edge_data.relationship.predicate == predicate {
                    if let Some((source, target)) = self.graph.edge_endpoints(edge_index) {
                        if let (Some(subject_name), Some(object_name)) =
                            (self.get_node_name(source), self.get_node_name(target)) {
                            facts.push(FactTriple {
                                subject: subject_name,
                                predicate: predicate.to_string(),
                                object: object_name,
                                confidence: edge_data.relationship.confidence,
                            });
                        }
                    }
                }
            }
        }

        facts
    }

    pub fn query(&self, query: &Query) -> QueryResult {
        match &query.query_type {
            QueryType::FindFacts { subject, predicate, object } => {
                self.find_facts(subject.as_deref(), predicate.as_deref(), object.as_deref())
            }
            QueryType::FindPath { from, to, max_depth } => {
                self.find_path(from, to, *max_depth)
            }
            QueryType::FindConnected { entity, relationship_type, max_depth } => {
                self.find_connected(entity, relationship_type.as_deref(), *max_depth)
            }
        }
    }

    fn find_facts(&self, subject: Option<&str>, predicate: Option<&str>, object: Option<&str>) -> QueryResult {
        let mut results = Vec::new();

        for edge_index in self.graph.edge_indices() {
            if let Some(edge_data) = self.graph.edge_weight(edge_index) {
                if let Some(predicate_filter) = predicate {
                    if edge_data.relationship.predicate != predicate_filter {
                        continue;
                    }
                }

                if let Some((source, target)) = self.graph.edge_endpoints(edge_index) {
                    let source_name = self.get_node_name(source);
                    let target_name = self.get_node_name(target);

                    if let (Some(s_name), Some(t_name)) = (source_name, target_name) {
                        // Check subject filter
                        if let Some(subject_filter) = subject {
                            if s_name != subject_filter && t_name != subject_filter {
                                continue;
                            }
                        }

                        // Check object filter
                        if let Some(object_filter) = object {
                            if s_name != object_filter && t_name != object_filter {
                                continue;
                            }
                        }

                        results.push(FactTriple {
                            subject: s_name.clone(),
                            predicate: edge_data.relationship.predicate.clone(),
                            object: t_name.clone(),
                            confidence: edge_data.relationship.confidence,
                        });
                    }
                }
            }
        }

        QueryResult {
            facts: results,
            paths: Vec::new(),
            entities: Vec::new(),
        }
    }

    fn find_path(&self, from: &str, to: &str, _max_depth: u32) -> QueryResult {
        use petgraph::algo::dijkstra;

        if let (Some(&from_index), Some(&to_index)) =
            (self.entity_names.get(from), self.entity_names.get(to)) {

            let node_map = dijkstra(&self.graph, from_index, Some(to_index), |_| 1);

            if node_map.contains_key(&to_index) {
                // Reconstruct path (simplified for this implementation)
                let path = vec![
                    PathStep {
                        entity: from.to_string(),
                        relationship: None,
                    },
                    PathStep {
                        entity: to.to_string(),
                        relationship: None,
                    },
                ];

                return QueryResult {
                    facts: Vec::new(),
                    paths: vec![path],
                    entities: Vec::new(),
                };
            }
        }

        QueryResult {
            facts: Vec::new(),
            paths: Vec::new(),
            entities: Vec::new(),
        }
    }

    fn find_connected(&self, entity: &str, relationship_type: Option<&str>, max_depth: u32) -> QueryResult {
        let mut connected_entities = Vec::new();

        if let Some(&entity_index) = self.entity_names.get(entity) {
            let mut visited = std::collections::HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back((entity_index, 0));

            while let Some((current_index, depth)) = queue.pop_front() {
                if depth >= max_depth || visited.contains(&current_index) {
                    continue;
                }

                visited.insert(current_index);

                if let Some(name) = self.get_node_name(current_index) {
                    if name != entity {
                        connected_entities.push(name);
                    }
                }

                let edges = self.graph.edges(current_index);
                for edge_ref in edges {
                    let neighbor_index = edge_ref.target();
                    if !visited.contains(&neighbor_index) {
                        queue.push_back((neighbor_index, depth + 1));
                    }
                }
            }
        }

        QueryResult {
            facts: Vec::new(),
            paths: Vec::new(),
            entities: connected_entities,
        }
    }

    pub fn get_node_name(&self, node_index: NodeIndex) -> Option<String> {
        match self.graph.node_weight(node_index)? {
            NodeData::Entity(entity) => Some(entity.name.clone()),
            NodeData::Concept(concept) => Some(concept.clone()),
        }
    }

    pub fn get_statistics(&self) -> GraphStatistics {
        let mut entity_types = HashMap::new();
        let mut relationship_types = HashMap::new();
        let mut total_confidence = 0.0;
        let mut confidence_count = 0;

        // Count entities by type
        for node_weight in self.graph.node_weights() {
            if let NodeData::Entity(entity) = node_weight {
                *entity_types.entry(entity.entity_type.clone()).or_insert(0) += 1;
            }
        }

        // Count relationships by type and calculate average confidence
        for edge_weight in self.graph.edge_weights() {
            let predicate = &edge_weight.relationship.predicate;
            *relationship_types.entry(predicate.clone()).or_insert(0) += 1;
            total_confidence += edge_weight.relationship.confidence;
            confidence_count += 1;
        }

        let average_confidence = if confidence_count > 0 {
            total_confidence / confidence_count as f64
        } else {
            0.0
        };

        GraphStatistics {
            entity_count: self.entities.len(),
            relationship_count: self.graph.edge_count(),
            fact_count: self.facts.len(),
            entity_types,
            relationship_types,
            average_confidence,
        }
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStep {
    pub entity: String,
    pub relationship: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QueryType {
    FindFacts {
        subject: Option<String>,
        predicate: Option<String>,
        object: Option<String>,
    },
    FindPath {
        from: String,
        to: String,
        max_depth: u32,
    },
    FindConnected {
        entity: String,
        relationship_type: Option<String>,
        max_depth: u32,
    },
}