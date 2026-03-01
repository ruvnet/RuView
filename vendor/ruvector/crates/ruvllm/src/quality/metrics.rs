//! Quality Metrics for Generation Evaluation
//!
//! This module defines the core quality metrics structure and weights
//! for multi-dimensional quality assessment.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Quality metrics for a single generation
///
/// Each dimension is scored from 0.0 (worst) to 1.0 (best).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Schema compliance score (0-1)
    /// Measures how well the output conforms to expected schema/structure
    pub schema_compliance: f32,

    /// Semantic coherence score (0-1)
    /// Measures logical consistency and meaningful content flow
    pub semantic_coherence: f32,

    /// Diversity score (0-1)
    /// Measures variation in content, avoiding repetitive patterns
    pub diversity: f32,

    /// Temporal realism score (0-1, for time-series data)
    /// Measures whether temporal patterns are realistic
    pub temporal_realism: f32,

    /// Uniqueness score (0-1)
    /// Measures how unique the content is (not duplicated)
    pub uniqueness: f32,

    /// Composite score (weighted average of all dimensions)
    pub composite_score: f32,

    /// Timestamp when metrics were computed
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,

    /// Generation ID this metric relates to (if applicable)
    pub generation_id: Option<String>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl QualityMetrics {
    /// Create new metrics with all scores set to zero
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// Create metrics with explicit values
    pub fn with_scores(
        schema_compliance: f32,
        semantic_coherence: f32,
        diversity: f32,
        temporal_realism: f32,
        uniqueness: f32,
    ) -> Self {
        let mut metrics = Self {
            schema_compliance: schema_compliance.clamp(0.0, 1.0),
            semantic_coherence: semantic_coherence.clamp(0.0, 1.0),
            diversity: diversity.clamp(0.0, 1.0),
            temporal_realism: temporal_realism.clamp(0.0, 1.0),
            uniqueness: uniqueness.clamp(0.0, 1.0),
            composite_score: 0.0,
            timestamp: Utc::now(),
            generation_id: None,
            metadata: std::collections::HashMap::new(),
        };
        metrics.compute_composite(&QualityWeights::default());
        metrics
    }

    /// Compute composite score using provided weights
    pub fn compute_composite(&mut self, weights: &QualityWeights) {
        // Validate weights sum to approximately 1.0
        let weight_sum = weights.total_weight();

        // Compute weighted average
        let weighted_sum = self.schema_compliance * weights.schema_compliance
            + self.semantic_coherence * weights.semantic_coherence
            + self.diversity * weights.diversity
            + self.temporal_realism * weights.temporal_realism
            + self.uniqueness * weights.uniqueness;

        // Normalize by weight sum to handle weights that don't sum to 1.0
        self.composite_score = if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            0.0
        };
    }

    /// Generate a human-readable summary
    pub fn to_summary(&self) -> QualitySummary {
        QualitySummary {
            overall_grade: self.compute_grade(),
            composite_score: self.composite_score,
            strongest_dimension: self.strongest_dimension(),
            weakest_dimension: self.weakest_dimension(),
            dimensions: vec![
                (QualityDimension::SchemaCompliance, self.schema_compliance),
                (QualityDimension::SemanticCoherence, self.semantic_coherence),
                (QualityDimension::Diversity, self.diversity),
                (QualityDimension::TemporalRealism, self.temporal_realism),
                (QualityDimension::Uniqueness, self.uniqueness),
            ],
            timestamp: self.timestamp,
        }
    }

    /// Compute letter grade from composite score
    fn compute_grade(&self) -> char {
        match self.composite_score {
            s if s >= 0.9 => 'A',
            s if s >= 0.8 => 'B',
            s if s >= 0.7 => 'C',
            s if s >= 0.6 => 'D',
            _ => 'F',
        }
    }

    /// Find the strongest quality dimension
    fn strongest_dimension(&self) -> QualityDimension {
        let scores = [
            (QualityDimension::SchemaCompliance, self.schema_compliance),
            (QualityDimension::SemanticCoherence, self.semantic_coherence),
            (QualityDimension::Diversity, self.diversity),
            (QualityDimension::TemporalRealism, self.temporal_realism),
            (QualityDimension::Uniqueness, self.uniqueness),
        ];

        scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(dim, _)| dim)
            .unwrap_or(QualityDimension::SchemaCompliance)
    }

    /// Find the weakest quality dimension
    fn weakest_dimension(&self) -> QualityDimension {
        let scores = [
            (QualityDimension::SchemaCompliance, self.schema_compliance),
            (QualityDimension::SemanticCoherence, self.semantic_coherence),
            (QualityDimension::Diversity, self.diversity),
            (QualityDimension::TemporalRealism, self.temporal_realism),
            (QualityDimension::Uniqueness, self.uniqueness),
        ];

        scores
            .into_iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(dim, _)| dim)
            .unwrap_or(QualityDimension::SchemaCompliance)
    }

    /// Check if metrics meet a minimum threshold
    pub fn meets_threshold(&self, threshold: f32) -> bool {
        self.composite_score >= threshold
    }

    /// Get the score for a specific dimension
    pub fn get_dimension_score(&self, dimension: QualityDimension) -> f32 {
        match dimension {
            QualityDimension::SchemaCompliance => self.schema_compliance,
            QualityDimension::SemanticCoherence => self.semantic_coherence,
            QualityDimension::Diversity => self.diversity,
            QualityDimension::TemporalRealism => self.temporal_realism,
            QualityDimension::Uniqueness => self.uniqueness,
        }
    }

    /// Set the score for a specific dimension
    pub fn set_dimension_score(&mut self, dimension: QualityDimension, score: f32) {
        let clamped = score.clamp(0.0, 1.0);
        match dimension {
            QualityDimension::SchemaCompliance => self.schema_compliance = clamped,
            QualityDimension::SemanticCoherence => self.semantic_coherence = clamped,
            QualityDimension::Diversity => self.diversity = clamped,
            QualityDimension::TemporalRealism => self.temporal_realism = clamped,
            QualityDimension::Uniqueness => self.uniqueness = clamped,
        }
    }
}

impl fmt::Display for QualityMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Quality[schema={:.2}, coherence={:.2}, diversity={:.2}, temporal={:.2}, unique={:.2}] = {:.2}",
            self.schema_compliance,
            self.semantic_coherence,
            self.diversity,
            self.temporal_realism,
            self.uniqueness,
            self.composite_score
        )
    }
}

/// Quality dimension enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QualityDimension {
    /// Schema compliance dimension
    SchemaCompliance,
    /// Semantic coherence dimension
    SemanticCoherence,
    /// Diversity dimension
    Diversity,
    /// Temporal realism dimension (for time-series)
    TemporalRealism,
    /// Uniqueness dimension
    Uniqueness,
}

impl fmt::Display for QualityDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaCompliance => write!(f, "Schema Compliance"),
            Self::SemanticCoherence => write!(f, "Semantic Coherence"),
            Self::Diversity => write!(f, "Diversity"),
            Self::TemporalRealism => write!(f, "Temporal Realism"),
            Self::Uniqueness => write!(f, "Uniqueness"),
        }
    }
}

impl QualityDimension {
    /// Get all quality dimensions
    pub fn all() -> &'static [QualityDimension] {
        &[
            Self::SchemaCompliance,
            Self::SemanticCoherence,
            Self::Diversity,
            Self::TemporalRealism,
            Self::Uniqueness,
        ]
    }

    /// Get short name for the dimension
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::SchemaCompliance => "schema",
            Self::SemanticCoherence => "coherence",
            Self::Diversity => "diversity",
            Self::TemporalRealism => "temporal",
            Self::Uniqueness => "uniqueness",
        }
    }
}

/// Weights for quality dimension scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityWeights {
    /// Weight for schema compliance (default: 0.20)
    pub schema_compliance: f32,

    /// Weight for semantic coherence (default: 0.25)
    pub semantic_coherence: f32,

    /// Weight for diversity (default: 0.20)
    pub diversity: f32,

    /// Weight for temporal realism (default: 0.15)
    pub temporal_realism: f32,

    /// Weight for uniqueness (default: 0.20)
    pub uniqueness: f32,
}

impl Default for QualityWeights {
    fn default() -> Self {
        Self {
            schema_compliance: 0.20,
            semantic_coherence: 0.25,
            diversity: 0.20,
            temporal_realism: 0.15,
            uniqueness: 0.20,
        }
    }
}

impl QualityWeights {
    /// Create weights optimized for structured data generation
    pub fn for_structured_data() -> Self {
        Self {
            schema_compliance: 0.35,
            semantic_coherence: 0.20,
            diversity: 0.15,
            temporal_realism: 0.10,
            uniqueness: 0.20,
        }
    }

    /// Create weights optimized for creative content
    pub fn for_creative_content() -> Self {
        Self {
            schema_compliance: 0.10,
            semantic_coherence: 0.25,
            diversity: 0.35,
            temporal_realism: 0.05,
            uniqueness: 0.25,
        }
    }

    /// Create weights optimized for time-series data
    pub fn for_time_series() -> Self {
        Self {
            schema_compliance: 0.20,
            semantic_coherence: 0.15,
            diversity: 0.15,
            temporal_realism: 0.35,
            uniqueness: 0.15,
        }
    }

    /// Create weights optimized for deduplication scenarios
    pub fn for_deduplication() -> Self {
        Self {
            schema_compliance: 0.15,
            semantic_coherence: 0.20,
            diversity: 0.20,
            temporal_realism: 0.05,
            uniqueness: 0.40,
        }
    }

    /// Create uniform weights (all equal)
    pub fn uniform() -> Self {
        Self {
            schema_compliance: 0.20,
            semantic_coherence: 0.20,
            diversity: 0.20,
            temporal_realism: 0.20,
            uniqueness: 0.20,
        }
    }

    /// Compute total weight (should sum to ~1.0)
    pub fn total_weight(&self) -> f32 {
        self.schema_compliance
            + self.semantic_coherence
            + self.diversity
            + self.temporal_realism
            + self.uniqueness
    }

    /// Normalize weights to sum to 1.0
    pub fn normalize(&mut self) {
        let total = self.total_weight();
        if total > 0.0 {
            self.schema_compliance /= total;
            self.semantic_coherence /= total;
            self.diversity /= total;
            self.temporal_realism /= total;
            self.uniqueness /= total;
        }
    }

    /// Get weight for a specific dimension
    pub fn get_weight(&self, dimension: QualityDimension) -> f32 {
        match dimension {
            QualityDimension::SchemaCompliance => self.schema_compliance,
            QualityDimension::SemanticCoherence => self.semantic_coherence,
            QualityDimension::Diversity => self.diversity,
            QualityDimension::TemporalRealism => self.temporal_realism,
            QualityDimension::Uniqueness => self.uniqueness,
        }
    }

    /// Set weight for a specific dimension
    pub fn set_weight(&mut self, dimension: QualityDimension, weight: f32) {
        let clamped = weight.clamp(0.0, 1.0);
        match dimension {
            QualityDimension::SchemaCompliance => self.schema_compliance = clamped,
            QualityDimension::SemanticCoherence => self.semantic_coherence = clamped,
            QualityDimension::Diversity => self.diversity = clamped,
            QualityDimension::TemporalRealism => self.temporal_realism = clamped,
            QualityDimension::Uniqueness => self.uniqueness = clamped,
        }
    }
}

/// Human-readable quality summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualitySummary {
    /// Overall letter grade (A-F)
    pub overall_grade: char,

    /// Composite score
    pub composite_score: f32,

    /// Strongest quality dimension
    pub strongest_dimension: QualityDimension,

    /// Weakest quality dimension
    pub weakest_dimension: QualityDimension,

    /// All dimension scores
    pub dimensions: Vec<(QualityDimension, f32)>,

    /// When the summary was generated
    pub timestamp: DateTime<Utc>,
}

impl fmt::Display for QualitySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Quality Summary (Grade: {})", self.overall_grade)?;
        writeln!(f, "  Composite Score: {:.1}%", self.composite_score * 100.0)?;
        writeln!(
            f,
            "  Strongest: {} ({:.1}%)",
            self.strongest_dimension,
            self.dimensions
                .iter()
                .find(|(d, _)| *d == self.strongest_dimension)
                .map(|(_, s)| s * 100.0)
                .unwrap_or(0.0)
        )?;
        writeln!(
            f,
            "  Weakest: {} ({:.1}%)",
            self.weakest_dimension,
            self.dimensions
                .iter()
                .find(|(d, _)| *d == self.weakest_dimension)
                .map(|(_, s)| s * 100.0)
                .unwrap_or(0.0)
        )?;
        writeln!(f, "  Dimensions:")?;
        for (dim, score) in &self.dimensions {
            let bar_len = (score * 20.0) as usize;
            let bar: String = (0..bar_len).map(|_| '#').collect();
            let empty: String = (0..(20 - bar_len)).map(|_| '-').collect();
            writeln!(
                f,
                "    {:<18} [{}{:<20}] {:.1}%",
                dim.to_string(),
                bar,
                empty,
                score * 100.0
            )?;
        }
        Ok(())
    }
}

/// Trend direction for quality tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Quality is improving
    Improving,
    /// Quality is stable
    Stable,
    /// Quality is declining
    Declining,
}

impl fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Improving => write!(f, "Improving"),
            Self::Stable => write!(f, "Stable"),
            Self::Declining => write!(f, "Declining"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_metrics_creation() {
        let metrics = QualityMetrics::with_scores(0.9, 0.8, 0.7, 0.6, 0.5);
        assert!((metrics.schema_compliance - 0.9).abs() < 0.001);
        assert!((metrics.semantic_coherence - 0.8).abs() < 0.001);
        assert!(metrics.composite_score > 0.0);
    }

    #[test]
    fn test_quality_metrics_clamping() {
        let metrics = QualityMetrics::with_scores(1.5, -0.1, 0.5, 0.5, 0.5);
        assert!((metrics.schema_compliance - 1.0).abs() < 0.001);
        assert!((metrics.semantic_coherence - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_composite_score_computation() {
        let mut metrics = QualityMetrics::new();
        metrics.schema_compliance = 1.0;
        metrics.semantic_coherence = 1.0;
        metrics.diversity = 1.0;
        metrics.temporal_realism = 1.0;
        metrics.uniqueness = 1.0;

        metrics.compute_composite(&QualityWeights::default());
        assert!((metrics.composite_score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_quality_weights_normalization() {
        let mut weights = QualityWeights {
            schema_compliance: 1.0,
            semantic_coherence: 1.0,
            diversity: 1.0,
            temporal_realism: 1.0,
            uniqueness: 1.0,
        };
        weights.normalize();
        assert!((weights.total_weight() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_quality_summary() {
        let metrics = QualityMetrics::with_scores(0.95, 0.85, 0.75, 0.65, 0.55);
        let summary = metrics.to_summary();

        assert_eq!(summary.overall_grade, 'B');
        assert_eq!(
            summary.strongest_dimension,
            QualityDimension::SchemaCompliance
        );
        assert_eq!(summary.weakest_dimension, QualityDimension::Uniqueness);
    }

    #[test]
    fn test_grade_computation() {
        let high_quality = QualityMetrics::with_scores(0.95, 0.95, 0.95, 0.95, 0.95);
        assert_eq!(high_quality.to_summary().overall_grade, 'A');

        let low_quality = QualityMetrics::with_scores(0.4, 0.4, 0.4, 0.4, 0.4);
        assert_eq!(low_quality.to_summary().overall_grade, 'F');
    }

    #[test]
    fn test_threshold_check() {
        let metrics = QualityMetrics::with_scores(0.8, 0.8, 0.8, 0.8, 0.8);
        assert!(metrics.meets_threshold(0.7));
        assert!(!metrics.meets_threshold(0.9));
    }

    #[test]
    fn test_dimension_access() {
        let mut metrics = QualityMetrics::new();
        metrics.set_dimension_score(QualityDimension::Diversity, 0.75);
        assert!((metrics.get_dimension_score(QualityDimension::Diversity) - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_preset_weights() {
        let structured = QualityWeights::for_structured_data();
        assert!(structured.schema_compliance > structured.diversity);

        let creative = QualityWeights::for_creative_content();
        assert!(creative.diversity > creative.schema_compliance);

        let time_series = QualityWeights::for_time_series();
        assert!(time_series.temporal_realism > time_series.diversity);
    }

    #[test]
    fn test_metrics_display() {
        let metrics = QualityMetrics::with_scores(0.8, 0.7, 0.6, 0.5, 0.4);
        let display = format!("{}", metrics);
        assert!(display.contains("0.80"));
        assert!(display.contains("0.70"));
    }
}
