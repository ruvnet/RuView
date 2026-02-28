//! Reflective Agent Wrapper
//!
//! Provides a wrapper around base agents that adds self-reflection and error recovery
//! capabilities. The reflective agent can retry with context, check confidence levels,
//! apply multi-perspective critique, and learn from execution trajectories.

use super::confidence::{ConfidenceChecker, ConfidenceConfig, ConfidenceLevel, WeakPoint};
use super::error_recovery::{ErrorPatternLearner, ErrorPatternLearnerConfig, RecoverySuggestion};
use super::perspectives::{CritiqueResult, Perspective, UnifiedCritique};
use crate::claude_flow::{AgentType, Verdict};
use crate::error::{Result, RuvLLMError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for reflection behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionConfig {
    /// Maximum reflection attempts before giving up
    pub max_reflection_attempts: u32,
    /// Timeout for each reflection attempt
    pub reflection_timeout_ms: u64,
    /// Whether to learn from successful recoveries
    pub learn_from_recovery: bool,
    /// Minimum quality threshold for accepting a result
    pub min_quality_threshold: f32,
    /// Whether to record trajectories for analysis
    pub record_trajectories: bool,
    /// Confidence configuration for IoE strategy
    pub confidence_config: ConfidenceConfig,
    /// Error learner configuration
    pub error_learner_config: ErrorPatternLearnerConfig,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            max_reflection_attempts: 3,
            reflection_timeout_ms: 30000, // 30 seconds
            learn_from_recovery: true,
            min_quality_threshold: 0.7,
            record_trajectories: true,
            confidence_config: ConfidenceConfig::default(),
            error_learner_config: ErrorPatternLearnerConfig::default(),
        }
    }
}

/// Configuration for retry strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Backoff multiplier between retries
    pub backoff_multiplier: f32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Whether to include previous error in retry context
    pub include_error_context: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_multiplier: 2.0,
            initial_delay_ms: 100,
            include_error_context: true,
        }
    }
}

/// Reflection strategy variants
#[derive(Clone, Serialize, Deserialize)]
pub enum ReflectionStrategy {
    /// Simple retry with reflection context on failure
    Retry(RetryConfig),

    /// If-or-Else pattern: only revise when confidence is LOW
    /// This is more efficient than always reflecting
    IfOrElse {
        /// Confidence checker for determining when to revise
        #[serde(skip)]
        checker: Option<Arc<ConfidenceChecker>>,
        /// Confidence threshold below which revision is triggered
        threshold: f32,
        /// Maximum revision budget
        revision_budget: u32,
    },

    /// Multi-perspective critique from different angles
    MultiPerspective {
        /// List of perspectives to apply
        #[serde(skip)]
        perspectives: Vec<Arc<dyn Perspective + Send + Sync>>,
        /// Minimum agreement ratio for accepting result
        min_agreement: f32,
    },

    /// Trajectory reflection - analyze entire execution path
    TrajectoryReflection {
        /// Window size for trajectory analysis
        window_size: usize,
        /// Whether to use SONA for trajectory learning
        use_sona: bool,
    },
}

impl std::fmt::Debug for ReflectionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retry(config) => f.debug_tuple("Retry").field(config).finish(),
            Self::IfOrElse {
                threshold,
                revision_budget,
                ..
            } => f
                .debug_struct("IfOrElse")
                .field("threshold", threshold)
                .field("revision_budget", revision_budget)
                .field("checker", &"<ConfidenceChecker>")
                .finish(),
            Self::MultiPerspective {
                min_agreement,
                perspectives,
            } => f
                .debug_struct("MultiPerspective")
                .field("min_agreement", min_agreement)
                .field("perspectives_count", &perspectives.len())
                .finish(),
            Self::TrajectoryReflection {
                window_size,
                use_sona,
            } => f
                .debug_struct("TrajectoryReflection")
                .field("window_size", window_size)
                .field("use_sona", use_sona)
                .finish(),
        }
    }
}

impl Default for ReflectionStrategy {
    fn default() -> Self {
        Self::Retry(RetryConfig::default())
    }
}

/// Context for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Task description
    pub task: String,
    /// Agent type performing the task
    pub agent_type: AgentType,
    /// Input data/context
    pub input: String,
    /// Previous attempts (if any)
    pub previous_attempts: Vec<PreviousAttempt>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Session identifier
    pub session_id: Option<String>,
    /// Parent task (for sub-tasks)
    pub parent_task: Option<String>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(task: impl Into<String>, agent_type: AgentType, input: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            agent_type,
            input: input.into(),
            previous_attempts: Vec::new(),
            metadata: HashMap::new(),
            session_id: None,
            parent_task: None,
        }
    }

    /// Add a previous attempt
    pub fn with_previous_attempt(mut self, attempt: PreviousAttempt) -> Self {
        self.previous_attempts.push(attempt);
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Record of a previous execution attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviousAttempt {
    /// Attempt number
    pub attempt_number: u32,
    /// Output from this attempt
    pub output: String,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Quality score (if available)
    pub quality_score: Option<f32>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Reflection applied (if any)
    pub reflection: Option<Reflection>,
}

/// Reflection generated during self-correction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reflection {
    /// Strategy used for this reflection
    pub strategy: String,
    /// Context about what went wrong
    pub context: String,
    /// Key insights from reflection
    pub insights: Vec<String>,
    /// Suggested improvements
    pub suggestions: Vec<String>,
    /// Confidence in the reflection
    pub confidence: f32,
    /// Weak points identified (for IoE strategy)
    pub weak_points: Vec<WeakPoint>,
    /// Critique results (for multi-perspective strategy)
    pub critique_results: Vec<CritiqueResult>,
    /// Time spent reflecting (ms)
    pub reflection_time_ms: u64,
}

impl Reflection {
    /// Create a new reflection
    pub fn new(strategy: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            strategy: strategy.into(),
            context: context.into(),
            insights: Vec::new(),
            suggestions: Vec::new(),
            confidence: 0.5,
            weak_points: Vec::new(),
            critique_results: Vec::new(),
            reflection_time_ms: 0,
        }
    }

    /// Add an insight
    pub fn with_insight(mut self, insight: impl Into<String>) -> Self {
        self.insights.push(insight.into());
        self
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Set confidence
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add weak points
    pub fn with_weak_points(mut self, weak_points: Vec<WeakPoint>) -> Self {
        self.weak_points = weak_points;
        self
    }

    /// Add critique results
    pub fn with_critiques(mut self, critiques: Vec<CritiqueResult>) -> Self {
        self.critique_results = critiques;
        self
    }
}

/// Result from reflective execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Final output
    pub output: String,
    /// Whether the result was recovered via reflection
    pub recovered_via_reflection: bool,
    /// Number of attempts made
    pub attempts: u32,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Final quality score
    pub quality_score: f32,
    /// Verdict for ReasoningBank integration
    pub verdict: Verdict,
    /// Reflection details (if recovery occurred)
    pub reflection: Option<Reflection>,
    /// All previous attempts
    pub attempt_history: Vec<PreviousAttempt>,
    /// Recovery suggestions that were applied
    pub applied_suggestions: Vec<RecoverySuggestion>,
}

impl ExecutionResult {
    /// Create a successful result
    pub fn success(output: impl Into<String>, attempts: u32, duration_ms: u64) -> Self {
        Self {
            output: output.into(),
            recovered_via_reflection: false,
            attempts,
            total_duration_ms: duration_ms,
            quality_score: 1.0,
            verdict: Verdict::Success {
                reason: "Task completed successfully".to_string(),
            },
            reflection: None,
            attempt_history: Vec::new(),
            applied_suggestions: Vec::new(),
        }
    }

    /// Create a recovered result
    pub fn recovered(
        output: impl Into<String>,
        original_error: impl Into<String>,
        recovery_strategy: impl Into<String>,
        attempts: u32,
        duration_ms: u64,
        reflection: Reflection,
    ) -> Self {
        Self {
            output: output.into(),
            recovered_via_reflection: true,
            attempts,
            total_duration_ms: duration_ms,
            quality_score: reflection.confidence,
            verdict: Verdict::RecoveredViaReflection {
                original_error: original_error.into(),
                recovery_strategy: recovery_strategy.into(),
                attempts,
            },
            reflection: Some(reflection),
            attempt_history: Vec::new(),
            applied_suggestions: Vec::new(),
        }
    }

    /// Create a failure result
    pub fn failure(error: impl Into<String>, attempts: u32, duration_ms: u64) -> Self {
        Self {
            output: String::new(),
            recovered_via_reflection: false,
            attempts,
            total_duration_ms: duration_ms,
            quality_score: 0.0,
            verdict: Verdict::Failure {
                reason: error.into(),
                error_code: None,
            },
            reflection: None,
            attempt_history: Vec::new(),
            applied_suggestions: Vec::new(),
        }
    }

    /// Add attempt history
    pub fn with_history(mut self, history: Vec<PreviousAttempt>) -> Self {
        self.attempt_history = history;
        self
    }
}

/// Base agent trait that reflective agent wraps
pub trait BaseAgent: Send + Sync {
    /// Execute a task
    fn execute(&self, context: &ExecutionContext) -> Result<String>;

    /// Get the agent type
    fn agent_type(&self) -> AgentType;

    /// Estimate confidence in an output
    fn estimate_confidence(&self, output: &str, _context: &ExecutionContext) -> f32 {
        // Default implementation: base confidence on output length and structure
        let has_content = !output.is_empty();
        let has_structure = output.contains('\n') || output.len() > 100;
        let output_lower = output.to_lowercase();
        let not_error = !output_lower.contains("error") && !output_lower.contains("failed");

        let score = (has_content as u8 as f32 * 0.3)
            + (has_structure as u8 as f32 * 0.3)
            + (not_error as u8 as f32 * 0.4);
        score
    }
}

/// Reflective agent wrapper that adds self-reflection capabilities
pub struct ReflectiveAgent<A: BaseAgent> {
    /// Base agent being wrapped
    base_agent: A,
    /// Reflection strategy to use
    strategy: ReflectionStrategy,
    /// Configuration
    config: ReflectionConfig,
    /// Error pattern learner for recovery suggestions
    error_learner: ErrorPatternLearner,
    /// Confidence checker for IoE strategy
    confidence_checker: ConfidenceChecker,
    /// Execution statistics
    stats: ReflectiveAgentStats,
}

/// Statistics for reflective agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReflectiveAgentStats {
    /// Total executions
    pub total_executions: u64,
    /// Successful first-try executions
    pub first_try_successes: u64,
    /// Recovered via reflection
    pub recovered_count: u64,
    /// Failed despite reflection
    pub failed_count: u64,
    /// Total reflection time (ms)
    pub total_reflection_time_ms: u64,
    /// Average attempts per task
    pub avg_attempts: f32,
    /// Recovery rate
    pub recovery_rate: f32,
}

impl<A: BaseAgent> ReflectiveAgent<A> {
    /// Create a new reflective agent
    pub fn new(base_agent: A, strategy: ReflectionStrategy) -> Self {
        let config = ReflectionConfig::default();
        let error_learner = ErrorPatternLearner::new(config.error_learner_config.clone());
        let confidence_checker = ConfidenceChecker::new(config.confidence_config.clone());

        Self {
            base_agent,
            strategy,
            config,
            error_learner,
            confidence_checker,
            stats: ReflectiveAgentStats::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        base_agent: A,
        strategy: ReflectionStrategy,
        config: ReflectionConfig,
    ) -> Self {
        let error_learner = ErrorPatternLearner::new(config.error_learner_config.clone());
        let confidence_checker = ConfidenceChecker::new(config.confidence_config.clone());

        Self {
            base_agent,
            strategy,
            config,
            error_learner,
            confidence_checker,
            stats: ReflectiveAgentStats::default(),
        }
    }

    /// Execute with automatic reflection on failure or low confidence
    pub fn execute_with_reflection(
        &mut self,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let start = Instant::now();
        let mut attempts = 0u32;
        let mut attempt_history = Vec::new();
        let mut last_error: Option<String> = None;
        let mut last_reflection: Option<Reflection> = None;
        let mut applied_suggestions = Vec::new();

        // Create mutable context for retries
        let mut current_context = context.clone();

        loop {
            attempts += 1;
            let attempt_start = Instant::now();

            // Check if we've exceeded max attempts
            if attempts > self.config.max_reflection_attempts {
                self.stats.failed_count += 1;
                self.stats.total_executions += 1;

                return Ok(ExecutionResult::failure(
                    last_error.unwrap_or_else(|| "Max reflection attempts exceeded".to_string()),
                    attempts - 1,
                    start.elapsed().as_millis() as u64,
                )
                .with_history(attempt_history));
            }

            // Execute the task
            let result = self.base_agent.execute(&current_context);

            match result {
                Ok(output) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;

                    // Check confidence based on strategy
                    let should_reflect = self.should_reflect(&output, &current_context);

                    if !should_reflect {
                        // Success!
                        self.stats.total_executions += 1;
                        if attempts == 1 {
                            self.stats.first_try_successes += 1;
                        } else {
                            self.stats.recovered_count += 1;
                        }
                        self.update_avg_attempts(attempts);

                        // Learn from successful recovery if applicable
                        if self.config.learn_from_recovery && last_error.is_some() {
                            if let Some(ref error) = last_error {
                                self.error_learner.learn_from_recovery(
                                    error,
                                    &output,
                                    last_reflection.as_ref(),
                                );
                            }
                        }

                        let mut exec_result = if attempts > 1 && last_error.is_some() {
                            ExecutionResult::recovered(
                                output,
                                last_error.unwrap(),
                                self.strategy_name(),
                                attempts,
                                start.elapsed().as_millis() as u64,
                                last_reflection.unwrap_or_else(|| {
                                    Reflection::new("retry", "Recovered on retry")
                                }),
                            )
                        } else {
                            ExecutionResult::success(
                                output,
                                attempts,
                                start.elapsed().as_millis() as u64,
                            )
                        };

                        exec_result.attempt_history = attempt_history;
                        exec_result.applied_suggestions = applied_suggestions;
                        return Ok(exec_result);
                    }

                    // Generate reflection for low confidence
                    let reflection_start = Instant::now();
                    let reflection = self.generate_reflection(&output, &current_context, None)?;
                    self.stats.total_reflection_time_ms +=
                        reflection_start.elapsed().as_millis() as u64;

                    // Record this attempt
                    attempt_history.push(PreviousAttempt {
                        attempt_number: attempts,
                        output: output.clone(),
                        error: None,
                        quality_score: Some(reflection.confidence),
                        duration_ms,
                        reflection: Some(reflection.clone()),
                    });

                    // Update context with reflection
                    current_context =
                        self.retry_with_context(&current_context, Some(&output), None, &reflection);

                    last_reflection = Some(reflection);
                }
                Err(e) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;
                    let error_msg = e.to_string();

                    // Get recovery suggestions
                    let suggestions = self.error_learner.suggest_recovery(&error_msg);

                    // Generate reflection for error
                    let reflection_start = Instant::now();
                    let reflection =
                        self.generate_reflection("", &current_context, Some(&error_msg))?;
                    self.stats.total_reflection_time_ms +=
                        reflection_start.elapsed().as_millis() as u64;

                    // Record this attempt
                    attempt_history.push(PreviousAttempt {
                        attempt_number: attempts,
                        output: String::new(),
                        error: Some(error_msg.clone()),
                        quality_score: Some(0.0),
                        duration_ms,
                        reflection: Some(reflection.clone()),
                    });

                    // Apply suggestions
                    for suggestion in &suggestions {
                        if suggestion.confidence > 0.5 {
                            applied_suggestions.push(suggestion.clone());
                        }
                    }

                    // Update context with reflection and error
                    current_context = self.retry_with_context(
                        &current_context,
                        None,
                        Some(&error_msg),
                        &reflection,
                    );

                    last_error = Some(error_msg);
                    last_reflection = Some(reflection);
                }
            }
        }
    }

    /// Determine if reflection is needed based on strategy
    fn should_reflect(&self, output: &str, context: &ExecutionContext) -> bool {
        match &self.strategy {
            ReflectionStrategy::Retry(_) => {
                // For retry strategy, only reflect on actual errors (handled in execute)
                false
            }
            ReflectionStrategy::IfOrElse {
                threshold,
                revision_budget,
                ..
            } => {
                // Only revise when confidence is LOW
                let confidence = self.base_agent.estimate_confidence(output, context);
                let attempts = context.previous_attempts.len() as u32;
                confidence < *threshold && attempts < *revision_budget
            }
            ReflectionStrategy::MultiPerspective {
                min_agreement,
                perspectives,
            } => {
                // Check agreement across perspectives
                if perspectives.is_empty() {
                    return false;
                }

                let mut agreements = 0;
                for perspective in perspectives {
                    let critique = perspective.critique(output, context);
                    if critique.passed {
                        agreements += 1;
                    }
                }

                let agreement_ratio = agreements as f32 / perspectives.len() as f32;
                agreement_ratio < *min_agreement
            }
            ReflectionStrategy::TrajectoryReflection { window_size, .. } => {
                // Analyze recent trajectory quality
                let recent_quality: f32 = context
                    .previous_attempts
                    .iter()
                    .rev()
                    .take(*window_size)
                    .filter_map(|a| a.quality_score)
                    .sum::<f32>()
                    / context.previous_attempts.len().min(*window_size).max(1) as f32;

                recent_quality < self.config.min_quality_threshold
            }
        }
    }

    /// Generate reflection based on current strategy
    pub fn generate_reflection(
        &self,
        output: &str,
        context: &ExecutionContext,
        error: Option<&str>,
    ) -> Result<Reflection> {
        let start = Instant::now();

        let mut reflection = match &self.strategy {
            ReflectionStrategy::Retry(config) => {
                let mut r = Reflection::new("retry", "Retry with accumulated context");
                if let Some(e) = error {
                    r.insights.push(format!("Error encountered: {}", e));
                    r.suggestions
                        .push("Review error and adjust approach".to_string());
                }
                if config.include_error_context && !context.previous_attempts.is_empty() {
                    r.insights.push(format!(
                        "Previous {} attempts failed",
                        context.previous_attempts.len()
                    ));
                }
                r
            }

            ReflectionStrategy::IfOrElse { threshold, .. } => {
                let confidence = self.base_agent.estimate_confidence(output, context);
                let weak_points = self
                    .confidence_checker
                    .identify_weak_points(output, context);

                let mut r = Reflection::new(
                    "if_or_else",
                    format!(
                        "Confidence {} ({:.2}) threshold {:.2}",
                        if confidence < *threshold {
                            "below"
                        } else {
                            "meets"
                        },
                        confidence,
                        threshold
                    ),
                );

                r.confidence = confidence;
                r.weak_points = weak_points.clone();

                for wp in &weak_points {
                    r.insights.push(format!(
                        "{}: {} (severity: {:.2})",
                        wp.location, wp.description, wp.severity
                    ));
                    r.suggestions.push(wp.suggestion.clone());
                }

                r
            }

            ReflectionStrategy::MultiPerspective { perspectives, .. } => {
                let mut r = Reflection::new("multi_perspective", "Multi-angle critique");
                let mut critiques = Vec::new();

                for perspective in perspectives {
                    let critique = perspective.critique(output, context);
                    r.insights.push(format!(
                        "[{}] {}: {}",
                        critique.perspective_name,
                        if critique.passed { "PASS" } else { "FAIL" },
                        critique.summary
                    ));

                    for issue in &critique.issues {
                        r.suggestions.push(format!(
                            "[{}] {}",
                            critique.perspective_name, issue.suggestion
                        ));
                    }

                    critiques.push(critique);
                }

                // Compute aggregate confidence
                let avg_score: f32 =
                    critiques.iter().map(|c| c.score).sum::<f32>() / critiques.len().max(1) as f32;
                r.confidence = avg_score;
                r.critique_results = critiques;

                r
            }

            ReflectionStrategy::TrajectoryReflection { window_size, .. } => {
                let mut r =
                    Reflection::new("trajectory", "Trajectory analysis over execution history");

                // Analyze patterns in previous attempts
                let recent: Vec<_> = context
                    .previous_attempts
                    .iter()
                    .rev()
                    .take(*window_size)
                    .collect();

                if !recent.is_empty() {
                    // Look for recurring errors
                    let error_count = recent.iter().filter(|a| a.error.is_some()).count();
                    if error_count > 0 {
                        r.insights.push(format!(
                            "{} errors in last {} attempts",
                            error_count,
                            recent.len()
                        ));
                    }

                    // Look for quality trends
                    let qualities: Vec<f32> =
                        recent.iter().filter_map(|a| a.quality_score).collect();
                    if qualities.len() >= 2 {
                        let trend = qualities[0] - qualities[qualities.len() - 1];
                        if trend > 0.1 {
                            r.insights.push("Quality improving".to_string());
                        } else if trend < -0.1 {
                            r.insights
                                .push("Quality declining - consider strategy change".to_string());
                            r.suggestions
                                .push("Try different approach or break task down".to_string());
                        }
                    }

                    // Compute trajectory confidence
                    let avg_quality = qualities.iter().sum::<f32>() / qualities.len().max(1) as f32;
                    r.confidence = avg_quality;
                }

                r
            }
        };

        reflection.reflection_time_ms = start.elapsed().as_millis() as u64;
        Ok(reflection)
    }

    /// Create new context with reflection information for retry
    pub fn retry_with_context(
        &self,
        original: &ExecutionContext,
        previous_output: Option<&str>,
        error: Option<&str>,
        reflection: &Reflection,
    ) -> ExecutionContext {
        let mut context = original.clone();

        // Add the current attempt to history
        let attempt_number = context.previous_attempts.len() as u32 + 1;
        context.previous_attempts.push(PreviousAttempt {
            attempt_number,
            output: previous_output.unwrap_or("").to_string(),
            error: error.map(String::from),
            quality_score: Some(reflection.confidence),
            duration_ms: 0,
            reflection: Some(reflection.clone()),
        });

        // Augment input with reflection insights
        let mut augmented_input = context.input.clone();
        augmented_input.push_str("\n\n--- Reflection Context ---\n");

        if let Some(e) = error {
            augmented_input.push_str(&format!("Previous error: {}\n", e));
        }

        if !reflection.insights.is_empty() {
            augmented_input.push_str("Insights:\n");
            for insight in &reflection.insights {
                augmented_input.push_str(&format!("- {}\n", insight));
            }
        }

        if !reflection.suggestions.is_empty() {
            augmented_input.push_str("Suggestions:\n");
            for suggestion in &reflection.suggestions {
                augmented_input.push_str(&format!("- {}\n", suggestion));
            }
        }

        context.input = augmented_input;
        context
    }

    /// Get the strategy name
    fn strategy_name(&self) -> String {
        match &self.strategy {
            ReflectionStrategy::Retry(_) => "retry".to_string(),
            ReflectionStrategy::IfOrElse { .. } => "if_or_else".to_string(),
            ReflectionStrategy::MultiPerspective { .. } => "multi_perspective".to_string(),
            ReflectionStrategy::TrajectoryReflection { .. } => "trajectory".to_string(),
        }
    }

    /// Update average attempts statistic
    fn update_avg_attempts(&mut self, attempts: u32) {
        let n = self.stats.total_executions as f32;
        self.stats.avg_attempts =
            (self.stats.avg_attempts * (n - 1.0) + attempts as f32) / n.max(1.0);

        // Update recovery rate
        let total =
            self.stats.first_try_successes + self.stats.recovered_count + self.stats.failed_count;
        if total > 0 {
            self.stats.recovery_rate = self.stats.recovered_count as f32
                / (self.stats.recovered_count + self.stats.failed_count).max(1) as f32;
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &ReflectiveAgentStats {
        &self.stats
    }

    /// Get reference to error learner
    pub fn error_learner(&self) -> &ErrorPatternLearner {
        &self.error_learner
    }

    /// Get mutable reference to error learner
    pub fn error_learner_mut(&mut self) -> &mut ErrorPatternLearner {
        &mut self.error_learner
    }

    /// Get reference to confidence checker
    pub fn confidence_checker(&self) -> &ConfidenceChecker {
        &self.confidence_checker
    }

    /// Get reference to base agent
    pub fn base_agent(&self) -> &A {
        &self.base_agent
    }

    /// Get mutable reference to base agent
    pub fn base_agent_mut(&mut self) -> &mut A {
        &mut self.base_agent
    }

    /// Set strategy
    pub fn set_strategy(&mut self, strategy: ReflectionStrategy) {
        self.strategy = strategy;
    }

    /// Get strategy
    pub fn strategy(&self) -> &ReflectionStrategy {
        &self.strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Simple test agent for testing
    struct TestAgent {
        agent_type: AgentType,
        fail_count: AtomicU32,
        max_fails: u32,
    }

    impl TestAgent {
        fn new(max_fails: u32) -> Self {
            Self {
                agent_type: AgentType::Coder,
                fail_count: AtomicU32::new(0),
                max_fails,
            }
        }
    }

    impl BaseAgent for TestAgent {
        fn execute(&self, context: &ExecutionContext) -> Result<String> {
            let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if count < self.max_fails {
                Err(RuvLLMError::InvalidOperation(format!(
                    "Simulated failure {}",
                    count + 1
                )))
            } else {
                Ok(format!(
                    "Success after {} failures for: {}",
                    count, context.task
                ))
            }
        }

        fn agent_type(&self) -> AgentType {
            self.agent_type
        }
    }

    #[test]
    fn test_reflective_agent_retry_success() {
        let base = TestAgent::new(2); // Fail twice then succeed
        let mut agent =
            ReflectiveAgent::new(base, ReflectionStrategy::Retry(RetryConfig::default()));

        let context = ExecutionContext::new("test task", AgentType::Coder, "test input");
        let result = agent.execute_with_reflection(&context).unwrap();

        assert!(result.recovered_via_reflection);
        assert_eq!(result.attempts, 3);
        assert!(result.output.contains("Success"));
    }

    #[test]
    fn test_reflective_agent_max_attempts() {
        let base = TestAgent::new(10); // Always fail
        let config = ReflectionConfig {
            max_reflection_attempts: 3,
            ..Default::default()
        };
        let mut agent = ReflectiveAgent::with_config(
            base,
            ReflectionStrategy::Retry(RetryConfig::default()),
            config,
        );

        let context = ExecutionContext::new("test task", AgentType::Coder, "test input");
        let result = agent.execute_with_reflection(&context).unwrap();

        assert!(!result.recovered_via_reflection);
        assert!(matches!(result.verdict, Verdict::Failure { .. }));
    }

    #[test]
    fn test_reflection_generation() {
        let base = TestAgent::new(0);
        let agent = ReflectiveAgent::new(base, ReflectionStrategy::Retry(RetryConfig::default()));

        let context = ExecutionContext::new("test", AgentType::Coder, "input");
        let reflection = agent
            .generate_reflection("output", &context, Some("test error"))
            .unwrap();

        assert_eq!(reflection.strategy, "retry");
        assert!(!reflection.insights.is_empty());
    }

    #[test]
    fn test_execution_context_builder() {
        let context = ExecutionContext::new("task", AgentType::Researcher, "input")
            .with_session("session-123")
            .with_metadata("key", "value");

        assert_eq!(context.session_id, Some("session-123".to_string()));
        assert_eq!(context.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_execution_result_variants() {
        let success = ExecutionResult::success("output", 1, 100);
        assert!(matches!(success.verdict, Verdict::Success { .. }));

        let recovered = ExecutionResult::recovered(
            "output",
            "error",
            "retry",
            2,
            200,
            Reflection::new("retry", "context"),
        );
        assert!(matches!(
            recovered.verdict,
            Verdict::RecoveredViaReflection { .. }
        ));
        assert!(recovered.recovered_via_reflection);

        let failure = ExecutionResult::failure("error", 3, 300);
        assert!(matches!(failure.verdict, Verdict::Failure { .. }));
    }

    #[test]
    fn test_stats_tracking() {
        let base = TestAgent::new(1);
        let mut agent =
            ReflectiveAgent::new(base, ReflectionStrategy::Retry(RetryConfig::default()));

        let context = ExecutionContext::new("test", AgentType::Coder, "input");
        let _ = agent.execute_with_reflection(&context);

        let stats = agent.stats();
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.recovered_count, 1);
    }
}
