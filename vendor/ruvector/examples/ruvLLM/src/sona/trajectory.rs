//! Lock-free trajectory buffer for SONA
//!
//! Provides efficient, non-blocking trajectory recording during inference.

use crate::sona::types::{QueryTrajectory, TrajectoryStep};
use crossbeam::queue::ArrayQueue;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Lock-free trajectory buffer using crossbeam ArrayQueue
pub struct TrajectoryBuffer {
    /// Internal queue
    buffer: ArrayQueue<QueryTrajectory>,
    /// Capacity
    capacity: usize,
    /// Count of dropped trajectories
    dropped: AtomicU64,
    /// Total trajectories seen
    total_seen: AtomicU64,
}

impl TrajectoryBuffer {
    /// Create new buffer with capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: ArrayQueue::new(capacity),
            capacity,
            dropped: AtomicU64::new(0),
            total_seen: AtomicU64::new(0),
        }
    }

    /// Record trajectory (non-blocking)
    ///
    /// Returns true if recorded, false if buffer full
    pub fn record(&self, trajectory: QueryTrajectory) -> bool {
        self.total_seen.fetch_add(1, Ordering::Relaxed);

        match self.buffer.push(trajectory) {
            Ok(()) => true,
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Try to pop single trajectory
    pub fn pop(&self) -> Option<QueryTrajectory> {
        self.buffer.pop()
    }

    /// Drain all trajectories
    pub fn drain(&self) -> Vec<QueryTrajectory> {
        let mut result = Vec::with_capacity(self.len());
        while let Some(t) = self.buffer.pop() {
            result.push(t);
        }
        result
    }

    /// Drain up to n trajectories
    pub fn drain_n(&self, n: usize) -> Vec<QueryTrajectory> {
        let mut result = Vec::with_capacity(n.min(self.len()));
        for _ in 0..n {
            match self.buffer.pop() {
                Some(t) => result.push(t),
                None => break,
            }
        }
        result
    }

    /// Get current length
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Check if full
    pub fn is_full(&self) -> bool {
        self.buffer.is_full()
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

/// Builder for constructing trajectories during inference
pub struct TrajectoryBuilder {
    /// Trajectory ID
    id: u64,
    /// Query embedding
    query_embedding: Vec<f32>,
    /// Steps collected
    steps: Vec<TrajectoryStep>,
    /// Start time
    start_time: Instant,
    /// Model route
    model_route: Option<String>,
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
            start_time: Instant::now(),
            model_route: None,
            context_ids: Vec::new(),
        }
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

    /// Set model route
    pub fn set_model_route(&mut self, route: &str) {
        self.model_route = Some(route.to_string());
    }

    /// Add context ID
    pub fn add_context(&mut self, context_id: &str) {
        self.context_ids.push(context_id.to_string());
    }

    /// Get current step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Finalize and build trajectory
    pub fn build(self, final_quality: f32) -> QueryTrajectory {
        let latency_us = self.start_time.elapsed().as_micros() as u64;

        QueryTrajectory {
            id: self.id,
            query_embedding: self.query_embedding,
            steps: self.steps,
            final_quality,
            latency_us,
            model_route: self.model_route,
            context_ids: self.context_ids,
        }
    }

    /// Build with explicit latency
    pub fn build_with_latency(self, final_quality: f32, latency_us: u64) -> QueryTrajectory {
        QueryTrajectory {
            id: self.id,
            query_embedding: self.query_embedding,
            steps: self.steps,
            final_quality,
            latency_us,
            model_route: self.model_route,
            context_ids: self.context_ids,
        }
    }
}

/// Trajectory ID generator
pub struct TrajectoryIdGen {
    counter: AtomicU64,
}

impl TrajectoryIdGen {
    /// Create new generator
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Create with starting ID
    pub fn with_start(start: u64) -> Self {
        Self {
            counter: AtomicU64::new(start),
        }
    }

    /// Generate next ID
    pub fn next(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Get current value without incrementing
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
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
    fn test_buffer_overflow() {
        let buffer = TrajectoryBuffer::new(3);

        for i in 0..5 {
            let trajectory = QueryTrajectory::new(i, vec![0.1]);
            buffer.record(trajectory);
        }

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.dropped_count(), 2);
        assert_eq!(buffer.total_seen(), 5);
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
        builder.set_model_route("llama-7b");
        builder.add_context("ctx-123");

        assert_eq!(builder.step_count(), 2);

        let trajectory = builder.build(0.85);

        assert_eq!(trajectory.id, 42);
        assert_eq!(trajectory.steps.len(), 2);
        assert_eq!(trajectory.final_quality, 0.85);
        assert_eq!(trajectory.model_route, Some("llama-7b".to_string()));
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
    fn test_success_rate() {
        let buffer = TrajectoryBuffer::new(2);

        for i in 0..4 {
            buffer.record(QueryTrajectory::new(i, vec![]));
        }

        assert!((buffer.success_rate() - 0.5).abs() < 1e-6);
    }
}
