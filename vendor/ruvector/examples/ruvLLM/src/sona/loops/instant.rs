//! Loop A - Instant Learning
//!
//! Per-request adaptation with <1ms overhead.

use crate::sona::lora::MicroLoRA;
use crate::sona::trajectory::{TrajectoryBuffer, TrajectoryIdGen};
use crate::sona::types::{LearningSignal, QueryTrajectory, SonaConfig};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration for instant loop
#[derive(Clone, Debug)]
pub struct InstantLoopConfig {
    /// Micro-LoRA rank
    pub micro_lora_rank: usize,
    /// Micro-LoRA learning rate
    pub micro_lora_lr: f32,
    /// Buffer capacity
    pub buffer_capacity: usize,
    /// Flush threshold (apply updates every N signals)
    pub flush_threshold: usize,
}

impl Default for InstantLoopConfig {
    fn default() -> Self {
        Self {
            micro_lora_rank: 1,
            micro_lora_lr: 0.001,
            buffer_capacity: 10000,
            flush_threshold: 100,
        }
    }
}

impl From<&SonaConfig> for InstantLoopConfig {
    fn from(config: &SonaConfig) -> Self {
        Self {
            micro_lora_rank: config.micro_lora_rank,
            micro_lora_lr: config.micro_lora_lr,
            buffer_capacity: config.trajectory_capacity,
            flush_threshold: 100,
        }
    }
}

/// Instant loop metrics
#[derive(Debug, Default)]
pub struct InstantLoopMetrics {
    /// Total trajectories processed
    pub trajectories_processed: AtomicU64,
    /// Total signals accumulated
    pub signals_accumulated: AtomicU64,
    /// Total flushes performed
    pub flushes_performed: AtomicU64,
    /// Total updates applied
    pub updates_applied: AtomicU64,
}

/// Instant learning loop (Loop A)
pub struct InstantLoop {
    /// Configuration
    config: InstantLoopConfig,
    /// Trajectory buffer
    trajectory_buffer: Arc<TrajectoryBuffer>,
    /// Micro-LoRA adapter
    micro_lora: Arc<RwLock<MicroLoRA>>,
    /// ID generator
    id_gen: TrajectoryIdGen,
    /// Pending signal count
    pending_signals: AtomicU64,
    /// Metrics
    pub metrics: InstantLoopMetrics,
}

impl InstantLoop {
    /// Create new instant loop
    pub fn new(hidden_dim: usize, config: InstantLoopConfig) -> Self {
        Self {
            trajectory_buffer: Arc::new(TrajectoryBuffer::new(config.buffer_capacity)),
            micro_lora: Arc::new(RwLock::new(MicroLoRA::new(
                hidden_dim,
                config.micro_lora_rank,
            ))),
            id_gen: TrajectoryIdGen::new(),
            pending_signals: AtomicU64::new(0),
            config,
            metrics: InstantLoopMetrics::default(),
        }
    }

    /// Create from SONA config
    pub fn from_sona_config(config: &SonaConfig) -> Self {
        Self::new(config.hidden_dim, InstantLoopConfig::from(config))
    }

    /// Generate next trajectory ID
    pub fn next_id(&self) -> u64 {
        self.id_gen.next()
    }

    /// Process completed trajectory
    pub fn on_trajectory(&self, trajectory: QueryTrajectory) {
        // Record to buffer
        self.trajectory_buffer.record(trajectory.clone());
        self.metrics
            .trajectories_processed
            .fetch_add(1, Ordering::Relaxed);

        // Generate learning signal
        let signal = LearningSignal::from_trajectory(&trajectory);

        // Accumulate gradient (non-blocking)
        if let Some(mut lora) = self.micro_lora.try_write() {
            lora.accumulate_gradient(&signal);
            self.metrics
                .signals_accumulated
                .fetch_add(1, Ordering::Relaxed);

            let pending = self.pending_signals.fetch_add(1, Ordering::Relaxed) + 1;

            // Auto-flush if threshold reached
            if pending >= self.config.flush_threshold as u64 {
                self.flush_internal(&mut lora);
            }
        }
    }

    /// Manually flush accumulated updates
    pub fn flush(&self) {
        if let Some(mut lora) = self.micro_lora.try_write() {
            self.flush_internal(&mut lora);
        }
    }

    fn flush_internal(&self, lora: &mut MicroLoRA) {
        let pending = lora.pending_updates();
        if pending > 0 {
            lora.apply_accumulated(self.config.micro_lora_lr);
            self.pending_signals.store(0, Ordering::Relaxed);
            self.metrics
                .flushes_performed
                .fetch_add(1, Ordering::Relaxed);
            self.metrics
                .updates_applied
                .fetch_add(pending as u64, Ordering::Relaxed);
        }
    }

    /// Drain trajectories for background processing
    pub fn drain_trajectories(&self) -> Vec<QueryTrajectory> {
        self.trajectory_buffer.drain()
    }

    /// Drain up to N trajectories
    pub fn drain_trajectories_n(&self, n: usize) -> Vec<QueryTrajectory> {
        self.trajectory_buffer.drain_n(n)
    }

    /// Get micro-LoRA reference for inference
    pub fn micro_lora(&self) -> &Arc<RwLock<MicroLoRA>> {
        &self.micro_lora
    }

    /// Get trajectory buffer reference
    pub fn buffer(&self) -> &Arc<TrajectoryBuffer> {
        &self.trajectory_buffer
    }

    /// Get pending trajectory count
    pub fn pending_count(&self) -> usize {
        self.trajectory_buffer.len()
    }

    /// Get buffer stats
    pub fn buffer_stats(&self) -> (usize, u64, f64) {
        (
            self.trajectory_buffer.len(),
            self.trajectory_buffer.dropped_count(),
            self.trajectory_buffer.success_rate(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sona::types::TrajectoryStep;

    fn make_trajectory(id: u64) -> QueryTrajectory {
        let mut t = QueryTrajectory::new(id, vec![0.1; 64]);
        t.add_step(TrajectoryStep::new(vec![0.5; 64], vec![], 0.8, 0));
        t.finalize(0.8, 1000);
        t
    }

    #[test]
    fn test_instant_loop_creation() {
        let loop_a = InstantLoop::new(64, InstantLoopConfig::default());
        assert_eq!(loop_a.pending_count(), 0);
    }

    #[test]
    fn test_trajectory_processing() {
        let loop_a = InstantLoop::new(64, InstantLoopConfig::default());

        let t = make_trajectory(loop_a.next_id());
        loop_a.on_trajectory(t);

        assert_eq!(loop_a.pending_count(), 1);
        assert_eq!(
            loop_a
                .metrics
                .trajectories_processed
                .load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn test_auto_flush() {
        let config = InstantLoopConfig {
            flush_threshold: 3,
            ..Default::default()
        };
        let loop_a = InstantLoop::new(64, config);

        for i in 0..5 {
            loop_a.on_trajectory(make_trajectory(i));
        }

        assert!(loop_a.metrics.flushes_performed.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_drain() {
        let loop_a = InstantLoop::new(64, InstantLoopConfig::default());

        for i in 0..10 {
            loop_a.on_trajectory(make_trajectory(i));
        }

        let drained = loop_a.drain_trajectories();
        assert_eq!(drained.len(), 10);
        assert_eq!(loop_a.pending_count(), 0);
    }
}
