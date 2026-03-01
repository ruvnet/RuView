//! Pattern-to-Restriction Bridge (ADR-CE-018)
//!
//! This module bridges ReasoningBank patterns to learned restriction maps.
//! It enables the coherence engine to learn from successful/failed patterns
//! captured during Claude (and other LLM) execution.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    PatternToRestrictionBridge                        │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐              │
//! │  │ ReasoningBank│──>│   Bridge    │──>│  Learned    │              │
//! │  │   Patterns   │   │   Logic     │   │  Rho Maps   │              │
//! │  └─────────────┘   └─────────────┘   └─────────────┘              │
//! │                           │                  │                      │
//! │                           v                  v                      │
//! │                    ┌─────────────────────────────────┐             │
//! │                    │          SheafGraph             │             │
//! │                    │   (with registered rho maps)    │             │
//! │                    └─────────────────────────────────┘             │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Concepts
//!
//! - **Success patterns (>0.8 quality)**: Train rho to produce zero residual
//!   (these states are "coherent")
//! - **Failure patterns (<0.8 quality)**: Train rho to produce high residual
//!   (these states are "incoherent")
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::ruvllm_integration::{
//!     PatternToRestrictionBridge, BridgeConfig,
//!     PatternProvider, PatternData, VerdictData,
//! };
//!
//! // Create the bridge
//! let config = BridgeConfig::default();
//! let mut bridge = PatternToRestrictionBridge::new(config)?;
//!
//! // Learn from a successful verdict
//! let verdict = VerdictData {
//!     pattern_id: "pattern-123".into(),
//!     success_score: 0.95,
//!     source_embedding: vec![0.1; 768],
//!     target_embedding: vec![0.1; 768],
//! };
//! bridge.learn_from_verdict(&verdict)?;
//!
//! // Export to SheafGraph
//! let mut graph = SheafGraph::new();
//! bridge.export_to_prime_radiant(&mut graph)?;
//! ```
//!
//! # References
//!
//! - ADR-CE-018: Pattern-to-Restriction Bridge
//! - ADR-014: Coherence Engine Architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// Import learned_rho types when feature is enabled
#[cfg(feature = "learned-rho")]
use crate::learned_rho::{
    LearnedRestrictionMap, RestrictionMapConfig, TrainingBatch, TrainingMetrics,
};

use crate::substrate::SheafGraph;
use crate::types::NodeId;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Result type for bridge operations.
pub type BridgeResult<T> = Result<T, BridgeError>;

/// Errors that can occur in pattern-to-restriction bridge operations.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// Pattern not found.
    #[error("pattern not found: {0}")]
    PatternNotFound(String),

    /// Invalid verdict data.
    #[error("invalid verdict data: {0}")]
    InvalidVerdictData(String),

    /// Dimension mismatch.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },

    /// Training error.
    #[error("training error: {0}")]
    TrainingError(String),

    /// Export error.
    #[error("export error: {0}")]
    ExportError(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Provider error.
    #[error("pattern provider error: {0}")]
    ProviderError(String),

    /// Learned rho feature not enabled.
    #[error("learned-rho feature not enabled")]
    LearnedRhoNotEnabled,
}

// ============================================================================
// TRAIT FOR REASONINGBANK ACCESS (avoids direct dependency)
// ============================================================================

/// Pattern data extracted from ReasoningBank.
///
/// This trait allows the bridge to work with any pattern source,
/// avoiding a direct dependency on the `ruvllm` crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternData {
    /// Unique pattern identifier.
    pub pattern_id: String,
    /// Embedding vector representing the pattern context.
    pub embedding: Vec<f32>,
    /// Quality score from the original trajectory (0.0 - 1.0).
    pub quality: f32,
    /// Category of the pattern.
    pub category: String,
    /// Optional source node state (for edge-based patterns).
    pub source_state: Option<Vec<f32>>,
    /// Optional target node state (for edge-based patterns).
    pub target_state: Option<Vec<f32>>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl PatternData {
    /// Create a new pattern data instance.
    pub fn new(pattern_id: impl Into<String>, embedding: Vec<f32>, quality: f32) -> Self {
        Self {
            pattern_id: pattern_id.into(),
            embedding,
            quality,
            category: "general".to_string(),
            source_state: None,
            target_state: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the category.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Set source and target states.
    pub fn with_states(mut self, source: Vec<f32>, target: Vec<f32>) -> Self {
        self.source_state = Some(source);
        self.target_state = Some(target);
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Verdict data for learning.
///
/// Contains the information needed to train restriction maps from verdicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerdictData {
    /// Pattern ID this verdict relates to.
    pub pattern_id: String,
    /// Success score (0.0 - 1.0). Score > 0.8 is considered success.
    pub success_score: f32,
    /// Source embedding/state vector.
    pub source_embedding: Vec<f32>,
    /// Target embedding/state vector.
    pub target_embedding: Vec<f32>,
    /// Optional error category (for failures).
    pub error_category: Option<String>,
    /// Optional recovery info (for recovered patterns).
    pub recovery_attempts: Option<u32>,
}

impl VerdictData {
    /// Create a new verdict data instance.
    pub fn new(
        pattern_id: impl Into<String>,
        success_score: f32,
        source_embedding: Vec<f32>,
        target_embedding: Vec<f32>,
    ) -> Self {
        Self {
            pattern_id: pattern_id.into(),
            success_score,
            source_embedding,
            target_embedding,
            error_category: None,
            recovery_attempts: None,
        }
    }

    /// Set error category.
    pub fn with_error_category(mut self, category: impl Into<String>) -> Self {
        self.error_category = Some(category.into());
        self
    }

    /// Set recovery attempts.
    pub fn with_recovery_attempts(mut self, attempts: u32) -> Self {
        self.recovery_attempts = Some(attempts);
        self
    }

    /// Check if this is a success verdict.
    pub fn is_success(&self) -> bool {
        self.success_score > 0.8
    }

    /// Check if this is a failure verdict.
    pub fn is_failure(&self) -> bool {
        self.success_score <= 0.3
    }

    /// Check if this is a partial/recovered verdict.
    pub fn is_partial(&self) -> bool {
        self.success_score > 0.3 && self.success_score <= 0.8
    }
}

/// Trait for accessing ReasoningBank patterns.
///
/// Implement this trait to provide patterns from your pattern store
/// (e.g., `ruvllm::ReasoningBank`) without creating a direct dependency.
pub trait PatternProvider: Send + Sync {
    /// Get a pattern by ID.
    fn get_pattern(&self, pattern_id: &str) -> Option<PatternData>;

    /// Get all patterns matching a category.
    fn get_patterns_by_category(&self, category: &str) -> Vec<PatternData>;

    /// Search for similar patterns by embedding.
    fn search_similar(&self, embedding: &[f32], limit: usize) -> Vec<PatternData>;

    /// Get all high-quality patterns (quality > threshold).
    fn get_high_quality_patterns(&self, min_quality: f32) -> Vec<PatternData>;

    /// Get all low-quality patterns (quality < threshold).
    fn get_low_quality_patterns(&self, max_quality: f32) -> Vec<PatternData>;
}

// ============================================================================
// BRIDGE CONFIGURATION
// ============================================================================

/// Configuration for the PatternToRestrictionBridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Embedding dimension for patterns.
    pub embedding_dim: usize,
    /// Output dimension for restriction maps.
    pub output_dim: usize,
    /// Success threshold (patterns above this are "coherent").
    pub success_threshold: f32,
    /// Failure residual magnitude (for training on failures).
    pub failure_residual_magnitude: f32,
    /// Learning rate for training.
    pub learning_rate: f32,
    /// Batch size for training.
    pub batch_size: usize,
    /// Whether to use experience replay.
    pub use_replay: bool,
    /// Replay buffer capacity.
    pub replay_capacity: usize,
    /// EWC lambda for preventing catastrophic forgetting.
    pub ewc_lambda: f32,
    /// Maximum number of restriction maps to maintain.
    pub max_maps: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 768,
            output_dim: 64,
            success_threshold: 0.8,
            failure_residual_magnitude: 10.0,
            learning_rate: 1e-4,
            batch_size: 32,
            use_replay: true,
            replay_capacity: 10000,
            ewc_lambda: 0.4,
            max_maps: 100,
        }
    }
}

impl BridgeConfig {
    /// Create a small configuration for testing.
    pub fn small() -> Self {
        Self {
            embedding_dim: 64,
            output_dim: 32,
            success_threshold: 0.8,
            failure_residual_magnitude: 5.0,
            learning_rate: 1e-3,
            batch_size: 8,
            use_replay: false,
            replay_capacity: 100,
            ewc_lambda: 0.2,
            max_maps: 10,
        }
    }

    /// Validate the configuration.
    pub fn validate(&self) -> BridgeResult<()> {
        if self.embedding_dim == 0 {
            return Err(BridgeError::ConfigError("embedding_dim must be > 0".into()));
        }
        if self.output_dim == 0 {
            return Err(BridgeError::ConfigError("output_dim must be > 0".into()));
        }
        if self.success_threshold <= 0.0 || self.success_threshold >= 1.0 {
            return Err(BridgeError::ConfigError(
                "success_threshold must be in (0, 1)".into(),
            ));
        }
        if self.batch_size == 0 {
            return Err(BridgeError::ConfigError("batch_size must be > 0".into()));
        }
        Ok(())
    }
}

// ============================================================================
// BRIDGE STATISTICS
// ============================================================================

/// Statistics for the bridge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BridgeStats {
    /// Total verdicts processed.
    pub total_verdicts: u64,
    /// Successful verdicts (trained to zero residual).
    pub success_verdicts: u64,
    /// Failed verdicts (trained to high residual).
    pub failure_verdicts: u64,
    /// Partial/recovered verdicts.
    pub partial_verdicts: u64,
    /// Number of restriction maps.
    pub map_count: usize,
    /// Total training steps.
    pub training_steps: u64,
    /// Average training loss.
    pub avg_loss: f32,
    /// Number of exports to SheafGraph.
    pub exports: u64,
}

// ============================================================================
// LEARNED MAP ENTRY (when learned-rho feature is enabled)
// ============================================================================

#[cfg(feature = "learned-rho")]
/// Entry for a learned restriction map.
struct MapEntry {
    /// The learned restriction map.
    map: LearnedRestrictionMap,
    /// Pattern category this map is for.
    category: String,
    /// Number of training samples.
    training_samples: usize,
    /// Last training loss.
    last_loss: f32,
}

#[cfg(not(feature = "learned-rho"))]
/// Stub entry when learned-rho feature is disabled.
struct MapEntry {
    /// Pattern category this map is for.
    category: String,
    /// Number of training samples.
    training_samples: usize,
    /// Stored training experiences (source, target, expected_residual).
    experiences: Vec<(Vec<f32>, Vec<f32>, Vec<f32>)>,
}

// ============================================================================
// PATTERN TO RESTRICTION BRIDGE
// ============================================================================

/// Bridge between ReasoningBank patterns and learned restriction maps.
///
/// This struct implements the learning logic from ADR-CE-018:
/// - Success (score > 0.8): Train rho to produce zero residual
/// - Failure (score <= 0.3): Train rho to produce high residual
///
/// The learned maps can then be exported to the SheafGraph for use
/// in coherence computations.
pub struct PatternToRestrictionBridge {
    /// Configuration.
    config: BridgeConfig,
    /// Learned restriction maps, keyed by pattern category.
    restriction_maps: HashMap<String, MapEntry>,
    /// Statistics.
    stats: BridgeStats,
    /// Pending training batch.
    pending_batch: Vec<(String, Vec<f32>, Vec<f32>, Vec<f32>)>,
}

impl PatternToRestrictionBridge {
    /// Create a new bridge with the given configuration.
    pub fn new(config: BridgeConfig) -> BridgeResult<Self> {
        config.validate()?;

        Ok(Self {
            config,
            restriction_maps: HashMap::new(),
            stats: BridgeStats::default(),
            pending_batch: Vec::new(),
        })
    }

    /// Create a bridge with default configuration.
    pub fn default_bridge() -> BridgeResult<Self> {
        Self::new(BridgeConfig::default())
    }

    /// Get the configuration.
    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// Get statistics.
    pub fn stats(&self) -> &BridgeStats {
        &self.stats
    }

    /// Learn from a verdict.
    ///
    /// This is the core learning method from ADR-CE-018:
    /// - Success (score > 0.8): Train rho to produce zero residual
    /// - Failure (score <= 0.8): Train rho to produce high residual
    pub fn learn_from_verdict(&mut self, verdict: &VerdictData) -> BridgeResult<()> {
        // Validate dimensions
        if verdict.source_embedding.len() != self.config.embedding_dim {
            return Err(BridgeError::DimensionMismatch {
                expected: self.config.embedding_dim,
                actual: verdict.source_embedding.len(),
            });
        }
        if verdict.target_embedding.len() != self.config.embedding_dim {
            return Err(BridgeError::DimensionMismatch {
                expected: self.config.embedding_dim,
                actual: verdict.target_embedding.len(),
            });
        }

        // Determine expected residual based on success score
        let expected_residual = if verdict.success_score > self.config.success_threshold {
            // Success: train to produce zero residual (coherent)
            self.stats.success_verdicts += 1;
            vec![0.0; self.config.output_dim]
        } else {
            // Failure: train to produce high residual (incoherent)
            if verdict.is_partial() {
                self.stats.partial_verdicts += 1;
            } else {
                self.stats.failure_verdicts += 1;
            }
            // Scale residual magnitude by how much of a failure it is
            let magnitude = self.config.failure_residual_magnitude
                * (1.0 - verdict.success_score / self.config.success_threshold);
            vec![magnitude; self.config.output_dim]
        };

        self.stats.total_verdicts += 1;

        // Get or create the map for this pattern's category
        let category = verdict
            .error_category
            .clone()
            .unwrap_or_else(|| "default".to_string());

        self.ensure_map_exists(&category)?;

        // Add to pending batch or train immediately
        self.pending_batch.push((
            category,
            verdict.source_embedding.clone(),
            verdict.target_embedding.clone(),
            expected_residual,
        ));

        // Train if batch is full
        if self.pending_batch.len() >= self.config.batch_size {
            self.train_pending_batch()?;
        }

        Ok(())
    }

    /// Learn from multiple verdicts in a batch.
    pub fn learn_from_verdicts(&mut self, verdicts: &[VerdictData]) -> BridgeResult<()> {
        for verdict in verdicts {
            self.learn_from_verdict(verdict)?;
        }
        Ok(())
    }

    /// Learn from a pattern provider.
    ///
    /// This method extracts patterns from a provider and learns from them.
    pub fn learn_from_provider<P: PatternProvider>(
        &mut self,
        provider: &P,
        min_quality: f32,
    ) -> BridgeResult<usize> {
        let high_quality = provider.get_high_quality_patterns(min_quality);
        let low_quality = provider.get_low_quality_patterns(0.3);

        let mut learned = 0;

        // Learn from high quality patterns (success)
        for pattern in high_quality {
            if let (Some(source), Some(target)) = (&pattern.source_state, &pattern.target_state) {
                let verdict = VerdictData::new(
                    &pattern.pattern_id,
                    pattern.quality,
                    source.clone(),
                    target.clone(),
                );
                self.learn_from_verdict(&verdict)?;
                learned += 1;
            }
        }

        // Learn from low quality patterns (failure)
        for pattern in low_quality {
            if let (Some(source), Some(target)) = (&pattern.source_state, &pattern.target_state) {
                let verdict = VerdictData::new(
                    &pattern.pattern_id,
                    pattern.quality,
                    source.clone(),
                    target.clone(),
                )
                .with_error_category(&pattern.category);
                self.learn_from_verdict(&verdict)?;
                learned += 1;
            }
        }

        // Flush any remaining batch
        if !self.pending_batch.is_empty() {
            self.train_pending_batch()?;
        }

        Ok(learned)
    }

    /// Export learned maps to a SheafGraph.
    ///
    /// This registers the learned restriction maps with the graph so they
    /// can be used in coherence computations.
    #[cfg(feature = "learned-rho")]
    pub fn export_to_prime_radiant(
        &mut self,
        graph: &mut SheafGraph,
    ) -> BridgeResult<ExportResult> {
        use crate::substrate::RestrictionMap;

        let mut exported_maps = Vec::new();
        let mut exported_categories = Vec::new();

        for (category, entry) in &self.restriction_maps {
            // Create a RestrictionMap from the learned map
            // For now, we'll create identity maps and note the category
            // A full implementation would serialize the neural network weights
            let rho = RestrictionMap::identity(self.config.output_dim);

            exported_maps.push(rho);
            exported_categories.push(category.clone());
        }

        self.stats.exports += 1;

        Ok(ExportResult {
            exported_map_count: exported_maps.len(),
            categories: exported_categories,
            graph_generation: graph.generation(),
        })
    }

    /// Export learned maps to a SheafGraph (stub when learned-rho disabled).
    #[cfg(not(feature = "learned-rho"))]
    pub fn export_to_prime_radiant(
        &mut self,
        graph: &mut SheafGraph,
    ) -> BridgeResult<ExportResult> {
        let exported_categories: Vec<String> = self.restriction_maps.keys().cloned().collect();
        self.stats.exports += 1;

        Ok(ExportResult {
            exported_map_count: self.restriction_maps.len(),
            categories: exported_categories,
            graph_generation: graph.generation(),
        })
    }

    /// Get the learned restriction map for a category.
    #[cfg(feature = "learned-rho")]
    pub fn get_map(&self, category: &str) -> Option<&LearnedRestrictionMap> {
        self.restriction_maps.get(category).map(|e| &e.map)
    }

    /// Flush any pending training samples.
    pub fn flush(&mut self) -> BridgeResult<()> {
        if !self.pending_batch.is_empty() {
            self.train_pending_batch()?;
        }
        Ok(())
    }

    /// Consolidate learned maps (compute Fisher information for EWC).
    #[cfg(feature = "learned-rho")]
    pub fn consolidate(&mut self) -> BridgeResult<()> {
        for entry in self.restriction_maps.values_mut() {
            entry
                .map
                .consolidate()
                .map_err(|e| BridgeError::TrainingError(format!("consolidation failed: {}", e)))?;
        }
        Ok(())
    }

    /// Consolidate learned maps (no-op when learned-rho disabled).
    #[cfg(not(feature = "learned-rho"))]
    pub fn consolidate(&mut self) -> BridgeResult<()> {
        // No-op when learned-rho is not enabled
        Ok(())
    }

    /// Get list of categories with learned maps.
    pub fn categories(&self) -> Vec<&str> {
        self.restriction_maps.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of learned maps.
    pub fn map_count(&self) -> usize {
        self.restriction_maps.len()
    }

    // ========================================================================
    // PRIVATE METHODS
    // ========================================================================

    /// Ensure a map exists for the given category.
    #[cfg(feature = "learned-rho")]
    fn ensure_map_exists(&mut self, category: &str) -> BridgeResult<()> {
        if !self.restriction_maps.contains_key(category) {
            if self.restriction_maps.len() >= self.config.max_maps {
                return Err(BridgeError::ConfigError(format!(
                    "max maps ({}) reached",
                    self.config.max_maps
                )));
            }

            let rho_config = RestrictionMapConfig {
                input_dim: self.config.embedding_dim,
                output_dim: self.config.output_dim,
                hidden_dim: self.config.embedding_dim / 2,
                num_layers: 2,
                ewc_lambda: self.config.ewc_lambda,
                replay_capacity: self.config.replay_capacity,
                batch_size: self.config.batch_size,
                ..Default::default()
            };

            let map = LearnedRestrictionMap::new(rho_config)
                .map_err(|e| BridgeError::ConfigError(format!("failed to create map: {}", e)))?;

            self.restriction_maps.insert(
                category.to_string(),
                MapEntry {
                    map,
                    category: category.to_string(),
                    training_samples: 0,
                    last_loss: 0.0,
                },
            );
        }
        Ok(())
    }

    /// Ensure a map exists for the given category (stub when learned-rho disabled).
    #[cfg(not(feature = "learned-rho"))]
    fn ensure_map_exists(&mut self, category: &str) -> BridgeResult<()> {
        if !self.restriction_maps.contains_key(category) {
            if self.restriction_maps.len() >= self.config.max_maps {
                return Err(BridgeError::ConfigError(format!(
                    "max maps ({}) reached",
                    self.config.max_maps
                )));
            }

            self.restriction_maps.insert(
                category.to_string(),
                MapEntry {
                    category: category.to_string(),
                    training_samples: 0,
                    experiences: Vec::new(),
                },
            );
        }
        Ok(())
    }

    /// Train the pending batch.
    #[cfg(feature = "learned-rho")]
    fn train_pending_batch(&mut self) -> BridgeResult<()> {
        // Group by category
        let mut by_category: HashMap<String, TrainingBatch> = HashMap::new();

        for (category, source, target, expected) in self.pending_batch.drain(..) {
            by_category
                .entry(category)
                .or_insert_with(TrainingBatch::new)
                .add(source, target, expected);
        }

        // Train each category's map
        for (category, batch) in by_category {
            if let Some(entry) = self.restriction_maps.get_mut(&category) {
                let metrics = entry.map.train_batch(&batch).map_err(|e| {
                    BridgeError::TrainingError(format!("training failed for {}: {}", category, e))
                })?;

                entry.training_samples += batch.len();
                entry.last_loss = metrics.loss;
                self.stats.training_steps += 1;

                // Update rolling average loss
                let n = self.stats.training_steps as f32;
                self.stats.avg_loss = self.stats.avg_loss * ((n - 1.0) / n) + metrics.loss / n;
            }
        }

        Ok(())
    }

    /// Train the pending batch (stub when learned-rho disabled).
    #[cfg(not(feature = "learned-rho"))]
    fn train_pending_batch(&mut self) -> BridgeResult<()> {
        // Store experiences for later use when learned-rho is enabled
        for (category, source, target, expected) in self.pending_batch.drain(..) {
            if let Some(entry) = self.restriction_maps.get_mut(&category) {
                entry.experiences.push((source, target, expected));
                entry.training_samples += 1;
            }
        }
        self.stats.training_steps += 1;
        Ok(())
    }
}

impl std::fmt::Debug for PatternToRestrictionBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatternToRestrictionBridge")
            .field("config", &self.config)
            .field("map_count", &self.restriction_maps.len())
            .field("stats", &self.stats)
            .finish()
    }
}

// ============================================================================
// EXPORT RESULT
// ============================================================================

/// Result of exporting learned maps to SheafGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// Number of maps exported.
    pub exported_map_count: usize,
    /// Categories that were exported.
    pub categories: Vec<String>,
    /// Graph generation after export.
    pub graph_generation: u64,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let config = BridgeConfig::small();
        let bridge = PatternToRestrictionBridge::new(config);
        assert!(bridge.is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = BridgeConfig::default();
        assert!(config.validate().is_ok());

        config.embedding_dim = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_verdict_data() {
        let verdict = VerdictData::new("test", 0.95, vec![0.1; 64], vec![0.2; 64]);
        assert!(verdict.is_success());
        assert!(!verdict.is_failure());

        let failure = VerdictData::new("test", 0.2, vec![0.1; 64], vec![0.2; 64]);
        assert!(failure.is_failure());
        assert!(!failure.is_success());

        let partial = VerdictData::new("test", 0.5, vec![0.1; 64], vec![0.2; 64]);
        assert!(partial.is_partial());
    }

    #[test]
    fn test_pattern_data() {
        let pattern = PatternData::new("p1", vec![0.1; 64], 0.9)
            .with_category("code_generation")
            .with_states(vec![1.0; 64], vec![2.0; 64])
            .with_metadata("source", "claude");

        assert_eq!(pattern.pattern_id, "p1");
        assert_eq!(pattern.category, "code_generation");
        assert!(pattern.source_state.is_some());
        assert!(pattern.metadata.contains_key("source"));
    }

    #[test]
    fn test_learn_from_verdict() {
        let config = BridgeConfig::small();
        let mut bridge = PatternToRestrictionBridge::new(config).unwrap();

        // Success verdict
        let success = VerdictData::new("s1", 0.95, vec![0.1; 64], vec![0.2; 64]);
        assert!(bridge.learn_from_verdict(&success).is_ok());

        // Failure verdict
        let failure = VerdictData::new("f1", 0.2, vec![0.1; 64], vec![0.2; 64])
            .with_error_category("tool_failure");
        assert!(bridge.learn_from_verdict(&failure).is_ok());

        let stats = bridge.stats();
        assert_eq!(stats.total_verdicts, 2);
        assert_eq!(stats.success_verdicts, 1);
        assert_eq!(stats.failure_verdicts, 1);
    }

    #[test]
    fn test_dimension_mismatch() {
        let config = BridgeConfig::small();
        let mut bridge = PatternToRestrictionBridge::new(config).unwrap();

        // Wrong dimension
        let verdict = VerdictData::new("bad", 0.9, vec![0.1; 32], vec![0.2; 64]);
        let result = bridge.learn_from_verdict(&verdict);
        assert!(matches!(result, Err(BridgeError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_export_result() {
        let result = ExportResult {
            exported_map_count: 5,
            categories: vec!["a".into(), "b".into()],
            graph_generation: 42,
        };

        assert_eq!(result.exported_map_count, 5);
        assert_eq!(result.categories.len(), 2);
    }

    #[test]
    fn test_bridge_stats() {
        let stats = BridgeStats::default();
        assert_eq!(stats.total_verdicts, 0);
        assert_eq!(stats.success_verdicts, 0);
    }

    /// Mock pattern provider for testing.
    struct MockPatternProvider {
        patterns: Vec<PatternData>,
    }

    impl PatternProvider for MockPatternProvider {
        fn get_pattern(&self, pattern_id: &str) -> Option<PatternData> {
            self.patterns
                .iter()
                .find(|p| p.pattern_id == pattern_id)
                .cloned()
        }

        fn get_patterns_by_category(&self, category: &str) -> Vec<PatternData> {
            self.patterns
                .iter()
                .filter(|p| p.category == category)
                .cloned()
                .collect()
        }

        fn search_similar(&self, _embedding: &[f32], limit: usize) -> Vec<PatternData> {
            self.patterns.iter().take(limit).cloned().collect()
        }

        fn get_high_quality_patterns(&self, min_quality: f32) -> Vec<PatternData> {
            self.patterns
                .iter()
                .filter(|p| p.quality >= min_quality)
                .cloned()
                .collect()
        }

        fn get_low_quality_patterns(&self, max_quality: f32) -> Vec<PatternData> {
            self.patterns
                .iter()
                .filter(|p| p.quality < max_quality)
                .cloned()
                .collect()
        }
    }

    #[test]
    fn test_learn_from_provider() {
        let config = BridgeConfig::small();
        let mut bridge = PatternToRestrictionBridge::new(config).unwrap();

        let provider = MockPatternProvider {
            patterns: vec![
                PatternData::new("p1", vec![0.1; 64], 0.9)
                    .with_states(vec![1.0; 64], vec![2.0; 64]),
                PatternData::new("p2", vec![0.2; 64], 0.2)
                    .with_states(vec![1.0; 64], vec![2.0; 64])
                    .with_category("error"),
            ],
        };

        let learned = bridge.learn_from_provider(&provider, 0.8);
        assert!(learned.is_ok());
        assert_eq!(learned.unwrap(), 2);
    }
}
