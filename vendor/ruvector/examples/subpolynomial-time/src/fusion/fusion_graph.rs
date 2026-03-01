//! FusionGraph: Unified Vector + Graph Layer
//!
//! Implements the core fusion layer that merges vector similarity edges
//! with graph relation edges into a unified weighted graph for minimum-cut analysis.

use std::collections::{HashMap, HashSet};

/// Unique identifier for fusion nodes
pub type NodeId = u64;

/// Origin of an edge in the fusion graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeOrigin {
    /// Edge derived from vector similarity
    Vector,
    /// Edge from explicit graph relation
    Graph,
    /// Edge learned from access patterns
    SelfLearn,
}

/// Type of graph relation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationType {
    /// Hierarchical parent-child relationship
    ParentChild,
    /// Reference or citation
    References,
    /// Co-occurrence in same context
    CoOccurs,
    /// Similarity link
    SimilarTo,
    /// Custom relation type
    Custom(u8),
}

impl RelationType {
    /// Get weight factor for this relation type
    pub fn weight_factor(&self) -> f64 {
        match self {
            RelationType::ParentChild => 1.0,
            RelationType::References => 0.8,
            RelationType::CoOccurs => 0.6,
            RelationType::SimilarTo => 0.9,
            RelationType::Custom(_) => 0.5,
        }
    }
}

/// A node in the fusion graph
#[derive(Debug, Clone)]
pub struct FusionNode {
    /// Unique identifier
    pub id: NodeId,
    /// Dense vector representation
    pub vector: Vec<f32>,
    /// Metadata as key-value pairs
    pub meta: HashMap<String, String>,
    /// Creation timestamp
    pub created_ts: u64,
    /// Whether the node is active
    pub active: bool,
}

impl FusionNode {
    /// Create a new fusion node
    pub fn new(id: NodeId, vector: Vec<f32>) -> Self {
        Self {
            id,
            vector,
            meta: HashMap::new(),
            created_ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            active: true,
        }
    }

    /// Add metadata
    pub fn with_meta(mut self, key: &str, value: &str) -> Self {
        self.meta.insert(key.to_string(), value.to_string());
        self
    }

    /// Compute cosine similarity with another node
    pub fn similarity(&self, other: &FusionNode) -> f64 {
        if self.vector.len() != other.vector.len() {
            return 0.0;
        }

        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..self.vector.len() {
            dot += f64::from(self.vector[i]) * f64::from(other.vector[i]);
            norm_a += f64::from(self.vector[i]) * f64::from(self.vector[i]);
            norm_b += f64::from(other.vector[i]) * f64::from(other.vector[i]);
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

/// An edge in the fusion graph
#[derive(Debug, Clone)]
pub struct FusionEdge {
    /// Source node
    pub src: NodeId,
    /// Destination node
    pub dst: NodeId,
    /// Origin of this edge
    pub origin: EdgeOrigin,
    /// Relation type (for Graph origin)
    pub relation_type: Option<RelationType>,
    /// Raw strength before fusion
    pub raw_strength: f64,
    /// Computed capacity after fusion
    pub capacity: f64,
}

impl FusionEdge {
    /// Create a vector similarity edge
    pub fn from_vector(src: NodeId, dst: NodeId, similarity: f64) -> Self {
        Self {
            src,
            dst,
            origin: EdgeOrigin::Vector,
            relation_type: None,
            raw_strength: similarity,
            capacity: similarity, // Will be recomputed
        }
    }

    /// Create a graph relation edge
    pub fn from_graph(src: NodeId, dst: NodeId, rel_type: RelationType, strength: f64) -> Self {
        Self {
            src,
            dst,
            origin: EdgeOrigin::Graph,
            relation_type: Some(rel_type),
            raw_strength: strength,
            capacity: strength, // Will be recomputed
        }
    }

    /// Create a self-learned edge
    pub fn from_learning(src: NodeId, dst: NodeId, strength: f64) -> Self {
        Self {
            src,
            dst,
            origin: EdgeOrigin::SelfLearn,
            relation_type: None,
            raw_strength: strength,
            capacity: strength,
        }
    }

    /// Get edge key for deduplication
    pub fn key(&self) -> (NodeId, NodeId) {
        if self.src < self.dst {
            (self.src, self.dst)
        } else {
            (self.dst, self.src)
        }
    }
}

/// Configuration for fusion graph
#[derive(Debug, Clone)]
pub struct FusionConfig {
    /// Weight for vector similarity edges (w_v)
    pub vector_weight: f64,
    /// Weight for graph relation edges (w_g)
    pub graph_weight: f64,
    /// Weight for self-learned edges
    pub learn_weight: f64,
    /// Minimum similarity threshold for vector edges
    pub similarity_threshold: f64,
    /// Top-k similar nodes to connect
    pub top_k: usize,
    /// Enable automatic brittleness detection
    pub enable_monitoring: bool,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            vector_weight: 0.6,
            graph_weight: 0.4,
            learn_weight: 0.3,
            similarity_threshold: 0.7,
            top_k: 10,
            enable_monitoring: true,
        }
    }
}

/// Result of a fusion query
#[derive(Debug, Clone)]
pub struct FusionResult {
    /// Retrieved node IDs
    pub nodes: Vec<NodeId>,
    /// Minimum cut value for the result subgraph
    pub min_cut: f64,
    /// Partition if cut is low
    pub partition: Option<(Vec<NodeId>, Vec<NodeId>)>,
    /// Brittleness warning
    pub brittleness_warning: Option<String>,
    /// Number of cut edges
    pub num_cut_edges: usize,
}

/// The fusion graph combining vector and graph layers
#[derive(Debug)]
pub struct FusionGraph {
    /// Configuration
    config: FusionConfig,
    /// All nodes by ID
    nodes: HashMap<NodeId, FusionNode>,
    /// All edges
    edges: Vec<FusionEdge>,
    /// Adjacency list (node -> set of neighbor nodes)
    adjacency: HashMap<NodeId, HashSet<NodeId>>,
    /// Edge lookup (normalized key -> edge index)
    edge_index: HashMap<(NodeId, NodeId), usize>,
    /// Next available node ID
    next_id: NodeId,
    /// Current minimum cut estimate
    min_cut_estimate: f64,
    /// Boundary edges in current cut
    boundary_edges: Vec<(NodeId, NodeId)>,
}

impl FusionGraph {
    /// Create a new fusion graph with default config
    pub fn new() -> Self {
        Self::with_config(FusionConfig::default())
    }

    /// Create a fusion graph with custom config
    pub fn with_config(config: FusionConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            edge_index: HashMap::new(),
            next_id: 1,
            min_cut_estimate: f64::INFINITY,
            boundary_edges: Vec::new(),
        }
    }

    /// Get configuration
    pub fn config(&self) -> &FusionConfig {
        &self.config
    }

    /// Number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Current minimum cut estimate
    pub fn min_cut(&self) -> f64 {
        self.min_cut_estimate
    }

    /// Get boundary edges
    pub fn boundary_edges(&self) -> &[(NodeId, NodeId)] {
        &self.boundary_edges
    }

    /// Ingest a new node
    pub fn ingest_node(&mut self, vector: Vec<f32>) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;

        let node = FusionNode::new(id, vector);

        // Find similar nodes and create vector edges
        let similar_nodes = self.find_similar_nodes(&node);

        self.nodes.insert(id, node);
        self.adjacency.insert(id, HashSet::new());

        // Add edges to similar nodes
        for (neighbor_id, similarity) in similar_nodes {
            if similarity >= self.config.similarity_threshold {
                self.add_vector_edge(id, neighbor_id, similarity);
            }
        }

        // Recompute min-cut estimate
        self.update_min_cut_estimate();

        id
    }

    /// Ingest a node with explicit ID
    pub fn ingest_node_with_id(&mut self, id: NodeId, vector: Vec<f32>) {
        let node = FusionNode::new(id, vector);

        // Find similar nodes
        let similar_nodes = self.find_similar_nodes(&node);

        self.nodes.insert(id, node);
        self.adjacency.insert(id, HashSet::new());

        // Add edges to similar nodes
        for (neighbor_id, similarity) in similar_nodes {
            if similarity >= self.config.similarity_threshold {
                self.add_vector_edge(id, neighbor_id, similarity);
            }
        }

        self.next_id = self.next_id.max(id + 1);
        self.update_min_cut_estimate();
    }

    /// Add a graph relation edge
    pub fn add_relation(
        &mut self,
        src: NodeId,
        dst: NodeId,
        rel_type: RelationType,
        strength: f64,
    ) {
        if !self.nodes.contains_key(&src) || !self.nodes.contains_key(&dst) {
            return;
        }

        let edge = FusionEdge::from_graph(src, dst, rel_type, strength);
        self.add_edge_internal(edge);
        self.update_min_cut_estimate();
    }

    /// Add a self-learned edge from access patterns
    pub fn add_learned_edge(&mut self, src: NodeId, dst: NodeId, strength: f64) {
        if !self.nodes.contains_key(&src) || !self.nodes.contains_key(&dst) {
            return;
        }

        let edge = FusionEdge::from_learning(src, dst, strength);
        self.add_edge_internal(edge);
        self.update_min_cut_estimate();
    }

    /// Delete a node and its edges
    pub fn delete_node(&mut self, id: NodeId) -> bool {
        if self.nodes.remove(&id).is_none() {
            return false;
        }

        // Remove all edges involving this node
        self.edges.retain(|e| e.src != id && e.dst != id);

        // Rebuild edge index
        self.edge_index.clear();
        for (i, edge) in self.edges.iter().enumerate() {
            self.edge_index.insert(edge.key(), i);
        }

        // Update adjacency
        self.adjacency.remove(&id);
        for neighbors in self.adjacency.values_mut() {
            neighbors.remove(&id);
        }

        self.update_min_cut_estimate();
        true
    }

    /// Query nodes by similarity with brittleness awareness
    pub fn query(&self, query_vector: &[f32], limit: usize) -> FusionResult {
        let mut scores: Vec<(NodeId, f64)> = Vec::new();

        for node in self.nodes.values() {
            if !node.active {
                continue;
            }

            let sim = self.cosine_similarity(&node.vector, query_vector);
            if sim > 0.0 {
                scores.push((node.id, sim));
            }
        }

        // Sort by similarity descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let result_nodes: Vec<NodeId> = scores.iter().take(limit).map(|(id, _)| *id).collect();

        // Compute min-cut for the result subgraph
        let (subgraph_cut, partition) = self.compute_subgraph_cut(&result_nodes);

        let brittleness_warning = if subgraph_cut < 2.0 && result_nodes.len() > 2 {
            Some(format!(
                "Low connectivity (λ={:.2}): results may be fragmented",
                subgraph_cut
            ))
        } else {
            None
        };

        let num_cut_edges = if subgraph_cut < f64::INFINITY {
            subgraph_cut as usize
        } else {
            0
        };

        FusionResult {
            nodes: result_nodes,
            min_cut: subgraph_cut,
            partition,
            brittleness_warning,
            num_cut_edges,
        }
    }

    /// Get all edges for a node
    pub fn get_node_edges(&self, id: NodeId) -> Vec<&FusionEdge> {
        self.edges
            .iter()
            .filter(|e| e.src == id || e.dst == id)
            .collect()
    }

    /// Get node by ID
    pub fn get_node(&self, id: NodeId) -> Option<&FusionNode> {
        self.nodes.get(&id)
    }

    /// Get all edges
    pub fn get_edges(&self) -> &[FusionEdge] {
        &self.edges
    }

    /// Get adjacency for export to mincut
    pub fn to_weighted_edges(&self) -> Vec<(u64, u64, f64)> {
        self.edges
            .iter()
            .map(|e| (e.src, e.dst, e.capacity))
            .collect()
    }

    // Private helper methods

    fn find_similar_nodes(&self, node: &FusionNode) -> Vec<(NodeId, f64)> {
        let mut similarities: Vec<(NodeId, f64)> = self
            .nodes
            .values()
            .filter(|n| n.id != node.id && n.active)
            .map(|n| (n.id, node.similarity(n)))
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(self.config.top_k);
        similarities
    }

    fn add_vector_edge(&mut self, src: NodeId, dst: NodeId, similarity: f64) {
        let edge = FusionEdge::from_vector(src, dst, similarity);
        self.add_edge_internal(edge);
    }

    fn add_edge_internal(&mut self, mut edge: FusionEdge) {
        let key = edge.key();

        // Check for existing edge
        if let Some(&idx) = self.edge_index.get(&key) {
            // Merge: take max capacity
            let new_capacity = self.compute_capacity(&edge);
            edge.capacity = new_capacity;
            if edge.capacity > self.edges[idx].capacity {
                self.edges[idx] = edge;
            }
            return;
        }

        // Compute capacity with fusion weights
        edge.capacity = self.compute_capacity(&edge);

        // Add to adjacency
        self.adjacency
            .entry(edge.src)
            .or_insert_with(HashSet::new)
            .insert(edge.dst);
        self.adjacency
            .entry(edge.dst)
            .or_insert_with(HashSet::new)
            .insert(edge.src);

        // Add edge
        let idx = self.edges.len();
        self.edge_index.insert(key, idx);
        self.edges.push(edge);
    }

    /// Compute edge capacity with fusion formula:
    /// c(u,v) = w_v * f_v(similarity) + w_g * f_g(relation_strength, relation_type)
    fn compute_capacity(&self, edge: &FusionEdge) -> f64 {
        match edge.origin {
            EdgeOrigin::Vector => {
                // f_v(s) = s^2 for similarity (emphasizes high similarity)
                let f_v = edge.raw_strength * edge.raw_strength;
                self.config.vector_weight * f_v
            }
            EdgeOrigin::Graph => {
                // f_g(strength, type) = strength * type_factor
                let type_factor = edge.relation_type.map(|r| r.weight_factor()).unwrap_or(1.0);
                let f_g = edge.raw_strength * type_factor;
                self.config.graph_weight * f_g
            }
            EdgeOrigin::SelfLearn => {
                // Learned edges use learn weight
                self.config.learn_weight * edge.raw_strength
            }
        }
    }

    fn update_min_cut_estimate(&mut self) {
        if self.nodes.len() < 2 {
            self.min_cut_estimate = f64::INFINITY;
            self.boundary_edges.clear();
            return;
        }

        // Simple min-cut approximation via minimum degree
        // Real implementation would use the full mincut algorithm
        let mut min_degree = f64::INFINITY;
        let mut min_node = None;

        for (&node_id, neighbors) in &self.adjacency {
            let degree: f64 = neighbors
                .iter()
                .filter_map(|&n| {
                    let key = if node_id < n {
                        (node_id, n)
                    } else {
                        (n, node_id)
                    };
                    self.edge_index.get(&key).map(|&i| self.edges[i].capacity)
                })
                .sum();

            if degree < min_degree && degree > 0.0 {
                min_degree = degree;
                min_node = Some(node_id);
            }
        }

        self.min_cut_estimate = min_degree;

        // Update boundary edges (edges incident to minimum degree node)
        self.boundary_edges.clear();
        if let Some(node_id) = min_node {
            if let Some(neighbors) = self.adjacency.get(&node_id) {
                for &n in neighbors {
                    self.boundary_edges.push((node_id, n));
                }
            }
        }
    }

    fn compute_subgraph_cut(&self, nodes: &[NodeId]) -> (f64, Option<(Vec<NodeId>, Vec<NodeId>)>) {
        if nodes.len() < 2 {
            return (f64::INFINITY, None);
        }

        let node_set: HashSet<_> = nodes.iter().copied().collect();

        // Compute induced subgraph edges
        let mut subgraph_adj: HashMap<NodeId, Vec<(NodeId, f64)>> = HashMap::new();

        for &node in nodes {
            subgraph_adj.insert(node, Vec::new());
        }

        for edge in &self.edges {
            if node_set.contains(&edge.src) && node_set.contains(&edge.dst) {
                subgraph_adj
                    .entry(edge.src)
                    .or_default()
                    .push((edge.dst, edge.capacity));
                subgraph_adj
                    .entry(edge.dst)
                    .or_default()
                    .push((edge.src, edge.capacity));
            }
        }

        // Find minimum degree (approximation)
        let mut min_cut = f64::INFINITY;
        let mut min_node = None;

        for (&node, neighbors) in &subgraph_adj {
            let degree: f64 = neighbors.iter().map(|(_, w)| w).sum();
            if degree < min_cut && degree > 0.0 {
                min_cut = degree;
                min_node = Some(node);
            }
        }

        // Generate partition
        let partition = min_node.map(|n| {
            let s = vec![n];
            let t: Vec<_> = nodes.iter().copied().filter(|&x| x != n).collect();
            (s, t)
        });

        (min_cut, partition)
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() {
            return 0.0;
        }

        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..a.len() {
            dot += f64::from(a[i]) * f64::from(b[i]);
            norm_a += f64::from(a[i]) * f64::from(a[i]);
            norm_b += f64::from(b[i]) * f64::from(b[i]);
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

impl Default for FusionGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fusion_graph_creation() {
        let graph = FusionGraph::new();
        assert_eq!(graph.num_nodes(), 0);
        assert_eq!(graph.num_edges(), 0);
    }

    #[test]
    fn test_node_ingestion() {
        let mut graph = FusionGraph::new();
        let id = graph.ingest_node(vec![1.0, 0.0, 0.0]);
        assert_eq!(id, 1);
        assert_eq!(graph.num_nodes(), 1);
    }

    #[test]
    fn test_similarity_edges() {
        let mut graph = FusionGraph::with_config(FusionConfig {
            similarity_threshold: 0.5,
            ..Default::default()
        });

        // Similar vectors
        graph.ingest_node_with_id(1, vec![1.0, 0.0, 0.0]);
        graph.ingest_node_with_id(2, vec![0.9, 0.1, 0.0]); // Similar to 1

        assert!(graph.num_edges() > 0);
    }

    #[test]
    fn test_relation_edge() {
        let mut graph = FusionGraph::new();
        graph.ingest_node_with_id(1, vec![1.0, 0.0]);
        graph.ingest_node_with_id(2, vec![0.0, 1.0]);

        graph.add_relation(1, 2, RelationType::References, 0.8);

        assert_eq!(graph.num_edges(), 1);
    }

    #[test]
    fn test_query() {
        let mut graph = FusionGraph::with_config(FusionConfig {
            similarity_threshold: 0.3,
            ..Default::default()
        });

        graph.ingest_node_with_id(1, vec![1.0, 0.0, 0.0]);
        graph.ingest_node_with_id(2, vec![0.9, 0.1, 0.0]);
        graph.ingest_node_with_id(3, vec![0.0, 1.0, 0.0]);

        let result = graph.query(&[1.0, 0.0, 0.0], 2);
        assert!(!result.nodes.is_empty());
        assert!(result.nodes.contains(&1));
    }

    #[test]
    fn test_capacity_computation() {
        let config = FusionConfig {
            vector_weight: 0.6,
            graph_weight: 0.4,
            ..Default::default()
        };
        let graph = FusionGraph::with_config(config);

        let vector_edge = FusionEdge::from_vector(1, 2, 0.9);
        let capacity = graph.compute_capacity(&vector_edge);
        assert!((capacity - 0.486).abs() < 0.01); // 0.6 * 0.9^2 = 0.486

        let graph_edge = FusionEdge::from_graph(1, 2, RelationType::ParentChild, 1.0);
        let capacity = graph.compute_capacity(&graph_edge);
        assert!((capacity - 0.4).abs() < 0.01); // 0.4 * 1.0 * 1.0 = 0.4
    }
}
