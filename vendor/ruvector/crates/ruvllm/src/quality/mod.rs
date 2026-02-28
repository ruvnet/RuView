//! Multi-dimensional Quality Scoring Framework for RuvLLM
//!
//! This module provides a comprehensive quality scoring system for evaluating
//! LLM-generated content across multiple dimensions including schema compliance,
//! semantic coherence, diversity, temporal realism, and uniqueness.
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | GenerationResult  |---->| QualityScoringEngine |
//! |                   |     |                   |
//! +-------------------+     | - score_generation|
//!                           | - track_quality   |
//!                           | - recommendations |
//!                           +--------+----------+
//!                                    |
//!          +-------------------------+-------------------------+
//!          |                         |                         |
//!          v                         v                         v
//! +--------+--------+     +----------+---------+     +---------+--------+
//! | CoherenceValidator   | | DiversityAnalyzer    | | SchemaValidator  |
//! | - semantic_check    | | - diversity_calc     | | - json_validate  |
//! | - contradiction_detect| | - mode_collapse_detect| | - type_check   |
//! +---------------------+ +---------------------+ +------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::quality::{
//!     QualityScoringEngine, QualityMetrics, QualityWeights,
//!     CoherenceValidator, DiversityAnalyzer, JsonSchemaValidator,
//! };
//!
//! // Create scoring engine with custom weights
//! let weights = QualityWeights {
//!     schema_compliance: 0.25,
//!     semantic_coherence: 0.25,
//!     diversity: 0.20,
//!     temporal_realism: 0.15,
//!     uniqueness: 0.15,
//! };
//! let engine = QualityScoringEngine::with_weights(weights);
//!
//! // Score a generation result
//! let metrics = engine.score_generation(&generation_result)?;
//! println!("Composite score: {:.2}", metrics.composite_score);
//!
//! // Get improvement recommendations
//! let recommendations = engine.improvement_recommendations(&metrics);
//! for rec in recommendations {
//!     println!("Recommendation: {}", rec);
//! }
//!
//! // Track quality over time
//! engine.track_quality_over_time(&metrics);
//! let trends = engine.get_quality_trends(100);
//! ```
//!
//! ## Quality Dimensions
//!
//! | Dimension | Description | Range |
//! |-----------|-------------|-------|
//! | Schema Compliance | Validates structure against JSON schema | 0.0 - 1.0 |
//! | Semantic Coherence | Logical consistency and flow | 0.0 - 1.0 |
//! | Diversity | Variation in generated content | 0.0 - 1.0 |
//! | Temporal Realism | Time-series validity (if applicable) | 0.0 - 1.0 |
//! | Uniqueness | Non-duplicate content detection | 0.0 - 1.0 |
//!
//! ## Visualization
//!
//! The module provides helpers for visualizing quality trends:
//!
//! ```rust,ignore
//! // Get ASCII visualization
//! let viz = engine.visualize_trends(50);
//! println!("{}", viz);
//!
//! // Export metrics for external visualization
//! let json = engine.export_metrics_json()?;
//! ```

pub mod coherence;
pub mod diversity;
pub mod metrics;
pub mod scoring_engine;
pub mod validators;

// Re-exports
pub use coherence::{
    CoherenceConfig, CoherenceValidator, CoherenceViolation, ContradictionResult,
    LogicalFlowResult, SemanticConsistencyResult,
};
pub use diversity::{
    DiversificationSuggestion, DiversityAnalyzer, DiversityConfig, DiversityResult,
    ModeCollapseResult,
};
pub use metrics::{
    QualityDimension, QualityMetrics, QualitySummary, QualityWeights, TrendDirection,
};
pub use scoring_engine::{
    ComparisonResult, ImprovementRecommendation, QualityHistory, QualityScoringEngine,
    ScoringConfig, ScoringContext, TrendAnalysis,
};
pub use validators::{
    CombinedValidator, FormatValidator, JsonSchemaValidator, RangeValidator, SchemaValidator,
    TypeValidator, ValidationCombinator, ValidationError, ValidationResult,
};
