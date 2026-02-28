//! Enhanced Verdict System for ReasoningBank
//!
//! Provides rich verdict types for classifying trajectory outcomes,
//! failure analysis, and pattern extraction from execution results.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{PatternCategory, StepOutcome, Trajectory};

/// Verdict for a trajectory execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Verdict {
    /// Fully successful execution
    Success,
    /// Execution failed
    Failure(RootCause),
    /// Partially completed
    Partial {
        /// Completion ratio (0.0 - 1.0)
        completion_ratio: f32,
    },
    /// Initially failed but recovered through reflection
    RecoveredViaReflection {
        /// Original failure cause
        original_failure: Box<RootCause>,
        /// Number of reflection attempts
        reflection_attempts: u32,
        /// Final quality after recovery
        final_quality: f32,
    },
}

impl Default for Verdict {
    fn default() -> Self {
        Self::Partial {
            completion_ratio: 0.0,
        }
    }
}

impl Verdict {
    /// Create a success verdict
    pub fn success() -> Self {
        Self::Success
    }

    /// Create a failure verdict
    pub fn failure(cause: RootCause) -> Self {
        Self::Failure(cause)
    }

    /// Create a partial verdict
    pub fn partial(completion_ratio: f32) -> Self {
        Self::Partial {
            completion_ratio: completion_ratio.clamp(0.0, 1.0),
        }
    }

    /// Create a recovered verdict
    pub fn recovered(original: RootCause, attempts: u32, quality: f32) -> Self {
        Self::RecoveredViaReflection {
            original_failure: Box::new(original),
            reflection_attempts: attempts,
            final_quality: quality.clamp(0.0, 1.0),
        }
    }

    /// Check if verdict represents success
    pub fn is_success(&self) -> bool {
        match self {
            Self::Success => true,
            Self::RecoveredViaReflection { final_quality, .. } => *final_quality >= 0.8,
            _ => false,
        }
    }

    /// Check if verdict represents failure
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure(_))
    }

    /// Get quality score
    pub fn quality_score(&self) -> f32 {
        match self {
            Self::Success => 1.0,
            Self::Failure(_) => 0.0,
            Self::Partial { completion_ratio } => *completion_ratio * 0.5 + 0.25,
            Self::RecoveredViaReflection { final_quality, .. } => *final_quality,
        }
    }

    /// Get description
    pub fn description(&self) -> String {
        match self {
            Self::Success => "Success".to_string(),
            Self::Failure(cause) => format!("Failure: {}", cause),
            Self::Partial { completion_ratio } => {
                format!("Partial: {:.0}% complete", completion_ratio * 100.0)
            }
            Self::RecoveredViaReflection {
                reflection_attempts,
                final_quality,
                ..
            } => {
                format!(
                    "Recovered after {} attempts, quality {:.0}%",
                    reflection_attempts,
                    final_quality * 100.0
                )
            }
        }
    }
}

/// Root cause analysis for failures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RootCause {
    /// Insufficient context or information
    InsufficientContext {
        /// What was missing
        missing: Vec<String>,
    },
    /// Invalid or ambiguous input
    InvalidInput {
        /// Details about the issue
        details: String,
    },
    /// Tool execution failure
    ToolFailure {
        /// Tool that failed
        tool: String,
        /// Error message
        error: String,
    },
    /// Reasoning error
    ReasoningError {
        /// Type of reasoning error
        error_type: ReasoningErrorType,
        /// Description
        description: String,
    },
    /// Resource constraints (time, memory, etc.)
    ResourceConstraint {
        /// Resource type
        resource: String,
        /// Limit reached
        limit: String,
    },
    /// External service failure
    ExternalFailure {
        /// Service name
        service: String,
        /// Error details
        error: String,
    },
    /// Model capability limitation
    CapabilityLimit {
        /// What was beyond capability
        limitation: String,
    },
    /// Unknown or unclassified failure
    Unknown {
        /// Any available details
        details: String,
    },
}

impl std::fmt::Display for RootCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientContext { missing } => {
                write!(f, "Insufficient context: missing {}", missing.join(", "))
            }
            Self::InvalidInput { details } => write!(f, "Invalid input: {}", details),
            Self::ToolFailure { tool, error } => write!(f, "Tool '{}' failed: {}", tool, error),
            Self::ReasoningError {
                error_type,
                description,
            } => {
                write!(f, "Reasoning error ({}): {}", error_type, description)
            }
            Self::ResourceConstraint { resource, limit } => {
                write!(f, "Resource constraint: {} exceeded {}", resource, limit)
            }
            Self::ExternalFailure { service, error } => {
                write!(f, "External failure ({}): {}", service, error)
            }
            Self::CapabilityLimit { limitation } => {
                write!(f, "Capability limit: {}", limitation)
            }
            Self::Unknown { details } => write!(f, "Unknown failure: {}", details),
        }
    }
}

/// Types of reasoning errors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningErrorType {
    /// Logical inconsistency
    LogicalInconsistency,
    /// Factual error
    FactualError,
    /// Hallucination
    Hallucination,
    /// Circular reasoning
    CircularReasoning,
    /// Non-sequitur
    NonSequitur,
    /// Over-generalization
    OverGeneralization,
    /// Under-specification
    UnderSpecification,
    /// Other
    Other,
}

impl std::fmt::Display for ReasoningErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LogicalInconsistency => write!(f, "logical_inconsistency"),
            Self::FactualError => write!(f, "factual_error"),
            Self::Hallucination => write!(f, "hallucination"),
            Self::CircularReasoning => write!(f, "circular_reasoning"),
            Self::NonSequitur => write!(f, "non_sequitur"),
            Self::OverGeneralization => write!(f, "over_generalization"),
            Self::UnderSpecification => write!(f, "under_specification"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// A pattern observed in failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Pattern ID
    pub id: String,
    /// Root cause type
    pub cause_type: String,
    /// Frequency of occurrence
    pub frequency: u32,
    /// Common triggers
    pub triggers: Vec<String>,
    /// Successful mitigations
    pub mitigations: Vec<String>,
    /// Associated step actions
    pub associated_actions: Vec<String>,
}

/// Strategy for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStrategy {
    /// Strategy name
    pub name: String,
    /// Description
    pub description: String,
    /// Applicable root causes
    pub applicable_causes: Vec<String>,
    /// Success rate when applied
    pub success_rate: f32,
    /// Recommended actions
    pub actions: Vec<String>,
    /// Estimated effort (1-10)
    pub effort_score: u8,
}

/// Analysis result from VerdictAnalyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerdictAnalysis {
    /// The verdict being analyzed
    pub verdict: Verdict,
    /// Root cause (if failure)
    pub root_cause: Option<RootCause>,
    /// Contributing factors
    pub contributing_factors: Vec<String>,
    /// Failure patterns identified
    pub failure_patterns: Vec<FailurePattern>,
    /// Recommended recovery strategies
    pub recovery_strategies: Vec<RecoveryStrategy>,
    /// Lessons learned
    pub lessons: Vec<String>,
    /// Pattern category
    pub pattern_category: PatternCategory,
    /// Confidence in analysis
    pub confidence: f32,
    /// Suggested improvements
    pub improvements: Vec<String>,
}

impl VerdictAnalysis {
    /// Create a new analysis
    pub fn new(verdict: Verdict) -> Self {
        let pattern_category = match &verdict {
            Verdict::Success => PatternCategory::General,
            Verdict::Failure(_) => PatternCategory::ErrorRecovery,
            Verdict::Partial { .. } => PatternCategory::General,
            Verdict::RecoveredViaReflection { .. } => PatternCategory::Reflection,
        };

        Self {
            verdict,
            root_cause: None,
            contributing_factors: Vec::new(),
            failure_patterns: Vec::new(),
            recovery_strategies: Vec::new(),
            lessons: Vec::new(),
            pattern_category,
            confidence: 0.5,
            improvements: Vec::new(),
        }
    }
}

/// Analyzer for extracting insights from verdicts
pub struct VerdictAnalyzer {
    /// Known failure patterns
    known_patterns: HashMap<String, FailurePattern>,
    /// Recovery strategy database
    recovery_strategies: Vec<RecoveryStrategy>,
    /// Analysis count
    analysis_count: u64,
}

impl VerdictAnalyzer {
    /// Create a new analyzer
    pub fn new() -> Self {
        Self {
            known_patterns: Self::initialize_patterns(),
            recovery_strategies: Self::initialize_strategies(),
            analysis_count: 0,
        }
    }

    /// Initialize known failure patterns
    fn initialize_patterns() -> HashMap<String, FailurePattern> {
        let mut patterns = HashMap::new();

        patterns.insert(
            "context_gap".to_string(),
            FailurePattern {
                id: "context_gap".to_string(),
                cause_type: "InsufficientContext".to_string(),
                frequency: 0,
                triggers: vec![
                    "vague query".to_string(),
                    "missing background".to_string(),
                    "ambiguous requirements".to_string(),
                ],
                mitigations: vec![
                    "ask clarifying questions".to_string(),
                    "search for context".to_string(),
                    "make assumptions explicit".to_string(),
                ],
                associated_actions: vec!["search".to_string(), "query".to_string()],
            },
        );

        patterns.insert(
            "tool_error".to_string(),
            FailurePattern {
                id: "tool_error".to_string(),
                cause_type: "ToolFailure".to_string(),
                frequency: 0,
                triggers: vec![
                    "invalid arguments".to_string(),
                    "permission denied".to_string(),
                    "resource not found".to_string(),
                ],
                mitigations: vec![
                    "validate inputs".to_string(),
                    "check permissions".to_string(),
                    "try alternative tool".to_string(),
                ],
                associated_actions: vec!["execute".to_string(), "run".to_string()],
            },
        );

        patterns.insert(
            "reasoning_flaw".to_string(),
            FailurePattern {
                id: "reasoning_flaw".to_string(),
                cause_type: "ReasoningError".to_string(),
                frequency: 0,
                triggers: vec![
                    "complex logic".to_string(),
                    "multiple constraints".to_string(),
                    "time pressure".to_string(),
                ],
                mitigations: vec![
                    "break down problem".to_string(),
                    "verify each step".to_string(),
                    "use structured approach".to_string(),
                ],
                associated_actions: vec!["analyze".to_string(), "reason".to_string()],
            },
        );

        patterns
    }

    /// Initialize recovery strategies
    fn initialize_strategies() -> Vec<RecoveryStrategy> {
        vec![
            RecoveryStrategy {
                name: "Clarification Loop".to_string(),
                description: "Ask clarifying questions to gather missing context".to_string(),
                applicable_causes: vec![
                    "InsufficientContext".to_string(),
                    "InvalidInput".to_string(),
                ],
                success_rate: 0.75,
                actions: vec![
                    "Identify what information is missing".to_string(),
                    "Formulate specific questions".to_string(),
                    "Request clarification".to_string(),
                    "Retry with new context".to_string(),
                ],
                effort_score: 3,
            },
            RecoveryStrategy {
                name: "Decomposition".to_string(),
                description: "Break the problem into smaller, manageable parts".to_string(),
                applicable_causes: vec![
                    "ReasoningError".to_string(),
                    "CapabilityLimit".to_string(),
                ],
                success_rate: 0.70,
                actions: vec![
                    "Identify sub-problems".to_string(),
                    "Solve each independently".to_string(),
                    "Integrate solutions".to_string(),
                    "Verify combined result".to_string(),
                ],
                effort_score: 5,
            },
            RecoveryStrategy {
                name: "Alternative Approach".to_string(),
                description: "Try a different method or tool to achieve the goal".to_string(),
                applicable_causes: vec!["ToolFailure".to_string(), "ExternalFailure".to_string()],
                success_rate: 0.65,
                actions: vec![
                    "Identify alternative methods".to_string(),
                    "Evaluate feasibility".to_string(),
                    "Implement alternative".to_string(),
                    "Compare results".to_string(),
                ],
                effort_score: 4,
            },
            RecoveryStrategy {
                name: "Self-Verification".to_string(),
                description: "Verify reasoning and outputs before finalizing".to_string(),
                applicable_causes: vec!["ReasoningError".to_string()],
                success_rate: 0.80,
                actions: vec![
                    "Review each reasoning step".to_string(),
                    "Check for logical consistency".to_string(),
                    "Verify facts if possible".to_string(),
                    "Correct identified errors".to_string(),
                ],
                effort_score: 2,
            },
        ]
    }

    /// Analyze a trajectory verdict
    pub fn analyze(&self, trajectory: &Trajectory) -> VerdictAnalysis {
        let mut analysis = VerdictAnalysis::new(trajectory.verdict.clone());

        // Extract root cause from verdict
        if let Verdict::Failure(ref cause) = trajectory.verdict {
            analysis.root_cause = Some(cause.clone());
        } else if let Verdict::RecoveredViaReflection {
            ref original_failure,
            ..
        } = trajectory.verdict
        {
            analysis.root_cause = Some((**original_failure).clone());
        }

        // Analyze contributing factors
        analysis.contributing_factors = self.extract_contributing_factors(trajectory);

        // Match failure patterns
        analysis.failure_patterns = self.match_failure_patterns(trajectory);

        // Suggest recovery strategies
        if let Some(ref cause) = analysis.root_cause {
            analysis.recovery_strategies = self.suggest_strategies(cause);
        }

        // Extract lessons
        analysis.lessons = self.extract_lessons(trajectory);

        // Determine pattern category
        analysis.pattern_category = self.determine_category(trajectory);

        // Compute confidence
        analysis.confidence = self.compute_confidence(&analysis);

        // Generate improvements
        analysis.improvements = self.suggest_improvements(trajectory, &analysis);

        analysis
    }

    /// Extract contributing factors from trajectory
    fn extract_contributing_factors(&self, trajectory: &Trajectory) -> Vec<String> {
        let mut factors = Vec::new();

        // Check step outcomes
        let failure_count = trajectory
            .steps
            .iter()
            .filter(|s| s.outcome.is_failure())
            .count();

        if failure_count > 0 {
            factors.push(format!("{} steps failed", failure_count));
        }

        // Check confidence levels
        let low_confidence = trajectory
            .steps
            .iter()
            .filter(|s| s.confidence < 0.5)
            .count();

        if low_confidence > 0 {
            factors.push(format!("{} steps had low confidence", low_confidence));
        }

        // Check for specific issues
        for step in &trajectory.steps {
            if let StepOutcome::NeedsRetry { reason, .. } = &step.outcome {
                factors.push(format!("Step '{}' needed retry: {}", step.action, reason));
            }
        }

        // Check latency
        if trajectory.total_latency_ms > 30000 {
            factors.push("Long execution time".to_string());
        }

        factors
    }

    /// Match known failure patterns
    fn match_failure_patterns(&self, trajectory: &Trajectory) -> Vec<FailurePattern> {
        let mut matched = Vec::new();

        for step in &trajectory.steps {
            if step.outcome.is_failure() {
                // Check each known pattern
                for (_, pattern) in &self.known_patterns {
                    if pattern
                        .associated_actions
                        .iter()
                        .any(|a| step.action.contains(a))
                    {
                        matched.push(pattern.clone());
                    }
                }
            }
        }

        // Deduplicate
        let mut unique: Vec<FailurePattern> = Vec::new();
        for p in matched {
            if !unique.iter().any(|u| u.id == p.id) {
                unique.push(p);
            }
        }

        unique
    }

    /// Suggest recovery strategies based on root cause
    fn suggest_strategies(&self, cause: &RootCause) -> Vec<RecoveryStrategy> {
        let cause_type = match cause {
            RootCause::InsufficientContext { .. } => "InsufficientContext",
            RootCause::InvalidInput { .. } => "InvalidInput",
            RootCause::ToolFailure { .. } => "ToolFailure",
            RootCause::ReasoningError { .. } => "ReasoningError",
            RootCause::ResourceConstraint { .. } => "ResourceConstraint",
            RootCause::ExternalFailure { .. } => "ExternalFailure",
            RootCause::CapabilityLimit { .. } => "CapabilityLimit",
            RootCause::Unknown { .. } => "Unknown",
        };

        self.recovery_strategies
            .iter()
            .filter(|s| s.applicable_causes.iter().any(|c| c == cause_type))
            .cloned()
            .collect()
    }

    /// Extract lessons from trajectory
    fn extract_lessons(&self, trajectory: &Trajectory) -> Vec<String> {
        let mut lessons = trajectory.lessons.clone();

        // Add automatic lessons based on analysis
        if trajectory.is_success() {
            // Learn from success
            let successful_actions: Vec<_> = trajectory
                .steps
                .iter()
                .filter(|s| s.outcome.is_success())
                .map(|s| &s.action)
                .collect();

            if !successful_actions.is_empty() {
                lessons.push(format!(
                    "Successful pattern: {}",
                    successful_actions
                        .iter()
                        .take(3)
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                ));
            }
        } else {
            // Learn from failure
            for step in &trajectory.steps {
                if let StepOutcome::Failure { error } = &step.outcome {
                    lessons.push(format!("Avoid: {} - {}", step.action, error));
                }
            }
        }

        // Add recovery lessons
        if let Verdict::RecoveredViaReflection {
            reflection_attempts,
            ..
        } = &trajectory.verdict
        {
            lessons.push(format!(
                "Recovery possible with {} reflection attempts",
                reflection_attempts
            ));
        }

        lessons
    }

    /// Determine pattern category from trajectory
    fn determine_category(&self, trajectory: &Trajectory) -> PatternCategory {
        // Check verdict first
        match &trajectory.verdict {
            Verdict::RecoveredViaReflection { .. } => return PatternCategory::Reflection,
            Verdict::Failure(_) => return PatternCategory::ErrorRecovery,
            _ => {}
        }

        // Check actions
        let actions: Vec<_> = trajectory.steps.iter().map(|s| s.action.as_str()).collect();

        if actions
            .iter()
            .any(|a| a.contains("code") || a.contains("implement"))
        {
            return PatternCategory::CodeGeneration;
        }
        if actions
            .iter()
            .any(|a| a.contains("search") || a.contains("research"))
        {
            return PatternCategory::Research;
        }
        if actions
            .iter()
            .any(|a| a.contains("tool") || a.contains("execute"))
        {
            return PatternCategory::ToolUse;
        }

        PatternCategory::General
    }

    /// Compute confidence in analysis
    fn compute_confidence(&self, analysis: &VerdictAnalysis) -> f32 {
        let mut confidence = 0.5;

        // More data points = higher confidence
        confidence += 0.1 * (analysis.contributing_factors.len() as f32).min(2.0);
        confidence += 0.1 * (analysis.failure_patterns.len() as f32).min(2.0);
        confidence += 0.1 * (analysis.lessons.len() as f32).min(3.0);

        // Having recovery strategies increases confidence
        if !analysis.recovery_strategies.is_empty() {
            confidence += 0.1;
        }

        confidence.min(1.0)
    }

    /// Suggest improvements
    fn suggest_improvements(
        &self,
        trajectory: &Trajectory,
        _analysis: &VerdictAnalysis,
    ) -> Vec<String> {
        let mut improvements = Vec::new();

        // Check for low confidence steps
        let low_confidence_steps: Vec<_> = trajectory
            .steps
            .iter()
            .filter(|s| s.confidence < 0.6)
            .collect();

        if !low_confidence_steps.is_empty() {
            improvements.push(format!(
                "Improve confidence in {} steps: {}",
                low_confidence_steps.len(),
                low_confidence_steps
                    .iter()
                    .take(3)
                    .map(|s| s.action.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Check for missing verification
        let has_verification = trajectory
            .steps
            .iter()
            .any(|s| s.action.contains("verify") || s.action.contains("check"));

        if !has_verification && trajectory.steps.len() > 2 {
            improvements.push("Consider adding verification steps".to_string());
        }

        // Check for error handling
        let has_error_handling = trajectory
            .steps
            .iter()
            .any(|s| matches!(s.outcome, StepOutcome::NeedsRetry { .. }));

        if !has_error_handling && trajectory.is_failure() {
            improvements
                .push("Consider implementing retry logic for recoverable errors".to_string());
        }

        improvements
    }

    /// Get analysis statistics
    pub fn stats(&self) -> VerdictAnalyzerStats {
        VerdictAnalyzerStats {
            known_patterns: self.known_patterns.len(),
            recovery_strategies: self.recovery_strategies.len(),
            analyses_performed: self.analysis_count,
        }
    }

    /// Learn from a new failure pattern
    pub fn learn_pattern(&mut self, pattern: FailurePattern) {
        self.known_patterns.insert(pattern.id.clone(), pattern);
    }

    /// Add a new recovery strategy
    pub fn add_strategy(&mut self, strategy: RecoveryStrategy) {
        self.recovery_strategies.push(strategy);
    }
}

impl Default for VerdictAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for the verdict analyzer
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerdictAnalyzerStats {
    /// Number of known patterns
    pub known_patterns: usize,
    /// Number of recovery strategies
    pub recovery_strategies: usize,
    /// Total analyses performed
    pub analyses_performed: u64,
}

#[cfg(test)]
mod tests {
    use super::super::trajectory::{StepOutcome, TrajectoryRecorder};
    use super::*;

    #[test]
    fn test_verdict_creation() {
        assert!(Verdict::success().is_success());
        assert!(Verdict::failure(RootCause::Unknown {
            details: "test".into()
        })
        .is_failure());
        assert!(!Verdict::partial(0.5).is_success());
    }

    #[test]
    fn test_verdict_quality_score() {
        assert_eq!(Verdict::success().quality_score(), 1.0);
        assert_eq!(
            Verdict::failure(RootCause::Unknown { details: "".into() }).quality_score(),
            0.0
        );
        assert!(Verdict::partial(0.5).quality_score() > 0.0);
    }

    #[test]
    fn test_root_cause_display() {
        let cause = RootCause::ToolFailure {
            tool: "git".to_string(),
            error: "not found".to_string(),
        };
        assert!(cause.to_string().contains("git"));
        assert!(cause.to_string().contains("not found"));
    }

    #[test]
    fn test_verdict_analyzer_creation() {
        let analyzer = VerdictAnalyzer::new();
        let stats = analyzer.stats();
        assert!(stats.known_patterns > 0);
        assert!(stats.recovery_strategies > 0);
    }

    #[test]
    fn test_verdict_analysis() {
        let analyzer = VerdictAnalyzer::new();

        let mut recorder = TrajectoryRecorder::new(vec![0.1; 768]);
        recorder.add_step(
            "analyze".to_string(),
            "analyzing".to_string(),
            StepOutcome::Success,
            0.9,
        );
        recorder.add_step(
            "execute".to_string(),
            "executing".to_string(),
            StepOutcome::Failure {
                error: "permission denied".to_string(),
            },
            0.6,
        );

        let trajectory = recorder.complete(Verdict::failure(RootCause::ToolFailure {
            tool: "shell".to_string(),
            error: "permission denied".to_string(),
        }));

        let analysis = analyzer.analyze(&trajectory);

        assert!(analysis.root_cause.is_some());
        assert!(!analysis.contributing_factors.is_empty());
        assert!(!analysis.recovery_strategies.is_empty());
    }

    #[test]
    fn test_recovered_verdict() {
        let verdict = Verdict::recovered(
            RootCause::ReasoningError {
                error_type: ReasoningErrorType::LogicalInconsistency,
                description: "test".to_string(),
            },
            2,
            0.85,
        );

        assert!(verdict.is_success());
        assert!(verdict.quality_score() > 0.8);
    }

    #[test]
    fn test_failure_pattern() {
        let pattern = FailurePattern {
            id: "test".to_string(),
            cause_type: "ToolFailure".to_string(),
            frequency: 5,
            triggers: vec!["trigger1".to_string()],
            mitigations: vec!["fix1".to_string()],
            associated_actions: vec!["execute".to_string()],
        };

        assert_eq!(pattern.frequency, 5);
    }

    #[test]
    fn test_recovery_strategy() {
        let strategy = RecoveryStrategy {
            name: "Test Strategy".to_string(),
            description: "A test".to_string(),
            applicable_causes: vec!["ToolFailure".to_string()],
            success_rate: 0.7,
            actions: vec!["action1".to_string()],
            effort_score: 3,
        };

        assert_eq!(strategy.effort_score, 3);
    }
}
