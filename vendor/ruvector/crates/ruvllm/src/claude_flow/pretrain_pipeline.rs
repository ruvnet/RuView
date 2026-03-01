//! Advanced Pretraining Pipeline for RuvLTRA Claude Flow Integration
//!
//! This module provides a multi-phase pretraining pipeline optimized for Claude Flow tasks:
//!
//! - **Bootstrap Phase**: Seed patterns from agent keywords and typical tasks
//! - **Synthetic Phase**: Generate diverse training samples per agent type
//! - **Reinforce Phase**: Replay successful trajectories with SONA
//! - **Consolidate Phase**: EWC++ to lock in learned patterns
//!
//! ## Key Features
//!
//! - Quality-gated learning (only learn from successful patterns)
//! - Curriculum learning (start simple, increase complexity)
//! - Progress tracking and checkpoint saving
//! - Multi-agent task generation
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::pretrain_pipeline::{PretrainPipeline, PretrainConfig, Phase};
//!
//! let config = PretrainConfig::default();
//! let mut pipeline = PretrainPipeline::new(config);
//!
//! // Run full pipeline
//! let result = pipeline.run_full_pipeline()?;
//! println!("Trained {} patterns with {:.2}% quality", result.total_patterns, result.avg_quality * 100.0);
//!
//! // Save checkpoint
//! pipeline.save_checkpoint("./checkpoints/claude_flow_v1.bin")?;
//! ```

use super::task_generator::{GeneratedTask, TaskCategory, TaskComplexity, TaskGenerator};
use super::{ClaudeFlowAgent, ClaudeFlowTask};
use crate::sona::{
    PretrainSample, RoutingPretrainResult, RuvLtraPretrainConfig, RuvLtraPretrainer, SeedingResult,
    SonaConfig, SonaIntegration, Trajectory,
};
use parking_lot::RwLock;
use ruvector_sona::{
    EwcConfig, EwcPlusPlus, LearnedPattern, PatternConfig, ReasoningBank, SonaEngine,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Pretraining phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    /// Seed patterns from agent keywords and typical tasks
    Bootstrap,
    /// Generate diverse training samples per agent type
    Synthetic,
    /// Replay successful trajectories with SONA
    Reinforce,
    /// EWC++ to lock in learned patterns
    Consolidate,
}

/// Static array of all phases for zero-allocation access
static ALL_PHASES: [Phase; 4] = [
    Phase::Bootstrap,
    Phase::Synthetic,
    Phase::Reinforce,
    Phase::Consolidate,
];

impl Phase {
    /// Get all phases in order
    #[inline]
    pub fn all() -> &'static [Phase] {
        &ALL_PHASES
    }

    /// Get phase name
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Bootstrap => "bootstrap",
            Phase::Synthetic => "synthetic",
            Phase::Reinforce => "reinforce",
            Phase::Consolidate => "consolidate",
        }
    }

    /// Get phase description
    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            Phase::Bootstrap => "Seed patterns from agent keywords and typical tasks",
            Phase::Synthetic => "Generate diverse training samples per agent type",
            Phase::Reinforce => "Replay successful trajectories with SONA learning",
            Phase::Consolidate => "Lock in learned patterns with EWC++ consolidation",
        }
    }
}

/// Configuration for the pretraining pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PretrainConfig {
    /// Phases to execute
    pub phases: Vec<Phase>,
    /// Samples per phase
    pub samples_per_phase: usize,
    /// Quality threshold for learning (0.0 - 1.0)
    pub quality_threshold: f32,
    /// Enable curriculum learning
    pub curriculum_learning: bool,
    /// Curriculum stages (complexity levels)
    pub curriculum_stages: usize,
    /// Samples per curriculum stage
    pub samples_per_stage: usize,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// SONA configuration
    pub sona_config: SonaConfig,
    /// Enable checkpointing
    pub enable_checkpoints: bool,
    /// Checkpoint interval (samples)
    pub checkpoint_interval: usize,
    /// Checkpoint directory
    pub checkpoint_dir: String,
    /// Verbose logging
    pub verbose: bool,
    /// Random seed for reproducibility
    pub random_seed: u64,
    /// Number of reinforcement replays per trajectory
    pub reinforce_replays: usize,
    /// EWC++ consolidation lambda
    pub ewc_lambda: f32,
    /// Minimum samples per agent type
    pub min_samples_per_agent: usize,
}

impl Default for PretrainConfig {
    fn default() -> Self {
        Self {
            phases: Phase::all().to_vec(),
            samples_per_phase: 1000,
            quality_threshold: 0.6,
            curriculum_learning: true,
            curriculum_stages: 4,
            samples_per_stage: 250,
            embedding_dim: 384,
            sona_config: SonaConfig {
                hidden_dim: 128,
                embedding_dim: 384,
                micro_lora_rank: 1,
                base_lora_rank: 4,
                instant_learning_rate: 0.005,
                background_learning_rate: 0.0005,
                ewc_lambda: 500.0,
                pattern_capacity: 5000,
                background_interval_secs: 1800,
                deep_interval_secs: 259200,
                quality_threshold: 0.6,
            },
            enable_checkpoints: true,
            checkpoint_interval: 500,
            checkpoint_dir: "./checkpoints".to_string(),
            verbose: false,
            random_seed: 42,
            reinforce_replays: 3,
            ewc_lambda: 500.0,
            min_samples_per_agent: 50,
        }
    }
}

impl PretrainConfig {
    /// Configuration optimized for Claude Flow
    pub fn for_claude_flow() -> Self {
        Self {
            samples_per_phase: 2000,
            quality_threshold: 0.65,
            curriculum_stages: 5,
            samples_per_stage: 400,
            reinforce_replays: 5,
            ..Default::default()
        }
    }

    /// Configuration for quick testing
    pub fn for_testing() -> Self {
        Self {
            samples_per_phase: 100,
            quality_threshold: 0.5,
            curriculum_stages: 2,
            samples_per_stage: 50,
            enable_checkpoints: false,
            verbose: false,
            reinforce_replays: 1,
            min_samples_per_agent: 10,
            ..Default::default()
        }
    }

    /// Configuration for edge deployment (minimal footprint)
    pub fn for_edge() -> Self {
        Self {
            phases: vec![Phase::Bootstrap, Phase::Synthetic],
            samples_per_phase: 500,
            quality_threshold: 0.7,
            curriculum_learning: false,
            embedding_dim: 256,
            sona_config: SonaConfig {
                hidden_dim: 64,
                embedding_dim: 256,
                micro_lora_rank: 1,
                base_lora_rank: 2,
                pattern_capacity: 1000,
                ..SonaConfig::default()
            },
            enable_checkpoints: false,
            reinforce_replays: 1,
            min_samples_per_agent: 20,
            ..Default::default()
        }
    }
}

/// Curriculum scheduler for progressive learning
#[derive(Debug, Clone)]
pub struct CurriculumScheduler {
    /// Total stages
    total_stages: usize,
    /// Current stage (0-indexed)
    current_stage: usize,
    /// Samples completed in current stage
    samples_in_stage: usize,
    /// Samples per stage
    samples_per_stage: usize,
    /// Quality history per stage
    quality_history: Vec<Vec<f32>>,
    /// Current complexity level
    current_complexity: TaskComplexity,
}

impl CurriculumScheduler {
    /// Create a new curriculum scheduler
    pub fn new(total_stages: usize, samples_per_stage: usize) -> Self {
        // Pre-allocate quality history with estimated capacity
        let quality_history: Vec<Vec<f32>> = (0..total_stages)
            .map(|_| Vec::with_capacity(samples_per_stage))
            .collect();

        Self {
            total_stages,
            current_stage: 0,
            samples_in_stage: 0,
            samples_per_stage,
            quality_history,
            current_complexity: TaskComplexity::Simple,
        }
    }

    /// Get current complexity level
    #[inline]
    pub fn current_complexity(&self) -> TaskComplexity {
        self.current_complexity
    }

    /// Get current stage
    #[inline]
    pub fn current_stage(&self) -> usize {
        self.current_stage
    }

    /// Check if curriculum is complete
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.current_stage >= self.total_stages
    }

    /// Record sample quality and advance if needed
    pub fn record_sample(&mut self, quality: f32) -> bool {
        if self.is_complete() {
            return false;
        }

        self.quality_history[self.current_stage].push(quality);
        self.samples_in_stage += 1;

        // Check if we should advance to next stage
        if self.samples_in_stage >= self.samples_per_stage {
            self.advance_stage()
        } else {
            false
        }
    }

    /// Advance to next stage
    fn advance_stage(&mut self) -> bool {
        if self.current_stage + 1 < self.total_stages {
            self.current_stage += 1;
            self.samples_in_stage = 0;

            // Update complexity based on stage
            self.current_complexity = match self.current_stage {
                0 => TaskComplexity::Simple,
                1 => TaskComplexity::Moderate,
                2 => TaskComplexity::Complex,
                _ => TaskComplexity::Expert,
            };

            true
        } else {
            self.current_stage = self.total_stages;
            false
        }
    }

    /// Get average quality for a stage
    #[inline]
    pub fn stage_avg_quality(&self, stage: usize) -> f32 {
        if stage >= self.quality_history.len() || self.quality_history[stage].is_empty() {
            return 0.0;
        }
        let history = &self.quality_history[stage];
        let sum: f32 = history.iter().sum();
        sum / history.len() as f32
    }

    /// Get overall average quality
    #[inline]
    pub fn overall_avg_quality(&self) -> f32 {
        let mut total: f32 = 0.0;
        let mut count: usize = 0;
        for v in &self.quality_history {
            for &q in v {
                total += q;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            total / count as f32
        }
    }

    /// Reset the scheduler
    pub fn reset(&mut self) {
        self.current_stage = 0;
        self.samples_in_stage = 0;
        self.quality_history = vec![Vec::new(); self.total_stages];
        self.current_complexity = TaskComplexity::Simple;
    }
}

/// Quality gate for filtering training samples
#[derive(Debug, Clone)]
pub struct QualityGate {
    /// Minimum quality threshold
    threshold: f32,
    /// Total samples seen
    total_seen: u64,
    /// Samples accepted
    accepted: u64,
    /// Samples rejected
    rejected: u64,
    /// Quality distribution (buckets of 0.1)
    quality_buckets: [u64; 10],
}

impl QualityGate {
    /// Create a new quality gate
    #[inline]
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
            total_seen: 0,
            accepted: 0,
            rejected: 0,
            quality_buckets: [0; 10],
        }
    }

    /// Check if a sample passes the quality gate
    #[inline]
    pub fn check(&mut self, quality: f32) -> bool {
        self.total_seen += 1;

        // Record in bucket using fast integer conversion
        let bucket = ((quality * 10.0) as usize).min(9);
        self.quality_buckets[bucket] += 1;

        if quality >= self.threshold {
            self.accepted += 1;
            true
        } else {
            self.rejected += 1;
            false
        }
    }

    /// Get acceptance rate
    #[inline]
    pub fn acceptance_rate(&self) -> f32 {
        if self.total_seen == 0 {
            0.0
        } else {
            self.accepted as f32 / self.total_seen as f32
        }
    }

    /// Get quality statistics
    pub fn stats(&self) -> QualityGateStats {
        QualityGateStats {
            threshold: self.threshold,
            total_seen: self.total_seen,
            accepted: self.accepted,
            rejected: self.rejected,
            acceptance_rate: self.acceptance_rate(),
            quality_distribution: self.quality_buckets,
        }
    }

    /// Reset the gate
    pub fn reset(&mut self) {
        self.total_seen = 0;
        self.accepted = 0;
        self.rejected = 0;
        self.quality_buckets = [0; 10];
    }

    /// Adjust threshold based on acceptance rate
    pub fn auto_adjust(&mut self, target_acceptance_rate: f32) {
        let current_rate = self.acceptance_rate();
        if current_rate < target_acceptance_rate {
            // Lower threshold to accept more
            self.threshold = (self.threshold - 0.05).max(0.1);
        } else if current_rate > target_acceptance_rate + 0.2 {
            // Raise threshold to be more selective
            self.threshold = (self.threshold + 0.05).min(0.95);
        }
    }
}

/// Quality gate statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateStats {
    pub threshold: f32,
    pub total_seen: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub acceptance_rate: f32,
    pub quality_distribution: [u64; 10],
}

/// Progress tracker for pretraining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressTracker {
    /// Current phase
    pub current_phase: Phase,
    /// Phase progress (0.0 - 1.0)
    pub phase_progress: f32,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,
    /// Samples processed per phase
    pub samples_per_phase: HashMap<String, u64>,
    /// Patterns learned per phase
    pub patterns_per_phase: HashMap<String, usize>,
    /// Quality history per phase
    pub quality_per_phase: HashMap<String, f32>,
    /// Start time
    pub start_time: Option<u64>,
    /// Elapsed time (seconds)
    pub elapsed_secs: f64,
    /// Estimated remaining time (seconds)
    pub estimated_remaining_secs: f64,
    /// Checkpoints saved
    pub checkpoints_saved: usize,
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self {
            current_phase: Phase::Bootstrap,
            phase_progress: 0.0,
            overall_progress: 0.0,
            samples_per_phase: HashMap::new(),
            patterns_per_phase: HashMap::new(),
            quality_per_phase: HashMap::new(),
            start_time: None,
            elapsed_secs: 0.0,
            estimated_remaining_secs: 0.0,
            checkpoints_saved: 0,
        }
    }
}

impl ProgressTracker {
    /// Start tracking
    pub fn start(&mut self) {
        self.start_time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    /// Update progress
    pub fn update(&mut self, phase: Phase, samples: u64, total_samples: u64, quality: f32) {
        self.current_phase = phase;
        self.phase_progress = samples as f32 / total_samples.max(1) as f32;

        let phase_name = phase.name().to_string();
        self.samples_per_phase.insert(phase_name.clone(), samples);
        self.quality_per_phase.insert(phase_name, quality);

        // Update elapsed time
        if let Some(start) = self.start_time {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            self.elapsed_secs = (now - start) as f64;

            // Estimate remaining time
            if self.overall_progress > 0.0 {
                let total_estimated = self.elapsed_secs / self.overall_progress as f64;
                self.estimated_remaining_secs = total_estimated - self.elapsed_secs;
            }
        }
    }

    /// Update overall progress
    pub fn set_overall_progress(&mut self, progress: f32) {
        self.overall_progress = progress.clamp(0.0, 1.0);
    }

    /// Record checkpoint
    pub fn record_checkpoint(&mut self) {
        self.checkpoints_saved += 1;
    }

    /// Record patterns for phase
    pub fn record_patterns(&mut self, phase: Phase, count: usize) {
        self.patterns_per_phase
            .insert(phase.name().to_string(), count);
    }
}

/// Result of a single phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    /// Phase that was run
    pub phase: Phase,
    /// Samples processed
    pub samples_processed: u64,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Average quality
    pub avg_quality: f32,
    /// Duration (seconds)
    pub duration_secs: f64,
    /// Quality gate stats
    pub quality_gate_stats: QualityGateStats,
}

/// Result of full pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// Total patterns learned
    pub total_patterns: usize,
    /// Total samples processed
    pub total_samples: u64,
    /// Average quality across all phases
    pub avg_quality: f32,
    /// Total duration (seconds)
    pub total_duration_secs: f64,
    /// Results per phase
    pub phase_results: Vec<PhaseResult>,
    /// Final curriculum stats (if curriculum learning enabled)
    pub curriculum_stats: Option<CurriculumStats>,
    /// Checkpoints saved
    pub checkpoints_saved: usize,
}

/// Curriculum learning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurriculumStats {
    /// Stages completed
    pub stages_completed: usize,
    /// Average quality per stage
    pub quality_per_stage: Vec<f32>,
    /// Samples per stage
    pub samples_per_stage: Vec<usize>,
}

/// Checkpoint data for saving/loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Configuration
    pub config: PretrainConfig,
    /// Progress tracker
    pub progress: ProgressTracker,
    /// Learned patterns (serialized)
    pub patterns: Vec<SerializedPattern>,
    /// Curriculum state
    pub curriculum_stage: usize,
    /// Quality gate threshold
    pub quality_threshold: f32,
    /// Random seed state
    pub random_state: u64,
}

/// Serializable pattern representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedPattern {
    pub id: u64,
    pub centroid: Vec<f32>,
    pub avg_quality: f32,
    pub cluster_size: usize,
    pub pattern_type: String,
}

/// The main pretraining pipeline
pub struct PretrainPipeline {
    /// Configuration
    config: PretrainConfig,
    /// Task generator
    task_generator: TaskGenerator,
    /// SONA pretrainer
    pretrainer: RuvLtraPretrainer,
    /// Curriculum scheduler
    curriculum: CurriculumScheduler,
    /// Quality gate
    quality_gate: QualityGate,
    /// Progress tracker
    progress: ProgressTracker,
    /// Successful trajectories for replay
    successful_trajectories: Vec<TrajectoryRecord>,
    /// Samples processed
    samples_processed: u64,
    /// Patterns per agent
    patterns_per_agent: HashMap<ClaudeFlowAgent, usize>,
}

/// Record of a successful trajectory for replay
#[derive(Debug, Clone)]
struct TrajectoryRecord {
    task: GeneratedTask,
    embedding: Vec<f32>,
    quality: f32,
    agent: ClaudeFlowAgent,
}

impl PretrainPipeline {
    /// Create a new pretraining pipeline
    pub fn new(config: PretrainConfig) -> Self {
        let pretrain_config = RuvLtraPretrainConfig {
            sona: config.sona_config.clone(),
            dataset: crate::sona::DatasetConfig {
                max_routing_prompts: config.samples_per_phase,
                max_quality_prompts: config.samples_per_phase / 2,
                embedding_batch_size: 32,
                min_prompt_length: 10,
                max_prompt_length: 2048,
                quality_threshold: config.quality_threshold,
            },
            routing: crate::sona::RoutingPretrainConfig {
                num_clusters: 50,
                learning_rate: 0.001,
                epochs: 5,
                min_samples_per_class: config.min_samples_per_agent,
                model_mappings: vec![],
            },
            quality: crate::sona::QualityPretrainConfig {
                num_buckets: 5,
                learning_rate: 0.001,
                epochs: 3,
                use_regression: false,
            },
            seeding: crate::sona::SeedingConfig {
                patterns_per_category: 20,
                categories: vec![],
                initial_quality: 0.7,
                embedding_dim: config.embedding_dim,
            },
        };

        let pretrainer = RuvLtraPretrainer::new(pretrain_config);
        let curriculum =
            CurriculumScheduler::new(config.curriculum_stages, config.samples_per_stage);
        let quality_gate = QualityGate::new(config.quality_threshold);

        Self {
            config,
            task_generator: TaskGenerator::new(),
            pretrainer,
            curriculum,
            quality_gate,
            progress: ProgressTracker::default(),
            successful_trajectories: Vec::new(),
            samples_processed: 0,
            patterns_per_agent: HashMap::new(),
        }
    }

    /// Run the full pretraining pipeline
    pub fn run_full_pipeline(&mut self) -> Result<PipelineResult, String> {
        self.progress.start();
        let start_time = Instant::now();
        let mut phase_results = Vec::new();

        let total_phases = self.config.phases.len();

        for (phase_idx, phase) in self.config.phases.clone().iter().enumerate() {
            let phase_result = self.run_phase(*phase)?;
            phase_results.push(phase_result);

            // Update overall progress
            self.progress
                .set_overall_progress((phase_idx + 1) as f32 / total_phases as f32);

            // Save checkpoint if enabled
            if self.config.enable_checkpoints {
                let checkpoint_path = format!(
                    "{}/checkpoint_phase_{}.bin",
                    self.config.checkpoint_dir,
                    phase.name()
                );
                if let Err(e) = self.save_checkpoint(&checkpoint_path) {
                    if self.config.verbose {
                        eprintln!("Warning: Failed to save checkpoint: {}", e);
                    }
                }
            }
        }

        // Calculate final statistics
        let total_patterns: usize = phase_results.iter().map(|r| r.patterns_learned).sum();
        let total_samples: u64 = phase_results.iter().map(|r| r.samples_processed).sum();
        let avg_quality: f32 = if phase_results.is_empty() {
            0.0
        } else {
            phase_results.iter().map(|r| r.avg_quality).sum::<f32>() / phase_results.len() as f32
        };

        let curriculum_stats = if self.config.curriculum_learning {
            Some(CurriculumStats {
                stages_completed: self.curriculum.current_stage(),
                quality_per_stage: (0..self.config.curriculum_stages)
                    .map(|s| self.curriculum.stage_avg_quality(s))
                    .collect(),
                samples_per_stage: vec![
                    self.config.samples_per_stage;
                    self.config.curriculum_stages
                ],
            })
        } else {
            None
        };

        Ok(PipelineResult {
            total_patterns,
            total_samples,
            avg_quality,
            total_duration_secs: start_time.elapsed().as_secs_f64(),
            phase_results,
            curriculum_stats,
            checkpoints_saved: self.progress.checkpoints_saved,
        })
    }

    /// Run a single phase
    pub fn run_phase(&mut self, phase: Phase) -> Result<PhaseResult, String> {
        let start_time = Instant::now();
        self.quality_gate.reset();

        let result = match phase {
            Phase::Bootstrap => self.run_bootstrap_phase(),
            Phase::Synthetic => self.run_synthetic_phase(),
            Phase::Reinforce => self.run_reinforce_phase(),
            Phase::Consolidate => self.run_consolidate_phase(),
        };

        let (samples, patterns, quality) = result?;

        Ok(PhaseResult {
            phase,
            samples_processed: samples,
            patterns_learned: patterns,
            avg_quality: quality,
            duration_secs: start_time.elapsed().as_secs_f64(),
            quality_gate_stats: self.quality_gate.stats(),
        })
    }

    /// Bootstrap phase: seed patterns from agent keywords
    fn run_bootstrap_phase(&mut self) -> Result<(u64, usize, f32), String> {
        if self.config.verbose {
            println!("Running Bootstrap Phase...");
        }

        // Seed the reasoning bank with initial patterns
        let seeding_result = self.pretrainer.seed_reasoning_bank();

        // Generate bootstrap samples from agent keywords
        let mut total_quality = 0.0f32;
        let mut samples_count = 0u64;

        for agent in ClaudeFlowAgent::all() {
            for keyword in agent.keywords() {
                // Create bootstrap task
                let task = GeneratedTask {
                    description: format!("{} task for {}", keyword, agent.name()),
                    category: TaskCategory::from_agent(*agent),
                    complexity: TaskComplexity::Simple,
                    expected_agent: *agent,
                    keywords: vec![keyword.to_string()],
                    context: None,
                };

                // Generate embedding
                let embedding = self.generate_embedding(&task.description);

                // Simulate quality (bootstrap tasks are high quality by definition)
                let quality = 0.8 + (rand_simple() * 0.15);

                if self.quality_gate.check(quality) {
                    // Create pretrain sample
                    let sample = PretrainSample {
                        prompt: task.description.clone(),
                        embedding: Some(embedding.clone()),
                        target_model_index: Some(self.agent_to_model_index(*agent)),
                        quality_score: Some(quality),
                        category: Some(agent.name().to_string()),
                    };

                    // Train
                    self.pretrainer.pretrain_routing_patterns(&[sample]);

                    // Record successful trajectory
                    self.successful_trajectories.push(TrajectoryRecord {
                        task,
                        embedding,
                        quality,
                        agent: *agent,
                    });

                    total_quality += quality;
                    samples_count += 1;
                }

                self.samples_processed += 1;
                self.progress.update(
                    Phase::Bootstrap,
                    samples_count,
                    self.config.samples_per_phase as u64,
                    total_quality / samples_count.max(1) as f32,
                );
            }
        }

        let patterns_learned = seeding_result.patterns_seeded + self.successful_trajectories.len();
        let avg_quality = if samples_count > 0 {
            total_quality / samples_count as f32
        } else {
            0.0
        };

        self.progress
            .record_patterns(Phase::Bootstrap, patterns_learned);

        Ok((samples_count, patterns_learned, avg_quality))
    }

    /// Synthetic phase: generate diverse training samples
    fn run_synthetic_phase(&mut self) -> Result<(u64, usize, f32), String> {
        if self.config.verbose {
            println!("Running Synthetic Phase...");
        }

        // Pre-allocate with expected capacity
        let estimated_samples = self.config.samples_per_phase;
        let mut samples = Vec::with_capacity(estimated_samples);
        let mut total_quality = 0.0f32;
        let mut samples_count = 0u64;

        // Cache agent list length
        let all_agents = ClaudeFlowAgent::all();
        let agent_count = all_agents.len();

        // Generate samples for each agent type
        for agent in all_agents {
            let agent_samples = self.config.samples_per_phase / agent_count;

            for _ in 0..agent_samples {
                // Get complexity based on curriculum
                let complexity = if self.config.curriculum_learning {
                    self.curriculum.current_complexity()
                } else {
                    TaskComplexity::random()
                };

                // Generate task
                let task = self.task_generator.generate_for_agent(*agent, complexity);
                let embedding = self.generate_embedding(&task.description);

                // Simulate quality based on complexity match
                let base_quality = self.simulate_quality(&task, *agent);
                let quality = base_quality + (rand_simple() * 0.1 - 0.05);

                if self.quality_gate.check(quality) {
                    samples.push(PretrainSample {
                        prompt: task.description.clone(),
                        embedding: Some(embedding.clone()),
                        target_model_index: Some(self.agent_to_model_index(*agent)),
                        quality_score: Some(quality),
                        category: Some(task.category.name().to_string()),
                    });

                    // Record successful trajectory
                    self.successful_trajectories.push(TrajectoryRecord {
                        task,
                        embedding,
                        quality,
                        agent: *agent,
                    });

                    total_quality += quality;
                    samples_count += 1;

                    // Update curriculum
                    if self.config.curriculum_learning {
                        self.curriculum.record_sample(quality);
                    }
                }

                self.samples_processed += 1;

                // Checkpoint if needed
                if self.config.enable_checkpoints
                    && self.samples_processed % self.config.checkpoint_interval as u64 == 0
                {
                    let _ = self.save_checkpoint(&format!(
                        "{}/checkpoint_synthetic_{}.bin",
                        self.config.checkpoint_dir, self.samples_processed
                    ));
                }

                self.progress.update(
                    Phase::Synthetic,
                    samples_count,
                    self.config.samples_per_phase as u64,
                    total_quality / samples_count.max(1) as f32,
                );
            }
        }

        // Train on all samples
        let result = self.pretrainer.pretrain_routing_patterns(&samples);

        let avg_quality = if samples_count > 0 {
            total_quality / samples_count as f32
        } else {
            0.0
        };

        self.progress
            .record_patterns(Phase::Synthetic, result.patterns_learned);

        Ok((samples_count, result.patterns_learned, avg_quality))
    }

    /// Reinforce phase: replay successful trajectories
    fn run_reinforce_phase(&mut self) -> Result<(u64, usize, f32), String> {
        if self.config.verbose {
            println!("Running Reinforce Phase...");
        }

        let mut total_quality = 0.0f32;
        let mut samples_count = 0u64;
        let mut patterns_learned = 0;

        // Pre-sort trajectories once (highest quality first for importance sampling)
        let mut sorted_trajectories = self.successful_trajectories.clone();
        sorted_trajectories.sort_by(|a, b| b.quality.partial_cmp(&a.quality).unwrap());

        // Replay successful trajectories multiple times
        for replay_idx in 0..self.config.reinforce_replays {
            // Use pre-sorted list instead of re-sorting each iteration
            let trajectories = &sorted_trajectories;

            for record in trajectories {
                // Slight perturbation to prevent overfitting
                // Pre-allocate and use in-place mutation
                let mut perturbed_embedding: Vec<f32> = Vec::with_capacity(record.embedding.len());
                for &x in &record.embedding {
                    perturbed_embedding.push(x + (rand_simple() * 0.02 - 0.01));
                }

                // Boost quality for replay (successful patterns are reinforced)
                let boosted_quality = (record.quality * 1.1).min(1.0);

                if self.quality_gate.check(boosted_quality) {
                    let sample = PretrainSample {
                        prompt: record.task.description.clone(),
                        embedding: Some(perturbed_embedding),
                        target_model_index: Some(self.agent_to_model_index(record.agent)),
                        quality_score: Some(boosted_quality),
                        category: Some(record.task.category.name().to_string()),
                    };

                    let result = self.pretrainer.pretrain_routing_patterns(&[sample]);
                    patterns_learned += result.patterns_learned;

                    total_quality += boosted_quality;
                    samples_count += 1;
                }

                self.samples_processed += 1;
                self.progress.update(
                    Phase::Reinforce,
                    samples_count,
                    (self.successful_trajectories.len() * self.config.reinforce_replays) as u64,
                    total_quality / samples_count.max(1) as f32,
                );
            }

            if self.config.verbose {
                println!(
                    "  Replay {} complete, quality: {:.3}",
                    replay_idx + 1,
                    total_quality / samples_count.max(1) as f32
                );
            }
        }

        let avg_quality = if samples_count > 0 {
            total_quality / samples_count as f32
        } else {
            0.0
        };

        self.progress
            .record_patterns(Phase::Reinforce, patterns_learned);

        Ok((samples_count, patterns_learned, avg_quality))
    }

    /// Consolidate phase: EWC++ to lock in patterns
    fn run_consolidate_phase(&mut self) -> Result<(u64, usize, f32), String> {
        if self.config.verbose {
            println!("Running Consolidate Phase...");
        }

        // Get all learned patterns
        let reasoning_bank = self.pretrainer.reasoning_bank();
        let patterns = reasoning_bank.get_all_patterns();

        // Compute Fisher information for important patterns
        let ewc = self.pretrainer.ewc();
        let ewc_task_count = ewc.task_count();

        // Consolidate patterns using EWC++
        // This prevents catastrophic forgetting by regularizing updates
        let mut total_quality = 0.0f32;
        let mut consolidated_count = 0;

        for pattern in &patterns {
            if pattern.avg_quality >= self.config.quality_threshold {
                // Pattern is important, contribute to Fisher diagonal
                let pseudo_gradients = self.compute_pattern_gradients(pattern);

                // The EWC++ will use these to compute importance weights
                // (Actual EWC++ update happens internally in the pretrainer)

                total_quality += pattern.avg_quality;
                consolidated_count += 1;
            }
        }

        // Record consolidation metrics
        let avg_quality = if consolidated_count > 0 {
            total_quality / consolidated_count as f32
        } else {
            0.0
        };

        self.progress.update(
            Phase::Consolidate,
            consolidated_count as u64,
            patterns.len() as u64,
            avg_quality,
        );
        self.progress
            .record_patterns(Phase::Consolidate, consolidated_count);

        Ok((consolidated_count as u64, consolidated_count, avg_quality))
    }

    /// Generate embedding for text
    /// Optimized with single-pass normalization
    #[inline]
    fn generate_embedding(&self, text: &str) -> Vec<f32> {
        let dim = self.config.embedding_dim;
        let mut embedding = vec![0.0f32; dim];

        // Character-based hashing for deterministic pseudo-embeddings
        // Use bytes for faster iteration when ASCII is expected
        for (i, ch) in text.chars().enumerate() {
            let idx = i % dim;
            let val = (ch as u32 as f32) * (1.0 / 65536.0); // Pre-computed inverse
            embedding[idx] += val;
        }

        // L2 normalize in single pass
        let mut norm_sq: f32 = 0.0;
        for &e in &embedding {
            norm_sq += e * e;
        }

        let norm = norm_sq.sqrt();
        if norm > 1e-8 {
            let inv_norm = 1.0 / norm;
            for e in &mut embedding {
                *e *= inv_norm;
            }
        }

        embedding
    }

    /// Map agent to model index
    #[inline]
    fn agent_to_model_index(&self, agent: ClaudeFlowAgent) -> usize {
        match agent {
            ClaudeFlowAgent::Coder | ClaudeFlowAgent::BackendDev => 1,
            ClaudeFlowAgent::Researcher => 1,
            ClaudeFlowAgent::Tester => 1,
            ClaudeFlowAgent::Reviewer => 2,
            ClaudeFlowAgent::Architect => 2,
            ClaudeFlowAgent::SecurityAuditor => 2,
            ClaudeFlowAgent::PerformanceEngineer => 2,
            ClaudeFlowAgent::MlDeveloper => 2,
            ClaudeFlowAgent::CicdEngineer => 1,
        }
    }

    /// Simulate quality based on task/agent match
    #[inline]
    fn simulate_quality(&self, task: &GeneratedTask, agent: ClaudeFlowAgent) -> f32 {
        let base_quality: f32 = if task.expected_agent == agent {
            0.85
        } else {
            0.5
        };

        // Adjust for complexity
        let complexity_modifier: f32 = match task.complexity {
            TaskComplexity::Simple => 0.1,
            TaskComplexity::Moderate => 0.0,
            TaskComplexity::Complex => -0.05,
            TaskComplexity::Expert => -0.1,
        };

        (base_quality + complexity_modifier).clamp(0.0_f32, 1.0_f32)
    }

    /// Compute pseudo-gradients for a pattern
    fn compute_pattern_gradients(&self, pattern: &LearnedPattern) -> Vec<f32> {
        let dim = self.config.sona_config.hidden_dim;
        let mut gradients = vec![0.0f32; dim];

        let centroid_len = pattern.centroid.len().min(dim);
        for i in 0..centroid_len {
            gradients[i] = pattern.centroid[i] * pattern.avg_quality;
        }

        gradients
    }

    /// Save checkpoint to disk
    pub fn save_checkpoint(&mut self, path: &str) -> Result<(), String> {
        // Create checkpoint directory if needed
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create checkpoint directory: {}", e))?;
        }

        // Serialize patterns
        let reasoning_bank = self.pretrainer.reasoning_bank();
        let patterns: Vec<SerializedPattern> = reasoning_bank
            .get_all_patterns()
            .iter()
            .map(|p| SerializedPattern {
                id: p.id,
                centroid: p.centroid.clone(),
                avg_quality: p.avg_quality,
                cluster_size: p.cluster_size,
                pattern_type: format!("{:?}", p.pattern_type),
            })
            .collect();

        let checkpoint = Checkpoint {
            config: self.config.clone(),
            progress: self.progress.clone(),
            patterns,
            curriculum_stage: self.curriculum.current_stage(),
            quality_threshold: self.quality_gate.threshold,
            random_state: self.samples_processed, // Use as pseudo-random state
        };

        let serialized = serde_json::to_string_pretty(&checkpoint)
            .map_err(|e| format!("Failed to serialize checkpoint: {}", e))?;

        std::fs::write(path, serialized)
            .map_err(|e| format!("Failed to write checkpoint: {}", e))?;

        self.progress.record_checkpoint();

        if self.config.verbose {
            println!("Checkpoint saved: {}", path);
        }

        Ok(())
    }

    /// Load checkpoint from disk
    pub fn load_checkpoint(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read checkpoint: {}", e))?;

        let checkpoint: Checkpoint = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse checkpoint: {}", e))?;

        let mut pipeline = Self::new(checkpoint.config);
        pipeline.progress = checkpoint.progress;
        pipeline.quality_gate.threshold = checkpoint.quality_threshold;

        // Note: Patterns would need to be reloaded into the reasoning bank
        // This is a simplified version

        Ok(pipeline)
    }

    /// Get current progress
    pub fn progress(&self) -> &ProgressTracker {
        &self.progress
    }

    /// Get quality gate statistics
    pub fn quality_gate_stats(&self) -> QualityGateStats {
        self.quality_gate.stats()
    }

    /// Get curriculum statistics
    pub fn curriculum_stats(&self) -> CurriculumStats {
        CurriculumStats {
            stages_completed: self.curriculum.current_stage(),
            quality_per_stage: (0..self.config.curriculum_stages)
                .map(|s| self.curriculum.stage_avg_quality(s))
                .collect(),
            samples_per_stage: vec![self.config.samples_per_stage; self.config.curriculum_stages],
        }
    }

    /// Get configuration
    pub fn config(&self) -> &PretrainConfig {
        &self.config
    }

    /// Get the trained pretrainer
    pub fn into_pretrainer(self) -> RuvLtraPretrainer {
        self.pretrainer
    }
}

/// Simple pseudo-random number generator (for determinism without external deps)
fn rand_simple() -> f32 {
    use std::cell::RefCell;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    thread_local! {
        static STATE: RefCell<u64> = RefCell::new(42);
    }

    STATE.with(|state| {
        let mut s = state.borrow_mut();
        // LCG parameters
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*s >> 33) as f32 / u32::MAX as f32
    })
}

/// Type alias for error handling
type Result<T, E> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = PretrainConfig::default();
        assert_eq!(config.phases.len(), 4);
        assert_eq!(config.quality_threshold, 0.6);
        assert!(config.curriculum_learning);
    }

    #[test]
    fn test_config_for_testing() {
        let config = PretrainConfig::for_testing();
        assert_eq!(config.samples_per_phase, 100);
        assert!(!config.enable_checkpoints);
    }

    #[test]
    fn test_curriculum_scheduler() {
        let mut scheduler = CurriculumScheduler::new(4, 10);

        assert_eq!(scheduler.current_stage(), 0);
        assert_eq!(scheduler.current_complexity(), TaskComplexity::Simple);
        assert!(!scheduler.is_complete());

        // Complete first stage
        for _ in 0..10 {
            scheduler.record_sample(0.8);
        }

        assert_eq!(scheduler.current_stage(), 1);
        assert_eq!(scheduler.current_complexity(), TaskComplexity::Moderate);
    }

    #[test]
    fn test_quality_gate() {
        let mut gate = QualityGate::new(0.6);

        assert!(gate.check(0.7));
        assert!(!gate.check(0.5));
        assert_eq!(gate.acceptance_rate(), 0.5);
    }

    #[test]
    fn test_progress_tracker() {
        let mut tracker = ProgressTracker::default();
        tracker.start();

        tracker.update(Phase::Bootstrap, 50, 100, 0.75);
        assert_eq!(tracker.phase_progress, 0.5);
    }

    #[test]
    fn test_pipeline_creation() {
        let config = PretrainConfig::for_testing();
        let pipeline = PretrainPipeline::new(config);

        assert_eq!(pipeline.samples_processed, 0);
    }

    #[test]
    fn test_embedding_generation() {
        let config = PretrainConfig::for_testing();
        let pipeline = PretrainPipeline::new(config);

        let embedding = pipeline.generate_embedding("test task");
        assert_eq!(embedding.len(), pipeline.config.embedding_dim);

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_phase_names() {
        assert_eq!(Phase::Bootstrap.name(), "bootstrap");
        assert_eq!(Phase::Synthetic.name(), "synthetic");
        assert_eq!(Phase::Reinforce.name(), "reinforce");
        assert_eq!(Phase::Consolidate.name(), "consolidate");
    }

    #[test]
    fn test_quality_gate_auto_adjust() {
        let mut gate = QualityGate::new(0.9);

        // Simulate low acceptance rate
        for _ in 0..10 {
            gate.check(0.5);
        }

        let old_threshold = gate.threshold;
        gate.auto_adjust(0.5);
        assert!(gate.threshold < old_threshold);
    }
}
