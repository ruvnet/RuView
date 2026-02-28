//! Trajectory Buffer: Lock-free buffer for learning trajectories

use crossbeam::queue::ArrayQueue;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A single learning trajectory
#[derive(Debug, Clone)]
pub struct DagTrajectory {
    pub query_hash: u64,
    pub dag_embedding: Vec<f32>,
    pub attention_mechanism: String,
    pub execution_time_ms: f64,
    pub improvement_ratio: f32,
    pub timestamp: std::time::Instant,
}

impl DagTrajectory {
    pub fn new(
        query_hash: u64,
        dag_embedding: Vec<f32>,
        attention_mechanism: String,
        execution_time_ms: f64,
        baseline_time_ms: f64,
    ) -> Self {
        let improvement_ratio = if baseline_time_ms > 0.0 {
            (baseline_time_ms - execution_time_ms) as f32 / baseline_time_ms as f32
        } else {
            0.0
        };

        Self {
            query_hash,
            dag_embedding,
            attention_mechanism,
            execution_time_ms,
            improvement_ratio,
            timestamp: std::time::Instant::now(),
        }
    }

    /// Compute quality score (0-1)
    pub fn quality(&self) -> f32 {
        // Quality based on improvement and execution time
        let time_score = 1.0 / (1.0 + self.execution_time_ms as f32 / 1000.0);
        let improvement_score = (self.improvement_ratio + 1.0) / 2.0;
        0.5 * time_score + 0.5 * improvement_score
    }
}

/// Lock-free trajectory buffer
pub struct DagTrajectoryBuffer {
    queue: ArrayQueue<DagTrajectory>,
    count: AtomicUsize,
    #[allow(dead_code)]
    capacity: usize,
}

impl DagTrajectoryBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: ArrayQueue::new(capacity),
            count: AtomicUsize::new(0),
            capacity,
        }
    }

    /// Push trajectory, dropping oldest if full
    pub fn push(&self, trajectory: DagTrajectory) {
        if self.queue.push(trajectory.clone()).is_err() {
            // Queue full, pop oldest and retry
            let _ = self.queue.pop();
            let _ = self.queue.push(trajectory);
        }
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Drain all trajectories for processing
    pub fn drain(&self) -> Vec<DagTrajectory> {
        let mut result = Vec::with_capacity(self.queue.len());
        while let Some(t) = self.queue.pop() {
            result.push(t);
        }
        result
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}
