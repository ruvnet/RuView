//! # Hypergraph Support for N-ary Relationships
//!
//! Implements hypergraph structures for representing complex multi-entity relationships
//! beyond traditional pairwise similarity. Based on HyperGraphRAG (NeurIPS 2025) architecture.

use crate::error::{Result, RuvectorError};
use crate::types::{DistanceMetric, VectorId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Hyperedge connecting multiple vectors with description and embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hyperedge {
    /// Unique identifier for the hyperedge
    pub id: String,
    /// Vector IDs connected by this hyperedge
    pub nodes: Vec<VectorId>,
    /// Natural language description of the relationship
    pub description: String,
    /// Embedding of the hyperedge description
    pub embedding: Vec<f32>,
    /// Confidence weight (0.0-1.0)
    pub confidence: f32,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

/// Temporal hyperedge with time attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalHyperedge {
    /// Base hyperedge
    pub hyperedge: Hyperedge,
    /// Creation timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Optional expiration timestamp
    pub expires_at: Option<u64>,
    /// Temporal context (hourly, daily, monthly)
    pub granularity: TemporalGranularity,
}

/// Temporal granularity for indexing
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TemporalGranularity {
    Hourly,
    Daily,
    Monthly,
    Yearly,
}

impl Hyperedge {
    /// Create a new hyperedge
    pub fn new(
        nodes: Vec<VectorId>,
        description: String,
        embedding: Vec<f32>,
        confidence: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            nodes,
            description,
            embedding,
            confidence: confidence.clamp(0.0, 1.0),
            metadata: HashMap::new(),
        }
    }

    /// Get hyperedge order (number of nodes)
    pub fn order(&self) -> usize {
        self.nodes.len()
    }

    /// Check if hyperedge contains a specific node
    pub fn contains_node(&self, node: &VectorId) -> bool {
        self.nodes.contains(node)
    }
}

impl TemporalHyperedge {
    /// Create a new temporal hyperedge with current timestamp
    pub fn new(hyperedge: Hyperedge, granularity: TemporalGranularity) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            hyperedge,
            timestamp,
            expires_at: None,
            granularity,
        }
    }

    /// Check if hyperedge is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            now > expires_at
        } else {
            false
        }
    }

    /// Get time bucket for indexing
    pub fn time_bucket(&self) -> u64 {
        match self.granularity {
            TemporalGranularity::Hourly => self.timestamp / 3600,
            TemporalGranularity::Daily => self.timestamp / 86400,
            TemporalGranularity::Monthly => self.timestamp / (86400 * 30),
            TemporalGranularity::Yearly => self.timestamp / (86400 * 365),
        }
    }
}

/// Hypergraph index with bipartite graph storage
pub struct HypergraphIndex {
    /// Entity nodes
    entities: HashMap<VectorId, Vec<f32>>,
    /// Hyperedges
    hyperedges: HashMap<String, Hyperedge>,
    /// Temporal hyperedges indexed by time bucket
    temporal_index: HashMap<u64, Vec<String>>,
    /// Bipartite graph: entity -> hyperedge IDs
    entity_to_hyperedges: HashMap<VectorId, HashSet<String>>,
    /// Bipartite graph: hyperedge -> entity IDs
    hyperedge_to_entities: HashMap<String, HashSet<VectorId>>,
    /// Distance metric for embeddings
    distance_metric: DistanceMetric,
}

impl HypergraphIndex {
    /// Create a new hypergraph index
    pub fn new(distance_metric: DistanceMetric) -> Self {
        Self {
            entities: HashMap::new(),
            hyperedges: HashMap::new(),
            temporal_index: HashMap::new(),
            entity_to_hyperedges: HashMap::new(),
            hyperedge_to_entities: HashMap::new(),
            distance_metric,
        }
    }

    /// Add an entity node
    pub fn add_entity(&mut self, id: VectorId, embedding: Vec<f32>) {
        self.entities.insert(id.clone(), embedding);
        self.entity_to_hyperedges.entry(id).or_default();
    }

    /// Add a hyperedge
    pub fn add_hyperedge(&mut self, hyperedge: Hyperedge) -> Result<()> {
        let edge_id = hyperedge.id.clone();

        // Verify all nodes exist
        for node in &hyperedge.nodes {
            if !self.entities.contains_key(node) {
                return Err(RuvectorError::InvalidInput(format!(
                    "Entity {} not found in hypergraph",
                    node
                )));
            }
        }

        // Update bipartite graph
        for node in &hyperedge.nodes {
            self.entity_to_hyperedges
                .entry(node.clone())
                .or_default()
                .insert(edge_id.clone());
        }

        let nodes_set: HashSet<VectorId> = hyperedge.nodes.iter().cloned().collect();
        self.hyperedge_to_entities
            .insert(edge_id.clone(), nodes_set);

        self.hyperedges.insert(edge_id, hyperedge);
        Ok(())
    }

    /// Add a temporal hyperedge
    pub fn add_temporal_hyperedge(&mut self, temporal_edge: TemporalHyperedge) -> Result<()> {
        let bucket = temporal_edge.time_bucket();
        let edge_id = temporal_edge.hyperedge.id.clone();

        self.add_hyperedge(temporal_edge.hyperedge)?;

        self.temporal_index.entry(bucket).or_default().push(edge_id);

        Ok(())
    }

    /// Search hyperedges by embedding similarity
    pub fn search_hyperedges(&self, query_embedding: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut results: Vec<(String, f32)> = self
            .hyperedges
            .iter()
            .map(|(id, edge)| {
                let distance = self.compute_distance(query_embedding, &edge.embedding);
                (id.clone(), distance)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(k);
        results
    }

    /// Get k-hop neighbors in hypergraph
    /// Returns all nodes reachable within k hops from the start node
    pub fn k_hop_neighbors(&self, start_node: VectorId, k: usize) -> HashSet<VectorId> {
        let mut visited = HashSet::new();
        let mut current_layer = HashSet::new();
        current_layer.insert(start_node.clone());
        visited.insert(start_node); // Start node is at distance 0

        for _hop in 0..k {
            let mut next_layer = HashSet::new();

            for node in current_layer.iter() {
                // Get all hyperedges containing this node
                if let Some(hyperedges) = self.entity_to_hyperedges.get(node) {
                    for edge_id in hyperedges {
                        // Get all nodes in this hyperedge
                        if let Some(nodes) = self.hyperedge_to_entities.get(edge_id) {
                            for neighbor in nodes.iter() {
                                if !visited.contains(neighbor) {
                                    visited.insert(neighbor.clone());
                                    next_layer.insert(neighbor.clone());
                                }
                            }
                        }
                    }
                }
            }

            if next_layer.is_empty() {
                break;
            }
            current_layer = next_layer;
        }

        visited
    }

    /// Query temporal hyperedges in a time range
    pub fn query_temporal_range(&self, start_bucket: u64, end_bucket: u64) -> Vec<String> {
        let mut results = Vec::new();
        for bucket in start_bucket..=end_bucket {
            if let Some(edges) = self.temporal_index.get(&bucket) {
                results.extend(edges.iter().cloned());
            }
        }
        results
    }

    /// Get hyperedge by ID
    pub fn get_hyperedge(&self, id: &str) -> Option<&Hyperedge> {
        self.hyperedges.get(id)
    }

    /// Get statistics
    pub fn stats(&self) -> HypergraphStats {
        let total_edges = self.hyperedges.len();
        let total_entities = self.entities.len();
        let avg_degree = if total_entities > 0 {
            self.entity_to_hyperedges
                .values()
                .map(|edges| edges.len())
                .sum::<usize>() as f32
                / total_entities as f32
        } else {
            0.0
        };

        HypergraphStats {
            total_entities,
            total_hyperedges: total_edges,
            avg_entity_degree: avg_degree,
        }
    }

    fn compute_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        crate::distance::distance(a, b, self.distance_metric).unwrap_or(f32::MAX)
    }
}

/// Hypergraph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypergraphStats {
    pub total_entities: usize,
    pub total_hyperedges: usize,
    pub avg_entity_degree: f32,
}

/// Causal hypergraph memory for agent reasoning
pub struct CausalMemory {
    /// Hypergraph index
    index: HypergraphIndex,
    /// Causal relationship tracking: (cause_id, effect_id) -> success_count
    causal_counts: HashMap<(VectorId, VectorId), u32>,
    /// Action latencies: action_id -> avg_latency_ms
    latencies: HashMap<VectorId, f32>,
    /// Utility function weights
    alpha: f32, // similarity weight
    beta: f32,  // causal uplift weight
    gamma: f32, // latency penalty weight
}

impl CausalMemory {
    /// Create a new causal memory with default utility weights
    pub fn new(distance_metric: DistanceMetric) -> Self {
        Self {
            index: HypergraphIndex::new(distance_metric),
            causal_counts: HashMap::new(),
            latencies: HashMap::new(),
            alpha: 0.7,
            beta: 0.2,
            gamma: 0.1,
        }
    }

    /// Set custom utility function weights
    pub fn with_weights(mut self, alpha: f32, beta: f32, gamma: f32) -> Self {
        self.alpha = alpha;
        self.beta = beta;
        self.gamma = gamma;
        self
    }

    /// Add a causal relationship
    pub fn add_causal_edge(
        &mut self,
        cause: VectorId,
        effect: VectorId,
        context: Vec<VectorId>,
        description: String,
        embedding: Vec<f32>,
        latency_ms: f32,
    ) -> Result<()> {
        // Create hyperedge connecting cause, effect, and context
        let mut nodes = vec![cause.clone(), effect.clone()];
        nodes.extend(context);

        let hyperedge = Hyperedge::new(nodes, description, embedding, 1.0);
        self.index.add_hyperedge(hyperedge)?;

        // Update causal counts
        *self
            .causal_counts
            .entry((cause.clone(), effect.clone()))
            .or_insert(0) += 1;

        // Update latency
        let entry = self.latencies.entry(cause).or_insert(0.0);
        *entry = (*entry + latency_ms) / 2.0; // Running average

        Ok(())
    }

    /// Query with utility function: U = α·similarity + β·causal_uplift - γ·latency
    pub fn query_with_utility(
        &self,
        query_embedding: &[f32],
        action_id: VectorId,
        k: usize,
    ) -> Vec<(String, f32)> {
        let mut results: Vec<(String, f32)> = self
            .index
            .hyperedges
            .iter()
            .filter(|(_, edge)| edge.contains_node(&action_id))
            .map(|(id, edge)| {
                let similarity = 1.0
                    - self
                        .index
                        .compute_distance(query_embedding, &edge.embedding);
                let causal_uplift = self.compute_causal_uplift(&edge.nodes);
                let latency = self.latencies.get(&action_id).copied().unwrap_or(0.0);

                let utility = self.alpha * similarity + self.beta * causal_uplift
                    - self.gamma * (latency / 1000.0); // Normalize latency to 0-1 range

                (id.clone(), utility)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // Sort by utility descending
        results.truncate(k);
        results
    }

    fn compute_causal_uplift(&self, nodes: &[VectorId]) -> f32 {
        if nodes.len() < 2 {
            return 0.0;
        }

        // Compute average causal strength for pairs in this hyperedge
        let mut total_uplift = 0.0;
        let mut count = 0;

        for i in 0..nodes.len() - 1 {
            for j in i + 1..nodes.len() {
                if let Some(&success_count) = self
                    .causal_counts
                    .get(&(nodes[i].clone(), nodes[j].clone()))
                {
                    total_uplift += (success_count as f32).ln_1p(); // Log scale
                    count += 1;
                }
            }
        }

        if count > 0 {
            total_uplift / count as f32
        } else {
            0.0
        }
    }

    /// Get hypergraph index
    pub fn index(&self) -> &HypergraphIndex {
        &self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyperedge_creation() {
        let nodes = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let desc = "Test relationship".to_string();
        let embedding = vec![0.1, 0.2, 0.3];
        let edge = Hyperedge::new(nodes, desc, embedding, 0.95);

        assert_eq!(edge.order(), 3);
        assert!(edge.contains_node(&"1".to_string()));
        assert!(!edge.contains_node(&"4".to_string()));
        assert_eq!(edge.confidence, 0.95);
    }

    #[test]
    fn test_temporal_hyperedge() {
        let nodes = vec!["1".to_string(), "2".to_string()];
        let desc = "Temporal relationship".to_string();
        let embedding = vec![0.1, 0.2];
        let edge = Hyperedge::new(nodes, desc, embedding, 1.0);

        let temporal = TemporalHyperedge::new(edge, TemporalGranularity::Hourly);

        assert!(!temporal.is_expired());
        assert!(temporal.time_bucket() > 0);
    }

    #[test]
    fn test_hypergraph_index() {
        let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

        // Add entities
        index.add_entity("1".to_string(), vec![1.0, 0.0, 0.0]);
        index.add_entity("2".to_string(), vec![0.0, 1.0, 0.0]);
        index.add_entity("3".to_string(), vec![0.0, 0.0, 1.0]);

        // Add hyperedge
        let edge = Hyperedge::new(
            vec!["1".to_string(), "2".to_string(), "3".to_string()],
            "Triple relationship".to_string(),
            vec![0.5, 0.5, 0.5],
            0.9,
        );
        index.add_hyperedge(edge).unwrap();

        let stats = index.stats();
        assert_eq!(stats.total_entities, 3);
        assert_eq!(stats.total_hyperedges, 1);
    }

    #[test]
    fn test_k_hop_neighbors() {
        let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

        // Create a small hypergraph
        index.add_entity("1".to_string(), vec![1.0]);
        index.add_entity("2".to_string(), vec![1.0]);
        index.add_entity("3".to_string(), vec![1.0]);
        index.add_entity("4".to_string(), vec![1.0]);

        let edge1 = Hyperedge::new(
            vec!["1".to_string(), "2".to_string()],
            "e1".to_string(),
            vec![1.0],
            1.0,
        );
        let edge2 = Hyperedge::new(
            vec!["2".to_string(), "3".to_string()],
            "e2".to_string(),
            vec![1.0],
            1.0,
        );
        let edge3 = Hyperedge::new(
            vec!["3".to_string(), "4".to_string()],
            "e3".to_string(),
            vec![1.0],
            1.0,
        );

        index.add_hyperedge(edge1).unwrap();
        index.add_hyperedge(edge2).unwrap();
        index.add_hyperedge(edge3).unwrap();

        let neighbors = index.k_hop_neighbors("1".to_string(), 2);
        assert!(neighbors.contains(&"1".to_string()));
        assert!(neighbors.contains(&"2".to_string()));
        assert!(neighbors.contains(&"3".to_string()));
    }

    #[test]
    fn test_causal_memory() {
        let mut memory = CausalMemory::new(DistanceMetric::Cosine);

        memory.index.add_entity("1".to_string(), vec![1.0, 0.0]);
        memory.index.add_entity("2".to_string(), vec![0.0, 1.0]);

        memory
            .add_causal_edge(
                "1".to_string(),
                "2".to_string(),
                vec![],
                "Action 1 causes effect 2".to_string(),
                vec![0.5, 0.5],
                100.0,
            )
            .unwrap();

        let results = memory.query_with_utility(&[0.6, 0.4], "1".to_string(), 5);
        assert!(!results.is_empty());
    }
}
