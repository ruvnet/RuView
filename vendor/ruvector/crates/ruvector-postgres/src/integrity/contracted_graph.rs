//! Contracted Graph Module for Integrity Control Plane
//!
//! This module implements the contracted operational graph - a fixed-size
//! meta-graph of ~1000 nodes representing partitions, centroids, shards, and
//! dependencies. This is NOT the full similarity graph.
//!
//! The contracted graph enables efficient mincut computation for integrity gating.

use std::collections::HashMap;
use std::fmt;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Node types in the contracted graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Data partition/segment
    Partition,
    /// IVFFlat centroid
    Centroid,
    /// Distributed shard
    Shard,
    /// External dependency (backup, compaction, etc.)
    ExternalDependency,
    /// Hybrid index node
    HybridIndex,
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeType::Partition => write!(f, "partition"),
            NodeType::Centroid => write!(f, "centroid"),
            NodeType::Shard => write!(f, "shard"),
            NodeType::ExternalDependency => write!(f, "external_dependency"),
            NodeType::HybridIndex => write!(f, "hybrid_index"),
        }
    }
}

impl NodeType {
    /// Parse node type from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "partition" => Some(NodeType::Partition),
            "centroid" => Some(NodeType::Centroid),
            "shard" => Some(NodeType::Shard),
            "external_dependency" => Some(NodeType::ExternalDependency),
            "hybrid_index" => Some(NodeType::HybridIndex),
            _ => None,
        }
    }
}

/// Edge types representing data flow between components
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Data flow between partitions
    PartitionLink,
    /// Query routing paths
    RoutingLink,
    /// Operational dependency
    Dependency,
    /// Replication stream
    Replication,
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdgeType::PartitionLink => write!(f, "partition_link"),
            EdgeType::RoutingLink => write!(f, "routing_link"),
            EdgeType::Dependency => write!(f, "dependency"),
            EdgeType::Replication => write!(f, "replication"),
        }
    }
}

impl EdgeType {
    /// Parse edge type from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "partition_link" => Some(EdgeType::PartitionLink),
            "routing_link" => Some(EdgeType::RoutingLink),
            "dependency" => Some(EdgeType::Dependency),
            "replication" => Some(EdgeType::Replication),
            _ => None,
        }
    }
}

/// A node in the contracted graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractedNode {
    /// Collection this node belongs to
    pub collection_id: i32,
    /// Type of the node
    pub node_type: NodeType,
    /// Unique identifier within the type
    pub node_id: i64,
    /// Human-readable name
    pub node_name: Option<String>,
    /// Additional metadata
    pub node_data: serde_json::Value,
    /// Health score (0.0 = failed, 1.0 = healthy)
    pub health_score: f32,
}

impl ContractedNode {
    /// Create a new contracted node
    pub fn new(collection_id: i32, node_type: NodeType, node_id: i64) -> Self {
        Self {
            collection_id,
            node_type,
            node_id,
            node_name: None,
            node_data: serde_json::json!({}),
            health_score: 1.0,
        }
    }

    /// Set the node name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.node_name = Some(name.into());
        self
    }

    /// Set node data
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.node_data = data;
        self
    }

    /// Set health score
    pub fn with_health(mut self, health: f32) -> Self {
        self.health_score = health.clamp(0.0, 1.0);
        self
    }

    /// Get the unique key for this node
    pub fn key(&self) -> (NodeType, i64) {
        (self.node_type, self.node_id)
    }
}

/// An edge in the contracted graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractedEdge {
    /// Collection this edge belongs to
    pub collection_id: i32,
    /// Source node type
    pub source_type: NodeType,
    /// Source node ID
    pub source_id: i64,
    /// Target node type
    pub target_type: NodeType,
    /// Target node ID
    pub target_id: i64,
    /// Type of the edge
    pub edge_type: EdgeType,
    /// Max-flow capacity
    pub capacity: f32,
    /// Current utilization
    pub current_flow: f32,
    /// Edge latency in milliseconds
    pub latency_ms: Option<f32>,
    /// Recent error rate (0.0-1.0)
    pub error_rate: f32,
}

impl ContractedEdge {
    /// Create a new contracted edge
    pub fn new(
        collection_id: i32,
        source_type: NodeType,
        source_id: i64,
        target_type: NodeType,
        target_id: i64,
        edge_type: EdgeType,
    ) -> Self {
        Self {
            collection_id,
            source_type,
            source_id,
            target_type,
            target_id,
            edge_type,
            capacity: 1.0,
            current_flow: 0.0,
            latency_ms: None,
            error_rate: 0.0,
        }
    }

    /// Set capacity
    pub fn with_capacity(mut self, capacity: f32) -> Self {
        self.capacity = capacity.max(0.0);
        self
    }

    /// Set current flow
    pub fn with_flow(mut self, flow: f32) -> Self {
        self.current_flow = flow.max(0.0);
        self
    }

    /// Set latency
    pub fn with_latency(mut self, latency_ms: f32) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Set error rate
    pub fn with_error_rate(mut self, error_rate: f32) -> Self {
        self.error_rate = error_rate.clamp(0.0, 1.0);
        self
    }

    /// Get effective capacity (adjusted for error rate)
    pub fn effective_capacity(&self) -> f64 {
        (self.capacity as f64) * (1.0 - self.error_rate as f64)
    }

    /// Get source key
    pub fn source_key(&self) -> (NodeType, i64) {
        (self.source_type, self.source_id)
    }

    /// Get target key
    pub fn target_key(&self) -> (NodeType, i64) {
        (self.target_type, self.target_id)
    }
}

/// The contracted graph structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractedGraph {
    /// Collection ID this graph belongs to
    pub collection_id: i32,
    /// All nodes in the graph
    pub nodes: Vec<ContractedNode>,
    /// All edges in the graph
    pub edges: Vec<ContractedEdge>,
    /// When the graph was last updated
    pub last_updated: std::time::SystemTime,
}

impl ContractedGraph {
    /// Create a new empty contracted graph
    pub fn new(collection_id: i32) -> Self {
        Self {
            collection_id,
            nodes: Vec::new(),
            edges: Vec::new(),
            last_updated: std::time::SystemTime::now(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: ContractedNode) {
        self.nodes.push(node);
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: ContractedEdge) {
        self.edges.push(edge);
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Build a node index for quick lookups
    pub fn build_node_index(&self) -> HashMap<(NodeType, i64), usize> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.key(), i))
            .collect()
    }

    /// Build adjacency matrix for mincut computation
    pub fn build_capacity_matrix(&self) -> (Vec<Vec<f64>>, HashMap<(NodeType, i64), usize>) {
        let n = self.nodes.len();
        let node_index = self.build_node_index();
        let mut capacity = vec![vec![0.0f64; n]; n];

        for edge in &self.edges {
            if let (Some(&i), Some(&j)) = (
                node_index.get(&edge.source_key()),
                node_index.get(&edge.target_key()),
            ) {
                let cap = edge.effective_capacity();
                capacity[i][j] = cap;
                capacity[j][i] = cap; // Undirected graph
            }
        }

        (capacity, node_index)
    }

    /// Get graph statistics
    pub fn stats(&self) -> ContractedGraphStats {
        let mut node_counts: HashMap<NodeType, usize> = HashMap::new();
        let mut edge_counts: HashMap<EdgeType, usize> = HashMap::new();
        let mut total_health = 0.0f32;
        let mut total_capacity = 0.0f32;
        let mut total_error_rate = 0.0f32;

        for node in &self.nodes {
            *node_counts.entry(node.node_type).or_insert(0) += 1;
            total_health += node.health_score;
        }

        for edge in &self.edges {
            *edge_counts.entry(edge.edge_type).or_insert(0) += 1;
            total_capacity += edge.capacity;
            total_error_rate += edge.error_rate;
        }

        let avg_health = if self.nodes.is_empty() {
            1.0
        } else {
            total_health / self.nodes.len() as f32
        };

        let avg_capacity = if self.edges.is_empty() {
            1.0
        } else {
            total_capacity / self.edges.len() as f32
        };

        let avg_error_rate = if self.edges.is_empty() {
            0.0
        } else {
            total_error_rate / self.edges.len() as f32
        };

        ContractedGraphStats {
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            node_counts,
            edge_counts,
            avg_node_health: avg_health,
            avg_edge_capacity: avg_capacity,
            avg_error_rate,
        }
    }
}

/// Statistics about the contracted graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractedGraphStats {
    /// Total node count
    pub node_count: usize,
    /// Total edge count
    pub edge_count: usize,
    /// Nodes by type
    pub node_counts: HashMap<NodeType, usize>,
    /// Edges by type
    pub edge_counts: HashMap<EdgeType, usize>,
    /// Average node health
    pub avg_node_health: f32,
    /// Average edge capacity
    pub avg_edge_capacity: f32,
    /// Average error rate
    pub avg_error_rate: f32,
}

/// Builder for constructing contracted graphs
pub struct ContractedGraphBuilder {
    collection_id: i32,
    nodes: Vec<ContractedNode>,
    edges: Vec<ContractedEdge>,
}

impl ContractedGraphBuilder {
    /// Create a new builder for a collection
    pub fn new(collection_id: i32) -> Self {
        Self {
            collection_id,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add partition nodes
    pub fn add_partition_nodes(&mut self, count: usize, health_scores: Option<&[f32]>) {
        for i in 0..count {
            let health = health_scores.and_then(|h| h.get(i).copied()).unwrap_or(1.0);

            let node = ContractedNode::new(self.collection_id, NodeType::Partition, i as i64)
                .with_name(format!("partition_{}", i))
                .with_data(serde_json::json!({"index": i}))
                .with_health(health);

            self.nodes.push(node);
        }
    }

    /// Add centroid nodes (for IVFFlat)
    pub fn add_centroid_nodes(&mut self, count: usize, health_scores: Option<&[f32]>) {
        for i in 0..count {
            let health = health_scores.and_then(|h| h.get(i).copied()).unwrap_or(1.0);

            let node = ContractedNode::new(self.collection_id, NodeType::Centroid, i as i64)
                .with_name(format!("centroid_{}", i))
                .with_data(serde_json::json!({"list_id": i}))
                .with_health(health);

            self.nodes.push(node);
        }
    }

    /// Add shard nodes
    pub fn add_shard_nodes(&mut self, count: usize, primary_index: usize) {
        for i in 0..count {
            let is_primary = i == primary_index;
            let node = ContractedNode::new(self.collection_id, NodeType::Shard, i as i64)
                .with_name(if is_primary {
                    format!("primary_shard_{}", i)
                } else {
                    format!("replica_shard_{}", i)
                })
                .with_data(serde_json::json!({
                    "type": if is_primary { "primary" } else { "replica" },
                    "index": i
                }))
                .with_health(1.0);

            self.nodes.push(node);
        }
    }

    /// Add external dependency nodes
    pub fn add_dependency_nodes(&mut self, dependencies: &[(&str, f32)]) {
        for (i, (name, health)) in dependencies.iter().enumerate() {
            let node =
                ContractedNode::new(self.collection_id, NodeType::ExternalDependency, i as i64)
                    .with_name(*name)
                    .with_data(serde_json::json!({"service": name}))
                    .with_health(*health);

            self.nodes.push(node);
        }
    }

    /// Add partition-to-partition edges (data flow)
    pub fn add_partition_links(&mut self) {
        let partition_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Partition)
            .collect();

        for i in 0..partition_nodes.len() {
            for j in (i + 1)..partition_nodes.len() {
                let edge = ContractedEdge::new(
                    self.collection_id,
                    NodeType::Partition,
                    partition_nodes[i].node_id,
                    NodeType::Partition,
                    partition_nodes[j].node_id,
                    EdgeType::PartitionLink,
                )
                .with_capacity(1.0);

                self.edges.push(edge);
            }
        }
    }

    /// Add centroid-to-shard edges (routing)
    pub fn add_routing_links(&mut self) {
        let centroid_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Centroid)
            .collect();

        let shard_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Shard)
            .collect();

        for centroid in &centroid_nodes {
            for shard in &shard_nodes {
                let edge = ContractedEdge::new(
                    self.collection_id,
                    NodeType::Centroid,
                    centroid.node_id,
                    NodeType::Shard,
                    shard.node_id,
                    EdgeType::RoutingLink,
                )
                .with_capacity(centroid.health_score);

                self.edges.push(edge);
            }
        }
    }

    /// Add shard-to-dependency edges
    pub fn add_dependency_links(&mut self) {
        let shard_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Shard)
            .collect();

        let dep_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::ExternalDependency)
            .collect();

        for shard in &shard_nodes {
            for dep in &dep_nodes {
                let edge = ContractedEdge::new(
                    self.collection_id,
                    NodeType::Shard,
                    shard.node_id,
                    NodeType::ExternalDependency,
                    dep.node_id,
                    EdgeType::Dependency,
                )
                .with_capacity(dep.health_score);

                self.edges.push(edge);
            }
        }
    }

    /// Add replication edges between shards
    pub fn add_replication_links(&mut self) {
        let shard_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Shard)
            .collect();

        // Connect primary to replicas
        if shard_nodes.len() > 1 {
            let primary = &shard_nodes[0];
            for replica in shard_nodes.iter().skip(1) {
                let edge = ContractedEdge::new(
                    self.collection_id,
                    NodeType::Shard,
                    primary.node_id,
                    NodeType::Shard,
                    replica.node_id,
                    EdgeType::Replication,
                )
                .with_capacity(1.0);

                self.edges.push(edge);
            }
        }
    }

    /// Build the contracted graph
    pub fn build(self) -> ContractedGraph {
        ContractedGraph {
            collection_id: self.collection_id,
            nodes: self.nodes,
            edges: self.edges,
            last_updated: std::time::SystemTime::now(),
        }
    }

    /// Build a default graph structure
    pub fn build_default(
        collection_id: i32,
        num_partitions: usize,
        num_centroids: usize,
        num_shards: usize,
    ) -> ContractedGraph {
        let mut builder = Self::new(collection_id);

        // Add nodes
        builder.add_partition_nodes(num_partitions.min(100), None);
        builder.add_centroid_nodes(num_centroids.min(500), None);
        builder.add_shard_nodes(num_shards.min(10), 0);
        builder.add_dependency_nodes(&[
            ("backup_service", 1.0),
            ("compaction_service", 1.0),
            ("gnn_trainer", 1.0),
        ]);

        // Add edges
        builder.add_partition_links();
        builder.add_routing_links();
        builder.add_dependency_links();
        builder.add_replication_links();

        builder.build()
    }
}

/// Global registry for contracted graphs
static GRAPH_REGISTRY: once_cell::sync::Lazy<DashMap<i32, ContractedGraph>> =
    once_cell::sync::Lazy::new(DashMap::new);

/// Get or create a contracted graph for a collection
pub fn get_or_create_graph(collection_id: i32) -> ContractedGraph {
    GRAPH_REGISTRY
        .entry(collection_id)
        .or_insert_with(|| {
            // Default: 10 partitions, 100 centroids, 1 shard
            ContractedGraphBuilder::build_default(collection_id, 10, 100, 1)
        })
        .clone()
}

/// Get an existing contracted graph
pub fn get_graph(collection_id: i32) -> Option<ContractedGraph> {
    GRAPH_REGISTRY.get(&collection_id).map(|g| g.clone())
}

/// Store or update a contracted graph
pub fn store_graph(graph: ContractedGraph) {
    GRAPH_REGISTRY.insert(graph.collection_id, graph);
}

/// Remove a contracted graph
pub fn remove_graph(collection_id: i32) -> Option<ContractedGraph> {
    GRAPH_REGISTRY.remove(&collection_id).map(|(_, g)| g)
}

/// List all collection IDs with contracted graphs
pub fn list_graph_collections() -> Vec<i32> {
    GRAPH_REGISTRY.iter().map(|e| *e.key()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contracted_node_creation() {
        let node = ContractedNode::new(1, NodeType::Partition, 42)
            .with_name("partition_42")
            .with_data(serde_json::json!({"size": 1000}))
            .with_health(0.95);

        assert_eq!(node.collection_id, 1);
        assert_eq!(node.node_type, NodeType::Partition);
        assert_eq!(node.node_id, 42);
        assert_eq!(node.node_name, Some("partition_42".to_string()));
        assert!((node.health_score - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_contracted_edge_creation() {
        let edge = ContractedEdge::new(
            1,
            NodeType::Partition,
            1,
            NodeType::Partition,
            2,
            EdgeType::PartitionLink,
        )
        .with_capacity(2.0)
        .with_error_rate(0.1);

        assert_eq!(edge.capacity, 2.0);
        assert!((edge.effective_capacity() - 1.8).abs() < 0.001);
    }

    #[test]
    fn test_graph_builder() {
        let graph = ContractedGraphBuilder::build_default(1, 5, 10, 2);

        assert_eq!(graph.collection_id, 1);
        assert!(graph.node_count() > 0);
        assert!(graph.edge_count() > 0);

        let stats = graph.stats();
        assert!(stats.node_counts.contains_key(&NodeType::Partition));
        assert!(stats.node_counts.contains_key(&NodeType::Centroid));
        assert!(stats.node_counts.contains_key(&NodeType::Shard));
    }

    #[test]
    fn test_capacity_matrix() {
        let graph = ContractedGraphBuilder::build_default(1, 3, 0, 1);
        let (matrix, index) = graph.build_capacity_matrix();

        assert_eq!(matrix.len(), graph.node_count());
        assert_eq!(index.len(), graph.node_count());
    }

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::Partition.to_string(), "partition");
        assert_eq!(NodeType::Centroid.to_string(), "centroid");
        assert_eq!(
            NodeType::ExternalDependency.to_string(),
            "external_dependency"
        );
    }

    #[test]
    fn test_edge_type_parsing() {
        assert_eq!(
            EdgeType::from_str("partition_link"),
            Some(EdgeType::PartitionLink)
        );
        assert_eq!(
            EdgeType::from_str("routing_link"),
            Some(EdgeType::RoutingLink)
        );
        assert_eq!(EdgeType::from_str("invalid"), None);
    }

    #[test]
    fn test_graph_registry() {
        let graph = ContractedGraphBuilder::build_default(999, 2, 2, 1);
        store_graph(graph.clone());

        let retrieved = get_graph(999);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().collection_id, 999);

        remove_graph(999);
        assert!(get_graph(999).is_none());
    }
}
