//! Change data capture and streaming for replication
//!
//! Provides mechanisms for streaming changes from the replication log
//! with support for checkpointing, resumption, and backpressure handling.

use crate::{LogEntry, ReplicationError, ReplicationLog, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Type of change operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeOperation {
    /// Insert operation
    Insert,
    /// Update operation
    Update,
    /// Delete operation
    Delete,
    /// Bulk operation
    Bulk,
}

/// A change event in the replication stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    /// Unique identifier for this event
    pub id: Uuid,
    /// Sequence number in the stream
    pub sequence: u64,
    /// Timestamp of the change
    pub timestamp: DateTime<Utc>,
    /// Type of operation
    pub operation: ChangeOperation,
    /// Collection/table name
    pub collection: String,
    /// Document/vector ID affected
    pub document_id: String,
    /// Serialized data for the change
    pub data: Vec<u8>,
    /// Metadata for the change
    pub metadata: serde_json::Value,
}

impl ChangeEvent {
    /// Create a new change event
    pub fn new(
        sequence: u64,
        operation: ChangeOperation,
        collection: String,
        document_id: String,
        data: Vec<u8>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sequence,
            timestamp: Utc::now(),
            operation,
            collection,
            document_id,
            data,
            metadata: serde_json::Value::Null,
        }
    }

    /// Add metadata to the change event
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Convert from a log entry
    pub fn from_log_entry(
        entry: &LogEntry,
        operation: ChangeOperation,
        collection: String,
        document_id: String,
    ) -> Self {
        Self {
            id: entry.id,
            sequence: entry.sequence,
            timestamp: entry.timestamp,
            operation,
            collection,
            document_id,
            data: entry.data.clone(),
            metadata: serde_json::json!({
                "source_replica": entry.source_replica,
                "checksum": entry.checksum,
            }),
        }
    }
}

/// Checkpoint for resuming a replication stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Last processed sequence number
    pub sequence: u64,
    /// Timestamp of the checkpoint
    pub timestamp: DateTime<Utc>,
    /// Optional consumer group ID
    pub consumer_group: Option<String>,
    /// Consumer ID within the group
    pub consumer_id: String,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(sequence: u64, consumer_id: impl Into<String>) -> Self {
        Self {
            sequence,
            timestamp: Utc::now(),
            consumer_group: None,
            consumer_id: consumer_id.into(),
        }
    }

    /// Set the consumer group
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.consumer_group = Some(group.into());
        self
    }
}

/// Configuration for a replication stream
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size for the channel
    pub buffer_size: usize,
    /// Batch size for events
    pub batch_size: usize,
    /// Enable automatic checkpointing
    pub auto_checkpoint: bool,
    /// Checkpoint interval (number of events)
    pub checkpoint_interval: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            batch_size: 100,
            auto_checkpoint: true,
            checkpoint_interval: 100,
        }
    }
}

/// Manages a replication stream
pub struct ReplicationStream {
    /// The replication log
    log: Arc<ReplicationLog>,
    /// Stream configuration
    config: StreamConfig,
    /// Current checkpoint
    checkpoint: Arc<RwLock<Option<Checkpoint>>>,
    /// Consumer ID
    consumer_id: String,
}

impl ReplicationStream {
    /// Create a new replication stream
    pub fn new(log: Arc<ReplicationLog>, consumer_id: impl Into<String>) -> Self {
        Self {
            log,
            config: StreamConfig::default(),
            checkpoint: Arc::new(RwLock::new(None)),
            consumer_id: consumer_id.into(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        log: Arc<ReplicationLog>,
        consumer_id: impl Into<String>,
        config: StreamConfig,
    ) -> Self {
        Self {
            log,
            config,
            checkpoint: Arc::new(RwLock::new(None)),
            consumer_id: consumer_id.into(),
        }
    }

    /// Start streaming from a given position
    pub async fn stream_from(
        &self,
        start_sequence: u64,
    ) -> Result<mpsc::Receiver<Vec<ChangeEvent>>> {
        let (tx, rx) = mpsc::channel(self.config.buffer_size);

        let log = self.log.clone();
        let batch_size = self.config.batch_size;
        let checkpoint = self.checkpoint.clone();
        let auto_checkpoint = self.config.auto_checkpoint;
        let checkpoint_interval = self.config.checkpoint_interval;
        let consumer_id = self.consumer_id.clone();

        tokio::spawn(async move {
            let mut current_sequence = start_sequence;
            let mut events_since_checkpoint = 0;

            loop {
                // Get batch of entries
                let entries =
                    log.get_range(current_sequence + 1, current_sequence + batch_size as u64);

                if entries.is_empty() {
                    // No new entries, wait a bit
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                // Convert to change events
                let mut events = Vec::new();
                for entry in &entries {
                    // In a real implementation, we would decode the operation type
                    // from the entry data. For now, we use a placeholder.
                    let event = ChangeEvent::from_log_entry(
                        entry,
                        ChangeOperation::Update,
                        "default".to_string(),
                        Uuid::new_v4().to_string(),
                    );
                    events.push(event);
                }

                // Update current sequence
                if let Some(last_entry) = entries.last() {
                    current_sequence = last_entry.sequence;
                }

                // Send batch
                if tx.send(events).await.is_err() {
                    // Receiver dropped, stop streaming
                    break;
                }

                events_since_checkpoint += entries.len();

                // Auto-checkpoint if enabled
                if auto_checkpoint && events_since_checkpoint >= checkpoint_interval {
                    let cp = Checkpoint::new(current_sequence, consumer_id.clone());
                    *checkpoint.write() = Some(cp);
                    events_since_checkpoint = 0;
                }
            }
        });

        Ok(rx)
    }

    /// Resume streaming from the last checkpoint
    pub async fn resume(&self) -> Result<mpsc::Receiver<Vec<ChangeEvent>>> {
        let checkpoint = self.checkpoint.read();
        let start_sequence = checkpoint.as_ref().map(|cp| cp.sequence).unwrap_or(0);
        drop(checkpoint);

        self.stream_from(start_sequence).await
    }

    /// Get the current checkpoint
    pub fn get_checkpoint(&self) -> Option<Checkpoint> {
        self.checkpoint.read().clone()
    }

    /// Set a checkpoint manually
    pub fn set_checkpoint(&self, checkpoint: Checkpoint) {
        *self.checkpoint.write() = Some(checkpoint);
    }

    /// Clear the checkpoint
    pub fn clear_checkpoint(&self) {
        *self.checkpoint.write() = None;
    }
}

/// Manager for multiple replication streams (consumer groups)
pub struct StreamManager {
    /// The replication log
    log: Arc<ReplicationLog>,
    /// Active streams by consumer ID
    streams: Arc<RwLock<Vec<Arc<ReplicationStream>>>>,
}

impl StreamManager {
    /// Create a new stream manager
    pub fn new(log: Arc<ReplicationLog>) -> Self {
        Self {
            log,
            streams: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new stream for a consumer
    pub fn create_stream(&self, consumer_id: impl Into<String>) -> Arc<ReplicationStream> {
        let stream = Arc::new(ReplicationStream::new(self.log.clone(), consumer_id));
        self.streams.write().push(stream.clone());
        stream
    }

    /// Create a stream with custom configuration
    pub fn create_stream_with_config(
        &self,
        consumer_id: impl Into<String>,
        config: StreamConfig,
    ) -> Arc<ReplicationStream> {
        let stream = Arc::new(ReplicationStream::with_config(
            self.log.clone(),
            consumer_id,
            config,
        ));
        self.streams.write().push(stream.clone());
        stream
    }

    /// Get all active streams
    pub fn active_streams(&self) -> Vec<Arc<ReplicationStream>> {
        self.streams.read().clone()
    }

    /// Get the number of active streams
    pub fn stream_count(&self) -> usize {
        self.streams.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_event_creation() {
        let event = ChangeEvent::new(
            1,
            ChangeOperation::Insert,
            "vectors".to_string(),
            "doc-1".to_string(),
            b"data".to_vec(),
        );

        assert_eq!(event.sequence, 1);
        assert_eq!(event.operation, ChangeOperation::Insert);
        assert_eq!(event.collection, "vectors");
    }

    #[test]
    fn test_checkpoint() {
        let cp = Checkpoint::new(100, "consumer-1").with_group("group-1");

        assert_eq!(cp.sequence, 100);
        assert_eq!(cp.consumer_id, "consumer-1");
        assert_eq!(cp.consumer_group, Some("group-1".to_string()));
    }

    #[tokio::test]
    async fn test_replication_stream() {
        let log = Arc::new(ReplicationLog::new("replica-1"));

        // Add some entries
        log.append(b"data1".to_vec());
        log.append(b"data2".to_vec());
        log.append(b"data3".to_vec());

        let stream = ReplicationStream::new(log.clone(), "consumer-1");
        let mut rx = stream.stream_from(0).await.unwrap();

        // Receive events
        if let Some(events) = rx.recv().await {
            assert!(!events.is_empty());
        }
    }

    #[test]
    fn test_stream_manager() {
        let log = Arc::new(ReplicationLog::new("replica-1"));
        let manager = StreamManager::new(log);

        let stream1 = manager.create_stream("consumer-1");
        let stream2 = manager.create_stream("consumer-2");

        assert_eq!(manager.stream_count(), 2);
    }

    #[test]
    fn test_stream_config() {
        let config = StreamConfig {
            buffer_size: 2000,
            batch_size: 50,
            auto_checkpoint: false,
            checkpoint_interval: 200,
        };

        assert_eq!(config.buffer_size, 2000);
        assert_eq!(config.batch_size, 50);
        assert!(!config.auto_checkpoint);
    }
}
