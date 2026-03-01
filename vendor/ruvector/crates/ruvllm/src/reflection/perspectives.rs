//! Multi-Perspective Critique System
//!
//! Implements a multi-perspective critique system that evaluates outputs from
//! different angles to provide comprehensive reflection. Each perspective focuses
//! on a specific quality dimension.
//!
//! ## Available Perspectives
//!
//! - **Correctness**: Verifies logical correctness and absence of errors
//! - **Completeness**: Checks if all requirements are addressed
//! - **Consistency**: Ensures internal consistency and follows conventions
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +----------------------+
//! | Perspective trait |<----| CorrectnessChecker   |
//! | - critique()      |     +----------------------+
//! | - name()          |<----| CompletenessChecker  |
//! +-------------------+     +----------------------+
//!                      <----| ConsistencyChecker   |
//!                           +----------------------+
//!           |
//!           v
//! +-------------------+     +----------------------+
//! | CritiqueResult    |---->| UnifiedCritique      |
//! | - passed          |     | - combine results    |
//! | - score           |     | - generate summary   |
//! | - issues          |     +----------------------+
//! +-------------------+
//! ```

use super::reflective_agent::ExecutionContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for perspectives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveConfig {
    /// Weight for this perspective in combined scoring
    pub weight: f32,
    /// Minimum score to pass
    pub pass_threshold: f32,
    /// Whether to provide detailed feedback
    pub detailed_feedback: bool,
    /// Custom checks to perform
    pub custom_checks: Vec<String>,
}

impl Default for PerspectiveConfig {
    fn default() -> Self {
        Self {
            weight: 1.0,
            pass_threshold: 0.6,
            detailed_feedback: true,
            custom_checks: Vec::new(),
        }
    }
}

/// Result of a critique from one perspective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CritiqueResult {
    /// Name of the perspective
    pub perspective_name: String,
    /// Whether the critique passed
    pub passed: bool,
    /// Score (0.0-1.0)
    pub score: f32,
    /// Summary of the critique
    pub summary: String,
    /// Specific issues found
    pub issues: Vec<CritiqueIssue>,
    /// Strengths identified
    pub strengths: Vec<String>,
    /// Time taken for critique (ms)
    pub critique_time_ms: u64,
}

impl CritiqueResult {
    /// Create a new passing critique result
    pub fn pass(perspective: impl Into<String>, score: f32, summary: impl Into<String>) -> Self {
        Self {
            perspective_name: perspective.into(),
            passed: true,
            score: score.clamp(0.0, 1.0),
            summary: summary.into(),
            issues: Vec::new(),
            strengths: Vec::new(),
            critique_time_ms: 0,
        }
    }

    /// Create a failing critique result
    pub fn fail(perspective: impl Into<String>, score: f32, summary: impl Into<String>) -> Self {
        Self {
            perspective_name: perspective.into(),
            passed: false,
            score: score.clamp(0.0, 1.0),
            summary: summary.into(),
            issues: Vec::new(),
            strengths: Vec::new(),
            critique_time_ms: 0,
        }
    }

    /// Add an issue
    pub fn with_issue(mut self, issue: CritiqueIssue) -> Self {
        self.issues.push(issue);
        self
    }

    /// Add a strength
    pub fn with_strength(mut self, strength: impl Into<String>) -> Self {
        self.strengths.push(strength.into());
        self
    }
}

/// A specific issue found during critique
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CritiqueIssue {
    /// Issue severity (0.0-1.0)
    pub severity: f32,
    /// Issue description
    pub description: String,
    /// Location (line number or section)
    pub location: Option<String>,
    /// Suggested fix
    pub suggestion: String,
    /// Category of issue
    pub category: IssueCategory,
}

impl CritiqueIssue {
    /// Create a new critique issue
    pub fn new(description: impl Into<String>, severity: f32, category: IssueCategory) -> Self {
        Self {
            severity: severity.clamp(0.0, 1.0),
            description: description.into(),
            location: None,
            suggestion: String::new(),
            category,
        }
    }

    /// Add location
    pub fn at(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Add suggestion
    pub fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = suggestion.into();
        self
    }
}

/// Categories of critique issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IssueCategory {
    /// Logical error
    Logic,
    /// Syntax or structural issue
    Syntax,
    /// Missing element
    Missing,
    /// Redundant element
    Redundant,
    /// Inconsistency
    Inconsistent,
    /// Style or convention violation
    Style,
    /// Security concern
    Security,
    /// Performance concern
    Performance,
    /// Documentation gap
    Documentation,
    /// Other
    Other,
}

/// Trait for perspective implementations
pub trait Perspective: Send + Sync {
    /// Get the perspective name
    fn name(&self) -> &str;

    /// Perform critique from this perspective
    fn critique(&self, output: &str, context: &ExecutionContext) -> CritiqueResult;

    /// Get the configuration
    fn config(&self) -> &PerspectiveConfig;
}

/// Correctness checker perspective
///
/// Verifies logical correctness, absence of errors, and proper functioning
pub struct CorrectnessChecker {
    config: PerspectiveConfig,
}

impl CorrectnessChecker {
    /// Create a new correctness checker
    pub fn new() -> Self {
        Self {
            config: PerspectiveConfig {
                weight: 1.2, // Higher weight for correctness
                pass_threshold: 0.7,
                detailed_feedback: true,
                custom_checks: Vec::new(),
            },
        }
    }

    /// Create with custom config
    pub fn with_config(config: PerspectiveConfig) -> Self {
        Self { config }
    }

    /// Check for error patterns in output
    fn check_for_errors(&self, output: &str) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();

        // Check for explicit error markers
        let error_patterns = [
            ("error[", "Compiler error present", IssueCategory::Syntax),
            ("Error:", "Runtime error present", IssueCategory::Logic),
            ("panic!", "Panic in code", IssueCategory::Logic),
            (
                "unwrap()",
                "Potential panic from unwrap",
                IssueCategory::Logic,
            ),
            (
                "expect()",
                "Potential panic from expect",
                IssueCategory::Logic,
            ),
            ("todo!()", "Unimplemented todo", IssueCategory::Missing),
            (
                "unimplemented!()",
                "Unimplemented code",
                IssueCategory::Missing,
            ),
            (
                "unreachable!()",
                "Unreachable code marker",
                IssueCategory::Logic,
            ),
        ];

        for (pattern, description, category) in error_patterns {
            if output.contains(pattern) {
                let count = output.matches(pattern).count();
                issues.push(
                    CritiqueIssue::new(
                        format!("{} ({} occurrence(s))", description, count),
                        if category == IssueCategory::Logic {
                            0.8
                        } else {
                            0.5
                        },
                        category,
                    )
                    .suggest(format!("Address or remove {}", pattern)),
                );
            }
        }

        // Check for unbalanced brackets (potential syntax errors)
        let open_parens = output.matches('(').count();
        let close_parens = output.matches(')').count();
        if open_parens != close_parens {
            issues.push(
                CritiqueIssue::new(
                    format!(
                        "Unbalanced parentheses: {} open, {} close",
                        open_parens, close_parens
                    ),
                    0.7,
                    IssueCategory::Syntax,
                )
                .suggest("Check for missing or extra parentheses"),
            );
        }

        let open_braces = output.matches('{').count();
        let close_braces = output.matches('}').count();
        if open_braces != close_braces {
            issues.push(
                CritiqueIssue::new(
                    format!(
                        "Unbalanced braces: {} open, {} close",
                        open_braces, close_braces
                    ),
                    0.7,
                    IssueCategory::Syntax,
                )
                .suggest("Check for missing or extra braces"),
            );
        }

        issues
    }

    /// Check for logic issues
    fn check_logic(&self, output: &str) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();

        // Check for potential infinite loops
        if output.contains("loop {") && !output.contains("break") {
            issues.push(
                CritiqueIssue::new(
                    "Potential infinite loop without break",
                    0.6,
                    IssueCategory::Logic,
                )
                .suggest("Add break condition or use while/for loop"),
            );
        }

        // Check for empty functions
        let empty_fn_pattern = "fn ";
        if output.contains(empty_fn_pattern) {
            // Simple heuristic: function with just {}
            if output.contains("{ }") || output.contains("{}") {
                issues.push(
                    CritiqueIssue::new("Empty function body detected", 0.4, IssueCategory::Missing)
                        .suggest("Implement function body or add todo!()"),
                );
            }
        }

        // Check for hardcoded values that might be problematic
        if output.contains("localhost") || output.contains("127.0.0.1") {
            issues.push(
                CritiqueIssue::new("Hardcoded localhost/IP address", 0.3, IssueCategory::Style)
                    .suggest("Consider using configuration or environment variables"),
            );
        }

        issues
    }
}

impl Default for CorrectnessChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl Perspective for CorrectnessChecker {
    fn name(&self) -> &str {
        "correctness"
    }

    fn critique(&self, output: &str, _context: &ExecutionContext) -> CritiqueResult {
        let start = std::time::Instant::now();

        if output.is_empty() {
            return CritiqueResult::fail(self.name(), 0.0, "Empty output").with_issue(
                CritiqueIssue::new("No output provided", 1.0, IssueCategory::Missing),
            );
        }

        let mut issues = Vec::new();
        let mut strengths = Vec::new();

        // Check for errors
        issues.extend(self.check_for_errors(output));

        // Check logic
        issues.extend(self.check_logic(output));

        // Identify strengths
        if output.contains("Result<") || output.contains("Option<") {
            strengths.push("Uses proper error handling types".to_string());
        }
        if output.contains("#[test]") {
            strengths.push("Includes tests".to_string());
        }
        if output.contains("///") || output.contains("//!") {
            strengths.push("Includes documentation".to_string());
        }

        // Calculate score
        let issue_penalty: f32 = issues.iter().map(|i| i.severity * 0.15).sum();
        let score = (1.0 - issue_penalty).clamp(0.0, 1.0);
        let passed = score >= self.config.pass_threshold;

        let summary = if passed {
            format!(
                "Code appears correct with {} minor issue(s)",
                issues.iter().filter(|i| i.severity < 0.5).count()
            )
        } else {
            format!("Found {} issue(s) affecting correctness", issues.len())
        };

        let mut result = if passed {
            CritiqueResult::pass(self.name(), score, summary)
        } else {
            CritiqueResult::fail(self.name(), score, summary)
        };

        result.issues = issues;
        result.strengths = strengths;
        result.critique_time_ms = start.elapsed().as_millis() as u64;
        result
    }

    fn config(&self) -> &PerspectiveConfig {
        &self.config
    }
}

/// Completeness checker perspective
///
/// Checks if all requirements are addressed and the output is complete
pub struct CompletenessChecker {
    config: PerspectiveConfig,
}

impl CompletenessChecker {
    /// Create a new completeness checker
    pub fn new() -> Self {
        Self {
            config: PerspectiveConfig {
                weight: 1.0,
                pass_threshold: 0.6,
                detailed_feedback: true,
                custom_checks: Vec::new(),
            },
        }
    }

    /// Create with custom config
    pub fn with_config(config: PerspectiveConfig) -> Self {
        Self { config }
    }

    /// Extract requirements from task
    fn extract_requirements(&self, task: &str) -> Vec<String> {
        let mut requirements = Vec::new();

        // Look for action verbs
        let action_words = [
            "implement",
            "create",
            "add",
            "build",
            "write",
            "define",
            "include",
            "support",
            "handle",
            "return",
            "take",
            "accept",
        ];

        for word in action_words {
            if task.to_lowercase().contains(word) {
                requirements.push(format!("Task mentions '{}' action", word));
            }
        }

        // Look for specific features mentioned
        if task.contains("error handling") || task.contains("handle error") {
            requirements.push("Error handling".to_string());
        }
        if task.contains("test") {
            requirements.push("Tests".to_string());
        }
        if task.contains("document") {
            requirements.push("Documentation".to_string());
        }
        if task.contains("async") {
            requirements.push("Async support".to_string());
        }

        requirements
    }

    /// Check if requirements are met
    fn check_requirements(&self, output: &str, requirements: &[String]) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();
        let output_lower = output.to_lowercase();

        for req in requirements {
            let req_lower = req.to_lowercase();

            // Simple keyword matching for requirement fulfillment
            let is_met = req_lower
                .split_whitespace()
                .any(|word| word.len() > 3 && output_lower.contains(word));

            if !is_met {
                issues.push(
                    CritiqueIssue::new(
                        format!("Requirement may not be addressed: {}", req),
                        0.4,
                        IssueCategory::Missing,
                    )
                    .suggest(format!("Ensure {} is implemented", req)),
                );
            }
        }

        issues
    }

    /// Check for incomplete markers
    fn check_incomplete_markers(&self, output: &str) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();

        let markers = [
            ("TODO", "Incomplete TODO item"),
            ("FIXME", "Incomplete FIXME item"),
            ("XXX", "XXX marker present"),
            ("HACK", "Temporary hack present"),
            ("...", "Ellipsis indicating incomplete"),
            ("// ...", "Code omitted marker"),
            ("/* ... */", "Code omitted block"),
        ];

        for (marker, description) in markers {
            if output.contains(marker) {
                let count = output.matches(marker).count();
                issues.push(
                    CritiqueIssue::new(
                        format!("{} ({} occurrence(s))", description, count),
                        0.5,
                        IssueCategory::Missing,
                    )
                    .suggest(format!("Complete or remove {} markers", marker)),
                );
            }
        }

        issues
    }
}

impl Default for CompletenessChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl Perspective for CompletenessChecker {
    fn name(&self) -> &str {
        "completeness"
    }

    fn critique(&self, output: &str, context: &ExecutionContext) -> CritiqueResult {
        let start = std::time::Instant::now();

        if output.is_empty() {
            return CritiqueResult::fail(self.name(), 0.0, "Empty output - nothing completed")
                .with_issue(CritiqueIssue::new(
                    "No output provided",
                    1.0,
                    IssueCategory::Missing,
                ));
        }

        let mut issues = Vec::new();
        let mut strengths = Vec::new();

        // Extract and check requirements
        let requirements = self.extract_requirements(&context.task);
        issues.extend(self.check_requirements(output, &requirements));

        // Check for incomplete markers
        issues.extend(self.check_incomplete_markers(output));

        // Check output length as proxy for completeness
        let line_count = output.lines().count();
        if line_count < 5 && context.task.len() > 50 {
            issues.push(
                CritiqueIssue::new(
                    "Output may be too brief for the task complexity",
                    0.3,
                    IssueCategory::Missing,
                )
                .suggest("Consider expanding the implementation"),
            );
        }

        // Identify completeness strengths
        if !output.contains("TODO") && !output.contains("FIXME") {
            strengths.push("No incomplete TODO/FIXME markers".to_string());
        }
        if output.lines().count() > 20 {
            strengths.push("Substantial implementation provided".to_string());
        }

        // Calculate score
        let issue_penalty: f32 = issues.iter().map(|i| i.severity * 0.2).sum();
        let score = (1.0 - issue_penalty).clamp(0.0, 1.0);
        let passed = score >= self.config.pass_threshold;

        let summary = if passed {
            "Output appears complete with all major requirements addressed"
        } else {
            "Output may be incomplete - some requirements not clearly addressed"
        };

        let mut result = if passed {
            CritiqueResult::pass(self.name(), score, summary)
        } else {
            CritiqueResult::fail(self.name(), score, summary)
        };

        result.issues = issues;
        result.strengths = strengths;
        result.critique_time_ms = start.elapsed().as_millis() as u64;
        result
    }

    fn config(&self) -> &PerspectiveConfig {
        &self.config
    }
}

/// Consistency checker perspective
///
/// Ensures internal consistency and adherence to conventions
pub struct ConsistencyChecker {
    config: PerspectiveConfig,
}

impl ConsistencyChecker {
    /// Create a new consistency checker
    pub fn new() -> Self {
        Self {
            config: PerspectiveConfig {
                weight: 0.8, // Slightly lower weight
                pass_threshold: 0.5,
                detailed_feedback: true,
                custom_checks: Vec::new(),
            },
        }
    }

    /// Create with custom config
    pub fn with_config(config: PerspectiveConfig) -> Self {
        Self { config }
    }

    /// Check naming conventions
    fn check_naming(&self, output: &str) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();

        // Check for mixed naming conventions (simple heuristic)
        let _has_snake_case = output.contains("_") && output.contains("fn ");
        let has_camel_case = output
            .chars()
            .zip(output.chars().skip(1))
            .any(|(a, b)| a.is_lowercase() && b.is_uppercase());

        // In Rust, we expect snake_case for functions/variables
        if has_camel_case && output.contains("fn ") && !output.contains("trait ") {
            issues.push(
                CritiqueIssue::new(
                    "Possible camelCase usage in Rust code (should use snake_case)",
                    0.3,
                    IssueCategory::Style,
                )
                .suggest("Use snake_case for function and variable names"),
            );
        }

        issues
    }

    /// Check for consistent formatting
    fn check_formatting(&self, output: &str) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();

        // Check for inconsistent indentation
        let lines: Vec<&str> = output.lines().collect();
        let mut indent_styles = HashMap::new();

        for line in &lines {
            if line.starts_with("    ") {
                *indent_styles.entry("4spaces").or_insert(0) += 1;
            } else if line.starts_with("  ") && !line.starts_with("    ") {
                *indent_styles.entry("2spaces").or_insert(0) += 1;
            } else if line.starts_with('\t') {
                *indent_styles.entry("tabs").or_insert(0) += 1;
            }
        }

        if indent_styles.len() > 1 {
            issues.push(
                CritiqueIssue::new(
                    "Inconsistent indentation style detected",
                    0.4,
                    IssueCategory::Style,
                )
                .suggest("Use consistent indentation (4 spaces recommended for Rust)"),
            );
        }

        // Check for trailing whitespace
        let trailing_ws_count = lines.iter().filter(|l| l.ends_with(' ')).count();
        if trailing_ws_count > 0 {
            issues.push(
                CritiqueIssue::new(
                    format!("Trailing whitespace on {} line(s)", trailing_ws_count),
                    0.2,
                    IssueCategory::Style,
                )
                .suggest("Remove trailing whitespace"),
            );
        }

        issues
    }

    /// Check for internal consistency
    fn check_internal_consistency(&self, output: &str) -> Vec<CritiqueIssue> {
        let mut issues = Vec::new();

        // Check for mix of error handling styles
        let uses_result = output.contains("Result<");
        let uses_option = output.contains("Option<");
        let uses_unwrap = output.contains(".unwrap()");
        let uses_question = output.contains("?;") || output.contains("?)");

        if (uses_result || uses_option) && uses_unwrap && uses_question {
            issues.push(
                CritiqueIssue::new(
                    "Inconsistent error handling: mixing ? operator and unwrap()",
                    0.4,
                    IssueCategory::Inconsistent,
                )
                .suggest("Prefer using ? operator consistently for error propagation"),
            );
        }

        // Check for consistent visibility modifiers
        let pub_count = output.matches("pub fn").count();
        let priv_count = output.matches("fn ").count() - pub_count;

        if pub_count > 0
            && priv_count > 0
            && (pub_count as f32 / (pub_count + priv_count) as f32) < 0.3
        {
            // This is actually fine, just noting it
        }

        issues
    }
}

impl Default for ConsistencyChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl Perspective for ConsistencyChecker {
    fn name(&self) -> &str {
        "consistency"
    }

    fn critique(&self, output: &str, _context: &ExecutionContext) -> CritiqueResult {
        let start = std::time::Instant::now();

        if output.is_empty() {
            return CritiqueResult::fail(self.name(), 0.0, "Empty output").with_issue(
                CritiqueIssue::new(
                    "No output to check consistency",
                    1.0,
                    IssueCategory::Missing,
                ),
            );
        }

        let mut issues = Vec::new();
        let mut strengths = Vec::new();

        // Check naming conventions
        issues.extend(self.check_naming(output));

        // Check formatting
        issues.extend(self.check_formatting(output));

        // Check internal consistency
        issues.extend(self.check_internal_consistency(output));

        // Identify strengths
        if !issues
            .iter()
            .any(|i| i.category == IssueCategory::Inconsistent)
        {
            strengths.push("Consistent coding style".to_string());
        }
        if output.contains("use std::") || output.contains("use crate::") {
            strengths.push("Proper import organization".to_string());
        }

        // Calculate score
        let issue_penalty: f32 = issues.iter().map(|i| i.severity * 0.15).sum();
        let score = (1.0 - issue_penalty).clamp(0.0, 1.0);
        let passed = score >= self.config.pass_threshold;

        let summary = if passed {
            "Code follows consistent conventions and style"
        } else {
            "Inconsistencies detected in style or conventions"
        };

        let mut result = if passed {
            CritiqueResult::pass(self.name(), score, summary)
        } else {
            CritiqueResult::fail(self.name(), score, summary)
        };

        result.issues = issues;
        result.strengths = strengths;
        result.critique_time_ms = start.elapsed().as_millis() as u64;
        result
    }

    fn config(&self) -> &PerspectiveConfig {
        &self.config
    }
}

/// Unified critique combining multiple perspectives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCritique {
    /// Individual critique results
    pub critiques: Vec<CritiqueResult>,
    /// Overall pass/fail
    pub passed: bool,
    /// Combined score (weighted average)
    pub combined_score: f32,
    /// Overall summary
    pub summary: String,
    /// Prioritized issues (sorted by severity)
    pub prioritized_issues: Vec<CritiqueIssue>,
    /// All identified strengths
    pub strengths: Vec<String>,
    /// Total critique time
    pub total_time_ms: u64,
}

impl UnifiedCritique {
    /// Create a unified critique from multiple perspective results
    pub fn combine(critiques: Vec<CritiqueResult>, weights: &[f32]) -> Self {
        let mut total_weight = 0.0f32;
        let mut weighted_sum = 0.0f32;
        let mut all_issues = Vec::new();
        let mut all_strengths = Vec::new();
        let mut total_time = 0u64;

        for (i, critique) in critiques.iter().enumerate() {
            let weight = weights.get(i).copied().unwrap_or(1.0);
            total_weight += weight;
            weighted_sum += critique.score * weight;

            all_issues.extend(critique.issues.clone());
            all_strengths.extend(critique.strengths.clone());
            total_time += critique.critique_time_ms;
        }

        let combined_score = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // Sort issues by severity
        all_issues.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate strengths
        all_strengths.sort();
        all_strengths.dedup();

        let pass_count = critiques.iter().filter(|c| c.passed).count();
        let passed = pass_count > critiques.len() / 2 && combined_score >= 0.6;

        let summary = if passed {
            format!(
                "Passed {}/{} perspectives with combined score {:.2}",
                pass_count,
                critiques.len(),
                combined_score
            )
        } else {
            format!(
                "Failed: only {}/{} perspectives passed, combined score {:.2}",
                pass_count,
                critiques.len(),
                combined_score
            )
        };

        Self {
            critiques,
            passed,
            combined_score,
            summary,
            prioritized_issues: all_issues,
            strengths: all_strengths,
            total_time_ms: total_time,
        }
    }

    /// Get the top N issues
    pub fn top_issues(&self, n: usize) -> Vec<&CritiqueIssue> {
        self.prioritized_issues.iter().take(n).collect()
    }

    /// Get issues by category
    pub fn issues_by_category(&self, category: IssueCategory) -> Vec<&CritiqueIssue> {
        self.prioritized_issues
            .iter()
            .filter(|i| i.category == category)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude_flow::AgentType;

    fn test_context() -> ExecutionContext {
        ExecutionContext::new("implement a function", AgentType::Coder, "test input")
    }

    #[test]
    fn test_critique_result_builders() {
        let pass = CritiqueResult::pass("test", 0.8, "Good job").with_strength("Clean code");
        assert!(pass.passed);
        assert!(!pass.strengths.is_empty());

        let fail = CritiqueResult::fail("test", 0.3, "Issues found")
            .with_issue(CritiqueIssue::new("Problem", 0.7, IssueCategory::Logic));
        assert!(!fail.passed);
        assert!(!fail.issues.is_empty());
    }

    #[test]
    fn test_critique_issue_builder() {
        let issue = CritiqueIssue::new("Test issue", 0.5, IssueCategory::Logic)
            .at("line 5")
            .suggest("Fix it");

        assert_eq!(issue.location, Some("line 5".to_string()));
        assert!(!issue.suggestion.is_empty());
    }

    #[test]
    fn test_correctness_checker_empty() {
        let checker = CorrectnessChecker::new();
        let context = test_context();
        let result = checker.critique("", &context);

        assert!(!result.passed);
        assert!(result.score < 0.5);
    }

    #[test]
    fn test_correctness_checker_with_errors() {
        let checker = CorrectnessChecker::new();
        let context = test_context();
        let output = r#"
            fn test() {
                panic!("error");
                todo!();
            }
        "#;

        let result = checker.critique(output, &context);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_correctness_checker_clean_code() {
        let checker = CorrectnessChecker::new();
        let context = test_context();
        let output = r#"
            /// Documentation
            pub fn example() -> Result<(), Error> {
                Ok(())
            }

            #[test]
            fn test_example() {
                assert!(example().is_ok());
            }
        "#;

        let result = checker.critique(output, &context);
        assert!(!result.strengths.is_empty());
    }

    #[test]
    fn test_completeness_checker_todo() {
        let checker = CompletenessChecker::new();
        let context = test_context();
        let output = "fn example() { // TODO: implement }";

        let result = checker.critique(output, &context);
        assert!(result
            .issues
            .iter()
            .any(|i| i.category == IssueCategory::Missing));
    }

    #[test]
    fn test_completeness_checker_complete() {
        let checker = CompletenessChecker::new();
        let context = ExecutionContext::new("implement function", AgentType::Coder, "input");
        let output = r#"
            pub fn implement_function() -> i32 {
                let value = 42;
                // Full implementation here
                value * 2
            }
        "#;

        let result = checker.critique(output, &context);
        assert!(result.passed || result.score > 0.5);
    }

    #[test]
    fn test_consistency_checker_mixed_indent() {
        let checker = ConsistencyChecker::new();
        let context = test_context();
        let output = "fn test() {\n    line1\n  line2\n\tline3\n}";

        let result = checker.critique(output, &context);
        assert!(result
            .issues
            .iter()
            .any(|i| i.category == IssueCategory::Style));
    }

    #[test]
    fn test_consistency_checker_clean() {
        let checker = ConsistencyChecker::new();
        let context = test_context();
        let output = r#"
use std::io;

fn clean_function() -> io::Result<()> {
    let value = 42;
    Ok(())
}
        "#;

        let result = checker.critique(output, &context);
        // Should pass or have high score
        assert!(result.score > 0.5);
    }

    #[test]
    fn test_unified_critique() {
        let correctness = CritiqueResult::pass("correctness", 0.8, "Good");
        let completeness = CritiqueResult::pass("completeness", 0.7, "Complete");
        let consistency = CritiqueResult::fail("consistency", 0.4, "Issues");

        let unified = UnifiedCritique::combine(
            vec![correctness, completeness, consistency],
            &[1.2, 1.0, 0.8],
        );

        assert!(unified.combined_score > 0.5);
        assert!(!unified.summary.is_empty());
    }

    #[test]
    fn test_unified_critique_issues_by_category() {
        let mut result = CritiqueResult::fail("test", 0.5, "Issues")
            .with_issue(CritiqueIssue::new("Logic issue", 0.7, IssueCategory::Logic))
            .with_issue(CritiqueIssue::new("Style issue", 0.3, IssueCategory::Style));

        let unified = UnifiedCritique::combine(vec![result], &[1.0]);

        let logic_issues = unified.issues_by_category(IssueCategory::Logic);
        assert_eq!(logic_issues.len(), 1);
    }

    #[test]
    fn test_perspective_trait_implementation() {
        let checker: Box<dyn Perspective> = Box::new(CorrectnessChecker::new());
        assert_eq!(checker.name(), "correctness");

        let context = test_context();
        let result = checker.critique("fn test() {}", &context);
        assert!(!result.perspective_name.is_empty());
    }
}
