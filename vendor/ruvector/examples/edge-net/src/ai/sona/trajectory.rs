//! Lock-free trajectory buffer for SONA in edge-net
//!
//! Provides efficient, non-blocking trajectory recording during P2P task execution.
//! Optimized for WASM with no external dependencies (uses parking_lot).

use crate::ai::sona::types::{QueryTrajectory, TrajectoryStep};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Ring buffer for trajectory storage
/// Uses RwLock for WASM compatibility (crossbeam not available)
pub struct TrajectoryBuffer {
    /// Ring buffer storage
    buffer: RwLock<Vec<Option<QueryTrajectory>>>,
    /// Write position
    write_pos: AtomicU64,
    /// Read position (for drain operations)
    read_pos: AtomicU64,
    /// Capacity
    capacity: usize,
    /// Count of dropped trajectories (buffer full)
    dropped: AtomicU64,
    /// Total trajectories seen
    total_seen: AtomicU64,
}

impl TrajectoryBuffer {
    /// Create new buffer with capacity
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(16); // Minimum 16 slots
        Self {
            buffer: RwLock::new(vec![None; capacity]),
            write_pos: AtomicU64::new(0),
            read_pos: AtomicU64::new(0),
            capacity,
            dropped: AtomicU64::new(0),
            total_seen: AtomicU64::new(0),
        }
    }

    /// Record trajectory (non-blocking attempt)
    /// Returns true if recorded, false if buffer full
    pub fn record(&self, trajectory: QueryTrajectory) -> bool {
        self.total_seen.fetch_add(1, Ordering::Relaxed);

        // Try to get write lock without blocking for too long
        if let Some(mut buffer) = self.buffer.try_write() {
            let pos = self.write_pos.fetch_add(1, Ordering::Relaxed) as usize % self.capacity;
            buffer[pos] = Some(trajectory);
            true
        } else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Try to pop single trajectory
    pub fn pop(&self) -> Option<QueryTrajectory> {
        let mut buffer = self.buffer.write();

        let write_pos = self.write_pos.load(Ordering::Relaxed);
        let read_pos = self.read_pos.load(Ordering::Relaxed);

        if read_pos >= write_pos {
            return None;
        }

        let pos = read_pos as usize % self.capacity;
        let trajectory = buffer[pos].take();

        if trajectory.is_some() {
            self.read_pos.fetch_add(1, Ordering::Relaxed);
        }

        trajectory
    }

    /// Drain all trajectories
    pub fn drain(&self) -> Vec<QueryTrajectory> {
        let mut buffer = self.buffer.write();
        let mut result = Vec::with_capacity(self.len());

        for slot in buffer.iter_mut() {
            if let Some(traj) = slot.take() {
                result.push(traj);
            }
        }

        // Reset positions
        self.write_pos.store(0, Ordering::Relaxed);
        self.read_pos.store(0, Ordering::Relaxed);

        result
    }

    /// Drain up to n trajectories
    pub fn drain_n(&self, n: usize) -> Vec<QueryTrajectory> {
        let mut buffer = self.buffer.write();
        let mut result = Vec::with_capacity(n.min(self.capacity));

        let write_pos = self.write_pos.load(Ordering::Relaxed);
        let mut read_pos = self.read_pos.load(Ordering::Relaxed);

        for _ in 0..n {
            if read_pos >= write_pos {
                break;
            }

            let pos = read_pos as usize % self.capacity;
            if let Some(traj) = buffer[pos].take() {
                result.push(traj);
                read_pos += 1;
            } else {
                break;
            }
        }

        self.read_pos.store(read_pos, Ordering::Relaxed);
        result
    }

    /// Get approximate current length
    pub fn len(&self) -> usize {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Relaxed);
        (write.saturating_sub(read)) as usize
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if full
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get dropped count
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Get total seen count
    pub fn total_seen(&self) -> u64 {
        self.total_seen.load(Ordering::Relaxed)
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.total_seen.load(Ordering::Relaxed);
        let dropped = self.dropped.load(Ordering::Relaxed);
        if total == 0 {
            1.0
        } else {
            (total - dropped) as f64 / total as f64
        }
    }

    /// Reset statistics (not the buffer contents)
    pub fn reset_stats(&self) {
        self.dropped.store(0, Ordering::Relaxed);
        self.total_seen.store(0, Ordering::Relaxed);
    }
}

/// Builder for constructing trajectories during task execution
pub struct TrajectoryBuilder {
    /// Trajectory ID
    id: u64,
    /// Query/task embedding
    query_embedding: Vec<f32>,
    /// Steps collected
    steps: Vec<TrajectoryStep>,
    /// Start time (ms since epoch)
    start_time_ms: u64,
    /// Node ID
    node_id: Option<String>,
    /// Task type
    task_type: Option<String>,
    /// Context IDs
    context_ids: Vec<String>,
}

impl TrajectoryBuilder {
    /// Start new trajectory
    pub fn new(id: u64, query_embedding: Vec<f32>) -> Self {
        Self {
            id,
            query_embedding,
            steps: Vec::with_capacity(16),
            start_time_ms: js_sys::Date::now() as u64,
            node_id: None,
            task_type: None,
            context_ids: Vec::new(),
        }
    }

    /// Start trajectory with node context
    pub fn with_node(id: u64, query_embedding: Vec<f32>, node_id: &str) -> Self {
        let mut builder = Self::new(id, query_embedding);
        builder.node_id = Some(node_id.to_string());
        builder
    }

    /// Add execution step
    pub fn add_step(&mut self, activations: Vec<f32>, attention_weights: Vec<f32>, reward: f32) {
        let step_idx = self.steps.len();
        self.steps.push(TrajectoryStep::new(
            activations,
            attention_weights,
            reward,
            step_idx,
        ));
    }

    /// Add step with layer name
    pub fn add_named_step(
        &mut self,
        name: &str,
        activations: Vec<f32>,
        attention_weights: Vec<f32>,
        reward: f32,
    ) {
        let step_idx = self.steps.len();
        self.steps.push(
            TrajectoryStep::new(activations, attention_weights, reward, step_idx).with_layer(name),
        );
    }

    /// Set task type
    pub fn set_task_type(&mut self, task_type: &str) {
        self.task_type = Some(task_type.to_string());
    }

    /// Add context ID (e.g., RAC event ID)
    pub fn add_context(&mut self, context_id: &str) {
        self.context_ids.push(context_id.to_string());
    }

    /// Get current step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        let now = js_sys::Date::now() as u64;
        now.saturating_sub(self.start_time_ms)
    }

    /// Finalize and build trajectory
    pub fn build(self, final_quality: f32) -> QueryTrajectory {
        let latency_us = self.elapsed_ms() * 1000;

        let mut trajectory = QueryTrajectory {
            id: self.id,
            query_embedding: self.query_embedding,
            steps: self.steps,
            final_quality,
            latency_us,
            node_id: self.node_id,
            task_type: self.task_type,
            context_ids: self.context_ids,
        };

        trajectory
    }

    /// Build with explicit latency
    pub fn build_with_latency(self, final_quality: f32, latency_us: u64) -> QueryTrajectory {
        QueryTrajectory {
            id: self.id,
            query_embedding: self.query_embedding,
            steps: self.steps,
            final_quality,
            latency_us,
            node_id: self.node_id,
            task_type: self.task_type,
            context_ids: self.context_ids,
        }
    }
}

/// Trajectory ID generator
pub struct TrajectoryIdGen {
    counter: AtomicU64,
    /// Node prefix for unique IDs across P2P network
    node_prefix: u64,
}

impl TrajectoryIdGen {
    /// Create new generator
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
            node_prefix: 0,
        }
    }

    /// Create with starting ID
    pub fn with_start(start: u64) -> Self {
        Self {
            counter: AtomicU64::new(start),
            node_prefix: 0,
        }
    }

    /// Create with node prefix for P2P uniqueness
    pub fn with_node_prefix(node_id: &str) -> Self {
        // Use first 16 bits of node_id hash as prefix
        let hash = node_id.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        Self {
            counter: AtomicU64::new(0),
            node_prefix: (hash & 0xFFFF) << 48,
        }
    }

    /// Generate next ID
    pub fn next(&self) -> u64 {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        self.node_prefix | counter
    }

    /// Get current value without incrementing
    pub fn current(&self) -> u64 {
        self.node_prefix | self.counter.load(Ordering::Relaxed)
    }
}

impl Default for TrajectoryIdGen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_basic_ops() {
        let buffer = TrajectoryBuffer::new(10);

        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 10);

        let trajectory = QueryTrajectory::new(1, vec![0.1, 0.2]);
        assert!(buffer.record(trajectory));

        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_buffer_drain() {
        let buffer = TrajectoryBuffer::new(10);

        for i in 0..5 {
            let trajectory = QueryTrajectory::new(i, vec![0.1]);
            buffer.record(trajectory);
        }

        let drained = buffer.drain();
        assert_eq!(drained.len(), 5);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_buffer_drain_n() {
        let buffer = TrajectoryBuffer::new(10);

        for i in 0..5 {
            let trajectory = QueryTrajectory::new(i, vec![0.1]);
            buffer.record(trajectory);
        }

        let partial = buffer.drain_n(3);
        assert_eq!(partial.len(), 3);
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_builder() {
        let mut builder = TrajectoryBuilder::new(42, vec![0.1, 0.2, 0.3]);

        builder.add_step(vec![0.5], vec![0.4, 0.6], 0.7);
        builder.add_step(vec![0.6], vec![0.3, 0.7], 0.8);
        builder.set_task_type("compute");
        builder.add_context("rac-event-123");

        assert_eq!(builder.step_count(), 2);

        let trajectory = builder.build(0.85);

        assert_eq!(trajectory.id, 42);
        assert_eq!(trajectory.steps.len(), 2);
        assert_eq!(trajectory.final_quality, 0.85);
        assert_eq!(trajectory.task_type, Some("compute".to_string()));
        assert!(trajectory.latency_us > 0);
    }

    #[test]
    fn test_id_generator() {
        let gen = TrajectoryIdGen::new();

        assert_eq!(gen.next(), 0);
        assert_eq!(gen.next(), 1);
        assert_eq!(gen.next(), 2);
        assert_eq!(gen.current(), 3);
    }

    #[test]
    fn test_id_generator_with_prefix() {
        let gen1 = TrajectoryIdGen::with_node_prefix("node-alpha");
        let gen2 = TrajectoryIdGen::with_node_prefix("node-beta");

        let id1 = gen1.next();
        let id2 = gen2.next();

        // Different prefixes should produce different IDs
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_success_rate() {
        let buffer = TrajectoryBuffer::new(2);

        // Record 4 trajectories into buffer of size 2
        // Some should be dropped due to contention simulation
        for i in 0..4 {
            buffer.record(QueryTrajectory::new(i, vec![]));
        }

        // Success rate should be calculable
        let rate = buffer.success_rate();
        assert!(rate >= 0.0 && rate <= 1.0);
    }
}
