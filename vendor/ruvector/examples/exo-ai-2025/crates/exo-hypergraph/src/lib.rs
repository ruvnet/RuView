//! Hypergraph Substrate for Higher-Order Relational Reasoning
//!
//! This crate provides a hypergraph-based substrate for representing and querying
//! complex, higher-order relationships between entities. It extends beyond simple
//! pairwise graphs to support hyperedges that span arbitrary sets of entities.
//!
//! # Features
//!
//! - **Hyperedge Support**: Relations spanning multiple entities (not just pairs)
//! - **Topological Data Analysis**: Persistent homology and Betti number computation
//! - **Sheaf Theory**: Consistency checks for distributed data structures
//! - **Thread-Safe**: Lock-free concurrent access using DashMap
//!
//! # Example
//!
//! ```rust
//! use exo_hypergraph::{HypergraphSubstrate, HypergraphConfig};
//! use exo_core::{EntityId, Relation, RelationType};
//!
//! let config = HypergraphConfig::default();
//! let mut hypergraph = HypergraphSubstrate::new(config);
//!
//! // Create entities
//! let entity1 = EntityId::new();
//! let entity2 = EntityId::new();
//! let entity3 = EntityId::new();
//!
//! // Add entities to the hypergraph
//! hypergraph.add_entity(entity1, serde_json::json!({"name": "Alice"}));
//! hypergraph.add_entity(entity2, serde_json::json!({"name": "Bob"}));
//! hypergraph.add_entity(entity3, serde_json::json!({"name": "Charlie"}));
//!
//! // Create a 3-way hyperedge
//! let relation = Relation {
//!     relation_type: RelationType::new("collaboration"),
//!     properties: serde_json::json!({"weight": 0.9}),
//! };
//!
//! let hyperedge_id = hypergraph.create_hyperedge(
//!     &[entity1, entity2, entity3],
//!     &relation
//! ).unwrap();
//! ```

pub mod hyperedge;
pub mod sheaf;
pub mod sparse_tda;
pub mod topology;

pub use hyperedge::{Hyperedge, HyperedgeIndex};
pub use sheaf::{SheafInconsistency, SheafStructure};
pub use sparse_tda::{
    PersistenceBar, PersistenceDiagram as SparsePersistenceDiagram, SparseRipsComplex,
};
pub use topology::{PersistenceDiagram, SimplicialComplex};

use dashmap::DashMap;
use exo_core::{
    EntityId, Error, HyperedgeId, HyperedgeResult, Relation, SectionId, SheafConsistencyResult,
    TopologicalQuery,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for hypergraph substrate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypergraphConfig {
    /// Enable sheaf consistency checking
    pub enable_sheaf: bool,
    /// Maximum dimension for topological computations
    pub max_dimension: usize,
    /// Epsilon tolerance for topology operations
    pub epsilon: f32,
}

impl Default for HypergraphConfig {
    fn default() -> Self {
        Self {
            enable_sheaf: false,
            max_dimension: 3,
            epsilon: 1e-6,
        }
    }
}

/// Hypergraph substrate for higher-order relations
///
/// Provides a substrate for storing and querying hypergraphs, supporting:
/// - Hyperedges spanning multiple entities
/// - Topological data analysis (persistent homology, Betti numbers)
/// - Sheaf-theoretic consistency checks
pub struct HypergraphSubstrate {
    /// Configuration
    #[allow(dead_code)]
    config: HypergraphConfig,
    /// Entity storage (placeholder - could integrate with actual graph DB)
    entities: Arc<DashMap<EntityId, EntityRecord>>,
    /// Hyperedge index (relations spanning >2 entities)
    hyperedges: HyperedgeIndex,
    /// Simplicial complex for TDA
    topology: SimplicialComplex,
    /// Sheaf structure for consistency (optional)
    sheaf: Option<SheafStructure>,
}

/// Entity record (minimal placeholder)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityRecord {
    id: EntityId,
    metadata: serde_json::Value,
}

impl HypergraphSubstrate {
    /// Create a new hypergraph substrate
    pub fn new(config: HypergraphConfig) -> Self {
        let sheaf = if config.enable_sheaf {
            Some(SheafStructure::new())
        } else {
            None
        };

        Self {
            config,
            entities: Arc::new(DashMap::new()),
            hyperedges: HyperedgeIndex::new(),
            topology: SimplicialComplex::new(),
            sheaf,
        }
    }

    /// Add an entity to the hypergraph
    pub fn add_entity(&self, id: EntityId, metadata: serde_json::Value) {
        self.entities.insert(id, EntityRecord { id, metadata });
    }

    /// Check if entity exists
    pub fn contains_entity(&self, id: &EntityId) -> bool {
        self.entities.contains_key(id)
    }

    /// Create hyperedge spanning multiple entities
    ///
    /// # Arguments
    ///
    /// * `entities` - Slice of entity IDs to connect
    /// * `relation` - Relation describing the connection
    ///
    /// # Returns
    ///
    /// The ID of the created hyperedge
    ///
    /// # Errors
    ///
    /// Returns `Error::EntityNotFound` if any entity doesn't exist
    pub fn create_hyperedge(
        &mut self,
        entities: &[EntityId],
        relation: &Relation,
    ) -> Result<HyperedgeId, Error> {
        // Validate entity existence (from pseudocode)
        for entity in entities {
            if !self.contains_entity(entity) {
                return Err(Error::NotFound(format!("Entity not found: {}", entity)));
            }
        }

        // Create hyperedge in index
        let hyperedge_id = self.hyperedges.insert(entities, relation);

        // Update simplicial complex
        self.topology.add_simplex(entities);

        // Update sheaf sections if enabled
        if let Some(ref mut sheaf) = self.sheaf {
            sheaf.update_sections(hyperedge_id, entities)?;
        }

        Ok(hyperedge_id)
    }

    /// Query hyperedges containing a specific entity
    pub fn hyperedges_for_entity(&self, entity: &EntityId) -> Vec<HyperedgeId> {
        self.hyperedges.get_by_entity(entity)
    }

    /// Get hyperedge by ID
    pub fn get_hyperedge(&self, id: &HyperedgeId) -> Option<Hyperedge> {
        self.hyperedges.get(id)
    }

    /// Topological query: find persistent features
    ///
    /// Computes persistent homology features in the specified dimension
    /// over the given epsilon range.
    pub fn persistent_homology(
        &self,
        dimension: usize,
        epsilon_range: (f32, f32),
    ) -> PersistenceDiagram {
        self.topology.persistent_homology(dimension, epsilon_range)
    }

    /// Query Betti numbers (topological invariants)
    ///
    /// Returns the Betti numbers up to max_dim, where:
    /// - β₀ = number of connected components
    /// - β₁ = number of 1-dimensional holes (loops)
    /// - β₂ = number of 2-dimensional holes (voids)
    /// - etc.
    pub fn betti_numbers(&self, max_dim: usize) -> Vec<usize> {
        (0..=max_dim)
            .map(|d| self.topology.betti_number(d))
            .collect()
    }

    /// Sheaf consistency: check local-to-global coherence
    ///
    /// Checks if local sections are consistent on their overlaps,
    /// following the sheaf axioms.
    pub fn check_sheaf_consistency(&self, sections: &[SectionId]) -> SheafConsistencyResult {
        match &self.sheaf {
            Some(sheaf) => sheaf.check_consistency(sections),
            None => SheafConsistencyResult::NotConfigured,
        }
    }

    /// Execute a topological query
    pub fn query(&self, query: &TopologicalQuery) -> Result<HyperedgeResult, Error> {
        match query {
            TopologicalQuery::PersistentHomology {
                dimension,
                epsilon_range,
            } => {
                let diagram = self.persistent_homology(*dimension, *epsilon_range);
                Ok(HyperedgeResult::PersistenceDiagram(diagram.pairs))
            }
            TopologicalQuery::BettiNumbers { max_dimension } => {
                let betti = self.betti_numbers(*max_dimension);
                Ok(HyperedgeResult::BettiNumbers(betti))
            }
            TopologicalQuery::SheafConsistency { local_sections } => {
                let result = self.check_sheaf_consistency(local_sections);
                Ok(HyperedgeResult::SheafConsistency(result))
            }
        }
    }

    /// Get statistics about the hypergraph
    pub fn stats(&self) -> HypergraphStats {
        HypergraphStats {
            num_entities: self.entities.len(),
            num_hyperedges: self.hyperedges.len(),
            max_hyperedge_size: self.hyperedges.max_size(),
        }
    }
}

/// Statistics about the hypergraph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypergraphStats {
    pub num_entities: usize,
    pub num_hyperedges: usize,
    pub max_hyperedge_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::RelationType;

    #[test]
    fn test_create_hyperedge() {
        let config = HypergraphConfig::default();
        let mut hg = HypergraphSubstrate::new(config);

        // Add entities
        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();

        hg.add_entity(e1, serde_json::json!({}));
        hg.add_entity(e2, serde_json::json!({}));
        hg.add_entity(e3, serde_json::json!({}));

        // Create 3-way hyperedge
        let relation = Relation {
            relation_type: RelationType::new("test"),
            properties: serde_json::json!({}),
        };

        let he_id = hg.create_hyperedge(&[e1, e2, e3], &relation).unwrap();

        // Verify
        assert!(hg.get_hyperedge(&he_id).is_some());
        assert_eq!(hg.hyperedges_for_entity(&e1).len(), 1);
    }

    #[test]
    fn test_betti_numbers() {
        let config = HypergraphConfig::default();
        let hg = HypergraphSubstrate::new(config);

        // Empty hypergraph should have β₀ = 0 (no components)
        let betti = hg.betti_numbers(2);
        assert_eq!(betti, vec![0, 0, 0]);
    }
}
