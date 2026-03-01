//! Loop B - Background Learning
//!
//! Hourly pattern extraction and base LoRA updates.

use crate::ewc::EwcPlusPlus;
use crate::lora::BaseLoRA;
use crate::reasoning_bank::ReasoningBank;
use crate::time_compat::Instant;
use crate::types::{LearnedPattern, QueryTrajectory, SonaConfig};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;

/// Background loop configuration
#[derive(Clone, Debug)]
pub struct BackgroundLoopConfig {
    /// Minimum trajectories to process
    pub min_trajectories: usize,
    /// Base LoRA learning rate
    pub base_lora_lr: f32,
    /// EWC lambda
    pub ewc_lambda: f32,
    /// Pattern extraction interval
    pub extraction_interval: Duration,
}

impl Default for BackgroundLoopConfig {
    fn default() -> Self {
        Self {
            min_trajectories: 100,
            base_lora_lr: 0.0001,
            ewc_lambda: 1000.0,
            extraction_interval: Duration::from_secs(3600),
        }
    }
}

impl From<&SonaConfig> for BackgroundLoopConfig {
    fn from(config: &SonaConfig) -> Self {
        Self {
            min_trajectories: 100,
            base_lora_lr: config.base_lora_lr,
            ewc_lambda: config.ewc_lambda,
            extraction_interval: Duration::from_millis(config.background_interval_ms),
        }
    }
}

/// Background cycle result
#[derive(Debug)]
pub struct BackgroundResult {
    pub trajectories_processed: usize,
    pub patterns_extracted: usize,
    pub ewc_updated: bool,
    pub elapsed: Duration,
    pub status: String,
}

impl BackgroundResult {
    fn skipped(reason: &str) -> Self {
        Self {
            trajectories_processed: 0,
            patterns_extracted: 0,
            ewc_updated: false,
            elapsed: Duration::ZERO,
            status: format!("skipped: {}", reason),
        }
    }
}

/// Background learning loop (Loop B)
pub struct BackgroundLoop {
    /// Configuration
    config: BackgroundLoopConfig,
    /// ReasoningBank for pattern storage
    reasoning_bank: Arc<RwLock<ReasoningBank>>,
    /// EWC++ for forgetting prevention
    ewc: Arc<RwLock<EwcPlusPlus>>,
    /// Base LoRA
    base_lora: Arc<RwLock<BaseLoRA>>,
    /// Last extraction time
    last_extraction: RwLock<Instant>,
}

impl BackgroundLoop {
    /// Create new background loop
    pub fn new(
        config: BackgroundLoopConfig,
        reasoning_bank: Arc<RwLock<ReasoningBank>>,
        ewc: Arc<RwLock<EwcPlusPlus>>,
        base_lora: Arc<RwLock<BaseLoRA>>,
    ) -> Self {
        Self {
            config,
            reasoning_bank,
            ewc,
            base_lora,
            last_extraction: RwLock::new(Instant::now()),
        }
    }

    /// Check if it's time for background cycle
    pub fn should_run(&self) -> bool {
        self.last_extraction.read().elapsed() >= self.config.extraction_interval
    }

    /// Run background learning cycle
    pub fn run_cycle(&self, trajectories: Vec<QueryTrajectory>) -> BackgroundResult {
        if trajectories.len() < self.config.min_trajectories {
            return BackgroundResult::skipped("insufficient trajectories");
        }

        let start = Instant::now();

        // 1. Add trajectories to reasoning bank
        {
            let mut bank = self.reasoning_bank.write();
            for trajectory in &trajectories {
                bank.add_trajectory(trajectory);
            }
        }

        // 2. Extract patterns
        let patterns = {
            let mut bank = self.reasoning_bank.write();
            bank.extract_patterns()
        };

        // 3. Compute gradients from patterns
        let gradients = self.compute_pattern_gradients(&patterns);

        // 4. Apply EWC++ constraints
        let constrained_gradients = {
            let ewc = self.ewc.read();
            ewc.apply_constraints(&gradients)
        };

        // 5. Check for task boundary
        let task_boundary = {
            let ewc = self.ewc.read();
            ewc.detect_task_boundary(&gradients)
        };

        if task_boundary {
            let mut ewc = self.ewc.write();
            ewc.start_new_task();
        }

        // 6. Update EWC++ Fisher
        {
            let mut ewc = self.ewc.write();
            ewc.update_fisher(&constrained_gradients);
        }

        // 7. Update base LoRA
        self.update_base_lora(&constrained_gradients);

        // Update last extraction time
        *self.last_extraction.write() = Instant::now();

        BackgroundResult {
            trajectories_processed: trajectories.len(),
            patterns_extracted: patterns.len(),
            ewc_updated: true,
            elapsed: start.elapsed(),
            status: "completed".to_string(),
        }
    }

    fn compute_pattern_gradients(&self, patterns: &[LearnedPattern]) -> Vec<f32> {
        if patterns.is_empty() {
            return Vec::new();
        }

        let dim = patterns[0].centroid.len();
        let mut gradient = vec![0.0f32; dim];
        let mut total_weight = 0.0f32;

        for pattern in patterns {
            let weight = pattern.avg_quality * pattern.cluster_size as f32;
            for (i, &v) in pattern.centroid.iter().enumerate() {
                if i < dim {
                    gradient[i] += v * weight;
                }
            }
            total_weight += weight;
        }

        if total_weight > 0.0 {
            for g in &mut gradient {
                *g /= total_weight;
            }
        }

        gradient
    }

    fn update_base_lora(&self, gradients: &[f32]) {
        let mut lora = self.base_lora.write();
        let num_layers = lora.num_layers();

        if num_layers == 0 || gradients.is_empty() {
            return;
        }

        let per_layer = gradients.len() / num_layers;

        for (layer_idx, layer) in lora.layers.iter_mut().enumerate() {
            let start = layer_idx * per_layer;
            let end = (start + per_layer).min(gradients.len());

            for (i, &grad) in gradients[start..end].iter().enumerate() {
                if i < layer.up_proj.len() {
                    layer.up_proj[i] += grad * self.config.base_lora_lr;
                }
            }
        }
    }

    /// Get reasoning bank reference
    pub fn reasoning_bank(&self) -> &Arc<RwLock<ReasoningBank>> {
        &self.reasoning_bank
    }

    /// Get EWC reference
    pub fn ewc(&self) -> &Arc<RwLock<EwcPlusPlus>> {
        &self.ewc
    }

    /// Get base LoRA reference
    pub fn base_lora(&self) -> &Arc<RwLock<BaseLoRA>> {
        &self.base_lora
    }
}
