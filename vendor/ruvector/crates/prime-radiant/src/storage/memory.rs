//! In-Memory Storage Implementation
//!
//! Thread-safe in-memory storage for testing and development.
//! Uses `parking_lot::RwLock` for high-performance concurrent access.
//!
//! # Usage
//!
//! ```rust,ignore
//! use prime_radiant::storage::{InMemoryStorage, GraphStorage, GovernanceStorage};
//!
//! let storage = InMemoryStorage::new();
//!
//! // Store node states
//! storage.store_node("node-1", &[1.0, 0.0, 0.0])?;
//!
//! // Store edges
//! storage.store_edge("node-1", "node-2", 1.0)?;
//!
//! // Store policies
//! let policy_id = storage.store_policy(b"policy-data")?;
//! ```

use super::{GovernanceStorage, GraphStorage, StorageConfig, StorageError};
use ordered_float::OrderedFloat;
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// In-memory storage implementation for testing and development.
///
/// This implementation provides:
/// - Thread-safe access via `parking_lot::RwLock`
/// - Efficient KNN search using brute-force (suitable for small datasets)
/// - Full governance storage support
/// - No persistence (data is lost on drop)
#[derive(Debug)]
pub struct InMemoryStorage {
    /// Node states: node_id -> state vector
    nodes: RwLock<HashMap<String, Vec<f32>>>,

    /// Edges: (source, target) -> weight
    edges: RwLock<HashMap<(String, String), f32>>,

    /// Adjacency list for efficient neighbor lookup: node_id -> set of neighbors
    adjacency: RwLock<HashMap<String, HashSet<String>>>,

    /// Policy bundles: policy_id -> serialized data
    policies: RwLock<HashMap<String, Vec<u8>>>,

    /// Witness records: witness_id -> serialized data
    witnesses: RwLock<HashMap<String, Vec<u8>>>,

    /// Witness records by action: action_id -> list of witness_ids
    witnesses_by_action: RwLock<HashMap<String, Vec<String>>>,

    /// Lineage records: lineage_id -> serialized data
    lineages: RwLock<HashMap<String, Vec<u8>>>,

    /// Event log for audit trail
    event_log: RwLock<Vec<StorageEvent>>,

    /// Configuration
    #[allow(dead_code)]
    config: StorageConfig,
}

/// Storage event for audit logging
#[derive(Debug, Clone)]
pub struct StorageEvent {
    /// Event timestamp (milliseconds since epoch)
    pub timestamp: i64,
    /// Event type
    pub event_type: StorageEventType,
    /// Entity ID involved
    pub entity_id: String,
    /// Optional details
    pub details: Option<String>,
}

/// Type of storage event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageEventType {
    /// Node stored
    NodeStored,
    /// Node retrieved
    NodeRetrieved,
    /// Node deleted
    NodeDeleted,
    /// Edge stored
    EdgeStored,
    /// Edge deleted
    EdgeDeleted,
    /// Policy stored
    PolicyStored,
    /// Policy retrieved
    PolicyRetrieved,
    /// Witness stored
    WitnessStored,
    /// Witness retrieved
    WitnessRetrieved,
    /// Lineage stored
    LineageStored,
}

impl InMemoryStorage {
    /// Create a new in-memory storage instance.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(StorageConfig::default())
    }

    /// Create a new in-memory storage instance with custom configuration.
    #[must_use]
    pub fn with_config(config: StorageConfig) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            adjacency: RwLock::new(HashMap::new()),
            policies: RwLock::new(HashMap::new()),
            witnesses: RwLock::new(HashMap::new()),
            witnesses_by_action: RwLock::new(HashMap::new()),
            lineages: RwLock::new(HashMap::new()),
            event_log: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Get the number of stored nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.read().len()
    }

    /// Get the number of stored edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.read().len()
    }

    /// Get all node IDs.
    #[must_use]
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.read().keys().cloned().collect()
    }

    /// Get all edges as (source, target, weight) tuples.
    #[must_use]
    pub fn all_edges(&self) -> Vec<(String, String, f32)> {
        self.edges
            .read()
            .iter()
            .map(|((s, t), w)| (s.clone(), t.clone(), *w))
            .collect()
    }

    /// Get neighbors of a node.
    #[must_use]
    pub fn get_neighbors(&self, node_id: &str) -> Vec<String> {
        self.adjacency
            .read()
            .get(node_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear all stored data.
    pub fn clear(&self) {
        self.nodes.write().clear();
        self.edges.write().clear();
        self.adjacency.write().clear();
        self.policies.write().clear();
        self.witnesses.write().clear();
        self.witnesses_by_action.write().clear();
        self.lineages.write().clear();
        self.event_log.write().clear();
    }

    /// Get the event log for audit purposes.
    #[must_use]
    pub fn get_event_log(&self) -> Vec<StorageEvent> {
        self.event_log.read().clone()
    }

    /// Log a storage event.
    fn log_event(&self, event_type: StorageEventType, entity_id: String, details: Option<String>) {
        let event = StorageEvent {
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type,
            entity_id,
            details,
        };
        self.event_log.write().push(event);
    }

    /// Compute cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Compute L2 (Euclidean) distance between two vectors.
    fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStorage for InMemoryStorage {
    fn store_node(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError> {
        self.nodes
            .write()
            .insert(node_id.to_string(), state.to_vec());
        self.log_event(
            StorageEventType::NodeStored,
            node_id.to_string(),
            Some(format!("dim={}", state.len())),
        );
        Ok(())
    }

    fn get_node(&self, node_id: &str) -> Result<Option<Vec<f32>>, StorageError> {
        let result = self.nodes.read().get(node_id).cloned();
        if result.is_some() {
            self.log_event(StorageEventType::NodeRetrieved, node_id.to_string(), None);
        }
        Ok(result)
    }

    fn store_edge(&self, source: &str, target: &str, weight: f32) -> Result<(), StorageError> {
        let key = (source.to_string(), target.to_string());
        self.edges.write().insert(key, weight);

        // Update adjacency list (both directions for undirected graph semantics)
        {
            let mut adj = self.adjacency.write();
            adj.entry(source.to_string())
                .or_default()
                .insert(target.to_string());
            adj.entry(target.to_string())
                .or_default()
                .insert(source.to_string());
        }

        self.log_event(
            StorageEventType::EdgeStored,
            format!("{}->{}", source, target),
            Some(format!("weight={}", weight)),
        );
        Ok(())
    }

    fn delete_edge(&self, source: &str, target: &str) -> Result<(), StorageError> {
        let key = (source.to_string(), target.to_string());
        self.edges.write().remove(&key);

        // Update adjacency list
        {
            let mut adj = self.adjacency.write();
            if let Some(neighbors) = adj.get_mut(source) {
                neighbors.remove(target);
            }
            if let Some(neighbors) = adj.get_mut(target) {
                neighbors.remove(source);
            }
        }

        self.log_event(
            StorageEventType::EdgeDeleted,
            format!("{}->{}", source, target),
            None,
        );
        Ok(())
    }

    fn find_similar(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, StorageError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let nodes = self.nodes.read();

        // Use a BTreeMap for efficient top-k extraction (sorted by similarity)
        let mut similarities: BTreeMap<OrderedFloat<f32>, Vec<String>> = BTreeMap::new();

        for (node_id, state) in nodes.iter() {
            let similarity = Self::cosine_similarity(query, state);
            similarities
                .entry(OrderedFloat(-similarity)) // Negative for descending order
                .or_default()
                .push(node_id.clone());
        }

        // Extract top k results
        let mut results = Vec::with_capacity(k);
        for (neg_sim, node_ids) in similarities {
            for node_id in node_ids {
                if results.len() >= k {
                    break;
                }
                results.push((node_id, -neg_sim.0));
            }
            if results.len() >= k {
                break;
            }
        }

        Ok(results)
    }
}

impl GovernanceStorage for InMemoryStorage {
    fn store_policy(&self, bundle: &[u8]) -> Result<String, StorageError> {
        let id = Uuid::new_v4().to_string();
        self.policies.write().insert(id.clone(), bundle.to_vec());
        self.log_event(
            StorageEventType::PolicyStored,
            id.clone(),
            Some(format!("size={}", bundle.len())),
        );
        Ok(id)
    }

    fn get_policy(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let result = self.policies.read().get(id).cloned();
        if result.is_some() {
            self.log_event(StorageEventType::PolicyRetrieved, id.to_string(), None);
        }
        Ok(result)
    }

    fn store_witness(&self, witness: &[u8]) -> Result<String, StorageError> {
        let id = Uuid::new_v4().to_string();
        self.witnesses.write().insert(id.clone(), witness.to_vec());
        self.log_event(
            StorageEventType::WitnessStored,
            id.clone(),
            Some(format!("size={}", witness.len())),
        );
        Ok(id)
    }

    fn get_witnesses_for_action(&self, action_id: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        let witness_ids = self.witnesses_by_action.read();
        let witnesses = self.witnesses.read();

        let ids = witness_ids.get(action_id);
        if ids.is_none() {
            return Ok(Vec::new());
        }

        let result: Vec<Vec<u8>> = ids
            .unwrap()
            .iter()
            .filter_map(|id| witnesses.get(id).cloned())
            .collect();

        if !result.is_empty() {
            self.log_event(
                StorageEventType::WitnessRetrieved,
                action_id.to_string(),
                Some(format!("count={}", result.len())),
            );
        }

        Ok(result)
    }

    fn store_lineage(&self, lineage: &[u8]) -> Result<String, StorageError> {
        let id = Uuid::new_v4().to_string();
        self.lineages.write().insert(id.clone(), lineage.to_vec());
        self.log_event(
            StorageEventType::LineageStored,
            id.clone(),
            Some(format!("size={}", lineage.len())),
        );
        Ok(id)
    }
}

/// Extended in-memory storage with additional indexing capabilities.
#[derive(Debug)]
pub struct IndexedInMemoryStorage {
    /// Base storage
    base: InMemoryStorage,

    /// Node metadata index: tag -> set of node_ids
    node_tags: RwLock<HashMap<String, HashSet<String>>>,

    /// Policy metadata index: name -> policy_id
    policy_by_name: RwLock<HashMap<String, String>>,
}

impl IndexedInMemoryStorage {
    /// Create a new indexed in-memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: InMemoryStorage::new(),
            node_tags: RwLock::new(HashMap::new()),
            policy_by_name: RwLock::new(HashMap::new()),
        }
    }

    /// Store a node with tags for indexing.
    pub fn store_node_with_tags(
        &self,
        node_id: &str,
        state: &[f32],
        tags: &[&str],
    ) -> Result<(), StorageError> {
        self.base.store_node(node_id, state)?;

        let mut tag_index = self.node_tags.write();
        for tag in tags {
            tag_index
                .entry((*tag).to_string())
                .or_default()
                .insert(node_id.to_string());
        }

        Ok(())
    }

    /// Find nodes by tag.
    #[must_use]
    pub fn find_by_tag(&self, tag: &str) -> Vec<String> {
        self.node_tags
            .read()
            .get(tag)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Store a policy with a name for lookup.
    pub fn store_policy_with_name(
        &self,
        name: &str,
        bundle: &[u8],
    ) -> Result<String, StorageError> {
        let id = self.base.store_policy(bundle)?;
        self.policy_by_name
            .write()
            .insert(name.to_string(), id.clone());
        Ok(id)
    }

    /// Get a policy by name.
    pub fn get_policy_by_name(&self, name: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let id = self.policy_by_name.read().get(name).cloned();
        match id {
            Some(id) => self.base.get_policy(&id),
            None => Ok(None),
        }
    }

    /// Get the base storage for direct access.
    #[must_use]
    pub fn base(&self) -> &InMemoryStorage {
        &self.base
    }
}

impl Default for IndexedInMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStorage for IndexedInMemoryStorage {
    fn store_node(&self, node_id: &str, state: &[f32]) -> Result<(), StorageError> {
        self.base.store_node(node_id, state)
    }

    fn get_node(&self, node_id: &str) -> Result<Option<Vec<f32>>, StorageError> {
        self.base.get_node(node_id)
    }

    fn store_edge(&self, source: &str, target: &str, weight: f32) -> Result<(), StorageError> {
        self.base.store_edge(source, target, weight)
    }

    fn delete_edge(&self, source: &str, target: &str) -> Result<(), StorageError> {
        self.base.delete_edge(source, target)
    }

    fn find_similar(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, StorageError> {
        self.base.find_similar(query, k)
    }
}

impl GovernanceStorage for IndexedInMemoryStorage {
    fn store_policy(&self, bundle: &[u8]) -> Result<String, StorageError> {
        self.base.store_policy(bundle)
    }

    fn get_policy(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        self.base.get_policy(id)
    }

    fn store_witness(&self, witness: &[u8]) -> Result<String, StorageError> {
        self.base.store_witness(witness)
    }

    fn get_witnesses_for_action(&self, action_id: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        self.base.get_witnesses_for_action(action_id)
    }

    fn store_lineage(&self, lineage: &[u8]) -> Result<String, StorageError> {
        self.base.store_lineage(lineage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_storage_nodes() {
        let storage = InMemoryStorage::new();

        // Store a node
        storage.store_node("node-1", &[1.0, 0.0, 0.0]).unwrap();
        storage.store_node("node-2", &[0.0, 1.0, 0.0]).unwrap();

        assert_eq!(storage.node_count(), 2);

        // Retrieve node
        let state = storage.get_node("node-1").unwrap();
        assert!(state.is_some());
        assert_eq!(state.unwrap(), vec![1.0, 0.0, 0.0]);

        // Non-existent node
        let missing = storage.get_node("node-999").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_in_memory_storage_edges() {
        let storage = InMemoryStorage::new();

        // Store nodes
        storage.store_node("a", &[1.0]).unwrap();
        storage.store_node("b", &[2.0]).unwrap();
        storage.store_node("c", &[3.0]).unwrap();

        // Store edges
        storage.store_edge("a", "b", 1.0).unwrap();
        storage.store_edge("b", "c", 2.0).unwrap();

        assert_eq!(storage.edge_count(), 2);

        // Check adjacency
        let neighbors = storage.get_neighbors("b");
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors.contains(&"a".to_string()));
        assert!(neighbors.contains(&"c".to_string()));

        // Delete edge
        storage.delete_edge("a", "b").unwrap();
        assert_eq!(storage.edge_count(), 1);

        let neighbors = storage.get_neighbors("b");
        assert_eq!(neighbors.len(), 1);
        assert!(!neighbors.contains(&"a".to_string()));
    }

    #[test]
    fn test_find_similar() {
        let storage = InMemoryStorage::new();

        // Store nodes with different orientations
        storage.store_node("north", &[0.0, 1.0, 0.0]).unwrap();
        storage.store_node("south", &[0.0, -1.0, 0.0]).unwrap();
        storage.store_node("east", &[1.0, 0.0, 0.0]).unwrap();
        storage
            .store_node("northeast", &[0.707, 0.707, 0.0])
            .unwrap();

        // Query for vectors similar to north
        let query = vec![0.0, 1.0, 0.0];
        let results = storage.find_similar(&query, 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "north");
        assert!((results[0].1 - 1.0).abs() < 0.001); // Perfect match
        assert_eq!(results[1].0, "northeast"); // Second closest
    }

    #[test]
    fn test_governance_storage() {
        let storage = InMemoryStorage::new();

        // Store policy
        let policy_data = b"test policy data";
        let policy_id = storage.store_policy(policy_data).unwrap();

        // Retrieve policy
        let retrieved = storage.get_policy(&policy_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), policy_data.to_vec());

        // Store witness
        let witness_data = b"test witness data";
        let witness_id = storage.store_witness(witness_data).unwrap();
        assert!(!witness_id.is_empty());

        // Store lineage
        let lineage_data = b"test lineage data";
        let lineage_id = storage.store_lineage(lineage_data).unwrap();
        assert!(!lineage_id.is_empty());
    }

    #[test]
    fn test_event_log() {
        let storage = InMemoryStorage::new();

        storage.store_node("test", &[1.0]).unwrap();
        storage.get_node("test").unwrap();
        storage.store_edge("a", "b", 1.0).unwrap();

        let log = storage.get_event_log();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].event_type, StorageEventType::NodeStored);
        assert_eq!(log[1].event_type, StorageEventType::NodeRetrieved);
        assert_eq!(log[2].event_type, StorageEventType::EdgeStored);
    }

    #[test]
    fn test_clear() {
        let storage = InMemoryStorage::new();

        storage.store_node("node", &[1.0]).unwrap();
        storage.store_edge("a", "b", 1.0).unwrap();
        storage.store_policy(b"policy").unwrap();

        assert!(storage.node_count() > 0);

        storage.clear();

        assert_eq!(storage.node_count(), 0);
        assert_eq!(storage.edge_count(), 0);
        assert_eq!(storage.get_event_log().len(), 0);
    }

    #[test]
    fn test_indexed_storage() {
        let storage = IndexedInMemoryStorage::new();

        // Store with tags
        storage
            .store_node_with_tags("node-1", &[1.0, 0.0], &["important", "category-a"])
            .unwrap();
        storage
            .store_node_with_tags("node-2", &[0.0, 1.0], &["important"])
            .unwrap();
        storage
            .store_node_with_tags("node-3", &[1.0, 1.0], &["category-a"])
            .unwrap();

        // Find by tag
        let important = storage.find_by_tag("important");
        assert_eq!(important.len(), 2);

        let category_a = storage.find_by_tag("category-a");
        assert_eq!(category_a.len(), 2);

        // Store and retrieve policy by name
        storage
            .store_policy_with_name("default", b"default policy")
            .unwrap();

        let policy = storage.get_policy_by_name("default").unwrap();
        assert!(policy.is_some());
        assert_eq!(policy.unwrap(), b"default policy".to_vec());
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let sim = InMemoryStorage::cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]);
        assert!((sim - 1.0).abs() < 0.001);

        // Orthogonal vectors
        let sim = InMemoryStorage::cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(sim.abs() < 0.001);

        // Opposite vectors
        let sim = InMemoryStorage::cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_l2_distance() {
        // Same point
        let dist = InMemoryStorage::l2_distance(&[0.0, 0.0], &[0.0, 0.0]);
        assert!(dist.abs() < 0.001);

        // Unit distance
        let dist = InMemoryStorage::l2_distance(&[0.0, 0.0], &[1.0, 0.0]);
        assert!((dist - 1.0).abs() < 0.001);

        // Diagonal
        let dist = InMemoryStorage::l2_distance(&[0.0, 0.0], &[1.0, 1.0]);
        assert!((dist - std::f32::consts::SQRT_2).abs() < 0.001);
    }
}
