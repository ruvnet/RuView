//! ReasoningBank Integration for RuvLTRA
//!
//! Implements intelligent pattern learning for Claude Flow agent routing:
//!
//! - **Trajectory Storage**: Records task executions with verdict judgments
//! - **Memory Distillation**: Extracts key patterns from multiple trajectories
//! - **EWC++ Consolidation**: Prevents catastrophic forgetting of learned patterns
//! - **Pattern-based Routing**: Recommends agents based on learned successes
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Task Execution    |---->| record_trajectory |
//! | (verdict, steps)  |     | - Store in buffer |
//! +-------------------+     | - Update quality  |
//!                           +--------+----------+
//!                                    |
//!                                    v (threshold reached)
//!                           +--------+----------+
//!                           | distill_patterns  |
//!                           | - Cluster similar |
//!                           | - Extract patterns|
//!                           | - Compute routing |
//!                           +--------+----------+
//!                                    |
//!                                    v (periodic)
//!                           +--------+----------+
//!                           | consolidate()     |
//!                           | - EWC++ update    |
//!                           | - Prune stale     |
//!                           | - Merge similar   |
//!                           +-------------------+
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::claude_flow::reasoning_bank::{
//!     ReasoningBankIntegration, ReasoningBankConfig, Verdict, TrajectoryStep
//! };
//!
//! let config = ReasoningBankConfig::default();
//! let mut bank = ReasoningBankIntegration::new(config);
//!
//! // Record a successful coder task
//! let steps = vec![
//!     TrajectoryStep::new("analyze_requirements", 0.8),
//!     TrajectoryStep::new("implement_code", 0.9),
//!     TrajectoryStep::new("run_tests", 0.95),
//! ];
//! bank.record_trajectory(
//!     "task-123",
//!     &embedding,
//!     steps,
//!     Verdict::Success { reason: "All tests passed".into() },
//! ).unwrap();
//!
//! // Get routing recommendation for new task
//! let rec = bank.get_recommendation(&new_embedding);
//! println!("Suggested agent: {:?} (confidence: {:.2})", rec.agent, rec.confidence);
//! ```

use super::AgentType;
use crate::error::{Result, RuvLLMError};
use crate::sona::{SonaConfig, SonaIntegration, Trajectory as SonaTrajectory};
use parking_lot::RwLock;
use ruvector_sona::{
    EwcConfig, EwcPlusPlus, LearnedPattern, PatternConfig, PatternType, ReasoningBank,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Verdict judgment for a trajectory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Verdict {
    /// Task completed successfully
    Success {
        /// Reason for success
        reason: String,
    },
    /// Task failed
    Failure {
        /// Reason for failure
        reason: String,
        /// Optional error code
        error_code: Option<String>,
    },
    /// Task partially completed
    Partial {
        /// Completion percentage (0.0 - 1.0)
        completion: f32,
        /// Reason for partial completion
        reason: String,
    },
    /// Task recovered via self-reflection
    ///
    /// This variant is used when a task initially failed but was successfully
    /// recovered through the reflection system. It tracks the original error
    /// and the recovery strategy that worked.
    RecoveredViaReflection {
        /// Original error that was encountered
        original_error: String,
        /// Recovery strategy that worked
        recovery_strategy: String,
        /// Number of attempts before successful recovery
        attempts: u32,
    },
}

impl Verdict {
    /// Get quality score for this verdict
    #[inline]
    pub fn quality_score(&self) -> f32 {
        match self {
            Verdict::Success { .. } => 1.0,
            Verdict::Failure { .. } => 0.0,
            Verdict::Partial { completion, .. } => *completion,
            // Recovered tasks get a slightly lower score than pure success
            // to reflect that they required extra effort
            Verdict::RecoveredViaReflection { attempts, .. } => {
                // More attempts = lower score, but still successful
                // 1 attempt = 0.95, 2 = 0.90, 3 = 0.85, etc.
                (1.0 - (*attempts as f32 - 1.0) * 0.05).clamp(0.7, 0.95)
            }
        }
    }

    /// Check if verdict is successful (>= 0.5 quality)
    #[inline]
    pub fn is_successful(&self) -> bool {
        self.quality_score() >= 0.5
    }

    /// Get verdict reason
    #[inline]
    pub fn reason(&self) -> &str {
        match self {
            Verdict::Success { reason } => reason,
            Verdict::Failure { reason, .. } => reason,
            Verdict::Partial { reason, .. } => reason,
            Verdict::RecoveredViaReflection {
                recovery_strategy, ..
            } => recovery_strategy,
        }
    }

    /// Check if this verdict involved recovery via reflection
    #[inline]
    pub fn is_recovered(&self) -> bool {
        matches!(self, Verdict::RecoveredViaReflection { .. })
    }

    /// Get the original error if this was a recovered verdict
    #[inline]
    pub fn original_error(&self) -> Option<&str> {
        match self {
            Verdict::RecoveredViaReflection { original_error, .. } => Some(original_error),
            _ => None,
        }
    }

    /// Get the number of recovery attempts if applicable
    #[inline]
    pub fn recovery_attempts(&self) -> Option<u32> {
        match self {
            Verdict::RecoveredViaReflection { attempts, .. } => Some(*attempts),
            _ => None,
        }
    }
}

impl Default for Verdict {
    fn default() -> Self {
        Verdict::Partial {
            completion: 0.5,
            reason: "Unknown".to_string(),
        }
    }
}

/// A single step in a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Step name/identifier
    pub name: String,
    /// Quality score for this step (0.0 - 1.0)
    pub quality: f32,
    /// Optional agent that performed this step
    pub agent: Option<AgentType>,
    /// Step duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl TrajectoryStep {
    /// Create a new trajectory step
    pub fn new(name: impl Into<String>, quality: f32) -> Self {
        Self {
            name: name.into(),
            quality: quality.clamp(0.0, 1.0),
            agent: None,
            duration_ms: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the agent for this step
    pub fn with_agent(mut self, agent: AgentType) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Set the duration for this step
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Add metadata to this step
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// A complete trajectory record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Unique task identifier
    pub task_id: String,
    /// Task embedding vector
    pub embedding: Vec<f32>,
    /// Execution steps
    pub steps: Vec<TrajectoryStep>,
    /// Final verdict
    pub verdict: Verdict,
    /// Computed quality score (from steps and verdict)
    pub quality_score: f32,
    /// Primary agent used
    pub primary_agent: Option<AgentType>,
    /// Task type classification
    pub task_type: Option<String>,
    /// Timestamp (Unix seconds)
    pub timestamp: u64,
    /// Total duration in milliseconds
    pub total_duration_ms: Option<u64>,
}

impl Trajectory {
    /// Create a new trajectory
    pub fn new(
        task_id: impl Into<String>,
        embedding: Vec<f32>,
        steps: Vec<TrajectoryStep>,
        verdict: Verdict,
    ) -> Self {
        let quality_score = Self::compute_quality(&steps, &verdict);
        let primary_agent = steps.iter().filter_map(|s| s.agent).next();
        let total_duration_ms = steps.iter().filter_map(|s| s.duration_ms).sum::<u64>();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            task_id: task_id.into(),
            embedding,
            steps,
            verdict,
            quality_score,
            primary_agent,
            task_type: None,
            timestamp: now,
            total_duration_ms: if total_duration_ms > 0 {
                Some(total_duration_ms)
            } else {
                None
            },
        }
    }

    /// Compute quality score from steps and verdict
    fn compute_quality(steps: &[TrajectoryStep], verdict: &Verdict) -> f32 {
        if steps.is_empty() {
            return verdict.quality_score();
        }

        // Weighted average: 70% steps, 30% verdict
        let step_avg = steps.iter().map(|s| s.quality).sum::<f32>() / steps.len() as f32;
        step_avg * 0.7 + verdict.quality_score() * 0.3
    }

    /// Set task type
    pub fn with_task_type(mut self, task_type: impl Into<String>) -> Self {
        self.task_type = Some(task_type.into());
        self
    }

    /// Check if trajectory is high quality
    pub fn is_high_quality(&self, threshold: f32) -> bool {
        self.quality_score >= threshold
    }
}

/// Configuration for ReasoningBank integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningBankConfig {
    /// Maximum trajectory capacity
    pub capacity: usize,
    /// Quality threshold for distillation
    pub distillation_threshold: f32,
    /// EWC++ lambda (regularization strength)
    pub ewc_lambda: f32,
    /// Minimum trajectories before distillation
    pub min_trajectories_for_distillation: usize,
    /// Pattern similarity threshold for consolidation
    pub consolidation_similarity: f32,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Number of pattern clusters
    pub num_clusters: usize,
    /// Minimum pattern quality
    pub min_pattern_quality: f32,
    /// Pattern decay factor (for aging)
    pub pattern_decay: f32,
    /// Maximum pattern age in seconds
    pub max_pattern_age_secs: u64,
    /// Enable automatic distillation
    pub auto_distill: bool,
    /// Distillation interval (trajectory count)
    pub distill_interval: usize,
}

impl Default for ReasoningBankConfig {
    fn default() -> Self {
        Self {
            capacity: 10000,
            distillation_threshold: 0.6,
            ewc_lambda: 2000.0,
            min_trajectories_for_distillation: 50,
            consolidation_similarity: 0.85,
            embedding_dim: 384,
            num_clusters: 100,
            min_pattern_quality: 0.3,
            pattern_decay: 0.99,
            max_pattern_age_secs: 604800, // 1 week
            auto_distill: true,
            distill_interval: 100,
        }
    }
}

impl ReasoningBankConfig {
    /// Create configuration optimized for RuvLTRA-Small
    pub fn for_ruvltra_small() -> Self {
        Self {
            capacity: 5000,
            distillation_threshold: 0.6,
            ewc_lambda: 500.0,
            min_trajectories_for_distillation: 30,
            consolidation_similarity: 0.9,
            embedding_dim: 384,
            num_clusters: 50,
            min_pattern_quality: 0.4,
            pattern_decay: 0.995,
            max_pattern_age_secs: 259200, // 3 days
            auto_distill: true,
            distill_interval: 50,
        }
    }

    /// Create configuration for edge deployment
    pub fn for_edge() -> Self {
        Self {
            capacity: 1000,
            distillation_threshold: 0.7,
            ewc_lambda: 1000.0,
            min_trajectories_for_distillation: 20,
            consolidation_similarity: 0.95,
            embedding_dim: 256,
            num_clusters: 20,
            min_pattern_quality: 0.5,
            pattern_decay: 0.99,
            max_pattern_age_secs: 86400, // 1 day
            auto_distill: true,
            distill_interval: 30,
        }
    }
}

/// Routing recommendation based on learned patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRecommendation {
    /// Recommended agent type
    pub agent: AgentType,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Number of patterns used for recommendation
    pub patterns_used: usize,
    /// Average quality of matching patterns
    pub avg_pattern_quality: f32,
    /// Alternative agent suggestions
    pub alternatives: Vec<(AgentType, f32)>,
    /// Reasoning for recommendation
    pub reasoning: String,
}

impl Default for RoutingRecommendation {
    fn default() -> Self {
        Self {
            agent: AgentType::Coder,
            confidence: 0.3,
            patterns_used: 0,
            avg_pattern_quality: 0.0,
            alternatives: Vec::new(),
            reasoning: "No patterns available, using default agent".to_string(),
        }
    }
}

/// Statistics for ReasoningBank
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReasoningBankStats {
    /// Total trajectories recorded
    pub total_trajectories: u64,
    /// Successful trajectories
    pub successful_trajectories: u64,
    /// Failed trajectories
    pub failed_trajectories: u64,
    /// Partial trajectories
    pub partial_trajectories: u64,
    /// Current trajectory buffer size
    pub buffer_size: usize,
    /// Number of learned patterns
    pub patterns_learned: usize,
    /// Distillation runs
    pub distillation_runs: u64,
    /// Consolidation runs
    pub consolidation_runs: u64,
    /// Average quality score
    pub avg_quality: f32,
    /// EWC task count
    pub ewc_tasks: usize,
}

/// Distilled pattern from multiple trajectories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledPattern {
    /// Pattern identifier
    pub id: u64,
    /// Centroid embedding
    pub centroid: Vec<f32>,
    /// Primary agent association
    pub primary_agent: AgentType,
    /// Agent score distribution
    pub agent_scores: HashMap<AgentType, f32>,
    /// Average quality
    pub avg_quality: f32,
    /// Number of trajectories distilled
    pub trajectory_count: usize,
    /// Task type association
    pub task_type: Option<String>,
    /// Created timestamp
    pub created_at: u64,
    /// Last accessed timestamp
    pub last_accessed: u64,
    /// Access count
    pub access_count: u32,
}

impl DistilledPattern {
    /// Compute similarity with embedding using optimized dot product
    #[inline]
    pub fn similarity(&self, embedding: &[f32]) -> f32 {
        let len = self.centroid.len();
        if len != embedding.len() {
            return 0.0;
        }

        // Compute all in single pass for cache efficiency
        let mut dot: f32 = 0.0;
        let mut norm_a_sq: f32 = 0.0;
        let mut norm_b_sq: f32 = 0.0;

        for i in 0..len {
            let a = self.centroid[i];
            let b = embedding[i];
            dot += a * b;
            norm_a_sq += a * a;
            norm_b_sq += b * b;
        }

        let norm_a = norm_a_sq.sqrt();
        let norm_b = norm_b_sq.sqrt();

        if norm_a > 1e-8 && norm_b > 1e-8 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Get best agent from this pattern
    #[inline]
    pub fn best_agent(&self) -> AgentType {
        self.agent_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(agent, _)| *agent)
            .unwrap_or(self.primary_agent)
    }

    /// Check if pattern should be pruned
    #[inline]
    pub fn should_prune(&self, min_quality: f32, max_age_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let age = now.saturating_sub(self.last_accessed);

        self.avg_quality < min_quality && age > max_age_secs && self.access_count < 5
    }
}

/// ReasoningBank integration for Claude Flow
pub struct ReasoningBankIntegration {
    /// Configuration
    config: ReasoningBankConfig,
    /// Trajectory buffer
    trajectory_buffer: Arc<RwLock<Vec<Trajectory>>>,
    /// Distilled patterns
    patterns: Arc<RwLock<HashMap<u64, DistilledPattern>>>,
    /// EWC++ for consolidation
    ewc: Arc<RwLock<EwcPlusPlus>>,
    /// Core reasoning bank (from ruvector_sona)
    core_bank: Arc<RwLock<ReasoningBank>>,
    /// SONA integration for trajectory recording
    sona: Option<Arc<RwLock<SonaIntegration>>>,
    /// Next pattern ID
    next_pattern_id: AtomicU64,
    /// Statistics
    stats: RwLock<ReasoningBankStats>,
    /// Trajectories since last distillation
    trajectories_since_distill: AtomicU64,
}

impl ReasoningBankIntegration {
    /// Create a new ReasoningBank integration
    pub fn new(config: ReasoningBankConfig) -> Self {
        let ewc_config = EwcConfig {
            param_count: config.embedding_dim,
            initial_lambda: config.ewc_lambda,
            max_lambda: config.ewc_lambda * 5.0,
            ..Default::default()
        };

        let pattern_config = PatternConfig {
            k_clusters: config.num_clusters,
            embedding_dim: config.embedding_dim.min(256),
            max_trajectories: config.capacity,
            quality_threshold: config.min_pattern_quality,
            ..Default::default()
        };

        Self {
            config,
            trajectory_buffer: Arc::new(RwLock::new(Vec::new())),
            patterns: Arc::new(RwLock::new(HashMap::new())),
            ewc: Arc::new(RwLock::new(EwcPlusPlus::new(ewc_config))),
            core_bank: Arc::new(RwLock::new(ReasoningBank::new(pattern_config))),
            sona: None,
            next_pattern_id: AtomicU64::new(0),
            stats: RwLock::new(ReasoningBankStats::default()),
            trajectories_since_distill: AtomicU64::new(0),
        }
    }

    /// Create with SONA integration
    pub fn with_sona(config: ReasoningBankConfig, sona_config: SonaConfig) -> Self {
        let mut bank = Self::new(config);
        bank.sona = Some(Arc::new(RwLock::new(SonaIntegration::new(sona_config))));
        bank
    }

    /// Record a trajectory
    pub fn record_trajectory(
        &self,
        task_id: impl Into<String>,
        embedding: &[f32],
        steps: Vec<TrajectoryStep>,
        verdict: Verdict,
    ) -> Result<()> {
        let trajectory = Trajectory::new(task_id, embedding.to_vec(), steps, verdict.clone());

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.total_trajectories += 1;
            match &verdict {
                Verdict::Success { .. } => stats.successful_trajectories += 1,
                Verdict::Failure { .. } => stats.failed_trajectories += 1,
                Verdict::Partial { .. } => stats.partial_trajectories += 1,
                Verdict::RecoveredViaReflection { .. } => {
                    // Count recovered as successful since task completed
                    stats.successful_trajectories += 1;
                }
            }
            // Update running average quality
            let n = stats.total_trajectories as f32;
            stats.avg_quality = stats.avg_quality * (n - 1.0) / n + trajectory.quality_score / n;
        }

        // Add to buffer
        {
            let mut buffer = self.trajectory_buffer.write();

            // Enforce capacity
            if buffer.len() >= self.config.capacity {
                buffer.remove(0);
            }

            buffer.push(trajectory.clone());
        }

        // Record to SONA if available
        if let Some(ref sona) = self.sona {
            let sona_trajectory = SonaTrajectory {
                request_id: trajectory.task_id.clone(),
                session_id: "reasoning-bank".to_string(),
                query_embedding: embedding.to_vec(),
                response_embedding: embedding.to_vec(),
                quality_score: trajectory.quality_score,
                routing_features: vec![
                    trajectory.quality_score,
                    verdict.quality_score(),
                    trajectory.steps.len() as f32 / 10.0,
                ],
                model_index: trajectory.primary_agent.map(|a| a as usize).unwrap_or(0),
                timestamp: chrono::Utc::now(),
            };

            let sona_guard = sona.read();
            let _ = sona_guard.record_trajectory(sona_trajectory);
        }

        // Record to core bank
        {
            let mut core = self.core_bank.write();
            let query_traj =
                ruvector_sona::QueryTrajectory::new(trajectory.timestamp, embedding.to_vec());
            core.add_trajectory(&query_traj);
        }

        // Check for auto-distillation
        let count = self
            .trajectories_since_distill
            .fetch_add(1, Ordering::SeqCst)
            + 1;
        if self.config.auto_distill && count >= self.config.distill_interval as u64 {
            self.distill_patterns()?;
            self.trajectories_since_distill.store(0, Ordering::SeqCst);
        }

        Ok(())
    }

    /// Distill patterns from trajectories
    pub fn distill_patterns(&self) -> Result<Vec<DistilledPattern>> {
        let trajectories: Vec<Trajectory> = {
            let buffer = self.trajectory_buffer.read();
            buffer
                .iter()
                .filter(|t| t.quality_score >= self.config.distillation_threshold)
                .cloned()
                .collect()
        };

        if trajectories.len() < self.config.min_trajectories_for_distillation {
            return Ok(Vec::new());
        }

        // Extract patterns from core bank
        {
            let mut core = self.core_bank.write();
            core.extract_patterns();
        }

        // Group trajectories by similarity
        let clusters = self.cluster_trajectories(&trajectories);

        let mut new_patterns = Vec::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for cluster in clusters {
            if cluster.is_empty() {
                continue;
            }

            // Compute centroid
            let dim = cluster[0].embedding.len();
            let mut centroid = vec![0.0f32; dim];
            for traj in &cluster {
                for (i, &e) in traj.embedding.iter().enumerate() {
                    if i < dim {
                        centroid[i] += e;
                    }
                }
            }
            for c in &mut centroid {
                *c /= cluster.len() as f32;
            }

            // Normalize centroid
            let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-8 {
                for c in &mut centroid {
                    *c /= norm;
                }
            }

            // Compute agent scores
            let mut agent_scores: HashMap<AgentType, f32> = HashMap::new();
            let mut total_quality = 0.0f32;
            let mut task_type: Option<String> = None;

            for traj in &cluster {
                if let Some(agent) = traj.primary_agent {
                    *agent_scores.entry(agent).or_insert(0.0) += traj.quality_score;
                }
                total_quality += traj.quality_score;
                if task_type.is_none() {
                    task_type = traj.task_type.clone();
                }
            }

            // Normalize agent scores
            let total_agent_score: f32 = agent_scores.values().sum();
            if total_agent_score > 0.0 {
                for score in agent_scores.values_mut() {
                    *score /= total_agent_score;
                }
            }

            // Determine primary agent
            let primary_agent = agent_scores
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(agent, _)| *agent)
                .unwrap_or(AgentType::Coder);

            let pattern_id = self.next_pattern_id.fetch_add(1, Ordering::SeqCst);

            let pattern = DistilledPattern {
                id: pattern_id,
                centroid,
                primary_agent,
                agent_scores,
                avg_quality: total_quality / cluster.len() as f32,
                trajectory_count: cluster.len(),
                task_type,
                created_at: now,
                last_accessed: now,
                access_count: 0,
            };

            // Store pattern
            {
                let mut patterns = self.patterns.write();
                patterns.insert(pattern_id, pattern.clone());
            }

            new_patterns.push(pattern);
        }

        // Update EWC with new patterns
        self.update_ewc_from_patterns(&new_patterns);

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.distillation_runs += 1;
            stats.patterns_learned = self.patterns.read().len();
        }

        Ok(new_patterns)
    }

    /// Cluster trajectories by embedding similarity
    fn cluster_trajectories(&self, trajectories: &[Trajectory]) -> Vec<Vec<Trajectory>> {
        if trajectories.is_empty() {
            return Vec::new();
        }

        // Simple K-means style clustering
        let k = self.config.num_clusters.min(trajectories.len() / 3).max(1);
        let dim = trajectories[0].embedding.len();

        // Initialize centroids with first k trajectories
        let mut centroids: Vec<Vec<f32>> = trajectories
            .iter()
            .take(k)
            .map(|t| t.embedding.clone())
            .collect();

        // Run clustering iterations
        let mut assignments = vec![0usize; trajectories.len()];

        for _ in 0..10 {
            // Assign each trajectory to nearest centroid
            let mut changed = false;
            for (i, traj) in trajectories.iter().enumerate() {
                let nearest = centroids
                    .iter()
                    .enumerate()
                    .map(|(j, c)| (j, self.cosine_similarity(&traj.embedding, c)))
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(j, _)| j)
                    .unwrap_or(0);

                if assignments[i] != nearest {
                    assignments[i] = nearest;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Recompute centroids
            let mut new_centroids = vec![vec![0.0f32; dim]; k];
            let mut counts = vec![0usize; k];

            for (i, traj) in trajectories.iter().enumerate() {
                let cluster = assignments[i];
                counts[cluster] += 1;
                for (j, &e) in traj.embedding.iter().enumerate() {
                    if j < dim {
                        new_centroids[cluster][j] += e;
                    }
                }
            }

            for (i, centroid) in new_centroids.iter_mut().enumerate() {
                if counts[i] > 0 {
                    for c in centroid.iter_mut() {
                        *c /= counts[i] as f32;
                    }
                }
            }

            centroids = new_centroids;
        }

        // Group trajectories by assignment
        let mut clusters: Vec<Vec<Trajectory>> = vec![Vec::new(); k];
        for (i, traj) in trajectories.iter().enumerate() {
            clusters[assignments[i]].push(traj.clone());
        }

        // Filter out small clusters
        clusters.into_iter().filter(|c| c.len() >= 2).collect()
    }

    /// Cosine similarity between two vectors
    /// Optimized to compute all norms in a single pass
    #[inline]
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let len = a.len();
        if len != b.len() {
            return 0.0;
        }

        // Single-pass computation for cache efficiency
        let mut dot: f32 = 0.0;
        let mut norm_a_sq: f32 = 0.0;
        let mut norm_b_sq: f32 = 0.0;

        for i in 0..len {
            let x = a[i];
            let y = b[i];
            dot += x * y;
            norm_a_sq += x * x;
            norm_b_sq += y * y;
        }

        let norm_a = norm_a_sq.sqrt();
        let norm_b = norm_b_sq.sqrt();

        if norm_a > 1e-8 && norm_b > 1e-8 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Update EWC from new patterns
    fn update_ewc_from_patterns(&self, patterns: &[DistilledPattern]) {
        let mut ewc = self.ewc.write();

        for pattern in patterns {
            // Use centroid as pseudo-gradients
            let gradients: Vec<f32> = pattern
                .centroid
                .iter()
                .take(self.config.embedding_dim)
                .copied()
                .chain(std::iter::repeat(0.0))
                .take(self.config.embedding_dim)
                .collect();

            ewc.update_fisher(&gradients);
        }

        // Start new task periodically
        if patterns.len() >= 10 {
            ewc.start_new_task();
        }
    }

    /// Get routing recommendation for an embedding
    pub fn get_recommendation(&self, embedding: &[f32]) -> RoutingRecommendation {
        let patterns = self.patterns.read();

        if patterns.is_empty() {
            return RoutingRecommendation::default();
        }

        // Find most similar patterns with pre-allocated capacity
        let mut scored: Vec<(&DistilledPattern, f32)> = Vec::with_capacity(patterns.len());
        for pattern in patterns.values() {
            scored.push((pattern, pattern.similarity(embedding)));
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_patterns: Vec<_> = scored.into_iter().take(5).collect();

        if top_patterns.is_empty() {
            return RoutingRecommendation::default();
        }

        // Update access counts for top patterns
        {
            let mut patterns_mut = self.patterns.write();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            for (pattern, _) in &top_patterns {
                if let Some(p) = patterns_mut.get_mut(&pattern.id) {
                    p.access_count += 1;
                    p.last_accessed = now;
                }
            }
        }

        // Weighted vote for best agent - pre-allocate for typical agent count
        let mut agent_votes: HashMap<AgentType, f32> = HashMap::with_capacity(16);
        let mut total_weight = 0.0f32;
        let mut total_quality = 0.0f32;

        for (pattern, similarity) in &top_patterns {
            let weight = similarity * pattern.avg_quality;
            total_weight += weight;
            total_quality += pattern.avg_quality;

            for (agent, score) in &pattern.agent_scores {
                *agent_votes.entry(*agent).or_insert(0.0) += weight * score;
            }

            // Also vote for primary agent
            *agent_votes.entry(pattern.primary_agent).or_insert(0.0) += weight * 0.5;
        }

        // Normalize votes
        if total_weight > 0.0 {
            for vote in agent_votes.values_mut() {
                *vote /= total_weight;
            }
        }

        // Find best agent
        let (best_agent, best_score) = agent_votes
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(agent, score)| (*agent, *score))
            .unwrap_or((AgentType::Coder, 0.0));

        // Get alternatives
        let mut alternatives: Vec<(AgentType, f32)> = agent_votes
            .into_iter()
            .filter(|(agent, _)| *agent != best_agent)
            .collect();
        alternatives.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        alternatives.truncate(3);

        // Compute confidence
        let confidence = if top_patterns.is_empty() {
            0.0
        } else {
            let max_similarity = top_patterns[0].1;
            (best_score * max_similarity).min(1.0)
        };

        let avg_pattern_quality = if top_patterns.is_empty() {
            0.0
        } else {
            total_quality / top_patterns.len() as f32
        };

        let reasoning = format!(
            "Based on {} similar patterns with avg quality {:.2}; best match similarity: {:.2}",
            top_patterns.len(),
            avg_pattern_quality,
            top_patterns.first().map(|(_, s)| *s).unwrap_or(0.0)
        );

        RoutingRecommendation {
            agent: best_agent,
            confidence,
            patterns_used: top_patterns.len(),
            avg_pattern_quality,
            alternatives,
            reasoning,
        }
    }

    /// Consolidate patterns with EWC++
    pub fn consolidate(&self) -> Result<()> {
        // Prune old/low-quality patterns
        {
            let mut patterns = self.patterns.write();
            let to_remove: Vec<u64> = patterns
                .iter()
                .filter(|(_, p)| {
                    p.should_prune(
                        self.config.min_pattern_quality,
                        self.config.max_pattern_age_secs,
                    )
                })
                .map(|(id, _)| *id)
                .collect();

            for id in to_remove {
                patterns.remove(&id);
            }
        }

        // Merge similar patterns
        {
            let mut patterns = self.patterns.write();
            let pattern_ids: Vec<u64> = patterns.keys().copied().collect();
            let mut merged_ids = Vec::new();

            for i in 0..pattern_ids.len() {
                for j in i + 1..pattern_ids.len() {
                    let id1 = pattern_ids[i];
                    let id2 = pattern_ids[j];

                    if merged_ids.contains(&id1) || merged_ids.contains(&id2) {
                        continue;
                    }

                    if let (Some(p1), Some(p2)) = (patterns.get(&id1), patterns.get(&id2)) {
                        let similarity = p1.similarity(&p2.centroid);
                        if similarity > self.config.consolidation_similarity {
                            // Merge p2 into p1
                            let merged = self.merge_patterns(p1, p2);
                            patterns.insert(id1, merged);
                            merged_ids.push(id2);
                        }
                    }
                }
            }

            for id in merged_ids {
                patterns.remove(&id);
            }
        }

        // Consolidate EWC tasks
        {
            let mut ewc = self.ewc.write();
            ewc.consolidate_all_tasks();
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.consolidation_runs += 1;
            stats.patterns_learned = self.patterns.read().len();
            stats.ewc_tasks = self.ewc.read().task_count();
        }

        Ok(())
    }

    /// Merge two patterns
    fn merge_patterns(&self, p1: &DistilledPattern, p2: &DistilledPattern) -> DistilledPattern {
        let total_count = p1.trajectory_count + p2.trajectory_count;
        let w1 = p1.trajectory_count as f32 / total_count as f32;
        let w2 = p2.trajectory_count as f32 / total_count as f32;

        // Merge centroids
        let centroid: Vec<f32> = p1
            .centroid
            .iter()
            .zip(&p2.centroid)
            .map(|(&a, &b)| a * w1 + b * w2)
            .collect();

        // Merge agent scores
        let mut agent_scores: HashMap<AgentType, f32> = p1.agent_scores.clone();
        for (agent, score) in &p2.agent_scores {
            *agent_scores.entry(*agent).or_insert(0.0) += score * w2;
        }

        // Normalize
        let total: f32 = agent_scores.values().sum();
        if total > 0.0 {
            for score in agent_scores.values_mut() {
                *score /= total;
            }
        }

        let primary_agent = agent_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(agent, _)| *agent)
            .unwrap_or(p1.primary_agent);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        DistilledPattern {
            id: p1.id,
            centroid,
            primary_agent,
            agent_scores,
            avg_quality: p1.avg_quality * w1 + p2.avg_quality * w2,
            trajectory_count: total_count,
            task_type: p1.task_type.clone().or_else(|| p2.task_type.clone()),
            created_at: p1.created_at.min(p2.created_at),
            last_accessed: now,
            access_count: p1.access_count + p2.access_count,
        }
    }

    /// Get statistics
    pub fn stats(&self) -> ReasoningBankStats {
        let mut stats = self.stats.read().clone();
        stats.buffer_size = self.trajectory_buffer.read().len();
        stats.patterns_learned = self.patterns.read().len();
        stats.ewc_tasks = self.ewc.read().task_count();
        stats
    }

    /// Get all patterns
    pub fn get_patterns(&self) -> Vec<DistilledPattern> {
        self.patterns.read().values().cloned().collect()
    }

    /// Get trajectory count
    pub fn trajectory_count(&self) -> usize {
        self.trajectory_buffer.read().len()
    }

    /// Get pattern count
    pub fn pattern_count(&self) -> usize {
        self.patterns.read().len()
    }

    /// Clear all data
    pub fn clear(&self) {
        self.trajectory_buffer.write().clear();
        self.patterns.write().clear();
        *self.stats.write() = ReasoningBankStats::default();
        self.trajectories_since_distill.store(0, Ordering::SeqCst);
    }

    /// Export patterns for persistence
    pub fn export_patterns(&self) -> Vec<DistilledPattern> {
        self.patterns.read().values().cloned().collect()
    }

    /// Import patterns
    pub fn import_patterns(&self, patterns: Vec<DistilledPattern>) {
        let mut pattern_map = self.patterns.write();
        for pattern in patterns {
            let id = pattern.id.max(self.next_pattern_id.load(Ordering::SeqCst));
            self.next_pattern_id.fetch_max(id + 1, Ordering::SeqCst);
            pattern_map.insert(pattern.id, pattern);
        }

        self.stats.write().patterns_learned = pattern_map.len();
    }
}

impl std::fmt::Debug for ReasoningBankIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReasoningBankIntegration")
            .field("config", &self.config)
            .field("trajectory_count", &self.trajectory_count())
            .field("pattern_count", &self.pattern_count())
            .field("stats", &self.stats())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verdict_quality_scores() {
        assert_eq!(
            Verdict::Success {
                reason: "ok".into()
            }
            .quality_score(),
            1.0
        );
        assert_eq!(
            Verdict::Failure {
                reason: "err".into(),
                error_code: None
            }
            .quality_score(),
            0.0
        );
        assert_eq!(
            Verdict::Partial {
                completion: 0.7,
                reason: "partial".into()
            }
            .quality_score(),
            0.7
        );
    }

    #[test]
    fn test_trajectory_step_creation() {
        let step = TrajectoryStep::new("test_step", 0.8)
            .with_agent(AgentType::Coder)
            .with_duration(100)
            .with_metadata("key", "value");

        assert_eq!(step.name, "test_step");
        assert_eq!(step.quality, 0.8);
        assert_eq!(step.agent, Some(AgentType::Coder));
        assert_eq!(step.duration_ms, Some(100));
        assert_eq!(step.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_trajectory_creation() {
        let steps = vec![
            TrajectoryStep::new("step1", 0.7).with_agent(AgentType::Researcher),
            TrajectoryStep::new("step2", 0.9).with_agent(AgentType::Coder),
        ];

        let traj = Trajectory::new(
            "task-1",
            vec![0.1, 0.2, 0.3],
            steps,
            Verdict::Success {
                reason: "done".into(),
            },
        );

        assert_eq!(traj.task_id, "task-1");
        assert_eq!(traj.steps.len(), 2);
        // Quality = 0.7 * ((0.7 + 0.9) / 2) + 0.3 * 1.0 = 0.56 + 0.3 = 0.86
        assert!((traj.quality_score - 0.86).abs() < 0.01);
    }

    #[test]
    fn test_reasoning_bank_creation() {
        let config = ReasoningBankConfig::default();
        let bank = ReasoningBankIntegration::new(config);

        assert_eq!(bank.trajectory_count(), 0);
        assert_eq!(bank.pattern_count(), 0);
    }

    #[test]
    fn test_record_trajectory() {
        let config = ReasoningBankConfig {
            auto_distill: false,
            ..Default::default()
        };
        let bank = ReasoningBankIntegration::new(config);

        let steps = vec![TrajectoryStep::new("step1", 0.8).with_agent(AgentType::Coder)];

        bank.record_trajectory(
            "task-1",
            &vec![0.1; 384],
            steps,
            Verdict::Success {
                reason: "done".into(),
            },
        )
        .unwrap();

        assert_eq!(bank.trajectory_count(), 1);

        let stats = bank.stats();
        assert_eq!(stats.total_trajectories, 1);
        assert_eq!(stats.successful_trajectories, 1);
    }

    #[test]
    fn test_distill_patterns() {
        let config = ReasoningBankConfig {
            min_trajectories_for_distillation: 5,
            distillation_threshold: 0.0, // Accept all
            num_clusters: 2,
            auto_distill: false,
            ..Default::default()
        };
        let bank = ReasoningBankIntegration::new(config);

        // Add trajectories
        for i in 0..10 {
            let embedding: Vec<f32> = if i < 5 {
                vec![1.0, 0.0, 0.0]
                    .into_iter()
                    .chain(std::iter::repeat(0.0))
                    .take(384)
                    .collect()
            } else {
                vec![0.0, 1.0, 0.0]
                    .into_iter()
                    .chain(std::iter::repeat(0.0))
                    .take(384)
                    .collect()
            };

            let steps = vec![TrajectoryStep::new("step", 0.8).with_agent(AgentType::Coder)];

            bank.record_trajectory(
                format!("task-{}", i),
                &embedding,
                steps,
                Verdict::Success {
                    reason: "done".into(),
                },
            )
            .unwrap();
        }

        let patterns = bank.distill_patterns().unwrap();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_get_recommendation() {
        let config = ReasoningBankConfig {
            min_trajectories_for_distillation: 2,
            distillation_threshold: 0.0,
            num_clusters: 1,
            auto_distill: false,
            ..Default::default()
        };
        let bank = ReasoningBankIntegration::new(config);

        // Add similar trajectories
        for i in 0..5 {
            let embedding: Vec<f32> = vec![1.0, 0.0, 0.0]
                .into_iter()
                .chain(std::iter::repeat(0.0))
                .take(384)
                .collect();

            let steps = vec![TrajectoryStep::new("step", 0.9).with_agent(AgentType::Tester)];

            bank.record_trajectory(
                format!("task-{}", i),
                &embedding,
                steps,
                Verdict::Success {
                    reason: "done".into(),
                },
            )
            .unwrap();
        }

        bank.distill_patterns().unwrap();

        // Get recommendation for similar embedding
        let query: Vec<f32> = vec![0.9, 0.1, 0.0]
            .into_iter()
            .chain(std::iter::repeat(0.0))
            .take(384)
            .collect();

        let rec = bank.get_recommendation(&query);
        assert!(rec.patterns_used > 0);
        assert!(rec.confidence > 0.0);
    }

    #[test]
    fn test_consolidate() {
        let config = ReasoningBankConfig {
            min_trajectories_for_distillation: 2,
            distillation_threshold: 0.0,
            num_clusters: 2,
            consolidation_similarity: 0.99, // High threshold for testing
            auto_distill: false,
            ..Default::default()
        };
        let bank = ReasoningBankIntegration::new(config);

        // Add trajectories
        for i in 0..6 {
            let embedding: Vec<f32> = vec![1.0 + (i as f32 * 0.001), 0.0, 0.0]
                .into_iter()
                .chain(std::iter::repeat(0.0))
                .take(384)
                .collect();

            let steps = vec![TrajectoryStep::new("step", 0.8).with_agent(AgentType::Coder)];

            bank.record_trajectory(
                format!("task-{}", i),
                &embedding,
                steps,
                Verdict::Success {
                    reason: "done".into(),
                },
            )
            .unwrap();
        }

        bank.distill_patterns().unwrap();
        let before = bank.pattern_count();

        bank.consolidate().unwrap();
        let after = bank.pattern_count();

        assert!(after <= before);
    }

    #[test]
    fn test_distilled_pattern_similarity() {
        let pattern = DistilledPattern {
            id: 1,
            centroid: vec![1.0, 0.0, 0.0, 0.0],
            primary_agent: AgentType::Coder,
            agent_scores: HashMap::new(),
            avg_quality: 0.9,
            trajectory_count: 10,
            task_type: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
        };

        let same = vec![1.0, 0.0, 0.0, 0.0];
        let orthogonal = vec![0.0, 1.0, 0.0, 0.0];

        assert!((pattern.similarity(&same) - 1.0).abs() < 0.01);
        assert!(pattern.similarity(&orthogonal).abs() < 0.01);
    }

    #[test]
    fn test_export_import_patterns() {
        let config = ReasoningBankConfig::default();
        let bank = ReasoningBankIntegration::new(config.clone());

        // Create some patterns manually
        let pattern = DistilledPattern {
            id: 42,
            centroid: vec![0.5; 384],
            primary_agent: AgentType::Researcher,
            agent_scores: HashMap::from([(AgentType::Researcher, 0.8), (AgentType::Coder, 0.2)]),
            avg_quality: 0.85,
            trajectory_count: 50,
            task_type: Some("research".to_string()),
            created_at: 1000,
            last_accessed: 2000,
            access_count: 10,
        };

        bank.import_patterns(vec![pattern.clone()]);
        assert_eq!(bank.pattern_count(), 1);

        let exported = bank.export_patterns();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].id, 42);
        assert_eq!(exported[0].primary_agent, AgentType::Researcher);
    }

    #[test]
    fn test_config_presets() {
        let default = ReasoningBankConfig::default();
        let small = ReasoningBankConfig::for_ruvltra_small();
        let edge = ReasoningBankConfig::for_edge();

        assert!(default.capacity > small.capacity);
        assert!(small.capacity > edge.capacity);
        assert!(edge.num_clusters < small.num_clusters);
    }
}
