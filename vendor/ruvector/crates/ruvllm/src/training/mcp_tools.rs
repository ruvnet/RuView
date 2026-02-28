//! # MCP Tool Training Module
//!
//! This module provides training infrastructure for improving Claude Flow MCP tool calling
//! through GRPO-based reinforcement learning.
//!
//! ## Overview
//!
//! The MCP tool training system enables fine-tuning models to:
//! - Select the correct tool for a given task
//! - Generate appropriate parameters
//! - Handle errors and recover gracefully
//! - Learn from trajectories of tool use
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::training::{McpToolTrainer, GrpoConfig, TrainingConfig};
//!
//! // Create trainer with GRPO optimization
//! let grpo_config = GrpoConfig::for_tool_use();
//! let training_config = TrainingConfig::default();
//! let trainer = McpToolTrainer::new(grpo_config, training_config)?;
//!
//! // Load tool definitions
//! trainer.load_tool_definitions()?;
//!
//! // Train on trajectories
//! let result = trainer.train_on_trajectories(&trajectories)?;
//! println!("Training loss: {:.4}", result.avg_loss);
//!
//! // Evaluate accuracy
//! let accuracy = trainer.evaluate_tool_accuracy(&test_set)?;
//! println!("Tool selection accuracy: {:.2}%", accuracy * 100.0);
//! ```
//!
//! ## 140+ Claude Flow MCP Tools Supported
//!
//! The trainer supports all MCP tools in the Claude Flow ecosystem:
//! - Agent management (spawn, terminate, status, list, pool, health)
//! - Memory operations (store, retrieve, search, delete, list, stats)
//! - Swarm coordination (init, status, shutdown, health)
//! - Task management (create, status, list, complete, update, cancel)
//! - Hooks & learning (pre-task, post-task, route, metrics, etc.)
//! - Session management (save, restore, list, delete)
//! - Workflow (create, execute, status, list, pause, resume)
//! - System (status, metrics, health, info, reset)
//! - And many more...

use crate::error::{Result, RuvLLMError};
use crate::training::grpo::{GrpoConfig, GrpoOptimizer, GrpoSample, GrpoUpdateResult, SampleGroup};
use crate::training::tool_dataset::{
    DifficultyLevel, McpToolDef, ToolCallDataset, ToolCallExample, ToolDatasetConfig,
};
use ndarray::Array2;
use parking_lot::RwLock;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Configuration for MCP tool training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTrainingConfig {
    /// GRPO optimizer configuration
    pub grpo: GrpoConfig,
    /// Embedding dimension for tool representations
    pub embedding_dim: usize,
    /// Maximum sequence length
    pub max_seq_length: usize,
    /// Batch size for training
    pub batch_size: usize,
    /// Number of training epochs
    pub epochs: usize,
    /// Learning rate for supervised pretraining
    pub supervised_lr: f32,
    /// Warmup steps
    pub warmup_steps: usize,
    /// Evaluation frequency (steps)
    pub eval_frequency: usize,
    /// Checkpoint frequency (steps)
    pub checkpoint_frequency: usize,
    /// Random seed
    pub seed: u64,
    /// Enable mixed precision
    pub mixed_precision: bool,
    /// Gradient accumulation steps
    pub gradient_accumulation: usize,
    /// Maximum gradient norm
    pub max_grad_norm: f32,
    /// Label smoothing for supervised learning
    pub label_smoothing: f32,
    /// Weight decay
    pub weight_decay: f32,
    /// Include parameter prediction training
    pub train_params: bool,
    /// Include error recovery training
    pub train_error_recovery: bool,
}

impl Default for McpTrainingConfig {
    fn default() -> Self {
        Self {
            grpo: GrpoConfig::for_tool_use(),
            embedding_dim: 768,
            max_seq_length: 2048,
            batch_size: 16,
            epochs: 10,
            supervised_lr: 2e-5,
            warmup_steps: 500,
            eval_frequency: 100,
            checkpoint_frequency: 1000,
            seed: 42,
            mixed_precision: true,
            gradient_accumulation: 4,
            max_grad_norm: 1.0,
            label_smoothing: 0.1,
            weight_decay: 0.01,
            train_params: true,
            train_error_recovery: true,
        }
    }
}

impl McpTrainingConfig {
    /// Create config for quick experimentation
    pub fn quick() -> Self {
        Self {
            batch_size: 8,
            epochs: 3,
            eval_frequency: 50,
            checkpoint_frequency: 500,
            gradient_accumulation: 2,
            ..Default::default()
        }
    }

    /// Create config for production training
    pub fn production() -> Self {
        Self {
            batch_size: 32,
            epochs: 20,
            eval_frequency: 200,
            checkpoint_frequency: 2000,
            gradient_accumulation: 8,
            train_params: true,
            train_error_recovery: true,
            ..Default::default()
        }
    }
}

/// Tool trajectory for reinforcement learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTrajectory {
    /// Unique trajectory ID
    pub id: String,
    /// Task description that initiated this trajectory
    pub task: String,
    /// Sequence of tool calls in this trajectory
    pub steps: Vec<TrajectoryStep>,
    /// Final outcome (success/failure)
    pub success: bool,
    /// Total reward for trajectory
    pub total_reward: f32,
    /// Trajectory metadata
    pub metadata: TrajectoryMetadata,
}

/// A single step in a tool trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Tool that was called
    pub tool_name: String,
    /// Parameters passed to the tool
    pub parameters: serde_json::Value,
    /// State embedding before the call
    pub state_embedding: Vec<f32>,
    /// Log probability of this tool selection
    pub log_prob: f32,
    /// Reference log probability
    pub ref_log_prob: f32,
    /// Immediate reward for this step
    pub reward: f32,
    /// Whether this step completed successfully
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Next state embedding (after execution)
    pub next_state_embedding: Option<Vec<f32>>,
}

/// Metadata for a trajectory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrajectoryMetadata {
    /// Timestamp of trajectory start
    pub timestamp: u64,
    /// User ID (if available)
    pub user_id: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Task complexity
    pub complexity: Option<DifficultyLevel>,
    /// Domain type
    pub domain: Option<String>,
    /// Any additional context
    pub context: HashMap<String, String>,
}

/// Training result for a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    /// Average loss
    pub avg_loss: f32,
    /// Tool selection accuracy
    pub tool_accuracy: f32,
    /// Parameter accuracy (if trained)
    pub param_accuracy: Option<f32>,
    /// GRPO update results
    pub grpo_results: Vec<GrpoUpdateResult>,
    /// Number of samples processed
    pub samples_processed: usize,
    /// Training step
    pub step: u64,
    /// Gradient norm
    pub grad_norm: f32,
    /// Learning rate at this step
    pub learning_rate: f32,
}

/// Evaluation metrics for tool calling
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    /// Overall tool selection accuracy
    pub tool_accuracy: f32,
    /// Accuracy per tool category
    pub accuracy_by_category: HashMap<String, f32>,
    /// Accuracy per difficulty level
    pub accuracy_by_difficulty: HashMap<String, f32>,
    /// Parameter accuracy
    pub param_accuracy: f32,
    /// Error recovery rate
    pub error_recovery_rate: f32,
    /// Average reward
    pub avg_reward: f32,
    /// Number of evaluation samples
    pub num_samples: usize,
    /// Confusion matrix (predicted vs actual)
    pub confusion: HashMap<String, HashMap<String, usize>>,
}

/// Training statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrainingStats {
    /// Total training steps
    pub total_steps: u64,
    /// Total samples processed
    pub total_samples: u64,
    /// Total trajectories processed
    pub total_trajectories: u64,
    /// Average training loss
    pub avg_loss: f32,
    /// Best evaluation accuracy
    pub best_accuracy: f32,
    /// Current learning rate
    pub current_lr: f32,
    /// Training history (loss per step)
    pub loss_history: Vec<f32>,
    /// Evaluation history (accuracy per eval)
    pub eval_history: Vec<f32>,
}

/// MCP Tool Trainer for fine-tuning on tool calling
pub struct McpToolTrainer {
    /// Configuration
    config: McpTrainingConfig,
    /// GRPO optimizer
    grpo: GrpoOptimizer,
    /// Tool definitions
    tool_defs: Vec<McpToolDef>,
    /// Tool name to index mapping
    tool_to_idx: HashMap<String, usize>,
    /// Index to tool name mapping
    idx_to_tool: Vec<String>,
    /// Training statistics
    stats: RwLock<TrainingStats>,
    /// Current step
    step: AtomicU64,
    /// Random number generator
    rng: RwLock<StdRng>,
    /// Trajectory buffer
    trajectory_buffer: RwLock<Vec<ToolTrajectory>>,
    /// Tool embeddings (learned)
    tool_embeddings: RwLock<Array2<f32>>,
}

impl McpToolTrainer {
    /// Create a new MCP tool trainer
    pub fn new(config: McpTrainingConfig) -> Result<Self> {
        let grpo = GrpoOptimizer::new(config.grpo.clone());
        let rng = StdRng::seed_from_u64(config.seed);

        Ok(Self {
            config,
            grpo,
            tool_defs: Vec::new(),
            tool_to_idx: HashMap::new(),
            idx_to_tool: Vec::new(),
            stats: RwLock::new(TrainingStats::default()),
            step: AtomicU64::new(0),
            rng: RwLock::new(rng),
            trajectory_buffer: RwLock::new(Vec::new()),
            tool_embeddings: RwLock::new(Array2::zeros((0, 0))),
        })
    }

    /// Load tool definitions from the dataset generator
    pub fn load_tool_definitions(&mut self) -> Result<()> {
        let config = ToolDatasetConfig::minimal();
        let dataset = ToolCallDataset::generate(config)?;

        self.tool_defs = dataset.tool_definitions;

        // Build index mappings
        self.tool_to_idx.clear();
        self.idx_to_tool.clear();

        for (idx, tool) in self.tool_defs.iter().enumerate() {
            self.tool_to_idx.insert(tool.name.clone(), idx);
            self.idx_to_tool.push(tool.name.clone());
        }

        // Initialize tool embeddings randomly
        let num_tools = self.tool_defs.len();
        let embed_dim = self.config.embedding_dim;
        let mut rng = self.rng.write();

        let mut embeddings = Array2::zeros((num_tools, embed_dim));
        for i in 0..num_tools {
            for j in 0..embed_dim {
                embeddings[[i, j]] = rng.gen::<f32>() * 0.02 - 0.01; // Xavier-like init
            }
        }

        *self.tool_embeddings.write() = embeddings;

        Ok(())
    }

    /// Get number of tools
    pub fn num_tools(&self) -> usize {
        self.tool_defs.len()
    }

    /// Get tool index by name
    pub fn tool_index(&self, name: &str) -> Option<usize> {
        self.tool_to_idx.get(name).copied()
    }

    /// Get tool name by index
    pub fn tool_name(&self, idx: usize) -> Option<&str> {
        self.idx_to_tool.get(idx).map(|s| s.as_str())
    }

    /// Add a trajectory to the buffer
    pub fn add_trajectory(&self, trajectory: ToolTrajectory) {
        let mut buffer = self.trajectory_buffer.write();
        buffer.push(trajectory);

        // Update stats
        self.stats.write().total_trajectories += 1;
    }

    /// Train on a batch of trajectories using GRPO
    pub fn train_on_trajectories(
        &mut self,
        trajectories: &[ToolTrajectory],
    ) -> Result<TrainingResult> {
        if trajectories.is_empty() {
            return Err(RuvLLMError::InvalidOperation(
                "No trajectories provided for training".to_string(),
            ));
        }

        let mut all_samples = Vec::new();
        let mut all_groups = Vec::new();

        // Convert trajectories to GRPO samples and groups
        for trajectory in trajectories {
            let samples = self.trajectory_to_samples(trajectory)?;
            let group = SampleGroup::new(
                samples.clone(),
                self.step.load(Ordering::SeqCst),
                trajectory.task.clone(),
            );
            all_groups.push(group);
            all_samples.extend(samples);
        }

        // Add groups to GRPO optimizer
        for group in all_groups {
            self.grpo.add_group(group);
        }

        // Process groups and get update results
        let grpo_results = self.grpo.process_groups()?;

        // Compute aggregate metrics
        let avg_loss = if grpo_results.is_empty() {
            0.0
        } else {
            grpo_results.iter().map(|r| r.total_loss).sum::<f32>() / grpo_results.len() as f32
        };

        let step = self.step.fetch_add(1, Ordering::SeqCst);

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_steps = step + 1;
            stats.total_samples += all_samples.len() as u64;
            stats.avg_loss = (stats.avg_loss * 0.99) + (avg_loss * 0.01);
            stats.loss_history.push(avg_loss);
        }

        // Compute tool accuracy from samples
        let tool_accuracy = self.compute_batch_accuracy(&all_samples);

        Ok(TrainingResult {
            avg_loss,
            tool_accuracy,
            param_accuracy: if self.config.train_params {
                Some(self.compute_param_accuracy(&all_samples))
            } else {
                None
            },
            grpo_results,
            samples_processed: all_samples.len(),
            step,
            grad_norm: avg_loss.abs().sqrt(), // Simplified
            learning_rate: self.config.supervised_lr,
        })
    }

    /// Convert a trajectory to GRPO samples
    fn trajectory_to_samples(&self, trajectory: &ToolTrajectory) -> Result<Vec<GrpoSample>> {
        let mut samples = Vec::new();

        for (i, step) in trajectory.steps.iter().enumerate() {
            let action = self.tool_index(&step.tool_name).unwrap_or(0);
            let is_done = i == trajectory.steps.len() - 1;

            samples.push(GrpoSample {
                state: step.state_embedding.clone(),
                action,
                log_prob: step.log_prob,
                ref_log_prob: step.ref_log_prob,
                reward: step.reward,
                done: is_done,
                value: None,
                tool_name: step.tool_name.clone(),
                parameters: Some(step.parameters.clone()),
            });
        }

        Ok(samples)
    }

    /// Compute batch accuracy
    fn compute_batch_accuracy(&self, samples: &[GrpoSample]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let correct = samples.iter().filter(|s| s.reward > 0.5).count();
        correct as f32 / samples.len() as f32
    }

    /// Compute parameter accuracy
    fn compute_param_accuracy(&self, samples: &[GrpoSample]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        // Simplified: check if parameters are non-empty
        let valid = samples
            .iter()
            .filter(|s| {
                s.parameters
                    .as_ref()
                    .map(|p| p.is_object() && !p.as_object().unwrap().is_empty())
                    .unwrap_or(false)
            })
            .count();
        valid as f32 / samples.len() as f32
    }

    /// Evaluate tool selection accuracy on a test set
    pub fn evaluate_tool_accuracy(
        &self,
        test_examples: &[ToolCallExample],
    ) -> Result<EvaluationMetrics> {
        if test_examples.is_empty() {
            return Ok(EvaluationMetrics::default());
        }

        let mut metrics = EvaluationMetrics::default();
        let mut correct = 0;
        let mut by_category: HashMap<String, (usize, usize)> = HashMap::new(); // (correct, total)
        let mut by_difficulty: HashMap<String, (usize, usize)> = HashMap::new();
        let mut confusion: HashMap<String, HashMap<String, usize>> = HashMap::new();

        for example in test_examples {
            // Simulate prediction (in real use, this would call the model)
            let predicted = self.predict_tool(&example.prompt)?;

            let is_correct = predicted == example.expected_tool;
            if is_correct {
                correct += 1;
            }

            // Track by category
            let cat_key = example.category.name().to_string();
            let entry = by_category.entry(cat_key.clone()).or_insert((0, 0));
            if is_correct {
                entry.0 += 1;
            }
            entry.1 += 1;

            // Track by difficulty
            let diff_key = format!("{:?}", example.difficulty);
            let entry = by_difficulty.entry(diff_key.clone()).or_insert((0, 0));
            if is_correct {
                entry.0 += 1;
            }
            entry.1 += 1;

            // Update confusion matrix
            *confusion
                .entry(example.expected_tool.clone())
                .or_default()
                .entry(predicted)
                .or_insert(0) += 1;

            metrics.avg_reward += example.quality_score;
        }

        metrics.tool_accuracy = correct as f32 / test_examples.len() as f32;
        metrics.num_samples = test_examples.len();
        metrics.avg_reward /= test_examples.len() as f32;

        // Convert category stats
        for (cat, (c, t)) in by_category {
            metrics
                .accuracy_by_category
                .insert(cat, c as f32 / t as f32);
        }

        // Convert difficulty stats
        for (diff, (c, t)) in by_difficulty {
            metrics
                .accuracy_by_difficulty
                .insert(diff, c as f32 / t as f32);
        }

        metrics.confusion = confusion;

        // Update best accuracy in stats
        {
            let mut stats = self.stats.write();
            if metrics.tool_accuracy > stats.best_accuracy {
                stats.best_accuracy = metrics.tool_accuracy;
            }
            stats.eval_history.push(metrics.tool_accuracy);
        }

        Ok(metrics)
    }

    /// Predict the tool for a given prompt
    pub fn predict_tool(&self, prompt: &str) -> Result<String> {
        // Simple keyword-based prediction (in production, use the model)
        let prompt_lower = prompt.to_lowercase();

        // Check for tool-specific keywords
        for tool in &self.tool_defs {
            for use_case in &tool.use_cases {
                if prompt_lower.contains(&use_case.to_lowercase()) {
                    return Ok(tool.name.clone());
                }
            }
        }

        // Fallback to category-based matching
        if prompt_lower.contains("spawn") || prompt_lower.contains("agent") {
            return Ok("agent_spawn".to_string());
        }
        if prompt_lower.contains("memory") || prompt_lower.contains("store") {
            return Ok("memory_store".to_string());
        }
        if prompt_lower.contains("search") {
            return Ok("memory_search".to_string());
        }
        if prompt_lower.contains("swarm") || prompt_lower.contains("initialize") {
            return Ok("swarm_init".to_string());
        }
        if prompt_lower.contains("task") {
            return Ok("task_create".to_string());
        }
        if prompt_lower.contains("hook") || prompt_lower.contains("route") {
            return Ok("hooks_route".to_string());
        }

        // Default fallback
        Ok("system_status".to_string())
    }

    /// Generate a synthetic tool calling dataset
    pub fn generate_tool_dataset(&self, config: ToolDatasetConfig) -> Result<ToolCallDataset> {
        ToolCallDataset::generate(config)
    }

    /// Get training statistics
    pub fn stats(&self) -> TrainingStats {
        self.stats.read().clone()
    }

    /// Get GRPO optimizer statistics
    pub fn grpo_stats(&self) -> crate::training::grpo::GrpoStats {
        self.grpo.stats()
    }

    /// Reset the trainer state
    pub fn reset(&mut self) {
        self.grpo.reset();
        self.step.store(0, Ordering::SeqCst);
        *self.stats.write() = TrainingStats::default();
        self.trajectory_buffer.write().clear();
    }

    /// Get configuration
    pub fn config(&self) -> &McpTrainingConfig {
        &self.config
    }

    /// Get tool definitions
    pub fn tool_definitions(&self) -> &[McpToolDef] {
        &self.tool_defs
    }

    /// Create a reward function for tool calling
    pub fn compute_reward(
        &self,
        predicted_tool: &str,
        expected_tool: &str,
        params_correct: bool,
        execution_success: bool,
    ) -> f32 {
        let mut reward = 0.0;

        // Tool selection reward
        if predicted_tool == expected_tool {
            reward += 0.5;
        } else if self.same_category(predicted_tool, expected_tool) {
            reward += 0.2; // Partial credit for same category
        }

        // Parameter reward
        if params_correct {
            reward += 0.3;
        }

        // Execution reward
        if execution_success {
            reward += 0.2;
        }

        reward
    }

    /// Check if two tools are in the same category
    fn same_category(&self, tool1: &str, tool2: &str) -> bool {
        let cat1 = self
            .tool_defs
            .iter()
            .find(|t| t.name == tool1)
            .map(|t| t.category);
        let cat2 = self
            .tool_defs
            .iter()
            .find(|t| t.name == tool2)
            .map(|t| t.category);
        cat1.is_some() && cat1 == cat2
    }

    /// Train on the buffered trajectories
    pub fn train_buffered(&mut self) -> Result<Option<TrainingResult>> {
        let trajectories = {
            let mut buffer = self.trajectory_buffer.write();
            if buffer.is_empty() {
                return Ok(None);
            }
            std::mem::take(&mut *buffer)
        };

        let result = self.train_on_trajectories(&trajectories)?;
        Ok(Some(result))
    }

    /// Export training checkpoint
    pub fn export_checkpoint(&self) -> TrainingCheckpoint {
        TrainingCheckpoint {
            step: self.step.load(Ordering::SeqCst),
            stats: self.stats.read().clone(),
            grpo_stats: self.grpo.stats(),
            tool_embeddings: {
                let (vec, _offset) = self
                    .tool_embeddings
                    .read()
                    .clone()
                    .into_raw_vec_and_offset();
                vec
            },
            embedding_shape: {
                let emb = self.tool_embeddings.read();
                (emb.nrows(), emb.ncols())
            },
            config: self.config.clone(),
        }
    }

    /// Import training checkpoint
    pub fn import_checkpoint(&mut self, checkpoint: TrainingCheckpoint) -> Result<()> {
        self.step.store(checkpoint.step, Ordering::SeqCst);
        *self.stats.write() = checkpoint.stats;

        let (rows, cols) = checkpoint.embedding_shape;
        if checkpoint.tool_embeddings.len() == rows * cols {
            let embeddings = Array2::from_shape_vec((rows, cols), checkpoint.tool_embeddings)
                .map_err(|e| RuvLLMError::InvalidOperation(e.to_string()))?;
            *self.tool_embeddings.write() = embeddings;
        }

        self.config = checkpoint.config;

        Ok(())
    }
}

/// Training checkpoint for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingCheckpoint {
    /// Current step
    pub step: u64,
    /// Training statistics
    pub stats: TrainingStats,
    /// GRPO statistics
    pub grpo_stats: crate::training::grpo::GrpoStats,
    /// Tool embeddings (flattened)
    pub tool_embeddings: Vec<f32>,
    /// Embedding shape
    pub embedding_shape: (usize, usize),
    /// Configuration
    pub config: McpTrainingConfig,
}

/// Builder for creating trajectories
pub struct TrajectoryBuilder {
    id: String,
    task: String,
    steps: Vec<TrajectoryStep>,
    metadata: TrajectoryMetadata,
}

impl TrajectoryBuilder {
    /// Create a new trajectory builder
    pub fn new(id: impl Into<String>, task: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            task: task.into(),
            steps: Vec::new(),
            metadata: TrajectoryMetadata::default(),
        }
    }

    /// Add a step to the trajectory
    pub fn add_step(mut self, step: TrajectoryStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: TrajectoryMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set complexity
    pub fn with_complexity(mut self, complexity: DifficultyLevel) -> Self {
        self.metadata.complexity = Some(complexity);
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.metadata.session_id = Some(session_id.into());
        self
    }

    /// Build the trajectory
    pub fn build(self) -> ToolTrajectory {
        let success = self.steps.last().map(|s| s.success).unwrap_or(false);
        let total_reward = self.steps.iter().map(|s| s.reward).sum();

        ToolTrajectory {
            id: self.id,
            task: self.task,
            steps: self.steps,
            success,
            total_reward,
            metadata: self.metadata,
        }
    }
}

/// Builder for trajectory steps
pub struct StepBuilder {
    tool_name: String,
    parameters: serde_json::Value,
    state_embedding: Vec<f32>,
    log_prob: f32,
    ref_log_prob: f32,
    reward: f32,
    success: bool,
    error: Option<String>,
    duration_ms: u64,
    next_state_embedding: Option<Vec<f32>>,
}

impl StepBuilder {
    /// Create a new step builder
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            parameters: serde_json::Value::Object(serde_json::Map::new()),
            state_embedding: Vec::new(),
            log_prob: 0.0,
            ref_log_prob: 0.0,
            reward: 0.0,
            success: true,
            error: None,
            duration_ms: 0,
            next_state_embedding: None,
        }
    }

    /// Set parameters
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }

    /// Set state embedding
    pub fn with_state(mut self, embedding: Vec<f32>) -> Self {
        self.state_embedding = embedding;
        self
    }

    /// Set log probability
    pub fn with_log_prob(mut self, log_prob: f32) -> Self {
        self.log_prob = log_prob;
        self
    }

    /// Set reference log probability
    pub fn with_ref_log_prob(mut self, ref_log_prob: f32) -> Self {
        self.ref_log_prob = ref_log_prob;
        self
    }

    /// Set reward
    pub fn with_reward(mut self, reward: f32) -> Self {
        self.reward = reward;
        self
    }

    /// Set success status
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Set error message
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.success = false;
        self
    }

    /// Set duration
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Build the step
    pub fn build(self) -> TrajectoryStep {
        TrajectoryStep {
            tool_name: self.tool_name,
            parameters: self.parameters,
            state_embedding: self.state_embedding,
            log_prob: self.log_prob,
            ref_log_prob: self.ref_log_prob,
            reward: self.reward,
            success: self.success,
            error: self.error,
            duration_ms: self.duration_ms,
            next_state_embedding: self.next_state_embedding,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trainer_creation() {
        let config = McpTrainingConfig::default();
        let trainer = McpToolTrainer::new(config).unwrap();
        assert_eq!(trainer.num_tools(), 0);
    }

    #[test]
    fn test_load_tool_definitions() {
        let config = McpTrainingConfig::default();
        let mut trainer = McpToolTrainer::new(config).unwrap();

        trainer.load_tool_definitions().unwrap();

        assert!(trainer.num_tools() > 0);
        assert!(trainer.tool_index("agent_spawn").is_some());
        assert!(trainer.tool_index("memory_store").is_some());
    }

    #[test]
    fn test_predict_tool() {
        let config = McpTrainingConfig::default();
        let mut trainer = McpToolTrainer::new(config).unwrap();
        trainer.load_tool_definitions().unwrap();

        let prediction = trainer.predict_tool("spawn a coder agent").unwrap();
        assert_eq!(prediction, "agent_spawn");

        let prediction = trainer.predict_tool("store this in memory").unwrap();
        assert_eq!(prediction, "memory_store");
    }

    #[test]
    fn test_generate_dataset() {
        let config = McpTrainingConfig::default();
        let trainer = McpToolTrainer::new(config).unwrap();

        let dataset_config = ToolDatasetConfig::minimal();
        let dataset = trainer.generate_tool_dataset(dataset_config).unwrap();

        assert!(!dataset.examples.is_empty());
    }

    #[test]
    fn test_trajectory_builder() {
        let step1 = StepBuilder::new("agent_spawn")
            .with_params(serde_json::json!({"agentType": "coder"}))
            .with_state(vec![0.1, 0.2, 0.3])
            .with_reward(0.8)
            .build();

        let step2 = StepBuilder::new("task_create")
            .with_params(serde_json::json!({"type": "feature"}))
            .with_state(vec![0.4, 0.5, 0.6])
            .with_reward(0.9)
            .build();

        let trajectory = TrajectoryBuilder::new("traj-1", "implement authentication")
            .add_step(step1)
            .add_step(step2)
            .with_complexity(DifficultyLevel::Medium)
            .build();

        assert_eq!(trajectory.steps.len(), 2);
        assert!(trajectory.success);
        assert!((trajectory.total_reward - 1.7).abs() < 0.01);
    }

    #[test]
    fn test_compute_reward() {
        let config = McpTrainingConfig::default();
        let mut trainer = McpToolTrainer::new(config).unwrap();
        trainer.load_tool_definitions().unwrap();

        // Correct tool, correct params, success
        let reward = trainer.compute_reward("agent_spawn", "agent_spawn", true, true);
        assert!((reward - 1.0).abs() < 0.01);

        // Wrong tool, wrong params, failure
        let reward = trainer.compute_reward("memory_store", "agent_spawn", false, false);
        assert!(reward < 0.3); // Could get partial credit for same category
    }

    #[test]
    fn test_train_on_trajectories() {
        let config = McpTrainingConfig::quick();
        let mut trainer = McpToolTrainer::new(config).unwrap();
        trainer.load_tool_definitions().unwrap();

        let step = StepBuilder::new("agent_spawn")
            .with_params(serde_json::json!({"agentType": "coder"}))
            .with_state(vec![0.1; 768])
            .with_log_prob(-0.5)
            .with_ref_log_prob(-0.5)
            .with_reward(0.8)
            .build();

        let trajectory = TrajectoryBuilder::new("test-traj", "test task")
            .add_step(step)
            .build();

        let result = trainer.train_on_trajectories(&[trajectory]).unwrap();
        assert!(result.samples_processed > 0);
    }

    #[test]
    fn test_evaluate_accuracy() {
        let config = McpTrainingConfig::default();
        let mut trainer = McpToolTrainer::new(config).unwrap();
        trainer.load_tool_definitions().unwrap();

        // Generate test examples
        let dataset_config = ToolDatasetConfig::minimal();
        let dataset = trainer.generate_tool_dataset(dataset_config).unwrap();

        let metrics = trainer
            .evaluate_tool_accuracy(&dataset.examples[..5])
            .unwrap();
        assert!(metrics.num_samples == 5);
        assert!(metrics.tool_accuracy >= 0.0 && metrics.tool_accuracy <= 1.0);
    }

    #[test]
    fn test_checkpoint() {
        let config = McpTrainingConfig::default();
        let mut trainer = McpToolTrainer::new(config).unwrap();
        trainer.load_tool_definitions().unwrap();

        // Export checkpoint
        let checkpoint = trainer.export_checkpoint();
        assert_eq!(checkpoint.step, 0);

        // Create new trainer and import
        let config2 = McpTrainingConfig::default();
        let mut trainer2 = McpToolTrainer::new(config2).unwrap();
        trainer2.import_checkpoint(checkpoint).unwrap();

        assert_eq!(trainer2.step.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_add_trajectory_to_buffer() {
        let config = McpTrainingConfig::default();
        let trainer = McpToolTrainer::new(config).unwrap();

        let trajectory = TrajectoryBuilder::new("buf-traj", "buffer test")
            .add_step(StepBuilder::new("system_status").build())
            .build();

        trainer.add_trajectory(trajectory);

        assert_eq!(trainer.stats().total_trajectories, 1);
    }

    #[test]
    fn test_same_category() {
        let config = McpTrainingConfig::default();
        let mut trainer = McpToolTrainer::new(config).unwrap();
        trainer.load_tool_definitions().unwrap();

        // Same category
        assert!(trainer.same_category("memory_store", "memory_search"));

        // Different categories
        assert!(!trainer.same_category("memory_store", "agent_spawn"));
    }
}
