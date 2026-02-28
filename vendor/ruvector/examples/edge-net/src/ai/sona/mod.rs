//! SONA - Self-Optimizing Neural Architecture
//!
//! Three temporal loops for continuous learning:
//! - Instant: Per-request MicroLoRA adaptation
//! - Background: Hourly consolidation and clustering
//! - Deep: Weekly EWC++ consolidation

use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;

/// SONA learning orchestrator
pub struct SonaLearner {
    /// Instant loop: per-request adaptation
    pub instant_loop: InstantAdapter,
    /// Background loop: hourly consolidation
    pub background_loop: BackgroundConsolidator,
    /// Deep loop: weekly EWC++ consolidation
    pub deep_loop: DeepConsolidator,
    /// Learning trajectory buffer
    pub trajectory_buffer: Arc<RwLock<VecDeque<Trajectory>>>,
    /// Configuration
    pub config: SonaConfig,
}

/// Configuration for SONA learning
#[derive(Clone, Debug)]
pub struct SonaConfig {
    /// Maximum trajectories to buffer
    pub max_trajectories: usize,
    /// Instant loop LoRA rank
    pub instant_lora_rank: u8,
    /// Background loop LoRA rank
    pub background_lora_rank: u8,
    /// Background consolidation interval (seconds)
    pub background_interval_secs: u64,
    /// Deep consolidation interval (seconds)
    pub deep_interval_secs: u64,
    /// EWC lambda (importance weighting)
    pub ewc_lambda: f32,
    /// K-means cluster count
    pub num_clusters: usize,
}

impl Default for SonaConfig {
    fn default() -> Self {
        Self {
            max_trajectories: 10_000,
            instant_lora_rank: 2,
            background_lora_rank: 8,
            background_interval_secs: 3600,      // 1 hour
            deep_interval_secs: 604_800,          // 1 week
            ewc_lambda: 2000.0,
            num_clusters: 100,
        }
    }
}

/// Learning trajectory record
#[derive(Clone, Debug)]
pub struct Trajectory {
    /// Query embedding
    pub query_embedding: Vec<f32>,
    /// Response quality score
    pub quality_score: f32,
    /// Latency in microseconds
    pub latency_us: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Activation patterns
    pub activations: Vec<f32>,
}

/// Instant loop adapter for per-request learning
pub struct InstantAdapter {
    /// Current LoRA rank
    pub rank: u8,
    /// Adaptation rate
    pub adaptation_rate: f32,
}

impl Default for InstantAdapter {
    fn default() -> Self {
        Self {
            rank: 2,
            adaptation_rate: 0.01,
        }
    }
}

/// Background consolidation for hourly learning
pub struct BackgroundConsolidator {
    /// K-means cluster centers
    pub cluster_centers: Vec<Vec<f32>>,
    /// Last consolidation timestamp
    pub last_consolidation: u64,
}

impl Default for BackgroundConsolidator {
    fn default() -> Self {
        Self {
            cluster_centers: Vec::new(),
            last_consolidation: 0,
        }
    }
}

/// Deep consolidation with EWC++
pub struct DeepConsolidator {
    /// Fisher information estimates
    pub fisher_diagonal: Vec<f32>,
    /// Reference parameters
    pub reference_params: Vec<f32>,
    /// EWC lambda
    pub lambda: f32,
    /// Last consolidation timestamp
    pub last_consolidation: u64,
}

impl Default for DeepConsolidator {
    fn default() -> Self {
        Self {
            fisher_diagonal: Vec::new(),
            reference_params: Vec::new(),
            lambda: 2000.0,
            last_consolidation: 0,
        }
    }
}

impl SonaLearner {
    /// Create a new SONA learner with default configuration
    pub fn new() -> Self {
        Self::with_config(SonaConfig::default())
    }

    /// Create a new SONA learner with custom configuration
    pub fn with_config(config: SonaConfig) -> Self {
        Self {
            instant_loop: InstantAdapter {
                rank: config.instant_lora_rank,
                ..Default::default()
            },
            background_loop: BackgroundConsolidator::default(),
            deep_loop: DeepConsolidator {
                lambda: config.ewc_lambda,
                ..Default::default()
            },
            trajectory_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_trajectories))),
            config,
        }
    }

    /// Record a learning trajectory
    pub fn record_trajectory(&self, trajectory: Trajectory) {
        let mut buffer = self.trajectory_buffer.write();
        if buffer.len() >= self.config.max_trajectories {
            buffer.pop_front();
        }
        buffer.push_back(trajectory);
    }

    /// Get trajectory count
    pub fn trajectory_count(&self) -> usize {
        self.trajectory_buffer.read().len()
    }
}

impl Default for SonaLearner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sona_learner_creation() {
        let learner = SonaLearner::new();
        assert_eq!(learner.config.instant_lora_rank, 2);
        assert_eq!(learner.trajectory_count(), 0);
    }

    #[test]
    fn test_trajectory_recording() {
        let learner = SonaLearner::new();
        let trajectory = Trajectory {
            query_embedding: vec![0.1, 0.2, 0.3],
            quality_score: 0.95,
            latency_us: 100,
            timestamp: 12345,
            activations: vec![0.5, 0.5],
        };
        learner.record_trajectory(trajectory);
        assert_eq!(learner.trajectory_count(), 1);
    }
}
