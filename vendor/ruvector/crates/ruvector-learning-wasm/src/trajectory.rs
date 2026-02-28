//! Trajectory tracking for reinforcement learning
//!
//! Records execution trajectories for post-hoc learning and pattern analysis.

use wasm_bindgen::prelude::*;

/// A single trajectory recording
#[derive(Clone)]
pub struct Trajectory {
    /// Embedding at query start
    pub embedding: Vec<f32>,
    /// Operator type that was executed (0-16)
    pub operator_type: u8,
    /// Attention mechanism used
    pub attention_type: u8,
    /// Execution time in milliseconds
    pub execution_ms: f32,
    /// Baseline execution time (for comparison)
    pub baseline_ms: f32,
    /// Improvement ratio (baseline / actual - 1.0)
    pub improvement: f32,
    /// Timestamp (simulation time or wall clock)
    pub timestamp: u64,
}

impl Trajectory {
    /// Create a new trajectory
    pub fn new(
        embedding: Vec<f32>,
        operator_type: u8,
        attention_type: u8,
        execution_ms: f32,
        baseline_ms: f32,
    ) -> Self {
        let improvement = if execution_ms > 0.0 {
            (baseline_ms / execution_ms) - 1.0
        } else {
            0.0
        };

        Self {
            embedding,
            operator_type,
            attention_type,
            execution_ms,
            baseline_ms,
            improvement,
            timestamp: 0,
        }
    }

    /// Get quality score (0.0 - 1.0)
    pub fn quality(&self) -> f32 {
        // Quality based on improvement, saturating at 2x speedup
        ((self.improvement + 1.0) / 2.0).clamp(0.0, 1.0)
    }

    /// Check if this trajectory represents a success
    pub fn is_success(&self) -> bool {
        self.improvement > 0.0
    }

    /// Get the gradient direction for learning
    pub fn gradient(&self) -> Vec<f32> {
        if self.is_success() {
            // Positive improvement: reinforce this direction
            self.embedding.clone()
        } else {
            // Negative improvement: push away from this direction
            self.embedding.iter().map(|x| -x).collect()
        }
    }
}

/// Statistics for a collection of trajectories
#[derive(Clone, Default)]
pub struct TrajectoryStats {
    /// Total trajectory count
    pub count: u64,
    /// Mean improvement ratio
    pub mean_improvement: f32,
    /// Variance of improvement
    pub variance: f32,
    /// Best improvement seen
    pub best_improvement: f32,
    /// Success rate (positive improvement)
    pub success_rate: f32,
    /// Most common attention type
    pub best_attention: u8,
}

impl TrajectoryStats {
    /// Update stats with a new trajectory
    pub fn update(&mut self, trajectory: &Trajectory) {
        let n = self.count as f32;
        let new_n = n + 1.0;

        // Welford's online algorithm for mean and variance
        let delta = trajectory.improvement - self.mean_improvement;
        self.mean_improvement += delta / new_n;
        let delta2 = trajectory.improvement - self.mean_improvement;
        self.variance += delta * delta2;

        // Update best
        if trajectory.improvement > self.best_improvement {
            self.best_improvement = trajectory.improvement;
            self.best_attention = trajectory.attention_type;
        }

        // Update success rate
        let successes = self.success_rate * n;
        let new_successes = if trajectory.is_success() {
            successes + 1.0
        } else {
            successes
        };
        self.success_rate = new_successes / new_n;

        self.count += 1;
    }

    /// Get variance (finalized)
    pub fn final_variance(&self) -> f32 {
        if self.count > 1 {
            self.variance / (self.count - 1) as f32
        } else {
            0.0
        }
    }
}

/// Ring buffer for trajectory storage
pub struct TrajectoryBuffer {
    /// Trajectories storage
    trajectories: Vec<Trajectory>,
    /// Maximum capacity
    capacity: usize,
    /// Write position
    write_pos: usize,
    /// Total count (may exceed capacity)
    total_count: u64,
    /// Running stats
    stats: TrajectoryStats,
}

impl TrajectoryBuffer {
    /// Create a new trajectory buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            trajectories: Vec::with_capacity(capacity),
            capacity,
            write_pos: 0,
            total_count: 0,
            stats: TrajectoryStats::default(),
        }
    }

    /// Push a new trajectory
    pub fn push(&mut self, trajectory: Trajectory) {
        self.stats.update(&trajectory);

        if self.trajectories.len() < self.capacity {
            self.trajectories.push(trajectory);
        } else {
            self.trajectories[self.write_pos] = trajectory;
        }

        self.write_pos = (self.write_pos + 1) % self.capacity;
        self.total_count += 1;
    }

    /// Get current buffer contents
    pub fn trajectories(&self) -> &[Trajectory] {
        &self.trajectories
    }

    /// Drain all trajectories (returns ownership, clears buffer)
    pub fn drain(&mut self) -> Vec<Trajectory> {
        let result = std::mem::take(&mut self.trajectories);
        self.write_pos = 0;
        result
    }

    /// Get statistics
    pub fn stats(&self) -> &TrajectoryStats {
        &self.stats
    }

    /// Get total count (may exceed capacity)
    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    /// Get current buffer size
    pub fn len(&self) -> usize {
        self.trajectories.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.trajectories.is_empty()
    }

    /// Get high-quality trajectories (quality > threshold)
    pub fn high_quality(&self, threshold: f32) -> Vec<&Trajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.quality() > threshold)
            .collect()
    }

    /// Get trajectories for a specific operator type
    pub fn by_operator(&self, op_type: u8) -> Vec<&Trajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.operator_type == op_type)
            .collect()
    }

    /// Reset buffer and stats
    pub fn reset(&mut self) {
        self.trajectories.clear();
        self.write_pos = 0;
        self.total_count = 0;
        self.stats = TrajectoryStats::default();
    }
}

impl Default for TrajectoryBuffer {
    fn default() -> Self {
        Self::new(1000)
    }
}

// ============ WASM Bindings ============

/// WASM-exposed trajectory buffer
#[wasm_bindgen]
pub struct WasmTrajectoryBuffer {
    buffer: TrajectoryBuffer,
    #[allow(dead_code)]
    embedding_dim: usize,
}

#[wasm_bindgen]
impl WasmTrajectoryBuffer {
    /// Create a new trajectory buffer
    ///
    /// @param capacity - Maximum number of trajectories to store
    /// @param embedding_dim - Dimension of embeddings (default 256)
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: Option<usize>, embedding_dim: Option<usize>) -> Self {
        Self {
            buffer: TrajectoryBuffer::new(capacity.unwrap_or(1000)),
            embedding_dim: embedding_dim.unwrap_or(256),
        }
    }

    /// Record a trajectory
    ///
    /// @param embedding - Embedding vector (Float32Array)
    /// @param op_type - Operator type (0-16)
    /// @param attention_type - Attention mechanism used
    /// @param execution_ms - Actual execution time
    /// @param baseline_ms - Baseline execution time
    #[wasm_bindgen]
    pub fn record(
        &mut self,
        embedding: &[f32],
        op_type: u8,
        attention_type: u8,
        execution_ms: f32,
        baseline_ms: f32,
    ) {
        let traj = Trajectory::new(
            embedding.to_vec(),
            op_type,
            attention_type,
            execution_ms,
            baseline_ms,
        );
        self.buffer.push(traj);
    }

    /// Get total count
    #[wasm_bindgen]
    pub fn total_count(&self) -> u64 {
        self.buffer.total_count()
    }

    /// Get buffer length
    #[wasm_bindgen]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if empty
    #[wasm_bindgen]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get mean improvement
    #[wasm_bindgen]
    pub fn mean_improvement(&self) -> f32 {
        self.buffer.stats().mean_improvement
    }

    /// Get best improvement
    #[wasm_bindgen]
    pub fn best_improvement(&self) -> f32 {
        self.buffer.stats().best_improvement
    }

    /// Get success rate
    #[wasm_bindgen]
    pub fn success_rate(&self) -> f32 {
        self.buffer.stats().success_rate
    }

    /// Get best attention type
    #[wasm_bindgen]
    pub fn best_attention(&self) -> u8 {
        self.buffer.stats().best_attention
    }

    /// Get variance
    #[wasm_bindgen]
    pub fn variance(&self) -> f32 {
        self.buffer.stats().final_variance()
    }

    /// Reset buffer
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.buffer.reset();
    }

    /// Get high quality trajectory count
    #[wasm_bindgen]
    pub fn high_quality_count(&self, threshold: f32) -> usize {
        self.buffer.high_quality(threshold).len()
    }

    /// Get trajectory count for operator
    #[wasm_bindgen]
    pub fn count_by_operator(&self, op_type: u8) -> usize {
        self.buffer.by_operator(op_type).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_creation() {
        let embedding = vec![1.0; 256];
        let traj = Trajectory::new(embedding, 2, 0, 100.0, 150.0);

        assert_eq!(traj.operator_type, 2);
        assert!(traj.improvement > 0.0); // 150/100 - 1 = 0.5
        assert!(traj.is_success());
    }

    #[test]
    fn test_trajectory_quality() {
        let embedding = vec![1.0; 256];

        // 2x speedup should give quality close to 1.0
        let fast = Trajectory::new(embedding.clone(), 0, 0, 50.0, 100.0);
        assert!(fast.quality() > 0.5);

        // Slowdown should give lower quality
        let slow = Trajectory::new(embedding, 0, 0, 150.0, 100.0);
        assert!(slow.quality() < 0.5);
    }

    #[test]
    fn test_buffer_push() {
        let mut buffer = TrajectoryBuffer::new(10);
        let embedding = vec![1.0; 256];

        for i in 0..15 {
            let traj = Trajectory::new(embedding.clone(), 0, 0, 100.0, 100.0 + i as f32);
            buffer.push(traj);
        }

        // Buffer should be at capacity
        assert_eq!(buffer.len(), 10);
        // Total count should include all pushes
        assert_eq!(buffer.total_count(), 15);
    }

    #[test]
    fn test_stats_update() {
        let mut stats = TrajectoryStats::default();
        let embedding = vec![1.0; 256];

        let traj1 = Trajectory::new(embedding.clone(), 0, 0, 100.0, 150.0); // 50% improvement
        let traj2 = Trajectory::new(embedding.clone(), 0, 1, 100.0, 200.0); // 100% improvement
        let traj3 = Trajectory::new(embedding, 0, 0, 150.0, 100.0); // -33% (failure)

        stats.update(&traj1);
        stats.update(&traj2);
        stats.update(&traj3);

        assert_eq!(stats.count, 3);
        assert!(stats.success_rate > 0.6); // 2/3 success
        assert_eq!(stats.best_attention, 1); // Best was attention type 1
    }

    #[test]
    fn test_high_quality_filter() {
        let mut buffer = TrajectoryBuffer::new(100);
        let embedding = vec![1.0; 256];

        // Add some trajectories with varying quality
        for i in 0..10 {
            let baseline = 100.0 + (i as f32) * 20.0;
            let traj = Trajectory::new(embedding.clone(), 0, 0, 100.0, baseline);
            buffer.push(traj);
        }

        let high_quality = buffer.high_quality(0.5);
        assert!(!high_quality.is_empty());
    }
}
