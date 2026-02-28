//! Trajectory Recording for ReasoningBank
//!
//! Provides structures and utilities for recording execution trajectories
//! during Claude/LLM interactions, enabling continuous learning.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use super::Verdict;

/// Global trajectory ID counter
static TRAJECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for a trajectory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrajectoryId(pub u64);

impl TrajectoryId {
    /// Generate a new unique trajectory ID
    pub fn new() -> Self {
        Self(TRAJECTORY_COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    /// Create from a specific value
    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }

    /// Get the inner u64 value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for TrajectoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrajectoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "traj-{}", self.0)
    }
}

/// Outcome of a trajectory step
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepOutcome {
    /// Step completed successfully
    Success,
    /// Step completed with a partial result
    Partial {
        /// What was achieved
        achieved: String,
        /// What was missing
        missing: String,
    },
    /// Step failed
    Failure {
        /// Error message
        error: String,
    },
    /// Step was skipped
    Skipped {
        /// Reason for skipping
        reason: String,
    },
    /// Step needs retry
    NeedsRetry {
        /// Reason for retry
        reason: String,
        /// Suggested modifications
        suggestions: Vec<String>,
    },
}

impl StepOutcome {
    /// Check if the outcome is successful
    pub fn is_success(&self) -> bool {
        matches!(self, StepOutcome::Success)
    }

    /// Check if the outcome is a failure
    pub fn is_failure(&self) -> bool {
        matches!(self, StepOutcome::Failure { .. })
    }

    /// Get a quality score for this outcome (0.0 - 1.0)
    pub fn quality_score(&self) -> f32 {
        match self {
            StepOutcome::Success => 1.0,
            StepOutcome::Partial { .. } => 0.6,
            StepOutcome::Failure { .. } => 0.0,
            StepOutcome::Skipped { .. } => 0.3,
            StepOutcome::NeedsRetry { .. } => 0.2,
        }
    }
}

/// A single step in a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Step index (0-based)
    pub index: usize,
    /// Action taken (e.g., "analyze", "search", "generate")
    pub action: String,
    /// Rationale for taking this action
    pub rationale: String,
    /// Outcome of the step
    pub outcome: StepOutcome,
    /// Confidence in the action (0.0 - 1.0)
    pub confidence: f32,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Timestamp when step was executed
    pub timestamp: DateTime<Utc>,
    /// Optional embedding of the action context
    pub context_embedding: Option<Vec<f32>>,
    /// Optional metadata
    pub metadata: Option<StepMetadata>,
}

/// Additional metadata for a trajectory step
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepMetadata {
    /// Tool used (if any)
    pub tool_used: Option<String>,
    /// Input tokens consumed
    pub input_tokens: Option<u32>,
    /// Output tokens generated
    pub output_tokens: Option<u32>,
    /// Model used for this step
    pub model: Option<String>,
    /// Custom tags
    pub tags: Vec<String>,
    /// Custom key-value attributes
    pub attributes: std::collections::HashMap<String, String>,
}

impl TrajectoryStep {
    /// Create a new trajectory step
    pub fn new(
        index: usize,
        action: String,
        rationale: String,
        outcome: StepOutcome,
        confidence: f32,
    ) -> Self {
        Self {
            index,
            action,
            rationale,
            outcome,
            confidence,
            latency_ms: 0,
            timestamp: Utc::now(),
            context_embedding: None,
            metadata: None,
        }
    }

    /// Set latency
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Set context embedding
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.context_embedding = Some(embedding);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: StepMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Get quality score for this step
    pub fn quality(&self) -> f32 {
        self.outcome.quality_score() * self.confidence
    }
}

/// Metadata for an entire trajectory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrajectoryMetadata {
    /// Session ID this trajectory belongs to
    pub session_id: Option<String>,
    /// User ID (if known)
    pub user_id: Option<String>,
    /// Request type or category
    pub request_type: Option<String>,
    /// Total input tokens
    pub total_input_tokens: u32,
    /// Total output tokens
    pub total_output_tokens: u32,
    /// Model(s) used
    pub models_used: Vec<String>,
    /// Tools invoked
    pub tools_invoked: Vec<String>,
    /// Custom tags
    pub tags: Vec<String>,
    /// Custom attributes
    pub attributes: std::collections::HashMap<String, String>,
}

/// A complete trajectory representing an execution path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Unique identifier
    pub id: TrajectoryId,
    /// UUID for external reference
    pub uuid: Uuid,
    /// Query embedding (input representation)
    pub query_embedding: Vec<f32>,
    /// Response embedding (output representation)
    pub response_embedding: Option<Vec<f32>>,
    /// Execution steps
    pub steps: Vec<TrajectoryStep>,
    /// Final verdict
    pub verdict: Verdict,
    /// Overall quality score (0.0 - 1.0)
    pub quality: f32,
    /// Total latency in milliseconds
    pub total_latency_ms: u64,
    /// Start timestamp
    pub started_at: DateTime<Utc>,
    /// End timestamp
    pub completed_at: DateTime<Utc>,
    /// Metadata
    pub metadata: TrajectoryMetadata,
    /// Lessons learned (extracted post-hoc)
    pub lessons: Vec<String>,
}

impl Trajectory {
    /// Create a new trajectory
    pub fn new(query_embedding: Vec<f32>) -> Self {
        let now = Utc::now();
        Self {
            id: TrajectoryId::new(),
            uuid: Uuid::new_v4(),
            query_embedding,
            response_embedding: None,
            steps: Vec::new(),
            verdict: Verdict::Partial {
                completion_ratio: 0.0,
            },
            quality: 0.0,
            total_latency_ms: 0,
            started_at: now,
            completed_at: now,
            metadata: TrajectoryMetadata::default(),
            lessons: Vec::new(),
        }
    }

    /// Create from a compressed trajectory
    pub fn from_compressed(compressed: &super::CompressedTrajectory) -> Self {
        let now = Utc::now();
        Self {
            id: TrajectoryId::from_u64(compressed.original_id),
            uuid: Uuid::new_v4(),
            query_embedding: compressed.key_embedding.clone(),
            response_embedding: None,
            steps: Vec::new(), // Compressed trajectories lose step details
            verdict: compressed.verdict.clone(),
            quality: compressed.quality,
            total_latency_ms: 0,
            started_at: now,
            completed_at: now,
            metadata: TrajectoryMetadata::default(),
            lessons: compressed.preserved_lessons.clone(),
        }
    }

    /// Add a step to the trajectory
    pub fn add_step(&mut self, step: TrajectoryStep) {
        self.steps.push(step);
    }

    /// Complete the trajectory with a verdict
    pub fn complete(&mut self, verdict: Verdict) {
        self.verdict = verdict;
        self.completed_at = Utc::now();
        self.total_latency_ms = (self.completed_at - self.started_at).num_milliseconds() as u64;
        self.quality = self.compute_quality();
    }

    /// Compute overall quality score
    fn compute_quality(&self) -> f32 {
        if self.steps.is_empty() {
            return match &self.verdict {
                Verdict::Success => 1.0,
                Verdict::Failure(_) => 0.0,
                Verdict::Partial { completion_ratio } => *completion_ratio,
                Verdict::RecoveredViaReflection { final_quality, .. } => *final_quality,
            };
        }

        // Compute step-weighted quality
        let step_quality: f32 = self.steps.iter().map(|s| s.quality()).sum();
        let avg_step_quality = step_quality / self.steps.len() as f32;

        // Factor in verdict
        let verdict_factor = match &self.verdict {
            Verdict::Success => 1.0,
            Verdict::Failure(_) => 0.3,
            Verdict::Partial { completion_ratio } => 0.5 + 0.5 * completion_ratio,
            Verdict::RecoveredViaReflection { final_quality, .. } => *final_quality,
        };

        avg_step_quality * verdict_factor
    }

    /// Get total token count
    pub fn total_tokens(&self) -> u32 {
        self.metadata.total_input_tokens + self.metadata.total_output_tokens
    }

    /// Get step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Get success rate of steps
    pub fn step_success_rate(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let successes = self.steps.iter().filter(|s| s.outcome.is_success()).count();
        successes as f32 / self.steps.len() as f32
    }

    /// Get average step confidence
    pub fn avg_confidence(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let total: f32 = self.steps.iter().map(|s| s.confidence).sum();
        total / self.steps.len() as f32
    }

    /// Check if trajectory was successful
    pub fn is_success(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Success | Verdict::RecoveredViaReflection { .. }
        )
    }

    /// Check if trajectory failed
    pub fn is_failure(&self) -> bool {
        matches!(self.verdict, Verdict::Failure(_))
    }

    /// Add a lesson learned
    pub fn add_lesson(&mut self, lesson: String) {
        self.lessons.push(lesson);
    }

    /// Set response embedding
    pub fn set_response_embedding(&mut self, embedding: Vec<f32>) {
        self.response_embedding = Some(embedding);
    }
}

/// Builder for recording trajectories in real-time
pub struct TrajectoryRecorder {
    /// The trajectory being built
    trajectory: Trajectory,
    /// Current step index
    current_step: usize,
    /// Step start time
    step_start: Option<std::time::Instant>,
}

impl TrajectoryRecorder {
    /// Create a new trajectory recorder
    pub fn new(query_embedding: Vec<f32>) -> Self {
        Self {
            trajectory: Trajectory::new(query_embedding),
            current_step: 0,
            step_start: None,
        }
    }

    /// Start timing a step
    pub fn start_step(&mut self) {
        self.step_start = Some(std::time::Instant::now());
    }

    /// Add a step with automatic timing
    pub fn add_step(
        &mut self,
        action: String,
        rationale: String,
        outcome: StepOutcome,
        confidence: f32,
    ) {
        let latency_ms = self
            .step_start
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let step = TrajectoryStep::new(self.current_step, action, rationale, outcome, confidence)
            .with_latency(latency_ms);

        self.trajectory.add_step(step);
        self.current_step += 1;
        self.step_start = None;
    }

    /// Add a step with full control
    pub fn add_full_step(&mut self, mut step: TrajectoryStep) {
        step.index = self.current_step;
        self.trajectory.add_step(step);
        self.current_step += 1;
    }

    /// Set session ID
    pub fn set_session_id(&mut self, session_id: String) {
        self.trajectory.metadata.session_id = Some(session_id);
    }

    /// Set user ID
    pub fn set_user_id(&mut self, user_id: String) {
        self.trajectory.metadata.user_id = Some(user_id);
    }

    /// Set request type
    pub fn set_request_type(&mut self, request_type: String) {
        self.trajectory.metadata.request_type = Some(request_type);
    }

    /// Add tag
    pub fn add_tag(&mut self, tag: String) {
        self.trajectory.metadata.tags.push(tag);
    }

    /// Record token usage
    pub fn record_tokens(&mut self, input_tokens: u32, output_tokens: u32) {
        self.trajectory.metadata.total_input_tokens += input_tokens;
        self.trajectory.metadata.total_output_tokens += output_tokens;
    }

    /// Record model used
    pub fn record_model(&mut self, model: String) {
        if !self.trajectory.metadata.models_used.contains(&model) {
            self.trajectory.metadata.models_used.push(model);
        }
    }

    /// Record tool invoked
    pub fn record_tool(&mut self, tool: String) {
        if !self.trajectory.metadata.tools_invoked.contains(&tool) {
            self.trajectory.metadata.tools_invoked.push(tool);
        }
    }

    /// Add a lesson learned
    pub fn add_lesson(&mut self, lesson: String) {
        self.trajectory.add_lesson(lesson);
    }

    /// Set response embedding
    pub fn set_response_embedding(&mut self, embedding: Vec<f32>) {
        self.trajectory.set_response_embedding(embedding);
    }

    /// Complete the trajectory with a verdict
    pub fn complete(mut self, verdict: Verdict) -> Trajectory {
        self.trajectory.complete(verdict);
        self.trajectory
    }

    /// Get the current step count
    pub fn step_count(&self) -> usize {
        self.current_step
    }

    /// Get read-only access to the trajectory being built
    pub fn trajectory(&self) -> &Trajectory {
        &self.trajectory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_id_generation() {
        let id1 = TrajectoryId::new();
        let id2 = TrajectoryId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_step_outcome_quality() {
        assert_eq!(StepOutcome::Success.quality_score(), 1.0);
        assert_eq!(
            StepOutcome::Failure {
                error: "test".into()
            }
            .quality_score(),
            0.0
        );
    }

    #[test]
    fn test_trajectory_step_creation() {
        let step = TrajectoryStep::new(
            0,
            "analyze".to_string(),
            "Need to understand context".to_string(),
            StepOutcome::Success,
            0.9,
        );

        assert_eq!(step.index, 0);
        assert_eq!(step.action, "analyze");
        assert_eq!(step.quality(), 0.9); // 1.0 * 0.9
    }

    #[test]
    fn test_trajectory_creation() {
        let trajectory = Trajectory::new(vec![0.1; 768]);
        assert_eq!(trajectory.steps.len(), 0);
        assert!(!trajectory.is_success());
    }

    #[test]
    fn test_trajectory_recorder() {
        let mut recorder = TrajectoryRecorder::new(vec![0.1; 768]);
        recorder.set_session_id("session-1".to_string());
        recorder.set_user_id("user-1".to_string());

        recorder.add_step(
            "search".to_string(),
            "Finding relevant context".to_string(),
            StepOutcome::Success,
            0.95,
        );

        recorder.add_step(
            "generate".to_string(),
            "Creating response".to_string(),
            StepOutcome::Success,
            0.9,
        );

        let trajectory = recorder.complete(Verdict::Success);

        assert_eq!(trajectory.steps.len(), 2);
        assert!(trajectory.is_success());
        assert!(trajectory.quality > 0.8);
    }

    #[test]
    fn test_trajectory_quality_computation() {
        let mut trajectory = Trajectory::new(vec![0.1; 768]);

        trajectory.add_step(TrajectoryStep::new(
            0,
            "step1".to_string(),
            "rationale1".to_string(),
            StepOutcome::Success,
            1.0,
        ));

        trajectory.add_step(TrajectoryStep::new(
            1,
            "step2".to_string(),
            "rationale2".to_string(),
            StepOutcome::Failure {
                error: "test".to_string(),
            },
            0.5,
        ));

        trajectory.complete(Verdict::Partial {
            completion_ratio: 0.5,
        });

        // Quality should reflect the mix of success/failure
        assert!(trajectory.quality < 1.0);
        assert!(trajectory.quality > 0.0);
    }

    #[test]
    fn test_trajectory_stats() {
        let mut recorder = TrajectoryRecorder::new(vec![0.1; 768]);

        recorder.add_step(
            "step1".to_string(),
            "r1".to_string(),
            StepOutcome::Success,
            0.9,
        );
        recorder.add_step(
            "step2".to_string(),
            "r2".to_string(),
            StepOutcome::Success,
            0.8,
        );
        recorder.add_step(
            "step3".to_string(),
            "r3".to_string(),
            StepOutcome::Failure {
                error: "e".to_string(),
            },
            0.7,
        );

        let trajectory = recorder.complete(Verdict::Partial {
            completion_ratio: 0.67,
        });

        assert_eq!(trajectory.step_count(), 3);
        assert!((trajectory.step_success_rate() - 0.666).abs() < 0.01);
        assert!((trajectory.avg_confidence() - 0.8).abs() < 0.01);
    }
}
