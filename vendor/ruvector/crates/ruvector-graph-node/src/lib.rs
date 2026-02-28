//! Node.js bindings for RuVector Graph Database via NAPI-RS
//!
//! High-performance native graph database with Cypher-like query support,
//! hypergraph capabilities, async/await support, and zero-copy buffer sharing.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_core::advanced::hypergraph::{
    CausalMemory as CoreCausalMemory, Hyperedge as CoreHyperedge,
    HypergraphIndex as CoreHypergraphIndex,
};
use ruvector_core::DistanceMetric;
use ruvector_graph::cypher::{parse_cypher, Statement};
use ruvector_graph::node::NodeBuilder;
use ruvector_graph::storage::GraphStorage;
use ruvector_graph::GraphDB;
use std::sync::{Arc, RwLock};

mod streaming;
mod transactions;
mod types;

pub use streaming::*;
pub use transactions::*;
pub use types::*;

/// Graph database for complex relationship queries
#[napi]
pub struct GraphDatabase {
    hypergraph: Arc<RwLock<CoreHypergraphIndex>>,
    causal_memory: Arc<RwLock<CoreCausalMemory>>,
    transaction_manager: Arc<RwLock<transactions::TransactionManager>>,
    /// Property graph database with Cypher support
    graph_db: Arc<RwLock<GraphDB>>,
    /// Persistent storage backend (optional)
    storage: Option<Arc<RwLock<GraphStorage>>>,
    /// Path to storage file (if persisted)
    storage_path: Option<String>,
}

#[napi]
impl GraphDatabase {
    /// Create a new graph database
    ///
    /// # Example
    /// ```javascript
    /// const db = new GraphDatabase({
    ///   distanceMetric: 'Cosine',
    ///   dimensions: 384
    /// });
    /// ```
    #[napi(constructor)]
    pub fn new(options: Option<JsGraphOptions>) -> Result<Self> {
        let opts = options.unwrap_or_default();
        let metric = opts.distance_metric.unwrap_or(JsDistanceMetric::Cosine);
        let core_metric: DistanceMetric = metric.into();

        // Check if storage path is provided for persistence
        let (storage, storage_path) = if let Some(ref path) = opts.storage_path {
            let gs = GraphStorage::new(path)
                .map_err(|e| Error::from_reason(format!("Failed to open storage: {}", e)))?;
            (Some(Arc::new(RwLock::new(gs))), Some(path.clone()))
        } else {
            (None, None)
        };

        Ok(Self {
            hypergraph: Arc::new(RwLock::new(CoreHypergraphIndex::new(core_metric))),
            causal_memory: Arc::new(RwLock::new(CoreCausalMemory::new(core_metric))),
            transaction_manager: Arc::new(RwLock::new(transactions::TransactionManager::new())),
            graph_db: Arc::new(RwLock::new(GraphDB::new())),
            storage,
            storage_path,
        })
    }

    /// Open an existing graph database from disk
    ///
    /// # Example
    /// ```javascript
    /// const db = GraphDatabase.open('./my-graph.db');
    /// ```
    #[napi(factory)]
    pub fn open(path: String) -> Result<Self> {
        let storage = GraphStorage::new(&path)
            .map_err(|e| Error::from_reason(format!("Failed to open storage: {}", e)))?;

        let metric = DistanceMetric::Cosine;

        Ok(Self {
            hypergraph: Arc::new(RwLock::new(CoreHypergraphIndex::new(metric))),
            causal_memory: Arc::new(RwLock::new(CoreCausalMemory::new(metric))),
            transaction_manager: Arc::new(RwLock::new(transactions::TransactionManager::new())),
            graph_db: Arc::new(RwLock::new(GraphDB::new())),
            storage: Some(Arc::new(RwLock::new(storage))),
            storage_path: Some(path),
        })
    }

    /// Check if persistence is enabled
    ///
    /// # Example
    /// ```javascript
    /// if (db.isPersistent()) {
    ///   console.log('Data is being saved to:', db.getStoragePath());
    /// }
    /// ```
    #[napi]
    pub fn is_persistent(&self) -> bool {
        self.storage.is_some()
    }

    /// Get the storage path (if persisted)
    #[napi]
    pub fn get_storage_path(&self) -> Option<String> {
        self.storage_path.clone()
    }

    /// Create a node in the graph
    ///
    /// # Example
    /// ```javascript
    /// const nodeId = await db.createNode({
    ///   id: 'node1',
    ///   embedding: new Float32Array([1, 2, 3]),
    ///   properties: { name: 'Alice', age: 30 }
    /// });
    /// ```
    #[napi]
    pub async fn create_node(&self, node: JsNode) -> Result<String> {
        let hypergraph = self.hypergraph.clone();
        let graph_db = self.graph_db.clone();
        let storage = self.storage.clone();
        let id = node.id.clone();
        let embedding = node.embedding.to_vec();
        let properties = node.properties.clone();
        let labels = node.labels.clone();

        tokio::task::spawn_blocking(move || {
            // Add to hypergraph index
            let mut hg = hypergraph.write().expect("RwLock poisoned");
            hg.add_entity(id.clone(), embedding);

            // Add to property graph
            let mut gdb = graph_db.write().expect("RwLock poisoned");
            let mut builder = NodeBuilder::new().id(&id);

            // Add labels if provided
            if let Some(node_labels) = labels {
                for label in node_labels {
                    builder = builder.label(&label);
                }
            }

            // Add properties if provided
            if let Some(props) = properties {
                for (key, value) in props {
                    builder = builder.property(&key, value);
                }
            }

            let graph_node = builder.build();

            // Persist to storage if enabled
            if let Some(ref storage_arc) = storage {
                let storage_guard = storage_arc.write().expect("Storage RwLock poisoned");
                storage_guard
                    .insert_node(&graph_node)
                    .map_err(|e| Error::from_reason(format!("Failed to persist node: {}", e)))?;
            }

            gdb.create_node(graph_node)
                .map_err(|e| Error::from_reason(format!("Failed to create node: {}", e)))?;

            Ok::<String, Error>(id)
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Create an edge between two nodes
    ///
    /// # Example
    /// ```javascript
    /// const edgeId = await db.createEdge({
    ///   from: 'node1',
    ///   to: 'node2',
    ///   description: 'knows',
    ///   embedding: new Float32Array([0.5, 0.5, 0.5]),
    ///   confidence: 0.95
    /// });
    /// ```
    #[napi]
    pub async fn create_edge(&self, edge: JsEdge) -> Result<String> {
        let hypergraph = self.hypergraph.clone();
        let nodes = vec![edge.from.clone(), edge.to.clone()];
        let description = edge.description.clone();
        let embedding = edge.embedding.to_vec();
        let confidence = edge.confidence.unwrap_or(1.0) as f32;

        tokio::task::spawn_blocking(move || {
            let core_edge = CoreHyperedge::new(nodes, description, embedding, confidence);
            let edge_id = core_edge.id.clone();
            let mut hg = hypergraph.write().expect("RwLock poisoned");
            hg.add_hyperedge(core_edge)
                .map_err(|e| Error::from_reason(format!("Failed to create edge: {}", e)))?;
            Ok(edge_id)
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Create a hyperedge connecting multiple nodes
    ///
    /// # Example
    /// ```javascript
    /// const hyperedgeId = await db.createHyperedge({
    ///   nodes: ['node1', 'node2', 'node3'],
    ///   description: 'collaborated_on_project',
    ///   embedding: new Float32Array([0.3, 0.6, 0.9]),
    ///   confidence: 0.85,
    ///   metadata: { project: 'AI Research' }
    /// });
    /// ```
    #[napi]
    pub async fn create_hyperedge(&self, hyperedge: JsHyperedge) -> Result<String> {
        let hypergraph = self.hypergraph.clone();
        let nodes = hyperedge.nodes.clone();
        let description = hyperedge.description.clone();
        let embedding = hyperedge.embedding.to_vec();
        let confidence = hyperedge.confidence.unwrap_or(1.0) as f32;

        tokio::task::spawn_blocking(move || {
            let core_edge = CoreHyperedge::new(nodes, description, embedding, confidence);
            let edge_id = core_edge.id.clone();
            let mut hg = hypergraph.write().expect("RwLock poisoned");
            hg.add_hyperedge(core_edge)
                .map_err(|e| Error::from_reason(format!("Failed to create hyperedge: {}", e)))?;
            Ok(edge_id)
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Query the graph using Cypher-like syntax
    ///
    /// # Example
    /// ```javascript
    /// const results = await db.query('MATCH (n) RETURN n LIMIT 10');
    /// ```
    #[napi]
    pub async fn query(&self, cypher: String) -> Result<JsQueryResult> {
        let graph_db = self.graph_db.clone();
        let hypergraph = self.hypergraph.clone();

        tokio::task::spawn_blocking(move || {
            // Parse the Cypher query
            let parsed = parse_cypher(&cypher)
                .map_err(|e| Error::from_reason(format!("Cypher parse error: {}", e)))?;

            let gdb = graph_db.read().expect("RwLock poisoned");
            let hg = hypergraph.read().expect("RwLock poisoned");

            let mut result_nodes: Vec<JsNodeResult> = Vec::new();
            let mut result_edges: Vec<JsEdgeResult> = Vec::new();

            // Execute each statement
            for statement in &parsed.statements {
                match statement {
                    Statement::Match(match_clause) => {
                        // Extract label from match patterns for query
                        for pattern in &match_clause.patterns {
                            if let ruvector_graph::cypher::ast::Pattern::Node(node_pattern) =
                                pattern
                            {
                                for label in &node_pattern.labels {
                                    let nodes = gdb.get_nodes_by_label(label);
                                    for node in nodes {
                                        result_nodes.push(JsNodeResult {
                                            id: node.id.clone(),
                                            labels: node
                                                .labels
                                                .iter()
                                                .map(|l| l.name.clone())
                                                .collect(),
                                            properties: node
                                                .properties
                                                .iter()
                                                .map(|(k, v)| (k.clone(), format!("{:?}", v)))
                                                .collect(),
                                        });
                                    }
                                }
                                // If no labels specified, return all nodes (simplified)
                                if node_pattern.labels.is_empty() && node_pattern.variable.is_some()
                                {
                                    // This would need iteration over all nodes - for now just stats
                                }
                            }
                        }
                    }
                    Statement::Create(create_clause) => {
                        // Handle CREATE - but we need mutable access, so skip in query
                    }
                    Statement::Return(_) => {
                        // RETURN is handled implicitly
                    }
                    _ => {}
                }
            }

            let stats = hg.stats();

            Ok::<JsQueryResult, Error>(JsQueryResult {
                nodes: result_nodes,
                edges: result_edges,
                stats: Some(JsGraphStats {
                    total_nodes: stats.total_entities as u32,
                    total_edges: stats.total_hyperedges as u32,
                    avg_degree: stats.avg_entity_degree as f64,
                }),
            })
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Query the graph synchronously
    ///
    /// # Example
    /// ```javascript
    /// const results = db.querySync('MATCH (n) RETURN n LIMIT 10');
    /// ```
    #[napi]
    pub fn query_sync(&self, cypher: String) -> Result<JsQueryResult> {
        let hg = self.hypergraph.read().expect("RwLock poisoned");
        let stats = hg.stats();

        // Simplified query result for now
        Ok(JsQueryResult {
            nodes: vec![],
            edges: vec![],
            stats: Some(JsGraphStats {
                total_nodes: stats.total_entities as u32,
                total_edges: stats.total_hyperedges as u32,
                avg_degree: stats.avg_entity_degree as f64,
            }),
        })
    }

    /// Search for similar hyperedges
    ///
    /// # Example
    /// ```javascript
    /// const results = await db.searchHyperedges({
    ///   embedding: new Float32Array([0.5, 0.5, 0.5]),
    ///   k: 10
    /// });
    /// ```
    #[napi]
    pub async fn search_hyperedges(
        &self,
        query: JsHyperedgeQuery,
    ) -> Result<Vec<JsHyperedgeResult>> {
        let hypergraph = self.hypergraph.clone();
        let embedding = query.embedding.to_vec();
        let k = query.k as usize;

        tokio::task::spawn_blocking(move || {
            let hg = hypergraph.read().expect("RwLock poisoned");
            let results = hg.search_hyperedges(&embedding, k);

            Ok::<Vec<JsHyperedgeResult>, Error>(
                results
                    .into_iter()
                    .map(|(id, score)| JsHyperedgeResult {
                        id,
                        score: f64::from(score),
                    })
                    .collect(),
            )
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Get k-hop neighbors from a starting node
    ///
    /// # Example
    /// ```javascript
    /// const neighbors = await db.kHopNeighbors('node1', 2);
    /// ```
    #[napi]
    pub async fn k_hop_neighbors(&self, start_node: String, k: u32) -> Result<Vec<String>> {
        let hypergraph = self.hypergraph.clone();
        let hops = k as usize;

        tokio::task::spawn_blocking(move || {
            let hg = hypergraph.read().expect("RwLock poisoned");
            let neighbors = hg.k_hop_neighbors(start_node, hops);
            Ok::<Vec<String>, Error>(neighbors.into_iter().collect())
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Begin a new transaction
    ///
    /// # Example
    /// ```javascript
    /// const txId = await db.begin();
    /// ```
    #[napi]
    pub async fn begin(&self) -> Result<String> {
        let tm = self.transaction_manager.clone();

        tokio::task::spawn_blocking(move || {
            let mut manager = tm.write().expect("RwLock poisoned");
            Ok::<String, Error>(manager.begin())
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Commit a transaction
    ///
    /// # Example
    /// ```javascript
    /// await db.commit(txId);
    /// ```
    #[napi]
    pub async fn commit(&self, tx_id: String) -> Result<()> {
        let tm = self.transaction_manager.clone();

        tokio::task::spawn_blocking(move || {
            let mut manager = tm.write().expect("RwLock poisoned");
            manager
                .commit(&tx_id)
                .map_err(|e| Error::from_reason(format!("Failed to commit: {}", e)))
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Rollback a transaction
    ///
    /// # Example
    /// ```javascript
    /// await db.rollback(txId);
    /// ```
    #[napi]
    pub async fn rollback(&self, tx_id: String) -> Result<()> {
        let tm = self.transaction_manager.clone();

        tokio::task::spawn_blocking(move || {
            let mut manager = tm.write().expect("RwLock poisoned");
            manager
                .rollback(&tx_id)
                .map_err(|e| Error::from_reason(format!("Failed to rollback: {}", e)))
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Batch insert nodes and edges
    ///
    /// # Example
    /// ```javascript
    /// await db.batchInsert({
    ///   nodes: [{ id: 'n1', embedding: new Float32Array([1, 2]) }],
    ///   edges: [{ from: 'n1', to: 'n2', description: 'knows' }]
    /// });
    /// ```
    #[napi]
    pub async fn batch_insert(&self, batch: JsBatchInsert) -> Result<JsBatchResult> {
        let hypergraph = self.hypergraph.clone();
        let nodes = batch.nodes;
        let edges = batch.edges;

        tokio::task::spawn_blocking(move || {
            let mut hg = hypergraph.write().expect("RwLock poisoned");
            let mut node_ids = Vec::new();
            let mut edge_ids = Vec::new();

            // Insert nodes
            for node in nodes {
                hg.add_entity(node.id.clone(), node.embedding.to_vec());
                node_ids.push(node.id);
            }

            // Insert edges
            for edge in edges {
                let nodes = vec![edge.from.clone(), edge.to.clone()];
                let embedding = edge.embedding.to_vec();
                let confidence = edge.confidence.unwrap_or(1.0) as f32;
                let core_edge = CoreHyperedge::new(nodes, edge.description, embedding, confidence);
                let edge_id = core_edge.id.clone();
                hg.add_hyperedge(core_edge)
                    .map_err(|e| Error::from_reason(format!("Failed to insert edge: {}", e)))?;
                edge_ids.push(edge_id);
            }

            Ok::<JsBatchResult, Error>(JsBatchResult { node_ids, edge_ids })
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }

    /// Subscribe to graph changes (returns a change stream)
    ///
    /// # Example
    /// ```javascript
    /// const unsubscribe = db.subscribe((change) => {
    ///   console.log('Graph changed:', change);
    /// });
    /// ```
    #[napi]
    pub fn subscribe(&self, callback: JsFunction) -> Result<()> {
        // Placeholder for event emitter pattern
        // In a real implementation, this would set up a change listener
        Ok(())
    }

    /// Get graph statistics
    ///
    /// # Example
    /// ```javascript
    /// const stats = await db.stats();
    /// console.log(`Nodes: ${stats.totalNodes}, Edges: ${stats.totalEdges}`);
    /// ```
    #[napi]
    pub async fn stats(&self) -> Result<JsGraphStats> {
        let hypergraph = self.hypergraph.clone();

        tokio::task::spawn_blocking(move || {
            let hg = hypergraph.read().expect("RwLock poisoned");
            let stats = hg.stats();

            Ok::<JsGraphStats, Error>(JsGraphStats {
                total_nodes: stats.total_entities as u32,
                total_edges: stats.total_hyperedges as u32,
                avg_degree: stats.avg_entity_degree as f64,
            })
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task failed: {}", e)))?
    }
}

/// Get the version of the library
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Test function to verify bindings
#[napi]
pub fn hello() -> String {
    "Hello from RuVector Graph Node.js bindings!".to_string()
}
