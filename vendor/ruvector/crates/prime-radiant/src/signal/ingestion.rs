//! Signal ingestion service.

use crate::types::{Hash, NodeId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A signal representing an incoming event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// Unique signal ID for idempotency
    pub id: Hash,
    /// Type of signal (e.g., "observation", "update", "correction")
    pub signal_type: String,
    /// Target node (if applicable)
    pub target_node: Option<NodeId>,
    /// Signal payload as JSON
    pub payload: serde_json::Value,
    /// Source of the signal
    pub source: String,
    /// Timestamp of signal generation
    pub timestamp: Timestamp,
}

impl Signal {
    /// Create a new signal.
    pub fn new(
        signal_type: impl Into<String>,
        payload: serde_json::Value,
        source: impl Into<String>,
    ) -> Self {
        let signal_type = signal_type.into();
        let source = source.into();

        // Generate ID from content
        let content = serde_json::json!({
            "type": signal_type,
            "payload": payload,
            "source": source,
        });
        let id = Hash::digest(content.to_string().as_bytes());

        Self {
            id,
            signal_type,
            target_node: None,
            payload,
            source,
            timestamp: Timestamp::now(),
        }
    }

    /// Set the target node.
    pub fn with_target(mut self, node_id: NodeId) -> Self {
        self.target_node = Some(node_id);
        self
    }
}

/// A batch of signals to be processed together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalBatch {
    /// Signals in the batch
    pub signals: Vec<Signal>,
    /// Batch creation timestamp
    pub created_at: Timestamp,
}

impl SignalBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self {
            signals: Vec::new(),
            created_at: Timestamp::now(),
        }
    }

    /// Add a signal to the batch.
    pub fn add(&mut self, signal: Signal) {
        self.signals.push(signal);
    }

    /// Get the number of signals.
    pub fn len(&self) -> usize {
        self.signals.len()
    }

    /// Check if batch is empty.
    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }
}

impl Default for SignalBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for ingesting signals.
pub struct SignalIngestion {
    /// Buffer for batching signals
    buffer: VecDeque<Signal>,
    /// Maximum batch size
    max_batch_size: usize,
    /// Set of processed signal IDs (for deduplication)
    processed_ids: std::collections::HashSet<Hash>,
}

impl SignalIngestion {
    /// Create a new ingestion service.
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            max_batch_size,
            processed_ids: std::collections::HashSet::new(),
        }
    }

    /// Ingest a signal.
    ///
    /// Returns true if the signal was accepted, false if it was a duplicate.
    pub fn ingest(&mut self, signal: Signal) -> bool {
        // Check for duplicates
        if self.processed_ids.contains(&signal.id) {
            return false;
        }

        self.processed_ids.insert(signal.id);
        self.buffer.push_back(signal);
        true
    }

    /// Get the next batch of signals if available.
    pub fn next_batch(&mut self) -> Option<SignalBatch> {
        if self.buffer.is_empty() {
            return None;
        }

        let mut batch = SignalBatch::new();
        while batch.len() < self.max_batch_size {
            if let Some(signal) = self.buffer.pop_front() {
                batch.add(signal);
            } else {
                break;
            }
        }

        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }

    /// Get the number of buffered signals.
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Clear the processed IDs set (for memory management).
    pub fn clear_processed_ids(&mut self) {
        self.processed_ids.clear();
    }
}

impl Default for SignalIngestion {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_creation() {
        let signal = Signal::new(
            "observation",
            serde_json::json!({"value": 42}),
            "test-source",
        );

        assert_eq!(signal.signal_type, "observation");
        assert_eq!(signal.source, "test-source");
    }

    #[test]
    fn test_duplicate_rejection() {
        let mut ingestion = SignalIngestion::new(10);

        let signal = Signal::new("test", serde_json::json!({}), "source");
        let signal_clone = signal.clone();

        assert!(ingestion.ingest(signal));
        assert!(!ingestion.ingest(signal_clone)); // Duplicate
    }

    #[test]
    fn test_batching() {
        let mut ingestion = SignalIngestion::new(2);

        for i in 0..5 {
            let signal = Signal::new("test", serde_json::json!({"i": i}), "source");
            ingestion.ingest(signal);
        }

        let batch1 = ingestion.next_batch().unwrap();
        assert_eq!(batch1.len(), 2);

        let batch2 = ingestion.next_batch().unwrap();
        assert_eq!(batch2.len(), 2);

        let batch3 = ingestion.next_batch().unwrap();
        assert_eq!(batch3.len(), 1);

        assert!(ingestion.next_batch().is_none());
    }
}
