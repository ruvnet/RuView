//! Query API for RuVector GNN
//!
//! Provides high-level query interfaces for vector search, neural search,
//! and subgraph extraction.

use serde::{Deserialize, Serialize};

/// Query mode for different search strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryMode {
    /// Pure HNSW vector search
    VectorSearch,
    /// GNN-enhanced neural search
    NeuralSearch,
    /// Extract k-hop subgraph around results
    SubgraphExtraction,
    /// Differentiable search with soft attention
    DifferentiableSearch,
}

/// Query configuration for RuVector searches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvectorQuery {
    /// Query vector for similarity search
    pub vector: Option<Vec<f32>>,
    /// Text query (requires embedding model)
    pub text: Option<String>,
    /// Node ID for subgraph extraction
    pub node_id: Option<u64>,
    /// Search mode
    pub mode: QueryMode,
    /// Number of results to return
    pub k: usize,
    /// HNSW search parameter (exploration factor)
    pub ef: usize,
    /// GNN depth for neural search
    pub gnn_depth: usize,
    /// Temperature for differentiable search (higher = softer)
    pub temperature: f32,
    /// Whether to return attention weights
    pub return_attention: bool,
}

impl Default for RuvectorQuery {
    fn default() -> Self {
        Self {
            vector: None,
            text: None,
            node_id: None,
            mode: QueryMode::VectorSearch,
            k: 10,
            ef: 50,
            gnn_depth: 2,
            temperature: 1.0,
            return_attention: false,
        }
    }
}

impl RuvectorQuery {
    /// Create a basic vector search query
    ///
    /// # Arguments
    /// * `vector` - Query vector
    /// * `k` - Number of results to return
    ///
    /// # Example
    /// ```
    /// use ruvector_gnn::query::RuvectorQuery;
    ///
    /// let query = RuvectorQuery::vector_search(vec![0.1, 0.2, 0.3], 10);
    /// assert_eq!(query.k, 10);
    /// ```
    pub fn vector_search(vector: Vec<f32>, k: usize) -> Self {
        Self {
            vector: Some(vector),
            mode: QueryMode::VectorSearch,
            k,
            ..Default::default()
        }
    }

    /// Create a GNN-enhanced neural search query
    ///
    /// # Arguments
    /// * `vector` - Query vector
    /// * `k` - Number of results to return
    /// * `gnn_depth` - Number of GNN layers to apply
    ///
    /// # Example
    /// ```
    /// use ruvector_gnn::query::RuvectorQuery;
    ///
    /// let query = RuvectorQuery::neural_search(vec![0.1, 0.2, 0.3], 10, 3);
    /// assert_eq!(query.gnn_depth, 3);
    /// ```
    pub fn neural_search(vector: Vec<f32>, k: usize, gnn_depth: usize) -> Self {
        Self {
            vector: Some(vector),
            mode: QueryMode::NeuralSearch,
            k,
            gnn_depth,
            ..Default::default()
        }
    }

    /// Create a subgraph extraction query
    ///
    /// # Arguments
    /// * `vector` - Query vector
    /// * `k` - Number of nodes in subgraph
    ///
    /// # Example
    /// ```
    /// use ruvector_gnn::query::RuvectorQuery;
    ///
    /// let query = RuvectorQuery::subgraph_search(vec![0.1, 0.2, 0.3], 20);
    /// assert_eq!(query.k, 20);
    /// ```
    pub fn subgraph_search(vector: Vec<f32>, k: usize) -> Self {
        Self {
            vector: Some(vector),
            mode: QueryMode::SubgraphExtraction,
            k,
            ..Default::default()
        }
    }

    /// Create a differentiable search query with temperature
    ///
    /// # Arguments
    /// * `vector` - Query vector
    /// * `k` - Number of results
    /// * `temperature` - Softmax temperature (higher = softer distribution)
    pub fn differentiable_search(vector: Vec<f32>, k: usize, temperature: f32) -> Self {
        Self {
            vector: Some(vector),
            mode: QueryMode::DifferentiableSearch,
            k,
            temperature,
            return_attention: true,
            ..Default::default()
        }
    }

    /// Set text query (requires embedding model)
    pub fn with_text(mut self, text: String) -> Self {
        self.text = Some(text);
        self
    }

    /// Set node ID for centered queries
    pub fn with_node(mut self, node_id: u64) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Set EF parameter for HNSW search
    pub fn with_ef(mut self, ef: usize) -> Self {
        self.ef = ef;
        self
    }

    /// Enable attention weight return
    pub fn with_attention(mut self) -> Self {
        self.return_attention = true;
        self
    }
}

/// Subgraph representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubGraph {
    /// Node IDs in the subgraph
    pub nodes: Vec<u64>,
    /// Edges as (from, to, weight) tuples
    pub edges: Vec<(u64, u64, f32)>,
}

impl SubGraph {
    /// Create a new empty subgraph
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Create subgraph with nodes and edges
    pub fn with_edges(nodes: Vec<u64>, edges: Vec<(u64, u64, f32)>) -> Self {
        Self { nodes, edges }
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Check if subgraph contains a node
    pub fn contains_node(&self, node_id: u64) -> bool {
        self.nodes.contains(&node_id)
    }

    /// Get average edge weight
    pub fn average_edge_weight(&self) -> f32 {
        if self.edges.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.edges.iter().map(|(_, _, w)| w).sum();
        sum / self.edges.len() as f32
    }
}

impl Default for SubGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Query result with nodes, scores, and optional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Matched node IDs
    pub nodes: Vec<u64>,
    /// Similarity scores (higher = more similar)
    pub scores: Vec<f32>,
    /// Optional node embeddings after GNN processing
    pub embeddings: Option<Vec<Vec<f32>>>,
    /// Optional attention weights from differentiable search
    pub attention_weights: Option<Vec<Vec<f32>>>,
    /// Optional subgraph extraction
    pub subgraph: Option<SubGraph>,
    /// Query latency in milliseconds
    pub latency_ms: u64,
}

impl QueryResult {
    /// Create a new empty query result
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            scores: Vec::new(),
            embeddings: None,
            attention_weights: None,
            subgraph: None,
            latency_ms: 0,
        }
    }

    /// Create query result with nodes and scores
    ///
    /// # Arguments
    /// * `nodes` - Node IDs
    /// * `scores` - Similarity scores
    ///
    /// # Example
    /// ```
    /// use ruvector_gnn::query::QueryResult;
    ///
    /// let result = QueryResult::with_nodes(vec![1, 2, 3], vec![0.9, 0.8, 0.7]);
    /// assert_eq!(result.nodes.len(), 3);
    /// ```
    pub fn with_nodes(nodes: Vec<u64>, scores: Vec<f32>) -> Self {
        Self {
            nodes,
            scores,
            embeddings: None,
            attention_weights: None,
            subgraph: None,
            latency_ms: 0,
        }
    }

    /// Add embeddings to the result
    pub fn with_embeddings(mut self, embeddings: Vec<Vec<f32>>) -> Self {
        self.embeddings = Some(embeddings);
        self
    }

    /// Add attention weights to the result
    pub fn with_attention(mut self, attention: Vec<Vec<f32>>) -> Self {
        self.attention_weights = Some(attention);
        self
    }

    /// Add subgraph to the result
    pub fn with_subgraph(mut self, subgraph: SubGraph) -> Self {
        self.subgraph = Some(subgraph);
        self
    }

    /// Set query latency
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Get number of results
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if result is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get top-k results
    pub fn top_k(&self, k: usize) -> Self {
        let k = k.min(self.nodes.len());
        Self {
            nodes: self.nodes[..k].to_vec(),
            scores: self.scores[..k].to_vec(),
            embeddings: self.embeddings.as_ref().map(|e| e[..k].to_vec()),
            attention_weights: self.attention_weights.as_ref().map(|a| a[..k].to_vec()),
            subgraph: self.subgraph.clone(),
            latency_ms: self.latency_ms,
        }
    }

    /// Get the best result (highest score)
    pub fn best(&self) -> Option<(u64, f32)> {
        if self.nodes.is_empty() {
            None
        } else {
            Some((self.nodes[0], self.scores[0]))
        }
    }

    /// Filter results by minimum score
    pub fn filter_by_score(mut self, min_score: f32) -> Self {
        let mut filtered_nodes = Vec::new();
        let mut filtered_scores = Vec::new();
        let mut filtered_embeddings = Vec::new();
        let mut filtered_attention = Vec::new();

        for i in 0..self.nodes.len() {
            if self.scores[i] >= min_score {
                filtered_nodes.push(self.nodes[i]);
                filtered_scores.push(self.scores[i]);

                if let Some(ref emb) = self.embeddings {
                    filtered_embeddings.push(emb[i].clone());
                }

                if let Some(ref att) = self.attention_weights {
                    filtered_attention.push(att[i].clone());
                }
            }
        }

        self.nodes = filtered_nodes;
        self.scores = filtered_scores;

        if !filtered_embeddings.is_empty() {
            self.embeddings = Some(filtered_embeddings);
        }

        if !filtered_attention.is_empty() {
            self.attention_weights = Some(filtered_attention);
        }

        self
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_mode_serialization() {
        let mode = QueryMode::NeuralSearch;
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: QueryMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn test_ruvector_query_default() {
        let query = RuvectorQuery::default();
        assert_eq!(query.k, 10);
        assert_eq!(query.ef, 50);
        assert_eq!(query.gnn_depth, 2);
        assert_eq!(query.temperature, 1.0);
        assert_eq!(query.mode, QueryMode::VectorSearch);
        assert!(!query.return_attention);
    }

    #[test]
    fn test_vector_search_query() {
        let vector = vec![0.1, 0.2, 0.3, 0.4];
        let query = RuvectorQuery::vector_search(vector.clone(), 5);

        assert_eq!(query.vector, Some(vector));
        assert_eq!(query.k, 5);
        assert_eq!(query.mode, QueryMode::VectorSearch);
    }

    #[test]
    fn test_neural_search_query() {
        let vector = vec![0.1, 0.2, 0.3];
        let query = RuvectorQuery::neural_search(vector.clone(), 10, 3);

        assert_eq!(query.vector, Some(vector));
        assert_eq!(query.k, 10);
        assert_eq!(query.gnn_depth, 3);
        assert_eq!(query.mode, QueryMode::NeuralSearch);
    }

    #[test]
    fn test_subgraph_search_query() {
        let vector = vec![0.5, 0.5];
        let query = RuvectorQuery::subgraph_search(vector.clone(), 20);

        assert_eq!(query.vector, Some(vector));
        assert_eq!(query.k, 20);
        assert_eq!(query.mode, QueryMode::SubgraphExtraction);
    }

    #[test]
    fn test_differentiable_search_query() {
        let vector = vec![0.3, 0.4, 0.5];
        let query = RuvectorQuery::differentiable_search(vector.clone(), 15, 0.5);

        assert_eq!(query.vector, Some(vector));
        assert_eq!(query.k, 15);
        assert_eq!(query.temperature, 0.5);
        assert_eq!(query.mode, QueryMode::DifferentiableSearch);
        assert!(query.return_attention);
    }

    #[test]
    fn test_query_builder_pattern() {
        let query = RuvectorQuery::vector_search(vec![0.1, 0.2], 5)
            .with_text("hello world".to_string())
            .with_node(42)
            .with_ef(100)
            .with_attention();

        assert_eq!(query.text, Some("hello world".to_string()));
        assert_eq!(query.node_id, Some(42));
        assert_eq!(query.ef, 100);
        assert!(query.return_attention);
    }

    #[test]
    fn test_subgraph_new() {
        let subgraph = SubGraph::new();
        assert_eq!(subgraph.node_count(), 0);
        assert_eq!(subgraph.edge_count(), 0);
    }

    #[test]
    fn test_subgraph_with_edges() {
        let nodes = vec![1, 2, 3];
        let edges = vec![(1, 2, 0.8), (2, 3, 0.6), (1, 3, 0.5)];
        let subgraph = SubGraph::with_edges(nodes.clone(), edges.clone());

        assert_eq!(subgraph.nodes, nodes);
        assert_eq!(subgraph.edges, edges);
        assert_eq!(subgraph.node_count(), 3);
        assert_eq!(subgraph.edge_count(), 3);
    }

    #[test]
    fn test_subgraph_contains_node() {
        let nodes = vec![1, 2, 3];
        let subgraph = SubGraph::with_edges(nodes, vec![]);

        assert!(subgraph.contains_node(1));
        assert!(subgraph.contains_node(2));
        assert!(subgraph.contains_node(3));
        assert!(!subgraph.contains_node(4));
    }

    #[test]
    fn test_subgraph_average_edge_weight() {
        let edges = vec![(1, 2, 0.8), (2, 3, 0.6), (1, 3, 0.4)];
        let subgraph = SubGraph::with_edges(vec![1, 2, 3], edges);

        let avg = subgraph.average_edge_weight();
        assert!((avg - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_subgraph_empty_average() {
        let subgraph = SubGraph::new();
        assert_eq!(subgraph.average_edge_weight(), 0.0);
    }

    #[test]
    fn test_query_result_new() {
        let result = QueryResult::new();
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
        assert_eq!(result.latency_ms, 0);
    }

    #[test]
    fn test_query_result_with_nodes() {
        let nodes = vec![1, 2, 3];
        let scores = vec![0.9, 0.8, 0.7];
        let result = QueryResult::with_nodes(nodes.clone(), scores.clone());

        assert_eq!(result.nodes, nodes);
        assert_eq!(result.scores, scores);
        assert_eq!(result.len(), 3);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_query_result_builder_pattern() {
        let embeddings = vec![vec![0.1, 0.2], vec![0.3, 0.4]];
        let attention = vec![vec![0.5, 0.5], vec![0.6, 0.4]];
        let subgraph = SubGraph::with_edges(vec![1, 2], vec![(1, 2, 0.8)]);

        let result = QueryResult::with_nodes(vec![1, 2], vec![0.9, 0.8])
            .with_embeddings(embeddings.clone())
            .with_attention(attention.clone())
            .with_subgraph(subgraph.clone())
            .with_latency(100);

        assert_eq!(result.embeddings, Some(embeddings));
        assert_eq!(result.attention_weights, Some(attention));
        assert_eq!(result.subgraph, Some(subgraph));
        assert_eq!(result.latency_ms, 100);
    }

    #[test]
    fn test_query_result_top_k() {
        let nodes = vec![1, 2, 3, 4, 5];
        let scores = vec![0.9, 0.8, 0.7, 0.6, 0.5];
        let result = QueryResult::with_nodes(nodes, scores);

        let top_3 = result.top_k(3);
        assert_eq!(top_3.len(), 3);
        assert_eq!(top_3.nodes, vec![1, 2, 3]);
        assert_eq!(top_3.scores, vec![0.9, 0.8, 0.7]);
    }

    #[test]
    fn test_query_result_top_k_overflow() {
        let result = QueryResult::with_nodes(vec![1, 2], vec![0.9, 0.8]);
        let top_10 = result.top_k(10);
        assert_eq!(top_10.len(), 2); // Should only return available results
    }

    #[test]
    fn test_query_result_best() {
        let result = QueryResult::with_nodes(vec![1, 2, 3], vec![0.9, 0.8, 0.7]);
        let best = result.best();
        assert_eq!(best, Some((1, 0.9)));
    }

    #[test]
    fn test_query_result_best_empty() {
        let result = QueryResult::new();
        assert_eq!(result.best(), None);
    }

    #[test]
    fn test_query_result_filter_by_score() {
        let nodes = vec![1, 2, 3, 4, 5];
        let scores = vec![0.9, 0.8, 0.7, 0.6, 0.5];
        let result = QueryResult::with_nodes(nodes, scores);

        let filtered = result.filter_by_score(0.7);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered.nodes, vec![1, 2, 3]);
        assert_eq!(filtered.scores, vec![0.9, 0.8, 0.7]);
    }

    #[test]
    fn test_query_result_filter_with_embeddings() {
        let nodes = vec![1, 2, 3];
        let scores = vec![0.9, 0.6, 0.8];
        let embeddings = vec![vec![0.1], vec![0.2], vec![0.3]];

        let result = QueryResult::with_nodes(nodes, scores).with_embeddings(embeddings);

        let filtered = result.filter_by_score(0.7);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.nodes, vec![1, 3]);
        assert_eq!(filtered.embeddings, Some(vec![vec![0.1], vec![0.3]]));
    }

    #[test]
    fn test_query_result_filter_with_attention() {
        let nodes = vec![1, 2, 3];
        let scores = vec![0.9, 0.5, 0.8];
        let attention = vec![vec![0.5, 0.5], vec![0.6, 0.4], vec![0.7, 0.3]];

        let result = QueryResult::with_nodes(nodes, scores).with_attention(attention);

        let filtered = result.filter_by_score(0.75);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.nodes, vec![1, 3]);
        assert_eq!(
            filtered.attention_weights,
            Some(vec![vec![0.5, 0.5], vec![0.7, 0.3]])
        );
    }

    #[test]
    fn test_query_serialization() {
        let query = RuvectorQuery::neural_search(vec![0.1, 0.2], 5, 2);
        let json = serde_json::to_string(&query).unwrap();
        let deserialized: RuvectorQuery = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.k, query.k);
        assert_eq!(deserialized.gnn_depth, query.gnn_depth);
        assert_eq!(deserialized.mode, query.mode);
    }

    #[test]
    fn test_result_serialization() {
        let result = QueryResult::with_nodes(vec![1, 2], vec![0.9, 0.8]).with_latency(50);

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: QueryResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.nodes, result.nodes);
        assert_eq!(deserialized.scores, result.scores);
        assert_eq!(deserialized.latency_ms, result.latency_ms);
    }

    #[test]
    fn test_subgraph_serialization() {
        let subgraph = SubGraph::with_edges(vec![1, 2, 3], vec![(1, 2, 0.8), (2, 3, 0.6)]);

        let json = serde_json::to_string(&subgraph).unwrap();
        let deserialized: SubGraph = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.nodes, subgraph.nodes);
        assert_eq!(deserialized.edges, subgraph.edges);
    }

    #[test]
    fn test_edge_case_empty_filter() {
        let result = QueryResult::with_nodes(vec![1, 2], vec![0.5, 0.4]);
        let filtered = result.filter_by_score(0.9);

        assert!(filtered.is_empty());
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_query_mode_variants() {
        // Test all query mode variants
        assert_eq!(QueryMode::VectorSearch, QueryMode::VectorSearch);
        assert_ne!(QueryMode::VectorSearch, QueryMode::NeuralSearch);
        assert_ne!(QueryMode::NeuralSearch, QueryMode::SubgraphExtraction);
        assert_ne!(
            QueryMode::SubgraphExtraction,
            QueryMode::DifferentiableSearch
        );
    }
}
