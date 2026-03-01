//! SONA Engine - Main interface for self-optimizing neural architecture

use crate::sona::loops::coordinator::{CoordinatorStats, LoopCoordinator};
use crate::sona::lora::MicroLoRA;
use crate::sona::trajectory::TrajectoryBuilder;
use crate::sona::types::{QueryTrajectory, SonaConfig};
use parking_lot::RwLock;
use std::sync::Arc;

/// Main SONA engine integrating all components
pub struct SonaEngine {
    /// Loop coordinator
    coordinator: LoopCoordinator,
    /// Configuration
    config: SonaConfig,
    /// Whether engine is enabled
    enabled: bool,
}

impl SonaEngine {
    /// Create new SONA engine with default config
    pub fn new(hidden_dim: usize) -> Self {
        Self::with_config(SonaConfig {
            hidden_dim,
            embedding_dim: hidden_dim,
            ..Default::default()
        })
    }

    /// Create with custom config
    pub fn with_config(config: SonaConfig) -> Self {
        Self {
            coordinator: LoopCoordinator::with_config(config.clone()),
            config,
            enabled: true,
        }
    }

    /// Start trajectory recording for a query
    pub fn begin_trajectory(&self, query_embedding: Vec<f32>) -> TrajectoryBuilder {
        let id = self.coordinator.next_trajectory_id();
        TrajectoryBuilder::new(id, query_embedding)
    }

    /// Complete trajectory and submit for learning
    pub fn end_trajectory(&self, builder: TrajectoryBuilder, quality: f32) {
        if !self.enabled {
            return;
        }

        let trajectory = builder.build(quality);
        self.coordinator.on_inference(trajectory);
    }

    /// Submit pre-built trajectory
    pub fn submit_trajectory(&self, trajectory: QueryTrajectory) {
        if self.enabled {
            self.coordinator.on_inference(trajectory);
        }
    }

    /// Apply micro-LoRA to hidden states
    pub fn apply_micro_lora(&self, input: &[f32], output: &mut [f32]) {
        if !self.enabled {
            return;
        }

        if let Some(lora) = self.coordinator.micro_lora().try_read() {
            lora.forward(input, output);
        }
    }

    /// Apply base-LoRA to layer output
    pub fn apply_base_lora(&self, layer_idx: usize, input: &[f32], output: &mut [f32]) {
        if !self.enabled {
            return;
        }

        if let Some(lora) = self.coordinator.base_lora().try_read() {
            lora.forward_layer(layer_idx, input, output);
        }
    }

    /// Run background learning cycle if due
    pub fn tick(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }

        if let Some(result) = self.coordinator.maybe_run_background() {
            Some(format!(
                "Background cycle: {} trajectories -> {} patterns in {:?}",
                result.trajectories_processed, result.patterns_extracted, result.elapsed
            ))
        } else {
            None
        }
    }

    /// Force background learning cycle
    pub fn force_learn(&self) -> String {
        let result = self.coordinator.force_background();
        format!(
            "Forced learning: {} trajectories -> {} patterns, status: {}",
            result.trajectories_processed, result.patterns_extracted, result.status
        )
    }

    /// Flush instant loop updates
    pub fn flush(&self) {
        self.coordinator.flush_instant();
    }

    /// Find similar patterns to query
    pub fn find_patterns(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Vec<crate::sona::LearnedPattern> {
        self.coordinator
            .reasoning_bank()
            .read()
            .find_similar(query_embedding, k)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get engine statistics
    pub fn stats(&self) -> CoordinatorStats {
        self.coordinator.stats()
    }

    /// Enable/disable engine
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get config
    pub fn config(&self) -> &SonaConfig {
        &self.config
    }
}

/// Builder for SonaEngine
pub struct SonaEngineBuilder {
    config: SonaConfig,
}

impl SonaEngineBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: SonaConfig::default(),
        }
    }

    /// Set hidden dimension
    pub fn hidden_dim(mut self, dim: usize) -> Self {
        self.config.hidden_dim = dim;
        self.config.embedding_dim = dim;
        self
    }

    /// Set micro-LoRA rank
    pub fn micro_lora_rank(mut self, rank: usize) -> Self {
        self.config.micro_lora_rank = rank.clamp(1, 2);
        self
    }

    /// Set base-LoRA rank
    pub fn base_lora_rank(mut self, rank: usize) -> Self {
        self.config.base_lora_rank = rank;
        self
    }

    /// Set micro-LoRA learning rate
    pub fn micro_lr(mut self, lr: f32) -> Self {
        self.config.micro_lora_lr = lr;
        self
    }

    /// Set base-LoRA learning rate
    pub fn base_lr(mut self, lr: f32) -> Self {
        self.config.base_lora_lr = lr;
        self
    }

    /// Set EWC lambda
    pub fn ewc_lambda(mut self, lambda: f32) -> Self {
        self.config.ewc_lambda = lambda;
        self
    }

    /// Set pattern clusters
    pub fn pattern_clusters(mut self, k: usize) -> Self {
        self.config.pattern_clusters = k;
        self
    }

    /// Set trajectory buffer capacity
    pub fn buffer_capacity(mut self, capacity: usize) -> Self {
        self.config.trajectory_capacity = capacity;
        self
    }

    /// Set quality threshold
    pub fn quality_threshold(mut self, threshold: f32) -> Self {
        self.config.quality_threshold = threshold;
        self
    }

    /// Build the engine
    pub fn build(self) -> SonaEngine {
        SonaEngine::with_config(self.config)
    }
}

impl Default for SonaEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sona::types::TrajectoryStep;

    #[test]
    fn test_engine_creation() {
        let engine = SonaEngine::new(256);
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_builder() {
        let engine = SonaEngineBuilder::new()
            .hidden_dim(512)
            .micro_lora_rank(2)
            .base_lora_rank(16)
            .micro_lr(0.002)
            .ewc_lambda(500.0)
            .build();

        assert_eq!(engine.config().hidden_dim, 512);
        assert_eq!(engine.config().micro_lora_rank, 2);
    }

    #[test]
    fn test_trajectory_workflow() {
        let engine = SonaEngine::new(64);

        // Begin trajectory
        let mut builder = engine.begin_trajectory(vec![0.1; 64]);
        builder.add_step(vec![0.5; 64], vec![], 0.8);
        builder.add_step(vec![0.6; 64], vec![], 0.9);

        // End trajectory
        engine.end_trajectory(builder, 0.85);

        let stats = engine.stats();
        assert_eq!(stats.trajectories_buffered, 1);
    }

    #[test]
    fn test_micro_lora_application() {
        let engine = SonaEngine::new(64);

        // Train a bit first
        for i in 0..10 {
            let mut builder = engine.begin_trajectory(vec![0.1; 64]);
            builder.add_step(vec![0.5; 64], vec![], 0.8);
            engine.end_trajectory(builder, 0.8);
        }
        engine.flush();

        // Apply LoRA
        let input = vec![1.0; 64];
        let mut output = vec![0.0; 64];
        engine.apply_micro_lora(&input, &mut output);

        // Output may or may not be modified depending on accumulated gradients
    }

    #[test]
    fn test_force_learn() {
        let engine = SonaEngine::new(256);

        for i in 0..150 {
            let mut builder = engine.begin_trajectory(vec![0.1; 256]);
            builder.add_step(vec![0.5; 256], vec![], 0.8);
            engine.end_trajectory(builder, 0.8);
        }

        let result = engine.force_learn();
        assert!(result.contains("150 trajectories"));
    }

    #[test]
    fn test_disabled_engine() {
        let mut engine = SonaEngine::new(64);
        engine.set_enabled(false);

        let builder = engine.begin_trajectory(vec![0.1; 64]);
        engine.end_trajectory(builder, 0.8);

        // Should not record when disabled
        let stats = engine.stats();
        assert_eq!(stats.trajectories_buffered, 0);
    }
}
