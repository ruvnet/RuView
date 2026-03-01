//! # RuVector Delta Consensus
//!
//! Distributed delta consensus using CRDTs and causal ordering.
//! Enables consistent delta application across distributed nodes.
//!
//! ## Key Features
//!
//! - CRDT-based delta merging
//! - Causal ordering with vector clocks
//! - Conflict resolution strategies
//! - Delta compression for network transfer

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod causal;
pub mod conflict;
pub mod crdt;
pub mod error;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ruvector_delta_core::{Delta, VectorDelta};

pub use causal::{CausalOrder, HybridLogicalClock, VectorClock};
pub use conflict::{ConflictResolver, ConflictStrategy, MergeResult};
pub use crdt::{DeltaCrdt, GCounter, LWWRegister, ORSet, PNCounter};
pub use error::{ConsensusError, Result};

/// A replica identifier
pub type ReplicaId = String;

/// A delta with causal metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalDelta {
    /// Unique delta ID
    pub id: Uuid,
    /// The delta data
    pub delta: VectorDelta,
    /// Vector clock for causal ordering
    pub vector_clock: VectorClock,
    /// Origin replica
    pub origin: ReplicaId,
    /// Timestamp (for HLC)
    pub timestamp: u64,
    /// Dependencies (delta IDs this depends on)
    pub dependencies: Vec<Uuid>,
}

impl CausalDelta {
    /// Create a new causal delta
    pub fn new(delta: VectorDelta, origin: ReplicaId, clock: VectorClock) -> Self {
        Self {
            id: Uuid::new_v4(),
            delta,
            vector_clock: clock,
            origin,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            dependencies: Vec::new(),
        }
    }

    /// Add a dependency
    pub fn with_dependency(mut self, dep: Uuid) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Check if this delta is causally before another
    pub fn is_before(&self, other: &CausalDelta) -> bool {
        self.vector_clock.happens_before(&other.vector_clock)
    }

    /// Check if deltas are concurrent
    pub fn is_concurrent(&self, other: &CausalDelta) -> bool {
        self.vector_clock.is_concurrent(&other.vector_clock)
    }
}

/// Configuration for consensus
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// This replica's ID
    pub replica_id: ReplicaId,
    /// Conflict resolution strategy
    pub conflict_strategy: ConflictStrategy,
    /// Maximum pending deltas before compaction
    pub max_pending: usize,
    /// Whether to enable causal delivery
    pub causal_delivery: bool,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            replica_id: Uuid::new_v4().to_string(),
            conflict_strategy: ConflictStrategy::LastWriteWins,
            max_pending: 1000,
            causal_delivery: true,
        }
    }
}

/// Delta consensus coordinator
pub struct DeltaConsensus {
    config: ConsensusConfig,
    /// Current vector clock
    clock: RwLock<VectorClock>,
    /// Pending deltas awaiting delivery
    pending: RwLock<HashMap<Uuid, CausalDelta>>,
    /// Applied delta IDs
    applied: RwLock<HashSet<Uuid>>,
    /// Conflict resolver
    resolver: Box<dyn ConflictResolver<VectorDelta> + Send + Sync>,
}

impl DeltaConsensus {
    /// Create a new consensus coordinator
    pub fn new(config: ConsensusConfig) -> Self {
        let resolver: Box<dyn ConflictResolver<VectorDelta> + Send + Sync> =
            match config.conflict_strategy {
                ConflictStrategy::LastWriteWins => Box::new(conflict::LastWriteWinsResolver),
                ConflictStrategy::FirstWriteWins => Box::new(conflict::FirstWriteWinsResolver),
                ConflictStrategy::Merge => Box::new(conflict::MergeResolver::default()),
                ConflictStrategy::Custom => Box::new(conflict::MergeResolver::default()),
            };

        Self {
            config,
            clock: RwLock::new(VectorClock::new()),
            pending: RwLock::new(HashMap::new()),
            applied: RwLock::new(HashSet::new()),
            resolver,
        }
    }

    /// Get current replica ID
    pub fn replica_id(&self) -> &ReplicaId {
        &self.config.replica_id
    }

    /// Create a new local delta with causal metadata
    pub fn create_delta(&self, delta: VectorDelta) -> CausalDelta {
        let mut clock = self.clock.write();
        clock.increment(&self.config.replica_id);

        CausalDelta::new(delta, self.config.replica_id.clone(), clock.clone())
    }

    /// Receive a delta from another replica
    pub fn receive(&self, delta: CausalDelta) -> Result<DeliveryStatus> {
        // Check if already applied
        if self.applied.read().contains(&delta.id) {
            return Ok(DeliveryStatus::AlreadyApplied);
        }

        // Check causal dependencies
        if self.config.causal_delivery {
            if !self.dependencies_satisfied(&delta) {
                // Queue for later delivery
                self.pending.write().insert(delta.id, delta);
                return Ok(DeliveryStatus::Pending);
            }
        }

        // Update vector clock
        {
            let mut clock = self.clock.write();
            clock.merge(&delta.vector_clock);
            clock.increment(&self.config.replica_id);
        }

        // Mark as applied
        self.applied.write().insert(delta.id);

        // Try to deliver pending deltas
        self.try_deliver_pending()?;

        Ok(DeliveryStatus::Delivered)
    }

    /// Apply delta to a base vector, handling conflicts
    pub fn apply_with_consensus(
        &self,
        delta: &CausalDelta,
        base: &mut Vec<f32>,
        concurrent_deltas: &[CausalDelta],
    ) -> Result<()> {
        if concurrent_deltas.is_empty() {
            // No conflicts, apply directly
            delta
                .delta
                .apply(base)
                .map_err(|e| ConsensusError::DeltaError(format!("{:?}", e)))?;
        } else {
            // Resolve conflicts
            let mut all_deltas: Vec<&CausalDelta> = vec![delta];
            all_deltas.extend(concurrent_deltas);

            let resolved = self.resolve_conflicts(&all_deltas)?;
            resolved
                .apply(base)
                .map_err(|e| ConsensusError::DeltaError(format!("{:?}", e)))?;
        }

        Ok(())
    }

    /// Get all pending deltas
    pub fn pending_deltas(&self) -> Vec<CausalDelta> {
        self.pending.read().values().cloned().collect()
    }

    /// Get number of pending deltas
    pub fn pending_count(&self) -> usize {
        self.pending.read().len()
    }

    /// Get current vector clock
    pub fn current_clock(&self) -> VectorClock {
        self.clock.read().clone()
    }

    /// Clear applied history (for memory management)
    pub fn clear_history(&self) {
        self.applied.write().clear();
    }

    // Private methods

    fn dependencies_satisfied(&self, delta: &CausalDelta) -> bool {
        let applied = self.applied.read();

        for dep in &delta.dependencies {
            if !applied.contains(dep) {
                return false;
            }
        }

        true
    }

    fn try_deliver_pending(&self) -> Result<usize> {
        let mut delivered = 0;

        loop {
            let pending = self.pending.read();
            let ready: Vec<Uuid> = pending
                .iter()
                .filter(|(_, d)| self.dependencies_satisfied(d))
                .map(|(id, _)| *id)
                .collect();
            drop(pending);

            if ready.is_empty() {
                break;
            }

            for id in ready {
                if let Some(delta) = self.pending.write().remove(&id) {
                    // Update clock
                    {
                        let mut clock = self.clock.write();
                        clock.merge(&delta.vector_clock);
                        clock.increment(&self.config.replica_id);
                    }

                    self.applied.write().insert(id);
                    delivered += 1;
                }
            }
        }

        Ok(delivered)
    }

    fn resolve_conflicts(&self, deltas: &[&CausalDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        if deltas.len() == 1 {
            return Ok(deltas[0].delta.clone());
        }

        // Sort by timestamp for deterministic resolution
        let mut sorted: Vec<_> = deltas.iter().collect();
        sorted.sort_by_key(|d| d.timestamp);

        // Use resolver
        let delta_refs: Vec<_> = sorted.iter().map(|d| &d.delta).collect();
        self.resolver.resolve(&delta_refs)
    }
}

/// Status of delta delivery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    /// Delta was delivered successfully
    Delivered,
    /// Delta is pending (waiting for dependencies)
    Pending,
    /// Delta was already applied
    AlreadyApplied,
    /// Delta was rejected
    Rejected,
}

/// Gossip protocol for delta dissemination
pub struct DeltaGossip {
    consensus: Arc<DeltaConsensus>,
    /// Known peers
    peers: RwLock<HashSet<ReplicaId>>,
    /// Deltas to send
    outbox: RwLock<Vec<CausalDelta>>,
}

impl DeltaGossip {
    /// Create new gossip protocol
    pub fn new(consensus: Arc<DeltaConsensus>) -> Self {
        Self {
            consensus,
            peers: RwLock::new(HashSet::new()),
            outbox: RwLock::new(Vec::new()),
        }
    }

    /// Add a peer
    pub fn add_peer(&self, peer: ReplicaId) {
        self.peers.write().insert(peer);
    }

    /// Remove a peer
    pub fn remove_peer(&self, peer: &ReplicaId) {
        self.peers.write().remove(peer);
    }

    /// Queue delta for gossip
    pub fn broadcast(&self, delta: CausalDelta) {
        self.outbox.write().push(delta);
    }

    /// Get deltas to send
    pub fn get_outbox(&self) -> Vec<CausalDelta> {
        let mut outbox = self.outbox.write();
        std::mem::take(&mut *outbox)
    }

    /// Receive gossip from peer
    pub fn receive_gossip(&self, deltas: Vec<CausalDelta>) -> Result<GossipResult> {
        let mut delivered = 0;
        let mut pending = 0;
        let mut already_applied = 0;

        for delta in deltas {
            match self.consensus.receive(delta)? {
                DeliveryStatus::Delivered => delivered += 1,
                DeliveryStatus::Pending => pending += 1,
                DeliveryStatus::AlreadyApplied => already_applied += 1,
                DeliveryStatus::Rejected => {}
            }
        }

        Ok(GossipResult {
            delivered,
            pending,
            already_applied,
        })
    }

    /// Get anti-entropy summary (for sync)
    pub fn get_summary(&self) -> GossipSummary {
        GossipSummary {
            replica_id: self.consensus.replica_id().clone(),
            clock: self.consensus.current_clock(),
            pending_count: self.consensus.pending_count(),
        }
    }
}

/// Result of gossip receive
#[derive(Debug, Clone)]
pub struct GossipResult {
    /// Deltas delivered
    pub delivered: usize,
    /// Deltas pending
    pub pending: usize,
    /// Deltas already applied
    pub already_applied: usize,
}

/// Summary for anti-entropy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipSummary {
    /// Replica ID
    pub replica_id: ReplicaId,
    /// Current vector clock
    pub clock: VectorClock,
    /// Number of pending deltas
    pub pending_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_delta() {
        let config = ConsensusConfig {
            replica_id: "replica1".to_string(),
            ..Default::default()
        };

        let consensus = DeltaConsensus::new(config);
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0]);
        let causal = consensus.create_delta(delta);

        assert_eq!(causal.origin, "replica1");
        assert!(!causal.id.is_nil());
    }

    #[test]
    fn test_receive_delta() {
        let config = ConsensusConfig {
            replica_id: "replica1".to_string(),
            causal_delivery: false,
            ..Default::default()
        };

        let consensus = DeltaConsensus::new(config);

        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0]);
        let causal = CausalDelta::new(delta, "replica2".to_string(), VectorClock::new());

        let status = consensus.receive(causal).unwrap();
        assert_eq!(status, DeliveryStatus::Delivered);
    }

    #[test]
    fn test_causal_ordering() {
        let clock1 = {
            let mut c = VectorClock::new();
            c.increment("r1");
            c
        };

        let clock2 = {
            let mut c = clock1.clone();
            c.increment("r1");
            c
        };

        let d1 = CausalDelta::new(VectorDelta::from_dense(vec![1.0]), "r1".to_string(), clock1);

        let d2 = CausalDelta::new(VectorDelta::from_dense(vec![2.0]), "r1".to_string(), clock2);

        assert!(d1.is_before(&d2));
        assert!(!d2.is_before(&d1));
    }

    #[test]
    fn test_concurrent_deltas() {
        let clock1 = {
            let mut c = VectorClock::new();
            c.increment("r1");
            c
        };

        let clock2 = {
            let mut c = VectorClock::new();
            c.increment("r2");
            c
        };

        let d1 = CausalDelta::new(VectorDelta::from_dense(vec![1.0]), "r1".to_string(), clock1);

        let d2 = CausalDelta::new(VectorDelta::from_dense(vec![2.0]), "r2".to_string(), clock2);

        assert!(d1.is_concurrent(&d2));
    }
}
