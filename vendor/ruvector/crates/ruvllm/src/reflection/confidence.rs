//! Confidence-Based Revision (If-or-Else Pattern)
//!
//! Implements the If-or-Else (IoE) pattern where revision is only triggered
//! when confidence is LOW. This is more efficient than always reflecting,
//! as high-confidence outputs are accepted immediately.
//!
//! ## Key Insight
//!
//! The IoE pattern recognizes that:
//! - Most outputs are acceptable and don't need revision
//! - Only LOW confidence outputs benefit from reflection
//! - Targeted revision based on weak points is more effective than generic retry
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +----------------------+
//! | ConfidenceChecker |---->| should_revise()      |
//! | - threshold       |     | - Check confidence   |
//! | - budget          |     | - Compare threshold  |
//! +-------------------+     +----------------------+
//!           |
//!           v (if LOW)
//! +-------------------+     +----------------------+
//! | identify_weak_pts |---->| generate_targeted_   |
//! | - Parse output    |     | revision()           |
//! | - Find issues     |     | - Focus on weak pts  |
//! +-------------------+     +----------------------+
//! ```

use super::reflective_agent::ExecutionContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for confidence checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceConfig {
    /// Threshold below which revision is triggered
    pub threshold: f32,
    /// Maximum revision attempts (budget)
    pub revision_budget: u32,
    /// Minimum improvement required to continue revising
    pub min_improvement: f32,
    /// Weights for different confidence factors
    pub factor_weights: ConfidenceFactorWeights,
    /// Whether to use structural analysis
    pub use_structural_analysis: bool,
    /// Patterns that indicate low confidence
    pub low_confidence_patterns: Vec<String>,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            threshold: 0.7,
            revision_budget: 3,
            min_improvement: 0.05,
            factor_weights: ConfidenceFactorWeights::default(),
            use_structural_analysis: true,
            low_confidence_patterns: vec![
                "I'm not sure".to_string(),
                "might be".to_string(),
                "possibly".to_string(),
                "could be wrong".to_string(),
                "uncertain".to_string(),
                "TODO".to_string(),
                "FIXME".to_string(),
                "not implemented".to_string(),
            ],
        }
    }
}

/// Weights for confidence factors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceFactorWeights {
    /// Weight for output completeness
    pub completeness: f32,
    /// Weight for output structure
    pub structure: f32,
    /// Weight for absence of uncertainty markers
    pub certainty: f32,
    /// Weight for task relevance
    pub relevance: f32,
    /// Weight for code validity (if applicable)
    pub code_validity: f32,
}

impl Default for ConfidenceFactorWeights {
    fn default() -> Self {
        Self {
            completeness: 0.25,
            structure: 0.20,
            certainty: 0.20,
            relevance: 0.20,
            code_validity: 0.15,
        }
    }
}

/// Confidence level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// Very high confidence (>0.9)
    VeryHigh,
    /// High confidence (0.7-0.9)
    High,
    /// Medium confidence (0.5-0.7)
    Medium,
    /// Low confidence (0.3-0.5)
    Low,
    /// Very low confidence (<0.3)
    VeryLow,
}

impl ConfidenceLevel {
    /// Create from score
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s > 0.9 => Self::VeryHigh,
            s if s > 0.7 => Self::High,
            s if s > 0.5 => Self::Medium,
            s if s > 0.3 => Self::Low,
            _ => Self::VeryLow,
        }
    }

    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VeryHigh => "very_high",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::VeryLow => "very_low",
        }
    }

    /// Check if revision is recommended
    pub fn should_revise(&self) -> bool {
        matches!(self, Self::Low | Self::VeryLow)
    }
}

/// A weak point identified in the output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeakPoint {
    /// Location in output (line number or description)
    pub location: String,
    /// Description of the weakness
    pub description: String,
    /// Severity (0.0-1.0)
    pub severity: f32,
    /// Type of weakness
    pub weakness_type: WeaknessType,
    /// Suggested fix
    pub suggestion: String,
    /// Confidence in this identification
    pub confidence: f32,
}

impl WeakPoint {
    /// Create a new weak point
    pub fn new(
        location: impl Into<String>,
        description: impl Into<String>,
        severity: f32,
        weakness_type: WeaknessType,
    ) -> Self {
        Self {
            location: location.into(),
            description: description.into(),
            severity: severity.clamp(0.0, 1.0),
            weakness_type,
            suggestion: String::new(),
            confidence: 0.8,
        }
    }

    /// Add suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = suggestion.into();
        self
    }
}

/// Types of weaknesses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaknessType {
    /// Incomplete implementation
    Incomplete,
    /// Uncertain/hedge words
    Uncertainty,
    /// Missing error handling
    MissingErrorHandling,
    /// Missing validation
    MissingValidation,
    /// Code smell or anti-pattern
    CodeSmell,
    /// Missing tests
    MissingTests,
    /// Documentation gap
    DocumentationGap,
    /// Security concern
    SecurityConcern,
    /// Performance issue
    PerformanceIssue,
    /// Logic error
    LogicError,
    /// Other
    Other,
}

/// Result of revision attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionResult {
    /// Original confidence
    pub original_confidence: f32,
    /// New confidence after revision
    pub new_confidence: f32,
    /// Improvement achieved
    pub improvement: f32,
    /// Weak points addressed
    pub addressed_weak_points: Vec<WeakPoint>,
    /// Remaining weak points
    pub remaining_weak_points: Vec<WeakPoint>,
    /// Revision count
    pub revision_count: u32,
    /// Whether revision was successful
    pub successful: bool,
}

/// Confidence checker for IoE pattern
#[derive(Debug)]
pub struct ConfidenceChecker {
    /// Configuration
    config: ConfidenceConfig,
    /// History of confidence checks
    check_history: Vec<ConfidenceCheckRecord>,
    /// Learned patterns that indicate low confidence
    learned_patterns: HashMap<String, f32>,
}

/// Record of a confidence check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceCheckRecord {
    /// Computed confidence score
    pub score: f32,
    /// Confidence level
    pub level: ConfidenceLevel,
    /// Weak points found
    pub weak_points: Vec<WeakPoint>,
    /// Factors contributing to score
    pub factors: HashMap<String, f32>,
    /// Task context
    pub task_summary: String,
    /// Timestamp
    pub timestamp: u64,
}

impl ConfidenceChecker {
    /// Create a new confidence checker
    pub fn new(config: ConfidenceConfig) -> Self {
        Self {
            config,
            check_history: Vec::new(),
            learned_patterns: HashMap::new(),
        }
    }

    /// Check if revision is needed based on confidence
    pub fn should_revise(&self, output: &str, context: &ExecutionContext) -> bool {
        let confidence = self.compute_confidence(output, context);
        let attempts = context.previous_attempts.len() as u32;

        // Only revise when:
        // 1. Confidence is below threshold
        // 2. We haven't exceeded the revision budget
        confidence < self.config.threshold && attempts < self.config.revision_budget
    }

    /// Compute confidence score for an output
    pub fn compute_confidence(&self, output: &str, context: &ExecutionContext) -> f32 {
        let weights = &self.config.factor_weights;
        let mut score = 0.0f32;

        // Factor 1: Completeness
        let completeness = self.assess_completeness(output, context);
        score += completeness * weights.completeness;

        // Factor 2: Structure
        let structure = self.assess_structure(output);
        score += structure * weights.structure;

        // Factor 3: Certainty (absence of uncertainty markers)
        let certainty = self.assess_certainty(output);
        score += certainty * weights.certainty;

        // Factor 4: Relevance to task
        let relevance = self.assess_relevance(output, context);
        score += relevance * weights.relevance;

        // Factor 5: Code validity (if applicable)
        let code_validity = self.assess_code_validity(output);
        score += code_validity * weights.code_validity;

        // Apply learned pattern adjustments
        for (pattern, weight) in &self.learned_patterns {
            if output.to_lowercase().contains(&pattern.to_lowercase()) {
                score *= 1.0 - weight; // Reduce confidence for negative patterns
            }
        }

        score.clamp(0.0, 1.0)
    }

    /// Assess output completeness
    fn assess_completeness(&self, output: &str, context: &ExecutionContext) -> f32 {
        if output.is_empty() {
            return 0.0;
        }

        let mut score = 0.5f32; // Base score

        // Check if output addresses the task
        let task_words: Vec<&str> = context.task.split_whitespace().collect();
        let output_lower = output.to_lowercase();
        let addressed_count = task_words
            .iter()
            .filter(|w| output_lower.contains(&w.to_lowercase()))
            .count();
        let addressed_ratio = addressed_count as f32 / task_words.len().max(1) as f32;
        score += addressed_ratio * 0.3;

        // Check for incomplete markers
        let incomplete_markers = ["TODO", "FIXME", "...", "to be continued", "incomplete"];
        let has_incomplete = incomplete_markers.iter().any(|m| output.contains(m));
        if has_incomplete {
            score -= 0.2;
        }

        // Bonus for substantial output
        if output.len() > 500 {
            score += 0.1;
        }
        if output.len() > 1000 {
            score += 0.1;
        }

        score.clamp(0.0, 1.0)
    }

    /// Assess output structure
    fn assess_structure(&self, output: &str) -> f32 {
        if !self.config.use_structural_analysis {
            return 0.8; // Default to high if disabled
        }

        let mut score = 0.5f32;

        // Check for code blocks
        let has_code_blocks = output.contains("```");
        if has_code_blocks {
            score += 0.2;
        }

        // Check for sections/headers
        let has_headers = output.contains("##") || output.contains("**");
        if has_headers {
            score += 0.1;
        }

        // Check for lists
        let has_lists =
            output.contains("\n- ") || output.contains("\n* ") || output.contains("\n1.");
        if has_lists {
            score += 0.1;
        }

        // Penalize very short outputs
        if output.len() < 50 {
            score -= 0.2;
        }

        // Check line count for multi-line responses
        let line_count = output.lines().count();
        if line_count > 5 {
            score += 0.1;
        }

        score.clamp(0.0, 1.0)
    }

    /// Assess certainty (absence of uncertainty markers)
    fn assess_certainty(&self, output: &str) -> f32 {
        let output_lower = output.to_lowercase();
        let mut uncertainty_count = 0;

        for pattern in &self.config.low_confidence_patterns {
            if output_lower.contains(&pattern.to_lowercase()) {
                uncertainty_count += 1;
            }
        }

        // More uncertainty markers = lower confidence
        match uncertainty_count {
            0 => 1.0,
            1 => 0.8,
            2 => 0.6,
            3 => 0.4,
            _ => 0.2,
        }
    }

    /// Assess relevance to task
    fn assess_relevance(&self, output: &str, context: &ExecutionContext) -> f32 {
        let task_lower = context.task.to_lowercase();
        let output_lower = output.to_lowercase();

        // Extract key terms from task
        let key_terms: Vec<&str> = task_lower
            .split_whitespace()
            .filter(|w| w.len() > 3) // Skip short words
            .collect();

        if key_terms.is_empty() {
            return 0.5;
        }

        let matched = key_terms
            .iter()
            .filter(|term| output_lower.contains(*term))
            .count();

        let ratio = matched as f32 / key_terms.len() as f32;
        (ratio * 0.5 + 0.5).clamp(0.0, 1.0) // Scale to 0.5-1.0 range
    }

    /// Assess code validity (basic heuristics)
    fn assess_code_validity(&self, output: &str) -> f32 {
        // Check if output contains code
        let has_code = output.contains("```")
            || output.contains("fn ")
            || output.contains("def ")
            || output.contains("function ")
            || output.contains("class ");

        if !has_code {
            return 0.8; // Not code-related, give neutral score
        }

        let mut score = 0.7f32;

        // Check for balanced brackets
        let open_parens = output.matches('(').count();
        let close_parens = output.matches(')').count();
        let open_braces = output.matches('{').count();
        let close_braces = output.matches('}').count();
        let open_brackets = output.matches('[').count();
        let close_brackets = output.matches(']').count();

        if open_parens == close_parens {
            score += 0.1;
        } else {
            score -= 0.2;
        }

        if open_braces == close_braces {
            score += 0.1;
        } else {
            score -= 0.2;
        }

        if open_brackets == close_brackets {
            score += 0.1;
        } else {
            score -= 0.1;
        }

        // Check for common error patterns
        if output.contains("error[") || output.contains("Error:") {
            score -= 0.3;
        }

        score.clamp(0.0, 1.0)
    }

    /// Identify weak points in the output
    pub fn identify_weak_points(&self, output: &str, context: &ExecutionContext) -> Vec<WeakPoint> {
        let mut weak_points = Vec::new();

        // Check for uncertainty markers
        for pattern in &self.config.low_confidence_patterns {
            if let Some(pos) = output.to_lowercase().find(&pattern.to_lowercase()) {
                let line_num = output[..pos].matches('\n').count() + 1;
                weak_points.push(
                    WeakPoint::new(
                        format!("line {}", line_num),
                        format!("Uncertainty marker: '{}'", pattern),
                        0.6,
                        WeaknessType::Uncertainty,
                    )
                    .with_suggestion(format!(
                        "Remove or clarify the uncertain statement at '{}'",
                        pattern
                    )),
                );
            }
        }

        // Check for TODO/FIXME
        for marker in ["TODO", "FIXME", "XXX", "HACK"] {
            if output.contains(marker) {
                let count = output.matches(marker).count();
                weak_points.push(
                    WeakPoint::new(
                        "multiple locations",
                        format!("Found {} {} markers", count, marker),
                        0.7,
                        WeaknessType::Incomplete,
                    )
                    .with_suggestion(format!("Address all {} items", marker)),
                );
            }
        }

        // Check for missing error handling in code
        if output.contains("fn ") || output.contains("async fn ") {
            if !output.contains("Result<") && !output.contains("Option<") && !output.contains("?") {
                weak_points.push(
                    WeakPoint::new(
                        "function definitions",
                        "Functions may lack proper error handling",
                        0.5,
                        WeaknessType::MissingErrorHandling,
                    )
                    .with_suggestion("Add Result/Option return types and error propagation"),
                );
            }
        }

        // Check for missing validation
        if context.task.to_lowercase().contains("input")
            || context.task.to_lowercase().contains("parameter")
        {
            if !output.to_lowercase().contains("valid")
                && !output.to_lowercase().contains("check")
                && !output.to_lowercase().contains("assert")
            {
                weak_points.push(
                    WeakPoint::new(
                        "input handling",
                        "May be missing input validation",
                        0.4,
                        WeaknessType::MissingValidation,
                    )
                    .with_suggestion("Add input validation and bounds checking"),
                );
            }
        }

        // Check for missing tests if task mentions testing
        if context.task.to_lowercase().contains("test") {
            if !output.contains("#[test]") && !output.contains("fn test_") {
                weak_points.push(
                    WeakPoint::new(
                        "test coverage",
                        "No test functions found",
                        0.6,
                        WeaknessType::MissingTests,
                    )
                    .with_suggestion("Add unit tests with #[test] attribute"),
                );
            }
        }

        weak_points
    }

    /// Generate a targeted revision based on weak points
    pub fn generate_targeted_revision(&self, output: &str, weak_points: &[WeakPoint]) -> String {
        if weak_points.is_empty() {
            return output.to_string();
        }

        let mut revision_prompt = String::from(
            "Please revise the following output to address these specific issues:\n\n",
        );

        for (i, wp) in weak_points.iter().enumerate() {
            revision_prompt.push_str(&format!(
                "{}. [{:?}] At {}: {}\n   Suggestion: {}\n\n",
                i + 1,
                wp.weakness_type,
                wp.location,
                wp.description,
                wp.suggestion
            ));
        }

        revision_prompt.push_str("\nOriginal output:\n");
        revision_prompt.push_str(output);

        revision_prompt
    }

    /// Record a confidence check for learning
    pub fn record_check(
        &mut self,
        output: &str,
        context: &ExecutionContext,
    ) -> ConfidenceCheckRecord {
        let score = self.compute_confidence(output, context);
        let level = ConfidenceLevel::from_score(score);
        let weak_points = self.identify_weak_points(output, context);

        let mut factors = HashMap::new();
        factors.insert(
            "completeness".to_string(),
            self.assess_completeness(output, context),
        );
        factors.insert("structure".to_string(), self.assess_structure(output));
        factors.insert("certainty".to_string(), self.assess_certainty(output));
        factors.insert(
            "relevance".to_string(),
            self.assess_relevance(output, context),
        );
        factors.insert(
            "code_validity".to_string(),
            self.assess_code_validity(output),
        );

        let record = ConfidenceCheckRecord {
            score,
            level,
            weak_points,
            factors,
            task_summary: context.task.chars().take(100).collect(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };

        self.check_history.push(record.clone());
        record
    }

    /// Learn from a pattern that indicated low quality
    pub fn learn_pattern(&mut self, pattern: String, weight: f32) {
        self.learned_patterns
            .insert(pattern, weight.clamp(0.0, 1.0));
    }

    /// Get check history
    pub fn history(&self) -> &[ConfidenceCheckRecord] {
        &self.check_history
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.check_history.clear();
    }

    /// Get configuration
    pub fn config(&self) -> &ConfidenceConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude_flow::AgentType;

    #[test]
    fn test_confidence_level_from_score() {
        assert_eq!(ConfidenceLevel::from_score(0.95), ConfidenceLevel::VeryHigh);
        assert_eq!(ConfidenceLevel::from_score(0.8), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(0.6), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.4), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_score(0.2), ConfidenceLevel::VeryLow);
    }

    #[test]
    fn test_should_revise_low_levels() {
        assert!(ConfidenceLevel::Low.should_revise());
        assert!(ConfidenceLevel::VeryLow.should_revise());
        assert!(!ConfidenceLevel::Medium.should_revise());
        assert!(!ConfidenceLevel::High.should_revise());
    }

    #[test]
    fn test_confidence_checker_creation() {
        let config = ConfidenceConfig::default();
        let checker = ConfidenceChecker::new(config);
        assert_eq!(checker.config().threshold, 0.7);
    }

    #[test]
    fn test_compute_confidence_empty() {
        let checker = ConfidenceChecker::new(ConfidenceConfig::default());
        let context = ExecutionContext::new("test task", AgentType::Coder, "input");
        let confidence = checker.compute_confidence("", &context);
        assert!(confidence < 0.5);
    }

    #[test]
    fn test_compute_confidence_with_uncertainty() {
        let checker = ConfidenceChecker::new(ConfidenceConfig::default());
        let context = ExecutionContext::new("implement function", AgentType::Coder, "input");

        let confident_output = "Here is the implementation:\n```rust\nfn example() { }\n```";
        let uncertain_output = "I'm not sure but possibly this might work...";

        let conf1 = checker.compute_confidence(confident_output, &context);
        let conf2 = checker.compute_confidence(uncertain_output, &context);

        assert!(conf1 > conf2);
    }

    #[test]
    fn test_identify_weak_points_todo() {
        let checker = ConfidenceChecker::new(ConfidenceConfig::default());
        let context = ExecutionContext::new("implement function", AgentType::Coder, "input");
        let output = "fn example() {\n    // TODO: implement this\n}";

        let weak_points = checker.identify_weak_points(output, &context);
        assert!(!weak_points.is_empty());
        assert!(weak_points
            .iter()
            .any(|wp| matches!(wp.weakness_type, WeaknessType::Incomplete)));
    }

    #[test]
    fn test_should_revise() {
        let checker = ConfidenceChecker::new(ConfidenceConfig {
            threshold: 0.7,
            revision_budget: 3,
            ..Default::default()
        });

        let mut context = ExecutionContext::new("test", AgentType::Coder, "input");

        // Low confidence output should trigger revision
        let low_conf_output = "I'm not sure, maybe...";
        assert!(checker.should_revise(low_conf_output, &context));

        // After exceeding budget, should not revise
        for _ in 0..3 {
            context
                .previous_attempts
                .push(crate::reflection::reflective_agent::PreviousAttempt {
                    attempt_number: 1,
                    output: String::new(),
                    error: None,
                    quality_score: None,
                    duration_ms: 0,
                    reflection: None,
                });
        }
        assert!(!checker.should_revise(low_conf_output, &context));
    }

    #[test]
    fn test_weak_point_builder() {
        let wp = WeakPoint::new(
            "line 5",
            "Missing error handling",
            0.7,
            WeaknessType::MissingErrorHandling,
        )
        .with_suggestion("Add Result return type");

        assert_eq!(wp.location, "line 5");
        assert!(!wp.suggestion.is_empty());
    }

    #[test]
    fn test_generate_targeted_revision() {
        let checker = ConfidenceChecker::new(ConfidenceConfig::default());
        let weak_points = vec![
            WeakPoint::new("line 1", "Issue 1", 0.5, WeaknessType::Incomplete)
                .with_suggestion("Fix it"),
        ];

        let revision = checker.generate_targeted_revision("original output", &weak_points);
        assert!(revision.contains("Issue 1"));
        assert!(revision.contains("Fix it"));
        assert!(revision.contains("original output"));
    }

    #[test]
    fn test_learn_pattern() {
        let mut checker = ConfidenceChecker::new(ConfidenceConfig::default());
        checker.learn_pattern("problematic pattern".to_string(), 0.3);

        let context = ExecutionContext::new("test", AgentType::Coder, "input");
        let output_with_pattern = "This has a problematic pattern in it";
        let output_without = "This is clean code";

        let conf1 = checker.compute_confidence(output_with_pattern, &context);
        let conf2 = checker.compute_confidence(output_without, &context);

        assert!(conf1 < conf2);
    }

    #[test]
    fn test_record_check() {
        let mut checker = ConfidenceChecker::new(ConfidenceConfig::default());
        let context = ExecutionContext::new("test task", AgentType::Coder, "input");

        let record = checker.record_check("test output", &context);

        assert!(!checker.history().is_empty());
        assert!(record.factors.contains_key("completeness"));
    }
}
