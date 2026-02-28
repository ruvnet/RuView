//! Training Pipeline: Fine-tuning Loop with EWC++ Regularization
//!
//! This module provides the training infrastructure for MicroLoRA:
//! - Single-example gradient computation
//! - EWC++ regularization to prevent catastrophic forgetting
//! - Learning rate scheduling
//! - Async adaptation support

use crate::error::{Result, RuvLLMError};
use crate::lora::micro_lora::{AdaptFeedback, EwcState, MicroLoRA, TargetModule};
use ndarray::{Array1, Array2};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration for the training pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Base learning rate
    pub learning_rate: f32,
    /// Minimum learning rate
    pub min_learning_rate: f32,
    /// Maximum learning rate
    pub max_learning_rate: f32,
    /// EWC regularization strength (lambda)
    pub ewc_lambda: f32,
    /// Fisher information decay factor (EMA)
    pub fisher_decay: f32,
    /// Batch size for gradient accumulation
    pub batch_size: usize,
    /// Quality threshold for learning (skip low-quality samples)
    pub quality_threshold: f32,
    /// Learning rate schedule
    pub lr_schedule: LearningRateSchedule,
    /// Warmup steps for learning rate
    pub warmup_steps: usize,
    /// Maximum gradient norm (for clipping)
    pub max_grad_norm: f32,
    /// Weight decay factor
    pub weight_decay: f32,
    /// Enable async adaptation
    pub async_adaptation: bool,
    /// Buffer size for async adaptation
    pub async_buffer_size: usize,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.002, // Optimized from benchmarks
            min_learning_rate: 1e-5,
            max_learning_rate: 0.01,
            ewc_lambda: 2000.0, // Optimized for forgetting prevention
            fisher_decay: 0.999,
            batch_size: 1, // Single-example by default for real-time
            quality_threshold: 0.3,
            lr_schedule: LearningRateSchedule::Cosine,
            warmup_steps: 100,
            max_grad_norm: 1.0,
            weight_decay: 0.01,
            async_adaptation: true,
            async_buffer_size: 64,
        }
    }
}

impl TrainingConfig {
    /// Create config for real-time adaptation (single-example)
    pub fn realtime() -> Self {
        Self {
            learning_rate: 0.001,
            batch_size: 1,
            async_adaptation: true,
            async_buffer_size: 32,
            ..Default::default()
        }
    }

    /// Create config for batch adaptation
    pub fn batch(batch_size: usize) -> Self {
        Self {
            learning_rate: 0.002,
            batch_size,
            async_adaptation: false,
            ..Default::default()
        }
    }

    /// Create config optimized for stability
    pub fn stable() -> Self {
        Self {
            learning_rate: 0.0005,
            ewc_lambda: 5000.0,
            max_grad_norm: 0.5,
            weight_decay: 0.02,
            quality_threshold: 0.5,
            ..Default::default()
        }
    }
}

/// Learning rate schedule types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LearningRateSchedule {
    /// Constant learning rate
    Constant,
    /// Linear decay
    Linear,
    /// Cosine annealing
    Cosine,
    /// Exponential decay
    Exponential,
    /// Step decay (reduce by factor at milestones)
    Step,
    /// Warmup then constant
    WarmupConstant,
    /// One-cycle policy
    OneCycle,
}

impl Default for LearningRateSchedule {
    fn default() -> Self {
        Self::Cosine
    }
}

/// Gradient accumulator for batch processing
pub struct GradientAccumulator {
    /// Accumulated gradients per module
    gradients: HashMap<TargetModule, ModuleGradients>,
    /// Number of accumulated samples
    sample_count: usize,
    /// Total quality of accumulated samples
    total_quality: f32,
}

/// Gradients for a single module
struct ModuleGradients {
    grad_a: Array2<f32>,
    grad_b: Array2<f32>,
}

impl GradientAccumulator {
    /// Create a new accumulator
    pub fn new() -> Self {
        Self {
            gradients: HashMap::new(),
            sample_count: 0,
            total_quality: 0.0,
        }
    }

    /// Initialize for a module with dimensions
    pub fn init_module(
        &mut self,
        module: TargetModule,
        in_features: usize,
        rank: usize,
        out_features: usize,
    ) {
        self.gradients.insert(
            module,
            ModuleGradients {
                grad_a: Array2::zeros((in_features, rank)),
                grad_b: Array2::zeros((rank, out_features)),
            },
        );
    }

    /// Accumulate gradients
    pub fn accumulate(
        &mut self,
        module: TargetModule,
        grad_a: &Array2<f32>,
        grad_b: &Array2<f32>,
        quality: f32,
    ) {
        if let Some(grads) = self.gradients.get_mut(&module) {
            grads.grad_a.zip_mut_with(grad_a, |a, g| *a += g * quality);
            grads.grad_b.zip_mut_with(grad_b, |b, g| *b += g * quality);
        }
        self.sample_count += 1;
        self.total_quality += quality;
    }

    /// Get average gradients
    pub fn average(&self) -> HashMap<TargetModule, (Array2<f32>, Array2<f32>)> {
        if self.sample_count == 0 {
            return HashMap::new();
        }

        let scale = 1.0 / self.sample_count as f32;
        self.gradients
            .iter()
            .map(|(module, grads)| {
                let avg_a = grads.grad_a.mapv(|v| v * scale);
                let avg_b = grads.grad_b.mapv(|v| v * scale);
                (*module, (avg_a, avg_b))
            })
            .collect()
    }

    /// Clear accumulated gradients
    pub fn clear(&mut self) {
        for grads in self.gradients.values_mut() {
            grads.grad_a.fill(0.0);
            grads.grad_b.fill(0.0);
        }
        self.sample_count = 0;
        self.total_quality = 0.0;
    }

    /// Get sample count
    pub fn count(&self) -> usize {
        self.sample_count
    }

    /// Get average quality
    pub fn average_quality(&self) -> f32 {
        if self.sample_count == 0 {
            0.0
        } else {
            self.total_quality / self.sample_count as f32
        }
    }
}

impl Default for GradientAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// EWC++ regularizer for preventing catastrophic forgetting
pub struct EwcRegularizer {
    /// EWC state per module
    states: HashMap<TargetModule, EwcState>,
    /// Regularization strength
    lambda: f32,
    /// Fisher decay factor
    decay: f32,
    /// Task count
    task_count: usize,
    /// Samples since last consolidation
    samples_since_consolidation: usize,
    /// Consolidation interval
    consolidation_interval: usize,
}

impl EwcRegularizer {
    /// Create a new EWC regularizer
    pub fn new(lambda: f32, decay: f32) -> Self {
        Self {
            states: HashMap::new(),
            lambda,
            decay,
            task_count: 0,
            samples_since_consolidation: 0,
            consolidation_interval: 1000,
        }
    }

    /// Initialize state for a module from adapter
    pub fn init_module(
        &mut self,
        module: TargetModule,
        adapter: &crate::lora::micro_lora::LoraAdapter,
    ) {
        self.states.insert(module, EwcState::from_adapter(adapter));
    }

    /// Update Fisher information with new gradients
    pub fn update_fisher(
        &mut self,
        module: &TargetModule,
        grad_a: &Array2<f32>,
        grad_b: &Array2<f32>,
    ) {
        if let Some(state) = self.states.get_mut(module) {
            state.update_fisher(grad_a, grad_b, self.decay);
        }
        self.samples_since_consolidation += 1;
    }

    /// Get EWC penalty for a module
    pub fn penalty(
        &self,
        module: &TargetModule,
        current_a: &Array2<f32>,
        current_b: &Array2<f32>,
    ) -> f32 {
        if let Some(state) = self.states.get(module) {
            let mut penalty = 0.0f32;

            // Penalty for A: sum(F_a * (w_a - w*_a)^2)
            for ((f, w), w_opt) in state
                .fisher_a
                .iter()
                .zip(current_a.iter())
                .zip(state.optimal_a.iter())
            {
                let diff = w - w_opt;
                penalty += f * diff * diff;
            }

            // Penalty for B: sum(F_b * (w_b - w*_b)^2)
            for ((f, w), w_opt) in state
                .fisher_b
                .iter()
                .zip(current_b.iter())
                .zip(state.optimal_b.iter())
            {
                let diff = w - w_opt;
                penalty += f * diff * diff;
            }

            self.lambda * penalty / 2.0
        } else {
            0.0
        }
    }

    /// Get EWC gradient adjustment
    pub fn gradient_adjustment(
        &self,
        module: &TargetModule,
        current_a: &Array2<f32>,
        current_b: &Array2<f32>,
    ) -> Option<(Array2<f32>, Array2<f32>)> {
        self.states.get(module).map(|state| {
            // Gradient of penalty: lambda * F * (w - w*)
            let adj_a = Array2::from_shape_fn(current_a.dim(), |(i, j)| {
                self.lambda * state.fisher_a[[i, j]] * (current_a[[i, j]] - state.optimal_a[[i, j]])
            });

            let adj_b = Array2::from_shape_fn(current_b.dim(), |(i, j)| {
                self.lambda * state.fisher_b[[i, j]] * (current_b[[i, j]] - state.optimal_b[[i, j]])
            });

            (adj_a, adj_b)
        })
    }

    /// Start a new task (consolidate current knowledge)
    pub fn start_new_task(
        &mut self,
        adapters: &HashMap<TargetModule, Arc<RwLock<crate::lora::micro_lora::LoraAdapter>>>,
    ) {
        // Update optimal weights to current
        for (module, adapter) in adapters {
            if let Some(state) = self.states.get_mut(module) {
                let adapter = adapter.read();
                state.update_optimal(&adapter);
            }
        }
        self.task_count += 1;
        self.samples_since_consolidation = 0;
    }

    /// Check if consolidation is needed
    pub fn needs_consolidation(&self) -> bool {
        self.samples_since_consolidation >= self.consolidation_interval
    }

    /// Get current lambda
    pub fn lambda(&self) -> f32 {
        self.lambda
    }

    /// Set lambda
    pub fn set_lambda(&mut self, lambda: f32) {
        self.lambda = lambda;
    }

    /// Get task count
    pub fn task_count(&self) -> usize {
        self.task_count
    }

    /// Get EWC state for a module
    pub fn get_state(&self, module: &TargetModule) -> Option<&EwcState> {
        self.states.get(module)
    }

    /// Export states for serialization
    pub fn export_states(&self) -> HashMap<TargetModule, EwcStateExport> {
        self.states
            .iter()
            .map(|(module, state)| {
                (
                    *module,
                    EwcStateExport {
                        fisher_a: state.fisher_a.iter().copied().collect(),
                        fisher_b: state.fisher_b.iter().copied().collect(),
                        optimal_a: state.optimal_a.iter().copied().collect(),
                        optimal_b: state.optimal_b.iter().copied().collect(),
                        shape_a: (state.fisher_a.nrows(), state.fisher_a.ncols()),
                        shape_b: (state.fisher_b.nrows(), state.fisher_b.ncols()),
                    },
                )
            })
            .collect()
    }
}

/// Serializable EWC state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EwcStateExport {
    pub fisher_a: Vec<f32>,
    pub fisher_b: Vec<f32>,
    pub optimal_a: Vec<f32>,
    pub optimal_b: Vec<f32>,
    pub shape_a: (usize, usize),
    pub shape_b: (usize, usize),
}

/// Training pipeline for MicroLoRA
pub struct TrainingPipeline {
    /// Configuration
    config: TrainingConfig,
    /// Gradient accumulator
    accumulator: GradientAccumulator,
    /// EWC regularizer
    ewc: EwcRegularizer,
    /// Current learning rate
    current_lr: f32,
    /// Total training steps
    total_steps: AtomicU64,
    /// Async feedback buffer
    feedback_buffer: RwLock<VecDeque<PendingFeedback>>,
    /// Training statistics
    stats: RwLock<TrainingStats>,
}

/// Pending feedback for async processing
struct PendingFeedback {
    input: Vec<f32>,
    feedback: AdaptFeedback,
    timestamp: std::time::Instant,
}

/// Training statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrainingStats {
    /// Total training steps completed
    pub total_steps: u64,
    /// Total samples processed
    pub total_samples: u64,
    /// Average loss
    pub avg_loss: f32,
    /// Average quality of training samples
    pub avg_quality: f32,
    /// Current learning rate
    pub current_lr: f32,
    /// EWC penalty
    pub ewc_penalty: f32,
    /// Gradient norm
    pub grad_norm: f32,
    /// Samples skipped (below quality threshold)
    pub skipped_samples: u64,
}

impl TrainingPipeline {
    /// Create a new training pipeline
    pub fn new(config: TrainingConfig) -> Self {
        let current_lr = config.learning_rate;
        let ewc = EwcRegularizer::new(config.ewc_lambda, config.fisher_decay);

        Self {
            config,
            accumulator: GradientAccumulator::new(),
            ewc,
            current_lr,
            total_steps: AtomicU64::new(0),
            feedback_buffer: RwLock::new(VecDeque::new()),
            stats: RwLock::new(TrainingStats::default()),
        }
    }

    /// Initialize for a MicroLoRA instance
    pub fn init_for_lora(&mut self, lora: &MicroLoRA) {
        let config = lora.config();
        for module in &config.target_modules {
            self.accumulator.init_module(
                *module,
                config.in_features,
                config.rank,
                config.out_features,
            );

            if let Some(adapter) = lora.get_adapter(module) {
                self.ewc.init_module(*module, &adapter.read());
            }
        }
    }

    /// Process a single training sample
    pub fn train_step(
        &self,
        lora: &MicroLoRA,
        input: &[f32],
        feedback: AdaptFeedback,
    ) -> Result<()> {
        // Skip low-quality samples
        if feedback.quality < self.config.quality_threshold {
            self.stats.write().skipped_samples += 1;
            return Ok(());
        }

        // Accumulate gradients
        lora.adapt(input, feedback.clone())?;

        // Check if we should apply updates
        let step = self.total_steps.fetch_add(1, Ordering::SeqCst);

        if (step + 1) as usize % self.config.batch_size == 0 {
            self.apply_step(lora, step)?;
        }

        Ok(())
    }

    /// Apply accumulated gradients
    fn apply_step(&self, lora: &MicroLoRA, step: u64) -> Result<()> {
        // Update learning rate based on schedule
        let lr = self.compute_lr(step);

        // Apply gradients with EWC
        let ewc_states: HashMap<TargetModule, EwcState> = self
            .ewc
            .states
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        lora.apply_updates_with_ewc(lr, &ewc_states, self.config.ewc_lambda);

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_steps = step;
            stats.current_lr = lr;
            stats.total_samples += self.config.batch_size as u64;
        }

        Ok(())
    }

    /// Compute learning rate based on schedule
    fn compute_lr(&self, step: u64) -> f32 {
        let step = step as f32;
        let warmup = self.config.warmup_steps as f32;
        let base_lr = self.config.learning_rate;
        let min_lr = self.config.min_learning_rate;
        let max_lr = self.config.max_learning_rate;

        // Warmup phase
        if step < warmup {
            return min_lr + (base_lr - min_lr) * (step / warmup);
        }

        let adjusted_step = step - warmup;

        match self.config.lr_schedule {
            LearningRateSchedule::Constant => base_lr,

            LearningRateSchedule::Linear => {
                let decay_steps = 10000.0; // Total decay steps
                let factor = 1.0 - (adjusted_step / decay_steps).min(1.0);
                min_lr + (base_lr - min_lr) * factor
            }

            LearningRateSchedule::Cosine => {
                let decay_steps = 10000.0;
                let factor =
                    0.5 * (1.0 + (std::f32::consts::PI * adjusted_step / decay_steps).cos());
                min_lr + (base_lr - min_lr) * factor
            }

            LearningRateSchedule::Exponential => {
                let decay_rate: f32 = 0.99;
                let factor = decay_rate.powf(adjusted_step / 100.0);
                (base_lr * factor).max(min_lr)
            }

            LearningRateSchedule::Step => {
                let milestones = [1000.0, 5000.0, 10000.0];
                let gamma = 0.1;
                let mut lr = base_lr;
                for &milestone in &milestones {
                    if adjusted_step >= milestone {
                        lr *= gamma;
                    }
                }
                lr.max(min_lr)
            }

            LearningRateSchedule::WarmupConstant => base_lr,

            LearningRateSchedule::OneCycle => {
                let cycle_steps = 10000.0;
                let pct = (adjusted_step % cycle_steps) / cycle_steps;
                if pct < 0.5 {
                    // Increase
                    let factor = 2.0 * pct;
                    base_lr + (max_lr - base_lr) * factor
                } else {
                    // Decrease
                    let factor = 2.0 * (1.0 - pct);
                    min_lr + (max_lr - min_lr) * factor
                }
            }
        }
    }

    /// Queue feedback for async processing
    pub fn queue_feedback(&self, input: Vec<f32>, feedback: AdaptFeedback) {
        if !self.config.async_adaptation {
            return;
        }

        let mut buffer = self.feedback_buffer.write();

        if buffer.len() >= self.config.async_buffer_size {
            buffer.pop_front();
        }

        buffer.push_back(PendingFeedback {
            input,
            feedback,
            timestamp: std::time::Instant::now(),
        });
    }

    /// Process queued feedback
    pub fn process_queued(&self, lora: &MicroLoRA) -> Result<usize> {
        let pending: Vec<_> = {
            let mut buffer = self.feedback_buffer.write();
            buffer.drain(..).collect()
        };

        let count = pending.len();
        for pf in pending {
            self.train_step(lora, &pf.input, pf.feedback)?;
        }

        Ok(count)
    }

    /// Start a new task (for EWC)
    pub fn start_new_task(&mut self, lora: &MicroLoRA) {
        let adapters: HashMap<_, _> = lora
            .config()
            .target_modules
            .iter()
            .filter_map(|m| lora.get_adapter(m).map(|a| (*m, a)))
            .collect();
        self.ewc.start_new_task(&adapters);
    }

    /// Get training statistics
    pub fn stats(&self) -> TrainingStats {
        self.stats.read().clone()
    }

    /// Get current learning rate
    pub fn current_lr(&self) -> f32 {
        self.current_lr
    }

    /// Get configuration
    pub fn config(&self) -> &TrainingConfig {
        &self.config
    }

    /// Reset training state
    pub fn reset(&mut self) {
        self.accumulator.clear();
        self.total_steps.store(0, Ordering::SeqCst);
        self.feedback_buffer.write().clear();
        *self.stats.write() = TrainingStats::default();
    }

    /// Export EWC states for serialization
    pub fn export_ewc(&self) -> HashMap<TargetModule, EwcStateExport> {
        self.ewc.export_states()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lora::micro_lora::MicroLoraConfig;

    #[test]
    fn test_training_config_default() {
        let config = TrainingConfig::default();
        assert!((config.learning_rate - 0.002).abs() < 1e-6);
        assert_eq!(config.batch_size, 1);
    }

    #[test]
    fn test_gradient_accumulator() {
        let mut acc = GradientAccumulator::new();
        acc.init_module(TargetModule::QProj, 64, 2, 64);

        let grad_a = Array2::from_elem((64, 2), 0.1);
        let grad_b = Array2::from_elem((2, 64), 0.1);

        acc.accumulate(TargetModule::QProj, &grad_a, &grad_b, 0.8);
        assert_eq!(acc.count(), 1);

        let avg = acc.average();
        assert!(avg.contains_key(&TargetModule::QProj));
    }

    #[test]
    fn test_learning_rate_schedule() {
        let config = TrainingConfig {
            learning_rate: 0.01,
            min_learning_rate: 0.001,
            warmup_steps: 10,
            lr_schedule: LearningRateSchedule::Cosine,
            ..Default::default()
        };

        let pipeline = TrainingPipeline::new(config);

        // Warmup phase
        let lr_0 = pipeline.compute_lr(0);
        let lr_5 = pipeline.compute_lr(5);
        let lr_10 = pipeline.compute_lr(10);

        assert!(lr_0 < lr_5);
        assert!(lr_5 < lr_10);

        // After warmup, should start decaying
        let lr_100 = pipeline.compute_lr(100);
        let lr_1000 = pipeline.compute_lr(1000);
        assert!(lr_100 > lr_1000);
    }

    #[test]
    fn test_ewc_regularizer() {
        let mut ewc = EwcRegularizer::new(1000.0, 0.999);

        let adapter = crate::lora::micro_lora::LoraAdapter::new(64, 64, 2, 4.0);
        ewc.init_module(TargetModule::QProj, &adapter);

        let grad_a = Array2::from_elem((64, 2), 0.1);
        let grad_b = Array2::from_elem((2, 64), 0.1);

        ewc.update_fisher(&TargetModule::QProj, &grad_a, &grad_b);

        assert!(ewc.get_state(&TargetModule::QProj).is_some());
    }

    #[test]
    fn test_training_pipeline() {
        let config = TrainingConfig::realtime();
        let mut pipeline = TrainingPipeline::new(config);

        let lora_config = MicroLoraConfig::for_hidden_dim(64);
        let lora = MicroLoRA::new(lora_config);

        pipeline.init_for_lora(&lora);

        let input = vec![0.1; 64];
        let feedback = AdaptFeedback::from_quality(0.8);

        pipeline.train_step(&lora, &input, feedback).unwrap();

        let stats = pipeline.stats();
        assert!(stats.total_steps > 0 || stats.total_samples > 0);
    }

    #[test]
    fn test_async_feedback() {
        let config = TrainingConfig {
            async_adaptation: true,
            async_buffer_size: 4,
            ..Default::default()
        };
        let pipeline = TrainingPipeline::new(config);

        for i in 0..6 {
            let input = vec![i as f32 * 0.1; 64];
            let feedback = AdaptFeedback::from_quality(0.8);
            pipeline.queue_feedback(input, feedback);
        }

        // Buffer should be capped at 4
        let buffer = pipeline.feedback_buffer.read();
        assert_eq!(buffer.len(), 4);
    }
}
