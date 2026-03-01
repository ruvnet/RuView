//! Hyperedge structures and indexing
//!
//! Implements hyperedges (edges connecting more than 2 vertices) and
//! efficient indices for querying them.

use dashmap::DashMap;
use exo_core::{EntityId, HyperedgeId, Relation, RelationType, SubstrateTime};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A hyperedge connecting multiple entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hyperedge {
    /// Unique identifier
    pub id: HyperedgeId,
    /// Entities connected by this hyperedge
    pub entities: Vec<EntityId>,
    /// Relation type and properties
    pub relation: Relation,
    /// Edge weight
    pub weight: f32,
    /// Creation timestamp
    pub created_at: SubstrateTime,
}

impl Hyperedge {
    /// Create a new hyperedge
    pub fn new(entities: Vec<EntityId>, relation: Relation) -> Self {
        Self {
            id: HyperedgeId::new(),
            entities,
            relation,
            weight: 1.0,
            created_at: SubstrateTime::now(),
        }
    }

    /// Get the arity (number of entities) of this hyperedge
    pub fn arity(&self) -> usize {
        self.entities.len()
    }

    /// Check if this hyperedge contains an entity
    pub fn contains_entity(&self, entity: &EntityId) -> bool {
        self.entities.contains(entity)
    }
}

/// Index structure for efficient hyperedge queries
///
/// Maintains inverted indices for fast lookups by entity and relation type.
pub struct HyperedgeIndex {
    /// Hyperedge storage
    edges: Arc<DashMap<HyperedgeId, Hyperedge>>,
    /// Inverted index: entity -> hyperedges containing it
    entity_index: Arc<DashMap<EntityId, Vec<HyperedgeId>>>,
    /// Relation type index
    relation_index: Arc<DashMap<RelationType, Vec<HyperedgeId>>>,
}

impl HyperedgeIndex {
    /// Create a new empty hyperedge index
    pub fn new() -> Self {
        Self {
            edges: Arc::new(DashMap::new()),
            entity_index: Arc::new(DashMap::new()),
            relation_index: Arc::new(DashMap::new()),
        }
    }

    /// Insert a hyperedge (from pseudocode: CreateHyperedge)
    ///
    /// Creates a new hyperedge and updates all indices.
    pub fn insert(&self, entities: &[EntityId], relation: &Relation) -> HyperedgeId {
        let hyperedge = Hyperedge::new(entities.to_vec(), relation.clone());
        let hyperedge_id = hyperedge.id;

        // Insert into hyperedge storage
        self.edges.insert(hyperedge_id, hyperedge);

        // Update inverted index (entity -> hyperedges)
        for entity in entities {
            self.entity_index
                .entry(*entity)
                .or_insert_with(Vec::new)
                .push(hyperedge_id);
        }

        // Update relation type index
        self.relation_index
            .entry(relation.relation_type.clone())
            .or_insert_with(Vec::new)
            .push(hyperedge_id);

        hyperedge_id
    }

    /// Get a hyperedge by ID
    pub fn get(&self, id: &HyperedgeId) -> Option<Hyperedge> {
        self.edges.get(id).map(|entry| entry.clone())
    }

    /// Get all hyperedges containing a specific entity
    pub fn get_by_entity(&self, entity: &EntityId) -> Vec<HyperedgeId> {
        self.entity_index
            .get(entity)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Get all hyperedges of a specific relation type
    pub fn get_by_relation(&self, relation_type: &RelationType) -> Vec<HyperedgeId> {
        self.relation_index
            .get(relation_type)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Get the number of hyperedges
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Get the maximum hyperedge size (arity)
    pub fn max_size(&self) -> usize {
        self.edges
            .iter()
            .map(|entry| entry.value().arity())
            .max()
            .unwrap_or(0)
    }

    /// Remove a hyperedge
    pub fn remove(&self, id: &HyperedgeId) -> Option<Hyperedge> {
        if let Some((_, hyperedge)) = self.edges.remove(id) {
            // Remove from entity index
            for entity in &hyperedge.entities {
                if let Some(mut entry) = self.entity_index.get_mut(entity) {
                    entry.retain(|he_id| he_id != id);
                }
            }

            // Remove from relation index
            if let Some(mut entry) = self
                .relation_index
                .get_mut(&hyperedge.relation.relation_type)
            {
                entry.retain(|he_id| he_id != id);
            }

            Some(hyperedge)
        } else {
            None
        }
    }

    /// Get all hyperedges
    pub fn all(&self) -> Vec<Hyperedge> {
        self.edges.iter().map(|entry| entry.clone()).collect()
    }

    /// Find hyperedges connecting a specific set of entities
    ///
    /// Returns hyperedges that contain all of the given entities.
    pub fn find_connecting(&self, entities: &[EntityId]) -> Vec<HyperedgeId> {
        if entities.is_empty() {
            return Vec::new();
        }

        // Start with hyperedges containing the first entity
        let mut candidates = self.get_by_entity(&entities[0]);

        // Filter to those containing all entities
        candidates.retain(|he_id| {
            if let Some(he) = self.get(he_id) {
                entities.iter().all(|e| he.contains_entity(e))
            } else {
                false
            }
        });

        candidates
    }
}

impl Default for HyperedgeIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::RelationType;

    #[test]
    fn test_hyperedge_creation() {
        let entities = vec![EntityId::new(), EntityId::new(), EntityId::new()];
        let relation = Relation {
            relation_type: RelationType::new("test"),
            properties: serde_json::json!({}),
        };

        let he = Hyperedge::new(entities.clone(), relation);

        assert_eq!(he.arity(), 3);
        assert!(he.contains_entity(&entities[0]));
        assert_eq!(he.weight, 1.0);
    }

    #[test]
    fn test_hyperedge_index() {
        let index = HyperedgeIndex::new();

        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();

        let relation = Relation {
            relation_type: RelationType::new("test"),
            properties: serde_json::json!({}),
        };

        // Insert hyperedge
        let he_id = index.insert(&[e1, e2, e3], &relation);

        // Verify retrieval
        assert!(index.get(&he_id).is_some());
        assert_eq!(index.get_by_entity(&e1).len(), 1);
        assert_eq!(index.get_by_entity(&e2).len(), 1);
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_find_connecting() {
        let index = HyperedgeIndex::new();

        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();
        let e4 = EntityId::new();

        let relation = Relation {
            relation_type: RelationType::new("test"),
            properties: serde_json::json!({}),
        };

        // Create multiple hyperedges
        index.insert(&[e1, e2], &relation);
        let he2 = index.insert(&[e1, e2, e3], &relation);
        index.insert(&[e1, e4], &relation);

        // Find hyperedges connecting e1, e2, e3
        let connecting = index.find_connecting(&[e1, e2, e3]);
        assert_eq!(connecting.len(), 1);
        assert_eq!(connecting[0], he2);
    }
}
