//! Synchronization modes and replication log management
//!
//! Provides different replication modes (sync, async, semi-sync)
//! and manages the replication log for tracking changes.

use crate::{ReplicaSet, ReplicationError, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

/// Synchronization mode for replication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
    /// Wait for all replicas to acknowledge
    Sync,
    /// Don't wait for replicas
    Async,
    /// Wait for a minimum number of replicas
    SemiSync { min_replicas: usize },
}

/// Entry in the replication log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique identifier for this entry
    pub id: Uuid,
    /// Sequence number in the log
    pub sequence: u64,
    /// Timestamp when the entry was created
    pub timestamp: DateTime<Utc>,
    /// The operation data (serialized)
    pub data: Vec<u8>,
    /// Checksum for data integrity
    pub checksum: u64,
    /// ID of the replica that originated this entry
    pub source_replica: String,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(sequence: u64, data: Vec<u8>, source_replica: String) -> Self {
        let checksum = Self::calculate_checksum(&data);
        Self {
            id: Uuid::new_v4(),
            sequence,
            timestamp: Utc::now(),
            data,
            checksum,
            source_replica,
        }
    }

    /// Calculate checksum for data
    fn calculate_checksum(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    /// Verify data integrity
    pub fn verify(&self) -> bool {
        Self::calculate_checksum(&self.data) == self.checksum
    }
}

/// Manages the replication log
pub struct ReplicationLog {
    /// Log entries indexed by sequence number
    entries: Arc<DashMap<u64, LogEntry>>,
    /// Current sequence number
    sequence: Arc<RwLock<u64>>,
    /// Replica ID
    replica_id: String,
}

impl ReplicationLog {
    /// Create a new replication log
    pub fn new(replica_id: impl Into<String>) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            sequence: Arc::new(RwLock::new(0)),
            replica_id: replica_id.into(),
        }
    }

    /// Append an entry to the log
    pub fn append(&self, data: Vec<u8>) -> LogEntry {
        let mut seq = self.sequence.write();
        *seq += 1;
        let entry = LogEntry::new(*seq, data, self.replica_id.clone());
        self.entries.insert(*seq, entry.clone());
        entry
    }

    /// Get an entry by sequence number
    pub fn get(&self, sequence: u64) -> Option<LogEntry> {
        self.entries.get(&sequence).map(|e| e.clone())
    }

    /// Get entries in a range
    pub fn get_range(&self, start: u64, end: u64) -> Vec<LogEntry> {
        let mut entries = Vec::new();
        for seq in start..=end {
            if let Some(entry) = self.entries.get(&seq) {
                entries.push(entry.clone());
            }
        }
        entries
    }

    /// Get the current sequence number
    pub fn current_sequence(&self) -> u64 {
        *self.sequence.read()
    }

    /// Get entries since a given sequence
    pub fn get_since(&self, since: u64) -> Vec<LogEntry> {
        let current = self.current_sequence();
        self.get_range(since + 1, current)
    }

    /// Truncate log before a given sequence
    pub fn truncate_before(&self, before: u64) {
        self.entries.retain(|seq, _| *seq >= before);
    }

    /// Get log size
    pub fn size(&self) -> usize {
        self.entries.len()
    }
}

/// Manages synchronization across replicas
pub struct SyncManager {
    /// The replica set
    replica_set: Arc<ReplicaSet>,
    /// Replication log
    log: Arc<ReplicationLog>,
    /// Synchronization mode
    sync_mode: Arc<RwLock<SyncMode>>,
    /// Timeout for synchronous operations
    sync_timeout: Duration,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(replica_set: Arc<ReplicaSet>, log: Arc<ReplicationLog>) -> Self {
        Self {
            replica_set,
            log,
            sync_mode: Arc::new(RwLock::new(SyncMode::Async)),
            sync_timeout: Duration::from_secs(5),
        }
    }

    /// Set the synchronization mode
    pub fn set_sync_mode(&self, mode: SyncMode) {
        *self.sync_mode.write() = mode;
    }

    /// Get the current synchronization mode
    pub fn sync_mode(&self) -> SyncMode {
        *self.sync_mode.read()
    }

    /// Set the sync timeout
    pub fn set_sync_timeout(&mut self, timeout: Duration) {
        self.sync_timeout = timeout;
    }

    /// Replicate data to all replicas according to sync mode
    pub async fn replicate(&self, data: Vec<u8>) -> Result<LogEntry> {
        // Append to local log
        let entry = self.log.append(data);

        // Get sync mode
        let mode = self.sync_mode();

        match mode {
            SyncMode::Sync => {
                self.replicate_sync(&entry).await?;
            }
            SyncMode::Async => {
                // Fire and forget
                let entry_clone = entry.clone();
                let replica_set = self.replica_set.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::send_to_replicas(&replica_set, &entry_clone).await {
                        tracing::error!("Async replication failed: {}", e);
                    }
                });
            }
            SyncMode::SemiSync { min_replicas } => {
                self.replicate_semi_sync(&entry, min_replicas).await?;
            }
        }

        Ok(entry)
    }

    /// Synchronous replication - wait for all replicas
    async fn replicate_sync(&self, entry: &LogEntry) -> Result<()> {
        timeout(
            self.sync_timeout,
            Self::send_to_replicas(&self.replica_set, entry),
        )
        .await
        .map_err(|_| ReplicationError::Timeout("Sync replication timed out".to_string()))?
    }

    /// Semi-synchronous replication - wait for minimum replicas
    async fn replicate_semi_sync(&self, entry: &LogEntry, min_replicas: usize) -> Result<()> {
        let secondaries = self.replica_set.get_secondaries();
        if secondaries.len() < min_replicas {
            return Err(ReplicationError::QuorumNotMet {
                needed: min_replicas,
                available: secondaries.len(),
            });
        }

        // Send to all and wait for min_replicas to respond
        let entry_clone = entry.clone();
        let replica_set = self.replica_set.clone();
        let min = min_replicas;

        timeout(self.sync_timeout, async move {
            // Simulate sending to replicas and waiting for acknowledgments
            // In a real implementation, this would use network calls
            let acks = secondaries.len().min(min);
            if acks >= min {
                Ok(())
            } else {
                Err(ReplicationError::QuorumNotMet {
                    needed: min,
                    available: acks,
                })
            }
        })
        .await
        .map_err(|_| ReplicationError::Timeout("Semi-sync replication timed out".to_string()))?
    }

    /// Send log entry to all replicas
    async fn send_to_replicas(replica_set: &ReplicaSet, entry: &LogEntry) -> Result<()> {
        let secondaries = replica_set.get_secondaries();

        // In a real implementation, this would send over the network
        // For now, we simulate successful replication
        for replica in secondaries {
            if replica.is_healthy() {
                tracing::debug!("Replicating entry {} to {}", entry.sequence, replica.id);
            }
        }

        Ok(())
    }

    /// Catch up a lagging replica
    pub async fn catchup(&self, replica_id: &str, from_sequence: u64) -> Result<Vec<LogEntry>> {
        let replica = self
            .replica_set
            .get_replica(replica_id)
            .ok_or_else(|| ReplicationError::ReplicaNotFound(replica_id.to_string()))?;

        let current_sequence = self.log.current_sequence();
        if from_sequence >= current_sequence {
            return Ok(Vec::new());
        }

        // Get missing entries
        let entries = self.log.get_since(from_sequence);

        tracing::info!(
            "Catching up replica {} with {} entries (from {} to {})",
            replica_id,
            entries.len(),
            from_sequence + 1,
            current_sequence
        );

        Ok(entries)
    }

    /// Get the current log position
    pub fn current_position(&self) -> u64 {
        self.log.current_sequence()
    }

    /// Verify log entry integrity
    pub fn verify_entry(&self, sequence: u64) -> Result<bool> {
        let entry = self
            .log
            .get(sequence)
            .ok_or_else(|| ReplicationError::InvalidState("Log entry not found".to_string()))?;
        Ok(entry.verify())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReplicaRole;

    #[test]
    fn test_log_entry_creation() {
        let data = b"test data".to_vec();
        let entry = LogEntry::new(1, data, "replica-1".to_string());
        assert_eq!(entry.sequence, 1);
        assert!(entry.verify());
    }

    #[test]
    fn test_replication_log() {
        let log = ReplicationLog::new("replica-1");

        let entry1 = log.append(b"data1".to_vec());
        let entry2 = log.append(b"data2".to_vec());

        assert_eq!(entry1.sequence, 1);
        assert_eq!(entry2.sequence, 2);
        assert_eq!(log.current_sequence(), 2);

        let entries = log.get_range(1, 2);
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_sync_manager() {
        let mut replica_set = ReplicaSet::new("cluster-1");
        replica_set
            .add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        replica_set
            .add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();

        let log = Arc::new(ReplicationLog::new("r1"));
        let manager = SyncManager::new(Arc::new(replica_set), log);

        manager.set_sync_mode(SyncMode::Async);
        let entry = manager.replicate(b"test".to_vec()).await.unwrap();
        assert_eq!(entry.sequence, 1);
    }

    #[tokio::test]
    async fn test_catchup() {
        let mut replica_set = ReplicaSet::new("cluster-1");
        replica_set
            .add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
            .unwrap();
        replica_set
            .add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
            .unwrap();

        let log = Arc::new(ReplicationLog::new("r1"));
        let manager = SyncManager::new(Arc::new(replica_set), log.clone());

        // Add some entries
        log.append(b"data1".to_vec());
        log.append(b"data2".to_vec());
        log.append(b"data3".to_vec());

        // Catchup from position 1
        let entries = manager.catchup("r2", 1).await.unwrap();
        assert_eq!(entries.len(), 2); // Entries 2 and 3
    }
}
