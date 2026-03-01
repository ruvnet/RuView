//! Replica management and coordination
//!
//! Provides structures and logic for managing distributed replicas,
//! including role management, health tracking, and promotion/demotion.

use crate::{ReplicationError, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Role of a replica in the replication topology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaRole {
    /// Primary replica that handles writes
    Primary,
    /// Secondary replica that replicates from primary
    Secondary,
    /// Witness replica for quorum without data replication
    Witness,
}

/// Current status of a replica
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaStatus {
    /// Replica is online and healthy
    Healthy,
    /// Replica is lagging behind
    Lagging,
    /// Replica is offline or unreachable
    Offline,
    /// Replica is recovering
    Recovering,
}

/// Represents a single replica in the replication topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replica {
    /// Unique identifier for the replica
    pub id: String,
    /// Network address of the replica
    pub address: String,
    /// Current role of the replica
    pub role: ReplicaRole,
    /// Current status of the replica
    pub status: ReplicaStatus,
    /// Replication lag in milliseconds
    pub lag_ms: u64,
    /// Last known position in the replication log
    pub log_position: u64,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
    /// Priority for failover (higher is better)
    pub priority: u32,
}

impl Replica {
    /// Create a new replica
    pub fn new(id: impl Into<String>, address: impl Into<String>, role: ReplicaRole) -> Self {
        Self {
            id: id.into(),
            address: address.into(),
            role,
            status: ReplicaStatus::Healthy,
            lag_ms: 0,
            log_position: 0,
            last_heartbeat: Utc::now(),
            priority: 100,
        }
    }

    /// Check if the replica is healthy
    pub fn is_healthy(&self) -> bool {
        self.status == ReplicaStatus::Healthy && self.lag_ms < 5000
    }

    /// Check if the replica is available for reads
    pub fn is_readable(&self) -> bool {
        matches!(self.status, ReplicaStatus::Healthy | ReplicaStatus::Lagging)
    }

    /// Check if the replica is available for writes
    pub fn is_writable(&self) -> bool {
        self.role == ReplicaRole::Primary && self.status == ReplicaStatus::Healthy
    }

    /// Update the replica's lag
    pub fn update_lag(&mut self, lag_ms: u64) {
        self.lag_ms = lag_ms;
        if lag_ms > 5000 {
            self.status = ReplicaStatus::Lagging;
        } else if self.status == ReplicaStatus::Lagging {
            self.status = ReplicaStatus::Healthy;
        }
    }

    /// Update the replica's log position
    pub fn update_position(&mut self, position: u64) {
        self.log_position = position;
    }

    /// Record a heartbeat
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
        if self.status == ReplicaStatus::Offline {
            self.status = ReplicaStatus::Recovering;
        }
    }

    /// Check if the replica has timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        let elapsed = Utc::now()
            .signed_duration_since(self.last_heartbeat)
            .to_std()
            .unwrap_or(Duration::MAX);
        elapsed > timeout
    }
}

/// Manages a set of replicas
pub struct ReplicaSet {
    /// Cluster identifier
    cluster_id: String,
    /// Map of replica ID to replica
    replicas: Arc<DashMap<String, Replica>>,
    /// Current primary replica ID
    primary_id: Arc<RwLock<Option<String>>>,
    /// Minimum number of replicas for quorum
    quorum_size: Arc<RwLock<usize>>,
}

impl ReplicaSet {
    /// Create a new replica set
    pub fn new(cluster_id: impl Into<String>) -> Self {
        Self {
            cluster_id: cluster_id.into(),
            replicas: Arc::new(DashMap::new()),
            primary_id: Arc::new(RwLock::new(None)),
            quorum_size: Arc::new(RwLock::new(1)),
        }
    }

    /// Add a replica to the set
    pub fn add_replica(
        &mut self,
        id: impl Into<String>,
        address: impl Into<String>,
        role: ReplicaRole,
    ) -> Result<()> {
        let id = id.into();
        let replica = Replica::new(id.clone(), address, role);

        if role == ReplicaRole::Primary {
            let mut primary = self.primary_id.write();
            if primary.is_some() {
                return Err(ReplicationError::InvalidState(
                    "Primary replica already exists".to_string(),
                ));
            }
            *primary = Some(id.clone());
        }

        self.replicas.insert(id, replica);
        self.update_quorum_size();
        Ok(())
    }

    /// Remove a replica from the set
    pub fn remove_replica(&mut self, id: &str) -> Result<()> {
        let replica = self
            .replicas
            .remove(id)
            .ok_or_else(|| ReplicationError::ReplicaNotFound(id.to_string()))?;

        if replica.1.role == ReplicaRole::Primary {
            let mut primary = self.primary_id.write();
            *primary = None;
        }

        self.update_quorum_size();
        Ok(())
    }

    /// Get a replica by ID
    pub fn get_replica(&self, id: &str) -> Option<Replica> {
        self.replicas.get(id).map(|r| r.clone())
    }

    /// Get the current primary replica
    pub fn get_primary(&self) -> Option<Replica> {
        let primary_id = self.primary_id.read();
        primary_id
            .as_ref()
            .and_then(|id| self.replicas.get(id).map(|r| r.clone()))
    }

    /// Get all secondary replicas
    pub fn get_secondaries(&self) -> Vec<Replica> {
        self.replicas
            .iter()
            .filter(|r| r.role == ReplicaRole::Secondary)
            .map(|r| r.clone())
            .collect()
    }

    /// Get all healthy replicas
    pub fn get_healthy_replicas(&self) -> Vec<Replica> {
        self.replicas
            .iter()
            .filter(|r| r.is_healthy())
            .map(|r| r.clone())
            .collect()
    }

    /// Promote a secondary to primary
    pub fn promote_to_primary(&mut self, id: &str) -> Result<()> {
        // Get the replica and verify it exists
        let mut replica = self
            .replicas
            .get_mut(id)
            .ok_or_else(|| ReplicationError::ReplicaNotFound(id.to_string()))?;

        if replica.role == ReplicaRole::Primary {
            return Ok(());
        }

        if replica.role == ReplicaRole::Witness {
            return Err(ReplicationError::InvalidState(
                "Cannot promote witness to primary".to_string(),
            ));
        }

        // Demote current primary if exists
        let old_primary_id = {
            let mut primary = self.primary_id.write();
            primary.take()
        };

        if let Some(old_id) = old_primary_id {
            if let Some(mut old_primary) = self.replicas.get_mut(&old_id) {
                old_primary.role = ReplicaRole::Secondary;
            }
        }

        // Promote new primary
        replica.role = ReplicaRole::Primary;
        let mut primary = self.primary_id.write();
        *primary = Some(id.to_string());

        tracing::info!("Promoted replica {} to primary", id);
        Ok(())
    }

    /// Demote a primary to secondary
    pub fn demote_to_secondary(&mut self, id: &str) -> Result<()> {
        let mut replica = self
            .replicas
            .get_mut(id)
            .ok_or_else(|| ReplicationError::ReplicaNotFound(id.to_string()))?;

        if replica.role != ReplicaRole::Primary {
            return Ok(());
        }

        replica.role = ReplicaRole::Secondary;
        let mut primary = self.primary_id.write();
        *primary = None;

        tracing::info!("Demoted replica {} to secondary", id);
        Ok(())
    }

    /// Check if quorum is available
    pub fn has_quorum(&self) -> bool {
        let healthy_count = self
            .replicas
            .iter()
            .filter(|r| r.is_healthy() && r.role != ReplicaRole::Witness)
            .count();
        let quorum = *self.quorum_size.read();
        healthy_count >= quorum
    }

    /// Get the required quorum size
    pub fn get_quorum_size(&self) -> usize {
        *self.quorum_size.read()
    }

    /// Set the quorum size
    pub fn set_quorum_size(&self, size: usize) {
        *self.quorum_size.write() = size;
    }

    /// Update quorum size based on replica count
    fn update_quorum_size(&self) {
        let replica_count = self
            .replicas
            .iter()
            .filter(|r| r.role != ReplicaRole::Witness)
            .count();
        let quorum = (replica_count / 2) + 1;
        *self.quorum_size.write() = quorum;
    }

    /// Get all replica IDs
    pub fn replica_ids(&self) -> Vec<String> {
        self.replicas.iter().map(|r| r.id.clone()).collect()
    }

    /// Get replica count
    pub fn replica_count(&self) -> usize {
        self.replicas.len()
    }

    /// Get the cluster ID
    pub fn cluster_id(&self) -> &str {
        &self.cluster_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replica_creation() {
        let replica = Replica::new("r1", "127.0.0.1:9001", ReplicaRole::Primary);
        assert_eq!(replica.id, "r1");
        assert_eq!(replica.role, ReplicaRole::Primary);
        assert!(replica.is_healthy());
        assert!(replica.is_writable());
    }

    #[test]
    fn test_replica_set() {
        let mut set = ReplicaSet::new("cluster-1");
        set.add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        set.add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();

        assert_eq!(set.replica_count(), 2);
        assert!(set.get_primary().is_some());
        assert_eq!(set.get_secondaries().len(), 1);
    }

    #[test]
    fn test_promotion() {
        let mut set = ReplicaSet::new("cluster-1");
        set.add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        set.add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();

        set.promote_to_primary("r2").unwrap();

        let primary = set.get_primary().unwrap();
        assert_eq!(primary.id, "r2");
        assert_eq!(primary.role, ReplicaRole::Primary);
    }

    #[test]
    fn test_quorum() {
        let mut set = ReplicaSet::new("cluster-1");
        set.add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        set.add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();
        set.add_replica("r3", "127.0.0.1:9003", ReplicaRole::Secondary)
            .unwrap();

        assert_eq!(set.get_quorum_size(), 2);
        assert!(set.has_quorum());
    }
}
