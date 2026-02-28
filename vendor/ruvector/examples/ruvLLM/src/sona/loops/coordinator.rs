//! Loop Coordinator - Orchestrates all learning loops

use crate::sona::ewc::{EwcConfig, EwcPlusPlus};
use crate::sona::loops::background::{BackgroundLoop, BackgroundLoopConfig, BackgroundResult};
use crate::sona::loops::instant::{InstantLoop, InstantLoopConfig};
use crate::sona::lora::{BaseLoRA, MicroLoRA};
use crate::sona::reasoning_bank::{PatternConfig, ReasoningBank};
use crate::sona::types::{QueryTrajectory, SonaConfig};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

/// Loop coordinator managing all learning loops
pub struct LoopCoordinator {
    /// Configuration
    config: SonaConfig,
    /// Instant loop (Loop A)
    instant: InstantLoop,
    /// Background loop (Loop B)
    background: BackgroundLoop,
    /// Shared components
    reasoning_bank: Arc<RwLock<ReasoningBank>>,
    ewc: Arc<RwLock<EwcPlusPlus>>,
    base_lora: Arc<RwLock<BaseLoRA>>,
    /// Enabled flags
    instant_enabled: bool,
    background_enabled: bool,
}

impl LoopCoordinator {
    /// Create new coordinator with default config
    pub fn new(hidden_dim: usize) -> Self {
        Self::with_config(SonaConfig {
            hidden_dim,
            embedding_dim: hidden_dim,
            ..Default::default()
        })
    }

    /// Create with custom config
    pub fn with_config(config: SonaConfig) -> Self {
        let reasoning_bank = Arc::new(RwLock::new(ReasoningBank::new(PatternConfig {
            embedding_dim: config.embedding_dim,
            k_clusters: config.pattern_clusters,
            ..Default::default()
        })));

        let ewc = Arc::new(RwLock::new(EwcPlusPlus::new(EwcConfig {
            param_count: config.hidden_dim * config.base_lora_rank * 2,
            initial_lambda: config.ewc_lambda,
            ..Default::default()
        })));

        let base_lora = Arc::new(RwLock::new(BaseLoRA::new(
            config.hidden_dim,
            config.base_lora_rank,
            12, // Default number of layers
        )));

        let instant = InstantLoop::from_sona_config(&config);
        let background = BackgroundLoop::new(
            BackgroundLoopConfig::from(&config),
            reasoning_bank.clone(),
            ewc.clone(),
            base_lora.clone(),
        );

        Self {
            config,
            instant,
            background,
            reasoning_bank,
            ewc,
            base_lora,
            instant_enabled: true,
            background_enabled: true,
        }
    }

    /// Process inference trajectory (Loop A)
    pub fn on_inference(&self, trajectory: QueryTrajectory) {
        if self.instant_enabled {
            self.instant.on_trajectory(trajectory);
        }
    }

    /// Generate next trajectory ID
    pub fn next_trajectory_id(&self) -> u64 {
        self.instant.next_id()
    }

    /// Run background cycle if needed (Loop B)
    pub fn maybe_run_background(&self) -> Option<BackgroundResult> {
        if !self.background_enabled {
            return None;
        }

        if self.background.should_run() {
            let trajectories = self.instant.drain_trajectories();
            if !trajectories.is_empty() {
                return Some(self.background.run_cycle(trajectories));
            }
        }

        None
    }

    /// Force background cycle
    pub fn force_background(&self) -> BackgroundResult {
        let trajectories = self.instant.drain_trajectories();
        self.background.run_cycle(trajectories)
    }

    /// Flush instant loop updates
    pub fn flush_instant(&self) {
        self.instant.flush();
    }

    /// Get micro-LoRA for inference
    pub fn micro_lora(&self) -> &Arc<RwLock<MicroLoRA>> {
        self.instant.micro_lora()
    }

    /// Get base-LoRA for inference
    pub fn base_lora(&self) -> &Arc<RwLock<BaseLoRA>> {
        &self.base_lora
    }

    /// Get reasoning bank
    pub fn reasoning_bank(&self) -> &Arc<RwLock<ReasoningBank>> {
        &self.reasoning_bank
    }

    /// Get EWC++
    pub fn ewc(&self) -> &Arc<RwLock<EwcPlusPlus>> {
        &self.ewc
    }

    /// Enable/disable instant loop
    pub fn set_instant_enabled(&mut self, enabled: bool) {
        self.instant_enabled = enabled;
    }

    /// Enable/disable background loop
    pub fn set_background_enabled(&mut self, enabled: bool) {
        self.background_enabled = enabled;
    }

    /// Get statistics
    pub fn stats(&self) -> CoordinatorStats {
        let (buffer_len, dropped, success_rate) = self.instant.buffer_stats();

        CoordinatorStats {
            trajectories_buffered: buffer_len,
            trajectories_dropped: dropped,
            buffer_success_rate: success_rate,
            patterns_stored: self.reasoning_bank.read().pattern_count(),
            ewc_tasks: self.ewc.read().task_count(),
            instant_enabled: self.instant_enabled,
            background_enabled: self.background_enabled,
        }
    }
}

/// Coordinator statistics
#[derive(Debug, Clone)]
pub struct CoordinatorStats {
    pub trajectories_buffered: usize,
    pub trajectories_dropped: u64,
    pub buffer_success_rate: f64,
    pub patterns_stored: usize,
    pub ewc_tasks: usize,
    pub instant_enabled: bool,
    pub background_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sona::types::TrajectoryStep;

    fn make_trajectory(id: u64) -> QueryTrajectory {
        let mut t = QueryTrajectory::new(id, vec![0.1; 256]);
        t.add_step(TrajectoryStep::new(vec![0.5; 256], vec![], 0.8, 0));
        t.finalize(0.8, 1000);
        t
    }

    #[test]
    fn test_coordinator_creation() {
        let coord = LoopCoordinator::new(256);
        let stats = coord.stats();
        assert_eq!(stats.trajectories_buffered, 0);
    }

    #[test]
    fn test_inference_processing() {
        let coord = LoopCoordinator::new(256);

        for i in 0..10 {
            let t = make_trajectory(coord.next_trajectory_id());
            coord.on_inference(t);
        }

        let stats = coord.stats();
        assert_eq!(stats.trajectories_buffered, 10);
    }

    #[test]
    fn test_force_background() {
        let coord = LoopCoordinator::new(256);

        for i in 0..150 {
            let t = make_trajectory(coord.next_trajectory_id());
            coord.on_inference(t);
        }

        let result = coord.force_background();
        assert_eq!(result.trajectories_processed, 150);
        assert!(result.patterns_extracted > 0);
    }
}
