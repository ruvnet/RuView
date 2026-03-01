//! Quality Scoring Engine
//!
//! Main engine for computing multi-dimensional quality scores,
//! tracking quality over time, and providing recommendations.

use super::coherence::{CoherenceConfig, CoherenceValidator};
use super::diversity::{DiversityAnalyzer, DiversityConfig};
use super::metrics::{QualityDimension, QualityMetrics, QualityWeights, TrendDirection};
use super::validators::{JsonSchemaValidator, SchemaValidator};
use crate::error::{Result, RuvLLMError};
use crate::serving::GenerationResult;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Configuration for the scoring engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// Quality weights for composite scoring
    pub weights: QualityWeights,
    /// Coherence validation config
    pub coherence: CoherenceConfig,
    /// Diversity analysis config
    pub diversity: DiversityConfig,
    /// Maximum history size for trend analysis
    pub max_history_size: usize,
    /// Minimum samples needed for trend analysis
    pub min_samples_for_trend: usize,
    /// Threshold for quality alerts
    pub alert_threshold: f32,
    /// Enable automatic recommendations
    pub auto_recommendations: bool,
    /// Trend window size (number of recent samples)
    pub trend_window: usize,
    /// Significance threshold for trend detection
    pub trend_significance: f32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            weights: QualityWeights::default(),
            coherence: CoherenceConfig::default(),
            diversity: DiversityConfig::default(),
            max_history_size: 1000,
            min_samples_for_trend: 10,
            alert_threshold: 0.5,
            auto_recommendations: true,
            trend_window: 50,
            trend_significance: 0.05,
        }
    }
}

/// Context for scoring operations
#[derive(Debug, Clone, Default)]
pub struct ScoringContext {
    /// Optional JSON schema for validation
    pub schema: Option<JsonValue>,
    /// Embeddings for semantic analysis
    pub embeddings: Option<Vec<f32>>,
    /// Reference texts for comparison
    pub reference_texts: Vec<String>,
    /// Time-series data (if applicable)
    pub time_series: Option<Vec<f64>>,
    /// Previous generations for uniqueness checking
    pub previous_generations: Vec<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

/// Quality history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityHistory {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Quality metrics
    pub metrics: QualityMetrics,
    /// Generation ID if available
    pub generation_id: Option<String>,
    /// Context summary
    pub context_summary: Option<String>,
}

/// Result of comparing two generations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Delta for each dimension
    pub dimension_deltas: HashMap<QualityDimension, f32>,
    /// Overall quality delta (positive = improvement)
    pub overall_delta: f32,
    /// Which generation is better (true = first, false = second)
    pub first_is_better: bool,
    /// Detailed comparison notes
    pub notes: Vec<String>,
    /// Statistical significance
    pub is_significant: bool,
}

/// Trend analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// Overall trend direction
    pub direction: TrendDirection,
    /// Trend slope (change per sample)
    pub slope: f32,
    /// Average quality in trend window
    pub average: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// Per-dimension trends
    pub dimension_trends: HashMap<QualityDimension, TrendDirection>,
    /// Predicted next value
    pub predicted_next: f32,
    /// Confidence in trend (0.0-1.0)
    pub confidence: f32,
}

/// Improvement recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementRecommendation {
    /// Target dimension
    pub dimension: QualityDimension,
    /// Priority (1-5, higher = more urgent)
    pub priority: u8,
    /// Recommendation message
    pub message: String,
    /// Specific actions to take
    pub actions: Vec<String>,
    /// Expected improvement range
    pub expected_improvement: (f32, f32),
}

/// Main quality scoring engine
pub struct QualityScoringEngine {
    /// Configuration
    config: ScoringConfig,
    /// Coherence validator
    coherence_validator: CoherenceValidator,
    /// Diversity analyzer
    diversity_analyzer: DiversityAnalyzer,
    /// Quality history
    history: Arc<RwLock<VecDeque<QualityHistory>>>,
    /// Schema validators cache
    schema_cache: Arc<RwLock<HashMap<String, Box<dyn SchemaValidator>>>>,
    /// Generation fingerprints for uniqueness
    fingerprints: Arc<RwLock<HashMap<String, u64>>>,
}

impl QualityScoringEngine {
    /// Create a new scoring engine with default configuration
    pub fn new() -> Self {
        Self::with_config(ScoringConfig::default())
    }

    /// Create a scoring engine with custom configuration
    pub fn with_config(config: ScoringConfig) -> Self {
        Self {
            coherence_validator: CoherenceValidator::new(config.coherence.clone()),
            diversity_analyzer: DiversityAnalyzer::new(config.diversity.clone()),
            config,
            history: Arc::new(RwLock::new(VecDeque::new())),
            schema_cache: Arc::new(RwLock::new(HashMap::new())),
            fingerprints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a scoring engine with custom weights
    pub fn with_weights(weights: QualityWeights) -> Self {
        Self::with_config(ScoringConfig {
            weights,
            ..Default::default()
        })
    }

    /// Score a generation result
    pub fn score_generation(
        &self,
        result: &GenerationResult,
        context: &ScoringContext,
    ) -> Result<QualityMetrics> {
        let mut metrics = QualityMetrics::new();
        metrics.generation_id = Some(format!("{}", result.request_id));

        // 1. Schema compliance
        metrics.schema_compliance = self.score_schema_compliance(result, context)?;

        // 2. Semantic coherence
        metrics.semantic_coherence = self.score_semantic_coherence(result, context)?;

        // 3. Diversity
        metrics.diversity = self.score_diversity(result, context)?;

        // 4. Temporal realism (if time-series context)
        metrics.temporal_realism = self.score_temporal_realism(result, context)?;

        // 5. Uniqueness
        metrics.uniqueness = self.score_uniqueness(result, context)?;

        // Compute composite score
        metrics.compute_composite(&self.config.weights);

        Ok(metrics)
    }

    /// Score a text directly (without GenerationResult)
    pub fn score_text(&self, text: &str, context: &ScoringContext) -> Result<QualityMetrics> {
        let mut metrics = QualityMetrics::new();

        // Create a minimal GenerationResult-like context
        let segments = split_into_segments(text);

        // 1. Schema compliance (if schema provided)
        if let Some(ref schema) = context.schema {
            if let Ok(json) = serde_json::from_str::<JsonValue>(text) {
                let validator = JsonSchemaValidator::new(schema.clone());
                let result = validator.validate(&json);
                metrics.schema_compliance = result.compliance_score;
            } else {
                metrics.schema_compliance = 0.0; // Not valid JSON
            }
        } else {
            metrics.schema_compliance = 1.0; // No schema to validate against
        }

        // 2. Semantic coherence
        if segments.len() > 1 {
            let coherence_result = self
                .coherence_validator
                .validate_semantic_consistency(&segments, None)?;
            metrics.semantic_coherence = coherence_result.consistency_score;
        } else {
            metrics.semantic_coherence = 1.0;
        }

        // 3. Diversity (compared to previous generations)
        if !context.previous_generations.is_empty() {
            let mut all_samples: Vec<String> = context.previous_generations.clone();
            all_samples.push(text.to_string());
            let diversity_result = self
                .diversity_analyzer
                .calculate_diversity(&all_samples, None);
            metrics.diversity = diversity_result.diversity_score;
        } else {
            metrics.diversity = 1.0;
        }

        // 4. Temporal realism
        metrics.temporal_realism = if context.time_series.is_some() {
            self.score_temporal_from_text(text, context)?
        } else {
            1.0
        };

        // 5. Uniqueness
        metrics.uniqueness = self.calculate_uniqueness(text, &context.previous_generations);

        // Compute composite
        metrics.compute_composite(&self.config.weights);

        Ok(metrics)
    }

    /// Generate improvement recommendations based on metrics
    pub fn improvement_recommendations(
        &self,
        metrics: &QualityMetrics,
    ) -> Vec<ImprovementRecommendation> {
        let mut recommendations = Vec::new();

        // Check each dimension
        let threshold = self.config.alert_threshold;

        if metrics.schema_compliance < threshold {
            recommendations.push(ImprovementRecommendation {
                dimension: QualityDimension::SchemaCompliance,
                priority: 5,
                message: "Schema compliance is low - outputs may not match expected format"
                    .to_string(),
                actions: vec![
                    "Review schema definition for clarity".to_string(),
                    "Add more specific field constraints".to_string(),
                    "Consider using few-shot examples in prompts".to_string(),
                ],
                expected_improvement: (0.1, 0.3),
            });
        }

        if metrics.semantic_coherence < threshold {
            recommendations.push(ImprovementRecommendation {
                dimension: QualityDimension::SemanticCoherence,
                priority: 4,
                message: "Semantic coherence is low - content may have logical inconsistencies"
                    .to_string(),
                actions: vec![
                    "Add explicit context in prompts".to_string(),
                    "Use chain-of-thought prompting".to_string(),
                    "Break complex requests into smaller steps".to_string(),
                ],
                expected_improvement: (0.1, 0.25),
            });
        }

        if metrics.diversity < threshold {
            recommendations.push(ImprovementRecommendation {
                dimension: QualityDimension::Diversity,
                priority: 3,
                message: "Diversity is low - outputs may be repetitive".to_string(),
                actions: vec![
                    "Increase temperature parameter".to_string(),
                    "Use diverse beam search".to_string(),
                    "Add variation to prompts".to_string(),
                ],
                expected_improvement: (0.15, 0.35),
            });
        }

        if metrics.temporal_realism < threshold {
            recommendations.push(ImprovementRecommendation {
                dimension: QualityDimension::TemporalRealism,
                priority: 2,
                message: "Temporal realism is low - time-series patterns may be unrealistic"
                    .to_string(),
                actions: vec![
                    "Provide more temporal context".to_string(),
                    "Include historical data in prompts".to_string(),
                    "Use domain-specific constraints".to_string(),
                ],
                expected_improvement: (0.1, 0.2),
            });
        }

        if metrics.uniqueness < threshold {
            recommendations.push(ImprovementRecommendation {
                dimension: QualityDimension::Uniqueness,
                priority: 4,
                message: "Uniqueness is low - many duplicates or near-duplicates".to_string(),
                actions: vec![
                    "Apply repetition penalty".to_string(),
                    "Use different seeds for each generation".to_string(),
                    "Add uniqueness constraints to prompts".to_string(),
                ],
                expected_improvement: (0.2, 0.4),
            });
        }

        // Sort by priority (descending)
        recommendations.sort_by(|a, b| b.priority.cmp(&a.priority));

        recommendations
    }

    /// Track quality metrics over time
    pub fn track_quality_over_time(&self, metrics: QualityMetrics) {
        let mut history = self.history.write();

        // Add new entry
        history.push_back(QualityHistory {
            timestamp: Utc::now(),
            metrics,
            generation_id: None,
            context_summary: None,
        });

        // Trim if over max size
        while history.len() > self.config.max_history_size {
            history.pop_front();
        }
    }

    /// Compare two generations
    pub fn compare_generations(
        &self,
        first: &QualityMetrics,
        second: &QualityMetrics,
    ) -> ComparisonResult {
        let mut dimension_deltas = HashMap::new();
        let mut notes = Vec::new();

        // Calculate deltas for each dimension
        for dim in QualityDimension::all() {
            let first_score = first.get_dimension_score(*dim);
            let second_score = second.get_dimension_score(*dim);
            let delta = first_score - second_score;
            dimension_deltas.insert(*dim, delta);

            if delta.abs() > 0.1 {
                let direction = if delta > 0.0 { "higher" } else { "lower" };
                notes.push(format!(
                    "{}: first is {} by {:.1}%",
                    dim,
                    direction,
                    delta.abs() * 100.0
                ));
            }
        }

        let overall_delta = first.composite_score - second.composite_score;
        let first_is_better = overall_delta > 0.0;
        let is_significant = overall_delta.abs() > self.config.trend_significance;

        ComparisonResult {
            dimension_deltas,
            overall_delta,
            first_is_better,
            notes,
            is_significant,
        }
    }

    /// Get quality trends from history
    pub fn get_quality_trends(&self, window: Option<usize>) -> Option<TrendAnalysis> {
        let history = self.history.read();
        let window_size = window.unwrap_or(self.config.trend_window);

        if history.len() < self.config.min_samples_for_trend {
            return None;
        }

        // Get recent samples
        let recent: Vec<&QualityHistory> = history
            .iter()
            .rev()
            .take(window_size)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        // Calculate composite score statistics
        let scores: Vec<f32> = recent.iter().map(|h| h.metrics.composite_score).collect();
        let average = scores.iter().sum::<f32>() / scores.len() as f32;
        let std_dev = calculate_std_dev(&scores, average);

        // Calculate slope using linear regression
        let (slope, confidence) = calculate_slope(&scores);

        // Determine trend direction
        let direction = if slope > self.config.trend_significance {
            TrendDirection::Improving
        } else if slope < -self.config.trend_significance {
            TrendDirection::Declining
        } else {
            TrendDirection::Stable
        };

        // Calculate per-dimension trends
        let mut dimension_trends = HashMap::new();
        for dim in QualityDimension::all() {
            let dim_scores: Vec<f32> = recent
                .iter()
                .map(|h| h.metrics.get_dimension_score(*dim))
                .collect();
            let (dim_slope, _) = calculate_slope(&dim_scores);

            dimension_trends.insert(
                *dim,
                if dim_slope > self.config.trend_significance {
                    TrendDirection::Improving
                } else if dim_slope < -self.config.trend_significance {
                    TrendDirection::Declining
                } else {
                    TrendDirection::Stable
                },
            );
        }

        // Predict next value
        let predicted_next = (average + slope).clamp(0.0, 1.0);

        Some(TrendAnalysis {
            direction,
            slope,
            average,
            std_dev,
            dimension_trends,
            predicted_next,
            confidence,
        })
    }

    /// Visualize quality trends as ASCII chart
    pub fn visualize_trends(&self, width: usize) -> String {
        let history = self.history.read();

        if history.is_empty() {
            return "No quality history available.".to_string();
        }

        let mut output = String::new();
        output.push_str("Quality Score Trend\n");
        output.push_str(&"=".repeat(width));
        output.push('\n');

        // Get recent scores
        let scores: Vec<f32> = history
            .iter()
            .rev()
            .take(width)
            .map(|h| h.metrics.composite_score)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        // Draw chart (10 rows)
        let chart_height = 10;
        for row in (0..chart_height).rev() {
            let threshold = (row as f32 + 0.5) / chart_height as f32;
            let label = format!("{:>4.0}%|", threshold * 100.0);
            output.push_str(&label);

            for score in &scores {
                if *score >= threshold {
                    output.push('#');
                } else {
                    output.push(' ');
                }
            }
            output.push('\n');
        }

        output.push_str("     +");
        output.push_str(&"-".repeat(scores.len()));
        output.push('\n');

        // Add statistics
        if !scores.is_empty() {
            let avg = scores.iter().sum::<f32>() / scores.len() as f32;
            let min = scores.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            output.push_str(&format!(
                "Avg: {:.1}%  Min: {:.1}%  Max: {:.1}%  Samples: {}\n",
                avg * 100.0,
                min * 100.0,
                max * 100.0,
                scores.len()
            ));
        }

        output
    }

    /// Export metrics history as JSON
    pub fn export_metrics_json(&self) -> Result<String> {
        let history = self.history.read();
        let entries: Vec<&QualityHistory> = history.iter().collect();
        serde_json::to_string_pretty(&entries)
            .map_err(|e| RuvLLMError::Serialization(e.to_string()))
    }

    /// Clear history
    pub fn clear_history(&self) {
        let mut history = self.history.write();
        history.clear();
    }

    /// Get current configuration
    pub fn config(&self) -> &ScoringConfig {
        &self.config
    }

    // Private scoring methods

    fn score_schema_compliance(
        &self,
        result: &GenerationResult,
        context: &ScoringContext,
    ) -> Result<f32> {
        // If no schema, return perfect compliance
        let schema = match &context.schema {
            Some(s) => s,
            None => return Ok(1.0),
        };

        // Try to parse generated text as JSON
        let text = result.generated_text.as_deref().unwrap_or("");
        let json = match serde_json::from_str::<JsonValue>(text) {
            Ok(j) => j,
            Err(_) => return Ok(0.0), // Not valid JSON
        };

        // Validate against schema
        let validator = JsonSchemaValidator::new(schema.clone());
        let validation_result = validator.validate(&json);

        Ok(validation_result.compliance_score)
    }

    fn score_semantic_coherence(
        &self,
        result: &GenerationResult,
        context: &ScoringContext,
    ) -> Result<f32> {
        let text = result.generated_text.as_deref().unwrap_or("");
        if text.is_empty() {
            return Ok(1.0);
        }

        // Split into segments
        let segments = split_into_segments(text);
        if segments.len() < 2 {
            return Ok(1.0);
        }

        // Get embeddings if available
        let embeddings = context.embeddings.as_ref().map(|e| vec![e.clone()]);

        // Validate consistency
        let coherence_result = self
            .coherence_validator
            .validate_semantic_consistency(&segments, embeddings.as_deref())?;

        // Check for contradictions
        let contradiction_result = self
            .coherence_validator
            .detect_contradictions(&segments, None)?;

        // Check logical flow
        let flow_result = self
            .coherence_validator
            .check_logical_flow(&segments, None)?;

        // Combine scores
        let combined = coherence_result.consistency_score * 0.4
            + (1.0 - contradiction_result.contradiction_score) * 0.3
            + flow_result.flow_score * 0.3;

        Ok(combined)
    }

    fn score_diversity(&self, result: &GenerationResult, context: &ScoringContext) -> Result<f32> {
        let text = result.generated_text.as_deref().unwrap_or("");
        if text.is_empty() {
            return Ok(1.0);
        }

        // Combine with previous generations for diversity check
        let mut all_samples: Vec<String> = context.previous_generations.clone();
        all_samples.push(text.to_string());

        if all_samples.len() < 2 {
            return Ok(1.0);
        }

        let diversity_result = self
            .diversity_analyzer
            .calculate_diversity(&all_samples, None);

        Ok(diversity_result.diversity_score)
    }

    fn score_temporal_realism(
        &self,
        result: &GenerationResult,
        context: &ScoringContext,
    ) -> Result<f32> {
        // If no time-series context, return neutral score
        let _time_series = match &context.time_series {
            Some(ts) if !ts.is_empty() => ts,
            _ => return Ok(1.0),
        };

        let text = result.generated_text.as_deref().unwrap_or("");
        self.score_temporal_from_text(text, context)
    }

    fn score_temporal_from_text(&self, text: &str, context: &ScoringContext) -> Result<f32> {
        let time_series = match &context.time_series {
            Some(ts) if !ts.is_empty() => ts,
            _ => return Ok(1.0),
        };

        // Extract numbers from generated text
        let generated_values = extract_numbers_from_text(text);
        if generated_values.is_empty() {
            return Ok(0.5); // No numbers to evaluate
        }

        // Check if generated values are within reasonable range of time-series
        let ts_min = time_series.iter().cloned().fold(f64::INFINITY, f64::min);
        let ts_max = time_series
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let ts_range = ts_max - ts_min;

        // Allow some extrapolation (20% beyond range)
        let allowed_min = ts_min - ts_range * 0.2;
        let allowed_max = ts_max + ts_range * 0.2;

        let in_range_count = generated_values
            .iter()
            .filter(|v| **v >= allowed_min && **v <= allowed_max)
            .count();

        let range_score = in_range_count as f32 / generated_values.len() as f32;

        // Check for trend consistency
        let ts_trend = if time_series.len() >= 2 {
            (time_series.last().unwrap() - time_series.first().unwrap()).signum()
        } else {
            0.0
        };

        let gen_trend = if generated_values.len() >= 2 {
            (generated_values.last().unwrap() - generated_values.first().unwrap()).signum()
        } else {
            0.0
        };

        let trend_score = if ts_trend == gen_trend { 1.0 } else { 0.5 };

        Ok(range_score * 0.6 + trend_score * 0.4)
    }

    fn score_uniqueness(&self, result: &GenerationResult, context: &ScoringContext) -> Result<f32> {
        let text = result.generated_text.as_deref().unwrap_or("");
        Ok(self.calculate_uniqueness(text, &context.previous_generations))
    }

    fn calculate_uniqueness(&self, text: &str, previous: &[String]) -> f32 {
        if previous.is_empty() {
            return 1.0;
        }

        // Calculate fingerprint
        let fingerprint = calculate_fingerprint(text);

        // Check against stored fingerprints
        let fingerprints = self.fingerprints.read();
        for prev in previous {
            let prev_fp = fingerprints
                .get(prev)
                .copied()
                .unwrap_or_else(|| calculate_fingerprint(prev));

            if fingerprint == prev_fp {
                return 0.0; // Exact duplicate
            }
        }

        // Calculate similarity-based uniqueness
        let mut max_similarity = 0.0f32;
        for prev in previous {
            let sim = jaccard_similarity(text, prev);
            max_similarity = max_similarity.max(sim);
        }

        // Convert similarity to uniqueness (inverse)
        1.0 - max_similarity
    }
}

impl Default for QualityScoringEngine {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

fn split_into_segments(text: &str) -> Vec<String> {
    // Split on sentence boundaries
    text.split(|c| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn calculate_std_dev(values: &[f32], mean: f32) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }

    let variance: f32 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (values.len() - 1) as f32;

    variance.sqrt()
}

fn calculate_slope(values: &[f32]) -> (f32, f32) {
    if values.len() < 2 {
        return (0.0, 0.0);
    }

    let n = values.len() as f32;

    // Linear regression
    let sum_x: f32 = (0..values.len()).map(|i| i as f32).sum();
    let sum_y: f32 = values.iter().sum();
    let sum_xy: f32 = values.iter().enumerate().map(|(i, y)| i as f32 * y).sum();
    let sum_x2: f32 = (0..values.len()).map(|i| (i as f32).powi(2)).sum();

    let denominator = n * sum_x2 - sum_x * sum_x;
    if denominator.abs() < f32::EPSILON {
        return (0.0, 0.0);
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denominator;

    // Calculate R-squared for confidence
    let mean_y = sum_y / n;
    let ss_tot: f32 = values.iter().map(|y| (y - mean_y).powi(2)).sum();
    let ss_res: f32 = values
        .iter()
        .enumerate()
        .map(|(i, y)| {
            let predicted = (i as f32 * slope) + (sum_y - slope * sum_x) / n;
            (y - predicted).powi(2)
        })
        .sum();

    let r_squared = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };

    (slope, r_squared.max(0.0))
}

fn extract_numbers_from_text(text: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        if c.is_numeric() || c == '.' || (c == '-' && current.is_empty()) {
            current.push(c);
        } else if !current.is_empty() {
            if let Ok(num) = current.parse::<f64>() {
                numbers.push(num);
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(num) = current.parse::<f64>() {
            numbers.push(num);
        }
    }

    numbers
}

fn calculate_fingerprint(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.to_lowercase().hash(&mut hasher);
    hasher.finish()
}

fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let words_a: std::collections::HashSet<&str> = a_lower.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b_lower.split_whitespace().collect();

    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serving::{FinishReason, RequestId};
    use uuid::Uuid;

    fn create_test_result(text: &str) -> GenerationResult {
        GenerationResult {
            request_id: RequestId(Uuid::nil()),
            generated_tokens: vec![1, 2, 3],
            generated_text: Some(text.to_string()),
            finish_reason: FinishReason::EndOfSequence,
            processing_time_ms: 100,
            tokens_per_second: 30.0,
            prompt_tokens: 10,
            completion_tokens: 3,
        }
    }

    #[test]
    fn test_scoring_engine_creation() {
        let engine = QualityScoringEngine::new();
        assert!(engine.config.alert_threshold > 0.0);
    }

    #[test]
    fn test_score_generation() {
        let engine = QualityScoringEngine::new();
        let result = create_test_result(
            "This is a test generation. It has multiple sentences. The content is coherent.",
        );
        let context = ScoringContext::default();

        let metrics = engine.score_generation(&result, &context).unwrap();
        assert!(metrics.composite_score >= 0.0);
        assert!(metrics.composite_score <= 1.0);
    }

    #[test]
    fn test_score_text() {
        let engine = QualityScoringEngine::new();
        let text = "The quick brown fox jumps over the lazy dog.";
        let context = ScoringContext::default();

        let metrics = engine.score_text(text, &context).unwrap();
        assert!(metrics.composite_score >= 0.0);
    }

    #[test]
    fn test_schema_compliance() {
        let engine = QualityScoringEngine::new();
        let result = create_test_result(r#"{"name": "test", "value": 42}"#);

        let context = ScoringContext {
            schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "value": { "type": "integer" }
                },
                "required": ["name", "value"]
            })),
            ..Default::default()
        };

        let metrics = engine.score_generation(&result, &context).unwrap();
        assert!(metrics.schema_compliance > 0.5);
    }

    #[test]
    fn test_improvement_recommendations() {
        let engine = QualityScoringEngine::new();
        let metrics = QualityMetrics::with_scores(0.3, 0.3, 0.3, 0.3, 0.3);

        let recommendations = engine.improvement_recommendations(&metrics);
        assert!(!recommendations.is_empty());
    }

    #[test]
    fn test_track_quality_over_time() {
        let engine = QualityScoringEngine::new();

        for i in 0..20 {
            let metrics = QualityMetrics::with_scores(
                0.5 + (i as f32 * 0.02),
                0.5 + (i as f32 * 0.02),
                0.5,
                0.5,
                0.5,
            );
            engine.track_quality_over_time(metrics);
        }

        let history = engine.history.read();
        assert_eq!(history.len(), 20);
    }

    #[test]
    fn test_get_quality_trends() {
        let engine = QualityScoringEngine::with_config(ScoringConfig {
            min_samples_for_trend: 5,
            ..Default::default()
        });

        // Add improving trend
        for i in 0..15 {
            let score = 0.5 + (i as f32 * 0.02);
            let metrics = QualityMetrics::with_scores(score, score, score, score, score);
            engine.track_quality_over_time(metrics);
        }

        let trends = engine.get_quality_trends(None);
        assert!(trends.is_some());

        let analysis = trends.unwrap();
        assert!(analysis.slope > 0.0);
    }

    #[test]
    fn test_compare_generations() {
        let engine = QualityScoringEngine::new();

        let first = QualityMetrics::with_scores(0.8, 0.8, 0.8, 0.8, 0.8);
        let second = QualityMetrics::with_scores(0.6, 0.6, 0.6, 0.6, 0.6);

        let comparison = engine.compare_generations(&first, &second);
        assert!(comparison.first_is_better);
        assert!(comparison.overall_delta > 0.0);
    }

    #[test]
    fn test_visualize_trends() {
        let engine = QualityScoringEngine::new();

        for i in 0..10 {
            let metrics = QualityMetrics::with_scores(0.5 + (i as f32 * 0.05), 0.5, 0.5, 0.5, 0.5);
            engine.track_quality_over_time(metrics);
        }

        let viz = engine.visualize_trends(40);
        assert!(viz.contains("Quality Score Trend"));
    }

    #[test]
    fn test_uniqueness_calculation() {
        let engine = QualityScoringEngine::new();

        // Exact duplicate
        let uniqueness = engine.calculate_uniqueness("Hello world", &["Hello world".to_string()]);
        assert!(uniqueness < 0.1);

        // Completely different
        let uniqueness = engine.calculate_uniqueness(
            "The quick brown fox",
            &["Completely different text here".to_string()],
        );
        assert!(uniqueness > 0.5);
    }

    #[test]
    fn test_export_metrics_json() {
        let engine = QualityScoringEngine::new();
        let metrics = QualityMetrics::with_scores(0.8, 0.8, 0.8, 0.8, 0.8);
        engine.track_quality_over_time(metrics);

        let json = engine.export_metrics_json().unwrap();
        assert!(json.contains("composite_score"));
    }

    #[test]
    fn test_split_into_segments() {
        let segments = split_into_segments("First sentence. Second sentence! Third sentence?");
        assert_eq!(segments.len(), 3);
    }

    #[test]
    fn test_extract_numbers() {
        let numbers = extract_numbers_from_text("The values are 42, -3.14, and 100.");
        assert_eq!(numbers.len(), 3);
        assert!((numbers[0] - 42.0).abs() < 0.001);
        assert!((numbers[1] - (-3.14)).abs() < 0.001);
        assert!((numbers[2] - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity() {
        let sim = jaccard_similarity("hello world", "hello world");
        assert!((sim - 1.0).abs() < 0.001);

        let sim = jaccard_similarity("hello world", "goodbye moon");
        assert!(sim < 0.5);
    }
}
