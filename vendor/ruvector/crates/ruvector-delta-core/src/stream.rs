//! Delta stream for event sourcing and temporal queries
//!
//! Provides ordered sequences of deltas with checkpointing,
//! compaction, and replay capabilities.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::time::Duration;

use crate::delta::{Delta, VectorDelta};
use crate::error::{DeltaError, Result};

/// Configuration for delta streams
#[derive(Debug, Clone)]
pub struct DeltaStreamConfig {
    /// Maximum number of deltas before automatic compaction
    pub max_deltas: usize,
    /// Checkpoint interval (in number of deltas)
    pub checkpoint_interval: usize,
    /// Maximum memory usage before eviction
    pub max_memory_bytes: usize,
    /// Enable automatic compaction
    pub auto_compact: bool,
}

impl Default for DeltaStreamConfig {
    fn default() -> Self {
        Self {
            max_deltas: 1000,
            checkpoint_interval: 100,
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB
            auto_compact: true,
        }
    }
}

/// A checkpoint in the delta stream
#[derive(Debug, Clone)]
pub struct StreamCheckpoint<T> {
    /// The base value at this checkpoint
    pub value: T,
    /// Sequence number of this checkpoint
    pub sequence: u64,
    /// Timestamp when created (nanoseconds since epoch)
    pub timestamp_ns: u64,
}

/// Entry in the delta stream
#[derive(Debug, Clone)]
struct StreamEntry<D: Clone> {
    /// The delta
    delta: D,
    /// Sequence number
    sequence: u64,
    /// Timestamp (nanoseconds)
    timestamp_ns: u64,
}

/// A stream of deltas with event sourcing capabilities
#[derive(Debug, Clone)]
pub struct DeltaStream<D: Delta>
where
    D: Clone,
    D::Base: Clone,
{
    /// Configuration
    config: DeltaStreamConfig,
    /// Ordered deltas
    deltas: VecDeque<StreamEntry<D>>,
    /// Checkpoints
    checkpoints: Vec<StreamCheckpoint<D::Base>>,
    /// Current sequence number
    current_sequence: u64,
    /// Memory usage estimate
    memory_usage: usize,
}

impl<D: Delta + Clone> DeltaStream<D>
where
    D::Base: Clone,
{
    /// Create a new delta stream with default configuration
    pub fn new() -> Self {
        Self::with_config(DeltaStreamConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: DeltaStreamConfig) -> Self {
        Self {
            config,
            deltas: VecDeque::new(),
            checkpoints: Vec::new(),
            current_sequence: 0,
            memory_usage: 0,
        }
    }

    /// Get current configuration
    pub fn config(&self) -> &DeltaStreamConfig {
        &self.config
    }

    /// Get the current sequence number
    pub fn sequence(&self) -> u64 {
        self.current_sequence
    }

    /// Get the number of deltas in the stream
    pub fn len(&self) -> usize {
        self.deltas.len()
    }

    /// Check if the stream is empty
    pub fn is_empty(&self) -> bool {
        self.deltas.is_empty()
    }

    /// Get the number of checkpoints
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Push a new delta to the stream
    pub fn push(&mut self, delta: D) {
        self.push_with_timestamp(delta, Self::current_timestamp_ns());
    }

    /// Push a delta with a specific timestamp
    pub fn push_with_timestamp(&mut self, delta: D, timestamp_ns: u64) {
        self.current_sequence += 1;

        let entry = StreamEntry {
            delta,
            sequence: self.current_sequence,
            timestamp_ns,
        };

        self.memory_usage += entry.delta.byte_size();
        self.deltas.push_back(entry);

        // Check if compaction is needed
        if self.config.auto_compact && self.needs_compaction() {
            let _ = self.compact();
        }
    }

    /// Create a checkpoint at the current position
    pub fn create_checkpoint(&mut self, value: D::Base) {
        let checkpoint = StreamCheckpoint {
            value,
            sequence: self.current_sequence,
            timestamp_ns: Self::current_timestamp_ns(),
        };
        self.checkpoints.push(checkpoint);
    }

    /// Replay from the beginning to reconstruct the current state
    pub fn replay(&self, initial: D::Base) -> core::result::Result<D::Base, D::Error> {
        let mut current = initial;
        for entry in &self.deltas {
            entry.delta.apply(&mut current)?;
        }
        Ok(current)
    }

    /// Replay from a specific checkpoint
    ///
    /// Returns `None` if the checkpoint index is out of bounds, otherwise
    /// returns the result of replaying deltas from that checkpoint.
    pub fn replay_from_checkpoint(
        &self,
        checkpoint_idx: usize,
    ) -> Option<core::result::Result<D::Base, D::Error>> {
        if checkpoint_idx >= self.checkpoints.len() {
            return None;
        }

        let checkpoint = &self.checkpoints[checkpoint_idx];
        let mut current = checkpoint.value.clone();

        // Find deltas after this checkpoint
        for entry in &self.deltas {
            if entry.sequence > checkpoint.sequence {
                if let Err(e) = entry.delta.apply(&mut current) {
                    return Some(Err(e));
                }
            }
        }

        Some(Ok(current))
    }

    /// Replay to a specific sequence number
    pub fn replay_to_sequence(
        &self,
        initial: D::Base,
        target_sequence: u64,
    ) -> core::result::Result<D::Base, D::Error> {
        let mut current = initial;

        for entry in &self.deltas {
            if entry.sequence > target_sequence {
                break;
            }
            entry.delta.apply(&mut current)?;
        }

        Ok(current)
    }

    /// Get deltas in a sequence range
    pub fn get_range(&self, start: u64, end: u64) -> Vec<&D> {
        self.deltas
            .iter()
            .filter(|e| e.sequence >= start && e.sequence <= end)
            .map(|e| &e.delta)
            .collect()
    }

    /// Get deltas in a time range
    pub fn get_time_range(&self, start_ns: u64, end_ns: u64) -> Vec<&D> {
        self.deltas
            .iter()
            .filter(|e| e.timestamp_ns >= start_ns && e.timestamp_ns <= end_ns)
            .map(|e| &e.delta)
            .collect()
    }

    /// Check if compaction is needed
    pub fn needs_compaction(&self) -> bool {
        self.deltas.len() > self.config.max_deltas
            || self.memory_usage > self.config.max_memory_bytes
    }

    /// Compact the stream by composing consecutive deltas
    pub fn compact(&mut self) -> Result<usize> {
        if self.deltas.len() < 2 {
            return Ok(0);
        }

        // Find the latest checkpoint sequence
        let checkpoint_sequence = self.checkpoints.last().map(|c| c.sequence).unwrap_or(0);

        // Only compact deltas after the latest checkpoint
        let mut compacted = 0;
        let mut new_deltas: VecDeque<StreamEntry<D>> = VecDeque::new();
        let mut pending: Option<StreamEntry<D>> = None;

        for entry in self.deltas.drain(..) {
            if entry.sequence <= checkpoint_sequence {
                // Keep deltas at or before checkpoint as-is
                if let Some(p) = pending.take() {
                    new_deltas.push_back(p);
                }
                new_deltas.push_back(entry);
            } else if let Some(p) = pending.take() {
                // Compose with pending
                let composed = p.delta.compose(entry.delta.clone());
                if composed.is_identity() {
                    // They cancel out
                    compacted += 2;
                } else {
                    pending = Some(StreamEntry {
                        delta: composed,
                        sequence: entry.sequence,
                        timestamp_ns: entry.timestamp_ns,
                    });
                    compacted += 1;
                }
            } else {
                pending = Some(entry);
            }
        }

        if let Some(p) = pending {
            new_deltas.push_back(p);
        }

        let old_len = self.deltas.len();
        self.deltas = new_deltas;

        // Recalculate memory usage
        self.memory_usage = self.deltas.iter().map(|e| e.delta.byte_size()).sum();

        Ok(old_len.saturating_sub(self.deltas.len()))
    }

    /// Trim deltas before a sequence number
    pub fn trim_before(&mut self, sequence: u64) {
        while let Some(front) = self.deltas.front() {
            if front.sequence < sequence {
                if let Some(entry) = self.deltas.pop_front() {
                    self.memory_usage = self.memory_usage.saturating_sub(entry.delta.byte_size());
                }
            } else {
                break;
            }
        }

        // Also trim old checkpoints
        self.checkpoints.retain(|c| c.sequence >= sequence);
    }

    /// Clear all deltas and checkpoints
    pub fn clear(&mut self) {
        self.deltas.clear();
        self.checkpoints.clear();
        self.memory_usage = 0;
    }

    /// Get current timestamp in nanoseconds
    fn current_timestamp_ns() -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

impl<D: Delta> Default for DeltaStream<D>
where
    D::Base: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

// Implement for VectorDelta specifically
impl DeltaStream<VectorDelta> {
    /// Create a stream optimized for vector deltas
    pub fn for_vectors(dimensions: usize) -> Self {
        let estimated_delta_size = dimensions * 4; // Worst case: dense f32
        let max_deltas = (64 * 1024 * 1024) / estimated_delta_size;

        Self::with_config(DeltaStreamConfig {
            max_deltas,
            checkpoint_interval: max_deltas / 10,
            max_memory_bytes: 64 * 1024 * 1024,
            auto_compact: true,
        })
    }
}

/// Iterator over stream entries
pub struct DeltaStreamIter<'a, D: Clone> {
    inner: alloc::collections::vec_deque::Iter<'a, StreamEntry<D>>,
}

impl<'a, D: Clone> Iterator for DeltaStreamIter<'a, D> {
    type Item = (u64, &'a D);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| (e.sequence, &e.delta))
    }
}

impl<D: Delta + Clone> DeltaStream<D>
where
    D::Base: Clone,
{
    /// Iterate over deltas with their sequence numbers
    pub fn iter(&self) -> DeltaStreamIter<'_, D> {
        DeltaStreamIter {
            inner: self.deltas.iter(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::VectorDelta;

    #[test]
    fn test_stream_push_replay() {
        let mut stream = DeltaStream::<VectorDelta>::new();

        let initial = vec![1.0f32, 2.0, 3.0];

        let delta1 = VectorDelta::from_dense(vec![0.5, 0.0, 0.5]);
        let delta2 = VectorDelta::from_dense(vec![0.0, 1.0, 0.0]);

        stream.push(delta1);
        stream.push(delta2);

        let result = stream.replay(initial.clone()).unwrap();

        assert!((result[0] - 1.5).abs() < 1e-6);
        assert!((result[1] - 3.0).abs() < 1e-6);
        assert!((result[2] - 3.5).abs() < 1e-6);
    }

    #[test]
    fn test_stream_checkpoint() {
        let mut stream = DeltaStream::<VectorDelta>::new();

        let initial = vec![0.0f32; 3];
        let delta1 = VectorDelta::from_dense(vec![1.0, 1.0, 1.0]);
        stream.push(delta1);

        let state_at_checkpoint = stream.replay(initial.clone()).unwrap();
        stream.create_checkpoint(state_at_checkpoint);

        let delta2 = VectorDelta::from_dense(vec![2.0, 2.0, 2.0]);
        stream.push(delta2);

        let from_checkpoint = stream.replay_from_checkpoint(0).unwrap().unwrap();

        assert!((from_checkpoint[0] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_stream_sequence_range() {
        let mut stream = DeltaStream::<VectorDelta>::new();

        for i in 0..10 {
            let delta = VectorDelta::from_dense(vec![i as f32; 3]);
            stream.push(delta);
        }

        let range = stream.get_range(3, 7);
        assert_eq!(range.len(), 5);
    }

    #[test]
    fn test_replay_to_sequence() {
        let mut stream = DeltaStream::<VectorDelta>::new();
        let initial = vec![0.0f32; 3];

        stream.push(VectorDelta::from_dense(vec![1.0, 0.0, 0.0]));
        stream.push(VectorDelta::from_dense(vec![0.0, 1.0, 0.0]));
        stream.push(VectorDelta::from_dense(vec![0.0, 0.0, 1.0]));

        let at_seq_2 = stream.replay_to_sequence(initial, 2).unwrap();
        assert!((at_seq_2[0] - 1.0).abs() < 1e-6);
        assert!((at_seq_2[1] - 1.0).abs() < 1e-6);
        assert!((at_seq_2[2] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_stream_trim() {
        let mut stream = DeltaStream::<VectorDelta>::new();

        for _ in 0..10 {
            let delta = VectorDelta::from_dense(vec![1.0; 3]);
            stream.push(delta);
        }

        assert_eq!(stream.len(), 10);

        stream.trim_before(5);
        assert_eq!(stream.len(), 6); // Sequences 5-10
    }
}
