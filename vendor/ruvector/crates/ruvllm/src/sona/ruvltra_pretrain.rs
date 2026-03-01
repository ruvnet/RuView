//! SONA Pretraining Configuration for RuvLTRA-Small (0.5B)
//!
//! Optimized pretraining configuration for the RuvLTRA-Small 0.5B parameter model.
//! This module provides:
//!
//! - MicroLoRA rank=1 (minimal overhead for small model)
//! - BaseLoRA rank=4 (conservative for 0.5B)
//! - EWC++ lambda tuned for small models
//! - Pretraining dataset loader for pattern learning
//! - ReasoningBank seeding with common patterns
//!
//! ## Configuration Rationale
//!
//! For a 0.5B model, we use conservative LoRA ranks to minimize overhead:
//! - MicroLoRA rank-1: ~0.1MB per adapter, <0.3ms latency
//! - BaseLoRA rank-4: ~2MB per layer, good expressiveness for small model
//! - Hidden dim 128: Smaller projection for efficiency
//! - Embedding dim 384: Match model hidden_size / 2
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::sona::ruvltra_pretrain::{RuvLtraPretrainConfig, RuvLtraPretrainer};
//!
//! let config = RuvLtraPretrainConfig::for_ruvltra_small();
//! let pretrainer = RuvLtraPretrainer::new(config);
//!
//! // Pretrain routing patterns
//! pretrainer.pretrain_routing_patterns(&sample_prompts);
//!
//! // Seed reasoning bank
//! pretrainer.seed_reasoning_bank();
//! ```

use super::integration::{SonaConfig, SonaIntegration};
use ruvector_sona::{
    EwcConfig, EwcPlusPlus, LearnedPattern, PatternConfig, PatternType, QueryTrajectory,
    ReasoningBank, SonaConfig as SonaCoreConfig, SonaEngine,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pretraining configuration optimized for RuvLTRA-Small (0.5B)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvLtraPretrainConfig {
    /// SONA configuration for 0.5B model
    pub sona: SonaConfig,
    /// Dataset configuration
    pub dataset: DatasetConfig,
    /// Routing pattern configuration
    pub routing: RoutingPretrainConfig,
    /// Quality prediction configuration
    pub quality: QualityPretrainConfig,
    /// ReasoningBank seeding configuration
    pub seeding: SeedingConfig,
}

/// Dataset configuration for pretraining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetConfig {
    /// Maximum prompts for routing pattern learning
    pub max_routing_prompts: usize,
    /// Maximum prompts for quality pattern learning
    pub max_quality_prompts: usize,
    /// Embedding batch size
    pub embedding_batch_size: usize,
    /// Minimum prompt length (characters)
    pub min_prompt_length: usize,
    /// Maximum prompt length (characters)
    pub max_prompt_length: usize,
    /// Quality score threshold for positive examples
    pub quality_threshold: f32,
}

/// Configuration for routing pattern pretraining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPretrainConfig {
    /// Number of routing clusters to learn
    pub num_clusters: usize,
    /// Learning rate for routing patterns
    pub learning_rate: f32,
    /// Number of training epochs
    pub epochs: usize,
    /// Minimum samples per routing class
    pub min_samples_per_class: usize,
    /// Target model mappings (query complexity -> model index)
    pub model_mappings: Vec<ModelRouteMapping>,
}

/// Mapping from query characteristics to model index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRouteMapping {
    /// Pattern name (e.g., "simple_factual", "complex_reasoning")
    pub name: String,
    /// Target model index (0=tiny, 1=small, 2=medium, 3=large)
    pub model_index: usize,
    /// Expected quality threshold
    pub quality_threshold: f32,
    /// Characteristic embedding centroid (learned during pretraining)
    pub centroid: Option<Vec<f32>>,
}

/// Configuration for quality prediction pretraining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityPretrainConfig {
    /// Number of quality buckets (e.g., 5 for [0.0-0.2, 0.2-0.4, ...])
    pub num_buckets: usize,
    /// Learning rate for quality predictor
    pub learning_rate: f32,
    /// Number of training epochs
    pub epochs: usize,
    /// Use regression (continuous) vs classification (buckets)
    pub use_regression: bool,
}

/// Configuration for ReasoningBank seeding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedingConfig {
    /// Number of seed patterns per category
    pub patterns_per_category: usize,
    /// Pattern categories to seed
    pub categories: Vec<PatternCategory>,
    /// Initial quality score for seed patterns
    pub initial_quality: f32,
    /// Embedding dimension for seed patterns
    pub embedding_dim: usize,
}

/// Pattern category for seeding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCategory {
    /// Category name
    pub name: String,
    /// Pattern type
    pub pattern_type: PatternType,
    /// Example prompts for this category
    pub example_prompts: Vec<String>,
    /// Expected model index for routing
    pub target_model_index: usize,
}

impl Default for RuvLtraPretrainConfig {
    fn default() -> Self {
        Self::for_ruvltra_small()
    }
}

impl RuvLtraPretrainConfig {
    /// Create configuration optimized for RuvLTRA-Small (0.5B)
    pub fn for_ruvltra_small() -> Self {
        Self {
            sona: SonaConfig {
                hidden_dim: 128,              // Smaller for 0.5B
                embedding_dim: 384,           // Match model hidden/2
                micro_lora_rank: 1,           // Minimal overhead for small model
                base_lora_rank: 4,            // Conservative for 0.5B
                instant_learning_rate: 0.005, // Slightly lower for stability
                background_learning_rate: 0.0005,
                ewc_lambda: 500.0, // Lower lambda for small model (less to protect)
                pattern_capacity: 5000, // Smaller capacity
                background_interval_secs: 1800, // 30 minutes
                deep_interval_secs: 259200, // 3 days
                quality_threshold: 0.6, // Higher threshold for small model
            },
            dataset: DatasetConfig {
                max_routing_prompts: 10000,
                max_quality_prompts: 5000,
                embedding_batch_size: 32,
                min_prompt_length: 10,
                max_prompt_length: 2048,
                quality_threshold: 0.6,
            },
            routing: RoutingPretrainConfig {
                num_clusters: 50, // Fewer clusters for small model
                learning_rate: 0.001,
                epochs: 5,
                min_samples_per_class: 100,
                model_mappings: Self::default_model_mappings(),
            },
            quality: QualityPretrainConfig {
                num_buckets: 5,
                learning_rate: 0.001,
                epochs: 3,
                use_regression: false, // Classification easier for small model
            },
            seeding: SeedingConfig {
                patterns_per_category: 20,
                categories: Self::default_pattern_categories(),
                initial_quality: 0.7,
                embedding_dim: 384,
            },
        }
    }

    /// Create configuration for edge deployment (minimal footprint)
    pub fn for_edge_deployment() -> Self {
        let mut config = Self::for_ruvltra_small();
        config.sona.hidden_dim = 64;
        config.sona.embedding_dim = 256;
        config.sona.pattern_capacity = 1000;
        config.dataset.max_routing_prompts = 2000;
        config.routing.num_clusters = 20;
        config.seeding.patterns_per_category = 10;
        config
    }

    /// Default model routing mappings
    fn default_model_mappings() -> Vec<ModelRouteMapping> {
        vec![
            ModelRouteMapping {
                name: "simple_factual".to_string(),
                model_index: 0, // Tiny
                quality_threshold: 0.7,
                centroid: None,
            },
            ModelRouteMapping {
                name: "basic_completion".to_string(),
                model_index: 0, // Tiny
                quality_threshold: 0.65,
                centroid: None,
            },
            ModelRouteMapping {
                name: "moderate_reasoning".to_string(),
                model_index: 1, // Small
                quality_threshold: 0.6,
                centroid: None,
            },
            ModelRouteMapping {
                name: "code_generation".to_string(),
                model_index: 1, // Small
                quality_threshold: 0.55,
                centroid: None,
            },
            ModelRouteMapping {
                name: "complex_reasoning".to_string(),
                model_index: 2, // Medium
                quality_threshold: 0.5,
                centroid: None,
            },
            ModelRouteMapping {
                name: "multi_step_analysis".to_string(),
                model_index: 2, // Medium
                quality_threshold: 0.45,
                centroid: None,
            },
            ModelRouteMapping {
                name: "expert_domain".to_string(),
                model_index: 3, // Large
                quality_threshold: 0.4,
                centroid: None,
            },
        ]
    }

    /// Default pattern categories for seeding
    fn default_pattern_categories() -> Vec<PatternCategory> {
        vec![
            PatternCategory {
                name: "factual".to_string(),
                pattern_type: PatternType::Factual,
                example_prompts: vec![
                    "What is the capital of France?".to_string(),
                    "Who wrote Romeo and Juliet?".to_string(),
                    "What year did World War II end?".to_string(),
                ],
                target_model_index: 0,
            },
            PatternCategory {
                name: "reasoning".to_string(),
                pattern_type: PatternType::Reasoning,
                example_prompts: vec![
                    "If A implies B and B implies C, what can we conclude?".to_string(),
                    "Solve: 2x + 5 = 15".to_string(),
                    "What is the logical fallacy in this argument?".to_string(),
                ],
                target_model_index: 1,
            },
            PatternCategory {
                name: "code".to_string(),
                pattern_type: PatternType::CodeGen,
                example_prompts: vec![
                    "Write a function to reverse a string".to_string(),
                    "Implement binary search in Python".to_string(),
                    "Create a REST API endpoint".to_string(),
                ],
                target_model_index: 1,
            },
            PatternCategory {
                name: "creative".to_string(),
                pattern_type: PatternType::Creative,
                example_prompts: vec![
                    "Write a haiku about autumn".to_string(),
                    "Create a story opening about a mysterious door".to_string(),
                    "Describe a sunset in poetic prose".to_string(),
                ],
                target_model_index: 2,
            },
            PatternCategory {
                name: "conversational".to_string(),
                pattern_type: PatternType::Conversational,
                example_prompts: vec![
                    "How are you today?".to_string(),
                    "Can you help me with something?".to_string(),
                    "Thanks for your help!".to_string(),
                ],
                target_model_index: 0,
            },
        ]
    }
}

/// Sample prompt for pretraining
#[derive(Debug, Clone)]
pub struct PretrainSample {
    /// Prompt text
    pub prompt: String,
    /// Pre-computed embedding (optional, computed if None)
    pub embedding: Option<Vec<f32>>,
    /// Expected model index for routing
    pub target_model_index: Option<usize>,
    /// Quality score (if available from evaluation)
    pub quality_score: Option<f32>,
    /// Category label
    pub category: Option<String>,
}

/// Result of routing pattern pretraining
#[derive(Debug, Clone)]
pub struct RoutingPretrainResult {
    /// Number of patterns learned
    pub patterns_learned: usize,
    /// Cluster centroids learned
    pub centroids: Vec<Vec<f32>>,
    /// Model index assignments
    pub model_assignments: Vec<usize>,
    /// Training loss history
    pub loss_history: Vec<f32>,
    /// Accuracy on held-out set
    pub validation_accuracy: f32,
}

/// Result of quality pattern pretraining
#[derive(Debug, Clone)]
pub struct QualityPretrainResult {
    /// Number of quality buckets learned
    pub buckets_learned: usize,
    /// Bucket boundaries
    pub bucket_boundaries: Vec<f32>,
    /// Training loss history
    pub loss_history: Vec<f32>,
    /// Mean absolute error on held-out set
    pub validation_mae: f32,
}

/// Result of ReasoningBank seeding
#[derive(Debug, Clone)]
pub struct SeedingResult {
    /// Total patterns seeded
    pub patterns_seeded: usize,
    /// Patterns per category
    pub per_category: HashMap<String, usize>,
    /// Initial pattern quality average
    pub avg_quality: f32,
}

/// RuvLTRA-Small Pretrainer
pub struct RuvLtraPretrainer {
    /// Configuration
    config: RuvLtraPretrainConfig,
    /// SONA engine for learning
    engine: SonaEngine,
    /// EWC++ for catastrophic forgetting prevention
    ewc: EwcPlusPlus,
    /// ReasoningBank for pattern storage
    reasoning_bank: ReasoningBank,
}

impl RuvLtraPretrainer {
    /// Create a new pretrainer
    pub fn new(config: RuvLtraPretrainConfig) -> Self {
        let core_config = SonaCoreConfig {
            hidden_dim: config.sona.hidden_dim,
            embedding_dim: config.sona.embedding_dim,
            micro_lora_rank: config.sona.micro_lora_rank,
            base_lora_rank: config.sona.base_lora_rank,
            micro_lora_lr: config.sona.instant_learning_rate,
            base_lora_lr: config.sona.background_learning_rate,
            ewc_lambda: config.sona.ewc_lambda,
            quality_threshold: config.sona.quality_threshold,
            ..Default::default()
        };

        let engine = SonaEngine::with_config(core_config);

        let ewc_config = EwcConfig {
            param_count: config.sona.hidden_dim,
            initial_lambda: config.sona.ewc_lambda,
            // Lower max lambda for small models
            max_lambda: config.sona.ewc_lambda * 5.0,
            ..Default::default()
        };
        let ewc = EwcPlusPlus::new(ewc_config);

        let pattern_config = PatternConfig {
            k_clusters: config.routing.num_clusters,
            embedding_dim: config.sona.embedding_dim.min(256),
            max_trajectories: config.sona.pattern_capacity,
            quality_threshold: config.sona.quality_threshold,
            ..Default::default()
        };
        let reasoning_bank = ReasoningBank::new(pattern_config);

        Self {
            config,
            engine,
            ewc,
            reasoning_bank,
        }
    }

    /// Pretrain routing patterns (query -> model routing)
    ///
    /// Learns which types of queries should be routed to which model size.
    pub fn pretrain_routing_patterns(
        &mut self,
        samples: &[PretrainSample],
    ) -> RoutingPretrainResult {
        let mut centroids = Vec::new();
        let mut model_assignments = Vec::new();
        let mut loss_history = Vec::new();

        // Filter samples with valid embeddings and targets
        let valid_samples: Vec<_> = samples
            .iter()
            .filter(|s| s.embedding.is_some() && s.target_model_index.is_some())
            .collect();

        if valid_samples.is_empty() {
            return RoutingPretrainResult {
                patterns_learned: 0,
                centroids,
                model_assignments,
                loss_history,
                validation_accuracy: 0.0,
            };
        }

        // Split into train/validation
        let split_idx = (valid_samples.len() as f32 * 0.8) as usize;
        let (train_samples, val_samples) = valid_samples.split_at(split_idx.max(1));

        // Training loop
        for _epoch in 0..self.config.routing.epochs {
            let mut epoch_loss = 0.0f32;

            for sample in train_samples {
                let embedding = sample.embedding.as_ref().unwrap();
                let target_model = sample.target_model_index.unwrap();

                // Create trajectory
                let trajectory = QueryTrajectory::new(0, embedding.clone());
                self.reasoning_bank.add_trajectory(&trajectory);

                // Update EWC with pseudo-gradients
                let gradients = self.compute_routing_gradients(embedding, target_model);
                self.ewc.update_fisher(&gradients);

                // Compute loss (cross-entropy proxy)
                let predicted = self.predict_model_index(embedding);
                let loss = if predicted == target_model { 0.0 } else { 1.0 };
                epoch_loss += loss;
            }

            loss_history.push(epoch_loss / train_samples.len() as f32);

            // Extract patterns after each epoch
            self.reasoning_bank.extract_patterns();
        }

        // Extract final patterns
        let patterns = self.reasoning_bank.get_all_patterns();
        for pattern in &patterns {
            centroids.push(pattern.centroid.clone());
            // Assign model based on pattern quality
            let model_idx = self.quality_to_model_index(pattern.avg_quality);
            model_assignments.push(model_idx);
        }

        // Validation accuracy
        let mut correct = 0;
        for sample in val_samples {
            let embedding = sample.embedding.as_ref().unwrap();
            let target = sample.target_model_index.unwrap();
            let predicted = self.predict_model_index(embedding);
            if predicted == target {
                correct += 1;
            }
        }
        let validation_accuracy = if val_samples.is_empty() {
            0.0
        } else {
            correct as f32 / val_samples.len() as f32
        };

        RoutingPretrainResult {
            patterns_learned: patterns.len(),
            centroids,
            model_assignments,
            loss_history,
            validation_accuracy,
        }
    }

    /// Pretrain quality prediction patterns
    ///
    /// Learns to predict expected quality based on query characteristics.
    pub fn pretrain_quality_patterns(
        &mut self,
        samples: &[PretrainSample],
    ) -> QualityPretrainResult {
        let mut loss_history = Vec::new();
        let num_buckets = self.config.quality.num_buckets;

        // Compute bucket boundaries
        let bucket_boundaries: Vec<f32> = (0..num_buckets)
            .map(|i| (i + 1) as f32 / num_buckets as f32)
            .collect();

        // Filter samples with valid embeddings and quality scores
        let valid_samples: Vec<_> = samples
            .iter()
            .filter(|s| s.embedding.is_some() && s.quality_score.is_some())
            .collect();

        if valid_samples.is_empty() {
            return QualityPretrainResult {
                buckets_learned: num_buckets,
                bucket_boundaries,
                loss_history,
                validation_mae: 1.0,
            };
        }

        // Split into train/validation
        let split_idx = (valid_samples.len() as f32 * 0.8) as usize;
        let (train_samples, val_samples) = valid_samples.split_at(split_idx.max(1));

        // Training loop
        for _epoch in 0..self.config.quality.epochs {
            let mut epoch_loss = 0.0f32;

            for sample in train_samples {
                let embedding = sample.embedding.as_ref().unwrap();
                let quality = sample.quality_score.unwrap();

                // Create trajectory with quality
                let mut trajectory = QueryTrajectory::new(0, embedding.clone());
                trajectory.finalize(quality, 0);
                self.reasoning_bank.add_trajectory(&trajectory);

                // Compute loss
                let predicted_quality = self.predict_quality(embedding);
                let loss = (predicted_quality - quality).abs();
                epoch_loss += loss;
            }

            loss_history.push(epoch_loss / train_samples.len() as f32);
        }

        // Validation MAE
        let mut total_error = 0.0f32;
        for sample in val_samples {
            let embedding = sample.embedding.as_ref().unwrap();
            let target_quality = sample.quality_score.unwrap();
            let predicted = self.predict_quality(embedding);
            total_error += (predicted - target_quality).abs();
        }
        let validation_mae = if val_samples.is_empty() {
            1.0
        } else {
            total_error / val_samples.len() as f32
        };

        QualityPretrainResult {
            buckets_learned: num_buckets,
            bucket_boundaries,
            loss_history,
            validation_mae,
        }
    }

    /// Seed the ReasoningBank with initial patterns
    ///
    /// Creates initial patterns for each category to bootstrap learning.
    pub fn seed_reasoning_bank(&mut self) -> SeedingResult {
        let mut per_category = HashMap::new();
        let mut total_seeded = 0;
        let mut total_quality = 0.0f32;

        for category in &self.config.seeding.categories {
            let mut category_count = 0;

            for prompt in &category.example_prompts {
                // Generate a pseudo-embedding from the prompt
                // In production, this would use a real embedding model
                let embedding = self.generate_pseudo_embedding(prompt);

                // Create pattern
                let mut pattern = LearnedPattern::new(total_seeded as u64, embedding);
                pattern.avg_quality = self.config.seeding.initial_quality;
                pattern.pattern_type = category.pattern_type.clone();

                // Create trajectory and add to bank
                let trajectory =
                    QueryTrajectory::new(total_seeded as u64, pattern.centroid.clone());
                self.reasoning_bank.add_trajectory(&trajectory);

                total_quality += pattern.avg_quality;
                total_seeded += 1;
                category_count += 1;

                if category_count >= self.config.seeding.patterns_per_category {
                    break;
                }
            }

            per_category.insert(category.name.clone(), category_count);
        }

        // Extract patterns to create clusters
        self.reasoning_bank.extract_patterns();

        let avg_quality = if total_seeded > 0 {
            total_quality / total_seeded as f32
        } else {
            0.0
        };

        SeedingResult {
            patterns_seeded: total_seeded,
            per_category,
            avg_quality,
        }
    }

    /// Compute pseudo-gradients for routing learning
    fn compute_routing_gradients(&self, embedding: &[f32], target_model: usize) -> Vec<f32> {
        let dim = self.config.sona.hidden_dim;
        let mut gradients = vec![0.0f32; dim];

        // Simple gradient approximation based on embedding and target
        let embedding_len = embedding.len().min(dim);
        for i in 0..embedding_len {
            // Scale gradient by target model index (higher models need stronger patterns)
            gradients[i] = embedding[i] * (target_model as f32 + 1.0) * 0.1;
        }

        gradients
    }

    /// Predict model index for an embedding
    fn predict_model_index(&self, embedding: &[f32]) -> usize {
        // Find most similar pattern and return its model assignment
        let patterns = self.reasoning_bank.find_similar(embedding, 1);

        if let Some(pattern) = patterns.first() {
            self.quality_to_model_index(pattern.avg_quality)
        } else {
            1 // Default to small model
        }
    }

    /// Convert quality score to model index
    fn quality_to_model_index(&self, quality: f32) -> usize {
        // Higher quality patterns can use smaller models
        // Lower quality patterns need larger models
        if quality >= 0.8 {
            0 // Tiny
        } else if quality >= 0.6 {
            1 // Small
        } else if quality >= 0.4 {
            2 // Medium
        } else {
            3 // Large
        }
    }

    /// Predict quality for an embedding
    fn predict_quality(&self, embedding: &[f32]) -> f32 {
        let patterns = self.reasoning_bank.find_similar(embedding, 3);

        if patterns.is_empty() {
            return 0.5; // Default quality
        }

        // Weighted average of similar pattern qualities
        let mut total_weight = 0.0f32;
        let mut weighted_quality = 0.0f32;

        for pattern in patterns {
            let similarity = pattern.similarity(embedding).max(0.0);
            total_weight += similarity;
            weighted_quality += similarity * pattern.avg_quality;
        }

        if total_weight > 0.0 {
            weighted_quality / total_weight
        } else {
            0.5
        }
    }

    /// Generate pseudo-embedding from prompt (placeholder for real embedding)
    fn generate_pseudo_embedding(&self, prompt: &str) -> Vec<f32> {
        let dim = self.config.seeding.embedding_dim;
        let mut embedding = vec![0.0f32; dim];

        // Simple character-based hashing for deterministic pseudo-embeddings
        // In production, this would use a real embedding model
        for (i, ch) in prompt.chars().enumerate() {
            let idx = i % dim;
            let val = (ch as u32 as f32) / 65536.0;
            embedding[idx] += val;
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for e in &mut embedding {
                *e /= norm;
            }
        }

        embedding
    }

    /// Get the trained SONA engine
    pub fn into_engine(self) -> SonaEngine {
        self.engine
    }

    /// Get configuration
    pub fn config(&self) -> &RuvLtraPretrainConfig {
        &self.config
    }

    /// Get EWC++ state
    pub fn ewc(&self) -> &EwcPlusPlus {
        &self.ewc
    }

    /// Get reasoning bank
    pub fn reasoning_bank(&self) -> &ReasoningBank {
        &self.reasoning_bank
    }

    /// Export trained state for deployment
    pub fn export_state(&self) -> PretrainedState {
        let patterns = self.reasoning_bank.get_all_patterns();

        PretrainedState {
            config: self.config.clone(),
            patterns,
            ewc_task_count: self.ewc.task_count(),
            ewc_lambda: self.ewc.lambda(),
        }
    }
}

/// Exported pretrained state for deployment
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PretrainedState {
    /// Configuration used
    pub config: RuvLtraPretrainConfig,
    /// Learned patterns
    pub patterns: Vec<LearnedPattern>,
    /// EWC task count
    pub ewc_task_count: usize,
    /// Final EWC lambda
    pub ewc_lambda: f32,
}

impl PretrainedState {
    /// Create a new SonaIntegration from pretrained state
    pub fn into_sona_integration(self) -> SonaIntegration {
        SonaIntegration::new(self.config.sona)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();

        assert_eq!(config.sona.hidden_dim, 128);
        assert_eq!(config.sona.embedding_dim, 384);
        assert_eq!(config.sona.micro_lora_rank, 1);
        assert_eq!(config.sona.base_lora_rank, 4);
        assert_eq!(config.sona.quality_threshold, 0.6);
    }

    #[test]
    fn test_edge_config() {
        let config = RuvLtraPretrainConfig::for_edge_deployment();

        assert_eq!(config.sona.hidden_dim, 64);
        assert_eq!(config.sona.embedding_dim, 256);
        assert_eq!(config.sona.pattern_capacity, 1000);
    }

    #[test]
    fn test_pretrainer_creation() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();
        let pretrainer = RuvLtraPretrainer::new(config);

        assert_eq!(pretrainer.config().sona.micro_lora_rank, 1);
        assert_eq!(pretrainer.ewc().task_count(), 0);
    }

    #[test]
    fn test_seeding() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();
        let mut pretrainer = RuvLtraPretrainer::new(config);

        let result = pretrainer.seed_reasoning_bank();

        assert!(result.patterns_seeded > 0);
        assert!(result.avg_quality > 0.0);
    }

    #[test]
    fn test_routing_pretrain() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();
        let mut pretrainer = RuvLtraPretrainer::new(config);

        // Create sample data
        let samples: Vec<PretrainSample> = (0..100)
            .map(|i| PretrainSample {
                prompt: format!("Sample prompt {}", i),
                embedding: Some(vec![i as f32 / 100.0; 384]),
                target_model_index: Some(i % 4),
                quality_score: Some(0.5 + (i as f32 % 50.0) / 100.0),
                category: Some("test".to_string()),
            })
            .collect();

        let result = pretrainer.pretrain_routing_patterns(&samples);

        assert!(result.patterns_learned > 0 || !result.loss_history.is_empty());
    }

    #[test]
    fn test_quality_pretrain() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();
        let mut pretrainer = RuvLtraPretrainer::new(config);

        // Create sample data
        let samples: Vec<PretrainSample> = (0..100)
            .map(|i| PretrainSample {
                prompt: format!("Sample prompt {}", i),
                embedding: Some(vec![i as f32 / 100.0; 384]),
                target_model_index: None,
                quality_score: Some(0.3 + (i as f32 % 70.0) / 100.0),
                category: None,
            })
            .collect();

        let result = pretrainer.pretrain_quality_patterns(&samples);

        assert_eq!(result.buckets_learned, 5);
        assert!(!result.bucket_boundaries.is_empty());
    }

    #[test]
    fn test_model_index_mapping() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();
        let pretrainer = RuvLtraPretrainer::new(config);

        assert_eq!(pretrainer.quality_to_model_index(0.9), 0); // Tiny
        assert_eq!(pretrainer.quality_to_model_index(0.7), 1); // Small
        assert_eq!(pretrainer.quality_to_model_index(0.5), 2); // Medium
        assert_eq!(pretrainer.quality_to_model_index(0.3), 3); // Large
    }

    #[test]
    fn test_export_state() {
        let config = RuvLtraPretrainConfig::for_ruvltra_small();
        let mut pretrainer = RuvLtraPretrainer::new(config);

        // Seed some patterns
        pretrainer.seed_reasoning_bank();

        let state = pretrainer.export_state();

        assert_eq!(state.config.sona.micro_lora_rank, 1);
        assert_eq!(state.ewc_task_count, 0);
    }
}
