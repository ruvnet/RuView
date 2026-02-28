//! Intelligent Model Router for Claude Flow
//!
//! Routes tasks to optimal Claude models (Haiku/Sonnet/Opus) based on:
//! - Task complexity analysis
//! - Token usage estimation
//! - Reasoning depth requirements
//! - Cost/latency trade-offs
//!
//! ## Routing Strategy
//!
//! | Model | Token Threshold | Complexity | Use Cases |
//! |-------|-----------------|------------|-----------|
//! | Haiku | < 500 tokens | Simple patterns | Bug fixes, formatting, simple transforms |
//! | Sonnet | 500-2000 tokens | Moderate | Feature impl, refactoring, testing |
//! | Opus | > 2000 tokens | Deep reasoning | Architecture, security, complex analysis |
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | TaskComplexity    |---->| ModelSelector     |
//! | Analyzer          |     | (routing logic)   |
//! +--------+----------+     +--------+----------+
//!          |                         |
//!          v                         v
//! +--------+----------+     +--------+----------+
//! | ComplexityScore   |     | RoutingDecision   |
//! | (multi-factor)    |     | (model + reason)  |
//! +-------------------+     +-------------------+
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::claude_integration::ClaudeModel;
use super::{AgentType, ClaudeFlowAgent, ClaudeFlowTask};
use crate::error::Result;

/// Fast case-insensitive substring search without allocation
/// Searches for `needle` (lowercase) in `haystack` (any case)
#[inline]
fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    let first_lower = needle[0];
    let first_upper = first_lower.to_ascii_uppercase();

    for i in 0..=(haystack.len() - needle.len()) {
        let c = haystack[i];
        if c == first_lower || c == first_upper {
            // Potential match, check rest
            let mut matches = true;
            for (j, &n) in needle.iter().enumerate().skip(1) {
                let h = haystack[i + j];
                if h != n && h != n.to_ascii_uppercase() {
                    matches = false;
                    break;
                }
            }
            if matches {
                return true;
            }
        }
    }
    false
}

// ============================================================================
// Complexity Analysis Types
// ============================================================================

/// Complexity factors for task analysis
#[derive(Debug, Clone, Default)]
pub struct ComplexityFactors {
    /// Estimated token usage
    pub token_estimate: usize,
    /// Reasoning depth required (0.0 - 1.0)
    pub reasoning_depth: f32,
    /// Domain expertise required (0.0 - 1.0)
    pub domain_expertise: f32,
    /// Code generation complexity (0.0 - 1.0)
    pub code_complexity: f32,
    /// Multi-step planning required (0.0 - 1.0)
    pub planning_complexity: f32,
    /// Security sensitivity (0.0 - 1.0)
    pub security_sensitivity: f32,
    /// Performance criticality (0.0 - 1.0)
    pub performance_criticality: f32,
}

/// Cached default weights - avoid repeated allocations
static DEFAULT_WEIGHTS: std::sync::LazyLock<ComplexityWeights> =
    std::sync::LazyLock::new(ComplexityWeights::default);

impl ComplexityFactors {
    /// Calculate weighted complexity score
    #[inline]
    pub fn weighted_score(&self) -> f32 {
        // Use cached weights
        let weights = &*DEFAULT_WEIGHTS;

        // Token-based complexity
        let token_factor = match self.token_estimate {
            0..=500 => 0.2,
            501..=1000 => 0.4,
            1001..=2000 => 0.6,
            2001..=5000 => 0.8,
            _ => 1.0,
        };

        (token_factor * weights.token_weight)
            + (self.reasoning_depth * weights.reasoning_weight)
            + (self.domain_expertise * weights.domain_weight)
            + (self.code_complexity * weights.code_weight)
            + (self.planning_complexity * weights.planning_weight)
            + (self.security_sensitivity * weights.security_weight)
            + (self.performance_criticality * weights.performance_weight)
    }
}

/// Weights for complexity factors
#[derive(Debug, Clone)]
pub struct ComplexityWeights {
    /// Token count weight
    pub token_weight: f32,
    /// Reasoning depth weight
    pub reasoning_weight: f32,
    /// Domain expertise weight
    pub domain_weight: f32,
    /// Code complexity weight
    pub code_weight: f32,
    /// Planning complexity weight
    pub planning_weight: f32,
    /// Security sensitivity weight
    pub security_weight: f32,
    /// Performance criticality weight
    pub performance_weight: f32,
}

impl Default for ComplexityWeights {
    fn default() -> Self {
        Self {
            token_weight: 0.20,
            reasoning_weight: 0.25,
            domain_weight: 0.10,
            code_weight: 0.15,
            planning_weight: 0.10,
            security_weight: 0.10,
            performance_weight: 0.10,
        }
    }
}

/// Complexity score with breakdown
#[derive(Debug, Clone)]
pub struct ComplexityScore {
    /// Overall complexity (0.0 - 1.0)
    pub overall: f32,
    /// Individual factors
    pub factors: ComplexityFactors,
    /// Recommended tier (1=Haiku, 2=Sonnet, 3=Opus)
    pub recommended_tier: u8,
    /// Confidence in assessment (0.0 - 1.0)
    pub confidence: f32,
    /// Analysis reasoning
    pub reasoning: String,
}

impl ComplexityScore {
    /// Get recommended model based on score
    #[inline]
    pub fn recommended_model(&self) -> ClaudeModel {
        match self.recommended_tier {
            1 => ClaudeModel::Haiku,
            2 => ClaudeModel::Sonnet,
            _ => ClaudeModel::Opus,
        }
    }

    /// Check if task is simple enough for Haiku
    #[inline]
    pub fn is_simple(&self) -> bool {
        self.overall < 0.35 && self.factors.token_estimate < 500
    }

    /// Check if task requires Opus
    #[inline]
    pub fn requires_opus(&self) -> bool {
        self.overall > 0.7
            || self.factors.token_estimate > 2000
            || self.factors.security_sensitivity > 0.8
            || self.factors.reasoning_depth > 0.8
    }
}

// ============================================================================
// Task Complexity Analyzer
// ============================================================================

/// Patterns that indicate high complexity
const HIGH_COMPLEXITY_PATTERNS: &[&str] = &[
    "architecture",
    "design pattern",
    "distributed",
    "concurrent",
    "security audit",
    "vulnerability",
    "performance optimization",
    "scalability",
    "migration",
    "refactor entire",
    "redesign",
    "multi-agent",
    "complex algorithm",
    "machine learning",
    "cryptography",
];

/// Patterns that indicate moderate complexity
const MODERATE_COMPLEXITY_PATTERNS: &[&str] = &[
    "implement",
    "create feature",
    "add functionality",
    "write tests",
    "integration test",
    "api endpoint",
    "database query",
    "refactor",
    "debugging",
    "error handling",
    "validation",
];

/// Patterns that indicate simple tasks
const SIMPLE_PATTERNS: &[&str] = &[
    "fix typo",
    "rename",
    "add comment",
    "format",
    "simple change",
    "quick fix",
    "update config",
    "minor change",
    "small update",
    "add import",
    "remove unused",
];

/// Task complexity analyzer
pub struct TaskComplexityAnalyzer {
    /// Pattern weights
    pattern_weights: HashMap<String, f32>,
    /// Task type complexity mapping
    task_type_complexity: HashMap<String, f32>,
    /// Historical accuracy data
    accuracy_history: Vec<AccuracyRecord>,
    /// Analysis count
    analysis_count: u64,
}

/// Accuracy record for learning
#[derive(Debug, Clone)]
struct AccuracyRecord {
    /// Predicted complexity
    predicted: f32,
    /// Actual complexity (from feedback)
    actual: Option<f32>,
    /// Model used
    model: ClaudeModel,
    /// Timestamp
    timestamp: Instant,
}

impl TaskComplexityAnalyzer {
    /// Create new analyzer
    pub fn new() -> Self {
        Self {
            pattern_weights: Self::build_pattern_weights(),
            task_type_complexity: Self::build_task_type_complexity(),
            accuracy_history: Vec::new(),
            analysis_count: 0,
        }
    }

    /// Build pattern weight mapping
    fn build_pattern_weights() -> HashMap<String, f32> {
        let mut weights = HashMap::new();

        // High complexity patterns
        for pattern in HIGH_COMPLEXITY_PATTERNS {
            weights.insert(pattern.to_string(), 0.9);
        }

        // Moderate complexity patterns
        for pattern in MODERATE_COMPLEXITY_PATTERNS {
            weights.insert(pattern.to_string(), 0.5);
        }

        // Simple patterns
        for pattern in SIMPLE_PATTERNS {
            weights.insert(pattern.to_string(), 0.2);
        }

        weights
    }

    /// Build task type complexity mapping
    fn build_task_type_complexity() -> HashMap<String, f32> {
        let mut map = HashMap::new();
        map.insert("CodeGeneration".to_string(), 0.5);
        map.insert("CodeReview".to_string(), 0.6);
        map.insert("Testing".to_string(), 0.4);
        map.insert("Research".to_string(), 0.5);
        map.insert("Documentation".to_string(), 0.3);
        map.insert("Debugging".to_string(), 0.5);
        map.insert("Refactoring".to_string(), 0.6);
        map.insert("Security".to_string(), 0.8);
        map.insert("Performance".to_string(), 0.7);
        map.insert("Architecture".to_string(), 0.9);
        map
    }

    /// Analyze task complexity
    pub fn analyze(&mut self, task: &str) -> ComplexityScore {
        self.analysis_count += 1;
        let lower_task = task.to_lowercase();

        // Estimate token usage
        let token_estimate = self.estimate_tokens(task);

        // Analyze reasoning depth
        let reasoning_depth = self.analyze_reasoning_depth(&lower_task);

        // Analyze domain expertise needed
        let domain_expertise = self.analyze_domain_expertise(&lower_task);

        // Analyze code complexity
        let code_complexity = self.analyze_code_complexity(&lower_task);

        // Analyze planning requirements
        let planning_complexity = self.analyze_planning(&lower_task);

        // Analyze security sensitivity
        let security_sensitivity = self.analyze_security(&lower_task);

        // Analyze performance criticality
        let performance_criticality = self.analyze_performance(&lower_task);

        let factors = ComplexityFactors {
            token_estimate,
            reasoning_depth,
            domain_expertise,
            code_complexity,
            planning_complexity,
            security_sensitivity,
            performance_criticality,
        };

        let overall = factors.weighted_score();

        // Determine tier
        let recommended_tier = if overall < 0.35 && token_estimate < 500 {
            1 // Haiku
        } else if overall < 0.7 && token_estimate < 2000 {
            2 // Sonnet
        } else {
            3 // Opus
        };

        // Calculate confidence based on pattern matches
        let confidence = self.calculate_confidence(&lower_task);

        // Generate reasoning
        let reasoning = self.generate_reasoning(&factors, recommended_tier);

        ComplexityScore {
            overall,
            factors,
            recommended_tier,
            confidence,
            reasoning,
        }
    }

    /// Estimate token usage for task
    /// Uses byte-level scanning to avoid allocation from to_lowercase()
    #[inline]
    fn estimate_tokens(&self, task: &str) -> usize {
        let base_tokens = task.len() / 4; // Rough estimate

        // Fast case-insensitive contains check without allocation
        let task_bytes = task.as_bytes();

        let multiplier = if contains_ci(task_bytes, b"entire")
            || contains_ci(task_bytes, b"all")
            || contains_ci(task_bytes, b"comprehensive")
        {
            3.0
        } else if contains_ci(task_bytes, b"full") || contains_ci(task_bytes, b"complete") {
            2.5
        } else if contains_ci(task_bytes, b"implement") || contains_ci(task_bytes, b"create") {
            2.0
        } else if contains_ci(task_bytes, b"fix") || contains_ci(task_bytes, b"update") {
            1.2
        } else {
            1.5
        };

        // Additional factors
        let factor = if contains_ci(task_bytes, b"architecture")
            || contains_ci(task_bytes, b"design")
        {
            3.0
        } else if contains_ci(task_bytes, b"test") {
            1.5
        } else if contains_ci(task_bytes, b"comment") || contains_ci(task_bytes, b"documentation") {
            1.2
        } else {
            1.0
        };

        ((base_tokens as f32 * multiplier * factor) as usize).max(100)
    }

    /// Analyze reasoning depth required
    #[inline]
    fn analyze_reasoning_depth(&self, task: &str) -> f32 {
        let mut depth: f32 = 0.3; // Base

        // High reasoning indicators
        if task.contains("why") || task.contains("explain") || task.contains("analyze") {
            depth += 0.2;
        }
        if task.contains("trade-off") || task.contains("compare") || task.contains("evaluate") {
            depth += 0.2;
        }
        if task.contains("design") || task.contains("architect") || task.contains("pattern") {
            depth += 0.3;
        }
        if task.contains("debug") || task.contains("investigate") || task.contains("root cause") {
            depth += 0.2;
        }

        // Complex reasoning
        if task.contains("distributed") || task.contains("concurrent") || task.contains("parallel")
        {
            depth += 0.3;
        }

        depth.min(1.0_f32)
    }

    /// Analyze domain expertise needed
    #[inline]
    fn analyze_domain_expertise(&self, task: &str) -> f32 {
        let mut expertise: f32 = 0.2; // Base

        // Technical domains
        if task.contains("database") || task.contains("sql") || task.contains("query") {
            expertise += 0.2;
        }
        if task.contains("network") || task.contains("protocol") || task.contains("http") {
            expertise += 0.2;
        }
        if task.contains("security") || task.contains("crypto") || task.contains("auth") {
            expertise += 0.3;
        }
        if task.contains("ml") || task.contains("machine learning") || task.contains("model") {
            expertise += 0.3;
        }
        if task.contains("system") || task.contains("kernel") || task.contains("low-level") {
            expertise += 0.3;
        }

        expertise.min(1.0_f32)
    }

    /// Analyze code complexity
    #[inline]
    fn analyze_code_complexity(&self, task: &str) -> f32 {
        let mut complexity: f32 = 0.3; // Base

        // Complex code patterns
        if task.contains("algorithm") || task.contains("data structure") {
            complexity += 0.3;
        }
        if task.contains("recursive") || task.contains("dynamic programming") {
            complexity += 0.3;
        }
        if task.contains("async") || task.contains("concurrent") || task.contains("thread") {
            complexity += 0.2;
        }
        if task.contains("generic") || task.contains("trait") || task.contains("interface") {
            complexity += 0.1;
        }

        // Simple code patterns reduce complexity
        if task.contains("simple") || task.contains("basic") || task.contains("minor") {
            complexity -= 0.2;
        }

        complexity.clamp(0.0_f32, 1.0_f32)
    }

    /// Analyze planning requirements
    #[inline]
    fn analyze_planning(&self, task: &str) -> f32 {
        let mut planning: f32 = 0.2; // Base

        // Multi-step indicators
        if task.contains("then") || task.contains("after") || task.contains("first") {
            planning += 0.2;
        }
        if task.contains("workflow") || task.contains("pipeline") || task.contains("process") {
            planning += 0.3;
        }
        if task.contains("migrate") || task.contains("upgrade") || task.contains("transition") {
            planning += 0.3;
        }
        if task.contains("coordinate") || task.contains("orchestrate") {
            planning += 0.2;
        }

        planning.min(1.0_f32)
    }

    /// Analyze security sensitivity
    #[inline]
    fn analyze_security(&self, task: &str) -> f32 {
        let mut sensitivity: f32 = 0.1; // Base

        // Security keywords
        if task.contains("security") || task.contains("secure") || task.contains("auth") {
            sensitivity += 0.3;
        }
        if task.contains("vulnerability") || task.contains("cve") || task.contains("exploit") {
            sensitivity += 0.4;
        }
        if task.contains("encrypt") || task.contains("decrypt") || task.contains("crypto") {
            sensitivity += 0.3;
        }
        if task.contains("password") || task.contains("secret") || task.contains("key") {
            sensitivity += 0.2;
        }
        if task.contains("injection") || task.contains("xss") || task.contains("csrf") {
            sensitivity += 0.3;
        }

        sensitivity.min(1.0_f32)
    }

    /// Analyze performance criticality
    #[inline]
    fn analyze_performance(&self, task: &str) -> f32 {
        let mut criticality: f32 = 0.1; // Base

        // Performance keywords
        if task.contains("performance") || task.contains("optimize") || task.contains("speed") {
            criticality += 0.3;
        }
        if task.contains("benchmark") || task.contains("profile") || task.contains("latency") {
            criticality += 0.2;
        }
        if task.contains("memory") || task.contains("cache") || task.contains("efficient") {
            criticality += 0.2;
        }
        if task.contains("scale") || task.contains("throughput") || task.contains("concurrent") {
            criticality += 0.2;
        }

        criticality.min(1.0_f32)
    }

    /// Calculate confidence in analysis
    fn calculate_confidence(&self, task: &str) -> f32 {
        let mut matches = 0;
        let total_patterns = self.pattern_weights.len();

        for pattern in self.pattern_weights.keys() {
            if task.contains(pattern) {
                matches += 1;
            }
        }

        // Base confidence
        let pattern_confidence = if matches > 0 {
            0.5 + (matches as f32 / total_patterns as f32) * 0.4
        } else {
            0.4
        };

        // Task length affects confidence
        let length_factor = if task.len() > 100 {
            1.0
        } else if task.len() > 50 {
            0.9
        } else {
            0.7
        };

        (pattern_confidence * length_factor).min(0.95)
    }

    /// Generate reasoning for recommendation
    fn generate_reasoning(&self, factors: &ComplexityFactors, tier: u8) -> String {
        let model = match tier {
            1 => "Haiku",
            2 => "Sonnet",
            _ => "Opus",
        };

        let mut reasons = Vec::new();

        if factors.token_estimate < 500 {
            reasons.push(format!("low token estimate (~{})", factors.token_estimate));
        } else if factors.token_estimate > 2000 {
            reasons.push(format!("high token estimate (~{})", factors.token_estimate));
        }

        if factors.reasoning_depth > 0.7 {
            reasons.push("deep reasoning required".to_string());
        }

        if factors.security_sensitivity > 0.7 {
            reasons.push("security-sensitive task".to_string());
        }

        if factors.code_complexity > 0.7 {
            reasons.push("complex code patterns".to_string());
        }

        if reasons.is_empty() {
            reasons.push("balanced complexity factors".to_string());
        }

        format!("Recommended {} due to: {}", model, reasons.join(", "))
    }

    /// Record feedback for learning
    pub fn record_feedback(&mut self, predicted: f32, actual: f32, model: ClaudeModel) {
        self.accuracy_history.push(AccuracyRecord {
            predicted,
            actual: Some(actual),
            model,
            timestamp: Instant::now(),
        });

        // Keep history bounded
        if self.accuracy_history.len() > 1000 {
            self.accuracy_history.remove(0);
        }
    }

    /// Returns signed calibration error.
    ///
    /// Positive value = systematically over-predicting complexity,
    /// negative value = systematically under-predicting.
    /// Returns 0.0 if no feedback has been recorded.
    pub fn calibration_bias(&self) -> f32 {
        let with_feedback: Vec<_> = self
            .accuracy_history
            .iter()
            .filter(|r| r.actual.is_some())
            .collect();

        if with_feedback.is_empty() {
            return 0.0;
        }

        let diffs: Vec<f32> = with_feedback
            .iter()
            .map(|r| r.predicted - r.actual.unwrap())
            .filter(|v| v.is_finite())
            .collect();

        if diffs.is_empty() {
            return 0.0;
        }

        let sum: f32 = diffs.iter().sum();
        sum / diffs.len() as f32
    }

    /// Get accuracy statistics
    pub fn accuracy_stats(&self) -> AnalyzerStats {
        let with_feedback: Vec<_> = self
            .accuracy_history
            .iter()
            .filter(|r| r.actual.is_some())
            .collect();

        if with_feedback.is_empty() {
            return AnalyzerStats::default();
        }

        let total_error: f32 = with_feedback
            .iter()
            .map(|r| (r.predicted - r.actual.unwrap()).abs())
            .sum();

        let avg_error = total_error / with_feedback.len() as f32;

        AnalyzerStats {
            total_analyses: self.analysis_count,
            feedback_count: with_feedback.len(),
            average_error: avg_error,
            accuracy: 1.0 - avg_error,
        }
    }
}

impl Default for TaskComplexityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyzer statistics
#[derive(Debug, Clone, Default)]
pub struct AnalyzerStats {
    /// Total analyses performed
    pub total_analyses: u64,
    /// Feedback records received
    pub feedback_count: usize,
    /// Average prediction error
    pub average_error: f32,
    /// Overall accuracy
    pub accuracy: f32,
}

// ============================================================================
// Model Selector
// ============================================================================

/// Model selection criteria
#[derive(Debug, Clone)]
pub struct SelectionCriteria {
    /// Prefer lower cost
    pub prefer_cost: bool,
    /// Prefer lower latency
    pub prefer_latency: bool,
    /// Minimum quality threshold
    pub min_quality: f32,
    /// Maximum cost per request (USD)
    pub max_cost: Option<f64>,
    /// Maximum acceptable latency (ms)
    pub max_latency: Option<u64>,
}

impl Default for SelectionCriteria {
    fn default() -> Self {
        Self {
            prefer_cost: false,
            prefer_latency: false,
            min_quality: 0.6,
            max_cost: None,
            max_latency: None,
        }
    }
}

/// Routing decision with full context
#[derive(Debug, Clone)]
pub struct ModelRoutingDecision {
    /// Selected model
    pub model: ClaudeModel,
    /// Complexity score
    pub complexity_score: ComplexityScore,
    /// Estimated cost (USD)
    pub estimated_cost: f64,
    /// Estimated latency (ms)
    pub estimated_latency: u64,
    /// Confidence in decision
    pub confidence: f32,
    /// Decision reasoning
    pub reasoning: String,
    /// Alternative models considered
    pub alternatives: Vec<(ClaudeModel, String)>,
}

/// Intelligent model selector
pub struct ModelSelector {
    /// Complexity analyzer
    analyzer: TaskComplexityAnalyzer,
    /// Selection criteria
    criteria: SelectionCriteria,
    /// Selection history
    selection_history: Vec<SelectionRecord>,
    /// Total selections
    total_selections: u64,
}

/// Record of model selection
#[derive(Debug, Clone)]
struct SelectionRecord {
    /// Selected model
    model: ClaudeModel,
    /// Task complexity
    complexity: f32,
    /// Outcome (if known)
    success: Option<bool>,
    /// Timestamp
    timestamp: Instant,
}

impl ModelSelector {
    /// Create new model selector
    pub fn new(criteria: SelectionCriteria) -> Self {
        Self {
            analyzer: TaskComplexityAnalyzer::new(),
            criteria,
            selection_history: Vec::new(),
            total_selections: 0,
        }
    }

    /// Select optimal model for task
    pub fn select_model(&mut self, task: &str) -> ModelRoutingDecision {
        self.total_selections += 1;

        // Analyze task complexity
        let complexity_score = self.analyzer.analyze(task);

        // Get base recommendation
        let base_model = complexity_score.recommended_model();

        // Apply criteria adjustments
        let model = self.apply_criteria(&complexity_score, base_model);

        // Estimate cost and latency
        let estimated_tokens = complexity_score.factors.token_estimate;
        let estimated_cost = self.estimate_cost(model, estimated_tokens);
        let estimated_latency = self.estimate_latency(model, estimated_tokens);

        // Generate alternatives
        let alternatives = self.generate_alternatives(model, &complexity_score);

        // Record selection
        self.selection_history.push(SelectionRecord {
            model,
            complexity: complexity_score.overall,
            success: None,
            timestamp: Instant::now(),
        });

        // Trim history
        if self.selection_history.len() > 1000 {
            self.selection_history.remove(0);
        }

        ModelRoutingDecision {
            model,
            complexity_score: complexity_score.clone(),
            estimated_cost,
            estimated_latency,
            confidence: complexity_score.confidence,
            reasoning: complexity_score.reasoning.clone(),
            alternatives,
        }
    }

    /// Apply selection criteria to adjust model choice
    fn apply_criteria(&self, score: &ComplexityScore, base_model: ClaudeModel) -> ClaudeModel {
        let mut model = base_model;

        // Check cost constraints
        if let Some(max_cost) = self.criteria.max_cost {
            let estimated_cost = self.estimate_cost(model, score.factors.token_estimate);
            if estimated_cost > max_cost {
                // Downgrade model
                model = match model {
                    ClaudeModel::Opus => ClaudeModel::Sonnet,
                    ClaudeModel::Sonnet => ClaudeModel::Haiku,
                    ClaudeModel::Haiku => ClaudeModel::Haiku,
                };
            }
        }

        // Check latency constraints
        if let Some(max_latency) = self.criteria.max_latency {
            let estimated_latency = self.estimate_latency(model, score.factors.token_estimate);
            if estimated_latency > max_latency {
                // Downgrade model for speed
                model = match model {
                    ClaudeModel::Opus => ClaudeModel::Sonnet,
                    ClaudeModel::Sonnet => ClaudeModel::Haiku,
                    ClaudeModel::Haiku => ClaudeModel::Haiku,
                };
            }
        }

        // Prefer cost if set
        if self.criteria.prefer_cost && score.overall < 0.5 {
            model = match model {
                ClaudeModel::Opus => ClaudeModel::Sonnet,
                ClaudeModel::Sonnet if score.is_simple() => ClaudeModel::Haiku,
                _ => model,
            };
        }

        // Prefer latency if set
        if self.criteria.prefer_latency && score.overall < 0.6 {
            model = match model {
                ClaudeModel::Opus => ClaudeModel::Sonnet,
                ClaudeModel::Sonnet if score.is_simple() => ClaudeModel::Haiku,
                _ => model,
            };
        }

        // Quality floor - don't downgrade too much for complex tasks
        if score.requires_opus() && model != ClaudeModel::Opus {
            model = ClaudeModel::Opus;
        }

        model
    }

    /// Estimate cost for model and token count
    #[inline]
    fn estimate_cost(&self, model: ClaudeModel, token_estimate: usize) -> f64 {
        // Assume output is similar to input for estimation
        let input_tokens = token_estimate as f64;
        let output_tokens = input_tokens * 1.5;

        // Pre-compute divisor to avoid multiple divisions
        let input_cost = (input_tokens * model.input_cost_per_1k()) / 1000.0;
        let output_cost = (output_tokens * model.output_cost_per_1k()) / 1000.0;

        input_cost + output_cost
    }

    /// Estimate latency for model and token count
    #[inline]
    fn estimate_latency(&self, model: ClaudeModel, token_estimate: usize) -> u64 {
        let base_ttft = model.typical_ttft_ms();

        // Estimate generation time (tokens per second varies by model)
        let tokens_per_second = match model {
            ClaudeModel::Haiku => 200.0,
            ClaudeModel::Sonnet => 100.0,
            ClaudeModel::Opus => 50.0,
        };

        let generation_time = (token_estimate as f64 / tokens_per_second * 1000.0) as u64;

        base_ttft + generation_time
    }

    /// Generate alternative model recommendations
    fn generate_alternatives(
        &self,
        selected: ClaudeModel,
        score: &ComplexityScore,
    ) -> Vec<(ClaudeModel, String)> {
        let mut alternatives = Vec::new();

        match selected {
            ClaudeModel::Haiku => {
                alternatives.push((
                    ClaudeModel::Sonnet,
                    "For better quality if needed".to_string(),
                ));
            }
            ClaudeModel::Sonnet => {
                if score.is_simple() {
                    alternatives.push((
                        ClaudeModel::Haiku,
                        "For cost savings on simple task".to_string(),
                    ));
                }
                if score.factors.reasoning_depth > 0.5 {
                    alternatives.push((
                        ClaudeModel::Opus,
                        "For deeper reasoning if quality insufficient".to_string(),
                    ));
                }
            }
            ClaudeModel::Opus => {
                if !score.requires_opus() {
                    alternatives.push((
                        ClaudeModel::Sonnet,
                        "May suffice for cost savings".to_string(),
                    ));
                }
            }
        }

        alternatives
    }

    /// Record outcome for learning
    pub fn record_outcome(&mut self, success: bool) {
        if let Some(record) = self.selection_history.last_mut() {
            record.success = Some(success);
        }
    }

    /// Get selector statistics
    pub fn stats(&self) -> SelectorStats {
        let with_outcome: Vec<_> = self
            .selection_history
            .iter()
            .filter(|r| r.success.is_some())
            .collect();

        let success_count = with_outcome
            .iter()
            .filter(|r| r.success == Some(true))
            .count();

        let success_rate = if !with_outcome.is_empty() {
            success_count as f32 / with_outcome.len() as f32
        } else {
            0.0
        };

        // Count by model
        let mut by_model: HashMap<ClaudeModel, usize> = HashMap::new();
        for record in &self.selection_history {
            *by_model.entry(record.model).or_insert(0) += 1;
        }

        SelectorStats {
            total_selections: self.total_selections,
            feedback_count: with_outcome.len(),
            success_rate,
            selections_by_model: by_model,
            analyzer_stats: self.analyzer.accuracy_stats(),
        }
    }

    /// Update selection criteria
    pub fn set_criteria(&mut self, criteria: SelectionCriteria) {
        self.criteria = criteria;
    }

    /// Get current criteria
    pub fn criteria(&self) -> &SelectionCriteria {
        &self.criteria
    }
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self::new(SelectionCriteria::default())
    }
}

/// Selector statistics
#[derive(Debug, Clone)]
pub struct SelectorStats {
    /// Total selections made
    pub total_selections: u64,
    /// Feedback records received
    pub feedback_count: usize,
    /// Success rate
    pub success_rate: f32,
    /// Selections by model
    pub selections_by_model: HashMap<ClaudeModel, usize>,
    /// Analyzer statistics
    pub analyzer_stats: AnalyzerStats,
}

// ============================================================================
// Integrated Router
// ============================================================================

/// Complete model routing system
pub struct ModelRouter {
    /// Model selector
    selector: ModelSelector,
    /// Agent type to model mapping overrides
    agent_overrides: HashMap<AgentType, ClaudeModel>,
    /// Task type to model mapping overrides
    task_overrides: HashMap<ClaudeFlowTask, ClaudeModel>,
}

impl ModelRouter {
    /// Create new model router
    pub fn new() -> Self {
        Self {
            selector: ModelSelector::default(),
            agent_overrides: Self::default_agent_overrides(),
            task_overrides: Self::default_task_overrides(),
        }
    }

    /// Create with custom criteria
    pub fn with_criteria(criteria: SelectionCriteria) -> Self {
        Self {
            selector: ModelSelector::new(criteria),
            agent_overrides: Self::default_agent_overrides(),
            task_overrides: Self::default_task_overrides(),
        }
    }

    /// Default agent type overrides
    fn default_agent_overrides() -> HashMap<AgentType, ClaudeModel> {
        let mut map = HashMap::new();
        // Security tasks always get Opus
        map.insert(AgentType::Security, ClaudeModel::Opus);
        // Simple reviewing can use Haiku
        map.insert(AgentType::Reviewer, ClaudeModel::Sonnet);
        map
    }

    /// Default task type overrides
    fn default_task_overrides() -> HashMap<ClaudeFlowTask, ClaudeModel> {
        let mut map = HashMap::new();
        // Architecture always needs deep reasoning
        map.insert(ClaudeFlowTask::Architecture, ClaudeModel::Opus);
        // Security tasks need careful analysis
        map.insert(ClaudeFlowTask::Security, ClaudeModel::Opus);
        // Documentation can be simpler
        map.insert(ClaudeFlowTask::Documentation, ClaudeModel::Haiku);
        map
    }

    /// Route task to optimal model
    pub fn route(
        &mut self,
        task: &str,
        agent_type: Option<AgentType>,
        task_type: Option<ClaudeFlowTask>,
    ) -> ModelRoutingDecision {
        // Check for overrides first
        if let Some(agent) = agent_type {
            if let Some(&model) = self.agent_overrides.get(&agent) {
                let mut decision = self.selector.select_model(task);
                decision.model = model;
                decision.reasoning =
                    format!("Agent type {:?} override: {}", agent, decision.reasoning);
                return decision;
            }
        }

        if let Some(task_t) = task_type {
            if let Some(&model) = self.task_overrides.get(&task_t) {
                let mut decision = self.selector.select_model(task);
                decision.model = model;
                decision.reasoning =
                    format!("Task type {:?} override: {}", task_t, decision.reasoning);
                return decision;
            }
        }

        // Standard routing
        self.selector.select_model(task)
    }

    /// Set agent type override
    pub fn set_agent_override(&mut self, agent: AgentType, model: ClaudeModel) {
        self.agent_overrides.insert(agent, model);
    }

    /// Remove agent type override
    pub fn remove_agent_override(&mut self, agent: AgentType) {
        self.agent_overrides.remove(&agent);
    }

    /// Set task type override
    pub fn set_task_override(&mut self, task: ClaudeFlowTask, model: ClaudeModel) {
        self.task_overrides.insert(task, model);
    }

    /// Remove task type override
    pub fn remove_task_override(&mut self, task: ClaudeFlowTask) {
        self.task_overrides.remove(&task);
    }

    /// Record routing outcome
    pub fn record_outcome(&mut self, success: bool) {
        self.selector.record_outcome(success);
    }

    /// Get routing statistics
    pub fn stats(&self) -> SelectorStats {
        self.selector.stats()
    }

    /// Update selection criteria
    pub fn set_criteria(&mut self, criteria: SelectionCriteria) {
        self.selector.set_criteria(criteria);
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_analyzer_simple_task() {
        let mut analyzer = TaskComplexityAnalyzer::new();
        let score = analyzer.analyze("fix typo in readme");

        assert!(score.overall < 0.5);
        assert!(score.is_simple());
        assert_eq!(score.recommended_tier, 1); // Haiku
    }

    #[test]
    fn test_complexity_analyzer_complex_task() {
        let mut analyzer = TaskComplexityAnalyzer::new();
        let score = analyzer.analyze(
            "Design and implement a distributed authentication system with OAuth2, JWT tokens, \
             and comprehensive security audit for vulnerabilities",
        );

        assert!(score.overall > 0.7);
        assert!(score.requires_opus());
        assert_eq!(score.recommended_tier, 3); // Opus
    }

    #[test]
    fn test_complexity_analyzer_moderate_task() {
        let mut analyzer = TaskComplexityAnalyzer::new();
        let score = analyzer
            .analyze("Implement a REST API endpoint for user registration with input validation");

        assert!(score.overall >= 0.35);
        assert!(score.overall < 0.7);
        assert_eq!(score.recommended_tier, 2); // Sonnet
    }

    #[test]
    fn test_model_selector() {
        let mut selector = ModelSelector::default();

        // Simple task
        let decision = selector.select_model("rename variable x to count");
        assert_eq!(decision.model, ClaudeModel::Haiku);

        // Complex task
        let decision = selector.select_model(
            "Design microservices architecture with distributed tracing and security audit",
        );
        assert_eq!(decision.model, ClaudeModel::Opus);
    }

    #[test]
    fn test_model_selector_cost_preference() {
        let criteria = SelectionCriteria {
            prefer_cost: true,
            ..Default::default()
        };
        let mut selector = ModelSelector::new(criteria);

        let decision = selector.select_model("write a simple unit test");
        assert_eq!(decision.model, ClaudeModel::Haiku);
    }

    #[test]
    fn test_model_router_overrides() {
        let mut router = ModelRouter::new();

        // Security agent should always get Opus
        let decision = router.route("fix a bug", Some(AgentType::Security), None);
        assert_eq!(decision.model, ClaudeModel::Opus);

        // Architecture task should get Opus
        let decision = router.route("update config", None, Some(ClaudeFlowTask::Architecture));
        assert_eq!(decision.model, ClaudeModel::Opus);
    }

    #[test]
    fn test_complexity_factors_weighted_score() {
        let factors = ComplexityFactors {
            token_estimate: 2500,
            reasoning_depth: 0.8,
            domain_expertise: 0.5,
            code_complexity: 0.6,
            planning_complexity: 0.7,
            security_sensitivity: 0.9,
            performance_criticality: 0.3,
        };

        let score = factors.weighted_score();
        assert!(score > 0.5); // Should be high given these factors
        assert!(score <= 1.0);
    }

    #[test]
    fn test_cost_estimation() {
        let selector = ModelSelector::default();

        let haiku_cost = selector.estimate_cost(ClaudeModel::Haiku, 1000);
        let sonnet_cost = selector.estimate_cost(ClaudeModel::Sonnet, 1000);
        let opus_cost = selector.estimate_cost(ClaudeModel::Opus, 1000);

        assert!(haiku_cost < sonnet_cost);
        assert!(sonnet_cost < opus_cost);
    }

    #[test]
    fn test_latency_estimation() {
        let selector = ModelSelector::default();

        let haiku_latency = selector.estimate_latency(ClaudeModel::Haiku, 500);
        let sonnet_latency = selector.estimate_latency(ClaudeModel::Sonnet, 500);
        let opus_latency = selector.estimate_latency(ClaudeModel::Opus, 500);

        assert!(haiku_latency < sonnet_latency);
        assert!(sonnet_latency < opus_latency);
    }
}
