//! Query coordinator for distributed graph execution
//!
//! Coordinates distributed query execution across multiple shards:
//! - Query planning and optimization
//! - Query routing to relevant shards
//! - Result aggregation and merging
//! - Transaction coordination across shards
//! - Query caching and optimization

use crate::distributed::shard::{EdgeData, GraphShard, NodeData, NodeId, ShardId};
use crate::{GraphError, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Query execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    /// Unique query ID
    pub query_id: String,
    /// Original query (Cypher-like syntax)
    pub query: String,
    /// Shards involved in this query
    pub target_shards: Vec<ShardId>,
    /// Execution steps
    pub steps: Vec<QueryStep>,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Whether this is a distributed query
    pub is_distributed: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Individual step in query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryStep {
    /// Scan nodes with optional filter
    NodeScan {
        shard_id: ShardId,
        label: Option<String>,
        filter: Option<String>,
    },
    /// Scan edges
    EdgeScan {
        shard_id: ShardId,
        edge_type: Option<String>,
    },
    /// Join results from multiple shards
    Join {
        left_shard: ShardId,
        right_shard: ShardId,
        join_key: String,
    },
    /// Aggregate results
    Aggregate {
        operation: AggregateOp,
        group_by: Option<String>,
    },
    /// Filter results
    Filter { predicate: String },
    /// Sort results
    Sort { key: String, ascending: bool },
    /// Limit results
    Limit { count: usize },
}

/// Aggregate operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateOp {
    Count,
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Query ID
    pub query_id: String,
    /// Result nodes
    pub nodes: Vec<NodeData>,
    /// Result edges
    pub edges: Vec<EdgeData>,
    /// Aggregate results
    pub aggregates: HashMap<String, serde_json::Value>,
    /// Execution statistics
    pub stats: QueryStats,
}

/// Query execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStats {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of shards queried
    pub shards_queried: usize,
    /// Total nodes scanned
    pub nodes_scanned: usize,
    /// Total edges scanned
    pub edges_scanned: usize,
    /// Whether query was cached
    pub cached: bool,
}

/// Shard coordinator for managing distributed queries
pub struct ShardCoordinator {
    /// Map of shard_id to GraphShard
    shards: Arc<DashMap<ShardId, Arc<GraphShard>>>,
    /// Query cache
    query_cache: Arc<DashMap<String, QueryResult>>,
    /// Active transactions
    transactions: Arc<DashMap<String, Transaction>>,
}

impl ShardCoordinator {
    /// Create a new shard coordinator
    pub fn new() -> Self {
        Self {
            shards: Arc::new(DashMap::new()),
            query_cache: Arc::new(DashMap::new()),
            transactions: Arc::new(DashMap::new()),
        }
    }

    /// Register a shard with the coordinator
    pub fn register_shard(&self, shard_id: ShardId, shard: Arc<GraphShard>) {
        info!("Registering shard {} with coordinator", shard_id);
        self.shards.insert(shard_id, shard);
    }

    /// Unregister a shard
    pub fn unregister_shard(&self, shard_id: ShardId) -> Result<()> {
        info!("Unregistering shard {}", shard_id);
        self.shards
            .remove(&shard_id)
            .ok_or_else(|| GraphError::ShardError(format!("Shard {} not found", shard_id)))?;
        Ok(())
    }

    /// Get a shard by ID
    pub fn get_shard(&self, shard_id: ShardId) -> Option<Arc<GraphShard>> {
        self.shards.get(&shard_id).map(|s| Arc::clone(s.value()))
    }

    /// List all registered shards
    pub fn list_shards(&self) -> Vec<ShardId> {
        self.shards.iter().map(|e| *e.key()).collect()
    }

    /// Create a query plan from a Cypher-like query
    pub fn plan_query(&self, query: &str) -> Result<QueryPlan> {
        let query_id = Uuid::new_v4().to_string();

        // Parse query and determine target shards
        // For now, simple heuristic: query all shards for distributed queries
        let target_shards: Vec<ShardId> = self.list_shards();

        let steps = self.parse_query_steps(query)?;

        let estimated_cost = self.estimate_cost(&steps, &target_shards);

        Ok(QueryPlan {
            query_id,
            query: query.to_string(),
            target_shards,
            steps,
            estimated_cost,
            is_distributed: true,
            created_at: Utc::now(),
        })
    }

    /// Parse query into execution steps
    fn parse_query_steps(&self, query: &str) -> Result<Vec<QueryStep>> {
        // Simplified query parsing
        // In production, use a proper Cypher parser
        let mut steps = Vec::new();

        // Example: "MATCH (n:Person) RETURN n"
        if query.to_lowercase().contains("match") {
            // Add node scan for each shard
            for shard_id in self.list_shards() {
                steps.push(QueryStep::NodeScan {
                    shard_id,
                    label: None,
                    filter: None,
                });
            }
        }

        // Add aggregation if needed
        if query.to_lowercase().contains("count") {
            steps.push(QueryStep::Aggregate {
                operation: AggregateOp::Count,
                group_by: None,
            });
        }

        // Add limit if specified
        if let Some(limit_pos) = query.to_lowercase().find("limit") {
            if let Some(count_str) = query[limit_pos..].split_whitespace().nth(1) {
                if let Ok(count) = count_str.parse::<usize>() {
                    steps.push(QueryStep::Limit { count });
                }
            }
        }

        Ok(steps)
    }

    /// Estimate query execution cost
    fn estimate_cost(&self, steps: &[QueryStep], target_shards: &[ShardId]) -> f64 {
        let mut cost = 0.0;

        for step in steps {
            match step {
                QueryStep::NodeScan { .. } => cost += 10.0,
                QueryStep::EdgeScan { .. } => cost += 15.0,
                QueryStep::Join { .. } => cost += 50.0,
                QueryStep::Aggregate { .. } => cost += 20.0,
                QueryStep::Filter { .. } => cost += 5.0,
                QueryStep::Sort { .. } => cost += 30.0,
                QueryStep::Limit { .. } => cost += 1.0,
            }
        }

        // Multiply by number of shards for distributed queries
        cost * target_shards.len() as f64
    }

    /// Execute a query plan
    pub async fn execute_query(&self, plan: QueryPlan) -> Result<QueryResult> {
        let start = std::time::Instant::now();

        info!(
            "Executing query {} across {} shards",
            plan.query_id,
            plan.target_shards.len()
        );

        // Check cache first
        if let Some(cached) = self.query_cache.get(&plan.query) {
            debug!("Query cache hit for: {}", plan.query);
            return Ok(cached.value().clone());
        }

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut aggregates = HashMap::new();
        let mut nodes_scanned = 0;
        let mut edges_scanned = 0;

        // Execute steps
        for step in &plan.steps {
            match step {
                QueryStep::NodeScan {
                    shard_id,
                    label,
                    filter,
                } => {
                    if let Some(shard) = self.get_shard(*shard_id) {
                        let shard_nodes = shard.list_nodes();
                        nodes_scanned += shard_nodes.len();

                        // Apply label filter
                        let filtered: Vec<_> = if let Some(label_filter) = label {
                            shard_nodes
                                .into_iter()
                                .filter(|n| n.labels.contains(label_filter))
                                .collect()
                        } else {
                            shard_nodes
                        };

                        nodes.extend(filtered);
                    }
                }
                QueryStep::EdgeScan {
                    shard_id,
                    edge_type,
                } => {
                    if let Some(shard) = self.get_shard(*shard_id) {
                        let shard_edges = shard.list_edges();
                        edges_scanned += shard_edges.len();

                        // Apply edge type filter
                        let filtered: Vec<_> = if let Some(type_filter) = edge_type {
                            shard_edges
                                .into_iter()
                                .filter(|e| &e.edge_type == type_filter)
                                .collect()
                        } else {
                            shard_edges
                        };

                        edges.extend(filtered);
                    }
                }
                QueryStep::Aggregate {
                    operation,
                    group_by,
                } => {
                    match operation {
                        AggregateOp::Count => {
                            aggregates.insert(
                                "count".to_string(),
                                serde_json::Value::Number(nodes.len().into()),
                            );
                        }
                        _ => {
                            // Implement other aggregations
                        }
                    }
                }
                QueryStep::Limit { count } => {
                    nodes.truncate(*count);
                }
                _ => {
                    // Implement other steps
                }
            }
        }

        let execution_time_ms = start.elapsed().as_millis() as u64;

        let result = QueryResult {
            query_id: plan.query_id.clone(),
            nodes,
            edges,
            aggregates,
            stats: QueryStats {
                execution_time_ms,
                shards_queried: plan.target_shards.len(),
                nodes_scanned,
                edges_scanned,
                cached: false,
            },
        };

        // Cache the result
        self.query_cache.insert(plan.query.clone(), result.clone());

        info!(
            "Query {} completed in {}ms",
            plan.query_id, execution_time_ms
        );

        Ok(result)
    }

    /// Begin a distributed transaction
    pub fn begin_transaction(&self) -> String {
        let tx_id = Uuid::new_v4().to_string();
        let transaction = Transaction::new(tx_id.clone());
        self.transactions.insert(tx_id.clone(), transaction);
        info!("Started transaction: {}", tx_id);
        tx_id
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, tx_id: &str) -> Result<()> {
        if let Some((_, tx)) = self.transactions.remove(tx_id) {
            // In production, implement 2PC (Two-Phase Commit)
            info!("Committing transaction: {}", tx_id);
            Ok(())
        } else {
            Err(GraphError::CoordinatorError(format!(
                "Transaction not found: {}",
                tx_id
            )))
        }
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(&self, tx_id: &str) -> Result<()> {
        if let Some((_, tx)) = self.transactions.remove(tx_id) {
            warn!("Rolling back transaction: {}", tx_id);
            Ok(())
        } else {
            Err(GraphError::CoordinatorError(format!(
                "Transaction not found: {}",
                tx_id
            )))
        }
    }

    /// Clear query cache
    pub fn clear_cache(&self) {
        self.query_cache.clear();
        info!("Query cache cleared");
    }
}

/// Distributed transaction
#[derive(Debug, Clone)]
struct Transaction {
    /// Transaction ID
    id: String,
    /// Participating shards
    shards: HashSet<ShardId>,
    /// Transaction state
    state: TransactionState,
    /// Created timestamp
    created_at: DateTime<Utc>,
}

impl Transaction {
    fn new(id: String) -> Self {
        Self {
            id,
            shards: HashSet::new(),
            state: TransactionState::Active,
            created_at: Utc::now(),
        }
    }
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionState {
    Active,
    Preparing,
    Committed,
    Aborted,
}

/// Main coordinator for the entire distributed graph system
pub struct Coordinator {
    /// Shard coordinator
    shard_coordinator: Arc<ShardCoordinator>,
    /// Coordinator configuration
    config: CoordinatorConfig,
}

impl Coordinator {
    /// Create a new coordinator
    pub fn new(config: CoordinatorConfig) -> Self {
        Self {
            shard_coordinator: Arc::new(ShardCoordinator::new()),
            config,
        }
    }

    /// Get the shard coordinator
    pub fn shard_coordinator(&self) -> Arc<ShardCoordinator> {
        Arc::clone(&self.shard_coordinator)
    }

    /// Execute a query
    pub async fn execute(&self, query: &str) -> Result<QueryResult> {
        let plan = self.shard_coordinator.plan_query(query)?;
        self.shard_coordinator.execute_query(plan).await
    }

    /// Get configuration
    pub fn config(&self) -> &CoordinatorConfig {
        &self.config
    }
}

/// Coordinator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// Enable query caching
    pub enable_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Maximum query execution time
    pub max_query_time_seconds: u64,
    /// Enable query optimization
    pub enable_optimization: bool,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            enable_cache: true,
            cache_ttl_seconds: 300,
            max_query_time_seconds: 60,
            enable_optimization: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::shard::ShardMetadata;
    use crate::distributed::shard::ShardStrategy;

    #[tokio::test]
    async fn test_shard_coordinator() {
        let coordinator = ShardCoordinator::new();

        let metadata = ShardMetadata::new(0, "node-1".to_string(), ShardStrategy::Hash);
        let shard = Arc::new(GraphShard::new(metadata));

        coordinator.register_shard(0, shard);

        assert_eq!(coordinator.list_shards().len(), 1);
        assert!(coordinator.get_shard(0).is_some());
    }

    #[tokio::test]
    async fn test_query_planning() {
        let coordinator = ShardCoordinator::new();

        let metadata = ShardMetadata::new(0, "node-1".to_string(), ShardStrategy::Hash);
        let shard = Arc::new(GraphShard::new(metadata));
        coordinator.register_shard(0, shard);

        let plan = coordinator.plan_query("MATCH (n:Person) RETURN n").unwrap();

        assert!(!plan.query_id.is_empty());
        assert!(!plan.steps.is_empty());
    }

    #[tokio::test]
    async fn test_transaction() {
        let coordinator = ShardCoordinator::new();

        let tx_id = coordinator.begin_transaction();
        assert!(!tx_id.is_empty());

        coordinator.commit_transaction(&tx_id).await.unwrap();
    }
}
