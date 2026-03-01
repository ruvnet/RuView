//! Graph sharding strategies for distributed hypergraphs
//!
//! Provides multiple partitioning strategies optimized for graph workloads:
//! - Hash-based node partitioning for uniform distribution
//! - Range-based partitioning for locality-aware queries
//! - Edge-cut minimization for reducing cross-shard communication

use crate::{GraphError, Result};
use blake3::Hasher;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_64;

/// Unique identifier for a graph node
pub type NodeId = String;

/// Unique identifier for a graph edge
pub type EdgeId = String;

/// Shard identifier
pub type ShardId = u32;

/// Graph sharding strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardStrategy {
    /// Hash-based partitioning using consistent hashing
    Hash,
    /// Range-based partitioning for ordered node IDs
    Range,
    /// Edge-cut minimization for graph partitioning
    EdgeCut,
    /// Custom partitioning strategy
    Custom,
}

/// Metadata about a graph shard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMetadata {
    /// Shard identifier
    pub shard_id: ShardId,
    /// Number of nodes in this shard
    pub node_count: usize,
    /// Number of edges in this shard
    pub edge_count: usize,
    /// Number of edges crossing to other shards
    pub cross_shard_edges: usize,
    /// Primary node responsible for this shard
    pub primary_node: String,
    /// Replica nodes
    pub replicas: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp
    pub modified_at: DateTime<Utc>,
    /// Partitioning strategy used
    pub strategy: ShardStrategy,
}

impl ShardMetadata {
    /// Create new shard metadata
    pub fn new(shard_id: ShardId, primary_node: String, strategy: ShardStrategy) -> Self {
        Self {
            shard_id,
            node_count: 0,
            edge_count: 0,
            cross_shard_edges: 0,
            primary_node,
            replicas: Vec::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            strategy,
        }
    }

    /// Calculate edge cut ratio (cross-shard edges / total edges)
    pub fn edge_cut_ratio(&self) -> f64 {
        if self.edge_count == 0 {
            0.0
        } else {
            self.cross_shard_edges as f64 / self.edge_count as f64
        }
    }
}

/// Hash-based node partitioner
pub struct HashPartitioner {
    /// Total number of shards
    shard_count: u32,
    /// Virtual nodes per physical shard for better distribution
    virtual_nodes: u32,
}

impl HashPartitioner {
    /// Create a new hash partitioner
    pub fn new(shard_count: u32) -> Self {
        assert!(shard_count > 0, "shard_count must be greater than zero");
        Self {
            shard_count,
            virtual_nodes: 150, // Similar to consistent hashing best practices
        }
    }

    /// Get the shard ID for a given node ID using xxHash
    pub fn get_shard(&self, node_id: &NodeId) -> ShardId {
        let hash = xxh3_64(node_id.as_bytes());
        (hash % self.shard_count as u64) as ShardId
    }

    /// Get the shard ID using BLAKE3 for cryptographic strength (alternative)
    pub fn get_shard_secure(&self, node_id: &NodeId) -> ShardId {
        let mut hasher = Hasher::new();
        hasher.update(node_id.as_bytes());
        let hash = hasher.finalize();
        let hash_bytes = hash.as_bytes();
        let hash_u64 = u64::from_le_bytes([
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
            hash_bytes[4],
            hash_bytes[5],
            hash_bytes[6],
            hash_bytes[7],
        ]);
        (hash_u64 % self.shard_count as u64) as ShardId
    }

    /// Get multiple candidate shards for replication
    pub fn get_replica_shards(&self, node_id: &NodeId, replica_count: usize) -> Vec<ShardId> {
        let mut shards = Vec::with_capacity(replica_count);
        let primary = self.get_shard(node_id);
        shards.push(primary);

        // Generate additional shards using salted hashing
        for i in 1..replica_count {
            let salted_id = format!("{}-replica-{}", node_id, i);
            let shard = self.get_shard(&salted_id);
            if !shards.contains(&shard) {
                shards.push(shard);
            }
        }

        shards
    }
}

/// Range-based node partitioner for ordered node IDs
pub struct RangePartitioner {
    /// Total number of shards
    shard_count: u32,
    /// Range boundaries (shard_id -> max_value in range)
    ranges: Vec<String>,
}

impl RangePartitioner {
    /// Create a new range partitioner with automatic range distribution
    pub fn new(shard_count: u32) -> Self {
        Self {
            shard_count,
            ranges: Vec::new(),
        }
    }

    /// Create a range partitioner with explicit boundaries
    pub fn with_boundaries(boundaries: Vec<String>) -> Self {
        Self {
            shard_count: boundaries.len() as u32,
            ranges: boundaries,
        }
    }

    /// Get the shard ID for a node based on range boundaries
    pub fn get_shard(&self, node_id: &NodeId) -> ShardId {
        if self.ranges.is_empty() {
            // Fallback to simple modulo if no ranges defined
            let hash = xxh3_64(node_id.as_bytes());
            return (hash % self.shard_count as u64) as ShardId;
        }

        // Binary search through sorted ranges
        for (idx, boundary) in self.ranges.iter().enumerate() {
            if node_id <= boundary {
                return idx as ShardId;
            }
        }

        // Last shard for values beyond all boundaries
        (self.shard_count - 1) as ShardId
    }

    /// Update range boundaries based on data distribution
    pub fn update_boundaries(&mut self, new_boundaries: Vec<String>) {
        info!(
            "Updating range boundaries: old={}, new={}",
            self.ranges.len(),
            new_boundaries.len()
        );
        self.ranges = new_boundaries;
        self.shard_count = self.ranges.len() as u32;
    }
}

/// Edge-cut minimization using METIS-like graph partitioning
pub struct EdgeCutMinimizer {
    /// Total number of shards
    shard_count: u32,
    /// Node to shard assignments
    node_assignments: Arc<DashMap<NodeId, ShardId>>,
    /// Edge information for partitioning decisions
    edge_weights: Arc<DashMap<(NodeId, NodeId), f64>>,
    /// Adjacency list representation
    adjacency: Arc<DashMap<NodeId, HashSet<NodeId>>>,
}

impl EdgeCutMinimizer {
    /// Create a new edge-cut minimizer
    pub fn new(shard_count: u32) -> Self {
        Self {
            shard_count,
            node_assignments: Arc::new(DashMap::new()),
            edge_weights: Arc::new(DashMap::new()),
            adjacency: Arc::new(DashMap::new()),
        }
    }

    /// Add an edge to the graph for partitioning consideration
    pub fn add_edge(&self, from: NodeId, to: NodeId, weight: f64) {
        self.edge_weights.insert((from.clone(), to.clone()), weight);

        // Update adjacency list
        self.adjacency
            .entry(from.clone())
            .or_insert_with(HashSet::new)
            .insert(to.clone());

        self.adjacency
            .entry(to)
            .or_insert_with(HashSet::new)
            .insert(from);
    }

    /// Get the shard assignment for a node
    pub fn get_shard(&self, node_id: &NodeId) -> Option<ShardId> {
        self.node_assignments.get(node_id).map(|r| *r.value())
    }

    /// Compute initial partitioning using multilevel k-way partitioning
    pub fn compute_partitioning(&self) -> Result<HashMap<NodeId, ShardId>> {
        info!("Computing edge-cut minimized partitioning");

        let nodes: Vec<_> = self.adjacency.iter().map(|e| e.key().clone()).collect();

        if nodes.is_empty() {
            return Ok(HashMap::new());
        }

        // Phase 1: Coarsening - merge highly connected nodes
        let coarse_graph = self.coarsen_graph(&nodes);

        // Phase 2: Initial partitioning using greedy approach
        let mut assignments = self.initial_partition(&coarse_graph);

        // Phase 3: Refinement using Kernighan-Lin algorithm
        self.refine_partition(&mut assignments);

        // Store assignments
        for (node, shard) in &assignments {
            self.node_assignments.insert(node.clone(), *shard);
        }

        info!(
            "Partitioning complete: {} nodes across {} shards",
            assignments.len(),
            self.shard_count
        );

        Ok(assignments)
    }

    /// Coarsen the graph by merging highly connected nodes
    fn coarsen_graph(&self, nodes: &[NodeId]) -> HashMap<NodeId, Vec<NodeId>> {
        let mut coarse: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut visited = HashSet::new();

        for node in nodes {
            if visited.contains(node) {
                continue;
            }

            let mut group = vec![node.clone()];
            visited.insert(node.clone());

            // Find best matching neighbor based on edge weight
            if let Some(neighbors) = self.adjacency.get(node) {
                let mut best_neighbor: Option<(NodeId, f64)> = None;

                for neighbor in neighbors.iter() {
                    if visited.contains(neighbor) {
                        continue;
                    }

                    let weight = self
                        .edge_weights
                        .get(&(node.clone(), neighbor.clone()))
                        .map(|w| *w.value())
                        .unwrap_or(1.0);

                    if let Some((_, best_weight)) = best_neighbor {
                        if weight > best_weight {
                            best_neighbor = Some((neighbor.clone(), weight));
                        }
                    } else {
                        best_neighbor = Some((neighbor.clone(), weight));
                    }
                }

                if let Some((neighbor, _)) = best_neighbor {
                    group.push(neighbor.clone());
                    visited.insert(neighbor);
                }
            }

            let representative = node.clone();
            coarse.insert(representative, group);
        }

        coarse
    }

    /// Initial partition using greedy approach
    fn initial_partition(
        &self,
        coarse_graph: &HashMap<NodeId, Vec<NodeId>>,
    ) -> HashMap<NodeId, ShardId> {
        let mut assignments = HashMap::new();
        let mut shard_sizes: Vec<usize> = vec![0; self.shard_count as usize];

        for (representative, group) in coarse_graph {
            // Assign to least-loaded shard
            let shard = shard_sizes
                .iter()
                .enumerate()
                .min_by_key(|(_, size)| *size)
                .map(|(idx, _)| idx as ShardId)
                .unwrap_or(0);

            for node in group {
                assignments.insert(node.clone(), shard);
                shard_sizes[shard as usize] += 1;
            }
        }

        assignments
    }

    /// Refine partition using simplified Kernighan-Lin algorithm
    fn refine_partition(&self, assignments: &mut HashMap<NodeId, ShardId>) {
        const MAX_ITERATIONS: usize = 10;
        let mut improved = true;
        let mut iteration = 0;

        while improved && iteration < MAX_ITERATIONS {
            improved = false;
            iteration += 1;

            for (node, current_shard) in assignments.clone().iter() {
                let current_cost = self.compute_node_cost(node, *current_shard, assignments);

                // Try moving to each other shard
                for target_shard in 0..self.shard_count {
                    if target_shard == *current_shard {
                        continue;
                    }

                    let new_cost = self.compute_node_cost(node, target_shard, assignments);

                    if new_cost < current_cost {
                        assignments.insert(node.clone(), target_shard);
                        improved = true;
                        break;
                    }
                }
            }

            debug!("Refinement iteration {}: improved={}", iteration, improved);
        }
    }

    /// Compute the cost (number of cross-shard edges) for a node in a given shard
    fn compute_node_cost(
        &self,
        node: &NodeId,
        shard: ShardId,
        assignments: &HashMap<NodeId, ShardId>,
    ) -> usize {
        let mut cross_shard_edges = 0;

        if let Some(neighbors) = self.adjacency.get(node) {
            for neighbor in neighbors.iter() {
                if let Some(neighbor_shard) = assignments.get(neighbor) {
                    if *neighbor_shard != shard {
                        cross_shard_edges += 1;
                    }
                }
            }
        }

        cross_shard_edges
    }

    /// Calculate total edge cut across all shards
    pub fn calculate_edge_cut(&self, assignments: &HashMap<NodeId, ShardId>) -> usize {
        let mut cut = 0;

        for entry in self.edge_weights.iter() {
            let ((from, to), _) = entry.pair();
            let from_shard = assignments.get(from);
            let to_shard = assignments.get(to);

            if from_shard.is_some() && to_shard.is_some() && from_shard != to_shard {
                cut += 1;
            }
        }

        cut
    }
}

/// Graph shard containing partitioned data
pub struct GraphShard {
    /// Shard metadata
    metadata: ShardMetadata,
    /// Nodes in this shard
    nodes: Arc<DashMap<NodeId, NodeData>>,
    /// Edges in this shard (including cross-shard edges)
    edges: Arc<DashMap<EdgeId, EdgeData>>,
    /// Partitioning strategy
    strategy: ShardStrategy,
}

/// Node data in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    pub id: NodeId,
    pub properties: HashMap<String, serde_json::Value>,
    pub labels: Vec<String>,
}

/// Edge data in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeData {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

impl GraphShard {
    /// Create a new graph shard
    pub fn new(metadata: ShardMetadata) -> Self {
        let strategy = metadata.strategy;
        Self {
            metadata,
            nodes: Arc::new(DashMap::new()),
            edges: Arc::new(DashMap::new()),
            strategy,
        }
    }

    /// Add a node to this shard
    pub fn add_node(&self, node: NodeData) -> Result<()> {
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Add an edge to this shard
    pub fn add_edge(&self, edge: EdgeData) -> Result<()> {
        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &NodeId) -> Option<NodeData> {
        self.nodes.get(node_id).map(|n| n.value().clone())
    }

    /// Get an edge by ID
    pub fn get_edge(&self, edge_id: &EdgeId) -> Option<EdgeData> {
        self.edges.get(edge_id).map(|e| e.value().clone())
    }

    /// Get shard metadata
    pub fn metadata(&self) -> &ShardMetadata {
        &self.metadata
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// List all nodes in this shard
    pub fn list_nodes(&self) -> Vec<NodeData> {
        self.nodes.iter().map(|e| e.value().clone()).collect()
    }

    /// List all edges in this shard
    pub fn list_edges(&self) -> Vec<EdgeData> {
        self.edges.iter().map(|e| e.value().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_partitioner() {
        let partitioner = HashPartitioner::new(16);

        let node1 = "node-1".to_string();
        let node2 = "node-2".to_string();

        let shard1 = partitioner.get_shard(&node1);
        let shard2 = partitioner.get_shard(&node2);

        assert!(shard1 < 16);
        assert!(shard2 < 16);

        // Same node should always map to same shard
        assert_eq!(shard1, partitioner.get_shard(&node1));
    }

    #[test]
    fn test_range_partitioner() {
        let boundaries = vec!["m".to_string(), "z".to_string()];
        let partitioner = RangePartitioner::with_boundaries(boundaries);

        assert_eq!(partitioner.get_shard(&"apple".to_string()), 0);
        assert_eq!(partitioner.get_shard(&"orange".to_string()), 1);
        assert_eq!(partitioner.get_shard(&"zebra".to_string()), 1);
    }

    #[test]
    fn test_edge_cut_minimizer() {
        let minimizer = EdgeCutMinimizer::new(2);

        // Create a simple graph: A-B-C-D
        minimizer.add_edge("A".to_string(), "B".to_string(), 1.0);
        minimizer.add_edge("B".to_string(), "C".to_string(), 1.0);
        minimizer.add_edge("C".to_string(), "D".to_string(), 1.0);

        let assignments = minimizer.compute_partitioning().unwrap();
        let cut = minimizer.calculate_edge_cut(&assignments);

        // Optimal partitioning should minimize edge cuts
        assert!(cut <= 2);
    }

    #[test]
    fn test_shard_metadata() {
        let metadata = ShardMetadata::new(0, "node-1".to_string(), ShardStrategy::Hash);

        assert_eq!(metadata.shard_id, 0);
        assert_eq!(metadata.edge_cut_ratio(), 0.0);
    }

    #[test]
    fn test_graph_shard() {
        let metadata = ShardMetadata::new(0, "node-1".to_string(), ShardStrategy::Hash);
        let shard = GraphShard::new(metadata);

        let node = NodeData {
            id: "test-node".to_string(),
            properties: HashMap::new(),
            labels: vec!["TestLabel".to_string()],
        };

        shard.add_node(node.clone()).unwrap();

        assert_eq!(shard.node_count(), 1);
        assert!(shard.get_node(&"test-node".to_string()).is_some());
    }
}
