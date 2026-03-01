//! SONA Learning Loops for LLM Inference
//!
//! Three learning loops optimized for LLM:
//! - Instant (<1ms): MicroLoRA per-request adaptation
//! - Background (100ms): Pattern consolidation, adapter merging
//! - Deep (minutes): Full fine-tuning triggers
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Inference Request |---->| Instant Loop      |
//! | + feedback        |     | - MicroLoRA adapt |
//! +-------------------+     | - <1ms latency    |
//!                           +--------+----------+
//!                                    |
//!                                    v (async, 100ms)
//!                           +--------+----------+
//!                           | Background Loop   |
//!                           | - Pattern merge   |
//!                           | - Adapter compose |
//!                           | - EWC++ update    |
//!                           +--------+----------+
//!                                    |
//!                                    v (triggered)
//!                           +--------+----------+
//!                           | Deep Loop         |
//!                           | - Full fine-tune  |
//!                           | - Model distill   |
//!                           | - Pattern bank    |
//!                           +-------------------+
//! ```

use crate::error::{Result, RuvLLMError};
use crate::lora::{
    AdaptFeedback, MicroLoRA, MicroLoraConfig, TargetModule, TrainingConfig, TrainingPipeline,
};
use crate::sona::{SonaConfig, SonaIntegration, Trajectory};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for SONA LLM integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaLlmConfig {
    /// MicroLoRA configuration
    pub micro_lora: MicroLoraConfig,
    /// Training pipeline configuration
    pub training: TrainingConfig,
    /// SONA core configuration
    pub sona: SonaConfig,
    /// Instant loop learning rate
    pub instant_lr: f32,
    /// Background loop interval (milliseconds)
    pub background_interval_ms: u64,
    /// Minimum samples for background consolidation
    pub background_min_samples: usize,
    /// Deep loop trigger threshold (accumulated quality)
    pub deep_trigger_threshold: f32,
    /// Maximum pending samples before forced consolidation
    pub max_pending_samples: usize,
    /// Consolidation strategy
    pub consolidation_strategy: ConsolidationStrategy,
    /// Enable async adaptation
    pub async_adaptation: bool,
}

impl Default for SonaLlmConfig {
    fn default() -> Self {
        Self {
            micro_lora: MicroLoraConfig::default(),
            training: TrainingConfig::realtime(),
            sona: SonaConfig::default(),
            instant_lr: 0.01,
            background_interval_ms: 100,
            background_min_samples: 10,
            deep_trigger_threshold: 100.0,
            max_pending_samples: 1000,
            consolidation_strategy: ConsolidationStrategy::EwcMerge,
            async_adaptation: true,
        }
    }
}

/// Strategy for consolidating learned patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsolidationStrategy {
    /// Merge with EWC++ regularization
    EwcMerge,
    /// Simple averaging
    Average,
    /// Weighted by quality
    QualityWeighted,
    /// Keep best performing
    BestOnly,
    /// Ensemble multiple adapters
    Ensemble,
}

impl Default for ConsolidationStrategy {
    fn default() -> Self {
        Self::EwcMerge
    }
}

/// Trigger for deep optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationTrigger {
    /// Accumulated quality threshold
    QualityThreshold(f32),
    /// Sample count threshold
    SampleCount(usize),
    /// Time-based (seconds)
    TimeBased(u64),
    /// Performance degradation detected
    PerformanceDegradation,
    /// Manual trigger
    Manual,
}

/// Training sample for SONA learning
#[derive(Debug, Clone)]
pub struct TrainingSample {
    /// Input embedding
    pub input_embedding: Vec<f32>,
    /// Output embedding
    pub output_embedding: Vec<f32>,
    /// Query text (optional)
    pub query: Option<String>,
    /// Response text (optional)
    pub response: Option<String>,
    /// Quality score (0.0 - 1.0)
    pub quality: f32,
    /// Latency in milliseconds
    pub latency_ms: f32,
    /// Token count
    pub token_count: usize,
    /// Model index used
    pub model_index: usize,
    /// Session identifier
    pub session_id: String,
    /// Timestamp
    pub timestamp: Instant,
}

impl TrainingSample {
    /// Create a new training sample
    pub fn new(input_embedding: Vec<f32>, output_embedding: Vec<f32>, quality: f32) -> Self {
        Self {
            input_embedding,
            output_embedding,
            query: None,
            response: None,
            quality,
            latency_ms: 0.0,
            token_count: 0,
            model_index: 0,
            session_id: String::new(),
            timestamp: Instant::now(),
        }
    }

    /// Set query text
    pub fn with_query(mut self, query: String) -> Self {
        self.query = Some(query);
        self
    }

    /// Set response text
    pub fn with_response(mut self, response: String) -> Self {
        self.response = Some(response);
        self
    }

    /// Set latency
    pub fn with_latency(mut self, latency_ms: f32) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = session_id;
        self
    }

    /// Convert to AdaptFeedback
    pub fn to_feedback(&self) -> AdaptFeedback {
        AdaptFeedback {
            quality: self.quality,
            gradient_estimate: self.output_embedding.clone(),
            reward: Some(self.quality),
            latency_us: (self.latency_ms * 1000.0) as u64,
            source_module: None,
            session_id: Some(self.session_id.clone()),
        }
    }
}

/// Result of an adaptation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationResult {
    /// Whether adaptation was applied
    pub applied: bool,
    /// Which loop processed this
    pub loop_type: String,
    /// Latency of adaptation (microseconds)
    pub latency_us: u64,
    /// Quality improvement estimate
    pub quality_delta: f32,
    /// Number of samples used
    pub samples_used: usize,
    /// Any warnings or notes
    pub notes: Vec<String>,
}

impl Default for AdaptationResult {
    fn default() -> Self {
        Self {
            applied: false,
            loop_type: "none".to_string(),
            latency_us: 0,
            quality_delta: 0.0,
            samples_used: 0,
            notes: Vec::new(),
        }
    }
}

/// Statistics for learning loops
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearningLoopStats {
    /// Instant loop invocations
    pub instant_count: u64,
    /// Instant loop average latency (microseconds)
    pub instant_avg_latency_us: f32,
    /// Background loop invocations
    pub background_count: u64,
    /// Background loop average latency (milliseconds)
    pub background_avg_latency_ms: f32,
    /// Deep loop invocations
    pub deep_count: u64,
    /// Deep loop average latency (seconds)
    pub deep_avg_latency_secs: f32,
    /// Total samples processed
    pub total_samples: u64,
    /// Accumulated quality improvement
    pub accumulated_quality: f32,
    /// Pending samples in buffer
    pub pending_samples: usize,
    /// Last background loop timestamp (seconds since start)
    pub last_background_secs: f32,
    /// Last deep loop timestamp (seconds since start)
    pub last_deep_secs: f32,
}

/// SONA integration for LLM inference
pub struct SonaLlm {
    /// Configuration
    config: SonaLlmConfig,
    /// MicroLoRA adapters
    micro_lora: Arc<RwLock<MicroLoRA>>,
    /// Training pipeline
    training: Arc<RwLock<TrainingPipeline>>,
    /// SONA integration (for ReasoningBank)
    sona: Arc<RwLock<SonaIntegration>>,
    /// Pending samples for background processing
    pending_samples: RwLock<VecDeque<TrainingSample>>,
    /// Accumulated quality for deep trigger
    accumulated_quality: RwLock<f32>,
    /// Last background loop time
    last_background: RwLock<Instant>,
    /// Last deep loop time
    last_deep: RwLock<Instant>,
    /// Start time for statistics
    start_time: Instant,
    /// Statistics
    stats: RwLock<LearningLoopStats>,
    /// Instant loop latency accumulator
    instant_latency_sum: AtomicU64,
    /// Instant loop count for averaging
    instant_count: AtomicU64,
}

impl SonaLlm {
    /// Create a new SONA LLM integration
    pub fn new(config: SonaLlmConfig) -> Self {
        let micro_lora = MicroLoRA::new(config.micro_lora.clone());
        let mut training = TrainingPipeline::new(config.training.clone());
        training.init_for_lora(&micro_lora);
        let sona = SonaIntegration::new(config.sona.clone());

        Self {
            config,
            micro_lora: Arc::new(RwLock::new(micro_lora)),
            training: Arc::new(RwLock::new(training)),
            sona: Arc::new(RwLock::new(sona)),
            pending_samples: RwLock::new(VecDeque::new()),
            accumulated_quality: RwLock::new(0.0),
            last_background: RwLock::new(Instant::now()),
            last_deep: RwLock::new(Instant::now()),
            start_time: Instant::now(),
            stats: RwLock::new(LearningLoopStats::default()),
            instant_latency_sum: AtomicU64::new(0),
            instant_count: AtomicU64::new(0),
        }
    }

    /// Instant loop: per-request MicroLoRA adaptation (<1ms target)
    pub fn instant_adapt(&self, request: &str, response: &str, feedback: f32) -> AdaptationResult {
        let start = Instant::now();

        // Skip if feedback is too low
        if feedback < self.config.training.quality_threshold {
            return AdaptationResult {
                applied: false,
                loop_type: "instant".to_string(),
                notes: vec!["Skipped: quality below threshold".to_string()],
                ..Default::default()
            };
        }

        // Create simple embedding from text (in production, use actual embeddings)
        let input_embedding = self.text_to_embedding(request);
        let output_embedding = self.text_to_embedding(response);

        // Create feedback
        let adapt_feedback = AdaptFeedback::from_quality(feedback);

        // Apply to MicroLoRA
        {
            let lora = self.micro_lora.read();
            if let Err(e) = lora.adapt(&input_embedding, adapt_feedback) {
                return AdaptationResult {
                    applied: false,
                    loop_type: "instant".to_string(),
                    notes: vec![format!("Adaptation error: {}", e)],
                    ..Default::default()
                };
            }
        }

        // Apply gradients immediately with instant learning rate
        {
            let lora = self.micro_lora.read();
            lora.apply_updates(self.config.instant_lr);
        }

        let elapsed = start.elapsed();
        let latency_us = elapsed.as_micros() as u64;

        // Update statistics
        self.instant_latency_sum
            .fetch_add(latency_us, Ordering::Relaxed);
        self.instant_count.fetch_add(1, Ordering::Relaxed);

        // Queue for background consolidation
        let sample = TrainingSample::new(input_embedding, output_embedding, feedback)
            .with_latency(elapsed.as_secs_f32() * 1000.0);

        self.queue_sample(sample);

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.instant_count += 1;
            let total_latency = self.instant_latency_sum.load(Ordering::Relaxed);
            let count = self.instant_count.load(Ordering::Relaxed);
            stats.instant_avg_latency_us = total_latency as f32 / count as f32;
            stats.total_samples += 1;
        }

        AdaptationResult {
            applied: true,
            loop_type: "instant".to_string(),
            latency_us,
            quality_delta: feedback * 0.01, // Estimated small improvement
            samples_used: 1,
            notes: vec![],
        }
    }

    /// Background loop: consolidate patterns, merge adapters (~100ms interval)
    pub fn background_consolidate(&self) -> AdaptationResult {
        let start = Instant::now();

        // Check if enough time has passed
        let last = *self.last_background.read();
        if last.elapsed().as_millis() < self.config.background_interval_ms as u128 {
            return AdaptationResult {
                applied: false,
                loop_type: "background".to_string(),
                notes: vec!["Skipped: too soon since last consolidation".to_string()],
                ..Default::default()
            };
        }

        // Get pending samples
        let samples: Vec<TrainingSample> = {
            let mut pending = self.pending_samples.write();
            if pending.len() < self.config.background_min_samples {
                return AdaptationResult {
                    applied: false,
                    loop_type: "background".to_string(),
                    notes: vec![format!(
                        "Skipped: only {} samples (need {})",
                        pending.len(),
                        self.config.background_min_samples
                    )],
                    ..Default::default()
                };
            }
            pending.drain(..).collect()
        };

        let sample_count = samples.len();

        // Consolidate based on strategy
        let quality_delta = match self.config.consolidation_strategy {
            ConsolidationStrategy::EwcMerge => self.consolidate_ewc(&samples),
            ConsolidationStrategy::Average => self.consolidate_average(&samples),
            ConsolidationStrategy::QualityWeighted => self.consolidate_quality_weighted(&samples),
            ConsolidationStrategy::BestOnly => self.consolidate_best(&samples),
            ConsolidationStrategy::Ensemble => self.consolidate_ensemble(&samples),
        };

        // Update SONA ReasoningBank
        {
            let sona = self.sona.write();
            for sample in &samples {
                let trajectory = Trajectory {
                    request_id: format!("bg-{}", self.instant_count.load(Ordering::Relaxed)),
                    session_id: sample.session_id.clone(),
                    query_embedding: sample.input_embedding.clone(),
                    response_embedding: sample.output_embedding.clone(),
                    quality_score: sample.quality,
                    routing_features: vec![sample.quality, sample.latency_ms / 1000.0],
                    model_index: sample.model_index,
                    timestamp: chrono::Utc::now(),
                };
                let _ = sona.record_trajectory(trajectory);
            }
        }

        // Update accumulated quality
        let quality_sum: f32 = samples.iter().map(|s| s.quality).sum();
        {
            let mut acc = self.accumulated_quality.write();
            *acc += quality_sum;
        }

        // Update last background time
        *self.last_background.write() = Instant::now();

        let elapsed = start.elapsed();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.background_count += 1;
            stats.background_avg_latency_ms = (stats.background_avg_latency_ms
                * (stats.background_count - 1) as f32
                + elapsed.as_secs_f32() * 1000.0)
                / stats.background_count as f32;
            stats.accumulated_quality = *self.accumulated_quality.read();
            stats.last_background_secs = self.start_time.elapsed().as_secs_f32();
        }

        // Check if deep loop should be triggered
        let should_trigger_deep =
            *self.accumulated_quality.read() >= self.config.deep_trigger_threshold;

        AdaptationResult {
            applied: true,
            loop_type: "background".to_string(),
            latency_us: elapsed.as_micros() as u64,
            quality_delta,
            samples_used: sample_count,
            notes: if should_trigger_deep {
                vec!["Deep loop triggered".to_string()]
            } else {
                vec![]
            },
        }
    }

    /// Deep loop: trigger full fine-tuning if needed
    pub fn deep_optimize(&self, dataset: &[TrainingSample]) -> AdaptationResult {
        let start = Instant::now();

        if dataset.is_empty() {
            return AdaptationResult {
                applied: false,
                loop_type: "deep".to_string(),
                notes: vec!["Skipped: empty dataset".to_string()],
                ..Default::default()
            };
        }

        // Start new task in training pipeline (for EWC)
        {
            let lora = self.micro_lora.read();
            let mut training = self.training.write();
            training.start_new_task(&lora);
        }

        // Process all samples through training pipeline
        let mut total_quality = 0.0f32;
        for sample in dataset {
            let feedback = sample.to_feedback();
            let training = self.training.read();
            let lora = self.micro_lora.read();

            if training
                .train_step(&lora, &sample.input_embedding, feedback)
                .is_ok()
            {
                total_quality += sample.quality;
            }
        }

        // Trigger SONA deep loop
        {
            let sona = self.sona.write();
            let _ = sona.trigger_deep_loop();
        }

        // Reset accumulated quality
        *self.accumulated_quality.write() = 0.0;
        *self.last_deep.write() = Instant::now();

        let elapsed = start.elapsed();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.deep_count += 1;
            stats.deep_avg_latency_secs = (stats.deep_avg_latency_secs
                * (stats.deep_count - 1) as f32
                + elapsed.as_secs_f32())
                / stats.deep_count as f32;
            stats.last_deep_secs = self.start_time.elapsed().as_secs_f32();
        }

        AdaptationResult {
            applied: true,
            loop_type: "deep".to_string(),
            latency_us: elapsed.as_micros() as u64,
            quality_delta: total_quality / dataset.len() as f32,
            samples_used: dataset.len(),
            notes: vec![],
        }
    }

    /// Queue a sample for background processing
    fn queue_sample(&self, sample: TrainingSample) {
        let mut pending = self.pending_samples.write();

        // Enforce max pending limit
        if pending.len() >= self.config.max_pending_samples {
            pending.pop_front();
        }

        pending.push_back(sample);

        // Update stats
        self.stats.write().pending_samples = pending.len();
    }

    /// Consolidate using EWC++ merge
    fn consolidate_ewc(&self, samples: &[TrainingSample]) -> f32 {
        let training = self.training.read();
        let lora = self.micro_lora.read();

        // Apply updates through training pipeline with EWC
        let ewc_states = training.export_ewc();
        let ewc_state_map: HashMap<TargetModule, crate::lora::micro_lora::EwcState> = ewc_states
            .into_iter()
            .filter_map(|(module, export)| {
                let fisher_a =
                    ndarray::Array2::from_shape_vec(export.shape_a, export.fisher_a).ok()?;
                let fisher_b =
                    ndarray::Array2::from_shape_vec(export.shape_b, export.fisher_b).ok()?;
                let optimal_a =
                    ndarray::Array2::from_shape_vec(export.shape_a, export.optimal_a).ok()?;
                let optimal_b =
                    ndarray::Array2::from_shape_vec(export.shape_b, export.optimal_b).ok()?;

                Some((
                    module,
                    crate::lora::micro_lora::EwcState {
                        fisher_a,
                        fisher_b,
                        optimal_a,
                        optimal_b,
                    },
                ))
            })
            .collect();

        lora.apply_updates_with_ewc(
            self.config.training.learning_rate,
            &ewc_state_map,
            self.config.training.ewc_lambda,
        );

        // Return average quality as improvement estimate
        samples.iter().map(|s| s.quality).sum::<f32>() / samples.len() as f32 * 0.1
    }

    /// Consolidate using simple averaging
    fn consolidate_average(&self, samples: &[TrainingSample]) -> f32 {
        let lora = self.micro_lora.read();
        lora.apply_updates(self.config.training.learning_rate);
        samples.iter().map(|s| s.quality).sum::<f32>() / samples.len() as f32 * 0.05
    }

    /// Consolidate weighted by quality
    fn consolidate_quality_weighted(&self, samples: &[TrainingSample]) -> f32 {
        let total_quality: f32 = samples.iter().map(|s| s.quality).sum();
        if total_quality <= 0.0 {
            return 0.0;
        }

        // Weight learning rate by average quality
        let avg_quality = total_quality / samples.len() as f32;
        let weighted_lr = self.config.training.learning_rate * avg_quality;

        let lora = self.micro_lora.read();
        lora.apply_updates(weighted_lr);

        avg_quality * 0.1
    }

    /// Consolidate keeping only best samples
    fn consolidate_best(&self, samples: &[TrainingSample]) -> f32 {
        // Take top 20% by quality
        let mut sorted: Vec<&TrainingSample> = samples.iter().collect();
        sorted.sort_by(|a, b| {
            b.quality
                .partial_cmp(&a.quality)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let top_count = (samples.len() as f32 * 0.2).ceil() as usize;
        let best: Vec<&TrainingSample> = sorted.into_iter().take(top_count.max(1)).collect();

        let avg_quality: f32 = best.iter().map(|s| s.quality).sum::<f32>() / best.len() as f32;

        // Apply with higher learning rate for best samples
        let lr = self.config.training.learning_rate * 1.5;
        let lora = self.micro_lora.read();
        lora.apply_updates(lr);

        avg_quality * 0.15
    }

    /// Consolidate using ensemble approach
    fn consolidate_ensemble(&self, samples: &[TrainingSample]) -> f32 {
        // For ensemble, we apply updates in smaller batches
        let batch_size = (samples.len() / 4).max(1);
        let mut total_delta = 0.0f32;

        for batch in samples.chunks(batch_size) {
            let batch_quality: f32 =
                batch.iter().map(|s| s.quality).sum::<f32>() / batch.len() as f32;
            let lr = self.config.training.learning_rate * batch_quality;

            let lora = self.micro_lora.read();
            lora.apply_updates(lr);

            total_delta += batch_quality * 0.02;
        }

        total_delta
    }

    /// Simple text to embedding (placeholder - use actual embeddings in production)
    fn text_to_embedding(&self, text: &str) -> Vec<f32> {
        let dim = self.config.micro_lora.in_features;
        let mut embedding = vec![0.0f32; dim];

        // Simple hash-based embedding for testing
        for (i, byte) in text.bytes().enumerate() {
            let idx = i % dim;
            embedding[idx] += (byte as f32 - 128.0) / 128.0;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    /// Get current statistics
    pub fn stats(&self) -> LearningLoopStats {
        let mut stats = self.stats.read().clone();
        stats.pending_samples = self.pending_samples.read().len();
        stats.accumulated_quality = *self.accumulated_quality.read();
        stats
    }

    /// Get MicroLoRA reference
    pub fn micro_lora(&self) -> Arc<RwLock<MicroLoRA>> {
        Arc::clone(&self.micro_lora)
    }

    /// Get training pipeline reference
    pub fn training(&self) -> Arc<RwLock<TrainingPipeline>> {
        Arc::clone(&self.training)
    }

    /// Get SONA integration reference
    pub fn sona(&self) -> Arc<RwLock<SonaIntegration>> {
        Arc::clone(&self.sona)
    }

    /// Check if deep loop should be triggered
    pub fn should_trigger_deep(&self) -> bool {
        *self.accumulated_quality.read() >= self.config.deep_trigger_threshold
    }

    /// Get pending sample count
    pub fn pending_count(&self) -> usize {
        self.pending_samples.read().len()
    }

    /// Reset all learning state
    pub fn reset(&self) {
        {
            let lora = self.micro_lora.read();
            lora.reset();
        }
        {
            let mut training = self.training.write();
            training.reset();
        }
        self.pending_samples.write().clear();
        *self.accumulated_quality.write() = 0.0;
        *self.last_background.write() = Instant::now();
        *self.last_deep.write() = Instant::now();
        *self.stats.write() = LearningLoopStats::default();
        self.instant_latency_sum.store(0, Ordering::Relaxed);
        self.instant_count.store(0, Ordering::Relaxed);
    }

    /// Forward pass through MicroLoRA
    pub fn forward(&self, input: &[f32], module: &TargetModule) -> Vec<f32> {
        let lora = self.micro_lora.read();
        lora.forward(input, module)
    }

    /// Forward pass that adds to existing output
    pub fn forward_add(&self, input: &[f32], module: &TargetModule, output: &mut [f32]) {
        let lora = self.micro_lora.read();
        lora.forward_add(input, module, output);
    }

    /// Run background loop if needed (non-blocking check)
    pub fn maybe_background(&self) -> Option<AdaptationResult> {
        let last = *self.last_background.read();
        let pending_count = self.pending_samples.read().len();

        if last.elapsed().as_millis() >= self.config.background_interval_ms as u128
            && pending_count >= self.config.background_min_samples
        {
            Some(self.background_consolidate())
        } else {
            None
        }
    }
}

impl Default for SonaLlm {
    fn default() -> Self {
        Self::new(SonaLlmConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sona_llm_config_default() {
        let config = SonaLlmConfig::default();
        assert!((config.instant_lr - 0.01).abs() < 0.001);
        assert_eq!(config.background_min_samples, 10);
    }

    #[test]
    fn test_sona_llm_creation() {
        let sona_llm = SonaLlm::new(SonaLlmConfig::default());
        let stats = sona_llm.stats();
        assert_eq!(stats.instant_count, 0);
        assert_eq!(stats.background_count, 0);
    }

    #[test]
    fn test_instant_adapt() {
        let config = SonaLlmConfig {
            training: TrainingConfig {
                quality_threshold: 0.0, // Accept all
                ..Default::default()
            },
            ..Default::default()
        };

        let sona_llm = SonaLlm::new(config);

        let result = sona_llm.instant_adapt("Hello world", "Response text", 0.8);
        assert!(result.applied);
        assert_eq!(result.loop_type, "instant");
        assert!(result.latency_us < 10000); // Should be < 10ms

        let stats = sona_llm.stats();
        assert_eq!(stats.instant_count, 1);
        assert_eq!(stats.pending_samples, 1);
    }

    #[test]
    fn test_instant_adapt_low_quality() {
        let config = SonaLlmConfig {
            training: TrainingConfig {
                quality_threshold: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };

        let sona_llm = SonaLlm::new(config);

        let result = sona_llm.instant_adapt("Hello", "World", 0.2);
        assert!(!result.applied);
        assert!(!result.notes.is_empty());
    }

    #[test]
    fn test_background_consolidate() {
        let config = SonaLlmConfig {
            background_interval_ms: 0, // Allow immediate
            background_min_samples: 2,
            training: TrainingConfig {
                quality_threshold: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let sona_llm = SonaLlm::new(config);

        // Add samples
        for i in 0..5 {
            sona_llm.instant_adapt(&format!("Query {}", i), &format!("Response {}", i), 0.7);
        }

        let result = sona_llm.background_consolidate();
        assert!(result.applied);
        assert_eq!(result.loop_type, "background");
        assert_eq!(result.samples_used, 5);

        let stats = sona_llm.stats();
        assert_eq!(stats.background_count, 1);
        assert_eq!(stats.pending_samples, 0);
    }

    #[test]
    fn test_deep_optimize() {
        let sona_llm = SonaLlm::new(SonaLlmConfig::default());

        let samples: Vec<TrainingSample> = (0..10)
            .map(|i| TrainingSample::new(vec![0.1 * i as f32; 768], vec![0.2 * i as f32; 768], 0.8))
            .collect();

        let result = sona_llm.deep_optimize(&samples);
        assert!(result.applied);
        assert_eq!(result.loop_type, "deep");
        assert_eq!(result.samples_used, 10);

        let stats = sona_llm.stats();
        assert_eq!(stats.deep_count, 1);
    }

    #[test]
    fn test_training_sample() {
        let sample = TrainingSample::new(vec![0.1; 64], vec![0.2; 64], 0.9)
            .with_query("Test query".to_string())
            .with_response("Test response".to_string())
            .with_latency(50.0)
            .with_session("session-123".to_string());

        assert_eq!(sample.query, Some("Test query".to_string()));
        assert_eq!(sample.session_id, "session-123");

        let feedback = sample.to_feedback();
        assert!((feedback.quality - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_consolidation_strategies() {
        for strategy in [
            ConsolidationStrategy::EwcMerge,
            ConsolidationStrategy::Average,
            ConsolidationStrategy::QualityWeighted,
            ConsolidationStrategy::BestOnly,
            ConsolidationStrategy::Ensemble,
        ] {
            let config = SonaLlmConfig {
                consolidation_strategy: strategy,
                background_interval_ms: 0,
                background_min_samples: 1,
                training: TrainingConfig {
                    quality_threshold: 0.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let sona_llm = SonaLlm::new(config);

            // Add some samples
            for i in 0..5 {
                sona_llm.instant_adapt(
                    &format!("Q{}", i),
                    &format!("R{}", i),
                    0.5 + i as f32 * 0.1,
                );
            }

            let result = sona_llm.background_consolidate();
            assert!(result.applied, "Strategy {:?} failed to apply", strategy);
        }
    }

    #[test]
    fn test_maybe_background() {
        let config = SonaLlmConfig {
            background_interval_ms: 10, // 10ms
            background_min_samples: 3,
            training: TrainingConfig {
                quality_threshold: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let sona_llm = SonaLlm::new(config);

        // Not enough samples
        sona_llm.instant_adapt("Q1", "R1", 0.8);
        assert!(sona_llm.maybe_background().is_none());

        // Add more samples
        sona_llm.instant_adapt("Q2", "R2", 0.8);
        sona_llm.instant_adapt("Q3", "R3", 0.8);

        // Wait for interval
        std::thread::sleep(std::time::Duration::from_millis(15));

        let result = sona_llm.maybe_background();
        assert!(result.is_some());
        assert!(result.unwrap().applied);
    }

    #[test]
    fn test_forward() {
        let config = SonaLlmConfig {
            micro_lora: MicroLoraConfig::for_hidden_dim(64),
            ..Default::default()
        };

        let sona_llm = SonaLlm::new(config);

        let input = vec![0.1; 64];
        let output = sona_llm.forward(&input, &TargetModule::QProj);
        assert_eq!(output.len(), 64);
    }

    #[test]
    fn test_reset() {
        let sona_llm = SonaLlm::new(SonaLlmConfig {
            training: TrainingConfig {
                quality_threshold: 0.0,
                ..Default::default()
            },
            ..Default::default()
        });

        // Add some state
        sona_llm.instant_adapt("Query", "Response", 0.8);
        assert!(sona_llm.pending_count() > 0);

        // Reset
        sona_llm.reset();

        let stats = sona_llm.stats();
        assert_eq!(stats.instant_count, 0);
        assert_eq!(stats.pending_samples, 0);
    }

    #[test]
    fn test_deep_trigger() {
        let config = SonaLlmConfig {
            deep_trigger_threshold: 5.0, // Low threshold for testing
            training: TrainingConfig {
                quality_threshold: 0.0,
                ..Default::default()
            },
            background_interval_ms: 0,
            background_min_samples: 1,
            ..Default::default()
        };

        let sona_llm = SonaLlm::new(config);

        assert!(!sona_llm.should_trigger_deep());

        // Add samples to accumulate quality
        for _ in 0..10 {
            sona_llm.instant_adapt("Q", "R", 0.9);
            sona_llm.background_consolidate();
        }

        assert!(sona_llm.should_trigger_deep());
    }
}
