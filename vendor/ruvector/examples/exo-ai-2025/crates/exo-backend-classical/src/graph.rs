//! Graph database wrapper for ruvector-graph

use exo_core::{
    EntityId, HyperedgeId, HyperedgeResult, Relation, SheafConsistencyResult, TopologicalQuery,
};
use ruvector_graph::{GraphDB, Hyperedge, Node};
use std::str::FromStr;

use exo_core::{Error as ExoError, Result as ExoResult};

#[cfg(test)]
use exo_core::RelationType;

/// Wrapper around ruvector GraphDB
pub struct GraphWrapper {
    /// Underlying graph database
    db: GraphDB,
}

impl GraphWrapper {
    /// Create a new graph wrapper
    pub fn new() -> Self {
        Self { db: GraphDB::new() }
    }

    /// Create a hyperedge spanning multiple entities
    pub fn create_hyperedge(
        &mut self,
        entities: &[EntityId],
        relation: &Relation,
    ) -> ExoResult<HyperedgeId> {
        // Ensure all entities exist as nodes (create if they don't)
        for entity_id in entities {
            let entity_id_str = entity_id.0.to_string();
            if self.db.get_node(&entity_id_str).is_none() {
                // Create node if it doesn't exist
                use ruvector_graph::types::{Label, Properties};
                let node = Node::new(entity_id_str, vec![Label::new("Entity")], Properties::new());
                self.db
                    .create_node(node)
                    .map_err(|e| ExoError::Backend(format!("Failed to create node: {}", e)))?;
            }
        }

        // Create hyperedge using ruvector-graph
        let entity_strs: Vec<String> = entities.iter().map(|e| e.0.to_string()).collect();

        let mut hyperedge = Hyperedge::new(entity_strs, relation.relation_type.0.clone());

        // Add properties if they're an object
        if let Some(obj) = relation.properties.as_object() {
            for (key, value) in obj {
                if let Ok(prop_val) = serde_json::from_value(value.clone()) {
                    hyperedge.properties.insert(key.clone(), prop_val);
                }
            }
        }

        let hyperedge_id_str = hyperedge.id.clone();

        self.db
            .create_hyperedge(hyperedge)
            .map_err(|e| ExoError::Backend(format!("Failed to create hyperedge: {}", e)))?;

        // Convert string ID to HyperedgeId
        let uuid = uuid::Uuid::from_str(&hyperedge_id_str).unwrap_or_else(|_| uuid::Uuid::new_v4());
        Ok(HyperedgeId(uuid))
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &EntityId) -> Option<Node> {
        self.db.get_node(&id.0.to_string())
    }

    /// Get a hyperedge by ID
    pub fn get_hyperedge(&self, id: &HyperedgeId) -> Option<Hyperedge> {
        self.db.get_hyperedge(&id.0.to_string())
    }

    /// Query the graph with topological queries
    pub fn query(&self, query: &TopologicalQuery) -> ExoResult<HyperedgeResult> {
        match query {
            TopologicalQuery::PersistentHomology {
                dimension: _,
                epsilon_range: _,
            } => {
                // Persistent homology is not directly supported on classical backend
                // This would require building a filtration and computing persistence
                // For now, return not supported
                Ok(HyperedgeResult::NotSupported)
            }
            TopologicalQuery::BettiNumbers { max_dimension } => {
                // Betti numbers computation
                // For classical backend, we can approximate:
                // - Betti_0 = number of connected components
                // - Higher Betti numbers require simplicial complex construction

                // Simple approximation: count connected components for Betti_0
                let betti_0 = self.approximate_connected_components();

                // For higher dimensions, we'd need proper TDA implementation
                // Return placeholder values for now
                let mut betti = vec![betti_0];
                for _ in 1..=*max_dimension {
                    betti.push(0); // Placeholder
                }

                Ok(HyperedgeResult::BettiNumbers(betti))
            }
            TopologicalQuery::SheafConsistency { local_sections: _ } => {
                // Sheaf consistency is an advanced topological concept
                // Not supported on classical discrete backend
                Ok(HyperedgeResult::SheafConsistency(
                    SheafConsistencyResult::Inconsistent(vec![
                        "Sheaf consistency not supported on classical backend".to_string(),
                    ]),
                ))
            }
        }
    }

    /// Approximate the number of connected components
    fn approximate_connected_components(&self) -> usize {
        // This is a simple approximation
        // In a full implementation, we'd use proper graph traversal
        // For now, return 1 as a placeholder
        1
    }

    /// Get hyperedges containing a specific node
    pub fn hyperedges_containing(&self, node_id: &EntityId) -> Vec<Hyperedge> {
        // Use the hyperedge index from GraphDB
        self.db.get_hyperedges_by_node(&node_id.0.to_string())
    }
}

impl Default for GraphWrapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_graph_creation() {
        let graph = GraphWrapper::new();
        // Basic test
        assert!(graph.db.get_node("nonexistent").is_none());
    }

    #[test]
    fn test_create_hyperedge() {
        let mut graph = GraphWrapper::new();

        let entities = vec![EntityId::new(), EntityId::new(), EntityId::new()];
        let relation = Relation {
            relation_type: RelationType::new("related_to"),
            properties: serde_json::json!({}),
        };

        let result = graph.create_hyperedge(&entities, &relation);
        assert!(result.is_ok());
    }

    #[test]
    fn test_topological_query() {
        let graph = GraphWrapper::new();

        let query = TopologicalQuery::BettiNumbers { max_dimension: 2 };
        let result = graph.query(&query);
        assert!(result.is_ok());

        if let Ok(HyperedgeResult::BettiNumbers(betti)) = result {
            assert_eq!(betti.len(), 3); // Dimensions 0, 1, 2
        }
    }
}
