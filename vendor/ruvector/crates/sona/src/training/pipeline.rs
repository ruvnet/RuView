//! Training Pipeline for SONA
//!
//! Structured training workflows with batching and callbacks.

use super::metrics::{EpochStats, TrainingMetrics, TrainingResult};
use super::templates::{DataSizeHint, TrainingMethod, TrainingTemplate};
use crate::engine::SonaEngine;
use crate::time_compat::Instant;
use crate::types::SonaConfig;
use serde::{Deserialize, Serialize};

/// Training example with all data needed for learning
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingExample {
    /// Input embedding
    pub embedding: Vec<f32>,
    /// Hidden activations (optional, defaults to embedding)
    pub activations: Option<Vec<f32>>,
    /// Attention weights (optional)
    pub attention: Option<Vec<f32>>,
    /// Quality score [0.0, 1.0]
    pub quality: f32,
    /// Reward signal (optional, defaults to quality)
    pub reward: Option<f32>,
    /// Model route identifier
    pub route: Option<String>,
    /// Context identifiers
    pub context: Vec<String>,
    /// Example weight for importance sampling
    pub weight: f32,
    /// Tags for filtering
    pub tags: Vec<String>,
}

impl TrainingExample {
    /// Create a new training example
    pub fn new(embedding: Vec<f32>, quality: f32) -> Self {
        Self {
            embedding,
            activations: None,
            attention: None,
            quality,
            reward: None,
            route: None,
            context: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        }
    }

    /// Set activations
    pub fn with_activations(mut self, activations: Vec<f32>) -> Self {
        self.activations = Some(activations);
        self
    }

    /// Set attention
    pub fn with_attention(mut self, attention: Vec<f32>) -> Self {
        self.attention = Some(attention);
        self
    }

    /// Set reward
    pub fn with_reward(mut self, reward: f32) -> Self {
        self.reward = Some(reward);
        self
    }

    /// Set route
    pub fn with_route(mut self, route: impl Into<String>) -> Self {
        self.route = Some(route.into());
        self
    }

    /// Add context
    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context.push(ctx.into());
        self
    }

    /// Set weight
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    /// Add tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Get activations or default to embedding
    pub fn get_activations(&self) -> Vec<f32> {
        self.activations
            .clone()
            .unwrap_or_else(|| self.embedding.clone())
    }

    /// Get attention or default
    pub fn get_attention(&self) -> Vec<f32> {
        self.attention
            .clone()
            .unwrap_or_else(|| vec![1.0 / 64.0; 64])
    }

    /// Get reward or default to quality
    pub fn get_reward(&self) -> f32 {
        self.reward.unwrap_or(self.quality)
    }
}

/// Batch configuration for training
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Batch size
    pub batch_size: usize,
    /// Shuffle examples
    pub shuffle: bool,
    /// Drop incomplete last batch
    pub drop_last: bool,
    /// Number of epochs
    pub epochs: usize,
    /// Early stopping patience (None = disabled)
    pub early_stopping_patience: Option<usize>,
    /// Minimum quality improvement for early stopping
    pub min_quality_improvement: f32,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            shuffle: true,
            drop_last: false,
            epochs: 1,
            early_stopping_patience: None,
            min_quality_improvement: 0.001,
        }
    }
}

impl BatchConfig {
    /// Create config for single pass (no batching)
    pub fn single_pass() -> Self {
        Self {
            batch_size: usize::MAX,
            shuffle: false,
            drop_last: false,
            epochs: 1,
            early_stopping_patience: None,
            min_quality_improvement: 0.0,
        }
    }

    /// Create config optimized for size hint
    pub fn for_data_size(hint: &DataSizeHint) -> Self {
        match hint {
            DataSizeHint::Tiny => Self {
                batch_size: 8,
                epochs: 10,
                early_stopping_patience: Some(3),
                ..Default::default()
            },
            DataSizeHint::Small => Self {
                batch_size: 16,
                epochs: 5,
                early_stopping_patience: Some(2),
                ..Default::default()
            },
            DataSizeHint::Medium => Self {
                batch_size: 32,
                epochs: 3,
                early_stopping_patience: Some(2),
                ..Default::default()
            },
            DataSizeHint::Large => Self {
                batch_size: 64,
                epochs: 2,
                ..Default::default()
            },
            DataSizeHint::Massive => Self {
                batch_size: 128,
                epochs: 1,
                ..Default::default()
            },
        }
    }
}

/// Pipeline stage for tracking progress
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStage {
    /// Not started
    Idle,
    /// Loading and preprocessing data
    Preprocessing,
    /// Training in progress
    Training,
    /// Running validation
    Validation,
    /// Extracting patterns
    PatternExtraction,
    /// Exporting results
    Export,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
}

impl std::fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineStage::Idle => write!(f, "idle"),
            PipelineStage::Preprocessing => write!(f, "preprocessing"),
            PipelineStage::Training => write!(f, "training"),
            PipelineStage::Validation => write!(f, "validation"),
            PipelineStage::PatternExtraction => write!(f, "pattern_extraction"),
            PipelineStage::Export => write!(f, "export"),
            PipelineStage::Completed => write!(f, "completed"),
            PipelineStage::Failed => write!(f, "failed"),
        }
    }
}

/// Callback trait for training events
pub trait TrainingCallback: Send + Sync {
    /// Called when stage changes
    fn on_stage_change(&self, _stage: &PipelineStage) {}

    /// Called after each batch
    fn on_batch_complete(&self, _batch_idx: usize, _total_batches: usize, _avg_quality: f32) {}

    /// Called after each epoch
    fn on_epoch_complete(&self, _epoch: usize, _stats: &EpochStats) {}

    /// Called when training completes
    fn on_training_complete(&self, _result: &TrainingResult) {}

    /// Called on error
    fn on_error(&self, _error: &str) {}
}

/// No-op callback implementation
pub struct NoOpCallback;
impl TrainingCallback for NoOpCallback {}

/// Logging callback implementation
#[allow(dead_code)]
pub struct LoggingCallback {
    prefix: String,
}

#[allow(dead_code)]
impl LoggingCallback {
    /// Create with prefix
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl TrainingCallback for LoggingCallback {
    fn on_stage_change(&self, stage: &PipelineStage) {
        println!("[{}] Stage: {}", self.prefix, stage);
    }

    fn on_batch_complete(&self, batch_idx: usize, total_batches: usize, avg_quality: f32) {
        if batch_idx % 10 == 0 || batch_idx == total_batches - 1 {
            println!(
                "[{}] Batch {}/{}: avg_quality={:.4}",
                self.prefix,
                batch_idx + 1,
                total_batches,
                avg_quality
            );
        }
    }

    fn on_epoch_complete(&self, epoch: usize, stats: &EpochStats) {
        println!(
            "[{}] Epoch {}: examples={}, avg_quality={:.4}, duration={:.2}s",
            self.prefix,
            epoch + 1,
            stats.examples_processed,
            stats.avg_quality,
            stats.duration_secs
        );
    }

    fn on_training_complete(&self, result: &TrainingResult) {
        println!(
            "[{}] Training complete: epochs={}, patterns={}, final_quality={:.4}",
            self.prefix, result.epochs_completed, result.patterns_learned, result.final_avg_quality
        );
    }

    fn on_error(&self, error: &str) {
        eprintln!("[{}] ERROR: {}", self.prefix, error);
    }
}

/// Training pipeline for structured training workflows
pub struct TrainingPipeline {
    /// Pipeline name
    name: String,
    /// SONA engine
    engine: SonaEngine,
    /// Batch configuration
    batch_config: BatchConfig,
    /// Training method
    training_method: TrainingMethod,
    /// Current stage
    stage: PipelineStage,
    /// Training examples buffer
    examples: Vec<TrainingExample>,
    /// Validation examples
    validation_examples: Vec<TrainingExample>,
    /// Training metrics
    metrics: TrainingMetrics,
    /// Callback
    callback: Box<dyn TrainingCallback>,
    /// Enable pattern extraction after training
    extract_patterns: bool,
}

impl TrainingPipeline {
    /// Create a new training pipeline
    pub fn new(name: impl Into<String>, config: SonaConfig) -> Self {
        let name = name.into();
        Self {
            name: name.clone(),
            engine: SonaEngine::with_config(config),
            batch_config: BatchConfig::default(),
            training_method: TrainingMethod::default(),
            stage: PipelineStage::Idle,
            examples: Vec::new(),
            validation_examples: Vec::new(),
            metrics: TrainingMetrics::new(&name),
            callback: Box::new(NoOpCallback),
            extract_patterns: true,
        }
    }

    /// Create from template
    pub fn from_template(template: TrainingTemplate) -> Self {
        let batch_config = BatchConfig::for_data_size(&template.expected_data_size);
        let mut pipeline = Self::new(&template.name, template.sona_config);
        pipeline.batch_config = batch_config;
        pipeline.training_method = template.training_method;
        pipeline
    }

    /// Set batch configuration
    pub fn with_batch_config(mut self, config: BatchConfig) -> Self {
        self.batch_config = config;
        self
    }

    /// Set training method
    pub fn with_training_method(mut self, method: TrainingMethod) -> Self {
        self.training_method = method;
        self
    }

    /// Set callback
    pub fn with_callback<C: TrainingCallback + 'static>(mut self, callback: C) -> Self {
        self.callback = Box::new(callback);
        self
    }

    /// Enable/disable pattern extraction
    pub fn with_pattern_extraction(mut self, enabled: bool) -> Self {
        self.extract_patterns = enabled;
        self
    }

    /// Add a training example
    pub fn add_example(&mut self, example: TrainingExample) {
        self.examples.push(example);
    }

    /// Add multiple training examples
    pub fn add_examples(&mut self, examples: impl IntoIterator<Item = TrainingExample>) {
        self.examples.extend(examples);
    }

    /// Add validation example
    pub fn add_validation_example(&mut self, example: TrainingExample) {
        self.validation_examples.push(example);
    }

    /// Get current stage
    pub fn stage(&self) -> &PipelineStage {
        &self.stage
    }

    /// Get number of examples
    pub fn example_count(&self) -> usize {
        self.examples.len()
    }

    /// Get metrics
    pub fn metrics(&self) -> &TrainingMetrics {
        &self.metrics
    }

    /// Get engine reference
    pub fn engine(&self) -> &SonaEngine {
        &self.engine
    }

    /// Get mutable engine reference
    pub fn engine_mut(&mut self) -> &mut SonaEngine {
        &mut self.engine
    }

    /// Run the training pipeline
    pub fn train(&mut self) -> Result<TrainingResult, String> {
        let start = Instant::now();

        // Preprocessing
        self.set_stage(PipelineStage::Preprocessing);
        self.preprocess()?;

        // Training
        self.set_stage(PipelineStage::Training);
        let epoch_stats = self.run_training()?;

        // Validation (if examples provided)
        if !self.validation_examples.is_empty() {
            self.set_stage(PipelineStage::Validation);
            self.run_validation()?;
        }

        // Pattern extraction
        if self.extract_patterns {
            self.set_stage(PipelineStage::PatternExtraction);
            self.engine.force_learn();
        }

        self.set_stage(PipelineStage::Completed);

        let result = TrainingResult {
            pipeline_name: self.name.clone(),
            epochs_completed: epoch_stats.len(),
            total_examples: self.metrics.total_examples,
            patterns_learned: self.metrics.patterns_learned,
            final_avg_quality: self.metrics.avg_quality(),
            total_duration_secs: start.elapsed().as_secs_f64(),
            epoch_stats,
            validation_quality: self.metrics.validation_quality,
        };

        self.callback.on_training_complete(&result);
        Ok(result)
    }

    /// Set stage and notify callback
    fn set_stage(&mut self, stage: PipelineStage) {
        self.stage = stage.clone();
        self.callback.on_stage_change(&stage);
    }

    /// Preprocess examples
    fn preprocess(&mut self) -> Result<(), String> {
        if self.examples.is_empty() {
            return Err("No training examples provided".into());
        }

        // Shuffle if configured
        if self.batch_config.shuffle {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            self.examples.shuffle(&mut rng);
        }

        Ok(())
    }

    /// Run training epochs
    fn run_training(&mut self) -> Result<Vec<EpochStats>, String> {
        let mut all_epoch_stats = Vec::new();
        let mut best_quality = 0.0f32;
        let mut patience_counter = 0usize;

        for epoch in 0..self.batch_config.epochs {
            let epoch_start = Instant::now();
            let mut epoch_quality_sum = 0.0f32;
            let mut epoch_examples = 0usize;

            // Create batch indices (to avoid borrow checker issues)
            let batch_size = self.batch_config.batch_size;
            let total_examples = self.examples.len();
            let mut batch_indices: Vec<(usize, usize)> = Vec::new();
            let mut start = 0;
            while start < total_examples {
                let end = (start + batch_size).min(total_examples);
                if end > start && (!self.batch_config.drop_last || end - start == batch_size) {
                    batch_indices.push((start, end));
                }
                start = end;
            }
            let total_batches = batch_indices.len();

            for (batch_idx, (start, end)) in batch_indices.into_iter().enumerate() {
                let batch_quality = self.train_batch_range(start, end)?;
                let batch_len = end - start;
                epoch_quality_sum += batch_quality * batch_len as f32;
                epoch_examples += batch_len;

                self.callback.on_batch_complete(
                    batch_idx,
                    total_batches,
                    epoch_quality_sum / epoch_examples as f32,
                );
            }

            let epoch_avg_quality = if epoch_examples > 0 {
                epoch_quality_sum / epoch_examples as f32
            } else {
                0.0
            };

            let epoch_stats = EpochStats {
                epoch,
                examples_processed: epoch_examples,
                avg_quality: epoch_avg_quality,
                duration_secs: epoch_start.elapsed().as_secs_f64(),
            };

            self.callback.on_epoch_complete(epoch, &epoch_stats);
            all_epoch_stats.push(epoch_stats);

            // Early stopping check
            if let Some(patience) = self.batch_config.early_stopping_patience {
                let improvement = epoch_avg_quality - best_quality;
                if improvement > self.batch_config.min_quality_improvement {
                    best_quality = epoch_avg_quality;
                    patience_counter = 0;
                } else {
                    patience_counter += 1;
                    if patience_counter >= patience {
                        break; // Early stop
                    }
                }
            }

            // Reshuffle for next epoch
            if self.batch_config.shuffle && epoch + 1 < self.batch_config.epochs {
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                self.examples.shuffle(&mut rng);
            }
        }

        Ok(all_epoch_stats)
    }

    /// Train on examples in a range
    fn train_batch_range(&mut self, start: usize, end: usize) -> Result<f32, String> {
        let mut quality_sum = 0.0f32;
        let batch_len = end - start;

        for idx in start..end {
            let example = &self.examples[idx];

            // Begin trajectory using builder API
            let mut builder = self.engine.begin_trajectory(example.embedding.clone());

            // Set route
            if let Some(ref route) = example.route {
                builder.set_model_route(route);
            }

            // Add context
            for ctx in &example.context {
                builder.add_context(ctx);
            }

            // Add step
            builder.add_step(
                example.get_activations(),
                example.get_attention(),
                example.get_reward() * example.weight,
            );

            // End trajectory
            self.engine.end_trajectory(builder, example.quality);

            quality_sum += example.quality;
            self.metrics.total_examples += 1;
            self.metrics.add_quality_sample(example.quality);
        }

        // Run tick to process accumulated trajectories
        self.engine.tick();

        Ok(quality_sum / batch_len as f32)
    }

    /// Run validation
    fn run_validation(&mut self) -> Result<(), String> {
        let mut quality_sum = 0.0f32;

        for example in &self.validation_examples {
            // Apply learned transformations
            let mut output = vec![0.0f32; example.embedding.len()];
            self.engine
                .apply_micro_lora(&example.embedding, &mut output);

            // In a real scenario, you'd evaluate the model output
            // For now, we track the expected quality
            quality_sum += example.quality;
        }

        self.metrics.validation_quality = Some(quality_sum / self.validation_examples.len() as f32);

        Ok(())
    }

    /// Clear examples (keep engine state)
    pub fn clear_examples(&mut self) {
        self.examples.clear();
        self.validation_examples.clear();
    }

    /// Reset pipeline (clear examples and metrics)
    pub fn reset(&mut self) {
        self.clear_examples();
        self.metrics = TrainingMetrics::new(&self.name);
        self.stage = PipelineStage::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_example() {
        let example = TrainingExample::new(vec![0.1; 256], 0.8)
            .with_route("test")
            .with_context("ctx1")
            .with_weight(1.5)
            .with_tag("test");

        assert_eq!(example.quality, 0.8);
        assert_eq!(example.route, Some("test".into()));
        assert_eq!(example.weight, 1.5);
    }

    #[test]
    fn test_batch_config() {
        let config = BatchConfig::for_data_size(&DataSizeHint::Small);
        assert_eq!(config.batch_size, 16);
        assert_eq!(config.epochs, 5);
    }

    #[test]
    fn test_pipeline_creation() {
        let pipeline = TrainingPipeline::new("test", SonaConfig::default());
        assert_eq!(pipeline.stage(), &PipelineStage::Idle);
        assert_eq!(pipeline.example_count(), 0);
    }

    #[test]
    fn test_pipeline_from_template() {
        let template = TrainingTemplate::code_agent().with_hidden_dim(256);
        let pipeline = TrainingPipeline::from_template(template);
        assert_eq!(pipeline.name, "code-agent");
    }

    #[test]
    fn test_pipeline_training() {
        let mut pipeline =
            TrainingPipeline::new("test", SonaConfig::default()).with_batch_config(BatchConfig {
                batch_size: 2,
                epochs: 2,
                ..Default::default()
            });

        // Add examples
        for i in 0..5 {
            pipeline.add_example(TrainingExample::new(
                vec![i as f32 * 0.1; 256],
                0.7 + i as f32 * 0.05,
            ));
        }

        let result = pipeline.train().unwrap();
        assert_eq!(result.epochs_completed, 2);
        assert!(result.total_examples > 0);
    }

    #[test]
    fn test_pipeline_with_validation() {
        let mut pipeline = TrainingPipeline::new("test", SonaConfig::default())
            .with_batch_config(BatchConfig::single_pass());

        pipeline.add_example(TrainingExample::new(vec![0.1; 256], 0.8));
        pipeline.add_validation_example(TrainingExample::new(vec![0.2; 256], 0.9));

        let result = pipeline.train().unwrap();
        assert!(result.validation_quality.is_some());
    }
}
