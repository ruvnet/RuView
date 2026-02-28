//! Transaction support for ACID guarantees with MVCC
//!
//! Provides multi-version concurrency control for high-throughput concurrent access

use crate::edge::Edge;
use crate::error::Result;
use crate::hyperedge::{Hyperedge, HyperedgeId};
use crate::node::Node;
use crate::types::{EdgeId, NodeId};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// Dirty reads allowed
    ReadUncommitted,
    /// Only committed data visible
    ReadCommitted,
    /// Repeatable reads (default)
    RepeatableRead,
    /// Full isolation
    Serializable,
}

/// Transaction ID type
pub type TxnId = u64;

/// Timestamp for MVCC
pub type Timestamp = u64;

/// Get current timestamp
fn now() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

/// Versioned value for MVCC
#[derive(Debug, Clone)]
struct Version<T> {
    /// Creation timestamp
    created_at: Timestamp,
    /// Deletion timestamp (None if not deleted)
    deleted_at: Option<Timestamp>,
    /// Transaction ID that created this version
    created_by: TxnId,
    /// Transaction ID that deleted this version
    deleted_by: Option<TxnId>,
    /// The actual value
    value: T,
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxnState {
    Active,
    Committed,
    Aborted,
}

/// Transaction metadata
struct TxnMetadata {
    id: TxnId,
    state: TxnState,
    isolation_level: IsolationLevel,
    start_time: Timestamp,
    commit_time: Option<Timestamp>,
}

/// Transaction manager for MVCC
pub struct TransactionManager {
    /// Next transaction ID
    next_txn_id: AtomicU64,
    /// Active transactions
    active_txns: Arc<DashMap<TxnId, TxnMetadata>>,
    /// Committed transactions (for cleanup)
    committed_txns: Arc<DashMap<TxnId, Timestamp>>,
    /// Node versions (key -> list of versions)
    node_versions: Arc<DashMap<NodeId, Vec<Version<Node>>>>,
    /// Edge versions
    edge_versions: Arc<DashMap<EdgeId, Vec<Version<Edge>>>>,
    /// Hyperedge versions
    hyperedge_versions: Arc<DashMap<HyperedgeId, Vec<Version<Hyperedge>>>>,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new() -> Self {
        Self {
            next_txn_id: AtomicU64::new(1),
            active_txns: Arc::new(DashMap::new()),
            committed_txns: Arc::new(DashMap::new()),
            node_versions: Arc::new(DashMap::new()),
            edge_versions: Arc::new(DashMap::new()),
            hyperedge_versions: Arc::new(DashMap::new()),
        }
    }

    /// Begin a new transaction
    pub fn begin(&self, isolation_level: IsolationLevel) -> Transaction {
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::SeqCst);
        let start_time = now();

        let metadata = TxnMetadata {
            id: txn_id,
            state: TxnState::Active,
            isolation_level,
            start_time,
            commit_time: None,
        };

        self.active_txns.insert(txn_id, metadata);

        Transaction {
            id: txn_id,
            manager: Arc::new(self.clone()),
            isolation_level,
            start_time,
            writes: Arc::new(RwLock::new(WriteSet::new())),
        }
    }

    /// Commit a transaction
    fn commit(&self, txn_id: TxnId, writes: &WriteSet) -> Result<()> {
        let commit_time = now();

        // Apply all writes
        for (node_id, node) in &writes.nodes {
            self.node_versions
                .entry(node_id.clone())
                .or_insert_with(Vec::new)
                .push(Version {
                    created_at: commit_time,
                    deleted_at: None,
                    created_by: txn_id,
                    deleted_by: None,
                    value: node.clone(),
                });
        }

        for (edge_id, edge) in &writes.edges {
            self.edge_versions
                .entry(edge_id.clone())
                .or_insert_with(Vec::new)
                .push(Version {
                    created_at: commit_time,
                    deleted_at: None,
                    created_by: txn_id,
                    deleted_by: None,
                    value: edge.clone(),
                });
        }

        for (hyperedge_id, hyperedge) in &writes.hyperedges {
            self.hyperedge_versions
                .entry(hyperedge_id.clone())
                .or_insert_with(Vec::new)
                .push(Version {
                    created_at: commit_time,
                    deleted_at: None,
                    created_by: txn_id,
                    deleted_by: None,
                    value: hyperedge.clone(),
                });
        }

        // Mark deletes
        for node_id in &writes.deleted_nodes {
            if let Some(mut versions) = self.node_versions.get_mut(node_id) {
                if let Some(last) = versions.last_mut() {
                    last.deleted_at = Some(commit_time);
                    last.deleted_by = Some(txn_id);
                }
            }
        }

        for edge_id in &writes.deleted_edges {
            if let Some(mut versions) = self.edge_versions.get_mut(edge_id) {
                if let Some(last) = versions.last_mut() {
                    last.deleted_at = Some(commit_time);
                    last.deleted_by = Some(txn_id);
                }
            }
        }

        // Update transaction state
        if let Some(mut metadata) = self.active_txns.get_mut(&txn_id) {
            metadata.state = TxnState::Committed;
            metadata.commit_time = Some(commit_time);
        }

        self.active_txns.remove(&txn_id);
        self.committed_txns.insert(txn_id, commit_time);

        Ok(())
    }

    /// Abort a transaction
    fn abort(&self, txn_id: TxnId) -> Result<()> {
        if let Some(mut metadata) = self.active_txns.get_mut(&txn_id) {
            metadata.state = TxnState::Aborted;
        }
        self.active_txns.remove(&txn_id);
        Ok(())
    }

    /// Read a node with MVCC
    fn read_node(&self, node_id: &NodeId, txn_id: TxnId, start_time: Timestamp) -> Option<Node> {
        self.node_versions.get(node_id).and_then(|versions| {
            versions
                .iter()
                .rev()
                .find(|v| {
                    v.created_at <= start_time
                        && v.deleted_at.map_or(true, |d| d > start_time)
                        && v.created_by != txn_id
                })
                .map(|v| v.value.clone())
        })
    }

    /// Read an edge with MVCC
    fn read_edge(&self, edge_id: &EdgeId, txn_id: TxnId, start_time: Timestamp) -> Option<Edge> {
        self.edge_versions.get(edge_id).and_then(|versions| {
            versions
                .iter()
                .rev()
                .find(|v| {
                    v.created_at <= start_time
                        && v.deleted_at.map_or(true, |d| d > start_time)
                        && v.created_by != txn_id
                })
                .map(|v| v.value.clone())
        })
    }
}

impl Clone for TransactionManager {
    fn clone(&self) -> Self {
        Self {
            next_txn_id: AtomicU64::new(self.next_txn_id.load(Ordering::SeqCst)),
            active_txns: Arc::clone(&self.active_txns),
            committed_txns: Arc::clone(&self.committed_txns),
            node_versions: Arc::clone(&self.node_versions),
            edge_versions: Arc::clone(&self.edge_versions),
            hyperedge_versions: Arc::clone(&self.hyperedge_versions),
        }
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Write set for a transaction
#[derive(Debug, Clone, Default)]
struct WriteSet {
    nodes: HashMap<NodeId, Node>,
    edges: HashMap<EdgeId, Edge>,
    hyperedges: HashMap<HyperedgeId, Hyperedge>,
    deleted_nodes: HashSet<NodeId>,
    deleted_edges: HashSet<EdgeId>,
    deleted_hyperedges: HashSet<HyperedgeId>,
}

impl WriteSet {
    fn new() -> Self {
        Self::default()
    }
}

/// Transaction handle
pub struct Transaction {
    id: TxnId,
    manager: Arc<TransactionManager>,
    /// The isolation level for this transaction
    pub isolation_level: IsolationLevel,
    start_time: Timestamp,
    writes: Arc<RwLock<WriteSet>>,
}

impl Transaction {
    /// Begin a new standalone transaction
    ///
    /// This creates an internal TransactionManager for simple use cases.
    /// For production use, prefer using a shared TransactionManager.
    pub fn begin(isolation_level: IsolationLevel) -> Result<Self> {
        let manager = TransactionManager::new();
        Ok(manager.begin(isolation_level))
    }

    /// Get transaction ID
    pub fn id(&self) -> TxnId {
        self.id
    }

    /// Write a node (buffered until commit)
    pub fn write_node(&self, node: Node) {
        let mut writes = self.writes.write();
        writes.nodes.insert(node.id.clone(), node);
    }

    /// Write an edge (buffered until commit)
    pub fn write_edge(&self, edge: Edge) {
        let mut writes = self.writes.write();
        writes.edges.insert(edge.id.clone(), edge);
    }

    /// Write a hyperedge (buffered until commit)
    pub fn write_hyperedge(&self, hyperedge: Hyperedge) {
        let mut writes = self.writes.write();
        writes.hyperedges.insert(hyperedge.id.clone(), hyperedge);
    }

    /// Delete a node (buffered until commit)
    pub fn delete_node(&self, node_id: NodeId) {
        let mut writes = self.writes.write();
        writes.deleted_nodes.insert(node_id);
    }

    /// Delete an edge (buffered until commit)
    pub fn delete_edge(&self, edge_id: EdgeId) {
        let mut writes = self.writes.write();
        writes.deleted_edges.insert(edge_id);
    }

    /// Read a node (with MVCC visibility)
    pub fn read_node(&self, node_id: &NodeId) -> Option<Node> {
        // Check write set first
        {
            let writes = self.writes.read();
            if writes.deleted_nodes.contains(node_id) {
                return None;
            }
            if let Some(node) = writes.nodes.get(node_id) {
                return Some(node.clone());
            }
        }

        // Read from MVCC store
        self.manager.read_node(node_id, self.id, self.start_time)
    }

    /// Read an edge (with MVCC visibility)
    pub fn read_edge(&self, edge_id: &EdgeId) -> Option<Edge> {
        // Check write set first
        {
            let writes = self.writes.read();
            if writes.deleted_edges.contains(edge_id) {
                return None;
            }
            if let Some(edge) = writes.edges.get(edge_id) {
                return Some(edge.clone());
            }
        }

        // Read from MVCC store
        self.manager.read_edge(edge_id, self.id, self.start_time)
    }

    /// Commit the transaction
    pub fn commit(self) -> Result<()> {
        let writes = self.writes.read();
        self.manager.commit(self.id, &writes)
    }

    /// Rollback the transaction
    pub fn rollback(self) -> Result<()> {
        self.manager.abort(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeBuilder;

    #[test]
    fn test_transaction_basic() {
        let manager = TransactionManager::new();
        let txn = manager.begin(IsolationLevel::ReadCommitted);

        assert_eq!(txn.isolation_level, IsolationLevel::ReadCommitted);
        assert!(txn.id() > 0);
    }

    #[test]
    fn test_mvcc_read_write() {
        let manager = TransactionManager::new();

        // Transaction 1: Write a node
        let txn1 = manager.begin(IsolationLevel::ReadCommitted);
        let node = NodeBuilder::new()
            .label("Person")
            .property("name", "Alice")
            .build();
        let node_id = node.id.clone();
        txn1.write_node(node.clone());
        txn1.commit().unwrap();

        // Transaction 2: Read the node
        let txn2 = manager.begin(IsolationLevel::ReadCommitted);
        let read_node = txn2.read_node(&node_id);
        assert!(read_node.is_some());
        assert_eq!(read_node.unwrap().id, node_id);
    }

    #[test]
    fn test_transaction_isolation() {
        let manager = TransactionManager::new();

        let node = NodeBuilder::new().build();
        let node_id = node.id.clone();

        // Txn1: Write but don't commit
        let txn1 = manager.begin(IsolationLevel::ReadCommitted);
        txn1.write_node(node.clone());

        // Txn2: Should not see uncommitted write
        let txn2 = manager.begin(IsolationLevel::ReadCommitted);
        assert!(txn2.read_node(&node_id).is_none());

        // Commit txn1
        txn1.commit().unwrap();

        // Txn3: Should see committed write
        let txn3 = manager.begin(IsolationLevel::ReadCommitted);
        assert!(txn3.read_node(&node_id).is_some());
    }
}
