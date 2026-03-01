//! DAG-based consensus protocol inspired by QuDAG
//!
//! Implements a directed acyclic graph for transaction ordering and consensus.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{ClusterError, Result};

/// A vertex in the consensus DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagVertex {
    /// Unique vertex ID
    pub id: String,
    /// Node that created this vertex
    pub node_id: String,
    /// Transaction data
    pub transaction: Transaction,
    /// Parent vertices (edges in the DAG)
    pub parents: Vec<String>,
    /// Timestamp when vertex was created
    pub timestamp: DateTime<Utc>,
    /// Vector clock for causality tracking
    pub vector_clock: HashMap<String, u64>,
    /// Signature (in production, this would be cryptographic)
    pub signature: String,
}

impl DagVertex {
    /// Create a new DAG vertex
    pub fn new(
        node_id: String,
        transaction: Transaction,
        parents: Vec<String>,
        vector_clock: HashMap<String, u64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            node_id,
            transaction,
            parents,
            timestamp: Utc::now(),
            vector_clock,
            signature: String::new(), // Would be computed cryptographically
        }
    }

    /// Verify the vertex signature
    pub fn verify_signature(&self) -> bool {
        // In production, verify cryptographic signature
        true
    }
}

/// A transaction in the consensus system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction ID
    pub id: String,
    /// Transaction type
    pub tx_type: TransactionType,
    /// Transaction data
    pub data: Vec<u8>,
    /// Nonce for ordering
    pub nonce: u64,
}

/// Type of transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    /// Write operation
    Write,
    /// Read operation
    Read,
    /// Delete operation
    Delete,
    /// Batch operation
    Batch,
    /// System operation
    System,
}

/// DAG-based consensus engine
pub struct DagConsensus {
    /// Node ID
    node_id: String,
    /// DAG vertices (vertex_id -> vertex)
    vertices: Arc<DashMap<String, DagVertex>>,
    /// Finalized vertices
    finalized: Arc<RwLock<HashSet<String>>>,
    /// Vector clock for this node
    vector_clock: Arc<RwLock<HashMap<String, u64>>>,
    /// Pending transactions
    pending_txs: Arc<RwLock<VecDeque<Transaction>>>,
    /// Minimum quorum size
    min_quorum_size: usize,
    /// Transaction nonce counter
    nonce_counter: Arc<RwLock<u64>>,
}

impl DagConsensus {
    /// Create a new DAG consensus engine
    pub fn new(node_id: String, min_quorum_size: usize) -> Self {
        let mut vector_clock = HashMap::new();
        vector_clock.insert(node_id.clone(), 0);

        Self {
            node_id,
            vertices: Arc::new(DashMap::new()),
            finalized: Arc::new(RwLock::new(HashSet::new())),
            vector_clock: Arc::new(RwLock::new(vector_clock)),
            pending_txs: Arc::new(RwLock::new(VecDeque::new())),
            min_quorum_size,
            nonce_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Submit a transaction to the consensus system
    pub fn submit_transaction(&self, tx_type: TransactionType, data: Vec<u8>) -> Result<String> {
        let mut nonce = self.nonce_counter.write();
        *nonce += 1;

        let transaction = Transaction {
            id: Uuid::new_v4().to_string(),
            tx_type,
            data,
            nonce: *nonce,
        };

        let tx_id = transaction.id.clone();

        let mut pending = self.pending_txs.write();
        pending.push_back(transaction);

        debug!("Transaction {} submitted to consensus", tx_id);
        Ok(tx_id)
    }

    /// Create a new vertex for pending transactions
    pub fn create_vertex(&self) -> Result<Option<DagVertex>> {
        let mut pending = self.pending_txs.write();

        if pending.is_empty() {
            return Ok(None);
        }

        // Take the next transaction
        let transaction = pending.pop_front().unwrap();

        // Find parent vertices (tips of the DAG)
        let parents = self.find_tips();

        // Update vector clock
        let mut clock = self.vector_clock.write();
        let count = clock.entry(self.node_id.clone()).or_insert(0);
        *count += 1;

        let vertex = DagVertex::new(self.node_id.clone(), transaction, parents, clock.clone());

        let vertex_id = vertex.id.clone();
        self.vertices.insert(vertex_id.clone(), vertex.clone());

        debug!(
            "Created vertex {} for transaction {}",
            vertex_id, vertex.transaction.id
        );
        Ok(Some(vertex))
    }

    /// Find tip vertices (vertices with no children)
    fn find_tips(&self) -> Vec<String> {
        let mut has_children = HashSet::new();

        // Mark all vertices that have children
        for entry in self.vertices.iter() {
            for parent in &entry.value().parents {
                has_children.insert(parent.clone());
            }
        }

        // Find vertices without children
        self.vertices
            .iter()
            .filter(|entry| !has_children.contains(entry.key()))
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Add a vertex from another node
    pub fn add_vertex(&self, vertex: DagVertex) -> Result<()> {
        // Verify signature
        if !vertex.verify_signature() {
            return Err(ClusterError::ConsensusError(
                "Invalid vertex signature".to_string(),
            ));
        }

        // Verify parents exist
        for parent_id in &vertex.parents {
            if !self.vertices.contains_key(parent_id) && !self.is_finalized(parent_id) {
                return Err(ClusterError::ConsensusError(format!(
                    "Parent vertex {} not found",
                    parent_id
                )));
            }
        }

        // Merge vector clock
        let mut clock = self.vector_clock.write();
        for (node, count) in &vertex.vector_clock {
            let existing = clock.entry(node.clone()).or_insert(0);
            *existing = (*existing).max(*count);
        }

        self.vertices.insert(vertex.id.clone(), vertex);
        Ok(())
    }

    /// Check if a vertex is finalized
    pub fn is_finalized(&self, vertex_id: &str) -> bool {
        let finalized = self.finalized.read();
        finalized.contains(vertex_id)
    }

    /// Finalize vertices using the wave algorithm
    pub fn finalize_vertices(&self) -> Result<Vec<String>> {
        let mut finalized_ids = Vec::new();

        // Find vertices that can be finalized
        // A vertex is finalized if it has enough confirmations from different nodes
        let mut confirmations: HashMap<String, HashSet<String>> = HashMap::new();

        for entry in self.vertices.iter() {
            let vertex = entry.value();

            // Count confirmations (vertices that reference this one)
            for other_entry in self.vertices.iter() {
                if other_entry.value().parents.contains(&vertex.id) {
                    confirmations
                        .entry(vertex.id.clone())
                        .or_insert_with(HashSet::new)
                        .insert(other_entry.value().node_id.clone());
                }
            }
        }

        // Finalize vertices with enough confirmations
        let mut finalized = self.finalized.write();

        for (vertex_id, confirming_nodes) in confirmations {
            if confirming_nodes.len() >= self.min_quorum_size && !finalized.contains(&vertex_id) {
                finalized.insert(vertex_id.clone());
                finalized_ids.push(vertex_id.clone());
                info!("Finalized vertex {}", vertex_id);
            }
        }

        Ok(finalized_ids)
    }

    /// Get the total order of finalized transactions
    pub fn get_finalized_order(&self) -> Vec<Transaction> {
        let finalized = self.finalized.read();
        let mut ordered_txs = Vec::new();

        // Topological sort of finalized vertices
        let finalized_vertices: Vec<_> = self
            .vertices
            .iter()
            .filter(|entry| finalized.contains(entry.key()))
            .map(|entry| entry.value().clone())
            .collect();

        // Sort by vector clock and timestamp
        let mut sorted = finalized_vertices;
        sorted.sort_by(|a, b| {
            // First by vector clock dominance
            let a_dominates = Self::vector_clock_dominates(&a.vector_clock, &b.vector_clock);
            let b_dominates = Self::vector_clock_dominates(&b.vector_clock, &a.vector_clock);

            if a_dominates && !b_dominates {
                std::cmp::Ordering::Less
            } else if b_dominates && !a_dominates {
                std::cmp::Ordering::Greater
            } else {
                // Fall back to timestamp
                a.timestamp.cmp(&b.timestamp)
            }
        });

        for vertex in sorted {
            ordered_txs.push(vertex.transaction);
        }

        ordered_txs
    }

    /// Check if vector clock a dominates vector clock b
    fn vector_clock_dominates(a: &HashMap<String, u64>, b: &HashMap<String, u64>) -> bool {
        let mut dominates = false;

        for (node, &a_count) in a {
            let b_count = b.get(node).copied().unwrap_or(0);
            if a_count < b_count {
                return false;
            }
            if a_count > b_count {
                dominates = true;
            }
        }

        dominates
    }

    /// Detect conflicts between transactions
    pub fn detect_conflicts(&self, tx1: &Transaction, tx2: &Transaction) -> bool {
        // In a real implementation, this would analyze transaction data
        // For now, conservatively assume all writes conflict
        matches!(
            (&tx1.tx_type, &tx2.tx_type),
            (TransactionType::Write, TransactionType::Write)
                | (TransactionType::Delete, TransactionType::Write)
                | (TransactionType::Write, TransactionType::Delete)
        )
    }

    /// Get consensus statistics
    pub fn get_stats(&self) -> ConsensusStats {
        let finalized = self.finalized.read();
        let pending = self.pending_txs.read();

        ConsensusStats {
            total_vertices: self.vertices.len(),
            finalized_vertices: finalized.len(),
            pending_transactions: pending.len(),
            tips: self.find_tips().len(),
        }
    }

    /// Prune old finalized vertices to save memory
    pub fn prune_old_vertices(&self, keep_count: usize) {
        let finalized = self.finalized.read();

        if finalized.len() <= keep_count {
            return;
        }

        // Remove oldest finalized vertices
        let mut vertices_to_remove = Vec::new();

        for vertex_id in finalized.iter() {
            if let Some(vertex) = self.vertices.get(vertex_id) {
                vertices_to_remove.push((vertex_id.clone(), vertex.timestamp));
            }
        }

        vertices_to_remove.sort_by_key(|(_, ts)| *ts);

        let to_remove = vertices_to_remove.len().saturating_sub(keep_count);
        for (vertex_id, _) in vertices_to_remove.iter().take(to_remove) {
            self.vertices.remove(vertex_id);
        }

        debug!("Pruned {} old vertices", to_remove);
    }
}

/// Consensus statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusStats {
    pub total_vertices: usize,
    pub finalized_vertices: usize,
    pub pending_transactions: usize,
    pub tips: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_creation() {
        let consensus = DagConsensus::new("node1".to_string(), 2);
        let stats = consensus.get_stats();

        assert_eq!(stats.total_vertices, 0);
        assert_eq!(stats.pending_transactions, 0);
    }

    #[test]
    fn test_submit_transaction() {
        let consensus = DagConsensus::new("node1".to_string(), 2);

        let tx_id = consensus
            .submit_transaction(TransactionType::Write, vec![1, 2, 3])
            .unwrap();

        assert!(!tx_id.is_empty());

        let stats = consensus.get_stats();
        assert_eq!(stats.pending_transactions, 1);
    }

    #[test]
    fn test_create_vertex() {
        let consensus = DagConsensus::new("node1".to_string(), 2);

        consensus
            .submit_transaction(TransactionType::Write, vec![1, 2, 3])
            .unwrap();

        let vertex = consensus.create_vertex().unwrap();
        assert!(vertex.is_some());

        let stats = consensus.get_stats();
        assert_eq!(stats.total_vertices, 1);
        assert_eq!(stats.pending_transactions, 0);
    }

    #[test]
    fn test_vector_clock_dominance() {
        let mut clock1 = HashMap::new();
        clock1.insert("node1".to_string(), 2);
        clock1.insert("node2".to_string(), 1);

        let mut clock2 = HashMap::new();
        clock2.insert("node1".to_string(), 1);
        clock2.insert("node2".to_string(), 1);

        assert!(DagConsensus::vector_clock_dominates(&clock1, &clock2));
        assert!(!DagConsensus::vector_clock_dominates(&clock2, &clock1));
    }

    #[test]
    fn test_conflict_detection() {
        let consensus = DagConsensus::new("node1".to_string(), 2);

        let tx1 = Transaction {
            id: "1".to_string(),
            tx_type: TransactionType::Write,
            data: vec![1],
            nonce: 1,
        };

        let tx2 = Transaction {
            id: "2".to_string(),
            tx_type: TransactionType::Write,
            data: vec![2],
            nonce: 2,
        };

        assert!(consensus.detect_conflicts(&tx1, &tx2));
    }

    #[test]
    fn test_finalization() {
        let consensus = DagConsensus::new("node1".to_string(), 2);

        // Create some vertices
        for i in 0..5 {
            consensus
                .submit_transaction(TransactionType::Write, vec![i])
                .unwrap();
            consensus.create_vertex().unwrap();
        }

        // Try to finalize
        let finalized = consensus.finalize_vertices().unwrap();

        // Without enough confirmations, nothing should be finalized yet
        // (would need vertices from other nodes)
        assert_eq!(finalized.len(), 0);
    }
}
